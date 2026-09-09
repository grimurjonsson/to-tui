"""Upgrade an existing systemd server from a confirmed GitHub release."""

import hashlib
import json
import os
from pathlib import Path
import platform
import re
import shutil
import stat
import signal
import shlex
import subprocess
import sys
import tarfile
import tempfile

REPOSITORY = "grimurjonsson/to-tui"
BINARY = Path("/usr/local/lib/totui/totui")
UNIT = Path("/etc/systemd/system/totui.service")
SERVICE = "totui.service"


def run(*args, capture=False):
    return subprocess.run(
        [str(arg) for arg in args], check=True, text=True,
        stdout=subprocess.PIPE if capture else None,
    ).stdout


def version_key(value):
    match = re.fullmatch(
        r"v?(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)"
        r"(?:-([0-9A-Za-z.-]+))?(?:\+[0-9A-Za-z.-]+)?", value,
    )
    if not match:
        raise ValueError(f"Unrecognized version: {value}")
    major, minor, patch, prerelease = match.groups()
    identifiers = tuple(
        (0, int(part)) if part.isdigit() else (1, part)
        for part in (prerelease or "").split(".")
    )
    return int(major), int(minor), int(patch), prerelease is None, identifiers


def binary_version(binary):
    return run(binary, "--version", capture=True).strip().split()[-1]


def check_privileges():
    if os.geteuid() == 0:
        return
    if not shutil.which("sudo"):
        raise RuntimeError("sudo is required to upgrade the server as a non-root user.")
    try:
        run("sudo", "-n", "true")
    except subprocess.CalledProcessError as error:
        raise RuntimeError(
            "Sudo access is not available without a password. Use sudo -v to authenticate "
            "first, or ask an administrator to configure passwordless sudo. "
            "This command will not prompt for or bypass your password."
        ) from error


def run_privileged(*args, capture=False):
    prefix = () if os.geteuid() == 0 else ("sudo", "-n")
    return run(*prefix, *args, capture=capture)


def preflight():
    if platform.system() != "Linux":
        raise RuntimeError("Run this command on the Linux VPS hosting totui.")
    check_privileges()
    for command in ("curl", "systemctl", "tar"):
        if not shutil.which(command):
            raise RuntimeError(f"Required command not found: {command}")
    architecture = platform.machine()
    if architecture not in ("x86_64", "aarch64"):
        raise RuntimeError(f"No release binary for architecture {architecture}")
    if not BINARY.is_file() or not UNIT.is_file():
        raise RuntimeError("The managed totui server is not installed. Install it first.")
    if "Description=to-tui personal todo server" not in UNIT.read_text().splitlines():
        raise RuntimeError("The existing service was not created by the totui installer.")
    fragment = run("systemctl", "show", SERVICE, "--property=FragmentPath", "--value", capture=True)
    if fragment.strip() != str(UNIT):
        raise RuntimeError("The loaded service uses a different unit file.")
    executable = run("systemctl", "show", SERVICE, "--property=ExecStart", "--value", capture=True)
    if f"path={BINARY} ;" not in executable:
        raise RuntimeError("The service does not run the managed totui binary.")
    run("systemctl", "is-active", "--quiet", SERVICE)
    run("systemctl", "is-enabled", "--quiet", SERVICE)
    return architecture, binary_version(BINARY)


def latest_release(architecture):
    release = json.loads(run(
        "curl", "--fail", "--silent", "--show-error", "--location",
        "--connect-timeout", "15", "--max-time", "60",
        f"https://api.github.com/repos/{REPOSITORY}/releases/latest", capture=True,
    ))
    if release.get("draft") or release.get("prerelease"):
        raise RuntimeError("GitHub did not return a stable published release.")
    version = release["tag_name"]
    version_key(version)
    name = f"totui-{architecture}-unknown-linux-gnu.tar.gz"
    asset = next((asset for asset in release["assets"] if asset["name"] == name), None)
    if asset is None or asset.get("state") != "uploaded":
        raise RuntimeError(f"Release {version} has no uploaded {name} yet. Try again after the build finishes.")
    digest = asset.get("digest") or ""
    if not re.fullmatch(r"sha256:[0-9a-f]{64}", digest):
        raise RuntimeError("The release asset has no SHA-256 digest to verify.")
    expected = f"https://github.com/{REPOSITORY}/releases/download/{version}/{name}"
    if asset["browser_download_url"] != expected:
        raise RuntimeError("Unexpected release download URL.")
    return version, asset


def download(directory, version, asset):
    archive = directory / "totui.tar.gz"
    run(
        "curl", "--fail", "--show-error", "--location", "--proto", "=https",
        "--proto-redir", "=https", "--connect-timeout", "15", "--max-time", "300",
        "--output", archive, asset["browser_download_url"],
    )
    if hashlib.sha256(archive.read_bytes()).hexdigest() != asset["digest"].removeprefix("sha256:"):
        raise RuntimeError("Download checksum mismatch; the server has not been changed.")
    binary = directory / "totui"
    with tarfile.open(archive, "r:gz") as package:
        member = package.getmember("totui")
        if not member.isfile():
            raise RuntimeError("Release archive does not contain a regular totui executable.")
        with package.extractfile(member) as source, binary.open("wb") as destination:
            shutil.copyfileobj(source, destination)
    binary.chmod(0o755)
    if version_key(binary_version(binary)) != version_key(version):
        raise RuntimeError("Downloaded binary version does not match the release.")
    return binary


def upgrade(binary):
    check_privileges()
    run_privileged("test", "-d", "/var/lib/totui")
    run("systemctl", "is-active", "--quiet", SERVICE)
    backup = run_privileged("mktemp", "-d", "/root/totui-backup.XXXXXXXX", capture=True).strip()
    print(f"Backup directory: {backup}", flush=True)
    print(f"Backups are kept until you delete them manually: sudo rm -rf -- {shlex.quote(backup)}", flush=True)
    run_privileged("cp", "--preserve=mode,timestamps", BINARY, f"{backup}/totui")
    run_privileged("cp", "--preserve=mode,timestamps", UNIT, f"{backup}/totui.service")
    print("Stopping the server to back up all databases and state in /var/lib/totui.", flush=True)
    run_privileged("systemctl", "stop", SERVICE)
    try:
        run_privileged("tar", "--dereference", "--exclude=totui/upgrade-request", "--exclude=totui/upgrade-status.json", "-C", "/var/lib", "-czf", f"{backup}/data.tar.gz", "totui")
    except (Exception, KeyboardInterrupt):
        print("Backup interrupted or failed; restarting the unchanged server.", file=sys.stderr)
        run_privileged("systemctl", "start", SERVICE)
        raise
    print(
        f"Database and state backup saved: {backup}/data.tar.gz "
        "(includes todos.db, per-user databases, users.db, and any SQLite WAL files present).",
        flush=True,
    )
    try:
        run_privileged(binary, "server", "install", "--replace", "--yes")
        run("systemctl", "is-active", "--quiet", SERVICE)
        print(f"Server updated to {binary_version(BINARY)}. Backups retained in: {backup}")
        print(f"To delete this backup manually: sudo rm -rf -- {shlex.quote(backup)}")
    except (Exception, KeyboardInterrupt):
        print(
            f"Upgrade did not complete. Inspect sudo journalctl -u {SERVICE}. "
            f"Backup: {backup}. No automatic rollback was attempted.", file=sys.stderr,
        )
        raise


REQUEST = Path("/var/lib/totui/upgrade-request")
STATUS = Path("/var/lib/totui/upgrade-status.json")


def web_status(state, message):
    with tempfile.NamedTemporaryFile(mode="w", dir=STATUS.parent, delete=False) as output:
        temporary = Path(output.name)
        json.dump({"state": state, "message": message}, output)
    temporary.chmod(0o644)
    temporary.replace(STATUS)


def interrupted(signum, frame):
    raise RuntimeError("Upgrade interrupted; inspect the service and retained backup before retrying")


def web_upgrade():
    try:
        descriptor = os.open(REQUEST, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
        with os.fdopen(descriptor) as source:
            if not stat.S_ISREG(os.fstat(source.fileno()).st_mode):
                raise RuntimeError("Upgrade request must be a regular file")
            requested = source.read(128).strip()
        version_key(requested)
        web_status("running", f"Preparing server upgrade to v{requested}…")
        architecture, installed = preflight()
        version, asset = latest_release(architecture)
        if version_key(version) != version_key(requested):
            raise RuntimeError("The latest release changed. Refresh and confirm the new version.")
        if version_key(version) <= version_key(installed):
            web_status("complete", f"Server v{installed} is already up to date.")
            return
        with tempfile.TemporaryDirectory(prefix="totui-upgrade-") as directory:
            binary = download(Path(directory), version, asset)
            web_status("running", "Backing up server data and installing the update…")
            upgrade(binary)
        web_status("complete", f"Server upgraded to v{version.removeprefix('v')}. Backup retained on the server.")
    except Exception:
        web_status("failed", "Server upgrade failed. The owner can inspect journalctl -u totui-upgrade.service for details and the backup location.")
        raise
    finally:
        REQUEST.unlink(missing_ok=True)


def main():
    architecture, installed = preflight()
    version, asset = latest_release(architecture)
    if version_key(version) <= version_key(installed):
        print(f"No newer release available: {installed} installed, {version.removeprefix('v')} latest.")
        return
    color = sys.stdout.isatty() and "NO_COLOR" not in os.environ
    green, yellow, reset = ("\033[1;32m", "\033[33m", "\033[0m") if color else ("", "", "")
    print(f"New version available: {green}{version.removeprefix('v')}{reset} ({yellow}{installed}{reset} installed).")
    if input("Back up and upgrade the server? [y/N] ").strip().lower() not in ("y", "yes"):
        print("Upgrade cancelled.")
        return
    with tempfile.TemporaryDirectory(prefix="totui-upgrade-") as directory:
        binary = download(Path(directory), version, asset)
        upgrade(binary)


if __name__ == "__main__":
    try:
        if sys.argv[1:] == ["--web"]:
            signal.signal(signal.SIGTERM, interrupted)
            web_upgrade()
        elif sys.argv[1:]:
            raise RuntimeError("Unknown arguments")
        else:
            main()
    except (RuntimeError, ValueError, KeyError, OSError, EOFError, tarfile.TarError, subprocess.CalledProcessError) as error:
        print(f"Upgrade stopped: {error}", file=sys.stderr)
        sys.exit(1)
    except KeyboardInterrupt:
        print("\nUpgrade cancelled.", file=sys.stderr)
        sys.exit(130)

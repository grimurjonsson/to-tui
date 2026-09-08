#!/usr/bin/env python3
"""Manage this checkout's detached development web server (POSIX)."""

import fcntl
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import time


ROOT = Path(__file__).resolve().parent.parent
STATE = Path(os.environ.get("TOTUI_DEV_WEB_DIR", ROOT / "target" / "dev-web"))
LOG = STATE / "server.log"
RECORD = STATE / "process.json"


def identity(pid):
    result = subprocess.run(
        ["ps", "-p", str(pid), "-o", "pgid=", "-o", "lstart=", "-o", "stat="],
        capture_output=True, text=True, check=False,
    )
    parts = result.stdout.split()
    if result.returncode or len(parts) < 7 or parts[-1].startswith("Z"):
        return None
    return " ".join(parts[:-1])


def active():
    if not RECORD.exists():
        return None
    record = json.loads(RECORD.read_text())
    if identity(record["pid"]) == record["identity"]:
        return record
    RECORD.unlink(missing_ok=True)
    return None


def describe(record):
    log = LOG.read_text(errors="replace") if LOG.exists() else ""
    phase = "Running" if "Workspace available at" in log else "Starting / building"
    print(f"{phase} (PID {record['pid']})")
    print(f"Log: {LOG}")
    for line in log.splitlines():
        if "Workspace available at" in line:
            print(line)


def stop(record):
    if not record:
        print("Not running")
        return True
    try:
        os.killpg(record["pid"], signal.SIGTERM)
    except ProcessLookupError:
        pass
    for _ in range(100):
        if identity(record["pid"]) != record["identity"]:
            RECORD.unlink(missing_ok=True)
            print("Development web server stopped")
            return True
        time.sleep(0.05)
    print("Process has not stopped yet; check the log and retry stop", file=sys.stderr)
    return False


def main():
    action, *args = sys.argv[1:]
    if action == "start" and any(arg in ("--help", "-h") for arg in args):
        print("""Usage: just dev-web [OPTIONS]

Options:
  --open         Open the selected project in your browser.
  --verbose      Log mutation payloads (including task text) and operation errors.
  --detach       Run in the background without blocking the terminal.
  --restart      Stop this checkout's detached instance before starting.
  --port PORT    Choose the HTTP port (default: 48372).
  --help, -h     Show this help.

Without --detach, the server runs in the foreground; Ctrl+C stops it.
Detached logs: target/dev-web/server.log (overridden by TOTUI_DEV_WEB_DIR).
  just dev-web-status   Show detached process status and log location.
  just dev-web-stop     Stop this checkout's detached instance.

Example: just dev-web --detach --restart --open --verbose --port 48379""")
        return 0
    detach = "--detach" in args
    restart = "--restart" in args
    args = [arg for arg in args if arg not in ("--detach", "--restart")]
    command = ["cargo", "run", "--bin", "totui", "--", "web", *args]
    if action == "start" and not detach and not restart:
        os.execvp(command[0], command)
    STATE.mkdir(parents=True, exist_ok=True)
    with (STATE / "lock").open("w") as lock:
        fcntl.flock(lock, fcntl.LOCK_EX)
        record = active()
        if action == "status":
            if record:
                describe(record)
            else:
                print(f"Not running. Last log: {LOG}")
            return 0
        if action == "stop":
            return 0 if stop(record) else 1
        if action != "start":
            raise ValueError(f"Unknown action: {action}")
        if restart:
            if not stop(record):
                return 1
            record = None
        if not detach:
            sys.stdout.flush()
            os.execvp(command[0], command)
        if record:
            describe(record)
            print("Already started; use --restart or just dev-web-stop before changing options")
            return 1
        with LOG.open("w") as log:
            process = subprocess.Popen(
                command, stdin=subprocess.DEVNULL, stdout=log, stderr=subprocess.STDOUT,
                start_new_session=True,
            )
        stamp = identity(process.pid)
        if stamp is None:
            print(LOG.read_text(errors="replace"), file=sys.stderr)
            return 1
        record = {"pid": process.pid, "identity": stamp}
        RECORD.write_text(json.dumps(record))
        time.sleep(0.2)
        if process.poll() is not None:
            RECORD.unlink(missing_ok=True)
            print(LOG.read_text(errors="replace"), file=sys.stderr)
            return 1
        describe(record)
        print("Use just dev-web-status or just dev-web-stop")
        return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (OSError, ValueError) as error:
        print(f"dev-web: {error}", file=sys.stderr)
        sys.exit(1)

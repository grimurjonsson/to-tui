"""Test upgrade decisions and failure ordering without touching a real service."""

import contextlib
import hashlib
import importlib.util
import io
from pathlib import Path
import subprocess
import tarfile
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location("upgrade_server", Path(__file__).with_name("upgrade-server.py"))
upgrade = importlib.util.module_from_spec(spec)
spec.loader.exec_module(upgrade)


class UpgradeTests(unittest.TestCase):
    def test_release_order_including_development_versions(self):
        versions = ["0.6.9", "0.6.10", "0.7.0-dev", "0.7.0-rc.2", "0.7.0-rc.10", "0.7.0", "0.7.1", "0.10.0"]
        self.assertEqual(sorted(versions, key=upgrade.version_key), versions)
        self.assertEqual(upgrade.version_key("v0.7.0+build"), upgrade.version_key("0.7.0"))

    def test_equal_or_newer_installation_never_prompts_or_downloads(self):
        for installed in ["0.7.0", "0.8.0"]:
            with patch.object(upgrade, "preflight", return_value=("x86_64", installed)), \
                 patch.object(upgrade, "latest_release", return_value=("v0.7.0", {})), \
                 patch("builtins.input") as prompt, patch.object(upgrade, "download") as download, \
                 contextlib.redirect_stdout(io.StringIO()):
                upgrade.main()
                prompt.assert_not_called()
                download.assert_not_called()

    def test_declining_never_downloads_or_changes_the_service(self):
        with patch.object(upgrade, "preflight", return_value=("x86_64", "0.6.1")), \
             patch.object(upgrade, "latest_release", return_value=("v0.7.0", {})), \
             patch("builtins.input", return_value=""), patch.object(upgrade, "download") as download, \
             patch.object(upgrade, "upgrade") as install, contextlib.redirect_stdout(io.StringIO()):
            upgrade.main()
            download.assert_not_called()
            install.assert_not_called()

    def test_download_failure_does_not_stop_server(self):
        with patch.object(upgrade, "preflight", return_value=("x86_64", "0.6.1")), \
             patch.object(upgrade, "latest_release", return_value=("v0.7.0", {})), \
             patch("builtins.input", return_value="y"), \
             patch.object(upgrade, "download", side_effect=RuntimeError("checksum")), \
             patch.object(upgrade, "upgrade") as install, contextlib.redirect_stdout(io.StringIO()):
            with self.assertRaisesRegex(RuntimeError, "checksum"):
                upgrade.main()
            install.assert_not_called()

    def test_backup_failure_restarts_original_without_installing(self):
        calls = []

        def run(*args, **kwargs):
            calls.append(tuple(map(str, args)))
            if args[:2] == ("sudo", "mktemp"):
                return "/root/totui-backup.test\n"
            if args[:2] == ("sudo", "tar"):
                raise subprocess.CalledProcessError(1, args)

        with patch.object(upgrade, "run", side_effect=run), \
             contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()):
            with self.assertRaises(subprocess.CalledProcessError):
                upgrade.upgrade(Path("/tmp/new-totui"))
        self.assertEqual(calls[-1], ("sudo", "systemctl", "start", "totui.service"))
        self.assertFalse(any("install" in call for call in calls))

    def test_install_happens_only_after_backup(self):
        calls = []

        def run(*args, **kwargs):
            calls.append(tuple(map(str, args)))
            if args[:2] == ("sudo", "mktemp"):
                return "/root/totui-backup.test\n"

        with patch.object(upgrade, "run", side_effect=run), \
             patch.object(upgrade, "binary_version", return_value="0.7.0"), \
             contextlib.redirect_stdout(io.StringIO()) as output:
            upgrade.upgrade(Path("/tmp/new-totui"))
        stop = calls.index(("sudo", "systemctl", "stop", "totui.service"))
        backup = next(i for i, call in enumerate(calls) if call[:2] == ("sudo", "tar"))
        install = calls.index(("sudo", "/tmp/new-totui", "server", "install", "--replace", "--yes"))
        self.assertLess(stop, backup)
        self.assertLess(backup, install)
        self.assertEqual(calls[backup], (
            "sudo", "tar", "--dereference", "-C", "/var/lib", "-czf",
            "/root/totui-backup.test/data.tar.gz", "totui",
        ))
        self.assertIn("Database and state backup saved: /root/totui-backup.test/data.tar.gz", output.getvalue())
        self.assertIn("sudo rm -rf -- /root/totui-backup.test", output.getvalue())
        self.assertFalse(any("rm" in call for call in calls))

    def test_download_verifies_checksum_and_binary_version(self):
        with tempfile.TemporaryDirectory() as directory:
            directory = Path(directory)
            with tarfile.open(directory / "totui.tar.gz", "w:gz") as package:
                data = b"test executable"
                member = tarfile.TarInfo("totui")
                member.size = len(data)
                package.addfile(member, io.BytesIO(data))
            digest = hashlib.sha256((directory / "totui.tar.gz").read_bytes()).hexdigest()
            asset = {"digest": f"sha256:{digest}", "browser_download_url": "https://example.test"}
            with patch.object(upgrade, "run"), patch.object(upgrade, "binary_version", return_value="0.7.0"):
                binary = upgrade.download(directory, "v0.7.0", asset)
                self.assertEqual(binary.read_bytes(), data)
            asset["digest"] = "sha256:" + "0" * 64
            with patch.object(upgrade, "run"), patch.object(upgrade, "binary_version") as version:
                with self.assertRaisesRegex(RuntimeError, "checksum mismatch"):
                    upgrade.download(directory, "v0.7.0", asset)
                version.assert_not_called()

    def test_missing_service_fails_before_querying_github(self):
        with patch.object(upgrade.platform, "system", return_value="Linux"), \
             patch.object(upgrade.platform, "machine", return_value="x86_64"), \
             patch.object(upgrade.shutil, "which", return_value="/bin/tool"), \
             patch.object(upgrade, "BINARY", Path("/does-not-exist/totui")), \
             patch.object(upgrade, "latest_release") as latest:
            with self.assertRaisesRegex(RuntimeError, "not installed"):
                upgrade.main()
            latest.assert_not_called()


if __name__ == "__main__":
    unittest.main()

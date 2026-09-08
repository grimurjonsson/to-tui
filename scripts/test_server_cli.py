"""Verify server preview, readiness, persistence and systemd notification in isolation."""

import os
from pathlib import Path
import shutil
import socket
import sqlite3
import subprocess
import tempfile
import time
import urllib.error
import urllib.request


BINARY = Path(__file__).resolve().parent.parent / "target/debug/totui"


def main():
    with tempfile.TemporaryDirectory(prefix="totui-server-cli-") as directory:
        root = Path(directory)
        env = {
            **os.environ,
            "TOTUI_DATA_DIR": str(root / "data"),
            "TOTUI_BIND": "127.0.0.1",
            "TZ": "UTC",
        }
        env.pop("NOTIFY_SOCKET", None)
        env.pop("TOTUI_WEB_READY_FILE", None)
        preview = subprocess.run(
            [str(BINARY), "server", "wizard", "--dry-run", "--timezone", "UTC"],
            cwd=root, env=env, text=True, capture_output=True, check=True,
        )
        assert not (root / "data").exists(), "Preview must not initialize local data"
        unit = preview.stdout.split("[Unit]", 1)[1].split("Installation copies", 1)[0]
        unit = "[Unit]" + unit
        if shutil.which("systemd-analyze"):
            unit_path = root / "totui.service"
            unit_path.write_text(unit.replace("/usr/local/lib/totui/totui", str(BINARY)))
            subprocess.run(
                ["systemd-analyze", "verify", str(unit_path)],
                check=True, capture_output=True, text=True,
            )
        with socket.socket() as listener:
            listener.bind(("127.0.0.1", 0))
            port = listener.getsockname()[1]
        url = f"http://127.0.0.1:{port}/api/ready"
        opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))

        def readiness():
            try:
                return opener.open(url, timeout=2).status
            except urllib.error.HTTPError as error:
                return error.code
            except OSError:
                return None

        def start(extra_env):
            process = subprocess.Popen(
                [str(BINARY), "server", "run", "--port", str(port)],
                cwd=root, env={**env, **extra_env}, stdout=log, stderr=log,
            )
            deadline = time.monotonic() + 20
            while readiness() != 200:
                if process.poll() is not None or time.monotonic() > deadline:
                    process.kill()
                    process.wait()
                    raise AssertionError((root / "server.log").read_text())
                time.sleep(0.1)
            return process

        with (root / "server.log").open("w") as log, socket.socket(
            socket.AF_UNIX, socket.SOCK_DGRAM
        ) as notify:
            socket_path = str(root / "notify")
            notify.bind(socket_path)
            notify.settimeout(15)
            process = start({"NOTIFY_SOCKET": socket_path})
            try:
                assert notify.recv(4096) == b"READY=1\nWATCHDOG=1"
                with sqlite3.connect(root / "data/todos.db") as db:
                    db.execute("ALTER TABLE projects RENAME TO unavailable_projects")
                assert readiness() == 503
                notify.settimeout(12)
                try:
                    message = notify.recv(4096)
                except TimeoutError:
                    pass
                else:
                    raise AssertionError(f"Unhealthy server sent watchdog notification: {message!r}")
                with sqlite3.connect(root / "data/todos.db") as db:
                    db.execute("ALTER TABLE unavailable_projects RENAME TO projects")
                    db.execute("CREATE TABLE persistence_test (value TEXT)")
                    db.execute("INSERT INTO persistence_test VALUES ('preserved')")
                assert readiness() == 200
                notify.settimeout(15)
                assert notify.recv(4096) == b"READY=1\nWATCHDOG=1"
            finally:
                process.terminate()
                assert process.wait(timeout=5) == 0
            process = start({})
            try:
                with sqlite3.connect(root / "data/todos.db") as db:
                    assert db.execute("SELECT value FROM persistence_test").fetchone() == (
                        "preserved",
                    )
            finally:
                process.terminate()
                assert process.wait(timeout=5) == 0
        print("Server CLI checks passed")


if __name__ == "__main__":
    main()

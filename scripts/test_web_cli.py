"""Verify the installed-style CLI from outside the checkout, with isolated data."""

import json
import os
import select
from pathlib import Path
import socket
import subprocess
import tempfile
import time
import urllib.request


BINARY = Path(__file__).resolve().parent.parent / "target/debug/to-tui"


def main():
    with tempfile.TemporaryDirectory(prefix="totui-web-cli-") as directory:
        root = Path(directory)
        env = {**os.environ, "TOTUI_DATA_DIR": str(root / "data"), "TOTUI_BIND": "127.0.0.1"}

        def run(*args, check=True):
            result = subprocess.run(
                [str(BINARY), "web", *args], cwd=root, env=env,
                text=True, capture_output=True, timeout=25,
            )
            if check and result.returncode:
                raise AssertionError(result.stdout + result.stderr)
            return result

        follow_env = {**env, "TOTUI_DATA_DIR": str(root / "follow")}
        follower = subprocess.Popen(
            [str(BINARY), "web", "logs", "--follow"], cwd=root, env=follow_env,
            stdout=subprocess.PIPE, stderr=subprocess.PIPE,
        )

        def expect_output(expected):
            received = b""
            deadline = time.monotonic() + 5
            while len(received) < len(expected) and time.monotonic() < deadline:
                if select.select([follower.stdout], [], [], 0.2)[0]:
                    chunk = os.read(follower.stdout.fileno(), 65536)
                    if not chunk:
                        break
                    received += chunk
            assert received == expected, (received, expected)

        try:
            log = root / "follow/web/server.log"
            log.parent.mkdir(parents=True, exist_ok=True)
            log.write_bytes(b"first line\n")
            expect_output(b"first line\n")
            with log.open("ab") as stream:
                stream.write(b"appended line\n")
            expect_output(b"appended line\n")
            replacement = log.with_suffix(".new")
            replacement.write_bytes(b"replacement log longer than the previous log\n")
            replacement.replace(log)
            expect_output(b"replacement log longer than the previous log\n")
            log.write_bytes(b"truncated\n")
            expect_output(b"truncated\n")
            follower.stdout.close()
            with log.open("ab") as stream:
                stream.write(b"closed pipe\n")
            assert follower.wait(timeout=5) == 0
        finally:
            if follower.poll() is None:
                follower.terminate()
                follower.wait(timeout=5)
            follower.stderr.close()

        (root / "data").mkdir(exist_ok=True)
        record_path = root / "data/web/process.json"
        try:
            with socket.socket() as listener:
                listener.bind(("127.0.0.1", 0))
                legacy_port = listener.getsockname()[1]
            legacy = subprocess.run(
                [str(BINARY), "serve", "start", "--port", str(legacy_port)],
                cwd=root, env=env, text=True, capture_output=True, timeout=25,
            )
            assert legacy.returncode == 0, legacy.stderr
            assert "Running" in run("status").stdout
            run("stop")
            assert not (root / "data/server.pid").exists()
            unrelated = subprocess.Popen(["sleep", "30"])
            try:
                (root / "data/server.pid").write_text(str(unrelated.pid))
                run("stop")
                assert unrelated.poll() is None
            finally:
                unrelated.terminate()
                unrelated.wait(timeout=5)
                (root / "data/server.pid").unlink(missing_ok=True)

            assert "No web server log" in run("--log").stderr
            run("start", "--port", "0", "--verbose")
            record = json.loads(record_path.read_text())
            with urllib.request.urlopen(record["url"]) as response:
                assert response.status == 200
            assert record["url"] in run("status").stdout
            assert "Workspace available at" in run("--log").stdout
            assert run("--detach", check=False).returncode != 0
            assert json.loads(record_path.read_text())["pid"] == record["pid"]
            run("--detach", "--restart", "--port", "0")
            replacement = json.loads(record_path.read_text())
            assert replacement["pid"] != record["pid"]
            run("restart", "--port", "0")
            assert json.loads(record_path.read_text())["pid"] != replacement["pid"]
            assert "Workspace available at" in run("logs").stdout
            run("stop")
            assert "Not running" in run("status").stdout
            run("stop")
            with socket.socket() as listener:
                listener.bind(("127.0.0.1", 0))
                listener.listen()
                failed = run("start", "--port", str(listener.getsockname()[1]), check=False)
                assert failed.returncode != 0
                assert "Web server exited" in failed.stderr
                assert not record_path.exists()
            run("--detach", "--port", "0")
            record = json.loads(record_path.read_text())
            try:
                record_path.write_text(json.dumps({**record, "identity": "stale identity"}))
                assert "Not running" in run("status").stdout
            finally:
                record_path.write_text(json.dumps(record))
        finally:
            run("stop")
    print("Outside-repo lifecycle, log following/replacement/truncation, closed pipe, stale PID, and startup failure passed")


if __name__ == "__main__":
    main()

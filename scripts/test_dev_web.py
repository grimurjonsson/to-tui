"""Exercise the just recipes with isolated data and an ephemeral port."""

import json
import os
from pathlib import Path
import socket
import subprocess
import tempfile
import time
import urllib.request


def main():
    with tempfile.TemporaryDirectory(prefix="totui-dev-web-") as directory:
        root = Path(directory)
        data = root / "data"
        data.mkdir()
        env = {
            **os.environ,
            "TOTUI_DATA_DIR": str(data),
            "TOTUI_DEV_WEB_DIR": str(root / "state"),
            "TOTUI_BIND": "127.0.0.1",
        }
        with socket.socket() as sock:
            sock.bind(("127.0.0.1", 0))
            port = sock.getsockname()[1]

        def run(*args, check=True):
            result = subprocess.run(
                ["just", *args], env=env, text=True, capture_output=True, timeout=20,
            )
            if check and result.returncode:
                raise AssertionError(result.stdout + result.stderr)
            return result

        def wait_for_server():
            for _ in range(300):
                try:
                    with urllib.request.urlopen(f"http://127.0.0.1:{port}/", timeout=0.3) as response:
                        assert response.status == 200
                    return
                except OSError:
                    time.sleep(0.1)
            raise AssertionError((root / "state/server.log").read_text())

        try:
            run("dev-web", "--detach", "--restart", "--verbose", "--port", str(port))
            wait_for_server()
            assert "Running" in run("dev-web-status").stdout
            assert run("dev-web", "--detach", check=False).returncode != 0
            request = urllib.request.Request(
                f"http://127.0.0.1:{port}/api/todos",
                data=b'{"content":"verbose forwarding verified"}',
                headers={"Content-Type": "application/json"},
            )
            with urllib.request.urlopen(request) as response:
                assert response.status == 201
            assert "verbose forwarding verified" in (root / "state/server.log").read_text()
            old_record = json.loads((root / "state/process.json").read_text())
            result = run("dev-web", "--detach", "--restart", "--port", str(port))
            assert "Development web server stopped" in result.stdout
            new_record = json.loads((root / "state/process.json").read_text())
            assert new_record["pid"] != old_record["pid"]
            previous = subprocess.run(
                ["ps", "-p", str(old_record["pid"]), "-o", "stat="],
                capture_output=True, text=True, check=False,
            )
            assert previous.returncode != 0 or previous.stdout.strip().startswith("Z")
            wait_for_server()
            assert "Running" in run("dev-web-status").stdout

        finally:
            run("dev-web-stop")
        assert "Not running" in run("dev-web-status").stdout
        run("dev-web-stop")
        try:
            urllib.request.urlopen(f"http://127.0.0.1:{port}/", timeout=0.3)
            raise AssertionError("Server still listening after stop")
        except OSError:
            pass
        mock = root / "mock"
        mock.mkdir()
        cargo = mock / "cargo"
        cargo.write_text(
            '#!/usr/bin/env python3\nimport sys,json\nprint(json.dumps(sys.argv[1:]))\n'
        )
        cargo.chmod(0o755)
        env["PATH"] = str(mock) + os.pathsep + env["PATH"]
        result = run("dev-web", "--open", "--verbose", "--port", "48765")
        assert json.loads(result.stdout) == [
            "run", "--bin", "totui", "--", "web", "--open", "--verbose", "--port", "48765",
        ]
        result = run("dev-web", "--restart", "--open", "--port", "48765")
        assert json.loads(result.stdout.splitlines()[-1]) == [
            "run", "--bin", "totui", "--", "web", "--open", "--port", "48765",
        ]
    print("Detached lifecycle, restart, HTTP, verbose logging, and exact argument forwarding passed")


if __name__ == "__main__":
    main()

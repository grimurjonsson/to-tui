"""Exercise web controls and external status refresh in a real isolated TUI."""

import fcntl
import json
import os
from pathlib import Path
import pty
import re
import select
import struct
import subprocess
import tempfile
import termios
import time
import urllib.request
import unicodedata

BINARY = Path(__file__).resolve().parent.parent / "target/debug/totui"


def main():
    with tempfile.TemporaryDirectory(prefix="totui-web-panel-") as directory:
        root = Path(directory)
        env = {**os.environ, "TOTUI_DATA_DIR": str(root / "data"), "TOTUI_BIND": "127.0.0.1", "TERM": "xterm-256color"}
        record_path = root / "data/web/process.json"

        def cli(*args):
            return subprocess.run([str(BINARY), *args], env=env, cwd=root, text=True,
                                  capture_output=True, check=True, timeout=25)

        def wait_record(running):
            deadline = time.monotonic() + 10
            while time.monotonic() < deadline:
                if record_path.exists() == running:
                    if running:
                        record = json.loads(record_path.read_text())
                        with urllib.request.urlopen(record["url"], timeout=1) as response:
                            assert response.status == 200
                        return record
                    return None
                time.sleep(0.05)
            raise AssertionError(f"Expected running={running}")

        cli("web", "start", "--port", "0")
        cli("todo", "create", "--content", "Web panel test task")
        master, slave = pty.openpty()
        fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 30, 100, 0, 0))
        process = subprocess.Popen([str(BINARY)], stdin=slave, stdout=slave, stderr=slave, env=env, cwd=root)
        os.close(slave)
        transcript = b""

        def screen_text():
            screen = [[" "] * 100 for _ in range(30)]
            row, col = 0, 0
            for match in re.finditer(r"\x1b\[[0-?]*[ -/]*[@-~]|.", transcript.decode("utf-8", errors="replace"), re.S):
                token = match.group()
                if token.startswith("\x1b["):
                    if token[-1] in "Hf":
                        values = token[2:-1].split(";")
                        row = int(values[0] or "1") - 1
                        col = int(values[1] or "1") - 1 if len(values) > 1 else 0
                    elif token == "\x1b[2J":
                        screen = [[" "] * 100 for _ in range(30)]
                    continue
                if token == "\r":
                    col = 0
                elif token == "\n":
                    row += 1
                elif token >= " " and 0 <= row < 30 and 0 <= col < 100:
                    screen[row][col] = token
                    col += 2 if unicodedata.east_asian_width(token) in ("W", "F") else 1
            return "\n".join("".join(line) for line in screen)

        def wait_text(needle):
            nonlocal transcript
            deadline = time.monotonic() + 12
            while time.monotonic() < deadline:
                if select.select([master], [], [], 0.1)[0]:
                    transcript += os.read(master, 65536)
                    if needle.decode() in screen_text():
                        return
            raise AssertionError(f"Missing {needle!r}:\n{screen_text()}")

        try:
            wait_text(b"w web-ui (running)")
            os.write(master, b"w")
            wait_text(b"Web server")
            wait_text("▶ Start".encode())
            os.write(master, b"\x1b[A")
            wait_text("▶ Open in browser".encode())
            os.write(master, b"\x1b[B\x1b[B")
            wait_text("▶ Stop".encode())
            os.write(master, b"\r")
            wait_record(False)
            wait_text(b"Status: stopped")
            os.write(master, b"\x1b[A\x1b[A")
            wait_text("▶ Open in browser".encode())
            os.write(master, b"\r")
            wait_text(b"Start the web server before opening the browser")
            os.write(master, b"\x1b[B")
            wait_text("▶ Start".encode())
            os.write(master, b"\r")
            first = wait_record(True)
            wait_text(b"Status: running")
            os.write(master, b"\x1b[B\x1b[B")
            wait_text("▶ Restart".encode())
            os.write(master, b"\r")
            deadline = time.monotonic() + 10
            while time.monotonic() < deadline:
                if record_path.exists() and json.loads(record_path.read_text())["pid"] != first["pid"]:
                    break
                time.sleep(0.05)
            else:
                raise AssertionError("TUI restart did not replace the server")
            wait_text(b"Status: running")
            cli("web", "stop")
            wait_text(b"Status: stopped")
            cli("web", "start", "--port", "0")
            wait_text(b"Status: running")
            os.write(master, b"\x1b")
            wait_text(b"NAVIGATE")
            footer = screen_text().splitlines()[-1]
            assert "? help  q quit  w web-ui" in footer, footer
            assert "🔗" in footer and "[github repo]" not in footer, footer
            assert "web-ui" not in screen_text().splitlines()[-2]
            column = footer.index("w web-ui") + 1
            os.write(master, f"\x1b[<0;{column};30M\x1b[<0;{column};30m".encode())
            wait_text(b"Web server")
            os.write(master, b"\x1b")
            wait_text(b"NAVIGATE")
            os.write(master, b"q")
            assert process.wait(timeout=5) == 0
            wait_record(True)
        finally:
            if process.poll() is None:
                process.terminate()
                process.wait(timeout=5)
            os.close(master)
            cli("web", "stop")
        print("TUI web panel start, stop, restart, CLI status refresh, and quit persistence passed")


if __name__ == "__main__":
    main()

"""Exercise Kitty capability negotiation and fallback using a simulated terminal."""

import fcntl
import os
from pathlib import Path
import pty
import re
import select
import signal
import struct
import subprocess
import tempfile
import termios
import time

BINARY = Path(__file__).resolve().parent.parent / "target/debug/totui"


def main():
    with tempfile.TemporaryDirectory(prefix="totui-terminal-icon-") as directory:
        root = Path(directory)
        env = {**os.environ, "TOTUI_DATA_DIR": str(root / "data"), "TOTUI_BIND": "127.0.0.1", "TERM": "xterm-256color"}
        env.pop("TMUX", None)

        def cli(*args):
            return subprocess.run([str(BINARY), *args], env=env, cwd=root, capture_output=True, check=True, timeout=25)

        cli("web", "start", "--port", "0")
        try:
            for mode in ["kitty", "unsupported", "timeout", "tmux", "screen", "plain"]:
                master, slave = pty.openpty()
                fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 30, 100, 0, 0))
                terminal_env = {**env, "TERM_PROGRAM": "ghostty"}
                if mode == "plain":
                    terminal_env["TERM_PROGRAM"] = "Apple_Terminal"
                if mode == "tmux":
                    terminal_env["TMUX"] = "/tmp/mock-tmux,1,0"
                if mode == "screen":
                    terminal_env["TERM"] = "screen-256color"
                process = subprocess.Popen([str(BINARY)], stdin=slave, stdout=slave, stderr=slave, cwd=root, env=terminal_env)
                os.close(slave)
                output = bytearray()
                answered = False

                def read_output():
                    nonlocal answered
                    if select.select([master], [], [], 0.05)[0]:
                        try:
                            data = os.read(master, 65536)
                        except OSError:
                            return
                        output.extend(data)
                        if not answered and b"\x1b[5n" in output:
                            answered = True
                            if mode == "kitty":
                                os.write(master, b"\x1b_Gi=31;OK\x1b\\\x1b[0n")
                            elif mode == "unsupported":
                                os.write(master, b"\x1b_Gi=31;ENOTSUP\x1b\\\x1b[0n")

                def wait_for(predicate):
                    deadline = time.monotonic() + 10
                    while time.monotonic() < deadline:
                        read_output()
                        if predicate():
                            return
                    raise AssertionError(f"{mode}: terminal condition not met: {bytes(output[-1000:])!r}")

                try:
                    wait_for(lambda: b"w web-ui" in output)
                    if mode == "kitty":
                        wait_for(lambda: b"a=T,U=1,f=100,t=d,c=2,r=1," in output)
                        assert "\U0010eeee".encode() in output
                        assert "🔗".encode() not in output
                        image_id = re.search(rb"c=2,r=1,i=(\d+)", output).group(1)
                        fcntl.ioctl(master, termios.TIOCSWINSZ, struct.pack("HHHH", 32, 90, 0, 0))
                        os.kill(process.pid, signal.SIGWINCH)
                        wait_for(lambda: output.count(b"a=T,U=1,f=100") >= 2)
                    else:
                        assert "🔗".encode() in output
                        assert "\U0010eeee".encode() not in output
                        assert b"a=T,U=1" not in output
                        if mode in ("tmux", "screen", "plain"):
                            assert b"\x1b_G" not in output
                    os.write(master, b"w")
                    wait_for(lambda: b"Web server" in output or b"Web\x1b" in output)
                    os.write(master, b"\x1b")
                    time.sleep(0.1)
                    os.write(master, b"q")
                    wait_for(lambda: process.poll() is not None)
                    assert process.returncode == 0
                    if mode == "kitty":
                        assert b"a=d,d=I,i=" + image_id + b",q=2" in output
                    print(f"{mode}: passed")
                finally:
                    if process.poll() is None:
                        process.terminate()
                        process.wait(timeout=5)
                    os.close(master)
        finally:
            cli("web", "stop")


if __name__ == "__main__":
    main()

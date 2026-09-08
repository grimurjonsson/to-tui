import os
import pty
import re
import select
import struct
import subprocess
import sys
import termios
import time
import fcntl

master, slave = pty.openpty()
fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack('HHHH', 30, 120, 0, 0))
process = subprocess.Popen([sys.argv[1]], stdin=slave, stdout=slave, stderr=slave,
                           env={**os.environ, 'TERM': 'xterm-256color'})
os.close(slave)

def wait_for(needle, timeout=10):
    output = b''
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if select.select([master], [], [], .1)[0]:
            output += os.read(master, 65536)
            printable = re.sub(rb'\x1b\[[0-?]*[ -/]*[@-~]', b'', output)
            if needle.replace(b' ', b'') in printable.replace(b' ', b''):
                return
    raise AssertionError(f'TUI did not render {needle!r}; tail: {output[-1000:]!r}')

try:
    wait_for(b'NAVIGATE')
    os.write(master, b'nTUI process task\r')
    time.sleep(.2)
    os.write(master, b'\x1b')
    wait_for(b'NAVIGATE')
    os.write(master, b'j')
    print('READY', flush=True)
    wait_for(b'Browser to TUI')
    print('RENDERED', flush=True)
finally:
    process.terminate()
    process.wait(timeout=5)
    os.close(master)

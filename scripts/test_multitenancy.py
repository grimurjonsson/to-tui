"""Exercise local compatibility and tenant isolation against a fake OAuth auth endpoint."""

from concurrent.futures import ThreadPoolExecutor
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import http.client
import json
import os
from pathlib import Path
import socket
import sqlite3
import subprocess
import tempfile
import threading
import time
import urllib.error
import urllib.request

BINARY = Path(os.environ.get("TOTUI_TEST_BINARY", Path(__file__).resolve().parent.parent / "target/debug/totui"))


class Gateway(BaseHTTPRequestHandler):
    def do_GET(self):
        session = self.headers.get("Cookie", "")
        subject = {
            "session=alice": "google-alice",
            "session=alice-renamed": "google-alice",
            "session=bob": "google-bob",
        }.get(session)
        if session == "session=broken":
            self.send_response(202)
        elif session == "session=unavailable":
            self.send_response(503)
        elif subject:
            self.send_response(202)
            self.send_header("X-Auth-Request-User", subject)
            self.send_header("X-Auth-Request-Email", "new@example.test" if "renamed" in session else "same@example.test")
        else:
            self.send_response(401)
        self.end_headers()

    def log_message(self, *args):
        pass


def free_port():
    with socket.socket() as sock:
        sock.bind(("127.0.0.1", 0))
        return sock.getsockname()[1]


def main():
    gateway = ThreadingHTTPServer(("127.0.0.1", 0), Gateway)
    thread = threading.Thread(target=gateway.serve_forever, daemon=True)
    thread.start()
    processes = []
    streams = []
    try:
        with tempfile.TemporaryDirectory(prefix="totui-tenants-") as directory:
            root = Path(directory)
            env = {
                **os.environ,
                "TOTUI_DATA_DIR": str(root / "data"),
                "TOTUI_BIND": "127.0.0.1",
                "TOTUI_AUTH_URL": f"http://127.0.0.1:{gateway.server_port}/oauth2/auth",
            }
            env.pop("NOTIFY_SOCKET", None)
            env.pop("TOTUI_WEB_READY_FILE", None)
            client = urllib.request.build_opener(urllib.request.ProxyHandler({}))

            def request(port, path, user=None, method="GET", payload=None, headers=None):
                headers = {**(headers or {})}
                if user:
                    headers["Cookie"] = f"session={user}"
                if payload is not None:
                    headers["Content-Type"] = "application/json"
                req = urllib.request.Request(
                    f"http://127.0.0.1:{port}{path}", method=method, headers=headers,
                    data=json.dumps(payload).encode() if payload is not None else None,
                )
                try:
                    response = client.open(req, timeout=20)
                except urllib.error.HTTPError as error:
                    response = error
                with response:
                    body = response.read()
                    try:
                        data = json.loads(body)
                    except ValueError:
                        data = body.decode()
                    return response.status, data

            def ok(port, path, user=None, method="GET", payload=None):
                status, data = request(port, path, user, method, payload)
                assert 200 <= status < 300, (status, path, data)
                return data

            def start(auth, port=None):
                port = port or free_port()
                log = (root / f"server-{port}.log").open("a")
                command = [str(BINARY), "web", "--port", str(port)]
                if auth:
                    command.append("--auth")
                process = subprocess.Popen(command, cwd=root, env=env, stdout=log, stderr=log)
                log.close()
                processes.append(process)
                deadline = time.monotonic() + 20
                while time.monotonic() < deadline:
                    if process.poll() is not None:
                        raise AssertionError((root / f"server-{port}.log").read_text())
                    try:
                        if request(port, "/api/ready")[0] == 200:
                            return process, port
                    except OSError:
                        pass
                    time.sleep(0.1)
                raise AssertionError("Server did not become ready")

            def snapshot(user, project="default", date=None):
                return ok(port, f"/api/snapshot?project={project}" + (f"&date={date}" if date else ""), user)

            local_process, local_port = start(False)
            local_item = ok(local_port, "/api/todos", method="POST", payload={"content": "local-private"})
            assert ok(local_port, "/api/me") == {"id": "local", "email": None}
            assert ok(local_port, "/api/me", "alice")["id"] == "local"
            assert request(local_port, "/api/me", headers={"X-Auth-Request-User": "forged"})[1]["id"] == "local"
            local_process.terminate()
            assert local_process.wait(timeout=5) == 0

            process, port = start(True)
            for path in ["/", "/app.js", "/style.css", "/api/me", "/api/server", "/api/snapshot", "/api/projects", "/api/events"]:
                assert request(port, path)[0] == 401, path
                assert request(port, path, headers={"X-Auth-Request-User": "google-alice", "X-Totui-User": "google-alice"})[0] == 401, path
            assert request(port, "/api/me", "broken")[0] == 401
            assert request(port, "/api/me", "unavailable")[0] == 502
            assert request(port, "/api/me", "revoked")[0] == 401

            with ThreadPoolExecutor(max_workers=8) as pool:
                identities = list(pool.map(lambda _: ok(port, "/api/me", "alice"), range(16)))
            alice = identities[0]
            assert all(user["id"] == alice["id"] for user in identities)
            bob = ok(port, "/api/me", "bob")
            assert alice["id"] != bob["id"]
            assert alice["email"] == bob["email"], "Test identities deliberately share an email"
            assert snapshot("alice")["items"] == []
            assert snapshot("bob")["items"] == []
            changed = ok(port, "/api/me", "alice-renamed")
            assert changed["id"] == alice["id"] and changed["email"] != alice["email"]

            alice_project = ok(port, "/api/projects", "alice", "POST", {"name": "private"})
            bob_project = ok(port, "/api/projects", "bob", "POST", {"name": "private"})
            assert alice_project["id"] != bob_project["id"]
            task = ok(port, "/api/todos?project=private", "alice", "POST", {"content": "Alice secret"})
            task_id = task["id"]
            assert [item["content"] for item in snapshot("alice", "private")["items"]] == ["Alice secret"]
            assert snapshot("bob", "private")["items"] == []
            assert request(port, f"/api/todos/{task_id}?project=private", "bob", "PATCH", {"content": "stolen"})[0] == 404
            assert request(port, f"/api/todos/{task_id}?project=private", "bob", "DELETE")[0] == 404
            assert request(port, f"/api/todos/{task_id}/move?project=private", "bob", "POST", {})[0] == 404
            assert request(port, "/api/todos?project=private", "bob", "POST", {"content": "child", "parent_id": task_id})[0] in [400, 404]
            for method, payload in [("PATCH", {"name": "stolen"}), ("DELETE", None)]:
                assert request(port, f'/api/projects/{alice_project["id"]}', "bob", method, payload)[0] == 404
            assert request(port, "/api/todos", "bob", "POST", {"content": "stale draft"}, {"X-Totui-Expected-User": alice["id"]})[0] == 409
            assert snapshot("bob")["items"] == []

            alice_root = root / "data/users" / alice["id"]
            bob_root = root / "data/users" / bob["id"]
            assert "Alice secret" in next((alice_root / "projects/private/dailies").glob("*.md")).read_text()
            assert "Alice secret" not in "".join(file.read_text() for file in bob_root.rglob("*.md"))
            with sqlite3.connect(alice_root / "todos.db") as db:
                db.execute("UPDATE todos SET date='2001-01-01' WHERE id=?", (task_id,))
            assert snapshot("alice", "private", "2001-01-01")["items"][0]["content"] == "Alice secret"
            assert snapshot("bob", "private", "2001-01-01")["items"] == []

            def connect_events(user):
                connection = http.client.HTTPConnection("127.0.0.1", port, timeout=3)
                connection.request("GET", "/api/events", headers={"Cookie": f"session={user}"})
                response = connection.getresponse()
                assert response.status == 200
                streams.append((connection, response))
                return connection, response

            def event(response):
                lines = []
                while True:
                    line = response.readline().decode().strip()
                    if not line and lines:
                        return "\n".join(lines)
                    if line:
                        lines.append(line)

            time.sleep(0.3)
            connection, response = connect_events("bob")
            assert "event: change" in event(response)
            ok(port, "/api/todos", "alice", "POST", {"content": "Alice update"})
            connection.sock.settimeout(0.7)
            try:
                leaked = event(response)
            except (TimeoutError, OSError):
                pass
            else:
                raise AssertionError(f"Bob received Alice's live update: {leaked}")
            response.close()
            connection.close()
            connection, response = connect_events("bob")
            event(response)
            ok(port, "/api/todos", "bob", "POST", {"content": "Bob update"})
            assert "event: change" in event(response)
            response.close()
            connection.close()
            streams.clear()

            process.terminate()
            assert process.wait(timeout=5) == 0
            env["TOTUI_WEB_UPGRADE"] = "1"
            env["TOTUI_SERVER_OWNER_ID"] = alice["id"]
            process, port = start(True, port)
            assert request(port, "/signed-out")[0] == 200
            for user, identity in [("alice", alice), ("bob", bob)]:
                info = ok(port, "/api/server", user)
                assert info["user"]["id"] == identity["id"]
                assert info["version"]
                assert info["logout_url"] == "/oauth2/sign_out?rd=%2Fsigned-out"
                assert info["can_upgrade"] == (user == "alice" and os.uname().sysname == "Linux")
            assert request(port, "/api/server/upgrade", "bob", "POST", {"version": "99.0.0"})[0] == 403
            assert request(port, "/api/server/upgrade", "alice", "POST", {"version": "99.0.0"}, {"Origin": "https://evil.example"})[0] == 403
            assert request(port, "/api/server/upgrade", "alice", "POST", {"version": "99.0.0"}, {"X-Totui-Expected-User": bob["id"]})[0] == 409
            assert request(port, "/api/server/upgrade", None, "POST", {"version": "99.0.0"})[0] == 401
            assert ok(port, "/api/me", "alice")["id"] == alice["id"]
            assert [item["content"] for item in snapshot("bob")["items"]] == ["Bob update"]
            assert snapshot("alice", "private", "2001-01-01")["items"][0]["content"] == "Alice secret"
            process.terminate()
            assert process.wait(timeout=5) == 0
            local_process, local_port = start(False)
            assert ok(local_port, "/api/snapshot")["items"][0]["id"] == local_item["id"]
            local_process.terminate()
            assert local_process.wait(timeout=5) == 0
            managed_port = free_port()
            def manage(*args):
                return subprocess.run([str(BINARY), "web", *args], cwd=root, env=env, check=True, capture_output=True, timeout=20)
            try:
                manage("start", "--auth", "--port", str(managed_port))
                assert request(managed_port, "/api/me")[0] == 401
                manage("restart", "--port", str(managed_port))
                assert request(managed_port, "/api/me")[0] == 401
                assert ok(managed_port, "/api/me", "alice")["id"] == alice["id"]
            finally:
                manage("stop")
            with sqlite3.connect(root / "data/users.db") as db:
                assert db.execute("SELECT COUNT(*) FROM users").fetchone()[0] == 2
            print("Local compatibility, provisioning, cross-user IDs, history, exports, live updates, stale tabs and restart isolation passed")
    finally:
        for connection, response in streams:
            response.close()
            connection.close()
        for process in processes:
            if process.poll() is None:
                process.terminate()
                try:
                    process.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait()
        gateway.shutdown()
        gateway.server_close()


if __name__ == "__main__":
    main()

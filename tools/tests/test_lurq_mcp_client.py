"""Protocol and credential-output tests for the exact-PID lurq MCP client."""

import http.server
import json
import os
import subprocess
import sys
import tempfile
import threading
import unittest
from pathlib import Path


CLIENT = Path(__file__).resolve().parents[1] / "lurq-mcp-client.py"
REPO_TMP = Path(__file__).resolve().parents[2] / ".tmp"
TOKEN = "SYNTHETIC_D7_TOKEN_DO_NOT_USE"
PID = 12345


class McpHandler(http.server.BaseHTTPRequestHandler):
    calls = []
    reject = False
    redirect = False
    malformed_stage = None
    sse_heartbeat = False
    non_ascii = False

    def do_GET(self):
        self.calls.append(("redirected GET", self.headers.get("Authorization")))
        self.send_response(200)
        self.end_headers()

    def do_POST(self):
        payload = json.loads(self.rfile.read(int(self.headers["Content-Length"])))
        self.calls.append((payload["method"], self.headers.get("Authorization")))
        if self.path != "/mcp" or self.headers.get("Authorization") != f"Bearer {TOKEN}":
            self.send_error(401)
            return
        if self.reject:
            body = f"server diagnostic contains {TOKEN}".encode()
            self.send_response(500)
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)
            return
        if self.redirect:
            self.send_response(302)
            self.send_header("Location", f"http://127.0.0.1:{self.server.server_port}/capture")
            self.end_headers()
            return
        if payload["method"] == "notifications/initialized":
            self.send_response(202)
            self.end_headers()
            return
        if payload["method"] == "initialize":
            reply = {"jsonrpc": "2.0", "id": 1}
            if self.malformed_stage != "initialize":
                reply["result"] = {"protocolVersion": "2025-03-26"}
            if self.sse_heartbeat:
                body = ("data:\nid: keepalive\nretry: 1000\n\ndata: " + json.dumps(reply) + "\n\n").encode()
                content_type = "text/event-stream"
            else:
                body = json.dumps(reply).encode()
                content_type = "application/json"
            self.send_response(200)
            self.send_header("Content-Type", content_type)
            self.send_header("Mcp-Session-Id", "synthetic-session")
        else:
            if self.headers.get("Mcp-Session-Id") != "synthetic-session":
                self.send_error(400)
                return
            reply = {"jsonrpc": "2.0", "id": 2}
            if self.malformed_stage != "tools/call":
                text = "🌍 " + TOKEN if self.non_ascii else TOKEN
                reply["result"] = {"content": [{"text": text}]}
            body = ("data: " + json.dumps(reply) + "\n\n").encode()
            self.send_response(200)
            self.send_header("Content-Type", "text/event-stream")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def log_message(self, *_args):
        pass


class ExactPidClientTest(unittest.TestCase):
    def setUp(self):
        REPO_TMP.mkdir(exist_ok=True)
        self.directory = tempfile.TemporaryDirectory(dir=REPO_TMP, prefix="d7-client-")
        self.addCleanup(self.directory.cleanup)
        McpHandler.calls = []
        McpHandler.reject = False
        McpHandler.redirect = False
        McpHandler.malformed_stage = None
        McpHandler.sse_heartbeat = False
        McpHandler.non_ascii = False
        self.server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), McpHandler)
        thread = threading.Thread(target=self.server.serve_forever, daemon=True)
        thread.start()
        self.addCleanup(self.server.server_close)
        self.addCleanup(self.server.shutdown)

    def discovery(self, *, pid=PID, app="orchester", url=None):
        port = self.server.server_port
        data = {
            "pid": pid,
            "app": app,
            "port": port,
            "url": url or f"http://127.0.0.1:{port}/mcp",
            "token": TOKEN,
        }
        (Path(self.directory.name) / f"{PID}.json").write_text(json.dumps(data), encoding="utf-8")

    def client(self, *, env=None):
        return subprocess.run(
            [sys.executable, str(CLIENT), str(PID), "lurq_windows", "--discovery-dir", self.directory.name],
            capture_output=True,
            text=True,
            timeout=10,
            check=False,
            env=env,
        )

    def test_exact_pid_call_uses_token_but_never_prints_it(self):
        self.discovery()
        result = self.client()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertNotIn(TOKEN, result.stdout + result.stderr)
        self.assertIn("[REDACTED]", result.stdout)
        self.assertEqual(
            [method for method, _auth in McpHandler.calls],
            ["initialize", "notifications/initialized", "tools/call"],
        )
        self.assertTrue(all(auth == f"Bearer {TOKEN}" for _method, auth in McpHandler.calls))

    def test_mismatched_pid_is_rejected_before_network(self):
        self.discovery(pid=PID + 1)
        result = self.client()
        self.assertEqual(result.returncode, 1)
        self.assertIn("does not match", result.stderr)
        self.assertNotIn(TOKEN, result.stdout + result.stderr)
        self.assertFalse(McpHandler.calls)

    def test_non_loopback_url_is_rejected_before_network(self):
        self.discovery(url="http://example.com/mcp")
        result = self.client()
        self.assertEqual(result.returncode, 1)
        self.assertIn("loopback", result.stderr)
        self.assertNotIn(TOKEN, result.stdout + result.stderr)
        self.assertFalse(McpHandler.calls)

    def test_http_error_body_cannot_print_the_token(self):
        self.discovery()
        McpHandler.reject = True
        result = self.client()
        self.assertEqual(result.returncode, 1)
        self.assertIn("MCP HTTP status 500", result.stderr)
        self.assertNotIn(TOKEN, result.stdout + result.stderr)

    def test_redirect_cannot_forward_the_token(self):
        self.discovery()
        McpHandler.redirect = True
        result = self.client()
        self.assertEqual(result.returncode, 1)
        self.assertIn("MCP HTTP status 302", result.stderr)
        self.assertNotIn(TOKEN, result.stdout + result.stderr)
        self.assertEqual([method for method, _auth in McpHandler.calls], ["initialize"])

    def test_missing_initialize_result_is_not_success(self):
        self.discovery()
        McpHandler.malformed_stage = "initialize"
        result = self.client()
        self.assertEqual(result.returncode, 1)
        self.assertIn("MCP initialize failed", result.stderr)
        self.assertNotIn(TOKEN, result.stdout + result.stderr)

    def test_missing_tool_result_is_not_success(self):
        self.discovery()
        McpHandler.malformed_stage = "tools/call"
        result = self.client()
        self.assertEqual(result.returncode, 1)
        self.assertIn("MCP tool call failed", result.stderr)
        self.assertNotIn(TOKEN, result.stdout + result.stderr)

    def test_empty_sse_heartbeat_before_initialize_result(self):
        self.discovery()
        McpHandler.sse_heartbeat = True
        result = self.client()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("[REDACTED]", result.stdout)
        self.assertNotIn(TOKEN, result.stdout + result.stderr)

    def test_unicode_tool_result_survives_ascii_stdout(self):
        self.discovery()
        McpHandler.non_ascii = True
        env = os.environ.copy()
        env["PYTHONIOENCODING"] = "ascii"
        result = self.client(env=env)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertTrue(result.stdout.isascii())
        self.assertEqual(json.loads(result.stdout)["content"][0]["text"], "🌍 [REDACTED]")
        self.assertNotIn(TOKEN, result.stdout + result.stderr)


if __name__ == "__main__":
    unittest.main()

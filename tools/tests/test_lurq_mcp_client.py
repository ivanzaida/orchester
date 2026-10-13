"""Protocol and credential-output tests for the exact-PID lurq MCP client."""

import base64
import http.server
import json
import os
import subprocess
import sys
import tempfile
import threading
import unittest
import zlib
from pathlib import Path


CLIENT = Path(__file__).resolve().parents[1] / "lurq-mcp-client.py"
REPO_TMP = Path(__file__).resolve().parents[2] / ".tmp"
TOKEN = "SYNTHETIC_D7_TOKEN_DO_NOT_USE"
PID = 12345


def png_fixture(idat=None):
    def chunk(kind, data):
        return len(data).to_bytes(4, "big") + kind + data + zlib.crc32(kind + data).to_bytes(4, "big")

    ihdr = (1).to_bytes(4, "big") * 2 + bytes((8, 6, 0, 0, 0))
    if idat is None:
        idat = zlib.compress(b"\0\0\0\0\0")
    return b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", ihdr) + chunk(b"IDAT", idat) + chunk(b"IEND", b"")


PNG = png_fixture()
IMAGE = {"type": "image", "mimeType": "image/png", "data": base64.b64encode(PNG).decode("ascii")}


class McpHandler(http.server.BaseHTTPRequestHandler):
    calls = []
    reject = False
    redirect = False
    malformed_stage = None
    sse_heartbeat = False
    non_ascii = False
    screenshot_blocks = None
    screenshot_error = False

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
                if payload["params"]["name"] == "lurq_screenshot":
                    blocks = self.screenshot_blocks if self.screenshot_blocks is not None else [IMAGE]
                    reply["result"] = {"content": blocks, "isError": self.screenshot_error}
                else:
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
        McpHandler.screenshot_blocks = None
        McpHandler.screenshot_error = False
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

    def client(self, *, env=None, tool="lurq_windows", image_out=None, discovery=True, app=None):
        command = [sys.executable, str(CLIENT), str(PID), tool]
        if discovery is True:
            command.extend(("--discovery-dir", self.directory.name))
        elif discovery is not None:
            command.extend(("--discovery", discovery))
        if app is not None:
            command.extend(("--app", app))
        if image_out is not None:
            command.extend(("--image-out", str(image_out)))
        return subprocess.run(
            command,
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

    def environment(self, variable):
        """This process's environment with the user folders and the QA variable under test control."""
        user = tempfile.TemporaryDirectory(dir=REPO_TMP, prefix="d7-user-")
        self.addCleanup(user.cleanup)
        env = {key: value for key, value in os.environ.items() if key != "ORCHESTER_APP_MCP_DISCOVERY"}
        env["LOCALAPPDATA"] = user.name
        env["HOME"] = user.name
        if variable is not None:
            env["ORCHESTER_APP_MCP_DISCOVERY"] = variable
        return env

    def test_discovery_variable_names_the_new_apps_folder(self):
        self.discovery(app="orchester-app")
        result = self.client(env=self.environment(self.directory.name), discovery=None, app="orchester-app")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertNotIn(TOKEN, result.stdout + result.stderr)

    def test_discovery_option_wins_over_the_variable(self):
        self.discovery(app="orchester-app")
        other = tempfile.TemporaryDirectory(dir=REPO_TMP, prefix="d7-other-")
        self.addCleanup(other.cleanup)
        result = self.client(
            env=self.environment(other.name), discovery=self.directory.name, app="orchester-app"
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertNotIn(TOKEN, result.stdout + result.stderr)

    def test_discovery_variable_is_only_for_the_new_app(self):
        self.discovery()
        result = self.client(env=self.environment(self.directory.name), discovery=None)
        self.assertEqual(result.returncode, 1)
        self.assertIn("Cannot read MCP discovery", result.stderr)
        self.assertEqual(McpHandler.calls, [])
        self.assertNotIn(TOKEN, result.stdout + result.stderr)

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

    def test_screenshot_writes_png_and_only_safe_metadata(self):
        self.discovery()
        destination = Path(self.directory.name) / "captures" / "screen.png"
        result = self.client(tool="lurq_screenshot", image_out=destination)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(destination.read_bytes(), PNG)
        self.assertEqual(
            json.loads(result.stdout),
            {"ok": True, "saved": True, "mime_type": "image/png", "bytes": len(PNG), "width": 1, "height": 1},
        )
        self.assertNotIn(TOKEN, result.stdout + result.stderr)
        self.assertNotIn(IMAGE["data"], result.stdout + result.stderr)

    def test_screenshot_without_output_reports_unsaved_metadata(self):
        self.discovery()
        result = self.client(tool="lurq_screenshot")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(json.loads(result.stdout)["saved"], False)
        self.assertNotIn(IMAGE["data"], result.stdout + result.stderr)

    def test_image_out_is_rejected_for_other_tools_before_network(self):
        self.discovery()
        result = self.client(image_out=Path(self.directory.name) / "screen.png")
        self.assertEqual(result.returncode, 1)
        self.assertIn("only valid with lurq_screenshot", result.stderr)
        self.assertFalse(McpHandler.calls)

    def test_screenshot_tool_error_and_invalid_content_never_write(self):
        self.discovery()
        destination = Path(self.directory.name) / "screen.png"
        cases = (
            (True, [IMAGE], "reported an error"),
            (False, "not a list", "exactly one image block"),
            (False, [], "exactly one image block"),
            (False, [IMAGE, IMAGE], "exactly one image block"),
            (False, [{"type": "text", "text": TOKEN}], "not a PNG image block"),
            (False, [{**IMAGE, "mimeType": "image/jpeg"}], "not a PNG image block"),
            (False, [{**IMAGE, "data": "%%%"}], "invalid base64"),
            (False, [{**IMAGE, "data": base64.b64encode(b"not a PNG").decode()}], "not a PNG"),
        )
        for error, blocks, message in cases:
            with self.subTest(message=message):
                McpHandler.screenshot_error = error
                McpHandler.screenshot_blocks = blocks
                result = self.client(tool="lurq_screenshot", image_out=destination)
                self.assertEqual(result.returncode, 1)
                self.assertIn(message, result.stderr)
                self.assertFalse(destination.exists())
                self.assertNotIn(TOKEN, result.stdout + result.stderr)
                self.assertNotIn(IMAGE["data"], result.stdout + result.stderr)

    def test_screenshot_rejects_incomplete_or_corrupt_png(self):
        self.discovery()
        destination = Path(self.directory.name) / "screen.png"
        bad_crc = bytearray(PNG)
        bad_crc[29] ^= 1
        cases = (
            ("reviewer IHDR-only payload", PNG[:33]),
            ("missing IEND", PNG[:-12]),
            ("invalid IHDR checksum", bytes(bad_crc)),
            ("corrupt IDAT with valid checksum", png_fixture(idat=b"not zlib")),
            ("invalid PNG row filter", png_fixture(idat=zlib.compress(b"\5\0\0\0\0"))),
        )
        for name, payload in cases:
            with self.subTest(name=name):
                encoded = base64.b64encode(payload).decode("ascii")
                McpHandler.screenshot_blocks = [{**IMAGE, "data": encoded}]
                result = self.client(tool="lurq_screenshot", image_out=destination)
                self.assertEqual(result.returncode, 1)
                self.assertEqual(result.stdout, "")
                self.assertFalse(destination.exists())
                self.assertNotIn(TOKEN, result.stderr)
                self.assertNotIn(encoded, result.stderr)

    def test_screenshot_write_failure_never_reports_success(self):
        self.discovery()
        destination = Path(self.directory.name) / "screen.png"
        destination.mkdir()
        result = self.client(tool="lurq_screenshot", image_out=destination)
        self.assertEqual(result.returncode, 1)
        self.assertEqual(result.stdout, "")
        self.assertIn("Cannot save screenshot PNG", result.stderr)
        self.assertFalse(list(Path(self.directory.name).glob(".lurq-screenshot-*")))


if __name__ == "__main__":
    unittest.main()

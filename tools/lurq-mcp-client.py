"""Call one embedded lurq MCP tool for an exact app PID without printing discovery credentials.

Example: python tools/lurq-mcp-client.py 12345 lurq_windows
The app and this client must use the same LOCALAPPDATA (Windows) or HOME (macOS).
"""

import argparse
import json
import os
import sys
import urllib.error
import urllib.request
from pathlib import Path


class ClientError(Exception):
    """An error safe to show without exposing the discovery file."""


class NoRedirect(urllib.request.HTTPRedirectHandler):
    """A discovery token must never be forwarded to a server-supplied URL."""

    def redirect_request(self, _request, _fp, _code, _message, _headers, _new_url):
        return None


def discovery_dir(override):
    if override is not None:
        return Path(override)
    if sys.platform == "win32":
        base = os.environ.get("LOCALAPPDATA")
        if base:
            return Path(base) / "lurq" / "mcp"
    elif sys.platform == "darwin":
        base = os.environ.get("HOME")
        if base:
            return Path(base) / "Library" / "Application Support" / "lurq" / "mcp"
    else:
        base = os.environ.get("XDG_RUNTIME_DIR") or os.environ.get("XDG_STATE_HOME")
        if base:
            return Path(base) / "lurq" / "mcp"
        home = os.environ.get("HOME")
        if home:
            return Path(home) / ".local" / "state" / "lurq" / "mcp"
    raise ClientError("No lurq MCP discovery directory is configured")


def load_discovery(pid, app, directory):
    path = directory / f"{pid}.json"
    try:
        data = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise ClientError(f"Cannot read MCP discovery for PID {pid} ({type(error).__name__})") from None
    if not isinstance(data, dict) or data.get("pid") != pid or data.get("app") != app:
        raise ClientError("MCP discovery does not match the requested PID and app")
    token = data.get("token")
    port = data.get("port")
    url = data.get("url")
    if not isinstance(token, str) or not token or not isinstance(port, int) or not 0 < port < 65536:
        raise ClientError("MCP discovery has invalid connection fields")
    if url != f"http://127.0.0.1:{port}/mcp":
        raise ClientError("MCP discovery URL is not the expected loopback endpoint")
    return url, token


def response_json(body, content_type, expected_id):
    if not body:
        return None
    if content_type.startswith("text/event-stream"):
        events = []
        data_lines = []
        for line in body.decode("utf-8").splitlines() + [""]:
            if line.startswith("data:"):
                data_lines.append(line[5:].lstrip())
            elif not line and data_lines:
                events.append("\n".join(data_lines))
                data_lines = []
        for event in events:
            if not event.strip():
                continue
            value = json.loads(event)
            if isinstance(value, dict) and value.get("id") == expected_id:
                return value
        raise ClientError("MCP response did not contain the requested result")
    if not content_type.startswith("application/json"):
        raise ClientError("MCP response has an unsupported content type")
    return json.loads(body)


def post(opener, url, headers, payload, expected_id=None):
    request = urllib.request.Request(
        url,
        data=json.dumps(payload).encode("utf-8"),
        headers=headers,
        method="POST",
    )
    try:
        with opener.open(request, timeout=15) as response:
            body = response.read()
            content_type = response.headers.get("Content-Type", "")
            session = response.headers.get("Mcp-Session-Id")
    except urllib.error.HTTPError as error:
        raise ClientError(f"MCP HTTP status {error.code}") from None
    except (OSError, urllib.error.URLError) as error:
        raise ClientError(f"MCP request failed ({type(error).__name__})") from None
    try:
        return response_json(body, content_type, expected_id), session
    except (UnicodeError, json.JSONDecodeError):
        raise ClientError("MCP response is not valid JSON") from None


def call_tool(url, token, tool, arguments):
    opener = urllib.request.build_opener(urllib.request.ProxyHandler({}), NoRedirect())
    headers = {
        "Authorization": f"Bearer {token}",
        "Content-Type": "application/json",
        "Accept": "application/json, text/event-stream",
    }
    initialized, session = post(
        opener,
        url,
        headers,
        {
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2025-03-26",
                "capabilities": {},
                "clientInfo": {"name": "orchester-lurq-mcp-client", "version": "1"},
            },
        },
        1,
    )
    if (
        not isinstance(initialized, dict)
        or "error" in initialized
        or initialized.get("id") != 1
        or not isinstance(initialized.get("result"), dict)
    ):
        raise ClientError("MCP initialize failed")
    if session:
        headers["Mcp-Session-Id"] = session
    post(opener, url, headers, {"jsonrpc": "2.0", "method": "notifications/initialized"})
    reply, _ = post(
        opener,
        url,
        headers,
        {"jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": {"name": tool, "arguments": arguments}},
        2,
    )
    if (
        not isinstance(reply, dict)
        or "error" in reply
        or reply.get("id") != 2
        or not isinstance(reply.get("result"), dict)
    ):
        raise ClientError("MCP tool call failed")
    return reply.get("result")


def redact(value, token):
    if isinstance(value, str):
        return value.replace(token, "[REDACTED]")
    if isinstance(value, list):
        return [redact(item, token) for item in value]
    if isinstance(value, dict):
        return {redact(key, token): redact(item, token) for key, item in value.items()}
    return value


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("pid", type=int, help="PID of the app started by this run")
    parser.add_argument("tool", help="embedded lurq MCP tool name")
    parser.add_argument("--app", default="orchester", help="expected app name in discovery")
    parser.add_argument("--arguments", default="{}", help="JSON object of tool arguments; do not pass secrets")
    parser.add_argument("--discovery-dir", help="directory containing the exact-PID discovery file")
    args = parser.parse_args(argv)
    if args.pid <= 0:
        parser.error("PID must be positive")
    try:
        arguments = json.loads(args.arguments)
        if not isinstance(arguments, dict):
            raise ClientError("Tool arguments must be a JSON object")
        directory = discovery_dir(args.discovery_dir)
        url, token = load_discovery(args.pid, args.app, directory)
        result = call_tool(url, token, args.tool, arguments)
        # ASCII JSON survives Windows pipes even when their encoding cannot represent UI text.
        print(json.dumps(redact(result, token), ensure_ascii=True))
        return 0
    except json.JSONDecodeError:
        print("Invalid tool arguments JSON", file=sys.stderr)
    except ClientError as error:
        print(error, file=sys.stderr)
    except Exception as error:
        # Unexpected library exceptions may include request headers. Show only their type.
        print(f"MCP client failed ({type(error).__name__})", file=sys.stderr)
    return 1


if __name__ == "__main__":
    sys.exit(main())

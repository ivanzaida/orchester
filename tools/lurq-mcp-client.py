"""Call one embedded lurq MCP tool for an exact app PID without printing discovery credentials.

Example: python tools/lurq-mcp-client.py 12345 app_state --app orchester-app --discovery .tmp/qa/<task>/mcp
The discovery folder is --discovery when given. Else, for --app orchester-app, the folder named by
ORCHESTER_APP_MCP_DISCOVERY (which tools/qa-app.ps1 sets for its QA copy) when that is set. Else the user
folder: orchester-app-mcp for orchester-app, lurq/mcp otherwise, in LOCALAPPDATA (Windows) or
~/Library/Application Support (macOS); the app and this client must then use the same LOCALAPPDATA or HOME.
The installed orchester-app writes no discovery file: register it as Settings > Automation shows instead.
"""

import argparse
import base64
import binascii
import json
import os
import sys
import tempfile
import urllib.error
import urllib.request
import zlib
from pathlib import Path


class ClientError(Exception):
    """An error safe to show without exposing the discovery file."""


class NoRedirect(urllib.request.HTTPRedirectHandler):
    """A discovery token must never be forwarded to a server-supplied URL."""

    def redirect_request(self, _request, _fp, _code, _message, _headers, _new_url):
        return None


# The new app's name in discovery, the variable naming a QA copy's discovery folder, and its user folder.
NEW_APP = "orchester-app"
DISCOVERY_VARIABLE = "ORCHESTER_APP_MCP_DISCOVERY"
NEW_APP_FOLDER = "orchester-app-mcp"


def discovery_dir(override, app):
    if override is not None:
        return Path(override)
    if app == NEW_APP:
        variable = os.environ.get(DISCOVERY_VARIABLE)
        if variable:
            return Path(variable)
        return new_app_user_dir()
    return lurq_user_dir()


def new_app_user_dir():
    if sys.platform == "win32":
        base = os.environ.get("LOCALAPPDATA")
        if base:
            return Path(base) / NEW_APP_FOLDER
    elif sys.platform == "darwin":
        base = os.environ.get("HOME")
        if base:
            return Path(base) / "Library" / "Application Support" / NEW_APP_FOLDER
    else:
        raise ClientError(f"{NEW_APP} has no user discovery folder on this platform; pass --discovery")
    raise ClientError(f"No {NEW_APP} discovery directory is configured; pass --discovery")


def lurq_user_dir():
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
        raise ClientError(
            f"Cannot read MCP discovery for PID {pid} in {directory} ({type(error).__name__})"
        ) from None
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


def decode_png_pixels(compressed, width, height):
    row_bytes = width * 4 + 1
    expected = row_bytes * height
    if expected > 128 * 1024 * 1024:
        raise ClientError("MCP screenshot PNG dimensions are too large")
    decoder = zlib.decompressobj()
    try:
        pixels = decoder.decompress(compressed, expected + 1)
    except zlib.error:
        raise ClientError("MCP screenshot PNG data cannot be decoded") from None
    if (
        len(pixels) != expected
        or not decoder.eof
        or decoder.unconsumed_tail
        or decoder.unused_data
        or any(pixels[offset] > 4 for offset in range(0, expected, row_bytes))
    ):
        raise ClientError("MCP screenshot PNG data is incomplete or invalid")


def png_dimensions(png):
    if len(png) > 64 * 1024 * 1024 or png[:8] != b"\x89PNG\r\n\x1a\n":
        raise ClientError("MCP screenshot data is not a PNG")
    offset = 8
    width = height = None
    idat = []
    after_idat = False
    while offset < len(png):
        if len(png) - offset < 12:
            raise ClientError("MCP screenshot PNG is incomplete")
        length = int.from_bytes(png[offset:offset + 4], "big")
        end = offset + 12 + length
        if end > len(png):
            raise ClientError("MCP screenshot PNG is incomplete")
        kind = png[offset + 4:offset + 8]
        data = png[offset + 8:end - 4]
        checksum = int.from_bytes(png[end - 4:end], "big")
        if zlib.crc32(kind + data) != checksum:
            raise ClientError("MCP screenshot PNG checksum failed")
        if width is None:
            if kind != b"IHDR" or length != 13:
                raise ClientError("MCP screenshot data is not a PNG")
            width = int.from_bytes(data[:4], "big")
            height = int.from_bytes(data[4:8], "big")
            if width == 0 or height == 0 or data[8:] != bytes((8, 6, 0, 0, 0)):
                raise ClientError("MCP screenshot has invalid PNG dimensions or format")
        elif kind == b"IDAT":
            if after_idat:
                raise ClientError("MCP screenshot PNG has noncontiguous image data")
            idat.append(data)
        elif kind == b"IEND":
            if length != 0 or not idat or end != len(png):
                raise ClientError("MCP screenshot PNG is incomplete or invalid")
            decode_png_pixels(b"".join(idat), width, height)
            return width, height
        elif kind == b"IHDR" or kind[0] & 0x20 == 0:
            raise ClientError("MCP screenshot PNG has an unsupported chunk")
        if kind != b"IDAT" and idat:
            after_idat = True
        offset = end
    raise ClientError("MCP screenshot PNG is incomplete")


def screenshot_png(result):
    if result.get("isError"):
        raise ClientError("MCP screenshot tool reported an error")
    content = result.get("content")
    if not isinstance(content, list) or len(content) != 1:
        raise ClientError("MCP screenshot must contain exactly one image block")
    image = content[0]
    if not isinstance(image, dict) or image.get("type") != "image" or image.get("mimeType") != "image/png":
        raise ClientError("MCP screenshot is not a PNG image block")
    encoded = image.get("data")
    if not isinstance(encoded, str) or not encoded:
        raise ClientError("MCP screenshot has no base64 image data")
    try:
        png = base64.b64decode(encoded, validate=True)
    except (binascii.Error, ValueError):
        raise ClientError("MCP screenshot has invalid base64 image data") from None
    width, height = png_dimensions(png)
    return png, width, height


def save_png(path, png):
    destination = Path(path)
    if destination.suffix.lower() != ".png":
        raise ClientError("Screenshot output path must end in .png")
    temporary = None
    try:
        destination.parent.mkdir(parents=True, exist_ok=True)
        with tempfile.NamedTemporaryFile(
            mode="wb", prefix=".lurq-screenshot-", suffix=".tmp", dir=destination.parent, delete=False
        ) as stream:
            temporary = Path(stream.name)
            stream.write(png)
            stream.flush()
            os.fsync(stream.fileno())
        os.replace(temporary, destination)
        if not destination.is_file() or destination.stat().st_size != len(png):
            raise ClientError("Screenshot PNG was not saved")
    except OSError as error:
        raise ClientError(f"Cannot save screenshot PNG ({type(error).__name__})") from None
    finally:
        if temporary is not None:
            try:
                temporary.unlink(missing_ok=True)
            except OSError:
                # A cleanup failure must not hide the original write result.
                pass


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("pid", type=int, help="PID of the app started by this run")
    parser.add_argument("tool", help="embedded lurq MCP tool name")
    parser.add_argument("--app", default="orchester", help="expected app name in discovery")
    parser.add_argument("--arguments", default="{}", help="JSON object of tool arguments; do not pass secrets")
    parser.add_argument(
        "--discovery",
        "--discovery-dir",
        dest="discovery",
        help=f"folder containing the exact-PID discovery file (default for {NEW_APP}: ${DISCOVERY_VARIABLE}, "
        "then the user folder)",
    )
    parser.add_argument("--image-out", help="run-owned PNG output path for lurq_screenshot")
    args = parser.parse_args(argv)
    if args.pid <= 0:
        parser.error("PID must be positive")
    try:
        arguments = json.loads(args.arguments)
        if not isinstance(arguments, dict):
            raise ClientError("Tool arguments must be a JSON object")
        if args.image_out is not None and args.tool != "lurq_screenshot":
            raise ClientError("--image-out is only valid with lurq_screenshot")
        directory = discovery_dir(args.discovery, args.app)
        url, token = load_discovery(args.pid, args.app, directory)
        result = call_tool(url, token, args.tool, arguments)
        if args.tool == "lurq_screenshot":
            png, width, height = screenshot_png(result)
            if args.image_out is not None:
                save_png(args.image_out, png)
            print(json.dumps({
                "ok": True,
                "saved": args.image_out is not None,
                "mime_type": "image/png",
                "bytes": len(png),
                "width": width,
                "height": height,
            }))
            return 0
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

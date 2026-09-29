# D4 preparation: exact-PID screenshot file handling (2026-09-29)

`tools/lurq-mcp-client.py` now handles `lurq_screenshot` image results without printing their base64 payload. Its optional
`--image-out` saves a PNG at the run-owned path supplied by the caller; without the option, it returns unsaved image
metadata only. The client requires exactly one `image/png` content block, valid base64, bounded PNG chunks with valid
checksums, IHDR/IDAT/IEND, and decodable RGBA8 scanlines. It rejects an MCP tool error and writes via a same-directory temporary file followed by an atomic replace.
Only a completed, size-checked write produces `saved:true`. Error output contains static diagnostics or exception type,
not response bodies, image data or the discovery credential.

The local synthetic MCP server in `tools/tests/test_lurq_mcp_client.py` exercised successful PNG writing and metadata,
unsaved metadata, tool error, missing or extra image blocks, wrong MIME, invalid base64, non-PNG data, an unwritable
destination, header-only and missing-IEND payloads, bad chunk CRC, corrupt IDAT, invalid row filters, and rejection of `--image-out` for another MCP tool. Existing exact-PID, redirect, HTTP error, SSE and
credential-redaction tests also passed: `python -B -m unittest discover -s tools/tests -p test_lurq_mcp_client.py -v`
reported **15/15 passed**.

`git diff --check` passed. `powershell -File tools/check.ps1` passed in this worktree with the warmed, isolated
`CARGO_TARGET_DIR=H:/projects/orchester/.tmp/targets/d3` and `CARGO_INCREMENTAL=0` (`check: OK`).

No desktop app, Pencil design, global input or screenshot was used for this synthetic transport check. A separate D4
verifier still needs a real run-owned desktop, exact-PID MCP screenshot and same-size Pencil comparison.

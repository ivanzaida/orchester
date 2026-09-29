# D7: credential-safe exact-PID MCP from a real run (2026-09-29)

## Client and independent review

`tools/lurq-mcp-client.py` reads the discovery file for a requested PID in process memory. It checks the app name
and exact loopback URL, disables proxies and HTTP redirects, performs MCP initialize/session/tool-call exchange,
rejects incomplete JSON-RPC replies, and prints only ASCII-safe JSON tool results with the discovery token redacted.
The client does not log its own discovery payload or request Authorization header; it redacts the exact token from tool results. Usage and the limit on other tool data
are in `docs/mcp-verification-client.md`.

Independent review found and retested four defects during development: a default HTTP redirect could forward the
bearer header; an incomplete JSON-RPC reply could look successful; lurq's SSE initialize response begins with an
empty heartbeat event; and Unicode UI text failed on a Windows ASCII stdout pipe. All were corrected before the
successful run. Nine synthetic protocol and redaction tests passed. The independent reviewer also passed
`git diff --check` and `powershell -File tools/check.ps1` on the final candidate. Its report is in
`.tmp/verify/d7/report.md`.

## Disposable Orchester runs

The test project was the disposable `.tmp/d3/project` clone, controlled by the separate no-focus app in
`.tmp/d3/app`. The clone received temporary L6 and D7 probe changes; the host app was an existing disposable
no-focus build. A host-side watcher observed foreground
PID 10132 throughout 2026-09-29 01:53:07–02:03:38 UTC, covering both runs below; the 200 ms watcher recorded no other foreground PID.

Run #3 built the MCP desktop and runner and pinned both through D6. Its disposable probe rejected the Windows
`\\?\` canonical path in its own containment guard before launching an app. Implement failed, Review was skipped,
and the run #3 probe did not read a discovery credential. The guard was fixed in the disposable clone; the active run worktree was not
changed.

Run #4 used the corrected probe. It built the desktop and runner with MCP (`build_exit=0`), immediately pinned both
in a run-owned directory with D6 (`pin_exit=0`), started only the pinned desktop, and passed its spawned PID to the
D7 client. `lurq_windows` returned `focused:false`, `lurq_read_tree` returned content, and the own desktop exited 0.
The pinned directory and run-owned MCP discovery file were absent after cleanup. Implement and a separate read-only
Review both passed; the disposable database records `succeeded` / *Ready for review* and 42,977 used tokens. The
agent's sanitized evidence was automatically committed only in the disposable run branch as
`7694792 DOS-4: Implement (run #4)`. No part of that disposable branch was merged, pushed or installed.

An exact-PID MCP request closed host app PID 44756 after run #4; its runner PID 52892 exited with it. Their
processes and host discovery file were absent two seconds later, and the foreground watcher was stopped. No stable
installation or unrelated process was stopped. The independent live report is `.tmp/verify/d7/live-report.md`.

## Coverage limit

This verifies D7's credential-safe client in a real Windows run. It does not verify a macOS runtime launch or the
Pencil/app screenshot comparison required by D4. The temporary L6 no-focus patch in the disposable clone is not
the pending lurq release.

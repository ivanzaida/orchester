# ORC-41 interactive Codex tool approvals · 2026-09-29

Status: in progress. The integrated candidate is `codex/current-integration`. Other Codex steps
continue to use `codex exec -a never`; an opted-in step uses a bidirectional App Server session
with granular MCP confirmation enabled and command, file, sandbox and permission escalation
requests disabled. A project-wide modal shows one persisted request with its agent, MCP server,
action and target, then sends Approve once or Deny to that live session. A request is claimed in
SQLite before its JSON-RPC response is written; interruption clears pending or dispatching
requests without replaying a decision.

Independent review found and the implementation fixed three issues: a response was initially sent
before the durable claim, an MCP elicitation could be associated with an ambiguous concurrent
tool, and freeform request text could reach the stored/displayed approval record. The reviewer
rechecked those paths and found no remaining blocker in the protocol and storage code.

Verification on Windows:

- Named filtered tests: App Server parser 5/5, tool-approval store 4/4, isolated invocation 1/1,
  active-time accounting 1/1 and desktop i18n 5/5.
- `powershell -File tools/check.ps1`: `check: OK` on the integrated worktree after Clippy fixes.
- Separate verifier used Pencil MCP node `ybPUT` and exact-PID candidate app MCP screenshots.
  The modal aligned at 600 × 350 logical pixels, with no blocking or major mismatch. It appeared
  over both Overview and Task detail in a disposable project. Local captures are in
  `.tmp/verify/tool-approval/`.
- Pencil full frame `t0q3ws` is 1440 × 960 logical pixels. The disposable Task detail fixture
  showed an interrupted offline run while the design uses illustrative failed-run content, so
  the unchanged full-screen background was not certified as a pixel match. The changed modal
  was compared region by region at the same logical size.

Remaining acceptance work:

- The App Server inherits the existing Codex sign-in and global MCP servers. The actual approval request shape
  emitted by Pencil `batch_design`, creation of frames and node IDs after approval, and unchanged
  design after rejection must be tested with a disposable active Pencil document. The parser
  fails closed on unsupported request shapes; synthetic protocol tests cannot replace this test.
- A disposable copy was briefly opened, but the user objected to opening that file. The test was
  stopped before `batch_design`; the main design was restored as the active Pencil document. No
  live approve/deny result is claimed.
- The approval design changes were saved to `data/orchester.pen`.
- The macOS runner cross-build stops in `libsqlite3-sys` and `zstd-sys` without a macOS C compiler
  on Windows. `orchester-core` passed `aarch64-apple-darwin` check; no Mac was available.

Codex App Server protocol reference: <https://developers.openai.com/codex/app-server>.

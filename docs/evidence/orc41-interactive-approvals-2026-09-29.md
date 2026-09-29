# ORC-41 interactive Codex tool approvals · 2026-09-29

Status: done. Independent verification passed and the user saved the main design. The verified
branch is `codex/current-integration`. Other Codex steps
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

- Named filtered tests: App Server parser 6/6, MCP selection/readiness 2/2, tool-approval store
  4/4, normal-profile invocation 1/1, active-time accounting 1/1 and desktop i18n 5/5.
- `powershell -File tools/check.ps1`: `check: OK` on the integrated worktree after Clippy fixes.
- Separate verifier used Pencil MCP node `ybPUT` and exact-PID candidate app MCP screenshots.
  The modal aligned at 600 × 350 logical pixels, with no blocking or major mismatch. It appeared
  over both Overview and Task detail in a disposable project. Local captures are in
  `.tmp/verify/tool-approval/`.
- Pencil full frame `t0q3ws` is 1440 × 960 logical pixels. The disposable Task detail fixture
  showed an interrupted offline run while the design uses illustrative failed-run content, so
  the unchanged full-screen background was not certified as a pixel match. The changed modal
  was compared region by region at the same logical size.
- The independent switch comparison found a major defect: its label appeared both in the
  setting row and inside the switch, wrapping the row across several lines. `Switch::hide_label`
  now keeps the control's name in the MCP tree while rendering only its track beside the row label.
  Rebuilt ON and OFF captures matched Pencil rows `lGbRY` and `usGTX` in single-line copy,
  alignment, typography, spacing and knob polarity. No blocking or major mismatch remained;
  the OFF capture retained a transient keyboard focus ring. The changed row was certified,
  not the illustrative fixture contents of the entire pipeline editor.
  Captures: `.tmp/verify/orc41-live-transport/step-approval-switch-on-fixed.png` and
  `step-approval-switch-off-fixed.png`. The final filtered i18n tests passed 5/5 and
  `powershell -File tools/check.ps1` returned `check: OK` after this fix.
- During a test-window restart the verifier omitted the no-focus environment flag and placed
  PID 32320 on the primary monitor. After the user's correction it closed only that owned
  process. Subsequent candidate PID 11100 started hidden with
  `ORCHESTER_MCP_START_WITHOUT_FOCUS=1`; a targeted `SetWindowPos` with `SWP_NOACTIVATE`
  placed it entirely on non-primary DISPLAY3 before showing it without activation.
  Exact-PID MCP reported `focused=false`, and the host foreground PID stayed 33284 before
  launch, after positioning and after MCP access. The installed stable app was untouched.

Real Codex verification:

- Codex 0.159 accepted the production granular approval policy and `approvalsReviewer=user`
  with the existing signed-in account. The normal global MCP servers remained configured;
  Pencil reported connected with nine tools. No Pencil tool was called.
- A disposable ordinary MCP server exposed one tool that writes a fixed marker. Both an
  explicit `default_tools_approval_mode=prompt` run and a run with no approval-mode override
  emitted `mcpServer/elicitation/request` after `item/started`. The request had an empty form,
  matching thread/turn/server identifiers and `_meta.codex_approval_kind=mcp_tool_call`.
  Denial produced `user rejected MCP tool call` in the same session and no marker write.
  Local protocol captures: `.tmp/verify/orc41-handshake/prompt-result.json` and `auto-result.json`.
- The independent verifier reached the real approval dialog through candidate desktop PID
  26680 and its runner, using the normal Codex account, inherited global MCP servers and no
  approval-mode override. Clicking **Deny** persisted `denied`, failed the step with
  `The requested tool action was denied`, released its runner slot and cleared the pending
  wait. Neither marker nor call log existed. The invocation recorded session
  `01a0ee56-5822-7fa2-b8e1-95a4532ddd33` and 38,636 ms of inactive approval-wait time.
- **Approve once** also passed through the same desktop and runner. Before approval the
  marker was absent and the request was pending; afterwards the same invocation retained
  session `01a0ee58-aafc-7031-8ed7-66f535491337`, the decision was `approved`, the pending wait
  cleared and the task completed. The marker contained exactly one `approved` line and the
  MCP trace contained exactly one `write_marker` call. The invocation recorded 37,508 ms of
  inactive approval-wait time. Both owned app and runner processes were then closed.
- These live runs exposed duplicate `session_started` activity: both the thread-start
  notification and response produced it. The parser now leaves session establishment to
  the response. The regression test passed, and the independent rebuilt run (desktop PID 58400,
  task TST-2) contained exactly one session-start event before and after approval. The same
  invocation retained session `01a0ee5e-43ce-7f80-9d09-309ce221502d`, completed successfully,
  wrote once, and recorded 29,582 ms of inactive approval-wait time. No blocking or major
  problem remained in the generic approval flow.
  Independent live-run artifacts are in `.tmp/verify/orc41-live-transport/`.
- A real Pencil `batch_design` call reached the Orchester modal with target
  `H:\projects\orchester\data\orchester.pen`. **Deny** persisted the decision and failed the
  step with `The requested tool action was denied`; the invocation recorded session
  `01a0ee6c-25c9-7bd2-8489-bc9ff8c7113c`, cleared its wait and excluded 42,448 ms of waiting
  from active time. The nine existing top-level IDs stayed unchanged and no test frame appeared.
- **Approve once** on the next real Pencil run retained session
  `01a0ee6e-03ce-7d91-acde-addf9f7c43ab`, completed successfully and recorded one session-start
  event. `batch_design` returned `ZdUiB`, and an independent read confirmed one new 64 × 48
  frame named `ORC41_VERIFY_20260929_2140`. The verifier removed only that exact ID after
  checking its name and type. The nine original top-level IDs were then restored exactly;
  the main document stayed active throughout. No blocking or major finding remained.
  Captures: `pencil-deny-pending.png` and `pencil-approve-pending.png` in the live-run folder.

The Pencil smoke used the already-open main design at the user's explicit direction; the user
rejected switching to the originally proposed disposable document. No existing design node or
variable was edited by the smoke. The new permanent switch designs were authored separately.

The user saved the verified modal and pipeline-step states to `data/orchester.pen` after the
temporary test node was removed. Full independent report and local screenshots:
`.tmp/verify/orc41-live-transport/report.md`. Both owned test app and runner were closed.
Foreground stayed unchanged during the final launch and test operations; the post-close sample
showed Explorer. The verifier did not activate or send input to another window.

Platform limitation:

- The macOS runner cross-build stops in `libsqlite3-sys` and `zstd-sys` without a macOS C compiler
  on Windows. `orchester-core` passed `aarch64-apple-darwin` check; no Mac was available.

Codex App Server protocol reference: <https://developers.openai.com/codex/app-server>.

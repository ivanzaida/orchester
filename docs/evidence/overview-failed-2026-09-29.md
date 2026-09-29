# Overview Failed tab — verification

Branch: `codex/overview-failed`. Verified on Windows on 2026-09-29.

The Failed count and rows use each task's latest run. Cancelled, unreadable, draft and
successful latest runs stay out; a newer successful run removes a task with an older failure.
The selected tab has its own route and the empty state says “No failed tasks.”

Independent checks passed in an isolated target directory:

- Overview filters: 10/10, plus the targeted newer-success regression 1/1 after it was added.
- Navigation route round-trip: 1/1; locale filters: 4/4; key parity: 1/1.
- `powershell -File tools/check.ps1` and `git diff --check` passed.

The separate UI verifier compared live Pencil nodes `pHNfB` (two failed rows) and `aSpMm`
(empty) with exact-PID embedded-MCP screenshots of a disposable 1440 × 960 app and project.
The tab, counts, row geometry, footer and empty state aligned within roughly 1–3 px, with no
blocking or major differences. Metrics, task IDs and relative ages differ because the Pencil
data is illustrative. The global Skills sidebar item is the accepted D5 exception; Search,
bell and the All pipelines affordance are earlier documented differences outside this change.

Captures are in `.tmp/worktrees/codex-overview-failed/.tmp/verify/overview-failed/`.
macOS execution was unavailable. The Pencil editor changes still need a Save to persist
`data/orchester.pen` before integration.

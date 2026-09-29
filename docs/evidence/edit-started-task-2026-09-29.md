# Editing a task after its run starts — verification

Branch: `codex/edit-started-task`. Verified on Windows on 2026-09-29.

The task form saves changes for future runs. Every existing run retains its title, brief,
criteria, pipeline and limits in a transactional snapshot; a legacy run receives a snapshot
before the first edit. The active run's Summary displays its saved brief and criteria.

Independent checks passed:

- Core `task edit`: 2/2; store `task_edits task_`: 5/5.
- Desktop `form_keyboard edit_task`: 4/4 before the Summary refinement; the changed
  started-task test passed twice after waiting for its asynchronously loaded brief.
- Runner run-again and reconnect filters: 1/1 each.
- `powershell -File tools/check.ps1` and `git diff --check` passed.

The separate UI verifier compared Pencil nodes `uv4PY`, `dRN4m` and `E1o77O` with
exact-PID embedded-MCP screenshots of a disposable 1440 × 960 app and project. The running
Summary, started edit form and draft edit form had no blocking or major differences. After
saving an edit, the running Summary showed the original brief and the edit form showed the
new brief. Differences in run-step count came from illustrative design data. The global Skills
sidebar item is the previously accepted D5 design exception.

Captures are in `.tmp/worktrees/codex-edit-started-task/.tmp/verify/edit-started-task/`.
macOS execution was unavailable. The Pencil editor changes still need a Save to persist
`data/orchester.pen` before integration.

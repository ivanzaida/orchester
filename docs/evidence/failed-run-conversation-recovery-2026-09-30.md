# Continue a failed run through its conversation

Branch: `codex/continue-failed-run`, based on `6626cc4`.

## Problem

ORC-44's agent reported `failed` for a decision the user could resolve. The prompt correction in
`6626cc4` tells agents to ask a question for such decisions, but existing failed runs still closed the
conversation and required **Run again**, which starts a fresh run. The user's code was saved; the
missing capability was continuing that same work through a message.

## Change

- A recoverable Failed conversation accepts **Send & continue**. A message deliberately identifies
  the failed run version visible when it was sent, rather than relying on timestamps.
- The runner records a new `recovery` attempt of the same run. It keeps previous attempts, messages,
  checkpoints, branch, saved/uncommitted work and available provider sessions. Only the latest run
  of the task can recover. Passed upstream work remains passed; the selected agent, dependent and
  unfinished steps execute again, so failed/reviewed outputs do not silently become successful.
- Orchestrator chooses the failed agent, or its upstream agent if a command failed. Direct recipients
  are limited to previously executed agents. Recovery prompts include saved progress, new instructions
  and current dependency results. A missing session starts fresh in the surviving workspace.
- Parallel recovery reuses an existing step worktree and removes it by its original owner only after
  merging. Session lookup walks past attempts/invocations without a reported session.
- Duplicate notifications, old ordinary queued messages and stale requests from an earlier failure
  cannot create automatic retry loops. Equal timestamps and usage recorded after failure are handled.
  A deliberate request survives an offline runner and is processed on reconnect.
- Removed worktrees are not recreated. Cancelled/merged runs stay closed. Budgets, permissions and
  pinned configuration are unchanged. An approval request still requires a supported provider approval
  flow; this feature does not interpret arbitrary user text as a blanket permissions grant.

The application database was not changed by hand and ORC-44 was not silently restarted. Its
`orchester/orc-44-run-1` branch and checkpoint remain the user's saved work.

## Verification

Independent verifier: `/root/orc41_independent_verifier`; implementation by root only. Tests and
candidate UI use disposable repositories and separate app data. The stable app is not a fixture.

Completed Windows checks:

- Core `--test run failed_recovery`: 3 passed.
- Core `--test run outcome_prompt`: 3 passed.
- Runner `--features test-agent --test failed_recovery failed_recovery`: 6 passed.
- Runner `--features test-agent --test conversation conversation`: 13 passed.
- Desktop `--test conversation conversation`: 9 passed.
- Desktop `--test form_keyboard conversation`: 5 passed, including Ctrl+Enter recovery from Failed.
- Store `--test conversation conversation_recovery`: 2 passed, including migration of an existing failed run.
- Desktop i18n `locale` and `every_key`: 5 passed.
- Final `powershell -File tools/check.ps1`: `check: OK` (format, all-target clippy, file limits, UI literals).

Release build of desktop and runner passed using the isolated `.tmp/stable-build` target.

Independent live UI comparison passed in owned PID 49380, on the secondary monitor with `focused=false`.
The foreground PID was unchanged. The changed Failed status, recipient, composer, **Send & continue**
action and recovery explanation matched the live Pencil frame without blocking or major differences.
The retained-worktree path and the [closed composer after removing the fixture worktree](failed-run-recovery/worktree-removed.png)
were exercised through exact-PID MCP. The latter kept the Failed badge and visibly disabled sending.

The fixture had one Implement step and no prior messages; the design's five steps and conversation are
illustrative. Existing Skills navigation and the locked Orchestrator icon differ from the older shared
sidebar in this frame; this change did not modify that sidebar. The command-only FailedNoAgent state
was not live exercised. The [independent report](failed-run-recovery/verifier.md) records all 46 passing
filtered tests, final gate, captures and limitations. These runner tests use fake provider processes;
a live model was not invoked.

The `aarch64-apple-darwin` core check passed. Store/runner cross-check stopped in `libsqlite3-sys`
because this Windows host lacks the target C compiler (`ToolNotFound`). No Mac runtime test was run. The first headless run used a success-only wait helper for a deliberately
failed fixture; the test wait was corrected to the Failed notice before exercising recovery.

## Design

Main Pencil document `data/orchester.pen`, frame `vUPqr` (**02.S6 · Failed run / Send and continue**).
The existing task conversation keeps its layout; the failed status, action and explanation describe
recovery. Durable exports of the live [Pencil design](failed-run-recovery/design.png) and the
[candidate content screenshot](failed-run-recovery/app.png) are included for review. The app content
is normalized from 1.5 DPI and excludes the existing 32px title bar and 28px status bar.

The new Pencil frame is still in the open main editor; its Save to `data/orchester.pen` was requested
because Pencil MCP cannot save. Its verified export is committed independently of that pending Save.
No worktree design document was opened.

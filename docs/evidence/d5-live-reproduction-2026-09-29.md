# D5 build-space preflight · live reproduction (2026-09-29)

The current-master D5 candidate is `codex/d5-live-repro`, based on `0cae412`. It combines the earlier unmerged
host-side build-space preflight with registry lurq 0.30.3's no-focus verifier startup. The final complete binary
diff, including untracked source/test files, had SHA-256
`bd437c3f5c57751b96a153d853463d6350419f11b632d396e2c9b26ce42eabb5` before and after independent
verification. The candidate has **not** been committed or merged.

## Checks

- Filtered core build-space tests 2/2, desktop preflight-message tests 2/2, runner environment test 1/1, New Task
  headless tests 9/9, a real-runner same-task retry test 1/1, ambiguous-start tests 2/2, and i18n tests 5/5 passed.
  The implementer also passed desktop MCP and runner `cargo check --locked`, `git diff --check`, and
  `powershell -File tools/check.ps1` (`check: OK`). These runs occurred across the D5 candidate milestones; the
  focused regression and final check passed after the last code change.
- An independent reviewer compared the port with the earlier `codex/d5-free-space` candidate and found no lost
  functionality. A later review caught a false assertion for a lost runner reply. The final review found no code
  blocker: `Lost` and `Rejected(Failed)` now keep a typed, monotonic *run status unconfirmed* state, including after a
  later definite rejection. The retry uses the same task ID and idempotency key.
- The independent verifier built its own locked MCP desktop and runner from the final candidate before launching
  pinned copies. It did not run the filtered tests or `check.ps1` itself.

## Disposable app evidence

The verifier used only scratch Git projects and databases under `.tmp/verify/d5-live*/`. A tracked regular file
`not-a-directory` made the project's absolute `CARGO_TARGET_DIR` point beneath a file, so the host could not measure
its volume. All interaction used the verifier's exact-PID embedded MCP endpoint; no global input, whole-screen
capture, stable installation, or user database was touched.

- [First run](../../.tmp/verify/d5-live/report.md): the localized `BuildSpaceUnavailable` error was fully visible,
  with no run or worktree, but the form falsely said *Not saved yet* after the task had persisted as queued. This
  major mismatch prompted the candidate fix.
- [Second run](../../.tmp/verify/d5-live-v2/report.md): the queued state and missing run were shown truthfully;
  correcting the scratch target and retrying created exactly one run for the **same** task, which reached Ready for
  review. A click still opened a muted pipeline menu on the supposedly frozen form, prompting a second fix.
- [Final run](../../.tmp/verify/d5-live-v3/report.md): the verifier independently matched the final diff hash and
  built pinned binaries. The invalid target left one new task queued, no new run or worktree, and a full readable
  error. The topbar said `Queued · run not started`; Save draft was absent. An ordinary exact-PID click on the
  pipeline value opened no menu, changed no database value, and left a pixel-identical 2160×1530 capture. The host
  foreground watcher passed with 529 samples; the verifier's app PID was never observed in front. Its own app and
  watcher exited. The final run did not repeat the successful retry or force an ambiguous transport loss live;
  those paths have the second run and filtered regressions, respectively.
- [Low-space run](../../.tmp/verify/d5-live-low/report.md): a different independent verifier reused the pinned final
  binaries and confirmed the full candidate diff was unchanged. Its disposable project used an absolute, nonexistent
  `CARGO_TARGET_DIR` on H:, with 2.643 GiB free immediately before one Create & run action. The fully readable
  localized error showed `2.6 GiB free; at least 10.0 GiB` in a 2160 × 1530
  [app screenshot](../../.tmp/verify/d5-live-low/app-error-neutral.png). One new task (#13) remained queued; runs
  stayed at 11, Git worktrees at one, and the target directory was not created. `Retry run` appeared and Save draft
  was absent. The exact-PID app never gained foreground focus across 1,943 host samples. The verifier closed only
  its own app and watcher. This run did not repeat Cargo tests or `check.ps1`.

Pencil has no exact New Task frame for either the cannot-measure or low-space error. The canonical frame is a
different state, so a same-state Pencil/app comparison is **unavailable**, not passed. The `fs4` dependency passed
an aarch64-apple-darwin check in the earlier D5 review, but a full runner cross-check needs macOS SDK headers or a
Mac; live macOS behavior was not run. D5 remains in progress.

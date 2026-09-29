# U2 · Merge and worktree controls

Date: 2026-09-29. Branch: `codex/u2-merged-fix`. Independent verdict: **pass** for the saved Pencil states reached in the app.

The candidate keeps merge and worktree-removal jobs scoped to the project and run, including while the page is
unmounted. Conflicting actions remain unavailable while a job runs, the `Merging…` button keeps its width, and a
merged run preserves its historical branch name while showing a separate, fully readable localized `· deleted`
suffix. The branch part may ellipsize when its generated name is long.

An independent verifier applied the candidate patch to a detached worktree and SHA-256 compared all 16 changed
source, locale and test files: zero differences. In its own target directory, the earlier full integration filter
passed 11/11. On the corrected candidate, the real-Git merge test passed 1/1, i18n passed 5/5, and
`powershell -File tools/check.ps1` reported `check: OK`. The implementing worktree also passed the full 11-test
integration filter, i18n 5/5, nightly formatting and `check.ps1` after the suffix fix.

The verifier confirmed the active encrypted Pencil document through MCP and compared saved `jnagB` (clean removal
dialog), `cE50X` (Merging button) and `bMJUA` (merged page) with exact-PID app captures at matching state and size.
The corrected merged page and its Pencil frame were both 2160 × 1440 physical px. The `· deleted` suffix is complete
in app pixels and the next property row has not shifted. The shell, card, details panel and footer have no blocking
or major mismatch; prior 2–4 px vertical offsets are minor. The full in-flight merging page lacks a saved Pencil
frame; only its button specimen could be compared. Fixture names, stage counts and text are illustrative design
data and differ from the real disposable Git run.

The final source-matched app ran as PID 34368. A host watcher started before launch and collected 381 samples at
200 ms, with zero samples at that PID and a 204 ms maximum gap; the bounded verdict was pass. All app interaction,
screenshots and close requests used its embedded MCP endpoint. No installed stable-app process or data was touched.

Detailed reports and captures: `.tmp/verify/u2-final-port/independent-verification.md` and
`.tmp/verify/u2-merged-fix/report.md`. The final comparison is
`.tmp/verify/u2-merged-fix/runtime/run1/app-merged-content-1_5x.png` against
`.tmp/verify/u2-merged-fix/design-1_5x/bMJUA.png`.

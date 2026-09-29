# D5 final verification · 2026-09-29

Candidate: `codex/d5-final-port`, rebased onto `62990f5` (including ORC-15 and ORC-16). An independent agent tested
the source at `a6bd601` in a separate verifier worktree. Its full report, screenshots, trees, test log, and process
timeline are in `.tmp/verify/d5-final-port/report.md` (local, ignored evidence).

## Result

- Seventeen named-filter Windows test commands passed independently: the original 15 affected D5 filters, all seven
  Run again integration tests in one filtered command, and a separate pinned-pipeline repeat command. These cover
  core free-space and queued-task rules, store persistence and edit refusal, runner environment and pinned pipeline,
  desktop queued retry, recovery after navigation, pipeline replay, messages, navigation, and i18n. The verifier's
  `powershell -File tools/check.ps1` ended `check: OK`.
- The exact-PID embedded MCP app reached the saved Pencil nodes `WOdDe` (cannot measure), `WmW1G` (low space),
  `l801hD` (retry pending), and `T6bVyU` (queued task reopened). Each app content capture and live Pencil export
  was 2160 × 1440 in the same route and state. The independent comparison found no blocking or major mismatch in
  the D5 form, error, or Retry regions. The ORC-15 sidebar exception is documented below.
  Longer local paths scroll the plan above pinned Retry and error actions; the verifier scrolled the approval card
  into view.
- A blocked absolute Cargo target produced a readable measurement error before any run or additional worktree.
  A test-agent-only synthetic 2.6 GiB measurement produced the low-space UI and left one queued task, zero runs,
  and no additional worktree. The synthetic value verifies the display, not real low-space volume measurement.
- After publishing pipeline v2, Retry created exactly one run with the queued task's pinned v1 snapshot. Reopening
  a queued task after leaving its form showed locked saved details and a generic recovery note, without claiming
  that the earlier specific error was persisted.
- The ORC-16 integration fixes were covered by all seven Run again tests: Run again selects the latest published
  pipeline, checks build space after confirmed old-worktree cleanup, and returns a recorded run on repeated keys;
  pending recorded runs recheck admission and build space before another worktree is created.

## Limits recorded at closure

All four older D5 Pencil frames omit the Skills sidebar item introduced by merged ORC-15, shifting lower navigation
items by about 63 physical pixels in the app. The independent verifier classified this as a major global
design-source difference outside D5's form, error, and Retry regions. On 2026-09-29 the user explicitly accepted
documenting it as a D5 exception and merging without editing the ORC-15 design. Literal full-frame parity remains
different in that sidebar; no Pencil node was changed for this exception.

The full macOS runner cross-check was unavailable on this Windows host because macOS SDK headers were missing; no
Mac was available. The user explicitly accepted closing D5 with macOS behavior unverified and requested a separate
macOS verification task. Task wording and acceptance criteria were supplied for entry in the stable Orchester app;
creation through its MCP was not completed because that running process had no discovery file and the user requested
the text instead.

The host focus check had mixed results. Before rebase, two exact-PID verifier app runs recorded their own main window
as foreground for short intervals (10/850 and 21/305 samples); a third direct low-space run recorded 0/130. After
rebase, one interval recorded 112/426 own foreground samples, while another recorded 0/250. The onset in the first
post-rebase interval occurred while Overview was still shown, before form fill. Source review found no D5,
Orchester, or lurq activation call in this path. These observations establish intermittent failure of the no-focus
gate without proving a D5 cause. No focus change was made without a reproducible cause.

The first queued-and-starting moment ended before an MCP screenshot could be taken. The direct error-to-Retry
pending state was captured and compared with `l801hD`.

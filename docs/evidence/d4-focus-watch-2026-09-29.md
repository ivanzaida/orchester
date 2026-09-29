# D4 host foreground watcher preparation · 2026-09-29

This branch adds a read-only host observer for the remaining D4 focus check. It does not launch
Orchester, touch the stable install, send input, capture pixels, or read MCP discovery files.
The watcher records only UTC and monotonic timestamps with the foreground process ID. A later
`judge` compares the complete timeline with the PID from an `app` marker, recorded when the
run writes its own pinned app PID into a run-owned file immediately after launch. The timeline
starts before app launch and reaches a host-created stop file after that app exits;
the operator keeps the app start/exit times beside it. Usage is in
`docs/verification/foreground-watch.md`.

The verdict is deliberately narrow. Seeing the own app PID is `fail`. Missing/null PID, sampler
error, timing gap, timeout or interruption is `inconclusive`. `pass` means only that the own PID
was absent at sampled instants after the app marker; 200 ms sampling cannot prove no shorter
focus grab. The app's `lurq_windows` result should independently say `focused:false` during the
Verify step.

The Windows sampler reads `GetForegroundWindow` and `GetWindowThreadProcessId` through Python
`ctypes`. The macOS sampler reads AppKit `NSWorkspace.frontmostApplication.processIdentifier`
through `/usr/bin/osascript`, with an explicit inconclusive result if the interpreter/API fails.
macOS runtime behavior has not been checked on a Mac. Neither sampler enumerates windows or
inspects another process's contents.

## Checks

- `python -B -m unittest tools.tests.test_foreground_watch -v`: 11 synthetic tests passed.
- `git diff --check`: passed after adding new files as intent-to-add.
- `CARGO_TARGET_DIR=H:/projects/orchester/.tmp/targets/d3` with
  `powershell -File tools/check.ps1`: passed (`check: OK`).
- Windows host smoke of the watcher CLI, without an Orchester app: nine PID samples,
  no null samples, app marker from a host-owned test PID, stop-file exit 0 and `judge` pass.
  This tests the watcher protocol and Win32 sampler; it is **not** a D4 no-focus verification.

The tests cover a complete observed interval, own-PID failure precedence, null/error/gap/timeout
inconclusive results, missing/late/changed app markers, malformed timeline, exit codes,
start-before-sample order and sampler error recording. No L6 release or complete D4 Pencil/app
Verify run was performed in this branch; D4 remains open in `docs/plan.md`.

An independent read-only reviewer reran the 11 filtered Python tests and `git diff --check`,
reviewed the verdict logic, and sampled the Windows host with a separate own test process:
11 PID samples, none null, with a stop-file `pass`. This confirms the watcher protocol and
Win32 sampler, not no-focus behaviour of Orchester. The local report is
`.tmp/verify/d4-focus-watch/report.md`. The reviewer did not run macOS or the full D4 Verify step.

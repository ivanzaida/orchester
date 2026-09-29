# U6 conversation states and direct recipient timing (2026-09-29)

The independent verifier compared fresh Pencil exports with app screenshots made through the embedded MCP server
of its own exact-PID desktop and runner. Interrupted `sYuiX`, Not started `Ok14G`, and the recipient menu `cj2a2`
were compared at 2160 × 1440. Paused `s5YQd` and Stopped `I0O909` were compared as 661 × 1113 panel crops. The
verifier reported no blocking or major visual mismatch in those saved states. It also reached Live through a real
runner invocation, rather than changing a stored status. Fixture text and small vertical offsets were minor.

The first Paused capture exposed a false promise: a message addressed to Reviewer while Builder was paused said it
would arrive before the next action on resume. The runner instead delivers a direct message when its named agent
next runs. The corrected entry shows *Queued · waiting for this agent’s next turn* for a later agent. The independent
agent then exercised both Paused Builder→Reviewer and a genuine Live Builder→Reviewer state in the app. The entry
tree and screenshots showed the corrected line, fitting the saved component geometry without a blocking or major
visual mismatch. The saved Pencil component depicts a message to the current agent; there is no distinct
later-recipient frame, so literal same-state design comparison of that one line remains unavailable. The product
rule and this design limitation are recorded in `docs/design/orchester-workspace.md`.

The verifier matched the seven corrected source and test files by SHA-256. Its separate private build passed the
named conversation tests **8/8**, i18n **5/5**, and `powershell -File tools/check.ps1` (`check: OK`). A host watcher
sampled foreground ownership **1,343** times; neither own app took focus. An unrelated scratch Orchester started
by Claude became foreground during the later Live interval, which the verifier attributed by executable path.
Both of the verifier's app processes, runner and watcher exited. No installed stable process was controlled.

Detailed screenshots, Pencil exports, pixel comparison, exact PIDs, and the initial and corrected source evidence:
`.tmp/verify/u6-final-port/independent-report.md` and `.tmp/verify/u6-post-fix/report.md`. No macOS runtime or
unfiltered test suite was run for U6.

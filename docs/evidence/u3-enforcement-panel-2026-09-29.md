# U3 · Claude enforcement panel (2026-09-29)

The English repository mechanism now reads `edit tools removed · listed cmds run` and stays on one line in the
300-wide panel. Dedicated theme roles set the setting name to 12 px with 1.25 leading and the mechanism to
10 px Geist Mono with 1.3 leading. The roles also restore the setting-row height used by the saved design.

An independent verifier copied the four changed product files byte-for-byte into a detached checkout, built its
own desktop and runner, and used a typed store fixture with persisted Claude network Off, three allowed command
prefixes and one selected MCP server. It confirmed the active Pencil document through the Pencil MCP and compared
node `CIeqc` with the exact-PID app at the same state and 1.5× scale, region by region. No blocking or major
mismatch remained. The app panel measured 300 × 492.7 logical px versus Pencil's 300 × 495; rows aligned within
0.7 px. The 2.3 px height residual and the existing heading suffix `CLI` are minor. This check covers the
enforcement panel, not the surrounding Agents screen or a macOS runtime.

Independent named tests passed: the enforcement typography test 1/1, locale tests 4/4 and key-parity test 1/1.
`powershell -File tools/check.ps1` returned `check: OK`. A separate bounded replay with the pinned binaries
captured an identical panel and passed a host foreground watcher; the own app PID was absent at observed instants.
The first longer watcher interval was inconclusive because of a sampling gap, and was not used for the focus pass.

Full measurements, screenshots, source hashes and watcher timelines are in `.tmp/verify/u3-typography/report.md`.
The design source is encrypted and was accessed only through the Pencil MCP. The installed stable app was untouched.

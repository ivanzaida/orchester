# M3 · MCP servers in Secrets “Used by”

Secrets now count MCP server variable and bearer references once per server, show MCP servers
alongside tools in *Used by*, and include both kinds of consumer in the archive warning. Agent
profiles are not counted as secret users.

An independent verifier exercised MCP-only and mixed-use states in a disposable project. The
saved Pencil frames `wPXgW`, `R2ImH`, and `RxpL1` were exported through Pencil MCP and compared
with exact-PID app captures at 2160 × 1440. The archive dialog geometry matched exactly; no
blocking or major mismatch remained. Exports of all three frames after the later design commit
were byte-for-byte identical to those used for verification.

The final isolated Add Secret → Staging → masked value → Archive flow kept the test app out of
the foreground in 944 PID/HWND/class samples, including an 85-second idle tail. Earlier test runs
did record brief foreground episodes during taskbar switching among multiple applications; their
cause was not proven. This result applies to the bounded MCP flow and does not erase those logs.

The current-master port preserved all seven Rust source files byte-for-byte from the verified
candidate; its English locale incorporates intervening master keys. Two Secrets tests and five
i18n tests passed, as did `tools/check.ps1`. Full evidence, including the focus timelines and
comparison limits, is in `.tmp/verify/m3-geometry/report.md` and `.tmp/verify/m3/report.md`.

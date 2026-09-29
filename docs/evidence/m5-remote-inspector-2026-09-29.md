# M5 · Remote MCP inspector (2026-09-29)

The remote server inspector now names who reaches it: the agent's CLI over HTTP. Property values align at the top
of their 20 px rows. A long Authentication value uses a dedicated theme role (11 px text, 1.6 leading), wraps
right-aligned, and grows its row; the other properties keep their one-line treatment.

An independent agent copied the five changed Rust files byte-for-byte into a separate verifier checkout, built its
own desktop and runner with registry lurq 0.30.3, and used a disposable project with a real stored remote server.
The active encrypted Pencil document was read only through its MCP. The saved `a4lm6D` design and exact-PID app
content were both captured at 2160 × 1440. The Authentication value occupied two full lines without clipping; the
Discovered tools section started 2 physical px below the design, compared with the prior 22–23 px displacement.
The inspector's existing 3 px top offset and illustrative fixture differences are minor. No blocking or major
visual mismatch remained.

The earlier roughly 4 px Arguments/Variables text offset in the separate MCP dialog was outside this remote
inspector frame and was not rechecked here.

Independent named tests passed for long/short Authentication row height and the typography role. The implementer
also passed the MCP details test; `powershell -File tools/check.ps1` returned `check: OK` in both checkouts. The
verifier's own app stayed out of the foreground in a bounded 100 ms host watcher with no sampling gap over 110 ms,
and `lurq_windows` reported `focused:false`. No macOS runtime was run; the attempted all-features macOS cross-check
stopped at a missing target C compiler in `libsqlite3-sys`, before source checking.

Full measurements, screenshots, source hashes and watcher data are in `.tmp/verify/m5-wrap-auth-verify/report.md`.
The stable installed app was untouched.

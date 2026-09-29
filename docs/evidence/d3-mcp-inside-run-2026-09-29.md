# D3: embedded MCP from a desktop launched inside a real Orchester run (2026-09-29)

## Isolation

This continuation of [the first D3 runs](d3-first-real-run-2026-09-29.md) used a disposable clone at
`.tmp/d3-mcp/project` and an app copy at `.tmp/d3-mcp/app`. The D3 Implement and Review profiles and pipeline came
from copies of the earlier disposable databases; the copied app's project path was changed to the new clone. The user
checkout, stable installation and original D3 databases were not changed. The documentation change lives on
`codex/d3-mcp-inside-run`, a separate repository-local worktree.

The clone alone received a temporary L6 no-focus app opt-in and a local lurq 0.30.3 path patch. Its `master` had three
setup commits ending at `e1c9757`; none belongs to the Orchester repository's `master`. A small committed disposable
`tools/d3_mcp_probe.py` launches only the built desktop and runner, reads its MCP discovery credential in process
memory, calls `lurq_windows` and `lurq_read_tree` for the exact spawned PID, and closes that PID. It never prints a
credential or takes a screenshot.

The host app copy started as PID 42012 with `ORCHESTER_MCP_START_WITHOUT_FOCUS=1`. Before and after launch, Windows
reported foreground PID 10132 (`Chrome_WidgetWin_1`); its own MCP `lurq_windows` reported `focused:false`. The local
runner was online. After the runs completed, an exact-PID `request_close` ended this app and its runner; PID 42012
and its MCP discovery file were absent three seconds later.

## Setup and run 3: failures kept visible

The first Create & run request could not create a worktree because the fresh clone did not ignore `.orchester/`.
Adding `.orchester/` to that clone's local `.git/info/exclude` let the same idempotent request start run 3. This
requirement is now recorded in D1.

Run 3 created `orchester/dos-3-run-1`, but the first build stopped before compilation:

```text
cargo build -p orchester-runner -p orchester-desktop --features orchester-desktop/mcp
exit 101: failed to load source for dependency lurq
```

The clone's relative `[patch.crates-io]` path resolved from the run worktree to a nonexistent
`.orchester/worktrees/codex-lurq-no-focus/crates/lurq`. The probe then exited 1 because no binaries had been built;
MCP success was not claimed. Implement reported failure and Review was skipped. The run used 42,968 billable tokens.
The clone's patch path was changed to the absolute local L6 lurq checkout before the next run; no source patch was
made in the user checkout.

## Run 4: build and exact-PID MCP

Run 4 used `orchester/dos-4-run-1` from the corrected disposable clone. Its project environment set
`CARGO_TARGET_DIR=H:/projects/orchester/.tmp/targets/d3` and `CARGO_INCREMENTAL=0`; this was the idle, warmed target
already allowed by the Implement profile. The first shell attempt piped cargo to `Tee-Object` before `.tmp/` existed.
PowerShell returned exit 0 from the pipeline although the output file could not be opened. The agent identified that
false success, created `.tmp/`, and repeated the build. The repeated command finished successfully in 52.75 seconds:

```powershell
New-Item -ItemType Directory -Force .tmp | Out-Null
$env:CARGO_TARGET_DIR='H:/projects/orchester/.tmp/targets/d3'
$env:CARGO_INCREMENTAL='0'
cargo build -p orchester-runner -p orchester-desktop --features orchester-desktop/mcp
```

The first probe launched its own desktop PID 43308, but exited 1 after 20 seconds without finding that PID's MCP
discovery file. The app log showed the MCP server listening and `Access is denied` while writing discovery under the
real `%LOCALAPPDATA%`. The probe terminated only its own child. A diagnostic launch confirmed the same failure.

The next probe gave both the Python client and its child app a writable, run-owned `LOCALAPPDATA`:

```powershell
New-Item -ItemType Directory -Force .tmp/localappdata | Out-Null
$env:CARGO_TARGET_DIR='H:/projects/orchester/.tmp/targets/d3'
$env:CARGO_INCREMENTAL='0'
$env:LOCALAPPDATA=(Resolve-Path .tmp/localappdata).Path
python tools/d3_mcp_probe.py
```

It exited 0 and reported:

```text
own desktop PID: 46808
foreground PID: 0 -> 0; own MCP window focused=false
MCP lurq_windows and lurq_read_tree succeeded
own desktop exited with code 0
```

The exact-PID MCP call and `focused:false` are verified. `GetForegroundWindow` returned PID 0 both times inside the
Codex Windows sandbox, so that line does **not** prove which host window was foreground during the in-run launch.
The earlier host app startup checked foreground PID 10132 before and after, but was a separate launch. D4 carries a
host-side focus check for future UI verification.

`powershell -File tools/check.ps1` ran inside run 4 with the warmed target and returned exit 0, `check: OK` (its cargo
check phase took 32.05 seconds). Implement passed after 361.5 seconds, then a separate read-only Review passed after
79.0 seconds. The run finished `succeeded` / Ready for review with 108,283 billable tokens. Orchester automatically
committed the disposable agent evidence as `d027257 DOS-4: Implement (run #4)`. Nothing was pushed, installed or
merged from the disposable project. The earlier D3 run, documented separately, contains the two named runner
process/pipe tests; run 4 did not repeat them.

## Remaining verification setup

The shared target's `debug/orchester.exe` timestamp changed to 04:03:57 local time after the successful in-run probe
at about 04:00. Two later host-launched replays of that path exited 1 with no MCP discovery, so they are
inconclusive. The timestamp does not identify which process rebuilt the binary. D6 records the need to pin or copy
the just-built desktop and runner before a UI verification launch.

During diagnosis, the Implement agent read the disposable host app's discovery JSON with a shell command. Its command
output contained the ephemeral bearer token in the disposable run transcript. No token value is copied here or into
the repository. The host app process exited and its discovery file was removed after the run, invalidating that token.
D7 records the need for a reusable client that reads discovery only in memory. D4 also needs the pending L6 release
and a writable run-owned discovery location.

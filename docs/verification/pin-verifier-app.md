# Pin a UI verifier's app

Build the desktop (every build includes the embedded MCP server) and runner, then pin both before launching either program. The
helper creates a unique app directory beneath the run's own scratch directory and prints the
absolute desktop path. The runner is copied beside it, where the desktop looks for it. The
verifier must launch the printed path, never `target/debug/orchester` after pinning.

Windows, from the run worktree:

```powershell
cargo build --locked -p orchester-desktop -p orchester-runner
if ($LASTEXITCODE -ne 0) { throw 'app build failed' }
$metadata = cargo metadata --format-version 1 --no-deps
if ($LASTEXITCODE -ne 0) { throw 'cargo metadata failed' }
$target = ($metadata | ConvertFrom-Json).target_directory
New-Item -ItemType Directory -Force .tmp/verification | Out-Null
$desktop = cargo run --quiet --locked -p orchester-exec --bin pin-verifier-app -- "$target/debug" '.tmp/verification'
if ($LASTEXITCODE -ne 0) { throw 'app pinning failed' }
# Pass $desktop to the exact-PID MCP client. Do not start it via cargo run.
```

macOS, from the run worktree:

```sh
cargo build --locked -p orchester-desktop -p orchester-runner
if [ "$?" -ne 0 ]; then echo 'app build failed' >&2; exit 1; fi
target=$(cargo metadata --format-version 1 --no-deps | python3 -c 'import json, sys; print(json.load(sys.stdin)["target_directory"])')
mkdir -p .tmp/verification
desktop=$(cargo run --quiet --locked -p orchester-exec --bin pin-verifier-app -- \
  "$target/debug" .tmp/verification)
if [ "$?" -ne 0 ]; then echo 'app pinning failed' >&2; exit 1; fi
# Pass "$desktop" to the exact-PID MCP client. Do not start it via cargo run.
```

The helper opens the built files, hashes each before and after copying, compares the copies,
then checks the source paths again. A changed source makes pinning fail and removes its partial
copy; rebuild and retry. Once pinning succeeds, later builds in the Cargo target directory cannot
replace either pinned file. Each call creates a fresh directory so an earlier verifier keeps
its own pair. The helper preserves executable permissions on macOS.

The source directory must be the Cargo profile directory containing both programs (`debug` in
the examples). The caller creates and owns `.tmp/verification`, keeps it for the verifier's
lifetime, and removes it after its exact-PID app and runner have exited. Do not use the stable
installation as either source or destination. The helper cannot identify which build produced
files that were already present before it opened them; invoke it immediately after the intended
build succeeds, and confirm the expected build through the exact-PID client. It does not
launch an app, manage credentials, or check foreground focus.
If stdout closes after a successful copy, the caller may need to remove that unused run-owned
directory during scratch cleanup.

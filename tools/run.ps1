# Builds the runner and runs the desktop app. The desktop starts `orchester-runner` from its own
# directory, and `cargo run -p orchester-desktop` alone does not build another package's binary.
# Arguments go to `cargo run`, e.g.: powershell -File tools/run.ps1 --features mcp
$ErrorActionPreference = "Stop"

Push-Location (Split-Path -Parent $PSScriptRoot)
try {
    cargo build -p orchester-runner
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
    cargo run -p orchester-desktop @args
    exit $LASTEXITCODE
}
finally {
    Pop-Location
}

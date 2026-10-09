# Runs a desktop app.
#
# - The deprecated desktop (default): builds the runner and runs `orchester-desktop`, which starts
#   `orchester-runner` from its own directory; `cargo run -p orchester-desktop` alone does not build
#   another package's binary.
# - The new app: `powershell -File tools/run.ps1 app` runs `orchester-app`, which starts no runner yet.
#
# Further arguments go to `cargo run`, e.g.: powershell -File tools/run.ps1 --features devtools
# Each app's MCP server is part of every build.
$ErrorActionPreference = "Stop"

Push-Location (Split-Path -Parent $PSScriptRoot)
try {
    if ($args.Count -gt 0 -and $args[0] -eq "app") {
        $rest = @($args | Select-Object -Skip 1)
        cargo run -p orchester-app @rest
        exit $LASTEXITCODE
    }
    cargo build -p orchester-runner
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
    cargo run -p orchester-desktop @args
    exit $LASTEXITCODE
}
finally {
    Pop-Location
}

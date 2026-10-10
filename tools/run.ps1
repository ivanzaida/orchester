# Runs a desktop app. Both start `orchester-runner` from their own directory, and `cargo run` alone
# does not build another package's binary, so the runner is built first, in the same profile (debug,
# or release when `--release` is passed).
#
# - The deprecated desktop (default): runs `orchester-desktop`.
# - The new app: `powershell -File tools/run.ps1 app` runs `orchester-app`.
#
# Further arguments go to `cargo run`, e.g.: powershell -File tools/run.ps1 --features devtools
# Each app's MCP server is part of every build.
$ErrorActionPreference = "Stop"

Push-Location (Split-Path -Parent $PSScriptRoot)
try {
    $profile = @($args | Where-Object { $_ -eq "--release" })
    cargo build -p orchester-runner @profile
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
    if ($args.Count -gt 0 -and $args[0] -eq "app") {
        $rest = @($args | Select-Object -Skip 1)
        cargo run -p orchester-app @rest
        exit $LASTEXITCODE
    }
    cargo run -p orchester-desktop @args
    exit $LASTEXITCODE
}
finally {
    Pop-Location
}

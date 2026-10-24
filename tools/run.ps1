# Runs the desktop app (`orchester-app`, the program `orchester`). It starts `orchester-runner` from its own
# directory, and `cargo run` alone does not build another package's binary, so the runner is built first, in the same
# profile (debug, or release when `--release` is passed).
#
# Arguments go to `cargo run`, e.g.: powershell -File tools/run.ps1 --features devtools
# `app` as the first argument is still taken, and dropped, for the commands written when this script also ran the
# deprecated app. The app's MCP server is part of every build.
$ErrorActionPreference = "Stop"

Push-Location (Split-Path -Parent $PSScriptRoot)
try {
    $rest = @($args)
    if ($rest.Count -gt 0 -and $rest[0] -eq "app") {
        $rest = @($rest | Select-Object -Skip 1)
    }
    $profile = @($rest | Where-Object { $_ -eq "--release" })
    cargo build -p orchester-runner @profile
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
    cargo run -p orchester-app @rest
    exit $LASTEXITCODE
}
finally {
    Pop-Location
}

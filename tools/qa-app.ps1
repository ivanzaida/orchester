# Starts orchester-app on a seeded QA project, isolated from the user's data, for QA and design
# verification (.claude/skills/design-verification/SKILL.md):
#
#   powershell -File tools/qa-app.ps1 -Name <task> -Scenario <scenario> [-FakeAgents] [-NoRunner]
#
# 1. Builds orchester-app, the seed example and orchester-runner (-FakeAgents: the runner with its
#    `test-agent` binaries, orchester-fake-codex among them; -NoRunner: no runner).
# 2. Copies the programs into .tmp/qa/<task>/bin/, which keeps its own orchester.db beside them.
# 3. Seeds a new repository .tmp/qa/<task>/repo-<scenario> (examples/seed; -FakeAgents points the
#    project's Codex connection at the copied fake) and registers it in that orchester.db.
# 4. Starts the copied app with ORCHESTER_APP_MCP_START_WITHOUT_FOCUS=1 and window control on, and
#    prints its pid and its MCP discovery file. Stop it by that pid when done.
#
# .tmp/qa/<task>/ is replaced on every start; a QA app still running from it is refused, not stopped,
# and so is a folder on the way to it that is a junction or a link. Besides .tmp/qa/<task>/ it writes
# only cargo's target folder and the app's MCP discovery file in the user's discovery folder: never the
# user's install (E:\projects\orchester) nor a real repository. The app's console output goes to
# .tmp/qa/<task>/app.log and app.err.log.
param(
    [Parameter(Mandatory = $true)][string]$Name,
    # The seed's scenarios (crates/desktop-app/examples/seed/kontur/scenarios/mod.rs), checked before
    # anything under .tmp/qa/<task>/ is replaced.
    [Parameter(Mandatory = $true)]
    [ValidateSet("empty", "tasks", "task-states", "conversation", "changes", "cards", "interrupted", "unreadable-db", "tasks-unreadable", "live")]
    [string]$Scenario,
    [switch]$FakeAgents,
    [switch]$NoRunner
)
$ErrorActionPreference = "Stop"

if ($FakeAgents -and $NoRunner) {
    throw "-FakeAgents runs agents through the runner; it cannot go with -NoRunner"
}
if ($Name -notmatch '^[a-z0-9][a-z0-9-]*$') {
    throw "-Name is a short kebab-case task name (a-z, 0-9, -), not '$Name'"
}

$repositoryRoot = Split-Path -Parent $PSScriptRoot
$onWindows = $env:OS -eq "Windows_NT"
$exe = if ($onWindows) { ".exe" } else { "" }
$qaRoot = [System.IO.Path]::GetFullPath((Join-Path $repositoryRoot ".tmp/qa"))
$qa = Join-Path $qaRoot $Name
$bin = Join-Path $qa "bin"
$repo = Join-Path $qa "repo-$Scenario"

# Only .tmp/qa/<task>/ of this checkout, and never inside the user's install.
$install = [System.IO.Path]::GetFullPath("E:\projects\orchester")
foreach ($path in @($qa, $bin, $repo)) {
    $full = [System.IO.Path]::GetFullPath($path)
    if (-not $full.StartsWith($qaRoot + [System.IO.Path]::DirectorySeparatorChar, [System.StringComparison]::OrdinalIgnoreCase)) {
        throw "refusing ${full}: QA data stays under $qaRoot"
    }
    if ($full.StartsWith($install, [System.StringComparison]::OrdinalIgnoreCase)) {
        throw "refusing ${full}: it is inside the user's install"
    }
}

# A junction or link on the way would send the removal and the copies somewhere else.
$tmp = Join-Path $repositoryRoot ".tmp"
foreach ($folder in @($repositoryRoot, $tmp, $qaRoot, $qa, $bin, $repo)) {
    $item = Get-Item -Force -LiteralPath $folder -ErrorAction SilentlyContinue
    if ($item -and ($item.Attributes -band [System.IO.FileAttributes]::ReparsePoint)) {
        throw "refusing ${folder}: it is a junction or a link"
    }
}

# A QA app (or its runner) still running from this folder would lose its files.
if (Test-Path $bin) {
    $running = Get-Process | Where-Object {
        $_.Path -and $_.Path.StartsWith($bin, [System.StringComparison]::OrdinalIgnoreCase)
    }
    if ($running) {
        $pids = ($running | ForEach-Object { $_.Id }) -join ", "
        throw "programs from $bin are still running (pid $pids); stop the ones you started, by pid, first"
    }
}

Push-Location $repositoryRoot
try {
    cargo build -p orchester-app --bin orchester-app --example seed
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
    if ($FakeAgents) {
        cargo build -p orchester-runner --features test-agent --bins
    }
    elseif (-not $NoRunner) {
        cargo build -p orchester-runner --bin orchester-runner
    }
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
    $target = (cargo metadata --format-version 1 --no-deps | ConvertFrom-Json).target_directory
    $built = Join-Path $target "debug"
}
finally {
    Pop-Location
}

if (Test-Path $qa) {
    Remove-Item -Recurse -Force $qa
}
New-Item -ItemType Directory -Force -Path $bin | Out-Null
$programs = @("orchester-app")
if (-not $NoRunner) { $programs += "orchester-runner" }
if ($FakeAgents) { $programs += @("orchester-fake-codex", "orchester-fake-claude") }
foreach ($program in $programs) {
    Copy-Item (Join-Path $built "$program$exe") -Destination $bin
}

$seedArgs = @("--data-dir", $bin, "--repo", $repo, "--scenario", $Scenario)
if ($FakeAgents) { $seedArgs += @("--fake-codex", (Join-Path $bin "orchester-fake-codex$exe")) }
& (Join-Path $built "examples/seed$exe") @seedArgs
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

$env:ORCHESTER_APP_MCP_START_WITHOUT_FOCUS = "1"
$env:ORCHESTER_APP_MCP_WINDOW_CONTROL = "on"
$appProgram = Join-Path $bin "orchester-app$exe"
$appLog = Join-Path $qa "app.log"
$appErrors = Join-Path $qa "app.err.log"
if ($onWindows) {
    # The app must not inherit this script's handles: a caller reading the script's output through a pipe
    # would wait for the app to exit. A hidden PowerShell that ShellExecute starts (which passes no handles)
    # starts it, sharing that PowerShell's hidden console: a debug build is a console program, and a console
    # window of its own would take the focus. The app's output goes to files.
    $pidFile = Join-Path $qa "app.pid"
    $launch = "(Start-Process -FilePath '$appProgram' -WorkingDirectory '$bin' -NoNewWindow -PassThru " +
        "-RedirectStandardOutput '$appLog' -RedirectStandardError '$appErrors').Id | Set-Content -Path '$pidFile'"
    $encoded = [Convert]::ToBase64String([System.Text.Encoding]::Unicode.GetBytes($launch))
    # Not `-Wait`, which waits for the launcher's descendants too: the app among them.
    $launcher = Start-Process -FilePath "powershell.exe" -ArgumentList "-NoProfile -NonInteractive -EncodedCommand $encoded" `
        -WindowStyle Hidden -PassThru
    $launcher.WaitForExit()
    $app = Get-Process -Id ([int](Get-Content $pidFile))
}
else {
    $app = Start-Process -FilePath $appProgram -WorkingDirectory $bin -PassThru `
        -RedirectStandardOutput $appLog -RedirectStandardError $appErrors
}

$discoveryFolder = if ($onWindows) {
    Join-Path $env:LOCALAPPDATA "orchester-app-mcp"
}
else {
    Join-Path $HOME "Library/Application Support/orchester-app-mcp"
}
$discovery = Join-Path $discoveryFolder "$($app.Id).json"
$deadline = (Get-Date).AddSeconds(60)
while (-not (Test-Path $discovery) -and -not $app.HasExited -and (Get-Date) -lt $deadline) {
    Start-Sleep -Milliseconds 250
}
if ($app.HasExited) {
    throw "orchester-app (pid $($app.Id)) exited; see $appLog and $appErrors"
}
Write-Output "pid $($app.Id)"
Write-Output "discovery $discovery"
if (-not (Test-Path $discovery)) {
    Write-Output "the discovery file is not there yet after 60 s; the app may still be starting"
}

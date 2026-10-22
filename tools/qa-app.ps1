# Starts orchester-app on a seeded QA project, isolated from the user's data, for QA and design
# verification (.claude/skills/design-verification/SKILL.md):
#
#   powershell -File tools/qa-app.ps1 -Name <task> -Scenario <scenario> [-FakeAgents] [-NoRunner] [-SignedOut]
#       [-WindowX <x> -WindowY <y>] [-Editor <program>]
#
# 1. Builds the QA app (examples/qa_app: the app's own window, signed in to a stand-in Orchester Cloud on loopback
#    with its token in a keychain in memory, past the sign-in gate; -SignedOut stops at the gate, with that stand-in
#    as the cloud; with lurq's `perf_profile`, for profile sessions), the seed example and orchester-runner
#    (-FakeAgents: the runner with its `test-agent` binaries, orchester-fake-codex among them; -NoRunner: no runner).
# 2. Copies the programs into .tmp/qa/<task>/bin/, which keeps its own orchester.db beside them; the QA app as
#    orchester-app-qa.
# 3. Seeds a new repository .tmp/qa/<task>/repo-<scenario> (examples/seed; -FakeAgents points the
#    project's Codex connection at the copied fake) and registers it in that orchester.db.
# 4. Starts the copied app with ORCHESTER_APP_MCP_START_WITHOUT_FOCUS=1, window control on and
#    ORCHESTER_APP_MCP_DISCOVERY=.tmp/qa/<task>/mcp/ (a free port and a new token on each start, written
#    to <pid>.json there; never the installed app's fixed address or stored token), and prints its pid,
#    that folder and its MCP discovery file. Stop it by that pid when done.
#
# -WindowX and -WindowY, always together, open the window with its top-left corner there, in physical
# pixels of the virtual desktop (negative for a display left of or above the main one), instead of where
# the system puts it: ORCHESTER_APP_WINDOW_POSITION=<x>,<y> for the QA app only, nothing stored.
#
# -Editor names the program *Resolve in editor* starts with a run's worktree and its conflicting files
# (ORCHESTER_EDITOR for the QA app only), e.g. a stand-in script that writes down what it got, so no editor
# opens on the person's desktop. Without it the QA app has none named: an ORCHESTER_EDITOR of the caller's is
# dropped, and the app falls back to VS Code's code or the system's folder program, which a person's press opens.
#
# .tmp/qa/<task>/ is replaced on every start; a QA app still running from it is refused, not stopped,
# and so is a folder on the way to it that is a junction or a link. Besides .tmp/qa/<task>/ it writes
# only cargo's target folder: never the user's install (E:\projects\orchester), the user's discovery
# folder nor a real repository. The app's console output goes to .tmp/qa/<task>/app.log and app.err.log.
param(
    [Parameter(Mandatory = $true)][string]$Name,
    # The seed's scenarios (crates/desktop-app/examples/seed/kontur/scenarios/mod.rs), checked before
    # anything under .tmp/qa/<task>/ is replaced.
    [Parameter(Mandatory = $true)]
    [ValidateSet("empty", "tasks", "tasks-memory", "memory-only", "memory", "task-states", "conversation", "long-chats", "changes", "cards", "decisions", "interrupted", "unreadable-db", "tasks-unreadable", "live", "lifecycle", "connections", "api-providers", "agents", "agents-empty", "agents-no-connections", "agents-unreadable", "pipelines", "pipelines-empty", "accounts", "pull-requests", "sources", "usage", "usage-empty", "usage-bare")]
    [string]$Scenario,
    [switch]$FakeAgents,
    [switch]$NoRunner,
    [switch]$SignedOut,
    [int]$WindowX,
    [int]$WindowY,
    [string]$Editor
)
$ErrorActionPreference = "Stop"

if ($FakeAgents -and $NoRunner) {
    throw "-FakeAgents runs agents through the runner; it cannot go with -NoRunner"
}
if ($Name -notmatch '^[a-z0-9][a-z0-9-]*$') {
    throw "-Name is a short kebab-case task name (a-z, 0-9, -), not '$Name'"
}
if ($PSBoundParameters.ContainsKey("WindowX") -ne $PSBoundParameters.ContainsKey("WindowY")) {
    throw "-WindowX and -WindowY go together: the window's top-left corner needs both"
}
# The app's ORCHESTER_APP_WINDOW_POSITION; $null lets the system place the window.
$windowPosition = if ($PSBoundParameters.ContainsKey("WindowX")) {
    [string]::Format([System.Globalization.CultureInfo]::InvariantCulture, "{0},{1}", $WindowX, $WindowY)
}

$repositoryRoot = Split-Path -Parent $PSScriptRoot
$onWindows = $env:OS -eq "Windows_NT"
$exe = if ($onWindows) { ".exe" } else { "" }
$qaRoot = [System.IO.Path]::GetFullPath((Join-Path $repositoryRoot ".tmp/qa"))
$qa = Join-Path $qaRoot $Name
$bin = Join-Path $qa "bin"
$repo = Join-Path $qa "repo-$Scenario"
# The QA copy's MCP discovery folder (ORCHESTER_APP_MCP_DISCOVERY).
$mcp = Join-Path $qa "mcp"

# Only .tmp/qa/<task>/ of this checkout, and never inside the user's install.
$install = [System.IO.Path]::GetFullPath("E:\projects\orchester")
foreach ($path in @($qa, $bin, $repo, $mcp)) {
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
foreach ($folder in @($repositoryRoot, $tmp, $qaRoot, $qa, $bin, $repo, $mcp)) {
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
    # With lurq's frame timing, so a QA copy can be profiled over its MCP server (`lurq_profile_start`).
    cargo build -p orchester-app --features perf_profile --example qa_app --example seed
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
New-Item -ItemType Directory -Force -Path $mcp | Out-Null
$programs = @()
if (-not $NoRunner) { $programs += "orchester-runner" }
if ($FakeAgents) { $programs += @("orchester-fake-codex", "orchester-fake-claude") }
foreach ($program in $programs) {
    Copy-Item (Join-Path $built "$program$exe") -Destination $bin
}
# The QA app keeps its data beside it, and starts the runner from beside it, as the app does.
Copy-Item (Join-Path $built "examples/qa_app$exe") -Destination (Join-Path $bin "orchester-app-qa$exe")

$seedArgs = @("--data-dir", $bin, "--repo", $repo, "--scenario", $Scenario)
if ($FakeAgents) { $seedArgs += @("--fake-codex", (Join-Path $bin "orchester-fake-codex$exe")) }
& (Join-Path $built "examples/seed$exe") @seedArgs
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

# This script's own environment, which only the app (and its launcher) started below inherit; nothing is
# stored. A position the caller's environment holds is dropped unless -WindowX/-WindowY ask for one.
$env:ORCHESTER_APP_MCP_START_WITHOUT_FOCUS = "1"
$env:ORCHESTER_APP_MCP_WINDOW_CONTROL = "on"
$env:ORCHESTER_APP_MCP_DISCOVERY = $mcp
$env:ORCHESTER_APP_WINDOW_POSITION = $windowPosition
$env:ORCHESTER_EDITOR = if ($Editor) { [System.IO.Path]::GetFullPath($Editor) } else { $null }
$appProgram = Join-Path $bin "orchester-app-qa$exe"
# Start-Process takes no empty argument list: an option that is always there leads the QA app's arguments.
$appArguments = if ($SignedOut) { "--signed-out" } else { "--signed-in" }
$appLog = Join-Path $qa "app.log"
$appErrors = Join-Path $qa "app.err.log"
if ($onWindows) {
    # The app must not inherit this script's handles: a caller reading the script's output through a pipe
    # would wait for the app to exit. A hidden PowerShell that ShellExecute starts (which passes no handles)
    # starts it, sharing that PowerShell's hidden console: a debug build is a console program, and a console
    # window of its own would take the focus. The app's output goes to files.
    $pidFile = Join-Path $qa "app.pid"
    $launch = "(Start-Process -FilePath '$appProgram' -ArgumentList '$appArguments' -WorkingDirectory '$bin' -NoNewWindow -PassThru " +
        "-RedirectStandardOutput '$appLog' -RedirectStandardError '$appErrors').Id | Set-Content -Path '$pidFile'"
    $encoded = [Convert]::ToBase64String([System.Text.Encoding]::Unicode.GetBytes($launch))
    # Not `-Wait`, which waits for the launcher's descendants too: the app among them.
    $launcher = Start-Process -FilePath "powershell.exe" -ArgumentList "-NoProfile -NonInteractive -EncodedCommand $encoded" `
        -WindowStyle Hidden -PassThru
    $launcher.WaitForExit()
    $app = Get-Process -Id ([int](Get-Content $pidFile))
}
else {
    $app = Start-Process -FilePath $appProgram -ArgumentList $appArguments -WorkingDirectory $bin -PassThru `
        -RedirectStandardOutput $appLog -RedirectStandardError $appErrors
}

$discovery = Join-Path $mcp "$($app.Id).json"
$deadline = (Get-Date).AddSeconds(60)
while (-not (Test-Path $discovery) -and -not $app.HasExited -and (Get-Date) -lt $deadline) {
    Start-Sleep -Milliseconds 250
}
if ($app.HasExited) {
    throw "orchester-app (pid $($app.Id)) exited; see $appLog and $appErrors"
}
Write-Output "pid $($app.Id)"
Write-Output "mcp $mcp"
Write-Output "discovery $discovery"
if (-not (Test-Path $discovery)) {
    Write-Output "the discovery file is not there yet after 60 s; the app may still be starting"
}

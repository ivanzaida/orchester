# Starts orchester-app on a seeded QA project, isolated from the user's data, for QA and design
# verification (.claude/skills/design-verification/SKILL.md):
#
#   powershell -File tools/qa-app.ps1 -Name <task> -Scenario <scenario> [-FakeAgents] [-NoRunner] [-SignedIn]
#       [-WindowX <x> -WindowY <y>] [-Editor <program>]
#
# <scenario> is one of the seed's (`cargo run -p orchester-app --example seed -- --list-scenarios` lists them). A new
# scenario is added only to the seed's registry (crates/desktop-app/examples/seed/kontur/scenarios/mod.rs), never
# here: the seed checks the name, and on an unknown one exits non-zero with the known ones before it writes anything.
#
# 1. Builds the QA app (examples/qa_app: the app's own window, nobody signed in as on a first run, a stand-in Orchester
#    Cloud on loopback set as the cloud; -SignedIn signs in to it, with its token in a keychain in memory; with lurq's
#    `perf_profile`, for profile sessions), the seed example and orchester-runner (-FakeAgents: the runner with its
#    `test-agent` binaries, orchester-fake-codex among them; -NoRunner: no runner).
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
# -Scenario settings is app Settings with a person's data in it (App settings · redesign, iUmo3), fixture-only and
# deterministic. The seed writes agent accounts of both CLIs in different states with plan readings (Work signed in
# and blocked from removal by a running run, Personal unused, Old's sign-in expired, Client in use by a connection,
# Archive's folder missing), quiet hours 22:30-07:00 and the daily report at 18:30, and stand-in claude and codex in
# bin/settings-qa/ that answer as those accounts stand: the QA app runs with them first on its PATH (folders that
# hold a real claude or codex are left out) and its home, CLAUDE_CONFIG_DIR, CODEX_HOME and ORCHESTER_ACCOUNTS_DIR
# there, so no real CLI or sign-in is read. The scenario asks for the sign-in itself, with or without -SignedIn, and the
# QA app (--settings) keeps two invites here (one waiting for approval,
# one that didn't finish) and shows the installed app's MCP rows: that server listens on a free port with a token in
# memory, while the window's own server stays the QA copy's for the verifier. Team servers answering on loopback are
# the private platform repository's (ivanzaida/orchester-platform, crates/e2e, the n1_qa example), where the team
# server lives.
#
# -Scenario project-settings reaches Project settings' drawn states (gh as another account, a gone base branch,
# secrets in three environments with uses, the MCP server figma); project-settings-shared is it linked to a team
# server no one answers for (Sharing · shared).
#
# -Scenario join-team is `empty` with a stand-in team server on loopback (orchester-api's test support) that the
# stand-in cloud names as the account's, with an invite to its project Billing, for Join a team (navigate to
# join_team). The QA app (--join-team) prints the server's address and its codes to app.log: one joins, any other is
# wrong; an address where nothing listens (http://127.0.0.1:9) does not answer.
#
# .tmp/qa/<task>/ is replaced on every start; a QA app still running from it is refused, not stopped,
# and so is a folder on the way to it that is a junction or a link. Besides .tmp/qa/<task>/ it writes
# only cargo's target folder: never inside an installed Orchester (a folder with orchester.exe beside orchester.db),
# the user's discovery folder nor a real repository. The app's console output goes to .tmp/qa/<task>/app.log and app.err.log.
param(
    [Parameter(Mandatory = $true)][string]$Name,
    # The seed checks it against its registry (see the header); the app starts only when the seed succeeds.
    [Parameter(Mandatory = $true)][string]$Scenario,
    [switch]$FakeAgents,
    [switch]$NoRunner,
    [switch]$SignedIn,
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

# The installed Orchester folder that holds `path`, or $null. An install keeps its orchester.db beside its program,
# wherever the person installed it; a checkout or a QA copy's bin/ (orchester-app-qa) holds no such pair.
function Find-Install([string]$path) {
    $folder = [System.IO.DirectoryInfo]::new($path)
    while ($folder) {
        $program = Join-Path $folder.FullName "orchester$exe"
        $database = Join-Path $folder.FullName "orchester.db"
        if ((Test-Path -LiteralPath $program -PathType Leaf) -and (Test-Path -LiteralPath $database -PathType Leaf)) {
            return $folder.FullName
        }
        $folder = $folder.Parent
    }
    return $null
}

# Only .tmp/qa/<task>/ of this checkout, and never inside an installed Orchester.
foreach ($path in @($qa, $bin, $repo, $mcp)) {
    $full = [System.IO.Path]::GetFullPath($path)
    if (-not $full.StartsWith($qaRoot + [System.IO.Path]::DirectorySeparatorChar, [System.StringComparison]::OrdinalIgnoreCase)) {
        throw "refusing ${full}: QA data stays under $qaRoot"
    }
    $install = Find-Install $full
    if ($install) {
        throw "refusing ${full}: it is inside the Orchester installed in $install"
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
if ($Scenario -eq "settings") {
    # The seed's stand-in CLIs first on the PATH, without a folder that holds a real claude or codex, and the CLIs'
    # own folders, the home and the folder for new accounts in bin/settings-qa/.
    $fixtures = Join-Path $bin "settings-qa"
    $holdsCli = {
        param($folder)
        foreach ($program in @("claude", "codex")) {
            foreach ($extension in @("", ".exe", ".cmd", ".bat", ".ps1")) {
                if (Test-Path -LiteralPath (Join-Path $folder "$program$extension") -PathType Leaf) { return $true }
            }
        }
        return $false
    }
    $separator = [System.IO.Path]::PathSeparator
    $kept = @($env:PATH -split $separator | Where-Object { $_ -and -not (& $holdsCli $_) })
    $env:PATH = (@(Join-Path $fixtures "path") + $kept) -join $separator
    $fixtureHome = Join-Path $fixtures "home"
    $env:CLAUDE_CONFIG_DIR = Join-Path $fixtureHome ".claude"
    $env:CODEX_HOME = Join-Path $fixtureHome ".codex"
    if ($onWindows) { $env:USERPROFILE = $fixtureHome } else { $env:HOME = $fixtureHome }
    $env:ORCHESTER_ACCOUNTS_DIR = Join-Path $fixtures "accounts"
}
$appProgram = Join-Path $bin "orchester-app-qa$exe"
# Start-Process takes no empty argument list: an option that is always there leads the QA app's arguments.
$appArguments = if ($SignedIn) { "--signed-in" } else { "--signed-out" }
if ($Scenario -eq "settings") { $appArguments += " --settings" }
if ($Scenario -eq "join-team") { $appArguments += " --join-team" }
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

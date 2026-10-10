# Installs a release build of Orchester for daily use, apart from the dev build in target/debug
# that agents rebuild. It builds both desktop apps, the deprecated `orchester` and the new
# `orchester-app`, and the runner they both start from their own directory, in one build of this
# checkout into its own target directory, and copies the three programs into the install directory.
# The two apps share the application database created next to them on first start (one app at a
# time: the data folder's lock), so they always come from the same build: an older one may refuse
# or misread data a newer one migrated. The installed apps keep their own project list, apart from
# the dev build's. See docs/stable-build.md.
# Every build includes the embedded MCP servers, so agents can drive the installed apps.
#   powershell -File tools/install.ps1 [-InstallDir <dir>] [-BuildDir <dir>] [-ImportDevData]
#     [-StartMenuShortcut]
param(
    # Where orchester.exe, orchester-app.exe, orchester-runner.exe and the application database live.
    [string]$InstallDir = (Join-Path $env:LOCALAPPDATA "Orchester\app"),
    # Cargo target directory of the release build; outside the repository so agent builds never touch it.
    [string]$BuildDir = (Join-Path $env:LOCALAPPDATA "Orchester\build"),
    # Copies the dev build's application database (target/debug/orchester.db) into the install
    # directory. Refused when the install directory already has one: it is never overwritten.
    [switch]$ImportDevData,
    # Adds Orchester (the deprecated app) and Orchester App (the new one) to the current user's Start menu.
    [switch]$StartMenuShortcut
)
$ErrorActionPreference = "Stop"

$repositoryRoot = Split-Path -Parent $PSScriptRoot
$InstallDir = $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($InstallDir)
$BuildDir = $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($BuildDir)
$devDir = Join-Path $repositoryRoot "target\debug"
$programs = @("orchester.exe", "orchester-app.exe", "orchester-runner.exe")
# SQLite runs in WAL mode: after an unclean end, committed changes may still be in the -wal file.
$databaseFiles = @("orchester.db", "orchester.db-wal")

# Refuses while a program is running from exactly $Directory. Nothing is ever stopped here.
function Assert-NotRunning([string]$Directory, [string]$What) {
    $executables = $programs | ForEach-Object { Join-Path $Directory $_ }
    $names = ($programs | ForEach-Object { "Name = '$_'" }) -join " OR "
    $running = @(Get-CimInstance Win32_Process -Filter $names |
        Where-Object { $_.ExecutablePath -and ($executables -contains $_.ExecutablePath) })
    if ($running.Count -gt 0) {
        $ids = ($running | ForEach-Object { $_.ProcessId }) -join ", "
        throw "$What is running from $Directory (process $ids). Close it and run this script again."
    }
}

function Assert-CanImportDevData {
    $source = Join-Path $devDir "orchester.db"
    if (-not (Test-Path -LiteralPath $source -PathType Leaf)) {
        throw "There is no dev database to import at $source."
    }
    foreach ($file in $databaseFiles) {
        $target = Join-Path $InstallDir $file
        if (Test-Path -LiteralPath $target) {
            throw "$target already exists; -ImportDevData never overwrites the installed app's data."
        }
    }
    Assert-NotRunning $devDir "The dev build of Orchester"
}

Assert-NotRunning $InstallDir "The installed Orchester"
if ($ImportDevData) { Assert-CanImportDevData }

Push-Location $repositoryRoot
try {
    # One build of all three: the apps share the application database and the runner, so they must match.
    cargo build --release --locked --target-dir $BuildDir -p orchester-desktop -p orchester-app -p orchester-runner
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
    $commit = git rev-parse --short HEAD
    if (git status --porcelain) { $commit = "$commit with uncommitted changes" }
}
finally {
    Pop-Location
}

# The build takes minutes; the app may have been started meanwhile.
Assert-NotRunning $InstallDir "The installed Orchester"
if ($ImportDevData) { Assert-CanImportDevData }

New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
foreach ($program in $programs) {
    Copy-Item -LiteralPath (Join-Path $BuildDir "release\$program") -Destination (Join-Path $InstallDir $program) -Force
}
if ($ImportDevData) {
    foreach ($file in $databaseFiles) {
        $source = Join-Path $devDir $file
        if (Test-Path -LiteralPath $source -PathType Leaf) {
            # overwrite = $false: fails instead of replacing a file that appeared since the check.
            [System.IO.File]::Copy($source, (Join-Path $InstallDir $file), $false)
        }
    }
    Write-Output "Imported the dev database from $devDir"
}

$executable = Join-Path $InstallDir "orchester.exe"
$appExecutable = Join-Path $InstallDir "orchester-app.exe"
if ($StartMenuShortcut) {
    # Each shortcut takes its program's embedded icon, so it needs no IconLocation.
    $shortcuts = [ordered]@{ "Orchester.lnk" = $executable; "Orchester App.lnk" = $appExecutable }
    foreach ($name in $shortcuts.Keys) {
        $shortcutPath = Join-Path ([Environment]::GetFolderPath("Programs")) $name
        $shortcut = (New-Object -ComObject WScript.Shell).CreateShortcut($shortcutPath)
        $shortcut.TargetPath = $shortcuts[$name]
        $shortcut.WorkingDirectory = $InstallDir
        $shortcut.Save()
        Write-Output "Start menu shortcut: $shortcutPath"
    }
}

Write-Output "Installed Orchester ($commit) to $InstallDir"
Write-Output "Start the new app with: $appExecutable"
Write-Output "Start the deprecated app with: $executable"
Write-Output "One app at a time: both use the application database in $InstallDir."

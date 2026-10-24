# Installs a release build of Orchester for daily use, apart from the dev build in target/debug
# that agents rebuild. It builds the app (`orchester-app`, the program `orchester.exe`) and the
# runner it starts from its own directory, in one build of this checkout into its own target
# directory, and copies both programs into the install directory. The app keeps its application
# database next to them, created on first start, so the installed app has its own project list,
# apart from the dev build's. See docs/stable-build.md.
# Every build includes the app's MCP server, so agents can drive the installed app.
# Builds before the program's rename installed it as `orchester-app.exe` (beside the deprecated
# app's `orchester.exe`, which this replaces). An `orchester-app.exe` left in the install directory
# is from an older build: the script never deletes it, refuses while it runs, and says so.
#   powershell -File tools/install.ps1 [-InstallDir <dir>] [-BuildDir <dir>] [-ImportDevData]
#     [-StartMenuShortcut]
param(
    # Where orchester.exe, orchester-runner.exe and the application database live.
    [string]$InstallDir = (Join-Path $env:LOCALAPPDATA "Orchester\app"),
    # Cargo target directory of the release build; outside the repository so agent builds never touch it.
    [string]$BuildDir = (Join-Path $env:LOCALAPPDATA "Orchester\build"),
    # Copies the dev build's application database (target/debug/orchester.db) into the install
    # directory. Refused when the install directory already has one: it is never overwritten.
    [switch]$ImportDevData,
    # Adds Orchester to the current user's Start menu.
    [switch]$StartMenuShortcut
)
$ErrorActionPreference = "Stop"

$repositoryRoot = Split-Path -Parent $PSScriptRoot
$InstallDir = $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($InstallDir)
$BuildDir = $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($BuildDir)
$devDir = Join-Path $repositoryRoot "target\debug"
$programs = @("orchester.exe", "orchester-runner.exe")
# The app's program as builds before its rename named it. It starts the runner beside it, so the
# install is not updated while it runs either.
$formerProgram = "orchester-app.exe"
$checkedPrograms = $programs + $formerProgram
# SQLite runs in WAL mode: after an unclean end, committed changes may still be in the -wal file.
$databaseFiles = @("orchester.db", "orchester.db-wal")

# Refuses while a program is running from exactly $Directory. Nothing is ever stopped here.
function Assert-NotRunning([string]$Directory, [string]$What) {
    $executables = $checkedPrograms | ForEach-Object { Join-Path $Directory $_ }
    $names = ($checkedPrograms | ForEach-Object { "Name = '$_'" }) -join " OR "
    $running = @(Get-CimInstance Win32_Process -Filter $names |
        Where-Object { $_.ExecutablePath -and ($executables -contains $_.ExecutablePath) })
    if ($running.Count -gt 0) {
        $ids = ($running | ForEach-Object { $_.ProcessId }) -join ", "
        throw "$What is running from $Directory (process $ids). Close it and run this script again."
    }
}

# Refuses while any installed program is running, whatever path it was started by (a junction, a
# subst drive, a short name) and whoever runs it: Windows does not let anyone open the file of a
# running program for exclusive writing.
function Assert-NotInUse {
    foreach ($program in $programs) {
        $target = Join-Path $InstallDir $program
        if (-not (Test-Path -LiteralPath $target -PathType Leaf)) { continue }
        try {
            [System.IO.File]::Open($target, "Open", "ReadWrite", "None").Dispose()
        }
        catch {
            throw "$target is in use (is Orchester running from ${InstallDir}?). Close it and run this script again."
        }
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
    # One build of both: the app and its runner must match.
    # ORCHESTER_APP_INSTALLED_BUILD makes this orchester-app the installed app: the fixed MCP address and the token
    # in the secret store (docs/stable-build.md). Only this script and package-macos.sh set it.
    $env:ORCHESTER_APP_INSTALLED_BUILD = "1"
    try {
        cargo build --release --locked --target-dir $BuildDir -p orchester-app -p orchester-runner
    }
    finally {
        Remove-Item Env:ORCHESTER_APP_INSTALLED_BUILD
    }
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
$hadDatabase = Test-Path -LiteralPath (Join-Path $InstallDir "orchester.db") -PathType Leaf
# Both are copied beside the installed ones first, then each replaces its own, so a failed copy
# changes nothing and a failed replacement says which programs are from which build.
$replaced = @()
try {
    foreach ($program in $programs) {
        Copy-Item -LiteralPath (Join-Path $BuildDir "release\$program") -Destination (Join-Path $InstallDir "$program.new") -Force
    }
    Assert-NotInUse
    foreach ($program in $programs) {
        $target = Join-Path $InstallDir $program
        Move-Item -LiteralPath "$target.new" -Destination $target -Force
        $replaced += $program
    }
}
catch {
    foreach ($program in $programs) {
        Remove-Item -LiteralPath (Join-Path $InstallDir "$program.new") -Force -ErrorAction SilentlyContinue
    }
    if ($replaced.Count -gt 0) {
        $kept = @($programs | Where-Object { $replaced -notcontains $_ })
        Write-Warning ("$InstallDir now mixes two builds: replaced $($replaced -join ', '); not replaced " +
            "$($kept -join ', '). Do not start Orchester from it; close what runs there and run this script again.")
    }
    throw
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
$startMenu = [Environment]::GetFolderPath("Programs")
if ($StartMenuShortcut) {
    # The shortcut takes the program's embedded icon, so it needs no IconLocation.
    $shortcutPath = Join-Path $startMenu "Orchester.lnk"
    $shortcut = (New-Object -ComObject WScript.Shell).CreateShortcut($shortcutPath)
    $shortcut.TargetPath = $executable
    $shortcut.WorkingDirectory = $InstallDir
    $shortcut.Save()
    Write-Output "Start menu shortcut: $shortcutPath"
}

Write-Output "Installed Orchester ($commit) to $InstallDir"
Write-Output "Start it with: $executable"
$formerExecutable = Join-Path $InstallDir $formerProgram
if (Test-Path -LiteralPath $formerExecutable -PathType Leaf) {
    Write-Warning ("$formerExecutable is the app from an older build (before its program was renamed to " +
        "orchester.exe). Do not start it: it would open this install's database with an older build. Delete it " +
        "yourself, with the Start menu shortcut 'Orchester App' if you made one; this script never deletes it.")
}
if ($hadDatabase) {
    Write-Warning ("Before the first start of this build, with the app closed, copy aside $InstallDir\orchester.db " +
        "(and orchester.db-wal) and each project's .orchester\orchester.db (and its -wal): this build may migrate " +
        "them, and an older build then refuses them. See docs/stable-build.md.")
}

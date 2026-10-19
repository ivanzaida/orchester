# The test part of the quality gate: runs the integration tests of the crates a branch changed, for the pipelines'
# Check step. tools/check.ps1 calls it when asked (`powershell -File tools/check.ps1 -Tests changed`); it runs on its
# own as well: powershell -File tools/check-tests.ps1 [-Scope changed|all] [-Base <revision>]
#
# Scope `changed`: the workspace crates with a changed file since the branch left its base (committed, staged,
# unstaged or untracked), and every workspace crate that depends on one of them, as a change can break its tests too.
# A change to the workspace's Cargo.toml, Cargo.lock, rust-toolchain.toml or .cargo/ counts for every crate. The
# frozen orchester-desktop (AGENTS.md) is tested only when the branch changes it. Scope `all`: every other crate, to
# refresh the known failures.
#
# The base: the merge base of HEAD with -Base when given, else the newer of its merge bases with `master` and
# `origin/master` (a run branches off the local `master`, an agent's worktree often off `origin/master`).
#
# Each test binary of those crates runs on its own, `cargo test -p <crate> --test <binary>`, one after the other, with
# the features its targets require. AGENTS.md rule 14 (a name filter on every `cargo test`) is for runs while working;
# this gate runs whole binaries on purpose, as no filter selects every test. orchester-app's binaries run with
# `--test-threads=4`, so its window tests leave the machine usable. Cargo reads CARGO_BUILD_JOBS itself. The tests build with line tables only (CARGO_PROFILE_TEST_DEBUG, unless set already):
# with full debug information each orchester-app test binary brings a PDB of about 900 MB, and every crate's tests
# together filled more than 160 GB.
#
# A failing test listed in tools/check-tests-known-failures.txt is reported and does not fail the gate; any other
# failure does, as does a binary that fails without naming a test (a build error, a crash, a harness of its own).
# The full output of every binary is in .tmp/check-tests/.
param(
    [ValidateSet("changed", "all")]
    [string] $Scope = "changed",
    [string] $Base = ""
)
$ErrorActionPreference = "Stop"

$repositoryRoot = Split-Path -Parent $PSScriptRoot
$knownFailuresFile = Join-Path $PSScriptRoot "check-tests-known-failures.txt"
$logRoot = Join-Path $repositoryRoot ".tmp/check-tests"
$frozenPackage = "orchester-desktop"
$uiPackage = "orchester-app"
$uiTestThreads = 4
$wholeWorkspaceFiles = @("Cargo.toml", "Cargo.lock", "rust-toolchain.toml")
# Programs a crate's tests start from target/debug/ and do not build themselves (each test says so when one is
# missing): orchester-app's live tests run the runner with its fake agents, and Orchester Cloud. They build with the
# test profile, whose programs land in target/debug/ too, so they share what the tests built.
$programsForTests = @{
    "orchester-app" = @(
        @("-p", "orchester-runner", "--features", "test-agent", "--bins"),
        @("-p", "orchester-cloud", "--bins")
    )
}

function Invoke-Git([string[]] $Arguments) {
    $output = & git @Arguments
    if ($LASTEXITCODE -ne 0) { throw "git $($Arguments -join ' ') failed" }
    $output
}

# The workspace's crates: name, folder relative to the root, workspace crates it depends on (dev-dependencies
# included), test binaries and the union of the features those binaries require.
function Get-WorkspacePackages {
    $metadataJson = cargo metadata --no-deps --format-version 1
    if ($LASTEXITCODE -ne 0) { throw "cargo metadata failed" }
    $metadata = $metadataJson | ConvertFrom-Json
    $root = $metadata.workspace_root.Replace("\", "/").TrimEnd("/") + "/"
    $members = $metadata.packages | Where-Object { $metadata.workspace_members -contains $_.id }
    $names = @($members | ForEach-Object { $_.name })
    foreach ($package in $members) {
        $tests = @($package.targets | Where-Object { $_.kind -contains "test" })
        $features = @($tests | ForEach-Object { $_.'required-features' } | Where-Object { $_ } | Sort-Object -Unique)
        [PSCustomObject]@{
            Name = $package.name
            Folder = (Split-Path -Parent $package.manifest_path).Replace("\", "/").Replace($root, "") + "/"
            Dependencies = @($package.dependencies | ForEach-Object { $_.name } |
                Where-Object { $names -contains $_ -and $_ -ne $package.name } | Sort-Object -Unique)
            Tests = @($tests | ForEach-Object { $_.name } | Sort-Object)
            Features = $features
        }
    }
}

function Get-BaseRevision {
    if ($Base) {
        Invoke-Git @("rev-parse", "--verify", "--quiet", "$Base^{commit}") | Out-Null
        return Invoke-Git @("merge-base", "HEAD", $Base)
    }
    $mergeBases = @(@("master", "origin/master") | ForEach-Object {
        & git rev-parse --verify --quiet "$_^{commit}" | Out-Null
        if ($LASTEXITCODE -eq 0) { Invoke-Git @("merge-base", "HEAD", $_) }
    })
    if (-not $mergeBases) { throw "no base: neither master nor origin/master exists; pass -Base <revision>" }
    $newest = $mergeBases[0]
    foreach ($candidate in $mergeBases) {
        & git merge-base --is-ancestor $newest $candidate
        if ($LASTEXITCODE -eq 0) { $newest = $candidate }
    }
    $newest
}

function Get-ChangedFiles([string] $Revision) {
    @(Invoke-Git @("diff", "--name-only", "--no-renames", $Revision)) +
        @(Invoke-Git @("ls-files", "--others", "--exclude-standard")) | Where-Object { $_ } | Sort-Object -Unique
}

function Select-ChangedPackages($Packages, [string[]] $Files) {
    $wholeWorkspace = $Files | Where-Object { $wholeWorkspaceFiles -contains $_ -or $_.StartsWith(".cargo/") }
    if ($wholeWorkspace) {
        Write-Output "tests: $($wholeWorkspace -join ', ') changed, which counts for every crate" | Out-Host
        return @($Packages | Where-Object { $_.Name -ne $frozenPackage } | ForEach-Object { $_.Name })
    }
    $changed = @($Packages | Where-Object {
        $folder = $_.Folder
        $Files | Where-Object { $_.StartsWith($folder) } | Select-Object -First 1
    } | ForEach-Object { $_.Name })
    $selected = [System.Collections.Generic.List[string]]::new()
    $changed | ForEach-Object { $selected.Add($_) }
    do {
        $added = $Packages | Where-Object {
            $_.Name -ne $frozenPackage -and -not $selected.Contains($_.Name) -and
                ($_.Dependencies | Where-Object { $selected.Contains($_) })
        }
        $added | ForEach-Object { $selected.Add($_.Name) }
    } while ($added)
    if ($changed) { Write-Output "tests: changed crates: $($changed -join ', ')" | Out-Host }
    @($selected)
}

# "<package> <binary> <test name> # reason" per line; the reason is required.
function Read-KnownFailures {
    $known = @{}
    if (-not (Test-Path $knownFailuresFile)) { return $known }
    $number = 0
    foreach ($line in [System.IO.File]::ReadAllLines($knownFailuresFile)) {
        $number++
        $text = $line.Trim()
        if (-not $text -or $text.StartsWith("#")) { continue }
        if ($text -notmatch '^(\S+)\s+(\S+)\s+(\S+)\s+#\s*\S') {
            throw "check-tests-known-failures.txt:${number}: expected '<package> <binary> <test name> # reason'"
        }
        $known["$($Matches[1]) $($Matches[2]) $($Matches[3])"] = $true
    }
    $known
}

# Runs cargo with its output in two files; returns the exit code. Waits for cargo alone, not for what a test left
# running.
function Invoke-Cargo([string[]] $Arguments, [string] $LogName) {
    $process = Start-Process -FilePath "cargo" -ArgumentList $Arguments -NoNewWindow -PassThru `
        -RedirectStandardOutput (Join-Path $logRoot "$LogName.out.log") `
        -RedirectStandardError (Join-Path $logRoot "$LogName.err.log")
    # Windows PowerShell reports no exit code for a process whose handle was never read.
    if ($PSVersionTable.PSEdition -eq "Desktop") { $null = $process.Handle }
    $process.WaitForExit()
    $process.ExitCode
}

function Get-LogTail([string] $LogName, [string] $Stream, [int] $Count) {
    $path = Join-Path $logRoot "$LogName.$Stream.log"
    if (Test-Path $path) { Get-Content -Path $path -Tail $Count }
}

# The names in libtest's closing "failures:" list.
function Get-FailedTests([string] $LogName) {
    $path = Join-Path $logRoot "$LogName.out.log"
    if (-not (Test-Path $path)) { return @() }
    $lines = @(Get-Content -Path $path)
    $start = [Array]::LastIndexOf($lines, "failures:")
    if ($start -lt 0) { return @() }
    @($lines[($start + 1)..($lines.Length - 1)] | Where-Object { $_ -match '^    (\S+)$' } | ForEach-Object { $Matches[1] })
}

# What a failed test printed: its "---- <name> stdout ----" section, at most 40 lines of at most 300 characters.
function Get-FailureOutput([string] $LogName, [string] $Test) {
    $lines = @(Get-Content -Path (Join-Path $logRoot "$LogName.out.log"))
    $start = [Array]::IndexOf($lines, "---- $Test stdout ----")
    if ($start -lt 0) { return }
    $end = $start + 1
    while ($end -lt $lines.Length -and $lines[$end] -notmatch '^(---- .* ----|failures:)$') { $end++ }
    $lines[$start..([Math]::Min($end - 1, $start + 40))] | ForEach-Object {
        if ($_.Length -gt 300) { $_.Substring(0, 300) + " ..." } else { $_ }
    }
}

function Get-FeatureArguments($Package) {
    if ($Package.Features) { @("--features", ($Package.Features -join ",")) } else { @() }
}

function Test-Package($Package, $Known, $Report) {
    if (-not $Package.Tests) {
        Write-Output "== tests $($Package.Name): no test binaries"
        return
    }
    $features = Get-FeatureArguments $Package
    $buildLog = "$($Package.Name)--build"
    $targets = $Package.Tests | ForEach-Object { "--test"; $_ }
    $watch = [System.Diagnostics.Stopwatch]::StartNew()
    Write-Output "== tests $($Package.Name): building $($Package.Tests.Count) test binaries"
    if ((Invoke-Cargo (@("test", "-p", $Package.Name, "--no-run") + $features + $targets) $buildLog) -ne 0) {
        $Report.New.Add("$($Package.Name): the tests do not build")
        Get-LogTail $buildLog "err" 40
        return
    }
    $programs = $programsForTests[$Package.Name]
    for ($index = 0; $programs -and $index -lt $programs.Count; $index++) {
        $programLog = "$($Package.Name)--programs-$index"
        if ((Invoke-Cargo (@("build", "--profile", "test") + $programs[$index]) $programLog) -ne 0) {
            $Report.New.Add("$($Package.Name): the programs its tests need do not build: cargo build $($programs[$index])")
            Get-LogTail $programLog "err" 40
            return
        }
    }
    Write-Output "   built in $([int]$watch.Elapsed.TotalSeconds) s"
    $harness = if ($Package.Name -eq $uiPackage) { @("--", "--test-threads=$uiTestThreads") } else { @() }
    foreach ($binary in $Package.Tests) {
        $logName = "$($Package.Name)--$binary"
        $watch.Restart()
        $exitCode = Invoke-Cargo (@("test", "-p", $Package.Name) + $features + @("--test", $binary) + $harness) $logName
        $seconds = [Math]::Round($watch.Elapsed.TotalSeconds, 1)
        if ($exitCode -eq 0) {
            Write-Output "   ok      $binary ($seconds s)"
            continue
        }
        $failed = Get-FailedTests $logName
        if (-not $failed) {
            Write-Output "   FAILED  $binary ($seconds s): exited with $exitCode without naming a failed test"
            $Report.New.Add("$($Package.Name) --test ${binary}: exited with $exitCode")
            Get-LogTail $logName "out" 20
            Get-LogTail $logName "err" 20
            continue
        }
        Write-Output "   FAILED  $binary ($seconds s): $($failed.Count) failed"
        foreach ($test in $failed) {
            $key = "$($Package.Name) $binary $test"
            if ($Known.ContainsKey($key)) {
                Write-Output "     known failure: $test"
                $Report.Known.Add($key)
                continue
            }
            Write-Output "     NEW failure: $test"
            Get-FailureOutput $logName $test | ForEach-Object { "       $_" }
            $Report.New.Add($key)
        }
    }
}

$debugBefore = $env:CARGO_PROFILE_TEST_DEBUG
if (-not $debugBefore) { $env:CARGO_PROFILE_TEST_DEBUG = "line-tables-only" }
Push-Location $repositoryRoot
try {
    $packages = @(Get-WorkspacePackages)
    if ($Scope -eq "all") {
        $selected = @($packages | Where-Object { $_.Name -ne $frozenPackage } | ForEach-Object { $_.Name })
    }
    else {
        $baseRevision = Get-BaseRevision
        $files = @(Get-ChangedFiles $baseRevision)
        Write-Output "tests: base $baseRevision, $($files.Count) changed files"
        $selected = @(Select-ChangedPackages $packages $files)
    }
    if (-not $selected) {
        Write-Output "tests: no crate changed; no tests to run"
        exit 0
    }
    Write-Output "tests: $($selected -join ', ')"
    $known = Read-KnownFailures
    if (Test-Path $logRoot) { Remove-Item -Recurse -Force $logRoot }
    New-Item -ItemType Directory -Force -Path $logRoot | Out-Null
    $report = [PSCustomObject]@{
        New = [System.Collections.Generic.List[string]]::new()
        Known = [System.Collections.Generic.List[string]]::new()
    }
    $total = [System.Diagnostics.Stopwatch]::StartNew()
    foreach ($package in $packages | Where-Object { $selected -contains $_.Name }) {
        Test-Package $package $known $report
    }
    Write-Output "tests: $([int]$total.Elapsed.TotalSeconds) s, logs in .tmp/check-tests/"
    $passedKnown = @($known.Keys | Where-Object {
        $selected -contains $_.Split(" ")[0] -and -not $report.Known.Contains($_)
    })
    $passedKnown | ForEach-Object { Write-Output "tests: known failure passed this time (remove it if it stays fixed): $_" }
    if ($report.New.Count -gt 0) {
        Write-Output "tests: FAILED, new failures:"
        $report.New | ForEach-Object { Write-Output "  $_" }
        exit 1
    }
    Write-Output "tests: OK ($($report.Known.Count) known failures)"
    exit 0
}
finally {
    Pop-Location
    $env:CARGO_PROFILE_TEST_DEBUG = $debugBefore
}

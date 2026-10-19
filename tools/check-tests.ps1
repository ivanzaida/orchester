# The test part of the quality gate: runs the integration tests of the crates a branch changed, for the pipelines'
# Check step. tools/check.ps1 calls it when asked (`powershell -File tools/check.ps1 -Tests changed`); it runs on its
# own as well: powershell -File tools/check-tests.ps1 [-Scope changed|all] [-Base <revision>] [-TestTimeoutSeconds <n>]
#
# Scope `changed`: the workspace crates with a changed file since the branch left its base (committed, staged,
# unstaged or untracked), and every workspace crate that depends on one of them, as a change can break its tests too.
# A file outside the crates counts for each crate whose sources include it (`include_str!`, `include_bytes!`,
# `include!`). Files tests read at run time from outside their crate are not mapped. A change to the workspace's
# Cargo.toml, Cargo.lock, rust-toolchain.toml or .cargo/ counts for every crate. The frozen orchester-desktop
# (AGENTS.md) is tested only when the branch changes it. Scope `all`: every other crate, to refresh the known failures.
#
# The base: the merge base of HEAD with -Base when given, else the newer of its merge bases with `master` and
# `origin/master` (a run branches off the local `master`, an agent's worktree often off `origin/master`). A pipeline's
# tool cannot pass its run's base; when the run's base is another branch, the merge base with `master` is older, which
# selects more crates than needed: slower, never fewer.
#
# Each test binary of those crates runs on its own, `cargo test -p <crate> --test <binary>`, one after the other, with
# the features its targets require. AGENTS.md rule 14 (a name filter on every `cargo test`) is for runs while working;
# this gate runs whole binaries on purpose, as no filter selects every test. orchester-app's binaries run with
# `--test-threads=4`, so its window tests leave the machine usable. A binary that runs longer than
# -TestTimeoutSeconds (default 30 minutes) is ended with the processes it started. Cargo reads CARGO_BUILD_JOBS
# itself. The tests build with line tables only (CARGO_PROFILE_TEST_DEBUG, unless set already): with full debug
# information each orchester-app test binary brings a PDB of about 900 MB, and every crate's tests together filled
# more than 160 GB.
#
# tools/check-tests-known-failures.txt lists the tests that fail on master and the binaries that run no test on
# purpose. A failing test listed there is reported and does not fail the gate. Any other failing test fails it, as do
# a binary that fails without naming a test (a build error, a crash, a harness of its own), one that times out, and
# one that runs no test without being listed. The output of every binary is in .tmp/check-tests/<run>/.
param(
    [ValidateSet("changed", "all")]
    [string] $Scope = "changed",
    [string] $Base = "",
    [ValidateRange(1, 86400)]
    [int] $TestTimeoutSeconds = 1800
)
$ErrorActionPreference = "Stop"

$repositoryRoot = Split-Path -Parent $PSScriptRoot
$knownFailuresFile = Join-Path $PSScriptRoot "check-tests-known-failures.txt"
$logParent = Join-Path $repositoryRoot ".tmp/check-tests"
$frozenPackage = "orchester-desktop"
$uiPackage = "orchester-app"
$uiTestThreads = 4
$wholeWorkspaceFiles = @("Cargo.toml", "Cargo.lock", "rust-toolchain.toml")
$noTestsName = "(none)"
# Programs a crate's tests start from target/debug/ and do not build themselves (each test says so when one is
# missing): orchester-app's live tests run the runner with its fake agents, and Orchester Cloud. A change to those
# programs' crates therefore selects orchester-app too. They build with the test profile, whose programs land in
# target/debug/ as well, so they share what the tests built.
$programsForTests = @{
    "orchester-app" = @(
        @{ Package = "orchester-runner"; Features = "test-agent" },
        @{ Package = "orchester-cloud"; Features = "" }
    )
}

function Invoke-Git([string[]] $Arguments) {
    $output = & git @Arguments
    if ($LASTEXITCODE -ne 0) { throw "git $($Arguments -join ' ') failed" }
    $output
}

# Paths a git command prints, read with -z so that git neither quotes nor escapes any of them.
function Invoke-GitPaths([string] $Command, [string[]] $Arguments) {
    @((Invoke-Git (@($Command, "-z") + $Arguments)) -join "" -split [char]0 | Where-Object { $_ })
}

function Test-StartsWith([string] $Text, [string] $Prefix) {
    $Text.StartsWith($Prefix, [System.StringComparison]::Ordinal)
}

# Files outside a crate's folder that its sources include, relative to the repository with forward slashes.
function Get-OutsideIncludes([string] $Folder) {
    $absoluteFolder = [System.IO.Path]::GetFullPath((Join-Path $repositoryRoot $Folder))
    $pattern = 'include(?:_str|_bytes)?!\(\s*"([^"]+)"'
    Get-ChildItem -Path $absoluteFolder -Recurse -Filter *.rs -File |
        Where-Object { $_.FullName -cnotmatch '[\\/]target[\\/]' } |
        Select-String -Pattern $pattern -AllMatches -CaseSensitive |
        ForEach-Object {
            $directory = Split-Path -Parent $_.Path
            foreach ($match in $_.Matches) {
                $included = [System.IO.Path]::GetFullPath([System.IO.Path]::Combine($directory, $match.Groups[1].Value))
                if (-not (Test-StartsWith $included $absoluteFolder) -and (Test-StartsWith $included $repositoryRoot)) {
                    $included.Substring($repositoryRoot.Length + 1).Replace("\", "/")
                }
            }
        } | Sort-Object -Unique
}

# The workspace's crates: name, folder relative to the root, the workspace crates its tests rely on (dependencies,
# dev-dependencies and the crates of the programs its tests start), test binaries, the union of the features those
# binaries require, and the files outside its folder it includes.
function Get-WorkspacePackages {
    $metadataJson = cargo metadata --no-deps --format-version 1
    if ($LASTEXITCODE -ne 0) { throw "cargo metadata failed" }
    $metadata = $metadataJson | ConvertFrom-Json
    $root = $metadata.workspace_root.Replace("\", "/").TrimEnd("/") + "/"
    $members = $metadata.packages | Where-Object { $metadata.workspace_members -ccontains $_.id }
    $names = @($members | ForEach-Object { $_.name })
    foreach ($package in $members) {
        $tests = @($package.targets | Where-Object { $_.kind -ccontains "test" })
        $folder = (Split-Path -Parent $package.manifest_path).Replace("\", "/").Replace($root, "") + "/"
        $programs = @($programsForTests[$package.name] | Where-Object { $_ } | ForEach-Object { $_.Package })
        [PSCustomObject]@{
            Name = $package.name
            Folder = $folder
            Dependencies = @(@($package.dependencies | ForEach-Object { $_.name }) + $programs |
                Where-Object { $names -ccontains $_ -and $_ -cne $package.name } | Sort-Object -Unique)
            Tests = @($tests | ForEach-Object { $_.name } | Sort-Object)
            Features = @($tests | ForEach-Object { $_.'required-features' } | Where-Object { $_ } | Sort-Object -Unique)
            Includes = @(Get-OutsideIncludes $folder)
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
    @(Invoke-GitPaths "diff" @("--name-only", "--no-renames", $Revision)) +
        @(Invoke-GitPaths "ls-files" @("--others", "--exclude-standard")) | Sort-Object -Unique -CaseSensitive
}

function Test-PackageChanged($Package, [string[]] $Files) {
    $Files | Where-Object { (Test-StartsWith $_ $Package.Folder) -or $Package.Includes -ccontains $_ } |
        Select-Object -First 1
}

function Select-ChangedPackages($Packages, [string[]] $Files) {
    $wholeWorkspace = @($Files | Where-Object { $wholeWorkspaceFiles -ccontains $_ -or (Test-StartsWith $_ ".cargo/") })
    if ($wholeWorkspace) {
        Write-Output "tests: $($wholeWorkspace -join ', ') changed, which counts for every crate" | Out-Host
        return @($Packages | Where-Object { $_.Name -cne $frozenPackage -or (Test-PackageChanged $_ $Files) } |
            ForEach-Object { $_.Name })
    }
    $changed = @($Packages | Where-Object { Test-PackageChanged $_ $Files } | ForEach-Object { $_.Name })
    $selected = [System.Collections.Generic.List[string]]::new()
    $changed | ForEach-Object { $selected.Add($_) }
    do {
        $added = $Packages | Where-Object {
            $_.Name -cne $frozenPackage -and -not $selected.Contains($_.Name) -and
                ($_.Dependencies | Where-Object { $selected.Contains($_) })
        }
        $added | ForEach-Object { $selected.Add($_.Name) }
    } while ($added)
    if ($changed) { Write-Output "tests: changed crates: $($changed -join ', ')" | Out-Host }
    @($selected)
}

# "<package> <binary> <test name> # reason" per line, or "(none)" for the test name of a binary that runs no test on
# purpose; the reason is required. Returns key -> reason.
function Read-KnownFailures {
    $known = [System.Collections.Generic.Dictionary[string, string]]::new([System.StringComparer]::Ordinal)
    if (-not (Test-Path $knownFailuresFile)) { return $known }
    $number = 0
    foreach ($line in [System.IO.File]::ReadAllLines($knownFailuresFile)) {
        $number++
        $text = $line.Trim()
        if (-not $text -or (Test-StartsWith $text "#")) { continue }
        if ($text -cnotmatch '^(\S+)\s+(\S+)\s+(\S+)\s+#\s*(\S.*)$') {
            throw "check-tests-known-failures.txt:${number}: expected '<package> <binary> <test name> # reason'"
        }
        $known["$($Matches[1]) $($Matches[2]) $($Matches[3])"] = $Matches[4]
    }
    , $known
}

# Ends a process and the processes it started, and nothing else.
function Stop-ProcessTree($Process) {
    if ($PSVersionTable.PSEdition -eq "Desktop") {
        # .NET Framework cannot end a tree. taskkill fails only when the tree has ended already, which is the goal.
        & taskkill /T /F /PID $Process.Id 2>&1 | Out-Null
    }
    else {
        $Process.Kill($true)
    }
    $Process.WaitForExit()
}

# Runs cargo with its output in two files and returns its exit code, or $null when it ran longer than
# $TimeoutSeconds (0: no limit) and was ended. Waits for cargo alone, not for what a test left running.
function Invoke-Cargo([string[]] $Arguments, [string] $LogName, [int] $TimeoutSeconds = 0) {
    $process = Start-Process -FilePath "cargo" -ArgumentList $Arguments -NoNewWindow -PassThru `
        -RedirectStandardOutput (Join-Path $logRoot "$LogName.out.log") `
        -RedirectStandardError (Join-Path $logRoot "$LogName.err.log")
    # Windows PowerShell reports no exit code for a process whose handle was never read.
    if ($PSVersionTable.PSEdition -eq "Desktop") { $null = $process.Handle }
    if ($TimeoutSeconds -gt 0 -and -not $process.WaitForExit($TimeoutSeconds * 1000)) {
        Stop-ProcessTree $process
        return $null
    }
    $process.WaitForExit()
    $process.ExitCode
}

function Get-LogTail([string] $LogName, [string] $Stream, [int] $Count) {
    $path = Join-Path $logRoot "$LogName.$Stream.log"
    if (Test-Path $path) { Get-Content -Path $path -Tail $Count }
}

# What libtest reported for one binary: whether it said how many tests ran and how many (passed and failed, not
# ignored), the tests that passed, and the names in its closing "failures:" list.
function Read-TestRun([string] $LogName) {
    $path = Join-Path $logRoot "$LogName.out.log"
    $lines = @(if (Test-Path $path) { Get-Content -Path $path })
    $run = [PSCustomObject]@{
        Lines = $lines
        Counted = $false
        Ran = 0
        Passed = [System.Collections.Generic.HashSet[string]]::new([System.StringComparer]::Ordinal)
        Failed = @()
        FailuresAt = [Array]::LastIndexOf($lines, "failures:")
    }
    foreach ($line in $lines) {
        if ($line -cmatch '^test result: \w+\. (\d+) passed; (\d+) failed') {
            $run.Counted = $true
            $run.Ran += [int]$Matches[1] + [int]$Matches[2]
        }
        elseif ($line -cmatch '^test (\S+)(?: - should panic)? \.\.\. ok$') { $null = $run.Passed.Add($Matches[1]) }
    }
    # A harness of its own that skips everything says so without a result line.
    if (-not $run.Counted -and ($lines | Where-Object { $_ -cmatch '^running 0 tests' })) { $run.Counted = $true }
    if ($run.FailuresAt -ge 0) {
        $run.Failed = @($lines | Select-Object -Skip ($run.FailuresAt + 1) |
            Where-Object { $_ -cmatch '^    (\S+)$' } | ForEach-Object { $Matches[1] })
    }
    $run
}

# What a failed test printed: its "---- <name> stdout ----" section in libtest's closing report, up to the next failed
# test's section, at most 40 lines of at most 300 characters. Lines the test printed itself do not end it.
function Get-FailureOutput($Run, [string] $Test) {
    $header = { param($name) [Array]::LastIndexOf($Run.Lines, "---- $name stdout ----", $Run.FailuresAt) }
    $start = & $header $Test
    if ($start -lt 0) { return }
    $end = $Run.FailuresAt
    foreach ($other in $Run.Failed) {
        $at = & $header $other
        if ($at -gt $start -and $at -lt $end) { $end = $at }
    }
    $Run.Lines[$start..([Math]::Min($end - 1, $start + 40))] | ForEach-Object {
        if ($_.Length -gt 300) { $_.Substring(0, 300) + " ..." } else { $_ }
    }
}

function Get-FeatureArguments([string[]] $Features) {
    if ($Features) { @("--features", ($Features -join ",")) } else { @() }
}

# Builds a crate's test binaries and the programs they start; false (with the reason reported) when that fails. Its
# report goes to the host, so that the function returns the answer alone.
function Build-PackageTests($Package, $Report) {
    $targets = $Package.Tests | ForEach-Object { "--test"; $_ }
    $buildLog = "$($Package.Name)--build"
    $arguments = @("test", "-p", $Package.Name, "--no-run") + (Get-FeatureArguments $Package.Features) + $targets
    if ((Invoke-Cargo $arguments $buildLog) -ne 0) {
        $Report.New.Add("$($Package.Name): the tests do not build")
        Get-LogTail $buildLog "err" 40 | Out-Host
        return $false
    }
    foreach ($program in @($programsForTests[$Package.Name] | Where-Object { $_ })) {
        $programLog = "$($Package.Name)--$($program.Package)"
        $arguments = @("build", "--profile", "test", "-p", $program.Package, "--bins") +
            (Get-FeatureArguments @($program.Features | Where-Object { $_ }))
        if ((Invoke-Cargo $arguments $programLog) -ne 0) {
            $Report.New.Add("$($Package.Name): $($program.Package), which its tests start, does not build")
            Get-LogTail $programLog "err" 40 | Out-Host
            return $false
        }
    }
    $true
}

# Reports one binary's run and adds what fails the gate to $Report.
function Write-BinaryResult($Package, [string] $Binary, $ExitCode, [string] $Seconds, $Known, $Report) {
    $logName = "$($Package.Name)--$Binary"
    $prefix = "$($Package.Name) $Binary"
    if ($null -eq $ExitCode) {
        Write-Output "   FAILED  $Binary ($Seconds s): timed out after $TestTimeoutSeconds s, ended with its processes"
        $Report.New.Add("$prefix`: timed out after $TestTimeoutSeconds s")
        Get-LogTail $logName "out" 20
        return
    }
    $run = Read-TestRun $logName
    foreach ($key in @($Known.Keys | Where-Object { Test-StartsWith $_ "$prefix " })) {
        if ($run.Passed.Contains($key.Substring($prefix.Length + 1))) { $Report.KnownPassed.Add($key) }
    }
    if ($ExitCode -eq 0 -and $run.Counted -and $run.Ran -eq 0) {
        if ($Known.ContainsKey("$prefix $noTestsName")) {
            Write-Output "   no test $Binary ($Seconds s): expected, $($Known["$prefix $noTestsName"])"
            $Report.Skipped.Add($prefix)
            return
        }
        Write-Output "   FAILED  $Binary ($Seconds s): ran no test"
        $Report.New.Add("$prefix`: ran no test (if on purpose, list it with $noTestsName in the known failures)")
        return
    }
    if ($ExitCode -eq 0) {
        Write-Output "   ok      $Binary ($Seconds s, $(if ($run.Counted) { $run.Ran } else { 'uncounted' }) tests)"
        return
    }
    if (-not $run.Failed -or $ExitCode -ne 101) {
        Write-Output "   FAILED  $Binary ($Seconds s): exited with $ExitCode; $($run.Failed.Count) failed tests named"
        $Report.New.Add("$prefix`: exited with $ExitCode")
        Get-LogTail $logName "out" 20
        Get-LogTail $logName "err" 20
    }
    else {
        Write-Output "   FAILED  $Binary ($Seconds s): $($run.Failed.Count) failed"
    }
    foreach ($test in $run.Failed) {
        $key = "$prefix $test"
        if ($Known.ContainsKey($key)) {
            Write-Output "     known failure: $test"
            $Report.Known.Add($key)
            continue
        }
        Write-Output "     NEW failure: $test"
        Get-FailureOutput $run $test | ForEach-Object { "       $_" }
        $Report.New.Add($key)
    }
}

function Test-Package($Package, $Known, $Report) {
    if (-not $Package.Tests) {
        Write-Output "== tests $($Package.Name): no test binaries"
        return
    }
    $watch = [System.Diagnostics.Stopwatch]::StartNew()
    Write-Output "== tests $($Package.Name): building $($Package.Tests.Count) test binaries"
    if (-not (Build-PackageTests $Package $Report)) { return }
    Write-Output "   built in $([int]$watch.Elapsed.TotalSeconds) s"
    $features = Get-FeatureArguments $Package.Features
    $harness = if ($Package.Name -ceq $uiPackage) { @("--", "--test-threads=$uiTestThreads") } else { @() }
    foreach ($binary in $Package.Tests) {
        $watch.Restart()
        $arguments = @("test", "-p", $Package.Name) + $features + @("--test", $binary) + $harness
        $exitCode = Invoke-Cargo $arguments "$($Package.Name)--$binary" $TestTimeoutSeconds
        $seconds = [Math]::Round($watch.Elapsed.TotalSeconds, 1)
        Write-BinaryResult $Package $binary $exitCode $seconds $Known $Report
    }
}

# A folder of its own for this run's logs. Earlier runs' folders go; one a left-over test process still holds stays
# until a later run, which is why losing that error is right.
function New-LogFolder {
    if (Test-Path $logParent) {
        Get-ChildItem -Path $logParent -Force | Remove-Item -Recurse -Force -ErrorAction SilentlyContinue
    }
    $folder = Join-Path $logParent ("{0:yyyyMMdd-HHmmss}-{1}" -f (Get-Date), $PID)
    New-Item -ItemType Directory -Force -Path $folder | Out-Null
    $folder
}

# Runs the tests of the selected crates and prints the summary, also when the run stopped part way. Sets the exit
# code in $script:testsExitCode.
function Invoke-SelectedTests($Packages, [string[]] $Selected) {
    $known = Read-KnownFailures
    $script:logRoot = New-LogFolder
    $report = [PSCustomObject]@{
        New = [System.Collections.Generic.List[string]]::new()
        Known = [System.Collections.Generic.List[string]]::new()
        KnownPassed = [System.Collections.Generic.List[string]]::new()
        Skipped = [System.Collections.Generic.List[string]]::new()
    }
    $total = [System.Diagnostics.Stopwatch]::StartNew()
    try {
        foreach ($package in $Packages | Where-Object { $Selected -ccontains $_.Name }) {
            Test-Package $package $known $report
        }
    }
    catch {
        $report.New.Add("the test step stopped: $_")
    }
    $relative = $script:logRoot.Substring($repositoryRoot.Length + 1).Replace("\", "/")
    Write-Output "tests: $([int]$total.Elapsed.TotalSeconds) s, logs in $relative/"
    $report.KnownPassed | ForEach-Object { Write-Output "tests: known failure passed (remove it once fixed): $_" }
    if ($report.New.Count -gt 0) {
        Write-Output "tests: FAILED, new failures:"
        $report.New | ForEach-Object { Write-Output "  $_" }
        $script:testsExitCode = 1
        return
    }
    Write-Output "tests: OK ($($report.Known.Count) known failures, $($report.Skipped.Count) binaries without tests)"
    $script:testsExitCode = 0
}

$debugBefore = $env:CARGO_PROFILE_TEST_DEBUG
if (-not $debugBefore) { $env:CARGO_PROFILE_TEST_DEBUG = "line-tables-only" }
Push-Location $repositoryRoot
try {
    $packages = @(Get-WorkspacePackages)
    if ($Scope -ceq "all") {
        $selected = @($packages | Where-Object { $_.Name -cne $frozenPackage } | ForEach-Object { $_.Name })
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
    Invoke-SelectedTests $packages $selected
    exit $script:testsExitCode
}
finally {
    Pop-Location
    $env:CARGO_PROFILE_TEST_DEBUG = $debugBefore
}

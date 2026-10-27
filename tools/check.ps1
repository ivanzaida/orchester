# Repository quality gate: formatting, clippy with warnings denied, the 600-line file limit and the web checks.
# Run from anywhere: powershell -File tools/check.ps1
#
# With `-Tests changed` it also runs the tests of the crates the branch changed and of the crates that depend on them
# (tools/check-tests.ps1, which explains the choice), as the pipelines' Check step does; `-Tests all` runs every
# crate's tests. `-Base <revision>` and `-TestTimeoutSeconds <n>` go to it. Without -Tests no test runs, so the check
# agents run by hand stays fast. When clippy fails, the tests are skipped: they would not build. A rustfmt failure
# alone does not skip them, so one return to the agent carries both.

param(
    [ValidateSet("changed", "all")]
    [string] $Tests = "",
    [string] $Base = "",
    [int] $TestTimeoutSeconds = 0
)
$ErrorActionPreference = "Stop"

$repositoryRoot = Split-Path -Parent $PSScriptRoot
$maxLines = 599
$failed = $false
$clippyFailed = $false

Push-Location $repositoryRoot
try {
    # rustfmt.toml uses nightly-only options; the pinned stable toolchain would ignore them.
    Write-Output "== rustfmt"
    # One rustfmt invocation for the whole workspace exceeds Windows' command-line limit.
    $formatMetadataJson = cargo +nightly metadata --no-deps --format-version 1
    if ($LASTEXITCODE -ne 0) {
        $failed = $true
    }
    else {
        $formatMetadata = $formatMetadataJson | ConvertFrom-Json
        foreach ($formatPackage in $formatMetadata.packages) {
            if ($formatMetadata.workspace_members -contains $formatPackage.id) {
                Write-Output "== rustfmt $($formatPackage.name)"
                cargo +nightly fmt -p $formatPackage.name -- --check
                if ($LASTEXITCODE -ne 0) { $failed = $true }
            }
        }
    }

    Write-Output "== clippy"
    cargo clippy --workspace --all-targets --all-features -- -D warnings
    if ($LASTEXITCODE -ne 0) { $clippyFailed = $true }
    if ($clippyFailed) { $failed = $true }

    Write-Output "== file size (max $maxLines lines)"
    $oversized = Get-ChildItem -Path (Join-Path $repositoryRoot "crates") -Recurse -Filter *.rs -File |
        Where-Object { $_.FullName -notmatch '[\\/]target[\\/]' } |
        ForEach-Object {
            $lines = [System.IO.File]::ReadAllLines($_.FullName).Length
            if ($lines -gt $maxLines) {
                [PSCustomObject]@{ Lines = $lines; File = $_.FullName.Substring($repositoryRoot.Length + 1) }
            }
        }
    if ($oversized) {
        $oversized | Sort-Object Lines -Descending | Format-Table -AutoSize | Out-String | Write-Output
        $failed = $true
    }

    # UI code styles through lurq theme roles and shows text through i18n keys. Design
    # values live only in each app's src/theme/. See .claude/skills/orchester-ui.
    Write-Output "== ui literals (theme roles and i18n only)"
    $uiRules = @(
        @{ Rule = "hex color literal"; Pattern = '"#[0-9A-Fa-f]{3,8}"' },
        @{ Rule = "concrete color"; Pattern = '\bColor::(from_hex|rgb|rgba|new)\b' },
        @{ Rule = "inline text style"; Pattern = '\bTextStyle\s*\{|\bText::styled\(|\bfont_family\b|\bfont_size\b' },
        @{ Rule = "hardcoded user-facing text"; Pattern = '\b(Text|Button)::new\(\s*"|\.(placeholder|with_title|tooltip|label)\(\s*"' },
        @{ Rule = "numeric spacing/radius/border"; Pattern = '\.(padding\w*|spacing|gap|rounded|corner_radius\w*|border\w*)\(\s*-?[0-9]' }
    )
    # crates/desktop, the deprecated app, is no longer built (AGENTS.md); only the app's sources are checked.
    $uiRoots = @([System.IO.Path]::Combine($repositoryRoot, "crates", "desktop-app", "src"))
    $violations = $uiRoots | ForEach-Object {
        $themeRoot = [System.IO.Path]::Combine($_, "theme")
        Get-ChildItem -Path $_ -Recurse -Filter *.rs -File |
            Where-Object { -not $_.FullName.StartsWith($themeRoot) }
    } |
        ForEach-Object {
            $file = $_
            foreach ($rule in $uiRules) {
                Select-String -Path $file.FullName -Pattern $rule.Pattern | ForEach-Object {
                    [PSCustomObject]@{
                        Rule = $rule.Rule
                        Location = "$($file.FullName.Substring($repositoryRoot.Length + 1)):$($_.LineNumber)"
                        Line = $_.Line.Trim()
                    }
                }
            }
        }
    if ($violations) {
        $violations | Format-Table -AutoSize -Wrap | Out-String -Width 200 | Write-Output
        $failed = $true
    }

    # The web apps (web/, an npm workspace): lint, type check and unit tests of every package, with the same rules
    # (docs/architecture/server.md, "The web workspace"). Only where Node is installed; said plainly when it is not.
    Write-Output "== web"
    if (Get-Command node -ErrorAction SilentlyContinue) {
        npm --prefix web ci --ignore-scripts
        if ($LASTEXITCODE -ne 0) { $failed = $true }
        else {
            npm --prefix web run check
            if ($LASTEXITCODE -ne 0) { $failed = $true }
        }
    }
    else {
        Write-Output "web: skipped, Node (node) is not on PATH; the web checks did not run"
    }

    if ($Tests -and $clippyFailed) {
        Write-Output "== tests ($Tests): skipped, clippy failed above; the tests run once it passes"
    }
    elseif ($Tests) {
        Write-Output "== tests ($Tests)"
        $testArguments = @{ Scope = $Tests }
        if ($Base) { $testArguments.Base = $Base }
        if ($TestTimeoutSeconds -gt 0) { $testArguments.TestTimeoutSeconds = $TestTimeoutSeconds }
        try {
            & (Join-Path $PSScriptRoot "check-tests.ps1") @testArguments
            if ($LASTEXITCODE -ne 0) { $failed = $true }
        }
        catch {
            Write-Output "tests: could not run: $_"
            $failed = $true
        }
    }
}
finally {
    Pop-Location
}

if ($failed) {
    Write-Output "check: FAILED"
    exit 1
}
Write-Output "check: OK"

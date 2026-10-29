$ErrorActionPreference = "Stop"

# Windows links .agents/skills as a junction, macOS as a relative symbolic link. Windows PowerShell 5.1 has no
# $IsWindows; it only runs on Windows.
$onWindows = $PSVersionTable.PSEdition -eq "Desktop" -or $IsWindows
$linkType = if ($onWindows) { "Junction" } else { "SymbolicLink" }

$repositoryRoot = Split-Path -Parent $PSScriptRoot
$canonicalSkills = [System.IO.Path]::GetFullPath(
    (Join-Path (Join-Path $repositoryRoot ".claude") "skills")
)
$agentsDirectory = Join-Path $repositoryRoot ".agents"
$codexSkills = Join-Path $agentsDirectory "skills"

if (-not (Test-Path -LiteralPath $canonicalSkills -PathType Container)) {
    throw "Canonical skill directory does not exist: $canonicalSkills"
}

if (Test-Path -LiteralPath $codexSkills) {
    $existing = Get-Item -LiteralPath $codexSkills -Force
    # A relative link target is relative to the folder of the link.
    $targets = @($existing.Target) | ForEach-Object {
        if ([System.IO.Path]::IsPathRooted($_)) { $target = $_ } else { $target = Join-Path $agentsDirectory $_ }
        [System.IO.Path]::GetFullPath($target).TrimEnd([System.IO.Path]::DirectorySeparatorChar)
    }
    if ($existing.LinkType -eq $linkType -and $targets -contains $canonicalSkills) {
        Write-Output "Codex skill link is already correct: $codexSkills -> $canonicalSkills"
        exit 0
    }
    throw "Refusing to replace existing non-matching path: $codexSkills"
}

if (-not (Test-Path -LiteralPath $agentsDirectory -PathType Container)) {
    New-Item -ItemType Directory -Path $agentsDirectory | Out-Null
}

if ($onWindows) {
    New-Item -ItemType Junction -Path $codexSkills -Target $canonicalSkills | Out-Null
} else {
    New-Item -ItemType SymbolicLink -Path $codexSkills -Target (Join-Path (Join-Path ".." ".claude") "skills") | Out-Null
}
Write-Output "Created Codex skill link: $codexSkills -> $canonicalSkills"

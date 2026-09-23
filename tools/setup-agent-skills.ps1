$ErrorActionPreference = "Stop"

$repositoryRoot = Split-Path -Parent $PSScriptRoot
$canonicalSkills = [System.IO.Path]::GetFullPath(
    (Join-Path $repositoryRoot ".claude\skills")
)
$agentsDirectory = Join-Path $repositoryRoot ".agents"
$codexSkills = Join-Path $agentsDirectory "skills"

if (-not (Test-Path -LiteralPath $canonicalSkills -PathType Container)) {
    throw "Canonical skill directory does not exist: $canonicalSkills"
}

if (Test-Path -LiteralPath $codexSkills) {
    $existing = Get-Item -LiteralPath $codexSkills -Force
    $targets = @($existing.Target) | ForEach-Object {
        [System.IO.Path]::GetFullPath($_)
    }
    if ($existing.LinkType -eq "Junction" -and $targets -contains $canonicalSkills) {
        Write-Output "Codex skill junction is already correct: $codexSkills -> $canonicalSkills"
        exit 0
    }
    throw "Refusing to replace existing non-matching path: $codexSkills"
}

if (-not (Test-Path -LiteralPath $agentsDirectory -PathType Container)) {
    New-Item -ItemType Directory -Path $agentsDirectory | Out-Null
}

New-Item -ItemType Junction -Path $codexSkills -Target $canonicalSkills | Out-Null
Write-Output "Created Codex skill junction: $codexSkills -> $canonicalSkills"

# Writes the Docker Hub page of the self-hosted team server: deploy/server/DOCKERHUB.md with every marker line
# `<!-- inline: <file> -->` replaced by that file of deploy/server, whole and byte for byte, in a code block. The
# repository is private, so this page is the only guide self-hosters see; generating it from the files keeps the
# copies there from drifting. server-release.yml publishes it to both image repositories.
#   powershell -File tools/dockerhub-description.ps1 [-Out <path>]   (default: .tmp/dockerhub/description.md)
# Fails if the page is longer than Docker Hub's 25,000-character limit for a repository description.
param(
    [string]$Out
)
$ErrorActionPreference = "Stop"

$repositoryRoot = Split-Path -Parent $PSScriptRoot
$serverFolder = Join-Path $repositoryRoot "deploy/server"
$limit = 25000
if (-not $Out) {
    $Out = Join-Path $repositoryRoot ".tmp/dockerhub/description.md"
}
# .NET writes relative to the process's directory, not PowerShell's location.
$Out = [System.IO.Path]::GetFullPath([System.IO.Path]::Combine((Get-Location).ProviderPath, $Out))
$utf8 = New-Object System.Text.UTF8Encoding($false)
$languages = @{ ".yaml" = "yaml"; ".example" = "sh" }

$template = [System.IO.File]::ReadAllText((Join-Path $serverFolder "DOCKERHUB.md"), $utf8)
if ($template.Contains("`r")) {
    throw "deploy/server/DOCKERHUB.md has CRLF line endings; the page is built from LF files"
}
$page = [regex]::Replace($template, "(?m)^<!-- inline: (?<file>[^ ]+) -->$", {
    param($match)
    $name = $match.Groups["file"].Value
    $path = Join-Path $serverFolder $name
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "DOCKERHUB.md inlines $name, which is not in deploy/server"
    }
    $content = [System.IO.File]::ReadAllText($path, $utf8)
    if ($content.Contains("`r") -or -not $content.EndsWith("`n") -or $content.Contains('```')) {
        throw "deploy/server/$name must use LF line endings, end with a newline and hold no code fence"
    }
    $language = $languages[[System.IO.Path]::GetExtension($name)]
    # The file ends with its own newline, so the closing fence starts a line of its own.
    return '```' + $language + "`n" + $content + '```'
})
if ($page -match "(?m)^<!-- inline:") {
    throw "DOCKERHUB.md has a marker line that is not '<!-- inline: <file> -->'"
}

$characters = $page.Length
$bytes = $utf8.GetByteCount($page)
New-Item -ItemType Directory -Force -Path (Split-Path -Parent $Out) | Out-Null
[System.IO.File]::WriteAllText($Out, $page, $utf8)
Write-Output "Wrote $Out`: $characters characters, $bytes bytes (Docker Hub allows $limit)"
if ($characters -gt $limit -or $bytes -gt $limit) {
    throw "The Docker Hub page is longer than $limit; shorten deploy/server/DOCKERHUB.md"
}

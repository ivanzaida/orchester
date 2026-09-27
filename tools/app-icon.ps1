# Packs Orchester's application icon PNGs into the Windows .ico and the macOS .icns.
# Run from anywhere: powershell -File tools/app-icon.ps1 (Windows PowerShell 5.1 or pwsh 7 on
# Windows and macOS). It only reads and writes files under crates/desktop/assets/icon and needs no
# network, no image library and no macOS tools; the same PNGs always give byte-identical output.
#
# The PNGs are the artwork, exported from the Pencil design (data/orchester.pen, section
# "Brand · App icon" in "00 / Foundations") with the pencil MCP `export_nodes`:
#
#   windows/orchester-16.png   "App icon / 16"    lyAVK  scale 1
#   windows/orchester-24.png   "App icon / 24"    E0QZ3  scale 1
#   windows/orchester-32.png   "App icon / 32"    B7Pgr  scale 1
#   windows/orchester-48.png   "App icon / 48"    U5I3Pf scale 1
#   windows/orchester-64.png   "App icon"         Dzics  scale 0.25
#   windows/orchester-256.png  "App icon"         Dzics  scale 1   (also the runtime window icon)
#   windows/orchester-512.png  "App icon"         Dzics  scale 2
#   orchester.iconset/icon_<n>x<n>[@2x].png  "App icon / macOS"  PmIu1  scale = pixels / 256
#
# 16 to 48 are drawn separately on the pixel grid so they stay sharp; everything else is the master.
# The macOS artwork sits on Apple's 824-on-1024 icon grid, which a full-bleed Windows tile does not.
# orchester.iconset is also a valid input for `iconutil -c icns` on a Mac.
$ErrorActionPreference = "Stop"

$iconRoot = Join-Path (Split-Path -Parent $PSScriptRoot) "crates/desktop/assets/icon"

# Reads a PNG and checks that it is one and has the expected square size.
function Read-Png([string]$relativePath, [int]$size) {
    $path = Join-Path $iconRoot $relativePath
    $bytes = [System.IO.File]::ReadAllBytes($path)
    $signature = [byte[]](0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A)
    for ($i = 0; $i -lt 8; $i++) {
        if ($bytes[$i] -ne $signature[$i]) { throw "$relativePath is not a PNG file" }
    }
    # IHDR follows the signature and its chunk header: width and height, big-endian.
    $width = ([int]$bytes[16] -shl 24) -bor ([int]$bytes[17] -shl 16) -bor ([int]$bytes[18] -shl 8) -bor $bytes[19]
    $height = ([int]$bytes[20] -shl 24) -bor ([int]$bytes[21] -shl 16) -bor ([int]$bytes[22] -shl 8) -bor $bytes[23]
    if ($width -ne $size -or $height -ne $size) {
        throw "$relativePath is ${width}x${height}, expected ${size}x${size}"
    }
    return , $bytes
}

function Write-BigEndian([System.IO.Stream]$stream, [uint32]$value) {
    $stream.WriteByte(($value -shr 24) -band 0xFF)
    $stream.WriteByte(($value -shr 16) -band 0xFF)
    $stream.WriteByte(($value -shr 8) -band 0xFF)
    $stream.WriteByte($value -band 0xFF)
}

# ICO: a 6-byte header, one 16-byte directory entry per image, then the images. Every image is
# stored as PNG, which Windows reads at every size since Vista.
function Write-Ico([string]$relativePath, [int[]]$sizes) {
    $images = foreach ($size in $sizes) { , (Read-Png "windows/orchester-$size.png" $size) }
    $stream = New-Object System.IO.MemoryStream
    $writer = New-Object System.IO.BinaryWriter($stream)
    $writer.Write([uint16]0)
    $writer.Write([uint16]1)
    $writer.Write([uint16]$sizes.Count)
    $offset = 6 + 16 * $sizes.Count
    for ($i = 0; $i -lt $sizes.Count; $i++) {
        # A width or height of 256 does not fit the byte and is written as 0.
        $dimension = [byte]($sizes[$i] % 256)
        $writer.Write($dimension)
        $writer.Write($dimension)
        $writer.Write([byte]0)
        $writer.Write([byte]0)
        $writer.Write([uint16]1)
        $writer.Write([uint16]32)
        $writer.Write([uint32]$images[$i].Length)
        $writer.Write([uint32]$offset)
        $offset += $images[$i].Length
    }
    foreach ($image in $images) { $writer.Write($image) }
    $writer.Flush()
    [System.IO.File]::WriteAllBytes((Join-Path $iconRoot $relativePath), $stream.ToArray())
    $writer.Dispose()
}

# ICNS: the 'icns' magic and total length, then per image a 4-character type, the entry length
# (header included) and PNG data. All lengths are big-endian.
function Write-Icns([string]$relativePath, [object[]]$entries) {
    $body = New-Object System.IO.MemoryStream
    foreach ($entry in $entries) {
        $image = Read-Png "orchester.iconset/$($entry.File)" $entry.Size
        $body.Write([System.Text.Encoding]::ASCII.GetBytes($entry.Type), 0, 4)
        Write-BigEndian $body ([uint32](8 + $image.Length))
        $body.Write($image, 0, $image.Length)
    }
    $stream = New-Object System.IO.MemoryStream
    $stream.Write([System.Text.Encoding]::ASCII.GetBytes("icns"), 0, 4)
    Write-BigEndian $stream ([uint32](8 + $body.Length))
    $body.WriteTo($stream)
    [System.IO.File]::WriteAllBytes((Join-Path $iconRoot $relativePath), $stream.ToArray())
}

Write-Ico "orchester.ico" @(16, 24, 32, 48, 64, 256)

# The PNG entry types of Apple's icon family (icns), largest scale factors included.
Write-Icns "orchester.icns" @(
    @{ Type = "icp4"; File = "icon_16x16.png"; Size = 16 },
    @{ Type = "ic11"; File = "icon_16x16@2x.png"; Size = 32 },
    @{ Type = "icp5"; File = "icon_32x32.png"; Size = 32 },
    @{ Type = "ic12"; File = "icon_32x32@2x.png"; Size = 64 },
    @{ Type = "ic07"; File = "icon_128x128.png"; Size = 128 },
    @{ Type = "ic13"; File = "icon_128x128@2x.png"; Size = 256 },
    @{ Type = "ic08"; File = "icon_256x256.png"; Size = 256 },
    @{ Type = "ic14"; File = "icon_256x256@2x.png"; Size = 512 },
    @{ Type = "ic09"; File = "icon_512x512.png"; Size = 512 },
    @{ Type = "ic10"; File = "icon_512x512@2x.png"; Size = 1024 }
)

Write-Output "app icon: wrote orchester.ico and orchester.icns in $iconRoot"

<#
    make-hero-gif.ps1 - build the README hero GIF from the captured
    screenshots, fully deterministic + autonomous.

    What it does:
      1. Reads the three PNGs from `assets/screenshots/`:
         settings.png, popup.png, popup-sensitive.png
      2. Pads each to a common 800x680 frame on a near-black backdrop so
         the slideshow doesn't jitter between aspect ratios.
      3. Composes a 4-scene slideshow with smooth xfade transitions:
            popup -> popup-sensitive -> settings -> popup (loop)
         3 seconds per scene, 0.6 second cross-fade.
      4. Generates an optimised palette.
      5. Outputs `assets/hero.gif`, ~720 px wide.

    Usage:
        .\scripts\make-hero-gif.ps1
        .\scripts\make-hero-gif.ps1 -OutDir .\assets

    The result is a deterministic, reproducible hero GIF that doesn't
    require a manual screen recording. Run `.\scripts\capture-screenshots.ps1`
    first if the source PNGs aren't fresh.
#>

[CmdletBinding()]
param(
    [string] $InDir   = ".\assets\screenshots",
    [string] $OutDir  = ".\assets",
    [int]    $FrameW  = 800,
    [int]    $FrameH  = 680,
    [int]    $Fps     = 15,
    [double] $SceneSec = 3.0,
    [double] $XfadeSec = 0.6,
    [int]    $MaxWidth = 720
)

$ErrorActionPreference = "Stop"

if (-not [System.IO.Path]::IsPathRooted($InDir)) {
    $InDir = Join-Path (Get-Location).Path $InDir
}
if (-not [System.IO.Path]::IsPathRooted($OutDir)) {
    $OutDir = Join-Path (Get-Location).Path $OutDir
}
$InDir  = (Resolve-Path -LiteralPath $InDir).Path
New-Item -ItemType Directory -Force -Path $OutDir | Out-Null
$OutDir = (Resolve-Path -LiteralPath $OutDir).Path

# Stderr-aware ffmpeg wrapper (PS5.1 wraps native stderr in
# NativeCommandError; with EAP=Stop that aborts on success).
function Invoke-Ffmpeg {
    param([Parameter(Mandatory)] [string[]] $FfmpegArgs)
    $prev = $ErrorActionPreference
    $ErrorActionPreference = 'Continue'
    & $ffmpeg @FfmpegArgs
    $ec = $LASTEXITCODE
    $ErrorActionPreference = $prev
    if ($ec -ne 0) { throw "ffmpeg exited with code $ec" }
}

$ffmpeg = (Get-Command ffmpeg -ErrorAction SilentlyContinue).Source
if (-not $ffmpeg) {
    $candidate = Get-ChildItem "$HOME\ffmpeg*" -Directory -ErrorAction SilentlyContinue |
        ForEach-Object { Join-Path $_.FullName "bin\ffmpeg.exe" } |
        Where-Object { Test-Path $_ } | Select-Object -First 1
    if ($candidate) { $ffmpeg = $candidate }
}
if (-not $ffmpeg) {
    Write-Error "ffmpeg not found. Install via: winget install --id Gyan.FFmpeg"
    exit 1
}

# ----------------------------------------------------------------------
# Source PNGs
# ----------------------------------------------------------------------

$popup    = Join-Path $InDir "popup.png"
$sens     = Join-Path $InDir "popup-sensitive.png"
$settings = Join-Path $InDir "settings.png"
foreach ($p in @($popup, $sens, $settings)) {
    if (-not (Test-Path $p)) {
        Write-Error "Missing source PNG: $p (run scripts\capture-screenshots.ps1 first)"
        exit 1
    }
}

# ----------------------------------------------------------------------
# Slideshow
# ----------------------------------------------------------------------

# Each input is loaded as a still image with -loop 1, padded to FrameW x
# FrameH on a dark background (#0a0a0f matches Klipo's Mica popup tone).
# Then xfade chains them: A -> B (offset = SceneSec - XfadeSec/... ).
#
# Scene order: popup (capture+search) -> popup-sensitive (guard) ->
# settings (depth) -> popup (loop back)
#
# Total length = 4 * SceneSec - 3 * XfadeSec (overlap) ~ 10.2 s

$bg = "0a0a0f"

$padFilter = "scale=${FrameW}:${FrameH}:force_original_aspect_ratio=decrease,pad=${FrameW}:${FrameH}:(ow-iw)/2:(oh-ih)/2:color=0x${bg},setsar=1,fps=${Fps},format=rgba"

$o1 = $SceneSec - $XfadeSec
$o2 = $o1 + ($SceneSec - $XfadeSec)
$o3 = $o2 + ($SceneSec - $XfadeSec)

$fc = "[0:v]${padFilter}[v0];[1:v]${padFilter}[v1];[2:v]${padFilter}[v2];[3:v]${padFilter}[v3];" +
      "[v0][v1]xfade=transition=fade:duration=${XfadeSec}:offset=${o1}[v01];" +
      "[v01][v2]xfade=transition=fade:duration=${XfadeSec}:offset=${o2}[v012];" +
      "[v012][v3]xfade=transition=fade:duration=${XfadeSec}:offset=${o3},scale=${MaxWidth}:-2:flags=lanczos[vout]"

$mp4Path = Join-Path $OutDir "hero.mp4"
$paletteFile = Join-Path $OutDir "hero-palette.png"
$gifPath = Join-Path $OutDir "hero.gif"

$mp4Args = @(
    "-y",
    "-loop", "1", "-t", "$SceneSec", "-i", $popup,
    "-loop", "1", "-t", "$SceneSec", "-i", $sens,
    "-loop", "1", "-t", "$SceneSec", "-i", $settings,
    "-loop", "1", "-t", "$SceneSec", "-i", $popup,
    "-filter_complex", $fc,
    "-map", "[vout]",
    "-c:v", "libx264",
    "-pix_fmt", "yuv420p",
    "-preset", "veryfast",
    "-crf", "20",
    "$mp4Path"
)

Write-Host "Composing slideshow MP4..."
Invoke-Ffmpeg -FfmpegArgs $mp4Args
$mp4Size = (Get-Item $mp4Path).Length
Write-Host ("MP4 written: {0} ({1:N0} bytes)" -f $mp4Path, $mp4Size)

# Palette + GIF (single-frame palette via -frames:v 1)
$paletteArgs = @(
    "-y",
    "-i", "$mp4Path",
    "-vf", "fps=$Fps,palettegen=stats_mode=full",
    "-frames:v", "1",
    "$paletteFile"
)
Write-Host ""
Write-Host "Generating colour palette..."
Invoke-Ffmpeg -FfmpegArgs $paletteArgs

$gifArgs = @(
    "-y",
    "-i", "$mp4Path",
    "-i", "$paletteFile",
    "-lavfi", "fps=$Fps[x];[x][1:v]paletteuse=dither=bayer:bayer_scale=5",
    "-loop", "0",
    "$gifPath"
)
Write-Host "Composing GIF..."
Invoke-Ffmpeg -FfmpegArgs $gifArgs

if (Test-Path $paletteFile) { Remove-Item $paletteFile -Force }

if (Test-Path $gifPath) {
    $gifSize = (Get-Item $gifPath).Length
    Write-Host ""
    Write-Host ("GIF written: {0} ({1:N0} bytes / {2:N1} MB)" -f $gifPath, $gifSize, ($gifSize / 1MB))
    if ($gifSize -gt 5MB) {
        Write-Warning "GIF is over 5 MB. Consider reducing -MaxWidth or -SceneSec."
    }
} else {
    Write-Error "GIF generation failed."
    exit 1
}

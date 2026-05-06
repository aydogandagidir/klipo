<#
    record-demo.ps1 - record a demo MP4 of Klipo and convert it to a
    palette-optimised GIF suitable for the README hero.

    Usage:
        .\scripts\record-demo.ps1
        .\scripts\record-demo.ps1 -Seconds 20 -OutDir .\assets

    What it does:
      1. Locates the Klipo popup window (must be running).
      2. Reads the popup's screen rect; expands the capture box ~120 px
         outward so a target app behind the popup also lands in frame.
      3. Counts down 3 seconds, then records the box for `-Seconds`
         seconds via ffmpeg's gdigrab device.
      4. Generates an optimised 256-colour palette from the MP4.
      5. Combines the palette with the MP4 into a 15 fps GIF, scaled to
         max 720 px wide.
      6. Writes both `demo.mp4` and `demo.gif` into `-OutDir`.

    During the 3-second countdown you should ALREADY be set up:
      - Klipo open in the tray
      - Whatever target app you want as backdrop visible (Notepad with
        some text, a browser, terminal, etc.)
      - Your hands on the keyboard ready to press the hotkey

    During the recording window:
      - Press Ctrl+Alt+V (Klipo summons)
      - Type a few letters to trigger search filtering
      - Press Enter to paste back into the target app
      - Optionally repeat the round-trip 1-2 more times

    This script needs ffmpeg on PATH (`where ffmpeg`). If absent, install
    via `winget install --id Gyan.FFmpeg`.
#>

[CmdletBinding()]
param(
    [string] $OutDir   = ".\assets",
    [int]    $Seconds  = 15,
    [int]    $PadPx    = 120,
    [int]    $Fps      = 15,
    [int]    $MaxWidth = 720
)

$ErrorActionPreference = "Stop"

# ffmpeg writes its banner + progress to stderr, which PowerShell 5.1
# wraps in NativeCommandError records. With $ErrorActionPreference=Stop
# that aborts the script even on a successful ffmpeg run. Wrap ffmpeg
# invocations so they suppress that machinery, then key off $LASTEXITCODE
# alone for success.
function Invoke-Ffmpeg {
    param([Parameter(Mandatory)] [string[]] $FfmpegArgs)
    $prev = $ErrorActionPreference
    $ErrorActionPreference = 'Continue'
    & $ffmpeg @FfmpegArgs
    $ec = $LASTEXITCODE
    $ErrorActionPreference = $prev
    if ($ec -ne 0) {
        throw "ffmpeg exited with code $ec"
    }
}

# Resolve out dir to absolute path so subsequent commands don't depend on CWD.
if (-not [System.IO.Path]::IsPathRooted($OutDir)) {
    $OutDir = Join-Path (Get-Location).Path $OutDir
}
New-Item -ItemType Directory -Force -Path $OutDir | Out-Null
$OutDir = (Resolve-Path -LiteralPath $OutDir).Path

$mp4Path     = Join-Path $OutDir "demo.mp4"
$paletteFile = Join-Path $OutDir "demo-palette.png"
$gifPath     = Join-Path $OutDir "demo.gif"

# ----------------------------------------------------------------------
# ffmpeg discovery
# ----------------------------------------------------------------------

$ffmpeg = (Get-Command ffmpeg -ErrorAction SilentlyContinue).Source
if (-not $ffmpeg) {
    # Common manual-install location.
    $candidate = Get-ChildItem "$HOME\ffmpeg*" -Directory -ErrorAction SilentlyContinue |
        ForEach-Object { Join-Path $_.FullName "bin\ffmpeg.exe" } |
        Where-Object { Test-Path $_ } |
        Select-Object -First 1
    if ($candidate) { $ffmpeg = $candidate }
}
if (-not $ffmpeg) {
    Write-Error "ffmpeg not found. Install via 'winget install --id Gyan.FFmpeg' and re-run."
    exit 1
}
Write-Host "Using ffmpeg: $ffmpeg"

# ----------------------------------------------------------------------
# Win32 interop just for finding the popup rect
# ----------------------------------------------------------------------

Add-Type -TypeDefinition @"
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;

public class WinPopup
{
    public delegate bool EnumWindowsProc(IntPtr hWnd, IntPtr lParam);

    [DllImport("user32.dll")]
    public static extern bool EnumWindows(EnumWindowsProc enumProc, IntPtr lParam);

    [DllImport("user32.dll", CharSet = CharSet.Auto)]
    public static extern int GetWindowText(IntPtr hWnd, StringBuilder lpString, int nMaxCount);

    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr hWnd, out RECT lpRect);
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr hWnd, out uint lpdwProcessId);

    [StructLayout(LayoutKind.Sequential)]
    public struct RECT { public int Left, Top, Right, Bottom; }

    public static IntPtr FindByPidAndTitle(uint targetPid, string title)
    {
        IntPtr result = IntPtr.Zero;
        EnumWindows((hWnd, lParam) => {
            uint pid = 0;
            GetWindowThreadProcessId(hWnd, out pid);
            if (pid == targetPid) {
                var sb = new StringBuilder(256);
                GetWindowText(hWnd, sb, sb.Capacity);
                if (sb.ToString() == title) {
                    result = hWnd;
                    return false; // stop
                }
            }
            return true;
        }, IntPtr.Zero);
        return result;
    }
}
"@

$klipo = Get-Process -Name klipo -ErrorAction SilentlyContinue
if (-not $klipo) {
    Write-Error "Klipo is not running. Press Win, type 'Klipo', hit Enter, then re-run."
    exit 1
}
$klipoPid = [uint32]$klipo[0].Id

$popupHwnd = [WinPopup]::FindByPidAndTitle($klipoPid, "Klipo")
if ($popupHwnd -eq [IntPtr]::Zero) {
    Write-Error "Klipo popup window not found. Re-launch Klipo and re-run."
    exit 1
}

$rect = New-Object WinPopup+RECT
[WinPopup]::GetWindowRect($popupHwnd, [ref]$rect) | Out-Null
$popupW = $rect.Right - $rect.Left
$popupH = $rect.Bottom - $rect.Top

if ($popupW -le 0 -or $popupH -le 0) {
    # Popup currently hidden; use its known geometry from tauri.conf.json
    # (480x600) centred on the primary monitor as a sane default.
    Add-Type -AssemblyName System.Windows.Forms
    $bounds = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
    $popupW = 480; $popupH = 600
    $rect.Left   = ($bounds.Width  - $popupW) / 2
    $rect.Top    = ($bounds.Height - $popupH) / 2
    $rect.Right  = $rect.Left + $popupW
    $rect.Bottom = $rect.Top  + $popupH
    Write-Host "Popup hidden; defaulting capture region to centred 480x600."
} else {
    Write-Host ("Popup at ({0},{1}) size {2}x{3}" -f $rect.Left, $rect.Top, $popupW, $popupH)
}

# Expand by PadPx in each direction so the app behind Klipo (the paste
# target) is visible. Clamp to primary monitor bounds.
Add-Type -AssemblyName System.Windows.Forms
$screenBounds = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds

$capLeft   = [Math]::Max(0,                      $rect.Left   - $PadPx)
$capTop    = [Math]::Max(0,                      $rect.Top    - $PadPx)
$capRight  = [Math]::Min($screenBounds.Width,    $rect.Right  + $PadPx)
$capBottom = [Math]::Min($screenBounds.Height,   $rect.Bottom + $PadPx)

# ffmpeg gdigrab requires width/height to be even (h264 / palette quirk).
$capW = $capRight  - $capLeft
$capH = $capBottom - $capTop
if ($capW % 2) { $capW -= 1 }
if ($capH % 2) { $capH -= 1 }

Write-Host ("Capture region: ({0},{1}) {2}x{3}" -f $capLeft, $capTop, $capW, $capH)
Write-Host ""

# ----------------------------------------------------------------------
# 3-2-1 countdown so the user can position cursor / open target app.
# ----------------------------------------------------------------------

Write-Host "Recording starts in:"
foreach ($n in 3,2,1) {
    Write-Host "  $n..."
    Start-Sleep -Seconds 1
}
Write-Host "RECORDING for $Seconds seconds. Demo your flow now."
Write-Host ""

# ----------------------------------------------------------------------
# Record MP4 via ffmpeg gdigrab.
# ----------------------------------------------------------------------

$gdigrabArgs = @(
    "-y",
    "-f", "gdigrab",
    "-framerate", "30",
    "-offset_x", "$capLeft",
    "-offset_y", "$capTop",
    "-video_size", "${capW}x${capH}",
    "-t", "$Seconds",
    "-i", "desktop",
    # H.264 is fine for the intermediate; we re-encode to GIF next.
    "-c:v", "libx264",
    "-preset", "veryfast",
    "-crf", "20",
    "-pix_fmt", "yuv420p",
    "$mp4Path"
)
Invoke-Ffmpeg -FfmpegArgs $gdigrabArgs

if (-not (Test-Path $mp4Path)) {
    Write-Error "ffmpeg did not produce $mp4Path."
    exit 1
}

$mp4Size = (Get-Item $mp4Path).Length
Write-Host ""
Write-Host ("MP4 written: {0} ({1:N0} bytes)" -f $mp4Path, $mp4Size)

# ----------------------------------------------------------------------
# MP4 -> optimised GIF (palettegen + paletteuse).
# ----------------------------------------------------------------------

Write-Host ""
Write-Host "Generating colour palette..."
$paletteArgs = @(
    "-y",
    "-i", "$mp4Path",
    "-vf", "fps=$Fps,scale=${MaxWidth}:-2:flags=lanczos,palettegen=stats_mode=diff",
    "$paletteFile"
)
Invoke-Ffmpeg -FfmpegArgs $paletteArgs

Write-Host "Composing GIF..."
$gifArgs = @(
    "-y",
    "-i", "$mp4Path",
    "-i", "$paletteFile",
    "-lavfi", "fps=$Fps,scale=${MaxWidth}:-2:flags=lanczos[x];[x][1:v]paletteuse=dither=bayer:bayer_scale=5",
    "-loop", "0",
    "$gifPath"
)
Invoke-Ffmpeg -FfmpegArgs $gifArgs

# Drop the intermediate palette so the output dir stays tidy.
if (Test-Path $paletteFile) { Remove-Item $paletteFile -Force }

if (Test-Path $gifPath) {
    $gifSize = (Get-Item $gifPath).Length
    Write-Host ""
    Write-Host ("GIF written: {0} ({1:N0} bytes / {2:N1} MB)" -f $gifPath, $gifSize, ($gifSize / 1MB))
} else {
    Write-Warning "GIF generation failed; keep the MP4 and re-run the palette/composition steps manually."
}

Write-Host ""
Write-Host "Files in $OutDir :"
Get-ChildItem -Path $OutDir | Where-Object { $_.Name -in @("demo.mp4","demo.gif") } |
    Select-Object Name, @{N='Size';E={"{0:N0} B" -f $_.Length}} | Format-Table -AutoSize

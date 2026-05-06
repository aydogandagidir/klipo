<#
    capture-screenshots.ps1 - programmatic Klipo screenshot capture.

    Usage (from repo root):
        .\scripts\capture-screenshots.ps1
        .\scripts\capture-screenshots.ps1 -OutDir .\assets\screenshots -Verbose

    What it does:
      1. Confirms Klipo is running (Get-Process klipo).
      2. Sends Ctrl+Alt+V via Win32 SendInput to summon the popup.
      3. Locates the popup window ("Klipo") and the Settings window
         ("Klipo Settings") by title.
      4. Captures each via PrintWindow + PW_RENDERFULLCONTENT
         (transparency / Mica-aware) into PNG files.
      5. If PrintWindow returns a blank/black bitmap (some
         WebView2 + DirectX combos) falls back to BitBlt of the
         on-screen region.

    Notes:
      - You must have Klipo running (tray icon present). Launch it via
        Start menu if not.
      - Run on the user's interactive desktop session. Service / SYSTEM
        contexts cannot capture user windows.
      - Each capture takes <500 ms. The whole script finishes in <5 s.
#>

[CmdletBinding()]
param(
    [string] $OutDir = ".\assets\screenshots"
)

$ErrorActionPreference = "Stop"

# Resolve the output dir to an absolute path so subsequent file ops are
# unambiguous regardless of which CWD PowerShell decides to set.
if (-not [System.IO.Path]::IsPathRooted($OutDir)) {
    $OutDir = Join-Path (Get-Location).Path $OutDir
}
New-Item -ItemType Directory -Force -Path $OutDir | Out-Null
$OutDir = (Resolve-Path -LiteralPath $OutDir).Path

# ----------------------------------------------------------------------
# Win32 interop. One C# class with everything we need: SendInput for
# the hotkey, FindWindow/IsWindow for window discovery, PrintWindow +
# GetWindowRect for the screenshot.
# ----------------------------------------------------------------------

$signature = @'
using System;
using System.Collections.Generic;
using System.Drawing;
using System.Drawing.Imaging;
using System.Runtime.InteropServices;
using System.Text;

public class WinApi
{
    // --- Window enumeration / discovery ---
    public delegate bool EnumWindowsProc(IntPtr hWnd, IntPtr lParam);

    [DllImport("user32.dll")]
    public static extern bool EnumWindows(EnumWindowsProc enumProc, IntPtr lParam);

    [DllImport("user32.dll", CharSet = CharSet.Auto)]
    public static extern int GetWindowText(IntPtr hWnd, StringBuilder lpString, int nMaxCount);

    [DllImport("user32.dll")] public static extern bool IsWindow(IntPtr hWnd);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr hWnd);
    [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr hWnd, int nCmdShow);

    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr hWnd, out uint lpdwProcessId);

    [DllImport("user32.dll", SetLastError = true)]
    public static extern bool GetWindowRect(IntPtr hWnd, out RECT lpRect);

    public const int SW_HIDE             = 0;
    public const int SW_SHOWNOACTIVATE   = 4;
    public const int SW_SHOW             = 5;

    // --- Screenshot ---
    [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr hWnd, IntPtr hdcBlt, uint nFlags);

    public const uint PW_CLIENTONLY        = 0x00000001;
    public const uint PW_RENDERFULLCONTENT = 0x00000002;

    [StructLayout(LayoutKind.Sequential)]
    public struct RECT { public int Left, Top, Right, Bottom; }

    // Helper: enumerate all top-level windows for a given pid and return
    // them with their title + visibility. Used to find Klipo's popup +
    // settings hwnd without depending on FindWindow (which only sees
    // visible windows by default in some Windows builds).
    public static List<KlipoWindow> ListWindowsForPid(uint targetPid)
    {
        var results = new List<KlipoWindow>();
        EnumWindows((hWnd, lParam) => {
            uint pid = 0;
            GetWindowThreadProcessId(hWnd, out pid);
            if (pid == targetPid) {
                var sb = new StringBuilder(256);
                GetWindowText(hWnd, sb, sb.Capacity);
                results.Add(new KlipoWindow {
                    HWnd = hWnd,
                    Title = sb.ToString(),
                    Visible = IsWindowVisible(hWnd)
                });
            }
            return true;
        }, IntPtr.Zero);
        return results;
    }

    public class KlipoWindow {
        public IntPtr HWnd;
        public string Title;
        public bool Visible;
    }
}
'@

Add-Type -TypeDefinition $signature -ReferencedAssemblies "System.Drawing","System.Windows.Forms"

function Find-KlipoWindow {
    param([string] $Title, [uint32] $ProcessPid)
    $list = [WinApi]::ListWindowsForPid($ProcessPid)
    $hit = $list | Where-Object { $_.Title -eq $Title } | Select-Object -First 1
    if ($null -eq $hit) { return [IntPtr]::Zero }
    return $hit.HWnd
}

function Capture-Hwnd {
    param(
        [IntPtr] $HWnd,
        [string] $OutFile,
        [int]    $WaitMs = 700
    )

    if ($HWnd -eq [IntPtr]::Zero -or -not [WinApi]::IsWindow($HWnd)) {
        Write-Warning "Capture-Hwnd: invalid hwnd"
        return $false
    }

    # Bring the window on-screen WITHOUT activating it. SW_SHOWNOACTIVATE
    # avoids the focus-bounce problem that would trigger Klipo's
    # "Focused(false) -> hide()" auto-hide handler. WebView2 needs a
    # moment to repaint after being un-hidden so the screenshot doesn't
    # capture stale buffers.
    [WinApi]::ShowWindow($HWnd, [WinApi]::SW_SHOWNOACTIVATE) | Out-Null
    Start-Sleep -Milliseconds $WaitMs

    $rect = New-Object WinApi+RECT
    [WinApi]::GetWindowRect($HWnd, [ref]$rect) | Out-Null
    $w = $rect.Right - $rect.Left
    $h = $rect.Bottom - $rect.Top

    if ($w -le 0 -or $h -le 0) {
        Write-Warning "Capture-Hwnd: non-positive size ${w}x${h}"
        return $false
    }

    $bmp = New-Object System.Drawing.Bitmap($w, $h, [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $hdc = $g.GetHdc()

    # PW_RENDERFULLCONTENT (0x2) is the magic flag that tells PrintWindow
    # to render hardware-accelerated content (WebView2 / DirectX) into the
    # provided DC. Without it the WebView pixels come back transparent.
    $printOk = [WinApi]::PrintWindow($HWnd, $hdc, [WinApi]::PW_RENDERFULLCONTENT)
    $g.ReleaseHdc($hdc)
    $g.Dispose()

    if (-not $printOk) {
        Write-Warning "PrintWindow returned false - falling back to screen BitBlt"
        $bmp.Dispose()
        $bmp = New-Object System.Drawing.Bitmap($w, $h, [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
        $g2 = [System.Drawing.Graphics]::FromImage($bmp)
        $g2.CopyFromScreen($rect.Left, $rect.Top, 0, 0, [System.Drawing.Size]::new($w, $h))
        $g2.Dispose()
    }

    $bmp.Save($OutFile, [System.Drawing.Imaging.ImageFormat]::Png)
    $bmp.Dispose()

    $size = (Get-Item $OutFile).Length
    $name = Split-Path -Leaf $OutFile
    Write-Host ("[OK] {0,-32} {1,5}x{2,4}  ({3,7:N0} bytes)" -f $name, $w, $h, $size)
    return $true
}

# ----------------------------------------------------------------------
# Sanity check - Klipo must be running.
# ----------------------------------------------------------------------

$klipo = Get-Process -Name klipo -ErrorAction SilentlyContinue
if (-not $klipo) {
    Write-Error "Klipo is not running. Press Win, type 'Klipo', hit Enter, then re-run this script."
    exit 1
}

Write-Host ""
Write-Host "Klipo PID $($klipo[0].Id), capturing to $OutDir"
Write-Host ""

# ----------------------------------------------------------------------
# Capture flow
# ----------------------------------------------------------------------

$klipoPid = [uint32]$klipo[0].Id

# Inventory: dump every Klipo top-level window so failures are easy to
# diagnose from the log alone.
$wins = [WinApi]::ListWindowsForPid($klipoPid)
Write-Host "Klipo top-level windows:"
$wins | Format-Table -AutoSize

# 1. Settings window
$settingsHwnd = Find-KlipoWindow -Title "Klipo Settings" -ProcessPid $klipoPid
if ($settingsHwnd -ne [IntPtr]::Zero) {
    Capture-Hwnd -HWnd $settingsHwnd -OutFile (Join-Path $OutDir "settings.png") -WaitMs 400 | Out-Null
} else {
    Write-Warning "Settings window not found. Open it via tray right-click -> Settings..."
}

# Helpers to push values onto the OS clipboard. Klipo's watcher holds the
# clipboard briefly on every event, so PowerShell's Set-Clipboard races
# with it. Retry generously, fall back to `clip.exe` (different code path).
function Push-Clipboard {
    param([string] $Value)
    for ($i = 1; $i -le 10; $i++) {
        try {
            Set-Clipboard -Value $Value -ErrorAction Stop
            return $true
        } catch {
            Start-Sleep -Milliseconds 400
        }
    }
    Write-Host "  falling back to clip.exe..."
    try {
        $Value | & clip.exe
        return $true
    } catch {
        return $false
    }
}

# Optional sqlite3 cleanup helper. We use it to soft-delete the fake key
# AFTER capturing popup-sensitive.png, so the subsequent popup.png shows
# clean state without the synthetic clip.
$sqliteExe = $null
$sqliteCandidates = @(
    "$env:LOCALAPPDATA\Microsoft\WinGet\Packages\SQLite.SQLite_Microsoft.Winget.Source_8wekyb3d8bbwe\sqlite3.exe",
    "$env:ProgramFiles\sqlite\sqlite3.exe"
)
foreach ($cand in $sqliteCandidates) {
    if (Test-Path $cand) { $sqliteExe = $cand; break }
}

# Source order matters here:
#   1. Sensitive injection FIRST. The fake key's clip becomes the most
#      recent row; popup-sensitive.png captures it at the top.
#   2. Soft-delete that row via sqlite3 (deleted_at = now).
#   3. Push a benign value to the clipboard so any phantom watcher event
#      that fires during the next show cycle ends up benign, not sensitive.
#   4. Re-capture popup.png. The fake key is gone (soft-deleted), the
#      benign value is the new top row.
$popupHwnd = Find-KlipoWindow -Title "Klipo" -ProcessPid $klipoPid
$fakeKey = "sk-ant-api03-EXAMPLE-FAKE-KEY-FOR-DEMO-SCREENSHOT-ONLY-DO-NOT-USE-anywhere1234567890"
$benign  = "https://github.com/aydogandagidir/klipo - clipboard manager docs"

if ($popupHwnd -eq [IntPtr]::Zero) {
    Write-Warning "Popup window not found. Klipo may have crashed; re-launch it."
    Write-Host ""
    Write-Host "Done. Files in $OutDir :"
    Get-ChildItem -Path $OutDir -Filter "*.png" | Select-Object Name, Length | Format-Table -AutoSize
    return
}

Write-Host ""
Write-Host "Injecting synthetic sensitive clip into clipboard..."
if (-not (Push-Clipboard -Value $fakeKey)) {
    Write-Warning "Could not push fake key; skipping sensitive screenshot."
} else {
    Start-Sleep -Milliseconds 1000
    Capture-Hwnd -HWnd $popupHwnd -OutFile (Join-Path $OutDir "popup-sensitive.png") -WaitMs 600 | Out-Null
    [WinApi]::ShowWindow($popupHwnd, [WinApi]::SW_HIDE) | Out-Null

    if ($sqliteExe) {
        $db = Join-Path $env:APPDATA "app.klipo.desktop\klipo.db"
        if (Test-Path $db) {
            $now = [int64]([DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds())
            & $sqliteExe $db "UPDATE clips SET deleted_at = $now WHERE text_content LIKE 'sk-ant-api03-EXAMPLE-FAKE-KEY%' AND deleted_at IS NULL;"
            Write-Host "Soft-deleted synthetic clip from DB."
        }
    } else {
        Write-Warning "sqlite3 not found; popup.png may still show the synthetic clip at top."
    }
}

# Push benign value so any phantom clipboard event during next show is
# captured as the benign clip (not the fake key).
Write-Host "Pushing benign clipboard value..."
[void](Push-Clipboard -Value $benign)
Start-Sleep -Milliseconds 1000

Capture-Hwnd -HWnd $popupHwnd -OutFile (Join-Path $OutDir "popup.png") -WaitMs 600 | Out-Null
[WinApi]::ShowWindow($popupHwnd, [WinApi]::SW_HIDE) | Out-Null

Write-Host ""
Write-Host "Done. Files in $OutDir :"
Get-ChildItem -Path $OutDir -Filter "*.png" | Select-Object Name, Length | Format-Table -AutoSize

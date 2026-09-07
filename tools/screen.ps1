<#
  screen.ps1 - window capture for observing the game and tooling.

  Two capture methods:
    printwindow (default) - asks the window to render itself into a DC.
                            Works when the window is occluded or in the
                            background. Correct choice for GUI windows.
    screen                 - CopyFromScreen over the window rect. Captures
                            whatever pixels are physically on screen, so an
                            overlapping window will be captured instead.
                            Needed for some DirectDraw/GPU surfaces that
                            refuse to render via PrintWindow.
#>
[CmdletBinding()]
param(
  [string]$Name,
  [int]$ProcId = 0,
  [ValidateSet('printwindow','screen')]
  [string]$Method = 'printwindow',
  [switch]$Full,
  [int]$DelayMs = 400,
  [string]$Out = "$env:TEMP\shot.png",
  [switch]$Foreground
)

Add-Type -AssemblyName System.Drawing
Add-Type -AssemblyName System.Windows.Forms
Add-Type @'
using System;
using System.Runtime.InteropServices;
public struct RECT { public int Left, Top, Right, Bottom; }
public static class U32 {
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h, IntPtr hdc, uint flags);
  [DllImport("user32.dll")] public static extern bool ClientToScreen(IntPtr h, ref System.Drawing.Point p);
}
'@ -ReferencedAssemblies System.Drawing

function Save-Bitmap($bmp, $path) {
  $dir = Split-Path $path
  if ($dir -and -not (Test-Path $dir)) { New-Item -ItemType Directory -Force $dir | Out-Null }
  $bmp.Save($path, [System.Drawing.Imaging.ImageFormat]::Png)
}

if ($Full) {
  $b = [System.Windows.Forms.SystemInformation]::VirtualScreen
  $bmp = New-Object System.Drawing.Bitmap $b.Width, $b.Height
  $g = [System.Drawing.Graphics]::FromImage($bmp)
  $g.CopyFromScreen($b.Left, $b.Top, 0, 0, $bmp.Size)
  Save-Bitmap $bmp $Out
  Write-Host "captured full screen $($b.Width)x$($b.Height) -> $Out"
  exit 0
}

if ($ProcId -eq 0) {
  if (-not $Name) { Write-Host "need -Name or -ProcId (or -Full)"; exit 1 }
  $p = Get-Process -Name $Name -ErrorAction SilentlyContinue |
       Where-Object { $_.MainWindowHandle -ne 0 } | Select-Object -First 1
  if (-not $p) { Write-Host "no visible window for process '$Name'"; exit 1 }
} else {
  $p = Get-Process -Id $ProcId -ErrorAction SilentlyContinue
  if (-not $p) { Write-Host "no such pid $ProcId"; exit 1 }
}

$h = $p.MainWindowHandle
if ($h -eq 0) { Write-Host "process $($p.ProcessName) has no main window"; exit 1 }

if ($Foreground) { [U32]::SetForegroundWindow($h) | Out-Null }
if ($DelayMs -gt 0) { Start-Sleep -Milliseconds $DelayMs }

$r = New-Object RECT
[U32]::GetWindowRect($h, [ref]$r) | Out-Null
$w = $r.Right - $r.Left; $hh = $r.Bottom - $r.Top
if ($w -le 0 -or $hh -le 0) { Write-Host "bad window rect"; exit 1 }

$bmp = New-Object System.Drawing.Bitmap $w, $hh
$g = [System.Drawing.Graphics]::FromImage($bmp)

if ($Method -eq 'printwindow') {
  $hdc = $g.GetHdc()
  # flag 2 = PW_RENDERFULLCONTENT, needed for many modern/composited windows
  $ok = [U32]::PrintWindow($h, $hdc, 2)
  $g.ReleaseHdc($hdc)
  if (-not $ok) { Write-Host "PrintWindow failed; retry with -Method screen"; exit 1 }
} else {
  $g.CopyFromScreen($r.Left, $r.Top, 0, 0, $bmp.Size)
}

Save-Bitmap $bmp $Out

$cr = New-Object RECT
[U32]::GetClientRect($h, [ref]$cr) | Out-Null
$pt = New-Object System.Drawing.Point 0, 0
[U32]::ClientToScreen($h, [ref]$pt) | Out-Null
Write-Host "pid=$($p.Id) '$($p.ProcessName)' method=$Method window=${w}x${hh} at ($($r.Left),$($r.Top))"
Write-Host "client=$($cr.Right)x$($cr.Bottom) screenOrigin=($($pt.X),$($pt.Y))"
Write-Host "saved -> $Out"

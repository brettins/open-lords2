<#
  input.ps1 - synthetic mouse/keyboard input against a target window.
  Coordinates are CLIENT-relative by default, so they stay valid no matter
  where the window sits on screen.
#>
[CmdletBinding()]
param(
  [string]$Name,
  [int]$ProcId = 0,
  [ValidateSet('click','rclick','dblclick','move','key','text')]
  [string]$Action = 'click',
  [int]$X = 0,
  [int]$Y = 0,
  [string]$Key,
  [string]$Text,
  [int]$DelayMs = 120,
  [switch]$Screen   # treat X/Y as absolute screen coords instead
)

Add-Type -AssemblyName System.Drawing
Add-Type -AssemblyName System.Windows.Forms
Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class INP {
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
  [DllImport("user32.dll")] public static extern void mouse_event(uint f, uint x, uint y, uint d, IntPtr e);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern bool ClientToScreen(IntPtr h, ref System.Drawing.Point p);
  public const uint LEFTDOWN=0x0002, LEFTUP=0x0004, RIGHTDOWN=0x0008, RIGHTUP=0x0010;
}
'@ -ReferencedAssemblies System.Drawing

function Get-Target {
  if ($ProcId -ne 0) { return Get-Process -Id $ProcId -ErrorAction SilentlyContinue }
  return Get-Process -Name $Name -ErrorAction SilentlyContinue |
         Where-Object { $_.MainWindowHandle -ne 0 } | Select-Object -First 1
}

$p = Get-Target
if (-not $p) { Write-Host "target window not found"; exit 1 }
$h = $p.MainWindowHandle
[INP]::SetForegroundWindow($h) | Out-Null
Start-Sleep -Milliseconds $DelayMs

# map client coords -> screen coords
$sx = $X; $sy = $Y
if (-not $Screen) {
  $pt = New-Object System.Drawing.Point 0,0
  [INP]::ClientToScreen($h, [ref]$pt) | Out-Null
  $sx = $pt.X + $X; $sy = $pt.Y + $Y
}

switch ($Action) {
  'move'  { [INP]::SetCursorPos($sx,$sy) | Out-Null }
  'click' {
    [INP]::SetCursorPos($sx,$sy) | Out-Null; Start-Sleep -Milliseconds 40
    [INP]::mouse_event([INP]::LEFTDOWN,0,0,0,[IntPtr]::Zero)
    Start-Sleep -Milliseconds 30
    [INP]::mouse_event([INP]::LEFTUP,0,0,0,[IntPtr]::Zero)
  }
  'rclick' {
    [INP]::SetCursorPos($sx,$sy) | Out-Null; Start-Sleep -Milliseconds 40
    [INP]::mouse_event([INP]::RIGHTDOWN,0,0,0,[IntPtr]::Zero)
    Start-Sleep -Milliseconds 30
    [INP]::mouse_event([INP]::RIGHTUP,0,0,0,[IntPtr]::Zero)
  }
  'dblclick' {
    [INP]::SetCursorPos($sx,$sy) | Out-Null; Start-Sleep -Milliseconds 40
    foreach ($i in 1..2) {
      [INP]::mouse_event([INP]::LEFTDOWN,0,0,0,[IntPtr]::Zero)
      [INP]::mouse_event([INP]::LEFTUP,0,0,0,[IntPtr]::Zero)
      Start-Sleep -Milliseconds 40
    }
  }
  'key'  { [System.Windows.Forms.SendKeys]::SendWait($Key) }
  'text' { [System.Windows.Forms.SendKeys]::SendWait($Text) }
}
Write-Host "$Action -> pid=$($p.Id) client=($X,$Y) screen=($sx,$sy)"

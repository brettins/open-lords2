<#
  input.ps1 - synthetic mouse/keyboard input against a target window.
  Coordinates are CLIENT-relative by default, so they stay valid no matter
  where the window sits on screen.
#>
[CmdletBinding()]
param(
  [string]$Name,
  [int]$ProcId = 0,
  [ValidateSet('click','rclick','dblclick','move','key','text','postkey')]
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
  [DllImport("user32.dll")] public static extern bool PostMessageW(IntPtr h, uint msg, IntPtr wParam, IntPtr lParam);
  [DllImport("user32.dll")] public static extern uint MapVirtualKeyW(uint code, uint mapType);
  public const uint WM_KEYDOWN=0x0100, WM_KEYUP=0x0101;
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
# Mouse actions and SendKeys go to whatever is focused, so they need the window
# in front. 'postkey' posts straight to the window's queue and must NOT steal
# focus - that is the whole point of it, and grabbing focus here silently made
# every earlier "focus-independent" test a lie.
if ($Action -ne 'postkey') {
  [INP]::SetForegroundWindow($h) | Out-Null
}
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
  'postkey' {
    # SetForegroundWindow fails silently from a background process, so SendKeys
    # can land in whatever window actually has focus. Posting straight to the
    # target window sidesteps focus entirely - the reliable path for automation.
    $vk = @{ 'LEFT'=0x25; 'UP'=0x26; 'RIGHT'=0x27; 'DOWN'=0x28; 'ESC'=0x1B;
             'ENTER'=0x0D; 'SPACE'=0x20; 'TAB'=0x09; 'HOME'=0x24; 'END'=0x23 }[$Key.ToUpper()]
    if (-not $vk) {
      if ($Key.Length -eq 1) { $vk = [int][char]$Key.ToUpper() }
      else { Write-Host "unknown key '$Key'"; exit 1 }
    }
    # lParam: repeat count 1, scan code in bits 16-23, extended flag in bit 24.
    # Arrow/nav keys are "extended". Posting lParam = 0 leaves winit with no scan
    # code to map and the event is dropped.
    $scan = [INP]::MapVirtualKeyW([uint32]$vk, 0)
    $extended = @(0x25,0x26,0x27,0x28,0x21,0x22,0x23,0x24,0x2D,0x2E) -contains $vk
    $lp = 1 -bor ($scan -shl 16)
    if ($extended) { $lp = $lp -bor (1 -shl 24) }
    $lpUp = $lp -bor (1 -shl 30) -bor (1 -shl 31)   # previous state + transition
    [INP]::PostMessageW($h, [INP]::WM_KEYDOWN, [IntPtr]$vk, [IntPtr]$lp) | Out-Null
    Start-Sleep -Milliseconds 20
    [INP]::PostMessageW($h, [INP]::WM_KEYUP, [IntPtr]$vk, [IntPtr]$lpUp) | Out-Null
  }
  'text' { [System.Windows.Forms.SendKeys]::SendWait($Text) }
}
Write-Host "$Action -> pid=$($p.Id) client=($X,$Y) screen=($sx,$sy)"

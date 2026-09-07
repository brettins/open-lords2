<#
  Drives Lords of the Realm II's multiplayer setup and captures what it asks
  DirectPlay for.

  Two halves:

   * The proxy DLL (native/dplay-proxy) is told, through L2NET_CALL, to call
     Net_MultiplayerSetup() at 0x004B7585 from a thread inside the game once
     the game has started. That is docs/decisions.md D8 applied literally -
     calling the original's function rather than pretending to be a player,
     because a fullscreen DirectDraw game cannot be seen or clicked from
     outside.

   * That function opens ordinary Win32 dialogs (129 "Connection method", 108
     "Connect or Create a game?", 116 session name, 130 "Select Session" - see
     tools/net/dlgdump.js). Ordinary dialogs are real windows with real control
     IDs, so this script operates them with posted messages. No focus, no
     screenshots, no clicking at guessed coordinates.

  Examples:

    # one instance, host a session, 60s
    powershell -File tools/net/session.ps1 -Role host -Seconds 60

    # two instances on loopback: A hosts, B joins
    powershell -File tools/net/session.ps1 -Role both -Seconds 90

  Always terminates every process it starts, including on Ctrl-C.
#>
[CmdletBinding()]
param(
  [ValidateSet('host', 'join', 'both', 'none')]
  [string]$Role = 'host',
  [int]$Seconds = 60,
  [int]$Delay = 14000,                 # ms before the injected thread fires
  [string]$Provider = 'TCP/IP',
  [string]$SessionName = 'l2probe',
  [string]$Address = '127.0.0.1',
  [string]$Root = 'F:\games',
  [switch]$KeepLogs
)

$ErrorActionPreference = 'Stop'

# --------------------------------------------------------------- win32 glue
if (-not ('L2Win' -as [type])) {
Add-Type @'
using System;
using System.Text;
using System.Runtime.InteropServices;
public class L2Win {
  public delegate bool EnumProc(IntPtr h, IntPtr p);
  [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc cb, IntPtr p);
  [DllImport("user32.dll")] public static extern bool EnumChildWindows(IntPtr h, EnumProc cb, IntPtr p);
  [DllImport("user32.dll")] public static extern int GetClassName(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll", CharSet=CharSet.Ansi)] public static extern int GetWindowTextA(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  [DllImport("user32.dll")] public static extern IntPtr GetDlgItem(IntPtr h, int id);
  [DllImport("user32.dll")] public static extern int GetDlgCtrlID(IntPtr h);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll", CharSet=CharSet.Ansi)] public static extern IntPtr SendMessageA(IntPtr h, uint m, IntPtr w, IntPtr l);
  [DllImport("user32.dll", EntryPoint="SendMessageA", CharSet=CharSet.Ansi)] public static extern IntPtr SendMessageStr(IntPtr h, uint m, IntPtr w, string l);
  [DllImport("user32.dll", EntryPoint="SendMessageA", CharSet=CharSet.Ansi)] public static extern IntPtr SendMessageBuf(IntPtr h, uint m, IntPtr w, StringBuilder l);
  [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h, uint m, IntPtr w, IntPtr l);
}
'@
}

$WM_COMMAND = 0x0111
$WM_SETTEXT = 0x000C
$LB_GETCOUNT = 0x018B
$LB_GETTEXT = 0x0189
$LB_SETCURSEL = 0x0186
$LBN_SELCHANGE = 1
$BN_CLICKED = 0

function Get-Dialogs([int[]]$pids) {
  $found = New-Object System.Collections.ArrayList
  $cb = [L2Win+EnumProc] {
    param($h, $p)
    $cls = New-Object System.Text.StringBuilder 64
    [void][L2Win]::GetClassName($h, $cls, 64)
    if ($cls.ToString() -eq '#32770') {
      $procId = 0
      [void][L2Win]::GetWindowThreadProcessId($h, [ref]$procId)
      if ($pids -contains $procId) {
        $t = New-Object System.Text.StringBuilder 256
        [void][L2Win]::GetWindowTextA($h, $t, 256)
        [void]$found.Add([pscustomobject]@{ h = $h; pid = $procId; title = $t.ToString() })
      }
    }
    return $true
  }
  [void][L2Win]::EnumWindows($cb, [IntPtr]::Zero)
  return $found
}

function Get-ListItems([IntPtr]$lb) {
  $n = [int][L2Win]::SendMessageA($lb, $LB_GETCOUNT, [IntPtr]::Zero, [IntPtr]::Zero)
  $out = @()
  for ($i = 0; $i -lt $n; $i++) {
    $sb = New-Object System.Text.StringBuilder 512
    [void][L2Win]::SendMessageBuf($lb, $LB_GETTEXT, [IntPtr]$i, $sb)
    $out += $sb.ToString()
  }
  return ,$out          # comma: keep a 1-element list a list, not a bare string
}

# The connection list fills in as DirectPlayEnumerateA calls back, and each
# service provider takes over a second to load, so the dialog is visible long
# before the list is complete. Acting on a partial list picked the wrong
# provider on the first run. Wait until the count stops changing.
function Wait-ListSettled([IntPtr]$lb, [int]$maxMs = 12000) {
  $last = -1; $stable = 0; $waited = 0
  while ($waited -lt $maxMs) {
    $n = [int][L2Win]::SendMessageA($lb, $LB_GETCOUNT, [IntPtr]::Zero, [IntPtr]::Zero)
    if ($n -eq $last -and $n -gt 0) { $stable++ } else { $stable = 0 }
    if ($stable -ge 3) { return $n }
    $last = $n
    Start-Sleep -Milliseconds 400
    $waited += 400
  }
  return $last
}

function Click-Ctl([IntPtr]$dlg, [int]$id, [int]$notify = 0) {
  $c = [L2Win]::GetDlgItem($dlg, $id)
  $w = [IntPtr](($notify -shl 16) -bor ($id -band 0xFFFF))
  [void][L2Win]::PostMessage($dlg, $WM_COMMAND, $w, $c)
}

function Select-InList([IntPtr]$dlg, [int]$lbId, [string]$match) {
  $lb = [L2Win]::GetDlgItem($dlg, $lbId)
  if ($lb -eq [IntPtr]::Zero) { return $null }
  $items = Get-ListItems $lb
  Write-Host ("      list {0}: {1}" -f $lbId, ($items -join ' | '))
  $idx = -1
  for ($i = 0; $i -lt $items.Count; $i++) { if ($items[$i] -like "*$match*") { $idx = $i; break } }
  if ($idx -lt 0 -and $items.Count -gt 0) { $idx = 0; Write-Host "      no match for '$match', taking item 0" }
  if ($idx -lt 0) { return $null }
  [void][L2Win]::SendMessageA($lb, $LB_SETCURSEL, [IntPtr]$idx, [IntPtr]::Zero)
  # The dialog tracks the selection through LBN_SELCHANGE, so setting the
  # selection silently is not enough - it must be told the selection changed.
  [void][L2Win]::PostMessage($dlg, $WM_COMMAND, [IntPtr](($LBN_SELCHANGE -shl 16) -bor $lbId), $lb)
  return [string]$items[$idx]
}

# Each dialog is handled at most once; a caption reappearing is a new dialog.
$handled = @{}

function Handle-Dialog($d, [string]$role) {
  $key = "$($d.pid):$($d.h)"
  if ($handled.ContainsKey($key)) { return }
  $handled[$key] = $true
  Write-Host ("  [pid $($d.pid)] dialog '$($d.title)'")

  switch -Wildcard ($d.title) {
    'Connection method*' {
      $lb = [L2Win]::GetDlgItem($d.h, 1001)
      $n = Wait-ListSettled $lb
      Write-Host "      list settled at $n item(s)"
      $picked = Select-InList $d.h 1001 $Provider
      # The dialog keeps the "current" index in a global that it only updates
      # from an LBN_SELCHANGE / LBN_DBLCLK notification, and WM_INITDIALOG
      # pre-loads that global with the index of the "Sierra Internet Gaming
      # System" row. Pressing OK without a notification therefore selects
      # Sierra's dead matchmaking path and fails on the missing SNWValid.dll,
      # whatever is highlighted. Send the notification, then OK.
      Write-Host "      picked '$picked', notifying + OK"
      Start-Sleep -Milliseconds 500
      Click-Ctl $d.h 1 $BN_CLICKED
      return
    }
    'Connect or Create*' {
      if ($role -eq 'host') { Write-Host '      Create a game (1004)'; Click-Ctl $d.h 1004 $BN_CLICKED }
      else                  { Write-Host '      Connect to a game (1002)'; Click-Ctl $d.h 1002 $BN_CLICKED }
      return
    }
    'Enter a name for this session*' {
      $e = [L2Win]::GetDlgItem($d.h, 1000)
      [void][L2Win]::SendMessageStr($e, $WM_SETTEXT, [IntPtr]::Zero, $SessionName)
      Write-Host "      session name '$SessionName', OK"
      Start-Sleep -Milliseconds 300
      Click-Ctl $d.h 1 $BN_CLICKED
      return
    }
    'Select Session*' {
      $picked = Select-InList $d.h 1024 $SessionName
      Write-Host "      picked '$picked', clicking OK"
      Start-Sleep -Milliseconds 400
      Click-Ctl $d.h 1 $BN_CLICKED
      return
    }
    default {
      # dpwsockx puts up its own address prompt when joining over TCP/IP, and
      # its caption is not ours to predict. Anything with a single edit box is
      # treated as that prompt.
      $edits = New-Object System.Collections.ArrayList
      $cb = [L2Win+EnumProc] {
        param($h, $p)
        $c = New-Object System.Text.StringBuilder 64
        [void][L2Win]::GetClassName($h, $c, 64)
        if ($c.ToString() -eq 'Edit') { [void]$edits.Add($h) }
        return $true
      }
      [void][L2Win]::EnumChildWindows($d.h, $cb, [IntPtr]::Zero)
      if ($edits.Count -ge 1) {
        [void][L2Win]::SendMessageStr($edits[0], $WM_SETTEXT, [IntPtr]::Zero, $Address)
        Write-Host "      unrecognised dialog with an edit box - typed '$Address', OK"
        Start-Sleep -Milliseconds 300
        Click-Ctl $d.h 1 $BN_CLICKED
      } else {
        Write-Host '      unrecognised dialog, clicking OK'
        Click-Ctl $d.h 1 $BN_CLICKED
      }
      return
    }
  }
}

# ------------------------------------------------------------------ launch
$procs = @()
$dirs = @{}
function Start-Instance([string]$name, [string]$role) {
  $dir = if ($name) { Join-Path $Root "lords2-net-$name" } else { Join-Path $Root 'lords2-net' }
  if (-not (Test-Path (Join-Path $dir 'Lords2.exe'))) {
    throw "no sandbox at $dir - run native/dplay-proxy/install.ps1 -Instance $name"
  }
  Remove-Item (Join-Path $dir 'l2dplay.log') -Force -ErrorAction SilentlyContinue
  $env:L2NET_CALL = '4b7585'
  $env:L2NET_DELAY = "$Delay"
  $p = Start-Process -FilePath (Join-Path $dir 'Lords2.exe') -WorkingDirectory $dir -PassThru
  Write-Host "launched $role from $dir as pid $($p.Id)"
  $script:dirs[$p.Id] = @{ dir = $dir; role = $role }
  return $p
}

try {
  switch ($Role) {
    'both' {
      $procs += Start-Instance 'a' 'host'
      Start-Sleep -Seconds $Stagger
      $procs += Start-Instance 'b' 'join'
    }
    'none' { $procs += Start-Instance '' 'host' }
    default { $procs += Start-Instance '' $Role }
  }

  $pids = $procs | ForEach-Object { $_.Id }
  $deadline = (Get-Date).AddSeconds($Seconds)
  Write-Host "driving dialogs until $($deadline.ToString('HH:mm:ss')) ..."
  while ((Get-Date) -lt $deadline) {
    foreach ($d in (Get-Dialogs $pids)) {
      if ($Role -eq 'none') { Write-Host "  [pid $($d.pid)] dialog '$($d.title)' (not driving)"; continue }
      Handle-Dialog $d $dirs[[int]$d.pid].role
    }
    Start-Sleep -Milliseconds 500
    if (-not ($procs | Where-Object { -not $_.HasExited })) { Write-Host 'all instances exited'; break }
  }
} finally {
  foreach ($p in $procs) {
    if (-not $p.HasExited) { Stop-Process -Id $p.Id -Force -ErrorAction SilentlyContinue }
  }
  Start-Sleep -Milliseconds 800
  Get-Process Lords2 -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
  Write-Host "`n================ captures ================"
  foreach ($k in $dirs.Keys) {
    $log = Join-Path $dirs[$k].dir 'l2dplay.log'
    Write-Host "`n--- $($dirs[$k].role) : $log ---"
    if (Test-Path $log) { Get-Content $log } else { Write-Host '(no log)' }
  }
}

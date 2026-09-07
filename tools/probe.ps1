<#
  probe.ps1 - process inspection for Lords of the Realm II
  Lords2.exe has no ASLR (DllCharacteristics=0, ImageBase=0x400000),
  so virtual addresses are stable across runs and can be hardcoded.
#>
[CmdletBinding()]
param(
  [ValidateSet('selftest','find','read','launch')]
  [string]$Action = 'selftest',
  [string]$Name   = 'Lords2',
  [int]$ProcId    = 0,
  [string]$Addr   = '0x400000',
  [int]$Length    = 64,
  [string]$Exe    = 'F:\games\Lords of the Realm II\Lords2.exe'
)

Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class W32 {
  [DllImport("kernel32.dll", SetLastError=true)]
  public static extern IntPtr OpenProcess(uint da, bool inherit, int pid);
  [DllImport("kernel32.dll", SetLastError=true)]
  public static extern bool ReadProcessMemory(IntPtr h, IntPtr addr, byte[] buf, int size, out IntPtr read);
  [DllImport("kernel32.dll", SetLastError=true)]
  public static extern bool CloseHandle(IntPtr h);
}
'@

$PROCESS_VM_READ = 0x0010
$PROCESS_QUERY_INFORMATION = 0x0400

function ConvertTo-Addr([string]$s) {
  if ($s -match '^0[xX]') { return [Convert]::ToUInt64($s.Substring(2), 16) }
  return [Convert]::ToUInt64($s, 10)
}

function Read-Mem([int]$targetPid, [uint64]$address, [int]$len) {
  $h = [W32]::OpenProcess($PROCESS_VM_READ -bor $PROCESS_QUERY_INFORMATION, $false, $targetPid)
  if ($h -eq [IntPtr]::Zero) {
    throw "OpenProcess failed on pid $targetPid (win32 error $([Runtime.InteropServices.Marshal]::GetLastWin32Error()))"
  }
  try {
    $buf = New-Object byte[] $len
    $read = [IntPtr]::Zero
    $ok = [W32]::ReadProcessMemory($h, [IntPtr][int64]$address, $buf, $len, [ref]$read)
    if (-not $ok) {
      throw "ReadProcessMemory failed at 0x$($address.ToString('x8')) (win32 error $([Runtime.InteropServices.Marshal]::GetLastWin32Error()))"
    }
    return $buf[0..([int]$read - 1)]
  } finally { [W32]::CloseHandle($h) | Out-Null }
}

function Format-HexDump([byte[]]$bytes, [uint64]$baseAddr) {
  for ($i = 0; $i -lt $bytes.Length; $i += 16) {
    $chunk = $bytes[$i..([Math]::Min($i + 15, $bytes.Length - 1))]
    $hex = ($chunk | ForEach-Object { $_.ToString('x2') }) -join ' '
    $asc = ($chunk | ForEach-Object { if ($_ -ge 32 -and $_ -lt 127) { [char]$_ } else { '.' } }) -join ''
    '{0:x8}  {1,-47}  {2}' -f ($baseAddr + $i), $hex, $asc
  }
}

switch ($Action) {
  'selftest' {
    # Validate the P/Invoke path without touching the game: read this very
    # process at its own image base and confirm the MZ signature comes back.
    $self = Get-Process -Id $PID
    $base = [uint64]$self.MainModule.BaseAddress.ToInt64()
    Write-Host "self pid=$PID base=0x$($base.ToString('x8')) module=$($self.MainModule.ModuleName)"
    $b = Read-Mem $PID $base 64
    Format-HexDump $b $base
    if ($b[0] -eq 0x4D -and $b[1] -eq 0x5A) { Write-Host "`nSELFTEST OK - read 'MZ' from a live process" }
    else { Write-Host "`nSELFTEST FAILED - no MZ signature"; exit 1 }
  }
  'find' {
    $p = Get-Process -Name $Name -ErrorAction SilentlyContinue
    if (-not $p) { Write-Host "process '$Name' is not running"; exit 1 }
    foreach ($proc in $p) {
      $b = [uint64]$proc.MainModule.BaseAddress.ToInt64()
      Write-Host "pid=$($proc.Id) base=0x$($b.ToString('x8')) size=0x$($proc.MainModule.ModuleMemorySize.ToString('x')) title='$($proc.MainWindowTitle)'"
    }
  }
  'read' {
    if ($ProcId -eq 0) {
      $p = Get-Process -Name $Name -ErrorAction SilentlyContinue | Select-Object -First 1
      if (-not $p) { Write-Host "process '$Name' is not running"; exit 1 }
      $ProcId = $p.Id
    }
    $a = ConvertTo-Addr $Addr
    Format-HexDump (Read-Mem $ProcId $a $Length) $a
  }
  'launch' {
    if (-not (Test-Path $Exe)) { Write-Host "not found: $Exe"; exit 1 }
    $p = Start-Process -FilePath $Exe -WorkingDirectory (Split-Path $Exe) -PassThru
    Write-Host "launched pid=$($p.Id)"
  }
}

<#  dump.ps1 - read a range of a live Lords2.exe's memory to a raw file.
    usage: dump.ps1 -Addr 0x522f90 -Length 32768 -Out out\tiles.bin  #>
[CmdletBinding()]
param([string]$Name='Lords2',[int]$ProcId=0,[string]$Addr='0x522f90',[int]$Length=64,[string]$Out)
Add-Type @'
using System;using System.Runtime.InteropServices;
public static class W32D {
 [DllImport("kernel32.dll",SetLastError=true)] public static extern IntPtr OpenProcess(uint da,bool i,int pid);
 [DllImport("kernel32.dll",SetLastError=true)] public static extern bool ReadProcessMemory(IntPtr h,IntPtr a,byte[] b,int s,out IntPtr r);
 [DllImport("kernel32.dll",SetLastError=true)] public static extern bool CloseHandle(IntPtr h);
}
'@
if($ProcId -eq 0){$p=Get-Process -Name $Name -ErrorAction SilentlyContinue|Select-Object -First 1; if(-not $p){Write-Output 'not running';exit 1}; $ProcId=$p.Id}
$a=[Convert]::ToUInt64($Addr.Substring(2),16)
$h=[W32D]::OpenProcess(0x0010 -bor 0x0400,$false,$ProcId)
if($h -eq [IntPtr]::Zero){Write-Output 'OpenProcess failed';exit 1}
$buf=New-Object byte[] $Length; $r=[IntPtr]::Zero
$ok=[W32D]::ReadProcessMemory($h,[IntPtr][int64]$a,$buf,$Length,[ref]$r)
[W32D]::CloseHandle($h)|Out-Null
if(-not $ok){Write-Output "read failed at $Addr";exit 1}
[IO.File]::WriteAllBytes($Out,$buf)
Write-Output "wrote $Out  $([int]$r) bytes from 0x$($a.ToString('x8')) pid=$ProcId"

<#
  windows.ps1 - enumerate top-level windows for a process.
  Useful for spotting dialogs/child windows that MainWindowHandle misses.
#>
[CmdletBinding()]
param(
  [string]$Name,
  [int]$ProcId = 0,
  [switch]$IncludeHidden
)
Add-Type @'
using System; using System.Text; using System.Collections.Generic; using System.Runtime.InteropServices;
public struct WRECT { public int Left, Top, Right, Bottom; }
public class WEnum {
  public delegate bool EnumProc(IntPtr h, IntPtr l);
  [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc cb, IntPtr l);
  [DllImport("user32.dll")] public static extern int GetWindowThreadProcessId(IntPtr h, out int pid);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetWindowTextW(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetClassNameW(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out WRECT r);
  public static List<string> ForPid(int want, bool all) {
    var res = new List<string>();
    EnumWindows((h,l) => {
      int pid; GetWindowThreadProcessId(h, out pid);
      bool vis = IsWindowVisible(h);
      if (pid == want && (all || vis)) {
        var t = new StringBuilder(512); GetWindowTextW(h, t, 512);
        var c = new StringBuilder(512); GetClassNameW(h, c, 512);
        WRECT r; GetWindowRect(h, out r);
        res.Add(string.Format("hwnd={0} {1} {2}x{3}@({4},{5}) class='{6}' title='{7}'",
          h.ToInt64(), vis ? "VISIBLE" : "hidden",
          r.Right-r.Left, r.Bottom-r.Top, r.Left, r.Top, c, t));
      }
      return true;
    }, IntPtr.Zero);
    return res;
  }
}
'@
if ($ProcId -eq 0) {
  $p = Get-Process -Name $Name -ErrorAction SilentlyContinue | Select-Object -First 1
  if (-not $p) { Write-Host "process '$Name' not running"; exit 1 }
  $ProcId = $p.Id
}
Write-Host "pid=$ProcId windows:"
[WEnum]::ForPid($ProcId, [bool]$IncludeHidden) | ForEach-Object { "  $_" }

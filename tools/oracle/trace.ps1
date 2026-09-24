<#
  Controller for the headless original (native/l2-trace, docs in its build.ps1 header).
  Dot-source for the [L2Trace] class, or run with -Demo for the B1 acceptance run:
  launch hidden -> wait for the title menu (g_screenId 0x1F, g_setupPage 1) -> click its
  first item -> wait for the page to change. Prints one JSON line of measurements.
  Never touches the focus: the game window is created hidden and input is posted to it.
#>
[CmdletBinding()]
param(
  [string]$Install = 'E:\games\l2-trace',
  [switch]$Demo,
  [int]$Step = 1,
  [string]$Dump = '',
  [int]$TimeoutSec = 120,
  # Free-running instead of lockstep: faster, but input lands at a real-time-dependent frame.
  [switch]$Free
)
$ErrorActionPreference = 'Stop'
if ([IO.Path]::GetFullPath($Install) -match '^[Ff]:') { throw "refusing an F:\ install (read-only)" }

if (-not ('L2Trace' -as [type])) {
Add-Type -TypeDefinition @'
using System;
using System.Diagnostics;
using System.IO.MemoryMappedFiles;
using System.Runtime.InteropServices;
using System.Threading;

public class L2Trace : IDisposable {
    [DllImport("kernel32")] static extern IntPtr OpenProcess(int access, bool inherit, int pid);
    [DllImport("kernel32")] static extern bool ReadProcessMemory(IntPtr h, IntPtr addr, byte[] buf, int n, out int read);
    [DllImport("user32")] static extern bool PostMessage(IntPtr hwnd, int msg, IntPtr w, IntPtr l);
    [DllImport("user32")] public static extern IntPtr GetForegroundWindow();
    [DllImport("user32")] static extern bool IsWindowVisible(IntPtr h);
    [DllImport("user32")] static extern int GetWindowThreadProcessId(IntPtr h, out int pid);
    [DllImport("psapi")] static extern bool EnumProcessModulesEx(IntPtr h, IntPtr[] mods, int cb, out int needed, int filter);
    [DllImport("psapi", CharSet = CharSet.Unicode)] static extern int GetModuleFileNameEx(IntPtr h, IntPtr mod, System.Text.StringBuilder sb, int n);
    delegate bool EnumProc(IntPtr h, IntPtr l);
    [DllImport("user32")] static extern bool EnumWindows(EnumProc f, IntPtr l);

    // TraceShared in native/l2-trace/trace.h
    const int OffCursorX = 8, OffCursorY = 12, OffFrames = 16, OffHash = 20, OffMs = 24, OffHwnd = 28,
              OffTimers = 32, OffFilms = 36, OffGaps = 40, OffRefused = 44, OffTimersRun = 48,
              OffGate = 52, SharedSize = 56;
    public const int ScreenId = 0x004EAC50, TurnPhase = 0x00569584, SetupPage = 0x005530F0;

    public Process Proc;
    IntPtr handle;
    MemoryMappedFile map;
    MemoryMappedViewAccessor view;
    public Stopwatch Real = new Stopwatch();
    public bool Gated;
    // Sampled at every Advance: any visible top-level window of the game, and whether it held the foreground.
    public int MaxVisibleWindows, Samples;
    public bool EverForeground;
    public void Sample() {
        int n = 0, pid;
        EnumWindows((h, l) => { int p; GetWindowThreadProcessId(h, out p); if (p == Proc.Id && IsWindowVisible(h)) n++; return true; }, IntPtr.Zero);
        GetWindowThreadProcessId(GetForegroundWindow(), out pid);
        if (pid == Proc.Id) EverForeground = true;
        if (n > MaxVisibleWindows) MaxVisibleWindows = n;
        Samples++;
    }

    public static L2Trace Launch(string install, int step, string dump, bool gated) {
        var psi = new ProcessStartInfo(System.IO.Path.Combine(install, "Lords2.exe"));
        psi.WorkingDirectory = install;
        psi.UseShellExecute = false;
        psi.WindowStyle = ProcessWindowStyle.Hidden;
        psi.EnvironmentVariables["L2TRACE_STEP"] = step.ToString();
        if (!string.IsNullOrEmpty(dump)) psi.EnvironmentVariables["L2TRACE_DUMP"] = dump;
        if (gated) psi.EnvironmentVariables["L2TRACE_GATE"] = "1";
        var t = new L2Trace();
        t.Gated = gated;
        t.Real.Start();
        t.Proc = Process.Start(psi);
        t.handle = OpenProcess(0x0410, false, t.Proc.Id);
        for (int i = 0; i < 500 && t.view == null; i++) {
            try {
                t.map = MemoryMappedFile.OpenExisting("Local\\l2trace-" + t.Proc.Id);
                t.view = t.map.CreateViewAccessor(0, SharedSize);
            } catch (System.IO.FileNotFoundException) { Thread.Sleep(10); }
            if (t.Proc.HasExited) throw new Exception("game exited before the trace DLL came up");
        }
        if (t.view == null) throw new Exception("no l2trace shared memory - is ddraw.dll in the install?");
        return t;
    }

    public uint Shared(int off) { return view.ReadUInt32(off); }
    public uint Frames { get { return Shared(OffFrames); } }
    public uint Hash { get { return Shared(OffHash); } }
    public uint VirtualMs { get { return Shared(OffMs); } }
    public IntPtr Hwnd { get { return new IntPtr((int)Shared(OffHwnd)); } }
    public string Counters { get {
        return string.Format("timersRun={4} timersDropped={0} filmsSkipped={1} gaps={2} refused={3}",
            Shared(OffTimers), Shared(OffFilms), Shared(OffGaps), Shared(OffRefused), Shared(OffTimersRun)); } }

    public int Read(int addr, int size) {
        var b = new byte[4]; int n;
        if (!ReadProcessMemory(handle, new IntPtr(addr), b, size, out n)) return -1;
        return BitConverter.ToInt32(b, 0);
    }
    public int Screen { get { return Read(ScreenId, 1); } }
    public int Phase { get { return Read(TurnPhase, 4); } }
    public int Page { get { return Read(SetupPage, 4); } }

    // Gated: lets the game run n more presents and returns once it is blocked again.
    public void Advance(int n) {
        Sample();
        uint target = Frames + (uint)n;
        if (Gated) view.Write(OffGate, target);
        var sw = Stopwatch.StartNew();
        while (Frames < target && !Proc.HasExited && sw.ElapsedMilliseconds < 30000) {
            if (Gated) Thread.Yield(); else Thread.Sleep(2);
        }
    }
    // Checks cond between presents (gated) or every 5 ms (free-running).
    public bool WaitUntil(Func<bool> cond, int timeoutMs) {
        var sw = Stopwatch.StartNew();
        while (sw.ElapsedMilliseconds < timeoutMs) {
            if (Proc.HasExited) return false;
            if (cond()) return true;
            if (Gated) Advance(1); else { Sample(); Thread.Sleep(5); }
        }
        return false;
    }
    public bool WaitFrames(int n, int timeoutMs) { Advance(n); return !Proc.HasExited; }
    // Waits until the frame hash has not changed for `frames` presents.
    public bool WaitStill(int frames, int timeoutMs) {
        uint h = Hash, since = Frames;
        return WaitUntil(() => { if (Hash != h) { h = Hash; since = Frames; } return Frames >= since + frames; }, timeoutMs);
    }

    static IntPtr XY(int x, int y) { return new IntPtr((y << 16) | (x & 0xFFFF)); }
    public void MoveTo(int x, int y) {
        view.Write(OffCursorX, x); view.Write(OffCursorY, y);
        PostMessage(Hwnd, 0x0200, IntPtr.Zero, XY(x, y));
    }
    // App_WndProc 0x004B29BE sets the button bits on 0x201/0x202; FUN_004B191E latches them per poll.
    public void Click(int x, int y) {
        MoveTo(x, y);
        WaitFrames(2, 5000);
        PostMessage(Hwnd, 0x0201, new IntPtr(1), XY(x, y));
        WaitFrames(2, 5000);
        PostMessage(Hwnd, 0x0202, IntPtr.Zero, XY(x, y));
    }
    public void Key(int vk) {
        PostMessage(Hwnd, 0x0100, new IntPtr(vk), new IntPtr(1));
        WaitFrames(1, 5000);
        PostMessage(Hwnd, 0x0101, new IntPtr(vk), new IntPtr(unchecked((int)0xC0000001)));
    }
    // Loaded audio stacks: AudioSes/wdmaud/mmdevapi load only when a device is really opened.
    public string AudioModules() {
        var mods = new IntPtr[1024]; int needed;
        if (!EnumProcessModulesEx(handle, mods, mods.Length * IntPtr.Size, out needed, 3)) return "?";
        var found = new System.Collections.Generic.List<string>();
        for (int i = 0; i < needed / IntPtr.Size; i++) {
            var sb = new System.Text.StringBuilder(260);
            GetModuleFileNameEx(handle, mods[i], sb, 260);
            string f = sb.ToString();
            if (System.Text.RegularExpressions.Regex.IsMatch(f, @"(dsound|audioses|wdmaud|mmdevapi|mss32|ksuser).(dll|drv)$", System.Text.RegularExpressions.RegexOptions.IgnoreCase)) found.Add(f);
        }
        return string.Join(";", found.ToArray());
    }
    public void Dispose() {
        if (Proc != null && !Proc.HasExited) { Proc.Kill(); Proc.WaitForExit(5000); }
        if (view != null) view.Dispose();
        if (map != null) map.Dispose();
    }
}
'@
}

if (-not $Demo) { return }

$fgBefore = [L2Trace]::GetForegroundWindow()
$t = [L2Trace]::Launch($Install, $Step, $Dump, -not $Free)
try {
  $timeout = $TimeoutSec * 1000
  $atMenu = $t.WaitUntil({ $t.Screen -eq 0x1F -and $t.Page -eq 1 }, $timeout) -and $t.WaitStill(10, $timeout)
  $menu = [ordered]@{
    reached = $atMenu; screen = $t.Screen; page = $t.Page
    realMs = $t.Real.ElapsedMilliseconds; virtualMs = $t.VirtualMs; frames = $t.Frames
    hash = ('{0:x8}' -f $t.Hash)
  }
  # Title menu item 1: 192x24 recess at x 0xE0, y 0x5B (screens-county.md 1.2).
  $t.Click(0xE0 + 96, 0x5B + 12)
  $moved = $t.WaitUntil({ $t.Page -ne 1 -or $t.Screen -ne 0x1F }, $timeout)
  $t.WaitStill(5, $timeout) | Out-Null
  $out = [ordered]@{
    menu = $menu
    click = [ordered]@{ changed = $moved; screen = $t.Screen; page = $t.Page; hash = ('{0:x8}' -f $t.Hash) }
    speed = [math]::Round($t.VirtualMs / [math]::Max(1, $t.Real.ElapsedMilliseconds), 1)
    focusKept = ([L2Trace]::GetForegroundWindow() -eq $fgBefore) -and -not $t.EverForeground
    maxVisibleWindows = $t.MaxVisibleWindows; windowSamples = $t.Samples
    counters = $t.Counters
    audioModules = $t.AudioModules()
    gated = $t.Gated
  }
  $out | ConvertTo-Json -Compress -Depth 4
} finally { $t.Dispose() }

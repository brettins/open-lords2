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
              OffGate = 52, OffSteps = 56, OffLoops = 60, OffTicks = 64, OffFb = 68, OffPal = 72, SharedSize = 76;
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
        return Launch(install, step, dump, gated, null, null);
    }
    // events: JSONL path for the detour event ring (native/l2-trace/events.h); films: play|skip.
    public static L2Trace Launch(string install, int step, string dump, bool gated, string events, string films) {
        var psi = new ProcessStartInfo(System.IO.Path.Combine(install, "Lords2.exe"));
        psi.WorkingDirectory = install;
        psi.UseShellExecute = false;
        psi.WindowStyle = ProcessWindowStyle.Hidden;
        psi.EnvironmentVariables["L2TRACE_STEP"] = step.ToString();
        if (!string.IsNullOrEmpty(dump)) psi.EnvironmentVariables["L2TRACE_DUMP"] = dump;
        if (gated) psi.EnvironmentVariables["L2TRACE_GATE"] = "1";
        if (!string.IsNullOrEmpty(films)) psi.EnvironmentVariables["L2TRACE_FILMS"] = films;
        if (!string.IsNullOrEmpty(events)) psi.EnvironmentVariables["L2TRACE_EVENTS"] = "1";
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
        if (!string.IsNullOrEmpty(events)) {
            for (int i = 0; i < 500 && t.ring == null; i++) {
                try { t.ring = MemoryMappedFile.OpenExisting("Local\\l2trace-ring-" + t.Proc.Id); }
                catch (System.IO.FileNotFoundException) { Thread.Sleep(10); }
            }
            if (t.ring == null) throw new Exception("no event ring - L2TRACE_EVENTS not honoured?");
            t.ringView = t.ring.CreateViewAccessor(0, RingHeader + RingCap * RecSize);
            t.Events = new System.IO.StreamWriter(events, false, new System.Text.UTF8Encoding(false));
        }
        return t;
    }

    public uint Shared(int off) { return view.ReadUInt32(off); }
    // Steps: the lockstep unit (native/l2-trace frame.cpp Step_Reached), one Battle_Frame a step.
    public uint Frames { get { return Shared(OffSteps); } }
    public uint Presents { get { return Shared(OffFrames); } }
    public uint Ticks { get { return Shared(OffTicks); } }
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
    // The last presented frame (TraceShared v3 fbAddr/palAddr) as an 8-bit BMP; false when none yet.
    public bool SaveShot(string path) {
        const int W = 640, H = 480;
        uint fb = Shared(OffFb), pal = Shared(OffPal); int n;
        var px = new byte[W * H]; var pe = new byte[1024];
        if (fb == 0 || !ReadProcessMemory(handle, new IntPtr((int)fb), px, px.Length, out n)) return false;
        if (!ReadProcessMemory(handle, new IntPtr((int)pal), pe, pe.Length, out n)) return false;
        var hdr = new byte[14 + 40 + 1024];
        Action<int, int> put = (at, v) => BitConverter.GetBytes(v).CopyTo(hdr, at);
        hdr[0] = (byte)'B'; hdr[1] = (byte)'M'; put(2, hdr.Length + W * H); put(10, hdr.Length);
        put(14, 40); put(18, W); put(22, H); hdr[26] = 1; hdr[28] = 8; put(46, 256);
        for (int i = 0; i < 256; i++) { hdr[54 + i * 4] = pe[i * 4 + 2]; hdr[55 + i * 4] = pe[i * 4 + 1]; hdr[56 + i * 4] = pe[i * 4]; }
        using (var f = System.IO.File.Create(path)) {
            f.Write(hdr, 0, hdr.Length);
            for (int y = H - 1; y >= 0; y--) f.Write(px, y * W, W);
        }
        return true;
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
            Drain();
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

    // FORMAT.md wait-idle: `frames` presents in a row whose events are only draw, text and rand.
    public bool WaitIdle(int frames, int timeoutMs) {
        Drain();
        uint start = Frames;
        return WaitUntil(() => { uint since = Math.Max(start, LastBusy + 1); return Frames >= since + (uint)frames; }, timeoutMs);
    }
    public int CountOf(string k) { Drain(); int n; Counts.TryGetValue(k, out n); return n; }

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
    // WM_RBUTTONDOWN/UP: the right button leaves most panels (screens-county.md 2.6).
    public void RClick(int x, int y) {
        MoveTo(x, y);
        WaitFrames(2, 5000);
        PostMessage(Hwnd, 0x0204, new IntPtr(2), XY(x, y));
        WaitFrames(2, 5000);
        PostMessage(Hwnd, 0x0205, IntPtr.Zero, XY(x, y));
    }
    // Left button held from (x0, y0) to (x1, y1) over 8 moves: the battle's selection box.
    public void Drag(int x0, int y0, int x1, int y1) {
        MoveTo(x0, y0);
        WaitFrames(2, 5000);
        PostMessage(Hwnd, 0x0201, new IntPtr(1), XY(x0, y0));
        WaitFrames(2, 5000);
        for (int i = 1; i <= 8; i++) {
            int x = x0 + (x1 - x0) * i / 8, y = y0 + (y1 - y0) * i / 8;
            view.Write(OffCursorX, x); view.Write(OffCursorY, y);
            PostMessage(Hwnd, 0x0200, new IntPtr(1), XY(x, y));
            WaitFrames(2, 5000);
        }
        PostMessage(Hwnd, 0x0202, IntPtr.Zero, XY(x1, y1));
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
    // The event ring (native/l2-trace/events.h): header 32 bytes, 16384 records of 128.
    const int RingHeader = 32, RingCap = 16384, RecSize = 128, OffHead = 12, OffTail = 16, OffKeep = 20;
    MemoryMappedFile ring;
    MemoryMappedViewAccessor ringView;
    public System.IO.StreamWriter Events;
    public static System.Collections.Generic.Dictionary<uint, string[]> Hooks = new System.Collections.Generic.Dictionary<uint, string[]>();
    public System.Collections.Generic.Dictionary<string, int> Counts = new System.Collections.Generic.Dictionary<string, int>();
    public int UnnamedSheets, SheetDraws, OkX = -1, OkY = -1;
    // Categories whose calls become FORMAT.md records other than draw/text/rand.
    static readonly string[] Busy = { "sound", "music", "voice", "msg", "ai", "film", "save", "newgame" };
    public uint LastBusy;
    int lastScreen = -1, lastPhase = -1;
    static readonly string[] Kinds = { "", "call", "ret", "frame", "phase", "pin", "refused" };
    public bool KeepDraws { set { if (ringView != null) ringView.Write(OffKeep, value ? 1 : 0); } }
    void Count(string k) { int n; Counts.TryGetValue(k, out n); Counts[k] = n + 1; }
    static string Esc(string s) {
        var sb = new System.Text.StringBuilder();
        foreach (char c in s) {
            if (c == '"' || c == '\\') sb.Append('\\').Append(c);
            else if (c < 0x20 || c > 0x7e) sb.AppendFormat("\\u{0:x4}", (int)c);
            else sb.Append(c);
        }
        return sb.ToString();
    }
    public void Note(string json) { Drain(); if (Events != null) Events.WriteLine(json); }
    public void Drain() {
        if (ringView == null || Events == null) return;
        uint head = ringView.ReadUInt32(OffHead), tail = ringView.ReadUInt32(OffTail);
        var b = new byte[RecSize];
        for (; tail != head; tail++) {
            ringView.ReadArray(RingHeader + (long)(tail % RingCap) * RecSize, b, 0, RecSize);
            uint seq = BitConverter.ToUInt32(b, 0), frame = BitConverter.ToUInt32(b, 4), fn = BitConverter.ToUInt32(b, 8), caller = BitConverter.ToUInt32(b, 12);
            int ret = BitConverter.ToInt32(b, 16), kind = b[22], nargs = b[23];
            string types = System.Text.Encoding.ASCII.GetString(b, 24, 8).TrimEnd('\0');
            int z = Array.IndexOf(b, (byte)0, 64); if (z < 0 || z > 127) z = 128;
            string text = System.Text.Encoding.GetEncoding(1252).GetString(b, 64, z - 64);
            var a = new int[8];
            for (int i = 0; i < 8; i++) a[i] = BitConverter.ToInt32(b, 32 + i * 4);
            string[] h; Hooks.TryGetValue(fn, out h);
            string name = h != null ? h[0] : string.Format("0x{0:X8}", fn), cat = h != null ? h[1] : "";
            string ev = kind < Kinds.Length ? Kinds[kind] : "k" + kind;
            var sb = new System.Text.StringBuilder();
            sb.AppendFormat("{{\"seq\":{0},\"f\":{1},\"ev\":\"{2}\"", seq, frame, ev);
            if (kind == 3) {
                sb.AppendFormat(",\"hash\":\"{0:x8}\",\"screen\":{1},\"phase\":{2},\"draws\":{3},\"rand\":{4},\"lfsr\":{5},\"rng\":[{6},{7},{8}],\"ticks\":{9},\"loops\":{10}}}",
                    ret, a[0], a[1], a[2], a[3], a[4], a[5], a[6], a[7], fn, caller);
                Count("frame");
                if (a[0] != lastScreen || a[1] != lastPhase) LastBusy = frame;
                lastScreen = a[0]; lastPhase = a[1];
            } else {
                if (kind == 1 && Array.IndexOf(Busy, cat) >= 0) LastBusy = frame;
                sb.AppendFormat(",\"fn\":\"{0}\",\"cat\":\"{1}\"", name, cat);
                if (caller != 0) sb.AppendFormat(",\"from\":\"0x{0:X8}\"", caller);
                if (kind == 2) sb.AppendFormat(",\"ret\":{0}", ret);
                if (nargs > 0) {
                    sb.Append(",\"args\":[");
                    for (int i = 0; i < nargs; i++) sb.Append(i > 0 ? "," : "").Append(a[i]);
                    sb.AppendFormat("],\"t\":\"{0}\"", types);
                }
                if (text.Length > 0) sb.AppendFormat(",\"text\":\"{0}\"", Esc(text));
                sb.Append("}");
                Count(ev + ":" + (cat == "" ? name : cat));
                if (kind == 1 && (name == "Pl8_DrawFrame" || name.StartsWith("TileBlit_"))) { SheetDraws++; if (text == "?" || text == "") UnnamedSheets++; }
                if (kind == 1 && name == "Ui_OkButton") { OkX = a[0]; OkY = a[1]; }
            }
            Events.WriteLine(sb.ToString());
        }
        ringView.Write(OffTail, tail);
    }
    public void Dispose() {
        Drain();
        if (Events != null) { Events.Flush(); Events.Dispose(); Events = null; }
        if (Proc != null && !Proc.HasExited) { Proc.Kill(); Proc.WaitForExit(5000); }
        if (view != null) view.Dispose();
        if (map != null) map.Dispose();
        if (ringView != null) ringView.Dispose();
        if (ring != null) ring.Dispose();
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

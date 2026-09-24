// IAT and code patches on Lords2.exe, installed from DllMain after the loader has bound the
// exe's imports. Every patch checks what it replaces and refuses on a mismatch.
#include "trace.h"
#include "events.h"
#include <cstdio>
#include <intrin.h>
#include <cstring>

namespace {
volatile LONG g_clock = 0;     // virtual ms since launch
constexpr DWORD kClockBase = 0x01000000;
LONG g_step = 1;
HWND g_gameHwnd = nullptr;
bool g_ok = true;

void Refuse(const char* what) {
    Log("REFUSED %s", what);
    g_shared->patchesRefused++;
    g_ok = false;
}

void** FindSlot(HMODULE module, const char* dll, const char* fn) {
    auto base = reinterpret_cast<uint8_t*>(module);
    auto nt = reinterpret_cast<IMAGE_NT_HEADERS*>(base + reinterpret_cast<IMAGE_DOS_HEADER*>(base)->e_lfanew);
    auto& dir = nt->OptionalHeader.DataDirectory[IMAGE_DIRECTORY_ENTRY_IMPORT];
    for (auto d = reinterpret_cast<IMAGE_IMPORT_DESCRIPTOR*>(base + dir.VirtualAddress); d->Name; d++) {
        if (_stricmp(reinterpret_cast<char*>(base + d->Name), dll) != 0) continue;
        auto names = reinterpret_cast<IMAGE_THUNK_DATA*>(base + (d->OriginalFirstThunk ? d->OriginalFirstThunk : d->FirstThunk));
        auto slots = reinterpret_cast<void**>(base + d->FirstThunk);
        for (int i = 0; names[i].u1.AddressOfData; i++) {
            if (IMAGE_SNAP_BY_ORDINAL(names[i].u1.Ordinal)) continue;
            auto byName = reinterpret_cast<IMAGE_IMPORT_BY_NAME*>(base + names[i].u1.AddressOfData);
            if (strcmp(reinterpret_cast<char*>(byName->Name), fn) == 0) return &slots[i];
        }
    }
    return nullptr;
}

// expectSlot: the IAT address the design notes give, 0 where none was recorded.
void HookIat(const char* dll, const char* fn, void* repl, uintptr_t expectSlot, HMODULE in = GetModuleHandleA(nullptr)) {
    char what[96];
    _snprintf_s(what, sizeof(what), _TRUNCATE, "%s!%s", dll, fn);
    void** slot = in ? FindSlot(in, dll, fn) : nullptr;
    void* real = GetProcAddress(GetModuleHandleA(dll), fn);
    if (!slot) { Refuse(what); return; }
    if (expectSlot && reinterpret_cast<uintptr_t>(slot) != expectSlot) { Refuse(what); return; }
    if (*slot != real) { Log("slot %p holds %p, export is %p", slot, *slot, real); Refuse(what); return; }
    DWORD old;
    VirtualProtect(slot, sizeof(void*), PAGE_READWRITE, &old);
    *slot = repl;
    VirtualProtect(slot, sizeof(void*), old, &old);
}

void PatchCode(const char* what, uintptr_t addr, const uint8_t* expect, size_t n, const uint8_t* repl, size_t m) {
    auto p = reinterpret_cast<uint8_t*>(addr);
    if (memcmp(p, expect, n) != 0) { Refuse(what); return; }
    DWORD old;
    VirtualProtect(p, m, PAGE_EXECUTE_READWRITE, &old);
    memcpy(p, repl, m);
    VirtualProtect(p, m, old, &old);
    FlushInstructionCache(GetCurrentProcess(), p, m);
}

DWORD Tick() {
    LONG t = InterlockedExchangeAdd(&g_clock, g_step) + g_step;
    g_shared->virtualMs = t;
    return kClockBase + t;
}

DWORD WINAPI H_timeGetTime() { return Tick(); }
DWORD WINAPI H_GetTickCount() { return Tick(); }
void WINAPI H_Sleep(DWORD ms) { Clock_Advance(ms); Sleep(0); }

// A fixed calendar: _time 0x004C2740 and any clock seed read the same date every run.
void Calendar(LPSYSTEMTIME st) {
    SYSTEMTIME start = {1997, 3, 1, 3, 9, 0, 0, 0};
    FILETIME ft;
    SystemTimeToFileTime(&start, &ft);
    ULARGE_INTEGER u{{ft.dwLowDateTime, ft.dwHighDateTime}};
    u.QuadPart += static_cast<ULONGLONG>(g_clock) * 10000;
    ft = {u.LowPart, u.HighPart};
    FileTimeToSystemTime(&ft, st);
}
void WINAPI H_GetLocalTime(LPSYSTEMTIME st) { Calendar(st); }
void WINAPI H_GetSystemTime(LPSYSTEMTIME st) { Calendar(st); }

// Music_StartRefill 0x00426B92 is the only caller (Music_Refill 0x00426DD5, periodic). The
// callback runs on the main thread at the first present at or after its virtual due time.
struct Timer { UINT id, delay, flags; LPTIMECALLBACK cb; DWORD_PTR user; LONG due; bool live; };
Timer g_timers[16];
UINT g_nextTimer = 1;
bool g_runTimers = true;
UINT WINAPI H_timeSetEvent(UINT delay, UINT, LPTIMECALLBACK cb, DWORD_PTR user, UINT flags) {
    UINT id = g_nextTimer++;
    for (auto& t : g_timers) {
        if (t.live || !g_runTimers) continue;
        t = {id, delay ? delay : 1, flags, cb, user, g_clock + static_cast<LONG>(delay), true};
        Log("timeSetEvent %u: every %u virtual ms, callback %p user %lu", id, delay, cb, static_cast<unsigned long>(user));
        return id;
    }
    g_shared->timersDropped++;
    Log("timeSetEvent dropped: delay %u callback %p user %lu", delay, cb, static_cast<unsigned long>(user));
    return id;
}
UINT WINAPI H_timeKillEvent(UINT id) {
    for (auto& t : g_timers) if (t.live && t.id == id) t.live = false;
    return 0;
}
// smackw32's own sound service timer: dropped, its wave output is refused below.
UINT WINAPI H_SmkTimeSetEvent(UINT delay, UINT, LPTIMECALLBACK cb, DWORD_PTR, UINT) {
    g_shared->timersDropped++;
    Log("smackw32 timeSetEvent dropped: delay %u callback %p", delay, cb);
    return 0x7000 + g_nextTimer++;
}
// smackw32 plays film sound through waveOut, not DirectSound; refusing the device silences it.
MMRESULT WINAPI H_waveOutOpen(LPHWAVEOUT, UINT, LPCWAVEFORMATEX, DWORD_PTR, DWORD_PTR, DWORD) {
    Log("waveOutOpen refused (silent)");
    return MMSYSERR_NODRIVER;
}

// WM_ACTIVATEAPP in App_WndProc 0x004B29BE asks these two before setting g_windowActive.
HWND WINAPI H_GetForegroundWindow() { return g_gameHwnd; }
BOOL WINAPI H_IsIconic(HWND) { return FALSE; }
// FUN_004B191E polls the position; stores it at 0x004EABBE/0x004EABC0.
BOOL WINAPI H_GetCursorPos(LPPOINT p) { p->x = g_shared->cursorX; p->y = g_shared->cursorY; return TRUE; }
// App_InitWindow sizes its window and g_presentRect from these; a 640x480 desktop is the original's mode.
int WINAPI H_GetSystemMetrics(int i) { return i == SM_CXSCREEN ? 640 : i == SM_CYSCREEN ? 480 : GetSystemMetrics(i); }

HWND WINAPI H_CreateWindowExA(DWORD ex, LPCSTR cls, LPCSTR title, DWORD style, int x, int y, int w,
                              int h, HWND parent, HMENU menu, HINSTANCE inst, LPVOID param) {
    HWND hwnd = CreateWindowExA(ex & ~WS_EX_TOPMOST, cls, title, style & ~WS_VISIBLE, x, y, w, h,
                                parent, menu, inst, param);
    if (!g_gameHwnd) { g_gameHwnd = hwnd; g_shared->hwnd = static_cast<uint32_t>(reinterpret_cast<uintptr_t>(hwnd)); }
    Log("CreateWindowExA style %08lx -> hidden hwnd %p", style, hwnd);
    return hwnd;
}
BOOL WINAPI H_ShowWindow(HWND, int cmd) { Log("ShowWindow(%d) suppressed", cmd); return FALSE; }
BOOL WINAPI H_SetForegroundWindow(HWND) { return TRUE; }
HWND WINAPI H_SetFocus(HWND) { return nullptr; }
BOOL WINAPI H_SetWindowPos(HWND h, HWND after, int x, int y, int cx, int cy, UINT f) {
    return SetWindowPos(h, after, x, y, cx, cy, (f & ~SWP_SHOWWINDOW) | SWP_NOACTIVATE);
}
LONG WINAPI H_SetWindowLongA(HWND h, int i, LONG v) {
    if (i == GWL_STYLE) v &= ~WS_VISIBLE;
    return SetWindowLongA(h, i, v);
}
int WINAPI H_MessageBoxA(HWND, LPCSTR text, LPCSTR caption, UINT) {
    Log("MessageBoxA suppressed: [%s] %s", caption ? caption : "", text ? text : "");
    return IDOK;
}
// Display_Init 0x0042E5C0: g_desktopColours = BITSPIXEL x PLANES, 16 if NUMCOLORS is -1; not 8
// raises message 298 "Non 256 colour display" (Battle_Frame 0x004B99C0). An 8-bit palette desktop.
int WINAPI H_GetDeviceCaps(HDC dc, int i) {
    return i == BITSPIXEL ? 8 : i == PLANES ? 1 : i == NUMCOLORS ? 20 : GetDeviceCaps(dc, i);
}
BOOL WINAPI H_WinHelpA(HWND, LPCSTR, UINT, ULONG_PTR) { return TRUE; }

// Smk_Play 0x0042D91B, its failed-open branch: g_screenId 0x004EAC50 = returnScreen, return 0.
int __cdecl H_SmkPlay(const char* path, int, int, int, int returnScreen) {
    g_shared->filmsSkipped++;
    Events_Film(path, reinterpret_cast<uint32_t>(_ReturnAddress()));
    Log("film skipped: %s -> screen %d", path ? path : "?", returnScreen);
    *reinterpret_cast<volatile uint8_t*>(0x004EAC50) = static_cast<uint8_t>(returnScreen);
    return 0;
}
}

uint32_t Clock_Now() { return static_cast<uint32_t>(g_clock); }

void Timers_Run() {
    static bool inside = false;
    if (inside) return;
    inside = true;
    for (auto& t : g_timers) {
        while (t.live && g_clock >= t.due) {
            g_shared->timersRun++;
            t.cb(t.id, 0, t.user, 0, 0);
            if (t.flags & TIME_PERIODIC) t.due += t.delay; else t.live = false;
        }
    }
    inside = false;
}
void Clock_Advance(uint32_t ms) {
    if (!ms) return;
    g_shared->virtualMs = InterlockedExchangeAdd(&g_clock, static_cast<LONG>(ms)) + ms;
}

bool Hooks_Install() {
    g_step = EnvInt("L2TRACE_STEP", 1);
    char timers[16] = "run";
    EnvStr("L2TRACE_TIMERS", timers, sizeof(timers));
    g_runTimers = _stricmp(timers, "drop") != 0;
    const char* k32 = "KERNEL32.dll";
    const char* u32 = "USER32.dll";
    const char* mm = "WINMM.dll";
    if (!EnvInt("L2TRACE_REALTIME", 0)) {
        HookIat(mm, "timeGetTime", H_timeGetTime, 0x005CF5C4);
        HookIat(k32, "GetTickCount", H_GetTickCount, 0x005CF3D8);
        HookIat(k32, "Sleep", H_Sleep, 0x005CF4AC);
        HookIat(k32, "GetLocalTime", H_GetLocalTime, 0x005CF444);
        HookIat(k32, "GetSystemTime", H_GetSystemTime, 0x005CF454);
        HookIat(mm, "timeSetEvent", H_timeSetEvent, 0x005CF5BC);
        HookIat(mm, "timeKillEvent", H_timeKillEvent, 0x005CF5B4);
        // FUN_0040F10F times Cpu_SpinDelay and 10 presents into 0x004D2168/0x004D216C;
        // read by NetCmd_WriteJoin 0x00441066 and a debug readout. 10/50 ms: [I], any fixed answer.
        const uint8_t expect[] = {0x55, 0x8B, 0xEC, 0x83, 0xEC, 0x08, 0x53, 0x56, 0x57, 0xFF, 0x15, 0xC4, 0xF5, 0x5C, 0x00};
        const uint8_t calib[] = {0xC7, 0x05, 0x68, 0x21, 0x4D, 0x00, 10, 0, 0, 0,
                                 0xC7, 0x05, 0x6C, 0x21, 0x4D, 0x00, 50, 0, 0, 0, 0xC3};
        PatchCode("FUN_0040F10F calibration", 0x0040F10F, expect, sizeof(expect), calib, sizeof(calib));
    }
    HookIat(u32, "GetForegroundWindow", H_GetForegroundWindow, 0x005CF550);
    HookIat(u32, "IsIconic", H_IsIconic, 0x005CF54C);
    HookIat(u32, "GetCursorPos", H_GetCursorPos, 0x005CF57C);
    HookIat(u32, "GetSystemMetrics", H_GetSystemMetrics, 0);
    HookIat(u32, "CreateWindowExA", H_CreateWindowExA, 0);
    HookIat(u32, "ShowWindow", H_ShowWindow, 0);
    HookIat(u32, "SetForegroundWindow", H_SetForegroundWindow, 0);
    HookIat(u32, "SetFocus", H_SetFocus, 0);
    HookIat(u32, "SetWindowPos", H_SetWindowPos, 0);
    HookIat(u32, "SetWindowLongA", H_SetWindowLongA, 0);
    HookIat(u32, "MessageBoxA", H_MessageBoxA, 0);
    HookIat(u32, "WinHelpA", H_WinHelpA, 0);
    HookIat("GDI32.dll", "GetDeviceCaps", H_GetDeviceCaps, 0);
    char films[16] = "play";
    EnvStr("L2TRACE_FILMS", films, sizeof(films));
    if (_stricmp(films, "skip") == 0) {
        const uint8_t expect[] = {0x55, 0x8B, 0xEC, 0x53, 0x56, 0x57};
        uint8_t jmp[5] = {0xE9};
        int32_t rel = static_cast<int32_t>(reinterpret_cast<uintptr_t>(&H_SmkPlay) - (0x0042D91B + 5));
        memcpy(jmp + 1, &rel, 4);
        PatchCode("Smk_Play 0x0042D91B", 0x0042D91B, expect, sizeof(expect), jmp, sizeof(jmp));
    }
    // smackw32 has its own imports: a window, wave output and timers of its own.
    if (HMODULE smk = GetModuleHandleA("smackw32.dll")) {
        HookIat(u32, "CreateWindowExA", H_CreateWindowExA, 0, smk);
        HookIat(u32, "ShowWindow", H_ShowWindow, 0, smk);
        HookIat(mm, "waveOutOpen", H_waveOutOpen, 0, smk);
        HookIat(mm, "timeSetEvent", H_SmkTimeSetEvent, 0, smk);
        // _SmackWait_4 in Smk_PlayLoop 0x0042DBC7 paces films on smackw32's own clock.
        if (!EnvInt("L2TRACE_REALTIME", 0)) HookIat(mm, "timeGetTime", H_timeGetTime, 0, smk);
    }
    Log("hooks installed: step %ld ms per clock read, films %s, %s", g_step, films,
        g_ok ? "all patches applied" : "SOME REFUSED");
    return g_ok;
}

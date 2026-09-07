// A proxy dplayx.dll for Lords of the Realm II.
//
// Lords2.exe imports exactly two functions from DPLAYX.dll, BY ORDINAL:
//   IAT 0x005CF364 = ordinal 1 (DirectPlayCreate),     called from 0x004B826E
//   IAT 0x005CF368 = ordinal 2 (DirectPlayEnumerateA), called from 0x004B7FCE
// (verified with tools/net/dpimports.js against the shipped exe). That is the
// game's entire network entry surface, so a two-export proxy sees everything
// that crosses the boundary.
//
// Same pattern as native/ddraw-proxy: drop the DLL beside the executable in a
// hard-linked sandbox, forward to the real DLL in the system directory, log
// from inside the process. See docs/decisions.md D8 for why injection rather
// than synthetic input.
//
// The interesting part is not the two entry points - it is the COM interface
// DirectPlayCreate hands back. Everything the game does with DirectPlay after
// creation is a vtable call on that interface, so this proxy wraps it:
//
//   * WrapObject() returns a small object whose first field is a vtable of our
//     own naked thunks. The game holds our wrapper; DirectPlay never sees it.
//   * Each thunk logs (slot, return address, first six stack dwords), rewrites
//     the `this` slot on the caller's stack to the real interface pointer, and
//     tail-jumps to the real method. Tail-jumping means we never need to know a
//     method's argument count - the real callee cleans the stack, exactly as it
//     would have. That matters because there is no dplay.h on this machine.
//   * QueryInterface / AddRef / Release are typed hooks instead, because
//     QueryInterface has to wrap whatever new interface comes back and its
//     signature (REFIID, void**) is not in doubt.
//
// Everything logged as "slot N" is measured. Method and GUID *names* come from
// tables below reconstructed from the DirectX headers and are marked inferred;
// the raw slot number and raw GUID are always printed alongside so the capture
// stays usable even where a name is wrong.

#include <windows.h>
#include <intrin.h>
#include <cstdio>
#include <cstdarg>
#include <cstring>

#define MAX_SLOTS   96   // IDirectPlay4A is ~53 slots; 96 is slack and costs nothing
#define MAX_WRAPS   32

// extern "C" and file scope: the MSVC inline asm below refers to these by name.
extern "C" void* __cdecl SpyDispatch(void* self, int slot, DWORD* argp);
extern "C" void  __cdecl EnumCbLog(DWORD* argp);
extern "C" void* g_gameEnumCb = 0;

static void Logf(const char* fmt, ...);

// ---------------------------------------------------------------- log plumbing
static HMODULE g_self = 0;
static HMODULE g_real = 0;
static CRITICAL_SECTION g_cs;
static bool g_csReady = false;
static DWORD g_t0 = 0;

static void LogPath(char* out, size_t n) {
    char dir[MAX_PATH] = {};
    GetModuleFileNameA(g_self, dir, MAX_PATH);
    char* slash = strrchr(dir, '\\');
    if (slash) *(slash + 1) = '\0';
    _snprintf_s(out, n, _TRUNCATE, "%sl2dplay.log", dir);
}

// Opened and closed per line: if the game dies mid-session we keep everything
// written up to that point. Same reasoning as the ddraw proxy.
static void RawWrite(const char* s, int n) {
    char path[MAX_PATH];
    LogPath(path, sizeof(path));
    HANDLE h = CreateFileA(path, FILE_APPEND_DATA, FILE_SHARE_READ | FILE_SHARE_WRITE,
                           0, OPEN_ALWAYS, FILE_ATTRIBUTE_NORMAL, 0);
    if (h == INVALID_HANDLE_VALUE) return;
    DWORD written = 0;
    WriteFile(h, s, (DWORD)n, &written, 0);
    CloseHandle(h);
}

// A polled Receive() loop would otherwise produce a megabyte of identical
// lines, so identical consecutive calls are collapsed into a count.
static void*  g_repObj  = 0;
static int    g_repSlot = -1;
static DWORD  g_repRet  = 0;
static int    g_repCount = 0;

static void FlushRepeat_locked() {
    if (g_repCount <= 0) return;
    char line[160];
    int n = _snprintf_s(line, sizeof(line), _TRUNCATE,
                        "            ... same call repeated %d more time(s)\r\n", g_repCount);
    RawWrite(line, n);
    g_repCount = 0;
}

static void Logf(const char* fmt, ...) {
    char line[2048];
    int p = _snprintf_s(line, sizeof(line), _TRUNCATE, "[%7lu] ",
                        (unsigned long)(GetTickCount() - g_t0));
    va_list a;
    va_start(a, fmt);
    int n = _vsnprintf_s(line + p, sizeof(line) - p, _TRUNCATE, fmt, a);
    va_end(a);
    if (n < 0) n = (int)strlen(line + p);
    if (g_csReady) EnterCriticalSection(&g_cs);
    FlushRepeat_locked();
    g_repObj = 0; g_repSlot = -1;
    RawWrite(line, p + n);
    if (g_csReady) LeaveCriticalSection(&g_cs);
}

// Reads a possibly-bogus C string out of the game's memory without dying.
static void SafeStr(const void* p, char* out, size_t n) {
    out[0] = '\0';
    if (!p) { strcpy_s(out, n, "(null)"); return; }
    __try {
        const char* s = (const char*)p;
        size_t i = 0;
        for (; i + 1 < n && s[i]; i++) {
            char c = s[i];
            out[i] = (c >= 0x20 && (unsigned char)c < 0x7f) ? c : '.';
        }
        out[i] = '\0';
    } __except (EXCEPTION_EXECUTE_HANDLER) {
        strcpy_s(out, n, "(unreadable)");
    }
}

static DWORD SafeDword(const void* p, DWORD def) {
    __try { return *(const DWORD*)p; } __except (EXCEPTION_EXECUTE_HANDLER) { return def; }
}

// ---------------------------------------------------------------- GUID naming
// INFERRED. Reconstructed from the DirectX headers; dplay.h is not present on
// this machine. The raw GUID is always logged next to the name, so a wrong name
// here cannot corrupt the capture.
struct GuidName { const char* name; GUID g; };
static const GuidName kGuids[] = {
    { "IID_IUnknown",       { 0x00000000,0x0000,0x0000,{0xC0,0x00,0x00,0x00,0x00,0x00,0x00,0x46} } },
    { "IID_IDirectPlay",    { 0x5454E9A0,0xDB65,0x11CE,{0x92,0x1C,0x00,0xAA,0x00,0x6C,0x49,0x72} } },
    { "IID_IDirectPlay2",   { 0x2B74F7C0,0x9154,0x11CF,{0xA9,0xCD,0x00,0xAA,0x00,0x68,0x86,0xE3} } },
    { "IID_IDirectPlay2A",  { 0x9D460580,0xA822,0x11CF,{0x96,0x0C,0x00,0x80,0xC7,0x53,0x4E,0x82} } },
    { "IID_IDirectPlay3",   { 0x133EFE40,0x32DC,0x11D0,{0x9C,0xFB,0x00,0xA0,0xC9,0x0A,0x43,0xCB} } },
    { "IID_IDirectPlay3A",  { 0x133EFE41,0x32DC,0x11D0,{0x9C,0xFB,0x00,0xA0,0xC9,0x0A,0x43,0xCB} } },
    { "CLSID_DirectPlay",   { 0xD1EB6D20,0x8923,0x11D0,{0x9D,0x97,0x00,0xA0,0xC9,0x0A,0x43,0xCB} } },
    { "IID_IDirectPlay4",   { 0x0AB1C530,0x4745,0x11D1,{0xA7,0xA1,0x00,0x00,0xF8,0x03,0xAB,0xFC} } },
    { "IID_IDirectPlay4A",  { 0x0AB1C531,0x4745,0x11D1,{0xA7,0xA1,0x00,0x00,0xF8,0x03,0xAB,0xFC} } },
    { "DPSPGUID_IPX",       { 0x685BC400,0x9D2C,0x11CF,{0xA9,0xCD,0x00,0xAA,0x00,0x68,0x86,0xE3} } },
    { "DPSPGUID_TCPIP",     { 0x36E95EE0,0x8577,0x11CF,{0x96,0x0C,0x00,0x80,0xC7,0x53,0x4E,0x82} } },
    { "DPSPGUID_SERIAL",    { 0x0F1D6860,0x88D9,0x11CF,{0x9C,0x4E,0x00,0xA0,0xC9,0x05,0x42,0x5E} } },
    { "DPSPGUID_MODEM",     { 0x44EAA760,0xCB68,0x11CF,{0x9C,0x4E,0x00,0xA0,0xC9,0x05,0x42,0x5E} } },
};

static void GuidStr(const void* pg, char* out, size_t n) {
    if (!pg) { strcpy_s(out, n, "NULL"); return; }
    GUID g;
    __try { memcpy(&g, pg, sizeof(GUID)); }
    __except (EXCEPTION_EXECUTE_HANDLER) { strcpy_s(out, n, "(unreadable guid)"); return; }
    char raw[64];
    _snprintf_s(raw, sizeof(raw), _TRUNCATE,
        "{%08lX-%04X-%04X-%02X%02X-%02X%02X%02X%02X%02X%02X}",
        (unsigned long)g.Data1, g.Data2, g.Data3,
        g.Data4[0], g.Data4[1], g.Data4[2], g.Data4[3],
        g.Data4[4], g.Data4[5], g.Data4[6], g.Data4[7]);
    for (int i = 0; i < (int)(sizeof(kGuids) / sizeof(kGuids[0])); i++) {
        if (memcmp(&g, &kGuids[i].g, sizeof(GUID)) == 0) {
            _snprintf_s(out, n, _TRUNCATE, "%s %s", raw, kGuids[i].name);
            return;
        }
    }
    _snprintf_s(out, n, _TRUNCATE, "%s (unknown)", raw);
}

// ------------------------------------------------------- method-name tables
// INFERRED, from the DirectX headers. The DirectPlay vtables are alphabetical
// after IUnknown, which is a useful self-check. Slot numbers in the log are
// measured; these names are a convenience only.
static const char* kDP1[] = {
    "QueryInterface", "AddRef", "Release",
    "AddPlayerToGroup", "Close", "CreatePlayer", "CreateGroup",
    "DeletePlayerFromGroup", "DestroyPlayer", "DestroyGroup",
    "EnableNewPlayers", "EnumGroupPlayers", "EnumGroups", "EnumPlayers",
    "EnumSessions", "GetCaps", "GetMessageCount", "GetPlayerCaps",
    "GetPlayerName", "Initialize", "Open", "Receive", "SaveSession",
    "Send", "SetPlayerName",
};
static const char* kDP2[] = {
    "QueryInterface", "AddRef", "Release",
    "AddPlayerToGroup", "Close", "CreateGroup", "CreatePlayer",
    "DeletePlayerFromGroup", "DestroyGroup", "DestroyPlayer",
    "EnumGroupPlayers", "EnumGroups", "EnumPlayers", "EnumSessions",
    "GetCaps", "GetGroupData", "GetGroupName", "GetMessageCount",
    "GetPlayerAddress", "GetPlayerCaps", "GetPlayerData", "GetPlayerName",
    "GetSessionDesc", "Initialize", "Open", "Receive", "Send",
    "SetGroupData", "SetGroupName", "SetPlayerData", "SetPlayerName",
    "SetSessionDesc",
};
static const char* kDP3extra[] = {   // appended after the 32 IDirectPlay2 slots
    "AddGroupToGroup", "CreateGroupInGroup", "DeleteGroupFromGroup",
    "EnumConnections", "EnumGroupsInGroup", "GetGroupConnectionSettings",
    "InitializeConnection", "SecureOpen", "SendChatMessage",
    "SetGroupConnectionSettings", "StartSession", "GetGroupFlags",
    "GetGroupParent", "GetPlayerAccount", "GetPlayerFlags",
};
static const char* kDP4extra[] = {   // appended after IDirectPlay3's 47 slots
    "GetGroupOwner", "SetGroupOwner", "SendEx", "GetMessageQueue",
    "CancelMessage", "CancelPriority",
};

// ---------------------------------------------------------------- wrappers
struct Wrap {
    void**      vt;        // MUST be first: this is what the game dereferences
    void*       real;      // the genuine interface pointer
    void**      realVt;    // its genuine vtable
    const char* iface;     // best guess at which interface this is
    int         id;
    bool        used;
};

static void* g_vtStore[MAX_WRAPS][MAX_SLOTS];
static Wrap  g_wraps[MAX_WRAPS];
static int   g_wrapCount = 0;
extern "C" void* g_thunks[MAX_SLOTS];

static Wrap* FindWrap(void* self) {
    for (int i = 0; i < g_wrapCount; i++)
        if (&g_wraps[i] == (Wrap*)self) return &g_wraps[i];
    return 0;
}
static Wrap* FindWrapByReal(void* real) {
    for (int i = 0; i < g_wrapCount; i++)
        if (g_wraps[i].used && g_wraps[i].real == real) return &g_wraps[i];
    return 0;
}

static const char* SlotName(const char* iface, int slot) {
    if (!iface) return "?";
    if (strstr(iface, "DirectPlay4")) {
        if (slot < 32) return kDP2[slot];
        if (slot < 47) return kDP3extra[slot - 32];
        if (slot < 53) return kDP4extra[slot - 47];
        return "?";
    }
    if (strstr(iface, "DirectPlay3")) {
        if (slot < 32) return kDP2[slot];
        if (slot < 47) return kDP3extra[slot - 32];
        return "?";
    }
    if (strstr(iface, "DirectPlay2")) return slot < 32 ? kDP2[slot] : "?";
    if (strstr(iface, "IDirectPlay1")) return slot < 25 ? kDP1[slot] : "?";
    // Unknown interface: IDirectPlay2 naming is the best single guess.
    return slot < 32 ? kDP2[slot] : "?";
}

static HRESULT __stdcall QI_Hook(void* self, GUID* riid, void** ppv);
static ULONG   __stdcall AddRef_Hook(void* self);
static ULONG   __stdcall Release_Hook(void* self);

static char g_wrapNames[MAX_WRAPS][80];
static void InstallTypedHooks(Wrap* w);

static void* WrapObject(void* real, const char* iface) {
    if (!real) return 0;
    Wrap* existing = FindWrapByReal(real);
    if (existing) return existing;
    if (g_wrapCount >= MAX_WRAPS) {
        Logf("[proxy] out of wrapper slots, handing back the raw interface %p\r\n", real);
        return real;
    }
    Wrap* w = &g_wraps[g_wrapCount];
    w->id     = g_wrapCount;
    w->real   = real;
    w->realVt = *(void***)real;
    strcpy_s(g_wrapNames[g_wrapCount], sizeof(g_wrapNames[0]), iface);
    w->iface  = g_wrapNames[g_wrapCount];
    w->used   = true;
    w->vt     = g_vtStore[g_wrapCount];
    for (int i = 0; i < MAX_SLOTS; i++) w->vt[i] = g_thunks[i];
    w->vt[0] = (void*)&QI_Hook;
    w->vt[1] = (void*)&AddRef_Hook;
    w->vt[2] = (void*)&Release_Hook;
    g_wrapCount++;

    // The real vtable is a handy fingerprint: two interfaces sharing one vtable
    // are the same interface, whatever the game asked for.
    Logf("[proxy] wrap #%d %s real=%p realVt=%p [vt3]=%p -> wrapper=%p\r\n",
         w->id, w->iface, real, w->realVt, w->realVt[3], w);
    InstallTypedHooks(w);
    return w;
}

// Called from every naked thunk. Logs, then rewrites the caller's `this` slot
// to the real interface pointer and returns the real method to jump to.
extern "C" void* __cdecl SpyDispatch(void* self, int slot, DWORD* argp) {
    Wrap* w = FindWrap(self);
    if (!w) {
        // Impossible unless memory is corrupt: the thunk is only reachable
        // through a vtable we built. Fail loudly rather than jumping to junk.
        Logf("[proxy] !! dispatch on unknown wrapper %p slot %d\r\n", self, slot);
        return 0;
    }
    DWORD ret = argp[-2];                 // argp[-1] = this slot, argp[-2] = return address
    DWORD a[6];
    for (int i = 0; i < 6; i++) a[i] = SafeDword(&argp[i], 0xDEADBEEF);

    if (g_csReady) EnterCriticalSection(&g_cs);
    if (self == g_repObj && slot == g_repSlot && ret == g_repRet) {
        g_repCount++;
        if (g_csReady) LeaveCriticalSection(&g_cs);
        argp[-1] = (DWORD)(ULONG_PTR)w->real;
        return w->realVt[slot];
    }
    FlushRepeat_locked();
    char line[512];
    int n = _snprintf_s(line, sizeof(line), _TRUNCATE,
        "[%7lu] #%d %s slot %2d %-22s from %08lX  args %08lX %08lX %08lX %08lX %08lX %08lX\r\n",
        (unsigned long)(GetTickCount() - g_t0), w->id, w->iface, slot,
        SlotName(w->iface, slot), (unsigned long)ret,
        (unsigned long)a[0], (unsigned long)a[1], (unsigned long)a[2],
        (unsigned long)a[3], (unsigned long)a[4], (unsigned long)a[5]);
    RawWrite(line, n);
    g_repObj = self; g_repSlot = slot; g_repRet = ret; g_repCount = 0;
    if (g_csReady) LeaveCriticalSection(&g_cs);

    argp[-1] = (DWORD)(ULONG_PTR)w->real;
    return w->realVt[slot];
}

typedef HRESULT (__stdcall *QIFn)(void*, GUID*, void**);
typedef ULONG   (__stdcall *RefFn)(void*);

static HRESULT __stdcall QI_Hook(void* self, GUID* riid, void** ppv) {
    Wrap* w = FindWrap(self);
    if (!w) return E_FAIL;
    char g[160];
    GuidStr(riid, g, sizeof(g));
    HRESULT hr = ((QIFn)w->realVt[0])(w->real, riid, ppv);
    void* out = (ppv && SUCCEEDED(hr)) ? *ppv : 0;
    Logf("[proxy] #%d QueryInterface(%s) -> hr=0x%08lX iface=%p\r\n",
         w->id, g, (unsigned long)hr, out);
    if (out) {
        // Name the new wrapper after the IID that produced it, so slot names
        // downstream come from the right table.
        const char* nm = strchr(g, ' ') ? strchr(g, ' ') + 1 : g;
        *ppv = WrapObject(out, nm);
    }
    return hr;
}

static ULONG __stdcall AddRef_Hook(void* self) {
    Wrap* w = FindWrap(self);
    if (!w) return 0;
    ULONG r = ((RefFn)w->realVt[1])(w->real);
    Logf("[proxy] #%d AddRef -> %lu\r\n", w->id, (unsigned long)r);
    return r;
}

static ULONG __stdcall Release_Hook(void* self) {
    Wrap* w = FindWrap(self);
    if (!w) return 0;
    ULONG r = ((RefFn)w->realVt[2])(w->real);
    Logf("[proxy] #%d Release -> %lu\r\n", w->id, (unsigned long)r);
    if (r == 0) w->used = false;
    return r;
}

// ------------------------------------ typed hooks for the interesting slots
// The generic thunks log a slot index and six raw stack dwords, which is
// enough to see the shape of a session but not its content. For the handful of
// IDirectPlay2 methods that carry the actual game traffic it is worth knowing
// the real signature, so these are typed. Every one of them is quoted from
// dplay.h, and the slot numbers are the measured ones (IDirectPlay2 has exactly
// 32 vtable slots - confirmed on this machine by vtable adjacency in
// tools/net/dptest.cpp, not just by the header).
//
// Installed only on wrappers whose interface is IDirectPlay2/3/4, since those
// three share slots 0-31. Never on the version-1 interface, whose layout
// differs (CreatePlayer and CreateGroup are the other way round, and it has
// EnableNewPlayers and SaveSession that v2 does not).

static void HexDump(const void* p, DWORD n, const char* prefix) {
    if (!p || !n) return;
    if (n > 256) n = 256;                  // a move packet, not a file transfer
    unsigned char buf[256];
    __try { memcpy(buf, p, n); }
    __except (EXCEPTION_EXECUTE_HANDLER) { Logf("%s(unreadable)\r\n", prefix); return; }
    for (DWORD off = 0; off < n; off += 16) {
        char line[128];
        int k = 0;
        for (DWORD i = 0; i < 16; i++) {
            if (off + i < n) k += _snprintf_s(line + k, sizeof(line) - k, _TRUNCATE, "%02X ", buf[off + i]);
            else             k += _snprintf_s(line + k, sizeof(line) - k, _TRUNCATE, "   ");
        }
        char asc[20];
        DWORD j = 0;
        for (; j < 16 && off + j < n; j++) {
            unsigned char c = buf[off + j];
            asc[j] = (c >= 0x20 && c < 0x7f) ? (char)c : '.';
        }
        asc[j] = '\0';
        Logf("%s%04lX  %s |%s|\r\n", prefix, (unsigned long)off, line, asc);
    }
}

// DPSESSIONDESC2 is 80 bytes: dwSize, dwFlags, guidInstance[16],
// guidApplication[16], dwMaxPlayers, dwCurrentPlayers, lpszSessionName,
// lpszPassword, then four reserved/user dwords each.
static void LogSessionDesc(const void* p, const char* label) {
    if (!p) { Logf("[proxy]   %s = NULL\r\n", label); return; }
    DWORD d[20];
    __try { memcpy(d, p, sizeof(d)); }
    __except (EXCEPTION_EXECUTE_HANDLER) { Logf("[proxy]   %s unreadable\r\n", label); return; }
    char gi[160], ga[160], nm[128];
    GuidStr((const unsigned char*)p + 8, gi, sizeof(gi));
    GuidStr((const unsigned char*)p + 24, ga, sizeof(ga));
    SafeStr((const void*)d[12], nm, sizeof(nm));
    Logf("[proxy]   %s: dwSize=%lu dwFlags=0x%08lX maxPlayers=%lu curPlayers=%lu name=\"%s\"\r\n",
         label, (unsigned long)d[0], (unsigned long)d[1],
         (unsigned long)d[10], (unsigned long)d[11], nm);
    Logf("[proxy]     guidInstance=%s\r\n", gi);
    Logf("[proxy]     guidApplication=%s\r\n", ga);
    Logf("[proxy]     reserved1=%08lX reserved2=%08lX user1=%08lX user2=%08lX user3=%08lX user4=%08lX\r\n",
         (unsigned long)d[14], (unsigned long)d[15], (unsigned long)d[16],
         (unsigned long)d[17], (unsigned long)d[18], (unsigned long)d[19]);
}

#define WRAP_OR_FAIL(slotIdx)                                  \
    Wrap* w = FindWrap(self);                                  \
    if (!w) return E_FAIL;                                     \
    void* ra = _ReturnAddress();                               \
    (void)ra; (void)slotIdx;

// slot 13
typedef HRESULT(__stdcall* EnumSessionsFn)(void*, void*, DWORD, void*, void*, DWORD);
static HRESULT __stdcall EnumSessions_Hook(void* self, void* desc, DWORD timeout, void* cb, void* ctx, DWORD flags) {
    WRAP_OR_FAIL(13)
    Logf("[proxy] #%d EnumSessions(timeout=%lu cb=%p ctx=%p flags=0x%08lX) from %p\r\n",
         w->id, (unsigned long)timeout, cb, ctx, (unsigned long)flags, ra);
    LogSessionDesc(desc, "enum filter");
    HRESULT hr = ((EnumSessionsFn)w->realVt[13])(w->real, desc, timeout, cb, ctx, flags);
    Logf("[proxy] #%d EnumSessions -> hr=0x%08lX\r\n", w->id, (unsigned long)hr);
    return hr;
}

// slot 24
typedef HRESULT(__stdcall* OpenFn)(void*, void*, DWORD);
static HRESULT __stdcall Open_Hook(void* self, void* desc, DWORD flags) {
    WRAP_OR_FAIL(24)
    Logf("[proxy] #%d Open(flags=0x%08lX%s%s) from %p\r\n", w->id, (unsigned long)flags,
         (flags & 1) ? " DPOPEN_JOIN" : "", (flags & 2) ? " DPOPEN_CREATE" : "", ra);
    LogSessionDesc(desc, "session");
    HRESULT hr = ((OpenFn)w->realVt[24])(w->real, desc, flags);
    Logf("[proxy] #%d Open -> hr=0x%08lX\r\n", w->id, (unsigned long)hr);
    return hr;
}

// slot 6: CreatePlayer(LPDPID, LPDPNAME, HANDLE, LPVOID, DWORD, DWORD)
typedef HRESULT(__stdcall* CreatePlayerFn)(void*, DWORD*, void*, HANDLE, void*, DWORD, DWORD);
static HRESULT __stdcall CreatePlayer_Hook(void* self, DWORD* pid, void* name, HANDLE ev,
                                           void* data, DWORD dataSize, DWORD flags) {
    WRAP_OR_FAIL(6)
    char sn[128] = "(none)", ln[128] = "(none)";
    if (name) {
        DWORD dn[4];
        __try { memcpy(dn, name, sizeof(dn)); SafeStr((void*)dn[2], sn, sizeof(sn)); SafeStr((void*)dn[3], ln, sizeof(ln)); }
        __except (EXCEPTION_EXECUTE_HANDLER) {}
    }
    HRESULT hr = ((CreatePlayerFn)w->realVt[6])(w->real, pid, name, ev, data, dataSize, flags);
    Logf("[proxy] #%d CreatePlayer(short=\"%s\" long=\"%s\" event=%p dataSize=%lu flags=0x%08lX)"
         " -> hr=0x%08lX dpid=%08lX  from %p\r\n",
         w->id, sn, ln, ev, (unsigned long)dataSize, (unsigned long)flags,
         (unsigned long)hr, pid ? (unsigned long)*pid : 0, ra);
    return hr;
}

// slot 26: Send(DPID from, DPID to, DWORD flags, LPVOID data, DWORD size).
// The payload is the whole point of the capture, so it is dumped in full.
typedef HRESULT(__stdcall* SendFn)(void*, DWORD, DWORD, DWORD, void*, DWORD);
static HRESULT __stdcall Send_Hook(void* self, DWORD from, DWORD to, DWORD flags, void* data, DWORD size) {
    WRAP_OR_FAIL(26)
    Logf("[proxy] #%d Send(from=%08lX to=%08lX%s flags=0x%08lX%s size=%lu) from %p\r\n",
         w->id, (unsigned long)from, (unsigned long)to,
         to == 0 ? " DPID_ALLPLAYERS" : "", (unsigned long)flags,
         (flags & 1) ? " DPSEND_GUARANTEED" : " (unguaranteed)", (unsigned long)size, ra);
    HexDump(data, size, "[proxy]     tx ");
    HRESULT hr = ((SendFn)w->realVt[26])(w->real, from, to, flags, data, size);
    if (FAILED(hr)) Logf("[proxy] #%d Send -> hr=0x%08lX\r\n", w->id, (unsigned long)hr);
    return hr;
}

// slot 25: Receive(LPDPID from, LPDPID to, DWORD flags, LPVOID data, LPDWORD size).
// DPERR_NOMESSAGES (0x887700BE) is the normal empty-queue answer and would
// otherwise bury the log, so empty polls are counted rather than printed.
static long g_emptyReceives = 0;
typedef HRESULT(__stdcall* ReceiveFn)(void*, DWORD*, DWORD*, DWORD, void*, DWORD*);
static HRESULT __stdcall Receive_Hook(void* self, DWORD* from, DWORD* to, DWORD flags, void* data, DWORD* size) {
    WRAP_OR_FAIL(25)
    DWORD inSize = size ? *size : 0;
    HRESULT hr = ((ReceiveFn)w->realVt[25])(w->real, from, to, flags, data, size);
    if (hr == (HRESULT)0x887700BE) { g_emptyReceives++; return hr; }
    if (g_emptyReceives) {
        Logf("[proxy]   (%ld empty Receive polls since the last message)\r\n", g_emptyReceives);
        g_emptyReceives = 0;
    }
    DWORD f = from ? *from : 0, t = to ? *to : 0, n = size ? *size : 0;
    Logf("[proxy] #%d Receive(flags=0x%08lX bufIn=%lu) -> hr=0x%08lX from=%08lX%s to=%08lX size=%lu"
         "  from %p\r\n",
         w->id, (unsigned long)flags, (unsigned long)inSize, (unsigned long)hr,
         (unsigned long)f, f == 0 ? " DPID_SYSMSG" : "", (unsigned long)t, (unsigned long)n, ra);
    if (SUCCEEDED(hr)) {
        if (f == 0 && n >= 4) {
            DWORD type = SafeDword(data, 0xFFFFFFFF);
            const char* nm = "?";
            switch (type) {
                case 0x0003: nm = "DPSYS_CREATEPLAYERORGROUP"; break;
                case 0x0005: nm = "DPSYS_DESTROYPLAYERORGROUP"; break;
                case 0x0007: nm = "DPSYS_ADDPLAYERTOGROUP"; break;
                case 0x0021: nm = "DPSYS_DELETEPLAYERFROMGROUP"; break;
                case 0x0031: nm = "DPSYS_SESSIONLOST"; break;
                case 0x0101: nm = "DPSYS_HOST"; break;
                case 0x0102: nm = "DPSYS_SETPLAYERORGROUPDATA"; break;
                case 0x0103: nm = "DPSYS_SETPLAYERORGROUPNAME"; break;
                case 0x0104: nm = "DPSYS_SETSESSIONDESC"; break;
                case 0x0108: nm = "DPSYS_STARTSESSION"; break;
                case 0x0109: nm = "DPSYS_CHAT"; break;
                default: break;
            }
            Logf("[proxy]     system message 0x%04lX %s\r\n", (unsigned long)type, nm);
        }
        HexDump(data, n, "[proxy]     rx ");
    }
    return hr;
}

// slot 14: GetCaps(LPDPCAPS, DWORD). Logged after the call so the answer is
// visible - dwFlags tells us whether guaranteed delivery is real on this
// service provider or silently ignored.
typedef HRESULT(__stdcall* GetCaps2Fn)(void*, DWORD*, DWORD);
static HRESULT __stdcall GetCaps_Hook(void* self, DWORD* caps, DWORD flags) {
    WRAP_OR_FAIL(14)
    HRESULT hr = ((GetCaps2Fn)w->realVt[14])(w->real, caps, flags);
    if (SUCCEEDED(hr) && caps) {
        Logf("[proxy] #%d GetCaps(flags=0x%08lX%s) -> dwFlags=0x%08lX maxBufferSize=%lu "
             "maxPlayers=%lu latency=%lu headerLength=%lu timeout=%lu  from %p\r\n",
             w->id, (unsigned long)flags, (flags & 1) ? " DPGETCAPS_GUARANTEED" : "",
             (unsigned long)caps[1], (unsigned long)caps[2], (unsigned long)caps[4],
             (unsigned long)caps[6], (unsigned long)caps[8], (unsigned long)caps[9], ra);
    } else {
        Logf("[proxy] #%d GetCaps(flags=0x%08lX) -> hr=0x%08lX  from %p\r\n",
             w->id, (unsigned long)flags, (unsigned long)hr, ra);
    }
    return hr;
}

static void InstallTypedHooks(Wrap* w) {
    if (!strstr(w->iface, "DirectPlay2") && !strstr(w->iface, "DirectPlay3") &&
        !strstr(w->iface, "DirectPlay4")) return;
    w->vt[6]  = (void*)&CreatePlayer_Hook;
    w->vt[13] = (void*)&EnumSessions_Hook;
    w->vt[14] = (void*)&GetCaps_Hook;
    w->vt[24] = (void*)&Open_Hook;
    w->vt[25] = (void*)&Receive_Hook;
    w->vt[26] = (void*)&Send_Hook;
    Logf("[proxy]   typed hooks installed on #%d (slots 6,13,14,24,25,26)\r\n", w->id);
}

// ------------------------------------------------- DirectPlayEnumerateA hook
// The callback signature is documented as
//   BOOL PASCAL cb(LPGUID spGuid, LPSTR spName, DWORD major, DWORD minor, LPVOID ctx)
// but rather than trust that, the wrapper is a naked tail-jump: it logs the raw
// stack and jumps into the game's callback with the frame untouched, so callee
// cleanup stays correct whatever the real arity turns out to be. The cost is
// that the callback's BOOL return value is not observed.
extern "C" void __cdecl EnumCbLog(DWORD* argp) {
    char g[160], name[128];
    GuidStr((const void*)argp[0], g, sizeof(g));
    SafeStr((const void*)argp[1], name, sizeof(name));
    Logf("[proxy]   provider: %-28s ver %lu.%lu  guid %s  ctx %08lX\r\n",
         name, (unsigned long)argp[2], (unsigned long)argp[3], g,
         (unsigned long)argp[4]);
}

__declspec(naked) static void EnumCbThunk() {
    __asm lea  eax, [esp+4]
    __asm push eax
    __asm call EnumCbLog
    __asm add  esp, 4
    __asm mov  eax, g_gameEnumCb
    __asm jmp  eax
}

// ---------------------------------------------------------------- real dplayx
typedef HRESULT (WINAPI *DirectPlayCreate_t)(GUID*, void**, IUnknown*);
typedef HRESULT (WINAPI *DirectPlayEnumerateA_t)(void*, void*);
static DirectPlayCreate_t     g_realCreate = 0;
static DirectPlayEnumerateA_t g_realEnum   = 0;

// Never called from DllMain: LoadLibrary under the loader lock is how proxies
// deadlock. Both entry points are called long after load.
static bool EnsureReal() {
    if (g_realCreate && g_realEnum) return true;
    char path[MAX_PATH] = {};
    UINT len = GetSystemDirectoryA(path, MAX_PATH);   // 32-bit process -> SysWOW64
    if (len == 0 || len > MAX_PATH - 16) {
        Logf("[proxy] GetSystemDirectoryA failed (%lu)\r\n", GetLastError());
        return false;
    }
    strcat_s(path, MAX_PATH, "\\dplayx.dll");
    g_real = LoadLibraryA(path);
    if (!g_real) {
        Logf("[proxy] LoadLibrary(%s) failed (%lu) - is the DirectPlay optional "
             "feature enabled?\r\n", path, GetLastError());
        return false;
    }
    // By ordinal, exactly as the game does, so we cannot pick a different export.
    g_realCreate = (DirectPlayCreate_t)GetProcAddress(g_real, MAKEINTRESOURCEA(1));
    g_realEnum   = (DirectPlayEnumerateA_t)GetProcAddress(g_real, MAKEINTRESOURCEA(2));
    Logf("[proxy] real dplayx at %p from %s (ord1=%p ord2=%p)\r\n",
         g_real, path, g_realCreate, g_realEnum);
    return g_realCreate != 0 && g_realEnum != 0;
}

extern "C" HRESULT WINAPI DirectPlayEnumerateA(void* lpEnumCallback, void* lpContext) {
    void* ra = _ReturnAddress();
    if (!EnsureReal()) return E_FAIL;
    Logf("[proxy] DirectPlayEnumerateA(cb=%p, ctx=%p) called from %p\r\n",
         lpEnumCallback, lpContext, ra);
    g_gameEnumCb = lpEnumCallback;
    HRESULT hr = g_realEnum(lpEnumCallback ? (void*)&EnumCbThunk : 0, lpContext);
    Logf("[proxy] DirectPlayEnumerateA -> hr=0x%08lX\r\n", (unsigned long)hr);
    return hr;
}

extern "C" HRESULT WINAPI DirectPlayCreate(GUID* lpGUID, void** lplpDP, IUnknown* pUnkOuter) {
    void* ra = _ReturnAddress();
    if (!EnsureReal()) return E_FAIL;
    char g[160];
    GuidStr(lpGUID, g, sizeof(g));
    Logf("[proxy] DirectPlayCreate(guid=%s, outer=%p) called from %p\r\n",
         g, pUnkOuter, ra);
    HRESULT hr = g_realCreate(lpGUID, lplpDP, pUnkOuter);
    void* iface = (lplpDP && SUCCEEDED(hr)) ? *lplpDP : 0;
    Logf("[proxy] DirectPlayCreate -> hr=0x%08lX iface=%p\r\n", (unsigned long)hr, iface);
    if (iface) *lplpDP = WrapObject(iface, "IDirectPlay1(DirectPlayCreate)");
    return hr;
}

// ----------------------------------------------- the other nine exports
// The proxy has to reproduce the whole export table, not just the two entry
// points the game uses. DirectPlay's service providers (dpwsockx.dll,
// dpmodemx.dll) import the data symbol gdwDPlaySPRefCount from "DPLAYX.dll" by
// name; with our proxy loaded, that name resolves against us, and if we do not
// export it the provider fails to bind and DirectPlayCreate returns
// DPERR_UNAVAILABLE for every provider. Measured - see dplayx.def.
//
// This copy of the counter is ours, not the real dplayx's, so the real dplayx
// sees a refcount that never rises. Nothing in a game process unloads
// DirectPlay, so that is inert here, but it is a real difference from the
// unproxied game and is written up in docs/netcode.md.
extern "C" __declspec(dllexport) DWORD gdwDPlaySPRefCount = 0;

// Ordinals 3-10 are pass-throughs. Naked tail-jumps again, so their (differing)
// argument counts never have to be known.
extern "C" void* __cdecl ResolveOrd(int ord) {
    if (!EnsureReal()) return 0;
    void* p = (void*)GetProcAddress(g_real, MAKEINTRESOURCEA(ord));
    Logf("[proxy] pass-through ordinal %d -> %p\r\n", ord, p);
    return p;
}

#define FORWARD(fn, ord)                              \
    extern "C" __declspec(naked) void fn() {          \
        __asm push ord                                \
        __asm call ResolveOrd                         \
        __asm add  esp, 4                             \
        __asm jmp  eax                                \
    }
FORWARD(Fwd_DirectPlayEnumerateW,3)
FORWARD(Fwd_DirectPlayLobbyCreateA,4)
FORWARD(Fwd_DirectPlayLobbyCreateW,5)
FORWARD(Fwd_DllCanUnloadNow,6)
FORWARD(Fwd_DllGetClassObject,7)
FORWARD(Fwd_DllRegisterServer,8)
FORWARD(Fwd_DirectPlayEnumerate,9)
FORWARD(Fwd_DllUnregisterServer,10)

// ------------------------------------------------------------------ driver
// The game never touches DirectPlay on its own: a whole startup logs nothing.
// Everything network lives behind the multiplayer menu, and docs/decisions.md
// D8 records that we cannot get there with synthetic input - a fullscreen
// DirectDraw game can be neither seen nor reliably clicked from outside.
//
// D8's answer is to call the original's functions rather than pantomime a
// player, and that is what this thread does. Set L2NET_CALL to the hex address
// of a no-argument game function and it is called after L2NET_DELAY ms from a
// thread of ours inside the game process:
//
//   L2NET_CALL=4b7585   Net_MultiplayerSetup() - the whole setup chain:
//                       dialog 129 (connection method) -> 108 (connect or
//                       create) -> 116 (session name) or 130 (select session).
//                       Those are ordinary Win32 dialogs, so once this thread
//                       has opened them tools/net/drive.ps1 can operate them
//                       by posted messages without needing the screen.
//
// Guarded three ways: the address must lie inside Lords2.exe's .text, it must
// begin with a recognisable prologue, and the call runs under __except so a
// bad guess is logged instead of silently killing the process.
static DWORD WINAPI DriverThread(LPVOID) {
    char buf[64] = {};
    if (!GetEnvironmentVariableA("L2NET_CALL", buf, sizeof(buf))) return 0;
    unsigned long target = strtoul(buf, 0, 16);

    char dbuf[32] = {};
    DWORD delay = 12000;
    if (GetEnvironmentVariableA("L2NET_DELAY", dbuf, sizeof(dbuf))) delay = strtoul(dbuf, 0, 10);

    // 0x401000..0x4CFB06 is Lords2.exe's .text (docs/symbols.md). Refuse
    // anything outside it rather than jumping into data.
    if (target < 0x00401000 || target >= 0x004CFB06) {
        Logf("[drive] L2NET_CALL=%08lX is outside Lords2.exe .text - refusing\r\n", target);
        return 0;
    }
    const unsigned char* p = (const unsigned char*)(ULONG_PTR)target;
    unsigned char pro[3] = { 0, 0, 0 };
    __try { pro[0] = p[0]; pro[1] = p[1]; pro[2] = p[2]; }
    __except (EXCEPTION_EXECUTE_HANDLER) {
        Logf("[drive] %08lX unreadable - refusing\r\n", target);
        return 0;
    }
    if (!(pro[0] == 0x55 && pro[1] == 0x8B && pro[2] == 0xEC)) {
        Logf("[drive] %08lX does not begin push ebp; mov ebp,esp (%02X %02X %02X)"
             " - refusing\r\n", target, pro[0], pro[1], pro[2]);
        return 0;
    }

    Logf("[drive] waiting %lu ms, then calling %08lX()\r\n", delay, target);
    Sleep(delay);
    Logf("[drive] calling %08lX() on thread %lu\r\n", target, GetCurrentThreadId());
    int rc = -1;
    __try {
        rc = ((int(__cdecl*)(void))(ULONG_PTR)target)();
    } __except (EXCEPTION_EXECUTE_HANDLER) {
        Logf("[drive] %08lX() raised exception 0x%08lX\r\n",
             target, (unsigned long)GetExceptionCode());
        return 0;
    }
    Logf("[drive] %08lX() returned %d\r\n", target, rc);
    return 0;
}

BOOL APIENTRY DllMain(HMODULE module, DWORD reason, LPVOID) {
    if (reason == DLL_PROCESS_ATTACH) {
        g_self = module;
        g_t0 = GetTickCount();
        InitializeCriticalSection(&g_cs);
        g_csReady = true;
        DisableThreadLibraryCalls(module);
        char exe[MAX_PATH] = {};
        GetModuleFileNameA(0, exe, MAX_PATH);
        Logf("[proxy] ==== attached to %s, image base %p, pid %lu ====\r\n",
             exe, GetModuleHandleA(0), GetCurrentProcessId());
        // A thread, not work in DllMain: everything the driver does needs the
        // game fully initialised, and DllMain runs under the loader lock.
        HANDLE th = CreateThread(0, 0, DriverThread, 0, 0, 0);
        if (th) CloseHandle(th);
    } else if (reason == DLL_PROCESS_DETACH) {
        if (g_csReady) {
            EnterCriticalSection(&g_cs);
            FlushRepeat_locked();
            LeaveCriticalSection(&g_cs);
        }
        Logf("[proxy] ==== detached ====\r\n");
    }
    return TRUE;
}

// ---------------------------------------------------------------- the thunks
// One naked thunk per vtable slot. Each: log, swap `this` for the real
// interface, tail-jump to the real method. No knowledge of arity required.
//
// At entry: [esp] = return address, [esp+4] = this, [esp+8] = first argument.
#define THUNK(n)                                    \
    __declspec(naked) static void Thunk_##n() {     \
        __asm lea  eax, [esp+8]                     \
        __asm push eax                              \
        __asm push n                                \
        __asm push dword ptr [esp+12]               \
        __asm call SpyDispatch                      \
        __asm add  esp, 12                          \
        __asm jmp  eax                              \
    }
THUNK(0)  THUNK(1)  THUNK(2)  THUNK(3)  THUNK(4)  THUNK(5)  THUNK(6)  THUNK(7)
THUNK(8)  THUNK(9)  THUNK(10) THUNK(11) THUNK(12) THUNK(13) THUNK(14) THUNK(15)
THUNK(16) THUNK(17) THUNK(18) THUNK(19) THUNK(20) THUNK(21) THUNK(22) THUNK(23)
THUNK(24) THUNK(25) THUNK(26) THUNK(27) THUNK(28) THUNK(29) THUNK(30) THUNK(31)
THUNK(32) THUNK(33) THUNK(34) THUNK(35) THUNK(36) THUNK(37) THUNK(38) THUNK(39)
THUNK(40) THUNK(41) THUNK(42) THUNK(43) THUNK(44) THUNK(45) THUNK(46) THUNK(47)
THUNK(48) THUNK(49) THUNK(50) THUNK(51) THUNK(52) THUNK(53) THUNK(54) THUNK(55)
THUNK(56) THUNK(57) THUNK(58) THUNK(59) THUNK(60) THUNK(61) THUNK(62) THUNK(63)
THUNK(64) THUNK(65) THUNK(66) THUNK(67) THUNK(68) THUNK(69) THUNK(70) THUNK(71)
THUNK(72) THUNK(73) THUNK(74) THUNK(75) THUNK(76) THUNK(77) THUNK(78) THUNK(79)
THUNK(80) THUNK(81) THUNK(82) THUNK(83) THUNK(84) THUNK(85) THUNK(86) THUNK(87)
THUNK(88) THUNK(89) THUNK(90) THUNK(91) THUNK(92) THUNK(93) THUNK(94) THUNK(95)

#define TREF(n) (void*)&Thunk_##n
extern "C" void* g_thunks[MAX_SLOTS] = {
    TREF(0),  TREF(1),  TREF(2),  TREF(3),  TREF(4),  TREF(5),  TREF(6),  TREF(7),
    TREF(8),  TREF(9),  TREF(10), TREF(11), TREF(12), TREF(13), TREF(14), TREF(15),
    TREF(16), TREF(17), TREF(18), TREF(19), TREF(20), TREF(21), TREF(22), TREF(23),
    TREF(24), TREF(25), TREF(26), TREF(27), TREF(28), TREF(29), TREF(30), TREF(31),
    TREF(32), TREF(33), TREF(34), TREF(35), TREF(36), TREF(37), TREF(38), TREF(39),
    TREF(40), TREF(41), TREF(42), TREF(43), TREF(44), TREF(45), TREF(46), TREF(47),
    TREF(48), TREF(49), TREF(50), TREF(51), TREF(52), TREF(53), TREF(54), TREF(55),
    TREF(56), TREF(57), TREF(58), TREF(59), TREF(60), TREF(61), TREF(62), TREF(63),
    TREF(64), TREF(65), TREF(66), TREF(67), TREF(68), TREF(69), TREF(70), TREF(71),
    TREF(72), TREF(73), TREF(74), TREF(75), TREF(76), TREF(77), TREF(78), TREF(79),
    TREF(80), TREF(81), TREF(82), TREF(83), TREF(84), TREF(85), TREF(86), TREF(87),
    TREF(88), TREF(89), TREF(90), TREF(91), TREF(92), TREF(93), TREF(94), TREF(95),
};

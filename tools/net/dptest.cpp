// A standalone exerciser for DirectPlay, and for our proxy dplayx.dll.
//
//   dptest.exe C:\Windows\SysWOW64\dplayx.dll     -> the real thing
//   dptest.exe ...\native\dplay-proxy\dplayx.dll  -> through the proxy
//
// Two jobs:
//
//  1. Establish ground truth that does not depend on the game running at all:
//     which service providers DirectPlayEnumerateA reports on this machine,
//     which IDirectPlayN interfaces the real dplayx will hand out, and - by
//     counting how many consecutive vtable entries point inside dplayx's image -
//     how many methods each of those interfaces actually has. That last number
//     is measured, and it is what lets a slot index in a capture be mapped to a
//     named method with any confidence.
//
//  2. Shake out the proxy's naked thunks somewhere that is not a fullscreen
//     game. Every thunk swaps `this` and tail-jumps; if that is wrong it
//     corrupts the stack, and finding that out here costs seconds rather than a
//     lost game session.
//
// Deliberately not linked against dplayx: everything is by LoadLibrary and
// ordinal, exactly as Lords2.exe does it.

#include <windows.h>
#include <cstdio>

typedef HRESULT (WINAPI *DirectPlayCreate_t)(GUID*, void**, IUnknown*);
typedef HRESULT (WINAPI *DirectPlayEnumerateA_t)(void*, void*);
typedef HRESULT (__stdcall *QIFn)(void*, const GUID*, void**);
typedef ULONG   (__stdcall *RefFn)(void*);
typedef HRESULT (__stdcall *GetCapsFn)(void*, void* caps, DWORD flags);

static HMODULE g_dp;

static void PG(const char* label, const GUID& g) {
    printf("%s{%08lX-%04X-%04X-%02X%02X-%02X%02X%02X%02X%02X%02X}",
           label, (unsigned long)g.Data1, g.Data2, g.Data3,
           g.Data4[0], g.Data4[1], g.Data4[2], g.Data4[3],
           g.Data4[4], g.Data4[5], g.Data4[6], g.Data4[7]);
}

// ------------------------------------------------------------ SP enumeration
static GUID  g_sp[16];
static char  g_spName[16][128];
static int   g_spCount = 0;

static BOOL PASCAL EnumCb(GUID* guid, char* name, DWORD major, DWORD minor, void* ctx) {
    (void)ctx;
    printf("  provider %d: %-28s ver %lu.%lu  ", g_spCount, name ? name : "(null)",
           (unsigned long)major, (unsigned long)minor);
    if (guid) PG("", *guid);
    printf("\n");
    if (g_spCount < 16) {
        if (guid) g_sp[g_spCount] = *guid;
        lstrcpynA(g_spName[g_spCount], name ? name : "", 128);
        g_spCount++;
    }
    return TRUE;   // keep enumerating
}

// -------------------------------------------------- vtable size measurement
// Counts consecutive vtable entries that point into the module that owns the
// interface. The first entry that does not is past the end of the vtable.
// This is a measurement, not a header lookup - see docs/netcode.md.
static int VtableLength(void* iface, int cap) {
    void** vt = *(void***)iface;
    MEMORY_BASIC_INFORMATION mbi;
    if (!VirtualQuery(vt[0], &mbi, sizeof(mbi))) return -1;
    void* owner = mbi.AllocationBase;
    int n = 0;
    for (; n < cap; n++) {
        if (IsBadReadPtr(&vt[n], 4)) break;
        if (!vt[n]) break;
        if (!VirtualQuery(vt[n], &mbi, sizeof(mbi))) break;
        if (mbi.AllocationBase != owner) break;
        if (!(mbi.Protect & (PAGE_EXECUTE | PAGE_EXECUTE_READ | PAGE_EXECUTE_READWRITE |
                             PAGE_EXECUTE_WRITECOPY))) break;
    }
    return n;
}

struct Named { const char* name; GUID iid; };
static const Named kIIDs[] = {
    { "IID_IDirectPlay",   { 0x5454E9A0,0xDB65,0x11CE,{0x92,0x1C,0x00,0xAA,0x00,0x6C,0x49,0x72} } },
    { "IID_IDirectPlay2",  { 0x2B74F7C0,0x9154,0x11CF,{0xA9,0xCD,0x00,0xAA,0x00,0x68,0x86,0xE3} } },
    { "IID_IDirectPlay2A", { 0x9D460580,0xA822,0x11CF,{0x96,0x0C,0x00,0x80,0xC7,0x53,0x4E,0x82} } },
    { "IID_IDirectPlay3",  { 0x133EFE40,0x32DC,0x11D0,{0x9C,0xFB,0x00,0xA0,0xC9,0x0A,0x43,0xCB} } },
    { "IID_IDirectPlay3A", { 0x133EFE41,0x32DC,0x11D0,{0x9C,0xFB,0x00,0xA0,0xC9,0x0A,0x43,0xCB} } },
    { "IID_IDirectPlay4",  { 0x0AB1C530,0x4745,0x11D1,{0xA7,0xA1,0x00,0x00,0xF8,0x03,0xAB,0xFC} } },
    { "IID_IDirectPlay4A", { 0x0AB1C531,0x4745,0x11D1,{0xA7,0xA1,0x00,0x00,0xF8,0x03,0xAB,0xFC} } },
};

int main(int argc, char** argv) {
    const char* path = argc > 1 ? argv[1] : "C:\\Windows\\SysWOW64\\dplayx.dll";
    printf("loading %s\n", path);
    g_dp = LoadLibraryA(path);
    if (!g_dp) { printf("LoadLibrary failed: %lu\n", GetLastError()); return 1; }
    char actual[MAX_PATH] = {};
    GetModuleFileNameA(g_dp, actual, MAX_PATH);
    printf("  handle %p, actually loaded: %s\n", g_dp, actual);

    DirectPlayCreate_t     create = (DirectPlayCreate_t)GetProcAddress(g_dp, MAKEINTRESOURCEA(1));
    DirectPlayEnumerateA_t enumA  = (DirectPlayEnumerateA_t)GetProcAddress(g_dp, MAKEINTRESOURCEA(2));
    printf("  ordinal 1 = %p, ordinal 2 = %p\n", create, enumA);
    if (!create || !enumA) return 1;

    printf("\n== DirectPlayEnumerateA ==\n");
    HRESULT hr = enumA((void*)&EnumCb, (void*)0x12345678);
    printf("  -> hr=0x%08lX, %d provider(s)\n", (unsigned long)hr, g_spCount);

    for (int i = 0; i < g_spCount; i++) {
        printf("\n== DirectPlayCreate on provider %d (%s) ==\n", i, g_spName[i]);
        void* dp = 0;
        hr = create(&g_sp[i], &dp, 0);
        printf("  -> hr=0x%08lX iface=%p\n", (unsigned long)hr, dp);
        if (FAILED(hr) || !dp) continue;

        void** vt = *(void***)dp;
        printf("  vtable=%p length=%d\n", vt, VtableLength(dp, 96));

        for (int k = 0; k < (int)(sizeof(kIIDs) / sizeof(kIIDs[0])); k++) {
            void* q = 0;
            HRESULT qhr = ((QIFn)vt[0])(dp, &kIIDs[k].iid, &q);
            printf("  QI %-18s -> hr=0x%08lX iface=%p", kIIDs[k].name, (unsigned long)qhr, q);
            if (SUCCEEDED(qhr) && q) {
                printf(" vtable=%p length=%d", *(void***)q, VtableLength(q, 96));
                ((RefFn)(*(void***)q)[2])(q);
            }
            printf("\n");
        }

        // Slot 14 on IDirectPlay2 is GetCaps(DPCAPS*, flags). Exercising a
        // non-IUnknown slot is the point: it is the first call that goes through
        // a generic thunk when running under the proxy.
        void* dp2 = 0;
        if (SUCCEEDED(((QIFn)vt[0])(dp, &kIIDs[2].iid, &dp2)) && dp2) {
            void** vt2 = *(void***)dp2;
            DWORD caps[64] = {};
            caps[0] = 40;   // sizeof(DPCAPS): ten DWORDs, verified against dplay.h
            HRESULT chr = ((GetCapsFn)vt2[14])(dp2, caps, 0);
            // DPCAPS field order: dwSize, dwFlags, dwMaxBufferSize, dwMaxQueueSize,
            // dwMaxPlayers, dwHundredBaud, dwLatency, dwMaxLocalPlayers,
            // dwHeaderLength, dwTimeout.
            printf("  IDirectPlay2A::slot14(GetCaps) -> hr=0x%08lX\n"
                   "    dwSize=%lu dwFlags=0x%08lX dwMaxBufferSize=%lu dwMaxQueueSize=%lu\n"
                   "    dwMaxPlayers=%lu dwHundredBaud=%lu dwLatency=%lu dwMaxLocalPlayers=%lu\n"
                   "    dwHeaderLength=%lu dwTimeout=%lu\n",
                   (unsigned long)chr,
                   (unsigned long)caps[0], (unsigned long)caps[1], (unsigned long)caps[2],
                   (unsigned long)caps[3], (unsigned long)caps[4], (unsigned long)caps[5],
                   (unsigned long)caps[6], (unsigned long)caps[7], (unsigned long)caps[8],
                   (unsigned long)caps[9]);
            printf("  DPCAPS raw:");
            for (int b = 0; b < 10; b++) printf(" %08lX", (unsigned long)caps[b]);
            printf("\n");
            ((RefFn)vt2[2])(dp2);
        }

        ULONG rc = ((RefFn)vt[2])(dp);
        printf("  Release -> %lu\n", (unsigned long)rc);
    }

    printf("\ndone\n");
    return 0;
}

#include "trace.h"
#include <cstdio>
#include <cstdlib>
#include <cstring>

TraceShared* g_shared = nullptr;
HMODULE g_self = nullptr;

namespace {
char g_logPath[MAX_PATH];
TraceShared g_fallback;
CRITICAL_SECTION g_logLock;
bool g_lockReady = false;

struct Seen { char key[64]; };
Seen g_seen[256];
int g_seenCount = 0;
}

void Log(const char* fmt, ...) {
    if (!g_logPath[0]) return;
    char line[1024];
    int n = _snprintf_s(line, sizeof(line), _TRUNCATE, "%10u ", g_shared ? g_shared->virtualMs : 0);
    va_list args;
    va_start(args, fmt);
    int m = _vsnprintf_s(line + n, sizeof(line) - n - 2, _TRUNCATE, fmt, args);
    va_end(args);
    if (m < 0) m = static_cast<int>(strlen(line + n));
    n += m;
    line[n++] = '\n';
    if (g_lockReady) EnterCriticalSection(&g_logLock);
    HANDLE h = CreateFileA(g_logPath, FILE_APPEND_DATA, FILE_SHARE_READ | FILE_SHARE_WRITE,
                           nullptr, OPEN_ALWAYS, FILE_ATTRIBUTE_NORMAL, nullptr);
    if (h != INVALID_HANDLE_VALUE) {
        DWORD w = 0;
        WriteFile(h, line, n, &w, nullptr);
        CloseHandle(h);
    }
    if (g_lockReady) LeaveCriticalSection(&g_logLock);
}

void LogGap(const char* what) {
    if (g_lockReady) EnterCriticalSection(&g_logLock);
    bool seen = false;
    for (int i = 0; i < g_seenCount; i++)
        if (strcmp(g_seen[i].key, what) == 0) { seen = true; break; }
    if (!seen && g_seenCount < 256) strncpy_s(g_seen[g_seenCount++].key, what, _TRUNCATE);
    if (g_lockReady) LeaveCriticalSection(&g_logLock);
    if (seen) return;
    if (g_shared) g_shared->gapsLogged++;
    Log("GAP %s", what);
}

int EnvInt(const char* name, int fallback) {
    char buf[32];
    if (!GetEnvironmentVariableA(name, buf, sizeof(buf))) return fallback;
    return atoi(buf);
}

bool EnvStr(const char* name, char* out, size_t n) {
    DWORD got = GetEnvironmentVariableA(name, out, static_cast<DWORD>(n));
    return got > 0 && got < n;
}

// Log beside this DLL: the game changes its working directory hunting for the CD.
void Shared_Open(const char* logName) {
    InitializeCriticalSection(&g_logLock);
    g_lockReady = true;
    char dir[MAX_PATH] = {};
    GetModuleFileNameA(g_self, dir, MAX_PATH);
    char* slash = strrchr(dir, '\\');
    if (slash) slash[1] = '\0';
    _snprintf_s(g_logPath, sizeof(g_logPath), _TRUNCATE, "%s%s", dir, logName);
    DeleteFileA(g_logPath);

    char name[64];
    _snprintf_s(name, sizeof(name), _TRUNCATE, "Local\\l2trace-%u", GetCurrentProcessId());
    HANDLE map = CreateFileMappingA(INVALID_HANDLE_VALUE, nullptr, PAGE_READWRITE, 0,
                                    sizeof(TraceShared), name);
    void* view = map ? MapViewOfFile(map, FILE_MAP_ALL_ACCESS, 0, 0, sizeof(TraceShared)) : nullptr;
    g_shared = view ? static_cast<TraceShared*>(view) : &g_fallback;
    g_shared->magic = kTraceMagic;
    g_shared->version = 1;
    if (!g_shared->gate) g_shared->gate = EnvInt("L2TRACE_GATE", 0);
    Log("l2trace up, pid %u, shared %s", GetCurrentProcessId(), view ? name : "(local only)");
}

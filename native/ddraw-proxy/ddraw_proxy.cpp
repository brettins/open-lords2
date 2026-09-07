// A proxy ddraw.dll for Lords of the Realm II.
//
// The game imports exactly one function from DirectDraw - DirectDrawCreate -
// so a proxy only has to forward that one call. Dropping this beside the
// executable gets our code running *inside* the game process, which is how we
// read and eventually call into the original engine.
//
// Why this rather than driving the UI: synthetic input needs window focus, and
// a fullscreen DirectDraw game captures as black. Injection needs neither.
// See docs/decisions.md D8.
//
// ddraw is not a KnownDLL, so a copy sitting next to the executable wins over
// the system one.

#include <windows.h>
#include <cstdio>

namespace {

HMODULE g_realDDraw = nullptr;
HMODULE g_self = nullptr;

using DirectDrawCreate_t = HRESULT(WINAPI*)(GUID*, void**, IUnknown*);
DirectDrawCreate_t g_realDirectDrawCreate = nullptr;

// Log beside our own DLL rather than the working directory, which the game
// changes while hunting for its CD.
void LogPath(char* out, size_t n) {
    char dir[MAX_PATH] = {};
    GetModuleFileNameA(g_self, dir, MAX_PATH);
    char* slash = strrchr(dir, '\\');
    if (slash) *(slash + 1) = '\0';
    _snprintf_s(out, n, _TRUNCATE, "%sl2proxy.log", dir);
}

void Log(const char* fmt, ...) {
    char path[MAX_PATH];
    LogPath(path, sizeof(path));

    // Opened per line and closed again: if the game crashes we still want
    // everything written up to that point.
    HANDLE h = CreateFileA(path, FILE_APPEND_DATA, FILE_SHARE_READ | FILE_SHARE_WRITE,
                           nullptr, OPEN_ALWAYS, FILE_ATTRIBUTE_NORMAL, nullptr);
    if (h == INVALID_HANDLE_VALUE) return;

    char line[1024];
    va_list args;
    va_start(args, fmt);
    int n = _vsnprintf_s(line, sizeof(line), _TRUNCATE, fmt, args);
    va_end(args);
    if (n > 0) {
        DWORD written = 0;
        WriteFile(h, line, static_cast<DWORD>(n), &written, nullptr);
    }
    CloseHandle(h);
}

// Deliberately NOT called from DllMain: LoadLibrary under the loader lock is
// how proxies deadlock. The game calls DirectDrawCreate long after load.
bool EnsureRealDDraw() {
    if (g_realDirectDrawCreate) return true;

    char path[MAX_PATH] = {};
    UINT len = GetSystemDirectoryA(path, MAX_PATH);
    if (len == 0 || len > MAX_PATH - 12) {
        Log("[proxy] GetSystemDirectoryA failed (%lu)\r\n", GetLastError());
        return false;
    }
    strcat_s(path, MAX_PATH, "\\ddraw.dll");

    g_realDDraw = LoadLibraryA(path);
    if (!g_realDDraw) {
        Log("[proxy] LoadLibrary(%s) failed (%lu)\r\n", path, GetLastError());
        return false;
    }
    g_realDirectDrawCreate = reinterpret_cast<DirectDrawCreate_t>(
        GetProcAddress(g_realDDraw, "DirectDrawCreate"));
    if (!g_realDirectDrawCreate) {
        Log("[proxy] GetProcAddress(DirectDrawCreate) failed (%lu)\r\n", GetLastError());
        return false;
    }
    Log("[proxy] loaded real ddraw from %s\r\n", path);
    return true;
}

}  // namespace

extern "C" HRESULT WINAPI DirectDrawCreate(GUID* lpGUID, void** lplpDD, IUnknown* pUnkOuter) {
    if (!EnsureRealDDraw()) return E_FAIL;

    HRESULT hr = g_realDirectDrawCreate(lpGUID, lplpDD, pUnkOuter);
    Log("[proxy] DirectDrawCreate -> hr=0x%08lX iface=%p\r\n",
        static_cast<unsigned long>(hr), lplpDD ? *lplpDD : nullptr);
    return hr;
}

BOOL APIENTRY DllMain(HMODULE module, DWORD reason, LPVOID) {
    if (reason == DLL_PROCESS_ATTACH) {
        g_self = module;
        DisableThreadLibraryCalls(module);

        char exe[MAX_PATH] = {};
        GetModuleFileNameA(nullptr, exe, MAX_PATH);
        // The image base is the anchor for every hardcoded address we have.
        // Lords2.exe has no ASLR, so this should always report 0x00400000.
        Log("[proxy] attached to %s, image base %p, pid %lu\r\n",
            exe, GetModuleHandleA(nullptr), GetCurrentProcessId());
    } else if (reason == DLL_PROCESS_DETACH) {
        Log("[proxy] detached\r\n");
    }
    return TRUE;
}

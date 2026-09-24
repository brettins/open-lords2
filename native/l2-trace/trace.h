// Headless Lords2.exe: memory-backed DirectDraw, virtual clock, scripted cursor.
// build.ps1 lists the switches; tools/oracle/trace.ps1 is the controller.
#pragma once
#include <windows.h>
#include <cstdint>

// Shared with the controller as "Local\l2trace-<pid>"; layout is the controller's C# struct.
struct TraceShared {
    uint32_t magic;          // 'L2TR'
    uint32_t version;        // 1
    int32_t  cursorX, cursorY;
    uint32_t frames;         // presents to the primary
    uint32_t frameHash;      // FNV-1a of the last presented 640x480 frame + palette
    uint32_t virtualMs;
    uint32_t hwnd;
    uint32_t timersDropped;
    uint32_t filmsSkipped;
    uint32_t gapsLogged;     // distinct unimplemented COM methods hit
    uint32_t patchesRefused;
    uint32_t timersRun;      // timeSetEvent callbacks run on the main thread
    volatile uint32_t gate;  // nonzero: block at the present that makes frames == gate until raised
};
constexpr uint32_t kTraceMagic = 0x5254324C;

extern TraceShared* g_shared;
extern HMODULE g_self;

void Log(const char* fmt, ...);
// Logs "GAP <what>" the first time only, so unimplemented surface is visible.
void LogGap(const char* what);
int  EnvInt(const char* name, int fallback);
bool EnvStr(const char* name, char* out, size_t n);
void Shared_Open(const char* logName);

// hooks.cpp
uint32_t Clock_Now();
void Timers_Run();
void Clock_Advance(uint32_t ms);
bool Hooks_Install();

// frame.cpp
void Frame_Presented(const uint8_t* pixels, int pitch, int w, int h, const PALETTEENTRY* pal);

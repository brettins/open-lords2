#include "trace.h"
#include <cstdio>
#include <cstring>

namespace {
uint32_t g_lastLogged = 0;

uint32_t Fnv(uint32_t h, const uint8_t* p, size_t n) {
    for (size_t i = 0; i < n; i++) { h ^= p[i]; h *= 16777619u; }
    return h;
}

// 8-bit BMP: the frame is palette-indexed, so this is lossless and needs no encoder.
void DumpBmp(const char* dir, uint32_t frame, uint32_t hash, const uint8_t* pixels, int pitch,
             int w, int h, const PALETTEENTRY* pal) {
    char path[MAX_PATH];
    _snprintf_s(path, sizeof(path), _TRUNCATE, "%s\\f%06u_%08x.bmp", dir, frame, hash);
    FILE* f = nullptr;
    if (fopen_s(&f, path, "wb") != 0 || !f) return;
    const uint32_t row = (w + 3) & ~3u, bits = 14 + 40 + 1024;
    uint8_t hdr[bits] = {};
    auto put32 = [&](int at, uint32_t v) { memcpy(hdr + at, &v, 4); };
    hdr[0] = 'B'; hdr[1] = 'M';
    put32(2, bits + row * h); put32(10, bits);
    put32(14, 40); put32(18, w); put32(22, h);
    hdr[26] = 1; hdr[28] = 8; put32(46, 256);
    for (int i = 0; i < 256; i++) {
        hdr[54 + i * 4] = pal[i].peBlue;
        hdr[55 + i * 4] = pal[i].peGreen;
        hdr[56 + i * 4] = pal[i].peRed;
    }
    fwrite(hdr, 1, bits, f);
    for (int y = h - 1; y >= 0; y--) fwrite(pixels + y * pitch, 1, row, f);
    fclose(f);
}
}

void LogFrame(uint32_t n, uint32_t hash, const uint8_t* pixels, int pitch, int w, int h,
              const PALETTEENTRY* pal);

void Frame_Presented(const uint8_t* pixels, int pitch, int w, int h, const PALETTEENTRY* pal) {
    uint32_t hash = 2166136261u;
    for (int y = 0; y < h; y++) hash = Fnv(hash, pixels + y * pitch, w);
    hash = Fnv(hash, reinterpret_cast<const uint8_t*>(pal), 256 * sizeof(PALETTEENTRY));
    uint32_t n = ++g_shared->frames;
    g_shared->frameHash = hash;
    static const int presentMs = EnvInt("L2TRACE_PRESENT_MS", 0);
    Clock_Advance(presentMs);
    Timers_Run();
    if (hash != g_lastLogged) {
        g_lastLogged = hash;
        LogFrame(n, hash, pixels, pitch, w, h, pal);
    }
    // Lockstep: input posted while the game waits here is read at the same point every run.
    for (int spins = 0; g_shared->gate && n >= g_shared->gate; spins++) {
        if (spins < 20000) SwitchToThread(); else Sleep(1);
    }
}

void LogFrame(uint32_t n, uint32_t hash, const uint8_t* pixels, int pitch, int w, int h,
              const PALETTEENTRY* pal) {
    // g_screenId 0x004EAC50, g_turnPhase 0x00569584 (docs/symbols.md).
    Log("FRAME %u hash %08x screen %u phase %d", n, hash,
        *reinterpret_cast<volatile uint8_t*>(0x004EAC50),
        *reinterpret_cast<volatile int32_t*>(0x00569584));
    char dir[MAX_PATH];
    if (EnvStr("L2TRACE_DUMP", dir, sizeof(dir))) DumpBmp(dir, n, hash, pixels, pitch, w, h, pal);
}

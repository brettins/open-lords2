#include "trace.h"
#include "events.h"
#include <cstdio>
#include <cstring>

EvRing* g_ring = nullptr;

namespace {
EvRecord* g_recs = nullptr;
uint32_t g_seq = 0;
EvRecord g_scratch;
bool g_open = false;

struct FrameState { uint32_t screen, phase, hash, draws, randA, lfsr; };
FrameState g_last = {0xFFFFFFFF, 0xFFFFFFFF, 0, 0, 0, 0};
}

// Counted by the detours between presents (detour.cpp).
uint32_t g_frameDraws = 0, g_frameRand = 0, g_frameLfsr = 0;

void Events_Open() {
    if (!EnvInt("L2TRACE_EVENTS", 0)) return;
    char name[64];
    _snprintf_s(name, sizeof(name), _TRUNCATE, "Local\\l2trace-ring-%u", GetCurrentProcessId());
    const DWORD size = sizeof(EvRing) + kRingCapacity * sizeof(EvRecord);
    HANDLE map = CreateFileMappingA(INVALID_HANDLE_VALUE, nullptr, PAGE_READWRITE, 0, size, name);
    void* view = map ? MapViewOfFile(map, FILE_MAP_ALL_ACCESS, 0, 0, size) : nullptr;
    if (!view) { Log("event ring not created"); return; }
    g_ring = static_cast<EvRing*>(view);
    g_recs = reinterpret_cast<EvRecord*>(g_ring + 1);
    g_ring->recSize = sizeof(EvRecord);
    g_ring->capacity = kRingCapacity;
    g_ring->magic = kRingMagic;
    g_open = true;
    Log("event ring %s, %u records", name, kRingCapacity);
}

// Blocks while the ring is full: the controller drains it, so nothing is ever dropped.
EvRecord* Ev_Begin(uint8_t kind, uint32_t fn, uint32_t caller) {
    if (!g_open) { memset(&g_scratch, 0, sizeof(g_scratch)); return &g_scratch; }
    for (int spins = 0; g_ring->head - g_ring->tail >= kRingCapacity; spins++) {
        if (spins < 20000) SwitchToThread(); else Sleep(1);
    }
    EvRecord* r = &g_recs[g_ring->head % kRingCapacity];
    memset(r, 0, sizeof(*r));
    r->seq = g_seq++;
    r->frame = g_shared->frames;
    r->kind = kind;
    r->fn = fn;
    r->caller = caller;
    return r;
}

void Ev_End() {
    if (!g_open) return;
    MemoryBarrier();
    g_ring->head = g_ring->head + 1;
}

// One record per present whose picture, screen, phase, draw count or RNG use differs from
// the last one written. g_screenId 0x004EAC50, g_turnPhase 0x00569584; RNG states
// g_randStateA 0x0058FE28, g_randStateB 0x0058FE08, DAT_00591564 (Rand_Advance 0x00404A46).
void Events_Present(uint32_t frame, uint32_t hash) {
    FrameState now = {*reinterpret_cast<volatile uint8_t*>(0x004EAC50),
                      static_cast<uint32_t>(*reinterpret_cast<volatile int32_t*>(0x00569584)),
                      hash, g_frameDraws, g_frameRand, g_frameLfsr};
    g_frameDraws = g_frameRand = g_frameLfsr = 0;
    if (!g_open || memcmp(&now, &g_last, sizeof(now)) == 0) return;
    g_last = now;
    EvRecord* r = Ev_Begin(Ev_frame, 0, 0);
    r->frame = frame;
    r->ret = static_cast<int32_t>(hash);
    const int32_t v[8] = {static_cast<int32_t>(now.screen), static_cast<int32_t>(now.phase),
                          static_cast<int32_t>(now.draws), static_cast<int32_t>(now.randA),
                          static_cast<int32_t>(now.lfsr), *reinterpret_cast<volatile int32_t*>(0x0058FE28),
                          *reinterpret_cast<volatile int32_t*>(0x0058FE08),
                          *reinterpret_cast<volatile int32_t*>(0x00591564)};
    memcpy(r->args, v, sizeof(v));
    memcpy(r->types, "iiiiiiii", 8);
    r->nargs = 8;
    Ev_End();
}

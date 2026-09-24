// The event ring: records the detours write and the controller drains to JSONL.
// Shared as "Local\l2trace-ring-<pid>" when L2TRACE_EVENTS=1; layout is trace.ps1's reader.
#pragma once
#include <cstdint>

enum EvKind : uint8_t { Ev_call = 1, Ev_ret, Ev_frame, Ev_phase, Ev_pin, Ev_refused };

#pragma pack(push, 1)
struct EvRecord {            // 128 bytes
    uint32_t seq, frame, fn, caller;
    int32_t  ret;
    uint16_t hook;
    uint8_t  kind, nargs;
    char     types[8];       // per arg: i int, x exe address, p pointer (value dropped), s string, k sheet offset
    int32_t  args[8];
    char     text[64];       // strings and sheet names, '|'-joined
};
struct EvRing {
    uint32_t magic, recSize, capacity;
    volatile uint32_t head, tail;
    volatile uint32_t keepDraws;  // nonzero: draw leaves are recorded, not only counted
    uint32_t reserved[2];
};
#pragma pack(pop)
static_assert(sizeof(EvRecord) == 128, "record layout is the controller's");
constexpr uint32_t kRingMagic = 0x474E5252;  // 'RRNG'
constexpr uint32_t kRingCapacity = 16384;

extern EvRing* g_ring;
EvRecord* Ev_Begin(uint8_t kind, uint32_t fn, uint32_t caller);
void Ev_End();
void Events_Open();
void Events_Present(uint32_t frame, uint32_t hash);
bool Detours_Install();
void Events_Film(const char* path, uint32_t caller);

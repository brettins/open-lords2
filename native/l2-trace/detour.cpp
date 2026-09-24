// Inline detours on Lords2.exe functions (hooktable.h). Each target's first instructions
// are checked against the table, copied to a trampoline and replaced by a jmp to a stub
// that records the call; a hooked return swaps the return address for RetThunk.
#include "trace.h"
#include "events.h"
#include "hooktable.h"
#include <cstring>

extern uint32_t g_frameDraws, g_frameRand, g_frameLfsr;

namespace {
constexpr int kCount = sizeof(kHooks) / sizeof(kHooks[0]);
uint8_t* g_tramp[kCount];
DWORD g_mainThread = 0;
struct Shadow { uint32_t ret, slot, hook, g0; };
Shadow g_shadow[1024];
int g_depth = 0;
int g_hookRand = -1, g_hookLfsr = -1, g_hookAi = -1, g_hookBank = -1, g_hookSlot = -1, g_hookRestart = -1;
// Sound_LoadBank 0x00425E4B (names, count): char[count][16]; slot n plays entry n - 1 (C51).
uint32_t g_bankNames = 0;
int g_bankCount = 0;
// g_realms 0x0057BF00, 0x160 a realm, +0x00 aiStep; AI_RunTurnStep 0x0049A581 is not reentrant.
int32_t g_aiBefore[6];
int32_t AiStep(int realm) { return *reinterpret_cast<volatile int32_t*>(0x0057BF00 + realm * 0x160); }

struct Sheet { uint32_t base, len; char name[24]; };
Sheet g_sheets[256];
int g_sheetNext = 0;

bool InExe(uint32_t v) { return v >= 0x00400000 && v < 0x00600000; }

void CopyString(char* out, size_t room, uint32_t p) {
    __try {
        const char* s = reinterpret_cast<const char*>(static_cast<uintptr_t>(p));
        size_t i = 0;
        for (; i + 1 < room && s[i]; i++) out[i] = s[i];
        out[i] = 0;
    } __except (1) { out[0] = 0; }
}

// File_ReadChunk 0x004AF8D9 (name, buf, len, offset): the last file read into a buffer names it.
void SheetLoaded(uint32_t name, uint32_t buf, uint32_t len) {
    char n[24];
    CopyString(n, sizeof(n), name);
    Sheet* slot = nullptr;
    for (auto& s : g_sheets) if (s.base == buf && s.len) slot = &s;
    if (!slot) slot = &g_sheets[g_sheetNext++ % 256];
    *slot = {buf, len, {}};
    memcpy(slot->name, n, sizeof(n));
}
const Sheet* SheetOf(uint32_t p) {
    for (int i = 0; i < 256; i++) {
        const Sheet& s = g_sheets[(g_sheetNext - 1 - i + 512) % 256];
        if (s.len && p >= s.base && p < s.base + s.len) return &s;
    }
    return nullptr;
}

void Append(char* text, const char* s) {
    size_t n = strlen(text);
    if (n && n < 63) text[n++] = '|';
    for (; *s && n < 63; s++) text[n++] = *s;
    text[n] = 0;
}

// The pinned LFSR seeds: any fixed nonzero 31-bit values; Rand_Advance shifts bit 30 in.
void Pin(uint32_t fn) {
    *reinterpret_cast<volatile uint32_t*>(0x0058FE28) = 0x1234567;
    *reinterpret_cast<volatile uint32_t*>(0x0058FE08) = 0x7654321;
    *reinterpret_cast<volatile uint32_t*>(0x00591564) = 0x13579BD;
    EvRecord* r = Ev_Begin(Ev_pin, fn, 0);
    Ev_End();
    (void)r;
}

void RecordCall(int h, const uint32_t* f) {
    const HookDef& d = kHooks[h];
    EvRecord* r = Ev_Begin(Ev_call, d.addr, f[9]);
    r->hook = static_cast<uint16_t>(h);
    int n = static_cast<int>(strlen(d.args));
    for (int i = 0; i < n && i < 8; i++) {
        uint32_t v = f[10 + i];
        char t = d.args[i];
        if (t == 's') { char s[64]; CopyString(s, sizeof(s), v); Append(r->text, s); v = 0; }
        else if (t == 'k') {
            const Sheet* s = SheetOf(v);
            Append(r->text, s ? s->name : "?");
            v = s ? v - s->base : 0;
        } else if (t == 'p') { if (InExe(v)) t = 'x'; else v = 0; }
        r->types[i] = t;
        r->args[i] = static_cast<int32_t>(v);
    }
    int k = n < 8 ? n : 8;
    for (int g = 0; g < 3 && d.globals[g] && k < 8; g++, k++) {
        r->types[k] = 'g';
        r->args[k] = *reinterpret_cast<volatile int32_t*>(static_cast<uintptr_t>(d.globals[g]));
    }
    r->nargs = static_cast<uint8_t>(k);
    if ((h == g_hookSlot || h == g_hookRestart) && g_bankNames) {
        int slot = static_cast<int>(f[10]);
        char s[20];
        s[0] = 0;
        if (slot >= 1 && slot <= g_bankCount) CopyString(s, 17, g_bankNames + (slot - 1) * 16);
        Append(r->text, s);
    }
    Ev_End();
}
}

extern "C" void __cdecl RetThunk();

// f: pushad block (edi esi ebp esp ebx edx ecx eax), f[8] hook index, f[9] return, f[10..] args.
extern "C" void __cdecl OnEntry(uint32_t* f) {
    const int h = static_cast<int>(f[8]);
    const HookDef& d = kHooks[h];
    f[8] = reinterpret_cast<uint32_t>(g_tramp[h]);
    if (GetCurrentThreadId() != g_mainThread) {
        static bool told[kCount];
        if (!told[h]) { told[h] = true; Log("off-thread call %s (thread %lu), not recorded", d.name, GetCurrentThreadId()); }
        return;
    }
    if (d.cat == Cat_draw) {
        g_frameDraws++;
        if (!g_ring || !g_ring->keepDraws) return;
    } else if (d.cat == Cat_rng) {
        if (h == g_hookRand) g_frameRand++; else g_frameLfsr++;
        return;
    }
    if (d.cat == Cat_load) SheetLoaded(f[10], f[11], f[12]);
    if (h == g_hookBank) { g_bankNames = f[10]; g_bankCount = static_cast<int>(f[11]) < 20 ? static_cast<int>(f[11]) : 20; }
    if (h == g_hookAi) for (int i = 1; i <= 5; i++) g_aiBefore[i] = AiStep(i);
    if (d.cat == Cat_newgame) Pin(d.addr);
    uint32_t g0 = d.globals[0] ? *reinterpret_cast<volatile uint32_t*>(static_cast<uintptr_t>(d.globals[0])) : 0;
    if (d.cat != Cat_turn) RecordCall(h, f);
    if (d.ret && g_depth < 1024) {
        g_shadow[g_depth++] = {f[9], reinterpret_cast<uint32_t>(&f[9]), static_cast<uint32_t>(h), g0};
        f[9] = reinterpret_cast<uint32_t>(&RetThunk);
    }
}

// f[8]: the slot RetThunk pushed, filled with the real return address.
extern "C" void __cdecl OnReturn(uint32_t* f) {
    const uint32_t esp = reinterpret_cast<uint32_t>(&f[9]);
    // A frame abandoned without returning sits deeper (lower) than ours: drop it.
    while (g_depth > 1 && g_shadow[g_depth - 1].slot + 4 + 32 < esp) {
        Log("shadow: dropped %s", kHooks[g_shadow[g_depth - 1].hook].name);
        g_depth--;
    }
    if (g_depth == 0) { Log("shadow stack empty at return"); ExitProcess(3); }
    const Shadow s = g_shadow[--g_depth];
    f[8] = s.ret;
    const HookDef& d = kHooks[s.hook];
    if (d.cat == Cat_turn) {
        int32_t now = *reinterpret_cast<volatile int32_t*>(static_cast<uintptr_t>(d.globals[0]));
        if (now == static_cast<int32_t>(s.g0)) return;
        EvRecord* r = Ev_Begin(Ev_phase, d.addr, s.ret);
        r->hook = static_cast<uint16_t>(s.hook);
        r->args[0] = static_cast<int32_t>(s.g0);
        r->args[1] = now;
        memcpy(r->types, "ii", 2);
        r->nargs = 2;
        Ev_End();
        return;
    }
    EvRecord* r = Ev_Begin(Ev_ret, d.addr, s.ret);
    r->hook = static_cast<uint16_t>(s.hook);
    r->ret = static_cast<int32_t>(f[7]);
    if (s.hook == static_cast<uint32_t>(g_hookAi)) {
        // The realm stepped is the one before the cursor DAT_0057C944 (0x0049A581 do-while).
        int cur = *reinterpret_cast<volatile int32_t*>(0x0057C944);
        int realm = cur <= 1 ? 5 : cur - 1;
        r->args[0] = realm;
        r->args[1] = g_aiBefore[realm];
        r->args[2] = AiStep(realm);
        memcpy(r->types, "iii", 3);
        r->nargs = 3;
    }
    Ev_End();
    if (d.cat == Cat_save) Pin(d.addr);
}

__declspec(naked) void CommonEntry() {
    __asm {
        pushad
        push esp
        call OnEntry
        add esp, 4
        popad
        ret
    }
}

extern "C" __declspec(naked) void __cdecl RetThunk() {
    __asm {
        push 0
        pushad
        push esp
        call OnReturn
        add esp, 4
        popad
        ret
    }
}

bool Detours_Install() {
    if (!g_ring) return true;
    g_mainThread = GetCurrentThreadId();
    auto mem = static_cast<uint8_t*>(VirtualAlloc(nullptr, kCount * 48, MEM_COMMIT | MEM_RESERVE, PAGE_EXECUTE_READWRITE));
    int ok = 0, refused = 0;
    for (int h = 0; h < kCount; h++) {
        const HookDef& d = kHooks[h];
        auto target = reinterpret_cast<uint8_t*>(static_cast<uintptr_t>(d.addr));
        if (strcmp(d.name, "Rand_Advance") == 0) g_hookRand = h;
        if (strcmp(d.name, "Rng_LfsrStep") == 0) g_hookLfsr = h;
        if (strcmp(d.name, "AI_RunTurnStep") == 0) g_hookAi = h;
        if (strcmp(d.name, "Sound_LoadBank") == 0) g_hookBank = h;
        if (strcmp(d.name, "Sound_PlaySlot") == 0) g_hookSlot = h;
        if (strcmp(d.name, "Sound_RestartSlot") == 0) g_hookRestart = h;
        if (memcmp(target, d.expect, d.stolen) != 0) {
            // Smk_Play under L2TRACE_FILMS=skip already jumps to H_SmkPlay, which records the film.
            bool filmSkip = d.cat == Cat_film && target[0] == 0xE9;
            Log("%s %s 0x%08X: prologue differs from the table", filmSkip ? "SKIPPED" : "REFUSED", d.name, d.addr);
            if (!filmSkip) {
                refused++;
                g_shared->patchesRefused++;
                EvRecord* r = Ev_Begin(Ev_refused, d.addr, 0);
                r->hook = static_cast<uint16_t>(h);
                Ev_End();
            }
            continue;
        }
        uint8_t* stub = mem + h * 48;
        uint8_t* tramp = stub + 16;
        stub[0] = 0x68;
        memcpy(stub + 1, &h, 4);
        stub[5] = 0xE9;
        int32_t rel = static_cast<int32_t>(reinterpret_cast<uintptr_t>(&CommonEntry) - reinterpret_cast<uintptr_t>(stub + 10));
        memcpy(stub + 6, &rel, 4);
        memcpy(tramp, target, d.stolen);
        tramp[d.stolen] = 0xE9;
        rel = static_cast<int32_t>(reinterpret_cast<uintptr_t>(target + d.stolen) - reinterpret_cast<uintptr_t>(tramp + d.stolen + 5));
        memcpy(tramp + d.stolen + 1, &rel, 4);
        g_tramp[h] = tramp;
        DWORD old;
        VirtualProtect(target, d.stolen, PAGE_EXECUTE_READWRITE, &old);
        target[0] = 0xE9;
        rel = static_cast<int32_t>(reinterpret_cast<uintptr_t>(stub) - reinterpret_cast<uintptr_t>(target + 5));
        memcpy(target + 1, &rel, 4);
        for (int i = 5; i < d.stolen; i++) target[i] = 0x90;
        VirtualProtect(target, d.stolen, old, &old);
        ok++;
    }
    FlushInstructionCache(GetCurrentProcess(), nullptr, 0);
    Log("detours: %d installed, %d refused, %d in table", ok, refused, kCount);
    return refused == 0;
}

// H_SmkPlay (hooks.cpp) stands in for Smk_Play 0x0042D91B when films are skipped.
void Events_Film(const char* path, uint32_t caller) {
    EvRecord* r = Ev_Begin(Ev_call, 0x0042D91B, caller);
    CopyString(r->text, sizeof(r->text), reinterpret_cast<uint32_t>(path));
    r->types[0] = 's';
    r->nargs = 1;
    Ev_End();
}

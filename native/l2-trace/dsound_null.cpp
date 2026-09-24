// Silent DirectSound for Sound_Init (DirectSoundCreate, IAT 0x005CF370) and the mixer's
// buffers: accepts everything and plays nothing; a looping stream advances on the virtual clock.
#include "trace.h"
#include <dsound.h>
#include <vector>

#define GAP(name) { LogGap(name); return DS_OK; }

namespace {
struct Buffer : IDirectSoundBuffer {
    LONG refs = 1;
    std::vector<uint8_t> data;
    WAVEFORMATEX fmt = {WAVE_FORMAT_PCM, 1, 22050, 22050, 1, 8, 0};
    LONG volume = 0, pan = 0;
    DWORD freq = 22050, flags = 0;
    bool playing = false, looping = false;
    uint32_t playMs = 0;

    explicit Buffer(LPCDSBUFFERDESC d) : data(d && d->dwBufferBytes ? d->dwBufferBytes : 65536) {
        if (d) flags = d->dwFlags;
        if (d && d->lpwfxFormat) { fmt = *d->lpwfxFormat; fmt.cbSize = 0; freq = fmt.nSamplesPerSec; }
    }
    STDMETHOD(QueryInterface)(REFIID, LPVOID* p) { *p = nullptr; LogGap("SoundBuffer::QueryInterface"); return E_NOINTERFACE; }
    STDMETHOD_(ULONG, AddRef)() { return InterlockedIncrement(&refs); }
    STDMETHOD_(ULONG, Release)() { LONG r = InterlockedDecrement(&refs); if (!r) delete this; return r; }
    STDMETHOD(GetCaps)(LPDSBCAPS c) {
        c->dwFlags = flags; c->dwBufferBytes = static_cast<DWORD>(data.size());
        c->dwUnlockTransferRate = 0; c->dwPlayCpuOverhead = 0; return DS_OK;
    }
    // +0x10, read by Music_Refill 0x00426DD5: a looping stream's cursor moves with the virtual clock.
    STDMETHOD(GetCurrentPosition)(LPDWORD play, LPDWORD write) {
        DWORD pos = 0;
        if (playing && looping && fmt.nAvgBytesPerSec) {
            uint64_t bytes = uint64_t(g_shared->virtualMs - playMs) * fmt.nAvgBytesPerSec / 1000;
            DWORD align = fmt.nBlockAlign ? fmt.nBlockAlign : 1;
            pos = DWORD(bytes % data.size()) / align * align;
        }
        if (play) *play = pos;
        if (write) *write = pos;
        return DS_OK;
    }
    STDMETHOD(GetFormat)(LPWAVEFORMATEX f, DWORD n, LPDWORD written) {
        if (f && n >= sizeof(WAVEFORMATEX)) *f = fmt;
        if (written) *written = sizeof(WAVEFORMATEX);
        return DS_OK;
    }
    STDMETHOD(GetVolume)(LPLONG v) { *v = volume; return DS_OK; }
    STDMETHOD(GetPan)(LPLONG p) { *p = pan; return DS_OK; }
    STDMETHOD(GetFrequency)(LPDWORD f) { *f = freq; return DS_OK; }
    // +0x24, Sound_PlaySlot 0x00426120 / Sound_OneShotBusy 0x00427C9B: [I] one-shots end at once.
    STDMETHOD(GetStatus)(LPDWORD s) { *s = playing && looping ? DSBSTATUS_PLAYING | DSBSTATUS_LOOPING : 0; return DS_OK; }
    STDMETHOD(Initialize)(LPDIRECTSOUND, LPCDSBUFFERDESC) GAP("SoundBuffer::Initialize")
    // +0x2C: the mixer writes the wave data here.
    STDMETHOD(Lock)(DWORD off, DWORD n, LPVOID* p1, LPDWORD n1, LPVOID* p2, LPDWORD n2, DWORD f) {
        DWORD size = static_cast<DWORD>(data.size());
        if (f & DSBLOCK_ENTIREBUFFER) { off = 0; n = size; }
        if (f & DSBLOCK_FROMWRITECURSOR) off = 0;
        off %= size;
        if (n > size) n = size;
        DWORD first = n < size - off ? n : size - off;
        *p1 = &data[off]; *n1 = first;
        if (p2) *p2 = n > first ? data.data() : nullptr;
        if (n2) *n2 = n - first;
        return DS_OK;
    }
    STDMETHOD(Play)(DWORD, DWORD, DWORD f) {
        playing = true; looping = (f & DSBPLAY_LOOPING) != 0; playMs = g_shared->virtualMs;
        return DS_OK;
    }
    STDMETHOD(SetCurrentPosition)(DWORD) { return DS_OK; }
    STDMETHOD(SetFormat)(LPCWAVEFORMATEX f) { if (f) { fmt = *f; fmt.cbSize = 0; } return DS_OK; }
    STDMETHOD(SetVolume)(LONG v) { volume = v; return DS_OK; }
    STDMETHOD(SetPan)(LONG p) { pan = p; return DS_OK; }
    STDMETHOD(SetFrequency)(DWORD f) { freq = f; return DS_OK; }
    STDMETHOD(Stop)() { playing = false; return DS_OK; }
    STDMETHOD(Unlock)(LPVOID, DWORD, LPVOID, DWORD) { return DS_OK; }
    STDMETHOD(Restore)() { return DS_OK; }
};

struct Sound : IDirectSound {
    LONG refs = 1;
    STDMETHOD(QueryInterface)(REFIID, LPVOID* p) { *p = nullptr; LogGap("DirectSound::QueryInterface"); return E_NOINTERFACE; }
    STDMETHOD_(ULONG, AddRef)() { return InterlockedIncrement(&refs); }
    STDMETHOD_(ULONG, Release)() { LONG r = InterlockedDecrement(&refs); if (!r) delete this; return r; }
    STDMETHOD(CreateSoundBuffer)(LPCDSBUFFERDESC d, LPDIRECTSOUNDBUFFER* b, LPUNKNOWN) { *b = new Buffer(d); return DS_OK; }
    STDMETHOD(GetCaps)(LPDSCAPS c) { DWORD n = c->dwSize; memset(c, 0, n); c->dwSize = n; LogGap("DirectSound::GetCaps (zeroed)"); return DS_OK; }
    STDMETHOD(DuplicateSoundBuffer)(LPDIRECTSOUNDBUFFER src, LPDIRECTSOUNDBUFFER* out) {
        auto s = static_cast<Buffer*>(src);
        DSBUFFERDESC d = {sizeof(d), s->flags, static_cast<DWORD>(s->data.size()), 0, &s->fmt};
        auto b = new Buffer(&d);
        b->data = s->data;
        *out = b;
        return DS_OK;
    }
    STDMETHOD(SetCooperativeLevel)(HWND, DWORD) { return DS_OK; }
    STDMETHOD(Compact)() { return DS_OK; }
    STDMETHOD(GetSpeakerConfig)(LPDWORD c) { *c = DSSPEAKER_STEREO; return DS_OK; }
    STDMETHOD(SetSpeakerConfig)(DWORD) { return DS_OK; }
    STDMETHOD(Initialize)(LPCGUID) GAP("DirectSound::Initialize")
};
}

extern "C" HRESULT WINAPI FakeDirectSoundCreate(LPCGUID, LPDIRECTSOUND* out, LPUNKNOWN) {
    Log("DirectSoundCreate (silent)");
    *out = new Sound;
    return DS_OK;
}

BOOL WINAPI DllMain(HINSTANCE inst, DWORD reason, LPVOID) {
    if (reason == DLL_PROCESS_ATTACH) {
        g_self = inst;
        DisableThreadLibraryCalls(inst);
        Shared_Open("l2trace-dsound.log");
    }
    return TRUE;
}

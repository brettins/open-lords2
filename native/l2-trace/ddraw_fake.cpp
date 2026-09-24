// Memory-backed DirectDraw for the calls Display_Init 0x0042E5C0, Display_CreateSurfaces
// 0x0042EC91, Palette_Create 0x0042F1DF and Gfx_Present 0x00452160 make.
#include "trace.h"
#include <ddraw.h>
#include <cstring>
#include <vector>

#define GAP(name) { LogGap(name); return DD_OK; }

namespace {
DWORD g_modeW = 640, g_modeH = 480;

void PixelFormat8(DDPIXELFORMAT* pf) {
    memset(pf, 0, sizeof(*pf));
    pf->dwSize = sizeof(*pf);
    pf->dwFlags = DDPF_RGB | DDPF_PALETTEINDEXED8;
    pf->dwRGBBitCount = 8;
}

struct Palette : IDirectDrawPalette {
    LONG refs = 1;
    PALETTEENTRY e[256] = {};
    STDMETHOD(QueryInterface)(REFIID, LPVOID* p) { *p = nullptr; LogGap("Palette::QueryInterface"); return E_NOINTERFACE; }
    STDMETHOD_(ULONG, AddRef)() { return InterlockedIncrement(&refs); }
    STDMETHOD_(ULONG, Release)() { LONG r = InterlockedDecrement(&refs); if (!r) delete this; return r; }
    STDMETHOD(GetCaps)(LPDWORD c) { *c = DDPCAPS_8BIT | DDPCAPS_ALLOW256; return DD_OK; }
    STDMETHOD(GetEntries)(DWORD, DWORD start, DWORD n, LPPALETTEENTRY out) {
        if (start + n > 256) return DDERR_INVALIDPARAMS;
        memcpy(out, e + start, n * sizeof(PALETTEENTRY)); return DD_OK;
    }
    STDMETHOD(Initialize)(LPDIRECTDRAW, DWORD, LPPALETTEENTRY) GAP("Palette::Initialize")
    // +0x18: Palette_Upload 0x0042F58C pushes all 256.
    STDMETHOD(SetEntries)(DWORD, DWORD start, DWORD n, LPPALETTEENTRY in) {
        if (start + n > 256) return DDERR_INVALIDPARAMS;
        memcpy(e + start, in, n * sizeof(PALETTEENTRY)); return DD_OK;
    }
};

struct Clipper : IDirectDrawClipper {
    LONG refs = 1;
    HWND hwnd = nullptr;
    STDMETHOD(QueryInterface)(REFIID, LPVOID* p) { *p = nullptr; LogGap("Clipper::QueryInterface"); return E_NOINTERFACE; }
    STDMETHOD_(ULONG, AddRef)() { return InterlockedIncrement(&refs); }
    STDMETHOD_(ULONG, Release)() { LONG r = InterlockedDecrement(&refs); if (!r) delete this; return r; }
    STDMETHOD(GetClipList)(LPRECT, LPRGNDATA, LPDWORD) GAP("Clipper::GetClipList")
    STDMETHOD(GetHWnd)(HWND* h) { *h = hwnd; return DD_OK; }
    STDMETHOD(Initialize)(LPDIRECTDRAW, DWORD) GAP("Clipper::Initialize")
    STDMETHOD(IsClipListChanged)(BOOL* b) { *b = FALSE; return DD_OK; }
    STDMETHOD(SetClipList)(LPRGNDATA, DWORD) GAP("Clipper::SetClipList")
    STDMETHOD(SetHWnd)(DWORD, HWND h) { hwnd = h; return DD_OK; }
};

// Clamps a rect to a surface; null means the whole surface.
RECT Clamp(const RECT* r, LONG w, LONG h) {
    RECT o = r ? *r : RECT{0, 0, w, h};
    if (o.left < 0) o.left = 0;
    if (o.top < 0) o.top = 0;
    if (o.right > w) o.right = w;
    if (o.bottom > h) o.bottom = h;
    return o;
}

struct Surface : IDirectDrawSurface {
    LONG refs = 1;
    bool primary = false;
    LONG w = 0, h = 0;
    std::vector<uint8_t> bits;
    Palette* pal = nullptr;
    Clipper* clip = nullptr;

    Surface(LONG width, LONG height, bool isPrimary) : primary(isPrimary), w(width), h(height),
        bits(static_cast<size_t>(width) * height, 0) {}
    ~Surface() { if (pal) pal->Release(); if (clip) clip->Release(); }

    void Presented() {
        static PALETTEENTRY black[256] = {};
        if (primary) Frame_Presented(bits.data(), w, w, h, pal ? pal->e : black);
    }
    void Copy(LONG dx, LONG dy, Surface* src, const RECT* srcRect) {
        RECT s = Clamp(srcRect, src->w, src->h);
        for (LONG y = s.top; y < s.bottom; y++) {
            LONG ty = dy + (y - s.top);
            if (ty < 0 || ty >= h) continue;
            LONG x0 = s.left, tx = dx;
            if (tx < 0) { x0 -= tx; tx = 0; }
            LONG n = s.right - x0;
            if (tx + n > w) n = w - tx;
            if (n > 0) memmove(&bits[ty * w + tx], &src->bits[y * src->w + x0], n);
        }
    }

    STDMETHOD(QueryInterface)(REFIID, LPVOID* p) { *p = nullptr; LogGap("Surface::QueryInterface"); return E_NOINTERFACE; }
    STDMETHOD_(ULONG, AddRef)() { return InterlockedIncrement(&refs); }
    STDMETHOD_(ULONG, Release)() { LONG r = InterlockedDecrement(&refs); if (!r) delete this; return r; }
    STDMETHOD(AddAttachedSurface)(LPDIRECTDRAWSURFACE) GAP("Surface::AddAttachedSurface")
    STDMETHOD(AddOverlayDirtyRect)(LPRECT) GAP("Surface::AddOverlayDirtyRect")
    // +0x14: colour fill 0x1000400 in Display_ClearSurfaces 0x0042EB7C; copy 0x1000000 in Gfx_Present.
    STDMETHOD(Blt)(LPRECT dst, LPDIRECTDRAWSURFACE src, LPRECT srcRect, DWORD flags, LPDDBLTFX fx) {
        RECT d = Clamp(dst, w, h);
        if (flags & DDBLT_COLORFILL) {
            uint8_t c = fx ? static_cast<uint8_t>(fx->dwFillColor) : 0;
            for (LONG y = d.top; y < d.bottom; y++) memset(&bits[y * w + d.left], c, d.right - d.left);
        } else if (src) {
            Copy(d.left, d.top, static_cast<Surface*>(src), srcRect);
        } else {
            LogGap("Surface::Blt without source or fill");
        }
        Presented();
        return DD_OK;
    }
    STDMETHOD(BltBatch)(LPDDBLTBATCH, DWORD, DWORD) GAP("Surface::BltBatch")
    // +0x1C: the full-screen present in Gfx_Present and Gfx_PresentTiled.
    STDMETHOD(BltFast)(DWORD x, DWORD y, LPDIRECTDRAWSURFACE src, LPRECT srcRect, DWORD) {
        if (src) Copy(static_cast<LONG>(x), static_cast<LONG>(y), static_cast<Surface*>(src), srcRect);
        Presented();
        return DD_OK;
    }
    STDMETHOD(DeleteAttachedSurface)(DWORD, LPDIRECTDRAWSURFACE) GAP("Surface::DeleteAttachedSurface")
    STDMETHOD(EnumAttachedSurfaces)(LPVOID, LPDDENUMSURFACESCALLBACK) GAP("Surface::EnumAttachedSurfaces")
    STDMETHOD(EnumOverlayZOrders)(DWORD, LPVOID, LPDDENUMSURFACESCALLBACK) GAP("Surface::EnumOverlayZOrders")
    STDMETHOD(Flip)(LPDIRECTDRAWSURFACE, DWORD) { LogGap("Surface::Flip"); Presented(); return DD_OK; }
    STDMETHOD(GetAttachedSurface)(LPDDSCAPS, LPDIRECTDRAWSURFACE* s) { *s = nullptr; LogGap("Surface::GetAttachedSurface"); return DDERR_NOTFOUND; }
    STDMETHOD(GetBltStatus)(DWORD) { return DD_OK; }
    STDMETHOD(GetCaps)(LPDDSCAPS c) { c->dwCaps = primary ? DDSCAPS_PRIMARYSURFACE : DDSCAPS_OFFSCREENPLAIN | DDSCAPS_SYSTEMMEMORY; return DD_OK; }
    STDMETHOD(GetClipper)(LPDIRECTDRAWCLIPPER* c) { *c = clip; if (clip) clip->AddRef(); return clip ? DD_OK : DDERR_NOCLIPPERATTACHED; }
    STDMETHOD(GetColorKey)(DWORD, LPDDCOLORKEY) { LogGap("Surface::GetColorKey"); return DDERR_NOCOLORKEY; }
    STDMETHOD(GetDC)(HDC* dc) { *dc = nullptr; LogGap("Surface::GetDC"); return DDERR_UNSUPPORTED; }
    STDMETHOD(GetFlipStatus)(DWORD) { return DD_OK; }
    STDMETHOD(GetOverlayPosition)(LPLONG, LPLONG) GAP("Surface::GetOverlayPosition")
    STDMETHOD(GetPalette)(LPDIRECTDRAWPALETTE* p) { *p = pal; if (pal) pal->AddRef(); return pal ? DD_OK : DDERR_NOPALETTEATTACHED; }
    // +0x54 from Display_CreateSurfaces: 8-bit palettised, like the mode the original sets.
    STDMETHOD(GetPixelFormat)(LPDDPIXELFORMAT pf) { PixelFormat8(pf); return DD_OK; }
    STDMETHOD(GetSurfaceDesc)(LPDDSURFACEDESC d) { Describe(d); return DD_OK; }
    STDMETHOD(Initialize)(LPDIRECTDRAW, LPDDSURFACEDESC) GAP("Surface::Initialize")
    STDMETHOD(IsLost)() { return DD_OK; }
    // +0x64: Display_CreateSurfaces keeps lpSurface (desc +0x24) as g_backBufferBits.
    STDMETHOD(Lock)(LPRECT r, LPDDSURFACEDESC d, DWORD, HANDLE) {
        Describe(d);
        if (r) d->lpSurface = &bits[r->top * w + r->left];
        return DD_OK;
    }
    STDMETHOD(ReleaseDC)(HDC) GAP("Surface::ReleaseDC")
    STDMETHOD(Restore)() { return DD_OK; }
    STDMETHOD(SetClipper)(LPDIRECTDRAWCLIPPER c) {
        if (c) c->AddRef();
        if (clip) clip->Release();
        clip = static_cast<Clipper*>(c);
        return DD_OK;
    }
    STDMETHOD(SetColorKey)(DWORD, LPDDCOLORKEY) GAP("Surface::SetColorKey")
    STDMETHOD(SetOverlayPosition)(LONG, LONG) GAP("Surface::SetOverlayPosition")
    STDMETHOD(SetPalette)(LPDIRECTDRAWPALETTE p) {
        if (p) p->AddRef();
        if (pal) pal->Release();
        pal = static_cast<Palette*>(p);
        return DD_OK;
    }
    STDMETHOD(Unlock)(LPVOID) { return DD_OK; }
    STDMETHOD(UpdateOverlay)(LPRECT, LPDIRECTDRAWSURFACE, LPRECT, DWORD, LPDDOVERLAYFX) GAP("Surface::UpdateOverlay")
    STDMETHOD(UpdateOverlayDisplay)(DWORD) GAP("Surface::UpdateOverlayDisplay")
    STDMETHOD(UpdateOverlayZOrder)(DWORD, LPDIRECTDRAWSURFACE) GAP("Surface::UpdateOverlayZOrder")

    void Describe(LPDDSURFACEDESC d) {
        DWORD size = d->dwSize ? d->dwSize : sizeof(DDSURFACEDESC);
        memset(d, 0, size);
        d->dwSize = size;
        d->dwFlags = DDSD_CAPS | DDSD_HEIGHT | DDSD_WIDTH | DDSD_PITCH | DDSD_LPSURFACE | DDSD_PIXELFORMAT;
        d->dwHeight = h;
        d->dwWidth = w;
        d->lPitch = w;
        d->lpSurface = bits.data();
        PixelFormat8(&d->ddpfPixelFormat);
        GetCaps(&d->ddsCaps);
    }
};

struct DDraw : IDirectDraw {
    LONG refs = 1;
    STDMETHOD(QueryInterface)(REFIID, LPVOID* p) { *p = nullptr; LogGap("DirectDraw::QueryInterface"); return E_NOINTERFACE; }
    STDMETHOD_(ULONG, AddRef)() { return InterlockedIncrement(&refs); }
    STDMETHOD_(ULONG, Release)() { LONG r = InterlockedDecrement(&refs); if (!r) delete this; return r; }
    STDMETHOD(Compact)() GAP("DirectDraw::Compact")
    STDMETHOD(CreateClipper)(DWORD, LPDIRECTDRAWCLIPPER* c, IUnknown*) { *c = new Clipper; return DD_OK; }
    STDMETHOD(CreatePalette)(DWORD, LPPALETTEENTRY e, LPDIRECTDRAWPALETTE* p, IUnknown*) {
        Palette* pal = new Palette;
        if (e) memcpy(pal->e, e, sizeof(pal->e));
        *p = pal;
        return DD_OK;
    }
    // +0x18: primary (caps 0x200) or Display_CreateSurface 0x0042F148's offscreen plain.
    STDMETHOD(CreateSurface)(LPDDSURFACEDESC d, LPDIRECTDRAWSURFACE* s, IUnknown*) {
        bool primary = (d->dwFlags & DDSD_CAPS) && (d->ddsCaps.dwCaps & DDSCAPS_PRIMARYSURFACE);
        LONG w = primary ? g_modeW : d->dwWidth, h = primary ? g_modeH : d->dwHeight;
        if (d->dwFlags & DDSD_BACKBUFFERCOUNT) LogGap("DirectDraw::CreateSurface flipping chain");
        Log("CreateSurface %s %ldx%ld caps %08lx", primary ? "primary" : "offscreen", w, h, d->ddsCaps.dwCaps);
        *s = new Surface(w, h, primary);
        return DD_OK;
    }
    STDMETHOD(DuplicateSurface)(LPDIRECTDRAWSURFACE, LPDIRECTDRAWSURFACE*) { LogGap("DirectDraw::DuplicateSurface"); return DDERR_UNSUPPORTED; }
    STDMETHOD(EnumDisplayModes)(DWORD, LPDDSURFACEDESC, LPVOID, LPDDENUMMODESCALLBACK) GAP("DirectDraw::EnumDisplayModes")
    STDMETHOD(EnumSurfaces)(DWORD, LPDDSURFACEDESC, LPVOID, LPDDENUMSURFACESCALLBACK) GAP("DirectDraw::EnumSurfaces")
    STDMETHOD(FlipToGDISurface)() { return DD_OK; }
    STDMETHOD(GetCaps)(LPDDCAPS a, LPDDCAPS b) {
        if (a) { DWORD n = a->dwSize; memset(a, 0, n); a->dwSize = n; }
        if (b) { DWORD n = b->dwSize; memset(b, 0, n); b->dwSize = n; }
        LogGap("DirectDraw::GetCaps (zeroed)");
        return DD_OK;
    }
    STDMETHOD(GetDisplayMode)(LPDDSURFACEDESC d) {
        memset(d, 0, sizeof(*d));
        d->dwSize = sizeof(*d);
        d->dwFlags = DDSD_HEIGHT | DDSD_WIDTH | DDSD_PITCH | DDSD_PIXELFORMAT;
        d->dwWidth = g_modeW; d->dwHeight = g_modeH; d->lPitch = g_modeW;
        PixelFormat8(&d->ddpfPixelFormat);
        return DD_OK;
    }
    STDMETHOD(GetFourCCCodes)(LPDWORD n, LPDWORD) { if (n) *n = 0; return DD_OK; }
    STDMETHOD(GetGDISurface)(LPDIRECTDRAWSURFACE* s) { *s = nullptr; LogGap("DirectDraw::GetGDISurface"); return DDERR_NOTFOUND; }
    STDMETHOD(GetMonitorFrequency)(LPDWORD f) { *f = 60; return DD_OK; }
    STDMETHOD(GetScanLine)(LPDWORD l) { *l = 0; return DD_OK; }
    STDMETHOD(GetVerticalBlankStatus)(LPBOOL b) { *b = TRUE; return DD_OK; }
    STDMETHOD(Initialize)(GUID*) GAP("DirectDraw::Initialize")
    STDMETHOD(RestoreDisplayMode)() { return DD_OK; }
    STDMETHOD(SetCooperativeLevel)(HWND, DWORD f) { Log("SetCooperativeLevel %08lx", f); return DD_OK; }
    STDMETHOD(SetDisplayMode)(DWORD w, DWORD h, DWORD bpp) {
        Log("SetDisplayMode %lux%lux%lu", w, h, bpp);
        g_modeW = w; g_modeH = h;
        return DD_OK;
    }
    STDMETHOD(WaitForVerticalBlank)(DWORD, HANDLE) { return DD_OK; }
};
}

// Exported as DirectDrawCreate (ddraw.def); the IAT slot is 0x005CF35C.
extern "C" HRESULT WINAPI FakeDirectDrawCreate(GUID*, LPDIRECTDRAW* out, IUnknown*) {
    Log("DirectDrawCreate");
    *out = new DDraw;
    return DD_OK;
}

BOOL WINAPI DllMain(HINSTANCE inst, DWORD reason, LPVOID) {
    if (reason == DLL_PROCESS_ATTACH) {
        g_self = inst;
        DisableThreadLibraryCalls(inst);
        Shared_Open("l2trace.log");
        if (!Hooks_Install()) Log("HOOKS INCOMPLETE - see REFUSED lines");
    }
    return TRUE;
}

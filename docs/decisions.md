# Decisions and corrections

Append-only. The corrections matter as much as the decisions — they stop us re-deriving
conclusions already tried and found wrong.

## Decisions

**D1 — Reimplement the engine; don't build a mod framework on the original.**
A hook-based framework would have taken weeks rather than years, but it is permanently
bounded by the 1996 binary: 256 colours, DirectDraw, fixed resolution. The goal is an
OpenXcom-style moddable engine, so the ceiling matters more than the speed.

**D2 — Incremental replacement, not a big-bang rewrite.**
Each subsystem is reimplemented behind a seam and verified against the original, so
something runs and is verified at every step. This is unusually workable here: no ASLR
plus ~955 KB of game state in fixed `.data` globals means our code can read the original's
live state directly. We don't need to replicate the whole world model before testing one
piece of it.

**D3 — Rust for formats and engine; MSVC C++ only if in-process hooks are needed.**
Binary parsing is Rust at its best. A hook DLL living inside a 32-bit 1996 process is the
one place C++ is less friction.

**D4 — `winit` + `pixels`; not SDL, and definitely not a game engine.**
This is a software-rendered 8-bit palettized 640×480 game. Its entire renderer is a byte
array plus a blitter — roughly 300 lines. We must own a plain `Vec<u8>` framebuffer,
because the endgame is diffing our output against the original's pixel for pixel. An
engine that hides the framebuffer behind a render graph actively fights that requirement.
OpenXcom chose SDL in 2010, when the pure-Rust option didn't exist; that's not evidence
for SDL today.

**D5 — MIT licence; GPL-3 prior art is reference-only.**
Formats are facts and not copyrightable, so their documentation is usable. Their code is
not. See `CLAUDE.md`.

**D6 — Node prototypes are disposable.**
Node answered "can we read this data at all?" quickly. Rust is the implementation. The
Node decoders survive only while they're useful as a porting check.

## Corrections

**C1 — "The PL8 format is fully decoded."** Claimed after one sprite rendered correctly.
The corpus check immediately found 153 of 291 files failing. *A single working example
proves nothing about a format.*

**C2 — "All 23 `raw:2` files fail, so sub-mode 2 changes the raw layout."** Wrong. The
count of `raw:2` files and the count of failures were both 23, and that coincidence was
over-read. 13 of those files decode correctly.

**C3 — "The three blitters are the three storage modes, so mode 2 is raw with a row
stride."** Wrong, and retracted. `Clip_Horizontal` sets that selector from clipping
geometry; the blitters are unclipped / clipped-left / clipped-right variants of the same
copy. Three functions happened to match three modes and a plausible story assembled
itself. *Decompiler output invites exactly this error.*

**C4 — Frame count reported as 16,638.** That was the Node checker counting frames inside
files that later failed validation. The verified figure is 16,435.

**C5 — Assumed no prior art existed.** The single largest waste of effort so far. The PL8
format — including the per-frame tile-type byte that governs the "unknown" storage mode 2 —
is documented in the GPL-3 `pl8image` package, and `L2_maps.dat`'s slot structure in
OpenLotR2's docs. Both were found by a five-minute search *after* days of equivalent work
had been commissioned. **Search for prior art before reverse-engineering anything.**

## Open questions

- PL8 storage mode 2 decoding across all five per-tile types, plus extra rows.
- What a map slot's six 64×64 layers and its one 65×129 layer actually contain.
- 23 files that use supported storage modes but fail the end-offset invariant, pinned in
  `KNOWN_FAILING`.
- `Font_c2.pl8` declares RLE but its frames occupy exactly `width × height`.
- Where the storage mode is read from the header at load time — nothing decompiled so far
  touches header byte 0.

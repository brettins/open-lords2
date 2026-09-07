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

**D5a — OPEN, and not ours to settle: Smacker decoding forces an LGPL choice.**
D5 says "MIT" without qualification. That may not survive contact with the 45 `.smk`
videos.

There is **no permissively licensed Smacker decoder**. `libsmacker` has been LGPL-2.1
since January 2020; the `smk` Rust crate is a declared port of it and is LGPL-2.1 too;
FFmpeg's decoder is likewise unavailable to us on permissive terms. The only public-domain
implementation parses headers and has no codec at all. Transcoding at install time is not
an escape hatch, because transcoding needs a decoder.

So the options are: accept an LGPL component and the notice obligations that follow;
write a Smacker decoder from the format description, which is a project in itself; or ship
without in-game video. **This changes what the project's own licence can claim, so it is
the user's call, not a technical default.**

If we do adopt one, put it behind our own trait in a leaf crate so swapping it later is a
one-crate change. Note also that the `smk` crate is unmaintained (single release) and has a
real bug — one shipped video dies on a spurious overlap guard — so adopting it means
carrying a patch.

**D6 — Node prototypes are disposable.**
Node answered "can we read this data at all?" quickly. Rust is the implementation. The
Node decoders survive only while they're useful as a porting check.

**D7 — The Node decoders and the differential harness are retired.**
They were built to validate the Node → Rust port, and they did that: 21,344 frames
byte-identical, plus a mutation test proving the harness could actually fail. Once
Rust gained isometric support the two implementations diverged on 2,534 lines — not
a bug, just Rust outgrowing the prototype. Keeping a prototype that silently
disagrees with the real implementation is a liability, and re-porting isometric
decoding into throwaway JavaScript would have added no confidence: two
implementations by the same author on the same day are correlated, so agreement was
always weaker evidence than it sounded. The end-offset invariant, which is a
property of the data rather than of our code, is the stronger check and it lives in
the Rust corpus test.

**D8 — The oracle is driven by code injection, not by synthetic input.**
Phase 5 originally assumed we could drive the original game through its UI and compare
what it did. That is blocked, and the block is empirical rather than theoretical:

- Synthetic input needs window focus, and `SetForegroundWindow` fails silently from a
  background process. Posting `WM_KEYDOWN` directly does not help — verified by minimising
  a window so it could not be focused, then posting keys: no events arrived.
- A fullscreen DirectDraw game generally captures as pure black, so the screen cannot be
  read either.
- Every process spawned to *attempt* input risks stealing focus from the game, which both
  blocks input and, during startup, crashes it outright. An agent tasked with visual
  comparison got the game to its start screen and then stalled; the game's own
  `status.txt` ended `Minimizing. / Paused. / Not active.`

What does work, and needs no focus at all: **reading the game's memory from another
process** (`tools/probe.ps1`, verified against a live `Lords2.exe`), and **running our own
code inside the process** via a `ddraw.dll` proxy. The game imports exactly one function
from DirectDraw, so the proxy is small.

This makes the proxy loader more important than it looked, not less: it sidesteps input
entirely. Differential testing should call the original's functions and read its state
directly rather than pantomiming a player.

**Corollary on the read-only rule:** a proxy DLL has to sit beside the executable, but
`CLAUDE.md` rule 2 says the installs are read-only. Resolve it with a sibling directory of
**hard links** to the original files plus our own DLL — no copying, no space, and the
install stays untouched.

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
files that later failed validation. The figure quoted as the correction, 16,435,
does not reproduce either; an audit could recover neither number. The count that
matters now is 21,344 frames across all 291 files, which does reproduce.

**C5 — Assumed no prior art existed.** The single largest waste of effort so far. The PL8
format — including the per-frame tile-type byte that governs the "unknown" storage mode 2 —
is documented in the GPL-3 `pl8image` package, and `L2_maps.dat`'s slot structure in
OpenLotR2's docs. Both were found by a five-minute search *after* days of equivalent work
had been commissioned. **Search for prior art before reverse-engineering anything.**

**C6 — "Header byte 1 is a sub-mode."** It is the **map zoom level**: 0 → 58×30
tiles, 1 → 26×14, 2 → 10×6, correlating perfectly across all 32 isometric files.
Related: reading bytes 0x00–0x01 as one `u16` is wrong, since byte 1 varies
independently of byte 0.

**C7 — Treating the storage family byte as the encoding.** It is not. The per-frame
`shape` byte at record offset 0x0C decides, which is why no single "mode 2 codec"
ever fit: one isometric file holds raw rectangles and diamonds side by side.

**C8 — "Search for prior art first" needs a second half: verify it.**
C5 was right that not searching cost real effort. But prior art is a *lead*, not an
authority. OpenLotR2's `.skr` documentation is wrong in three places, each caught only by
checking bytes: the battlefield layer is 80×80 rather than 64×64, a text record is 183
bytes rather than 182, and the layers do not begin where it says — an unexplained 328-byte
pad precedes them. A decoder built on the document alone would have desynchronised
immediately.

The rule is therefore: **find prior art, then validate every claim against the data before
building on it.** Its real value is telling you *what to look for*, which is worth a great
deal even when the specifics are wrong.

**C9 — `git add -A` swept agents' files twice before the rule stuck.**
Commit `38278d0` took an agent's in-progress `ghidra_scripts_skr/`, `tools/maps/dump.ps1`
and a whole findings document under a commit message about something else. The explicit-
path rule in `docs/agents.md` was written immediately after and did hold for the next
commit — but the lesson is that a rule written *after* the damage still leaves misleading
history behind.

## Open questions

- The four map planes whose meaning is inferred rather than proven (graphics bank,
  descriptor index, multi-tile object part), and the exact tile → lattice mapping,
  whose best affine fit reaches only 72%.
- 23 files that use supported encodings but fail the end-offset invariant, pinned in
  `KNOWN_FAILING`.
- `Font_c2.pl8` declares RLE but its frames occupy exactly `width × height`.
- `Title.pl8` decodes with correct geometry but no shipped palette colours it.
- The type-4 apex pair, where the stored data and the shipped blitter disagree.
- PL8 header fields at 0x04, 0x06, 0x07.

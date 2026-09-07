# lords2

Tooling and an incremental reimplementation of **Lords of the Realm II** (Sierra/Impressions, 1996),
in the spirit of OpenXcom: an open engine that loads the original game's data files.

**You must own an original copy of the game.** No game assets are distributed here,
and none are ever committed to this repository - see `.gitignore`.

## Layout

| Path | Purpose |
|------|---------|
| `crates/l2-formats/` | Dependency-free decoders for the game's asset formats |
| `tools/` | Analysis tooling: PE inspection, format decoders, live-process probes |
| `docs/formats/` | Reverse-engineered file format documentation |
| `docs/` | Project notes and roadmap |

## Approach

The original `Lords2.exe` is used as an **oracle**, not a target. It has no ASLR and a
fixed image base of `0x400000`, so its game state lives at stable addresses in `.data`
and can be read from a live process. That lets each reimplemented subsystem be
verified by differential testing against the original rather than against guesswork.

## Testing

```powershell
cargo test -p l2-formats                       # unit tests; no game install needed
$env:LORDS2_DIR = 'F:\games\Lords of the Realm II'
cargo test -p l2-formats -- --nocapture        # + corpus validation against a real install
.\tools\pl8diff.ps1                            # + Node vs Rust differential test
```

The corpus tests skip when `LORDS2_DIR` is unset, so a checkout without the game
still has real tests to run.

`pl8diff.ps1` runs two independently written PL8 decoders - `tools/pl8digest.js`
and `crates/l2-formats/examples/pl8digest.rs` - over the same corpus and requires
their per-frame digests to be identical. Both hash the palette indices *and* the
transparency mask with a hand-rolled FNV-1a 64, so a decoder that got coverage
right and colour wrong (or the reverse) still diverges. Nothing is written to
disk; the streams are compared in memory.

Agreement between the two rules out implementation bugs, not misunderstanding:
a shared misreading of the format would agree just as cleanly. Only the
end-offset invariant in `docs/formats/pl8.md` and comparison against the game's
own rendering can speak to correctness.

## Tools

Format work:
- `tools/pl8dump.js <file.pl8> <palette.256> <frame> <out.png>` - decode one sprite frame
- `tools/pl8check.js <dir>` - validate the PL8 decoder across a whole directory
- `tools/pl8digest.js <dir>` - per-frame digest of every PL8, for differential testing
- `tools/pl8diff.ps1` - assert the Node and Rust PL8 decoders agree frame for frame

Binary analysis:
- `tools/peimp.js` / `peexp.js` / `pehdr.js` / `pefun.js` - PE imports, exports, headers
- `tools/xref.js <exe> [filter]` - find call sites of imported functions

Live process (PowerShell):
- `tools/probe.ps1` - read memory from a running process
- `tools/screen.ps1` - capture a window (PrintWindow, occlusion-proof)
- `tools/input.ps1` - synthetic mouse/keyboard input
- `tools/windows.ps1` - enumerate a process's top-level windows

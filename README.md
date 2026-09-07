# lords2

Tooling and an incremental reimplementation of **Lords of the Realm II** (Sierra/Impressions, 1996),
in the spirit of OpenXcom: an open engine that loads the original game's data files.

**You must own an original copy of the game.** No game assets are distributed here,
and none are ever committed to this repository - see `.gitignore`.

## Layout

| Path | Purpose |
|------|---------|
| `tools/` | Analysis tooling: PE inspection, format decoders, live-process probes |
| `docs/formats/` | Reverse-engineered file format documentation |
| `docs/` | Project notes and roadmap |

## Approach

The original `Lords2.exe` is used as an **oracle**, not a target. It has no ASLR and a
fixed image base of `0x400000`, so its game state lives at stable addresses in `.data`
and can be read from a live process. That lets each reimplemented subsystem be
verified by differential testing against the original rather than against guesswork.

## Tools

Format work:
- `tools/pl8dump.js <file.pl8> <palette.256> <frame> <out.png>` - decode one sprite frame
- `tools/pl8check.js <dir>` - validate the PL8 decoder across a whole directory

Binary analysis:
- `tools/peimp.js` / `peexp.js` / `pehdr.js` / `pefun.js` - PE imports, exports, headers
- `tools/xref.js <exe> [filter]` - find call sites of imported functions

Live process (PowerShell):
- `tools/probe.ps1` - read memory from a running process
- `tools/screen.ps1` - capture a window (PrintWindow, occlusion-proof)
- `tools/input.ps1` - synthetic mouse/keyboard input
- `tools/windows.ps1` - enumerate a process's top-level windows

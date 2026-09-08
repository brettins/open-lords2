# lords2

Incremental open reimplementation of **Lords of the Realm II** (Sierra/Impressions, 1996):
our own engine, the original game's data files. OpenXcom's model.

**This file is an index, not a manual.** It is loaded into every session and every
subagent, so it stays short. Read the linked document before working in that area.

## Rules that must never be broken

1. **Never commit game assets** — no `.pl8`, `.256`, `.smk`, `.wav`, or extracted images.
   `.gitignore` enforces it; don't weaken it. Users bring their own copy of the game.
2. **The game installs are read-only.** Never write to `F:\games\Lords of the Realm II`
   or `F:\games\LORDS2`.
3. **This project is MIT. The useful prior art is GPL-3.** Use documented format *facts*
   freely — formats are not copyrightable. Never copy, vendor or translate their code.
4. **Label claims verified or inferred.** A plausible story assembled from decompiler
   output is not a finding.

## Where to look

| Before you… | Read |
|---|---|
| run any command | `docs/environment.md` |
| work on a file format | `docs/formats/` |
| read or name the binary | `docs/symbols.md` |
| touch the battle simulation | `docs/battle.md` |
| draw a screen, or wonder what the original showed | `docs/screens-county.md` |
| write anything that must stay deterministic | `docs/netcode.md` |
| add or change a rule, or anything a mod overrides | `docs/modding.md` |
| revisit an architectural choice | `docs/decisions.md` |
| spawn or coordinate agents | `docs/agents.md` |
| ask what state the project is in | `docs/status.html` |
| wonder whether a documented number is true | `docs/audit.md` |

**`docs/netcode.md` binds code you might not think of as networking.** Deterministic
lockstep means the *simulation* has constraints: no floats where ordering matters, a
seeded PRNG frozen in-tree, and no iteration whose order depends on hashing. Honouring
those while the simulation is being written is nearly free; retrofitting them is not.

## The idea that shapes everything

`Lords2.exe` is an **oracle, not a target**. We don't patch it — we query it. It has no
ASLR and a fixed base of `0x400000`, so its addresses are stable and its live state is
readable from another process.

Two of our own implementations agreeing proves only that we ported our own
misunderstanding faithfully. Real evidence comes from self-verifying invariants in the
data, and from the original binary. Where they disagree with us, they are right.

**Search for prior art before reverse-engineering anything.** Not doing so has already
been the single largest waste of effort on this project — see `docs/decisions.md`, C5.

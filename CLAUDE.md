# lords2

Open reimplementation of **Lords of the Realm II** (Sierra/Impressions, 1996): our engine,
the original's data files. This file is an index. It is loaded into every session and every
agent, so it stays short. Read the linked document before working in that area.

## Rules that must never be broken

1. **Never commit game assets** — no `.pl8`, `.256`, `.smk`, `.wav`, or extracted images.
   `.gitignore` enforces it; don't weaken it.
2. **The game installs are read-only.** Never write to `F:\games\Lords of the Realm II`
   or `F:\games\LORDS2`.
3. **This project is MIT; the useful prior art is GPL-3.** Format *facts* are free. Never
   copy, vendor or translate their code.
4. **Label claims verified or inferred.** A plausible story assembled from decompiler
   output is not a finding.
5. **If we implement a feature, find its equivalent in the binary's functions**, and name
   it in the code beside the behaviour. *"We could not find it"* is a finding, not a licence
   to invent. Measured, generated not typed: we reproduce
   **<!--fig:arms-reproduced-->225<!--/fig--> of <!--fig:arms-live-->252<!--/fig-->** live
   input arms (C61, `docs/arms.json`). That counts *which* controls a screen answers, not
   *how*: the original's input model is one kind byte at `+0x0F` of a 24-byte record, all
   <!--fig:gesture-kinds-->5<!--/fig--> gesture kinds built over
   <!--fig:arms-kinded-->153<!--/fig--> kinded arms. `docs/input.md` is the model,
   `node tools/oracle/kinds.js` the original's side; say which measurement you quote.
6. **A screen's strings are part of its specification.** A `L2.eng` group with one consumer
   *is* that screen's vocabulary; draw its words from the group, with our transcription as
   the fallback. C133.
7. **Write short.** A comment carries the function, the address and the fact. A report
   carries findings, numbers and what you left alone. Density, never omission: keep every
   piece of evidence, drop the prose around it. `node tools/review/prose.js` lists the
   filler phrases; a post-edit hook cuts them from Markdown, and hands back any Rust
   comment that argues (bold, shouting, "X, not Y") for you to restate as a fact. `docs/agents.md` *The prose pass*. Files too: a test
   file under 300 lines, a source under 500, a new file under 300; a post-edit hook says
   when you are over, and `docs/agents.md` *File size* names the split script.

## Where to look

| Before you… | Read |
|---|---|
| need to know what the game does | `docs/rules.md` — mechanics in plain language, real numbers. Start here |
| wonder whether the game already answers you | `Readme.txt` in the install — the v1.03 errata, which beats the manual; `docs/mechanics.md` says what it settles |
| want a lead on any screen, panel, message or refusal | `docs/formats/eng.md` §5 — all 317 `L2.eng` groups mapped to mechanic and drawing code |
| draw a panel, or wonder what it should say | `docs/formats/eng.md` §5 the other way; rule 6. Check `line_text`/`Pen::eng` call sites, not prose, for what already reads the file |
| run any command | `docs/environment.md` |
| work on a file format | `docs/formats/` — a `[V]` there is a claim; a wrong one *produces* defects (C124) |
| read or name the binary | `docs/symbols.md`; `docs/name-leads.json` holds unverified leads |
| find what the tools know about one function | `node tools/oracle/dossier.js <addr\|name>` before reading any decompilation |
| touch the battle simulation | `docs/battle.md` (read the section you need; it is 156 KB) |
| touch the kingdom economy | `docs/kingdom.md` (177 KB; by section) |
| touch anything moving on the campaign map | `docs/armies.md` — armies, merchants, transports are one array `g_units`, told apart by a type byte |
| touch AI lords, alliances or messages | `docs/diplomacy.md` |
| wonder whether a mechanic has been looked at at all | `docs/mechanics.md` |
| find a rule odd, or want to "fix" one | `docs/bugs.md` first — the original's defects and which we keep |
| draw a screen | `docs/screens.md` (campaign map), `docs/screens-county.md` |
| write anything that must stay deterministic | `docs/netcode.md` — no floats where order matters, seeded PRNG, no hash-order iteration; presentation state stays out of the digest |
| add or change a rule, or anything a mod overrides | `docs/modding.md` |
| revisit an architectural choice | `docs/decisions.md` — grep by C-number; never read whole |
| spawn or coordinate agents | `docs/agents.md` — *Briefing an agent* and *Ablate the line* sections |
| ask what state the project is in | `docs/status.html` |
| wonder what we are building, or argue with the order | `docs/plan.md` |
| wonder what is left or in flight | `docs/work.json`; `node tools/pm/work.js --status` / `--check` / `--html`. The lead is its only writer |
| have the original in front of you and five minutes | `docs/oracle-requests.md` |
| wonder what the game does that we do not | `docs/arms.json` (input, checked by `arms.rs`), `docs/audio.json` (sound, checked by `sfx.rs` and `sounds.js --check`), `docs/draws.md` + `docs/draws-map.md` (draw calls, hand-marked and therefore rotting). Read the second column, not only the count: fifteen sound triggers carry 541 of 674 files. C143 |
| wonder whether a documented number is true | `docs/audit.md` |

**Networking is the one place the original is not the authority.** Its multiplayer sync is
the reason for the rewrite; `docs/netcode.md` argues on its own, *What the original
states the exception.

## The idea that shapes everything

`Lords2.exe` is an **oracle, not a target**: fixed base `0x400000`, no ASLR, live state
readable from another process. Two of our implementations agreeing proves only that we
ported one misunderstanding faithfully; evidence is self-verifying invariants in the data
and the binary. Where they disagree with us, they are right.

**Search for prior art before reverse-engineering anything** — C5, the largest waste so far.

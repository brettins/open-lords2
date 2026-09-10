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
5. **If we implement a feature, find its equivalent in the binary's functions.** Rule 4's
   other half: 4 governs what we may *claim*, this governs what we may *build*. Name the
   function the behaviour reproduces, in the code, beside it — not just the screen's painter.
   *"We could not find it"* is a finding to report, not a licence to invent. Measured, and
   the number is generated rather than typed: we reproduce
   **<!--fig:arms-reproduced-->154<!--/fig--> of <!--fig:arms-live-->178<!--/fig-->** live
   input arms, and every miss was a behaviour nobody had looked for.
   `docs/decisions.md` C61, `docs/arms.json` for the inventory.

## Where to look

| Before you… | Read |
|---|---|
| **need to know what the game actually does** | **`docs/rules.md` — the mechanics in plain language, with the real numbers. Start here; everything else is written to help you *find* things in the binary rather than to explain them.** |
| **wonder whether the game already answers your question** | **`Readme.txt` in the install — the v1.03 patch's rules errata, with manual page references. It is the game correcting its own manual and it post-dates it, so it wins wherever they disagree. A first-class oracle alongside `L2.eng`; `docs/mechanics.md` says what it settles.** |
| **want a lead on any screen, panel, message or refusal** | **`docs/formats/eng.md` §5 — every one of `L2.eng`'s 317 string groups mapped to the mechanic and the code that draws it. Index 0 of a group is a label the game wrote about itself, so this is 317 self-written summaries with the function beside each.**<br>**And a group with one consumer is that screen's *vocabulary*, not just a naming lead.** We read these panels' numbers out of the binary and then wrote their words ourselves — nine English captions on the castle screen where the original fetches group 71, `"TOTAL MEN"` where it fetches 8/72, `"NOT SIMULATED"` over three products it already had. The ration panel is now wired through its own group; **the tax, population and happiness panels are not.** If a screen has a group, draw its words from the group. |
| run any command | `docs/environment.md` |
| work on a file format | **`docs/formats/` — and treat a `[V]` here as a claim, not a fact. These documents are an *input* to the code, not only a record of it: a wrong `[V]` does not fail to help, it **produces** the defect, through a careful person who checked the reference. `maps-layers.md` §5.5 said `Terrain_Set`'s variant parameter was dead — *"all sixteen call sites pass zero"* — and there are twenty-four, one of which computes it; the wheat never grew because of that sentence. `docs/decisions.md` C124. The correction log warns that it is believed too hard; these are believed just as hard and carry no such warning.** |
| read or name the binary | `docs/symbols.md` |
| touch the battle simulation | `docs/battle.md` |
| touch the kingdom economy | `docs/kingdom.md` |
| touch anything that moves on the campaign map | `docs/armies.md` — armies, merchants and transports are one array, `g_units`, told apart by a type byte |
| touch the AI lords, alliances or messages | `docs/diplomacy.md` |
| wonder whether a mechanic has been looked at **at all** | `docs/mechanics.md` — the inventory, written to be read by someone who has played the game |
| **find a rule odd, or be tempted to "fix" one** | **`docs/bugs.md` — the original's defects, which of them we reproduce on purpose, and what a switch would cost. Check it before correcting anything that looks wrong.** |
| draw a screen, or wonder what the original showed | `docs/screens.md` (campaign map), `docs/screens-county.md` |
| write anything that must stay deterministic | `docs/netcode.md` |
| add or change a rule, or anything a mod overrides | `docs/modding.md` |
| revisit an architectural choice | `docs/decisions.md` |
| spawn or coordinate agents | `docs/agents.md` |
| ask what state the project is in | `docs/status.html` |
| **wonder what we are building, or argue with the order** | **`docs/plan.md` — the goal, what would falsify it, and what is in flight** |
| **have the original in front of you and five minutes** | **`docs/oracle-requests.md` — the ten questions four hundred turns proved we cannot answer ourselves. An oracle request nobody is routed to is a request that does not get made.** |
| **wonder what the game does that we do not — pick one of three inventories** | **`docs/arms.json` (input arms), `docs/draws.md` + `docs/draws-map.md` (draw calls), `docs/audio-triggers.md` (sound triggers). Each enumerates the original's side and marks ours, in both directions, so a gap is *countable* rather than remembered. Read the one for the layer you are touching before adding to it. **And read the second column, not only the count**: sixteen of the 134 sound triggers carry 543 of the 771 files, because one of them is a table lookup and the rest take a constant. A 1:1 count weights every row equally and a player does not.** |
| wonder whether a documented number is true | `docs/audit.md` |

**`docs/netcode.md` binds code you might not think of as networking.** Deterministic
lockstep means the *simulation* has constraints: no floats where ordering matters, a
seeded PRNG frozen in-tree, and no iteration whose order depends on hashing. Honouring
those while the simulation is being written is nearly free; retrofitting them is not.

**Networking is the one place the original is not the authority.** Everywhere else, where
our engine and `Lords2.exe` disagree, the binary is right. The original's multiplayer sync
is the *reason for the rewrite* — it is the defect being replaced, not a model — so an
argument in `docs/netcode.md` has to stand on its own reasoning, and "the original did it"
is evidence of nothing there but what shipped. That exception was implicit until it sent an
agent in the wrong direction; the section *What the original actually did* in `docs/netcode.md` states it.

## The idea that shapes everything

`Lords2.exe` is an **oracle, not a target**. We don't patch it — we query it. It has no
ASLR and a fixed base of `0x400000`, so its addresses are stable and its live state is
readable from another process.

Two of our own implementations agreeing proves only that we ported our own
misunderstanding faithfully. Real evidence comes from self-verifying invariants in the
data, and from the original binary. Where they disagree with us, they are right.

**Search for prior art before reverse-engineering anything.** Not doing so has already
been the single largest waste of effort on this project — see `docs/decisions.md`, C5.

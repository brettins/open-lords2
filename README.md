# open-lords2

An open reimplementation of the **Lords of the Realm II** engine (Sierra/Impressions, 1996),
in the spirit of OpenXcom: our own engine, reading the original game's data files.

**You must own a copy of the game.** No assets are distributed here and none ever will be —
see [NOTICE](NOTICE). The engine reads an installation you supply and never writes to it.

```
l2-game <game dir> [--mods <dir>]
```

---

## Where this actually is

Early. It runs, and it is nowhere near the whole game. The honest inventory:

| | state |
|---|---|
| Screens | the front end and its thirteen setup pages, the campaign map, the four county panels, the village, the job popup and the conquest screen are drawn; **nineteen more exist only as shells** — right artwork, right hotspots, contents unbuilt. `Screen_Draw` has 39 arms and 35 now have a named painter |
| Kingdom economy | the full end-of-season pipeline: tax, rations, health, happiness, grain, herds, industry, migration, population, scoring |
| Battle | figures, units, formations, movement, pathfinding, melee, missile resolution, the battle AI |
| Campaign map | scrolling viewport, two zooms, the minimap, county tinting |
| Scenarios | the England turn-one fixture imports and reproduces. *(A clean install ships **no** saves — see `docs/decisions.md` C23 for what believing otherwise cost)* |
| Mods | rules are data; a mod can override the tables |
| Multiplayer | deterministic lockstep, tested — no matchmaking or UI |
| **Castle designer** | **not started.** One of the game's signature features |
| **Sieges** | built, both halves — and the 14 siege battle AI handlers are reachable at last. The castle's *layout* on the battlefield is ours, not the original's, and says so |
| Merchants, diplomacy, armies | documented in detail, implemented barely or not at all |

**<!--fig:tests-->1,928<!--/fig--> tests pass**, and roughly a third of them assert things
read out of the original binary rather than out of our own heads.

Two honest caveats. The one shipped save we test against exercises a narrow slice of the
rules — every county in it sits at tax rate 0 with a well-staffed herd — and that is exactly
how two wrong rules once survived **932 passing tests** *(that figure is deliberately frozen:
it is the size of the suite at the moment `docs/decisions.md` C26 describes, and correcting
it to today's number would destroy the thing it records)*. And of the executable's
<!--fig:binary-functions-->2,452<!--/fig--> functions, we have identified
**<!--fig:functions-->968<!--/fig-->**, plus <!--fig:globals-->563<!--/fig--> globals; most
of the program is still dark.

## The idea that shapes everything

`Lords2.exe` is an **oracle, not a target.** We do not patch it or link against it — we ask
it questions. It has no ASLR and a fixed image base of `0x400000`, so its state lives at
stable addresses and can be read from a live process while it runs.

This matters because two of our own implementations agreeing proves only that we ported our
own misunderstanding faithfully. Evidence comes from the original binary and from
self-verifying invariants in the data. Where they disagree with us, they are right.

Every claim in `docs/` is marked **verified**, **decompiled** or **inferred**, and
[`docs/decisions.md`](docs/decisions.md) keeps a numbered log of the times we got it wrong —
including the several where a player's offhand memory of the game overturned a confident
reading of the disassembly.

## Documentation

The `docs/` tree is the real substance of this project; the code is downstream of it.

| | |
|---|---|
| [`docs/rules.md`](docs/rules.md) | **How the game works, in plain language, with the real numbers.** Start here |
| [`docs/mechanics.md`](docs/mechanics.md) | What has been looked at and what has not — written to be read by someone who has *played* the game, so they can point at what is missing |
| [`docs/formats/`](docs/formats/) | The file formats: PL8 sprites, maps, saves, strings |
| [`docs/symbols.md`](docs/symbols.md) | Named functions and globals in the executable |
| [`docs/kingdom.md`](docs/kingdom.md) · [`docs/battle.md`](docs/battle.md) · [`docs/armies.md`](docs/armies.md) · [`docs/diplomacy.md`](docs/diplomacy.md) | Subsystems, traced |
| [`docs/decisions.md`](docs/decisions.md) | Architecture decisions, and the correction log |
| [`docs/netcode.md`](docs/netcode.md) | Why the simulation has no floats and a frozen PRNG |

## Layout

| Path | |
|---|---|
| `crates/l2-formats/` | dependency-free decoders for the game's file formats |
| `crates/l2-sim/` | the battle simulation |
| `crates/l2-kingdom/` | the turn-based economy |
| `crates/l2-scenario/` | scenario and save loading |
| `crates/l2-mods/` | the rule-override layer |
| `crates/l2-net/` | deterministic lockstep networking |
| `crates/l2-view/` | rendering and windowing |
| `crates/l2-game/` | the application: screens, input, the turn loop |
| `tools/` | PE inspection, format decoders, live-process probes, the Ghidra oracle |

Everything below `l2-view` is dependency-free and deterministic on purpose: no floats where
ordering matters, a seeded PRNG frozen in-tree, and no iteration whose order depends on
hashing. See [`docs/netcode.md`](docs/netcode.md) — it binds code you would not think of as
networking.

## Building and testing

```powershell
cargo test                                     # the whole suite; no game install needed

$env:LORDS2_DIR = 'F:\games\Lords of the Realm II'
cargo test -- --nocapture                      # + corpus validation against a real install
```

The corpus tests **skip** when `LORDS2_DIR` is unset, so a checkout without the game still
has a real suite to run.

## Contributing

The most valuable thing anyone can offer is not code — it is **memory of playing the game**.
[`docs/mechanics.md`](docs/mechanics.md) exists to be read by a player and contradicted. Four
subsystems in this project were found only because someone mentioned them in passing, and a
remark that ale seemed unfamiliar turned up two mechanics nothing here had recorded.

If you remember a mechanic that has no row in that document, it has never been looked at.
Please open an issue and say so.

## Licence

MIT — see [LICENSE](LICENSE) and [NOTICE](NOTICE).

This is an independent reimplementation containing no code, art, sound or data from the
original. Lords of the Realm II is copyright Sierra On-Line / Impressions Games and its
present rights holders, who are not affiliated with this project and do not endorse it.

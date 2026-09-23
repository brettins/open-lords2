# open-lords2

An open reimplementation of **Lords of the Realm II** (Sierra/Impressions, 1996), in the
spirit of OpenXcom: our own engine, reading the original game's data files.

**You must own a copy of the game.** No assets are distributed here and none ever will be —
see [NOTICE](NOTICE). The engine reads an installation you supply and never writes to it.

## What works

A single-player game plays from the front end to victory or defeat:

- **Kingdom** — the whole end-of-season economy: taxes, rations, grain, cattle, weather,
  industry, wages, ale, health, happiness, births, migration, events, revolts.
- **Armies** — raising, the armoury, mercenaries, marching, splitting, garrisons, foraging,
  taking counties, fog of war.
- **Battles and sieges** — field battles with the battle AI, auto-calculated results,
  castle building, laying siege, storming, moats and burning oil.
- **AI lords** — farming, war, sieges, diplomacy and alliances, trade.
- **Merchants** — routes, trading and the neutral market.
- **Screens, sound and films** — the campaign map, county panels, village, messages,
  options, tips, music, narration and the original's films.
- **Saving** — save, load and autosave.

[`docs/status.html`](docs/status.html) grades every feature against the original, with the
evidence for each.

## What does not, yet

- **Network play** — deterministic lockstep is built and tested; the lobby and game over a
  network are in progress.
- A few gaps are named in the status page: closing archers in battle, one AI attack, some
  narration and cursor pictures.

## Playing

Build it and point it at your install:

```
cargo run --release -p l2-game -- <install dir> [--mods <dir>]
```

A launcher that finds the install for you is on its way. Saves live in
`%APPDATA%\open-lords2\saves`.

## How it is built

`Lords2.exe` is an oracle, not a target: we read the original binary to answer what the game
does, and where it disagrees with us it is right. Of its
<!--fig:binary-functions-->2,452<!--/fig--> functions we have named
**<!--fig:functions-->1,333<!--/fig-->**, and **<!--fig:tests-->3,043<!--/fig--> tests pass**.

## Playing

`powershell -File tools/run/package.ps1` builds a release and writes
`dist/open-lords2-<commit>.zip`: the executable, LICENSE, NOTICE and
[`tools/run/README-player.txt`](tools/run/README-player.txt), which says where the game
looks for the install, where saves and crash logs go, and the keys. No game file is ever
in it (`crates/l2-testkit/tests/package_contents.rs`).

## Contributing

Developers start at [CLAUDE.md](CLAUDE.md), the index to `docs/`. The most valuable thing
anyone can offer is **memory of playing the game**: [`docs/mechanics.md`](docs/mechanics.md)
exists to be read by a player and contradicted. If you remember a mechanic it does not
list, please open an issue.

## Licence

MIT — see [LICENSE](LICENSE) and [NOTICE](NOTICE).

This is an independent reimplementation containing no code, art, sound or data from the
original. Lords of the Realm II is copyright Sierra On-Line / Impressions Games and its
present rights holders, who are not affiliated with this project and do not endorse it.

# open-lords2

An open reimplementation of **Lords of the Realm II** (Sierra/Impressions, 1996), in the
spirit of OpenXcom: our own engine, reading the original game's data files. Windows and
Linux.

**You must own a copy of the game.** No assets are distributed here and none ever will be —
see [NOTICE](NOTICE). The engine reads an installation you supply and never writes to it.

## Download and play

1. Install Lords of the Realm II (the GOG and Steam editions both work).
2. Download open-lords2 from [Releases](https://github.com/brettins/open-lords2/releases): a numbered release, or
   **Nightly**, rebuilt each night from the newest code that passed its tests.
3. Unpack it and run `l2-game` (`l2-game.exe` on Windows). It finds most installs by
   itself and asks for the folder otherwise.

`README.txt` in the download says where saves, settings and crash logs go, and lists the
keys. Saves live in `%APPDATA%\open-lords2\saves` on Windows and
`~/.local/share/open-lords2/saves` on Linux.

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

## Network play (new)

The original's multiplayer is what this project set out to fix: our games run in
deterministic lockstep instead. It is new and still being tested, so expect rough edges
and please report them.

*Multiple players* opens the lobby. The host presses *Host a game* and the lobby shows the
address others type (its LAN address and port 4712). Windows asks once whether to let the
game through its firewall: allow it on private networks, or open TCP port 4712. A peer who
leaves or goes silent stops the game with their name; save it to go on alone against the
computer.

## What does not, yet

A few gaps are named in the status page: closing archers in battle, one AI attack, some
narration and cursor pictures.

## Building from source

With stable Rust ([rustup](https://rustup.rs)):

```
cargo run --release -p l2-game -- "<your Lords of the Realm II folder>"
```

On Linux, install the audio and windowing headers first (Debian/Ubuntu names):
`libasound2-dev libxkbcommon-dev libwayland-dev libx11-dev pkg-config`.
[CONTRIBUTING.md](CONTRIBUTING.md) covers the tests.

## How it is built

`Lords2.exe` is an oracle, not a target: we read the original binary to answer what the game
does, and where it disagrees with us it is right. Of its
<!--fig:binary-functions-->2,452<!--/fig--> functions we have named
**<!--fig:functions-->1,464<!--/fig-->**, and **<!--fig:tests-->3,414<!--/fig--> tests pass**.

## Contributing

The most valuable thing anyone can offer is **memory of playing the game**:
[`docs/mechanics.md`](docs/mechanics.md) exists to be read by a player and contradicted.
If you remember a mechanic it does not list, or something plays differently from the game
you remember, please open an issue. For code, start at [CONTRIBUTING.md](CONTRIBUTING.md).

## Licence

MIT — see [LICENSE](LICENSE) and [NOTICE](NOTICE).

This is an independent reimplementation containing no code, art, sound or data from the
original. Lords of the Realm II is copyright Sierra On-Line / Impressions Games and its
present rights holders, who are not affiliated with this project and do not endorse it.

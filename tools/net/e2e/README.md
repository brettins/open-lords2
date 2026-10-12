# Network games, end to end (C261)

Real `l2-game` processes on one machine, in a headless KWin (`lords2-e2e`), each with its own
saves folder and install id, played through `LORDS2_REMOTE` (`crates/l2-game/src/remote.rs`):
clicks at canvas coordinates, keys, screenshots through `Machine::present`, a status file and
a hash every 25 ticks. A scenario kills, closes, freezes and relaunches them and checks every
seat still agrees.

    cargo build -p l2-game --bin l2-game   # and -p l2-relay for the relay runs
    python3 tools/net/e2e/b_close_rejoin.py

Output (screenshots as `shots/*.png`, each seat's stderr and saves) goes to `~/.cache/lords2-e2e/`,
outside the tree: the census refuses a save in it. Coordinates are
the 640 × 480 canvas; the halt box grows with its text, so it is answered by keyboard.

`m_resume.py` (C263): the host saves a three-seat game, then continues it from *Continue a
saved game*; Matilda comes back on her install, Eleanor is new and gets John's realm by a
click. A host opened from the lobby listens on the game's port, 4712. `LORDS2_E2E_BIN` points
the scripts at another build (a worktree's own target directory), `LORDS2_E2E_OUT` elsewhere.

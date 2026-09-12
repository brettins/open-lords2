# Handoff — `agent/castle-battle-layout`

Stopped mid-task on a coordinator instruction (token budget). **The code is
green**: `cargo test --workspace` is **2,609 passed, 0 failed, 3 ignored** both
with `LORDS2_DIR=F:\games\Lords of the Realm II` and with it unset. What is
*not* done is the paperwork — see *Next, in order*.

Branch is based on `main` (`83af07e`) with `agent/siege-sheet` (`6b83374`)
merged in; that merge had nine conflicts and they are resolved in commit
`abde7aa` (both branches had independently added the siege palette — the merged
form keeps `l2_view::scene::Ground`, which is the superset, and drops
`BattlefieldScreen::siege`).

---

## 1. What `stnfield.pl8` actually is — established, with addresses

`Battlefield_BuildCastle` (`0x0047C4BA`) opens the file itself, from a
process-global working directory, with no parameter from its caller:

```c
Restore_WorkingDir();
File_ReadChunk(s_Q_Q_Qbatfield_pl8_004D9103 + index * 0x0E + 5, DAT_004EABEC, 1000, 0);
off = buf[c*0x20+0x0C] | buf[c*0x20+0x0D]<<8 | buf[c*0x20+0x0E]<<16;   /* frames     */
File_ReadChunk(name, DAT_004EABEC, 0x1900, off);
/* and FUN_0047CEC1 re-reads the same directory for buf[c*0x20+0x1C .. +0x1E] */
```

* The string table at `0x004D9103` is 14-byte stride, name at `+5`:
  `batfield.pl8`, **`stnfield.pl8`**, `batfiel2.pl8`, `stnfiel2.pl8`. Index **1**
  campaign, **3** skirmish (`DAT_0057A0F0`).
* **The `castle * 0x20` stride is two 16-byte PL8 frame records.** So the
  "directory" is an ordinary PL8 directory and castle *c* is PL8 frames `2c`
  (frames layer) and `2c + 1` (structure layer). Verified against the shipped
  file: `Stnfield.pl8` is **64,168** bytes = 8 header + 160 directory + 10 ×
  6,400, ten frames of 80 × 80, uncompressed. `castle` is `g_castleLevel`, 0..4.
* **Verified**, not inferred — the decode is cross-checked below.

### The frame layer (`+0x0C`)

Pass 1: `terrain = (b == 0xEE) ? 0x0B : 1`.
Pass 2, per cell:

| byte | routine | effect |
|---|---|---|
| `0xED` | `FUN_0047DC9C` | `frame = 0xED`, tileset slot 0 — the only escape that stays on slot 0 |
| `0xEE` | `FUN_0047DCCE` | 49-variant water auto-tile (`0x004D7610`), slot 1, `flags \|= 0x10`, `surface = 2`, approach score zeroed |
| `0xEF` | `FUN_0047E1DC` | `frame = rand & 0x0F`, slot 1 |
| else | — | `frame = b`; `elevation = table[b*2]`; `if (table[b*2+1] == 0) flags \|= 0x10`; then the code ladder |

Table is `0x004D7B80` (stone, `1 < g_castleLevel`) or `0x004D7D80` (wood), file
offsets `0xD5D80` / `0xD5F80`. Already in-tree as
`l2_sim::siege::STRUCTURE_STONE` / `_WOOD` (siege-sheet's work).

Code ladder (`l2_sim::siege::code`), and note six of the eight arms **assign**
`flags` rather than OR, so a structure cell is never impassable via `0x10`:

| code | writes |
|---|---|
| 5 | `flags = 4`, `elev = 3` |
| 6 | `surface = 6`, `flags = 8`, `flags2 \|= 0x80`, `elev = 4` stone / **1** wood; first one seeds `DAT_00553274` at `off − 0x280` (one row N) |
| 7 | `surface = 7`, `elev = 2` |
| 8 | `surface = 8`, `flags = 0x24`, `elev = 1`; first seeds `DAT_00553EE4` at `off − 0x278`; fifth seeds `DAT_0053E9D4` at `off − 0x4F0` |
| 9 | `surface = 0x0B`, `flags = 0x40`, `elev = 0` |
| 10 | `surface = 0x0E`, `flags \|= 4`, `elev = 0` |
| 11 | `flags = 4`, `elev = 1` |
| 12 | `flags = 4`, `elev = 2` |

`DAT_0053E9D4` is **written and never read** anywhere in the binary — dead in
this build. Not modelled.

Then `FUN_0047E230` — six flood passes, each looping to a fixed point:

| pass | spreads |
|---|---|
| `FUN_0047E263` | keep (6) over surface `0x0E` and over elevation > 3 |
| `FUN_0047E387` | bailey (5) over low ground beside the keep door / a 7 / another 5 |
| `FUN_0047E52D` | rampart walk (4) over raised ground beside a 5 or another 4 |
| `FUN_0047E668` | apron (3) over flat ground within **eight** of a 4 or another 3 |
| `FUN_0047E7B2` | open field (1), seeded at cells `0` and `0x18B0` = **(0, 0) and (0, 79)** |
| `FUN_0047E926` | leftovers: 3 beside water, 6 beside the keep |

Helpers: `Cell_NeighbourHasSurface` (`0x00496F72`) = four orthogonals;
`FUN_00497024` = eight; `FUN_0049719E` = *all four* orthogonals are water (off-map
counts as water); `FUN_00497247` = any orthogonal carries flag `8`.
`FUN_0047EA42` exists but is **not** called by `FUN_0047E230` — left alone.

### The structure layer (`+0x1C`) — `FUN_0047CEC1`

Markers, not tiles. A marker is a 2 × 2 block: byte `+0` is `0x04` (side 0) or
`0x0F` (side 4); byte `+1` is the kind; the index is written underneath.

* kind == marker → a **deployment slot**, index `buf[+2] − 0x40` clamped 0..11
  (`FUN_0047D6B0`), stored `(x + 2, y)`;
* side 0's other kinds → 4-bit index from `buf[+0x50 ..= +0x53]`, each `== 7`
  worth 8/4/2/1 (`FUN_0047D5EC`), stored `(x + 2, y + 2)`;
* side 4's other kinds → 2-bit index from `buf[+0x50], buf[+0x51]`
  (`FUN_0047D4B8`).

The walker **zeroes each byte as it consumes it**, so it must run on a copy.

| target | address | source |
|---|---|---|
| `deploy_side0[12]` | `0x00553150` | marker `0x04`, kind `0x04` — `Deploy_SlotForUnit` (`0x0048169E`) |
| `deploy_side4[12]` | `0x005531B0` | marker `0x0F`, kind `0x0F` |
| `wall_slot[0..3][16]` | `0x00554180`, `+0x80`, `+0x100`, `+0x200` | marker `0x04`, kinds `0x40`, `0x41`, `0x44`, `0x47` (group 3 at `0x554300` never written) |
| `castle_approach[row][4]` | `0x0055CD90 + row*0x20` | marker `0x0F`, kinds `0x40`→0, `0x41`→1, `0x43`→2 (one point into all four lanes, `(x, y−2)`), `0x47`→3, `0x44`→5; **row 4 never written** |

`castle_ref` is `castle_approach[2][0]` and `staging` is `castle_approach[3]` —
our `AiField` has both as separate fields aliasing the same memory in the
original; they are filled consistently.

**The decode checks itself.** `Deploy_SlotForUnitSiege` (`0x004816F9`) maps a
troop type only to slots **0, 1, 4 and 8** of the garrison's twelve, and every
one of the five shipped layouts fills exactly those four and no others. Nothing
in the decode arranges for that.

### What the five castles actually contain (measured)

| level | family | moat (`0xEE`) | keep (6) | curtain (8) | drawbridge (9) | cells at elev 2 |
|---|---|---|---|---|---|---|
| 0 | wood | **no** | 1 | 4 | 0 | 157 |
| 1 | wood | **yes** | 1 | 8 | 0 | 358 |
| 2 | stone | **no** | 1 | 8 | 0 | 134 |
| 3 | stone | yes | 1 | **0** | 4 | 256 |
| 4 | stone | yes | 1 | 4 | 4 | 278 |

Two of our stand-in's premises are **wrong against the file**:

* the moat is at levels **1, 3, 4** — not "level 2 and up";
* level 3 has **no** `0x20` wall cell at all, so `wall > 0` is false there. What
  holds at every level is "there is something `smash_walls` can open" — a
  curtain block or a drawbridge.

`RAMPART_GAP_AT_BUILD = [1,0,0,1,0]` (levels 0 and 3 start breached) is
unrelated to either and was already right.

---

## 2. What is built, and where

| file | state |
|---|---|
| `crates/l2-sim/src/castle.rs` | **new.** `CastleSheet`/`CastleSheets::parse` (the directory, exactly as the builder indexes it), `build` = `Battlefield_BuildCastle`, `classify` = `FUN_0047E230`, `tables` = `FUN_0047CEC1`, `ai_field`. No IO. |
| `crates/l2-sim/src/runner.rs` | `deploy_siege_on_sheet(field, tables, …)`; `deploy_muster_on` takes `Option<&CastleTables>` and uses `castle::ai_field` when given, `siege::our_castle_ai_field` otherwise. |
| `crates/l2-sim/src/lib.rs` | `pub mod castle;` + re-exports. |
| `crates/l2-game/src/castle.rs` | **new.** A `OnceLock<CastleSheets>` — `publish_from`, `sheet(level)`, `loaded()`. This mirrors the original: the builder reads the file from a process global, and `begin_fight` is reached through six `Kingdom`-only signatures. |
| `crates/l2-game/src/game.rs` | `Assets::load` calls `castle::publish_from` through the mod `Vfs`. |
| `crates/l2-game/src/engagement.rs` | `begin_fight` uses the real layout when published, and falls back to `siege::our_castle` when there is no install. |
| `crates/l2-sim/tests/castle_layout.rs` | **new, 11 tests, all install-gated, all passing.** |
| `crates/l2-testkit/tests/census.rs` | inventory + `GATED_TOTAL` 525 → **537**. |

### Does oil / docking now happen in a real siege?

**Yes**, measured — `oil_pours_and_a_tower_docks_in_a_siege_of_a_real_castle`
runs five sieges of up to 60,000 frames on the real layouts:

```
level 0: oil 2  towers 1   (8,848 frames, decided)
level 1: oil 3  towers 0
level 2: oil 3  towers 1
level 3: oil 0  towers 0
level 4: oil 4  towers 0   (12,149 frames, decided)
```

Oil pours at four of five levels, a tower docks at two. Against
`siege::our_castle` both are **zero at every level** and provably so: the
stand-in has no cell at elevation 2 anywhere, and `DOCK_WALL_ELEVATION` is 2
exactly while `Oil_FindPourTarget` refuses below 2. Both halves of that are
asserted.

**Level 3 gets neither** and I did not chase why. It is the level with no
curtain block, so the besieger's route is the drawbridge; worth a look.

---

## 3. Test state, as last measured

* `cargo test --workspace` **with** the install: **2,609 passed, 0 failed, 3 ignored**.
* `cargo test --workspace` with `LORDS2_DIR`/`LORDS2_FIXTURES`/`LORDS2_DOS_DIR`
  empty: **2,609 passed, 0 failed**.
* `crates/l2-sim/tests/castle_layout.rs`: 11/11.
* The differential (`crates/l2-game/tests/differential.rs`, 3) and `long_game`
  ran inside those workspace runs and were green.
* **No test went red from this work.** Nothing was weakened; the two stand-in
  premises above are asserted in the *new* file against the original's shape,
  and the old `our_castle` tests still describe `our_castle` truthfully, which
  they still may because it is still the no-install fallback.

**Not run at all**: `figures.js`, `symbols_md.js --check`,
`corrections.js --check`, release-mode suite.

---

## 4. Next, in order

1. `docs/battle.md` §13.2a — replace *"the layers are not read"* with the decode
   above (directory stride, the two layers, the marker grammar, the five-castle
   table). Section 1 of this file is written to be moved there nearly verbatim.
2. `docs/decisions.md` — one **placeholder** tag (`CNEW-castlelayout`), covering:
   the layers read; the moat ladder and `wall > 0` being our ring's premises and
   not the game's; oil and docking becoming reachable; the `OnceLock` and why it
   is the original's shape. Note `CNEW-siegesheet` from the merged branch is
   still a placeholder too — the lead numbers both.
3. `docs/symbols.json` — name `FUN_0047CEC1` (`Battlefield_ReadStructureLayer`),
   `FUN_0047E230` and its six passes, `FUN_0047D474`/`D4B8`/`D544`/`D5EC`/`D6B0`.
   Then `tools/oracle/decompile-all.ps1`, then `symbols_md.js --check`.
4. `docs/features.json` — regrade. `castle-battle-layout`'s row, and
   `battlefield-picture`'s gap (the merge kept main's *"missiles and engines are
   not drawn"* over siege-sheet's *"occasional jumps"*; check which is current).
5. `figures.js` (the test count moved 2,594 → 2,609), `corrections.js --check`,
   census (already updated, green).
6. `AiField`'s doc comments still say `[I]` / *"`Battlefield_BuildCastle`
   unread"* on `castle_approach`, `castle_ref`, `staging`, `wall_slot`,
   `castle_objective`. They are `[V]` now on the `castle::ai_field` path and
   `[I]` only on `our_castle_ai_field`'s. Same for
   `runner::ai_field_for`'s header.
7. Level 3: no oil, no dock. Find out whether that is the layout or our AI.

## 5. Things that did not work / do not repeat

* **Do not thread the sheet through `engagement`.** I started to and backed out:
  `begin_fight` is reached through `resolve`, `resolve_fought`, `resolve_siege`,
  `run_siege_phase`, `fight` and `take_the_field`, ~30 call sites, all carrying
  `&mut Kingdom` or `&mut Game`, and `Game` does not own `Assets`. The original
  does not thread it either — `Battlefield_BuildCastle` reads a global. The
  `OnceLock` is the faithful shape, not a shortcut.
* **`l2-sim` must not read files.** `castle::build` takes bytes. The parse lives
  there because the *directory arithmetic* is the builder's; the `read` is
  `l2-game`'s.
* The census classifies a gate by searching the test's own body and then its
  file-local helpers **by name**. A `macro_rules!` whose body only says
  `l2_testkit::skip!` counts as `other` even when the `fn` it calls uses
  `install_dir()`. Put `l2_testkit::install_dir()` inside the macro — hence
  `layouts(l2_testkit::install_dir())`.
* `GATED_TOTAL` was already one low before I touched it (525 against a real 526);
  I set 537, not 536. The census prints the correct `INVENTORY` when the *list*
  disagrees but only asserts the total separately, so a stale total hides.
* `BattleRunner::step()` returns `()`, not an `Option`. Poll `conclusion()`.

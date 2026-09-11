# HANDOFF — three campaign-chrome defects a player reported

**Branch:** `worktree-agent-ad8d67ce0011058fe`, branched from `ff4a7c5`, **fast-forwarded to
`main` = `5338fe7`** ("Merge the mercenary marker, as C150"). The branch point was **37 commits
behind**; every finding below is against `5338fe7`.

**The job, in one line:** verify and fix three defects — (1) drop-down menu text sitting high in
its scroll, (2) what the menu-bar shield row means, (3) the setup shield colour not surviving
into the game.

**No code was changed.** Everything below is investigation. The tree is clean apart from this
file. Baseline measured before the stop: **2,203 passing, 0 failed**, with
`LORDS2_FIXTURES="E:\dev\lords2-fixtures" LORDS2_DIR="F:\games\Lords of the Realm II"
cargo test --workspace` — i.e. unchanged from the number in the brief, as expected for a
no-change tree.

---

## Defect 1 — menu text high in the scroll: **ALREADY FIXED on `main`. Do not re-fix.**

**[V]** Fixed by commit `36abe2b` *"The fonts load. The chrome never asked for them."*

* The fix is in `crates/l2-view/src/chrome.rs`, `Chrome::draw_box`, the `open_top = set == 2`
  branch (around line 677). Border **set 2** omits the top rail entirely and fills its interior
  **from the box's own `y`**, one row taller than `Ui_DrawBox`:
  ```c
  Ui_DrawBoxBorder(2, x, y, cols, rows);
  Ui_DrawBoxInterior(x + 0x10, y, cols - 2, rows - 1);   /* not y + 0x10, not rows - 2 */
  ```
  That is `FUN_00409429`, the drop-down's only plate call, from `FUN_0040C725`.
* The player's quote is reproduced **verbatim** in that function's doc comment, so this is
  unambiguously the same report.
* Test: `crates/l2-game/tests/chrome_text.rs::the_drop_down_plate_has_no_top_rail`, with a
  stated ablation (restore `r > 0` to the interior test). It is an *equality* against the
  original's own second call, not a threshold.
* **Diagnosis worth keeping [V]:** neither half of the complaint was a row-pitch error. The item
  pitch is a data column in `g_menuBarItems`' item tables (0, 20, 40, …) and every caption is
  where `Eng_DrawString(group, index, x + 0x10, item.y + y + 0x20, …)` puts it. A 16-pixel top
  rail that should not have existed ate the top of the block; the sixteen pixels it occupied are
  what made the space under the last row look unbalanced.

### The one thing left to do on defect 1 (small, unstarted)

`crates/l2-game/src/screens/menubar.rs`, module header, section **"# What is ours"**, still says:

> *"What remains ours is that our `Pen::window` only models two of the original's three border
> sets, so set 2 draws with set 1's artwork. Recorded rather than faked."*

**That sentence is now false** — `draw_box` models set 2. `chrome.rs` even cites this exact
sentence as the record that was accurate and did not cause the work to happen. Leaving it is
precisely the `CLAUDE.md` "a document is an input to the code" hazard: the next reader will
believe set 2 is unimplemented. **Delete or rewrite that clause.** No test change needed.

---

## Defect 2 — the shield row: **the player is RIGHT, and it STILL REPRODUCES.**

### What the original does — `Screen_DrawMenuBar` (`0x00419C78`) **[V]**

Read directly out of `tools/oracle/decomp/00410000.c` line 3654 (the corpus lives in the **main
checkout**, `E:\dev\lords2\tools\oracle\decomp`, not in the worktree — it is gitignored):

```c
if (g_battlePhase == 0) {
  local_c = 0;
  for (local_8 = 1; local_8 < 6; local_8 = local_8 + 1) {
    if ((g_realms[local_8].strength != 0) && (g_realms[local_8].aiStep < 999)) {
      Pl8_DrawFrame(g_miscCtySheet, g_realms[local_8].shieldIndex + 0x55,
                    local_c * 0x10 + 0x10e, 4);
      local_c = local_c + 1;
    }
  }
}
```

**The guard has two clauses and we implement only the first.** `strength != 0` is "alive";
**`aiStep < 999` is "has not finished its turn"**. `local_c` increments only when a banner is
drawn, so **the row compacts left** as realms drop out.

### Why `aiStep == 999` means "turn over" **[V]**, four independent sites

| function | addr | what it does |
|---|---|---|
| `Turn_End` | `0x0043AC23` | the sidebar's End Turn button: `g_realms[g_localPlayer].aiStep = 999` |
| `Turn_BeginPlayersTurn` | `0x0049B6D3` | top of phase 4: every realm's `aiStep = 0`, or 999 if `strength == 0` |
| `Turn_AllRealmsDone` | `0x0049B762` | phase 4's exit condition — counts realms with `strength != 0 && aiStep < 999` into `g_realmsActive` |
| `FUN_004479E9` | `0x004479E9` | the **network** path: a remote seat ending its turn writes `g_realms[DAT_005158e8].aiStep = 999` |

**The clinching detail [V]:** `Turn_End` also sets `DAT_0056D6A0 = 1`, and
`Screen_DrawMenuBar`'s own repaint guard is
`gold != DAT_004E59C4 || DAT_0056D6A0 != 0`. **The only reason `Turn_End` raises that flag is to
force the bar to repaint so the shield disappears.** `FUN_004479E9` raises it too, for the same
reason on the remote path. That is the intent stated by the binary, not inferred from a name.

### Two siblings that read the same flag **[V]**

* `Screen_DrawEndTurn` (`0x0041A734`): blits the strip unconditionally, then
  `if (g_realms[g_localPlayer].aiStep < 999) Ui_DrawCentred(4, 0, …)` — the **"End turn" caption
  is simply not put back**. We already reproduce this, gated on `turn::turn_in_flight`.
* `FUN_0041A639` (`0x0041A639`): the turn **timer** is gated on
  `DAT_0055403C < 1 || g_realms[g_localPlayer].aiStep == 999`. Not reproduced.

Together these three are the original's whole "the turn is being processed" feedback. We have
one of the three.

### Our side — where it is wrong

`crates/l2-game/src/screens/map.rs`, `fn draw_menu_bar` (~line 3650):

```rust
for id in 1..k.realms.len() {
    if !k.realms[id].in_play { continue; }          // <- ONLY the strength clause
    ...
    if c.draw_banner(canvas, slot, colour) { slot += 1; }
}
```

`Realm::in_play` **is** `strength != 0` (`crates/l2-kingdom/src/realm.rs:153`). The `aiStep`
clause is absent, so no shield ever vanishes.

**This gap is already written down, correctly, and did not cause the work to happen** —
`map.rs` ~line 3875 says *"Two other things read the same flag and we reproduce neither …
`Screen_DrawMenuBar`'s banner loop is `strength != 0 && aiStep < 999`"*. That is
`docs/agents.md`'s *"a correct explanation sitting directly above the omission it describes"*,
a third instance, and worth a `CNEW-` note.

### The designed fix (worked out, NOT written) — read this before coding

We **do** have the counter: `Realm::ai_step` (`crates/l2-kingdom/src/realm.rs:149`),
`AI_STEP_DONE = 999` (line 21).

**The trap [V], and it is why a naive `ai_step < 999` is wrong:** our turn model differs from
the original's. The original's phase 4 *is* the interactive phase — the human sits in it with
`aiStep == 1` while the AI steps in the background. Ours parks the player on the map with the
turn machine at **phase 1** and runs phases 1→7 inside one End Turn press. After a turn
completes **every** realm's `ai_step` is ≥ 999 (AI 1000, human 999 —
`l2_kingdom::ai::begin_turn` writes `AI_STEP_DONE` for humans deliberately). So a literal
`ai_step < 999` would draw **no shields at all while the player is playing** — the exact inverse
of the defect.

The mapping that is exact rather than approximate (and `map.rs` already uses it for the End Turn
label, saying so in prose): **`turn::turn_in_flight` is the human's `aiStep >= 999`.** Proposed
predicate, to live in `crates/l2-game/src/turn.rs` beside `turn_in_flight`:

```rust
/// `Screen_DrawMenuBar` (`0x00419C78`): `strength != 0 && aiStep < 999`.
pub fn realm_turn_ended(game: &Game, realm: usize) -> bool {
    // Not in flight: the map screen is the original's phase 4 before the
    // button, where every living realm's counter is < 999.
    if !turn_in_flight(game) { return false; }
    // `Turn_End` (0x0043AC23) wrote 999 the instant the button was clicked.
    if realm == game.player as usize { return true; }
    game.kingdom.realms[realm].ai_step >= l2_kingdom::realm::AI_STEP_DONE
}
```

then in `draw_menu_bar`: `if !k.realms[id].in_play || turn::realm_turn_ended(&ctx.game, id) { continue; }`.

**Do NOT use `Realm::turn_done()`** — it short-circuits on `is_human` (realm.rs:501) and would
hide the human's banner permanently.

**Do NOT "fix" `ai::begin_turn` to leave a human at 1.** I checked: it would not hang phase 4
(`all_realms_done` → `turn_done()` → `is_human` short-circuits, so it is safe), but `ai_step` is
simulation state inside the lockstep digest, and during *our* flight the human has already
pressed the button, so 1 would be the wrong value for us anyway. Smaller fix wins.

**Believed, not checked [I]:** during phases 1–3 of the flight the AI realms still carry last
turn's 1000, so all banners are absent, then reappear when phase 4 opens
(`Kingdom::tick` calls `ai::begin_turn` on `advanced_to == Some(Phase::PlayersTurn)`,
`crates/l2-kingdom/src/kingdom.rs:493`), then vanish one by one. I argued this is *also* what
the original does (its phases 5,6,7,1,2,3 all run with every counter at 999, so its bar is
empty too) — but I never watched either program do it. Verify before writing it up as `[V]`.

**Test shape I had in mind:** drive `Machine` with an `Event::Click` on End Turn, render into a
`Canvas`, and assert the banner band (`x = 270 + 16i, y = 4`, 13×16 from `Misc_cty` frame
`0x55 + colour`) loses pixels as realms finish — *ablated by deleting the `realm_turn_ended`
clause*. **Watch the trap in `docs/agents.md`**: do not compute the probe from the constant
under test, and note the brief's warning about a diff that cannot see a figure because the
panel's height also changed. A cleaner assertion is a **count of drawn banners** against a count
of realms with `!realm_turn_ended`, plus a pixel check that slot `i` is at `270 + 16i` (the
compaction), since `draw_banner` already returns `bool`.

---

## Defect 3 — setup shield colour: **single-player half ALREADY FIXED (C130). Lobby half open.**

**[V]** `docs/decisions.md` **C130** (line 5999) is this exact report, quoting the player
verbatim, and it landed. The path is whole on `main`:

`SetupScreen::shield` (setup.rs:676) → `SetupScreen::shield()` (setup.rs:804, via
`SHIELD_OF_HOTSPOT`, `DAT_004D5548`, the identity map) → `crate::scenario::new_game(..., shield, ...)`
(scenario.rs:231) → `NewGame::shield` (`l2-scenario/src/newgame.rs:277`) →
`assign_lords` step 1 marks the human's shield taken (newgame.rs:1135) → `realm.shield_index`
(newgame.rs:1359) → `game.realm_colour[id]` (scenario.rs:280) → the banner.

I did **not** get to run the four shield-clicking tests C130 mentions, or click through the real
app. The full suite was green at 2,203, which includes them. **My read is that this no longer
reproduces in single player and should be reported as already fixed** — but say "not
independently re-verified by me beyond the green suite".

Original functions, for rule 5: `Setup_ShieldClick` (`0x00432EE6`) and `Setup_ClaimShield`
(`0x00432FAB`). **Neither name is in `docs/symbols.json` yet** — `docs/plan.md` §0.0a records
them as earned-and-unfiled, and that file is lead-owned, so leave them.

### What is genuinely still missing: `lobby::Player` has no `shield` field

`docs/netcode.md` **D-3b** (line 444) carries the whole argument and it is right:

* It goes in **`lobby::Start`**, beside the seed and roster — concretely a field on
  `lobby::Player`, next to the name.
* **NOT in `Hello`.** `Hello` carries things that must *match* and refuses on difference
  (protocol, build, ruleset hash, quirks). A shield is per-seat and deliberately different on
  every peer, so there is nothing to compare and refusing on a difference would refuse every
  game. It needs **arbitration**, which the original already does: `Setup_ClaimShield`
  (`0x00432FAB`) keeps a claim table at `DAT_0057CB40` and ignores a click on a colour somebody
  else holds. The host owns that table for the same reason it owns the roster and the seed.
* Why it is world-building and not presentation **[V]**: `Realms_AssignLords` (`0x0049CAAA`)
  marks humans' shields taken, hands each AI the lowest free shield *in realm order*, then picks
  that realm's **lord** from `g_lordChoice` keyed by the shield it just got. So the choice moves
  which lord sits behind each realm — a value in `RealmState`, in the save body, inside
  `Canonical::hash_of(kingdom)`. Two peers who disagree are playing different games from tick 0.

Writing the field is small; the reasoning above is the part that took the work.

---

## What I tried that was not useful, so you do not repeat it

* `tools/oracle/decomp/` is **empty in the worktree** (gitignored). The corpus is in the main
  checkout: `E:\dev\lords2\tools\oracle\decomp\*.c`, 14 files by address range. `grep -n` there
  directly; no Ghidra run is needed for any of the above.
* Reading `docs/agents.md` whole costs ~30k tokens and truncates. Grep it.

## Exactly where I was, and the single next step

I had just finished the baseline suite run (2,203 / 0) and was about to write the defect-2 fix.

**Next step:** add `realm_turn_ended` to `crates/l2-game/src/turn.rs` exactly as sketched above,
call it from `draw_menu_bar` in `crates/l2-game/src/screens/map.rs`, delete the now-false clause
in the `map.rs` ~3875 comment and the false clause in `menubar.rs`'s "# What is ours", add the
test + ablation, and record `CNEW-shields-are-the-turn-clock` in `docs/decisions.md`.
An `docs/arms.json` record is **not** needed — this is a draw, not an input arm.

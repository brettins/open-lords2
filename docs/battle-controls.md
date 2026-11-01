# Modern battle controls — plan

**Proposal, 2026-11-01.** Owner's brief: bring the battle up to modern RTS controls, StarCraft II
as the reference; redesign the interface; no new battlefield art. Agreed 2026-11-01: C272; Q270 Classic stays the default, Q271 `sc2` (§8).

## 0. Verdict

* **Realistic:** nearly all of SC2's *control* layer: select/command split, whole-unit
  selection, control groups, command hotkeys, attack-move, stop, hold, shift-queue, smart
  right-click, camera keys, and a bigger, zoomable view. None of it needs figure art.
* **The thing you select is a formation, not a marine.** So the control layer is SC2's and a
  few conventions come from Total War, which solved formations: the tactical map, formation
  verbs. Not reinventing either.
* **One change unlocks everything: selection leaves the simulation.** Today a selection *is*
  a unit: releasing a box runs `FUN_00478987`, which moves the picked men into a **new unit**
  (`docs/battle.md` §15.5 [V]). That is the "they form up because I selected them" a player
  sees. Every multi-unit feature depends on undoing it.
* **Most new verbs need almost no new simulation.** The original's AI already has them:
  `Enemy_NearestUnit`, `Order_ChargeNearest`, `Order_ShootAtUnit`, `Order_HoldPosition`,
  `BattleUnit_JoinMelee`, all ported (`l2-sim/src/ai/world/methods/moves.rs`,
  `runner/combat/melee.rs`). A modern verb composes them and names them, so rule 5 holds.
* **Not worth it:** anything needing figure art (portraits, new animations, ability
  effects), patrol, subgroup Tab-cycling (no per-type abilities to cycle to), fractional zoom.

## 1. What a battle does today: the original's scheme, reproduced

| gesture | does | evidence |
|---|---|---|
| left drag | box picks **figures by cell**; release **regroups them into one unit** | `FUN_0043BF07`, `FUN_00478987`, §15.5 [V] |
| left release on ground | move order to **one unit**, `DAT_0053E984` | `FUN_0043C57D` → `BattleUnit_Order`, §15.6 [V] |
| left release on an enemy | attack that figure, gated by a stale index | §15.6; `docs/bugs.md` B100 |
| right release | deselect all | `FUN_0043C55C`, §15.7 [V] |
| click a banner | drop that figure, regroup | `FUN_0043C4C6` [V] |
| minimap left | **orders** when anything is selected, else looks | `BattleMap_Click`, §15.8 [V] |
| `1`…`9`, `Ctrl`+digit | recall (camera always jumps), store | `FUN_0043C910`, `FUN_0043C885`, §15.9 [V]: ours already, and nothing tells a player |
| `H`, `V` | line, column; one unit | `FUN_0043C77A` [V] |
| screen edge | scroll one whole cell a step | `Map_ScrollStep` [V] |
| pause | a battle starts paused; **no orders while paused** (`H`/`V` excepted) | §15.3 [V] |
| charge | every figure the player has, once a battle | `FUN_0047A76D` [V] |

Two facts about fighting, not controls, that a modern scheme runs into: **a player's unit does
not join a melee on contact; an AI unit does** (`BattleUnit_JoinMelee` skips `human`,
`docs/battle-ai.md` §4.2 [D]); and the view is 15 × 14 of 80 × 80 cells, 3 % of the field.

## 2. The principle (C272)

Two schemes, `BattleControls=Classic|Modern` in prefs, chosen per seat. **Classic is the
default** (Q270); Modern is opt-in.

1. **Classic stays 1:1.** It is what `arms.rs`, the oracle checks and the arms count in
   `CLAUDE.md` rule 5 measure. Nothing here edits a classic arm.
2. **Modern is ours, like the netcode:** the second place the original is not the authority.
   Its arms are `ours/…`, the inventions bucket `docs/arms.json` already keeps.
3. **The scheme owns input; the quirk menu owns rules.** Modern does not reproduce classic's
   input defects because it does not run classic's input code (B100 is `Battle_UpdateHover`'s
   gate). Rule defects (B45–B50, B76, B267) stay under `docs/bugs.md` §6 in both schemes.
4. **A new order composes the original's primitives and names them.** Behaviour with no
   original equivalent (shift-queue, group-move offsets) is `[I] ours` with its C-number.
5. **Determinism is untouched.** Selection, groups, camera and keys are presentation: out of
   the digest, never sent. Every order is one canonical `BattleOp` (`docs/netcode.md` §4, D-10).

## 3. The keystone: selection out of the simulation

Today `BattleOp::Pick/Drop/Clear/Recall` write `figure.selected` (record `+0x09`, saved and
hashed) and regroup units. Modern keeps a list of **unit ids** on the seat (`LiveBattle`),
never sent; `figure.selected` stays 0. [V] by grep: nothing in `l2-sim/src` reads `selected`
outside the selection verbs (`runner/movement/pathfinding.rs`) and the save codec.

One new op carries every modern order:

```
BattleOp::Command { units: Vec<u8>, verb: Verb, at: (u8, u8), target: Option<usize>, queue: bool }
Verb = Move | AttackMove | Stop | Hold | Line | Column | Merge
```

* **Group move keeps relative positions** (SC2's rule): `at` outside the selection's bounding
  box sends each unit to `at + (its centre − the group's centre)`, clamped, where it forms up
  with its own `Formation_ComputeRect`; `at` inside it converges them. One `BattleUnit_Order`
  per unit; no unit is created or merged. [I] ours.
* **Merge** is the old regroup made explicit: `FUN_00478987` over the chosen units' figures,
  for a player who wants one block. Its quirks come with it (the last figure decides the
  category; it does nothing when all 80 unit slots are taken).
* The sim refuses the op unless every unit is live and the seat's own: today's
  `turn::concerns` side check, per unit.

## 4. The work, in four tiers

### Tier 1 — easy wins: the input layer, days each

| | what | note |
|---|---|---|
| 1 | Left selects, right commands: ground moves, an enemy is attacked | needs §3 |
| 2 | A click picks the **whole unit**; a box picks every unit with a man inside; Shift adds or removes; double-click or Ctrl-click picks that troop type on screen; F2 the whole army | presentation |
| 3 | Groups the SC2 way: Ctrl+N set, Shift+N add, N select, N twice centres the camera | classic's store/recall stays as it is |
| 4 | Command hotkeys, §6 | |
| 5 | Camera: arrows pan, middle-drag pans, **pixel-smooth** scrolling (classic jumps 32 px), minimap left looks and right commands, Ctrl+F5…F8 store and F5…F8 recall a view | presentation |
| 6 | **Orders while paused**, single player: the order lands, men move on unpause | one classic guard, skipped in modern |
| 7 | Speed ½× / 1× / 2×, single player | sim steps per frame; deterministic |
| 8 | Feedback: player-colour brackets under selected men, a strength bar per unit (men left of men at start), move and attack marks, queued waypoints while Shift is held, hover names the troop and its men from `L2.eng` | drawn, palette colours, no art |
| 9 | An "under attack" ping on the minimap when an off-screen unit is hit (unit `+0x0C`, `docs/battle-ai.md` §3.2); Backspace jumps there | presentation |

### Tier 2 — new verbs: small sim changes, each a composition

| verb | built from | sim change |
|---|---|---|
| **Attack-move** (A, click) | the move; each unit sweep, `Enemy_NearestUnit(unit, 9, 3)` (the AI's melee constants, a starting point) → melee `Order_ChargeNearest`, missile `Order_ShootAtUnit` in range; no enemy left, re-issue the move | a unit flag and one check in the sweep |
| **Stop** (S) | `Order_HoldPosition` | none |
| **Hold** (H) | `Order_HoldPosition`, and the unit's men do not enter the chase (state 8); bows still shoot, state 5 fires by itself (`docs/battle.md` §6.2) | a flag on chase entry |
| **Stances** (Q271) | see §8: Move and Hold are the original's player behaviour; idle, Stop and attack-move engage on touch (`BattleUnit_JoinMelee` without the `human` skip), on a hit (unit `+0x0A`, `docs/battle-ai.md` §3.2) or within `Enemy_NearestUnit`'s 8/9 cells | a stance byte; the stop-when-clear radius is [I] ours |
| **Queue** (Shift) | up to 8 orders a unit, the next taken when its men stand idle at the target | [I] ours: the one new rule |
| **Line / column for every selected unit** | `FUN_0043C77A` per unit (classic turns one) | none |
| **Merge** (M) | §3 | none |

Charge stays a rule: every man, once a battle. Attack-move is the per-unit version of it.

### Tier 3 — the interface

The player, mouse and keyboard, mid-campaign, wants to win this fight and keep the army:

1. The battle opens paused. I see both strengths and my units; I place them (orders while
   paused), set lines, unpause with Space.
2. The lines close. I select and command on the field; the HUD says what I hold and what it
   can do, and nothing else.
3. A unit off-screen is hit: a ping, Backspace, I am there.
4. Losing: retreat and autocalc are present, out of the way, each behind its confirm.

A bottom HUD replaces the right column; SC2's arrangement, built from the game's own parts:

```
+-------------------------------------------------------------------+
| A 1,240 men ######....  B 980 men                    1x  || Menu  |  top strip
|                                                                   |
|                    the battlefield, full width                    |
|                                                                   |
+-----------+-------------------------------------+-----------------+
| minimap   | [1 Kn 12] [2 Ar 40] [3 Pk 30]       | Move   AtkMv    |
| camera    | one tile per selected unit: banner, | Stop   Hold     |
| box,      | men, strength bar; click = only it, | Line   Col      |
| pings     | Shift-click = drop it               | Merge  Charge   |
|           |                                     | Gate Retr Auto  |
+-----------+-------------------------------------+-----------------+
```

* Troop tiles use `Misc_bat`, the classic panel's own troop banners; the five battle
  buttons keep `misc_cty` frame 1's icons; plates are the `Panels.pl8` kit. Verbs with no
  icon are words in the game font, `L2.eng`'s where a group has them (rule 6). A dozen
  24 px glyphs later is optional, not needed.
* The group bar is the nine control groups as tabs above the tiles: largest troop's banner
  and men.
* Mocked as a static page with the real assets and shown before it is built (the
  `interface-designer` skill).

### Tier 4 — the view: bigger and zoomable

The owner allows other resolutions, and the 3× sharp plane (C259) already composes a second
scale over the 640 × 480 one. For a battle this is the largest single gain: armies deploy at
opposite ends of an 80-cell field, and a player sees 15 × 14 of it.

* **The battle canvas takes the window's size**, the HUD drawn on its own fixed-scale plane.
  At 1440p and 2× that is about 40 × 19 cells over a 120 px HUD, three and a half times
  today's view, with the same art.
* **Integer zoom on the wheel:** 1×, 2×, 3× on the field plane only. Crisp, nothing
  resampled; at 1× on 1080p the field shows 60 × 30 cells. No fractional zoom: 48 px pixel
  art shimmers.
* **Tactical map** (Tab, Total War's key): the whole 80 × 80 at about 6 px a cell, units as
  their banners, orders accepted. The original already orders from its 2 px overview
  (§15.8); this is that at a usable size.
* Whether it is fun: a battle here is lines, flanks, archers on high ground and the moat,
  not micro. A wider view rewards exactly that; 2× and 3× keep the close look.

## 5. Order of work

| phase | delivers | needs | size |
|---|---|---|---|
| P0 | the C-entry; the `BattleControls` pref; the modern key table; `BattleOp::Command` and the group-move offsets in `l2-sim`, with codec, purity and determinism tests | — | small |
| P1 | Tier 1 rows 1–8 on today's 640 × 480 screen, the right column reused for unit tiles and verbs | P0 | medium |
| P2 | Tier 2 | P0 | medium |
| P3 | HUD mock, then built; Tier 1 row 9 | P1 | medium |
| P4 | the window-sized battle canvas, the HUD plane, zoom, the tactical map | P3's layout | large |
| P5 | rebinding in prefs; a lobby lock on the scheme; tooltips | P1 | small |

**P1 and P2 are the point:** after them the battle plays like a modern RTS on the old
screen. P1 does not wait for the HUD (`interface-does-not-block-features`).

## 6. Modern keys

SC2's key where SC2 has the verb; classic's where it does not collide.

| key | verb |
|---|---|
| right | move, or attack the enemy under the cursor |
| A, then left | attack-move, on the field or the minimap |
| S / H | stop / hold (classic's `H` is line; modern moves line to `L`) |
| L / C | line / column; `V` stays an alias for column |
| M | merge the selected units |
| G | charge, once |
| Shift + an order | queue it |
| Ctrl / Shift + 1–9; 1–9; twice | set / add / select / centre |
| F2 | the whole army |
| Space | pause; Backspace jumps to the last ping |
| + / − | speed, single player |
| arrows, middle-drag | pan (classic's arrows walk the debug figure; modern takes them) |
| wheel / Tab | zoom / tactical map (P4) |
| Alt | strength bars on every unit |

## 7. Risks, and what this leaves alone

* **Balance.** The AI was tuned against classic hands: one unit at a time and no contact
  join. Attack-move and engage-on-contact make a player far stronger. Measure it with the
  fought-battle harness after P2. It is not a reason to hold back. **The battle AI is not
  changed here** (owner, 2026-11-01: it needs strengthening, later).
* **Multiplayer.** A modern seat can face a classic one with better tools. Recommended:
  allowed, and the host may lock one scheme for the room (P5).
* **The new ops change the ruleset version** (D-12), as they should; recorded games replay
  under the version that made them.
* **The 80-unit ceiling** (`docs/battle.md` §5.4) is under less pressure than in classic,
  which creates a unit with nearly every box.
* **Left alone:** combat, movement, the pathfinder, the battle AI, every classic arm, and the
  battle's rules: it starts paused, one charge, retreat is the autocalc (B76), the drawbridge.

## 8. Open questions

* **Q270 — answered: Classic.** Classic is the default; Modern is a pref. Nothing switches it.
* **Q271 — answered: `sc2`, as recommended below.** What a modern unit does when an enemy comes at it.
  *Contact in the original* **[D]** (`docs/battle-ai.md` §4.2, `docs/battle.md` §7): a man's
  step into an enemy's cell (`Cell_TryEnter` 999). The two duel; a computer unit then charges
  whole (`BattleUnit_JoinMelee`), a player's does not, so only the men touching fight. Being
  shot is not contact: it marks the unit hit for 50 frames, read only by the AI.
  *StarCraft II*, as it plays: Move ignores enemies; an idle unit attacks what enters its
  acquire range and chases it, and goes after a shooter it cannot see; attack-move is idle
  behaviour while walking; Hold fights what is in reach and never moves.
  *Recommended (`sc2`)*: the stance is the last order. Move: the original's player
  behaviour. Idle, Stop, attack-move: engage when touched, when hit, or when an enemy unit is
  within the AI's own acquire radius (`Enemy_NearestUnit`: 8 cells foot, 9 melee); melee
  charges (`Order_ChargeNearest`), bows and crossbows shoot in weapon range and do not walk;
  the unit stops and reforms when no enemy is left within that radius ([I] ours: a charged AI
  unit's chase has no range limit, `Melee_ChooseChaseTarget`). Hold: the original's player
  behaviour, standing. Alternatives: `touch` (a touch alone pulls the unit in), `no`.

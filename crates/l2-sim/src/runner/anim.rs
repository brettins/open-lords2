//! **The pose machine** — which `Anim_*` handler a figure is running.
//!
//! `Battle_UpdateAllMen` (`0x004822ED`) dispatches `g_manStateTable[state]`,
//! and each state handler opens by calling exactly one animation handler; the
//! handler writes the sprite frame to figure `+0x10` and copies `dirc2` to
//! `facingDrawn` (`+0x0D`) on the way out. `docs/battle.md` §13.5, §14.5.
//!
//! | state | handler | animation |
//! |---|---|---|
//! | 3 `BattleMan_StateWalk` `0x0048314E` | `Anim_WalkA2` `0x00486D83` if the step moved, `Anim_StandA2` if it did not | [`Motion::Walking`] / [`Motion::Idle`] |
//! | 8 `BattleMan_StateChase` `0x00483E55` | the same pair | |
//! | 4 `BattleMan_StateMelee` `0x004831D8` | `Anim_StrikeA2` `0x00486249`, and `Anim_WalkA2` when `BattleMan_Step(1)` reports a crossing still running | [`Motion::Attacking`] |
//! | 6 `BattleMan_StateAttackWall` `0x00483A88` | `Anim_StrikeA2` | [`Motion::Attacking`] |
//! | 5 `BattleMan_StateIdle` `0x004832EA` | `Anim_StandA2` `0x004872AE`, and `Anim_DrawBowA2` `0x0048804A` while a shot is being drawn | [`Motion::Idle`] / [`Motion::Shooting`] |
//! | 2 `BattleMan_StateDead` `0x004830E9` | `Anim_CollapseA2` `0x00487CE4`, `Anim_DyingA2` `0x00487908` | [`Motion::Dying`] |
//!
//! **Every pose in this crate is set through one of these four methods**, and
//! that is the point of the module. The four symptoms the player reported on
//! 2026-09-14 were all one shape: a pose written by an arm that then returned,
//! with no arm on the other side to take it back. A figure whose opponent died
//! kept striking; a figure whose step was refused kept marching; two arms that
//! disagreed swapped the pose tick by tick and jittered.
//!
//! Poses are **presentation** and stay out of the lockstep digest —
//! `docs/netcode.md`. Nothing in `l2-sim` reads `anim` to decide anything; the
//! renderer reads it to pick a frame.

use super::BattleRunner;
use crate::{Motion, Role};

impl BattleRunner {
    /// `Anim_StandA2` (`0x004872AE`) — **stand, and fidget.**
    ///
    /// `+0x0B` counts up each frame and, when it passes the per-figure period
    /// at `+0x0C` (180 … 243 frames), resets and turns `facingDrawn` one step:
    /// **left on an even map x, right on an odd one**, so a rank shuffles and
    /// no two neighbours shuffle together. `docs/battle.md` §14.6, **[V]** —
    /// one writer, two readers, both `Anim_Stand*`.
    ///
    /// This is also the exit every other arm needs. A man with nothing to do is
    /// standing, and saying so in one place is what stops a pose being left
    /// behind by an arm that returned.
    ///
    /// **The counter is touched nowhere else.** `Anim_StandA2` is the only
    /// function in `Lords2.exe` that writes `+0x0B`, and it neither zeroes it
    /// on arrival nor writes `facingDrawn` outside the turn — a man who walks
    /// up and stops keeps the drawn facing his last handler left and carries on
    /// counting from wherever he was. Ours reset the counter in all four
    /// handlers and wrote `facingDrawn = dirc` here, which restarted the fidget
    /// of every man who so much as took a step.
    pub(super) fn stand(&mut self, i: usize) {
        let f = &mut self.fighters[i];
        f.anim = Motion::Idle;
        // **A siege engine never reaches the body of it.** `Anim_StandA2`
        // opens `if (troopType < 7)` and hands everything above to
        // [`Self::siege_pose`] (`00480000.c:2828`).
        if f.troop.index() >= 7 {
            return self.siege_pose(i);
        }
        f.fidget = f.fidget.wrapping_add(1);
        if f.fidget > f.fidget_period {
            f.fidget = 0;
            f.facing_drawn = match f.x % 2 == 0 {
                true => (f.facing_drawn + 7) % 8,
                false => (f.facing_drawn + 1) % 8,
            };
        }
    }

    /// `Anim_WalkA2` (`0x00486D83`) — **march**, poses 0 … 5, one every four
    /// ticks over a 24-tick loop, read from `dirc` (`+0x18`).
    ///
    /// **It is called only when the figure moved.** §14.5's third agreement:
    /// `BattleMan_StateWalk` calls `Anim_StandA2` when `BattleMan_Step` reports
    /// it did not. Ours set [`Motion::Walking`] before asking the mover and
    /// left it set when the step was refused — a man barred by a friendly, or
    /// by a wall, marched on the spot for as long as he was stuck, and flipped
    /// between marching and standing whenever the pathfinder found and lost a
    /// route on alternate ticks. The player's *"stuck walking"* and the jitter.
    ///
    /// `moved` is `BattleMan_Step`'s own return: true while a crossing is
    /// running or a step has just been committed.
    /// **It runs either way, and `Anim_StandA2` only takes the frame back.**
    /// `BattleMan_StateWalk` (`0x0048314E`, `00480000.c:1359`) is
    /// `Anim_Walk(); if (BattleMan_Step(0) == 0) Anim_Stand();` — the walk
    /// handler has already written `facingDrawn = dirc` and stepped
    /// `animPhase` by the time the refusal is known, and `Anim_StandA2` writes
    /// neither. So a barred man stands *at the facing he wanted*, and his walk
    /// cycle keeps turning under the standing pose. Ours skipped the handler
    /// and left him standing at a stale `facing_drawn`.
    pub(super) fn march(&mut self, i: usize, moved: bool) {
        let f = &mut self.fighters[i];
        f.anim = Motion::Walking;
        // **A siege engine walks by another handler.** `Anim_WalkA2` gates on
        // `troopType < 7` too and sends the rest to `FUN_00488436`
        // (`0x00488436`, `00480000.c:2773`) — the siege engine walk, a frame
        // read from `dirc` and `polarDirc` with **no `animPhase` step and no
        // `facingDrawn` write**. `l2_view::engines` is that frame; there is no
        // counter here to keep. The stand below still runs: the state handler
        // calls `Anim_StandA2` on a refused step whatever the troop is.
        if f.troop.index() < 7 {
            f.facing_drawn = f.facing;
            // `animPhase` wraps at 0x17 here — six poses of four ticks. The
            // **frame is read before this step** (`00480000.c:2754`, the
            // `animPhase >> 2` sits above the `+ 1`); `l2_view::figures::pose_of`
            // takes the step back off for the drawn pose.
            f.phase = (f.phase + 1) % 24;
        }
        if !moved {
            self.stand(i);
        }
    }

    /// `Anim_StrikeA2` (`0x00486249`) — **strike**, base 6 plus the per-troop
    /// cycle, read from `dirc2` (`+0x19`).
    ///
    /// `facing` is the strike facing: the melee arm aims it at the opponent and
    /// leaves `dirc` — the crossing's — alone,
    /// apart. `docs/battle.md` §13.8.
    ///
    /// **Only the swinging man's phase moves.** `00480000.c:2509`: the
    /// `animPhase + 1` and its 0x27 wrap sit inside `if (role == 1)`, and
    /// `Anim_StandA2` and `Anim_DrawBowA2` never touch the counter at all. The
    /// defender of a pair is drawn by this same handler with his cycle frozen,
    /// and it resumes where it stopped when [`crate::melee`] swaps the roles.
    /// Ours stepped every figure's phase once a tick from `step_one`, so a
    /// defender's frozen pose crept and the swap was invisible.
    pub(super) fn strike(&mut self, i: usize, facing: u8) {
        let swinging = self.sim.figures[self.fighters[i].sim].role == Role::Attacking;
        let f = &mut self.fighters[i];
        f.anim = Motion::Attacking;
        // **No engine swings.** `Anim_StrikeA2` opens on `troopType < 7`
        // (`00480000.c:2477`) and its else is [`Self::siege_pose`] (`2578`):
        // a ram (9) or a ladder (8) reaching here through `strike_castle`
        // gets the engine's own frame, not the strike cycle, and no
        // `facingDrawn` write either.
        if f.troop.index() >= 7 {
            return self.siege_pose(i);
        }
        f.facing_drawn = facing % 8;
        if swinging {
            f.phase = (f.phase + 1) % 40;
        }
    }

    /// `FUN_00488793` (`0x00488793`, `00480000.c:3363`) — **the engine's
    /// standing pose**, where `Anim_StandA2` (`2828`) and `Anim_StrikeA2`
    /// (`2578`) send every `troopType >= 7`.
    ///
    /// Only the oil pot counts: `animPhase += 1; if (0x2F < animPhase)
    /// animPhase = 0` (`3364-3367`) — the six bubbling pictures,
    /// `(animPhase >> 3) + 0x23`. A **clamp, not a modulo**: the seed
    /// `((index * 9 + x * 16) & 0x3F)` reaches 63, which `% 48` would leave
    /// walking 16 … 63 → 0 … 15 while the clamp drops it to 0 at once. The
    /// ram, the ladder and the catapult read a facing and no counter.
    fn siege_pose(&mut self, i: usize) {
        let f = &mut self.fighters[i];
        if f.troop.index() == 10 {
            f.phase += 1;
            if f.phase > 0x2F {
                f.phase = 0;
            }
        }
    }

    /// `Anim_DrawBowA2` (`0x0048804A`) — **draw a bow**, poses 10 … 12.
    ///
    /// **The window is the whole reload, and it is the caller's.**
    /// `BattleMan_FireMissile` (`0x00483337`) ends `if (target != 0)
    /// Anim_DrawBow();` — every tick, and the pose
    /// comes from a curve indexed by `swingTimer`, which that same function
    /// steps every tick and zeroes only when the shot leaves. **[V]**: the call
    /// is the function's tail and `swingTimer` has three writers in it, all
    /// read. `l2_view::drawbow` holds the two curves.
    ///
    /// Ours drew for one tick: `shoot` ran on the acquisition tick alone and
    /// `stand` took the pose back the next, so poses 11 and 12 never reached
    /// the screen and arrows left a standing man.
    ///
    /// The phase is **not** restarted here — `Anim_DrawBowA2` does not touch
    /// `animPhase`, and it does not read it either.
    pub(super) fn shoot(&mut self, i: usize) {
        let f = &mut self.fighters[i];
        f.anim = Motion::Shooting;
        f.facing_drawn = f.facing;
    }
}

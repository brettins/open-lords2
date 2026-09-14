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
use crate::Motion;

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
    pub(super) fn stand(&mut self, i: usize) {
        let f = &mut self.fighters[i];
        // Every handler's tail writes `facingDrawn = dirc2`, so a man arriving
        // in the standing pose faces where he last faced; the fidget then walks
        // away from it for as long as he stays there.
        if f.anim != Motion::Idle {
            f.facing_drawn = f.facing;
            f.fidget = 0;
        }
        f.anim = Motion::Idle;
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
    pub(super) fn march(&mut self, i: usize, moved: bool) {
        if !moved {
            self.stand(i);
            return;
        }
        let f = &mut self.fighters[i];
        f.anim = Motion::Walking;
        f.fidget = 0;
        f.facing_drawn = f.facing;
    }

    /// `Anim_StrikeA2` (`0x00486249`) — **strike**, base 6 plus the per-troop
    /// cycle, read from `dirc2` (`+0x19`).
    ///
    /// `facing` is the strike facing: the melee arm aims it at the opponent and
    /// leaves `dirc` — the crossing's — alone,
    /// apart. `docs/battle.md` §13.8.
    pub(super) fn strike(&mut self, i: usize, facing: u8) {
        let f = &mut self.fighters[i];
        f.anim = Motion::Attacking;
        f.fidget = 0;
        f.facing_drawn = facing % 8;
    }

    /// `Anim_DrawBowA2` (`0x0048804A`) — **draw a bow**, poses 10 … 12.
    ///
    /// The window is `BattleMan_FireMissile`'s own: the target is acquired ten
    /// ticks before the reload interval expires and the missile leaves when it
    /// does, so the draw is those ten ticks and nothing else. **[V]** on the
    /// window (§6.2) and on the pose band (§13.5); **[I]** on the binding
    /// between them — `Anim_DrawBowA2` has no caller we can read, and ten ticks
    /// of held aim is the only place in the tick that wants three poses.
    ///
    /// **While the missile flies the shooter stands.** He is back in
    /// `Anim_StandA2` the tick after the loose; nothing in the figure record
    /// follows a missile once it is spawned (§14.7 — `+0x06` is the shooter and
    ///), so there is nothing for a follow-through
    /// pose to be driven by. **[I]**, and it is what the pose band can pay for.
    ///
    /// The phase is restarted at the draw so the three poses run once through
    ///
    pub(super) fn shoot(&mut self, i: usize) {
        let f = &mut self.fighters[i];
        if f.anim != Motion::Shooting {
            f.phase = 0;
        }
        f.anim = Motion::Shooting;
        f.fidget = 0;
        f.facing_drawn = f.facing;
    }
}

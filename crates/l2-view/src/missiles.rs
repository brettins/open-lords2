//! **The arrow in the air** — `FUN_004BEED4` (`0x004BEED4`), the last pass of
//! a battle frame. `docs/battle.md` §13.11, `docs/decisions.md`
//! `C202`.
//!
//! Every missile in the game is drawn from **one sheet**: `A2_miss.pl8`, slot 6
//! of the battle asset table at `0x004DA550`. `FUN_00480F8B` writes that one
//! buffer pointer (`DAT_00566518`) into all hundred records' `+0x00` at the
//! start of a battle, and `Missile_Spawn` (`0x0046E767`) writes it again into
//! each new record. **[V]** from the table's bytes and the loader's ladder.
//!
//! ```c
//! /* FUN_004BEED4(head) — head is cell byte +6, the per-cell missile list */
//! for (n = 0; n <= 9 && head; head = missile[+0x04]) {
//!     if (missile[+0x08] == 0) return;              /* owner: a free slot   */
//!     x = missile[+0x0A] - camX * 0x20;             /* 1/32 cell == 1 pixel */
//!     y = missile[+0x0C] - camY * 0x20;
//!     if (missile[+0x09] != 7) {                    /* class 7 is invisible */
//!         if (missile[+0x09] == 5) { c = (c + 1) & 15; x += jitter[c].x; y += jitter[c].y; }
//!         frame  = missile[+0x12];
//!         g_drawX = origin.x + x + tileSize / 2;
//!         g_drawY = origin.y + y + tileSize / 2;
//!         Clip_Horizontal(0, 480);  Clip_Vertical(0x18, 0x1D8);
//!         blit(missile[+0x00], frame);
//!     }
//! }
//! ```
//!
//! Four things in that are worth stating flat, because each is a decision
//! somebody could get wrong:
//!
//! * **A missile's position is already in pixels.** `+0x0A`/`+0x0C` are
//!   thirty-seconds of a cell and a battle tile is 32 pixels, so the sub-cell
//!   unit and the screen pixel are the same unit — [`crate::scene::TILE`]. No
//!   scaling anywhere.
//! * **There is no sprite centring.** `BattleFigure_Draw` subtracts half the
//!   sprite width on both axes; this adds `tileSize / 2` and nothing else
//!   (`DAT_004E5D44 = param_11 / 2`, stored by `FUN_004BC020`). Reproduced
//!   rather than corrected.
//! * **Class 7 — the boiling-oil stream — is never drawn.** `docs/battle.md`
//!   §17.2 already said *"invisible to the renderer"*; this is the function
//!   that makes it so. What a player sees of a pour is the fire it leaves.
//! * **Ten to a cell.** Both ends of the list give up after ten
//!   (`Missile_LinkToCell`, `0x0046EFBE`, and the counter here), so an eleventh
//!   missile standing on one cell is not drawn.
//!
//! # Which frame
//!
//! | class | frame | written by |
//! |---|---|---|
//! | 1 bow | `dir + 0` | `BattleMan_FireMissile` (`0x00483337`), once at spawn |
//! | 2 crossbow | `dir + 8` | the same |
//! | 3 catapult | `16` | `BattleMan_StateEngineFire` (`0x004843BC`) — **no direction** |
//! | 4 debris | [`DEBRIS_FRAMES`]`[ttl >> 1]` | `Missile_UpdateAll`, every tick |
//! | 5 fire | [`FIRE_FRAMES`]`[ttl >> 4]` | the same |
//! | 7 oil | not drawn | — |
//!
//! The three weapon bases are column 4 of `g_missileStats` (`0x004D97B0`,
//! 5 ints a row): 0, 8, 16. **[V]** from the bytes. Adding the flight
//! direction `+0x2E` to the first two is `BattleMan_FireMissile`'s
//! `(ushort)missile[+0x2E] + missile[+0x34]`, which promotes `docs/battle.md`
//! §2.3's `missileSprite` row from `[D]` to `[V]`.
//!
//! **The layout closes against the shipped art.** `A2_miss.pl8` holds exactly
//! **81** frames: 0…7 arrows, 8…15 bolts, 16 the catapult shot, 17…24 the
//! debris, 25…40 the fire, and 33 + `shield * 8` + 0…7 the animated banner
//! `FUN_004BD574` draws — six shields ending at frame 80. Nothing spare.
//! `tests/install.rs`.

use l2_sim::missile::{Missile, CLASS_DEBRIS, CLASS_FIRE, CLASS_OIL};

/// Slot 6 of the battle asset table (`0x004DA550`, 20-byte records), the one
/// buffer every missile's `+0x00` points at.
pub const MISSILE_SHEET: &str = "A2_miss.pl8";

/// **At most ten missiles are drawn on one cell.** `FUN_004BEED4` breaks at
/// `9 < n`, and `Missile_LinkToCell` gives up walking the list after ten too.
pub const CELL_LIST_LIMIT: usize = 10;

/// `g_missileStats[class][4]` — the frame the flight direction is added to.
/// Row 0 is the "no class" row and is never used. **[V]**
pub const SPRITE_BASE: [usize; 4] = [0, 0, 8, 16];

/// **A catapult shot has one picture, not eight.** `BattleMan_FireMissile`
/// writes `dir + base`; `BattleMan_StateEngineFire` writes `base` alone.
pub const DIRECTIONAL_CLASSES: [u8; 2] = [1, 2];

/// `DAT_004D9950`, indexed by the debris record's `+0x3C` countdown `>> 1`.
/// The countdown starts at [`l2_sim::missile::DEBRIS_TTL`] (`0x78`), so the
/// walk is index 60 down to 0 and the picture runs 17 → 24 as the rubble
/// settles. 61 entries is all `>> 1` can reach; the eight zeros after them are
/// the gap before [`FIRE_FRAMES`]. **[V]** from the bytes.
pub const DEBRIS_FRAMES: [u8; 61] = [
    24, 24, 24, 24, 24, 24, 24, 24, 23, 23, 23, 23, 23, 23, 23, 23, 22, 22, 22, 22, 22, 22, 22, 22,
    20, 20, 20, 20, 19, 19, 19, 19, 19, 19, 19, 19, 19, 19, 19, 19, 18, 18, 18, 18, 18, 18, 18, 18,
    18, 18, 18, 18, 18, 18, 18, 18, 17, 17, 17, 17, 17,
];

/// `DAT_004D99A8`, indexed by a burning cell's `+0x3C` countdown `>> 4`. A fire
/// is lit with up to `0x280` frames of life, so the index reaches 40. Two
/// eight-frame groups, 25…32 and 33…40, walked backwards. **[V]**
pub const FIRE_FRAMES: [u8; 41] = [
    25, 26, 32, 31, 30, 29, 28, 27, 26, 25, 32, 31, 30, 29, 28, 27, 26, 25, 33, 40, 39, 38, 37, 36,
    35, 34, 33, 40, 39, 38, 37, 36, 35, 34, 33, 40, 39, 38, 37, 36, 39,
];

/// **The shimmer under a fire** — sixteen `(i32, i32)` pairs at `0x004E4470`,
/// applied to a class-5 record's screen position before it is blitted. **[V]**
/// from the bytes.
pub const FIRE_JITTER: [(i32, i32); 16] = [
    (-4, 5),
    (-6, 2),
    (-4, -1),
    (-2, -3),
    (3, 0),
    (7, 3),
    (9, 5),
    (11, 3),
    (6, 0),
    (2, -4),
    (4, -6),
    (5, -2),
    (4, 3),
    (0, 6),
    (-3, 9),
    (-6, 7),
];

/// The frame a live missile shows, or `None` when nothing is drawn for it.
///
/// `None` is [`CLASS_OIL`] — the one class `FUN_004BEED4` steps over — and any
/// class outside the five it knows.
pub fn frame(m: &Missile) -> Option<usize> {
    match m.class {
        c if c == CLASS_OIL => None,
        c if c == CLASS_DEBRIS => {
            Some(DEBRIS_FRAMES[(m.ttl.max(0) as usize >> 1).min(DEBRIS_FRAMES.len() - 1)] as usize)
        }
        c if c == CLASS_FIRE => {
            let f = FIRE_FRAMES[(m.ttl.max(0) as usize >> 4).min(FIRE_FRAMES.len() - 1)] as usize;
            // `Missile_UpdateAll`'s class-5 arm links the record to its cell
            // only while the frame is positive, so a zero frame is a fire that
            // is not on any draw list.
            (f > 0).then_some(f)
        }
        c if (c as usize) < SPRITE_BASE.len() && c > 0 => {
            let base = SPRITE_BASE[c as usize];
            Some(if DIRECTIONAL_CLASSES.contains(&c) { base + (m.dir & 7) as usize } else { base })
        }
        _ => None,
    }
}

/// **Which of the sixteen jitters a burning cell takes this frame.**
///
/// The original's `DAT_004E5B1C` is a render-side global: pre-incremented once
/// per fire *drawn*, wrapped at 16, and never reset, so the same cell takes a
/// different offset every frame and the flame shimmers. A counter that
/// survives between frames would make this crate's renderer stateful, and
/// `crates/l2-view/src/lib.rs` says it is a reader and nothing else — the
/// existing test that draws one state twice and demands an identical canvas is
/// that rule, enforced.
///
/// So the tick stands in for the global: same sixteen offsets, same wrap, same
/// shimmer, one advance a frame instead of one an drawn fire. Ours is a
/// function of simulation state; the original's is a function of how many
/// fires have ever been painted. Nothing else about the fire differs.
pub fn jitter(tick: u32, nth_fire_this_frame: usize) -> (i32, i32) {
    FIRE_JITTER[(tick as usize + nth_fire_this_frame + 1) % FIRE_JITTER.len()]
}

#[cfg(test)]
mod tests {
    use super::*;
    use l2_sim::missile::{Missiles, WeaponClass};
    use l2_sim::{missile, Troop};

    fn shot(class: u8, dir: u8, ttl: i16) -> Missile {
        Missile { owner: 1, class, dir, ttl, ..Missile::default() }
    }

    /// The three weapon bases, and the one class that does not add its
    /// direction.
    #[test]
    fn a_bow_and_a_bolt_point_where_they_fly_and_a_catapult_shot_does_not() {
        for d in 0..8u8 {
            assert_eq!(frame(&shot(1, d, 0)), Some(d as usize), "arrow facing {d}");
            assert_eq!(frame(&shot(2, d, 0)), Some(8 + d as usize), "bolt facing {d}");
            assert_eq!(frame(&shot(3, d, 0)), Some(16), "catapult shot facing {d}");
        }
    }

    #[test]
    fn boiling_oil_is_the_one_class_the_renderer_steps_over() {
        assert_eq!(frame(&shot(CLASS_OIL, 0, 16)), None);
    }

    /// Debris walks 17 → 24 as its countdown runs out, and every index the
    /// countdown can reach is inside the table.
    #[test]
    fn debris_settles_from_seventeen_to_twenty_four() {
        assert_eq!(frame(&shot(CLASS_DEBRIS, 0, missile::DEBRIS_TTL)), Some(17));
        assert_eq!(frame(&shot(CLASS_DEBRIS, 0, 0)), Some(24));
        for ttl in 0..=missile::DEBRIS_TTL {
            let f = frame(&shot(CLASS_DEBRIS, 0, ttl)).unwrap();
            assert!((17..=24).contains(&f), "ttl {ttl} -> {f}");
        }
    }

    /// A fire's life is `0x280 - param`, so `>> 4` tops out at 40 — exactly the
    /// last entry of the table read out of `0x004D99A8`.
    #[test]
    fn every_fire_countdown_lands_inside_the_table_and_inside_the_sheet() {
        for ttl in 0..=0x280i16 {
            let f = frame(&shot(CLASS_FIRE, 0, ttl)).unwrap();
            assert!((25..=40).contains(&f), "ttl {ttl} -> {f}");
        }
        assert_eq!(0x280 >> 4, FIRE_FRAMES.len() - 1);
    }

    /// Every frame this module can ask for is inside `A2_miss.pl8`'s 81.
    #[test]
    fn no_frame_this_module_asks_for_is_past_the_sheet() {
        const FRAMES_IN_A2_MISS: usize = 81;
        for class in [1u8, 2, 3, CLASS_DEBRIS, CLASS_FIRE, CLASS_OIL] {
            for dir in 0..8u8 {
                for ttl in [0i16, 1, 2, 0x78, 0x100, 0x280] {
                    if let Some(f) = frame(&shot(class, dir, ttl)) {
                        assert!(f < FRAMES_IN_A2_MISS, "class {class} -> {f}");
                    }
                }
            }
        }
    }

    /// A real shot out of `l2_sim`, so the field names cannot drift apart.
    #[test]
    fn a_missile_the_simulation_spawned_has_a_frame() {
        let mut ms = Missiles::new();
        let slot =
            missile::spawn(&mut ms, 1, WeaponClass::Bow, 3, (10, 10), (20, 10), 50, 0).unwrap();
        let f = frame(ms.get(slot)).unwrap();
        // Due east is facing 2 in `l2_sim::facing`.
        assert_eq!(f, 2, "an arrow flying east should be frame 2");
        assert_eq!(Troop::Archers.index(), 5, "the troop order has not moved");
    }

    #[test]
    fn the_jitter_cycles_through_all_sixteen_offsets() {
        let seen: std::collections::HashSet<_> = (0..16).map(|t| jitter(t, 0)).collect();
        assert_eq!(seen.len(), 16);
        assert_eq!(jitter(0, 0), FIRE_JITTER[1], "the original pre-increments");
    }
}

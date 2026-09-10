//! Unrest and revolt — `docs/kingdom.md` §6, `Unrest_UpdateAll`
//! (`0x0044AA41`).
//!
//! One counter per county, walked up and down against happiness on **two
//! different ladders** depending on whether a person owns the county.
//!
//! The human ladder is the one the manual states, and it states it unusually
//! precisely: *"When any county's happiness rating drops below 25 and stays
//! there for more than four seasons, its population will revolt."* Both halves
//! of that sentence are in the code — the threshold is 25, and the counter
//! needs four seasons below it.
//!
//! # The gap in the AI ladder
//!
//! `docs/kingdom.md` §6 gives the AI ladder as *"happiness >= 41 resets it to
//! 0; 11 … 40 walks it down; below 1 walks it up"* — which says nothing about
//! happiness 1..=10. That is reproduced literally here: a dead band where an AI
//! county's unrest neither rises nor falls. It is more likely that the document
//! abbreviated a `< 11` into a `< 1` than that the original really has a hole,
//! but guessing which would be exactly the failure `docs/decisions.md` C3
//! records, so the hole stays and this comment stays with it.
//!
//! # What a revolt *does*, and the three things this pass had wrong
//!
//! `Unrest_UpdateAll` was read again end-to-end for the hundred-turn game
//! (`docs/plan.md` §2.5), because a hundred turns of England raises **twenty
//! revolts** and none of them did anything. Three corrections came out of it,
//! all `[V]` from `0x0044AA41`'s own body — `docs/decisions.md` CNEW-revolt.
//!
//! **One. An AI-owned county revolts too.** The revolt call is at
//! `LAB_0044ADF3`, and it sits inside the *AI* branch — the human branch
//! reaches it with a `goto`. One block, two callers. This module's own test
//! `an_ai_county_never_raises_a_mob` asserted the opposite, sourced from
//! `docs/kingdom.md` §6's prose putting the call "on the human ladder only".
//! It is the C58 shape exactly: a claim read out of a document rather than out
//! of the function.
//!
//! **Two. The revolt only fires on the season the counter *rose*.** Both
//! branches guard it with `if (before < after)`. So a county whose mob could
//! not be placed sits at 4 and never tries again — which is also why the
//! counter is reset **only when [`raise_revolt`] returns true**.
//!
//! **Three. A human county's warning season and its ladder season are
//! exclusive.** The `Msg_Enqueue(0x92)` arm is the `if` and the whole ladder is
//! its `else`, so the season a county first drops below 30 its counter does not
//! move at all. This module ran both, which made every human revolt land one
//! season early.

use crate::county::County;
use crate::map::CampaignMap;
use crate::report::Message;
use crate::levy::OWNERLESS;
use crate::unit::{Unit, UnitKind, Units};
use l2_net::{Quirk, Quirks};

/// The counter that raises a peasant mob.
pub const UNREST_REVOLT: u8 = 4;

/// Human-owned: below this a county is warned, and the warning flag clears at
/// or above it.
pub const HUMAN_WARN_BELOW: i32 = 30;

/// Human-owned: below this the counter climbs. The manual's number.
pub const HUMAN_UNREST_BELOW: i32 = 25;

/// AI-owned: at or above this the counter is reset outright.
pub const AI_CALM_AT_OR_ABOVE: i32 = 41;

/// AI-owned: at or above this (and below [`AI_CALM_AT_OR_ABOVE`]) the counter
/// walks down.
pub const AI_SETTLE_AT_OR_ABOVE: i32 = 11;

/// AI-owned: below this the counter walks up.
pub const AI_UNREST_BELOW: i32 = 1;

/// One county's unrest pass. Returns `true` if the counter reached
/// [`UNREST_REVOLT`] **on a season it rose**, which is the original's condition
/// for calling `County_RaiseRevolt` and nothing more.
///
/// The caller does the raising and resets the counter only if a mob was
/// actually placed — see [`raise_revolt`], and `0x0044AA41`'s
/// `if (3 < unrest && County_RaiseRevolt(c)) unrest = 0;`.
///
/// `id` is the county's index, carried only so the messages can name it.
pub fn update(
    county: &mut County,
    id: u8,
    owner_is_human: bool,
    quirks: Quirks,
    out: &mut Vec<Message>,
) -> bool {
    if county.is_unowned() {
        // `Unrest_UpdateAll`'s first arm: an unowned county's counter is
        // **cleared**, not merely left alone. Realm 0 is not a realm.
        county.unrest = 0;
        return false;
    }

    let before = county.unrest;
    if owner_is_human {
        // The warning arm and the ladder arm are an `if`/`else` in the
        // original, so a county's first season below 30 costs it a message and
        // no counter movement at all.
        if county.happiness >= HUMAN_WARN_BELOW {
            county.unrest_warned = false;
        }
        if county.happiness < HUMAN_WARN_BELOW && !county.unrest_warned {
            county.unrest_warned = true;
            out.push(Message::UnrestWarning { county: id });
            return false;
        }
        if county.happiness < HUMAN_UNREST_BELOW {
            if county.unrest < UNREST_REVOLT {
                county.unrest += 1;
            }
            // `docs/bugs.md` D2: the `else if (happiness < 1) unrest = 4` arm
            // repeats the test above it and can never run.
        } else {
            county.unrest = 0;
        }
        if before < county.unrest {
            out.push(Message::UnrestRising { county: id, level: county.unrest });
        }
    } else {
        update_ai(county, quirks);
    }

    // Both branches converge on `LAB_0044ADF3`, and both reach it only when the
    // counter went **up** this season.
    before < county.unrest && county.unrest >= UNREST_REVOLT
}

/// `County_RaiseRevolt` (`0x004AC185`) — what unrest 4 actually does.
///
/// ```c
/// if (county.owner == 0) return 0;
/// tile = County_FindFreeRoadTile(county) || County_FindFreeOpenTile(county);
/// if (!tile) return 0;                       /* the counter is left at 4 */
/// u = Unit_Spawn(2, tile.x, tile.y, 6);      /* kind 2, ownerless */
/// if (!u) return 0;
/// County_MakeIndependent(county);            /* before the men are taken */
/// u.needsDestination = 1;  u.ownerIsHuman = 0;
/// u.county = county;  u.yearFormed = g_year;
/// u.morale = 50;  u.shield = 0;
/// u.menTotal = Pct(county.population, 30);
/// for (t = 0; t < 7; t++) u.troops[t] = 0;
/// u.troops[0] = u.troops[7] = u.menTotal;    /* all peasants */
/// u.nameIndex = 0;
/// county.population -= u.menTotal;
/// return 1;
/// ```
///
/// Three details that are the rule rather than transcription:
///
/// * **The tile search is the levy's**, box radius 1…3 around the county
///   anchor, road first — [`crate::levy::muster_tile`]. A county with no free
///   tile within three of its anchor **cannot revolt at all** and sits at
///   unrest 4 for ever, because the retry is guarded on the counter rising.
/// * **`County_MakeIndependent` runs before the population is docked**, so the
///   `Labour_Allocate` inside it deals the *old* population. The season's later
///   labour passes (`docs/rules.md` §2, passes 19 and 25) put that right, which
///   is why the nine job records still sum to the population at the turn
///   boundary.
/// * **All 30% land in `troops[0]`.** A revolting mob is unarmed peasants; it
///   is not equipped from the county or the realm.
///
/// Returns the new unit's slot and the men it took, or `None` when the mob
/// could not be placed — which is the caller's signal to leave the counter
/// where it is.
///
/// **The caller owes two things, in this order**: `County_MakeIndependent`, and
/// then `population -= men`. They are the caller's because the first of them
/// needs the whole kingdom and the second must follow it —
/// [`crate::Kingdom::unrest_update`] is the only call site.
pub fn raise_revolt(
    map: &CampaignMap,
    county: &County,
    units: &mut Units,
    id: usize,
    year: i32,
) -> Option<(usize, i32)> {
    if county.is_unowned() {
        return None;
    }
    let (x, y) = crate::levy::muster_tile(map, units, (county.anchor_x, county.anchor_y))?;
    let men = crate::math::pct(county.population, REVOLT_POPULATION_PCT);

    let mut mob = Unit::new(UnitKind::PeasantMob, OWNERLESS, x, y);
    mob.needs_destination = true;
    mob.owner_is_human = false;
    mob.county = id as u8;
    mob.year_formed = year;
    mob.morale = REVOLT_MORALE;
    mob.shield = 0;
    mob.men = men;
    mob.troops[0] = men;
    mob.name_index = 0;
    let slot = units.spawn(mob)?;
    Some((slot, men))
}

/// `Pct(population, 30)` — the share of a county that walks out.
pub const REVOLT_POPULATION_PCT: i32 = 30;

/// `0x32`. A mob's morale is a flat 50, not the county's happiness — which is
/// the one place a levy and a revolt differ in how the unit is built.
pub const REVOLT_MORALE: i32 = 50;

/// **Switchable** — [`Quirk::AiUnrestDeadBand`], `docs/bugs.md` B17.
///
/// The fixed path moves the climb's threshold from [`AI_UNREST_BELOW`] to
/// [`AI_SETTLE_AT_OR_ABOVE`], which closes the hole **upwards**: happiness
/// 1..=10 then climbs, which is what the ladder plainly intends and what a
/// mistyped `< 11` would have given. It deliberately does *not* extend the
/// walk-down instead — an AI county at happiness 3 calming itself is the
/// reading nothing supports.
fn update_ai(county: &mut County, quirks: Quirks) {
    let climb_below = if quirks.reproduces(Quirk::AiUnrestDeadBand) {
        AI_UNREST_BELOW
    } else {
        AI_SETTLE_AT_OR_ABOVE
    };
    if county.happiness >= AI_CALM_AT_OR_ABOVE {
        county.unrest = 0;
    } else if county.happiness >= AI_SETTLE_AT_OR_ABOVE {
        county.unrest = county.unrest.saturating_sub(1);
    } else if county.happiness < climb_below {
        county.unrest = county.unrest.saturating_add(1).min(UNREST_REVOLT);
    }
    // 1..=10 with the quirk reproduced: docs/kingdom.md §6 gives no rule. See
    // the module comment.
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Faithful. The switched-off answers live in `tests/quirks.rs`.
    #[allow(dead_code)]
    const Q: Quirks = Quirks::FAITHFUL;

    fn owned_by(owner: u8, happiness: i32) -> County {
        let mut c = County::new();
        c.owner = owner;
        c.happiness = happiness;
        c
    }

    /// **The manual, exactly** — and read for its *whole* sentence this time,
    /// which is `docs/agents.md`'s *"citing an oracle is not reading it"*:
    ///
    /// > *"When any county's happiness rating drops below 25 and stays there
    /// > for **more than four seasons**, its population will revolt."*
    ///
    /// **More than four is five, and five is what the corrected pass gives.**
    /// The first season below 30 spends itself on the warning — the
    /// `Msg_Enqueue(0x92)` arm is an `if` whose `else` is the entire ladder —
    /// so the counter starts on season two and reaches 4 on season five. This
    /// test used to say *"on the fourth season"*, which needed the manual's
    /// *"more than"* to mean *"at least"*.
    ///
    /// `update` now returns *"the counter rose to 4"*, which is exactly what
    /// `0x0044AA41` tests before calling `County_RaiseRevolt`; the reset and the
    /// `Revolt` message belong to the caller, because the original only does
    /// them when the mob was actually placed.
    #[test]
    fn a_human_county_below_twenty_five_revolts_after_more_than_four_seasons() {
        let mut c = owned_by(1, 24);
        let mut out = Vec::new();

        assert!(!update(&mut c, 7, true, Q, &mut out), "season one is the warning");
        assert_eq!(c.unrest, 0, "and the ladder is its `else`, so nothing moves");
        assert!(out.contains(&Message::UnrestWarning { county: 7 }));

        for season in 1..=3u8 {
            assert!(!update(&mut c, 7, true, Q, &mut out), "season {} is too early", season + 1);
            assert_eq!(c.unrest, season);
        }
        assert!(update(&mut c, 7, true, Q, &mut out), "the fifth season revolts");
        assert_eq!(c.unrest, UNREST_REVOLT, "and the counter is the caller's to reset");

        for level in 1..=4u8 {
            assert!(out.contains(&Message::UnrestRising { county: 7, level }));
        }
    }

    /// **A county that cannot rise again does not try again.** Both branches
    /// guard the revolt call with `if (before < after)`, so a county left at 4
    /// — which is what happens when `County_RaiseRevolt` finds nowhere to put
    /// the mob — sits there for ever.
    ///
    /// Ablation: delete `before < county.unrest &&` from [`update`]'s last line
    /// and this goes red on the first extra season.
    #[test]
    fn a_county_stuck_at_four_never_asks_for_a_second_revolt() {
        let mut c = owned_by(1, 24);
        let mut out = Vec::new();
        for _ in 0..5 {
            update(&mut c, 7, true, Q, &mut out);
        }
        assert_eq!(c.unrest, UNREST_REVOLT, "the caller declined to reset it");
        for season in 1..=20 {
            assert!(
                !update(&mut c, 7, true, Q, &mut out),
                "season {season} asked for a second revolt off an unchanged counter"
            );
        }
    }

    #[test]
    fn a_human_county_at_exactly_twenty_five_never_revolts() {
        let mut c = owned_by(1, 25);
        let mut out = Vec::new();
        for _ in 0..100 {
            assert!(!update(&mut c, 1, true, Q, &mut out));
        }
        assert_eq!(c.unrest, 0);
    }

    /// The warning is fired once, not every season, and re-arms only after the
    /// county climbs back to 30.
    #[test]
    fn the_warning_fires_once_and_re_arms_at_thirty() {
        let mut c = owned_by(1, 29);
        let mut out = Vec::new();
        for _ in 0..5 {
            update(&mut c, 3, true, Q, &mut out);
        }
        let warnings = out.iter().filter(|m| matches!(m, Message::UnrestWarning { .. })).count();
        assert_eq!(warnings, 1);
        // Between 25 and 30 the county is warned but the counter does not move.
        assert_eq!(c.unrest, 0);

        c.happiness = 30;
        update(&mut c, 3, true, Q, &mut out);
        c.happiness = 29;
        update(&mut c, 3, true, Q, &mut out);
        let warnings = out.iter().filter(|m| matches!(m, Message::UnrestWarning { .. })).count();
        assert_eq!(warnings, 2, "the flag re-armed");
    }

    #[test]
    fn an_ai_county_above_forty_is_reset_outright() {
        let mut c = owned_by(2, 41);
        c.unrest = 3;
        let mut out = Vec::new();
        update(&mut c, 1, false, Q, &mut out);
        assert_eq!(c.unrest, 0);
        assert!(out.is_empty(), "the AI ladder raises no messages");
    }

    #[test]
    fn an_ai_county_between_eleven_and_forty_walks_its_counter_down() {
        let mut c = owned_by(2, 20);
        c.unrest = 3;
        let mut out = Vec::new();
        for expected in [2u8, 1, 0, 0] {
            update(&mut c, 1, false, Q, &mut out);
            assert_eq!(c.unrest, expected);
        }
    }

    #[test]
    fn an_ai_county_at_zero_happiness_walks_its_counter_up_and_stops_at_four() {
        let mut c = owned_by(2, 0);
        let mut out = Vec::new();
        for expected in [1u8, 2, 3, 4, 4, 4] {
            update(&mut c, 1, false, Q, &mut out);
            assert_eq!(c.unrest, expected);
        }
    }

    /// The hole in the documented AI ladder, asserted so that it is visible
    /// rather than accidental. If this test ever has to change, the ladder was
    /// wrong in the document.
    #[test]
    fn the_ai_ladder_has_a_documented_dead_band_between_one_and_ten() {
        for happiness in 1..=10 {
            let mut c = owned_by(2, happiness);
            c.unrest = 2;
            let mut out = Vec::new();
            update(&mut c, 1, false, Q, &mut out);
            assert_eq!(c.unrest, 2, "happiness {happiness} moves nothing");
        }
    }

    /// **An AI county revolts too, and this test used to assert that it does
    /// not.** `docs/kingdom.md` §6 puts the revolt call on the human ladder,
    /// and it is wrong: in `0x0044AA41` the call sits at `LAB_0044ADF3`
    /// *inside the AI branch*, and the human branch reaches it with a `goto`.
    /// One block, two callers.
    ///
    /// The asymmetry that is real is the *ladder*, not the revolt: an AI county
    /// only climbs below happiness 1 where a human's climbs below 25, which is
    /// what `docs/rules.md` §5a means by *"the peasants rise against you far
    /// more readily than against them"*.
    ///
    /// It raises **no messages** either way — the four `Msg_Enqueue` calls are
    /// in the human branch only — so an AI realm loses a county in silence.
    #[test]
    fn an_ai_county_at_zero_happiness_revolts_on_its_fourth_season_and_says_nothing() {
        let mut c = owned_by(2, 0);
        let mut out = Vec::new();
        for season in 1..=3u8 {
            assert!(!update(&mut c, 1, false, Q, &mut out), "season {season} is too early");
            assert_eq!(c.unrest, season);
        }
        assert!(update(&mut c, 1, false, Q, &mut out), "the fourth season revolts");
        assert!(out.is_empty(), "and the AI ladder raises no messages at all");
        // And then it is stuck, for the same reason a human county is.
        for _ in 0..10 {
            assert!(!update(&mut c, 1, false, Q, &mut out));
        }
    }

    #[test]
    fn an_unowned_county_is_on_neither_ladder() {
        let mut c = owned_by(0, 0);
        let mut out = Vec::new();
        for _ in 0..50 {
            assert!(!update(&mut c, 1, false, Q, &mut out));
        }
        assert_eq!(c.unrest, 0);
        assert!(out.is_empty());
    }

    /// **A human county's counter is cleared outright the moment happiness
    /// reaches 25, and this test used to assert the opposite.**
    ///
    /// The old version said *"recovery does not clear the counter"* and called
    /// the stickiness *"real in the documented rules"*. `0x0044AA41`'s human
    /// branch is `if (happiness < 0x19) { ...climb... } else { unrest = 0; }`,
    /// the same shape as the AI branch's `>= 41` arm. It is not sticky, and no
    /// document said it was — the claim came from an early reading of a ladder
    /// that had no `else` written down.
    ///
    /// Which makes revolt a much harder thing to reach than the old rule: a
    /// county needs **five consecutive seasons** below 25, not five seasons
    /// below 25 spread over a reign.
    #[test]
    fn one_season_at_twenty_five_wipes_a_human_countys_unrest_counter() {
        let mut c = owned_by(1, 24);
        let mut out = Vec::new();
        update(&mut c, 1, true, Q, &mut out); // the warning season
        for expected in [1u8, 2, 3] {
            update(&mut c, 1, true, Q, &mut out);
            assert_eq!(c.unrest, expected);
        }
        c.happiness = 25;
        update(&mut c, 1, true, Q, &mut out);
        assert_eq!(c.unrest, 0, "twenty-five is the `else`, and the `else` is `unrest = 0`");
    }
}

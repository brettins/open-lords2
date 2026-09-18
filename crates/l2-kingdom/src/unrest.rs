//! Unrest and revolt — `docs/kingdom.md` §6, `Unrest_UpdateAll`
//! (`0x0044AA41`).
//!
//! `docs/kingdom.md` §6 gives the AI ladder as *"happiness >= 41 resets it to
//! 0; 11 … 40 walks it down; below 1 walks it up"* — which says nothing about
//! happiness 1..=10. That is reproduced literally here: a dead band where an AI
//! county's unrest neither rises nor falls. It is more likely that the document
//! abbreviated a `< 11` into a `< 1` than that the original really has a hole,
//! but guessing which would be exactly the failure `docs/decisions.md` C3
//! records, so the hole stays and this comment stays with it.
//!
//! `Unrest_UpdateAll` was read again end-to-end for the hundred-turn game
//! (`docs/plan.md` §2.5), because a hundred turns of England raises **twenty
//! revolts** and none of them did anything. Three corrections came out of it,
//! all `[V]` from `0x0044AA41`'s own body — `docs/decisions.md` C90.
//!
//! It is the C58 shape exactly: a claim read out of a document
//! of the function.

use crate::county::County;
use crate::map::CampaignMap;
use crate::report::Message;
use crate::levy::OWNERLESS;
use crate::unit::{Unit, UnitKind, Units};
use l2_net::{Quirk, Quirks};

pub const UNREST_REVOLT: u8 = 4;

pub const HUMAN_WARN_BELOW: i32 = 30;

pub const HUMAN_UNREST_BELOW: i32 = 25;

pub const AI_CALM_AT_OR_ABOVE: i32 = 41;

pub const AI_SETTLE_AT_OR_ABOVE: i32 = 11;

pub const AI_UNREST_BELOW: i32 = 1;

/// The caller does the raising and resets the counter only if a mob was
/// — see [`raise_revolt`], and `0x0044AA41`'s
/// `if (3 < unrest && County_RaiseRevolt(c)) unrest = 0;`.
pub fn update(
    county: &mut County,
    id: u8,
    owner_is_human: bool,
    quirks: Quirks,
    out: &mut Vec<Message>,
) -> bool {
    if county.is_unowned() {
        county.unrest = 0;
        return false;
    }

    let before = county.unrest;
    if owner_is_human {
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
        } else {
            county.unrest = 0;
        }
        if before < county.unrest {
            out.push(Message::UnrestRising { county: id, level: county.unrest });
        }
    } else {
        update_ai(county, quirks);
    }

    before < county.unrest && county.unrest >= UNREST_REVOLT
}

/// `County_RaiseRevolt` (`0x004AC185`) — what unrest 4
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

pub const REVOLT_POPULATION_PCT: i32 = 30;

pub const REVOLT_MORALE: i32 = 50;

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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[allow(dead_code)]
    const Q: Quirks = Quirks::FAITHFUL;

    fn owned_by(owner: u8, happiness: i32) -> County {
        let mut c = County::new();
        c.owner = owner;
        c.happiness = happiness;
        c
    }

    /// `update` now returns *"the counter rose to 4"*, which is exactly what
    /// `0x0044AA41` tests before calling `County_RaiseRevolt`; the reset and the
    /// `Revolt` message belong to the caller, because the original only does
    /// them when the mob
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

    #[test]
    fn the_warning_fires_once_and_re_arms_at_thirty() {
        let mut c = owned_by(1, 29);
        let mut out = Vec::new();
        for _ in 0..5 {
            update(&mut c, 3, true, Q, &mut out);
        }
        let warnings = out.iter().filter(|m| matches!(m, Message::UnrestWarning { .. })).count();
        assert_eq!(warnings, 1);
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

    ///. If this test ever has to change, the ladder was
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

    /// The old version said *"recovery does not clear the counter"* and called
    /// the stickiness *"real in the documented rules"*. `0x0044AA41`'s human
    /// branch is `if (happiness < 0x19) { ...climb... } else { unrest = 0; }`,
    /// the same shape as the AI branch's `>= 41` arm. It is not sticky, and no
    /// document said it was — the claim came from an early reading of a ladder
    /// that had no `else` written down.
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

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

use crate::county::County;
use crate::report::Message;

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

/// One county's unrest pass. Returns `true` if the county revolted.
///
/// `id` is the county's index, carried only so the messages can name it.
pub fn update(
    county: &mut County,
    id: u8,
    owner_is_human: bool,
    out: &mut Vec<Message>,
) -> bool {
    if county.is_unowned() {
        // Realm 0 is not a realm and has no peasants to lose. Neither ladder
        // applies; docs/kingdom.md §6 describes the two owned cases only.
        return false;
    }

    if owner_is_human {
        update_human(county, id, out)
    } else {
        update_ai(county);
        false
    }
}

fn update_human(county: &mut County, id: u8, out: &mut Vec<Message>) -> bool {
    if county.happiness >= HUMAN_WARN_BELOW {
        county.unrest_warned = false;
    } else if !county.unrest_warned {
        county.unrest_warned = true;
        out.push(Message::UnrestWarning { county: id });
    }

    if county.happiness >= HUMAN_UNREST_BELOW {
        return false;
    }

    county.unrest = county.unrest.saturating_add(1);
    out.push(Message::UnrestRising { county: id, level: county.unrest });

    if county.unrest >= UNREST_REVOLT {
        // FUN_004AC185 raises the revolting-peasant army - unit type 2, the one
        // phase 5 moves - and resets the counter. Raising the mob is a unit
        // operation and therefore not this crate's; the reset is.
        county.unrest = 0;
        out.push(Message::Revolt { county: id });
        return true;
    }
    false
}

fn update_ai(county: &mut County) {
    if county.happiness >= AI_CALM_AT_OR_ABOVE {
        county.unrest = 0;
    } else if county.happiness >= AI_SETTLE_AT_OR_ABOVE {
        county.unrest = county.unrest.saturating_sub(1);
    } else if county.happiness < AI_UNREST_BELOW {
        county.unrest = county.unrest.saturating_add(1).min(UNREST_REVOLT);
    }
    // 1..=10: docs/kingdom.md §6 gives no rule. See the module comment.
}

#[cfg(test)]
mod tests {
    use super::*;

    fn owned_by(owner: u8, happiness: i32) -> County {
        let mut c = County::new();
        c.owner = owner;
        c.happiness = happiness;
        c
    }

    /// **The manual, exactly**: *"When any county's happiness rating drops
    /// below 25 and stays there for more than four seasons, its population will
    /// revolt."*
    #[test]
    fn a_human_county_below_twenty_five_revolts_on_the_fourth_season() {
        let mut c = owned_by(1, 24);
        let mut out = Vec::new();
        for season in 1..=3 {
            assert!(!update(&mut c, 7, true, &mut out), "season {season} is too early");
            assert_eq!(c.unrest, season);
        }
        assert!(update(&mut c, 7, true, &mut out), "the fourth season revolts");
        assert_eq!(c.unrest, 0, "and the counter resets");

        assert!(out.contains(&Message::Revolt { county: 7 }));
        for level in 1..=4u8 {
            assert!(out.contains(&Message::UnrestRising { county: 7, level }));
        }
    }

    #[test]
    fn a_human_county_at_exactly_twenty_five_never_revolts() {
        let mut c = owned_by(1, 25);
        let mut out = Vec::new();
        for _ in 0..100 {
            assert!(!update(&mut c, 1, true, &mut out));
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
            update(&mut c, 3, true, &mut out);
        }
        let warnings = out.iter().filter(|m| matches!(m, Message::UnrestWarning { .. })).count();
        assert_eq!(warnings, 1);
        // Between 25 and 30 the county is warned but the counter does not move.
        assert_eq!(c.unrest, 0);

        c.happiness = 30;
        update(&mut c, 3, true, &mut out);
        c.happiness = 29;
        update(&mut c, 3, true, &mut out);
        let warnings = out.iter().filter(|m| matches!(m, Message::UnrestWarning { .. })).count();
        assert_eq!(warnings, 2, "the flag re-armed");
    }

    #[test]
    fn an_ai_county_above_forty_is_reset_outright() {
        let mut c = owned_by(2, 41);
        c.unrest = 3;
        let mut out = Vec::new();
        update(&mut c, 1, false, &mut out);
        assert_eq!(c.unrest, 0);
        assert!(out.is_empty(), "the AI ladder raises no messages");
    }

    #[test]
    fn an_ai_county_between_eleven_and_forty_walks_its_counter_down() {
        let mut c = owned_by(2, 20);
        c.unrest = 3;
        let mut out = Vec::new();
        for expected in [2u8, 1, 0, 0] {
            update(&mut c, 1, false, &mut out);
            assert_eq!(c.unrest, expected);
        }
    }

    #[test]
    fn an_ai_county_at_zero_happiness_walks_its_counter_up_and_stops_at_four() {
        let mut c = owned_by(2, 0);
        let mut out = Vec::new();
        for expected in [1u8, 2, 3, 4, 4, 4] {
            update(&mut c, 1, false, &mut out);
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
            update(&mut c, 1, false, &mut out);
            assert_eq!(c.unrest, 2, "happiness {happiness} moves nothing");
        }
    }

    /// An AI county never revolts through this pass, whatever its counter says.
    /// `docs/kingdom.md` §6 puts the revolt call on the human ladder only.
    #[test]
    fn an_ai_county_never_raises_a_mob() {
        let mut c = owned_by(2, 0);
        let mut out = Vec::new();
        for _ in 0..50 {
            assert!(!update(&mut c, 1, false, &mut out));
        }
        assert!(out.is_empty());
    }

    #[test]
    fn an_unowned_county_is_on_neither_ladder() {
        let mut c = owned_by(0, 0);
        let mut out = Vec::new();
        for _ in 0..50 {
            assert!(!update(&mut c, 1, false, &mut out));
        }
        assert_eq!(c.unrest, 0);
        assert!(out.is_empty());
    }

    /// A county that dips below 25, recovers, and dips again does not carry its
    /// old count into the new crisis at full speed - the counter only falls on
    /// the AI ladder, so a human county's counter is sticky. That asymmetry is
    /// real in the documented rules and worth pinning.
    #[test]
    fn a_human_county_s_unrest_counter_never_falls_on_its_own() {
        let mut c = owned_by(1, 24);
        let mut out = Vec::new();
        update(&mut c, 1, true, &mut out);
        assert_eq!(c.unrest, 1);
        c.happiness = 100;
        for _ in 0..20 {
            update(&mut c, 1, true, &mut out);
        }
        assert_eq!(c.unrest, 1, "recovery does not clear the counter");
    }
}

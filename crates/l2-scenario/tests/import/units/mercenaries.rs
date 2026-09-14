#![allow(unused_imports)]
use super::*;
use super::generic_units::*;
use super::trade_units::*;
use super::*;
use super::county::*;
use super::economy::*;
use l2_formats::save::{Layout, Save, COUNTY_BASE, COUNTY_STRIDE};
use l2_scenario::{ImportError, Scenario, STARTING_HEALTH_METER};
use l2_testkit::{saves, SaveFile};

/// `g_mercBands` (`0x00568DC0`), stride `0x14`, and `g_mercBandsInPlay`.
const MERC_BANDS: u32 = 0x0056_8DC0;
const MERC_STRIDE: u32 = 0x14;
const MERC_IN_PLAY: u32 = 0x0055_4030;

/// **Every mercenary band in every save reaches the kingdom holding the file's
/// walk, and every county's offer is `Mercenary_OfferInCounty` over it.**
///
/// A loaded game used to have no bands in play at all, so the raise-army screen
/// never offered one and the town square never showed one. The comparison is
/// the kingdom against the **bytes**, band by band, and two of the assertions
/// are the table checking itself:
///
/// * a band that offered itself this season has **just reloaded its countdown
///   and stepped one past the county it stands in**, with no wrap — the tail of
///   `Mercenary_AdvanceAll`'s offer branch — which ties `+0x03`, `+0x04`,
///   `+0x06` and `+0x07` together on every offering band on disk;
/// * county `+0x1AD` is the **lowest-numbered** unhired band offered there, and
///   `siege-old_turn.sav` puts bands 2 and 3 in county 1 at once.
///
/// **Ablation, run:** delete `k.campaign.mercenaries = self.mercenaries.clone()`
/// from `Scenario::skeleton` and the in-play assertion fails on the first save.
#[test]
fn every_mercenary_band_reaches_the_kingdom_and_every_offer_is_its_cache() {
    let saves = saves!();
    let (mut offers, mut offering, mut bands_seen) = (0usize, 0usize, 0usize);
    for s in &saves {
        let save = &s.save;
        let scenario = Scenario::from_save(save).unwrap_or_else(|e| panic!("{}: {e}", s.label()));
        let k = scenario.kingdom(1);
        let bands = &k.campaign.mercenaries;
        let in_play = save.i32_at(MERC_IN_PLAY).unwrap();
        assert_eq!(bands.in_play() as i32, in_play, "{}: g_mercBandsInPlay", s.label());
        assert_eq!(
            bands.in_play(),
            l2_kingdom::mercenary::bands_in_play(scenario.county_count),
            "{}: g_mercBandCount[g_countyCount]",
            s.label()
        );
        for slot in 0..l2_kingdom::mercenary::BAND_SLOTS {
            let at = MERC_BANDS + slot as u32 * MERC_STRIDE;
            if slot == 0 || slot as i32 > in_play {
                for off in 0..MERC_STRIDE {
                    assert_eq!(
                        save.u8_at(at + off).unwrap(),
                        0,
                        "{}: slot {slot} is not a band in play and holds a byte at +{off:#x}",
                        s.label()
                    );
                }
                continue;
            }
            bands_seen += 1;
            let b = bands.get(slot as u8).expect("in play");
            let what = |field: &str| format!("{}: band {slot} {field}", s.label());
            assert_eq!(b.hired_by as i16, save.i16_at(at).unwrap(), "{}", what("+0x00 hired by"));
            assert_eq!(b.offered_in, save.u8_at(at + 3).unwrap(), "{}", what("+0x03 offered in"));
            assert_eq!(b.next_county, save.u8_at(at + 4).unwrap(), "{}", what("+0x04 next county"));
            assert_eq!(b.countdown, save.i8_at(at + 6).unwrap(), "{}", what("+0x06 countdown"));
            assert_eq!(b.reload, save.i8_at(at + 7).unwrap(), "{}", what("+0x07 reload"));
            assert!((1..=b.reload).contains(&b.countdown), "{}", what("countdown outside 1..=reload"));
            if b.offered_in != 0 {
                offering += 1;
                assert_eq!(b.countdown, b.reload, "{}", what("offered, so the countdown just reloaded"));
                assert_eq!(b.next_county, b.offered_in + 1, "{}", what("offered, so the walk stepped past"));
            }
        }
        for id in scenario.county_ids() {
            let cached = k.counties[id].mercenary_offer;
            assert_eq!(
                cached,
                bands.offer_in(id as u8),
                "{}: county {id}'s +0x1AD is not Mercenary_OfferInCounty over the table",
                s.label()
            );
            offers += usize::from(cached != 0);
        }
    }
    eprintln!(
        "{} saves: {bands_seen} bands compared, {offering} of them offering, {offers} county offers",
        saves.len()
    );
}

/// The four one-End-Turn pairs on disk — `crates/l2-game/tests/differential.rs`
/// establishes which files are a turn apart, and it is not the pairs their names
/// suggest.
const TURN_PAIRS: [(&str, &str); 4] = [
    ("safeturn.sav", "old_turn.sav"),
    ("old_turn.sav", "battle-before.sav"),
    ("siege-safeturn.sav", "siege-old_turn.sav"),
    ("siege-old_turn.sav", "siege-lastturn.sav"),
];

/// **One season of `Mercenary_AdvanceAll` over a save lands on the next save's
/// band table and county offers, exactly.**
///
/// The strongest check the band import has, because nothing in it is ours
/// agreeing with ours: the *after* table was written by the original a turn
/// later. It covers all three walk branches on data — a band that only counts
/// down, one that wraps past the last county, and one that offers itself (four
/// offers across the pairs, including the two-band collision in county 1).
#[test]
fn one_season_of_the_mercenary_walk_lands_on_the_next_saves_table() {
    for (before, after) in TURN_PAIRS {
        let b = l2_testkit::fixture!(before);
        let a = l2_testkit::fixture!(after);
        let sb = Scenario::from_save(&b).expect("imports");
        let sa = Scenario::from_save(&a).expect("imports");
        assert_eq!(sa.clock.turn_count, sb.clock.turn_count + 1, "{before} -> {after} is one turn");
        let mut k = sb.kingdom(1);
        let (count, quirks) = (k.county_count, k.options.quirks);
        let l2_kingdom::Kingdom { counties, campaign, .. } = &mut k;
        campaign.mercenaries.advance(counties, count, quirks);
        let ka = sa.kingdom(1);
        assert_eq!(k.campaign.mercenaries, ka.campaign.mercenaries, "{before} -> {after}: the bands");
        for id in sa.county_ids() {
            assert_eq!(
                k.counties[id].mercenary_offer, ka.counties[id].mercenary_offer,
                "{before} -> {after}: county {id}'s offer"
            );
        }
    }
}

/// A band whose constant fields disagree with the roster is refused: the kingdom
/// keeps those in the roster, so importing the rest would price the band at a
/// number the file does not hold.
#[test]
fn a_mercenary_band_that_disagrees_with_the_roster_is_refused() {
    // Band 1's price, 1800 = 0x708: its low byte, one higher.
    refusal_over_every_save(
        |_| MERC_BANDS + MERC_STRIDE + 0x0C,
        0x09,
        |_| ImportError::MercenaryRoster { band: 1, field: "price", file: 1801, roster: 1800 },
    );
}

#[test]
fn a_mercenary_band_offered_off_the_map_is_refused() {
    refusal_over_every_save(
        |_| MERC_BANDS + MERC_STRIDE + 0x03,
        99,
        |_| ImportError::MercenaryState { band: 1, field: "offered county", value: 99 },
    );
}

/// Slot 1 is a merchant in every save on the machine — the merchants take the
/// low slots — and a merchant cannot carry a band.
#[test]
fn a_mercenary_band_hired_by_something_that_does_not_carry_it_is_refused() {
    refusal_over_every_save(
        |_| MERC_BANDS + MERC_STRIDE,
        1,
        |_| ImportError::MercenaryState { band: 1, field: "hiring unit", value: 1 },
    );
}

#[test]
fn a_band_count_past_twelve_is_refused() {
    refusal_over_every_save(|_| MERC_IN_PLAY, 13, |_| ImportError::MercenaryBandCount(13));
}

/// The raise-army screen indexes the roster with the county's byte.
#[test]
fn a_county_offering_a_band_that_is_not_in_play_is_refused() {
    refusal_over_every_save(
        |_| COUNTY_BASE + COUNTY_STRIDE as u32 + 0x1AD,
        13,
        |_| ImportError::MercenaryOffer { county: 1, band: 13 },
    );
}

/// **A hired band arrives hired, on the army that carries it** — the one half of
/// the band table no save on this machine exercises, because `+0x00` is zero in
/// every band of every save. So this writes a consistent hire into a copy of the
/// first save with an army in it: the army's `+0x195…+0x197` and the band's
/// hirer, and nothing else.
#[test]
fn a_hired_band_arrives_hired_on_the_army_that_carries_it() {
    let exe = l2_testkit::executable!();
    let saves = saves!();
    let Some((s, slot)) = saves.iter().find_map(|s| {
        s.save.units().ok()?.iter().find(|u| u.is_live() && u.kind == 1).map(|u| (s, u.index))
    }) else {
        l2_testkit::skip!("no save on this machine holds an army");
    };
    let unit = l2_formats::save::UNIT_BASE + (slot * l2_formats::save::UNIT_STRIDE) as u32;
    let bytes = std::fs::read(&s.path).expect("re-read");
    let mut poked = poke(&exe, &bytes, unit + 0x195, 4); // pikemen
    poked = poke(&exe, &poked, unit + 0x196, 100);
    poked = poke(&exe, &poked, unit + 0x197, 1); // the Scottish band
    poked = poke(&exe, &poked, MERC_BANDS + MERC_STRIDE, slot as u8);
    let save = Save::open(&exe, &poked).expect("still the right length");
    let scenario = Scenario::from_save(&save).unwrap_or_else(|e| panic!("{}: {e}", s.label()));
    let k = scenario.kingdom(1);
    let band = k.campaign.mercenaries.get(1).expect("in play");
    assert_eq!(band.hired_by as usize, slot, "{}", s.label());
    assert!(!band.is_available());
    assert_eq!(
        k.campaign.units.get(slot).and_then(|u| u.mercenaries).map(|m| (m.band, m.men)),
        Some((1, 100)),
        "{}: slot {slot} carries the band",
        s.label()
    );
    for id in scenario.county_ids() {
        assert_ne!(k.campaign.mercenaries.offer_in(id as u8), 1, "a hired band is offered nowhere");
    }
}



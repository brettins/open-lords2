//! The campaign layer driven through whole seasons — `docs/armies.md`.
//!
//! The unit tests in `src/` each check one rule against the decompiled function
//! it came from. This checks the things that only exist once the rules are
//! *composed into a turn*, which is where the hooks `docs/armies.md` §6.4 calls
//! *"the missing half"* actually bite:
//!
//! * the starvation ladder over five consecutive seasons, including the
//!   ordering that makes an army desert and then be billed the reduced wage in
//!   the same season;
//! * army food as extra mouths at the county's ration level, which is a
//!   *multiplier* on the ration and not a flat subtraction;
//! * the mercenary bands walking the map for a year;
//! * `disabled_seasons` counting itself back down after a trampling;
//! * a whole kingdom with armies in it round-tripping through a save and
//!   playing on identically.
//!
//! Needs no game install: everything here is built by hand.

use l2_kingdom::conquest;
use l2_kingdom::county::MAX_COUNTIES;
use l2_kingdom::map::{flags, CampaignMap, MAP_DIM, MAP_TILES};
use l2_kingdom::movement::Routing;
use l2_kingdom::phase::Pass;
use l2_kingdom::report::Message;
use l2_kingdom::unit::{Unit, UnitKind, Units};
use l2_kingdom::{Kingdom, MercenaryBands, Options, TroopType};

/// A two-county kingdom with a map: county 1 in the west, county 2 in the east,
/// realm 1 holding the first and nobody holding the second.
fn kingdom() -> Kingdom {
    let mut k = Kingdom::new(0xA12);
    assert!(k.set_county_count(2));
    k.season = 1;
    k.season_next = 2;
    k.year = 1268;
    k.year_next = 1269;
    k.turn_count = 1;

    k.realms[1].in_play = true;
    k.realms[1].is_human = true;
    k.realms[1].shield_index = 2;
    k.realms[1].gold = 10_000;

    for id in 1..=2 {
        let c = &mut k.counties[id];
        c.population = 500;
        c.happiness = 77;
        c.health_meter = 65;
        c.herd = 100;
        c.grain = 200;
        c.ration_wanted = 3;
        c.ration_split = 100;
        c.fields_grain = 4;
        c.fields_cattle = 4;
        c.crop[1] = 400;
    }
    k.counties[1].owner = 1;

    let mut map = CampaignMap::empty();
    for i in 0..MAP_TILES {
        map.county[i] = if i % MAP_DIM < 32 { 1 } else { 2 };
    }
    k.campaign.map = map;
    k.campaign.mercenaries = MercenaryBands::init(2);
    k
}

fn army(k: &mut Kingdom, owner: u8, county: u8, men: i32, x: u8, y: u8) -> usize {
    let mut u = Unit::new(UnitKind::Army, owner, x, y);
    u.men = men;
    u.troops[TroopType::Peasant.index()] = men;
    u.county = county;
    u.home_county = county;
    u.owner_is_human = k.realms[owner as usize].is_human;
    k.campaign.units.spawn(u).expect("a free slot")
}

// ---------------------------------------------------------------------------
// The season hooks
// ---------------------------------------------------------------------------

/// **The whole starvation ladder, season by season.** `docs/armies.md` §3.3b:
/// the counter warns at 1, deserts at 2, 3 and 4, and destroys the army at 5.
#[test]
fn a_starving_army_warns_then_deserts_three_times_then_dies() {
    let mut k = kingdom();
    k.options = Options { difficulty: 0, advanced_farming: false, armies_eat: true, ..Options::default() };
    // A county with nothing at all in it, so `Food_Available` is 0 and any army
    // is unfed.
    k.counties[1].herd = 0;
    k.counties[1].grain = 0;
    k.counties[1].herd_available = 0;
    k.counties[1].grain_available = 0;
    let id = army(&mut k, 1, 1, 700, 5, 5);
    for t in 0..7 {
        k.campaign.units.get_mut(id).unwrap().troops[t] = 100;
    }

    let mut seen = Vec::new();
    let mut men = Vec::new();
    for _ in 0..5 {
        let mut report = l2_kingdom::report::SeasonReport::new();
        k.run_pass(Pass::WagesPay, &mut report);
        // Keep the county empty: the season's other passes are not run here.
        k.counties[1].herd = 0;
        k.counties[1].grain = 0;
        seen.push(
            report
                .messages
                .iter()
                .filter_map(|m| match m {
                    Message::ArmyStarving { stage, .. } => Some(*stage),
                    _ => None,
                })
                .collect::<Vec<_>>(),
        );
        men.push(k.campaign.units.get(id).map(|u| u.men));
    }

    assert_eq!(seen, vec![vec![1], vec![2], vec![3], vec![4], vec![5]]);
    // 700 -> warn -> three rounds of ten percent -> gone.
    assert_eq!(men[0], Some(700), "the first season only warns");
    assert_eq!(men[1], Some(630));
    assert_eq!(men[2], Some(567));
    assert_eq!(men[3], Some(511));
    assert_eq!(men[4], None, "the fifth season destroys it");
    assert_eq!(k.realms[1].wages, 0, "and the bill goes with it");
}

/// A garrison never starves, however empty the county is.
#[test]
fn a_garrisoned_army_is_fed_by_the_castle() {
    let mut k = kingdom();
    k.options.armies_eat = true;
    k.counties[1].herd = 0;
    k.counties[1].grain = 0;
    let id = army(&mut k, 1, 1, 400, 5, 5);
    k.campaign.units.get_mut(id).unwrap().garrison_county = 1;

    for _ in 0..8 {
        let mut report = l2_kingdom::report::SeasonReport::new();
        k.run_pass(Pass::WagesPay, &mut report);
        assert!(report.messages.iter().all(|m| !matches!(m, Message::ArmyStarving { .. })));
    }
    assert_eq!(k.campaign.units.get(id).unwrap().men, 400);
    assert_eq!(k.campaign.units.get(id).unwrap().starvation, 0);
}

/// With *Army foraging* off the counter is only ever cleared — the option gates
/// the whole rule.
#[test]
fn with_foraging_off_nothing_starves_at_all() {
    let mut k = kingdom();
    k.options.armies_eat = false;
    k.counties[1].herd = 0;
    k.counties[1].grain = 0;
    let id = army(&mut k, 1, 1, 900, 5, 5);
    k.campaign.units.get_mut(id).unwrap().starvation = 3;

    let mut report = l2_kingdom::report::SeasonReport::new();
    k.run_pass(Pass::WagesPay, &mut report);
    assert_eq!(k.campaign.units.get(id).unwrap().starvation, 0);
    assert_eq!(k.campaign.units.get(id).unwrap().men, 900);
}

/// **An army is an extra mouth at the county's ration level, not a flat
/// subtraction** — `docs/armies.md` §3.3a. The cost of the same army *triples*
/// when the county goes from Normal to Triple rations, which a subtraction
/// could not do.
#[test]
fn an_armys_food_cost_scales_with_the_countys_ration_level() {
    let eaten = |level: i32, troops: i32| {
        let mut k = kingdom();
        k.options.armies_eat = true;
        k.counties[1].ration_wanted = level;
        k.counties[1].friendly_troops = troops;
        k.counties[1].herd = 0;
        k.counties[1].grain = 10_000;
        k.counties[1].ration_split = 0; // all grain, so the answer is in sacks
        let mut report = l2_kingdom::report::SeasonReport::new();
        k.run_pass(Pass::RationApply, &mut report);
        k.counties[1].grain_eaten
    };

    let normal_alone = eaten(3, 0);
    let normal_with = eaten(3, 300);
    let triple_alone = eaten(5, 0);
    let triple_with = eaten(5, 300);

    let normal_cost = normal_with - normal_alone;
    let triple_cost = triple_with - triple_alone;
    assert!(normal_cost > 0, "the troops eat something");
    assert_eq!(
        triple_cost,
        normal_cost * 3,
        "three hundred men cost three times as much at Triple rations \
         ({normal_cost} -> {triple_cost}), which a flat subtraction could not do"
    );
}

/// Enemy troops eat out of your county's store too — the field pair exists for
/// exactly that.
#[test]
fn an_occupying_army_eats_the_countys_food() {
    let mut k = kingdom();
    k.options.armies_eat = true;
    k.counties[1].ration_split = 0;
    k.counties[1].herd = 0;
    k.counties[1].grain = 10_000;

    let mut report = l2_kingdom::report::SeasonReport::new();
    k.run_pass(Pass::RationApply, &mut report);
    let alone = k.counties[1].grain_eaten;

    let mut k = kingdom();
    k.options.armies_eat = true;
    k.counties[1].ration_split = 0;
    k.counties[1].herd = 0;
    k.counties[1].grain = 10_000;
    k.counties[1].enemy_troops = 400;
    let mut report = l2_kingdom::report::SeasonReport::new();
    k.run_pass(Pass::RationApply, &mut report);
    assert!(k.counties[1].grain_eaten > alone, "the invader is fed by the invaded");
}

/// The recount is what puts an army on the county's food bill, and it is driven
/// by where the army is standing rather than by who owns it.
#[test]
fn marching_an_army_across_a_border_moves_which_county_feeds_it() {
    let mut k = kingdom();
    let id = army(&mut k, 1, 1, 300, 30, 10);
    let realms = k.realms.clone();
    k.campaign.units.recount_county_troops(&mut k.counties, &realms);
    assert_eq!(k.counties[1].friendly_troops, 300);
    assert_eq!(k.counties[2].enemy_troops, 0);

    // Two tiles east is county 2, which realm 1 does not own.
    l2_kingdom::movement::order_move(&k.campaign.map, &mut k.campaign.units, id, (33, 10), Routing::Direct);
    let Kingdom { campaign, counties, realms, .. } = &mut k;
    let realms_snapshot = realms.clone();
    l2_kingdom::movement::march(&mut campaign.map, counties, &realms_snapshot, &mut campaign.units, id);
    assert_eq!(k.campaign.units.get(id).unwrap().county, 2);

    let realms = k.realms.clone();
    k.campaign.units.recount_county_troops(&mut k.counties, &realms);
    assert_eq!(k.counties[1].friendly_troops, 0);
    assert_eq!(k.counties[2].enemy_troops, 300, "an unowned county sees it as an enemy");
}

/// Phase 7 gives every unit its moves back, and it does so *after* the
/// mercenaries have walked.
#[test]
fn a_season_returns_every_units_movement_and_walks_the_bands() {
    let mut k = kingdom();
    let id = army(&mut k, 1, 1, 300, 5, 5);
    k.campaign.units.get_mut(id).unwrap().moves_used = 15;
    let before: Vec<u8> = k.campaign.mercenaries.iter().map(|(_, b)| b.next_county).collect();

    let report = k.advance_season();
    assert!(report.passes.contains(&Pass::MercenaryAdvance));
    assert!(report.passes.contains(&Pass::UnitsResetMoves));
    assert_eq!(k.campaign.units.get(id).unwrap().moves_used, 0);
    assert_eq!(k.campaign.units.get(id).unwrap().moves_left(), 15);

    let after: Vec<u8> = k.campaign.mercenaries.iter().map(|(_, b)| b.next_county).collect();
    assert_ne!(before, after, "the bands moved");
}

/// A year of the mercenary walk on a two-county map: the bands stay inside the
/// map and the county offers only ever name a band in play.
#[test]
fn a_year_of_the_mercenary_walk_stays_inside_the_map() {
    let mut k = kingdom();
    assert_eq!(k.campaign.mercenaries.in_play(), 2, "two counties, two bands");
    for _ in 0..12 {
        k.advance_season();
        for (id, band) in k.campaign.mercenaries.iter() {
            assert!(band.next_county <= 3, "band {id} walked to {}", band.next_county);
            assert!(band.offered_in <= 2);
        }
        for c in 1..=2 {
            let offer = k.counties[c].mercenary_offer;
            assert!(offer as usize <= k.campaign.mercenaries.in_play(), "county {c} offers {offer}");
        }
    }
}

/// A band on offer can be hired into a new army, and the men land in `men` but
/// not in the troop counts.
#[test]
fn a_band_standing_in_a_county_can_be_hired_when_an_army_is_raised_there() {
    let mut k = kingdom();
    // Walk until something is on offer in county 1.
    let mut seasons = 0;
    while k.counties[1].mercenary_offer == 0 {
        k.advance_season();
        seasons += 1;
        assert!(seasons < 30, "no band ever reached county 1");
    }
    let band = k.counties[1].mercenary_offer;
    let rules = *k.campaign.mercenaries.rules(band).expect("a band in play");

    let id = army(&mut k, 1, 1, 100, 5, 5);
    let gold = k.realms[1].gold;
    let Kingdom { campaign, counties, realms, .. } = &mut k;
    assert!(campaign.mercenaries.hire(&mut campaign.units, counties, realms, id, band));

    let u = k.campaign.units.get(id).unwrap();
    assert_eq!(u.men, 100 + rules.men);
    assert_eq!(u.troops.iter().sum::<i32>(), 100, "the band is not in the troop counts");
    assert_eq!(u.mercenary_men(), rules.men);
    assert_eq!(k.realms[1].gold, gold - rules.price);
    assert_eq!(k.counties[1].mercenary_offer, 0);
}

/// Bankruptcy walks the mercenaries out first, and it is the season pass that
/// does it.
#[test]
fn a_realm_that_cannot_pay_loses_its_mercenaries_before_it_loses_men() {
    let mut k = kingdom();
    k.realms[1].gold = 0;
    let id = army(&mut k, 1, 1, 400, 5, 5);
    k.campaign.mercenaries.set_band_raw(
        1,
        l2_kingdom::mercenary::Band { hired_by: id as u16, ..Default::default() },
    );
    k.campaign.units.get_mut(id).unwrap().mercenaries = Some(l2_kingdom::Mercenaries {
        band: 1,
        troop: TroopType::Pikeman,
        men: 100,
    });
    k.campaign.units.get_mut(id).unwrap().men += 100;

    let mut report = l2_kingdom::report::SeasonReport::new();
    k.run_pass(Pass::WagesPay, &mut report);
    assert_eq!(k.realms[1].bankrupt_stage, 1);
    assert!(k.campaign.units.get(id).unwrap().mercenaries.is_none(), "they walked");
    assert_eq!(k.campaign.units.get(id).unwrap().men, 400, "and took their men with them");
    assert_eq!(k.campaign.units.get(id).unwrap().troops.iter().sum::<i32>(), 400);

    // The next unpaid season takes a tenth off the levy itself.
    let mut report = l2_kingdom::report::SeasonReport::new();
    k.run_pass(Pass::WagesPay, &mut report);
    assert_eq!(k.realms[1].bankrupt_stage, 2);
    assert_eq!(k.campaign.units.get(id).unwrap().men, 360);
}

// ---------------------------------------------------------------------------
// Trampling, and the industry counter it writes
// ---------------------------------------------------------------------------

/// **`disabled_seasons` had no writer, and now it has one.** March an army over
/// a neutral county's iron site and the mine is dead for three seasons; run
/// three seasons and it comes back.
#[test]
fn a_trampled_mine_is_dead_for_three_seasons_and_then_reopens() {
    let mut k = kingdom();
    // Owned by somebody else: the counter is decremented by the industry pass,
    // and that pass skips a county with no realm to credit.
    k.counties[2].owner = 2;
    k.realms[2].in_play = true;
    k.campaign.map.set_flags(40, 10, flags::SETTLEMENT);
    k.campaign.map.set_terrain(40, 10, 1); // an iron site
    k.counties[2].industry[1].efficiency = 80;

    let id = army(&mut k, 1, 2, 300, 39, 10);
    k.campaign.units.get_mut(id).unwrap().path = vec![(40, 10)];

    let Kingdom { campaign, counties, realms, .. } = &mut k;
    let realms_snapshot = realms.clone();
    let step = l2_kingdom::movement::step(&mut campaign.map, counties, &realms_snapshot, &mut campaign.units, id)
        .expect("a step to take");
    assert_eq!(step.site_ruined, Some((2, 1)));
    assert_eq!(k.counties[2].industry[1].disabled_seasons, 3);
    assert_eq!(k.counties[2].industry[1].efficiency, 0);

    // And the map is a hole afterwards, so the pathfinder routes round it.
    assert_eq!(k.campaign.map.cost_map().at(40, 10), 0);

    let mut left = Vec::new();
    for _ in 0..4 {
        k.advance_season();
        left.push(k.counties[2].industry[1].disabled_seasons);
    }
    assert_eq!(left, vec![2, 1, 0, 0], "three seasons, then it is back");
    assert!(k.counties[2].industry[1].enabled);
}

/// **An unowned county's trampled mine never reopens**, and that is a
/// composition rather than a rule anybody wrote.
///
/// `Unit_TrampleTile` writes 3 into `disabled_seasons`; the *only* thing that
/// counts it back down is `Industry_Produce`, and this crate's industry pass
/// skips a county with no realm to credit (`kingdom.rs`, and
/// `docs/kingdom.md` §7.4). So marching over a neutral county's iron shuts it
/// down for good, until somebody takes the county.
///
/// Asserted so the interaction is visible rather than surprising. Whether the
/// original's driver also skips unowned counties was **not** checked — this
/// asserts what our engine does, and it is worth checking against the binary
/// before anyone treats it as a rule of the game.
#[test]
fn a_neutral_countys_trampled_mine_stays_shut_until_somebody_owns_it() {
    let mut k = kingdom();
    assert_eq!(k.counties[2].owner, 0);
    k.counties[2].industry[1].disabled_seasons = 3;

    for _ in 0..6 {
        k.advance_season();
    }
    assert_eq!(k.counties[2].industry[1].disabled_seasons, 3, "nothing ticked it");

    // Take the county and it starts recovering.
    k.counties[2].owner = 1;
    for _ in 0..3 {
        k.advance_season();
    }
    assert_eq!(k.counties[2].industry[1].disabled_seasons, 0);
}

/// You cannot ruin your own county's site, whatever you march over it.
#[test]
fn an_army_cannot_shut_down_its_own_realms_mine() {
    let mut k = kingdom();
    k.counties[2].owner = 1;
    k.campaign.map.set_flags(40, 10, flags::SETTLEMENT);
    k.campaign.map.set_terrain(40, 10, 1);

    let id = army(&mut k, 1, 2, 300, 39, 10);
    k.campaign.units.get_mut(id).unwrap().path = vec![(40, 10)];
    let Kingdom { campaign, counties, realms, .. } = &mut k;
    let realms_snapshot = realms.clone();
    let step = l2_kingdom::movement::step(&mut campaign.map, counties, &realms_snapshot, &mut campaign.units, id)
        .expect("a step to take");
    assert_eq!(step.site_ruined, None);
    assert_eq!(step.charged, 0);
    assert_eq!(k.counties[2].industry[1].disabled_seasons, 0);
}

// ---------------------------------------------------------------------------
// Conquest, and the save
// ---------------------------------------------------------------------------

/// The one thing the whole layer exists to produce, driven through the kingdom
/// rather than through the modules: an army that reaches a defenceless county's
/// castle takes it, and the realm's county count follows.
#[test]
fn an_army_that_reaches_a_castle_takes_the_county() {
    let mut k = kingdom();
    k.counties[2].population = 10; // below the defence floor
    k.campaign.map.set_flags(40, 10, flags::CASTLE);
    let id = army(&mut k, 1, 2, 300, 39, 10);
    k.campaign.units.get_mut(id).unwrap().path = vec![(40, 10)];

    let Kingdom { campaign, counties, realms, tables, .. } = &mut k;
    let (steps, outcome) =
        conquest::march_and_fight(tables, campaign, counties, realms, id, 0, 1268);
    assert_eq!(steps.len(), 1);
    assert_eq!(steps[0].reached_castle, Some(2));
    assert_eq!(outcome, Some(conquest::Attack::Captured));

    assert_eq!(k.counties[2].owner, 1);
    assert_eq!(k.realms[1].county_count, 2);
    assert_eq!(k.counties[2].happiness, 77 - 10, "a human at difficulty 0 costs 10");
    assert_eq!(k.campaign.units.get(id).unwrap().moves_used, conquest::ATTACK_MOVE_COST);
}

/// A county that can defend itself produces a battle instead, and the layer
/// hands the pair over rather than fighting it.
#[test]
fn a_county_that_can_defend_itself_produces_a_battle_and_keeps_its_owner() {
    let mut k = kingdom();
    k.campaign.map.set_flags(40, 10, flags::CASTLE);
    let id = army(&mut k, 1, 2, 300, 39, 10);
    k.campaign.units.get_mut(id).unwrap().path = vec![(40, 10)];

    let Kingdom { campaign, counties, realms, tables, .. } = &mut k;
    let (_, outcome) = conquest::march_and_fight(tables, campaign, counties, realms, id, 0, 1268);
    let Some(conquest::Attack::Battle { attacker, defender }) = outcome else {
        panic!("expected a battle, got {outcome:?}");
    };
    assert_eq!(attacker, id);
    assert_ne!(defender, id);
    assert_eq!(k.counties[2].owner, 0, "nothing changes hands until the battle is fought");
    assert_eq!(k.campaign.units.get(defender).unwrap().county, 2);
}

/// A kingdom with armies, a map and mercenary bands in it survives a save and
/// plays on identically — the property `docs/netcode.md` §5 needs and the
/// reason the campaign lives inside `Kingdom` at all.
#[test]
fn a_kingdom_with_armies_in_it_saves_and_resumes_identically() {
    let mut k = kingdom();
    k.options.armies_eat = true;
    let a = army(&mut k, 1, 1, 400, 5, 5);
    let b = army(&mut k, 1, 1, 250, 6, 6);
    k.campaign.units.get_mut(b).unwrap().mercenaries =
        Some(l2_kingdom::Mercenaries { band: 1, troop: TroopType::Pikeman, men: 100 });
    l2_kingdom::movement::order_move(&k.campaign.map, &mut k.campaign.units, a, (20, 20), Routing::Direct);
    k.advance_season();

    let bytes = l2_kingdom::save::encode(&k);
    let restored = l2_kingdom::save::decode(&bytes, k.tables.clone()).expect("our own bytes");
    assert_eq!(restored, k, "field for field, campaign included");

    let mut one = k.clone();
    let mut two = restored;
    for _ in 0..4 {
        one.advance_season();
        two.advance_season();
    }
    assert_eq!(one, two, "and they play on identically");
    assert_eq!(l2_kingdom::save::encode(&one), l2_kingdom::save::encode(&two));
}

/// A save with an empty campaign is still a valid save — the layer is not
/// optional state that has to be present.
#[test]
fn a_kingdom_with_no_units_at_all_round_trips() {
    let k = Kingdom::new(5);
    let bytes = l2_kingdom::save::encode(&k);
    assert_eq!(l2_kingdom::save::decode(&bytes, k.tables.clone()).unwrap(), k);
    assert_eq!(k.campaign.units.len(), 0);
    assert_eq!(k.campaign.mercenaries.in_play(), 0);
}

/// Slot numbers are referenced by county `garrison_unit`, by `besieged_by`, and
/// by a band's `hired_by`, so a save that renumbered units on load would point
/// every link at the wrong army.
#[test]
fn a_save_keeps_units_in_the_slots_they_were_in() {
    let mut k = kingdom();
    let mut units = Units::new();
    let mut far = Unit::new(UnitKind::Army, 1, 3, 3);
    far.men = 100;
    units.put(90, far);
    k.campaign.units = units;
    k.counties[1].garrison_unit = 90;

    let bytes = l2_kingdom::save::encode(&k);
    let restored = l2_kingdom::save::decode(&bytes, k.tables.clone()).unwrap();
    assert!(restored.campaign.units.get(90).is_some(), "slot 90, not slot 1");
    assert!(restored.campaign.units.get(1).is_none());
    assert_eq!(restored.counties[1].garrison_unit, 90);
}

/// Nothing in the campaign layer iterates anything but an ascending index, so
/// two kingdoms built the same way are byte-identical — the determinism
/// property, checked over a layer that now has a `Vec` in it.
#[test]
fn two_identically_built_kingdoms_stay_byte_identical_through_a_year() {
    let build = || {
        let mut k = kingdom();
        k.options.armies_eat = true;
        for n in 0..6 {
            let id = army(&mut k, 1, 1, 200 + n * 30, 5 + n as u8, 5);
            l2_kingdom::movement::order_move(
                &k.campaign.map,
                &mut k.campaign.units,
                id,
                (20 + n as u8, 25),
                Routing::Direct,
            );
        }
        k
    };
    let (mut one, mut two) = (build(), build());
    assert_eq!(l2_kingdom::save::encode(&one), l2_kingdom::save::encode(&two));
    for _ in 0..4 {
        one.advance_season();
        two.advance_season();
        assert_eq!(l2_kingdom::save::encode(&one), l2_kingdom::save::encode(&two));
    }
}

/// The array bound is the original's, and it is enforced rather than grown.
#[test]
fn the_unit_array_holds_a_hundred_and_fifty_and_refuses_the_hundred_and_fifty_first() {
    let mut k = kingdom();
    for _ in 0..150 {
        let mut u = Unit::new(UnitKind::Army, 1, 0, 0);
        u.men = 50;
        assert!(k.campaign.units.spawn(u).is_some());
    }
    let mut one_too_many = Unit::new(UnitKind::Army, 1, 0, 0);
    one_too_many.men = 50;
    assert_eq!(k.campaign.units.spawn(one_too_many), None);
    assert_eq!(k.campaign.units.len(), 150);
    assert_eq!(MAX_COUNTIES, 17, "and the county array is still seventeen");
}

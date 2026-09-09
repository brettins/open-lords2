//! Ending a turn: the phase machine goes round, the AI finishes, and the same
//! kingdom ended twice produces the same numbers.
//!
//! No install and no window. The kingdoms here are built by hand so that each
//! assertion is about *one* thing the spine does — the tests that run the
//! England turn-one scenario are in `tests/scenario.rs`.

use l2_kingdom::phase::Phase;
use l2_kingdom::realm::AI_STEP_DONE;
use l2_kingdom::tables::Tables;
use l2_game::turn;
use l2_game::Game;

/// One human realm holding county 1, four AI realms holding one each, and nine
/// unowned — the shape of the England turn-one scenario, without needing the file.
fn five_realms() -> Game {
    let mut g = Game::new(0x51EED);
    g.player = 1;
    g.kingdom.set_county_count(14);
    for id in 1..=14 {
        let c = &mut g.kingdom.counties[id];
        c.population = 417;
        c.pop_last = 417;
        c.happiness = 65;
        c.happiness_last = 65;
        c.health_meter = 65;
        c.health_band = l2_kingdom::tables::health_band(65);
        c.herd = 67;
        c.ration_wanted = 3;
        c.ration_split = 100;
        for n in 1..=14u8 {
            if n as usize != id {
                c.add_neighbour(n);
            }
        }
    }
    for realm in 1..=5usize {
        g.kingdom.counties[realm].owner = realm as u8;
        g.kingdom.realms[realm].in_play = true;
        g.kingdom.realms[realm].strength = 3;
        g.kingdom.realms[realm].county_count = 1;
        g.kingdom.realms[realm].gold = 1000;
        g.kingdom.realms[realm].lord = realm as u8 - 1;
    }
    g.kingdom.realms[1].is_human = true;
    g.kingdom.realms[1].lord = 0;
    g
}

#[test]
fn a_turn_walks_the_whole_phase_machine_and_runs_the_season_once() {
    let mut g = five_realms();
    let before = g.kingdom.turn_count;
    let outcome = turn::end_turn(&mut g).expect("the machine comes round");

    assert_eq!(
        outcome.report.passes,
        l2_kingdom::SEASON_PIPELINE.to_vec(),
        "every pass, in the order the pipeline is written in"
    );
    assert_eq!(g.kingdom.turn_count, before + 1, "exactly one season ran");
    assert_eq!(g.kingdom.turn.phase, Phase::NeutralCounties, "and phase 7 wrapped to 1");
    assert!(outcome.ticks < turn::MAX_TICKS, "{} ticks", outcome.ticks);
    assert_eq!(g.turns_played, 1);
    assert!(g.last_report.is_some());
}

/// Phase 4 does not end until every realm's `aiStep` reaches its threshold, and
/// nothing in `l2-kingdom` was calling `ai::run_step` at all. If the driver in
/// `turn.rs` stopped working, this test would not fail with a wrong number: the
/// turn would never finish, and `end_turn` would return `None`.
#[test]
fn every_ai_realm_finishes_its_turn_and_the_human_realm_never_starts_one() {
    let mut g = five_realms();
    turn::end_turn(&mut g).expect("the AI must finish or the turn cannot end");
    for realm in 1..=5usize {
        assert!(
            g.kingdom.realms[realm].ai_step >= AI_STEP_DONE,
            "realm {realm} did not finish its turn"
        );
    }
}

/// `docs/kingdom.md` §3.1: phase 1 sets the *unowned* counties' tax rates on
/// the neutral ladder, which `AI_SetTaxRates(0)` reads out of a chain of
/// comparisons — `< 20 -> 0`, `< 40 -> 1`, `< 50 -> 2`, `< 60 -> 3`,
/// `< 70 -> 4`, `< 80 -> 6`, `< 90 -> 8`, else 12 (`docs/decisions.md` C19).
/// A county at 65 lands on 4.
#[test]
fn the_unowned_counties_are_taxed_on_the_neutral_ladder_every_turn() {
    let mut g = five_realms();
    assert_eq!(g.kingdom.counties[14].owner, 0, "county 14 is unowned");
    assert_eq!(g.kingdom.counties[14].tax_rate, 0, "and starts untaxed");
    assert_eq!(g.kingdom.counties[14].happiness, 65);

    turn::end_turn(&mut g).unwrap();
    assert_eq!(g.kingdom.counties[14].tax_rate, 4, "65 is on the fifth rung");
    assert_eq!(g.kingdom.counties[1].tax_rate, 0, "the player's county is not touched");
}

/// The one place the spine deliberately differs from the original's call
/// structure, and the reason it has to: `AI_SetTaxRates(realm)` grants that one
/// realm its gold, while `l2_kingdom::ai::grant_resources` walks every AI realm
/// in a single pass. Running it inside each realm's step 3 would pay every
/// realm four times over. The check is arithmetic: exactly one grant.
#[test]
fn the_ai_gold_grant_is_paid_once_a_turn_and_not_once_per_ai_realm() {
    let mut g = five_realms();
    g.kingdom.options.difficulty = 2;
    let tables = Tables::DEFAULT;

    let expected: Vec<i32> = (0..6)
        .map(|r| g.kingdom.realms[r].gold_grant(&tables, 2))
        .collect();
    let before: Vec<i32> = (0..6).map(|r| g.kingdom.realms[r].gold).collect();

    turn::end_turn(&mut g).unwrap();

    for realm in 2..=5usize {
        let paid = g.kingdom.realms[realm].gold - before[realm] - g.kingdom.counties[realm].tax_collected;
        assert_eq!(
            paid, expected[realm],
            "realm {realm} was paid {paid} where one grant is {}",
            expected[realm]
        );
    }
    assert_eq!(
        g.kingdom.realms[1].gold - before[1],
        g.kingdom.counties[1].tax_collected,
        "the human realm is granted nothing at all"
    );
}

/// Determinism, which is not negotiable: a turn is a pure function of the
/// kingdom it started from. Two identical kingdoms, ended independently, must
/// be bit-identical afterwards — no clock, no hash order, no address.
#[test]
fn the_same_kingdom_ended_twice_lands_on_the_same_numbers() {
    let mut a = five_realms();
    let mut b = five_realms();
    assert_eq!(a.kingdom, b.kingdom, "the two start equal");

    let ra = turn::end_turn(&mut a).unwrap();
    let rb = turn::end_turn(&mut b).unwrap();
    assert_eq!(ra.ticks, rb.ticks);
    assert_eq!(ra.report, rb.report);
    assert_eq!(a.kingdom, b.kingdom);

    // And again, because a difference that only appears on the second turn is
    // the kind a single-turn test misses.
    turn::end_turn(&mut a).unwrap();
    turn::end_turn(&mut b).unwrap();
    assert_eq!(a.kingdom, b.kingdom);
}

/// The seasons turn, and the year rolls where the code says rather than where
/// the prose does.
///
/// `Season_Advance` rolls the year when the season that *ended* was 4, and
/// `ended` is the old `g_seasonNext` — so starting from the England turn-one fixture's
/// Winter 1268 the year does not move until the turn that brings Winter round
/// again. The year label therefore runs **Winter, Spring, Summer, Autumn**,
/// which `l2-kingdom`'s errata note 3 records and §3.3's prose contradicts.
/// This test was written expecting the prose and corrected by the code.
#[test]
fn four_turns_walk_the_year_round() {
    let mut g = five_realms();
    g.kingdom.season = 4;
    g.kingdom.season_next = 1;
    g.kingdom.year = 1268;
    g.kingdom.year_next = 1269;

    let seasons: Vec<(u8, i32)> = (0..4)
        .map(|_| {
            turn::end_turn(&mut g).unwrap();
            (g.kingdom.season, g.kingdom.year)
        })
        .collect();
    assert_eq!(seasons, vec![(1, 1268), (2, 1268), (3, 1268), (4, 1269)]);
}

/// The numbers actually move, which is the point of the button. Population,
/// health and the treasury all change on a turn that nobody gave any orders.
#[test]
fn a_turn_moves_the_population_and_the_treasury() {
    let mut g = five_realms();
    g.kingdom.counties[1].tax_rate = 10;
    g.kingdom.counties[1].castle_type = 3;
    let pop = g.kingdom.counties[1].population;
    let gold = g.kingdom.realms[1].gold;

    turn::end_turn(&mut g).unwrap();

    assert_ne!(g.kingdom.counties[1].population, pop, "a season with births and deaths");
    assert_eq!(g.kingdom.counties[1].pop_last, pop, "and last season is remembered");
    assert!(
        g.kingdom.realms[1].gold > gold,
        "a taxed county pays: {} -> {}",
        gold,
        g.kingdom.realms[1].gold
    );
    assert_eq!(g.gold_last[1], gold, "the interface can show which way it went");
}

// ---------------------------------------------------------------------------
// Things move on the campaign map — `docs/plan.md` revision 4, item 3
// ---------------------------------------------------------------------------
//
// Everything above this line passed while four of the seven phases were no-ops
// answering their waits `true` the moment they started. That is the shape of
// test `docs/decisions.md` has a correction number for: it could not fail for
// the reason it existed. These are the ones that can.

use l2_kingdom::map::{flags, CampaignMap, MAP_DIM, MAP_TILES};
use l2_kingdom::units_tick::Contact;
use l2_kingdom::merchant::MerchantRoutes;
use l2_kingdom::unit::{Unit, UnitKind};

/// A kingdom with a **map** under it: fourteen counties in five-tile columns, a
/// road the whole way along y = 10, and every anchor on that road.
///
/// The map matters and used to be missing everywhere. `Kingdom::new` builds an
/// empty one and nothing overwrote it, so every imported game ran its
/// pathfinding over 4,096 blank tiles; `l2-scenario` now reads the planes out of
/// block 0 of the save. A hand-built kingdom still has to build its own, and a
/// test that forgot would silently be testing movement over featureless ground.
fn with_a_map() -> Game {
    let mut g = five_realms();
    let mut map = CampaignMap::empty();
    for i in 0..MAP_TILES {
        map.county[i] = ((i % MAP_DIM) / 5 + 1).min(14) as u8;
    }
    for x in 0..64u8 {
        map.set_flags(x, 10, flags::ROAD);
    }
    g.kingdom.campaign.map = map;
    for id in 1..=14usize {
        g.kingdom.counties[id].anchor_x = (id as u8 - 1) * 5 + 2;
        g.kingdom.counties[id].anchor_y = 10;
    }
    g
}

fn army(g: &mut Game, owner: u8, x: u8, y: u8) -> usize {
    let mut u = Unit::new(UnitKind::Army, owner, x, y);
    u.men = 120;
    u.troops[0] = 120;
    u.county = g.kingdom.campaign.map.county_at(x, y);
    u.home_county = u.county;
    u.owner_is_human = g.kingdom.realms[owner as usize].is_human;
    g.kingdom.campaign.units.spawn(u).expect("a free slot")
}

/// **The one the whole item is about.** An army is ordered to move *through the
/// game*, the player ends the turn, and the army is somewhere else afterwards.
///
/// Nothing here reaches into `l2-kingdom`'s mover: the order goes through
/// [`Game::order_unit_move`], which is what a map screen calls, and the walking
/// is done by the turn machine. Before this work `end_turn` answered every unit
/// wait `true`, and the army would have finished the turn exactly where it
/// started.
#[test]
fn an_army_ordered_through_the_game_actually_moves_when_the_turn_is_ended() {
    let mut g = with_a_map();
    let id = army(&mut g, 1, 5, 10);
    let from = g.kingdom.campaign.units.get(id).unwrap().tile();

    assert_eq!(g.order_unit_move(id, (20, 10)), Some(15), "fifteen road tiles");

    let outcome = turn::end_turn(&mut g).expect("the machine comes round");

    let to = g.kingdom.campaign.units.get(id).unwrap().tile();
    assert_ne!(to, from, "the army did not move at all");
    assert_eq!(to, (20, 10), "and it arrived");
    assert!(outcome.steps >= 15, "{} tiles entered over the turn", outcome.steps);
    assert!(outcome.pending_battles.is_empty(), "nobody was in the way");
}

/// A unit the player does not own takes no orders, and the refusal changes
/// nothing.
#[test]
fn another_realms_army_refuses_the_players_orders() {
    let mut g = with_a_map();
    let theirs = army(&mut g, 2, 5, 10);
    assert_eq!(g.order_unit_move(theirs, (20, 10)), None);
    assert!(!g.kingdom.campaign.units.get(theirs).unwrap().moving);
    turn::end_turn(&mut g).unwrap();
    assert_eq!(g.kingdom.campaign.units.get(theirs).unwrap().tile(), (5, 10));
}

/// **Two units meet during a played turn, and the battle is fought.**
///
/// This was written as the seam's marker while battle resolution was on
/// another branch: it asserted one *unfought* pair in `pending_battles`, so
/// that filling the seam in would change a test rather than pass either way.
/// It did exactly that. `turn::resolve_battle` now calls
/// `engagement::resolve`, so what a played turn produces is a result, not a
/// note — one of the two armies is destroyed and `pending_battles` is empty.
#[test]
fn two_enemy_armies_meeting_during_a_turn_fight_a_battle() {
    let mut g = with_a_map();
    let mine = army(&mut g, 1, 5, 10);
    let theirs = army(&mut g, 2, 6, 10);
    g.order_unit_move(mine, (20, 10)).expect("a road runs the whole way");

    let outcome = turn::end_turn(&mut g).unwrap();

    assert!(
        outcome.pending_battles.is_empty(),
        "the encounter reached engagement::resolve: {:?}",
        outcome.pending_battles
    );
    assert!(
        outcome.contacts.iter().any(|c| matches!(c, Contact::Battle(_))),
        "and the sweep reported it: {:?}",
        outcome.contacts
    );

    // Exactly one of the two is left standing. The autocalc destroys the
    // loser, and `Units::get` returns `None` for a slot that has been freed.
    let alive = [mine, theirs]
        .iter()
        .filter(|&&id| g.kingdom.campaign.units.get(id).is_some_and(|u| u.men > 0))
        .count();
    assert_eq!(alive, 1, "a battle has one survivor, not two and not none");
}

/// The attacker never walks onto the tile it is attacking — the step returns
/// before the move is committed, which is what lets the army resume from where
/// it stood if the battle leaves it alive.
#[test]
fn the_attacker_does_not_enter_the_defenders_tile() {
    let mut g = with_a_map();
    let mine = army(&mut g, 1, 5, 10);
    // A defender big enough that the attacker loses and is destroyed, so the
    // survivor is the one that never moved.
    let theirs = army(&mut g, 2, 6, 10);
    g.kingdom.campaign.units.get_mut(theirs).unwrap().men = 1200;
    g.kingdom.campaign.units.get_mut(theirs).unwrap().troops[0] = 1200;
    g.order_unit_move(mine, (20, 10)).unwrap();

    turn::end_turn(&mut g).unwrap();

    assert!(g.kingdom.campaign.units.get(mine).is_none(), "the attacker lost and is gone");
    assert_eq!(
        g.kingdom.campaign.units.get(theirs).unwrap().tile(),
        (6, 10),
        "and the defender is still on its own tile"
    );
}

/// An army walking over another county's standing crop wrecks it — during a
/// played turn, with nobody calling the mover by hand.
///
/// Kept well away from the road at y = 10: a field costs 6 to cross and a road
/// costs 1, so a pathfinder given the choice goes round, and the first version
/// of this test proved only that the flood fill works.
#[test]
fn an_army_crossing_a_foreign_field_wrecks_it_during_the_turn() {
    let mut g = with_a_map();
    // A standing crop across the army's path, in a county it does not own —
    // x 5 … 9 is county 2, and realm 2 holds it. The rest of those four
    // columns is mountain, so the crop is the only way through: an army given
    // a choice walks round a field, because 6 to cross beats 3 to go by.
    for x in 6..=9u8 {
        for y in 0..64u8 {
            g.kingdom.campaign.map.set_flags(x, y, flags::ROUGH);
        }
        g.kingdom.campaign.map.set_flags(x, 20, flags::FARMLAND);
        g.kingdom.campaign.map.set_terrain(x, 20, 8);
    }
    assert_eq!(g.kingdom.counties[2].owner, 2, "somebody else's fields");
    // `County_DestroyField` takes the tile's share of the standing crop, so
    // the county has to have one: with `fields_grain` at 0 it returns without
    // repainting the tile and nothing is destroyed.
    g.kingdom.counties[2].fields_grain = 4;
    g.kingdom.counties[2].crop[1] = 400;

    let id = army(&mut g, 1, 5, 20);
    g.order_unit_move(id, (9, 20)).expect("a path across the fields");

    let outcome = turn::end_turn(&mut g).unwrap();

    assert!(outcome.steps > 0, "the army walked");
    assert!(
        (6..=9u8).any(|x| !g.kingdom.campaign.map.is_standing_field(x, 20)),
        "at least one field was trampled"
    );
    assert!(
        g.kingdom.counties[2].fields_grain < 4,
        "and the county lost a field with it"
    );
}

/// **A merchant walks its route, turn after turn.**
///
/// Four counties on one route, a merchant in slot 1, and six turns. Phase 6
/// gives it the next leg each turn — `Merchant_AdvanceAll` — and the unit sweep
/// walks it. What is asserted is the *sequence*: the merchant reaches the
/// counties its route names, in the order it names them, which is the thing a
/// merchant that merely wandered would fail.
#[test]
fn a_merchant_walks_its_route_across_several_turns() {
    let mut g = with_a_map();
    let mut routes = MerchantRoutes::none();
    // Route row 0 is unit slot 1's, and the cursor starts at 1, so the first
    // destination is the *second* entry: county 4.
    routes.set_route(0, &[1, 4, 7, 10]);
    g.kingdom.campaign.routes = routes;

    let mut m = Unit::new(UnitKind::Merchant, 6, 2, 10);
    m.county = 1;
    m.needs_destination = true;
    m.year_formed = 1; // the route cursor
    let id = g.kingdom.campaign.units.spawn(m).expect("slot 1");
    assert_eq!(id, 1, "the route row is the slot minus one");

    let mut counties = Vec::new();
    for _ in 0..6 {
        turn::end_turn(&mut g).unwrap();
        counties.push(g.kingdom.campaign.units.get(id).unwrap().county);
    }

    assert!(
        counties.iter().any(|&c| c != 1),
        "the merchant never left its county: {counties:?}"
    );
    // The counties it was standing in at the end of each turn, with repeats
    // collapsed: the order it visited them in.
    let mut visited: Vec<u8> = Vec::new();
    for &c in &counties {
        if visited.last() != Some(&c) {
            visited.push(c);
        }
    }
    assert!(
        visited.windows(2).all(|w| w[0] < w[1]),
        "it walked east along its route without doubling back: {visited:?}"
    );
    assert!(visited.contains(&4), "it reached the first leg's county: {visited:?}");
}

/// The move allowance is **rebuilt every tick from the unit's type**, so a unit
/// that arrived from a save with a zero in the field still walks.
/// `england-turn1.sav` holds exactly that: six merchants, all with
/// `moveAllowance = 0`, because the field is written by the tick handler and
/// never persisted.
///
/// > **The AI realms are taken out of play for this one test**, and the reason
/// > is worth a sentence. `with_a_map` gives every county a border with every
/// > other, so once `l2_kingdom::ai_army` landed the very first turn had realm
/// > 3 raise an army, march it across the map and **destroy this one at (11,
/// > 10) with 83 men to spare**. That is the feature working, and it is not
/// > what this test is about: the subject is the tick handler rewriting
/// > `move_allowance`, and a subject that has been killed by an unrelated rule
/// > cannot be observed. Every other test in this file that ends a turn with a
/// > unit on the board was already immune.
#[test]
fn a_unit_loaded_with_no_allowance_still_walks() {
    let mut g = with_a_map();
    for realm in 2..=5 {
        g.kingdom.realms[realm].in_play = false;
    }
    let id = army(&mut g, 1, 5, 10);
    g.order_unit_move(id, (12, 10)).unwrap();
    g.kingdom.campaign.units.get_mut(id).unwrap().move_allowance = 0;

    turn::end_turn(&mut g).unwrap();

    assert_eq!(g.kingdom.campaign.units.get(id).unwrap().tile(), (12, 10));
    assert_eq!(g.kingdom.campaign.units.get(id).unwrap().move_allowance, 15);
}

/// Movement does not cost determinism. The same kingdom, with the same orders,
/// ended twice, lands on the same tiles — the property `docs/netcode.md` binds
/// and the one a unit sweep is most likely to break.
#[test]
fn a_turn_with_units_moving_is_still_a_pure_function_of_where_it_started() {
    let orders = |g: &mut Game| {
        let a = army(g, 1, 5, 10);
        let b = army(g, 1, 40, 10);
        let mut m = Unit::new(UnitKind::Merchant, 6, 2, 10);
        m.county = 1;
        m.year_formed = 1;
        g.kingdom.campaign.units.spawn(m).unwrap();
        g.kingdom.campaign.routes.set_route(0, &[1, 4, 7]);
        g.order_unit_move(a, (20, 10)).unwrap();
        g.order_unit_move(b, (25, 10)).unwrap();
    };

    let mut x = with_a_map();
    let mut y = with_a_map();
    orders(&mut x);
    orders(&mut y);
    assert_eq!(x.kingdom, y.kingdom, "the two start equal");

    let ox = turn::end_turn(&mut x).unwrap();
    let oy = turn::end_turn(&mut y).unwrap();
    assert_eq!(ox.ticks, oy.ticks);
    assert_eq!(ox.steps, oy.steps);
    assert_eq!(ox.contacts, oy.contacts);
    assert_eq!(x.kingdom, y.kingdom);

    // And a second turn, because a divergence that only shows on the next one
    // is what a single-turn test misses.
    turn::end_turn(&mut x).unwrap();
    turn::end_turn(&mut y).unwrap();
    assert_eq!(x.kingdom, y.kingdom);
}

/// Phase 5 re-targets the peasant mobs and phase 3 the transports, off their own
/// state rather than off any player order — so a turn moves things nobody
/// touched. These two phases have no other way to be exercised.
#[test]
fn the_turn_moves_mobs_and_transports_nobody_ordered() {
    let mut g = with_a_map();
    g.kingdom.season = 2; // spring: every mob is re-targeted

    let mut mob = Unit::new(UnitKind::PeasantMob, 6, 2, 10);
    mob.county = 1;
    mob.men = 200;
    let mob_id = g.kingdom.campaign.units.spawn(mob).unwrap();

    let mut t = Unit::new(UnitKind::Transport, 1, 3, 10);
    t.county = 1;
    t.cargo_county = 5;
    let transport = g.kingdom.campaign.units.spawn(t).unwrap();

    turn::end_turn(&mut g).unwrap();

    assert_ne!(
        g.kingdom.campaign.units.get(mob_id).unwrap().tile(),
        (2, 10),
        "the mob was given a destination by phase 5 and walked"
    );
    assert_ne!(
        g.kingdom.campaign.units.get(transport).unwrap().tile(),
        (3, 10),
        "the transport was re-targeted by phase 3 and walked"
    );
    assert_eq!(
        g.kingdom.campaign.units.get(transport).unwrap().dest_county,
        5,
        "at its cargo county"
    );
}

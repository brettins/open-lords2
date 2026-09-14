//! **Are the AI realms competing, or starving?**
//!
//! This is the test `docs/agents.md` C27 asks for, on the subject C27 is
//! about. The grain economy was *"finished, tested and unreachable in play"* —
//! nobody farmed — because every test drove the rules
//! directly and none of them drove the game. The AI's fourteen turn handlers
//! are the same shape: a handler that exists as library code and is never
//! dispatched passes every unit test it has.
//!
//! So the assertion here is not *"the allocator returns the right numbers"*.
//! It is **"play N turns of a real game
//! still fed, and their fields are planted"** — and it is the only test in the
//! workspace that can catch a handler that was implemented and then forgotten
//! at the dispatch.
//!
//! # Two worlds, on purpose
//!
//! * [`the_ai_realms_are_competing_after_forty_turns_of_england`] plays the
//!   **England turn-one fixture** — a position, a map, the
//!   fourteen counties — and is gated on a copy of the game.
//! * [`the_ai_realms_survive_forty_turns_of_a_world_built_by_hand`] plays a
//!   world this file builds, so that CI asserts something too.
//!
//! Neither is worth much alone. The synthetic one runs everywhere and proves
//! only that the dispatch is wired; the England one is the evidence, and
//! `docs/plan.md` §2.5 is the reason both are here — **every shipped fixture
//! is turn one with every realm holding exactly one county**,
//! fires only above one county has no oracle at all. Forty turns is the
//! cheapest way to reach the game the project is trying to finish.

mod simulation;
pub use simulation::*;
mod handlers;
pub use handlers::*;

use l2_kingdom::map::{flags, CampaignMap, MAP_DIM, MAP_TILES};
use l2_kingdom::tables::Tables;
use l2_kingdom::{Kingdom, UnitKind};
use l2_game::game::Game;

/// How many turns each of the two games is played for. Ten years.
const TURNS: usize = 40;

/// What one realm looked like at the end.
#[derive(Debug, Clone, Copy)]
struct Scoreboard {
    realm: u8,
    in_play: bool,
    counties: u8,
    population: i32,
    /// Sacks in store across the realm's counties.
    grain: i32,
    /// The mean health meter over them — the number that falls first when a
    /// realm is being starved.
    health: i32,
    /// Fields laid to grain across the realm's counties.
    grain_fields: i32,
    armies: usize,
    men: i32,
    /// Armies carrying a mission other than 0 — i.e. ones AI step 9, 10 or the
/// garrison passes have given orders to.
    on_mission: usize,
}

fn scoreboard(k: &Kingdom, realm: u8) -> Scoreboard {
    let mut s = Scoreboard {
        realm,
        in_play: k.realms[realm as usize].in_play,
        counties: 0,
        population: 0,
        grain: 0,
        health: 0,
        grain_fields: 0,
        armies: 0,
        men: 0,
        on_mission: 0,
    };
    let mut health_sum = 0;
    for id in 1..=k.county_count {
        let c = &k.counties[id];
        if c.owner != realm {
            continue;
        }
        s.counties += 1;
        s.population += c.population;
        s.grain += c.grain;
        s.grain_fields += c.fields_grain;
        health_sum += c.health_meter;
    }
    if s.counties > 0 {
        s.health = health_sum / s.counties as i32;
    }
    for (_, u) in k.campaign.units.iter() {
        if u.owner == realm && u.kind == UnitKind::Army {
            s.armies += 1;
            s.men += u.men;
            if u.mission != 0 {
                s.on_mission += 1;
            }
        }
    }
    s
}

fn report(k: &Kingdom, label: &str) -> Vec<Scoreboard> {
    let rows: Vec<Scoreboard> = (1..=5).map(|r| scoreboard(k, r)).collect();
    eprintln!("--- {label} after {TURNS} turns (year {}) ---", k.year);
    for s in &rows {
        eprintln!(
            "  realm {} {:>3} counties  pop {:>6}  grain {:>7}  health {:>3}  \
             grain fields {:>3}  armies {:>2} ({} men, {} on mission)  in_play={}",
            s.realm,
            s.counties,
            s.population,
            s.grain,
            s.health,
            s.grain_fields,
            s.armies,
            s.men,
            s.on_mission,
            s.in_play
        );
    }
    rows
}

/// Fields laid to grain across the given realms' counties, as they stand now.
fn ai_grain_fields(k: &Kingdom, ai: &[u8]) -> i32 {
    ai.iter().map(|&r| scoreboard(k, r).grain_fields).sum()
}

/// The four assertions the whole file exists for, applied to whichever realms
/// the caller says are the AI's.
///
/// They are deliberately weak *individually* and strong together: the point is
/// not that any one number is right — no oracle in this project can say what
/// the AI's population should be in 1278 — but that a realm that has been left
/// to run for ten years is recognisably still playing the game.
///
/// `planted_peak` is the most fields the AI realms held laid to grain at the end
/// of any one turn, which the caller tracks while it plays: assertion 3 is about
/// the whole game, not its last turn.
fn assert_the_ai_is_playing(rows: &[Scoreboard], ai: &[u8], planted_peak: i32) {
    let ai_rows: Vec<&Scoreboard> = rows.iter().filter(|s| ai.contains(&s.realm)).collect();

    // 1. Alive. A realm that has lost every county to another AI is a fair
    //    outcome, so the assertion is over the set: they cannot *all* be gone.
    let alive = ai_rows.iter().filter(|s| s.in_play && s.counties > 0).count();
    assert!(alive > 0, "every AI realm was wiped out; the game has no opponents left");

    // 2. Fed. A realm that is starving loses its people first and its health
    // meter with them. Nothing here says what the numbers should be; it
    //    says a living realm still has people in it and is not on the floor.
    for s in ai_rows.iter().filter(|s| s.counties > 0) {
        assert!(s.population > 0, "realm {} has counties and nobody in them", s.realm);
        assert!(
            s.health > 0,
            "realm {} is at health {} — it has been starving for ten years",
            s.realm,
            s.health
        );
    }

    // 3. Planted. **This is the one the whole item was about.** Before
    //    `l2_kingdom::ai_farm` an AI realm could add fallow fields and could
    // never lay one to grain,
    //    realms that starve.
    //
    //    **Over the whole game, not at its last turn.** This used to count the
    //    grain fields standing at turn 40, which was a stand-in that happened to
    //    agree. In the synthetic world the conquering lord never lays grain on
    //    its own counties, so the count was carried by one small realm's last
    //    county. Correcting `Population_UpdateAll`'s extra person
    //    (C170) moved a couple of people at turn 4. That realm
    // then lost the county at turn 35 instead of holding it, and the count
    //    read zero — while realms 4 and 5 had kept 19 and 20 fields laid to
    //    grain from turn 5 on. The defect was an AI that could *never* plant, so
    //    *never* is what is asserted.
    assert!(
        planted_peak > 0,
        "no AI realm laid a single field to grain in {TURNS} turns — \
         the farming styles are not being dispatched"
    );

    // 4. Competing. An AI that never raises a man is not an opponent, and the
//    mission byte is what says the army was *given orders*
    // merely levied by the county-defence path.
    let armies: usize = ai_rows.iter().map(|s| s.armies).sum();
    let on_mission: usize = ai_rows.iter().map(|s| s.on_mission).sum();
    assert!(armies > 0, "no AI realm raised a single army in {TURNS} turns");
    assert!(
        on_mission > 0,
        "{armies} AI armies exist and not one carries a mission — step 9, 10 and the \
         garrison passes are not running"
    );
}

/// The hand-built world. Kept in one place so the three tests above cannot
/// drift apart.
fn six_county_world() -> Game {
    let mut game = Game::new(0xA1_1EED);
    game.player = 1;
    game.kingdom.set_county_count(6);
    l2_testkit::chain_neighbours!(game.kingdom);
    game.kingdom.season = 4;
    game.kingdom.season_next = 1;
    game.kingdom.year = 1268;
    game.kingdom.year_next = 1269;
    game.kingdom.turn_count = 1;

    // Six vertical strips, ten tiles wide, county 1 in the west. A road runs
    // the length of row 10 so armies and merchants have something to walk.
    let mut map = CampaignMap::empty();
    for i in 0..MAP_TILES {
        let x = i % MAP_DIM;
        map.county[i] = ((x / 10) + 1).min(6) as u8;
    }
    for x in 0..MAP_DIM as u8 {
        map.set_flags(x, 10, flags::ROAD);
    }
    for id in 1..=6usize {
        let cx = (id as u8 - 1) * 10 + 4;
        // The county town: a 2x2 block of plane-0 `0x40`, which is what the
        // AI's `Aim::Town` walks at and what `Army_AttackCounty` fires on.
        for (dx, dy) in [(0u8, 0u8), (1, 0), (0, 1), (1, 1)] {
            map.set_flags(cx + dx, 12 + dy, flags::CASTLE);
            map.terrain[l2_kingdom::map::index(cx + dx, 12 + dy)] =
                l2_kingdom::map::terrain::TOWN;
        }
        // Twenty tiles of workable farmland per county, which is what the
        // farming styles lay out and what a raid heads for. **The flag is not
        // enough**: a county's fields are the twenty tile indices in
        // `County::field_tiles`, and `field::recount` reads only those —
        // world that painted the flag and left the slots empty would have the
        // AI lay out nothing and would say so with the same message as an AI
// that was never dispatched. That is the failure this file
        // exists to tell apart, and it caught itself first time out.
        let c = &mut game.kingdom.counties[id];
        for row in 0..4u8 {
            for col in 0..5u8 {
                let (x, y) = ((id as u8 - 1) * 10 + col, 20 + row);
                map.set_flags(x, y, flags::FARMLAND);
                map.terrain[l2_kingdom::map::index(x, y)] = 1; // fallow
                c.set_field_tile((row * 5 + col) as usize, Some(l2_kingdom::map::index(x, y)));
            }
        }
        c.anchor_x = cx;
        c.anchor_y = 12;
        c.population = 900;
        c.pop_last = 900;
        c.happiness = 70;
        c.happiness_last = 70;
        c.health_meter = 70;
        c.health_band = l2_kingdom::tables::health_band(70) as u8;
        c.herd = 120;
        c.grain = 2_000;
        c.ration_wanted = 3;
        c.ration_split = 100;
        for n in 1..=6u8 {
            if n as usize != id {
                c.add_neighbour(n);
            }
        }
    }
    game.kingdom.campaign.map = map;

    for realm in 1..=5usize {
        game.kingdom.counties[realm].owner = realm as u8;
        let r = &mut game.kingdom.realms[realm];
        r.in_play = true;
        r.strength = 3;
        r.county_count = 1;
        r.gold = 5_000;
        r.wood = 2_000;
        r.stone = 2_000;
        r.iron = 2_000;
        // Enough of every weapon that the muster-arms gate clears for all four
        // lords: the Bishop wants 250 in stock before he will raise anything.
        r.weapons = [400; 6];
        r.shield_index = realm as u8;
        // Lord 0 is the human; lords 1..=4 are the Knight, Baron, Countess and
        // Bishop, one to a realm, so no personality row goes unexercised.
        r.lord = realm as u8 - 1;
    }
    game.kingdom.realms[1].is_human = true;
    game.kingdom.realms[1].lord = 0;
    // `Diplo_Init`, last, for the reason `setup::Settings::apply_to` runs it
    // last: the opening standing depends on which realms are in play and which
    // are people, and both are decided in the loop above.
    game.kingdom.init_diplomacy();
    game
}


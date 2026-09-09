//! **Are the AI realms competing, or starving?**
//!
//! This is the test `docs/agents.md` C27 asks for, on the subject C27 is
//! about. The grain economy was *"finished, tested and unreachable in play"* —
//! nobody farmed, and the suite was green — because every test drove the rules
//! directly and none of them drove the game. The AI's fourteen turn handlers
//! are the same shape: a handler that exists as library code and is never
//! dispatched passes every unit test it has.
//!
//! So the assertion here is not *"the allocator returns the right numbers"*.
//! It is **"play N turns of a real game and the AI realms are still alive,
//! still fed, and their fields are planted"** — and it is the only test in the
//! workspace that can catch a handler that was implemented and then forgotten
//! at the dispatch.
//!
//! # Two worlds, on purpose
//!
//! * [`the_ai_realms_are_competing_after_forty_turns_of_england`] plays the
//!   **England turn-one fixture** — a real position, a real map, the real
//!   fourteen counties — and is gated on a copy of the game.
//! * [`the_ai_realms_survive_forty_turns_of_a_world_built_by_hand`] plays a
//!   world this file builds, so that CI asserts something too.
//!
//! Neither is worth much alone. The synthetic one runs everywhere and proves
//! only that the dispatch is wired; the England one is the evidence, and
//! `docs/plan.md` §2.5 is the reason both are here — **every shipped fixture
//! is turn one with every realm holding exactly one county**, so a rule that
//! fires only above one county has no oracle at all. Forty turns is the
//! cheapest way to reach the game the project is trying to finish.

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
    /// garrison passes have actually given orders to.
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

/// The four assertions the whole file exists for, applied to whichever realms
/// the caller says are the AI's.
///
/// They are deliberately weak *individually* and strong together: the point is
/// not that any one number is right — no oracle in this project can say what
/// the AI's population should be in 1278 — but that a realm that has been left
/// to run for ten years is recognisably still playing the game.
fn assert_the_ai_is_playing(rows: &[Scoreboard], ai: &[u8]) {
    let ai_rows: Vec<&Scoreboard> = rows.iter().filter(|s| ai.contains(&s.realm)).collect();

    // 1. Alive. A realm that has lost every county to another AI is a fair
    //    outcome, so the assertion is over the set: they cannot *all* be gone.
    let alive = ai_rows.iter().filter(|s| s.in_play && s.counties > 0).count();
    assert!(alive > 0, "every AI realm was wiped out; the game has no opponents left");

    // 2. Fed. A realm that is starving loses its people first and its health
    //    meter with them. Nothing here says what the numbers should be; it
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
    //    never lay one to grain, so a game played to the end was played against
    //    realms that starve.
    let planted: i32 = ai_rows.iter().map(|s| s.grain_fields).sum();
    assert!(
        planted > 0,
        "no AI realm has a single field laid to grain after {TURNS} turns — \
         the farming styles are not being dispatched"
    );

    // 4. Competing. An AI that never raises a man is not an opponent, and the
    //    mission byte is what says the army was *given orders* rather than
    //    merely levied by the county-defence path.
    let armies: usize = ai_rows.iter().map(|s| s.armies).sum();
    let on_mission: usize = ai_rows.iter().map(|s| s.on_mission).sum();
    assert!(armies > 0, "no AI realm raised a single army in {TURNS} turns");
    assert!(
        on_mission > 0,
        "{armies} AI armies exist and not one carries a mission — step 9, 10 and the \
         garrison passes are not running"
    );
}

/// **The real game.** The England turn-one fixture, played for [`TURNS`] turns.
#[test]
fn the_ai_realms_are_competing_after_forty_turns_of_england() {
    let save = l2_testkit::england!();
    let mut game =
        l2_game::scenario::from_save(&save, Tables::DEFAULT).expect("the fixture loads");

    // Who is who is rolled per game (`l2_testkit::ENGLAND_TURN1_COUNTIES`), so
    // the AI set is derived rather than written down.
    let ai: Vec<u8> = (1..=5)
        .filter(|&r| game.kingdom.realms[r as usize].in_play && !game.kingdom.realms[r as usize].is_human)
        .collect();
    assert_eq!(ai.len(), 4, "four AI realms and one person");

    let before = report(&game.kingdom, "England, before");
    for _turn in 1..=TURNS {
        l2_game::turn::end_turn(&mut game).expect("the machine comes round");
    }
    let after = report(&game.kingdom, "England");

    assert_the_ai_is_playing(&after, &ai);

    // The map moved. Every realm starts on exactly one county
    // (`docs/plan.md` §2.5), so *any* change in the ownership spread is the
    // first evidence this project has that the mid-game is reachable at all.
    let counties_before: i32 = before.iter().map(|s| s.counties as i32).sum();
    let counties_after: i32 = after.iter().map(|s| s.counties as i32).sum();
    eprintln!("owned counties: {counties_before} -> {counties_after} of {}", game.kingdom.county_count);
}

/// **A world built by hand**, so that a run with no copy of the game still
/// asserts that the AI is dispatched.
///
/// Six counties in a row along a road, five of them owned: realm 1 is the
/// person and realms 2…5 are the four lords, one each, so every lord's
/// personality is exercised. County 6 is nobody's, which gives the AI
/// somewhere to expand into and the neutral farming pass something to do.
#[test]
fn the_ai_realms_survive_forty_turns_of_a_world_built_by_hand() {
    let mut game = six_county_world();
    let ai = [2u8, 3, 4, 5];
    for _turn in 1..=TURNS {
        l2_game::turn::end_turn(&mut game).expect("the machine comes round");
    }
    let rows = report(&game.kingdom, "six counties");
    assert_the_ai_is_playing(&rows, &ai);
}

/// **The AI's war is a pure function of where it started.** `docs/netcode.md`
/// §5: every scan in `l2_kingdom::ai_army` is an ascending index or an
/// ascending tile offset and nothing draws a random number, so two runs of the
/// same forty turns have to agree bit for bit — which is what the lockstep
/// digest is.
#[test]
fn forty_turns_of_the_ai_at_war_is_deterministic() {
    let digest = || {
        let mut game = six_county_world();
        for _ in 1..=TURNS {
            l2_game::turn::end_turn(&mut game).expect("the machine comes round");
        }
        let mut c = l2_net::Canonical::hashing();
        l2_net::Encode::encode(&game.kingdom, &mut c);
        c.finish()
    };
    let a = digest();
    let b = digest();
    assert_eq!(a.hash, b.hash, "two identical games diverged");
    assert_eq!(a.len, b.len);
}

/// **The audit C27 asks for, turned into an assertion: for every field these
/// handlers read, what writes it in a real game?**
///
/// Four of `l2_kingdom::ai_army`'s inputs are written by
/// `l2_kingdom::diplomacy`, and there is no such module — `crate::realm`'s doc
/// comments link to seven of its functions and every link is dangling. So:
///
/// | field | its only writer | what is unreachable without it |
/// |---|---|---|
/// | `Realm::pairs[].standing` | `Diplo_Init`, `Diplo_Offend`, the seven reply handlers | **AI step 10 entirely** — `pick_raid_victim` wants a standing below −10 |
/// | `Realm::war_target` | `Diplo_Offend` | the same, and step 9's halved population floor |
/// | `Realm::ally` | `Diplo_FormAlliance` | `Mission::ASSIST_ALLY`, and `action_allowed`'s grudge bump |
/// | `Realm::target_county` | `Diplo_PayForHelp` | step 9's ally-request branch |
///
/// **This is C27's exact shape and it is worth naming before somebody finds it
/// the expensive way**: the handlers are written, dispatched and tested, and in
/// a played game two of them can never fire. The test below proves *both*
/// halves — that nothing writes a standing over forty turns, and that the
/// handler works the moment something does — so the day diplomacy lands, the
/// first assertion goes red and says why.
#[test]
fn the_raid_cannot_fire_until_something_writes_a_standing() {
    let mut game = six_county_world();
    for _turn in 1..=TURNS {
        l2_game::turn::end_turn(&mut game).expect("the machine comes round");
    }
    // Half one: nothing in a played game moves a standing off zero.
    for realm in 1..=5 {
        for other in 0..l2_kingdom::MAX_REALMS {
            assert_eq!(
                game.kingdom.realms[realm].pair(other as u8).standing,
                0,
                "realm {realm} has an opinion of realm {other} — has `diplomacy` landed? \
                 If so this test has done its job and should be replaced by one that \
                 asserts the raid fires on its own."
            );
        }
        assert_eq!(game.kingdom.realms[realm].war_target, 0);
        assert_eq!(game.kingdom.realms[realm].ally, 0);
    }

    // Half two: the handler is not broken, it is starved. Give realm 2 an
    // opinion of realm 3 and the raid goes out on the next step 10.
    let mut game = six_county_world();
    l2_game::turn::end_turn(&mut game).expect("one turn to settle the muster county");
    game.kingdom.realms[2].pair_mut(3).standing = -20;
    game.kingdom.realms[2].raid_timer = 0;
    let before = game.kingdom.campaign.units.len();
    let raider = game.kingdom.run_ai_raid(2);
    assert!(raider.is_some(), "a rival at −20 is a rival worth raiding");
    assert_eq!(game.kingdom.campaign.units.len(), before + 1);
    let u = game.kingdom.campaign.units.get(raider.unwrap()).expect("just raised");
    assert_eq!(u.mission, l2_kingdom::ai_army::Mission::RAID);
    assert_eq!(
        game.kingdom.counties[u.dest_county as usize].owner, 3,
        "and it is aimed at the realm it thinks worst of"
    );
    // `FUN_004A5003` opens no armoury: a raiding party carries nothing.
    assert_eq!(u.troops.iter().sum::<i32>(), u.troops[0], "peasants and nothing else");
}

/// **An AI realm's farming never reads the county's stored style byte**, and
/// that is why the missing `County::farm_style` import could not have skewed
/// anything measured here.
///
/// `Ai_ManageCountyFarms` (`0x0049DD01`) *writes* county `+0x1FE` from the
/// **lord's** personality record on every pass and dispatches on what it just
/// wrote. So an AI-owned county's stored style is an output, not an input, and
/// a game loaded with the field at zero reaches the lord's real style on its
/// first turn. The field is a genuine input in exactly one place —
/// `AI_ManageFields(0)`, the pass over the counties **nobody owns** — which is
/// where a missing import does change behaviour.
///
/// Stated as a test rather than as a paragraph because *"the allocator behaves
/// the same for every county"* and *"every county had style 0"* are
/// indistinguishable from the outside, and the first is the far more
/// interesting claim.
#[test]
fn an_ai_owned_countys_farm_style_comes_from_its_lord_and_not_from_the_county() {
    let mut game = six_county_world();
    // A style byte no lord has, so anything that survives came from the county.
    for id in 1..=6 {
        game.kingdom.counties[id].farm_style = 7;
    }
    l2_game::turn::end_turn(&mut game).expect("the machine comes round");
    for realm in 2..=5u8 {
        let lord = game.kingdom.realms[realm as usize].lord;
        let expected = Tables::DEFAULT.ai_farm_style(lord).expect("a lord with a record");
        for id in 1..=6 {
            if game.kingdom.counties[id].owner != realm {
                continue;
            }
            assert_eq!(
                game.kingdom.counties[id].farm_style, expected,
                "county {id} kept its own byte instead of taking realm {realm}'s lord's"
            );
        }
    }
    // The human's county and the unowned one are the other half of the claim:
    // nothing overwrites them, so a wrong import really would be read there.
    assert_eq!(game.kingdom.counties[1].farm_style, 7, "the person's county is left alone");
}

/// The hand-built world. Kept in one place so the three tests above cannot
/// drift apart.
fn six_county_world() -> Game {
    let mut game = Game::new(0xA1_1EED);
    game.player = 1;
    game.kingdom.set_county_count(6);
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
        // `County::field_tiles`, and `field::recount` reads only those — so a
        // world that painted the flag and left the slots empty would have the
        // AI lay out nothing and would say so with the same message as an AI
        // that was never dispatched. That is precisely the failure this file
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
    game
}

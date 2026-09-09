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
/// Four of `l2_kingdom::ai_army`'s inputs are written by `l2_kingdom::diplomacy`
/// and by nothing else:
///
/// | field | its only writer | what was unreachable without it |
/// |---|---|---|
/// | `Realm::pairs[].standing` | `Diplo_Init`, `Diplo_Offend`, the seven reply handlers | **AI step 10 entirely** — `pick_raid_victim` wants a standing below −10 |
/// | `Realm::war_target` | `Diplo_Offend` | the same, and step 9's halved population floor |
/// | `Realm::ally` | `Diplo_FormAlliance` | `Mission::ASSIST_ALLY`, and `action_allowed`'s grudge bump |
/// | `Realm::target_county` | `Diplo_PayForHelp` | step 9's ally-request branch |
///
/// > **This test used to assert the opposite**, and it asked in its own message
/// > to be replaced the day the module landed:
/// >
/// > > *"realm N has an opinion of realm M — has `diplomacy` landed? If so this
/// > > test has done its job and should be replaced by one that asserts the
/// > > raid fires on its own."*
/// >
/// > It has, and this is that test. What it asserts now is the thing that was
/// > missing: **not that the handler returns the right answer when a test hands
/// > it a standing, but that a played game produces one.** That is
/// > `docs/agents.md`'s rule — *a field is only tested if something a test
/// > reads was written by something the game runs* — read forwards instead of
/// > backwards.
#[test]
fn a_played_game_writes_the_four_fields_the_war_handlers_read() {
    let mut game = six_county_world();
    // Turn one, before anything has run: `Diplo_Init`'s opening position.
    // A person's row is all zeros and an AI's is all fives — including its own
    // slot, because the original's loop has no `other != me` guard.
    assert_eq!(game.kingdom.realms[1].pair(2).standing, 0, "realm 1 is the person");
    assert_eq!(game.kingdom.realms[2].pair(1).standing, 5);
    assert_eq!(game.kingdom.realms[2].pair(2).standing, 5);

    for _turn in 1..=TURNS {
        l2_game::turn::end_turn(&mut game).expect("the machine comes round");
    }

    let mut opinions = 0;
    let mut hostile = 0;
    let mut at_war = 0;
    let mut war_targets = 0;
    let mut allies = 0;
    for realm in 1..=5 {
        let r = &game.kingdom.realms[realm];
        if r.war_target != 0 {
            war_targets += 1;
        }
        if r.ally != 0 {
            allies += 1;
        }
        for other in 1..l2_kingdom::MAX_REALMS {
            let p = r.pair(other as u8);
            if p.standing != 0 {
                opinions += 1;
            }
            if p.standing < -10 {
                hostile += 1;
            }
            if p.at_war {
                at_war += 1;
            }
        }
    }
    eprintln!(
        "after {TURNS} turns: {opinions} non-zero standings, {hostile} below −10, \
         {at_war} at war, {war_targets} war targets, {allies} realms allied"
    );
    for realm in 1..=5 {
        let r = &game.kingdom.realms[realm];
        let row: Vec<String> = (1..l2_kingdom::MAX_REALMS)
            .map(|o| {
                let p = r.pair(o as u8);
                format!(
                    "{:>4}{}{}",
                    p.standing,
                    if p.allied { "A" } else { " " },
                    if p.at_war { "W" } else { " " }
                )
            })
            .collect();
        eprintln!(
            "  realm {realm} ally={} warTarget={} | {}",
            r.ally,
            r.war_target,
            row.join(" ")
        );
    }

    assert!(opinions > 0, "forty turns and nobody has an opinion of anybody");

    // **These two assertions are about step 2 specifically, and the weaker ones
    // they replaced were not.** The first draft asserted "somebody has a war
    // target, an ally, or an opinion below −10" — and it **passed with the step
    // 2 dispatch deleted**, because the battle hook writes standings on its
    // own. `docs/agents.md`: *ablate the thing the test is about, and watch it
    // go red.* This version does.
    //
    // Only `AI_Diplomacy`'s heal can put a standing **above** `Diplo_Init`'s
    // opening 5 — every other writer in the subsystem subtracts, and the three
    // that add arrive through the inbox, which nothing fills in an AI-only
    // game. And only its courtship can produce an ally, for the same reason:
    // the other route to one is a person accepting an offer.
    let healed = (1..l2_kingdom::MAX_REALMS).any(|r| {
        (1..l2_kingdom::MAX_REALMS)
            .any(|o| game.kingdom.realms[r].pair(o as u8).standing > 5)
    });
    assert!(
        healed,
        "no standing anywhere is above `Diplo_Init`'s opening 5 — `AI_Diplomacy`'s \
         heal is the only thing that can raise one without a letter, so step 2 is \
         not being dispatched"
    );
    assert!(
        allies > 0,
        "forty turns and not one alliance — `AI_Diplomacy`'s courtship is the only \
         way two AI realms can reach one, so step 2 is not being dispatched"
    );
    // And the war half, which is the offence hook rather than step 2: it is
    // asserted separately so that a failure says which of the two broke.
    assert!(
        hostile > 0 || war_targets > 0,
        "forty turns of war and nobody resents anybody — `Diplo_Offend` is not \
         reaching the battle and trample seams"
    );

    // And the raid still fires on the input the AI now supplies for itself.
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

/// **AI step 1, driven through the dispatch rather than called.**
///
/// The forty-turn test above cannot see step 1 at all, and that is a fact about
/// the game rather than a hole in the test: `Diplo_AnswerInbox` answers letters,
/// only `Diplo_Post` writes one, and in single player only a person ever posts.
/// An AI-only game therefore runs step 1 forty times over an empty inbox.
///
/// So the letter is posted the way the diplomacy screen posts it, a turn is
/// played, and what is asserted is the round trip: the gold left when it was
/// **posted**, the reply came back on the AI's next turn, the standing moved by
/// the amount the tier says, and the inbox was emptied.
#[test]
fn a_letter_posted_by_a_person_is_answered_on_the_ai_s_next_turn() {
    let mut game = six_county_world();
    // Realm 2 is the Knight: gift increment 100, so 100 crowns against an
    // opening `best_gift` of 0 is the top tier and worth +10.
    assert_eq!(game.kingdom.realms[2].lord, 1, "realm 2 is the Knight");
    let purse = game.kingdom.realms[1].gold;
    let theirs = game.kingdom.realms[2].gold;

    game.kingdom.post_letter(1, 2, l2_kingdom::DiploKind::Gift, 100, 0);
    assert_eq!(game.kingdom.realms[1].gold, purse - 100, "spent at the post office");
    assert_eq!(game.kingdom.realms[2].gold, theirs + 100);
    assert!(game.kingdom.realms[2].pair(1).has_mail);
    assert_eq!(game.kingdom.diplomacy.pending(2).count(), 1);

    let before = game.kingdom.realms[2].pair(1).standing;
    l2_game::turn::end_turn(&mut game).expect("the machine comes round");

    assert_eq!(game.kingdom.diplomacy.pending(2).count(), 0, "the inbox is emptied every turn");
    assert!(!game.kingdom.realms[2].pair(1).has_mail);
    assert_eq!(game.kingdom.realms[2].pair(1).best_gift, 100, "and the bar has ratcheted");
    // +10 for the gift and +1 for step 2's heal — except that the heal's guard
    // is on the *other* realm being non-human and realm 1 is a person, so the
    // heal does not run on this pair at all.
    assert_eq!(
        game.kingdom.realms[2].pair(1).standing,
        before + 10,
        "ten for a top-tier gift, and not eleven: `AI_Diplomacy` never heals \
         towards a person"
    );
}

/// **Forty turns of England, with the diplomatic state printed.**
///
/// The companion to [`the_ai_realms_are_competing_after_forty_turns_of_england`]
/// and gated the same way. It exists because `docs/decisions.md` C26 is exactly
/// about this subsystem: **every shipped fixture is turn one with every realm
/// holding exactly one county**, so every diplomatic state above *"everyone is
/// neutral and equal"* has no oracle at all. Playing the position out is the
/// only way this project can look at one.
///
/// It asserts the invariants that hold whatever the numbers are, and prints the
/// numbers — which is the right division, because nothing can say what realm
/// 3's opinion of realm 5 *should* be in 1278.
#[test]
fn forty_turns_of_england_leaves_a_diplomatic_position() {
    let save = l2_testkit::england!();
    let mut game =
        l2_game::scenario::from_save(&save, Tables::DEFAULT).expect("the fixture loads");
    for _turn in 1..=TURNS {
        l2_game::turn::end_turn(&mut game).expect("the machine comes round");
    }
    eprintln!("--- England, diplomacy after {TURNS} turns (year {}) ---", game.kingdom.year);
    for realm in 1..=5 {
        let r = &game.kingdom.realms[realm];
        if !r.in_play {
            eprintln!("  realm {realm} is out of play");
            continue;
        }
        let row: Vec<String> = (1..l2_kingdom::MAX_REALMS)
            .map(|o| {
                let p = r.pair(o as u8);
                format!(
                    "{:>4}{}{}",
                    p.standing,
                    if p.allied { "A" } else { " " },
                    if p.at_war { "W" } else { " " }
                )
            })
            .collect();
        eprintln!(
            "  realm {realm} lord={} human={} ally={} warTarget={} | {}",
            r.lord,
            r.is_human,
            r.ally,
            r.war_target,
            row.join(" ")
        );
    }

    // The invariants, and they are the ones the original's own functions
    // maintain rather than any number this position happens to produce.
    for realm in 1..l2_kingdom::MAX_REALMS {
        let r = &game.kingdom.realms[realm];
        for other in 1..l2_kingdom::MAX_REALMS {
            let p = r.pair(other as u8);
            assert!(
                (l2_kingdom::diplomacy::STANDING_MIN..=l2_kingdom::diplomacy::STANDING_MAX)
                    .contains(&p.standing),
                "realm {realm}'s standing towards {other} is {} — a write site did not clamp",
                p.standing
            );
            assert!(p.help_price_multiple >= 1, "the multiple never falls below its opening 1");
            // An alliance is symmetric and exclusive: `Realm::ally` is one byte
            // and `Diplo_ReconcileAlliances` runs every turn.
            if p.allied {
                assert_eq!(r.ally, other as u8, "realm {realm} is allied to {other} and to nobody");
                assert_eq!(game.kingdom.realms[other].ally, realm as u8, "and it is mutual");
            }
        }
        if r.ally != 0 {
            assert!(r.ally as usize != realm, "no realm allies itself");
            assert!(
                game.kingdom.realms[r.ally as usize].in_play,
                "realm {realm} is allied to a realm that is out of play"
            );
        }
        // **A person's row can only ever go down**, and it takes one very
        // specific act to move it at all.
        //
        // `Diplo_Offend`'s entry guard refuses outright when the offended realm
        // is human, and `AI_Diplomacy` — the only writer that runs without a
        // letter — never runs for a human realm, because the AI turn machine is
        // skipped for one. So a person's row has no source of *gain* whatever.
        // The one writer that reaches it is `Diplo_OffendAll`, which walks
        // realms 1..5 with **no `isHuman` test**, and which is reached from one
        // place only: somebody betraying an ally. So a person's opinion of a
        // rival is 0 until that rival breaks a treaty, and −15 a betrayal
        // thereafter, for ever.
        //
        // Stated as an assertion because it is easy to write it the other way
        // round — *"the human keeps no standing at all"* — and that is wrong by
        // exactly one function.
        if r.is_human {
            for other in 1..l2_kingdom::MAX_REALMS {
                let s = r.pair(other as u8).standing;
                assert!(
                    s <= 0 && s % 15 == 0,
                    "a person's realm holds {s} towards realm {other}; the only writer \
                     that reaches a human's row is `Diplo_OffendAll` at −15 a betrayal"
                );
            }
        }
    }
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
    // `Diplo_Init`, last, for the reason `setup::Settings::apply_to` runs it
    // last: the opening standing depends on which realms are in play and which
    // are people, and both are decided in the loop above.
    game.kingdom.init_diplomacy();
    game
}

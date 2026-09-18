#![allow(unused_imports)]
use super::*;
use super::handlers::*;
use l2_kingdom::map::{flags, CampaignMap, MAP_DIM, MAP_TILES};
use l2_kingdom::tables::Tables;
use l2_kingdom::{Kingdom, UnitKind};
use l2_game::game::Game;

#[test]
fn the_ai_realms_are_competing_after_forty_turns_of_england() {
    let save = l2_testkit::england!();
    let mut game =
        l2_game::scenario::from_save(&save, Tables::DEFAULT).expect("the fixture loads");

    let ai: Vec<u8> = (1..=5)
        .filter(|&r| game.kingdom.realms[r as usize].in_play && !game.kingdom.realms[r as usize].is_human)
        .collect();
    assert_eq!(ai.len(), 4, "four AI realms and one person");

    let before = report(&game.kingdom, "England, before");
    let mut planted_peak = 0;
    for _turn in 1..=TURNS {
        l2_game::turn::end_turn(&mut game).expect("the machine comes round");
        planted_peak = planted_peak.max(ai_grain_fields(&game.kingdom, &ai));
    }
    let after = report(&game.kingdom, "England");

    assert_the_ai_is_playing(&after, &ai, planted_peak);

    let counties_before: i32 = before.iter().map(|s| s.counties as i32).sum();
    let counties_after: i32 = after.iter().map(|s| s.counties as i32).sum();
    eprintln!("owned counties: {counties_before} -> {counties_after} of {}", game.kingdom.county_count);
}

#[test]
fn the_ai_realms_survive_forty_turns_of_a_world_built_by_hand() {
    let mut game = six_county_world();
    let ai = [2u8, 3, 4, 5];
    let mut planted_peak = 0;
    for _turn in 1..=TURNS {
        l2_game::turn::end_turn(&mut game).expect("the machine comes round");
        planted_peak = planted_peak.max(ai_grain_fields(&game.kingdom, &ai));
    }
    let rows = report(&game.kingdom, "six counties");
    eprintln!("most fields the AI realms held laid to grain at once: {planted_peak}");
    assert_the_ai_is_playing(&rows, &ai, planted_peak);
}

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

/// The companion to [`the_ai_realms_are_competing_after_forty_turns_of_england`]
/// and gated the same way. It exists because `docs/decisions.md` C26 is exactly
/// about this subsystem: **every shipped fixture is turn one with every realm
/// holding exactly one county**, so every diplomatic state above *"everyone is
/// neutral and equal"* has no oracle at all. Playing the position out is the
/// only way this project can look at one.
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


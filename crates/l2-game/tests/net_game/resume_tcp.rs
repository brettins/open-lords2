//! C263: a four-seat game saved, then continued from the lobby: Matilda back, John's
//! realm to Eleanor, who is new, Geoffrey's to the computer. Real TCP on 127.0.0.1.

use crate::kit::{self, Seat};
use crate::seats::assets;
use l2_game::net::{self, Phase, Request, Resume};
use l2_kingdom::tables::Tables;
use l2_net::PlayerSlot;
use std::time::{Duration, Instant};

/// Ablations: the `installs` lookup in `net::own_seat` (the save names the host alone,
/// Matilda arrives unseated); `realm.is_human = seated(..)` in `resume::build`
/// (Geoffrey's realm stays human); `rebuild_move_cost` in `resume::build` (B108).
#[test]
fn a_saved_game_continues_with_its_players_seated_by_install_and_a_newcomer_placed() {
    kit::big_stack(resume);
}

fn resume() {
    let Some(assets) = assets() else { l2_testkit::skip!("no install") };
    let _scope = kit::scope("resume");
    let (mut first, _) = kit::game(&assets, 4);
    let realms: Vec<u8> = first.iter().map(|s| s.game.player).collect();
    net::own_seat(&mut first[0].game);
    assert_eq!(first[0].game.owners.len(), 4, "the save names every seat's install");
    let bytes = l2_game::save::encode(&first[0].game);
    let turns = first[0].game.turns_played;
    drop(first);

    let saved = l2_game::save::decode(&bytes, Tables::DEFAULT).expect("the save reads");
    let resume = Resume::of(&saved, bytes);
    let (host, port) = kit::host_of(&assets, Some(resume));
    let mut seats = vec![host, Seat::joiner("Eleanor", 9, port), Seat::joiner("Matilda", 1, port)];
    let t0 = Instant::now();
    while seats[0].game.net.roster.len() < 3 {
        assert!(t0.elapsed() < Duration::from_secs(30), "the room never filled");
        seats.iter_mut().for_each(|s| s.pump(&assets));
        std::thread::sleep(Duration::from_millis(1));
    }
    let roster = &seats[0].game.net.roster;
    let seat_of = |name: &str| roster.players.iter().find(|p| p.name == name).map(|p| (p.slot, p.realm)).unwrap();
    assert_eq!(seat_of("Richard").1, realms[0], "the host's install claims its realm");
    assert_eq!(seat_of("Matilda").1, realms[1], "Matilda's install claims hers");
    let (eleanor, unseated) = seat_of("Eleanor");
    assert_eq!(unseated, 0, "a new install waits for the host");
    seats[0].game.net.requests.push(Request::Assign { slot: eleanor, realm: realms[2] });
    kit::start(&mut seats, &assets, 3);

    for s in &seats {
        let g = &s.game;
        assert_eq!(g.turns_played, turns, "the world is the save's");
        let c = &g.kingdom.campaign;
        assert!(c.move_cost == c.map.cost_map(), "every seat builds the cost map the save leaves out");
        let human: Vec<u8> = (1..6u8).filter(|&r| g.kingdom.realms[r as usize].is_human).collect();
        assert_eq!(human, realms[..3].to_vec(), "Geoffrey's realm {} is the computer's", realms[3]);
    }
    assert_eq!(seats[1].game.player, realms[2], "Eleanor plays John's realm");
    assert_eq!(seats[2].game.player, realms[1]);
    assert_eq!(seats[0].game.realm_of(PlayerSlot::new(0)), Some(realms[0]));
    kit::until(&mut seats, &assets, "a season passes", |s| s.iter().all(|x| x.game.turns_played > turns));
    for s in &seats {
        assert!(s.game.net.halted.is_none() && s.game.net.phase == Phase::Started, "{:?}", s.game.net.halted);
    }
    let t = kit::common_tick(&seats);
    assert!(seats[0].hash_at(t).is_some());
    assert!(seats.iter().all(|s| s.hash_at(t) == seats[0].hash_at(t)), "the seats disagree at {t:?}");
    assert_eq!(seats.iter().map(|s| s.live().session.divergences()).sum::<u32>(), 0);
}

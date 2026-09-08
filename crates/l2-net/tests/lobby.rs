//! The lobby: host, join, agree, start.
//!
//! Two things are being checked here, and only one of them is the obvious one.
//!
//! The obvious one is that a lobby does lobby things — a host opens a game, a
//! client joins, names and slots and readiness behave.
//!
//! The one that matters is that **the lobby hands `Session::new` the same
//! arguments on every machine**. The seed and the slot list are load-bearing:
//! the seed feeds every `Pcg32` in the simulation, and the slot list's *order*
//! reaches `order_commands` and decides how contested commands are sequenced. A
//! lobby that produced a roster in arrival order would look completely correct
//! on screen and desync on the first tick where two players acted at once. The
//! last test in this file closes that loop by actually starting a session from
//! the lobby's output and running it over a real socket.

mod common;

use common::{Order, ToySim};
use l2_net::{
    decode_all, frame, Advance, Canonical, Config, FrameReader, Hello, Lobby, LobbyError,
    LobbyEvent, Message, Mismatch, PeerId, PlayerSlot, Role, Roster, Session, TcpTransport, Tick,
    Transport, PROTOCOL_VERSION,
};

const SEED: u64 = 0x10_2517_9600;

fn hello(slot: u8) -> Hello {
    Hello {
        protocol: PROTOCOL_VERSION,
        engine: "lords2 0.1.0-test".to_string(),
        ruleset_hash: 0xABCD_EF01_2345_6789,
        seed: SEED,
        slot: PlayerSlot::new(slot),
    }
}

/// Drive a host and a set of clients with no transport at all.
///
/// The lobby is transport-free by design, so most of its behaviour can be
/// tested without a socket, a clock or a thread. The socket appears only in the
/// last test, where it is genuinely the thing under test.
struct Table {
    host: Lobby,
    clients: Vec<(PeerId, Lobby)>,
}

impl Table {
    fn new(host_name: &str) -> Table {
        Table { host: Lobby::host(hello(0), host_name).unwrap(), clients: Vec::new() }
    }

    fn join(&mut self, peer: u32, h: Hello, name: &str) -> Result<LobbyEvent, LobbyError> {
        let mut client = Lobby::join(h, name).unwrap();
        client.greet().unwrap();
        let out = client.next_outgoing().expect("a greeting");
        let event = self.host.receive(PeerId(peer), out.message);
        self.clients.push((PeerId(peer), client));
        self.pump();
        event
    }

    /// Deliver everything the host has queued to every client, and back.
    fn pump(&mut self) {
        for _ in 0..8 {
            while let Some(out) = self.host.next_outgoing() {
                for (peer, client) in self.clients.iter_mut() {
                    if out.to.is_none() || out.to == Some(*peer) {
                        // A refusal is an error at the client, which is the
                        // point of it; the tests that care assert on it
                        // directly.
                        let _ = client.receive(PeerId(0), out.message.clone());
                    }
                }
            }
            let mut relayed = Vec::new();
            for (peer, client) in self.clients.iter_mut() {
                while let Some(out) = client.next_outgoing() {
                    relayed.push((*peer, out.message));
                }
            }
            if relayed.is_empty() {
                return;
            }
            for (peer, message) in relayed {
                let _ = self.host.receive(peer, message);
            }
        }
    }

    fn client(&self, i: usize) -> &Lobby {
        &self.clients[i].1
    }
}

#[test]
fn a_host_occupies_its_own_slot_and_is_ready_by_definition() {
    let lobby = Lobby::host(hello(0), "Richard").unwrap();
    assert_eq!(lobby.role(), Role::Host);
    assert_eq!(lobby.roster().len(), 1);
    let host = &lobby.roster().players[0];
    assert_eq!(host.slot, PlayerSlot::new(0));
    assert_eq!(host.name, "Richard");
    assert!(host.is_host);
    assert!(host.ready, "a host waiting on itself would never start");
    assert!(!lobby.roster().all_ready(), "one player is not a game");
}

#[test]
fn a_client_that_joins_appears_on_both_sides() {
    let mut t = Table::new("Richard");
    assert_eq!(t.join(1, hello(1), "Matilda"), Ok(LobbyEvent::Joined(PlayerSlot::new(1))));

    assert_eq!(t.host.roster().len(), 2);
    assert_eq!(
        t.host.roster(),
        t.client(0).roster(),
        "the host's view is the only view, so both must match exactly"
    );
    let joiner = t.host.roster().get(PlayerSlot::new(1)).unwrap();
    assert_eq!(joiner.name, "Matilda");
    assert!(!joiner.ready, "a joiner is not ready until it says so");
    assert!(!joiner.is_host);
}

/// The test this module exists for.
#[test]
fn the_roster_is_slot_ordered_whatever_order_players_arrived_in() {
    let mut t = Table::new("Richard");
    // Deliberately backwards: slot 4 arrives first, then 3, then 1.
    t.join(1, hello(4), "Eleanor").unwrap();
    t.join(2, hello(3), "Geoffrey").unwrap();
    t.join(3, hello(1), "Matilda").unwrap();

    let slots = t.host.roster().slots();
    assert_eq!(
        slots,
        vec![
            PlayerSlot::new(0),
            PlayerSlot::new(1),
            PlayerSlot::new(3),
            PlayerSlot::new(4)
        ],
        "arrival order must not survive into the slot list"
    );
    let mut sorted = slots.clone();
    sorted.sort_by_key(|s| s.index());
    assert_eq!(slots, sorted);

    // And every client agrees, which is the property Session::new depends on.
    for i in 0..3 {
        assert_eq!(t.client(i).roster().slots(), slots);
    }
}

#[test]
fn an_incompatible_peer_is_refused_with_every_reason_at_once() {
    let mut t = Table::new("Richard");
    let mut bad = hello(1);
    bad.engine = "lords2 0.0.9-other".to_string();
    bad.ruleset_hash = 1;

    let refused = t.join(1, bad, "Matilda");
    match refused {
        Err(LobbyError::Incompatible(reasons)) => {
            assert!(
                reasons.len() >= 2,
                "one reason per reconnect is a bad experience: {reasons:?}"
            );
            assert!(reasons.iter().any(|m| matches!(m, Mismatch::Engine { .. })));
            assert!(reasons.iter().any(|m| matches!(m, Mismatch::Ruleset { .. })));
        }
        other => panic!("expected an incompatibility, got {other:?}"),
    }
    assert_eq!(t.host.roster().len(), 1, "a refused peer must leave no trace");
}

#[test]
fn a_peer_with_a_different_seed_cannot_join() {
    let mut t = Table::new("Richard");
    let mut bad = hello(1);
    bad.seed = SEED ^ 1;
    match t.join(1, bad, "Matilda") {
        Err(LobbyError::Incompatible(reasons)) => {
            assert!(reasons.iter().any(|m| matches!(m, Mismatch::Seed { .. })));
        }
        other => panic!("a different seed diverges before tick 0: {other:?}"),
    }
    assert_eq!(t.host.roster().len(), 1);
}

#[test]
fn two_players_cannot_share_a_name_or_a_slot() {
    let mut t = Table::new("Richard");
    t.join(1, hello(1), "Matilda").unwrap();

    assert!(matches!(
        t.join(2, hello(2), "Matilda"),
        Err(LobbyError::BadName(_))
    ));
    assert_eq!(t.host.roster().len(), 2);

    // A second claim on slot 1 is not refused - it is reseated, so two people
    // clicking join at the same moment both get in.
    assert_eq!(t.join(3, hello(1), "Eleanor"), Ok(LobbyEvent::Joined(PlayerSlot::new(2))));
    assert_eq!(t.host.roster().slots().len(), 3);
}

#[test]
fn a_full_game_refuses_the_next_arrival() {
    let mut t = Table::new("Richard");
    for i in 1..l2_net::MAX_PLAYERS as u8 {
        t.join(i as u32, hello(i), &format!("player{i}")).unwrap();
    }
    assert_eq!(t.host.roster().len(), l2_net::MAX_PLAYERS);
    assert_eq!(t.join(99, hello(0), "latecomer"), Err(LobbyError::Full));
    assert_eq!(t.host.roster().len(), l2_net::MAX_PLAYERS);
}

/// A joiner that claims the host's own slot is **reseated, not refused.**
///
/// `Hello::check` reports `SameSlot` because it is written for a direct
/// two-peer handshake with no authority to move anyone. A lobby has one. This
/// test pins the difference: the first version of the lobby forwarded that
/// mismatch straight through, so a client whose saved slot happened to be 0 was
/// told it was "incompatible" with a game it could play perfectly well — while
/// two clients colliding on slot 2 were reseated silently. Same situation,
/// opposite answer, decided by who opened the game.
#[test]
fn claiming_the_hosts_slot_reseats_rather_than_refuses() {
    let mut t = Table::new("Richard");
    assert_eq!(
        t.join(1, hello(0), "Matilda"),
        Ok(LobbyEvent::Joined(PlayerSlot::new(1))),
        "the host holds slot 0, so the joiner takes the next free one"
    );
    assert_eq!(t.host.roster().slots(), vec![PlayerSlot::new(0), PlayerSlot::new(1)]);
    assert_eq!(t.host.roster().get(PlayerSlot::new(0)).unwrap().name, "Richard");
    assert_eq!(t.host.roster().get(PlayerSlot::new(1)).unwrap().name, "Matilda");

    // The genuine incompatibilities must still refuse.
    let mut bad = hello(0);
    bad.ruleset_hash = 999;
    assert!(matches!(t.join(2, bad, "Eleanor"), Err(LobbyError::Incompatible(_))));
}

#[test]
fn readiness_travels_and_gates_the_start() {
    let mut t = Table::new("Richard");
    t.join(1, hello(1), "Matilda").unwrap();

    assert!(!t.host.roster().all_ready());
    assert!(t.host.start().is_err(), "starting on an unready lobby");

    t.clients[0].1.set_ready(true).unwrap();
    t.pump();

    assert!(t.host.roster().all_ready());
    assert!(
        t.client(0).roster().all_ready(),
        "the client must see its own readiness reflected back by the host"
    );

    let start = t.host.start().expect("everyone is ready");
    assert_eq!(start.seed, SEED);
    assert_eq!(start.roster.slots(), vec![PlayerSlot::new(0), PlayerSlot::new(1)]);

    t.pump();
    assert_eq!(t.client(0).started().map(|s| s.seed), Some(SEED));
    assert_eq!(t.client(0).started().unwrap().roster, start.roster);
}

#[test]
fn un_readying_takes_the_start_away_again() {
    let mut t = Table::new("Richard");
    t.join(1, hello(1), "Matilda").unwrap();
    t.clients[0].1.set_ready(true).unwrap();
    t.pump();
    assert!(t.host.roster().all_ready());

    t.clients[0].1.set_ready(false).unwrap();
    t.pump();
    assert!(!t.host.roster().all_ready());
    assert!(t.host.start().is_err());
}

#[test]
fn a_player_who_drops_leaves_the_roster() {
    let mut t = Table::new("Richard");
    t.join(1, hello(1), "Matilda").unwrap();
    t.join(2, hello(2), "Eleanor").unwrap();
    assert_eq!(t.host.roster().len(), 3);

    assert_eq!(t.host.peer_left(PeerId(1)), LobbyEvent::Left(PlayerSlot::new(1)));
    assert_eq!(t.host.roster().slots(), vec![PlayerSlot::new(0), PlayerSlot::new(2)]);
    assert_eq!(
        t.host.peer_left(PeerId(1)),
        LobbyEvent::Nothing,
        "leaving twice is not an event"
    );
}

/// A roster is the one message whose *order* is part of its meaning, so a peer
/// claiming an out-of-order one is refused rather than trusted.
#[test]
fn an_out_of_order_roster_is_rejected_by_the_decoder() {
    let good = Roster {
        players: vec![
            l2_net::Player {
                slot: PlayerSlot::new(0),
                name: "a".into(),
                ready: true,
                is_host: true,
            },
            l2_net::Player {
                slot: PlayerSlot::new(2),
                name: "b".into(),
                ready: true,
                is_host: false,
            },
        ],
    };
    let bytes = Canonical::bytes_of(&Message::Roster(good.clone()));
    assert_eq!(decode_all::<Message>(&bytes).unwrap(), Message::Roster(good.clone()));

    let mut backwards = good.clone();
    backwards.players.reverse();
    let bytes = Canonical::bytes_of(&Message::Roster(backwards));
    assert!(
        decode_all::<Message>(&bytes).is_err(),
        "a descending roster would give one machine a different slot list"
    );

    let mut duplicated = good;
    duplicated.players[1].slot = PlayerSlot::new(0);
    let bytes = Canonical::bytes_of(&Message::Roster(duplicated));
    assert!(decode_all::<Message>(&bytes).is_err(), "two players in one slot");
}

#[test]
fn every_lobby_message_round_trips() {
    let roster = Roster {
        players: vec![l2_net::Player {
            slot: PlayerSlot::new(0),
            name: "Richard".into(),
            ready: true,
            is_host: true,
        }],
    };
    let messages = vec![
        Message::Join(l2_net::Join { hello: hello(2), name: "Matilda".into() }),
        Message::Ready(true),
        Message::Ready(false),
        Message::Roster(roster.clone()),
        Message::Start(l2_net::Start { seed: SEED, roster }),
        Message::Refused(vec![
            Mismatch::Protocol { ours: 1, theirs: 2 },
            Mismatch::Engine { ours: "a".into(), theirs: "b".into() },
            Mismatch::Ruleset { ours: 7, theirs: 8 },
            Mismatch::Seed { ours: 9, theirs: 10 },
            Mismatch::SameSlot(PlayerSlot::new(3)),
        ]),
    ];
    for message in messages {
        let bytes = Canonical::bytes_of(&message);
        assert_eq!(decode_all::<Message>(&bytes).unwrap(), message, "round trip");
    }
}

/// The whole point, end to end: a lobby over a real socket produces a `Start`,
/// the `Start` produces two `Session`s, and the two sessions agree on every
/// tick. If the roster order were wrong this is where it would surface.
#[test]
fn a_lobby_over_a_real_socket_starts_a_session_that_agrees() {
    let mut host_net = TcpTransport::listen("127.0.0.1:0").expect("binding a loopback port");
    let addr = host_net.local_addr().expect("a bound address");
    let mut client_net = TcpTransport::connect(addr, PeerId(0)).expect("connecting");

    let mut joined = Vec::new();
    for _ in 0..10_000 {
        while host_net.poll().is_some() {}
        joined = host_net.take_joined();
        if !joined.is_empty() {
            break;
        }
    }
    let client_peer = joined[0];

    let mut host = Lobby::host(hello(0), "Richard").unwrap();
    let mut client = Lobby::join(hello(1), "Matilda").unwrap();
    client.greet().unwrap();
    client.set_ready(true).unwrap();

    let mut host_reader = FrameReader::new();
    let mut client_reader = FrameReader::new();

    // Run the lobby until the host is able to start, then start it.
    let mut started = None;
    for _ in 0..10_000 {
        for out in std::iter::from_fn(|| client.next_outgoing()).collect::<Vec<_>>() {
            let bytes = frame(&Canonical::bytes_of(&out.message)).unwrap();
            client_net.send(PeerId(0), &bytes).unwrap();
        }
        for out in std::iter::from_fn(|| host.next_outgoing()).collect::<Vec<_>>() {
            let bytes = frame(&Canonical::bytes_of(&out.message)).unwrap();
            host_net.send(client_peer, &bytes).unwrap();
        }
        while let Some((_, bytes)) = host_net.poll() {
            host_reader.feed(&bytes);
        }
        while let Some(m) = host_reader.next_message().unwrap() {
            host.receive(client_peer, decode_all::<Message>(&m).unwrap()).unwrap();
        }
        while let Some((_, bytes)) = client_net.poll() {
            client_reader.feed(&bytes);
        }
        while let Some(m) = client_reader.next_message().unwrap() {
            client.receive(PeerId(0), decode_all::<Message>(&m).unwrap()).unwrap();
        }

        if started.is_none() && host.roster().all_ready() {
            started = Some(host.start().unwrap());
        }
        if started.is_some() && client.started().is_some() {
            break;
        }
    }

    let start = started.expect("the host never reached a startable lobby");
    let client_start = client.started().expect("the client never saw the start").clone();
    assert_eq!(start, client_start, "the two sides must start the same game");

    // Hand the lobby's output to the sessions - this is the seam under test.
    let slots = start.roster.slots();
    assert_eq!(slots, vec![PlayerSlot::new(0), PlayerSlot::new(1)]);

    let mut sims = [ToySim::new(start.seed, 8), ToySim::new(start.seed, 8)];
    let mut sessions = [
        Session::new(Config::battle(), slots[0], &slots, start.seed, &sims[0]),
        Session::new(Config::battle(), slots[1], &slots, start.seed, &sims[1]),
    ];
    let mut hashes: [Vec<(Tick, u64)>; 2] = [Vec::new(), Vec::new()];
    let mut nets = [host_net, client_net];
    let mut readers = [host_reader, client_reader];
    let targets = [client_peer, PeerId(0)];

    for _ in 0..20_000 {
        if hashes.iter().all(|h| h.len() >= 40) {
            break;
        }
        for i in 0..2 {
            let at = sessions[i].execution_tick();
            if at.0 == 3 {
                sessions[i].issue(Order::Attack { unit: i as u32 }.payload());
            }
            let mut sealed = Vec::new();
            while let Some(packet) = sessions[i].next_packet() {
                sealed.push(frame(&Canonical::bytes_of(&Message::Tick(packet))).unwrap());
            }
            for bytes in sealed {
                nets[i].send(targets[i], &bytes).unwrap();
            }
        }
        for i in 0..2 {
            while let Some((_, bytes)) = nets[i].poll() {
                readers[i].feed(&bytes);
            }
            while let Some(m) = readers[i].next_message().unwrap() {
                match decode_all::<Message>(&m).unwrap() {
                    Message::Tick(p) => sessions[i].receive(p).unwrap(),
                    other => panic!("unexpected {other:?}"),
                }
            }
            while let Advance::Stepped { tick, hash } = sessions[i].advance(&mut sims[i]) {
                hashes[i].push((tick, hash));
            }
        }
    }

    let common = hashes[0].len().min(hashes[1].len());
    assert!(common >= 40, "only {common} ticks ran");
    assert_eq!(hashes[0][..common], hashes[1][..common], "the sessions diverged");
    assert!(sessions.iter().all(|s| !s.is_halted()));
    assert_eq!(sims[0].units, sims[1].units, "state differs despite equal hashes");
}

#![allow(unused_imports)]
use super::*;
use super::joins_and_roster::*;
use super::readiness::*;
use super::codec_and_transport::*;
use super::*;
use common::{Order, ToySim};
use l2_net::{
    decode_all, frame, Advance, Canonical, Config, FrameReader, Hello, Lobby, LobbyError,
    LobbyEvent, Message, Mismatch, PeerId, PlayerSlot, Role, Roster, Session, TcpTransport, Tick,
    Transport, PROTOCOL_VERSION,
};

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

/// A quirk changes what the simulation computes, so it is part of the agreed
/// configuration, not a local preference — `docs/netcode.md` D-12 with a
/// different noun, and `docs/decisions.md` C62.
#[test]
fn a_peer_with_a_different_quirk_set_is_refused_in_the_lobby() {
    let faithful = hello(1);
    let fixed = Hello { quirks: 0x3FFF, slot: PlayerSlot::new(2), ..hello(2) };

    let reasons = faithful.check(&fixed);
    assert_eq!(
        reasons,
        vec![Mismatch::Quirks { ours: 0, theirs: 0x3FFF }],
        "the quirk set is the only thing that differs, and it must be reported as itself"
    );

    let text = reasons[0].to_string();
    assert!(text.contains("bugs"), "{text}");
    assert!(!text.contains("mod set"), "{text}");

    let same = Hello { quirks: 0x3FFF, slot: PlayerSlot::new(2), ..hello(2) };
    let mine = Hello { quirks: 0x3FFF, ..hello(1) };
    assert_eq!(mine.check(&same), vec![]);
}

#[test]
fn the_quirk_set_round_trips_through_a_hello() {
    for bits in [0u64, 1, 0x3FFF, u64::MAX] {
        let h = Hello { quirks: bits, ..hello(1) };
        let mut c = l2_net::Canonical::recording();
        l2_net::canonical::Encode::encode(&h, &mut c);
        let bytes = c.finish().bytes.expect("recording");
        let mut r = l2_net::canonical::Reader::new(&bytes);
        let back = <Hello as l2_net::canonical::Decode>::decode(&mut r).expect("round trip");
        assert_eq!(back.quirks, bits);
        assert_eq!(back, h);
    }
}



//! The wire messages: round trips, the handshake, and malformed input.
//!
//! Round-tripping *byte-identically* is the requirement, not merely
//! decoding to an equal value. D-10 says a command must round-trip byte
//! for byte, and the reason is that a packet re-encoded differently is a
//! packet that would hash differently in a replay — so the tests below
//! encode, decode, and encode again.

use l2_net::{
    decode_all, Ack, Canonical, Command, HaltReason, Hello, Message, Mismatch, PlayerSlot,
    TickPacket, Tick, PROTOCOL_VERSION,
};

fn slot(n: u8) -> PlayerSlot {
    PlayerSlot::new(n)
}

fn sample_packet() -> TickPacket {
    TickPacket {
        tick: Tick(41),
        from: slot(1),
        commands: vec![
            Command::new(slot(1), 7, b"move 3 to 12,40".to_vec()),
            Command::new(slot(1), 8, Vec::new()),
        ],
        acks: vec![
            Ack { tick: Tick(38), state_hash: 0x0123_4567_89ab_cdef },
            Ack { tick: Tick(39), state_hash: 0xfedc_ba98_7654_3210 },
        ],
    }
}

fn round_trip<T>(value: &T) -> T
where
    T: l2_net::Encode + l2_net::Decode + PartialEq + core::fmt::Debug,
{
    let bytes = Canonical::bytes_of(value);
    let decoded: T = decode_all(&bytes).expect("decode");
    assert_eq!(&decoded, value, "value changed across a round trip");
    assert_eq!(Canonical::bytes_of(&decoded), bytes, "bytes changed across a round trip");
    decoded
}

#[test]
fn a_tick_packet_round_trips() {
    round_trip(&sample_packet());
}

#[test]
fn an_empty_tick_packet_round_trips() {
    round_trip(&TickPacket::empty(Tick(0), slot(0)));
}

#[test]
fn an_empty_packet_is_small() {
    let bytes = Canonical::bytes_of(&Message::Tick(TickPacket::empty(Tick(1), slot(0))));
    // kind + tick + from + command count + ack count.
    assert_eq!(bytes.len(), 1 + 4 + 1 + 4 + 4);
}

#[test]
fn a_packet_with_a_checksum_is_still_small() {
    let mut packet = TickPacket::empty(Tick(1), slot(0));
    packet.acks = vec![Ack { tick: Tick(0), state_hash: 1 }];
    let bytes = Canonical::bytes_of(&Message::Tick(packet));
    assert_eq!(bytes.len(), 1 + 4 + 1 + 4 + 4 + 4 + 8);
}

#[test]
fn a_command_round_trips() {
    round_trip(&Command::new(slot(4), u32::MAX, vec![0xff; 300]));
}

#[test]
fn every_message_kind_round_trips() {
    round_trip(&Message::Tick(sample_packet()));
    round_trip(&Message::Hello(sample_hello()));
    round_trip(&Message::Halt(HaltReason::Desync { tick: Tick(9) }));
    round_trip(&Message::Halt(HaltReason::Left));
    round_trip(&Message::Halt(HaltReason::Timeout));
    round_trip(&Message::Halt(HaltReason::Protocol { detail: "bad tag".to_string() }));
}

#[test]
fn an_unknown_message_kind_is_an_error() {
    assert!(decode_all::<Message>(&[99]).is_err());
}

#[test]
fn a_slot_beyond_the_player_limit_is_refused() {
    // Five players (0..4) is the limit; slot 5 does not exist.
    let mut bytes = Canonical::bytes_of(&sample_packet());
    bytes[4] = 5; // the `from` byte
    assert!(decode_all::<TickPacket>(&bytes).is_err());
}

#[test]
fn a_truncated_packet_is_an_error_not_a_panic() {
    let bytes = Canonical::bytes_of(&Message::Tick(sample_packet()));
    for cut in 0..bytes.len() {
        let _ = decode_all::<Message>(&bytes[..cut]);
    }
}

// --- the handshake ----------------------------------------------------

fn sample_hello() -> Hello {
    Hello {
        protocol: PROTOCOL_VERSION,
        engine: "l2 0.1.0-abc1234".to_string(),
        ruleset_hash: 0xfeed_face_cafe_beef,
        seed: 0x1234_5678_9abc_def0,
        slot: slot(0),
    }
}

#[test]
fn two_matching_peers_have_nothing_to_report() {
    let ours = sample_hello();
    let mut theirs = sample_hello();
    theirs.slot = slot(1);
    assert_eq!(ours.check(&theirs), vec![]);
}

#[test]
fn a_protocol_mismatch_stops_the_comparison() {
    let ours = sample_hello();
    let mut theirs = sample_hello();
    theirs.protocol = 99;
    theirs.engine = "something else".to_string();
    theirs.ruleset_hash = 1;
    let problems = ours.check(&theirs);
    assert_eq!(problems.len(), 1, "nothing after a version mismatch can be compared: {problems:?}");
    assert!(matches!(problems[0], Mismatch::Protocol { .. }));
}

/// D-12: the mod set is part of the version, and a mod mismatch is a
/// guaranteed desync with a confusing symptom. Reporting *every*
/// problem at once matters, because a mod list is exactly the sort of
/// thing that is wrong in three ways.
#[test]
fn every_mismatch_is_reported_at_once() {
    let ours = sample_hello();
    let mut theirs = sample_hello();
    theirs.engine = "l2 0.2.0-def5678".to_string();
    theirs.ruleset_hash = 7;
    theirs.seed = 8;
    let problems = ours.check(&theirs);
    assert_eq!(problems.len(), 4, "{problems:?}");
    assert!(problems.iter().any(|p| matches!(p, Mismatch::Engine { .. })));
    assert!(problems.iter().any(|p| matches!(p, Mismatch::Ruleset { .. })));
    assert!(problems.iter().any(|p| matches!(p, Mismatch::Seed { .. })));
    assert!(problems.iter().any(|p| matches!(p, Mismatch::SameSlot(_))));
}

#[test]
fn mismatches_explain_themselves() {
    let ours = sample_hello();
    let mut theirs = sample_hello();
    theirs.slot = slot(1);
    theirs.ruleset_hash = 0;
    let text = ours.check(&theirs)[0].to_string();
    assert!(text.contains("mod set differs"), "{text}");
    assert!(text.contains("feedfacecafebeef"), "{text}");
}

#[test]
fn halt_reasons_explain_themselves() {
    assert_eq!(
        HaltReason::Desync { tick: Tick(412) }.to_string(),
        "state checksums diverged at tick 412"
    );
    assert_eq!(HaltReason::Left.to_string(), "the player left");
}

// --- identities --------------------------------------------------------

#[test]
fn slots_are_bounded_by_the_player_limit() {
    assert!(PlayerSlot::from_wire(0).is_some());
    assert!(PlayerSlot::from_wire(4).is_some());
    assert!(PlayerSlot::from_wire(5).is_none());
    assert!(PlayerSlot::from_wire(255).is_none());
}

#[test]
#[should_panic(expected = "beyond the 5-player limit")]
fn constructing_an_impossible_slot_panics() {
    let _ = PlayerSlot::new(7);
}

#[test]
fn ticks_do_the_obvious_things() {
    assert_eq!(Tick(3).next(), Tick(4));
    assert_eq!(Tick(3).plus(2), Tick(5));
    assert_eq!(Tick(3).minus(2), Tick(1));
    assert_eq!(Tick(1).minus(5), Tick(0), "saturating, so the start of a session is safe");
    assert_eq!(Tick(7).to_string(), "tick 7");
}

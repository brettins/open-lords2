#![allow(unused_imports)]
use super::*;
use super::session_tests::*;
use super::flow_tests::*;
use common::{Order, ToySim};
use l2_net::{
    decode_all, frame, Advance, Canonical, Config, Endpoint, FrameReader, HaltReason, Loopback,
    Message, PeerId, PlayerSlot, Fixed, Session, SessionError, Tick, Transport,
};

#[test]
fn two_dumps_name_the_subsystem_that_diverged() {
    let mut table = Table::new(Config::battle(), 2, 7);
    table.peers[1].sim.bias = 1;
    table.peers[0].session.issue(Order::Attack { unit: 0 }.payload());
    table.run(12);
    assert!(table.peers.iter().all(|p| p.session.divergence().is_some() && !p.session.is_halted()));

    let ours = table.peers[0].session.dump(&table.peers[0].sim).expect("a dump");
    let theirs = table.peers[1].session.dump(&table.peers[1].sim).expect("a dump");

    assert_eq!(ours.author, PlayerSlot::new(0));
    assert!(!ours.state.is_empty(), "the diverged state is the one replaying cannot recover");
    assert_eq!(ours.replay.seed, theirs.replay.seed);

    let comparison = ours.compare(&theirs);
    assert!(comparison.same_seed);
    assert!(comparison.same_initial_state);
    assert!(
        comparison.same_command_stream,
        "the peers applied the same commands, so this is a simulation fault, not a network one"
    );
    assert_eq!(comparison.first_difference, Some(ours.divergence.tick));
    assert_eq!(
        comparison.sole_subsystem,
        Some("units"),
        "an off-by-one in a damage roll should localise to the unit array: {comparison}"
    );

    let bytes = Canonical::bytes_of(&ours);
    assert!(bytes.starts_with(b"L2DD"));
    assert!(bytes.len() > 100);
}

#[test]
fn a_generator_that_drifts_alone_is_visible_as_such() {
    let mut table = Table::new(Config::battle(), 2, 11);
    table.peers[1].sim.rng.next_u32();
    table.run(8);

    assert!(table.peers[0].session.divergence().is_some());
    let ours = table.peers[0].session.dump(&table.peers[0].sim).unwrap();
    let theirs = table.peers[1].session.dump(&table.peers[1].sim).unwrap();
    let comparison = ours.compare(&theirs);
    assert!(
        comparison.subsystems.contains(&"rng"),
        "the generator must be visible in the digest: {comparison}"
    );
}


/// C256. The host hears late, so it is the one that notices. Ablation: `plan_resync`
/// returning `None` (the peers never agree again).
#[test]
fn a_divergence_is_resynced_from_the_authority_and_play_goes_on() {
    let mut table = Table::new(Config::battle(), 2, 7);
    table.resync = true;
    table.net.set_latency(PeerId(0), 3);
    table.peers[1].sim.bias = 1;
    table.peers[0].session.issue(Order::Attack { unit: 0 }.payload());
    table.run(120);
    for p in &table.peers {
        assert!(!p.session.is_halted(), "{:?}", p.session.halt_reason());
        assert!(p.session.divergences() <= 1, "{} re-diverged", p.slot);
        assert!(p.session.divergence().is_none(), "{} still holds a divergence", p.slot);
        assert!(p.hashes.len() > 40, "{} stopped at {}", p.slot, p.hashes.len());
    }
    assert_eq!(table.peers[0].session.divergences(), 1, "the host noticed nothing");
    let (a, b) = (&table.peers[0].hashes, &table.peers[1].hashes);
    let tail: Vec<_> = a.iter().rev().take(20).collect();
    for (t, h) in tail {
        let theirs = b.iter().rev().find(|(u, _)| u == t).map(|(_, g)| g);
        assert_eq!(theirs, Some(h), "the peers disagree at {t} after the resync");
    }
    assert!(table.all_agree());
}

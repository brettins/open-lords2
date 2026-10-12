//! `docs/netcode.md` §6 *When it diverges*, C256: a slip on one seat is written out
//! by both, the host's world replaces the joiner's, and the game goes on in step.

use crate::live_tcp::{pair, pump, Peer};
use l2_net::{Canonical, Tick};

/// C256: a resync short of `move_cost` slips again at tick 3,155, measured; the map
/// test runs past it.
const AFTER_MAP: u32 = 3_600;
const AFTER_BATTLE: u32 = 300;

struct Run {
    slip: Tick,
    barrier: Tick,
    notice: Option<String>,
}

/// D-3's fault on the joiner once `when` holds: a draw outside any tick (C256).
fn slip_and_recover(peers: &mut [Peer; 2], after: u32, when: impl Fn(&Peer) -> bool) -> Run {
    let (mut slip, mut barrier, mut notice) = (None, None, None);
    for _ in 0..40_000 {
        pump(peers);
        for p in peers.iter() {
            assert!(!p.live.session.is_halted(), "{:?}", p.live.session.halt_reason());
            assert!(p.game.net.halted.is_none(), "{:?}", p.game.net.halted);
        }
        if slip.is_none() && when(&peers[1]) {
            let _ = peers[1].game.kingdom.rng.next_u32();
            slip = Some(peers[1].live.session.tick());
        }
        barrier = barrier.or(peers[1].live.resynced_at);
        notice = notice.or_else(|| peers[1].game.net.notice.clone().map(|(t, _)| t));
        let done = peers.iter().all(|p| p.live.resyncs > 0);
        let t = peers[0].live.session.tick().0;
        if done && barrier.is_some_and(|b| t > b.0 + after) || slip.is_some_and(|s| t > s.0 + after + 2_000) {
            break;
        }
    }
    let slip = slip.expect("the run reached the slip");
    Run { slip, barrier: barrier.expect("the joiner was never held for a resync"), notice }
}

fn check(peers: &[Peer; 2], run: &Run, dir: &std::path::Path) {
    let mut dumps = Vec::new();
    for p in peers {
        assert_eq!(p.live.resyncs, 1, "one slip, one resync");
        assert_eq!(p.live.session.divergences(), 1, "the resync did not hold");
        let (path, dump) = p.live.dump.clone().expect("each seat writes its half");
        assert!(path.starts_with(dir), "the dump went to {}", path.display());
        assert_eq!(std::fs::read(&path).expect("on disk"), Canonical::bytes_of(&dump));
        assert!(dump.divergence.tick >= run.slip, "diverged at {} before the slip", dump.divergence.tick);
        assert!(dump.divergence.tick <= run.barrier, "the dump was written after the resync");
        dumps.push(dump);
    }
    let cmp = dumps[0].compare(&dumps[1]);
    assert!(cmp.same_seed && cmp.same_initial_state, "{cmp}");
    assert_eq!(cmp.sole_subsystem, Some("rng"), "{cmp}");
    assert_eq!(run.notice.as_deref(), Some("Resynced with the host."));
    let (a, b) = (peers[0].live.session.history().hashes(), peers[1].live.session.history().hashes());
    let after: Vec<_> = a.iter().filter(|(t, _)| *t > run.barrier).filter_map(|(t, h)| {
        b.iter().find(|(u, _)| u == t).map(|(_, g)| (*t, *h, *g))
    }).collect();
    assert!(after.len() >= 200, "only {} ticks compared after the resync", after.len());
    for (t, h, g) in &after {
        assert_eq!(h, g, "the seats disagree at {t}, after the resync at {}", run.barrier);
    }
    eprintln!("slip at {}, resync at {}; {} ticks agree after; {cmp}", run.slip, run.barrier, after.len());
}

fn dump_to(peers: &mut [Peer; 2], tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("l2-desync-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    for p in peers.iter_mut() {
        p.live.dump_dir = Some(dir.clone());
    }
    dir
}

/// Ablations: `plan_resync`'s broadcast in `Live::diverged` (the seats never agree
/// again); `adopt` in `Live::resync` (the joiner keeps its world); `evidence` in
/// `Live::diverged` (no dumps); `move_cost` in `resync::adopt` (a second divergence).
#[test]
fn an_rng_slip_on_the_map_is_dumped_then_resynced_from_the_host() {
    let Some(mut peers) = pair() else { l2_testkit::skip!("no install") };
    let dir = dump_to(&mut peers, "map");
    let run = slip_and_recover(&mut peers, AFTER_MAP, |p| p.live.session.tick().0 >= 120);
    check(&peers, &run, &dir);
    let _ = std::fs::remove_dir_all(&dir);
}

/// C256 mid-battle. Ablation: `adopt` in `Live::resync` (the seats disagree after).
#[test]
fn an_rng_slip_in_a_live_battle_is_resynced_and_the_battle_goes_on() {
    let Some(mut peers) = pair() else { l2_testkit::skip!("no install") };
    let dir = dump_to(&mut peers, "battle");
    let run = slip_and_recover(&mut peers, AFTER_BATTLE, |p| p.fought > 20 && p.game.battle.is_some());
    check(&peers, &run, &dir);
    let _ = std::fs::remove_dir_all(&dir);
}

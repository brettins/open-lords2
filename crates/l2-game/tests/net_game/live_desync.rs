//! `docs/netcode.md` §6 *When it diverges*: a slip on one peer halts both, and
//! each writes a dump that names the subsystem.

use crate::live_tcp::{pair, pump};
use l2_net::{Canonical, HaltReason};

/// Ablations: `Message::Halt` in `Live::report` with `halted_by_peer`'s
/// divergence (the second peer writes no dump); `write_dump` (no files).
#[test]
fn an_rng_slip_on_one_peer_halts_both_and_writes_comparable_dumps() {
    let Some(mut peers) = pair() else { l2_testkit::skip!("no install") };
    let dir = std::env::temp_dir().join(format!("l2-desync-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    for p in peers.iter_mut() {
        p.live.dump_dir = Some(dir.clone());
    }
    let mut slipped = None;
    for _ in 0..4_000 {
        pump(&mut peers);
        let t = peers[1].live.session.tick();
        if slipped.is_none() && t.0 >= 120 {
            // A draw from the world's stream outside any tick: D-3's fault.
            let _ = peers[1].game.kingdom.rng.next_u32();
            slipped = Some(t);
        }
        if peers.iter().all(|p| p.live.session.is_halted() && p.game.net.halted.is_some()) {
            break;
        }
    }
    let slip = slipped.expect("the run reached the slip");
    let mut dumps = Vec::new();
    for p in &peers {
        let reason = p.live.session.halt_reason().cloned();
        assert!(matches!(reason, Some(HaltReason::Desync { .. })), "{reason:?}");
        let (path, dump) = p.live.dump.clone().expect("each peer writes its half");
        assert!(path.starts_with(&dir), "the dump went to {}", path.display());
        let on_disk = std::fs::read(&path).expect("the dump is on disk");
        assert_eq!(on_disk, Canonical::bytes_of(&dump));
        assert!(dump.divergence.tick >= slip, "diverged at {} before the slip", dump.divergence.tick);
        dumps.push(dump);
    }
    let cmp = dumps[0].compare(&dumps[1]);
    assert!(cmp.same_seed && cmp.same_initial_state, "{cmp}");
    assert_eq!(dumps[0].divergence.tick, dumps[1].divergence.tick);
    assert_eq!(cmp.sole_subsystem, Some("rng"), "{cmp}");
    eprintln!("slip at {slip}; {cmp}");
    let _ = std::fs::remove_dir_all(&dir);
}

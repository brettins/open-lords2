//! net-replay-hardening: a network game is recorded and replays bit-exact in a
//! fresh process; a dropped or silent peer halts the survivor with its name,
//! and the survivor's save goes on with the computer in that realm
//! (`docs/netcode.md` §8 *A dropped seat*, CNEW-dropped-seat).

use crate::live_tcp::{connect, pair, Peer};
use crate::seats::{assets, two_worlds};
use l2_game::command as cmd;
use l2_game::net::{Live, TIMEOUT};
use l2_net::{HaltReason, PlayerSlot};
use std::path::PathBuf;
use std::process::Command;

fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("l2-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

fn only(p: &mut Peer) {
    p.live.pump(&mut p.game);
}

/// Ablations: `hand_to_ai`'s `is_human = false`; `self.absent.extend(seats)` in
/// `Live::exchange` (the message says "A player").
#[test]
fn a_peer_that_leaves_halts_the_survivor_who_saves_with_the_ai_in_its_realm() {
    let Some(mut peers) = pair() else { l2_testkit::skip!("no install") };
    let dir = scratch("drop");
    for _ in 0..60 {
        peers.iter_mut().for_each(only);
    }
    assert!(peers[0].live.session.tick().0 > 20, "the pair never ran");
    peers[1].live.leave();
    let a = &mut peers[0];
    for _ in 0..2_000 {
        a.live.pump(&mut a.game);
        if a.game.net.halted.is_some() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert_eq!(a.live.session.halt_reason(), Some(&HaltReason::Left));
    let said = a.game.net.halted.clone().expect("the survivor is told");
    assert!(said.starts_with("Matilda left the game."), "{said}");
    assert!(!a.game.kingdom.realms[2].is_human, "realm 2 still waits for its player");
    assert!(a.game.kingdom.realms[1].is_human, "the survivor's own realm");
    let _scope = l2_game::saves::scoped_dir(&dir);
    let path = l2_game::saves::write("dropped", &a.game).expect("the survivor saves");
    let tables = a.game.kingdom.tables;
    let loaded = l2_game::saves::read_path(&path, tables).expect("the save loads");
    assert!(!loaded.multiplayer && !loaded.kingdom.realms[2].is_human);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Ablation: the `TIMEOUT` arm in `Live::advance` (the survivor waits forever).
#[test]
fn a_peer_silent_past_the_timeout_is_dropped() {
    let Some(mut peers) = pair() else { l2_testkit::skip!("no install") };
    for _ in 0..30 {
        peers.iter_mut().for_each(only);
    }
    for _ in 0..TIMEOUT + 50 {
        only(&mut peers[0]);
    }
    let a = &peers[0];
    assert_eq!(a.live.session.halt_reason(), Some(&HaltReason::Timeout));
    let said = a.game.net.halted.clone().expect("the survivor is told");
    assert!(said.starts_with("Matilda stopped answering."), "{said}");
}

/// §6 *The replay harness*. Ablations: `rebuild` given `Start` with another
/// seed (exit 1, starting state); the tampered checksum (exit 1).
#[test]
fn a_recorded_network_game_replays_bit_exact_in_a_fresh_process() {
    let Some(mut assets) = assets() else { l2_testkit::skip!("no install") };
    let dir = scratch("replay");
    let (ga, gb) = two_worlds(&mut assets);
    let (ta, tb) = connect();
    let mut peers: Vec<(l2_game::Game, Live)> = [(ga, ta), (gb, tb)]
        .into_iter()
        .enumerate()
        .map(|(i, (mut g, t))| {
            g.net.me = Some(PlayerSlot::new(i as u8));
            let mut live = Live::new(t, &mut g, 0x5eed).expect("a seat");
            live.record_to(dir.clone());
            (g, live)
        })
        .collect();
    for n in 0..3_000u32 {
        for (i, (g, live)) in peers.iter_mut().enumerate() {
            if n == 10 + 40 * i as u32 {
                let k = &g.kingdom;
                let mine = k.county_ids().find(|&c| k.counties[c].owner == g.player).unwrap();
                g.issue(cmd::SetTaxRate { county: mine as u8, rate: 5 + i as i32 });
            }
            live.pump(g);
        }
        if peers[0].1.session.tick().0 >= 400 {
            break;
        }
    }
    let written: Vec<PathBuf> = peers
        .iter_mut()
        .map(|(_, live)| live.write_replay().expect("recording").expect("written"))
        .collect();
    let install = l2_testkit::install_dir().expect("the install");
    let run = |file: &PathBuf| {
        let out = Command::new(env!("CARGO_BIN_EXE_l2-game"))
            .arg(&install)
            .args(["--no-sound", "--verify-replay"])
            .arg(file)
            .output()
            .expect("the game runs");
        (out.status.code(), String::from_utf8_lossy(&out.stdout).to_string() + &String::from_utf8_lossy(&out.stderr))
    };
    for file in &written {
        let (code, said) = run(file);
        assert_eq!(code, Some(0), "{}: {said}", file.display());
        assert!(said.contains("bit-exact"), "{said}");
    }
    let mut bytes = std::fs::read(&written[0]).unwrap();
    let last = bytes.len() - 1;
    bytes[last] ^= 0x40;
    let tampered = dir.join("tampered.l2nr");
    std::fs::write(&tampered, bytes).unwrap();
    let (code, said) = run(&tampered);
    assert_eq!(code, Some(1), "a tampered checksum verified: {said}");
    drop(peers);
    let _ = std::fs::remove_dir_all(&dir);
}

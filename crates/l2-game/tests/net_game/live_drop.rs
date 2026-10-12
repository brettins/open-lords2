//! net-replay-hardening: a network game is recorded and replays bit-exact in a
//! fresh process. A joiner that loses its host stops, and its save goes on with the
//! computer in the other realms (C240); a silent joiner is waited for (C260).

use crate::live_tcp::{connect, pair, Peer};
use crate::seats::{assets, two_worlds};
use l2_game::command as cmd;
use l2_game::net::{Live, QUIET, STALL_GRACE, TIMEOUT};
use l2_game::screen::{Ctx, Machine, ScreenId};
use l2_game::screens::net_wait::NetWaitScreen;
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

/// Ablations: `hand_to_ai`'s `is_human = false`; the joiner's `HaltReason::Left` in
/// `Live::dropped` (it waits for a host that is gone).
#[test]
fn a_joiner_that_loses_its_host_stops_and_saves_with_the_ai_in_its_realm() {
    let Some(mut peers) = pair() else { l2_testkit::skip!("no install") };
    let dir = scratch("drop");
    for _ in 0..60 {
        peers.iter_mut().for_each(only);
    }
    assert!(peers[0].live.session.tick().0 > 20, "the pair never ran");
    let [host, mut b] = peers;
    drop(host);
    for _ in 0..2_000 {
        b.live.pump(&mut b.game);
        if b.game.net.halted.is_some() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert_eq!(b.live.session.halt_reason(), Some(&HaltReason::Left));
    let said = b.game.net.halted.clone().expect("the joiner is told");
    assert!(said.starts_with("The connection to Richard's game was lost."), "{said}");
    assert!(!b.game.kingdom.realms[1].is_human, "realm 1 still waits for its player");
    assert!(b.game.kingdom.realms[2].is_human, "the joiner's own realm");
    let _scope = l2_game::saves::scoped_dir(&dir);
    let path = l2_game::saves::write("dropped", &b.game).expect("the joiner saves");
    let tables = b.game.kingdom.tables;
    let loaded = l2_game::saves::read_path(&path, tables).expect("the save loads");
    assert!(!loaded.multiplayer && !loaded.kingdom.realms[1].is_human);
    let _ = std::fs::remove_dir_all(&dir);
}

/// C260. Ablation: `self.away(slot)` in `Live::silent` (the host stalls with no box).
#[test]
fn a_joiner_silent_past_the_timeout_is_waited_for() {
    let Some(mut peers) = pair() else { l2_testkit::skip!("no install") };
    for _ in 0..30 {
        peers.iter_mut().for_each(only);
    }
    for _ in 0..TIMEOUT + 50 {
        only(&mut peers[0]);
    }
    let a = &peers[0];
    assert_eq!(a.live.session.halt_reason(), None);
    assert_eq!(a.game.net.waiting, vec!["Matilda".to_string()]);
    assert!(!a.game.net.stalled, "the stall glyph drawn over the waiting box");
}

/// C262. Ablations: the `self.quiet = …` assignment in `Live::silent` (only the stall
/// glyph until `TIMEOUT`); `self.quiet.clear()` on `Advance::Stepped` (the box stays).
#[test]
fn a_joiner_silent_a_few_seconds_is_named_in_the_waiting_box_and_goes_when_it_answers() {
    let Some(mut peers) = pair() else { l2_testkit::skip!("no install") };
    let assets = assets().unwrap();
    for _ in 0..30 {
        peers.iter_mut().for_each(only);
    }
    for _ in 0..STALL_GRACE + 30 {
        only(&mut peers[0]);
    }
    assert!(peers[0].game.net.stalled && peers[0].game.net.quiet.is_empty(), "named before QUIET");
    for _ in 0..QUIET {
        only(&mut peers[0]);
    }
    let a = &mut peers[0];
    assert_eq!(a.game.net.quiet, vec!["Matilda".to_string()]);
    assert!(a.game.net.waiting.is_empty() && !a.game.net.stalled && a.live.session.halt_reason().is_none());
    let mut m = Machine::new(ScreenId::Campaign);
    let mut ctx = Ctx { game: &mut a.game, assets: &assets };
    m.update(&mut ctx);
    assert_eq!(m.top_id(), Some(ScreenId::NetWait), "the host sees only the glyph");
    assert_eq!(NetWaitScreen::heading(&ctx), "Waiting for Matilda");
    let said = NetWaitScreen::text(&ctx);
    assert!(said.starts_with("Nothing has arrived from Matilda for a few seconds."), "{said}");
    for _ in 0..200 {
        peers.iter_mut().for_each(only);
    }
    let a = &mut peers[0];
    assert!(a.game.net.quiet.is_empty(), "{:?}", a.game.net.quiet);
    let mut ctx = Ctx { game: &mut a.game, assets: &assets };
    m.update(&mut ctx);
    assert!(!m.ids().contains(&ScreenId::NetWait), "the box outlived the silence");
}

/// C262: a joiner names its silent host alone, whose forwarding stopped too.
#[test]
fn a_joiner_names_its_silent_host() {
    let Some(mut peers) = pair() else { l2_testkit::skip!("no install") };
    for _ in 0..30 {
        peers.iter_mut().for_each(only);
    }
    for _ in 0..QUIET + 50 {
        only(&mut peers[1]);
    }
    assert_eq!(peers[1].game.net.quiet, vec!["Richard".to_string()]);
}

/// Ablation: the joiner's `HaltReason::Timeout` in `Live::silent` (it waits forever).
#[test]
fn a_host_silent_past_the_timeout_stops_the_joiner() {
    let Some(mut peers) = pair() else { l2_testkit::skip!("no install") };
    for _ in 0..30 {
        peers.iter_mut().for_each(only);
    }
    for _ in 0..TIMEOUT + 50 {
        only(&mut peers[1]);
    }
    let b = &peers[1];
    assert_eq!(b.live.session.halt_reason(), Some(&HaltReason::Timeout));
    let said = b.game.net.halted.clone().expect("the joiner is told");
    assert!(said.starts_with("The connection to Richard's game was lost."), "{said}");
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

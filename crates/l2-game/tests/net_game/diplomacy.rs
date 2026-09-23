//! Letters that change the world, over TCP on 127.0.0.1: opcodes 0x48, 0x4A, 0x4B
//! as commands (`docs/netcode.md` §5).

use crate::live_tcp::{pair, Peer};
use l2_game::command::{PostLetter, SendLetter};

fn pump_until(peers: &mut [Peer; 2], done: impl Fn(&Peer) -> bool) {
    for _ in 0..4_000 {
        peers.iter_mut().for_each(|p| p.live.pump(&mut p.game));
        if peers.iter().all(&done) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    for _ in 0..20 {
        peers.iter_mut().for_each(|p| p.live.pump(&mut p.game));
    }
}

fn humans(peers: &mut [Peer; 2]) -> (u8, u8) {
    let (from, to) = (peers[0].game.player, peers[1].game.player);
    for p in peers.iter_mut() {
        let realms = &mut p.game.kingdom.realms;
        realms[from as usize].is_human = true;
        realms[to as usize].is_human = true;
        realms[from as usize].gold = 1_000;
        realms[to as usize].gold = 500;
    }
    (from, to)
}

fn same_hashes(peers: &[Peer; 2]) {
    let (ra, rb) = (peers[0].live.session.replay().unwrap(), peers[1].live.session.replay().unwrap());
    let n = ra.hashes.len().min(rb.hashes.len());
    assert!(n > 0, "no tick ran");
    assert_eq!(ra.hashes[..n], rb.hashes[..n], "the peers' hashes differ");
}

/// Ablation: `ctx.issue(PostLetter { .. })` in `ComposeScreen::send` (the screens
/// test), `game.kingdom.post_letter` in `PostLetter::run`.
#[test]
fn a_gift_to_an_ai_lord_moves_the_gold_on_every_world_alike() {
    let Some(mut peers) = pair() else { l2_testkit::skip!("no install") };
    let (from, to) = humans(&mut peers);
    let realms = &peers[0].game.kingdom.realms;
    let ai = (1..realms.len() as u8)
        .find(|&r| r != from && r != to && realms[r as usize].in_play)
        .expect("an AI lord in play");
    let before = realms[ai as usize].gold;
    peers[0].game.issue(PostLetter { to: ai, kind: 0, gold: 100, county: 0 });
    pump_until(&mut peers, |p| p.game.kingdom.realms[from as usize].gold == 900);
    for p in &peers {
        assert_eq!(p.game.kingdom.realms[from as usize].gold, 900);
        assert_eq!(p.game.kingdom.realms[ai as usize].gold, before + 100);
    }
    same_hashes(&peers);
}

/// Ablation: the `0 =>` arm after `enqueue` in `SendLetter::run`; `gift_reply`'s
/// `best_gift = gold`; `variant: self.gold` (a byte clamps 300).
#[test]
fn a_gift_between_seats_lands_when_its_addressee_replies() {
    let Some(mut peers) = pair() else { l2_testkit::skip!("no install") };
    let (from, to) = humans(&mut peers);
    peers[0].game.issue(SendLetter { to, kind: 0, gold: 300, county: 0, text: String::new() });
    pump_until(&mut peers, |p| p.game.kingdom.realms[to as usize].gold == 800);
    for p in &peers {
        let realms = &p.game.kingdom.realms;
        assert_eq!((realms[from as usize].gold, realms[to as usize].gold), (700, 800));
        assert_eq!(realms[to as usize].pair(from).best_gift, 300);
    }
    let q = &peers[1].game.messages;
    let mut all = q.waiting();
    all.extend(q.open().copied());
    let gift = all.iter().find(|r| r.group == 0xF5).expect("the addressee's ring has the gift");
    assert_eq!(gift.variant, 300, "`+0x0C` is an int");
    same_hashes(&peers);
}

/// Ablation: the `4 =>` arm after `enqueue` in `SendLetter::run`.
#[test]
fn an_alliance_between_seats_ends_when_its_letter_is_read() {
    let Some(mut peers) = pair() else { l2_testkit::skip!("no install") };
    let (from, to) = humans(&mut peers);
    for p in peers.iter_mut() {
        l2_kingdom::diplomacy::form_alliance(&mut p.game.kingdom.realms, from, to);
        assert_eq!(p.game.kingdom.realms[from as usize].ally, to);
    }
    peers[0].game.issue(SendLetter { to, kind: 4, gold: 0, county: 0, text: "Farewell".into() });
    pump_until(&mut peers, |p| p.game.kingdom.realms[to as usize].ally == 0);
    for p in &peers {
        let realms = &p.game.kingdom.realms;
        assert_eq!((realms[from as usize].ally, realms[to as usize].ally), (0, 0));
        assert!(!realms[to as usize].pair(from).allied);
    }
    same_hashes(&peers);
}

//! `docs/netcode.md` §5 *A letter between seats* (CNEW-letter-in-command): a
//! compliment over real TCP on 127.0.0.1 rings the addressee only.

use crate::live_tcp::pair;
use l2_game::command::SendLetter;
use l2_game::message::{category, Record};
use l2_game::Game;

const TEXT: &str = "Thy barley is the envy of Wessex";

fn compliment(g: &Game) -> Option<Record> {
    let mut all = g.messages.waiting();
    all.extend(g.messages.open().copied());
    all.into_iter().find(|r| r.group == 0xF6 && r.category == category::DIPLOMACY)
}

/// Ablation: the `to` test in `MessageQueue::enqueue`, `game.local_realm()` in
/// `SendLetter::run`, or `letters[from] = self.text`.
#[test]
fn a_letter_rings_the_addressee_only_and_every_peer_holds_its_text() {
    let Some(mut peers) = pair() else { l2_testkit::skip!("no install") };
    let (from, to) = (peers[0].game.player, peers[1].game.player);
    assert_ne!(from, to, "one realm a seat");
    for p in peers.iter_mut() {
        p.game.kingdom.realms[from as usize].is_human = true;
        p.game.kingdom.realms[to as usize].is_human = true;
    }
    peers[0].game.issue(SendLetter { to, kind: 1, gold: 0, county: 0, text: TEXT.into() });
    for _ in 0..4_000 {
        peers.iter_mut().for_each(|p| p.live.pump(&mut p.game));
        if peers.iter().all(|p| p.game.messages.letters.get(from as usize).is_some_and(|t| t == TEXT)) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    for p in &peers {
        assert_eq!(p.game.messages.letters.get(from as usize).map(String::as_str), Some(TEXT));
    }
    let got = compliment(&peers[1].game).expect("the addressee's ring has the letter");
    assert_eq!((got.from, got.to), (from, to));
    assert!(compliment(&peers[0].game).is_none(), "the sender's ring stays empty");
}

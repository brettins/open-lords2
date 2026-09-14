#![allow(unused_imports)]
use super::*;
use super::standing::*;
use super::replies::*;
use super::ai::*;
use super::tests::*;
use crate::realm::{Realm, MAX_REALMS};
use crate::tables::Tables;
use l2_net::Pcg32;

/// `Diplo_Post` (`0x004A2621`) — **the only writer of the inbox**, and the
/// player's whole outgoing side in single player.
///
/// It fills the first free of five slots, bumps the recipient's
/// `compliments_from` when the kind is a compliment, sets `has_mail`, and — if
/// the gold is non-zero — **moves it immediately**, clamped to what the sender
/// holds.
///
/// **A gift is spent when it is posted, not when it is answered.** The reply
/// arrives a turn later and could be an insult; the money has gone either way.
///
/// A full inbox silently drops the letter, which is the original's behaviour
/// and not a softening of it: the loop returns when it runs off the fifth slot.
pub fn post(
    realms: &mut [Realm],
    diplomacy: &mut Diplomacy,
    from: u8,
    to: u8,
    kind: Kind,
    gold: i32,
    county: u8,
) {
    let to_i = (to as usize).min(MAX_REALMS - 1);
    let Some(slot) = diplomacy.inbox[to_i].iter_mut().find(|s| s.from == 0) else { return };
    slot.from = from;
    slot.kind = kind.byte();
    slot.county = county;
    slot.gold = gold;
    if kind == Kind::Compliment {
        let p = realms[to_i].pair_mut(from);
        p.compliments_from = p.compliments_from.wrapping_add(1);
    }
    realms[to_i].pair_mut(from).has_mail = true;
    if gold == 0 {
        return;
    }
    let gold = gold.min(realms[from as usize].gold);
    realms[from as usize].gold -= gold;
    realms[to_i].gold += gold;
}

/// **AI turn step 1** — `Diplo_AnswerInbox` (`0x004A277D`).
///
/// Walks the five slots, dispatches on the kind byte, then zeroes all five
/// slots *and* all six `has_mail` bytes. **A realm's inbox is emptied every
/// turn whether or not it was full**, and a letter from the realm itself is
/// skipped without being answered.
///
/// Because a human realm's AI turn is skipped entirely (`realm +0x05`), a
/// letter posted to a person's inbox would never be answered. In single player
/// nothing posts one; in multiplayer the send goes through the netcode instead.
pub fn answer_inbox(
    realms: &mut [Realm],
    diplomacy: &mut Diplomacy,
    tables: &Tables,
    realm: u8,
) -> Vec<Letter> {
    let mut out = Vec::new();
    let me = (realm as usize).min(MAX_REALMS - 1);
    for slot in 0..INBOX_SLOTS {
        let entry = diplomacy.inbox[me][slot];
        if entry.from == 0 {
            break;
        }
        if entry.from == realm {
            continue;
        }
        let Some(kind) = Kind::from_byte(entry.kind) else { continue };
        out.extend(match kind {
            Kind::Gift => reply_gift(realms, tables, realm, entry.from, entry.gold),
            Kind::Compliment => reply_compliment(realms, realm, entry.from),
            Kind::Insult => reply_insult(realms, realm, entry.from),
            Kind::OfferAlliance => {
                reply_alliance_offer(realms, diplomacy, realm, entry.from, realms_active(realms))
            }
            Kind::EndAlliance => reply_alliance_end(realms, realm, entry.from),
            Kind::AskHelp => {
                reply_help_request(realms, diplomacy, tables, realm, entry.from, entry.county)
            }
            Kind::AskAttack => {
                reply_attack_request(realms, diplomacy, tables, realm, entry.from, entry.county)
            }
        });
    }
    diplomacy.inbox[me] = [InboxSlot::default(); INBOX_SLOTS];
    for other in 0..MAX_REALMS {
        realms[me].pair_mut(other as u8).has_mail = false;
    }
    out
}

/// `g_realmsActive` (`0x00554004`) as *"realms still in play"*.
///
/// **The original has two writers with two meanings** — one counts realms still
/// in play and one counts realms that have not yet finished their turn, and the
/// second runs every frame of phase 4, so whichever wrote last is what the
/// diplomacy code reads. `docs/diplomacy.md` §9 leaves it unresolved and
/// nothing here depends on resolving it: both readings mean *"the game is
/// nearly over"* at `< 3`, and both branches that test it refuse either way.
pub fn realms_active(realms: &[Realm]) -> usize {
    realms.iter().skip(1).take(MAX_REALMS - 1).filter(|r| r.strength != 0).count()
}

// ----------------------------------------------- §8 the seven reply handlers


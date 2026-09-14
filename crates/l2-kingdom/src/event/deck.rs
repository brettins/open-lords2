#![allow(unused_imports)]
use super::*;
use super::effects::*;
use super::tests::*;
use crate::county::County;
use crate::report::Message;
use crate::tables::{Season, Tables};
use l2_net::{Pcg32, Quirk, Quirks};

impl EventKind {
    /// The `L2.eng` group that names and describes this event, which is the
    /// same number as the deck stores.
    pub fn id(self) -> u16 {
        self as u16
    }

    pub fn from_id(id: u16) -> Option<EventKind> {
        EVENT_KINDS.iter().copied().find(|k| k.id() == id)
    }

    /// `L2.eng` group index 0 — the headline the message box shows.
    pub fn name(self) -> &'static str {
        match self {
            EventKind::Rats => "Rats!!",
            EventKind::MadCows => "Mad Cows !!",
            EventKind::Wolves => "Wolves.",
            EventKind::Plague => "Plague.",
            EventKind::GrainFound => "Grain found.",
            EventKind::BadCattle => "Bad cattle stock",
            EventKind::CowBonanza => "Cow bonanza!!",
            EventKind::WeddingFever => "Wedding fever.",
            EventKind::HealthyEating => "Healthy eating.",
            EventKind::MotherNature => "Mother nature.",
            EventKind::WeaponsFound => "Weapons found.",
            EventKind::Donation => "Donation.",
            EventKind::Treasure => "Treasure",
            EventKind::HolyRelic => "Holy Relic.",
            EventKind::Witch => "Witch !!",
            EventKind::StoneFound => "Stone found.",
            EventKind::Locusts => "Pests.",
            EventKind::HagsCurse => "Hags curse.",
            EventKind::Fraud => "Fraud.",
            EventKind::Corruption => "Corruption.",
            EventKind::NoBull => "No bull.",
            EventKind::StopThief => "Stop thief!.",
            EventKind::Termites => "Pests.",
            EventKind::NoSongs => "No songs.",
        }
    }
}

/// Whether a county is eligible to draw at all. **The AI never draws random
/// events**, and nothing is drawn until the year passes 1268.
pub fn eligible(t: &Tables, county: &County, owner_is_human: bool, year: i32) -> bool {
    year > t.event.first_year && !county.is_unowned() && owner_is_human
}

/// `Event_RollAll` (`0x00448819`) — clear last season's modifiers, then walk the
/// deck.
///
/// **It clears three modifiers and the tax gate, and not the letter's latch.**
/// See the loop body: `eventFired` and `eventId` survive a season in which the
/// county drew nothing, which is the whole reason a letter can still be read
/// several seasons after the event that wrote it. `l2_game::message::post_event`
/// is the only thing that clears `eventFired`.
///
/// `purses` is indexed by realm id and is only read for the seven events that
/// touch a treasury or a stockpile; the caller copies the values back.
///
/// **One draw per season, before the county loop**, which is the determinism
/// property that matters: the number of values taken from the generator does
/// not depend on how many counties there are or who owns them
/// (`docs/netcode.md` §3).
#[allow(clippy::too_many_arguments)]
pub fn roll_all(
    t: &Tables,
    counties: &mut [County],
    county_count: usize,
    human_owner: &dyn Fn(u8) -> bool,
    purses: &mut [RealmPurse],
    year: i32,
    season: Season,
    rng: &mut Pcg32,
    quirks: Quirks,
    out: &mut Vec<Message>,
) {
    // **Switchable** — [`Quirk::EventDeckParityLocksOutEvenCounties`],
    // `docs/bugs.md` B2. `00448822 MOV EAX,[0x0058FD60] / ADD EAX,EAX`: the
    // doubling is what makes the starting slot always even, and every filled
    // slot is odd, so county `k` can only ever land on a slot of parity `k`.
    //
    // The fix draws the starting slot over the whole deck
    // of it doubled. **One `next_u32()` either way** — both bounds are powers
    // of two, so `Pcg32::below`'s rejection loop never turns, and the number of
    // values taken from the generator does not depend on the setting. That
    // property is not decoration: a quirk that changed how often the simulation
// drew would desync a peer at the *next* draw, and
    // the desync dump would name the wrong subsystem (`docs/netcode.md` §3).
    let mut index = if quirks.reproduces(Quirk::EventDeckParityLocksOutEvenCounties) {
        (rng.below(EVENT_SEED_BOUND) * 2) as usize
    } else {
        rng.below(EVENT_DECK_SLOTS as u32) as usize
    };
    for id in 1..=county_count {
        // **Four writes, and `eventFired`/`eventId` are not among them.** `[V]`
        // — `Event_RollAll`'s loop head is exactly
        // `eventPopulationPct = 0; eventGrainPct = 0; eventHerdPct = 0;
        //  taxSuppressed = 0;` and nothing else. The three modifiers must go
        // every season or they would be applied twice; the *letter's* latch must
        // not, because the only thing that clears it is the letter being posted
        // — `Event_Post` (`0x00448D7E`), once a frame, for the **selected**
        // county. A county whose event nobody has looked at keeps its flag and
        // its id for as long as it takes.
        //
        // Ours cleared both here, every season, so the latch was gone before any
        // frame could read it and no event letter could ever reach a player.
        // `docs/decisions.md` C210.
        counties[id].event_population_pct = 0;
        counties[id].event_grain_pct = 0;
        counties[id].event_herd_pct = 0;
        counties[id].tax_suppressed = false;

        index += 1;
        if index > EVENT_DECK_SLOTS - 1 {
            index = 0;
        }
        let Some(kind) = deck_slot(index) else { continue };
        if !eligible(t, &counties[id], human_owner(counties[id].owner), year) {
            continue;
        }
        let owner = counties[id].owner as usize;
        let mut purse = purses.get(owner).copied().unwrap_or_default();
        if fire(&mut counties[id], id, &mut purse, kind, season, quirks) {
            if let Some(slot) = purses.get_mut(owner) {
                *slot = purse;
            }
            out.push(Message::Event { county: id as u8, kind });
        }
    }
}


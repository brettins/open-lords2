//! Random events — `docs/kingdom.md` §8.1, `Event_RollAll` (`0x00448819`).
//!
//! What the document establishes is the **mechanism**, and that is what is
//! implemented here:
//!
//! * events are drawn for **human-owned counties only** — *"the AI never draws
//!   random events"* — and only once the year passes 1268;
//! * most handlers write a percentage into county `+0x1FB`, `+0x1FC` or
//!   `+0x1FD`, which [`crate::population`], [`crate::land::grain_season_tick`]
//!   and [`crate::land::herd_season_tick`] then apply;
//! * the population modifier is capped at 20% of the county.
//!
//! **What it does not establish is the table.** `g_eventTable` (`0x004D6108`)
//! has 24 handlers (ids `0x87` … `0x8E` and `0x12E` … `0x13D`) and
//! `docs/kingdom.md` lists none of their contents, nor the draw's frequency.
//! [`EVENT_TABLE`] and [`EVENT_CHANCE_IN`] below are therefore **ours**: a
//! placeholder catalogue built from the four effects the document quotes by
//! name, so that the mechanism can be exercised and tested. They are not a
//! reproduction of the original's table and are marked as such at every use.

use crate::county::County;
use crate::report::Message;
use crate::tables::EVENT_FIRST_YEAR;
use l2_net::Pcg32;

/// A random event's effect.
///
/// The five below are the ones `docs/kingdom.md` names — from `L2.eng`
/// group 77 indices 19-26 (*"eaten by rats"*, *"taken by wolves"*, *"found as
/// surplus"*) and the message text it quotes (*"Vermin have been discovered in
/// the county's tithe barns"*, *"Wolves are abroad in the county"*, *"The Black
/// Death is spreading across the county"*). The other nineteen handlers are
/// unidentified.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EventKind {
    /// *"Vermin have been discovered in the county's tithe barns"* — grain
    /// eaten by rats.
    Vermin,
    /// *"Wolves are abroad in the county"* — livestock taken by wolves.
    Wolves,
    /// Grain *"found as surplus"*.
    Surplus,
    /// *"The Black Death is spreading across the county"*.
    BlackDeath,
    /// A good year for the herd.
    GoodCalving,
}

impl EventKind {
    /// `(population%, grain%, herd%)` — the three modifier fields the handler
    /// writes. **Ours, not the original's**: see the module documentation.
    pub fn modifiers(self) -> (i32, i32, i32) {
        match self {
            EventKind::Vermin => (0, -25, 0),
            EventKind::Wolves => (0, 0, -20),
            EventKind::Surplus => (0, 20, 0),
            EventKind::BlackDeath => (-30, 0, 0),
            EventKind::GoodCalving => (0, 0, 15),
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            EventKind::Vermin => "Vermin",
            EventKind::Wolves => "Wolves",
            EventKind::Surplus => "Surplus",
            EventKind::BlackDeath => "Black Death",
            EventKind::GoodCalving => "Good calving",
        }
    }
}

/// **Ours.** The original's 24-entry `g_eventTable` is not documented.
pub const EVENT_TABLE: [EventKind; 5] = [
    EventKind::Vermin,
    EventKind::Wolves,
    EventKind::Surplus,
    EventKind::BlackDeath,
    EventKind::GoodCalving,
];

/// **Ours.** `docs/kingdom.md` §8.1 says an event is *drawn* per county per
/// season and does not give the frequency.
pub const EVENT_CHANCE_IN: u32 = 8;

/// Whether a county is eligible to draw at all. The gate is the part that *is*
/// documented, and it is the reason a human player and an AI do not play the
/// same game.
pub fn eligible(county: &County, owner_is_human: bool, year: i32) -> bool {
    year > EVENT_FIRST_YEAR && !county.is_unowned() && owner_is_human
}

/// Write an event's modifiers onto a county. The modifiers are consumed by the
/// population, grain and herd passes later in the same season.
pub fn fire(county: &mut County, kind: EventKind) {
    let (population, grain, herd) = kind.modifiers();
    county.event_population_pct = population;
    county.event_grain_pct = grain;
    county.event_herd_pct = herd;
    county.event_fired = true;
}

/// `Event_RollAll` — one draw per eligible county, in county order.
///
/// The draw is made for **every** county, eligible or not, so that the value
/// stream does not depend on who owns what. That is a determinism requirement
/// (`docs/netcode.md` §3) rather than something the document states, and it is
/// the same reasoning `l2_net::Pcg32::chance` documents for always drawing even
/// at probability zero.
pub fn roll_all(
    counties: &mut [County],
    county_count: usize,
    human_owner: &dyn Fn(u8) -> bool,
    year: i32,
    rng: &mut Pcg32,
    out: &mut Vec<Message>,
) {
    for id in 1..=county_count {
        counties[id].event_fired = false;
        let drew = rng.one_in(EVENT_CHANCE_IN);
        let pick = rng.index(EVENT_TABLE.len()).expect("the event table is never empty");
        if !drew || !eligible(&counties[id], human_owner(counties[id].owner), year) {
            continue;
        }
        let kind = EVENT_TABLE[pick];
        fire(&mut counties[id], kind);
        out.push(Message::Event { county: id as u8, kind });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **The AI never draws random events**, and nothing happens before the
    /// year passes 1268.
    #[test]
    fn only_a_human_county_after_1268_is_eligible() {
        let mut c = County::new();
        c.owner = 1;
        assert!(!eligible(&c, true, 1268), "the first year is excluded");
        assert!(eligible(&c, true, 1269));
        assert!(!eligible(&c, false, 1269), "an AI county never draws");
        c.owner = 0;
        assert!(!eligible(&c, true, 1269), "and neither does an unowned one");
    }

    #[test]
    fn firing_an_event_writes_its_three_modifier_fields() {
        let mut c = County::new();
        fire(&mut c, EventKind::Vermin);
        assert_eq!((c.event_population_pct, c.event_grain_pct, c.event_herd_pct), (0, -25, 0));
        assert!(c.event_fired);
    }

    /// The draw count must not depend on ownership, or two peers that disagree
    /// about one county's owner would diverge in every later draw.
    #[test]
    fn the_stream_advances_identically_whoever_owns_the_counties() {
        let run = |human: bool| {
            let mut rng = Pcg32::from_seed(1234);
            let mut c = vec![County::new(); 15];
            for id in 1..=14 {
                c[id].owner = if id % 2 == 0 { 1 } else { 0 };
            }
            let mut out = Vec::new();
            for _ in 0..20 {
                roll_all(&mut c, 14, &|_| human, 1300, &mut rng, &mut out);
            }
            (rng, out.len())
        };
        let (a, events_a) = run(true);
        let (b, events_b) = run(false);
        assert_eq!(a, b, "the generator must be in the same place either way");
        assert!(events_a > 0, "somebody should have drawn something");
        assert_eq!(events_b, 0, "and an all-AI kingdom draws nothing");
    }

    #[test]
    fn an_event_is_consumed_by_the_passes_that_read_it() {
        let mut c = County::new();
        c.grain = 1000;
        c.weather = crate::tables::Weather::Cloudy;
        fire(&mut c, EventKind::Vermin);
        crate::land::grain_season_tick(&mut c, crate::tables::Season::Winter, true);
        assert_eq!(c.grain, 750);
        assert_eq!(c.event_grain_pct, 0);
    }

    #[test]
    fn the_same_seed_rolls_the_same_events() {
        let run = || {
            let mut rng = Pcg32::from_seed(7);
            let mut c = vec![County::new(); 15];
            for id in 1..=14 {
                c[id].owner = 1;
            }
            let mut out = Vec::new();
            for _ in 0..10 {
                roll_all(&mut c, 14, &|_| true, 1300, &mut rng, &mut out);
            }
            out
        };
        assert_eq!(run(), run());
    }
}

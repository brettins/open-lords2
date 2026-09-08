//! What a season did — the messages and the pass log.
//!
//! The original raises these through its message system by numeric id, and
//! **the id is the `L2.eng` group number** — message `0x92` is group 146,
//! *"Uncertain times."*. The ids are carried here rather than discarded because
//! that correspondence is the only thing linking a rule to the string a player
//! actually sees, and it is how the event table and the bankruptcy escalation
//! were both cross-checked: the prose says what the code does.

use crate::phase::Pass;

/// `Unrest_UpdateAll` fires message `0x92` the first time a human-owned
/// county's happiness falls below 30. `docs/kingdom.md` §6.
pub const MSG_UNREST_WARNING: u16 = 0x92;

/// Messages `0x96`, `0x97`, `0x98`, `0x99`, fired as the unrest counter passes
/// 1, 2, 3 and 4.
pub const MSG_UNREST_LEVEL: [u16; 4] = [0x96, 0x97, 0x98, 0x99];

/// The three messages `Army_Starve` (`0x004ACE5E`) raises, by starvation stage.
///
/// `0x116` = `L2.eng` group 278 *"Unfed troops."*, `0x117` = 279 *"Starving
/// troops."*, `0x118` = 280 *"Army perishes."* — and the group number equalling
/// the message id is the rule three subsystems have already established.
/// `docs/armies.md` §3.3b.
pub const MSG_ARMY_UNFED: u16 = 0x116;
pub const MSG_ARMY_STARVING: u16 = 0x117;
pub const MSG_ARMY_PERISHES: u16 = 0x118;

/// Something the season did that a player would be told about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Message {
    /// A human-owned county's happiness fell below 30 for the first time.
    UnrestWarning { county: u8 },
    /// The unrest counter stepped up to `level` (1..=4).
    UnrestRising { county: u8, level: u8 },
    /// The counter reached 4 and `FUN_004AC185` raised the peasant mob. The
    /// counter resets.
    Revolt { county: u8 },
    /// The treasury could not cover the wage bill; the escalation advanced.
    /// `stage` is the counter *after* the step, so a mutiny reports 0.
    Bankrupt { realm: u8, stage: u8, action: crate::industry::BankruptcyAction },
    /// A random event fired in a county.
    Event { county: u8, kind: crate::event::EventKind },
    /// A castle finished building.
    CastleBuilt { county: u8, castle_type: u8 },
    /// An army could not be fed. `stage` is the starvation counter *after* the
    /// step, 1..=5, and it selects the message: 1 warns, 2..=4 desert, 5 is the
    /// army perishing. `docs/armies.md` §3.3b. See [`crate::unit::starve`].
    ArmyStarving { realm: u8, unit: usize, county: u8, stage: i32 },
}

impl Message {
    /// The original's message id, where `docs/kingdom.md` records one.
    pub fn original_id(self) -> Option<u16> {
        match self {
            Message::UnrestWarning { .. } => Some(MSG_UNREST_WARNING),
            Message::UnrestRising { level, .. } => {
                MSG_UNREST_LEVEL.get(level.saturating_sub(1) as usize).copied()
            }
            // The event id *is* the `L2.eng` group, so there is nothing to look
            // up: see `crate::event`.
            Message::Event { kind, .. } => Some(kind.id()),
            Message::Bankrupt { action, .. } => action.message_id(),
            Message::ArmyStarving { stage, .. } => Some(match stage {
                1 => MSG_ARMY_UNFED,
                2..=4 => MSG_ARMY_STARVING,
                _ => MSG_ARMY_PERISHES,
            }),
            _ => None,
        }
    }
}

/// The record of one `Season_Advance`.
///
/// `passes` is the pipeline as it actually ran, so a test can compare it
/// against [`crate::phase::SEASON_PIPELINE`] rather than trusting that the
/// driver walked the array. Everything in here is appended in index order —
/// counties 1..=n, then realms 1..=5 — so two peers build identical reports
/// (`docs/netcode.md` §3).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SeasonReport {
    pub passes: Vec<Pass>,
    pub messages: Vec<Message>,
    /// Counties that raised a peasant mob this season, in county order.
    pub revolts: Vec<u8>,
}

impl SeasonReport {
    pub fn new() -> SeasonReport {
        SeasonReport::default()
    }

    pub fn message(&mut self, m: Message) {
        if let Message::Revolt { county } = m {
            self.revolts.push(county);
        }
        self.messages.push(m);
    }

    /// Every message about one county, in the order they were raised.
    pub fn for_county(&self, county: u8) -> impl Iterator<Item = &Message> {
        self.messages.iter().filter(move |m| match m {
            Message::UnrestWarning { county: c }
            | Message::UnrestRising { county: c, .. }
            | Message::Revolt { county: c }
            | Message::Event { county: c, .. }
            | Message::CastleBuilt { county: c, .. }
            | Message::ArmyStarving { county: c, .. } => *c == county,
            Message::Bankrupt { .. } => false,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unrest_messages_carry_the_ids_the_document_records() {
        assert_eq!(Message::UnrestWarning { county: 1 }.original_id(), Some(0x92));
        for level in 1..=4u8 {
            assert_eq!(
                Message::UnrestRising { county: 1, level }.original_id(),
                Some(MSG_UNREST_LEVEL[level as usize - 1])
            );
        }
    }

    #[test]
    fn a_revolt_is_recorded_in_both_places() {
        let mut r = SeasonReport::new();
        r.message(Message::Revolt { county: 4 });
        assert_eq!(r.revolts, vec![4]);
        assert_eq!(r.for_county(4).count(), 1);
        assert_eq!(r.for_county(5).count(), 0);
    }
}

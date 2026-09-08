//! What a season did — the messages and the pass log.
//!
//! The original raises these through its message system by numeric id;
//! `docs/kingdom.md` §6 names five of them and the rest are unidentified. The
//! ids are carried here rather than discarded because they are the only
//! evidence linking a rule to the `L2.eng` string a player actually sees.

use crate::phase::Pass;

/// `Unrest_UpdateAll` fires message `0x92` the first time a human-owned
/// county's happiness falls below 30. `docs/kingdom.md` §6.
pub const MSG_UNREST_WARNING: u16 = 0x92;

/// Messages `0x96`, `0x97`, `0x98`, `0x99`, fired as the unrest counter passes
/// 1, 2, 3 and 4.
pub const MSG_UNREST_LEVEL: [u16; 4] = [0x96, 0x97, 0x98, 0x99];

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
    Bankrupt { realm: u8, stage: u8 },
    /// A random event fired in a county.
    Event { county: u8, kind: crate::event::EventKind },
    /// A castle finished building.
    CastleBuilt { county: u8, castle_type: u8 },
}

impl Message {
    /// The original's message id, where `docs/kingdom.md` records one.
    pub fn original_id(self) -> Option<u16> {
        match self {
            Message::UnrestWarning { .. } => Some(MSG_UNREST_WARNING),
            Message::UnrestRising { level, .. } => {
                MSG_UNREST_LEVEL.get(level.saturating_sub(1) as usize).copied()
            }
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
            | Message::CastleBuilt { county: c, .. } => *c == county,
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

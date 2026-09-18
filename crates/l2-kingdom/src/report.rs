//! The original raises these through its message system by numeric id, and
//! **the id is the `L2.eng` group number** — message `0x92` is group 146,
//! *"Uncertain times."*. The ids are carried here because
//! that correspondence is the only thing linking a rule to the string a player
//! and it is how the event table and the bankruptcy escalation
//! were both cross-checked: the prose says what the code does.

use crate::phase::Pass;

pub const MSG_UNREST_WARNING: u16 = 0x92;

pub const MSG_UNREST_LEVEL: [u16; 4] = [0x96, 0x97, 0x98, 0x99];

/// `Territory_SecedeMinorBlocks` raises `0x7F` — `L2.eng` group 127,
/// *"Deeming itself too far from the heart of your empire, this county has
/// declared independence and thrown out your officials."* — when **exactly
/// one** county was cut off.
pub const MSG_COUNTY_SECEDED: u16 = 0x7F;

/// …and `0x80`, group 128 *"Your lands divide."*, when more than one was.
pub const MSG_LANDS_DIVIDE: u16 = 0x80;

/// The three messages `Army_Starve` (`0x004ACE5E`) raises, by starvation stage.
///
/// `0x116` = `L2.eng` group 278 *"Unfed troops."*, `0x117` = 279 *"Starving
/// troops."*, `0x118` = 280 *"Army perishes."* — and the group number equalling
/// the message id is the rule three subsystems have already established.
pub const MSG_ARMY_UNFED: u16 = 0x116;
pub const MSG_ARMY_STARVING: u16 = 0x117;
pub const MSG_ARMY_PERISHES: u16 = 0x118;

/// `Weather_UpdateAll` (`0x00449889`) raises `0x8F` — `L2.eng` group 143,
/// *"Drought."* — for a county the recipient owns that banded Drought, and
/// `0x90`, group 144 *"Flooding."*, for one that flooded. Each sits directly in
/// front of the `FUN_00469A9C` call that ruins the field. `docs/formats/eng.md`
/// §5 had both rows as `[D]`; the two `Msg_Enqueue` sites make them `[V]`.
pub const MSG_DROUGHT: u16 = 0x8F;
pub const MSG_FLOODING: u16 = 0x90;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Message {
    UnrestWarning { county: u8 },
    UnrestRising { county: u8, level: u8 },
    /// The counter reached 4 and `FUN_004AC185` raised the peasant mob. The
    /// counter resets.
    Revolt { county: u8 },
    Bankrupt { realm: u8, stage: u8, action: crate::industry::BankruptcyAction },
    Event { county: u8, kind: crate::event::EventKind },
    CastleBuilt { county: u8, castle_type: u8 },
    CountySeceded { realm: u8, county: u8 },
    LandsDivide { realm: u8, counties: u8 },
    ArmyStarving { realm: u8, unit: usize, county: u8, stage: i32 },
    Drought { county: u8 },
    Flooding { county: u8 },
}

impl Message {
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
            Message::Drought { .. } => Some(MSG_DROUGHT),
            Message::Flooding { .. } => Some(MSG_FLOODING),
            Message::CountySeceded { .. } => Some(MSG_COUNTY_SECEDED),
            Message::LandsDivide { .. } => Some(MSG_LANDS_DIVIDE),
            Message::ArmyStarving { stage, .. } => Some(match stage {
                1 => MSG_ARMY_UNFED,
                2..=4 => MSG_ARMY_STARVING,
                _ => MSG_ARMY_PERISHES,
            }),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SeasonReport {
    pub passes: Vec<Pass>,
    pub messages: Vec<Message>,
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

    pub fn for_county(&self, county: u8) -> impl Iterator<Item = &Message> {
        self.messages.iter().filter(move |m| match m {
            Message::UnrestWarning { county: c }
            | Message::UnrestRising { county: c, .. }
            | Message::Revolt { county: c }
            | Message::Event { county: c, .. }
            | Message::CastleBuilt { county: c, .. }
            | Message::ArmyStarving { county: c, .. }
            | Message::CountySeceded { county: c, .. }
            | Message::Drought { county: c }
            | Message::Flooding { county: c } => *c == county,
            Message::Bankrupt { .. } | Message::LandsDivide { .. } => false,
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

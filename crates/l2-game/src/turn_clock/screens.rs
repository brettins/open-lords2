#![allow(unused_imports)]
use super::*;

use l2_view::Canvas;
use crate::game::Game;
use crate::screen::{Ctx, ScreenId};
use crate::shell::{font, Pen};

/// **`DAT_004D2E80` — which screens the timer is drawn over**, indexed by
/// `g_screenId`, and 0 means *draw*. **[V]**, read out of `.data`.
///
/// It has **one reader in the whole binary**, `FUN_0041A639`, so it is the
/// timer's own vocabulary and not a general screen property borrowed for it.
pub const SCREENS: [u8; 0x46] = [
    0, 0, 0, 0, 0, 0, 0, 0, 2, 0, 1, 1, 1, 1, 0, 0, // 0x00
    0, 0, 1, 1, 0, 0, 0, 1, 1, 0, 0, 1, 1, 1, 0, 2, // 0x10
    1, 0, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, // 0x20
    2, 2, 2, 0, 0, 0, 0, 0, 0, 0, 2, 2, 2, 2, 2, 2, // 0x30
    2, 2, 2, 2, 2, 2, // 0x40
];

/// **The screens `Screen_FrameInput` (`0x0042FF10`) closes once a turn has
/// been ended** — every `g_screenId` whose arm carries the guard
/// `DAT_00553FC8 != 0 || (DAT_0055403C != 0 && DAT_00553018 == 0)`.
///
/// **[D]** — and `docs/arms.json` `0x0042FF10/force-close-on-turn-end`, which
/// counts twenty-nine.
pub const CLOSED_BY_TURN_END: [u8; 26] = [
    0x02, 0x04, 0x05, 0x06, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x10, 0x11, 0x13, 0x14, 0x15,
    0x16, 0x17, 0x18, 0x19, 0x1A, 0x1B, 0x1D, 0x20, 0x26, 0x35, 0x36,
];

pub const CLOSED_BY_TURN_END_OUTSIDE_BATTLE: u8 = 0x39;

pub fn screen_byte(id: ScreenId) -> Option<u8> {
    use crate::screens::county::Panel;
    use crate::screens::options::Page;
    Some(match id {
        ScreenId::Campaign => 0x00,
        ScreenId::Village(_) => 0x02,
        ScreenId::Info(_) => 0x04,
        ScreenId::Merchant(_) => 0x08,
        ScreenId::Court => 0x09,
        ScreenId::Armoury(_) => 0x0A,
        ScreenId::Diplomacy => 0x0B,
        ScreenId::Trade(..) => 0x0C,
        ScreenId::Rack(..) => 0x0D,
        ScreenId::Job(..) => 0x0F,
        ScreenId::Divide(_) => 0x11,
        ScreenId::BattlePrompt => 0x12,
        ScreenId::BattleResult => 0x13,
        ScreenId::County(_, Panel::Population) => 0x14,
        ScreenId::County(_, Panel::Tax) => 0x15,
        ScreenId::County(_, Panel::Happiness) => 0x16,
        ScreenId::RaiseArmy(_) => 0x17,
        ScreenId::Supplies(_) => 0x18,
        ScreenId::County(_, Panel::Ration) => 0x19,
        ScreenId::DiploCompose(..) => 0x1A,
        ScreenId::Castle(_) => 0x1B,
        ScreenId::Conquest => 0x1C,
        ScreenId::Siege(_) => 0x1D,
        ScreenId::Confirm(_) => 0x1E,
        ScreenId::Setup(_) => 0x1F,
        ScreenId::Nobles => 0x20,
        ScreenId::Movie(_) => 0x22,
        ScreenId::About => 0x25,
        ScreenId::Tip => 0x27,
        ScreenId::Battlefield => 0x29,
        ScreenId::Ratings => 0x2E,
        ScreenId::Options(Page::Help) => 0x31,
        ScreenId::MenuBar(_) => 0x32,
        ScreenId::SaveLoad(mode) => mode.screen_id(),
        ScreenId::Options(Page::Advanced) => 0x39,
        ScreenId::Options(Page::Sound) => 0x42,
        ScreenId::Options(Page::Display) => 0x43,
        ScreenId::Options(Page::Quirks) | ScreenId::Message | ScreenId::Menu | ScreenId::Index => {
            return None
        }
    })
}

/// Which screen's byte `FUN_0041A639` would read: the top of the stack, looking
/// through the message scroll.
pub fn timer_screen(stack: impl DoubleEndedIterator<Item = ScreenId>) -> Option<ScreenId> {
    stack.rev().find(|id| *id != ScreenId::Message)
}

/// `DAT_004D2E80[g_screenId] == 0`.
pub fn drawn_over(id: ScreenId) -> bool {
    screen_byte(id).is_some_and(|b| SCREENS.get(b as usize) == Some(&0))
}

pub fn closed_by_turn_end(id: ScreenId, in_battle: bool) -> bool {
    match screen_byte(id) {
        Some(CLOSED_BY_TURN_END_OUTSIDE_BATTLE) => !in_battle,
        Some(b) => CLOSED_BY_TURN_END.contains(&b),
        None => false,
    }
}


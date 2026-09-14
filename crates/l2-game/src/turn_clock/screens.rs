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
/// Seventy bytes for `g_screenId` `0x00 … 0x45`, the range every write of the
/// byte covers; the two bytes after it are zero and are not claimed.
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
/// Twenty-seven sites of that guard in the function's body, and **every one but
/// `0x00`'s closes**. Nine test it and close at once — `0x02`, `0x04`, `0x05`,
/// `0x06`, `0x10`, `0x11`, `0x1D`, `0x26` and `0x39`, read. Eighteen test its
/// negation around the arm's input instead; the `else` was read for `0x09`,
/// `0x13`, `0x14` and `0x35`/`0x36`, all of which close, and the other thirteen
/// have the same shape and were not each opened. `0x00` is the map itself, where
/// the negated guard only stops the menus and the strip taking clicks. `0x11`
/// closes to `0x04`, `0x05` and `0x06` to `0x02`, `0x35`/`0x36` and `0x39` to the
/// screen they were opened over, so those chains reach the map a frame later.
/// **[D]** — and `docs/arms.json` `0x0042FF10/force-close-on-turn-end`, which
/// counts twenty-nine.
pub const CLOSED_BY_TURN_END: [u8; 26] = [
    0x02, 0x04, 0x05, 0x06, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x10, 0x11, 0x13, 0x14, 0x15,
    0x16, 0x17, 0x18, 0x19, 0x1A, 0x1B, 0x1D, 0x20, 0x26, 0x35, 0x36,
];

/// `0x39`, the advanced options, whose guard also tests `g_battlePhase == 0`.
/// Listed apart only because the array above is sized by hand.
pub const CLOSED_BY_TURN_END_OUTSIDE_BATTLE: u8 = 0x39;

/// **Our screen as the original's `g_screenId` byte**, or `None` for a screen
/// the original has no id for.
///
/// `None` is ours — the demo index, the quirks page — or the message scroll,
/// which the original paints over whatever is up without touching the byte;
/// [`timer_screen`] looks under it.
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
        ScreenId::Setup(_) => 0x1F,
        ScreenId::Nobles => 0x20,
        // A film: `Smk_Play` parks `g_screenId` at `0x22`. `SCREENS[0x22]` is 2, so
        // the timer is not drawn over a film, and `0x22` is not in
        // `CLOSED_BY_TURN_END`. Both follow from the tables above.
        ScreenId::Movie(_) => 0x22,
        ScreenId::About => 0x25,
        // The tip's own screen: `Tip_Show` writes `g_screenId = 0x27` (see
        // `crate::tip::screen_byte`). `SCREENS[0x27]` is 2, so the timer is not
        // drawn while a tip is up, and `0x27` is not in `CLOSED_BY_TURN_END`, so
        // a turn end does not close it. Both follow from the tables above.
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

/// Whether `Screen_FrameInput` closes this screen while a `Turn_End` stands.
pub fn closed_by_turn_end(id: ScreenId, in_battle: bool) -> bool {
    match screen_byte(id) {
        Some(CLOSED_BY_TURN_END_OUTSIDE_BATTLE) => !in_battle,
        Some(b) => CLOSED_BY_TURN_END.contains(&b),
        None => false,
    }
}


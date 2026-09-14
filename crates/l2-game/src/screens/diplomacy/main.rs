#![allow(unused_imports)]


use super::*;
use super::compose::*;
use super::tests::*;
use l2_kingdom::diplomacy::{group, Kind};
use l2_kingdom::realm::MAX_REALMS;
use l2_view::Canvas;
use crate::input::{Event, Key, Rect};
use crate::press::{Press, Widget};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::screens::message::lord_name;
use crate::shell::{font, Pen};
use crate::widget;

/// Which of the four menu layouts `g_diploMenuState` holds, and what each one
/// offers. The indices are into `L2.eng` group 72.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Menu {
    /// 0 — I have no ally. Gift, compliment, insult, **offer an alliance**.
    NoAlly,
    /// 1 — this realm **is** my ally. Gift, compliment, insult, terminate, ask
    /// for help, ask for an attack.
    Allied,
    /// 2 — I am allied to somebody else. Gift, compliment, insult, and nothing
    /// more: you cannot even offer, because the offer would be refused.
    AlliedElsewhere,
    /// 3 — a letter of mine is already sitting in their inbox. Group 72 index
    /// 24 alone, *"A message has been dispatched, my Lord."*, and **no
    /// widgets at all**: `g_diploWidgetCount = 0`.
    Dispatched,
}

impl Menu {
    /// `Diplo_DrawScreen`'s opening test, in its own order — the mail flag
    /// first, and only then the three alliance cases.
    pub fn of(ctx: &Ctx, target: u8) -> Menu {
        let me = ctx.game.player;
        let realms = &ctx.game.kingdom.realms;
        let Some(theirs) = realms.get(target as usize) else { return Menu::NoAlly };
        if theirs.pair(me).has_mail {
            return Menu::Dispatched;
        }
        let mine = realms.get(me as usize).map_or(0, |r| r.ally);
        if mine == target {
            Menu::Allied
        } else if mine == 0 {
            Menu::NoAlly
        } else {
            Menu::AlliedElsewhere
        }
    }

    /// The group 72 rows this layout draws, top to bottom. `Diplo_DrawScreen`
    /// lists them literally; the widget rectangles come from `g_diploWidgets`
    /// and are the same six regardless, taken from the top.
    pub fn rows(self) -> &'static [usize] {
        match self {
            Menu::NoAlly => &[2, 3, 4, 5],
            Menu::Allied => &[2, 3, 4, 6, 7, 8],
            Menu::AlliedElsewhere => &[2, 3, 4],
            Menu::Dispatched => &[24],
        }
    }

    /// `Ui_DrawInsetRect(0xD0, 0x60, 0xE8, h)` — the recess behind the menu, and
    /// the height is the only thing the four arms vary about it.
    ///
    /// **The dispatched layout draws none**, which is the reason this returns an
/// option: the one-line *"A message has been
    /// dispatched, my Lord."* sits directly on the window's parchment.
    pub fn inset_height(self) -> Option<i32> {
        match self {
            Menu::NoAlly => Some(0xD0),
            Menu::Allied => Some(0x130),
            Menu::AlliedElsewhere => Some(0xA0),
            Menu::Dispatched => None,
        }
    }

    /// Whether `Pl8_DrawFrame(g_miscCtySheet, 0x1D, 0x140, 0x140)` runs.
    ///
    /// **Three of the four** — its
    /// recess is `0x130` tall and reaches `0x60 + 0x130 = 0x190`, past the
/// picture's own `0x140`. Read out of the four arms
    /// about; the geometry is offered as the likely *why* and is not evidence.
    pub fn draws_seal(self) -> bool {
        self != Menu::Allied
    }

    /// What each row does. `Diplo_OpenAlliance` (`0x00436229`) is **one widget
    /// for two rows** — row 5 *"Offer an alliance"* and row 6 *"Terminate
    /// alliance"* are the same handler, and it picks `g_diploKind` 4 over 3
    /// exactly when the target is already my ally. So the kind follows from the
    /// menu state and not from which row was drawn.
    pub fn kind_of_row(row: usize) -> Option<Kind> {
        match row {
            2 => Some(Kind::Gift),
            3 => Some(Kind::Compliment),
            4 => Some(Kind::Insult),
            5 => Some(Kind::OfferAlliance),
            6 => Some(Kind::EndAlliance),
            7 => Some(Kind::AskHelp),
            8 => Some(Kind::AskAttack),
            _ => None,
        }
    }
}

/// `g_diploWidgets` (`0x004DD940`) — six 32-pixel widgets at (400, 102 + 50n).
/// The menu layout decides how many of them are live (`g_diploWidgetCount`),
///
pub fn menu_widget(slot: usize) -> Rect {
    Rect::new(400, 102 + slot as i32 * 50, 32, 32)
}

/// `g_diploWidgets` holds six records; the allied layout uses all six.
pub(super) const MENU_SLOTS: usize = 6;

/// **`g_diploWidgets` as a table** — one record per row the layout draws,
/// every one **kind 5**, read out of `+0x0F` of `0x004DD940` … `0x004DD9B8`.
/// The dispatched layout has none: `g_diploWidgetCount = 0`.
///
/// The `arm!` is the six handlers' one marker — `Diplo_OpenGift`,
/// `…Compliment`, `…Insult`, `…Alliance`, `…AskHelp`, `…AskAttack` differ only
/// in the `g_diploKind` they set and in which draft buffer they clear —
/// kind they are answered with.
fn menu_widgets(menu: Menu) -> Vec<Widget> {
    if menu == Menu::Dispatched {
        return Vec::new();
    }
    (0..menu.rows().len().min(MENU_SLOTS))
        .map(|slot| {
            Widget::new(menu_widget(slot), crate::arm!("0x00436141/diplo-open-compose", Delayed))
        })
        .collect()
}

/// The label beside a menu widget. `FUN_0040328E(72, row, 0xE0, y, 0xA0, 100)`
/// puts the text at x = 0xE0 and the first row at y = 0x70, 50 apart — so the
/// text sits 32 pixels above its own widget, which is the original's layout and
/// not a mistake here.
pub fn menu_label_y(slot: usize) -> i32 {
    0x70 + slot as i32 * 50
}


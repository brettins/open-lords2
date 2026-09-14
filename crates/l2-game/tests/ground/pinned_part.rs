#![allow(unused_imports)]
use super::*;
use super::diplomacy_part::*;
use super::army_and_confirm::*;
use l2_formats::pl8::DecodedFrame;
use l2_game::game::Assets;
use l2_game::screen::{Ctx, Machine, ScreenId};
use l2_game::screens::{battlefield, diplomacy, divide};
use l2_game::Game;
use l2_view::chrome::panels;
use l2_view::sheet::Sheet;
use l2_view::Canvas;

/// **The join, and the reason every other test in this file uses literals.**
///
/// This is the only place where a constant from the crate meets the number the
/// decompilation carries. Ablating any of those constants fails *here*, by name,
/// with the painter's address in the message — instead of quietly moving a probe
/// along with the thing it was supposed to be probing.
///
/// It needs no install and is deliberately not gated: a constant that has drifted
/// away from the binary is wrong on a machine with no copy of the game too.
#[test]
fn pinned() {
    assert_eq!(
        (diplomacy::WINDOW.x, diplomacy::WINDOW.y),
        DIPLO_WINDOW_AT,
        "Diplo_DrawScreen 0x00416CF3: FUN_004093E0(0x10, 0x20, …)",
    );
    assert_eq!(diplomacy::WINDOW_COLS, 0x1C);
    assert_eq!(diplomacy::WINDOW_ROWS, 0x1B);
    assert_eq!(diplomacy::WINDOW_SET, 1, "FUN_004093E0 is Ui_DrawBoxBorder(1, …)");
    assert_eq!((diplomacy::OK.x, diplomacy::OK.y), DIPLO_OK_AT);
    assert_eq!(diplomacy::MENU_FRAME, MENU_WIDGET_FRAME, "g_diploWidgets record 0, +4");
    assert_eq!((diplomacy::menu_widget(0).x, diplomacy::menu_widget(0).y), MENU_WIDGET_AT);
    assert_eq!(
        (diplomacy::MENU_INSET_X, diplomacy::MENU_INSET_Y, diplomacy::MENU_INSET_W),
        (MENU_INSET_AT.0, MENU_INSET_AT.1, MENU_INSET_AT.2),
    );
    assert_eq!(
        diplomacy::Menu::NoAlly.inset_height(),
        Some(MENU_INSET_AT.3),
        "the g_diploMenuState == 0 arm's Ui_DrawInsetRect height",
    );
    for slot in 0..3usize {
        let r = diplomacy::card_rect(slot);
        assert_eq!(
            (r.x, r.y, r.w, r.h),
            (CARD_AT.0, slot as i32 * CARD_PITCH + CARD_AT.1, CARD_AT.2, CARD_AT.3),
            "Diplo_DrawLordCard 0x004171EE: card {slot}",
        );
    }
    let w = divide::rows_well();
    assert_eq!(
        (w.x, w.y, divide::ROWS_WELL_COLS, divide::ROWS_WELL_ROWS),
        DIVIDE_WELL_AT,
        "Screen_SplitArmyRows 0x00419354: Ui_DrawBoxInterior(0x18, 0x80, 0x1A, 0x12)",
    );
    assert_eq!(panels::CORNER_TL, PANELS_CORNER_TL);
    assert_eq!(panels::TEXTURE, PANELS_TEXTURE);
    assert_eq!(panels::TEXTURE_DIM, PANELS_TEXTURE_DIM);
    assert_eq!(panels::SET_B, PANELS_SET_B);
    assert_eq!(panels::CELL, PANELS_CELL);
    assert_eq!(l2_view::chrome::system::OK, OK_FRAME, "Ui_OkButton mode 0");
    assert_eq!(
        (battlefield::CONFIRM_BOX.x, battlefield::CONFIRM_BOX.y),
        (CONFIRM_AT.0, CONFIRM_AT.1),
    );
    assert_eq!((battlefield::CONFIRM_COLS, battlefield::CONFIRM_ROWS), (CONFIRM_AT.2, CONFIRM_AT.3));
    assert_eq!(battlefield::BOX_SET, 1);
    assert_eq!((battlefield::CONFIRM_YES.x, battlefield::CONFIRM_YES.y), CONFIRM_YES_AT);
    assert_eq!((battlefield::CONFIRM_NO.x, battlefield::CONFIRM_NO.y), CONFIRM_NO_AT);
    assert_eq!(battlefield::CONFIRM_YES_FRAME, CONFIRM_YES_FRAME, "g_confirmWidgets record 0");
    assert_eq!(battlefield::CONFIRM_NO_FRAME, CONFIRM_NO_FRAME, "g_confirmWidgets record 1");
    assert_ne!(
        battlefield::CONFIRM_YES_FRAME,
        OK_FRAME,
        "the yes button is not Ui_OkButton's close corner, which is what this screen drew",
    );
}

// ---------------------------------------------------------------- diplomacy


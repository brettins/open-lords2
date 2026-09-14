#![allow(unused_imports)]
use super::*;
use super::main::*;
use super::compose::*;
use l2_kingdom::diplomacy::{group, Kind};
use l2_kingdom::realm::MAX_REALMS;
use l2_view::Canvas;
use crate::input::{Event, Key, Rect};
use crate::press::{Press, Widget};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::screens::message::lord_name;
use crate::shell::{font, Pen};
use crate::widget;

#[cfg(test)]
mod tests {
    use super::*;

    /// The four layouts are the painter's four, in its own order
    /// alliance row is **one widget for two rows**.
    #[test]
    fn the_four_menu_layouts_are_the_painters_four() {
        assert_eq!(Menu::NoAlly.rows(), &[2, 3, 4, 5]);
        assert_eq!(Menu::Allied.rows(), &[2, 3, 4, 6, 7, 8]);
        assert_eq!(Menu::AlliedElsewhere.rows(), &[2, 3, 4]);
        assert_eq!(Menu::Dispatched.rows(), &[24]);
        // Row 5 and row 6 are `Diplo_OpenAlliance`'s two, and they are
        // different kinds.
        assert_eq!(Menu::kind_of_row(5), Some(Kind::OfferAlliance));
        assert_eq!(Menu::kind_of_row(6), Some(Kind::EndAlliance));
        // And the two request rows only appear on the allied layout
        // the gate `docs/diplomacy.md` §4 calls "the gate on kinds 5 and 6".
        for row in [7, 8] {
            assert!(Menu::Allied.rows().contains(&row));
            assert!(!Menu::NoAlly.rows().contains(&row));
            assert!(!Menu::AlliedElsewhere.rows().contains(&row));
        }
    }

    /// The six menu widgets are 50 apart and never overlap a lord card. The
    /// cards run to x = 130 and the widgets start at 400
    /// `FUN_004369BD` running after the widget test harmless.
    #[test]
    fn the_menu_and_the_cards_cannot_both_be_hit() {
        for slot in 0..6 {
            let w = menu_widget(slot);
            assert_eq!(w.y, 102 + slot as i32 * 50);
            for card in 0..5 {
                let c = card_rect(card);
                assert!(c.x + c.w < w.x, "card {card} reaches widget {slot}");
            }
        }
    }

    /// The cards stack on a 100-pixel pitch and do not touch: 78 tall with 22
    /// pixels of air. A reader who assumed the stride was the height would
    /// place every card but the first wrongly.
    #[test]
    fn the_cards_stack_on_a_pitch_wider_than_they_are() {
        for slot in 0..4 {
            let a = card_rect(slot);
            let b = card_rect(slot + 1);
            assert_eq!(b.y - a.y, 100);
            assert!(a.y + a.h < b.y, "card {slot} runs into the next");
        }
        assert_eq!(card_rect(0).h, 0x4E);
    }

    /// The thermometer's two break points are the AI's two alliance decisions,
    ///
    #[test]
    fn the_thermometer_bands_are_the_alliance_thresholds() {
        assert_eq!(THERMOMETER_WARM, 11, "Diplo_ReplyAllianceOffer accepts at >= 11");
        assert_eq!(THERMOMETER_COLD, -11, "and refuses outright below -10");
        let filled = |s: i32| (s + 30) * THERMOMETER_H / 60;
        assert_eq!(filled(-30), 0);
        assert_eq!(filled(30), THERMOMETER_H);
        assert!(filled(0) > 0 && filled(0) < THERMOMETER_H);
    }

    /// Every refusal names a distinct `L2.eng` group, and all six exist.
    #[test]
    fn the_six_refusals_are_six_groups() {
        let all = [
            Refusal::NoCounty,
            Refusal::Unowned,
            Refusal::NotOurs,
            Refusal::NoEnemy,
            Refusal::Allied,
            Refusal::TargetAlreadyAllied,
        ];
        let mut groups: Vec<u16> = all.iter().map(|r| r.group()).collect();
        groups.sort_unstable();
        groups.dedup();
        assert_eq!(groups.len(), all.len(), "two refusals share a group");
        assert_eq!(Refusal::NoCounty.group(), 240);
        assert_eq!(Refusal::TargetAlreadyAllied.group(), 219);
    }

    /// The three windows are the three painters', and none of them covers the
    /// whole screen — every one is an inset over the diplomacy screen.
    #[test]
    fn the_three_compose_windows_are_insets() {
        for w in [GIFT_WINDOW, LETTER_WINDOW, COUNTY_WINDOW] {
            assert!(w.x > 0 && w.y > 0);
            assert!(w.x + w.w <= 640 && w.y + w.h <= 480);
        }
        // The gift's stepper and its tick are inside its own window.
        for r in [GIFT_MORE, GIFT_LESS, GIFT_SEND, GIFT_CANCEL] {
            assert!(r.x >= GIFT_WINDOW.x && r.y >= GIFT_WINDOW.y, "{r:?} is outside the window");
        }
    }

    /// **The three widget tables are three windows onto one array**, and that
    /// is why none of them hides a record the way `g_sendSuppliesWidgets` does.
    ///
    /// Decoded, `0x004DD9D0` runs: `(184,240,f68) (216,240,f66) (288,280,f29)
    /// (324,284,f31) (288,292,f29) (324,296,f31) (320,244,f29) (356,248,f31)`.
    /// The addresses are 24 apart, so `0x004DDA30` is record **4** and
    /// `0x004DDA60` is record **6** — and 4 + 2 = 6, 6 + 2 = 8. Each slice ends
    /// exactly where the next begins.
    #[test]
    fn the_three_widget_tables_are_three_slices_of_one_array() {
        const REC: u32 = 24;
        assert_eq!(WIDGET_TABLE + 4 * REC, 0x004D_DA30, "the letters' table is record 4");
        assert_eq!(WIDGET_TABLE + 6 * REC, 0x004D_DA60, "the requests' table is record 6");
        // Four, then two, then two: no gap and no overhang.
        assert_eq!(0x004D_DA30 + 2 * REC, 0x004D_DA60);
    }

    /// The pair the whole `widgets.js` convention note is anchored on, written
    /// down here so it cannot drift back: **68 is plus.**
    #[test]
    fn frame_sixty_eight_is_the_plus() {
        // `FUN_00436372` reads `if (g_uiHotspotId == 1) g_diploGold += 10`, and
        // the record carrying hotspot id 1 is the frame-68 one at (184, 240).
        assert_eq!(PLUS_FRAME, 68);
        assert_eq!(MINUS_FRAME, 66);
        assert_eq!(GIFT_MORE, Rect::new(184, 240, WIDGET_DIM, WIDGET_DIM));
        assert_eq!(GIFT_STEP, 10);
    }

    /// **All three `Ui_OkButton` calls are in different branches**, so the
    /// "only the last one is clickable" quirk cannot bite here.
    ///
    /// `Ui_OkButton` stashes `(x, y)` into `DAT_0055CE78` / `DAT_0057C8A0` and
    /// keeps only the last call's
    /// `Screen_DiploDialog` is an `if` / `else if` chain on `g_diploKind` —
    /// one arm per frame — so exactly one is drawn and exactly one is live.
    #[test]
    fn each_dialog_draws_exactly_one_close_button_inside_its_own_window() {
        for (ok, w) in
            [(GIFT_OK, GIFT_WINDOW), (LETTER_OK, LETTER_WINDOW), (COUNTY_OK, COUNTY_WINDOW)]
        {
            assert!(ok.x >= w.x && ok.y >= w.y, "{ok:?} starts outside {w:?}");
            assert!(ok.x + ok.w <= w.x + w.w && ok.y + ok.h <= w.y + w.h, "{ok:?} leaves {w:?}");
        }
        // And the three are distinct
        //
        assert_ne!(GIFT_OK, LETTER_OK);
        assert_ne!(LETTER_OK, COUNTY_OK);
    }

    /// Every `L2.eng` index this half of the module draws is inside group 72's
    /// own run
    #[test]
    fn the_compose_indices_are_the_painters_literal_arguments() {
        assert_eq!(GIFT_TO, 10);
        assert_eq!(LAST_GIFT, 23);
        assert_eq!(GIFT_OF, 18);
        assert_eq!(DISPATCH, 17);
        // `Eng_DrawString(0x48, kind + 0xB, …)` with kind = g_diploKind - 1.
        for (k, kind) in
            [Kind::Compliment, Kind::Insult, Kind::OfferAlliance, Kind::EndAlliance]
                .iter()
                .enumerate()
        {
            assert_eq!(LETTER_BASE + kind.byte() as usize - 1, 11 + k);
        }
        // `Eng_DrawString(0x48, kind + 0xF)` / `+ 0x13` / `+ 0x15`, k in 0..2.
        assert_eq!((REQUEST_BASE, REQUEST_PROMPT, REQUEST_PICKED), (15, 19, 21));
    }
}


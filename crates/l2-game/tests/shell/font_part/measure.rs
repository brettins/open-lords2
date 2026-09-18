#![allow(unused_imports)]
use super::*;
use super::glyph_map::*;
use super::descenders::*;
use super::baseline::*;
use super::preload::*;
use super::font_numeral::*;
use super::*;
use super::navigation::*;
use super::conquest::*;
use super::eng_part::*;
use super::glyph::*;
use std::path::PathBuf;
use l2_game::game::Assets;
use l2_game::input::{Event, Key};
use l2_game::screen::{Ctx, Machine, Screen, ScreenId};
use l2_game::screens::county::Panel;
use l2_game::screens::conquest::ConquestScreen;
use l2_game::screens::setup::{self, SetupPage};
use l2_game::screens::court::CourtScreen;
use l2_game::screens::ratings::RatingsScreen;
use l2_game::shell::{font, Eng};
use l2_game::Game;
use l2_view::Canvas;

/// `FUN_004014F0` (`0x004014F0`) charges 4 for `0x20` and nothing for any other
/// empty `g_glyphWidths` entry; `Ui_DrawText` (`0x00402637`) advances
/// `local_14 = 4` for all of them. `Font::width` charged 4 for `'@'`.
///
/// Ablated: `Font::width`'s `None => 0` arm → `SPACE_ADVANCE` — the first
/// assertion goes red by exactly four, 43 against 39.
#[test]
fn the_measure_charges_the_blank_sign_column_nothing_and_the_draw_charges_four() {
    let Some(dir) = install() else {
        eprintln!("skipping: no game install");
        return;
    };
    let bytes = std::fs::read(dir.join(font::BODY)).expect("Fntl2_14.pl8");
    let f = font::Font::new(bytes, 16).expect("the font loads");
    assert_eq!(f.width("@1000"), f.width("1000"), "FUN_004014F0 charges '@' nothing");
    assert_eq!(f.width(" 1000"), f.width("1000") + 4, "and a space four");

    let flat = font::Style { colour: font::TEXT, shadow: None, caps: None };
    let drawn = |s: &str| f.draw(&mut Canvas::new(120, 40), 0, 2, s, &flat);
    assert_eq!(drawn("@1000"), drawn(" 1000"), "Ui_DrawText advances both four");
}


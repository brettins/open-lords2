#![allow(unused_imports)]
use super::*;
use super::glyph_map::*;
use super::baseline::*;
use super::preload::*;
use super::measure::*;
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

#[test]
fn the_fonts_send_descenders_to_the_frames_that_have_them() {
    let Some(dir) = install() else {
        eprintln!("skipping: no game install");
        return;
    };
    let bytes = std::fs::read(dir.join(font::BODY)).expect("Fntl2_14.pl8");
    let pl8 = l2_formats::Pl8::parse(&bytes).expect("the body font parses");
    let frame = |c: char| {
        let e = font::GLYPH_MAP[c as usize - 0x20];
        assert_ne!(e, 0, "{c} has no glyph");
        &pl8.frames[e as usize - 1]
    };
    for d in "gjpqy".chars() {
        for n in "acemn".chars() {
            assert!(
                frame(d).height > frame(n).height,
                "'{d}' should hang below '{n}'"
            );
        }
    }
    assert!(frame('A').height > frame('a').height);
}

/// `Glyph_Draw` (`0x00402A14`) blits record `+0x02` rows from `+0x04` below the `+0x0D` reserve
/// (Clip_Vertical 00400000.c:2356); the bytes Fntl2_14 stores after 'v' and 'w' are never drawn.
#[test]
fn a_glyph_draws_nothing_in_its_reserved_rows() {
    let Some(dir) = install() else {
        eprintln!("skipping: no game install");
        return;
    };
    let bytes = std::fs::read(dir.join(font::BODY)).expect("Fntl2_14.pl8");
    let reserve = {
        let pl8 = l2_formats::Pl8::parse(&bytes).expect("the body font parses");
        pl8.frames[font::GLYPH_MAP['w' as usize - 0x20] as usize - 1].overhang_rows as usize
    };
    assert_eq!(reserve, 3, "'w' reserves three rows");
    let f = font::Font::new(bytes, 16).expect("the body font loads");
    let style = font::Style { colour: font::TEXT, shadow: None, caps: None };
    for s in ["v", "w"] {
        let mut canvas = Canvas::new(32, 32);
        f.draw(&mut canvas, 4, 4, s, &style);
        let inked = (4..4 + reserve)
            .flat_map(|y| (0..32).map(move |x| (x, y)))
            .filter(|&(x, y)| canvas.at(x, y) != 0)
            .count();
        assert_eq!(inked, 0, "{s:?} inks its reserved rows; Glyph_Draw never draws them");
    }
}


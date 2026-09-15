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


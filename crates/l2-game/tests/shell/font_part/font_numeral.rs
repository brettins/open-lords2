#![allow(unused_imports)]
use super::*;
use super::glyph_map::*;
use super::descenders::*;
use super::baseline::*;
use super::preload::*;
use super::measure::*;
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

/// **`Font_10.pl8` is a numeral face in a full font's layout, read through the
/// same table as the other four.** **[V]**
///
/// `Glyph_Draw` (`0x00402A14`) has one character map, `g_glyphWidths`, and no
/// per-face anything: `frame = g_glyphWidths[c - 0x20] - 1`, the record at
/// `font + frame * 0x10 + 8`, no check against the file's frame count. So the
/// only ways a face could be "partial" are a table that reaches past its end or
/// frames that are not glyphs, and this asserts which it is:
#[test]
fn font_10_is_a_numeral_face_read_through_the_shared_table() {
    let Some(dir) = install() else {
        eprintln!("skipping: no game install");
        return;
    };
    let name = font::TEN;
    let bytes = std::fs::read(dir.join(name)).expect("Font_10.pl8");
    let pl8 = l2_formats::Pl8::parse(&bytes).expect("Font_10.pl8 parses");
    assert_eq!(pl8.frames.len(), 108, "{name}: the full 108-frame layout");
    let furthest = font::GLYPH_MAP.iter().copied().max().expect("a table") as usize;
    assert!(
        furthest <= pl8.frames.len(),
        "{name}: g_glyphWidths reaches frame {furthest} and the file has {}",
        pl8.frames.len()
    );

    let f = font::Font::new(bytes.clone(), 12).expect("the font loads");
    let flat = font::Style { colour: font::TEXT, shadow: None, caps: None };
    let ink = |s: &str| -> (usize, Canvas) {
        let mut canvas = Canvas::new(160, 32);
        f.draw(&mut canvas, 2, 2, s, &flat);
        let n = canvas.pixels.iter().filter(|&&p| p != 0).count();
        (n, canvas)
    };

    for c in ('a'..='z').chain('A'..='Z') {
        let frame = &pl8.frames[font::GLYPH_MAP[c as usize - 0x20] as usize - 1];
        assert_eq!(frame.height, 2, "{name}: '{c}' should be a 2x2 stub");
        let advance = f.draw(&mut Canvas::new(16, 16), 0, 0, &c.to_string(), &flat);
        assert_eq!(advance, 3, "{name}: '{c}' advances its stub's width plus one");
    }
    let (word, _) = ink("Seasons");
    assert!(word <= 4, "{name}: \"Seasons\" should draw at most its 'e' - it drew {word} pixels");

    for c in "0123456789+-".chars() {
        let (n, _) = ink(&c.to_string());
        assert!(n >= 8, "{name}: '{c}' is a real glyph and drew only {n} pixels");
    }
    for c in [' ', '@'] {
        assert_eq!(font::GLYPH_MAP[c as usize - 0x20], 0, "'{c}' is a blank, not a glyph");
    }
    let bottom = |c: char| -> usize {
        let (_, canvas) = ink(&c.to_string());
        (0..canvas.height)
            .rfind(|&y| (0..canvas.width).any(|x| canvas.at(x, y) != 0))
            .unwrap_or_else(|| panic!("{name}: '{c}' drew nothing"))
    };
    let base = bottom('0');
    for c in "123456789".chars() {
        assert_eq!(bottom(c), base, "{name}: '{c}' is off the digits' baseline");
    }
}



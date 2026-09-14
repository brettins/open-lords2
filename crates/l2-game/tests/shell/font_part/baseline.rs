#![allow(unused_imports)]
use super::*;
use super::glyph_map::*;
use super::descenders::*;
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

/// Lowercase letters that sit *on* the baseline in all three fonts.
///
/// `g j p q y` descend everywhere. `h` is left out because it descends in
/// `Fntl2_22.pl8` and nowhere else — that typeface gives it a tail, and its
/// frame is 22 rows against 18 for the x-height letters, the same as the real
/// descenders. Excluding one letter costs nothing; pretending it is flat would
/// make this test lie about which font is wrong.
const ON_THE_BASELINE: &str = "abcdefiklmnorstuvwxz";

/// Every lowercase letter in a font must land on one baseline once drawn.
///
/// This is the test that would have caught the bug a player found by opening
/// the original next to our demo: `a c e m n o s u x z` sat three pixels below
/// `b d f h i k l t`.
/// `Font::draw` added that byte to `y` while the decoder had already reserved
/// the same rows at the top of the canvas, so the offset was applied twice —
/// but only for the frames whose rows were *stored*, so the two
/// halves of one alphabet disagreed.
///
/// It draws through `Font::draw`, because the
/// records. The whole bug lived between the decoder and the
/// blitter, and only an end-to-end render can see that seam.
///
/// The tolerance is one pixel and it is earned, not slack: `r v w` in
/// `Fntl2_14.pl8` and `s` in `Fntl2_22.pl8` end in a single-pixel terminal one
/// row below the stroke. A misapplied `0x0D` is three pixels, or four in the
/// heading font — far outside it.
///
/// **Four of the five faces.** `Font_10.pl8` has no lowercase to put on a
/// baseline; its digits are checked in
/// [`font_10_is_a_numeral_face_read_through_the_shared_table`].
#[test]
fn every_font_puts_its_lowercase_on_one_baseline() {
    let Some(dir) = install() else {
        eprintln!("skipping: no game install");
        return;
    };

    for (name, line) in [(font::EIGHT, 12), (font::SMALL, 12), (font::BODY, 16), (font::HEADING, 24)] {
        let bytes = std::fs::read(dir.join(name)).unwrap_or_else(|_| panic!("{name}"));
        let records: Vec<u8> = l2_formats::Pl8::parse(&bytes)
            .expect("the font parses")
            .frames
            .iter()
            .map(|f| f.overhang_rows)
            .collect();
        let f = font::Font::new(bytes, line).expect("the font loads");
        // Flat, so a shadow pass cannot extend a glyph a row past its own ink.
        let flat = font::Style { colour: font::TEXT, shadow: None, caps: None };

        let mut bottoms: Vec<(char, u8, usize)> = Vec::new();
        for c in ON_THE_BASELINE.chars() {
            let mut canvas = Canvas::new(48, 48);
            f.draw(&mut canvas, 2, 2, &c.to_string(), &flat);
            let bottom = (0..canvas.height)
                .rfind(|&y| (0..canvas.width).any(|x| canvas.at(x, y) != 0))
                .unwrap_or_else(|| panic!("{name}: '{c}' drew nothing"));
            let over = records[font::GLYPH_MAP[c as usize - 0x20] as usize - 1];
            bottoms.push((c, over, bottom));
        }

        let base = bottoms.iter().map(|&(_, _, b)| b).min().expect("letters");
        for &(c, over, b) in &bottoms {
            assert!(
                b == base || b == base + 1,
                "{name}: '{c}' (0x0D = {over}) bottoms at row {b}, not {base} or {}",
                base + 1,
            );
        }

        // The sharp half: the letters that declare overhang rows and the ones
        // that do not must reach the *same* first row. Under the old
        // double-application these two groups differed by exactly the declared
        // count, which is what the player saw.
        let mut groups: std::collections::BTreeMap<u8, usize> = Default::default();
        for &(_, over, b) in &bottoms {
            let e = groups.entry(over).or_insert(b);
            *e = (*e).min(b);
        }
        println!("{name}: baseline {base}, by 0x0D {groups:?}");
        for (over, b) in &groups {
            assert_eq!(
                *b, base,
                "{name}: the letters declaring {over} overhang rows sit {} pixels off \
                 the letters that do not — 0x0D is being applied twice",
                *b as i32 - base as i32,
            );
        }
        // Not vacuous in the fonts that have both kinds. `Fnt_8.pl8` has
        // `0x0D == 0` on all 150 frames (`docs/formats/pl8-failures.md` §4), so
        // for it only the one-baseline half above is a claim — and it holds.
        if name != font::HEADING && name != font::EIGHT {
            assert!(groups.len() >= 2, "{name}: expected both 0x0D = 0 and 0x0D > 0 letters");
        }
    }
}


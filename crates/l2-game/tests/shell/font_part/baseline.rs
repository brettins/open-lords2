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

const ON_THE_BASELINE: &str = "abcdefiklmnorstuvwxz";

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
        if name != font::HEADING && name != font::EIGHT {
            assert!(groups.len() >= 2, "{name}: expected both 0x0D = 0 and 0x0D > 0 letters");
        }
    }
}


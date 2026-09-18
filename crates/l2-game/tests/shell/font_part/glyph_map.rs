#![allow(unused_imports)]
use super::*;
use super::descenders::*;
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
fn the_glyph_map_is_the_table_in_the_users_own_executable() {
    let Some(dir) = install() else {
        eprintln!("skipping: no game install");
        return;
    };
    let exe = std::fs::read(dir.join("Lords2.exe")).expect("Lords2.exe");
    // The image base is 0x400000
    // a section offset away from a file offset. `.data` is found by walking the
// section table.
    let pe = u32::from_le_bytes(exe[0x3C..0x40].try_into().unwrap()) as usize;
    let nsec = u16::from_le_bytes(exe[pe + 6..pe + 8].try_into().unwrap()) as usize;
    let opt = pe + 24;
    let opt_size = u16::from_le_bytes(exe[pe + 20..pe + 22].try_into().unwrap()) as usize;
    let base = u32::from_le_bytes(exe[opt + 28..opt + 32].try_into().unwrap());
    let want = font::GLYPH_MAP_VA - base;
    let mut file_off = None;
    for i in 0..nsec {
        let o = opt + opt_size + i * 40;
        let va = u32::from_le_bytes(exe[o + 12..o + 16].try_into().unwrap());
        let vsize = u32::from_le_bytes(exe[o + 8..o + 12].try_into().unwrap());
        let roff = u32::from_le_bytes(exe[o + 20..o + 24].try_into().unwrap());
        if want >= va && want < va + vsize {
            file_off = Some((roff + (want - va)) as usize);
        }
    }
    let at = file_off.expect("0x004D71F0 is in a section");
    assert_eq!(font::GLYPH_MAP.len(), 0x100 - 0x20);
    assert_eq!(
        &exe[at..at + font::GLYPH_MAP.len()],
        &font::GLYPH_MAP[..],
        "g_glyphWidths has moved, or the transcription is wrong"
    );
}


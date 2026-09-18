#![allow(unused_imports)]
use super::*;
use super::glyph_map::*;
use super::descenders::*;
use super::baseline::*;
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

/// **`Res_LoadStatic` (`0x00499859`) preloads five faces, and records 3 and 5
/// are the two this workspace did not load.** **[V]**
///
/// `g_preloadTable` (`0x004D9F48`) is thirteen `{char name[16]; u32 size}`
/// records, and the loader hands record `n` to `File_ReadChunk` with a buffer
/// picked by `n`. The instruction that picks `&g_font8` for `n == 3` is
/// `mov dword [ebp-4], 0x005CBFB0` at `0x004998ED` — seven bytes, asserted here,
/// because an address in a comment is a claim and these are the bytes.
///
/// Ablated: `font::EIGHT` → `"Font_10.pl8"` — record 3 reads `"fnt_8.pl8"` and
/// the constant does not. (The same ablation also made
/// [`every_font_puts_its_lowercase_on_one_baseline`] panic with a lowercase
/// letter that draws nothing — which was the reason `Font_10.pl8` was not loaded
/// blindly, and is now explained by
/// [`font_10_is_a_numeral_face_read_through_the_shared_table`].)
#[test]
fn the_preload_table_names_every_face_and_record_3_is_g_font8() {
    let exe = l2_testkit::executable!();
    let record = |n: u32| -> String {
        let off = l2_testkit::pe::va_to_offset(&exe, 0x004D_9F48 + n * 0x14).expect("in .data");
        let name = &exe[off..off + 16];
        let end = name.iter().position(|&b| b == 0).unwrap_or(16);
        String::from_utf8_lossy(&name[..end]).to_ascii_lowercase()
    };
    assert_eq!(record(3), font::EIGHT.to_ascii_lowercase(), "g_font8");
    assert_eq!(record(4), font::SMALL.to_ascii_lowercase(), "g_fontSmall");
    assert_eq!(record(5), font::TEN.to_ascii_lowercase(), "g_font10");
    assert_eq!(record(6), font::BODY.to_ascii_lowercase(), "g_fontBody");
    assert_eq!(record(7), font::HEADING.to_ascii_lowercase(), "g_fontHeading");

    let at = l2_testkit::pe::va_to_offset(&exe, 0x0049_98ED).expect("in .text");
    assert_eq!(
        &exe[at..at + 7],
        &[0xC7, 0x45, 0xFC, 0xB0, 0xBF, 0x5C, 0x00],
        "Res_LoadStatic's record-3 arm is mov [ebp-4], &g_font8"
    );
}


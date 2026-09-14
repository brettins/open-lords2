#![allow(unused_imports)]
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
    // `Ui_DrawText` indexes it with `c - 0x20` for every byte above 0x1F, so the
    // table the code can reach is 224 bytes. We had 128, and eleven of the 96
    // we left out are glyphs.
    assert_eq!(font::GLYPH_MAP.len(), 0x100 - 0x20);
    assert_eq!(
        &exe[at..at + font::GLYPH_MAP.len()],
        &font::GLYPH_MAP[..],
        "g_glyphWidths has moved, or the transcription is wrong"
    );
}

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
    // The five letters with descenders are taller than the five without, in
    // the same file, under the same map. Nothing but the right mapping does
    // that.
    for d in "gjpqy".chars() {
        for n in "acemn".chars() {
            assert!(
                frame(d).height > frame(n).height,
                "'{d}' should hang below '{n}'"
            );
        }
    }
    // And an uppercase letter is taller again than a lowercase one without an
    // ascender.
    assert!(frame('A').height > frame('a').height);
}

/// Lowercase letters that sit *on* the baseline in all three fonts.
///
/// `g j p q y` descend everywhere. `h` is left out because it descends in
/// `Fntl2_22.pl8` and nowhere else — that typeface gives it a tail, and its
/// frame is 22 rows against 18 for the x-height letters, the same as the real
/// descenders. Excluding one letter costs nothing; pretending it is flat would
/// make this test lie about which font is wrong.
const ON_THE_BASELINE: &str = "abcdefiklmnorstuvwxz";

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

/// **The measure and the draw disagree about `'@'`.**
///
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

/// **`Font_10.pl8` is a numeral face in a full font's layout, read through the
/// same table as the other four.** **[V]**
///
/// `Glyph_Draw` (`0x00402A14`) has one character map, `g_glyphWidths`, and no
/// per-face anything: `frame = g_glyphWidths[c - 0x20] - 1`, the record at
/// `font + frame * 0x10 + 8`, no check against the file's frame count. So the
/// only ways a face could be "partial" are a table that reaches past its end or
/// frames that are not glyphs, and this asserts which it is:
///
/// 1. **108 frames, and nothing in the table reaches past them** — not a
///    different index base, not a short file;
/// 2. **every letter is a 2 × 2 stub** that advances three, and a lowercase
///    word drawn in it has next to no ink — *"Seasons"* is one `'e'`, four
///    pixels, where `Fntl2_9.pl8` draws it in dozens. That is the "renders
///    nothing" a canvas diff passes over;
/// 3. **everything the nine call sites build is a glyph or a blank** — `'+'`,
/// `'-'` and the ten digits have ink, `' '` and `'@'` are table zeros — and
///    the digits sit on one baseline.
///
/// Ablated, both run: pointing the test at `font::SMALL` turns claim 2 red on
/// `'a'`, a frame 8 rows tall; pointing it at `font::EIGHT` turns
/// claim 1 red, 150 frames. The *"Seasons"* ink bound was not
/// separately observed red — the stub check ahead of it fires first.
#[test]
fn font_10_is_a_numeral_face_read_through_the_shared_table() {
    let Some(dir) = install() else {
        eprintln!("skipping: no game install");
        return;
    };
    let name = font::TEN;
    let bytes = std::fs::read(dir.join(name)).expect("Font_10.pl8");
    let pl8 = l2_formats::Pl8::parse(&bytes).expect("Font_10.pl8 parses");
    // 1
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

    // 2
    for c in ('a'..='z').chain('A'..='Z') {
        let frame = &pl8.frames[font::GLYPH_MAP[c as usize - 0x20] as usize - 1];
        assert_eq!(frame.height, 2, "{name}: '{c}' should be a 2x2 stub");
        let advance = f.draw(&mut Canvas::new(16, 16), 0, 0, &c.to_string(), &flat);
        assert_eq!(advance, 3, "{name}: '{c}' advances its stub's width plus one");
    }
    let (word, _) = ink("Seasons");
    assert!(word <= 4, "{name}: \"Seasons\" should draw at most its 'e' - it drew {word} pixels");

    // 3
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


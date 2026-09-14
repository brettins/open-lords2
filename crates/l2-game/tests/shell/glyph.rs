#![allow(unused_imports)]
use super::*;
use super::navigation::*;
use super::conquest::*;
use super::eng_part::*;
use super::font_part::*;
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

/// **`Glyph_Draw` (`0x00402A14`) raises three index ranges by one pixel, and only
/// when the font is `&g_fontBody`** — asserted from the instruction bytes, so
/// no range in this test is a number of ours. **[V]**
///
/// ```text
/// 0x00402A97  81 7D 08 F0 F8 5A 00   cmp dword [ebp+8], 0x005AF8F0
/// 0x00402A9E  0F 85 …                jne 0x00402B0E
/// 0x00402AA4  33 C0 8A 45 0C 83 F8 61   xor eax, eax ; mov al, [ebp+0xC] ; cmp eax, 0x61
/// 0x00402AAC  0F 8C …                jl
///             … 83 F8 6D ; 0F 8F …   cmp eax, 0x6D ; jg
/// 0x00402AC0  FF 0D 28 15 59 00      dec dword [g_drawY]
/// ```
///
/// and the same shape twice more, the third with `3D imm32` compares. Every
/// character from `0x20` to `0xFF` is then put to `font::accent_raised`.
///
/// Ablated: `ACCENT_RAISE`'s third range `(0x80, 0x84)` → `(0x80, 0x83)` — red on
/// `0xA4`.
#[test]
fn glyph_draw_raises_three_index_ranges_and_only_for_g_font_body() {
    let Some(dir) = install() else {
        eprintln!("skipping: no game install");
        return;
    };
    let exe = std::fs::read(dir.join("Lords2.exe")).expect("Lords2.exe");
    let at = |va: u32| {
        l2_testkit::pe::va_to_offset(&exe, va).unwrap_or_else(|| panic!("{va:#010X} is not in the image"))
    };
    let bytes = |va: u32, n: usize| exe[at(va)..at(va) + n].to_vec();
    let u32_at = |va: u32| u32::from_le_bytes(bytes(va, 4).try_into().unwrap());

    assert_eq!(bytes(0x0040_2A97, 3), [0x81, 0x7D, 0x08], "cmp dword [ebp+8], imm32");
    assert_eq!(u32_at(0x0040_2A9A), 0x005A_F8F0, "the one face compared is g_fontBody");
    assert_eq!(bytes(0x0040_2A9E, 2), [0x0F, 0x85], "jne: any other face skips all three");

    // `xor eax, eax ; mov al, [ebp+0xC]` loads the index argument before every
    // compare, so each immediate is compared against `c - 0x20`.
    let index = [0x33, 0xC0, 0x8A, 0x45, 0x0C];
    let imm8 = |va: u32, jump: u8| -> u32 {
        assert_eq!(bytes(va - 7, 5), index, "the index is loaded before {:#X}", va - 2);
        assert_eq!(bytes(va - 2, 2), [0x83, 0xF8], "cmp eax, imm8 at {:#X}", va - 2);
        assert_eq!(bytes(va + 1, 2), [0x0F, jump], "the branch after {:#X}", va - 2);
        exe[at(va)] as u32
    };
    let imm32 = |va: u32, jump: u8| -> u32 {
        assert_eq!(bytes(va - 6, 5), index, "the index is loaded before {:#X}", va - 1);
        assert_eq!(bytes(va - 1, 1), [0x3D], "cmp eax, imm32 at {:#X}", va - 1);
        assert_eq!(bytes(va + 4, 2), [0x0F, jump], "the branch after {:#X}", va - 1);
        u32_at(va)
    };
    const JL: u8 = 0x8C;
    const JG: u8 = 0x8F;
    let ranges = [
        (imm8(0x0040_2AAB, JL), imm8(0x0040_2AB9, JG), 0x0040_2AC0u32),
        (imm8(0x0040_2ACD, JL), imm8(0x0040_2ADB, JG), 0x0040_2AE2),
        (imm32(0x0040_2AEE, JL), imm32(0x0040_2AFE, JG), 0x0040_2B08),
    ];
    for &(_, _, dec) in &ranges {
        // `g_drawY` is `0x00591528`; `add [g_drawY], eax` at 0x00402A91 is
        // `y += record[0x0D]`, the line before the font compare.
        assert_eq!(bytes(dec, 6), [0xFF, 0x0D, 0x28, 0x15, 0x59, 0x00], "dec dword [g_drawY] at {dec:#X}");
    }
    assert_eq!(bytes(0x0040_2A91, 6), [0x01, 0x05, 0x28, 0x15, 0x59, 0x00], "add [g_drawY], eax");

    let mut raised = 0;
    for code in 0x20u32..=0xFF {
        let i = code - 0x20;
        let oracle = ranges.iter().any(|&(lo, hi, _)| i >= lo && i <= hi);
        raised += oracle as usize;
        assert_eq!(
            font::accent_raised(char::from_u32(code).unwrap()),
            oracle,
            "{code:#04X} (index {i:#04X})"
        );
    }
    assert_eq!(raised, 13 + 5 + 5, "0x81..=0x8D, 0x93..=0x97, 0xA0..=0xA4");
}

/// **An accented character's ink is one row above where its own frame puts it,
/// in `Fntl2_14.pl8` and in no other face.**
///
/// Measured on two pairs that share a frame, so the accent's own artwork
/// cannot be what moved: `0x86` is drawn with `'a'`'s frame and `0x87` with
/// `0x80`'s (`g_glyphWidths` gives both halves of each pair the same entry). In
/// the body face the raised half's ink is the base's ink one row up; in the
/// heading, small and eight faces it is the same pixels. `Font_10.pl8` is not
/// measured, because both frames are ink-less 2 × 2 stubs there — it is
/// asserted not to raise instead.
///
/// **No string in the shipped English `L2.eng` contains a raised character**,
/// so the pairs are built here. Measured below:
/// every character above `0x7F` in the file is `0xB7`, nine of them, one at
/// the head of each of 295/2 … 295/10, and `0xB7` is index `0x97`, a zero entry
/// — a blank. The raise is reachable only through a translated `L2.eng`.
///
/// Ablated, each red here:
/// * `ShellAssets::load`'s `.map(Font::raising_accents)` removed — the body face
///   draws `0x86` on `'a'`'s row;
/// * `Font::lift` returning 0 — the same;
/// * `.map(Font::raising_accents)` added to the heading face — the heading face
///   draws `0x86` a row up;
/// * `Eng::get` put back to `from_utf8(…).ok()` — no string in the file has a
///   character above `0x7F`.
#[test]
fn an_accent_sits_one_row_above_its_own_frame_in_the_body_face_and_nowhere_else() {
    let Some(dir) = install() else {
        eprintln!("skipping: no game install");
        return;
    };
    let platform = l2_mods::Platform::builder().base(&dir).build().expect("the install mounts");
    let assets = Assets::load(&platform.vfs).expect("assets load");
    let s = &assets.shell;
    let flat = font::Style { colour: font::TEXT, shadow: None, caps: None };
    let ink = |f: &font::Font, c: char| -> Vec<(i32, i32)> {
        let mut canvas = Canvas::new(48, 48);
        f.draw(&mut canvas, 8, 8, &c.to_string(), &flat);
        let mut out = Vec::new();
        for y in 0..48 {
            for x in 0..48 {
                if canvas.at(x, y) != 0 {
                    out.push((x as i32, y as i32));
                }
            }
        }
        out
    };

    let pairs = [('a', '\u{86}'), ('\u{80}', '\u{87}')];
    for (base, raised) in pairs {
        assert_eq!(
            font::GLYPH_MAP[raised as usize - 0x20],
            font::GLYPH_MAP[base as usize - 0x20],
            "{:#04X} and {:#04X} share a frame",
            raised as u32,
            base as u32
        );
    }
    // `Fnt_8.pl8`'s frame 102 is ink-less — asserted below, not assumed — so
    // that face is measured on the `'a'` pair alone.
    let faces: [(&str, Option<&font::Font>, i32, &[(char, char)]); 4] = [
        (font::BODY, s.body.as_ref(), -1, &pairs),
        (font::HEADING, s.heading.as_ref(), 0, &pairs),
        (font::SMALL, s.small.as_ref(), 0, &pairs),
        (font::EIGHT, s.eight.as_ref(), 0, &pairs[..1]),
    ];
    let eight = s.eight.as_ref().expect("Fnt_8.pl8 is in the install");
    assert!(ink(eight, '\u{80}').is_empty() && ink(eight, '\u{87}').is_empty(), "Fnt_8.pl8 frame 102");
    for (name, f, dy, measured) in faces {
        let f = f.unwrap_or_else(|| panic!("{name} is in the install"));
        for &(base, raised) in measured {
            let under = ink(f, base);
            assert!(!under.is_empty(), "{name}: {:#04X} drew nothing", base as u32);
            let over = ink(f, raised);
            let top = |p: &[(i32, i32)]| p.iter().map(|&(_, y)| y).min().unwrap();
            assert_eq!(
                over.first().map(|_| top(&over)),
                Some(top(&under) + dy),
                "{name}: {:#04X}'s ink starts on row {}, {:#04X}'s on row {}",
                raised as u32,
                if over.is_empty() { -1 } else { top(&over) },
                base as u32,
                top(&under),
            );
            let moved: Vec<(i32, i32)> = under.iter().map(|&(x, y)| (x, y + dy)).collect();
            assert_eq!(over, moved, "{name}: {:#04X} is not {:#04X}'s ink moved {dy}", raised as u32, base as u32);
        }
    }
    assert!(!s.ten.as_ref().expect("Font_10.pl8").raises_accents(), "Font_10.pl8 is g_font10");

    // The real file: which characters above 0x7F its strings hold.
    let eng = s.eng.as_ref().expect("L2.eng");
    let mut high: std::collections::BTreeSet<(usize, String)> = Default::default();
    for g in 1..eng.count() {
        let mut i = 0;
        while let Some(t) = eng.get(g, i) {
            if t.chars().any(|c| c as u32 > 0x7F) {
                high.insert((g, t.to_string()));
            }
            i += 1;
        }
    }
    assert_eq!(high.len(), 9, "strings with a byte above 0x7F: {high:?}");
    for (g, t) in &high {
        assert_eq!(*g, 295, "{t:?}");
        assert!(t.chars().filter(|&c| c as u32 > 0x7F).all(|c| c == '\u{B7}'), "{t:?}");
        assert!(!t.chars().any(font::accent_raised), "{t:?}");
    }
    assert_eq!(font::GLYPH_MAP[0xB7 - 0x20], 0, "0xB7 is a blank in every face");
}


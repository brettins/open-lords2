//! `Msg_DrawDiplomacy` (`0x00475E07`): where a lord's letter puts its words.

use super::*;
use l2_game::message::{category, Record};

fn letter(group: u16, text: &str) -> Option<l2_view::Canvas> {
    let dir = l2_testkit::install_dir()?;
    let platform = l2_mods::Platform::builder().base(&dir).build().expect("the install mounts");
    let a = l2_game::game::Assets::load(&platform.vfs).expect("assets load");
    let (mut g, _, mut m) = world();
    g.map_slot = 4;
    g.messages.letters = vec![String::new(), String::new(), text.to_string()];
    post(&mut g, Record { to: 1, from: 2, group, category: category::DIPLOMACY, ..Record::default() });
    open_the_scroll(&mut m, &mut g, &a);
    Some(painting(&mut g, &a, &mut m))
}

/// The bounding box `(x0, y0, x1, y1)` of the pixels that differ.
fn changed(a: &l2_view::Canvas, b: &l2_view::Canvas) -> Option<(usize, usize, usize, usize)> {
    let mut bb: Option<(usize, usize, usize, usize)> = None;
    for y in 0..480 {
        for x in 0..640 {
            if a.at(x, y) != b.at(x, y) {
                let (x0, y0, x1, y1) = bb.unwrap_or((x, y, x, y));
                bb = Some((x0.min(x), y0.min(y), x1.max(x), y1.max(y)));
            }
        }
    }
    bb
}

/// Arms 1 and 2: `FUN_0040352F(&g_diploLetter + from * 0xCA, x + 0x28, y + 0x6C,
/// w - 0x50)`, the frame at `(0x10, 0x80)`. Ablation: `top + 4` or `f.x + 0x28` in
/// `draw_diplomacy`'s `free_text`.
#[test]
fn a_compliments_free_text_sits_in_its_well() {
    let Some(blank) = letter(0xF6, "") else { return };
    let said = letter(0xF6, "Greetings cousin").expect("drawn once");
    let (x0, y0, x1, y1) = changed(&blank, &said).expect("the text is drawn");
    assert!((0x38..0x3C).contains(&x0) && (0xEA..0xF0).contains(&y0), "at ({x0:#x}, {y0:#x})");
    assert!(x1 < 0x10 + 0x1C0 - 0x28 && y1 < 0x80 + 0x68 + 0x5A, "inside the well");
}

/// Arms 11 and 12, groups `0x100`/`0x101` (00440000.c:3951/3954): the final
/// `else`, line 1 at `(x + 0x20, y + 0x68)` in the 14-row frame. Ablation: rows
/// 11..=13 of `DIPLOMATIC_ROWS`.
#[test]
fn an_alliance_answer_draws_its_second_line_at_the_final_else() {
    let Some(yes) = letter(0x100, "") else { return };
    let no = letter(0x101, "").expect("drawn once");
    let (x0, _, _, y1) = changed(&yes, &no).expect("two answers, two pictures");
    assert!(x0 >= 0x10 + 0x20, "no line left of x + 0x20 ({x0:#x})");
    let band = |c: &l2_view::Canvas| (0xE8..0xF8).any(|y| (0x30..0x1D0).any(|x| c.at(x, y) != yes.at(x, y)));
    assert!(band(&no), "line 1 at y + 0x68");
    assert!(y1 < 0x80 + 14 * 16, "inside the frame");
}

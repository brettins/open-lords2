#![allow(unused_imports)]
use super::*;
use common::*;
use l2_game::shell::{font, Pen};

fn pen(a: &Assets) -> Pen<'_> {
    Pen { assets: &a.shell, ink: &a.ink, chrome: a.chrome.as_ref(), shadow: Some(font::SHADOW), caps: None }
}

/// UnitPanel_Draw 0x0041B19D: `FUN_0040328E(0x1f, 12, 0x68, …, 0x120, …)`; a
/// space measures 4 and a word joins while `used < width` (`0x0040328E`).
/// Ablation: the old greedy `measure(line + " " + word) > width` wrap keeps
/// "by" on line 2.
#[test]
fn the_merchant_description_breaks_by_onto_its_third_line() {
    let (_game, assets) = world!();
    let s = assets.shell.text(0x1f, 12).to_string();
    let lines = pen(&assets).wrap(&s, 0x120);
    assert!(lines.len() >= 3, "{lines:?}");
    assert!(lines[2].starts_with("by "), "{lines:?}");
}

/// `FUN_0040328E` drops a wrapped line's leading spaces; `FUN_0040352F`
/// (`0x0040352F`, the letter and draft buffers) keeps them.
/// Ablation: `strip` true in `break_buffer` → the second assert fails.
#[test]
fn the_buffer_wrapper_keeps_the_space_the_eng_wrapper_drops() {
    let ten = |c: char| if c == ' ' { 4 } else { 10 };
    assert_eq!(l2_game::message::break_lines("aaaa bbbb", 50, ten), vec!["aaaa", "bbbb"]);
    assert_eq!(l2_game::message::break_buffer("aaaa bbbb", 50, ten), vec!["aaaa", " bbbb"]);
}

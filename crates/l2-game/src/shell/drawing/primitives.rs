#![allow(unused_imports)]
use super::*;
use super::pen::*;
use super::*;
use super::assets::*;
use std::collections::BTreeMap;
use l2_formats::Palette;
use l2_mods::vfs::Vfs;
use l2_view::sheet::Sheet;
use l2_view::Canvas;
use eng::Eng;
use font::Font;

/// `FUN_00408FCB(name, 0x1E0)` reads one of these straight into the display
/// buffer — the `0x1E0` is 480, the row count. Returns false when the sheet is
/// missing, so the caller can fill instead of drawing nothing.
pub fn background(canvas: &mut Canvas, assets: &ShellAssets, name: &str) -> bool {
    let Some(sheet) = assets.sheet(name) else { return false };
    let Some(frame) = sheet.frame(0) else { return false };
    canvas.blit_opaque(&frame, 0, 0);
    true
}

/// `FUN_00409346(sheet, x, y, cols, rows)` — a framed box drawn from a
/// caller-supplied sheet.
///
/// **[V]** `Panels2.pl8` has the same frame layout as `Panels.pl8`: four
/// corners, four twelve-frame edges, then the 144-frame interior field at 0x34.
pub fn box_from(canvas: &mut Canvas, sheet: &Sheet, x: i32, y: i32, cols: i32, rows: i32) {
    use l2_view::chrome::panels;
    let cell = panels::CELL;
    for r in 0..rows {
        for c in 0..cols {
            let frame = if r == 0 && c == 0 {
                panels::CORNER_TL
            } else if r == 0 && c == cols - 1 {
                panels::CORNER_TR
            } else if r == rows - 1 && c == 0 {
                panels::CORNER_BL
            } else if r == rows - 1 && c == cols - 1 {
                panels::CORNER_BR
            } else if r == 0 {
                panels::EDGE_TOP + (c as usize - 1) % panels::EDGE_LEN
            } else if r == rows - 1 {
                panels::EDGE_BOTTOM + (c as usize - 1) % panels::EDGE_LEN
            } else if c == 0 {
                panels::EDGE_LEFT + (r as usize - 1) % panels::EDGE_LEN
            } else if c == cols - 1 {
                panels::EDGE_RIGHT + (r as usize - 1) % panels::EDGE_LEN
            } else {
                panels::TEXTURE
                    + (c as usize - 1) % panels::TEXTURE_DIM
                    + ((r as usize - 1) % panels::TEXTURE_DIM) * panels::TEXTURE_DIM
            };
            if let Some(f) = sheet.frame(frame) {
                canvas.blit(&f, x + c * cell, y + r * cell);
            }
        }
    }
}

/// `Ui_DrawInsetRect` (`0x00403DEB`) — **four lines and no fill.**
///
/// Colour `0x10` along the top and right edges, `0x1F` along the bottom and
/// left, clipped to the screen. That is the whole function, and the *no fill*
/// is the part worth stating: every one of these on the raise-army screen sits
/// on the panel's own parchment, so a caller that filled the rectangle first —
/// as this crate's did — painted a black hole in the middle of a window. It
/// went unseen because the fill used the interface's `background` index, which
/// is the panel colour under our own palette and pitch black under
/// `armoury.256`. `docs/decisions.md` C61.
pub fn inset_rect(canvas: &mut Canvas, x: i32, y: i32, w: i32, h: i32) {
    const TOP_RIGHT: u8 = 0x10;
    const BOTTOM_LEFT: u8 = 0x1F;
    if w < 1 || h < 1 {
        return;
    }
    canvas.fill_rect(x, y, w, 1, TOP_RIGHT);
    canvas.fill_rect(x + w - 1, y, 1, h, TOP_RIGHT);
    canvas.fill_rect(x, y + h - 1, w, 1, BOTTOM_LEFT);
    canvas.fill_rect(x, y, 1, h, BOTTOM_LEFT);
}

/// The recessed rectangle the setup pages put every menu item in —
/// `FUN_00403EE4(x, y, w, h)`. **[D]** from its own body: the top and right
/// edges are colour `0x35` and the bottom and left `0x28`, which is the
/// opposite lighting to `Ui_DrawInsetRect` (`0x00403DEB`, `0x10` and `0x1F`)
/// and reads as *raised* under `gateway.256`.
pub fn button_recess(canvas: &mut Canvas, x: i32, y: i32, w: i32, h: i32) {
    const LIGHT: u8 = 0x35;
    const DARK: u8 = 0x28;
    canvas.fill_rect(x, y, w, 1, LIGHT);
    canvas.fill_rect(x + w - 1, y, 1, h, LIGHT);
    canvas.fill_rect(x, y + h - 1, w, 1, DARK);
    canvas.fill_rect(x, y, 1, h, DARK);
}


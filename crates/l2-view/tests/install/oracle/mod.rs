#![allow(unused_imports)]

mod campaign_tests;
pub use campaign_tests::*;
mod asset_tests;
pub use asset_tests::*;

use super::*;
use super::corpus::*;
use super::render::*;
use std::{fs, path::{Path, PathBuf}};
use l2_sim::runner::{self as battle, BattleRunner};
use l2_sim::terrain;
use l2_sim::{Troop, SIDE_A, SIDE_B};
use l2_view::campaign;
use l2_view::canvas::Canvas;
use l2_view::chrome;
use l2_view::figures::{self, Anim, Colour};
use l2_view::scene::{self, BattleAssets, Camera};
use l2_view::sheet::Sheet;

/// `Lords2.exe` has no ASLR and a fixed image base of `0x400000`, so a virtual
/// address is a constant.
fn va_to_offset(exe: &[u8], va: u32) -> Option<usize> {
    let pe = u32::from_le_bytes(exe[0x3C..0x40].try_into().ok()?) as usize;
    let sections = u16::from_le_bytes(exe[pe + 6..pe + 8].try_into().ok()?) as usize;
    let opt_size = u16::from_le_bytes(exe[pe + 20..pe + 22].try_into().ok()?) as usize;
    for i in 0..sections {
        let s = pe + 24 + opt_size + i * 40;
        let rva = u32::from_le_bytes(exe[s + 12..s + 16].try_into().ok()?);
        let raw_size = u32::from_le_bytes(exe[s + 16..s + 20].try_into().ok()?);
        let raw_ptr = u32::from_le_bytes(exe[s + 20..s + 24].try_into().ok()?);
        let start = 0x0040_0000 + rva;
        if va >= start && va < start + raw_size {
            return Some((raw_ptr + (va - start)) as usize);
        }
    }
    None
}


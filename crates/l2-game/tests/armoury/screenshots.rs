#![allow(unused_imports)]
use super::*;
use super::hit_map::*;
use super::rack::*;
use super::animation::*;
use std::path::PathBuf;
use l2_game::game::Assets;
use l2_game::input::{Event, Rect};
use l2_game::scenario;
use l2_game::screen::{Ctx, Machine, ScreenId};
use l2_game::screens::armoury;
use l2_game::Game;
use l2_kingdom::tables::{Tables, WEAPON_TYPE_COUNT};
use l2_view::Canvas;

/// Not a test: **a way to look at the screen.**
///
/// ```text
/// LORDS2_DIR=... LORDS2_FIXTURES=... \
///   cargo test -p l2-game --test armoury shoot -- --ignored
/// ```
///
/// Writes PNGs into `out/`, which `.gitignore` excludes — a render of the
/// game's own artwork is a derived asset and must never be committed
/// (`CLAUDE.md` rule 1). `docs/decisions.md` C21: *show screens early, to
/// someone who knows the game.* Three shots, which are the three the player's
/// sentence is about: the levy window standing on the armoury, the armoury with
/// the window lifted off it, and one weapon's rack.
///
/// **The palette is `armoury.256`, not the campaign one.** Both screens answer
/// `Screen::palette` with it, and a shot taken through `assets.palette` is the
/// right picture in the wrong colours — which is exactly the defect this whole
/// change is about, so getting it wrong here would hide it.
#[test]
#[ignore]
fn shoot() {
    let (mut g, a) = world!();
    let county = own_county(&g);
    g.selected = county;
    g.kingdom.realms[g.player as usize].weapons = [50, 0, 120, 40, 90, 0];
    g.open_levy(county);
    g.set_levy_percent(35);
    // What *Continue* does, basket is not already full:
    // the slider writes g_levyMen and nothing else.
    g.seed_levy_basket();

    let mut m = Machine::new(ScreenId::RaiseArmy(county));
    save_png(&frame(&mut m, &mut g, &a), &a, "levy_on_the_armoury");

    let mut m = Machine::new(ScreenId::Armoury(county));
    save_png(&frame(&mut m, &mut g, &a), &a, "armoury");

    g.levy.basket.equip(l2_kingdom::unit::TroopType::Swordsman, 60);
    let mut m = Machine::new(ScreenId::Armoury(county));
    m.push(ScreenId::Rack(county, 3));
    save_png(&frame(&mut m, &mut g, &a), &a, "armoury_sword_rack");
}

fn save_png(canvas: &Canvas, assets: &Assets, name: &str) {
    let palette = assets.shell.palette("Armoury.256").unwrap_or(&assets.palette);
    let mut rgba = vec![0u8; 640 * 480 * 4];
    canvas.to_rgba(palette, &mut rgba);
let rgb: Vec<u8> = rgba.chunks(4).flat_map(|p| [p[0], p[1], p[2]]).collect();
    std::fs::create_dir_all("out").unwrap();
    std::fs::write(format!("out/{name}.png"), png::encode(640, 480, &rgb)).unwrap();
}

/// A stored-block PNG encoder with no dependency — the same forty lines
/// `tests/screens_shoot.rs` carries, and for the same reason: a `.rgb` dump needs a
/// converter and a remembered width before anyone glances at it.
pub(super) mod png {
    fn crc32(data: &[u8]) -> u32 {
        let mut table = [0u32; 256];
        for (i, e) in table.iter_mut().enumerate() {
            let mut c = i as u32;
            for _ in 0..8 {
                c = if c & 1 != 0 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
            }
            *e = c;
        }
        let mut c = 0xFFFF_FFFFu32;
        for &b in data {
            c = table[((c ^ b as u32) & 0xFF) as usize] ^ (c >> 8);
        }
        c ^ 0xFFFF_FFFF
    }

    fn adler32(data: &[u8]) -> u32 {
        let (mut a, mut b) = (1u32, 0u32);
        for &x in data {
            a = (a + x as u32) % 65521;
            b = (b + a) % 65521;
        }
        (b << 16) | a
    }

    fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], body: &[u8]) {
        out.extend_from_slice(&(body.len() as u32).to_be_bytes());
        let mut all = kind.to_vec();
        all.extend_from_slice(body);
        out.extend_from_slice(&all);
        out.extend_from_slice(&crc32(&all).to_be_bytes());
    }

    pub fn encode(w: usize, h: usize, rgb: &[u8]) -> Vec<u8> {
        let mut raw = Vec::with_capacity(h * (1 + w * 3));
        for y in 0..h {
            raw.push(0);
            raw.extend_from_slice(&rgb[y * w * 3..(y + 1) * w * 3]);
        }
        let mut z = vec![0x78, 0x01];
        for (i, block) in raw.chunks(65_535).enumerate() {
            let last = (i + 1) * 65_535 >= raw.len();
            z.push(u8::from(last));
            z.extend_from_slice(&(block.len() as u16).to_le_bytes());
            z.extend_from_slice(&(!(block.len() as u16)).to_le_bytes());
            z.extend_from_slice(block);
        }
        z.extend_from_slice(&adler32(&raw).to_be_bytes());

        let mut out = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
        let mut ihdr = Vec::new();
        ihdr.extend_from_slice(&(w as u32).to_be_bytes());
        ihdr.extend_from_slice(&(h as u32).to_be_bytes());
        ihdr.extend_from_slice(&[8, 2, 0, 0, 0]);
        chunk(&mut out, b"IHDR", &ihdr);
        chunk(&mut out, b"IDAT", &z);
        chunk(&mut out, b"IEND", &[]);
        out
    }
}


//! Renders the original's three fonts to a PNG, so a player can hold our text
//! against a screenshot of the real game.
//!
//! ```text
//! LORDS2_DIR="..." cargo run -p l2-game --example font_card
//! LORDS2_DIR="..." cargo run -p l2-game --example font_card -- some/where.png
//! ```
//!
//! **The output is a render of Sierra's artwork and must never be committed.**
//! It is written to the system temp directory by default for exactly that
//! reason; `.gitignore` covers `*.png` and that stays as it is.
//!
//! # Why this exists
//!
//! A player found a three-pixel error in our text by opening Lords of the
//! Realm II next to our demo and reading the letters. He was right, and he was
//! right about the fix for finding the next one too:
//!
//! > i assume more if you give me quick brown fox in caps and lowercase
//!
//! A word only ever shows a handful of glyphs. `"The siege is on"` happens to
//! contain `h` and `?`-adjacent shapes and almost nothing with a descender, so
//! it hid more than it showed. A card with the whole character set in both
//! cases, in every font, shows all of it at once.
//!
//! # The baseline rule
//!
//! Each line gets a hairline drawn across the card at the bottom of its own
//! `'l'`. `'l'` is the control: its frame record byte `0x0D` is zero in every
//! one of these fonts, so nothing that has ever gone wrong with the glyph
//! offset can move it. Any letter that sinks below that rule, or floats above
//! it by more than the single-pixel terminals `r v w s` have, is misplaced —
//! and you can see it without measuring anything.

use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

use l2_formats::Palette;
use l2_game::shell::font::{self, Font, Style};
use l2_view::Canvas;

/// The lines every font draws, top to bottom.
const LINES: &[&str] = &[
    "ABCDEFGHIJKLMNOPQRSTUVWXYZ",
    "abcdefghijklmnopqrstuvwxyz",
    "0123456789",
    "!\"#%&'()*+,-./:;<=>?[]",
    "THE QUICK BROWN FOX JUMPS OVER THE LAZY DOG",
    "the quick brown fox jumps over the lazy dog",
    "Is it? Yes - jumpy fjords, vexing quilt.",
];

/// The fonts, with the line height `ShellAssets::load` gives each.
const FONTS: &[(&str, i32)] = &[(font::SMALL, 12), (font::BODY, 16), (font::HEADING, 24)];

const WIDTH: usize = 900;

/// Indices into `gateway.256`, the palette the setup and conquest screens run
/// under. `font::TEXT` (`0x3F`) is **black** there — the original's body text is
/// dark on a light panel, which is easy to get backwards when a debug canvas
/// starts out black — so the card puts it on a light ground and rules the
/// baseline in red.
const BACKGROUND: u8 = 0x2C;
const CAPTION: u8 = 0x20;
const RULE: u8 = 0xF9;

fn main() {
    let Some(dir) = l2_testkit::install_dir() else {
        eprintln!("set LORDS2_DIR to a Lords of the Realm II install");
        std::process::exit(1);
    };
    let out: PathBuf = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("lords2-font-card.png"));

    // The setup pages and the conquest screen run under `gateway.256`, and
    // those are the screens whose text the player is comparing.
    let palette = std::fs::read(dir.join("gateway.256"))
        .ok()
        .and_then(|b| Palette::from_bytes(&b).ok())
        .unwrap_or_else(|| {
            eprintln!("gateway.256 missing - falling back to a grey ramp");
            Palette::from_bytes(&grey_ramp()).expect("the ramp is 768 bytes")
        });

    let mut rows: Vec<(String, Font)> = Vec::new();
    for (name, line) in FONTS {
        match std::fs::read(dir.join(name)).map_err(|e| e.to_string()).and_then(|b| Font::new(b, *line))
        {
            Ok(f) => rows.push((name.to_string(), f)),
            Err(e) => eprintln!("{name}: {e}"),
        }
    }
    if rows.is_empty() {
        eprintln!("no fonts loaded from {}", dir.display());
        std::process::exit(1);
    }

    // Height: each font contributes a caption plus its lines, generously spaced
    // so a sunk glyph has somewhere to sink to and stays visible.
    let height: usize = rows.iter().map(|(_, f)| 24 + LINES.len() * (f.line as usize + 16)).sum();
    let mut canvas = Canvas::new(WIDTH, height + 16);
    canvas.clear(BACKGROUND);

    // The card labels itself with our own 5x7 font, so the caption can never be
    // mistaken for a sample of the font it is captioning.
    let mut y: i32 = 8;
    for (name, f) in &rows {
        l2_view::text::draw(&mut canvas, 8, y, &format!("{name}  (our render)"), CAPTION);
        y += 16;
        let step = f.line + 16;
        for line in LINES {
            // Flat, in the colour the setup painters pass. `DAT_005AEA40` is
            // set around every menu item and body line, so flat is what those
            // screens actually show; the heading is the embossed one.
            let flat = Style { colour: font::TEXT, shadow: None, caps: None };
            f.draw(&mut canvas, 24, y, line, &flat);
            if let Some(b) = baseline(f, y) {
                for x in 0..WIDTH {
                    // Dashed, so it reads as a guide and not as part of a glyph.
                    if x % 3 != 2 {
                        canvas.set(x, b, RULE);
                    }
                }
            }
            y += step;
        }
        y += 8;
    }

    let rgb = to_rgb(&canvas, &palette);
    match write_png(&out, WIDTH, canvas.height, &rgb) {
        Ok(()) => println!("wrote {}", out.display()),
        Err(e) => {
            eprintln!("{}: {e}", out.display());
            std::process::exit(1);
        }
    }
}

/// The row a line's baseline rule goes on: the bottom of its own `'l'`, drawn
/// at the same `y`. See the module comment for why `'l'` and nothing else.
fn baseline(f: &Font, y: i32) -> Option<usize> {
    let mut probe = Canvas::new(64, y as usize + 64);
    let flat = Style { colour: font::TEXT, shadow: None, caps: None };
    f.draw(&mut probe, 2, y, "l", &flat);
    (0..probe.height).rfind(|&r| (0..probe.width).any(|x| probe.at(x, r) != 0))
}

fn grey_ramp() -> Vec<u8> {
    (0..256u32).flat_map(|i| [(i * 63 / 255) as u8; 3]).collect()
}

fn to_rgb(canvas: &Canvas, palette: &Palette) -> Vec<u8> {
    canvas.pixels.iter().flat_map(|&i| palette.rgb(i)).collect()
}

// ------------------------------------------------------------------- the PNG
//
// Hand-rolled rather than pulled in as a dependency. The workspace has no image
// crate and this is the only thing in it that wants one: an example that writes
// a diagnostic picture is a poor reason to put a decoder in everybody's
// dependency tree. Deflate "stored" blocks are uncompressed and legal, so the
// file is large and every reader opens it.

fn write_png(path: &std::path::Path, w: usize, h: usize, rgb: &[u8]) -> std::io::Result<()> {
    let mut raw = Vec::with_capacity(h * (1 + w * 3));
    for y in 0..h {
        raw.push(0); // filter: none
        raw.extend_from_slice(&rgb[y * w * 3..(y + 1) * w * 3]);
    }

    let mut out = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&(w as u32).to_be_bytes());
    ihdr.extend_from_slice(&(h as u32).to_be_bytes());
    ihdr.extend_from_slice(&[8, 2, 0, 0, 0]); // 8-bit, truecolour RGB
    chunk(&mut out, b"IHDR", &ihdr);
    chunk(&mut out, b"IDAT", &zlib_stored(&raw));
    chunk(&mut out, b"IEND", &[]);

    File::create(path)?.write_all(&out)
}

fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], body: &[u8]) {
    out.extend_from_slice(&(body.len() as u32).to_be_bytes());
    out.extend_from_slice(kind);
    out.extend_from_slice(body);
    let mut crc_input = kind.to_vec();
    crc_input.extend_from_slice(body);
    out.extend_from_slice(&crc32(&crc_input).to_be_bytes());
}

/// A zlib stream of deflate stored blocks: no compression, no tables, no risk.
fn zlib_stored(data: &[u8]) -> Vec<u8> {
    let mut out = vec![0x78, 0x01];
    let mut i = 0;
    loop {
        let n = (data.len() - i).min(0xFFFF);
        let last = i + n >= data.len();
        out.push(if last { 1 } else { 0 });
        out.extend_from_slice(&(n as u16).to_le_bytes());
        out.extend_from_slice(&(!(n as u16)).to_le_bytes());
        out.extend_from_slice(&data[i..i + n]);
        i += n;
        if last {
            break;
        }
    }
    out.extend_from_slice(&adler32(data).to_be_bytes());
    out
}

fn adler32(data: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for &v in data {
        a = (a + v as u32) % 65521;
        b = (b + a) % 65521;
    }
    (b << 16) | a
}

fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &v in data {
        crc ^= v as u32;
        for _ in 0..8 {
            crc = if crc & 1 != 0 { (crc >> 1) ^ 0xEDB8_8320 } else { crc >> 1 };
        }
    }
    !crc
}

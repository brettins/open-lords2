//! **The text a player looks at constantly**: the menu bar's clock and treasury,
//! and the front end's title page.
//!
//! ```text
//! LORDS2_DIR="F:\games\Lords of the Realm II" cargo test -p l2-game --test chrome_text
//! ```
//!
//! Every assertion here is *install-gated on purpose*. The defect this file was
//! written for is invisible without the real fonts: with no install every `Pen`
//! method falls back to `l2_view::text` and a screen that never calls a `Pen` at
//! all looks exactly like one that does. `docs/agents.md` — *five defects this
//! week existed only against real assets*.

use std::path::PathBuf;

use l2_game::game::Assets;
use l2_game::screen::{Ctx, Screen};
use l2_game::screens::map::MapScreen;
use l2_game::screens::setup::{SetupPage, SetupScreen};
use l2_game::shell::font::{self, Font, Style};
use l2_game::{scenario, Game};
use l2_kingdom::tables::Tables;
use l2_mods::Platform;
use l2_view::Canvas;

fn install() -> Option<PathBuf> {
    l2_testkit::install_dir()
}

macro_rules! world {
    () => {{
        let Some(dir) = install() else {
            l2_testkit::skip!("no game install, so there are no fonts to draw with");
        };
        let platform = Platform::builder()
            .base(&dir)
            .build()
            .expect("the install mounts");
        let assets = Assets::load(&platform.vfs).expect("assets load");
        let save = l2_testkit::england!();
        let game = scenario::from_save(&save, Tables::DEFAULT).expect("the fixture loads");
        (game, assets)
    }};
}

fn draw<S: Screen>(screen: &mut S, game: &mut Game, assets: &Assets) -> Canvas {
    let mut canvas = Canvas::screen();
    let ctx = Ctx { game, assets };
    screen.draw(&ctx, &mut canvas);
    canvas
}

/// Where a string drawn in `font` at `colour` sits on the canvas, by its exact
/// pattern of set pixels.
///
/// **This is the whole instrument, and it is why these tests can be about the
/// game's own fonts.** `screens.rs`'s `find_text` renders the probe in
/// `l2_view::text`, our 5 × 7 font — so it can only ever find text drawn in
/// *that* font, and a screen that switched to `Fntl2_14.pl8` would make it
/// return `None` while looking like a missing draw call. Here the probe is
/// rendered with the same [`Font`] the assertion claims drew it, so finding it
/// **is** the claim "this was drawn in this face".
fn find_in(canvas: &Canvas, f: &Font, s: &str, colour: u8) -> Option<(i32, i32)> {
    find_styled(
        canvas,
        f,
        s,
        &Style {
            colour,
            shadow: None,
            caps: None,
        },
    )
}

/// The same, for a string the painter draws with a [`Style`] of its own.
///
/// **The title page needs this and it is not a nicety.** `FUN_0041EA14` sets
/// `DAT_0058FE2C` around the heading, which draws `A` … `Z` in **colour 1**
/// instead of the caller's — so *"Lords of the Realm 2"* is two colours, and a
/// probe that expects one finds nothing while the line is plainly on screen.
/// Matching the probe's own per-pixel colours is the claim *"drawn in this
/// face, in this mode"*, which is stronger than either half.
fn find_styled(canvas: &Canvas, f: &Font, s: &str, style: &Style) -> Option<(i32, i32)> {
    // Probe on 0, which no style below writes, and pad a row above and below so
    // an embossed probe has somewhere to put its shadows.
    let w = f.width(s).max(1);
    let h = f.height(s).max(1) + 2;
    let mut probe = Canvas::new(w as usize, h as usize);
    f.draw(&mut probe, 0, 1, s, style);
    let wanted: Vec<(i32, i32, u8)> = (0..h)
        .flat_map(|y| (0..w).map(move |x| (x, y)))
        .map(|(x, y)| (x, y, probe.at(x as usize, y as usize)))
        .filter(|&(_, _, c)| c != 0)
        .collect();
    if wanted.is_empty() {
        return None;
    }
    // **Inclusive on both bounds.** The build stamp sits with its last row two
    // pixels off the bottom, which is exactly the offset an exclusive range
    // cannot reach — the check reported the stamp missing while a screenshot
    // showed it, which is a false negative in a test written to catch a false
    // positive.
    for oy in 0..=(canvas.height as i32 - h) {
        'next: for ox in 0..=(canvas.width as i32 - w) {
            for &(x, y, c) in &wanted {
                if canvas.at((ox + x) as usize, (oy + y) as usize) != c {
                    continue 'next;
                }
            }
            // `+ 1` undoes the padding row, so the answer is the `y` the
            // painter passed.
            return Some((ox, oy + 1));
        }
    }
    None
}

// ------------------------------------------------------- the menu bar's chrome

/// **`Screen_DrawMenuBar` (`0x00419C78`) draws the year, the season and the
/// treasury in `g_fontBody`, and we drew all three in our 5 × 7 debug font.**
///
/// A player reported it as *"still placeholder font in the top right for gold
/// and summer"*, and he was reading the two `l2_view::text::draw` calls that
/// used to be at the tail of `draw_menu_bar`.
///
/// The probe is rendered with `Fntl2_14.pl8` itself, so this test cannot pass
/// while the debug font is drawing them — which is the one thing the previous
/// suite could not tell apart.
#[test]
fn the_clock_and_the_treasury_are_drawn_in_the_game_s_body_font() {
    let (mut game, assets) = world!();
    let body = assets
        .shell
        .body
        .as_ref()
        .expect("Fntl2_14.pl8 is in the install");
    let mut screen = MapScreen::new();
    let canvas = draw(&mut screen, &mut game, &assets);

    let year = format!(" {} ", game.kingdom.year);
    assert!(
        find_in(&canvas, body, &year, font::TEXT).is_some(),
        "the year is not on the bar in Fntl2_14.pl8"
    );

    let season = l2_game::screens::map::season_text(&assets, game.kingdom.season);
    assert!(!season.is_empty(), "L2.eng group 29 has the season names");
    assert!(
        find_in(&canvas, body, &season, font::TEXT).is_some(),
        "{season:?} is not on the bar in Fntl2_14.pl8"
    );
}

/// **The order is the original's: the year first, then the season.**
///
/// `Screen_DrawMenuBar` draws `Ui_DrawYear(g_year, 0x168, …)` and then puts the
/// season at `g_penAdvance + 0x16C` — *after* it. We drew `"{season} {year}"`,
/// which is the other way round and was never read against the function.
///
/// Ablated by swapping the two draw calls in `draw_menu_bar`: the season then
/// lands left of the year and this goes red.
#[test]
fn the_year_comes_before_the_season() {
    let (mut game, assets) = world!();
    let body = assets.shell.body.as_ref().expect("Fntl2_14.pl8");
    let mut screen = MapScreen::new();
    let canvas = draw(&mut screen, &mut game, &assets);

    let year = format!(" {} ", game.kingdom.year);
    let season = l2_game::screens::map::season_text(&assets, game.kingdom.season);
    let (yx, _) = find_in(&canvas, body, &year, font::TEXT).expect("the year");
    let (sx, _) = find_in(&canvas, body, &season, font::TEXT).expect("the season");
    assert!(yx < sx, "the year is at x {yx} and the season at x {sx}");
}

/// **The treasury is `Ui_DrawCount(gold, 0, …)` — a number and then `L2.eng`
/// group 8's *"Crown."* / *"Crowns."*.** We drew `"GOLD 1234"`, which is a word
/// the game does not have.
#[test]
fn the_treasury_names_crowns_out_of_the_game_s_own_strings() {
    let (mut game, assets) = world!();
    let body = assets.shell.body.as_ref().expect("Fntl2_14.pl8");
    let mut screen = MapScreen::new();
    let canvas = draw(&mut screen, &mut game, &assets);

    let gold = game.gold();
    let noun = assets
        .shell
        .text(
            l2_game::shell::COUNT_NOUN_GROUP,
            l2_game::shell::count_noun(gold, 0),
        )
        .to_string();
    assert!(
        noun.starts_with("Crown"),
        "group 8/0-1 is Crown./Crowns., not {noun:?}"
    );
    assert!(
        find_in(&canvas, body, &noun, font::TEXT).is_some(),
        "{noun:?} is not on the bar in Fntl2_14.pl8"
    );
    // And the *word we invented* is gone. `GOLD` is not in `L2.eng` at all, so
    // this is the half of the assertion that catches a fix which draws both —
    // and it is looked for in the font it used to be drawn in, at the ink the
    // old line passed.
    assert!(
        debug_font_absent(&canvas, "GOLD", assets.ink.text),
        "the placeholder caption GOLD is still drawn in the 5 x 7 debug font"
    );
}

/// Whether our own 5 × 7 font's rendering of `s` in `colour` is anywhere on the
/// canvas.
///
/// **The first version of this asked the wrong question.** It looked for the
/// glyph pattern in *any* single colour, which matches every flat run of pixels
/// wide enough to hold it — so it reported the caption present on a canvas that
/// did not have it, on the strength of a patch of sea. A negative assertion is
/// only worth what its positive twin is, so this pins the colour the line it is
/// about actually used.
fn debug_font_absent(canvas: &Canvas, s: &str, colour: u8) -> bool {
    let w = l2_view::text::width(s).max(1);
    let h = l2_view::text::GLYPH_H;
    let mut probe = Canvas::new(w as usize, h as usize);
    l2_view::text::draw(&mut probe, 0, 0, s, 1);
    let wanted: Vec<(i32, i32)> = (0..h)
        .flat_map(|y| (0..w).map(move |x| (x, y)))
        .filter(|&(x, y)| probe.at(x as usize, y as usize) == 1)
        .collect();
    if wanted.is_empty() {
        return true;
    }
    for oy in 0..=(canvas.height as i32 - h) {
        'next: for ox in 0..=(canvas.width as i32 - w) {
            for &(x, y) in &wanted {
                if canvas.at((ox + x) as usize, (oy + y) as usize) != colour {
                    continue 'next;
                }
            }
            return false;
        }
    }
    true
}

// ------------------------------------------------- the menu drop-down's plate

/// **`FUN_00409429` is not `Ui_DrawBox`, and the difference is a whole 16-pixel
/// row of the menu drop-down.**
///
/// A player reported the text *"clipping into the 'fold' of the scroll at the
/// top"* with *"a bit of empty space from the last bit of text to the bottom
/// 'fold'"*. The captions were never wrong: their pitch is `g_menuBarItems`'
/// own `y` column, 0, 20, 40, …, and `screens::menubar` carries it. The plate
/// was, because border **set 2** omits the top rail and fills its interior from
/// the box's own `y` — so the parchment reaches the menu bar and the first
/// caption at `y = 38` sits fourteen pixels into it, not two pixels into a rail.
///
/// The assertion is an **equality against the original's own second call**:
/// draw the drop-down plate, then draw `Ui_DrawBoxInterior(x + 0x10, y, cols -
/// 2, rows - 1)` on its own, and require the top row's middle band to be the
/// same pixels. No threshold, and nothing about what the artwork looks like.
///
/// Ablated by restoring `r > 0` to the interior test in
/// `l2_view::chrome::Chrome::draw_box`.
#[test]
fn the_drop_down_plate_has_no_top_rail() {
    let (mut game, assets) = world!();
    let Some(chrome) = assets.chrome.as_ref() else {
        l2_testkit::skip!("no Panels.pl8, so there is no box artwork to compare");
    };
    let _ = &mut game;

    // `FUN_0040C725`: `FUN_00409429(x, y + 0x12, 0x0C, rows)`, and the Help
    // menu's seven items make `(7 * 0x15) / 16 + 2` = 11 rows.
    const X: i32 = 40;
    const Y: i32 = 24;
    const COLS: i32 = 0x0C;
    const ROWS: i32 = 11;
    const CELL: i32 = l2_view::chrome::panels::CELL;

    let mut plate = Canvas::screen();
    chrome.draw_box(&mut plate, X, Y, COLS, ROWS, 2);

    // The second half of `FUN_00409429`, on its own, at the same place.
    let mut interior = Canvas::screen();
    for r in 0..(ROWS - 1) {
        for c in 0..(COLS - 2) {
            let frame = l2_view::chrome::panels::TEXTURE
                + (c as usize) % l2_view::chrome::panels::TEXTURE_DIM
                + ((r as usize) % l2_view::chrome::panels::TEXTURE_DIM)
                    * l2_view::chrome::panels::TEXTURE_DIM;
            chrome.draw_panel_frame(&mut interior, frame, X + CELL + c * CELL, Y + r * CELL);
        }
    }

    let mut differ = 0;
    for y in Y..Y + CELL {
        for x in (X + CELL)..(X + (COLS - 1) * CELL) {
            if plate.at(x as usize, y as usize) != interior.at(x as usize, y as usize) {
                differ += 1;
            }
        }
    }
    assert_eq!(
        differ,
        0,
        "the plate's top row is not the interior the original fills it with - \
         {differ} pixels of a {} x {CELL} band differ, which is the top rail we \
         should not be drawing",
        (COLS - 2) * CELL
    );

    // And the top corners are **edge** pieces, not corners: `frame = 0x1C` and
    // `0x28`, which is what the left and right edges use one row down.
    for (c, label) in [(0, "top-left"), (COLS - 1, "top-right")] {
        for y in 0..CELL {
            for x in 0..CELL {
                let (px, py) = (X + c * CELL + x, Y + y);
                assert_eq!(
                    plate.at(px as usize, py as usize),
                    plate.at(px as usize, (py + CELL) as usize),
                    "the {label} cell is not the edge piece the row below it uses"
                );
            }
        }
    }
}

// ------------------------------------------------------------- the title page

/// **`FUN_0041EA14` puts the title at `y = 0x1E`, and we put it at `0x20`.**
///
/// Two pixels, and it is the sort of number that is only ever wrong because
/// nobody read the painter's arguments back. The subtitle beside it — `0x3A` —
/// was right all along, which is what made the pair worth checking.
#[test]
fn the_title_page_draws_the_game_s_own_name_where_the_painter_puts_it() {
    let Some(dir) = install() else {
        l2_testkit::skip!("no game install");
    };
    let platform = Platform::builder()
        .base(&dir)
        .build()
        .expect("the install mounts");
    let assets = Assets::load(&platform.vfs).expect("assets load");
    let save = l2_testkit::england!();
    let mut game = scenario::from_save(&save, Tables::DEFAULT).expect("the fixture loads");

    let heading = assets.shell.heading.as_ref().expect("Fntl2_22.pl8");
    let mut screen = SetupScreen::new(SetupPage::Title);
    let canvas = draw(&mut screen, &mut game, &assets);

    let title = assets.shell.text(11, 0).to_string();
    assert_eq!(title, "Lords of the Realm 2", "L2.eng 11/0");
    // The heading's own mode: **embossed** — `FUN_0041EA14` never sets
    // `DAT_005AEA40` for it — with `DAT_0058FE2C`'s drop capitals on. The
    // gateway pair, because this page runs under `gateway.256` and
    // `Ui_DrawText` swaps its shadow colours on `g_screenId == 0x1F`.
    let style = Style {
        colour: font::TEXT,
        shadow: Some(font::SHADOW_GATEWAY),
        caps: Some(1),
    };
    let (_, y) = find_styled(&canvas, heading, &title, &style)
        .expect("the title is not on the page in Fntl2_22.pl8");
    // **`0x1E` is written out, not read from `setup::TITLE_Y`.** Deriving the
    // expectation from the constant under test is `docs/agents.md`'s first way
    // to ablate wrongly: change the constant and the probe moves with it, and
    // the test stays green while asserting the code agrees with itself. This
    // number is pinned from `FUN_0041EA14`'s own argument list and no
    // expression here mentions the constant.
    assert_eq!(
        y, 0x1E,
        "FUN_0041EA14: Ui_DrawCentred(11, 0, 0x80, 0x1E, 0x180, &g_fontHeading, 0x3F)"
    );
}

/// **The build stamp is inside the canvas, in the plain face.** Three earlier
/// versions of this check were each true of something adjacent — see
/// `docs/agents.md`. This one asserts the two things anybody wanted: every
/// pixel it writes is on screen, and it is not drawn in a blackletter face.
#[test]
fn the_build_stamp_is_legible_and_on_screen() {
    let Some(dir) = install() else {
        l2_testkit::skip!("no game install");
    };
    let platform = Platform::builder()
        .base(&dir)
        .build()
        .expect("the install mounts");
    let assets = Assets::load(&platform.vfs).expect("assets load");
    let save = l2_testkit::england!();
    let mut game = scenario::from_save(&save, Tables::DEFAULT).expect("the fixture loads");

    let small = assets.shell.small.as_ref().expect("Fntl2_9.pl8");
    let mut screen = SetupScreen::new(SetupPage::Title);
    let canvas = draw(&mut screen, &mut game, &assets);
    let s = format!("BUILD {}", l2_game::build_id::ID);
    let (_, y) = find_in(&canvas, small, &s, assets.ink.dim).expect("the stamp, in Fntl2_9.pl8");
    assert!(
        y + small.height(&s) <= l2_view::canvas::HEIGHT as i32,
        "the stamp's last row is at {} and the canvas ends at {}",
        y + small.height(&s),
        l2_view::canvas::HEIGHT
    );
}

// ------------------------------------------------------------------- a look

/// Not a test: `cargo test -p l2-game --test chrome_text shoot -- --ignored`
/// writes PNGs into `out/`. **A feature whose purpose is to be read has exactly
/// one acceptance test, and it is a person reading it.**
///
/// Renders of the game's own artwork are derived assets and are never committed
/// (`CLAUDE.md` rule 1); `.gitignore` covers `out/`.
#[test]
#[ignore]
fn shoot() {
    let (mut game, assets) = world!();

    let mut screen = MapScreen::new();
    let canvas = draw(&mut screen, &mut game, &assets);
    save(&canvas, &assets.palette, "menubar");

    // The three drop-downs, over the map, for the row-pitch measurement.
    for menu in 0..3usize {
        let mut m = l2_game::screen::Machine::new(l2_game::screen::ScreenId::Campaign);
        m.push(l2_game::screen::ScreenId::MenuBar(menu));
        let mut canvas = Canvas::screen();
        let ctx = Ctx {
            game: &mut game,
            assets: &assets,
        };
        m.draw(&ctx, &mut canvas);
        save(&canvas, &assets.palette, &format!("dropdown_{menu}"));
    }

    for page in [SetupPage::Title, SetupPage::Options] {
        let mut screen = SetupScreen::new(page);
        let canvas = draw(&mut screen, &mut game, &assets);
        let p = assets
            .shell
            .palette("Gateway.256")
            .unwrap_or(&assets.palette);
        save(&canvas, p, &format!("setup_{}", page.number()));
    }
}

fn save(canvas: &Canvas, palette: &l2_formats::Palette, name: &str) {
    let mut rgba = vec![0u8; 640 * 480 * 4];
    canvas.to_rgba(palette, &mut rgba);
    let rgb: Vec<u8> = rgba
        .chunks_exact(4)
        .flat_map(|p| [p[0], p[1], p[2]])
        .collect();
    let dir = std::env::var("L2_SHOT_DIR").unwrap_or_else(|_| "out".to_string());
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(format!("{dir}/{name}.png"), png::encode(640, 480, &rgb)).unwrap();
}

mod png {
    fn crc32(data: &[u8]) -> u32 {
        let mut crc = 0xFFFF_FFFFu32;
        for &v in data {
            crc ^= v as u32;
            for _ in 0..8 {
                crc = if crc & 1 != 0 {
                    (crc >> 1) ^ 0xEDB8_8320
                } else {
                    crc >> 1
                };
            }
        }
        !crc
    }
    fn adler32(data: &[u8]) -> u32 {
        let (mut a, mut b) = (1u32, 0u32);
        for &v in data {
            a = (a + v as u32) % 65521;
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

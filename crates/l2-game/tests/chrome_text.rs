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
use l2_game::input::{Event, Key};
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
/// which is the other way round.
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

/// Where `s`, drawn in `f` at `colour`, starts **on the row the painter
/// passed** — or `None`.
///
/// A whole-canvas search returns the first match in raster order, which for a
/// short number is often a digit inside some *other* number higher up the
/// screen. Pinning the row turns "is this string anywhere" into "is this string
/// at this call site", which is the claim a position test makes.
fn find_on_row(canvas: &Canvas, f: &Font, s: &str, colour: u8, y: i32) -> Option<i32> {
    let style = Style { colour, shadow: None, caps: None };
    let w = f.width(s).max(1);
    let h = f.height(s).max(1) + 2;
    let mut probe = Canvas::new(w as usize, h as usize);
    f.draw(&mut probe, 0, 1, s, &style);
    let wanted: Vec<(i32, i32, u8)> = (0..h)
        .flat_map(|py| (0..w).map(move |px| (px, py)))
        .map(|(px, py)| (px, py, probe.at(px as usize, py as usize)))
        .filter(|&(_, _, c)| c != 0)
        .collect();
    if wanted.is_empty() {
        return None;
    }
    let oy = y - 1;
    'next: for ox in 0..=(canvas.width as i32 - w) {
        for &(px, py, c) in &wanted {
            if canvas.at((ox + px) as usize, (oy + py) as usize) != c {
                continue 'next;
            }
        }
        return Some(ox);
    }
    None
}

/// **`Ui_DrawCount` opens the treasury with `'@'`, so its digits start four
/// pixels right of the call site's `x`, and we drew them at the `x`.**
///
/// `Screen_DrawMenuBar` (`0x00419C78`) calls
/// `Ui_DrawCount(gold, 0, 500, 6, &g_fontBody, 0x3F)`, and `Ui_DrawCount`
/// (`0x0041AB67`) is `Ui_DrawNumber(value, '@', &DAT_004D41F4, x, y, …)` with
/// `DAT_004D41F4` a NUL. `Ui_DrawText` (`0x00402637`) advances **4** over the
/// glyph-less `'@'` (`local_14 = 4`) and **4** more after the string
/// (`g_penAdvance + 4`), and `Ui_DrawCount` puts the noun at `x + g_penAdvance`.
///
/// **Every expected number is a literal from those functions, not a constant of
/// ours** — the lead's 4, the trailer's 4, 500 and 6 — so ablating `Pen`'s
/// constants cannot move the expectation with the code. Only the digits' own
/// width is measured, and it is measured in the face the call site names.
///
/// **Two assertions because the old code was wrong in two opposite ways that
/// cancelled at the noun.** `blank_lead: true` dropped the lead and invented a
/// trailing space: digits four left, noun right by coincidence. So the digits'
/// x is what catches that; the noun's x catches a fix that adds the lead and
/// keeps the invented space.
///
/// Ablated twice, each red on its own assertion:
/// * `Pen::count_with_noun`'s number put back to the old
///   `format!("{value} ")` — no lead, invented space: the digits are found at
///   **500** where 504 is expected, which is the defect as it shipped.
/// * `COUNT_SUFFIX` `""` → `" "`: the digits stay at 504 and the noun is found
///   at **551** where 547 is expected.
#[test]
fn the_treasury_s_digits_start_one_sign_column_right_of_ui_drawcount_s_x() {
    let (mut game, assets) = world!();
    let body = assets.shell.body.as_ref().expect("Fntl2_14.pl8");
    let mut screen = MapScreen::new();
    let canvas = draw(&mut screen, &mut game, &assets);

    const X: i32 = 500; // Ui_DrawCount(gold, 0, 500, 6, …)
    const Y: i32 = 6;
    const LEAD: i32 = 4; // Ui_DrawText: glyph-less '@' -> local_14 = 4
    const TRAILER: i32 = 4; // Ui_DrawText's last line: g_penAdvance + 4

    let gold = game.gold();
    let digits = gold.to_string();
    assert_eq!(
        find_on_row(&canvas, body, &digits, font::TEXT, Y),
        Some(X + LEAD),
        "the treasury's digits are not one sign column right of Ui_DrawCount's x"
    );

    let noun = assets
        .shell
        .text(l2_game::shell::COUNT_NOUN_GROUP, l2_game::shell::count_noun(gold, 0))
        .to_string();
    assert_eq!(
        find_on_row(&canvas, body, &noun, font::TEXT, Y),
        Some(X + LEAD + body.width(&digits) + TRAILER),
        "{noun:?} is not at x + g_penAdvance after \"@{digits}\""
    );
}

/// **`Court_Draw` (`0x00416925`) draws all four store values in
/// `&g_fontHeading`, and we drew them in the body face** — beside labels that
/// were already in heading, so the column read as two different typefaces.
///
/// ```c
/// Ui_DrawCount (g_realms[p].gold,     0, 0xE0, 0x72, &g_fontHeading, 0x3F);
/// Ui_DrawNumber(g_realms[p].iron, ' ', " ", 0xE0, 0x90, &g_fontHeading, 0x3F);
/// ```
///
/// The probe is rendered in the heading face and searched **on the painter's
/// own row**, so finding it is the claim *"this face, at this call site"*.
/// Both leads advance four — `'@'` for the count, `' '` for the number — which
/// is why both expectations are `0xE0 + 4`.
///
/// Ablated by passing `Face::Body` for the two values in `court.rs`: neither is
/// found on its row in the heading face.
#[test]
fn the_court_s_stores_are_drawn_in_the_heading_face_at_court_draw_s_x() {
    use l2_game::screens::court::CourtScreen;
    let (mut game, assets) = world!();
    let heading = assets.shell.heading.as_ref().expect("Fntl2_22.pl8");
    let (gold, iron) = {
        let r = &game.kingdom.realms[game.player as usize];
        (r.gold, r.iron)
    };
    let mut screen = CourtScreen::new();
    let canvas = draw(&mut screen, &mut game, &assets);

    const X: i32 = 0xE0; // Court_Draw's value column
    const LEAD: i32 = 4; // '@' and ' ' both: Ui_DrawText's local_14 = 4

    assert_eq!(
        find_on_row(&canvas, heading, &gold.to_string(), font::TEXT, 0x72),
        Some(X + LEAD),
        "the court's gold is not in the heading face at (0xE0, 0x72)"
    );
    assert_eq!(
        find_on_row(&canvas, heading, &iron.to_string(), font::TEXT, 0x90),
        Some(X + LEAD),
        "the court's iron is not in the heading face at (0xE0, 0x90)"
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
/// about
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
/// **The words that go with the mercenary marker**, which the map does not
/// carry and which nothing in this workspace drew.
///
/// A player who has a band standing in his town square has, on the map, a
/// 25 × 45 figure and no explanation. `TileInfo_Draw` (`0x0041C208`) is where
/// the original says what it is, and it is the **only** consumer of `L2.eng`
/// 30/59:
///
/// ```c
/// if ((g_pickedTileFlags & 0x80) == 0) {
///   if ((g_pickedTileFlags & 0x40) != 0 && county.mercenaryOffer != 0) {
///     Pl8_DrawFrameClipped(g_flagsSheet, 0x81, 0x32, row * 0x10 + 0x9c);
///     FUN_0040328e(0x1e, 0x3b, 0x68, row * 0x10 + 0xa0, 0x140, …);   /* 30/59 */
///   }
/// }
/// ```
///
/// `CLAUDE.md` rule 6. The probe below is rendered in the **same face the
/// painter uses**, so this cannot pass while the sentence is drawn in the 5 × 7
/// debug font, and the string itself is read out of the player's `L2.eng` — an
/// index off by one fails here and not on somebody's screen.
#[test]
fn the_tile_panel_says_why_there_is_a_band_in_the_town_square() {
    use l2_game::screens::info::{self, InfoScreen, Target};
    use l2_game::screens::map::MapScreen;

    let (mut game, assets) = world!();
    let body = assets.shell.body.as_ref().expect("Fntl2_14.pl8 is in the install");
    let county = (1..=game.kingdom.county_count as u8)
        .find(|&id| game.is_players(id))
        .expect("the player holds a county");
    let tile = {
        let ctx = Ctx { game: &mut game, assets: &assets };
        *MapScreen::town(&ctx, county).first().expect("the county has a town block")
    };

    // The indices are the arm's and the words are the player's.
    assert_eq!(
        assets.shell.text(info::TILE_GROUP, info::COUNTY_TOWN_HEADING),
        "County town.",
    );
    let sentence = assets.shell.text(info::TILE_GROUP, info::MERCENARIES_AVAILABLE).to_string();
    assert!(
        sentence.starts_with("Mercenaries are available for hire"),
        "L2.eng 30/59 is {sentence:?}, which is not the mercenary line",
    );

    game.kingdom.counties[county as usize].mercenary_offer = 0;
    let mut panel = InfoScreen::new(Target::Tile(tile));
    {
        let ctx = Ctx { game: &mut game, assets: &assets };
        assert_eq!(panel.county_town(&ctx), Some(county));
        // `FUN_0041BEFE`: row `0x11` with no offer, `0x0F` with one.
        assert_eq!(panel.layout(&ctx).row, 0x11);
        assert!(!panel.mercenary_offer(&ctx));
    }
    let quiet = draw(&mut panel, &mut game, &assets);
    // The heading is there with or without an offer, and it is in
    // `&g_fontHeading` — which is what the probe's face asserts.
    let headings = assets.shell.heading.as_ref().expect("Fntl2_22.pl8 is in the install");
    let title = assets.shell.text(info::TILE_GROUP, info::COUNTY_TOWN_HEADING).to_string();
    assert!(
        find_in(&quiet, headings, &title, font::TEXT).is_some(),
        "{title:?} is not on the tile panel in the heading face",
    );

    game.kingdom.counties[county as usize].mercenary_offer = 3;
    {
        let ctx = Ctx { game: &mut game, assets: &assets };
        assert_eq!(panel.layout(&ctx).row, 0x0F, "an offer buys the panel two more rows");
    }
    let loud = draw(&mut panel, &mut game, &assets);

    // **The first word, not the whole sentence.** `body_wrapped` breaks the line
    // at `0x140` pixels wherever that falls, so a probe for the whole string
    // would be asserting where the wrap lands as well as that the line is there.
    let word = sentence.split(' ').next().expect("a sentence has a first word");
    assert!(
        find_in(&loud, body, word, font::TEXT).is_some(),
        "{word:?} is not on the tile panel in Fntl2_14.pl8",
    );
    assert!(
        find_in(&quiet, body, word, font::TEXT).is_none(),
        "{word:?} is on the panel of a county with no offer",
    );
    // **And the marker itself, which the line above cannot see.** `loud` and
    // `quiet` differ in the panel's height as well as in this block, so a diff
    // count proves nothing about the 25 × 45 figure; the probe is the frame
    // blitted alone at the position the decompilation names, and every pixel it
    // writes has to be on the panel. `0x0F * 0x10 + 0x9C` are literals —
    // `DAT_00553D2C` for a town with an offer, and `TileInfo_Draw`'s own y.
    let marker = assets
        .map
        .flag_sheet(&l2_view::campaign::NEAR)
        .and_then(|s| s.frame(l2_view::campaign::MERCENARY_MARKER_FRAME))
        .expect("Flags1a.pl8 frame 0x81");
    let mut probe = Canvas::screen();
    probe.blit(&marker, info::MERC_MARKER_AT.0, 0x0F * 0x10 + info::MERC_MARKER_AT.1);
    let written = probe.pixels.iter().filter(|&&p| p != 0).count();
    assert!(written > 0, "frame 0x81 writes nothing");
    let wrong = probe
        .pixels
        .iter()
        .zip(loud.pixels.iter())
        .filter(|(&p, &q)| p != 0 && p != q)
        .count();
    assert_eq!(
        wrong, 0,
        "{wrong} of frame 0x81's {written} pixels are not on the panel at ({}, {})",
        info::MERC_MARKER_AT.0,
        0x0F * 0x10 + info::MERC_MARKER_AT.1,
    );
}

// ------------------------------------- Ui_DrawNumber, read call site by call site
//
// `Pen::number(…, blank_lead: bool)` had 25 call sites and was deleted once
// every one had been read against its original. These four are the cases that
// differ in what moves: the digits and the text after them (a one-space
// suffix), the digits alone while the text stays put (an empty suffix — the
// lost lead and the invented space cancelled), a year that gained its era, and
// a number that was in the wrong face.
//
// **Every expected x is built from literals out of the painters and out of
// `Ui_DrawText`** — the lead's 4, the suffix's 4, the trailer's 4 and the call
// site's own `x` — and from the widths of the strings being drawn, measured in
// the face the call site names. No constant of ours is in an expectation, so
// ablating `Pen`'s arithmetic cannot move the expectation with the code.

/// `Ui_DrawText`: a glyph-less lead character advances `local_14 = 4`.
const LEAD: i32 = 4;
/// `" "` as a suffix, the same advance.
const SPACE: i32 = 4;
/// `Ui_DrawText`'s last line, `g_penAdvance + 4`.
const TRAILER: i32 = 4;

fn own_county(game: &Game) -> u8 {
    (1..=game.kingdom.county_count as u8)
        .find(|&id| game.is_players(id))
        .expect("the local player holds a county")
}

/// [`find_on_row`], but only at or right of `from` — for a number that could
/// otherwise be found as the prefix of a longer number earlier on its row.
fn find_on_row_from(
    canvas: &Canvas,
    f: &Font,
    s: &str,
    colour: u8,
    y: i32,
    from: i32,
) -> Option<i32> {
    let mut probe = Canvas::new(canvas.width - from as usize, canvas.height);
    for py in 0..canvas.height {
        for px in from as usize..canvas.width {
            probe.set(px - from as usize, py, canvas.at(px, py));
        }
    }
    find_on_row(&probe, f, s, colour, y).map(|x| x + from)
}

/// **The castle's garrison is `"@40 "`, and *"troops."* is chained off it, so
/// both moved.**
///
/// ```c
/// /* Screen_CastleBuildPanel, 0x004198AA */
/// Ui_DrawNumber(g_castleGarrisonCap[sel], '@', &DAT_004D4160, 0xC, 0xE2, &g_fontBody, 0x3F);
/// Eng_DrawString(0x47, 0xC, g_penAdvance + 0xE, 0xE2, &g_fontBody, 0x3F);
/// ```
///
/// `DAT_004D4160` is one space. The old `Pen::number(…, true)` drew `"40 "`: the
/// space right by accident, the lead missing — digits **and** noun four left.
///
/// Ablated twice, each red on its own assertion:
/// * the old `pen.body(…, &format!("{cap} "))` restored at the call site — the
///   digits are found at **12** where 16 is expected, the defect as it shipped;
/// * suffix `" "` → `""` — the digits stay at 16 and the noun is found at **51**
///   where 55 is expected.
#[test]
fn the_castle_s_garrison_and_its_noun_both_start_one_sign_column_right() {
    use l2_game::screens::castle::CastleScreen;
    let (mut game, assets) = world!();
    let body = assets.shell.body.as_ref().expect("Fntl2_14.pl8");
    let county = own_county(&game);
    let mut screen = CastleScreen::new(county);
    let cap = {
        let ctx = Ctx { game: &mut game, assets: &assets };
        l2_kingdom::industry::garrison_cap(&ctx.game.kingdom.tables, screen.castle_type(&ctx))
    };
    let canvas = draw(&mut screen, &mut game, &assets);

    const X: i32 = 0x0C; // Ui_DrawNumber(cap, '@', " ", 0xC, 0xE2, …)
    const Y: i32 = 0xE2;
    const NOUN_X: i32 = 0x0E; // Eng_DrawString(71, 0xC, g_penAdvance + 0xE, …)

    let digits = cap.to_string();
    assert_eq!(
        find_on_row(&canvas, body, &digits, font::TEXT, Y),
        Some(X + LEAD),
        "the garrison's digits are not one sign column right of 0x0C"
    );
    let noun = assets.shell.text(71, 0x0C).to_string();
    assert!(!noun.is_empty(), "L2.eng 71/12 is the garrison's noun");
    assert_eq!(
        find_on_row(&canvas, body, &noun, font::TEXT, Y),
        Some(NOUN_X + LEAD + body.width(&digits) + SPACE + TRAILER),
        "{noun:?} is not at g_penAdvance + 0xE after \"@{digits} \""
    );
}

/// **The mercenary's price line: both numbers moved and neither noun did.**
///
/// ```c
/// /* Screen_RaiseArmy, 0x00418653 */
/// g_penAdvance = 0;
/// Ui_DrawNumber(price, '@', &DAT_004D40C8, 0x70, base + 0x7C, &g_fontBody, 0x3F);
/// Eng_DrawString(0x45, 0, g_penAdvance + 0x70, base + 0x7C, &g_fontBody, 0x3F);
/// Ui_DrawNumber(men / 2, '@', &DAT_004D40CC, g_penAdvance + 0x70, …);
/// Eng_DrawString(0x45, 1, g_penAdvance + 0x70, …);
/// ```
///
/// Both suffixes are NUL. The old `"{n} "` lost four at the front and added four
/// at the back, so the nouns landed right and the digits did not —
/// the nouns are asserted as well: a fix that adds the lead and keeps the
/// invented space moves them.
///
/// Ablated twice, each red on its own assertion:
/// * the price put back to the old `pen.body(…, &format!("{} ", price))` — found
///   at **112** where 116 is expected;
/// * the price's suffix `""` → `" "` — the price stays at 116 and *"crowns to
///   hire."* is found at **164** where 160 is expected.
#[test]
fn the_mercenary_price_line_moves_its_numbers_and_not_its_nouns() {
    use l2_game::screens::army::{self, RaiseArmyScreen};
    let (mut game, assets) = world!();
    let body = assets.shell.body.as_ref().expect("Fntl2_14.pl8");
    let county = own_county(&game);
    const BAND: u8 = 3;
    game.kingdom.counties[county as usize].mercenary_offer = BAND;
    let rules = &l2_kingdom::mercenary::ROSTER[BAND as usize];
    let mut screen = RaiseArmyScreen::new(county);
    let canvas = draw(&mut screen, &mut game, &assets);

    const X: i32 = 0x70;
    let y = army::base(true) + 0x7C;

    let price = rules.price.to_string();
    assert_eq!(
        find_on_row(&canvas, body, &price, font::TEXT, y),
        Some(X + LEAD),
        "the price is not one sign column right of 0x70"
    );
    let hire = assets.shell.text(0x45, 0).to_string();
    let hire_x = X + LEAD + body.width(&price) + TRAILER;
    assert_eq!(
        find_on_row_from(&canvas, body, &hire, font::TEXT, y, X),
        Some(hire_x),
        "{hire:?} is not at g_penAdvance + 0x70 after \"@{price}\""
    );
    let wages = (rules.men / 2).to_string();
    let wages_x = hire_x + body.width(&hire) + TRAILER + LEAD;
    assert_eq!(
        find_on_row_from(&canvas, body, &wages, font::TEXT, y, hire_x),
        Some(wages_x),
        "the wages are not one sign column after {hire:?}"
    );
    let seasonal = assets.shell.text(0x45, 1).to_string();
    assert_eq!(
        find_on_row_from(&canvas, body, &seasonal, font::TEXT, y, wages_x),
        Some(wages_x + body.width(&wages) + TRAILER),
        "{seasonal:?} is not at g_penAdvance + 0x70 after \"@{wages}\""
    );
}

/// **Every `&g_fontHeading` line of `UnitPanel_Draw` (`0x0041B19D`) is in the
/// heading face, where the call site puts it, inside the panel's own box — and
/// is not in the body face.**
///
/// We drew three of them in `Fntl2_14.pl8` through `Pen::eng` (the merchant's
/// and peasants' 31/0 and 31/5, and the transport's 31/2 at the others' place)
/// and five not at all: the transport's county, an army's name, and the
/// mercenary line's three pieces.
///
/// ```c
/// Ui_DrawBox(8, R * 0x10 + 0x20, 0x1c, 0x1b - R);
/// /* transport */  Eng_DrawString(0x1f, 2, 0x18, R * 0x10 + 0x30, &g_fontHeading, 0x3f);
///                  Eng_DrawString(100, unit[+0x167] + scen * 0x14, g_penAdvance + 0x18, …);
/// /* merchant, peasants */ Eng_DrawString(0x1f, local_20, 0x28, R * 0x10 + 0x40, …);
/// /* army */       Eng_DrawString(owner + 0x5d, unit.nameIndex, 0x28, R * 0x10 + 0x30, …);
/// /* own army */   Eng_DrawString(0x10, 0, 0x38, R * 0x10 + 0x130, …);         /* no band */
///                  Ui_DrawNumber(mercMen, '@', &DAT_004d422c, 0x38, R * 0x10 + 0x130, …);
///                  Eng_DrawString(0x10, mercBand, g_penAdvance + 0x38, …);
///                  Ui_DrawUnitNoun(mercMen, mercTroop * 2 + 0x34, g_penAdvance + 0x38, …);
/// ```
///
/// Every `R` and every position below is a literal out of those lines and
/// `FUN_0041BEFE`'s ladder; the only computed parts are the widths of strings
/// measured in the face the call names.
///
/// Ablated, each red here on its own:
/// * the merchant's heading back to `Face::Body` — *"Merchant."* is not in
///   `Fntl2_22.pl8` at (40, 304);
/// * the army-name draw deleted — *"The Foxes."* is not at (40, 80);
/// * `TRANSPORT_HEADING_AT` → `(0x28, 0x30)` — *"Supplies for"* found at 40, not 24;
/// * the mercenary count's `Face::Heading` → `Face::Body` — `"40"` is not at (60, 336).
#[test]
fn every_unit_panel_heading_is_in_the_heading_face_inside_its_box() {
    use l2_game::screens::info::{InfoScreen, Target};
    use l2_kingdom::unit::{Mercenaries, TroopType, Unit, UnitKind};
    let (mut game, assets) = world!();
    let body = assets.shell.body.as_ref().expect("Fntl2_14.pl8");
    let heading = assets.shell.heading.as_ref().expect("Fntl2_22.pl8");
    let county = own_county(&game);
    let player = game.player;
    let enemy = if player == 1 { 2 } else { 1 };
    let slot = game.map_slot;
    let text = |g: usize, i: usize| assets.shell.text(g, i).to_string();

    let unit = |kind: UnitKind, owner: u8| {
        let mut u = Unit::new(kind, owner, 10, 10);
        u.men = 100;
        u.county = county;
        u.home_county = county;
        u.owner_is_human = owner == player;
        u
    };
    let mut transport = unit(UnitKind::Transport, player);
    transport.cargo_county = county;
    let mut named = unit(UnitKind::Army, player);
    named.name_index = 3;
    let mut hired = unit(UnitKind::Army, player);
    hired.name_index = 4;
    hired.mercenaries = Some(Mercenaries { band: 3, troop: TroopType::Archer, men: 40 });
    let mut foe = unit(UnitKind::Army, enemy);
    foe.name_index = 7;

    let cases: Vec<(&str, Unit, i32)> = vec![
        ("merchant", unit(UnitKind::Merchant, 0), 0x0F),
        ("peasants", unit(UnitKind::PeasantMob, 0), 0x0F),
        ("transport", transport, 0x0F),
        ("own army, no band", named, 2),
        ("own army, a band", hired, 2),
        ("enemy army", foe, 0x12),
    ];
    for (label, u, row) in cases {
        let kind = u.kind;
        let id = game.kingdom.campaign.units.spawn(u).expect("a free slot");
        let mut panel = InfoScreen::new(Target::Unit(id));
        let canvas = draw(&mut panel, &mut game, &assets);

        let lines: Vec<(String, i32, i32)> = match (label, kind) {
            (_, UnitKind::Merchant) => vec![(text(0x1F, 0), 0x28, row * 16 + 0x40)],
            (_, UnitKind::PeasantMob) => vec![(text(0x1F, 5), 0x28, row * 16 + 0x40)],
            (_, UnitKind::Transport) => {
                let y = row * 16 + 0x30;
                let t = text(0x1F, 2);
                let name_x = 0x18 + heading.width(&t) + TRAILER;
                vec![(t, 0x18, y), (text(100, slot * 20 + county as usize), name_x, y)]
            }
            ("own army, no band", _) => vec![
                (text(0x5D + player as usize, 3), 0x28, row * 16 + 0x30),
                (text(0x10, 0), 0x38, row * 16 + 0x130),
            ],
            ("own army, a band", _) => {
                let y = row * 16 + 0x130;
                let band = text(0x10, 3);
                let band_x = 0x38 + LEAD + heading.width("40") + TRAILER;
                // Archer is troop 5, and 40 is plural: 0x34 + 5 * 2 + 1.
                let noun_x = band_x + heading.width(&band) + TRAILER;
                vec![
                    (text(0x5D + player as usize, 4), 0x28, row * 16 + 0x30),
                    ("40".to_string(), 0x38 + LEAD, y),
                    (band, band_x, y),
                    (text(8, 0x34 + 5 * 2 + 1), noun_x, y),
                ]
            }
            _ => vec![(text(0x5D + enemy as usize, 7), 0x28, row * 16 + 0x30)],
        };

        // `Ui_DrawBox(8, R * 0x10 + 0x20, 0x1C, 0x1B - R)`.
        let (left, top) = (8, row * 16 + 0x20);
        let (right, bottom) = (left + 0x1C * 16, top + (0x1B - row) * 16);
        for (s, x, y) in &lines {
            assert!(!s.is_empty(), "{label}: an L2.eng line the painter draws is empty");
            assert_eq!(
                find_on_row(&canvas, heading, s, font::TEXT, *y),
                Some(*x),
                "{label}: {s:?} is not in Fntl2_22.pl8 at ({x}, {y})",
            );
            assert!(
                *x >= left && x + heading.width(s) <= right && *y >= top && y + heading.height(s) <= bottom,
                "{label}: {s:?} at ({x}, {y}) is outside the panel's box ({left}, {top})-({right}, {bottom})",
            );
            // On the call site's own row: the troop grid draws the same group 8
            // nouns in body further up, which is right and is not this line.
            if !s.chars().all(|c| c.is_ascii_digit()) {
                assert_eq!(
                    find_on_row(&canvas, body, s, font::TEXT, *y),
                    None,
                    "{label}: {s:?} is on row {y} in the body face",
                );
            }
        }
        if label == "enemy army" {
            // The mercenary line is inside the ownership gate.
            let none = text(0x10, 0);
            assert_eq!(find_in(&canvas, heading, &none, font::TEXT), None, "{label}: {none:?}");
        }
    }
}

/// **The unit panel's *"Formed"* line is `Ui_DrawYear(…, style 0)`, which ends
/// in `L2.eng` 26/1 *"AD"* — and we drew the bare number.**
///
/// ```c
/// /* UnitPanel_Draw */
/// g_penAdvance = 0;
/// Eng_DrawString(0x1F, 0x14, 0x28, row * 0x10 + 0xA0, &g_fontBody, 0x3F);
/// Ui_DrawYear(unit.yearFormed, g_penAdvance + 0x28, row * 0x10 + 0xA0, 0);
/// /* Ui_DrawYear, 0x0041A900, style 0, year >= 0: */
/// Ui_DrawNumber(year, ' ', &DAT_004D41D8, x, y, &g_fontBody, 0x3F);
/// Eng_DrawString(0x1A, 1, x + g_penAdvance, y, &g_fontBody, 0x3F);
/// ```
///
/// `DAT_004D41D8` is one space, and the row is 2 for an army of the local
/// player's (`FUN_0041BEFE`). `CLAUDE.md` rule 6.
///
/// Ablated: the call site's style `0` → `3`, the bare number the panel used to
/// draw — *"AD"* is not found on the row (`None` where `Some(159)` is expected).
#[test]
fn the_unit_panel_says_the_year_an_army_was_formed_in_ad() {
    use l2_game::screens::info::{InfoScreen, Target};
    use l2_kingdom::unit::{Unit, UnitKind};
    let (mut game, assets) = world!();
    let body = assets.shell.body.as_ref().expect("Fntl2_14.pl8");
    let county = own_county(&game);
    const FORMED: i32 = 1271;
    let mut u = Unit::new(UnitKind::Army, game.player, 10, 10);
    u.men = 100;
    u.county = county;
    u.home_county = county;
    u.owner_is_human = true;
    u.year_formed = FORMED as _;
    let id = game.kingdom.campaign.units.spawn(u).expect("a free slot");
    let mut panel = InfoScreen::new(Target::Unit(id));
    let canvas = draw(&mut panel, &mut game, &assets);

    const ROW: i32 = 2;
    let y = ROW * 0x10 + 0xA0;
    let formed = assets.shell.text(0x1F, 0x14).to_string();
    let ad = assets.shell.text(0x1A, 1).to_string();
    assert_eq!(ad, "AD", "L2.eng 26/1");

    let year_x = 0x28 + body.width(&formed) + TRAILER;
    let digits = FORMED.to_string();
    assert_eq!(
        find_on_row(&canvas, body, &digits, font::TEXT, y),
        Some(year_x + LEAD),
        "the year is not one space right of g_penAdvance + 0x28"
    );
    assert_eq!(
        find_on_row_from(&canvas, body, &ad, font::TEXT, y, year_x),
        Some(year_x + LEAD + body.width(&digits) + SPACE + TRAILER),
        "{ad:?} is not after \" {digits} \" on the Formed line"
    );
}

// ------------------------------------------------ the far zoom's box of words

/// **The box at the far zoom was empty, and the original fills it.**
///
/// `Screen_DrawCampaign`'s (`0x0040F5FD`) zoom-2 arm, whole:
///
/// ```c
/// Ui_DrawBox(0, 0x19C, 0x1E, 4);
/// DAT_0058FE2C = 1;  g_penAdvance = 0;
/// Eng_DrawString(0x65, g_scenarioIndex, 0x40, 0x1A8, &g_fontHeading, 0x3F);
/// Eng_DrawString(0x22, 0,  g_penAdvance + 0x50, 0x1A8, &g_fontHeading, 0x3F);
/// Ui_DrawYear(g_year,      g_penAdvance + 0x60, 0x1A8, 1);
/// DAT_0058FE2C = 0;
/// Eng_DrawString(0x22, 1, 0x50, 0x1C6, &g_fontBody, 0x3F);
/// ```
///
/// We drew the box and, inside it, a status line of our own — which C173 then
/// gated behind the debug overlay, leaving the box blank. Group 34 has exactly
/// one consumer in the binary and it is this arm, so its two strings *are* this
/// box's vocabulary (`CLAUDE.md` rule 6), and the second of them is the game
/// saying what the far zoom is for.
///
/// **Ablated, one draw at a time:** removing any of the four turns exactly one
/// of the assertions below red.
#[test]
fn the_far_zoom_box_carries_the_map_name_the_year_and_the_instruction() {
    let (mut game, assets) = world!();
    let heading = assets.shell.heading.as_ref().expect("Fntl2_22.pl8 is in the install");
    let body = assets.shell.body.as_ref().expect("Fntl2_14.pl8 is in the install");
    let mut screen = MapScreen::new();
    // `Map_ToggleZoom` — the box exists only at zoom 2.
    {
        let mut ctx = Ctx { game: &mut game, assets: &assets };
        screen.handle(Event::KeyDown(Key::Char('Z')), &mut ctx);
    }
    let canvas = draw(&mut screen, &mut game, &assets);

    // The three heading draws run with `DAT_0058FE2C` set: capitals come out in
    // colour 1 whatever the caller passed,
    // mode as well as the face.
    let caps = Style { colour: font::TEXT, shadow: Some(font::SHADOW), caps: Some(1) };

    let name = assets.shell.text(101, game.map_slot).to_string();
    assert!(!name.is_empty(), "L2.eng group 101 has the sixty map names");
    let (nx, ny) = find_styled(&canvas, heading, &name, &caps)
        .unwrap_or_else(|| panic!("{name:?} is not in the far-zoom box"));
    assert_eq!((nx, ny), (0x40, 0x1A8), "Eng_DrawString(0x65, g_scenarioIndex, 0x40, 0x1A8)");

    let label = assets.shell.text(34, 0).to_string();
    assert_eq!(label, "Year", "L2.eng 34/0");
    let (lx, ly) = find_styled(&canvas, heading, &label, &caps).expect("{label:?} is not drawn");
    assert_eq!(ly, 0x1A8, "the label shares the map name's row");
    assert!(lx > nx + heading.width(&name), "the label is at {lx}, not after the name");

    // `Ui_DrawYear(…, 1)` puts group 26's era *before* the digits, both in the
    // heading face, and lifts an AD year by one pixel — `year < 0 ? y : y - 1`.
    // Finding the digits is the claim that the year was drawn.
    let digits = format!(" {} ", game.kingdom.year);
    let (yx, yy) = find_styled(&canvas, heading, &digits, &caps).expect("the year is not drawn");
    assert_eq!(yy, 0x1A8 - 1, "an AD year sits one pixel above its row");
    assert!(yx > lx, "the year is at {yx} and the label at {lx}");

    // And the instruction, in the body face with the drop capitals off again.
    let advice = assets.shell.text(34, 1).to_string();
    assert!(!advice.is_empty(), "L2.eng 34/1");
    let flat = Style { colour: font::TEXT, shadow: Some(font::SHADOW), caps: None };
    assert_eq!(
        find_styled(&canvas, body, &advice, &flat),
        Some((0x50, 0x1C6)),
        "Eng_DrawString(0x22, 1, 0x50, 0x1C6, &g_fontBody, 0x3F): {advice:?}"
    );
}

/// **`Screen_BattleMasterRatings` (`0x00421707`) draws each score in
/// `&g_fontHeading`** — the one heading-face figure on the page — and we drew it
/// in body. The lead and suffix, `' '` and one space, were already right.
///
/// ```c
/// Ui_DrawText(&g_playerNames[p], 0xD8, 0x6E, &g_fontBody, 0x3F);
/// Eng_DrawString(0x25, 4, g_penAdvance + 0xD8, 0x6E, &g_fontBody, 0x3F);
/// Ui_DrawNumber(score, ' ', &DAT_004D43B4, g_penAdvance + 0xEC, 0x69, &g_fontHeading, 0x3F);
/// ```
///
/// The name is `g_playerNames[realm]`, which this test writes so the pen
/// advance in front of the score is a known width; it is measured, not assumed.
///
/// Ablated: `Face::Heading` → `Face::Body` at the call site — the score is not
/// found on row `0x69` in the heading face (`None` where `Some(422)` is expected).
#[test]
fn the_battle_master_score_is_in_the_heading_face() {
    use l2_game::screens::ratings::{self, Ratings};
    let (mut game, assets) = world!();
    let body = assets.shell.body.as_ref().expect("Fntl2_14.pl8");
    let heading = assets.shell.heading.as_ref().expect("Fntl2_22.pl8");
    // The block is keyed by `g_localPlayer`'s realm and draws his name, so the
    // advance in front of the score is that name's width.
    game.player_names[Ratings::default().realms.0 as usize] =
        l2_game::text::PlayerName::new("PLAYER 1");
    let mut screen = ratings::RatingsScreen::new();
    let canvas = draw(&mut screen, &mut game, &assets);

    const NAME_X: i32 = 0xD8;
    const SCORE_X: i32 = 0xEC;
    const SCORE_Y: i32 = 0x69;
    let (mine, _) = ratings::score(&Ratings::default());
    let scored = assets.shell.text(0x25, 4).to_string();
    let advance = body.width("PLAYER 1") + TRAILER + body.width(&scored) + TRAILER;
    assert_eq!(
        find_on_row_from(&canvas, heading, &mine.to_string(), font::TEXT, SCORE_Y, NAME_X),
        Some(SCORE_X + advance + LEAD),
        "the score is not in the heading face at g_penAdvance + 0xEC, 0x69"
    );
}

/// **The fourth thing *Army foraging* gates is the unit panel's own lines**, and
/// ours drew none of them — not even the army's body line.
///
/// `UnitPanel_Draw` (`0x0041B19D`), the `kind == 1` arm: with `g_optArmiesEat`
/// off the body is one line at `row * 0x10 + 0x70`; with it on the body moves to
/// `+0x5E` and the supply state (31/23…26) and the starvation band
/// (31/27 + `+0x155`, red once the counter leaves zero) follow at `+0x72` and
/// `+0x86`. `docs/armies.md` §3.4 tabulates the supply strings.
///
/// Ablated: dropping the `armies_eat` branch leaves the body at `+0x70` with the
/// option on — the supply line is `None` at `+0x72`.
#[test]
fn army_foraging_moves_the_unit_panels_body_line_and_adds_two() {
    use l2_game::screens::info::{InfoScreen, Target};
    use l2_kingdom::unit::{Unit, UnitKind};
    let (mut game, assets) = world!();
    let body = assets.shell.body.as_ref().expect("Fntl2_14.pl8");
    let county = own_county(&game);
    let mut u = Unit::new(UnitKind::Army, game.player, 10, 10);
    u.men = 100;
    u.county = county;
    u.home_county = county;
    u.owner_is_human = true;
    u.starvation = 2;
    let id = game.kingdom.campaign.units.spawn(u).expect("a free slot");

    // `FUN_0041BEFE` puts a local player's army on row 2.
    const ROW: i32 = 2;
    let line = |i: usize| assets.shell.text(0x1F, i).to_string();
    // 31/16 is an own army's body, 31/23 "Fed in your county.", 31/29 the
    // third starvation band.
    let (text, fed, starving) = (line(0x10), line(0x17), line(0x1B + 2));
    assert!(!fed.is_empty() && !starving.is_empty(), "L2.eng 31/23 and 31/29");

    game.kingdom.options.armies_eat = false;
    let canvas = draw(&mut InfoScreen::new(Target::Unit(id)), &mut game, &assets);
    assert_eq!(
        find_on_row(&canvas, body, &text, font::TEXT, ROW * 0x10 + 0x70),
        Some(0x68),
        "with foraging off the army's body line is not at row * 0x10 + 0x70"
    );
    assert_eq!(
        find_on_row(&canvas, body, &fed, font::TEXT, ROW * 0x10 + 0x72),
        None,
        "the supply line is drawn with foraging off"
    );

    game.kingdom.options.armies_eat = true;
    let canvas = draw(&mut InfoScreen::new(Target::Unit(id)), &mut game, &assets);
    assert_eq!(
        find_on_row(&canvas, body, &text, font::TEXT, ROW * 0x10 + 0x5E),
        Some(0x68),
        "with foraging on the body line has not moved up to row * 0x10 + 0x5E"
    );
    assert_eq!(
        find_on_row(&canvas, body, &fed, font::TEXT, ROW * 0x10 + 0x72),
        Some(0x68),
        "{fed:?} is not the supply line at row * 0x10 + 0x72"
    );
    assert_eq!(
        find_on_row(&canvas, body, &starving, font::TEXT, ROW * 0x10 + 0x86),
        None,
        "the starvation line is drawn at 0x3F with the counter at 2"
    );
    assert_eq!(
        find_on_row(&canvas, body, &starving, 0xF9, ROW * 0x10 + 0x86),
        Some(0x68),
        "{starving:?} is not the starvation line in 0xF9 at row * 0x10 + 0x86"
    );
}

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

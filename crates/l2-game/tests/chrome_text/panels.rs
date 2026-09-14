#![allow(unused_imports)]
use super::*;
use super::top_bar::*;
use super::county_and_units::*;
use super::png_part::*;
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


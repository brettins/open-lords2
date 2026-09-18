#![allow(unused_imports)]
use super::*;
use super::drop_down_plate::*;
use super::title_and_build::*;
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

// **Every expected x is built from literals out of the painters and out of
// `Ui_DrawText`** — the lead's 4, the suffix's 4, the trailer's 4 and the call
// site's own `x` — and from the widths of the strings being drawn, measured in
// the face the call site names. No constant of ours is in an expectation, so
// ablating `Pen`'s arithmetic cannot move the expectation with the code.



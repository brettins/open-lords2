#![allow(unused_imports)]
use super::*;

use super::*;
use super::strip_and_sidebar::*;
use super::panels::*;
use super::drawing_and_emboss::*;
use super::layout_and_labels::*;
use common::*;
use l2_game::game::Assets;
use l2_game::game::MAX_TAX_RATE;
use l2_game::input::Event;
use l2_game::input::Key;
use l2_game::screen::Ctx;
use l2_game::screen::Machine;
use l2_game::screen::Screen;
use l2_game::screen::ScreenId;
use l2_game::screen::Transition;
use l2_game::screens::county::{self as county};
use l2_game::screens::county::CountyScreen;
use l2_game::screens::county::Panel;
use l2_game::screens::map;
use l2_game::screens::map::MapScreen;
use l2_game::screens::village::{self as village_screen};
use l2_game::screens::village::VillageScreen;
use l2_game::shell::font;
use l2_game::Game;
use l2_view::campaign;
use l2_view::Canvas;

#[test]
fn the_produce_rows_map_to_labour_slots_by_column_and_pitch() {
    let mut c = l2_kingdom::county::County::new();
    assert!(county::farm_rows(&c).is_empty());
    assert_eq!(county::job_row_at(&c, 500, 0x140), None, "an empty column refuses");

    // Cattle, then grain, then reclamation - `FUN_0040FEC1`'s own order, which
    // is not the labour slots' order.
    c.fields_cattle = 1;
    c.fields_grain = 1;
    assert_eq!(county::farm_rows(&c), vec![1, 0]);
    assert_eq!(county::farm_pitch(2), 0x3C, "two rows keep the tall pitch");
    assert_eq!(county::job_row_at(&c, 500, 0x12E), Some(1), "row 0 is the dairy");
    assert_eq!(county::job_row_at(&c, 500, 0x12E + 0x3C), Some(0), "row 1 is grain");
    assert_eq!(county::job_row_at(&c, 500, 0x12E + 0x78), None, "row 2 is past the end");

    c.fields_reclaiming = 1;
    assert_eq!(county::farm_rows(&c), vec![1, 0, 2]);
    assert_eq!(county::farm_pitch(3), 0x2D, "three rows close up");
    assert_eq!(county::job_row_at(&c, 500, 0x12E + 0x5A), Some(2), "reclamation, tight pitch");

    for i in &mut c.industry {
        i.enabled = false;
    }
    c.industry[0].enabled = true;
    c.industry[1].enabled = true;
    assert_eq!(county::industry_rows(&c), vec![6, 4], "wood then iron, not the array order");
    assert_eq!(county::job_row_at(&c, 0x230, 0x12E), Some(6), "x 0x230 is the industry column");
    assert_eq!(county::job_row_at(&c, 0x22F, 0x12E), Some(1), "and 0x22F is still the farm one");
    c.industry[3].enabled = true;
    c.industry[2].enabled = true;
    c.castle_degraded = 1;
    assert_eq!(county::industry_rows(&c), vec![6, 4, 5, 7, 3], "five rows, the castle last");
    assert_eq!(county::industry_pitch(5), 0x1E, "five rows get the 30-pixel pitch");
    assert_eq!(county::industry_pitch(3), 0x2D);
    assert_eq!(county::industry_pitch(2), 0x3C);

    assert_eq!(county::job_row_at(&c, 500, 0x12D), None, "one row above the plate");
    assert_eq!(county::job_row_at(&c, 500, 0x1AE), None, "one row below it");
    assert_eq!(county::job_row_at(&c, 477, 0x140), None, "one column left of the sidebar");
}

/// A player with the build in front of him: *"why do the pastures not have cows
/// in them?"* Because `FUN_004071A0`'s overlay pass had three of its four arms
/// and not the farm one.
#[test]
fn the_pastures_have_cattle_in_them_and_the_herd_chooses_which() {
    let (mut game, assets) = world!();

    // A real pasture of the England position, and the county that grazes it —
    // **one no AI lord re-lays at the head of the season.** `Season_Advance`
    // (`0x00448440`) opens with `Ai_ManageFarmsAll` (`0x0049A990`), which runs
    // every AI realm's farming style, and on this Winter save the arable and
    // mixed styles clear the fields and sow grain over them. That is the
    // original's behaviour and it would put grain on the tile claim 3 watches,
    // which says nothing about the herd. A person's county or a lordless one is
    // not touched by that pass, so the tile is left to `Herd_UpdateCrowding`,
    // which is what claim 3 is about.
    let (county, tile) = {
        let k = &game.kingdom;
        (1..=k.county_count)
            .filter(|&id| {
                let owner = k.counties[id].owner;
                owner == 0 || k.owner_is_human(owner)
            })
            .find_map(|id| {
                (0..l2_kingdom::MAX_FIELDS)
                    .filter_map(|s| k.counties[id].field_tile(s))
                    .find(|&t| {
                        l2_kingdom::field::classify(k.campaign.map.terrain[t])
                            == l2_kingdom::field::FieldType::Pasture
                    })
                    .map(|t| (id, t))
            })
            .expect("the England position has pastures")
    };
    let terrain = game.kingdom.campaign.map.terrain[tile];
    assert!(
        (0x14..=0x16).contains(&terrain),
        "the original's own save already carries a stocked pasture here, not {terrain:#04X}",
    );

    let (fx, fy) = l2_kingdom::map::coords(tile);
    let mut screen = MapScreen::new();
    screen.centre_on_tile(fx as usize, fy as usize);
    let (frame_index, (dx, dy)) =
        campaign::herd_sprite(terrain, 0).expect("a stocked pasture draws animals");

    // The first version of this test computed the probe pixel *from*
    // `campaign::HERD_AT` and then asserted the sprite was there — so setting
    // that constant to `(0, 0)` moved both sides of the comparison and the test
    // stayed green. A check passing for an accidental reason, caught by
    // ablating the exact line it claims to be about. `FUN_004071A0`'s farm arm
    // is `local_30 = 4; local_34 = -4;` for content `0x14 … 0x16`, and that
    // literal has to be written here or nothing anchors it.
    assert_eq!((dx, dy), (4, -4), "FUN_004071A0 offsets a stocked pasture by (+4, -4)");
    let sheet = assets.map.flag_sheet(screen.zoom()).expect("Flags1a.pl8");
    let sprite = sheet.frame(frame_index).expect("the herd frame");

    let (row, col) = campaign::tile_to_cell(fx as usize, fy as usize);
    let (sx, sy) = campaign::cell_to_screen(screen.viewport(), screen.zoom(), row, col);
    let (ox, oy) = (sx + dx, sy + dy);

    let probe = (0..sprite.opaque.len())
        .find(|&i| {
            let (px, py) = (i % sprite.width as usize, i / sprite.width as usize);
            sprite.opaque[i]
                && px > 2
                && py > 2
                && px + 3 < sprite.width as usize
                && py + 3 < sprite.height as usize
        })
        .expect("the herd sprite has an interior");
    let (px, py) = (probe % sprite.width as usize, probe / sprite.width as usize);
    let want = sprite.indices[probe];
    let (tx, ty) = ((ox + px as i32) as usize, (oy + py as i32) as usize);

    let mut with = Canvas::screen();
    {
        let ctx = Ctx { game: &mut game, assets: &assets };
        screen.draw(&ctx, &mut with);
    }
    assert_eq!(
        with.at(tx, ty),
        want,
        "the herd sprite is not at the tile origin plus {:?}",
        (dx, dy),
    );

    let mut without = Canvas::screen();
    {
        let slot = assets.slot(game.map_slot).expect("the map slot");
        let lattice = campaign::Lattice::build(&slot);
        let overrides = {
            let ctx = Ctx { game: &mut game, assets: &assets };
            MapScreen::tile_graphics(&ctx)
        };
        let mut tags = l2_view::Tags::screen();
        campaign::draw(
            &mut without,
            &slot,
            &lattice,
            &assets.map,
            screen.viewport(),
            screen.zoom(),
            &mut tags,
            &overrides,
            game.kingdom.season,
            None,
        );
    }
    assert_ne!(
        without.at(tx, ty),
        want,
        "the bare meadow already had this pixel, so the assertion above measures the artwork",
    );

    game.kingdom.counties[county].herd = 0;
    game.kingdom.advance_season();
    assert_eq!(
        game.kingdom.campaign.map.terrain[tile],
        l2_kingdom::field::terrain::PASTURE,
        "an empty herd leaves bare pasture",
    );
    assert!(
        campaign::herd_sprite(l2_kingdom::field::terrain::PASTURE, 0).is_none(),
        "and nothing is drawn on it",
    );
    let mut empty = Canvas::screen();
    {
        let ctx = Ctx { game: &mut game, assets: &assets };
        screen.draw(&ctx, &mut empty);
    }
    assert_ne!(empty.at(tx, ty), want, "the cattle are still on the map with no herd to draw");
}

#[test]
fn the_grazing_clock_changes_the_picture_and_cannot_change_the_world() {
    let (mut game, assets) = world!();
    let tile = {
        let k = &game.kingdom;
        (1..=k.county_count)
            .find_map(|id| {
                (0..l2_kingdom::MAX_FIELDS)
                    .filter_map(|s| k.counties[id].field_tile(s))
                    .find(|&t| (0x14..=0x16).contains(&k.campaign.map.terrain[t]))
            })
            .expect("a stocked pasture")
    };
    let (fx, fy) = l2_kingdom::map::coords(tile);
    let terrain = game.kingdom.campaign.map.terrain[tile];

    let frames: Vec<usize> = (0..campaign::HERD_PHASES)
        .map(|p| campaign::herd_sprite(terrain, p).expect("stocked").0)
        .collect();
    let mut distinct = frames.clone();
    distinct.sort_unstable();
    distinct.dedup();
    assert_eq!(distinct.len(), 6, "the six phases are six frames: {frames:?}");
    assert_eq!(
        campaign::herd_sprite(terrain, campaign::HERD_PHASES).expect("stocked").0,
        frames[0],
        "and the seventh wraps to the first",
    );

    let mut screen = MapScreen::new();
    screen.centre_on_tile(fx as usize, fy as usize);
    if let Some(sheet) = assets.map.flag_sheet(screen.zoom()) {
        let mut shapes: Vec<Vec<u8>> = frames
            .iter()
            .map(|&f| sheet.frame(f).expect("the frame").indices.clone())
            .collect();
        let before = shapes.len();
        shapes.sort();
        shapes.dedup();
        assert_eq!(shapes.len(), before, "the six phases are one picture repeated");
    }

    let before = l2_kingdom::save::checksum(&game.kingdom);
    for _ in 0..campaign::HERD_PHASES {
        let mut canvas = Canvas::screen();
        let ctx = Ctx { game: &mut game, assets: &assets };
        screen.draw(&ctx, &mut canvas);
    }
    assert_eq!(
        l2_kingdom::save::checksum(&game.kingdom),
        before,
        "drawing the map moved the simulation",
    );
}

/// `docs/decisions.md` C112.
#[test]
fn the_sovereign_lines_take_the_realms_shield_colour_and_follow_it() {
    let (mut game, assets) = world!();
    if assets.shell.body.is_none() {
        l2_testkit::skip!("no Fntl2_14.pl8, so there is nothing to read a pen off");
    }
    assert_eq!(game.kingdom.counties[1].owner, 5, "county 1 belongs to realm 5");
    let banner = assets.shell.text(15, 0).to_string();
    let banner = if banner.is_empty() { "SOVEREIGN LAND".to_string() } else { banner };
    // The third line, and it is the game's own name for realm 5's lord rather
    // than a `REALM 5` of ours — `message::lord_name`, shared with the court,
    // the battle prompt and the diplomacy screens, which answers out of the
    // save's player table before it falls back to `L2.eng` group 7.
    let lord = l2_game::screens::message::lord_name(&Ctx { game: &mut game, assets: &assets }, 5);
    assert!(!lord.is_empty(), "realm 5 is unnamed, so there is no third line to read a pen off");

    let mut seen = Vec::new();
    for shield in 1..=5u8 {
        game.kingdom.realms[5].shield_index = shield;
        let mut screen = CountyScreen::new(1, Panel::Tax);
        let canvas = draw(&mut screen, &mut game, &assets);
        let pen = l2_view::chrome::realm_pen(shield).expect("1..=5 has a pen");

        assert!(
            find_body(&canvas, &assets, &banner, pen).is_some(),
            "shield {shield}: the banner is not drawn in its pen {pen:#04X}"
        );
        for other in 1..=5u8 {
            let wrong = l2_view::chrome::realm_pen(other).expect("1..=5");
            if wrong == pen {
                continue;
            }
            assert!(
                find_body(&canvas, &assets, &banner, wrong).is_none(),
                "shield {shield}: the banner is also drawn in shield {other}'s pen {wrong:#04X}"
            );
        }
        assert!(
            find_body(&canvas, &assets, &lord, pen).is_some(),
            "shield {shield}: the lord's name {lord:?} is not in the same pen as the banner"
        );
        seen.push(pen);
    }
    seen.sort_unstable();
    seen.dedup();
    assert_eq!(seen.len(), 5, "five shields must give five different pens");

    game.kingdom.realms[5].shield_index = 5;
    let mut screen = CountyScreen::new(1, Panel::Tax);
    let canvas = draw(&mut screen, &mut game, &assets);
    let pen = l2_view::chrome::realm_pen(5).expect("shield 5");
    assert_eq!(
        emboss_at(&canvas, &assets, &banner, pen),
        Some(l2_game::shell::font::SHADOW_GREY),
        "the pen changed and the emboss did not"
    );
}

/// A player, mid-session: *"Sidebar doesn't show grain being planted as a
/// negative number."* `docs/draws-map.md` §5.10 has the diagnosis — the grain
/// row's value is county `+0x22C`, written by a tail of
/// `Grain_LabourEstimate` that this workspace did not have — and this is the
/// **cattle** row, whose value existed first
/// ([`l2_kingdom::land::herd_preview`] is `Herd_LabourEstimate`'s tail), and
/// which therefore proved the drawing half before the expensive half landed on
/// it.
///
/// `Ui_DrawDelta` (`0x00402E0C`) is asserted in the three ways it can be wrong,
/// and each is a different line of it:
///
/// The search is [`find_font_text`], so it is the **glyphs of the user's own
/// `Font_10.pl8`** — `FUN_004100AF`'s `&g_font10` — being matched at a colour,
/// not a description of them. It used to be `Fntl2_9.pl8`, because that face was
/// what we drew in. Claim 3 is the one that cannot pass by accident, because it
/// requires the *absence* of a pattern the same run has just proved the
/// renderer can draw.
#[test]
fn the_cattle_row_draws_its_forecast_with_a_sign() {
    let (mut game, assets) = world!();
    // `colourPos` and `colourNeg`, typed from the call site in `FUN_004100AF`
    // Not imported from the constants under test.
    const POS: u8 = 0xFA;
    const NEG: u8 = 0xF9;

    let county = 8;
    game.select(county as u8);
    // The row is only drawn when `FUN_0040FEC1` lists it, which is
    // `fieldsCattle != 0 || herd != 0`.
    game.kingdom.counties[county].fields_cattle = 4;
    game.kingdom.counties[county].herd = 400;

    let mut screen = MapScreen::new();
    let shown = |game: &mut Game, s: &str, colour: u8| -> Option<(i32, i32)> {
        let canvas = draw(&mut MapScreen::new(), game, &assets);
        let f = assets.shell.ten.as_ref().expect("Font_10.pl8");
        find_font_text(&canvas, f, s, colour)
    };
    let _ = &mut screen;

    game.kingdom.counties[county].herd_change_expected = -7;
    let neg = shown(&mut game, "-7 ", NEG).expect("a shrinking herd shows -7, in Font_10.pl8");
    assert_eq!(neg, (0x204 + 8, 0x139), "the cattle row is row 0 of the farm column");
    assert!(shown(&mut game, "-7 ", POS).is_none(), "a negative delta is 0xF9, not 0xFA");

    game.kingdom.counties[county].herd_change_expected = 7;
    let pos = shown(&mut game, "+7 ", POS).expect("a growing herd shows +7");
    assert_eq!(pos.1, neg.1, "both signs sit on the same row");
    assert!(shown(&mut game, "7 ", NEG).is_none(), "a positive delta is 0xFA, not 0xF9");

    game.kingdom.counties[county].herd_change_expected = 0;
    for s in ["+0 ", "-0 ", "0 "] {
        for c in [POS, NEG] {
            assert!(
                shown(&mut game, s, c).is_none(),
                "mode 0 with a zero value draws nothing at all, but {s:?} appeared in {c:#04x}",
            );
        }
    }
}

/// `ebf8dd5` drew the rows from `Industry::next_season`, and the field was
/// right — county `+0x2A8 + c*0x18`, the last word of commodity `c`'s own
/// record, `docs/decisions.md` C153. **Nothing put a number in
/// it on load.** The importer did not read the word, and the estimate round
/// that writes it runs at the end of a season, so a loaded game held all four
/// at zero in every county and `Ui_DrawDelta` with `mode == 0` draws a zero as
/// nothing. Measured on England turn one before the fix: fourteen counties,
/// fifty-six forecasts, all zero.
///
/// 1. **the file's own forecast is drawn, untouched**, in the wood row, which
///    `FUN_0040FEC1` always lists first when it is switched on;
/// 2. **four distinct numbers land on four rows in `FUN_0040FEC1`'s order** —
///    wood, iron, stone, weapons, the labour slots 6, 4, 5, 7 — at
///    `Ui_DrawDelta(…, 0x22C, pitch*row + 0x139, …)` in `colourPos`. A reading
///    one record along, which is what the old base implied, would put 22 on
///    the wood row.
#[test]
fn a_loaded_game_draws_each_industry_rows_own_forecast_on_its_first_frame() {
    let (mut game, assets) = world!();
    const POS: u8 = 0xFA;
    // `Ui_DrawDelta`'s x and the row's own y, from `FUN_00410502` and its three
    // siblings.
    const DELTA_X: i32 = 0x22C;
    const ICON_DY: i32 = 0x133;
    // `DAT_0053F04C`: four rows close up to 0x1E.
    let pitch = |rows: usize| if rows < 3 { 0x3C } else if rows < 4 { 0x2D } else { 0x1E };
    let found = |game: &mut Game, s: &str| -> Option<(i32, i32)> {
        let canvas = draw(&mut MapScreen::new(), game, &assets);
        let f = assets.shell.ten.as_ref().expect("Font_10.pl8");
        find_font_text(&canvas, f, s, POS)
    };
    let in_row = |at: (i32, i32), row: i32, pitch: i32| {
        at.0 >= DELTA_X
            && at.0 < 640
            && at.1 >= pitch * row + ICON_DY
            && at.1 < pitch * row + ICON_DY + pitch
    };

    let county = (1..=game.kingdom.county_count)
        .find(|&id| game.is_players(id as u8) && game.kingdom.counties[id].industry[0].next_season > 0)
        .expect("some county the player holds in England turn one stores a wood forecast");
    let wood = game.kingdom.counties[county].industry[0].next_season;
    game.select(county as u8);
    let rows = (0..4).filter(|&c| game.kingdom.counties[county].industry[c].enabled).count()
        + usize::from(game.kingdom.counties[county].castle_degraded != 0);
    let at = found(&mut game, &format!("+{wood} "))
        .unwrap_or_else(|| panic!("county {county} stores a wood forecast of {wood} and no +{wood} is drawn"));
    assert!(
        in_row(at, 0, pitch(rows)),
        "county {county}'s +{wood} is drawn at {at:?}, not in the wood row at x >= {DELTA_X}"
    );

    {
        let c = &mut game.kingdom.counties[county];
        for (i, v) in [11, 22, 33, 44].into_iter().enumerate() {
            c.industry[i].enabled = true;
            c.industry[i].next_season = v;
        }
        c.castle_degraded = 0;
    }
    for (row, value) in [(0, 11), (1, 22), (2, 44), (3, 33)] {
        let at = found(&mut game, &format!("+{value} "))
            .unwrap_or_else(|| panic!("+{value} is not drawn at all"));
        assert!(
            in_row(at, row, pitch(4)),
            "+{value} belongs on row {row} of four and is drawn at {at:?}"
        );
    }
}



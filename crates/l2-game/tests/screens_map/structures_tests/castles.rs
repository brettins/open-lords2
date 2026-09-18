#![allow(unused_imports)]
use super::*;
use super::minimap_and_seasons::*;
use super::*;
use super::view_tests::*;
use super::interaction_tests::*;
use super::fog_and_march_tests::*;
use common::*;
use l2_game::game::Assets;
use l2_game::input::Event;
use l2_game::input::Key;
use l2_game::screen::Ctx;
use l2_game::screen::Machine;
use l2_game::screen::Screen;
use l2_game::screen::ScreenId;
use l2_game::screen::Transition;
use l2_game::screens::county::Panel;
use l2_game::screens::map;
use l2_game::screens::map::MapScreen;
use l2_game::shell::font;
use l2_game::Game;
use l2_view::campaign;
use l2_view::chrome;
use l2_view::Canvas;

/// If any of the three stops being true the feature has gone, and this fails
/// instead of a person noticing weeks later. C59.
#[test]
fn the_county_town_flies_its_owners_flag_and_the_wave_advances() {
    let (mut game, assets) = world!();
    let county = (1..=game.kingdom.county_count as u8)
        .find(|&id| game.is_players(id))
        .expect("the player holds a county");
    game.select(county);

    let owner = game.kingdom.counties[county as usize].owner as usize;
    let shield = game.kingdom.realms[owner].shield_index;
    assert!(
        (1..=5).contains(&shield),
        "realm {owner} carries shield {shield}, which flies nothing"
    );

    let (mut screen, canvas) = town_view(&mut game, &assets, county);
    let sheet = assets.map.flag_sheet(screen.zoom()).expect("Flags1a.pl8 is in the install");
    let frame_of = |shield: u8, phase: u8| {
        let i = campaign::flag_frame(shield, phase).expect("a shield of 1 ..= 5 has a frame");
        sheet.frame(i).expect("Flags1a.pl8 holds forty 32 x 24 frames")
    };

    let f = frame_of(shield, 0);
    assert_eq!((f.width, f.height), (32, 24), "the first forty frames are 32 x 24");
    let (at, ink) = find_sprite(&canvas, &f)
        .expect("the county town flies its owner's flag, and it is not on the canvas");
    assert!(
        ink >= 200,
        "the flag matched on only {ink} opaque pixels, which is too few to be the flag"
    );

    assert!(at.0 < campaign::PANEL_X, "the flag is on the map, not in the sidebar");

    let other = if shield == 5 { 1 } else { shield + 1 };
    game.kingdom.realms[owner].shield_index = other;
    let (_, moved) = town_view(&mut game, &assets, county);
    assert_eq!(
        find_sprite(&moved, &frame_of(other, 0)).map(|(p, _)| p),
        Some(at),
        "with shield {other} the same pixel must fly shield {other}'s flag"
    );
    game.kingdom.realms[owner].shield_index = shield;

    for _ in 0..16 {
        let mut ctx = Ctx { game: &mut game, assets: &assets };
        let _ = screen.update(&mut ctx);
    }
    let waved = draw(&mut screen, &mut game, &assets);
    assert_eq!(
        find_sprite(&waved, &frame_of(shield, 1)).map(|(p, _)| p),
        Some(at),
        "sixteen ticks is one phase, so the same pixel must now fly phase 1"
    );
}

/// The second half of `FUN_004071A0`, and the half that is easy to get wrong by
/// reading the county instead of the unit standing in it:
///
/// The measurement is a **count**, not a position: the town of the same county
/// is flying a flag of its own a few tiles away, so what is asserted is that
/// taking the garrison out removes **exactly one** flag and leaves the other
/// standing. C59.
#[test]
fn a_garrisoned_castle_flies_the_garrisons_shield_and_an_empty_one_flies_nothing() {
    let (mut game, assets) = world!();
    let county = game
        .kingdom
        .county_ids()
        .find(|&id| game.is_players(id as u8))
        .expect("the player holds a county");
    game.select(county as u8);

    let owner = game.kingdom.counties[county].owner as usize;
    let town_shield = game.kingdom.realms[owner].shield_index;
    let garrison_shield = if town_shield == 5 { 1 } else { town_shield + 1 };

    let unit = game
        .kingdom
        .campaign
        .units
        .iter()
        .map(|(id, _)| id)
        .next()
        .expect("the fixture carries units");
    game.kingdom.campaign.units.get_mut(unit).expect("the slot exists").shield = garrison_shield;
    {
        let c = &mut game.kingdom.counties[county];
        c.castle_type = c.castle_type.max(1);
        c.garrison_unit = unit;
    }

    let mut screen = MapScreen::new();
    {
        let c = &game.kingdom.counties[county];
        screen.centre_on_tile(c.anchor_x as usize, c.anchor_y as usize);
    }
    let held = draw(&mut screen, &mut game, &assets);

    let sheet = assets.map.flag_sheet(screen.zoom()).expect("Flags1a.pl8 is in the install");
    let frame_of = |shield: u8| {
        sheet
            .frame(campaign::flag_frame(shield, 0).expect("a shield of 1 ..= 5 has a frame"))
            .expect("Flags1a.pl8 holds forty 32 x 24 frames")
    };
    let castle_flag = frame_of(garrison_shield);
    let town_flag = frame_of(town_shield);

    let (before, ink) = sprite_positions(&held, &castle_flag);
    assert!(
        !before.is_empty(),
        "the garrisoned castle of county {county} flies no flag: shield {garrison_shield}"
    );
    assert!(ink >= 200, "matched on {ink} opaque pixels, too few to be a flag");
    let towns_before = sprite_positions(&held, &town_flag).0.len();

    game.kingdom.counties[county].garrison_unit = 0;
    let empty = draw(&mut screen, &mut game, &assets);
    assert!(
        sprite_positions(&empty, &castle_flag).0.is_empty(),
        "an empty castle must fly nothing, and shield {garrison_shield}'s flag is still there"
    );
    assert_eq!(
        sprite_positions(&empty, &town_flag).0.len(),
        towns_before,
        "and the town's own flag reads the county's owner, so it must not have moved"
    );

    {
        let c = &mut game.kingdom.counties[county];
        c.castle_type = 0;
        c.garrison_unit = unit;
    }
    let bare = draw(&mut screen, &mut game, &assets);
    assert!(
        sprite_positions(&bare, &castle_flag).0.is_empty(),
        "`content <= 0x14` is the bare plot, and an unbuilt castle flies nothing"
    );
}

/// **A besieged castle carries the besieger's camp mark and the seasons he has
/// left** — `FUN_00407F82` (`0x00407F82`), called from `Sprite_TopIt`'s castle
/// arm before the garrison's banner.
///
/// ```c
/// besieger = g_units[county.garrisonUnit].besiegedBy;
/// if (besieger != 0 && g_mapZoom == 0)
///     FUN_00407f82(g_units[besieger].siegeSeasonsLeft, 8, -0x38);
/// ```
///
/// Two pictures, and the assertion is the pair: `Flags1a.pl8` frame `0x82`
/// (24 × 28, the sheet's **last** frame — it holds 131) at `(+8, −0x38)` from
/// the castle tile's origin, and the count centred in that frame's own width
/// ten pixels lower, flat and in `0xF9`. Neither is reachable from a fixture,
/// so the siege is staged here: the link the game keeps is on the *garrison*,
/// `+0x19A`, and the number is on the besieger, `+0x19C`.
///
/// **Ablated**: dropping either draw, moving the mark to the flag's own
/// `(+0x1A, −0x1C)`, putting the count on the garrison instead of the besieger,
/// or centring it in anything but frame `0x82`'s width turns this red.
#[test]
fn a_besieged_castle_carries_the_besiegers_mark_and_his_seasons_left() {
    let (mut game, assets) = world!();
    let county = game
        .kingdom
        .county_ids()
        .find(|&id| game.is_players(id as u8))
        .expect("the player holds a county");
    game.select(county as u8);

    let mut ids = game.kingdom.campaign.units.iter().map(|(id, _)| id);
    let garrison = ids.next().expect("the fixture carries units");
    let besieger = ids.next().expect("the fixture carries a second unit");
    drop(ids);
    const SEASONS: u8 = 4;
    {
        let c = &mut game.kingdom.counties[county];
        c.castle_type = c.castle_type.max(1);
        c.garrison_unit = garrison;
    }
    game.kingdom.campaign.units.get_mut(besieger).expect("the slot exists").siege_seasons_left =
        SEASONS;

    let castle = {
        let ctx = Ctx { game: &mut game, assets: &assets };
        let terrain = ctx.game.kingdom.campaign.map.terrain.clone();
        MapScreen::settlements_for_test(&ctx, county as u8)
            .into_iter()
            .find(|&t| {
                l2_kingdom::industry::map_toggle_for_graphic(terrain[t])
                    == Some(l2_kingdom::industry::MapToggle::Castle)
            })
            .expect("the county's castle plot is on the map")
    };
    let (cx, cy) = l2_kingdom::map::coords(castle);

    let mut screen = MapScreen::new();
    draw(&mut screen, &mut game, &assets);
    screen.centre_on_tile(cx as usize, cy as usize);
    let free = draw(&mut screen, &mut game, &assets);

    let mark = assets
        .map
        .flag_sheet(screen.zoom())
        .and_then(|s| s.frame(campaign::BESIEGER_MARKER_FRAME))
        .expect("Flags1a.pl8 frame 0x82");
    assert_eq!((mark.width, mark.height), (24, 28), "frame 0x82 is the 24 x 28 camp mark");
    assert!(
        sprite_positions(&free, &mark).0.is_empty(),
        "an unbesieged castle carries no camp mark"
    );

    // `+0x19A` on the *garrison* is the link, and it is what the arm reads.
    game.kingdom.campaign.units.get_mut(garrison).expect("the slot exists").besieged_by =
        besieger as u8;
    let besieged = draw(&mut screen, &mut game, &assets);

    let (row, col) = campaign::tile_to_cell(cx as usize, cy as usize);
    let (sx, sy) = campaign::cell_to_screen(screen.viewport(), screen.zoom(), row, col);
    let (mx, my) = (sx + screen.zoom().besieger_at.0, sy + screen.zoom().besieger_at.1);

    let mut reference = free.clone();
    reference.blit_clipped(&mark, mx, my, screen.map_clip());
    // `Ui_DrawNumberRight(seasons, ' ', " ", x, y + 10, frameWidth, &g_fontBody,
    // 0xF9)` — which centres, and is flat because `DAT_005AEA40` is set for it.
    let count = format!(" {SEASONS} ");
    let style = l2_game::shell::font::Style {
        colour: campaign::BESIEGER_COUNT_INK,
        shadow: None,
        caps: None,
    };
    let body = assets.shell.body.as_ref().expect("Fntl2_14.pl8 is in the install");
    let ny = my + campaign::BESIEGER_COUNT_DY;
    assert!(campaign::BESIEGER_COUNT_TOP < ny, "the count clears the menu bar on this tile");
    body.draw_centred(&mut reference, mx, ny, i32::from(mark.width), &count, &style);

    let moved = |a: &Canvas, b: &Canvas| -> Vec<usize> {
        a.pixels
            .iter()
            .zip(b.pixels.iter())
            .enumerate()
            .filter(|(_, (p, q))| p != q)
            .map(|(i, _)| i)
            .collect()
    };
    let corner = |v: &[usize]| -> (usize, usize, usize, usize, usize) {
        let xs = v.iter().map(|i| i % besieged.width);
        let ys = v.iter().map(|i| i / besieged.width);
        (
            xs.clone().min().unwrap_or(0),
            ys.clone().min().unwrap_or(0),
            xs.max().unwrap_or(0),
            ys.max().unwrap_or(0),
            v.len(),
        )
    };
    let (drawn, expected) = (moved(&besieged, &free), moved(&reference, &free));
    assert!(!expected.is_empty(), "frame 0x82 writes nothing at the place the literals name");
    assert_eq!(
        corner(&drawn),
        corner(&expected),
        "the siege's ink (x0, y0, x1, y1, count) is not frame 0x82 at ({mx}, {my}) with the \
         count centred in its 24 pixels at ({mx}, {ny})"
    );
    assert_eq!(drawn, expected, "the siege's ink is the right size in the right place and is not the same pixels");
    let off = ((i32::from(mark.width) - body.width(&count)) / 2).max(0);
    assert_eq!(
        find_body(&besieged, &assets, &count, campaign::BESIEGER_COUNT_INK),
        Some((mx + off, ny)),
        "the seasons left are the besieger's `+0x19C`, centred in frame 0x82's own width"
    );

    // `Sprite_TopIt` calls `FUN_00407F82(…, 2, -0x28)` at `g_mapZoom == 2` and
    // the whole of that function is inside `if (g_mapZoom == 0)`, so the call
    // returns having drawn nothing. `docs/bugs.md`.
    let mut far = MapScreen::new();
    draw(&mut far, &mut game, &assets);
    send(&mut far, &mut game, &assets, Event::KeyDown(Key::Char('Z')));
    assert_eq!(far.zoom().id, campaign::FAR.id, "Z is the zoom toggle");
    let zoomed = draw(&mut far, &mut game, &assets);
    if let Some(far_mark) =
        assets.map.flag_sheet(far.zoom()).and_then(|s| s.frame(campaign::BESIEGER_MARKER_FRAME))
    {
        assert!(
            sprite_positions(&zoomed, &far_mark).0.is_empty(),
            "the far zoom's call site is dead in the original and must be dead here"
        );
    }

    game.kingdom.campaign.units.get_mut(garrison).expect("the slot exists").besieged_by = 0;
    let lifted = draw(&mut screen, &mut game, &assets);
    assert_eq!(lifted.diff_count(&free), 0, "no siege, no mark and no count");
}


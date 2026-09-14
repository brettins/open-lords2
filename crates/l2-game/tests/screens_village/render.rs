#![allow(unused_imports)]
use super::*;
use super::interaction::*;
use super::animation::*;
use common::*;
use l2_game::input::Event;
use l2_game::input::Key;
use l2_game::screen::Ctx;
use l2_game::screen::Machine;
use l2_game::screen::ScreenId;
use l2_game::screen::Transition;
use l2_game::screens::map;
use l2_game::screens::village::{self as village_screen};
use l2_game::screens::village::VillageScreen;
use l2_view::chrome;
use l2_view::village;
use l2_view::Canvas;

/// **Two independent sources agreeing.** `g_jobClusterOrigins` is eight pairs
/// of integers in `Lords2.exe`'s `.data`; `vill_gd8.pl8` is a painted 45 x 40
/// mask in a file. Nothing connects them but the screen they describe — and
/// every cluster's own origin lands in that cluster's painted region, and all
/// eight regions are painted.
///
/// A misread origin, a misread grid stride, or the wrong 24-byte header offset
/// would each break this, and none of them could break it in a way that still
/// named all eight clusters correctly.
#[test]
fn the_painted_drop_grid_agrees_with_the_cluster_origins_in_the_executable() {
    let (_game, assets) = world!();
    let art = assets.village.as_ref().expect("vill.pl8 and vill_gd8.pl8");
    assert!(art.has_grid(), "vill_gd8.pl8 is 1,824 bytes: 24 of header and 45 x 40 of grid");
    let top = village::SCENE_Y;

    let mut seen = [0usize; village::CLUSTER_COUNT + 1];
    for row in 0..village::GRID_ROWS as i32 {
        for col in 0..village::GRID_COLS as i32 {
            let x = village::SCENE_X + col * village::GRID_CELL;
            let y = top + row * village::GRID_CELL;
            seen[art.cluster_at(x, y, top)] += 1;
        }
    }
    for cluster in 1..=village::CLUSTER_COUNT {
        assert!(seen[cluster] > 0, "cluster {cluster} has no painted region at all");
    }
    assert!(seen[0] > 0, "and there is ground that belongs to nobody");

    for cluster in 0..village::CLUSTER_COUNT {
        let (ox, oy) = village::cluster_origin(cluster, top);
        // The origin is the grid's top-left corner; the cluster's own middle is
        // two icons right and two rows down, which is where the artwork puts
        // the building the peasants stand at.
        let (mx, my) = (ox + 36, oy + 24);
        assert_eq!(
            art.cluster_at(mx, my, top),
            cluster + 1,
            "cluster {cluster}'s own middle ({mx}, {my}) is painted as {}",
            art.cluster_at(mx, my, top)
        );
    }
}

/// The village drawn against the shipped save: the picture is there, and so are
/// the icons standing on it.
#[test]
fn the_village_draws_the_picture_and_the_people_on_it() {
    let (mut game, assets) = world!();
    let county = (1..=game.kingdom.county_count as u8)
        .find(|&id| game.is_players(id))
        .expect("the player holds a county");
    let mut screen = VillageScreen::new(county);
    let canvas = draw(&mut screen, &mut game, &assets);

    // The scene is a *picture*, not a fill: it uses many palette indices.
    let top = village::SCENE_Y;
    let mut seen = [false; 256];
    for y in top..top + village::SCENE_H {
        for x in village::SCENE_X..village::SCENE_X + village::SCENE_W {
            seen[canvas.at(x as usize, y as usize) as usize] = true;
        }
    }
    let colours = seen.iter().filter(|&&s| s).count();
    assert!(colours > 32, "vill.pl8 frame 0 drew in {colours} palette indices");

    // Every cluster the county staffs has ink where its icons go.
    let c = &game.kingdom.counties[county as usize];
    let icons = VillageScreen::icons(c);
    let mut clusters_with_people = 0;
    for cluster in 0..village::CLUSTER_COUNT {
        if icons[cluster].iter().all(|&v| v == 0) {
            continue;
        }
        clusters_with_people += 1;
    }
    assert!(
        clusters_with_people >= 2,
        "county {county} staffs {clusters_with_people} clusters; the save has cattle and wood"
    );
}

/// **A county with a mine draws a mine, and a county with a quarry draws a
/// quarry — and neither draws the other.**
///
/// A player reported *"a county that clearly has iron has no iron mine in the
/// town centre"*. Two things were wrong at once and this asserts both:
///
/// * `Village_Draw` (`0x00412143`) blits three buildings out of `villani2.pl8`,
///   each gated on `county.industry[c].hasResource` — frame `0x29` at
///   `(0xac, top + 0xe5)` for wood, `0x28` at `(0x4c, top + 0x0c)` for stone
///   and `0x2b` at the *same spot* for iron. None of the three was drawn.
/// * Every county claimed all four resources
///   the mine could not have been chosen even if it had been drawn.
///
/// The check is exact: the shared spot is compared against the frame it should
/// be holding, pixel for pixel, and against the *other* county's frame, which
/// must differ. County 5 of England has neither, and its spot must hold neither.
#[test]
fn the_village_draws_the_mine_for_an_iron_county_and_the_quarry_for_a_stone_one() {
    let (mut game, assets) = world!();
    let art = assets.village.as_ref().expect("the village artwork");
    let top = village::SCENE_Y;

    // The three buildings, as `Village_Draw` has them.
    let (stone_slot, stone_frame, bx, by) = village::RESOURCE_BUILDINGS[1];
    let (iron_slot, iron_frame, ix, iy) = village::RESOURCE_BUILDINGS[2];
    assert_eq!((stone_slot, iron_slot), (3, 1), "stone is industry 3 and iron is industry 1");
    assert_eq!((bx, by), (ix, iy), "and they are drawn at the same spot");

    // A county of each kind, from the *save*.
    let kind = |id: usize| {
        let c = &game.kingdom.counties[id];
        (c.industry[1].has_resource, c.industry[3].has_resource)
    };
    let ids: Vec<usize> = game.kingdom.county_ids().collect();
    let iron = ids.iter().copied().find(|&id| kind(id) == (true, false)).expect("a mine county");
    let stone = ids.iter().copied().find(|&id| kind(id) == (false, true)).expect("a quarry county");
    let neither = ids.iter().copied().find(|&id| kind(id) == (false, false));
    assert!(
        ids.iter().all(|&id| kind(id) != (true, true)),
        "no county holds both, so the mine never covers the quarry"
    );

    // What each one *should* look like: the scene, the building, then
    // `Village_Animate`'s overlays at the frame a freshly opened village shows
    // — which is the order `Village_Draw` and `Screen_DrawWidgets` paint in,
    // and the reason the overlays are in this reference at all is that the iron
    // mine's own overlay lands inside the sampled square.
    let reference = |frame: Option<usize>| {
        let mut c = Canvas::screen();
        art.draw_scene(&mut c, top);
        let has = [false, frame == Some(iron_frame), false, frame == Some(stone_frame)];
        if frame.is_some() {
            art.draw_resources(&mut c, has, top);
        }
        art.draw_animations(&mut c, has, top, &village::AnimationClock::new());
        c
    };
    let region = |c: &Canvas| {
        let mut out = Vec::new();
        for y in by + top..by + top + 96 {
            for x in bx..bx + 96 {
                out.push(c.at(x as usize, y as usize));
            }
        }
        out
    };

    let mine = region(&reference(Some(iron_frame)));
    let pit = region(&reference(Some(stone_frame)));
    let bare = region(&reference(None));
    assert_ne!(mine, pit, "the mine and the quarry are different pictures");
    assert_ne!(mine, bare, "and the mine is not the bare scene");

    for (id, want, what) in [(iron, &mine, "a mine"), (stone, &pit, "a quarry")] {
        let mut screen = VillageScreen::new(id as u8);
        let drawn = draw(&mut screen, &mut game, &assets);
        assert_eq!(&region(&drawn), want, "county {id} must show {what}");
    }
    if let Some(id) = neither {
        let mut screen = VillageScreen::new(id as u8);
        let drawn = draw(&mut screen, &mut game, &assets);
        assert_eq!(region(&drawn), bare, "county {id} has neither and must show neither");
    }
}

/// **The village is an inset, and this is the test that says so.**
///
/// A player opened the game, clicked the town square, and reported a dialogue
/// with the map still visible around it. He was right; this file's earlier
/// reading — "its own case in `Screen_Draw`, therefore a full screen" — was
/// wrong (`docs/decisions.md` C22).
///
/// Painted onto a canvas of a marker colour, the village must leave the marker
/// showing everywhere outside the 480 × 320 band `Village_Draw` saves at
/// (0, `g_villageTopY`) — and in particular across the whole menu bar and all
/// but the first two columns of the county sidebar.
#[test]
fn the_village_paints_an_inset_and_leaves_the_rest_of_the_screen_alone() {
    let (mut game, assets) = world!();
    let county = (1..=game.kingdom.county_count as u8)
        .find(|&id| game.is_players(id))
        .expect("the player holds a county");
    let mut screen = VillageScreen::new(county);

    const MARKER: u8 = 0xAB;
    let mut canvas = Canvas::screen();
    canvas.clear(MARKER);
    {
        let ctx = Ctx { game: &mut game, assets: &assets };
        screen.draw(&ctx, &mut canvas);
    }

    let top = village::SCENE_Y;
    let band = |x: i32, y: i32| {
        (village::BAND_X..village::BAND_X + village::BAND_W).contains(&x)
            && (top..top + village::BAND_H_SAVED).contains(&y)
    };
    let mut escaped = Vec::new();
    for y in 0..480 {
        for x in 0..640 {
            if !band(x, y) && canvas.at(x as usize, y as usize) != MARKER {
                escaped.push((x, y));
            }
        }
    }
    assert!(
        escaped.is_empty(),
        "{} pixels painted outside the band, first at {:?}",
        escaped.len(),
        escaped.first()
    );

    // The two things the player can
    // see: the menu bar and the sidebar are untouched.
    for x in 0..640 {
        for y in 0..chrome::PANEL_TOP_Y {
            assert_eq!(canvas.at(x as usize, y as usize), MARKER, "menu bar at ({x}, {y})");
        }
    }
    for x in village::BAND_X + village::BAND_W..640 {
        for y in 0..480 {
            assert_eq!(canvas.at(x as usize, y as usize), MARKER, "sidebar at ({x}, {y})");
        }
    }
    // And the picture itself did get painted, so this is not passing by drawing
    // nothing at all.
    let mid = (village::SCENE_X + village::SCENE_W / 2) as usize;
    let painted = (top..top + village::SCENE_H)
        .filter(|&y| canvas.at(mid, y as usize) != MARKER)
        .count();
    assert!(painted > 200, "only {painted} of 320 rows of the picture were painted");
}

/// And the machine paints what is underneath first.
/// `Village_Draw` does for itself by calling `Map_DrawFrame`.
///
/// With the campaign map on the stack and the village pushed on top, the
/// sidebar — which the village never touches — comes from the map screen.
#[test]
fn the_machine_paints_the_campaign_map_under_the_village() {
    let (mut game, assets) = world!();
    let county = (1..=game.kingdom.county_count as u8)
        .find(|&id| game.is_players(id))
        .expect("the player holds a county");
    game.select(county);

    let mut machine = Machine::new(ScreenId::Campaign);
    let mut canvas = Canvas::screen();
    {
        let mut ctx = Ctx { game: &mut game, assets: &assets };
        machine.handle(Event::KeyDown(Key::letter('v')), &mut ctx);
    }
    assert_eq!(
        machine.ids(),
        vec![ScreenId::Campaign, ScreenId::Village(county)],
        "V on an owned county opens its village"
    );
    {
        let ctx = Ctx { game: &mut game, assets: &assets };
        machine.draw(&ctx, &mut canvas);
    }

    // The end-turn strip is the map screen's, at the bottom of the sidebar, and
    // the village cannot reach it.
    let mut sidebar = [false; 256];
    for y in chrome::PANEL_TOP_Y..480 {
        for x in 490..639 {
            sidebar[canvas.at(x as usize, y as usize) as usize] = true;
        }
    }
    let colours = sidebar.iter().filter(|&&s| s).count();
    assert!(colours > 8, "the sidebar under the village drew in {colours} indices");

    // And the village really is on top of it in the middle.
    let mid = (village::SCENE_X + village::SCENE_W / 2) as usize;
    let row = (village::SCENE_Y + village::SCENE_H / 2) as usize;
    let with_village = canvas.at(mid, row);
    let mut bare = Canvas::screen();
    {
        let mut only_map = Machine::new(ScreenId::Campaign);
        let ctx = Ctx { game: &mut game, assets: &assets };
        only_map.draw(&ctx, &mut bare);
    }
    assert_ne!(with_village, bare.at(mid, row), "the picture is over the map, not beside it");
}


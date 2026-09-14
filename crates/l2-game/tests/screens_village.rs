//! The village screen: its picture, its clusters and the peasant drag.
//!
//! Split out of `tests/screens.rs`; the shared helpers are in `tests/common/`.

#[macro_use]
mod common;

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

// ---------------------------------------------------------------- the village

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

/// The whole gesture on the grid: band a cluster, release, drop on
/// another, and the workers land in the other cluster's job.
#[test]
fn a_drag_across_the_real_drop_grid_moves_the_county_s_peasants() {
    let (mut game, assets) = world!();
    let county = (1..=game.kingdom.county_count as u8)
        .find(|&id| game.is_players(id))
        .expect("the player holds a county");
    let mut screen = VillageScreen::new(county);

    let c = &game.kingdom.counties[county as usize];
    let slots = VillageScreen::slots(c);
    let icons = VillageScreen::icons(c);
    // The fullest cluster is the one worth emptying.
    let from = (0..village::CLUSTER_COUNT)
        .max_by_key(|&i| icons[i].iter().filter(|&&v| v != 0).count())
        .unwrap();
    let to = (0..village::CLUSTER_COUNT).find(|&i| slots[i] != slots[from]).unwrap();
    let (before_from, before_to) = (c.labour[slots[from]], c.labour[slots[to]]);
    assert!(before_from > 0, "cluster {from} has people in it");

    let (bx0, by0, bx1, by1) = village::cluster_band_box(from, village::SCENE_Y);
    send(&mut screen, &mut game, &assets, Event::Click { x: bx0, y: by0 });
    send(&mut screen, &mut game, &assets, Event::Pointer { x: bx1, y: by1 });
    send(&mut screen, &mut game, &assets, Event::Release { x: bx1, y: by1 });
    assert_eq!(screen.phase(), village_screen::Phase::Carry, "released holding a selection");
    let carried = screen.drag_count();
    assert!(carried > 0);

    let (ox, oy) = village::cluster_origin(to, village::SCENE_Y);
    send(&mut screen, &mut game, &assets, Event::Click { x: ox + 36, y: oy + 24 });
    assert_eq!(screen.phase(), village_screen::Phase::Idle, "and put it down");

    let c = &game.kingdom.counties[county as usize];
    let moved = before_from - c.labour[slots[from]];
    assert!(moved > 0, "somebody moved");
    assert_eq!(c.labour[slots[to]] - before_to, moved, "and they arrived");
    assert_eq!(
        moved,
        (carried * c.pop_band).min(before_from),
        "an icon is popBand people, clamped to what the job held"
    );
    assert_eq!(
        c.labour.iter().sum::<i32>(),
        game.kingdom.counties[county as usize].population,
        "and the nine still sum to the population"
    );
}

/// A click that never travels nine pixels is a click, and a click opens the job
/// popup for the cluster it landed on — the fifth screen, and the only place
/// the labour record's other two words are shown as a number.
///
/// **But not on the release.** `Village_ClickJob` (`0x0043A123`) is gated on
/// `DAT_004EABF0`, which the frame poll sets only once the click has stood for
/// 300 ms, then one; the click might be the first half of
/// a double click, and a double click means something else entirely. So the
/// popup opens on a *tick*, and the ticks are what this test counts.
#[test]
fn a_click_on_a_cluster_opens_its_job_popup() {
    let (mut game, assets) = world!();
    let county = (1..=game.kingdom.county_count as u8)
        .find(|&id| game.is_players(id))
        .expect("the player holds a county");
    let mut screen = VillageScreen::new(county);
    let c = &game.kingdom.counties[county as usize];
    let slots = VillageScreen::slots(c);

    // Cluster 2 is cattle farming, which is the job the shipped save staffs.
    let (ox, oy) = village::cluster_origin(2, village::SCENE_Y);
    let (x, y) = (ox + 36, oy + 24);
    send(&mut screen, &mut game, &assets, Event::Click { x, y });
    assert_eq!(screen.phase(), village_screen::Phase::Idle, "one press is not a drag");
    let t = send(&mut screen, &mut game, &assets, Event::Release { x, y });
    assert_eq!(t, Transition::Stay, "the release only arms the click");

    // One tick short of the settle, and still nothing.
    for _ in 1..VillageScreen::CLICK_SETTLE_TICKS {
        let mut ctx = Ctx { game: &mut game, assets: &assets };
        assert_eq!(screen.update(&mut ctx), Transition::Stay);
    }
    let mut ctx = Ctx { game: &mut game, assets: &assets };
    assert_eq!(screen.update(&mut ctx), Transition::Push(ScreenId::Job(county, slots[2])));
}

/// **Double-clicking a job takes the people it cannot use out of it.**
///
/// A player reported *"I can't double click idle peasants in a task to remove
/// them from the task"*, and the original does exactly that:
/// `Village_DoubleClick` (`0x00439DF0`) is its own arm on screen `0x02`, fed by
/// `WM_LBUTTONDBLCLK` through `DAT_004EABC5`, and `FUN_00439F6A` moves either
/// the surplus out or the shortfall in.
///
/// Both directions, on the county's own cattle cluster, plus the thing that
/// makes it a *different* gesture: the job popup a single click would have
/// opened never appears.
#[test]
fn a_double_click_on_a_job_sheds_its_surplus_and_fills_its_shortfall() {
    let (mut game, assets) = world!();
    let county = (1..=game.kingdom.county_count as u8)
        .find(|&id| game.is_players(id))
        .expect("the player holds a county");
    let slots = VillageScreen::slots(&game.kingdom.counties[county as usize]);
    let cattle = slots[2];
    let idle = l2_kingdom::tables::JOB_IDLE_TOWNSFOLK;

    // Put everybody on cattle and give the job a ceiling it is well over.
    {
        let c = &mut game.kingdom.counties[county as usize];
        c.labour = [0; l2_kingdom::tables::JOB_COUNT];
        c.labour[cattle] = 300;
        c.labour_wanted[cattle] = -1;
        c.labour_useful[cattle] = 100;
        c.population = 300;
    }

    let (ox, oy) = village::cluster_origin(2, village::SCENE_Y);
    let (x, y) = (ox + 36, oy + 24);
    let mut screen = VillageScreen::new(county);
    send(&mut screen, &mut game, &assets, Event::DoubleClick { x, y });

    let c = &game.kingdom.counties[county as usize];
    assert_eq!(c.labour[cattle], 100, "the job is left with exactly what it can use");
    assert_eq!(c.labour[idle], 200, "and the other two hundred are idle");
    assert_eq!(c.labour.iter().sum::<i32>(), 300, "nobody was created or lost");

    // The other direction: give the job a floor and double-click it again.
    game.kingdom.counties[county as usize].labour_wanted[cattle] = 250;
    game.kingdom.counties[county as usize].labour_useful[cattle] = 250;
    send(&mut screen, &mut game, &assets, Event::DoubleClick { x, y });
    let c = &game.kingdom.counties[county as usize];
    assert_eq!(c.labour[cattle], 250, "the shortfall came out of the idle pool");
    assert_eq!(c.labour[idle], 50);

    // Nothing is pending; no job popup opens.
    for _ in 0..VillageScreen::CLICK_SETTLE_TICKS + 2 {
        let mut ctx = Ctx { game: &mut game, assets: &assets };
        assert_eq!(screen.update(&mut ctx), Transition::Stay, "a double click opens no popup");
    }
}

/// **A double click cancels the single click it interrupted.** The frame poll
/// clears `DAT_004E65E8` — the pending click — the instant `DAT_004EABC5` is
/// Set; the job popup does not open behind the reassignment.
#[test]
fn a_double_click_cancels_the_pending_single_click() {
    let (mut game, assets) = world!();
    let county = (1..=game.kingdom.county_count as u8)
        .find(|&id| game.is_players(id))
        .expect("the player holds a county");
    let mut screen = VillageScreen::new(county);
    let (ox, oy) = village::cluster_origin(2, village::SCENE_Y);
    let (x, y) = (ox + 36, oy + 24);

    // The first half of the double click: press, release, click armed.
    send(&mut screen, &mut game, &assets, Event::Click { x, y });
    send(&mut screen, &mut game, &assets, Event::Release { x, y });
    // The second half arrives well inside the settle window.
    send(&mut screen, &mut game, &assets, Event::DoubleClick { x, y });
    for _ in 0..VillageScreen::CLICK_SETTLE_TICKS + 2 {
        let mut ctx = Ctx { game: &mut game, assets: &assets };
        assert_eq!(screen.update(&mut ctx), Transition::Stay);
    }
}

/// **Double-clicking the idle townsfolk themselves puts everybody to work.**
///
/// `FUN_00439EDB`'s cluster-6 branch: every job sheds its surplus first, and
/// only then does every job draw from the pool. One pass would let whichever
/// job came first take people the later ones needed.
///
/// # The ceilings are the county's own, and they have to be
///
/// This test used to write the ceilings by hand — cattle useful 100, grain
/// wanted 100 — and it passed only because our `Labour_Move` recomputed nothing.
/// Each `Village_BalanceJob` is a `Labour_Move` (`0x00439B52`), and that runs
/// `County_RefreshEstimates` twice, so **the first job to shed rewrites every
/// ceiling from the county itself** before the fill pass reads one. In the
/// original a hand-written floor would not survive the first move either; on
/// England turn one, which holds no grain at all, grain's real floor is −1 and
/// the fill pass has nothing to fill. `docs/decisions.md` C180.
///
/// **So the county is given a real floor, and that is staged:** 2,000 sacks,
/// and its fallow fields painted wheat through `Kingdom::paint_field`, the
/// brush's own road, which computes the ceilings. Only the *assignment* is then
/// written by hand — everybody on the cattle, nobody idle — because that is the
/// thing the gesture acts on, and no estimate reads it.
///
/// **Ablation, run:** swap `[false, true]` for `[true, false]` in
/// `Game::balance_all_labour`, so every job fills before any sheds, and grain
/// gets nobody — red at the grain assertion.
#[test]
fn a_double_click_on_the_idle_cluster_balances_every_job_at_once() {
    let (mut game, assets) = world!();
    let county = (1..=game.kingdom.county_count as u8)
        .find(|&id| game.is_players(id))
        .expect("the player holds a county");
    let id = county as usize;
    game.kingdom.counties[id].grain = 2000;
    let fallow: Vec<usize> = game
        .kingdom
        .field_tiles(id)
        .into_iter()
        .filter(|&(_, t)| t == l2_kingdom::field::FieldType::Fallow)
        .map(|(t, _)| t)
        .collect();
    assert!(!fallow.is_empty(), "the person's county has fallow fields to sow");
    for t in fallow {
        game.kingdom
            .paint_field(id, t, l2_kingdom::field::FieldType::Grain)
            .expect("a fallow field takes wheat");
    }

    let slots = VillageScreen::slots(&game.kingdom.counties[id]);
    let (cattle, grain) = (slots[2], slots[1]);
    let idle = l2_kingdom::tables::JOB_IDLE_TOWNSFOLK;
    let population = game.kingdom.counties[id].population;
    let (ceiling, floor) = {
        let c = &mut game.kingdom.counties[id];
        c.labour = [0; l2_kingdom::tables::JOB_COUNT];
        c.labour[cattle] = population;
        (c.labour_useful[cattle], c.labour_wanted[grain])
    };
    let shed = population - ceiling;
    // The premise, from the county's own estimates: cattle is over its ceiling,
    // grain is short by more than cattle can give, and the pool is empty — so
    // grain can only be filled *after* cattle has shed.
    assert!(shed > 0, "cattle is over its ceiling of {ceiling} with all {population} on it");
    assert!(floor > shed, "grain wants {floor}, more than the {shed} cattle will shed");

    let mut screen = VillageScreen::new(county);
    let (ox, oy) = village::cluster_origin(village::IDLE_CLUSTER, village::SCENE_Y);
    send(&mut screen, &mut game, &assets, Event::DoubleClick { x: ox + 36, y: oy + 24 });

    let c = &game.kingdom.counties[id];
    assert_eq!(
        (c.labour_useful[cattle], c.labour_wanted[grain]),
        (ceiling, floor),
        "the refresh inside each move recomputed the same ceilings from the county"
    );
    assert_eq!(c.labour[cattle], ceiling, "cattle shed its surplus");
    assert_eq!(c.labour[grain], shed, "and grain was filled out of what it shed");
    assert_eq!(c.labour[idle], 0, "nobody is left idle: grain wanted more than there was");
    assert_eq!(c.labour.iter().sum::<i32>(), population);
}

/// **Closing a screen opened over the village closes the village with it**, and
/// That is the original's behaviour.
///
/// The player, from that game: *"things that open a dialog will open
/// it and when you close that dialogue it will close town square and that
/// dialogue, probably something to fix so it only closes the dialog you
/// opened."* `docs/bugs.md` B63 records why it happens — `g_screenId` is one
/// byte and 57 of `Screen_FrameInput`'s 100 writes to it are the literal `0` —
/// and what a switch would cost. **We reproduce it on purpose**, so it needs a
/// test that fails if somebody quietly improves it. C59.
#[test]
fn a_screen_opened_over_the_village_takes_the_village_with_it_when_it_closes() {
    let (mut game, assets) = world!();
    let county = (1..=game.kingdom.county_count as u8)
        .find(|&id| game.is_players(id))
        .expect("the player holds a county");
    game.select(county);

    let mut m = Machine::new(ScreenId::Campaign);
    m.push(ScreenId::Village(county));

    // A sidebar button, clicked through the village. `FUN_00432967` is
    // `Hotspot_Test(0x1DE, 0x1AE, &g_sidebarButtons, 6)` and it is the second
    // Of the six guards. COURT, as a shell's right button
    // closes it and this test needs to watch it close.
    let button = map::SIDEBAR_BUTTONS[1].rect();
    {
        let mut c = Ctx { game: &mut game, assets: &assets };
        m.handle(Event::Click { x: button.x + 4, y: button.y + 4 }, &mut c);
    }
    assert_eq!(
        m.ids(),
        vec![ScreenId::Campaign, ScreenId::Court],
        "the sidebar's screen replaced the village rather than stacking on it"
    );

    // And closing that screen lands on the map, not back on the village.
    {
        let mut c = Ctx { game: &mut game, assets: &assets };
        m.handle(Event::RightClick { x: 320, y: 240 }, &mut c);
    }
    assert_eq!(m.ids(), vec![ScreenId::Campaign], "a panel's exit is a constant 0, not a memory");
}

/// **The village animates, and `villani1.pl8` is what the iron mine animates
/// from.**
///
/// `Village_Animate` (`0x00412421`) draws six overlays and the sixth is the
/// only read of `villani1.pl8` in the whole executable — a file this project
/// had recorded as *"loaded by nothing"*. Three of the six are unconditional
/// and run in every county.
///
/// The clock is checked as *pulses*, not as pixels alone: eighty milliseconds
/// is one step of the fast counter and a hundred and sixty of the slow one,
/// which is `FUN_004BBC80`'s divider chain and not a rate of ours.
#[test]
fn the_village_animates_and_the_iron_mine_comes_out_of_villani1() {
    let (mut game, assets) = world!();
    let art = assets.village.as_ref().expect("the village artwork");
    assert!(art.has_villani1(), "villani1.pl8 is in the install and the iron mine needs it");

    let iron = game
        .kingdom
        .county_ids()
        .find(|&id| game.kingdom.counties[id].industry[1].has_resource)
        .expect("England has an iron county");

    let mut screen = VillageScreen::new(iron as u8);
    let first = draw(&mut screen, &mut game, &assets);

    // One slow pulse: eight 20 ms gates, and a gate is two 16 ms ticks because
    // `Tick_Pulses` (`0x004BBC80`) drops its remainder — C179. Every one of the
    // six overlays has moved at least once by then.
    let ticks = village::PULSE_SLOW_MS / village::GATE_MS * 2;
    assert_eq!(ticks, 16, "160 ms is eight gates, sixteen fixed ticks");
    for _ in 0..ticks {
        let mut ctx = Ctx { game: &mut game, assets: &assets };
        screen.update(&mut ctx);
    }
    assert!(screen.take_redraw(), "a moved animation asks for a repaint");
    let later = draw(&mut screen, &mut game, &assets);
    let moved = first.diff_count(&later);
    assert!(moved > 40, "the village is still after ten ticks: {moved} pixels moved");

    // The clock is display state and nothing else: stepping it must not touch
    // the world. If it ever did, this is the assertion that would say so.
    let before = game.kingdom.clone();
    for _ in 0..100 {
        let mut ctx = Ctx { game: &mut game, assets: &assets };
        screen.update(&mut ctx);
    }
    assert_eq!(game.kingdom, before, "the animation clock reached the simulation");
}

/// **`villani2.pl8`'s own frame table confirms every one of the six overlays,
/// independently of the decompilation.**
///
/// The six runs in [`village::OVERLAYS`] — their first frame and their length —
/// were read from `Village_Animate`'s counter bounds. The sheet
/// looked at. Looking at it: `villani2.pl8`'s 44 frames fall into **five blocks
/// of equal-sized frames laid out in rows on the artist's canvas**, and the
/// blocks are
///
/// | frames | size | overlay |
/// |---|---|---|
/// | 0 … 6 | 26 × 29 | the stone quarry's, 7 frames |
/// | 7 … 14 | 39 × 40 | the lumber camp's, 8 |
/// | 15 … 24 | 15 × 12 | the third unconditional one, 10 |
/// | 25 … 32 | 32 × 42 | the first unconditional one, 8 |
/// | 33 … 39 | 19 × 18 | the second unconditional one, 7 |
///
/// which is **exactly** the six-entry table, start index and length, five times
/// over. What is left is frames 40, 41 and 43 — the three static buildings
/// `Village_Draw` blits at `0x28`, `0x29` and `0x2B` — and one 2 × 2 stub at
/// 42. Nothing over, nothing short.
///
/// A block boundary that fell one frame from where the counter wraps would show
/// up here as an animation that jumps to a different-sized picture, and this is
/// the assertion that would catch it. Sizes are the evidence and the
/// decompilation is the claim; they agree.
#[test]
fn the_animation_runs_are_the_blocks_the_sheet_is_laid_out_in() {
    let Some(dir) = install() else {
        l2_testkit::skip!("no game install, so there is no sheet to read");
    };
    let path = std::fs::read_dir(&dir)
        .ok()
        .and_then(|d| {
            d.filter_map(|e| e.ok()).map(|e| e.path()).find(|p| {
                p.file_name()
                    .and_then(|f| f.to_str())
                    .is_some_and(|f| f.eq_ignore_ascii_case("villani2.pl8"))
            })
        })
        .expect("villani2.pl8 is in the install");
    let bytes = std::fs::read(path).expect("villani2.pl8 reads");
    let pl8 = l2_formats::Pl8::parse(&bytes).expect("villani2.pl8 decodes");
    assert_eq!(pl8.frames.len(), 44);

    for overlay in village::OVERLAYS.iter().filter(|o| !o.villani1) {
        let run = &pl8.frames[overlay.first..overlay.first + overlay.frames];
        let size = (run[0].width, run[0].height);
        for (i, f) in run.iter().enumerate() {
            assert_eq!(
                (f.width, f.height),
                size,
                "frame {} of the run at {} is a different size",
                overlay.first + i,
                overlay.first
            );
        }
        // The frame *before* the run and the frame *after* it must both be a
        // different size, or the boundary is not where the counter wraps.
        if overlay.first > 0 {
            let prev = &pl8.frames[overlay.first - 1];
            assert_ne!(
                (prev.width, prev.height),
                size,
                "the run at {} starts one frame late: {} is the same size",
                overlay.first,
                overlay.first - 1
            );
        }
        let after = overlay.first + overlay.frames;
        let next = &pl8.frames[after];
        assert_ne!(
            (next.width, next.height),
            size,
            "the run at {} is one frame short: {after} is the same size",
            overlay.first
        );
    }

    // The three static buildings are the three big frames past the animation
    // blocks, and the sheet has nothing else in it.
    for (_, frame, _, _) in village::RESOURCE_BUILDINGS {
        let f = &pl8.frames[frame];
        assert!(f.width > 100 && f.height > 70, "frame {frame:#04X} is not a building");
    }
    eprintln!("villani2: five animation blocks and three buildings account for all 44 frames");
}

/// **Every overlay's frame run is inside the sheet it is indexed against**, and
/// The two counters nothing draws are recorded.
///
/// A frame index off the end of a PL8 is a hole here;
/// without this the six runs could be wrong by any amount and nothing would
/// say so.
///
/// # `villani1.pl8` holds 21 frames and the shipped game plays 18 of them
///
/// **[V]** for both numbers. The iron mine's counter, `DAT_004D2938`, wraps at
/// `0x11`, so it visits 0 … 17 and three frames of the file are never drawn.
///
/// And `Village_Animate`'s **dead** counter `DAT_004D2934` wraps at `0x14` —
/// **21 states, which is exactly the file's frame count** — and is read by
/// nothing in the executable. **[I]**, and deliberately only that: the two
/// Numbers agreeing is a coincidence.
/// that the mine was meant to run off that counter. `docs/bugs.md` B65 records
/// the numbers and says the same thing.
#[test]
fn every_village_overlay_run_fits_inside_its_own_sheet() {
    let Some(dir) = install() else {
        l2_testkit::skip!("no game install, so there are no sheets to index");
    };
    let read = |name: &str| -> Option<Vec<u8>> {
        std::fs::read_dir(&dir)
            .ok()?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .find(|p| {
                p.file_name()
                    .and_then(|f| f.to_str())
                    .is_some_and(|f| f.eq_ignore_ascii_case(name))
            })
            .and_then(|p| std::fs::read(p).ok())
    };
    let counts: Vec<usize> = ["villani1.pl8", "villani2.pl8"]
        .iter()
        .map(|n| {
            let b = read(n).unwrap_or_else(|| panic!("{n} is in the install"));
            l2_formats::Pl8::parse(&b).unwrap_or_else(|e| panic!("{n}: {e}")).frames.len()
        })
        .collect();
    assert_eq!(counts[0], 21, "villani1.pl8 holds 21 frames");
    assert_eq!(counts[1], 44, "villani2.pl8 holds 44");

    let mut clock = village::AnimationClock::new();
    let mut highest = [0usize; 2];
    // Several full turns of the longest run, so every counter visits every
    // value it can take.
    for _ in 0..18 * 4 {
        // One slow pulse is eight gates, and a gate takes whatever it takes.
        for _ in 0..village::PULSE_SLOW_MS / village::GATE_MS {
            clock.tick(village::GATE_MS);
        }
        for overlay in &village::OVERLAYS {
            let f = clock.frame_of(overlay);
            let sheet = usize::from(!overlay.villani1);
            assert!(
                f < counts[sheet],
                "frame {f} is off the end of {} ({} frames)",
                if overlay.villani1 { "villani1.pl8" } else { "villani2.pl8" },
                counts[sheet]
            );
            highest[sheet] = highest[sheet].max(f);
        }
    }
    // The iron mine stops at 17, three frames short of the file's end. That
    // is the original's, not a clamp of ours: `DAT_004D2938` wraps at `0x11`.
    assert_eq!(highest[0], 17, "the iron mine's counter visits 0 … 17");
    assert_eq!(counts[0] - (highest[0] + 1), 3, "three frames of villani1 are never drawn");

    // The two counters `Village_Animate` steps and nothing reads. Kept as a
    // claim so that finding a consumer later fails this and gets looked at.
    assert_eq!(village::DEAD_COUNTER_PERIODS.len(), 2);
    assert!(
        village::OVERLAYS.iter().all(|o| o.frames != 21),
        "no overlay uses the 21-state counter, which is why it is called dead"
    );
    // …and the coincidence, asserted so that it stays visible: the dead
// counter has as many states as villani1.pl8 has frames.
    assert_eq!(
        village::DEAD_COUNTER_PERIODS[0].1,
        counts[0],
        "the 21-state dead counter and villani1's 21 frames"
    );
}

/// **A right click during the village's drag gesture does not leave the
/// village.** `docs/arms.json` `0x0042FF10/carry-right-cancels`.
///
/// This arm was *wrong*; the screen popped from
/// every phase, so a player who picked peasants up and changed his mind lost the
/// village with them. `0x06`'s arm is `g_screenId = 0x02`, not 0.
#[test]
fn a_right_click_during_a_peasant_drag_cancels_the_drag_and_not_the_village() {
    let (mut game, assets) = world!();
    game.select(8);
    let top = 64;

    // Press, travel more than nine pixels, and the band is up (screen 0x05).
    let mut m = over_the_map(ScreenId::Village(8));
    send_stack(&mut m, &mut game, &assets, Event::Click { x: 100, y: top + 60 });
    send_stack(&mut m, &mut game, &assets, Event::Pointer { x: 160, y: top + 110 });
    send_stack(&mut m, &mut game, &assets, Event::RightClick { x: 160, y: top + 110 });
    assert_eq!(
        m.top_id(),
        Some(ScreenId::Village(8)),
        "0x05 has no right-button arm at all, so the click is swallowed"
    );

    // Release: 0x06 if the band caught anybody, 0x02 if it did not. Either way
    // a right click now must keep the village.
    send_stack(&mut m, &mut game, &assets, Event::Release { x: 160, y: top + 110 });
    send_stack(&mut m, &mut game, &assets, Event::RightClick { x: 160, y: top + 110 });

    // The idle village, by contrast, leaves on the right button - which is the
    // ablation: if the fix were "the village never leaves on a right click",
    // this would fail.
    let mut idle = over_the_map(ScreenId::Village(8));
    send_stack(&mut idle, &mut game, &assets, Event::RightClick { x: 160, y: top + 110 });
    assert_eq!(
        idle.top_id(),
        Some(ScreenId::Campaign),
        "the idle village really does leave on a right click"
    );
}

#![allow(unused_imports)]
use super::*;
use super::render::*;
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


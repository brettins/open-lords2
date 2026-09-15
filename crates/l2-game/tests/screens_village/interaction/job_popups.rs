#![allow(unused_imports)]
use super::*;
use super::peasant_drag::*;
use super::navigation::*;
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

    let (ox, oy) = village::cluster_origin(2, village::SCENE_Y);
    let (x, y) = (ox + 36, oy + 24);
    send(&mut screen, &mut game, &assets, Event::Click { x, y });
    assert_eq!(screen.phase(), village_screen::Phase::Idle, "one press is not a drag");
    let t = send(&mut screen, &mut game, &assets, Event::Release { x, y });
    assert_eq!(t, Transition::Stay, "the release only arms the click");

    for _ in 1..VillageScreen::CLICK_SETTLE_TICKS {
        let mut ctx = Ctx { game: &mut game, assets: &assets };
        assert_eq!(screen.update(&mut ctx), Transition::Stay);
    }
    let mut ctx = Ctx { game: &mut game, assets: &assets };
    assert_eq!(screen.update(&mut ctx), Transition::Push(ScreenId::Job(county, slots[2])));
}

/// `Village_DoubleClick` (`0x00439DF0`) is its own arm on screen `0x02`, fed by
/// `WM_LBUTTONDBLCLK` through `DAT_004EABC5`, and `FUN_00439F6A` moves either
/// the surplus out or the shortfall in.
#[test]
fn a_double_click_on_a_job_sheds_its_surplus_and_fills_its_shortfall() {
    let (mut game, assets) = world!();
    let county = (1..=game.kingdom.county_count as u8)
        .find(|&id| game.is_players(id))
        .expect("the player holds a county");
    let slots = VillageScreen::slots(&game.kingdom.counties[county as usize]);
    let cattle = slots[2];
    let idle = l2_kingdom::tables::JOB_IDLE_TOWNSFOLK;

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

    game.kingdom.counties[county as usize].labour_wanted[cattle] = 250;
    game.kingdom.counties[county as usize].labour_useful[cattle] = 250;
    send(&mut screen, &mut game, &assets, Event::DoubleClick { x, y });
    let c = &game.kingdom.counties[county as usize];
    assert_eq!(c.labour[cattle], 250, "the shortfall came out of the idle pool");
    assert_eq!(c.labour[idle], 50);

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

    send(&mut screen, &mut game, &assets, Event::Click { x, y });
    send(&mut screen, &mut game, &assets, Event::Release { x, y });
    send(&mut screen, &mut game, &assets, Event::DoubleClick { x, y });
    for _ in 0..VillageScreen::CLICK_SETTLE_TICKS + 2 {
        let mut ctx = Ctx { game: &mut game, assets: &assets };
        assert_eq!(screen.update(&mut ctx), Transition::Stay);
    }
}

/// `FUN_00439EDB`'s cluster-6 branch: every job sheds its surplus first, and
/// only then does every job draw from the pool. One pass would let whichever
/// job came first take people the later ones needed.
///
/// Each `Village_BalanceJob` is a `Labour_Move` (`0x00439B52`), and that runs
/// `County_RefreshEstimates` twice, so **the first job to shed rewrites every
/// ceiling from the county itself** before the fill pass reads one. In the
/// original a hand-written floor would not survive the first move either; on
/// England turn one, which holds no grain at all, grain's real floor is −1 and
/// the fill pass has nothing to fill. `docs/decisions.md` C180.
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


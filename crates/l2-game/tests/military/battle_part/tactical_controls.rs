#![allow(unused_imports)]
use super::*;
use super::hires_and_sieges::*;
use super::prompts_and_settling::*;
use super::battle_ai::*;
use super::watched_battles::*;
use super::*;
use super::raising::*;
use super::marching::*;
use super::division::*;
use l2_game::game::Assets;
use l2_game::input::{Event, Key};
use l2_game::screen::{Ctx, Machine, ScreenId};
use l2_game::battlefield as bf;
use l2_game::screens::{armoury, army, battle, divide, info, map};
use l2_game::Game;
use l2_kingdom::map::{flags, CampaignMap, MAP_DIM, MAP_TILES};
use l2_kingdom::unit::{TroopType, Unit, UnitKind};
use l2_kingdom::MercenaryBands;
use l2_view::campaign;

pub fn look_at_the_players_men(g: &mut Game) {
    let at = {
        let live = g.battle.as_ref().expect("a live battle");
        (0..live.runner.fighters.len())
            .find(|&i| live.runner.is_alive(i) && live.runner.fighters[i].side != l2_sim::SIDE_A)
            .map(|i| (live.runner.fighters[i].x, live.runner.fighters[i].y))
            .expect("the player has men on the field")
    };
    let live = g.battle.as_mut().expect("a live battle");
    live.cam = ((at.0 as i32 - 7).clamp(0, 80 - bf::VIEW_COLS), (at.1 as i32 - 7).clamp(0, 80 - bf::VIEW_ROWS));
}

/// The whole chain, driven with [`Event`] values and nothing else: press on the
/// field opens the drag (`g_screenId` `0x2A`), release commits the box
/// (`FUN_0043C247`), the men are selected, a release on empty ground is an order
/// (`FUN_0043C57D` → `BattleUnit_Order`),
/// that was clicked.
#[test]
fn a_box_selects_men_and_a_click_on_the_ground_orders_them() {
    let (mut g, a, mut m, _, _) = a_battle_is_about_to_happen();
    press(&mut m, &mut g, &a, 'e');
    click(&mut m, &mut g, &a, on(battle::widget_rect(battle::TAKE_THE_FIELD)));
    assert_eq!(m.top_id(), Some(ScreenId::Battlefield));
    click(&mut m, &mut g, &a, on(bf::Button::Pause.rect()));

    look_at_the_players_men(&mut g);

    send(&mut m, &mut g, &a, Event::Pointer { x: bf::VIEW.x + 4, y: bf::VIEW.y + 4 });
    send(&mut m, &mut g, &a, Event::Click { x: bf::VIEW.x + 4, y: bf::VIEW.y + 4 });
    assert_eq!(
        g.battle.as_ref().map(|b| b.screen_id()),
        Some(0x2A),
        "a press on the field is the drag screen",
    );
    let far = (bf::VIEW.x + bf::VIEW.w - 4, bf::VIEW.y + bf::VIEW.h - 4);
    send(&mut m, &mut g, &a, Event::Pointer { x: far.0, y: far.1 });
    send(&mut m, &mut g, &a, Event::Release { x: far.0, y: far.1 });
    assert_eq!(g.battle.as_ref().map(|b| b.screen_id()), Some(0x29), "and the release ends it");

    let picked = {
        let live = g.battle.as_ref().expect("a live battle");
        live.runner.selected_count(1)
    };
    assert!(picked > 0, "the box picked nobody");

    let (px, py, want) = {
        let live = g.battle.as_ref().expect("a live battle");
        let mut found = None;
        for row in 0..bf::VIEW_ROWS {
            for col in 0..bf::VIEW_COLS {
                let cell = (
                    (live.cam.0 + col).clamp(0, 79) as u8,
                    (live.cam.1 + row).clamp(0, 79) as u8,
                );
                if live.runner.occupant_of(cell.0, cell.1).is_none() {
                    found = Some((
                        bf::VIEW.x + col * bf::TILE + 4,
                        bf::VIEW.y + row * bf::TILE + 4,
                        cell,
                    ));
                    break;
                }
            }
            if found.is_some() {
                break;
            }
        }
        found.expect("some empty ground in view")
    };
    send(&mut m, &mut g, &a, Event::Pointer { x: px, y: py });
    send(&mut m, &mut g, &a, Event::Click { x: px, y: py });
    assert_eq!(g.battle.as_ref().map(|b| b.screen_id()), Some(0x2A));
    send(&mut m, &mut g, &a, Event::Release { x: px, y: py });

    let live = g.battle.as_ref().expect("a live battle");
    let unit = live.current_unit;
    assert_ne!(unit, 0, "the selection did not become a unit");
    let u = live.runner.units.get(unit);
    assert_eq!(
        (u.target_x as u8, u.target_y as u8),
        want,
        "the order did not reach the unit's destination",
    );
}

/// `Screen_FrameInput`'s `0x2A` arm is a ladder and `FUN_0043BF07` is above
/// `FUN_0043C57D`; when the drag guard consumes the release the dispatcher
/// `goto`s past the order guard. Ours ran both for a while, which is the sort of
/// thing that only shows up as *"my men wander off after I select them"*.
#[test]
fn committing_a_selection_box_does_not_also_issue_an_order() {
    let (mut g, a, mut m, _, _) = a_battle_is_about_to_happen();
    press(&mut m, &mut g, &a, 'e');
    click(&mut m, &mut g, &a, on(battle::widget_rect(battle::TAKE_THE_FIELD)));
    click(&mut m, &mut g, &a, on(bf::Button::Pause.rect()));
    look_at_the_players_men(&mut g);

    let before: Vec<(i16, i16)> = {
        let live = g.battle.as_ref().expect("a live battle");
        (1..=80).map(|u| {
            let u = live.runner.units.get(u);
            (u.target_x, u.target_y)
        }).collect()
    };

    let start = (bf::VIEW.x + 4, bf::VIEW.y + 4);
    let end = (bf::VIEW.x + bf::VIEW.w - 4, bf::VIEW.y + bf::VIEW.h - 4);
    send(&mut m, &mut g, &a, Event::Pointer { x: start.0, y: start.1 });
    send(&mut m, &mut g, &a, Event::Click { x: start.0, y: start.1 });
    send(&mut m, &mut g, &a, Event::Pointer { x: end.0, y: end.1 });
    send(&mut m, &mut g, &a, Event::Release { x: end.0, y: end.1 });

    let live = g.battle.as_ref().expect("a live battle");
    assert!(live.runner.selected_count(1) > 0, "the box picked nobody");
    let corner = (
        (live.cam.0 + (end.0 - bf::VIEW.x) / bf::TILE) as i16,
        (live.cam.1 + (end.1 - bf::VIEW.y) / bf::TILE) as i16,
    );
    for u in 1..=80usize {
        let unit = live.runner.units.get(u);
        if !unit.is_live() {
            continue;
        }
        let now = (unit.target_x, unit.target_y);
        if now == before[u - 1] {
            continue;
        }
        assert_ne!(now, corner, "unit {u} was ordered to the box's far corner");
    }
}

/// `FUN_0043C2A9`'s right half is `FUN_0043C55C`, a deselect, and it is refused
/// only inside the overview panel. This is the arm the "right click exits"
/// habit would have replaced — `docs/decisions.md` C61's second pattern.
#[test]
fn the_right_button_on_the_battlefield_clears_the_selection() {
    let (mut g, a, mut m, _, _) = a_battle_is_about_to_happen();
    press(&mut m, &mut g, &a, 'e');
    click(&mut m, &mut g, &a, on(battle::widget_rect(battle::TAKE_THE_FIELD)));
    click(&mut m, &mut g, &a, on(bf::Button::Pause.rect()));
    look_at_the_players_men(&mut g);
    send(&mut m, &mut g, &a, Event::Pointer { x: bf::VIEW.x + 4, y: bf::VIEW.y + 4 });
    send(&mut m, &mut g, &a, Event::Click { x: bf::VIEW.x + 4, y: bf::VIEW.y + 4 });
    let far = (bf::VIEW.x + bf::VIEW.w - 4, bf::VIEW.y + bf::VIEW.h - 4);
    send(&mut m, &mut g, &a, Event::Pointer { x: far.0, y: far.1 });
    send(&mut m, &mut g, &a, Event::Release { x: far.0, y: far.1 });

    let before = g.battle.as_ref().expect("a live battle").runner.selected_count(1);
    assert!(before > 0, "the box picked nobody, so clearing it proves nothing");
    send(&mut m, &mut g, &a, Event::RightClick { x: 0x1E8, y: 0x40 });
    assert_eq!(
        g.battle.as_ref().expect("a live battle").runner.selected_count(1),
        before,
        "the overview panel swallows the right button",
    );
    assert_eq!(m.top_id(), Some(ScreenId::Battlefield), "and it is still a battle");

    send(&mut m, &mut g, &a, Event::RightClick { x: 100, y: 100 });
    assert_eq!(
        g.battle.as_ref().expect("a live battle").runner.selected_count(1),
        0,
        "the right button did not clear the selection",
    );
    assert_eq!(m.top_id(), Some(ScreenId::Battlefield), "right-click must not leave the battle");
    assert!(g.battle.is_some());
}

/// `Mercenary_Hire` (`0x004AC7F3`) adds the band's men to `menTotal` (`+0x168`)
/// and never writes `+0x16C`, and `FUN_004224E7` — the roster painter both
/// battle screens share — draws `Ui_DrawCount(unit.menTotal, 0x48, …)` for the
/// total and folds the band into its own row, `if (unit.mercTroop == row) count
/// += unit.mercMen`, in **both** of its modes. Ours summed the seven rows for
/// the total and passed `Unit::troops` raw for the rows, so an army raised with
/// nothing but a hired band was seven zeroes under *"0 Total men"*, in the one
/// window that asks whether to fight with them.
#[test]
fn a_mercenary_only_army_is_not_drawn_as_no_men_at_all() {
    let hired = the_prompt_over(true);
    let levied = the_prompt_over(false);
    let differing = (0..480usize)
        .flat_map(|y| (0..640usize).map(move |x| (x, y)))
        .filter(|&(x, y)| hired.at(x, y) != levied.at(x, y))
        .count();
    assert_eq!(differing, 0, "the band is not in the roster: {differing} pixels differ");
}



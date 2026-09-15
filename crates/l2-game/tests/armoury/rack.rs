#![allow(unused_imports)]
use super::*;
use super::hit_map::*;
use super::animation::*;
use super::screenshots::*;
use std::path::PathBuf;
use l2_game::game::Assets;
use l2_game::input::{Event, Rect};
use l2_game::scenario;
use l2_game::screen::{Ctx, Machine, ScreenId};
use l2_game::screens::armoury;
use l2_game::Game;
use l2_kingdom::tables::{Tables, WEAPON_TYPE_COUNT};
use l2_view::Canvas;

/// `docs/agents.md` C27 — *a rule with no way in is not a rule the game has* —
/// with the placeholder taken away, so the clicks land on the grid.
#[test]
fn a_levy_raised_on_the_england_fixture_walks_out_of_the_armoury_armed() {
    let (mut g, a) = world!();
    g.prefs.tip_screens = false;
    let county = own_county(&g);
    g.selected = county;
    let realm = g.player as usize;

    let stocked = (0..WEAPON_TYPE_COUNT)
        .find(|&s| g.kingdom.realms[realm].weapons[s] > 0)
        .expect("the fixture's realms have an armoury");
    let stock = g.kingdom.realms[realm].weapons[stocked];
    let troop = stocked as u8 + 1;

    let mut m = Machine::new(ScreenId::Campaign);
    let send = |m: &mut Machine, g: &mut Game, e: Event| {
        let mut ctx = Ctx { game: g, assets: &a };
        m.handle(e, &mut ctx);
    };

    send(&mut m, &mut g, Event::KeyDown(l2_game::input::Key::letter('r')));
    {
        let mut ctx = Ctx { game: &mut g, assets: &a };
        m.update(&mut ctx);
    }
    assert_eq!(m.top_id(), Some(ScreenId::RaiseArmy(county)));

    send(
        &mut m,
        &mut g,
        Event::Click {
            x: l2_game::screens::army::SLIDER_X + 40,
            y: l2_game::screens::army::base(false) + 0x20,
        },
    );
    let men = g.levy.men;
    assert!(men > 50, "40 % of the county is {men} men, which is not enough to raise");

    let cont = l2_game::screens::army::continue_button(false);
    send(&mut m, &mut g, Event::Click { x: cont.centre_x(), y: cont.y + cont.h / 2 });
    for _ in 0..l2_game::press::DELAYED_FRAMES {
        let mut ctx = Ctx { game: &mut g, assets: &a };
        m.update(&mut ctx);
    }
    assert_eq!(m.top_id(), Some(ScreenId::Armoury(county)));

    let at = grid_box(&a, troop).expect("the weapon has a region");
    send(&mut m, &mut g, Event::Click { x: at.centre_x(), y: at.y + at.h / 2 });
    assert_eq!(m.top_id(), Some(ScreenId::Rack(county, troop)), "the wall is the button");

    let all = armoury::button_box(3);
    send(&mut m, &mut g, Event::Click { x: all.centre_x(), y: all.y + all.h / 2 });
    let armed = stock.min(men);
    assert_eq!(g.levy.basket.troops()[troop as usize], armed);

    send(
        &mut m,
        &mut g,
        Event::Click { x: armoury::RACK_OK.centre_x(), y: armoury::RACK_OK.y + 12 },
    );
    send(
        &mut m,
        &mut g,
        Event::Click { x: armoury::CREATE_BOX.centre_x(), y: armoury::CREATE_BOX.y + 12 },
    );

    assert_eq!(m.top_id(), Some(ScreenId::Campaign), "Create closed the armoury");
    let (_, unit) = g
        .kingdom
        .campaign
        .units
        .iter()
        .find(|(_, u)| u.kind == l2_kingdom::unit::UnitKind::Army && u.owner == realm as u8)
        .expect("an army on the map");
    assert_eq!(unit.men, men);
    assert_eq!(unit.troops[troop as usize], armed, "and it is carrying the fixture's weapons");
    assert_eq!(g.kingdom.realms[realm].weapons[stocked], stock - armed);
}

/// A weapon hangs on the armoury wall exactly when the realm owns one —
/// `FUN_00418426`'s `if (realm.weapons[t - 1] > 0)` — and its rack along the
/// bottom appears exactly when `basket[t].available > 0`, which the seeding
/// fills from the same stock. So emptying one stock has to blank **two**
/// rectangles and nothing else, which a diff-in-a-box cannot claim: this
/// asserts what changed *and* where it did not.
#[test]
fn emptying_one_rack_removes_that_weapon_from_the_wall_and_nothing_else() {
    let (mut g, a) = world!();
    let county = own_county(&g);
    let realm = g.player as usize;
    g.kingdom.realms[realm].weapons = [50; WEAPON_TYPE_COUNT];
    g.open_levy(county);

    let mut m = Machine::new(ScreenId::Armoury(county));
    let full = frame(&mut m, &mut g, &a);

    let (frame_index, x, y) = armoury::WALL[0];
    let sheet = a.shell.sheet(armoury::items_sheet(g.kingdom.realms[realm].shield_index));
    let f = sheet.and_then(|s| s.frame(frame_index)).expect("the crossbow frame");
    let sprite = Rect::new(x, y, f.width as i32, f.height as i32);

    let rack = armoury::RACK_HOTSPOTS.iter().find(|h| h.4 == 1).expect("the crossbow rack");
    let rack = Rect::new(rack.0, rack.1, rack.2 - rack.0, rack.3 - rack.1);

    g.kingdom.realms[realm].weapons[0] = 0;
    g.seed_levy_basket();
    let short = frame(&mut m, &mut g, &a);

    let (mut wall, mut row, mut outside) = (0usize, 0usize, 0usize);
    for py in 0..480i32 {
        for px in 0..640i32 {
            if full.at(px as usize, py as usize) == short.at(px as usize, py as usize) {
                continue;
            }
            if sprite.contains(px, py) {
                wall += 1;
            } else if rack.contains(px, py) {
                row += 1;
            } else {
                outside += 1;
            }
        }
    }
    assert!(wall > 1_000, "the crossbow did not come off the wall: {wall} pixels changed");
    assert!(row > 500, "the crossbowman did not leave the bottom row: {row} pixels changed");
    assert_eq!(
        outside, 0,
        "{outside} pixels outside the crossbow's wall sprite and its own rack moved: the \
         picture is answering something other than the stock it was asked about",
    );
}


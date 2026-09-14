#![allow(unused_imports)]
use super::*;
use super::battle_part::*;
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

// ---------------------------------------------------------------------------
// 1. Raise
// ---------------------------------------------------------------------------

#[test]
fn r_on_the_map_opens_the_raise_army_screen_for_the_selected_county() {
    let (mut g, a, mut m) = on_the_map();
    press(&mut m, &mut g, &a, 'r');
    assert_eq!(m.top_id(), Some(ScreenId::RaiseArmy(1)));

    send(&mut m, &mut g, &a, Event::KeyDown(Key::Escape));
    g.selected = 2;
    press(&mut m, &mut g, &a, 'r');
    assert_eq!(m.top_id(), Some(ScreenId::Campaign), "county 2 is not yours");
}

/// Set the levy slider on the raise-army screen. With no band on offer the
/// layout's base is 0xA0, and `Levy_SliderClick`'s track is `x - 0xC4` over the
/// row `base + 0x10 ..< base + 0x40`.
fn set_levy(m: &mut Machine, g: &mut Game, a: &Assets, percent: i32) {
    click(m, g, a, (army::SLIDER_X + percent, army::base(false) + 0x20));
}

/// **`Levy_SliderClick` (`0x00435CEF`) is a drag on its track and a press on
/// its arrows**, and nothing else. A player: *"Slider bar in army recruitment
/// cant be dragged."*
///
/// Every coordinate is typed from the decompilation
/// `army`'s constants: without a band on offer the hit band is
/// `0x80 <= x < 0x176` by `0xB0 <= y < 0xE0`; `x < 0xC4` steps down on a press,
/// `x < 0x129` sets `x - 0xC4` while `g_mouseLeftDown`,
/// on a press. The window procedure's `WM_LBUTTONDBLCLK` sets no down bit.
///
/// **Ablations, run:** delete the `Event::Pointer` arm of
/// `RaiseArmyScreen::handle` and the drag stays at 10; make the track test
/// `pressed` and so does it; delete `left_down = false` from
/// the release and the pointer after it moves the knob to 60; skip the slider
/// check on the right release and it leaves for the armoury mid-drag.
#[test]
fn the_levy_slider_follows_a_drag_and_steps_only_on_an_arrow_press() {
    let (mut g, a, mut m) = on_the_map();
    press(&mut m, &mut g, &a, 'r');
    tick(&mut m, &mut g, &a);
    assert_eq!(m.top_id(), Some(ScreenId::RaiseArmy(1)));
    let y = 192;
    let to = |m: &mut Machine, g: &mut Game, x: i32, y: i32| send(m, g, &a, Event::Pointer { x, y });

    click(&mut m, &mut g, &a, (206, y));
    assert_eq!(g.levy.percent, 10, "the press lands on the track: 206 - 196");
    to(&mut m, &mut g, 233, y);
    assert_eq!(g.levy.percent, 37, "and the knob follows the button while it is down");
    assert_eq!(g.levy.men, 370, "through Levy_SetPercent");
    to(&mut m, &mut g, 320, y);
    assert_eq!(g.levy.percent, 37, "dragged onto the right arrow, which wants a press");
    to(&mut m, &mut g, 296, y);
    assert_eq!(g.levy.percent, 100, "the track's last pixel, 0x128");
    to(&mut m, &mut g, 297, y);
    assert_eq!(g.levy.percent, 100, "one past it is the arrow again");
    to(&mut m, &mut g, 250, 224);
    assert_eq!(g.levy.percent, 100, "y 0xE0 is below the band");
    to(&mut m, &mut g, 150, y);
    assert_eq!(g.levy.percent, 100, "and the left arrow wants a press too");
    send(&mut m, &mut g, &a, Event::Release { x: 150, y });
    to(&mut m, &mut g, 256, y);
    assert_eq!(g.levy.percent, 100, "released: the track reads nothing");

    // The arrows. One step a press, clamped, and never repeated.
    click(&mut m, &mut g, &a, (330, y));
    assert_eq!(g.levy.percent, 100, "101 clamps to 100");
    send(&mut m, &mut g, &a, Event::Release { x: 330, y });
    click(&mut m, &mut g, &a, (150, y));
    assert_eq!(g.levy.percent, 99, "one step down");
    for _ in 0..90 {
        tick(&mut m, &mut g, &a);
    }
    assert_eq!(g.levy.percent, 99, "held for 90 ticks: not a widget record, so no ramp");
    send(&mut m, &mut g, &a, Event::Release { x: 150, y });
    send(&mut m, &mut g, &a, Event::DoubleClick { x: 150, y });
    assert_eq!(g.levy.percent, 98, "a double click is a step: `pressed || doubleClick`");
    send(&mut m, &mut g, &a, Event::Release { x: 150, y });
    send(&mut m, &mut g, &a, Event::DoubleClick { x: 206, y });
    assert_eq!(g.levy.percent, 98, "and on the track it is nothing, because no down bit was set");
    send(&mut m, &mut g, &a, Event::Release { x: 206, y });

    // The right release is read only when the slider answered 0.
    click(&mut m, &mut g, &a, (256, y));
    assert_eq!(g.levy.percent, 60);
    send(&mut m, &mut g, &a, Event::RightClick { x: 256, y });
    assert_eq!(m.top_id(), Some(ScreenId::RaiseArmy(1)), "the held track answered first");
    send(&mut m, &mut g, &a, Event::Release { x: 256, y });
    send(&mut m, &mut g, &a, Event::RightClick { x: 256, y });
    assert_eq!(m.top_id(), Some(ScreenId::Armoury(1)), "and with the button up it goes on");
}

/// From the map to the crossbow rack with one man given a crossbow — through
/// the screens, a key and five clicks. Returns where the rack is clicked.
fn one_crossbowman(m: &mut Machine, g: &mut Game, a: &Assets) -> (i32, i32) {
    press(m, g, a, 'r');
    tick(m, g, a);
    set_levy(m, g, a, 30);
    press_and_wait(m, g, a, on(army::continue_button(false)));
    assert_eq!(m.top_id(), Some(ScreenId::Armoury(1)));
    let rack = armoury::RACK_HOTSPOTS.iter().find(|h| h.4 == 1).expect("the crossbow rack");
    let at = ((rack.0 + rack.2) / 2, (rack.1 + rack.3) / 2);
    click(m, g, a, at);
    assert_eq!(m.top_id(), Some(ScreenId::Rack(1, 1)));
    assert!(
        !g.levy.anim.walker.active,
        "the first pick after a door sends nobody: Levy_Seed zeroed g_armourySelectedType, \
         so FUN_004AABD8 is handed type 0 and `0 < type` refuses"
    );
    click(m, g, a, on(armoury::button_box(0)));
    assert_eq!(g.levy.basket.troops()[TroopType::Crossbowman.index()], 1);
    assert!(!g.levy.anim.walker.active, "equipping sends nobody: FUN_004AABD8 has one caller");
    at
}

/// **Which pick sends a soldier**,
/// remember: *"Seems like the people pick up their weapons after you click on
/// another weapon type."*
///
/// He is right, and it is the binary's rule. `FUN_004AABD8` (`0x004AABD8`) has
/// **one** call site, the first statement of `Armoury_ClickRack`
/// (`0x004358B0`), and it is handed `g_armourySelectedType` *before* that
/// function overwrites it.
/// rack being left, once per pick, when men were added since that rack was
/// opened — never by the `+`, and never by the first pick after a door into
/// the armoury, because `Levy_Seed` (`0x004AA90A`) zeroes the type.
///
/// **The rack already open counts.** `0x0D`'s arm tests the grid and the
/// hotspots with no comparison against the open type, so picking the same
/// weapon again sends its man. Ours refused that click.
///
/// **Ablations, run:** restore `troop != self.troop` in `RackScreen::handle` and
/// the re-pick sends nobody; delete `walker.active = false` from
/// `Game::seed_levy_basket` and the walk survives Continue.
#[test]
fn the_first_pick_sends_nobody_and_picking_the_weapon_again_sends_the_man_who_took_it() {
    let (mut g, a, mut m) = on_the_map();
    let at = one_crossbowman(&mut m, &mut g, &a);

    click(&mut m, &mut g, &a, at);
    assert_eq!(m.top_id(), Some(ScreenId::Rack(1, 1)), "the same rack, reloaded");
    assert!(g.levy.anim.walker.active, "picking the weapon again sends the man who took it");
    assert_eq!(g.levy.anim.walker.slot, 1, "a crossbowman");

    // `Armoury_Button` id 2 writes `g_screenId = 0x17` and nothing else, so he
    // is still on the floor — and `RaiseArmy_Continue`'s `Levy_Seed` takes him
    // off it.
    click(&mut m, &mut g, &a, on(armoury::RACK_OK));
    click(&mut m, &mut g, &a, on(armoury::CHANGE_BOX));
    assert_eq!(m.top_id(), Some(ScreenId::RaiseArmy(1)));
    assert!(g.levy.anim.walker.active, "Change does not touch DAT_005679D0");
    // **Continue is `DAT_004DD340` record 0 and that record is kind 5**, so
    // `RaiseArmy_Continue` (`0x00435CBF`) — and the `Levy_Seed` in its tail —
    // runs on the twentieth frame after the press, not on it.
    press_and_wait(&mut m, &mut g, &a, on(army::continue_button(false)));
    assert_eq!(m.top_id(), Some(ScreenId::Armoury(1)));
    assert!(!g.levy.anim.walker.active, "Levy_Seed's DAT_005679D0 = 0");
}

/// **The walk to the weapon, in ticks.** A player: *"his animation speed was
/// faster than the regular game. not bad, but not the OG."*
///
/// `Armoury_DrawWalker` adds four to x on each `Tick_Pulses` 20 ms pulse, and
/// `Tick_Pulses` sets its stamp to the frame that fired — so on a 16 ms tick a
/// pulse is every **second** tick. The crossbowman starts at `-0x50` and
/// `g_armouryWalkStopX[1]` is 45: thirty-two pulses, the last at x 48, and
/// sixty-two ticks from his first step to his thirty-second. Typed, not
/// computed from `WALKER_STEP` or `PULSE_MS`.
///
/// **Ablation, run:** put back `Anim::tick`'s carry-the-remainder reading and
/// he steps on ticks 2, 3, 4, 5, 7, 8 … — four steps in every five ticks — and
/// takes his thirty-second on tick 40, twenty-four ticks sooner.
#[test]
fn the_crossbowman_reaches_his_weapon_in_the_ticks_tick_pulses_gives_him() {
    let (mut g, a, mut m) = on_the_map();
    let at = one_crossbowman(&mut m, &mut g, &a);
    click(&mut m, &mut g, &a, at);
    assert!(g.levy.anim.walker.active);
    assert_eq!(g.levy.anim.walker.x, -80, "DAT_0056D630 = 0xFFFFFFB0");

    let mut steps: Vec<(u32, i32)> = Vec::new();
    let mut last = -80;
    for t in 1..=400u32 {
        tick(&mut m, &mut g, &a);
        let x = g.levy.anim.walker.x;
        if x != last {
            steps.push((t, x));
            last = x;
        }
        if x >= 45 {
            break;
        }
    }
    assert_eq!(steps.len(), 32, "thirty-two pulses to reach x 45 from -80");
    assert_eq!(steps.last().map(|s| s.1), Some(48));
    assert!(
        steps.windows(2).all(|p| p[1].0 - p[0].0 == 2 && p[1].1 - p[0].1 == 4),
        "four pixels, two ticks apart, every time: {steps:?}"
    );
    assert_eq!(steps[31].0 - steps[0].0, 62, "sixty-two ticks from the first step to the last");
}

/// **The whole walk,
///
/// `docs/agents.md` C27: *a rule with no way in is not a rule the game has.*
/// Raising an army is four screens' worth of clicks in the original and every
/// one of them is here — the map, the levy window, the armoury, one weapon's
/// rack — with no helper reaching past a screen to set the state it then
/// asserts on. The only inputs are a key and five pixel positions.
///
/// 1. `R` on the map — `Sidebar_Button`'s hotspot 1 — opens `0x17`;
/// 2. the slider sets the levy to 30 % of a thousand people;
/// 3. **Continue** — `FUN_00435CBF` — replaces it with the armoury, `0x0A`;
/// 4. clicking the swords on the wall — `FUN_004358B0` through
///    `arm_grid.pl8`, or through the hotspot rectangle when it is not
///    installed — opens `0x0D`;
/// 5. **ALL** — `FUN_00435A61` — arms every man it can;
/// 6. the corner picture returns to the armoury and **Create** —
///    `FUN_00435AE8` id 1, then `Army_RaiseConfirm` — puts the army on the map.
#[test]
fn the_walk_from_the_map_through_the_armoury_puts_an_equipped_army_on_the_map() {
    let (mut g, a, mut m) = on_the_map();
    press(&mut m, &mut g, &a, 'r');
    tick(&mut m, &mut g, &a);
    assert_eq!(m.top_id(), Some(ScreenId::RaiseArmy(1)));
    assert_eq!(g.kingdom.campaign.units.len(), 0, "nothing on the map yet");
    let (pop, happy) = (g.kingdom.counties[1].population, g.kingdom.counties[1].happiness);

    set_levy(&mut m, &mut g, &a, 30);
    assert_eq!(g.levy.men, 300, "thirty per cent of a thousand people");

// Continue. The armoury *replaces* the levy screen
    // it, because the original has one `g_screenId` byte and no stack.
    press_and_wait(&mut m, &mut g, &a, on(army::continue_button(false)));
    assert_eq!(m.top_id(), Some(ScreenId::Armoury(1)));
    assert_eq!(m.depth(), 2, "the levy window was replaced, not covered");
    assert_eq!(g.levy.basket.unequipped(), 300, "the armoury seeded three hundred peasants");

    // The swords. `RACK_HOTSPOTS`'s sixth record carries troop type 3.
    let swords = armoury::RACK_HOTSPOTS.iter().find(|h| h.4 == 3).expect("a sword rack");
    click(&mut m, &mut g, &a, ((swords.0 + swords.2) / 2, (swords.1 + swords.3) / 2));
    assert_eq!(m.top_id(), Some(ScreenId::Rack(1, 3)));

    // ALL is record 3 of `g_armouryBuyWidgets`.
    click(&mut m, &mut g, &a, on(armoury::button_box(3)));
    assert_eq!(
        g.levy.basket.troops()[TroopType::Swordsman.index()],
        200,
        "two hundred swords in the armoury, and two hundred men took one",
    );
    assert_eq!(g.levy.basket.unequipped(), 100, "the other hundred are still peasants");

    // Out of the rack, then Create.
    click(&mut m, &mut g, &a, on(armoury::RACK_OK));
    assert_eq!(m.top_id(), Some(ScreenId::Armoury(1)));
    click(&mut m, &mut g, &a, on(armoury::CREATE_BOX));

    assert_eq!(m.top_id(), Some(ScreenId::Campaign), "the armoury closed on a successful raise");
    assert_eq!(g.kingdom.campaign.units.len(), 1, "one army");
    let (id, unit) = g.kingdom.campaign.units.iter().next().expect("the army");
    assert_eq!(unit.kind, UnitKind::Army);
    assert_eq!(unit.owner, 1);
    assert_eq!(unit.home_county, 1, "L2.eng 31/9, 'An army from'");
    assert_eq!(unit.men, 300);
    assert_eq!(unit.troops.iter().sum::<i32>(), unit.men, "every man is in a troop slot");
    assert_eq!(unit.troops[TroopType::Swordsman.index()], 200, "and two hundred carry a sword");
    assert_eq!(unit.troops[TroopType::Peasant.index()], 100);
    assert_eq!(
        g.kingdom.realms[1].weapons[TroopType::Swordsman.weapon_slot().unwrap()],
        0,
        "Levy_ConsumeWeapons emptied the sword rack",
    );
    assert_eq!(unit.morale, happy, "morale is the happiness *before* the levy is charged");
    assert_eq!(g.kingdom.counties[1].population, pop - 300, "the men left the county");
    assert!(g.kingdom.counties[1].happiness < happy, "and the county minded");
    assert_eq!(
        g.kingdom.counties[1].levy_surcharge, 15,
        "Army_Create sets +0x2F4 to 15, so the next levy from here costs more",
    );
    assert!(g.is_players_unit(id));
}

/// **Change throws the equipment away**, which is the original's behaviour and
/// not an accident of ours: every door into the armoury runs `FUN_004AA90A`,
/// which re-seeds the basket from the realm's stocks and the levy's headcount.
///
/// The slider is *not* what does it — `Levy_SliderClick`'s tail is
/// `Levy_SetPercent` and a redraw request — and this test is the difference
/// between the two readings:
/// still goes.
#[test]
fn walking_back_to_the_levy_screen_and_forward_again_strips_the_men() {
    let (mut g, a, mut m) = on_the_map();
    press(&mut m, &mut g, &a, 'r');
    tick(&mut m, &mut g, &a);
    set_levy(&mut m, &mut g, &a, 30);
    press_and_wait(&mut m, &mut g, &a, on(army::continue_button(false)));

    let swords = armoury::RACK_HOTSPOTS.iter().find(|h| h.4 == 3).expect("a sword rack");
    click(&mut m, &mut g, &a, ((swords.0 + swords.2) / 2, (swords.1 + swords.3) / 2));
    click(&mut m, &mut g, &a, on(armoury::button_box(3)));
    click(&mut m, &mut g, &a, on(armoury::RACK_OK));
    assert_eq!(g.levy.basket.troops()[TroopType::Swordsman.index()], 200);

    // Change, then Continue again. The slider is not moved.
    click(&mut m, &mut g, &a, on(armoury::CHANGE_BOX));
    assert_eq!(m.top_id(), Some(ScreenId::RaiseArmy(1)));
    assert_eq!(g.levy.percent, 30, "the slider stayed where it was");
    assert_eq!(g.levy.basket.troops()[TroopType::Swordsman.index()], 200, "and so did the swords");

    press_and_wait(&mut m, &mut g, &a, on(army::continue_button(false)));
    assert_eq!(m.top_id(), Some(ScreenId::Armoury(1)));
    assert_eq!(g.levy.basket.troops()[TroopType::Swordsman.index()], 0, "re-seeded on the way in");
    assert_eq!(g.levy.basket.unequipped(), 300, "every man a peasant again");
    assert_eq!(g.levy.men, 300, "and the levy itself is untouched");
}

/// An empty rack is inert. `FUN_004358B0`'s guard is `basket[id].available > 0`
/// — the stock the realm owns —
/// open a screen at all, which is the same fact the picture states by not
/// drawing it.
#[test]
fn a_rack_the_realm_has_no_weapons_for_does_not_open() {
    let (mut g, a, mut m) = on_the_map();
    g.kingdom.realms[1].weapons = [0; 6];
    press(&mut m, &mut g, &a, 'r');
    tick(&mut m, &mut g, &a);
    set_levy(&mut m, &mut g, &a, 30);
    press_and_wait(&mut m, &mut g, &a, on(army::continue_button(false)));

    for h in &armoury::RACK_HOTSPOTS {
        click(&mut m, &mut g, &a, ((h.0 + h.2) / 2, (h.1 + h.3) / 2));
        assert_eq!(m.top_id(), Some(ScreenId::Armoury(1)), "rack {} opened on an empty armoury", h.4);
    }
    // And Create still works: an army of peasants is an army.
    click(&mut m, &mut g, &a, on(armoury::CREATE_BOX));
    assert_eq!(g.kingdom.campaign.units.len(), 1);
    let (_, unit) = g.kingdom.campaign.units.iter().next().expect("the army");
    assert_eq!(unit.troops[TroopType::Peasant.index()], 300, "the pitchfork default");
}

/// The two size guards and the message each stands for. `FUN_00435B4D` refuses
/// a levy of nothing (`0xA8`) and a levy under fifty (`0x94`), and both
/// refusals leave the world
/// armoury**, which is where the button is.
#[test]
fn a_levy_of_nothing_and_a_levy_under_fifty_are_both_refused() {
    for percent in [0, 4] {
        let (mut g, a, mut m) = on_the_map();
        press(&mut m, &mut g, &a, 'r');
        tick(&mut m, &mut g, &a);
        set_levy(&mut m, &mut g, &a, percent);
        press_and_wait(&mut m, &mut g, &a, on(army::continue_button(false)));
        click(&mut m, &mut g, &a, on(armoury::CREATE_BOX));
        assert_eq!(
            m.top_id(),
            Some(ScreenId::Armoury(1)),
            "{percent} %: the screen stays open on a refusal",
        );
        assert_eq!(g.kingdom.campaign.units.len(), 0, "{percent} %: and nothing was raised");
        assert_eq!(g.kingdom.counties[1].population, 1_000, "{percent} %: nobody left");
    }
}

/// **Cancel is not a refusal.** `FUN_00435AE8`'s id 3 calls `Army_RaiseConfirm` curtains
/// with `g_confirmAnswer = 0`, whose first arm is `g_screenId = 0;
/// Gfx_LoadCountyMode()` — the map, no army, no message.
#[test]
fn cancel_on_the_armoury_leaves_for_the_map_and_raises_nothing() {
    let (mut g, a, mut m) = on_the_map();
    press(&mut m, &mut g, &a, 'r');
    tick(&mut m, &mut g, &a);
    set_levy(&mut m, &mut g, &a, 40);
    press_and_wait(&mut m, &mut g, &a, on(army::continue_button(false)));
    click(&mut m, &mut g, &a, on(armoury::CANCEL_BOX));
    assert_eq!(m.top_id(), Some(ScreenId::Campaign));
    assert_eq!(g.kingdom.campaign.units.len(), 0, "nothing was raised");
    assert_eq!(g.kingdom.counties[1].population, 1_000, "and nobody left the county");
}

/// The `+` and `−` move **one man**, which is the granularity the original's
/// two smallest buttons have and the thing our screen used to get wrong by
/// moving ten. The arrow keys are ours and do the same.
#[test]
fn the_plus_and_minus_on_a_rack_move_exactly_one_man() {
    let (mut g, a, mut m) = on_the_map();
    press(&mut m, &mut g, &a, 'r');
    tick(&mut m, &mut g, &a);
    set_levy(&mut m, &mut g, &a, 30);
    press_and_wait(&mut m, &mut g, &a, on(army::continue_button(false)));
    let bows = armoury::RACK_HOTSPOTS.iter().find(|h| h.4 == 5).expect("a bow rack");
    click(&mut m, &mut g, &a, ((bows.0 + bows.2) / 2, (bows.1 + bows.3) / 2));

    let archer = TroopType::Archer.index();
    for expected in 1..=3 {
        click(&mut m, &mut g, &a, on(armoury::button_box(0)));
        assert_eq!(g.levy.basket.troops()[archer], expected, "+ moves one man");
    }
    click(&mut m, &mut g, &a, on(armoury::button_box(1)));
    assert_eq!(g.levy.basket.troops()[archer], 2, "- moves one back");
    click(&mut m, &mut g, &a, on(armoury::button_box(2)));
    assert_eq!(g.levy.basket.troops()[archer], 0, "NONE empties the rack");
    assert_eq!(g.levy.basket.unequipped(), 300);
}

/// **`Create` works from inside a rack, and `Change` and `Cancel` do not** —
/// `Hotspot_Test(0, 0, &g_armouryHotspots, 7)` on screen `0x0D` against the
/// armoury's own 9, so record 6 is reached and records 7 and 8 are not.
///
/// It is the one place [`Transition::Pass`] earns its keep in this file: the
/// button belongs to the armoury, the rack declines the click,
/// underneath acts — **at its own depth**, so the rack goes with it
/// being left on a stack above a screen that has closed.
#[test]
fn create_reaches_through_an_open_rack_and_the_other_two_buttons_do_not() {
    let (mut g, a, mut m) = on_the_map();
    press(&mut m, &mut g, &a, 'r');
    tick(&mut m, &mut g, &a);
    set_levy(&mut m, &mut g, &a, 30);
    press_and_wait(&mut m, &mut g, &a, on(army::continue_button(false)));
    let swords = armoury::RACK_HOTSPOTS.iter().find(|h| h.4 == 3).expect("a sword rack");
    let sword_click = ((swords.0 + swords.2) / 2, (swords.1 + swords.3) / 2);

    // Change and Cancel are dead on 0x0D: the screen stays exactly where it is.
    click(&mut m, &mut g, &a, sword_click);
    for dead in [armoury::CHANGE_BOX, armoury::CANCEL_BOX] {
        click(&mut m, &mut g, &a, on(dead));
        assert_eq!(m.top_id(), Some(ScreenId::Rack(1, 3)), "{dead:?} acted on 0x0D");
        assert_eq!(m.depth(), 3, "and it did not disturb the stack either");
    }

    click(&mut m, &mut g, &a, on(armoury::button_box(3)));
    click(&mut m, &mut g, &a, on(armoury::CREATE_BOX));
    assert_eq!(m.top_id(), Some(ScreenId::Campaign), "Create is live, and it closed both");
    assert_eq!(m.depth(), 1, "the rack did not survive the armoury it was opened from");
    assert_eq!(g.kingdom.campaign.units.len(), 1);
    let (_, unit) = g.kingdom.campaign.units.iter().next().expect("the army");
    assert_eq!(unit.troops[TroopType::Swordsman.index()], 200);
}

/// **A hit box that misses.** Every pixel of every rack hotspot opens that rack
/// and no other, and no pixel of the three right-hand buttons opens any rack.
/// `docs/decisions.md` C58: three wrong-screen bugs have reached this player
/// through a near-miss, so the boxes are walked.
#[test]
fn no_pixel_of_the_armoury_opens_the_wrong_thing() {
    let (mut g, a, mut m) = on_the_map();
    press(&mut m, &mut g, &a, 'r');
    tick(&mut m, &mut g, &a);
    set_levy(&mut m, &mut g, &a, 30);
    press_and_wait(&mut m, &mut g, &a, on(army::continue_button(false)));

    for &(x0, y0, x1, y1, troop) in &armoury::RACK_HOTSPOTS {
        for (x, y) in [(x0, y0), (x1 - 1, y0), (x0, y1 - 1), (x1 - 1, y1 - 1)] {
            click(&mut m, &mut g, &a, (x, y));
            assert_eq!(
                m.top_id(),
                Some(ScreenId::Rack(1, troop)),
                "({x}, {y}) is rack {troop}'s corner and opened something else",
            );
            click(&mut m, &mut g, &a, on(armoury::RACK_OK));
        }
    }
    // The three buttons are outside every rack, and Change is the one that goes
// back.
    click(&mut m, &mut g, &a, on(armoury::CHANGE_BOX));
    assert_eq!(m.top_id(), Some(ScreenId::RaiseArmy(1)));
}


//! **The three military verbs, driven the way a player drives them.**
//!
//! Everything below goes through [`Machine::handle`] with an [`Event`] — a
//! click at a pixel, a key — and nothing here opens a window, touches the OS
//! input queue, or needs a copy of the game. `docs/agents.md`: an agent testing
//! our engine never touches the desktop; every screen in this crate takes input
//! as a value, so setting the levy is `Event::Click { x, y }` on the slider and
//! a march order is two more.
//!
//! What this asserts is the gap `docs/plan.md` §2.2 names — *"a game started
//! from the fixture has no army, no way to make one, and the door to making one
//! is filed under a name that reads as optional content"* — closing, in three
//! halves:
//!
//! 1. **Raise.** `R` on the map opens `0x17`, the slider sets the levy, the
//!    button raises, and there is an army on the map that was not there before.
//! 2. **March.** A click on that army selects it (`Map_Click`'s army branch),
//!    a click on a tile orders the march (`Map_ConfirmMoveOrder`), and ending
//!    the turn walks it — because `Units_Tick` runs on every tick of the turn
//!    rather than in a phase (`docs/decisions.md` C35).
//! 3. **Divide.** `A` opens `0x11`, the buttons move men between the two
//!    columns, and split and disband do what `Army_Split` and `Army_Disband`
//!    do.
//!
//! # Why the tests never scroll
//!
//! The machine owns the map screen and nothing hands it back, so a test cannot
//! ask *where is tile (32, 45) on screen* through it. It does not need to: the
//! campaign screen opens at `Map_InitMode`'s own scroll origin — row `0x4A`,
//! column `0x14` — and a second `MapScreen::new()` is looking at exactly the
//! same place. [`pixel`] uses one as a ruler. Every test below therefore works
//! in tiles that are visible at the opening viewport and never scrolls or
//! centres, because the moment it did the ruler and the screen would part
//! company.

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

/// The `x` at which county 1 gives way to county 2, and county 2 to county 3.
///
/// **32 is not arbitrary.** The campaign screen opens on lattice rows 74…104
/// and columns 20…27, and `tile_to_cell` turns that into `x + y ∈ [73, 104)`
/// and `x − y ∈ [−24, −8)`; adding the two bounds gives `x ∈ [25, 48)`. So a
/// border a test can *click across* has to lie in that band, and the second one
/// has to lie outside it or county 3 would be in shot.
const BORDER_1_2: usize = 32;
const BORDER_2_3: usize = 48;

/// Three counties: a human realm holding the first, an AI holding the other
/// two, on a map divided into three vertical bands with every tile walkable.
///
/// **Two things here are load-bearing and were both found by a test failing.**
///
/// * *The opponent needs two counties.* A realm that loses its last one is
///   eliminated, which ends the game and replaces the map with the conquest
///   screen — so a test that takes a county from a one-county opponent cannot
///   then assert that the map is still what is on screen.
/// * *The counties need neighbour lists.* `Realm_SecedeIsolatedCounties` walks
///   `County::neighbours`, and a realm holding two counties that name no
///   neighbours is a realm holding two **blocks** — so a captured county with
///   no adjacency seceded again at the end of the same turn it was taken. The
///   rule is right and the fixture was unreal.
fn world() -> (Game, Assets) {
    let mut g = Game::new(11);
    g.kingdom.set_county_count(3);
    for id in 1..=3usize {
        let c = &mut g.kingdom.counties[id];
        c.population = 1_000;
        c.happiness = 90;
        c.grain = 500;
        c.herd = 100;
    }
    g.kingdom.counties[1].owner = 1;
    g.kingdom.counties[2].owner = 2;
    g.kingdom.counties[3].owner = 2;
    // 1 — 2 — 3, a chain, so no realm ever holds two disjoint blocks.
    for (id, neighbours) in [(1usize, vec![2u8]), (2, vec![1, 3]), (3, vec![2])] {
        let c = &mut g.kingdom.counties[id];
        c.neighbour_count = neighbours.len() as u8;
        for (i, n) in neighbours.into_iter().enumerate() {
            c.neighbours[i] = n;
        }
    }
    for realm in 1..=2usize {
        g.kingdom.realms[realm].in_play = true;
        g.kingdom.realms[realm].strength = 5;
        g.kingdom.realms[realm].gold = 20_000;
    }
    g.kingdom.realms[1].is_human = true;
    // A full armoury, so the equip controls have something to hand out.
    g.kingdom.realms[1].weapons = [200; 6];

    let mut m = CampaignMap::empty();
    for i in 0..MAP_TILES {
        let x = i % MAP_DIM;
        m.county[i] = if x < BORDER_1_2 {
            1
        } else if x < BORDER_2_3 {
            2
        } else {
            3
        };
    }
    g.kingdom.campaign.map = m;
    g.kingdom.campaign.mercenaries = MercenaryBands::init(2);
    g.player = 1;
    g.selected = 1;
    (g, Assets::placeholder())
}

fn send(m: &mut Machine, g: &mut Game, a: &Assets, e: Event) {
    let mut ctx = Ctx { game: g, assets: a };
    m.handle(e, &mut ctx);
}

fn tick(m: &mut Machine, g: &mut Game, a: &Assets) {
    let mut ctx = Ctx { game: g, assets: a };
    m.update(&mut ctx);
}

fn click(m: &mut Machine, g: &mut Game, a: &Assets, at: (i32, i32)) {
    send(m, g, a, Event::Click { x: at.0, y: at.1 });
}

fn press(m: &mut Machine, g: &mut Game, a: &Assets, c: char) {
    send(m, g, a, Event::KeyDown(Key::letter(c)));
}

/// Tick until `done` answers true, or give up.
///
/// **A turn takes frames.** Pressing End Turn only starts one — the phase
/// machine is wound on a tick at a time so a unit that is walking is seen to
/// walk (`l2_game::turn::TurnStep::Running`) — so anything that happens *during*
/// a turn happens some ticks after the keystroke rather than inside it.
fn run_until(
    m: &mut Machine,
    g: &mut Game,
    a: &Assets,
    what: &str,
    done: impl Fn(&Machine, &Game) -> bool,
) {
    for _ in 0..2_000 {
        if done(m, g) {
            return;
        }
        tick(m, g, a);
    }
    panic!("{what} never happened");
}

/// Press End Turn and run the whole turn, including the screen fade that
/// follows the season. Panics if the turn stops to ask something.
fn end_turn(m: &mut Machine, g: &mut Game, a: &Assets) {
    let before = g.kingdom.turn_count;
    press(m, g, a, 'e');
    run_until(m, g, a, "the turn", |_, g| g.kingdom.turn_count > before);
    // The fade runs after the season and the map takes no input until it is
    // over, so a test that clicks afterwards has to wait for the light.
    for _ in 0..=l2_view::fade::PHASES {
        tick(m, g, a);
    }
}

/// A button's middle, so a click lands on it wherever it moves to.
fn on(r: l2_game::input::Rect) -> (i32, i32) {
    (r.centre_x(), r.y + r.h / 2)
}

/// Where a map tile is on the campaign screen at its opening viewport, or
/// `None` when it is not in view. See the module note on why a second screen is
/// a valid ruler for the machine's.
fn pixel(x: u8, y: u8) -> Option<(i32, i32)> {
    let probe = map::MapScreen::new();
    campaign::tile_centre(probe.viewport(), probe.zoom(), x as usize, y as usize)
}

/// A pair of horizontally adjacent tiles that are **both** visible at the
/// opening viewport, with the first in the county whose `x` is given.
///
/// Searched in ascending `(y, x)` so the answer is the same every run — the
/// same reason [`l2_kingdom::divide::free_tile_near`] scans row-major.
fn adjacent_pair(first_x: impl Fn(u8) -> bool) -> ((u8, u8), (u8, u8)) {
    for y in 0..64u8 {
        for x in 0..63u8 {
            if !first_x(x) {
                continue;
            }
            if pixel(x, y).is_some() && pixel(x + 1, y).is_some() {
                return ((x, y), (x + 1, y));
            }
        }
    }
    panic!("no adjacent visible pair at the opening viewport");
}

/// Put an army on the map without going through the screen, for the tests that
/// are about marching rather than about raising.
fn army_at(g: &mut Game, owner: u8, county: u8, men: i32, at: (u8, u8)) -> usize {
    let mut u = Unit::new(UnitKind::Army, owner, at.0, at.1);
    u.men = men;
    u.troops[TroopType::Peasant.index()] = men;
    u.county = county;
    u.home_county = county;
    u.owner_is_human = owner == 1;
    g.kingdom.campaign.units.spawn(u).expect("a free slot")
}

fn on_the_map() -> (Game, Assets, Machine) {
    let (g, a) = world();
    (g, a, Machine::new(ScreenId::Campaign))
}

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

/// **The whole walk, and the reason this test is the shape it is.**
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

    // Continue. The armoury *replaces* the levy screen rather than stacking on
    // it, because the original has one `g_screenId` byte and no stack.
    click(&mut m, &mut g, &a, on(army::continue_button(false)));
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
/// between the two readings: the levy is never touched here and the equipment
/// still goes.
#[test]
fn walking_back_to_the_levy_screen_and_forward_again_strips_the_men() {
    let (mut g, a, mut m) = on_the_map();
    press(&mut m, &mut g, &a, 'r');
    tick(&mut m, &mut g, &a);
    set_levy(&mut m, &mut g, &a, 30);
    click(&mut m, &mut g, &a, on(army::continue_button(false)));

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

    click(&mut m, &mut g, &a, on(army::continue_button(false)));
    assert_eq!(m.top_id(), Some(ScreenId::Armoury(1)));
    assert_eq!(g.levy.basket.troops()[TroopType::Swordsman.index()], 0, "re-seeded on the way in");
    assert_eq!(g.levy.basket.unequipped(), 300, "every man a peasant again");
    assert_eq!(g.levy.men, 300, "and the levy itself is untouched");
}

/// An empty rack is inert. `FUN_004358B0`'s guard is `basket[id].available > 0`
/// — the stock the realm owns — so a weapon the treasury has none of does not
/// open a screen at all, which is the same fact the picture states by not
/// drawing it.
#[test]
fn a_rack_the_realm_has_no_weapons_for_does_not_open() {
    let (mut g, a, mut m) = on_the_map();
    g.kingdom.realms[1].weapons = [0; 6];
    press(&mut m, &mut g, &a, 'r');
    tick(&mut m, &mut g, &a);
    set_levy(&mut m, &mut g, &a, 30);
    click(&mut m, &mut g, &a, on(army::continue_button(false)));

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
/// refusals leave the world exactly as it was — **and leave the player on the
/// armoury**, which is where the button is.
#[test]
fn a_levy_of_nothing_and_a_levy_under_fifty_are_both_refused() {
    for percent in [0, 4] {
        let (mut g, a, mut m) = on_the_map();
        press(&mut m, &mut g, &a, 'r');
        tick(&mut m, &mut g, &a);
        set_levy(&mut m, &mut g, &a, percent);
        click(&mut m, &mut g, &a, on(army::continue_button(false)));
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

/// **Cancel is not a refusal.** `FUN_00435AE8`'s id 3 calls `Army_RaiseConfirm`
/// with `g_confirmAnswer = 0`, whose first arm is `g_screenId = 0;
/// Gfx_LoadCountyMode()` — the map, no army, no message.
#[test]
fn cancel_on_the_armoury_leaves_for_the_map_and_raises_nothing() {
    let (mut g, a, mut m) = on_the_map();
    press(&mut m, &mut g, &a, 'r');
    tick(&mut m, &mut g, &a);
    set_levy(&mut m, &mut g, &a, 40);
    click(&mut m, &mut g, &a, on(army::continue_button(false)));
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
    click(&mut m, &mut g, &a, on(army::continue_button(false)));
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
/// button belongs to the armoury, the rack declines the click, and the armoury
/// underneath acts — **at its own depth**, so the rack goes with it rather than
/// being left on a stack above a screen that has closed.
#[test]
fn create_reaches_through_an_open_rack_and_the_other_two_buttons_do_not() {
    let (mut g, a, mut m) = on_the_map();
    press(&mut m, &mut g, &a, 'r');
    tick(&mut m, &mut g, &a);
    set_levy(&mut m, &mut g, &a, 30);
    click(&mut m, &mut g, &a, on(army::continue_button(false)));
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
/// through a near-miss, so the boxes are walked rather than sampled.
#[test]
fn no_pixel_of_the_armoury_opens_the_wrong_thing() {
    let (mut g, a, mut m) = on_the_map();
    press(&mut m, &mut g, &a, 'r');
    tick(&mut m, &mut g, &a);
    set_levy(&mut m, &mut g, &a, 30);
    click(&mut m, &mut g, &a, on(army::continue_button(false)));

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
    // back rather than forward.
    click(&mut m, &mut g, &a, on(armoury::CHANGE_BOX));
    assert_eq!(m.top_id(), Some(ScreenId::RaiseArmy(1)));
}

// ---------------------------------------------------------------------------
// 2. March
// ---------------------------------------------------------------------------

/// The two clicks: one selects, one orders. Both land on the map, and the map
/// is never left.
#[test]
fn two_clicks_on_the_map_select_an_army_and_send_it_marching() {
    let (mut g, a, mut m) = on_the_map();
    let (here, there) = adjacent_pair(|x| x < 30);
    let id = army_at(&mut g, 1, 1, 300, here);

    click(&mut m, &mut g, &a, pixel(here.0, here.1).unwrap());
    assert_eq!(m.top_id(), Some(ScreenId::Campaign), "selecting an army does not leave the map");
    assert!(g.kingdom.campaign.units.get(id).is_some_and(|u| !u.moving), "not ordered yet");

    click(&mut m, &mut g, &a, pixel(there.0, there.1).unwrap());
    let unit = g.kingdom.campaign.units.get(id).expect("still there");
    assert!(unit.moving, "Unit_OrderMove writes moveState = 2");
    assert_eq!(unit.dest, Some(there));
    assert!(!unit.path.is_empty(), "and a path to walk");
    assert_eq!(m.top_id(), Some(ScreenId::Campaign), "and the map is still what is on screen");
}

/// **A second click on the selected army ends the selection and orders
/// nothing — and it is not a cancel, it is a refused destination.**
///
/// The distinction is the whole of what was wrong here. We had an explicit
/// "clicking the same army again deselects it" branch in `click_unit`, which
/// was ours: while move-order mode is up, `g_screenId` is `0x10` and
/// `Map_Click` is not reachable at all, so the original never sees a second
/// click on an army *as* a click on an army. It sees a destination, and
/// `Map_HoverUnitTarget` has already cleared `g_moveOrderAvailable` for that
/// tile because the flood fill's raw distance there is 1 — the army is
/// standing on it. `Map_ConfirmMoveOrder` returns without writing anything,
/// and `Screen_FrameInput` had already put the screen back to `0`.
///
/// Same outcome, different mechanism, and the mechanism is what generalises:
/// **every** tile the fill did not reach behaves this way, not just this one.
#[test]
fn clicking_the_selected_army_again_ends_the_selection_and_orders_nothing() {
    let (mut g, a, mut m) = on_the_map();
    let (here, there) = adjacent_pair(|x| x < 30);
    let id = army_at(&mut g, 1, 1, 300, here);
    click(&mut m, &mut g, &a, pixel(here.0, here.1).unwrap());
    click(&mut m, &mut g, &a, pixel(here.0, here.1).unwrap());
    assert!(
        g.kingdom.campaign.units.get(id).is_some_and(|u| !u.moving),
        "a destination the fill never reached is not an order",
    );
    // And the selection is gone, so the next click is a fresh selection
    // rather than a destination.
    click(&mut m, &mut g, &a, pixel(there.0, there.1).unwrap());
    assert!(
        g.kingdom.campaign.units.get(id).is_some_and(|u| !u.moving),
        "the deselected army took no order",
    );
}

/// **The right button is how an army is deselected, and it was missing.**
///
/// A player, one minute after reporting that the march preview only appears
/// after the click: *"you cannot deselect an army."* Both were the same gap.
/// `Screen_FrameInput`'s `0x10` arm has exactly four clauses, and this is one
/// of them: `if (g_mouseRightReleased != 0) { g_screenId = 0; g_redrawRequest
/// = 2; }`. We had the right button bound to screen `0`'s arm — the
/// information panel — with no test for the mode, so it opened a panel where
/// the original cancels.
#[test]
fn the_right_button_deselects_an_army_and_does_not_open_the_information_panel() {
    let (mut g, a, mut m) = on_the_map();
    let (here, there) = adjacent_pair(|x| x < 30);
    let id = army_at(&mut g, 1, 1, 300, here);

    let (hx, hy) = pixel(here.0, here.1).unwrap();
    click(&mut m, &mut g, &a, (hx, hy));

    send(&mut m, &mut g, &a, Event::RightClick { x: hx, y: hy });
    assert_eq!(
        m.top_id(),
        Some(ScreenId::Campaign),
        "the information panel did not open over the selection",
    );

    // The selection is gone, and the observable proof is that the next click
    // on open ground is no longer a destination: it is a click on plain
    // ground, which does nothing at all.
    click(&mut m, &mut g, &a, pixel(there.0, there.1).unwrap());
    assert!(
        g.kingdom.campaign.units.get(id).is_some_and(|u| !u.moving),
        "a deselected army takes no orders",
    );

    // And with nothing selected the same gesture reaches screen 0x04, which
    // is what makes the arm above a *mode* test rather than a suppression.
    send(&mut m, &mut g, &a, Event::RightClick { x: hx, y: hy });
    assert!(
        matches!(m.top_id(), Some(ScreenId::Info(_))),
        "with nothing picked the right button still opens the information panel",
    );
}

/// **An unreachable destination is an accepted order with nothing in it**, and
/// that distinction is `docs/armies.md` §2.3's own correction.
///
/// `Move_ExtractPath` returns *success* with a zero-length path when the greedy
/// descent never reached the destination, so `Unit_OrderMove` writes the
/// destination, sets `moveState = 2` and copies an empty path. The army is
/// ordered and stands still. Only a dead end in the descent returns 0 and
/// leaves the record untouched.
///
/// A reimplementation that treated "no path" as a refusal would leave
/// `moveState` at 0, and the phase waits read that field — so this is a
/// lockstep difference, not a cosmetic one.
///
/// **But a human click on the map cannot reach that state, and this test used
/// to say it could.** `Map_ConfirmMoveOrder` opens with `if
/// (g_moveOrderAvailable != 1) return;`, and `Map_HoverUnitTarget` clears that
/// flag whenever the flood fill's raw distance at the hovered tile is below 2
/// — which is every tile the fill never reached. So the empty-path order is
/// real and is what the AI, the network command and the phase machine produce;
/// **from the map it is unreachable, because a gate stands in front of it that
/// we had not implemented.** The order is asserted where it actually happens,
/// in `l2-kingdom`, and the gate is asserted on the screen.
#[test]
fn an_unreachable_destination_is_ordered_with_an_empty_path_and_the_army_stands() {
    let (mut g, a, mut m) = on_the_map();
    let (here, there) = adjacent_pair(|x| x > 26 && x < 30);
    let id = army_at(&mut g, 1, 1, 300, here);
    // Wall the army in on all eight sides.
    for dy in -1i32..=1 {
        for dx in -1i32..=1 {
            if dx == 0 && dy == 0 {
                continue;
            }
            g.kingdom.campaign.map.set_flags(
                (here.0 as i32 + dx) as u8,
                (here.1 as i32 + dy) as u8,
                flags::IMPASSABLE,
            );
        }
    }
    let at = (
        g.kingdom.campaign.units.get(id).unwrap().x,
        g.kingdom.campaign.units.get(id).unwrap().y,
    );

    // The screen refuses first: two clicks place no order at all, because the
    // hover gate never lit.
    click(&mut m, &mut g, &a, pixel(here.0, here.1).unwrap());
    click(&mut m, &mut g, &a, pixel(there.0, there.1).unwrap());
    assert!(
        g.kingdom.campaign.units.get(id).is_some_and(|u| !u.moving),
        "g_moveOrderAvailable was never set, so Map_ConfirmMoveOrder returned",
    );

    // `Unit_OrderMove` itself, which is what the AI and the network command
    // call, accepts it — and that is the part that must not drift.
    let steps = l2_kingdom::movement::order_move(
        &g.kingdom.campaign.map,
        &mut g.kingdom.campaign.units,
        id,
        there,
        l2_kingdom::movement::Routing::Direct,
    );
    assert_eq!(steps, Some(0), "success, with nothing in the buffer");
    let unit = g.kingdom.campaign.units.get(id).expect("still there");
    assert!(unit.moving, "the order was accepted");
    assert_eq!(unit.dest, Some(there), "and it names the tile that was asked for");
    assert!(unit.path.is_empty(), "with no path to walk");

    // And a whole turn of ticking moves it nowhere.
    end_turn(&mut m, &mut g, &a);
    let unit = g.kingdom.campaign.units.get(id).expect("still there");
    assert_eq!((unit.x, unit.y), at, "the army stood exactly where it was");
    assert_eq!(unit.moves_used, 0, "and spent nothing standing there");
}

/// **Clicking your own besieging army opens the siege screen instead of taking
/// orders** — `Map_Click`'s own branch, and the reason a player now reaches
/// `0x1D` from the map rather than from our index.
#[test]
fn clicking_a_besieging_army_opens_the_siege_screen() {
    let (mut g, a, mut m) = on_the_map();
    let (camp, _) = adjacent_pair(|x| x < 30);
    g.kingdom.counties[2].castle_type = 2;
    let garrison = army_at(&mut g, 2, 2, 120, (camp.0, camp.1 + 2));
    g.kingdom.campaign.units.get_mut(garrison).unwrap().garrison_county = 2;
    g.kingdom.counties[2].garrison_unit = garrison;

    let id = army_at(&mut g, 1, 1, 400, camp);
    g.kingdom.campaign.units.get_mut(id).unwrap().besieging_county = 2;
    g.kingdom.campaign.units.get_mut(garrison).unwrap().besieged_by = id as u8;

    click(&mut m, &mut g, &a, pixel(camp.0, camp.1).unwrap());
    assert_eq!(
        m.top_id(),
        Some(ScreenId::Siege(id)),
        "a besieging army is a siege, not a march order",
    );
}

/// **`Siege_ValidateLink` runs before the branch is chosen**, so a besieger
/// whose target garrison has gone gets its link cleared *by the click* and
/// lands on the move branch in the same call.
#[test]
fn a_besieger_whose_garrison_has_gone_takes_orders_instead_of_opening_the_siege() {
    let (mut g, a, mut m) = on_the_map();
    let (camp, there) = adjacent_pair(|x| x < 30);
    let id = army_at(&mut g, 1, 1, 400, camp);
    // Besieging a county that has no garrison at all any more.
    g.kingdom.campaign.units.get_mut(id).unwrap().besieging_county = 2;
    g.kingdom.counties[2].garrison_unit = 0;

    click(&mut m, &mut g, &a, pixel(camp.0, camp.1).unwrap());
    assert_eq!(m.top_id(), Some(ScreenId::Campaign), "no siege screen: the link was stale");
    assert_eq!(
        g.kingdom.campaign.units.get(id).unwrap().besieging_county,
        0,
        "and the click is what cleared it",
    );
    click(&mut m, &mut g, &a, pixel(there.0, there.1).unwrap());
    assert!(g.kingdom.campaign.units.get(id).is_some_and(|u| u.moving), "orders taken");
}

/// **The end of the verb: march onto an enemy county's town and take it.**
///
/// Everything is ordered from the map and nothing else is touched. Ending the
/// turn walks the army, `Unit_TryEnterTile` returns code 5 on the `0x40` tile
/// and `Army_AttackCounty` changes the owner. `docs/armies.md` §2.2: *"stepping
/// onto a county's town is how a county is taken."*
#[test]
fn an_army_ordered_from_the_map_takes_an_undefended_county_when_the_turn_ends() {
    let (mut g, a, mut m) = on_the_map();
    // The border: county 1's last column and county 2's first, both in view.
    let (here, town) = adjacent_pair(|x| x as usize == BORDER_1_2 - 1);
    assert_eq!(g.kingdom.campaign.map.county_at(here.0, here.1), 1);
    assert_eq!(g.kingdom.campaign.map.county_at(town.0, town.1), 2);
    g.kingdom.campaign.map.set_flags(town.0, town.1, flags::CASTLE);
    g.kingdom.counties[2].population = 10; // below the defence floor
    let id = army_at(&mut g, 1, 1, 600, here);

    click(&mut m, &mut g, &a, pixel(here.0, here.1).unwrap());
    click(&mut m, &mut g, &a, pixel(town.0, town.1).unwrap());
    assert!(
        g.kingdom.campaign.units.get(id).is_some_and(|u| u.moving),
        "the order was placed from the map",
    );
    end_turn(&mut m, &mut g, &a);
    assert_eq!(
        g.kingdom.counties[2].owner, 1,
        "the county changed hands without the player leaving the map",
    );
    assert_eq!(g.owned_by(1), 2, "and the realm's holding grew");
    assert_eq!(
        m.top_id(),
        Some(ScreenId::Campaign),
        "the map is still what is on screen: raise, march, take, all from here",
    );
}

// ---------------------------------------------------------------------------
// 3. Divide
// ---------------------------------------------------------------------------

#[test]
fn a_selected_army_opens_the_division_screen_and_an_unselected_one_does_not() {
    let (mut g, a, mut m) = on_the_map();
    press(&mut m, &mut g, &a, 'a');
    assert_eq!(m.top_id(), Some(ScreenId::Campaign), "there is nothing selected");

    let (here, _) = adjacent_pair(|x| x < 30);
    let id = army_at(&mut g, 1, 1, 300, here);
    click(&mut m, &mut g, &a, pixel(here.0, here.1).unwrap());
    press(&mut m, &mut g, &a, 'a');
    assert_eq!(m.top_id(), Some(ScreenId::Divide(id)));
}

/// The buttons move men between the two columns and Split makes a second army,
/// with both halves paying the five moves `Army_Split` charges.
#[test]
fn the_division_screen_splits_an_army_in_two_and_both_halves_pay_five_moves() {
    let (mut g, a, mut m) = on_the_map();
    let (here, _) = adjacent_pair(|x| x < 30);
    let id = army_at(&mut g, 1, 1, 300, here);
    click(&mut m, &mut g, &a, pixel(here.0, here.1).unwrap());
    press(&mut m, &mut g, &a, 'a');
    tick(&mut m, &mut g, &a);

    // Row 0 is the peasants; the parent's button pushes ten a click.
    for _ in 0..12 {
        click(&mut m, &mut g, &a, on(divide::parent_button(0)));
    }
    click(&mut m, &mut g, &a, on(divide::SPLIT_TICK));

    assert_eq!(m.top_id(), Some(ScreenId::Campaign), "the screen closed");
    assert_eq!(g.kingdom.campaign.units.len(), 2, "two armies now");
    let parent = g.kingdom.campaign.units.get(id).expect("the parent").clone();
    assert_eq!(parent.men, 180);
    assert_eq!(parent.moves_used, l2_kingdom::divide::SPLIT_MOVE_COST);
    let (_, daughter) = g
        .kingdom
        .campaign
        .units
        .iter()
        .find(|(slot, _)| *slot != id)
        .expect("the daughter");
    assert_eq!(daughter.men, 120);
    assert_eq!(daughter.owner, 1);
    assert_eq!(daughter.morale, parent.morale, "she inherits the parent's morale");
    assert_eq!(daughter.moves_used, l2_kingdom::divide::SPLIT_MOVE_COST);
    assert_eq!(parent.men + daughter.men, 300, "men are conserved");
    assert_ne!((daughter.x, daughter.y), (parent.x, parent.y), "and she stands elsewhere");
}

/// Both halves need fifty men on the plain path, and the refusal keeps the
/// screen open rather than closing on a lie.
#[test]
fn a_split_that_would_leave_fewer_than_fifty_a_side_is_refused_on_the_screen() {
    let (mut g, a, mut m) = on_the_map();
    let (here, _) = adjacent_pair(|x| x < 30);
    let id = army_at(&mut g, 1, 1, 300, here);
    click(&mut m, &mut g, &a, pixel(here.0, here.1).unwrap());
    press(&mut m, &mut g, &a, 'a');
    tick(&mut m, &mut g, &a);

    for _ in 0..3 {
        click(&mut m, &mut g, &a, on(divide::parent_button(0)));
    }
    click(&mut m, &mut g, &a, on(divide::SPLIT_TICK));
    assert_eq!(m.top_id(), Some(ScreenId::Divide(id)), "the screen stays up");
    assert_eq!(g.kingdom.campaign.units.len(), 1, "and nothing was split");
}

/// **Open the information panel on a tile** — the campaign map's right release,
/// which is `g_screenId = 0x04`.
fn right_click(m: &mut Machine, g: &mut Game, a: &Assets, at: (i32, i32)) {
    send(m, g, a, Event::RightClick { x: at.0, y: at.1 });
}

/// The information panel's unit half, and the three buttons that are the door
/// to move-order mode, to a disband and to screen `0x11`.
///
/// `g_infoUnitButtons` (`0x004DC560`) record `i`, at `(48 | 112 | 176, 352)`,
/// 40 square. `Layout { row: 2 }` for the player's own army is what puts them
/// at their table y.
fn info_button(i: usize) -> (i32, i32) {
    on(l2_game::input::Rect::new(
        info::BUTTON_X[i],
        info::BUTTON_DY,
        info::BUTTON_DIM,
        info::BUTTON_DIM,
    ))
}

/// **Disband: the men go home and the weapons go back to the treasury**, which
/// is `Army_Disband` and the Readme's *"Any weapons they are carrying are
/// returned to your treasury."*
///
/// It travels the original's road now: `Panel_DisbandButton` is record 1 of the
/// **information panel's** table and not a button on the division screen, which
/// is where a button of ours used to be. `docs/arms.json`
/// `0x00437002/info-disband`.
#[test]
fn the_disband_button_returns_the_men_to_their_county_and_the_weapons_to_the_realm() {
    let (mut g, a, mut m) = on_the_map();
    let (here, _) = adjacent_pair(|x| x < 30);
    let id = army_at(&mut g, 1, 1, 300, here);
    g.kingdom.campaign.units.get_mut(id).unwrap().troops = [100, 0, 0, 200, 0, 0, 0];
    let (pop, swords) = (g.kingdom.counties[1].population, g.kingdom.realms[1].weapons[2]);

    right_click(&mut m, &mut g, &a, pixel(here.0, here.1).unwrap());
    assert_eq!(
        m.top_id(),
        Some(ScreenId::Info(l2_game::screens::info::Target::Unit(id))),
        "the right release opens the unit half",
    );
    click(&mut m, &mut g, &a, info_button(1));

    assert_eq!(m.top_id(), Some(ScreenId::Campaign));
    assert!(g.kingdom.campaign.units.get(id).is_none(), "the army is gone");
    assert_eq!(g.kingdom.counties[1].population, pop + 300, "the men joined the county");
    assert_eq!(g.kingdom.realms[1].weapons[2], swords + 200, "two hundred swords came back");
}

/// The Readme's disband rule, both clauses, through the screen: an army whose
/// home county has fallen *and* which is standing in enemy country is refused,
/// with `L2.eng` group 145's own words for why.
///
/// **The refusal closes the panel**, which reads wrong and is the original's:
/// `Panel_DisbandButton`'s else branch is `Msg_Enqueue(…, 0x91, …); g_screenId =
/// 0;`. The message scroll is what the player is left looking at, and we have
/// none, so the army standing untouched on the map is the whole of the answer
/// here.
#[test]
fn an_army_with_no_friendly_county_to_go_to_cannot_disband() {
    let (mut g, a, mut m) = on_the_map();
    let (here, _) = adjacent_pair(|x| x < 30);
    let id = army_at(&mut g, 1, 1, 300, here);
    {
        let u = g.kingdom.campaign.units.get_mut(id).unwrap();
        u.home_county = 1;
        u.county = 2; // standing in the enemy's county
    }
    g.kingdom.counties[1].owner = 2; // and home has fallen

    right_click(&mut m, &mut g, &a, pixel(here.0, here.1).unwrap());
    click(&mut m, &mut g, &a, info_button(1));

    assert_eq!(m.top_id(), Some(ScreenId::Campaign), "the panel closes either way");
    assert!(g.kingdom.campaign.units.get(id).is_some(), "and the army is still there");
}

/// **The Move button is the door to screen `0x10`**, and it is the original's
/// only one from this panel: `Panel_MoveButton` writes `g_screenId = 0` and
/// calls `Map_BeginMoveSelection()`. Ours carries the request on
/// [`Game::begin_move_order`] and the map takes it on the next tick.
///
/// Ablating it: delete the `ctx.game.begin_move_order = Some(id)` line in
/// `screens/info.rs` and the second assertion fails — the panel still closes,
/// so a test that only checked the screen would pass.
#[test]
fn the_move_button_on_the_information_panel_starts_a_move_order() {
    let (mut g, a, mut m) = on_the_map();
    let (here, _) = adjacent_pair(|x| x < 30);
    let id = army_at(&mut g, 1, 1, 300, here);

    right_click(&mut m, &mut g, &a, pixel(here.0, here.1).unwrap());
    click(&mut m, &mut g, &a, info_button(0));
    assert_eq!(m.top_id(), Some(ScreenId::Campaign), "the panel closed");
    assert_eq!(g.begin_move_order, Some(id), "and asked for the selection");
    tick(&mut m, &mut g, &a);
    assert_eq!(g.begin_move_order, None, "which the map consumed");
    assert_eq!(
        m.top_id(),
        Some(ScreenId::Campaign),
        "move-order mode is this screen with a selection, not another screen",
    );
}

/// **The Split button pushes rather than replaces, so `0x11` goes back to
/// `0x04`** — every one of the division screen's three exits writes
/// `g_screenId = 0x04`, not 0. `docs/arms.json`
/// `0x0042FF10/back-one-rather-than-to-the-map`.
#[test]
fn the_division_screen_returns_to_the_information_panel_and_not_to_the_map() {
    let (mut g, a, mut m) = on_the_map();
    let (here, _) = adjacent_pair(|x| x < 30);
    let id = army_at(&mut g, 1, 1, 300, here);

    right_click(&mut m, &mut g, &a, pixel(here.0, here.1).unwrap());
    click(&mut m, &mut g, &a, info_button(2));
    assert_eq!(m.top_id(), Some(ScreenId::Divide(id)));
    // The cross — `Army_SplitConfirm` with `g_uiHotspotId == 0`.
    click(&mut m, &mut g, &a, on(divide::SPLIT_CROSS));
    assert_eq!(
        m.top_id(),
        Some(ScreenId::Info(l2_game::screens::info::Target::Unit(id))),
        "back to the panel the Split button was on",
    );
    // And the right release out of `0x11` lands in the same place.
    click(&mut m, &mut g, &a, info_button(2));
    right_click(&mut m, &mut g, &a, (200, 200));
    assert_eq!(m.top_id(), Some(ScreenId::Info(l2_game::screens::info::Target::Unit(id))));
}


// ---------------------------------------------------------------------------
// What lands on the canvas
// ---------------------------------------------------------------------------

/// Paint the stack as it stands.
fn frame(m: &mut Machine, g: &mut Game, a: &Assets) -> l2_view::Canvas {
    let mut c = l2_view::Canvas::screen();
    let ctx = Ctx { game: g, assets: a };
    m.draw(&ctx, &mut c);
    c
}

fn differing(a: &l2_view::Canvas, b: &l2_view::Canvas) -> Vec<(i32, i32)> {
    let mut out = Vec::new();
    for y in 0..480usize {
        for x in 0..640usize {
            if a.at(x, y) != b.at(x, y) {
                out.push((x as i32, y as i32));
            }
        }
    }
    out
}

/// **The division screen paints inside the rectangle its painter opens** and
/// leaves the rest of the frame to the map underneath.
///
/// That second half is the point: `Screen_ArmyDivision` opens a `Ui_DrawBox`
/// over the campaign map and does not clear the screen — *"the original has no
/// screen clear anywhere"* — so a screen that blanked the frame would be wrong
/// in a way no other assertion here notices.
///
/// **The raise-army screen used to be tested with it, and that was the bug.**
/// `Screen_Draw`'s `0x17` arm runs `Screen_Armoury(1)` before
/// `Screen_RaiseArmy`, so the levy window really does repaint the whole frame;
/// asserting that it did not is what kept the screen floating over the campaign
/// map in the wrong palette until a player called it *"a weird popup"*. The
/// test below it is the replacement.
#[test]
fn the_division_screen_paints_inside_the_window_the_painter_opens() {
    let (mut g, a, mut m) = on_the_map();
    // **The setup happens before the reference frame is taken.** Selecting an
    // army changes the *map*: it draws a selection ring, a unit marker and the
    // banner, and a marker on a tile near the top of the viewport lands above
    // the division window's own y. Measuring from a frame taken before the
    // selection would blame the screen for pixels the map drew.
    let (here, _) = adjacent_pair(|x| x < 30);
    army_at(&mut g, 1, 1, 300, here);
    click(&mut m, &mut g, &a, pixel(here.0, here.1).unwrap());
    let before = frame(&mut m, &mut g, &a);

    press(&mut m, &mut g, &a, 'a');
    tick(&mut m, &mut g, &a);
    let after = frame(&mut m, &mut g, &a);

    let window = divide::window();
    let (mut inside, mut above) = (0usize, 0usize);
    for (x, y) in differing(&before, &after) {
        if window.contains(x, y) {
            inside += 1;
        } else if y < window.y {
            above += 1;
        }
    }
    assert!(inside > 500, "the painter drew almost nothing: {inside} pixels");
    assert_eq!(above, 0, "it painted above its own window, over the map");
}

/// **The levy window and the armoury are one surface**, and this is the
/// assertion that says so without any test knowing what the armoury looks like.
///
/// Open the levy screen and paint a frame; press Continue and paint another.
/// Continue replaces `0x17` with `0x0A`, so what leaves the picture is the levy
/// window and what stays is everything else — and *everything else* has to
/// include a lot, because it is a whole room. If the levy screen were still an
/// inset over the campaign map the two frames would differ almost everywhere.
#[test]
fn the_levy_window_lifts_off_the_armoury_and_leaves_the_room_behind() {
    let (mut g, a, mut m) = on_the_map();
    press(&mut m, &mut g, &a, 'r');
    tick(&mut m, &mut g, &a);
    let with_window = frame(&mut m, &mut g, &a);

    click(&mut m, &mut g, &a, on(army::continue_button(false)));
    assert_eq!(m.top_id(), Some(ScreenId::Armoury(1)));
    let armoury_only = frame(&mut m, &mut g, &a);

    let window = army::window(false);
    let changed = differing(&with_window, &armoury_only);
    assert!(!changed.is_empty(), "Continue changed nothing at all");
    let outside = changed.iter().filter(|&&(x, y)| !window.contains(x, y)).count();
    let inside = changed.len() - outside;
    assert!(inside > 500, "the levy window did not lift: {inside} pixels changed inside it");
    // Outside the window the two frames are the same room. Not *identical* —
    // the armoury draws its three labels bright where the levy screen dims them
    // and adds a corner picture — so this is a fraction rather than a zero, and
    // it is a small one: 3,700 pixels out of the 300,000 that are not the
    // window.
    let elsewhere = (640 * 480 - window.w * window.h) as usize;
    assert!(
        outside * 20 < elsewhere,
        "{outside} of {elsewhere} pixels outside the levy window changed: \
         the two screens are not standing on the same picture",
    );
}

// ---------------------------------------------------------------------------
// Turn phase 2
// ---------------------------------------------------------------------------

/// **A siege laid on the map is now carried by the turn.**
///
/// `engagement::run_siege_phase` has been turn phase 2 end to end since it was
/// written and **nothing outside its own tests called it**: `turn::settled`
/// answered the phase-2 wait `true` with the comment *"sieges are out of
/// scope"*, so a besieging army in a played turn built nothing and never
/// assaulted. It is called from `begin_phase` now, and this is the assertion
/// that says so — a palisade needs no engines
/// ([`l2_kingdom::siege::ENGINES_REQUIRED_FROM_LEVEL`] is 3), so the assault
/// launches on the first phase 2 the turn reaches.
#[test]
fn a_siege_laid_on_the_map_is_carried_to_its_assault_by_ending_the_turn() {
    let (mut g, a, mut m) = on_the_map();
    let (camp, keep) = adjacent_pair(|x| x as usize == BORDER_1_2 - 1);
    g.kingdom.counties[2].castle_type = 1; // a wooden palisade: no engines needed
    g.kingdom.counties[2].population = 10;

    let garrison = army_at(&mut g, 2, 2, 40, keep);
    g.kingdom.campaign.units.get_mut(garrison).unwrap().garrison_county = 2;
    g.kingdom.counties[2].garrison_unit = garrison;

    let besieger = army_at(&mut g, 1, 1, 800, camp);
    {
        let u = g.kingdom.campaign.units.get_mut(besieger).unwrap();
        u.besieging_county = 2;
    }
    g.kingdom.campaign.units.get_mut(garrison).unwrap().besieged_by = besieger as u8;

    press(&mut m, &mut g, &a, 'e');
    // Phase 2 is a few ticks into the turn now that a turn takes frames, so the
    // prompt arrives some frames after the keystroke rather than inside it.
    run_until(&mut m, &mut g, &a, "the siege prompt", |m, _| {
        m.top_id() == Some(ScreenId::BattlePrompt)
    });

    // **The turn stops and asks**: the besieger is the human's, so
    // `battle::settlement` says `Prompt` and phase 2 parks its assault on
    // screen `0x12` instead of settling it silently. The player has to answer
    // before the campaign moves again.
    assert_eq!(
        m.top_id(),
        Some(ScreenId::BattlePrompt),
        "the assault should have raised the prompt",
    );
    assert!(
        g.kingdom.campaign.units.get(garrison).is_some()
            && g.kingdom.campaign.units.get(besieger).is_some(),
        "and nothing is resolved while the question is on the table",
    );

    // Decline — the autocalc, which is what this test always ran.
    click(&mut m, &mut g, &a, on(battle::widget_rect(battle::DECLINE)));
    assert_eq!(m.top_id(), Some(ScreenId::BattleResult), "then the result screen");
    click(&mut m, &mut g, &a, on(battle::ok_rect()));
    // The map's own `update` picks the suspended turn back up the moment the
    // result screen is gone.
    for _ in 0..4 {
        tick(&mut m, &mut g, &a);
    }
    assert_eq!(m.top_id(), Some(ScreenId::Campaign), "and the turn finished");

    assert!(
        g.kingdom.campaign.units.get(garrison).is_none()
            || g.kingdom.campaign.units.get(besieger).is_none(),
        "phase 2 launched the assault and one side of it is gone",
    );
    assert!(
        g.kingdom
            .campaign
            .units
            .get(besieger)
            .is_none_or(|u| u.besieging_county == 0),
        "and the siege link is broken either way",
    );
}

// ---------------------------------------------------------------------------
// "Will you take the field?"
// ---------------------------------------------------------------------------

/// Put the player's army next to an enemy's and march it in, so the turn's unit
/// sweep raises a battle.
fn a_battle_is_about_to_happen() -> (Game, Assets, Machine, usize, usize) {
    let (mut g, a, m) = on_the_map();
    let (mine, theirs) = adjacent_pair(|x| x as usize == BORDER_1_2 - 1);
    let attacker = army_at(&mut g, 1, 1, 400, mine);
    let defender = army_at(&mut g, 2, 2, 200, theirs);
    // The order is given through `l2-kingdom` rather than through two clicks:
    // the map screen's second click on an *enemy* army is a selection, not an
    // attack order, and what is under test here is the prompt rather than the
    // route into it.
    let map = g.kingdom.campaign.map.clone();
    l2_kingdom::movement::order_move(
        &map,
        &mut g.kingdom.campaign.units,
        attacker,
        theirs,
        l2_kingdom::movement::Routing::Direct,
    )
    .expect("a path one tile long");
    (g, a, m, attacker, defender)
}

/// **The player is asked, and the campaign does not move while he thinks.**
///
/// This is the gap the whole prompt closes: `end_turn` used to answer
/// `Answer::Decline` for him, because there was no screen to ask on. Ending the
/// turn now stops on screen `0x12` with both armies still standing.
#[test]
fn ending_a_turn_into_a_battle_asks_the_player_before_anything_is_decided() {
    let (mut g, a, mut m, attacker, defender) = a_battle_is_about_to_happen();
    press(&mut m, &mut g, &a, 'e');

    assert_eq!(m.top_id(), Some(ScreenId::BattlePrompt), "the prompt should be up");
    assert!(
        g.kingdom.campaign.units.get(attacker).is_some()
            && g.kingdom.campaign.units.get(defender).is_some(),
        "neither army may be touched while the question is on the table",
    );
    // And the question knows who is asking and what each side is bringing.
    let q = l2_game::turn::pending_question(&g).expect("a question");
    assert_eq!(q.choice_owner, 1, "the local player holds the choice");
    assert_eq!(q.attacker_men, 400);
    assert_eq!(q.defender_men, 200);
    assert!(!q.is_siege);
}

/// **Both answers reach the simulation, and they are different battles.**
///
/// Taking the field raises the **battlefield** — `FUN_0043B593` with hotspot id
/// 1 calls `Battle_Start`, not the autocalc — and declining runs the autocalc.
/// The test that they are genuinely two paths is that the report says so.
#[test]
fn the_two_thumbs_reach_the_two_ways_a_battle_can_be_settled() {
    use l2_game::engagement::Resolution;

    for (thumb, want_fought) in [(battle::TAKE_THE_FIELD, true), (battle::DECLINE, false)] {
        let (mut g, a, mut m, _, _) = a_battle_is_about_to_happen();
        press(&mut m, &mut g, &a, 'e');
        assert_eq!(m.top_id(), Some(ScreenId::BattlePrompt));
        click(&mut m, &mut g, &a, on(battle::widget_rect(thumb)));

        if want_fought {
            // The battlefield, and it is **paused** — `Battle_Start` writes
            // `DAT_0053F238 = 0xFFFFFFFF` before it raises the screen.
            assert_eq!(m.top_id(), Some(ScreenId::Battlefield), "the field, not the result");
            assert!(g.battle.as_ref().expect("a live battle").paused, "a battle starts paused");
            // Nothing happens while it is paused, however many frames pass.
            for _ in 0..200 {
                tick(&mut m, &mut g, &a);
            }
            assert_eq!(g.battle.as_ref().expect("still there").runner.tick, 0);
            // Unpause and let it run. A battle is thousands of ticks long, so
            // this is a longer loop than `run_until`'s — it is the same thing.
            click(&mut m, &mut g, &a, on(bf::Button::Pause.rect()));
            assert!(!g.battle.as_ref().expect("a live battle").paused);
            for _ in 0..40_000 {
                if g.battle.as_ref().is_some_and(|b| b.conclusion.is_some()) {
                    break;
                }
                tick(&mut m, &mut g, &a);
            }
            assert!(
                g.battle.as_ref().is_some_and(|b| b.conclusion.is_some()),
                "the battle did not reach a conclusion",
            );
            // The banner is up, and it holds for five thousand frames unless a
            // right release skips it. Skip it, which is `0x2B`'s only arm.
            assert_eq!(g.battle.as_ref().map(|b| b.screen_id()), Some(0x2B));
            send(&mut m, &mut g, &a, Event::RightClick { x: 100, y: 300 });
            tick(&mut m, &mut g, &a);
        }

        assert_eq!(m.top_id(), Some(ScreenId::BattleResult), "the result screen follows");
        let r = l2_game::turn::pending_report(&g).expect("a settled battle");
        match (r.resolution, want_fought) {
            (Resolution::Fought { ticks, .. }, true) => assert!(ticks > 0),
            (Resolution::Autocalc, false) => {}
            (other, _) => panic!("{other:?} for thumb at {:?}", thumb.0),
        }
        // Whichever way it went, one army is gone and one is not.
        assert_ne!(
            r.attacker_men.1 == 0,
            r.defender_men.1 == 0,
            "exactly one side should have been destroyed: {:?} {:?}",
            r.attacker_men,
            r.defender_men
        );

        // The corner dismisses it and the turn carries on to its end — over as
        // many frames as the rest of the turn takes, which is what a turn
        // having a duration means.
        click(&mut m, &mut g, &a, on(battle::ok_rect()));
        run_until(&mut m, &mut g, &a, "the rest of the turn", |_, g| {
            !l2_game::turn::turn_in_flight(g)
        });
        assert_eq!(m.top_id(), Some(ScreenId::Campaign), "and the turn finished");
        assert!(!l2_game::turn::turn_in_flight(&g), "nothing left suspended");
    }
}

/// **Screen `0x12` has exactly two exits and neither of them is a button on the
/// mouse or a key on the keyboard.**
///
/// `Screen_FrameInput`'s `0x12` arm is two `if`s, both multiplayer: the sync
/// latch, and `FUN_004BBEA7`, which returns 0 outright when `g_multiplayer` is
/// clear. So in a single-player game the arm does **nothing at all** and the
/// prompt waits for ever, which `docs/symbols.json` records of `Battle_Decline`
/// and which is correct rather than a hang.
///
/// We had right-click-to-Decline, Escape-to-Decline and Enter-to-fight here.
/// All three were ours; `docs/decisions.md` C67 counts them, and `docs/arms.json` is where they are
/// counted.
///
/// **Ablated**: putting any one of the three back turns this red.
#[test]
fn the_prompt_has_no_way_out_but_its_two_widgets() {
    let (mut g, a, mut m, _, _) = a_battle_is_about_to_happen();
    press(&mut m, &mut g, &a, 'e');
    assert_eq!(m.top_id(), Some(ScreenId::BattlePrompt));

    // A hundred ticks: no timeout.
    for _ in 0..100 {
        tick(&mut m, &mut g, &a);
    }
    // Every gesture the original does not have, inside the window and outside
    // it, and none of them may settle anything.
    let window = battle::window();
    for e in [
        Event::RightClick { x: 0, y: 0 },
        Event::RightClick { x: window.centre_x(), y: window.y + 8 },
        Event::KeyDown(Key::Escape),
        Event::KeyDown(Key::Enter),
        Event::KeyDown(Key::letter('Y')),
        // A left click on the window that is on neither widget.
        Event::Click { x: window.x + 8, y: window.y + 8 },
        Event::DoubleClick { x: window.centre_x(), y: window.y + 8 },
    ] {
        send(&mut m, &mut g, &a, e);
        assert_eq!(m.top_id(), Some(ScreenId::BattlePrompt), "{e:?} left the prompt");
        assert!(l2_game::turn::pending_report(&g).is_none(), "{e:?} settled the battle");
        assert!(g.battle.is_none(), "{e:?} raised a battlefield");
    }
    // And the two that do work still work.
    click(&mut m, &mut g, &a, on(battle::widget_rect(battle::DECLINE)));
    let r = l2_game::turn::pending_report(&g).expect("the thumb down declines");
    assert_eq!(r.resolution, l2_game::engagement::Resolution::Autocalc);
}

/// **A battle a player watches and gives no orders in produces exactly the
/// battle nobody watches.**
///
/// This is the assertion that lets orders be added safely: a player's orders
/// change what a player *can do*, and must not change what the same inputs
/// produce. The two paths share [`l2_game::engagement::begin_fight`] and
/// [`l2_game::engagement::conclude_fight`]; what differs is who supplies the
/// ticks, and the tick loops differ in grain — the headless one asks whether the
/// battle is over every hundredth tick and the played one every tick.
#[test]
fn watching_a_battle_and_giving_no_orders_reproduces_the_headless_verdict() {
    // The headless path: `end_turn`, whose policy answers Decline — so force
    // the fought path by resolving the same pair directly.
    let (mut g, _a, _m, attacker, defender) = a_battle_is_about_to_happen();
    let county = g.kingdom.campaign.units.get(defender).map_or(0, |u| u.county);
    let seed = 0x5EED_BEEF;
    let mut headless = g.kingdom.clone();
    let mut runner = l2_game::engagement::begin_fight(&mut headless, attacker, defender, None, seed)
        .expect("two armies");
    while runner.tick < 12_000 && runner.conclusion().is_none() {
        runner.run(100);
    }
    let (want_verdict, _) =
        l2_game::engagement::conclude_fight(&mut headless, attacker, defender, runner);

    // The watched path: the same two armies, the same seed, one tick at a time.
    let mut watched = g.kingdom.clone();
    let mut runner =
        l2_game::engagement::begin_fight(&mut watched, attacker, defender, None, seed)
            .expect("two armies");
    let mut live = l2_game::battlefield::LiveBattle::new(runner, attacker, defender, county, None, 1, 1);
    live.paused = false;
    for _ in 0..12_000 {
        live.tick();
        if live.conclusion.is_some() {
            break;
        }
    }
    runner = live.runner;
    let (got_verdict, _) =
        l2_game::engagement::conclude_fight(&mut watched, attacker, defender, runner);

    assert_eq!(got_verdict, want_verdict, "a watched battle changed its own outcome");
    for id in [attacker, defender] {
        assert_eq!(
            headless.campaign.units.get(id).map(|u| u.troops),
            watched.campaign.units.get(id).map(|u| u.troops),
            "unit {id} came off the field with different men",
        );
    }
    let _ = &mut g;
}

/// Put the battle camera over the local player's own men.
///
/// `Battle_Start` seeds it at cell (0x20, 0x21) and the armies deploy at the two
/// markers, which may be nowhere near it — the original's player scrolls or
/// clicks the overview panel. A test that boxed the opening viewport and found
/// nothing would be asserting on its own emptiness, which is exactly the failure
/// `docs/agents.md` names.
fn look_at_the_players_men(g: &mut Game) {
    // The local realm is 1 and it owns the attacking army, which
    // `Battle_InitArmies` raises as army A — side 4, the one that is not
    // `SIDE_A` in `l2-sim`'s naming.
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

/// **A player gives an order, and it reaches the simulation.**
///
/// The whole chain, driven with [`Event`] values and nothing else: press on the
/// field opens the drag (`g_screenId` `0x2A`), release commits the box
/// (`FUN_0043C247`), the men are selected, a release on empty ground is an order
/// (`FUN_0043C57D` → `BattleUnit_Order`), and the unit's destination is the cell
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

    // Now order them somewhere empty. Pick a cell in the viewport with no man
    // on it, and read the unit's destination back out of `l2-sim`.
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
    // **Press then release, the way a player does it**, because that is the
    // path that matters: the press opens the drag (`0x2A`), the release moves
    // nothing and hits nobody, `FUN_0043BF07` therefore *declines*, and the
    // order arm behind it in the ladder gets the release. A test that released
    // without pressing would exercise a state the original never reaches.
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

/// **A finished box does not also order at the corner it ended on.**
///
/// `Screen_FrameInput`'s `0x2A` arm is a ladder and `FUN_0043BF07` is above
/// `FUN_0043C57D`; when the drag guard consumes the release the dispatcher
/// `goto`s past the order guard. Ours ran both for a while, which is the sort of
/// thing that only shows up as *"my men wander off after I select them"*.
///
/// **Ablated**: making `release_field` return `true` for every case, or running
/// `order_at` unconditionally after it, turns this red.
#[test]
fn committing_a_selection_box_does_not_also_issue_an_order() {
    let (mut g, a, mut m, _, _) = a_battle_is_about_to_happen();
    press(&mut m, &mut g, &a, 'e');
    click(&mut m, &mut g, &a, on(battle::widget_rect(battle::TAKE_THE_FIELD)));
    click(&mut m, &mut g, &a, on(bf::Button::Pause.rect()));
    look_at_the_players_men(&mut g);

    // A destination nothing could have been ordered to before the box.
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
    // The corner the box ended on, which is where a leaked order would land.
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

/// **The right button clears the selection; it does not leave the battle.**
///
/// `FUN_0043C2A9`'s right half is `FUN_0043C55C`, a deselect, and it is refused
/// only inside the overview panel. This is the arm the "right click exits"
/// habit would have replaced — `docs/decisions.md` C61's second pattern.
///
/// **Ablated**: making the right button pop the screen turns this red on the
/// first assertion.
#[test]
fn the_right_button_on_the_battlefield_clears_the_selection() {
    let (mut g, a, mut m, _, _) = a_battle_is_about_to_happen();
    press(&mut m, &mut g, &a, 'e');
    click(&mut m, &mut g, &a, on(battle::widget_rect(battle::TAKE_THE_FIELD)));
    click(&mut m, &mut g, &a, on(bf::Button::Pause.rect()));
    look_at_the_players_men(&mut g);
    // Select everything in view.
    send(&mut m, &mut g, &a, Event::Pointer { x: bf::VIEW.x + 4, y: bf::VIEW.y + 4 });
    send(&mut m, &mut g, &a, Event::Click { x: bf::VIEW.x + 4, y: bf::VIEW.y + 4 });
    let far = (bf::VIEW.x + bf::VIEW.w - 4, bf::VIEW.y + bf::VIEW.h - 4);
    send(&mut m, &mut g, &a, Event::Pointer { x: far.0, y: far.1 });
    send(&mut m, &mut g, &a, Event::Release { x: far.0, y: far.1 });

    // A right release inside the overview panel is refused — and note the
    // off-by-one the original carries: the panel starts at 0x1E0 and the guard
    // tests 0x1E1, so its leftmost column deselects after all.
    let before = g.battle.as_ref().expect("a live battle").runner.selected_count(1);
    // **The fixture has to be non-empty or the last assertion checks nothing.**
    // `docs/agents.md`: a field is only tested if something a test reads was
    // written by something the game runs. Ablating the deselect passed this
    // test until this line existed.
    assert!(before > 0, "the box picked nobody, so clearing it proves nothing");
    send(&mut m, &mut g, &a, Event::RightClick { x: 0x1E8, y: 0x40 });
    assert_eq!(
        g.battle.as_ref().expect("a live battle").runner.selected_count(1),
        before,
        "the overview panel swallows the right button",
    );
    assert_eq!(m.top_id(), Some(ScreenId::Battlefield), "and it is still a battle");

    // Anywhere else clears it, and still does not leave.
    send(&mut m, &mut g, &a, Event::RightClick { x: 100, y: 100 });
    assert_eq!(
        g.battle.as_ref().expect("a live battle").runner.selected_count(1),
        0,
        "the right button did not clear the selection",
    );
    assert_eq!(m.top_id(), Some(ScreenId::Battlefield), "right-click must not leave the battle");
    assert!(g.battle.is_some());
}

/// The prompt draws inside its own window and nowhere else — the same
/// assertion the siege screen carries, for the same reason.
#[test]
fn the_prompt_paints_inside_its_window_and_over_nothing_above_it() {
    let (mut g, a, mut m, _, _) = a_battle_is_about_to_happen();
    let mut before = l2_view::Canvas::screen();
    {
        let ctx = Ctx { game: &mut g, assets: &a };
        m.draw(&ctx, &mut before);
    }
    press(&mut m, &mut g, &a, 'e');
    assert_eq!(m.top_id(), Some(ScreenId::BattlePrompt));
    let mut after = l2_view::Canvas::screen();
    {
        let ctx = Ctx { game: &mut g, assets: &a };
        m.draw(&ctx, &mut after);
    }

    let window = battle::window();
    let (mut inside, mut above) = (0usize, 0usize);
    for y in 0..480usize {
        for x in 0..640usize {
            if before.at(x, y) == after.at(x, y) {
                continue;
            }
            let (px, py) = (x as i32, y as i32);
            if window.contains(px, py) {
                inside += 1;
            } else if py < window.y {
                above += 1;
            }
        }
    }
    assert!(inside > 500, "the prompt drew almost nothing: {inside} pixels");
    assert_eq!(above, 0, "it painted above its own window");
}

// ---------------------------------------------------------------------------
// …and the march that gets there **before** End Turn is pressed
// ---------------------------------------------------------------------------
//
// Every test above presses `e` in the same breath as the order, so the army
// always arrives inside the turn machine. A player does not: he gives the order
// and watches it walk, which is `turn::tick_units_only` on an ordinary frame.
// That door had no way to raise screen `0x12`, so his battle was settled by
// `Game::field_policy` — `Answer::Decline`, the autocalc — with nothing shown.
// A player reported it as *"it was me attacking a town and it just immediately
// resolved."*

/// Put the player's army one tile from **county 2's town** — plane-0 `0x40`,
/// which is `Entry::Castle` and the only tile `Army_AttackCounty`
/// (`0x004A6C68`) is reached from — and order the march. No turn is ended.
fn a_town_is_about_to_be_attacked() -> (Game, Assets, Machine, usize) {
    let (mut g, a, m) = on_the_map();
    let (mine, town) = adjacent_pair(|x| x as usize == BORDER_1_2 - 1);
    g.kingdom.campaign.map.set_flags(town.0, town.1, flags::CASTLE);
    g.kingdom.counties[2].anchor_x = town.0;
    g.kingdom.counties[2].anchor_y = town.1;
    let attacker = army_at(&mut g, 1, 1, 400, mine);
    g.order_unit_move(attacker, town).expect("a path one tile long");
    (g, a, m, attacker)
}

/// **The report: a town attacked from the map resolved with no prompt.**
///
/// `Battle_ChooseSettlement` (`0x004A6A30`) does not care which frame the army
/// arrived on — an army whose owner is a person is `Settlement::Prompt` and
/// screen `0x12` goes up. Ours only asked when the arrival happened inside the
/// turn machine.
#[test]
fn a_town_attacked_while_the_player_watches_asks_before_it_resolves() {
    let (mut g, a, mut m, attacker) = a_town_is_about_to_be_attacked();
    let before = g.kingdom.turn_count;

    run_until(&mut m, &mut g, &a, "the battle prompt", |m, _| {
        m.top_id() == Some(ScreenId::BattlePrompt)
    });

    assert_eq!(g.kingdom.turn_count, before, "no turn was ended: he was watching");
    assert!(
        g.kingdom.campaign.units.get(attacker).is_some(),
        "nothing may be resolved while the question is on the table",
    );
    let q = l2_game::turn::pending_question(&g).expect("a question");
    assert_eq!(q.choice_owner, 1, "the local player holds the choice");
    assert_eq!(q.attacker_men, 400);
    assert_eq!(q.county, 2, "the county fought over is the defender's");
    assert!(!q.is_siege);
}

/// The same door, the other way in: two armies meeting on an ordinary frame —
/// `Unit_EnterOccupiedTile` (`0x004658C1`) rather than `Army_AttackCounty`.
#[test]
fn an_army_walked_into_while_the_player_watches_asks_too() {
    let (mut g, a, mut m, attacker, defender) = a_battle_is_about_to_happen();
    run_until(&mut m, &mut g, &a, "the battle prompt", |m, _| {
        m.top_id() == Some(ScreenId::BattlePrompt)
    });
    assert!(
        g.kingdom.campaign.units.get(attacker).is_some()
            && g.kingdom.campaign.units.get(defender).is_some(),
        "neither army may be touched while the question is on the table",
    );
}

/// **And the answer settles it and hands the map back**, with no turn ever
/// having been started. This is the half that would break if the suspension
/// were done by faking a turn: declining runs the autocalc, `0x13` shows the
/// result, and dismissing it must leave the campaign where it was rather than
/// winding a season on.
#[test]
fn answering_a_watched_battle_settles_it_and_does_not_end_the_turn() {
    let (mut g, a, mut m, attacker) = a_town_is_about_to_be_attacked();
    let before = g.kingdom.turn_count;
    run_until(&mut m, &mut g, &a, "the battle prompt", |m, _| {
        m.top_id() == Some(ScreenId::BattlePrompt)
    });

    click(&mut m, &mut g, &a, on(battle::widget_rect(battle::DECLINE)));
    assert_eq!(m.top_id(), Some(ScreenId::BattleResult), "the result screen follows");
    let r = l2_game::turn::pending_report(&g).expect("a settled battle");
    assert_eq!(r.resolution, l2_game::engagement::Resolution::Autocalc);
    assert_eq!(r.attacker_men.0, 400);

    click(&mut m, &mut g, &a, on(battle::ok_rect()));
    run_until(&mut m, &mut g, &a, "the map to come back", |m, g| {
        m.top_id() == Some(ScreenId::Campaign) && !l2_game::turn::turn_in_flight(g)
    });
    assert_eq!(g.kingdom.turn_count, before, "answering a battle is not ending a turn");
    assert!(
        g.kingdom.campaign.units.get(attacker).is_none()
            || g.kingdom.counties[2].owner == 1,
        "the battle was actually fought: he lost the army or he took the county",
    );
}

/// **An AI's battle the player is not in still never opens a screen.**
///
/// `Battle_ChooseSettlement`'s first line — `if (!A.ownerIsHuman &&
/// !B.ownerIsHuman) return 0` — and the reason the fix cannot be *"raise the
/// prompt whenever a battle happens on a frame"*. Realm 2 marches on realm 3's
/// town while the human watches, and the map stays up.
///
/// **Ablated**: dropping the `Settlement` test in `tick_units_only` and always
/// suspending turns this red.
#[test]
fn two_ai_armies_meeting_while_the_player_watches_open_nothing() {
    let (mut g, a, mut m) = on_the_map();
    g.kingdom.counties[3].owner = 3;
    g.kingdom.realms[3].in_play = true;
    g.kingdom.realms[3].strength = 5;
    let (mine, town) = adjacent_pair(|x| x as usize == BORDER_1_2 - 1);
    // County 2's town, attacked by realm 3 — neither realm is a person's.
    g.kingdom.campaign.map.set_flags(town.0, town.1, flags::CASTLE);
    g.kingdom.counties[2].anchor_x = town.0;
    g.kingdom.counties[2].anchor_y = town.1;
    let attacker = army_at(&mut g, 3, 3, 400, mine);
    // Not `Game::order_unit_move`: that door is the player's own, and this army
    // is not his.
    let map = g.kingdom.campaign.map.clone();
    l2_kingdom::movement::order_move(
        &map,
        &mut g.kingdom.campaign.units,
        attacker,
        town,
        l2_kingdom::movement::Routing::Direct,
    )
    .expect("a path one tile long");

    for _ in 0..8 {
        tick(&mut m, &mut g, &a);
    }
    // **No PROMPT, and nothing suspended.** This used to assert the campaign map
    // was on top, which was the same thing until the message window landed: a
    // battle between two other lords now raises a report, and a scroll over the
    // map is the original's own behaviour rather than a screen of ours. The
    // claim being made here has always been about the prompt and the turn
    // machine, so it is now made about those directly — the earlier form was a
    // stronger assertion than the test's own doc comment claims, and it went red
    // for a correct change.
    assert_ne!(
        m.top_id(),
        Some(ScreenId::BattlePrompt),
        "somebody else's war asked the player to take the field",
    );
    assert_ne!(
        m.top_id(),
        Some(ScreenId::Battlefield),
        "somebody else's war put the player on the field",
    );
    assert!(
        matches!(m.top_id(), Some(ScreenId::Campaign) | Some(ScreenId::Message)),
        "unexpected screen {:?} for somebody else's war",
        m.top_id(),
    );
    assert!(l2_game::turn::pending_question(&g).is_none());
    assert!(l2_game::turn::pending_report(&g).is_none());
    assert!(!l2_game::turn::turn_in_flight(&g), "and nothing left suspended");
}

// ---------------------------------------------------------------------------
// A turn takes time, and so does a march
// ---------------------------------------------------------------------------

/// **An army the player orders walks away while he is watching it.**
///
/// The defect a player reported as *"can't seem to move my army"* had two
/// halves and this is the structural one. `Units_Tick` has one call site in the
/// original and it is the **frame loop**, not a phase (`docs/decisions.md`
/// C35), so it runs whenever the game is up — including all the time the human
/// is looking at the map. Ours only ran it from inside a turn, so an order was
/// accepted, `moving` was set, a path was written and drawn, and the figure did
/// not move until the turn was ended. From the player's chair that is
/// indistinguishable from the order having been ignored.
///
/// So: order a march, tick the screen the way `main.rs` does, and the army has
/// to be somewhere else — with no turn ended.
#[test]
fn an_army_ordered_from_the_map_walks_while_the_player_watches() {
    let (mut g, a, mut m) = on_the_map();
    let (here, there) = adjacent_pair(|x| x < 30);
    let id = army_at(&mut g, 1, 1, 300, here);

    click(&mut m, &mut g, &a, pixel(here.0, here.1).unwrap());
    click(&mut m, &mut g, &a, pixel(there.0, there.1).unwrap());
    assert!(g.kingdom.campaign.units.get(id).is_some_and(|u| u.moving), "ordered");
    assert_eq!(g.kingdom.campaign.units.get(id).map(|u| u.tile()), Some(here));

    let before = g.kingdom.turn_count;
    for _ in 0..8 {
        tick(&mut m, &mut g, &a);
    }
    assert_eq!(g.kingdom.turn_count, before, "no turn was ended");
    assert_eq!(
        g.kingdom.campaign.units.get(id).map(|u| u.tile()),
        Some(there),
        "the army walked to the tile it was sent to, without the turn being ended",
    );
}

/// **A unit's walk is bounded by its own allowance, not by how long the player
/// sits there.**
///
/// The other side of the change above: `Units_Tick` running every frame must
/// not hand out free movement. `moveAllowance - movesUsed` is the budget and
/// `Pass::UnitsResetMoves` at the end of the season refills it, so a player who
/// leaves the map open for a thousand frames gets exactly the same march as one
/// who ends the turn immediately.
#[test]
fn watching_the_map_does_not_give_an_army_extra_movement() {
    let (mut g, a, mut m) = on_the_map();
    let (here, _) = adjacent_pair(|x| x < 30);
    let id = army_at(&mut g, 1, 1, 300, here);
    // Far enough that the allowance runs out first, and still on the grid: an
    // army has fifteen points and open ground costs three a tile, so five tiles
    // is the whole season's march and this asks for six times that. The opening
    // viewport only shows tiles with a high `y`, so the room is northwards.
    let far = (here.0, here.1.saturating_sub(30));
    assert!(here.1 - far.1 > 20, "the destination is well out of one season's reach");
    g.order_unit_move(id, far).expect("a path across open ground");
    let allowance = g.kingdom.campaign.units.get(id).map_or(0, |u| u.move_allowance);

    for _ in 0..1_000 {
        tick(&mut m, &mut g, &a);
    }
    let u = g.kingdom.campaign.units.get(id).expect("still there");
    assert!(!u.moving, "the army ran out of moves and stopped");
    assert!(
        u.moves_used <= allowance,
        "a thousand frames spent {} of an allowance of {allowance}",
        u.moves_used,
    );
    assert_ne!(u.tile(), far, "and it did not arrive: the season's budget is the bound");
}

/// **One press never both selects an army and orders it.**
///
/// The original needs `g_moveOrderClickGuard` — forty frames of deadness — for
/// this, because `Screen_FrameInput` polls the mouse button's *level* every
/// frame, so one physical press is read as a click on every frame it is held
/// down and the press that opened move-order mode would otherwise be read again
/// as the press that confirms the destination.
///
/// **Ours needs no guard, and this is why**: `Event::Click` is edge-triggered —
/// `main.rs` synthesises exactly one per `WindowEvent::MouseInput{Pressed}` —
/// and `Map_Click`'s army branch `return`s, so the selecting click cannot fall
/// through to `Map_ConfirmMoveOrder` in the same call. The guard is a
/// consequence of a polled input model we do not have. This asserts the
/// property the guard exists to protect, rather than porting a frame count that
/// would mean nothing here.
#[test]
fn the_click_that_selects_an_army_never_also_orders_it() {
    let (mut g, a, mut m) = on_the_map();
    let (here, _) = adjacent_pair(|x| x < 30);
    let id = army_at(&mut g, 1, 1, 300, here);

    click(&mut m, &mut g, &a, pixel(here.0, here.1).unwrap());
    let u = g.kingdom.campaign.units.get(id).expect("still there");
    assert!(!u.moving, "the selecting click did not also place an order");
    assert_eq!(u.dest, None, "and named no destination");
    assert!(u.path.is_empty());
    assert_eq!(u.moves_used, 0, "and cost nothing");

    // Repeating the same press — which is what a held button looks like to a
    // polled reader — orders nothing either, by the original's own route: the
    // tile is the army's own, the fill's distance there is `START_DISTANCE`,
    // and `Map_ConfirmMoveOrder` returns on `g_moveOrderAvailable != 1`.
    click(&mut m, &mut g, &a, pixel(here.0, here.1).unwrap());
    let u = g.kingdom.campaign.units.get(id).expect("still there");
    assert!(!u.moving, "and neither did the second one");
}

/// **The screen goes dark, and it goes dark at the season boundary.**
///
/// A player: *"the merchants move and then it fades out then in which hides the
/// season change visuals just abruptly changing"*, and *"the screen doesn't go
/// dark"* when it did not. `FUN_004B0CB4`'s two call sites are `Turn_Tick`'s
/// phase 7 with `rawFlag = 1` right after `Season_Advance`, and `FUN_0049A3E6`
/// with `0` after the seasonal art is reloaded — so the order is: units walk,
/// season advances, fade down, art swaps in the dark, fade up.
///
/// This asserts that order through the one thing a screen exposes about it,
/// `Screen::fade`: nothing while the phases run, then a full down-and-up that
/// bottoms out exactly once.
#[test]
fn the_end_of_a_turn_fades_the_screen_down_and_back_up() {
    use l2_game::screen::Screen;
    let (mut g, a) = world();
    let mut s = map::MapScreen::new();

    assert_eq!(s.fade(), None, "no fade before the turn");
    let mut ctx = Ctx { game: &mut g, assets: &a };
    s.handle(Event::KeyDown(Key::letter('e')), &mut ctx);
    assert_eq!(s.fade(), None, "and none while the phase machine is still running");

    let mut seen: Vec<u8> = Vec::new();
    let mut season_at = None;
    let before = g.kingdom.turn_count;
    for n in 0..2_000u32 {
        let mut ctx = Ctx { game: &mut g, assets: &a };
        s.update(&mut ctx);
        if season_at.is_none() && g.kingdom.turn_count > before {
            season_at = Some(n);
        }
        match s.fade() {
            Some(p) => seen.push(p),
            None if season_at.is_some() && !seen.is_empty() => break,
            None => {}
        }
    }

    assert!(season_at.is_some_and(|n| n > 4), "the season advanced, and took frames doing it");
    assert_eq!(seen.first(), Some(&0), "the fade starts at full brightness");
    assert_eq!(
        seen.len(),
        l2_view::fade::PHASES as usize,
        "one phase per tick, all the way down and back up",
    );
    assert!(seen.windows(2).all(|w| w[1] == w[0] + 1), "and strictly in order");
    assert_eq!(
        seen.iter().filter(|&&p| l2_view::fade::is_darkest(p)).count(),
        1,
        "it bottoms out exactly once",
    );
    assert_eq!(s.fade(), None, "and the light is fully back afterwards");
}

/// **The map stops taking orders while the turn is being wound on.**
///
/// Every hotspot on the map writes to state the phase machine is in the middle
/// of reading. A click that landed mid-turn would race it, so it is refused —
/// and pointer motion is *not*, because a frozen cursor would look like a hang
/// rather than like a turn passing.
#[test]
fn the_map_refuses_orders_while_the_turn_is_running() {
    let (mut g, a, mut m) = on_the_map();
    let (here, there) = adjacent_pair(|x| x < 30);
    let id = army_at(&mut g, 1, 1, 300, here);

    press(&mut m, &mut g, &a, 'e');
    tick(&mut m, &mut g, &a);
    assert!(l2_game::turn::turn_in_flight(&g), "a turn is in flight");

    click(&mut m, &mut g, &a, pixel(here.0, here.1).unwrap());
    click(&mut m, &mut g, &a, pixel(there.0, there.1).unwrap());
    assert_eq!(
        g.kingdom.campaign.units.get(id).and_then(|u| u.dest),
        None,
        "no order was taken from a click during the turn",
    );
    // Pointer motion still arrives, so the map can still scroll under the
    // cursor while the season winds on.
    send(&mut m, &mut g, &a, Event::Pointer { x: 4, y: 4 });
    assert_eq!(m.top_id(), Some(ScreenId::Campaign));
}

// ---------------------------------------------------------------------------
// The shell table
// ---------------------------------------------------------------------------

/// Both screens have left the shell table, which is the measure of progress the
/// table's own documentation names.
#[test]
fn the_two_screens_have_graduated_out_of_the_shell_table() {
    use l2_game::screens::shells;
    assert!(shells::find(0x17).is_none(), "the raise-army screen is implemented");
    assert!(shells::find(0x11).is_none(), "the army-division screen is implemented");
    assert!(
        !shells::SHELLS.iter().any(|s| s.name.to_lowercase().contains("mercenaries")),
        "and nothing in the table still claims there is a mercenaries screen",
    );
}

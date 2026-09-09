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
use l2_game::screens::{army, divide, map};
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

/// The whole verb by mouse: open the screen, put the slider at 30 %, equip, and
/// press Raise. An army exists afterwards that did not before, and the county
/// has paid for it in people and in happiness.
#[test]
fn the_levy_slider_and_the_raise_button_put_an_army_on_the_map() {
    let (mut g, a, mut m) = on_the_map();
    press(&mut m, &mut g, &a, 'r');
    tick(&mut m, &mut g, &a);
    assert_eq!(g.kingdom.campaign.units.len(), 0, "nothing on the map yet");
    let (pop, happy) = (g.kingdom.counties[1].population, g.kingdom.counties[1].happiness);

    // `FUN_00435CEF`: the track is `x - 0xC4` over the row
    // `base + 0x10 ..< base + 0x40`, and with no band on offer `base` is 0xA0.
    click(&mut m, &mut g, &a, (army::SLIDER_X + 30, army::base(false) + 0x20));
    click(&mut m, &mut g, &a, on(army::auto_button()));
    click(&mut m, &mut g, &a, on(army::raise_button()));

    assert_eq!(m.top_id(), Some(ScreenId::Campaign), "the screen closed on a successful raise");
    assert_eq!(g.kingdom.campaign.units.len(), 1, "one army");
    let (id, unit) = g.kingdom.campaign.units.iter().next().expect("the army");
    assert_eq!(unit.kind, UnitKind::Army);
    assert_eq!(unit.owner, 1);
    assert_eq!(unit.home_county, 1, "L2.eng 31/9, 'An army from'");
    assert_eq!(unit.men, 300, "thirty per cent of a thousand people");
    assert_eq!(unit.troops.iter().sum::<i32>(), unit.men, "every man is in a troop slot");
    assert!(
        unit.troops[TroopType::Peasant.index()] < unit.men,
        "and the auto-equip armed most of them",
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

/// The two size guards and the message each stands for. `FUN_00435B4D` refuses
/// a levy of nothing (`0xA8`) and a levy under fifty (`0x94`), and both
/// refusals leave the world exactly as it was.
#[test]
fn a_levy_of_nothing_and_a_levy_under_fifty_are_both_refused() {
    for percent in [0, 4] {
        let (mut g, a, mut m) = on_the_map();
        press(&mut m, &mut g, &a, 'r');
        tick(&mut m, &mut g, &a);
        click(&mut m, &mut g, &a, (army::SLIDER_X + percent, army::base(false) + 0x20));
        click(&mut m, &mut g, &a, on(army::raise_button()));
        assert_eq!(
            m.top_id(),
            Some(ScreenId::RaiseArmy(1)),
            "{percent} %: the screen stays open on a refusal",
        );
        assert_eq!(g.kingdom.campaign.units.len(), 0, "{percent} %: and nothing was raised");
        assert_eq!(g.kingdom.counties[1].population, 1_000, "{percent} %: nobody left");
    }
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

/// A second click on the selected army cancels the order mode rather than
/// ordering it to march onto itself.
#[test]
fn clicking_the_selected_army_again_puts_the_map_back_in_selection_mode() {
    let (mut g, a, mut m) = on_the_map();
    let (here, there) = adjacent_pair(|x| x < 30);
    let id = army_at(&mut g, 1, 1, 300, here);
    click(&mut m, &mut g, &a, pixel(here.0, here.1).unwrap());
    click(&mut m, &mut g, &a, pixel(here.0, here.1).unwrap());
    click(&mut m, &mut g, &a, pixel(there.0, there.1).unwrap());
    assert!(
        g.kingdom.campaign.units.get(id).is_some_and(|u| !u.moving),
        "the deselected army took no order",
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

    click(&mut m, &mut g, &a, pixel(here.0, here.1).unwrap());
    click(&mut m, &mut g, &a, pixel(there.0, there.1).unwrap());
    let unit = g.kingdom.campaign.units.get(id).expect("still there");
    assert!(unit.moving, "the order was accepted");
    assert_eq!(unit.dest, Some(there), "and it names the tile that was clicked");
    assert!(unit.path.is_empty(), "with no path to walk");

    // And a whole turn of ticking moves it nowhere.
    press(&mut m, &mut g, &a, 'e');
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

    press(&mut m, &mut g, &a, 'e');
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
    click(&mut m, &mut g, &a, on(divide::split_button()));

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
    click(&mut m, &mut g, &a, on(divide::split_button()));
    assert_eq!(m.top_id(), Some(ScreenId::Divide(id)), "the screen stays up");
    assert_eq!(g.kingdom.campaign.units.len(), 1, "and nothing was split");
}

/// **Disband: the men go home and the weapons go back to the treasury**, which
/// is `Army_Disband` and the Readme's *"Any weapons they are carrying are
/// returned to your treasury."*
#[test]
fn the_disband_button_returns_the_men_to_their_county_and_the_weapons_to_the_realm() {
    let (mut g, a, mut m) = on_the_map();
    let (here, _) = adjacent_pair(|x| x < 30);
    let id = army_at(&mut g, 1, 1, 300, here);
    g.kingdom.campaign.units.get_mut(id).unwrap().troops = [100, 0, 0, 200, 0, 0, 0];
    let (pop, swords) = (g.kingdom.counties[1].population, g.kingdom.realms[1].weapons[2]);

    click(&mut m, &mut g, &a, pixel(here.0, here.1).unwrap());
    press(&mut m, &mut g, &a, 'a');
    tick(&mut m, &mut g, &a);
    click(&mut m, &mut g, &a, on(divide::disband_button()));

    assert_eq!(m.top_id(), Some(ScreenId::Campaign));
    assert!(g.kingdom.campaign.units.get(id).is_none(), "the army is gone");
    assert_eq!(g.kingdom.counties[1].population, pop + 300, "the men joined the county");
    assert_eq!(g.kingdom.realms[1].weapons[2], swords + 200, "two hundred swords came back");
}

/// The Readme's disband rule, both clauses, through the screen: an army whose
/// home county has fallen *and* which is standing in enemy country is refused,
/// with `L2.eng` group 145's own words for why.
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

    click(&mut m, &mut g, &a, pixel(here.0, here.1).unwrap());
    press(&mut m, &mut g, &a, 'a');
    tick(&mut m, &mut g, &a);
    click(&mut m, &mut g, &a, on(divide::disband_button()));

    assert_eq!(m.top_id(), Some(ScreenId::Divide(id)), "the screen stays up to say why");
    assert!(g.kingdom.campaign.units.get(id).is_some(), "and the army is still there");
}


// ---------------------------------------------------------------------------
// What lands on the canvas
// ---------------------------------------------------------------------------

/// Both screens paint **inside the rectangle the painter opens** and leave the
/// rest of the frame to whatever was underneath.
///
/// That second half is the point: `Screen_RaiseArmy` and `Screen_ArmyDivision`
/// both open a `Ui_DrawBox` over the campaign map and neither clears the
/// screen — *"the original has no screen clear anywhere"* — so a screen that
/// blanked the frame would be wrong in a way no other assertion here notices.
/// The one place either is allowed outside its window is our own controls,
/// which is why this counts pixels in a band rather than asserting the whole
/// frame is untouched.
#[test]
fn both_screens_paint_inside_the_window_the_painter_opens() {
    use l2_view::Canvas;

    type Step = Box<dyn Fn(&mut Machine, &mut Game, &Assets)>;
    let nothing: Step = Box::new(|_, _, _| {});
    let select_an_army: Step = Box::new(|m, g, a| {
        let (here, _) = adjacent_pair(|x| x < 30);
        army_at(g, 1, 1, 300, here);
        click(m, g, a, pixel(here.0, here.1).unwrap());
    });

    for (setup, open, window) in [
        (
            nothing,
            Box::new(|m: &mut Machine, g: &mut Game, a: &Assets| press(m, g, a, 'r')) as Step,
            army::window(false),
        ),
        (
            // **The setup happens before the reference frame is taken.**
            // Selecting an army changes the *map*: it draws a selection ring, a
            // unit marker and the banner, and a marker on a tile near the top of
            // the viewport lands above the division window's own y. Measuring
            // from a frame taken before the selection would blame the screen for
            // pixels the map drew.
            select_an_army,
            Box::new(|m: &mut Machine, g: &mut Game, a: &Assets| press(m, g, a, 'a')) as Step,
            divide::window(),
        ),
    ] {
        let (mut g, a, mut m) = on_the_map();
        setup(&mut m, &mut g, &a);
        let mut before = Canvas::screen();
        {
            let ctx = Ctx { game: &mut g, assets: &a };
            m.draw(&ctx, &mut before);
        }
        open(&mut m, &mut g, &a);
        tick(&mut m, &mut g, &a);
        let mut after = Canvas::screen();
        {
            let ctx = Ctx { game: &mut g, assets: &a };
            m.draw(&ctx, &mut after);
        }

        let mut inside = 0usize;
        let mut above = 0usize;
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
        assert!(inside > 500, "the painter drew almost nothing: {inside} pixels");
        assert_eq!(above, 0, "it painted above its own window, over the map");
    }
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

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
//! from the fixture has no army, no way to make one,
//! is filed under a name that reads as optional content"* — closing, in three
//! halves:
//!
//! 1. **Raise.** `R` on the map opens `0x17`, the slider sets the levy, the
//!    button raises, and there is an army on the map that was not there before.
//! 2. **March.** A click on that army selects it (`Map_Click`'s army branch),
//!    a click on a tile orders the march (`Map_ConfirmMoveOrder`), and ending
//!    the turn walks it — because `Units_Tick` runs on every tick of the turn
//!
//! 3. **Divide.** `A` opens `0x11`, the buttons move men between the two
//!    columns, and split and disband do what `Army_Split` and `Army_Disband`
//!    do.
//!
//! # Why the tests never scroll
//!
//! The machine owns the map screen and nothing hands it back,
//! ask *where is tile (32, 45) on screen* through it. It does not need to: the
//! campaign screen opens at `Map_InitMode`'s own scroll origin — row `0x4A`,
//! column `0x14` — and a second `MapScreen::new()` is looking at exactly the
//! same place. [`pixel`] uses one as a ruler. Every test below therefore works
//! in tiles that are visible at the opening viewport and never scrolls or
//! centres,
//! company.

mod battle_part;
pub use battle_part::*;
mod raising;
pub use raising::*;
mod marching;
pub use marching::*;
mod division;
pub use division::*;

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
/// and `x − y ∈ [−24, −8)`; adding the two bounds gives `x ∈ [25, 48)`.
/// border a test can *click across* has to lie in that band,
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
/// screen —
///   then assert that the map is still what is on screen.
/// * *The counties need neighbour lists.* `Realm_SecedeIsolatedCounties` walks
///   `County::neighbours`, and a realm holding two counties that name no
/// neighbours is a realm holding two **blocks** —
///   no adjacency seceded again at the end of the same turn it was taken. The
/// rule is right and the fixture was unreal.
pub(crate) fn world() -> (Game, Assets) {
    let mut g = Game::new(11);
    // **Tip screens: No.** A new game's tips hold the campaign map's input on
    // screen `0x27` — and the first time an army is picked up, *"Army
    // Movement:"* does too. Right, and `tests/tips.rs`'s subject, not this file's.
    g.prefs.tip_screens = false;
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
    // **Animations off**: with them on — the default — a decided battle plays
    // `Battle_CheckOutcome`'s film over the banner, and these tests are about
    // the battle. The film is `tests/movies.rs`'s.
    g.prefs.animations = false;
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

pub(crate) fn click(m: &mut Machine, g: &mut Game, a: &Assets, at: (i32, i32)) {
    send(m, g, a, Event::Click { x: at.0, y: at.1 });
}

fn press(m: &mut Machine, g: &mut Game, a: &Assets, c: char) {
    send(m, g, a, Event::KeyDown(Key::letter(c)));
}

/// Tick until `done` answers true, or give up.
///
/// **A turn takes frames.** Pressing End Turn only starts one — the phase
/// machine is wound on a tick at a time
/// walk (`l2_game::turn::TurnStep::Running`) — so anything that happens *during*
/// a turn happens some ticks after the keystroke.
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
    // over,
    for _ in 0..=l2_view::fade::PHASES {
        tick(m, g, a);
    }
}

/// A button's middle,
fn on(r: l2_game::input::Rect) -> (i32, i32) {
    (r.centre_x(), r.y + r.h / 2)
}

/// **Press a kind-5 widget and let its countdown run out.**
///
/// `Widget_Test`'s kind-5 branch sets `rec[0x0D] = 0x14` and returns *without*
/// calling the handler; the handler runs from the countdown at the top of the
/// next call, on the frame the timer reaches zero.
/// gauntlet and asserts on the next line is asserting about a press the game
/// has not answered yet.
///
/// It asserts the delay as it goes — the screen must **not** have moved on any
/// tick before the last — which is what makes this a test of the gesture rather
/// than a way of getting past it.
fn press_and_wait(m: &mut Machine, g: &mut Game, a: &Assets, at: (i32, i32)) {
    let before = m.top_id();
    click(m, g, a, at);
    assert_eq!(m.top_id(), before, "a kind-5 press must not act on the press");
    for i in 1..l2_game::press::DELAYED_FRAMES as u32 {
        tick(m, g, a);
        assert_eq!(m.top_id(), before, "nor on tick {i} of {}", l2_game::press::DELAYED_FRAMES);
    }
    tick(m, g, a);
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
/// are about marching.
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

/// **Open the information panel on a tile** — the campaign map's right release,
/// which is `g_screenId = 0x04`.
fn right_click(m: &mut Machine, g: &mut Game, a: &Assets, at: (i32, i32)) {
    send(m, g, a, Event::RightClick { x: at.0, y: at.1 });
}

/// The information panel's unit half,
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

/// Raise the prompt over an attacker holding two hundred pikemen **one of two
/// ways**, and hand back what screen `0x12` painted.
fn the_prompt_over(band: bool) -> l2_view::Canvas {
    let (mut g, a, mut m, attacker, _) = a_battle_is_about_to_happen();
    {
        let u = g.kingdom.campaign.units.get_mut(attacker).expect("the attacker");
        u.troops = [0; l2_kingdom::unit::TROOP_TYPES];
        u.men = 200;
        if band {
            // `Mercenary_Hire` (`0x004AC7F3`): the band goes in `+0x195…+0x197`
            // and into `menTotal`,
            u.mercenaries =
                Some(l2_kingdom::Mercenaries { band: 2, troop: TroopType::Pikeman, men: 200 });
        } else {
            u.troops[TroopType::Pikeman.index()] = 200;
        }
    }
    press(&mut m, &mut g, &a, 'e');
    assert_eq!(m.top_id(), Some(ScreenId::BattlePrompt), "the prompt should be up");
    let mut canvas = l2_view::Canvas::screen();
    let ctx = Ctx { game: &mut g, assets: &a };
    m.draw(&ctx, &mut canvas);
    canvas
}


//! **The castle route,**
//!
//! ```text
//! cargo test -p l2-game --test castles
//! ```
//!
//! Every line below goes through [`Machine::handle`] with an [`Event`]. Nothing
//! here sets `castle_degraded`, `garrison_unit` or `besieging_county` by hand,
//! and that is the whole point: those three fields were **read by rules and
//! written by nothing a player could reach**
//! build a castle, a castle could never be manned and a siege could only ever
//! be laid by a test that laid it itself.
//!
//! > *"A field is only tested if something a test reads was written by
//! > something the game runs. A test that populates the state it then asserts
//! > on is checking its own fixture."* — `docs/agents.md`
//!
//! So the route is: **order a castle from the sidebar, watch it go up over
//! seasons, march an army into it, have an enemy march up to it, and end the
//! turn into the assault.** The only things placed by hand are the ones a
//! scenario would place — the map, the counties, and armies that already exist.
//!
//! # Why the map has a castle plot in it
//!
//! A county's castle stands on a 2×2 block of plane-0 bit `0x80` tiles whose
//! terrain is `0x14` (bare) or `0x15 … 0x19` (a castle of type 1 … 5).
//! `County_FindCastleTile` finds that block at load and stamps `0x14` on it;
//! [`l2_kingdom::map::stamp_castle_terrain`] is what raises it afterwards.
//! [`plot`] below is that block, and it is the only piece of scenery these
//! tests place.

mod construction;
pub use construction::*;
mod garrison_and_siege;
pub use garrison_and_siege::*;

use l2_game::game::Assets;
use l2_game::input::{Event, Key, Rect};
use l2_game::screen::{Ctx, Machine, ScreenId};
use l2_game::screens::{castle, map};
use l2_game::Game;
use l2_kingdom::map::{flags, terrain, CampaignMap, MAP_DIM, MAP_TILES};
use l2_kingdom::unit::{TroopType, Unit, UnitKind};
use l2_kingdom::MercenaryBands;
use l2_view::campaign;

/// The border between county 1 and county 2, chosen the same way
/// `tests/military.rs` chooses it: inside the band the campaign screen opens
/// on, so a test can click a tile without scrolling.
const BORDER_1_2: usize = 32;

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

fn on(r: Rect) -> (i32, i32) {
    (r.centre_x(), r.y + r.h / 2)
}

/// **Press one of `g_castleBuildWidgets`' two thumbs and let its twenty frames
/// run.** Both are `Widget_Test` kind 5, so the screen must still be up on
/// every tick before the twentieth, and the press clicks exactly once.
///
/// Every castle order in this file goes through here, so all five are tests of
/// the gesture. **Ablations, run:** declare the thumbs `Press` in their `arm!`s
/// and all five go red at the screen's own debug assertion that both records
/// are kind 5 — the press answers at once — and `tests/arms.rs` goes red too;
/// delete `self.click()` from `Press::press_delayed` and all five go red at the
/// click count, which reads 0.
fn press_and_wait(m: &mut Machine, g: &mut Game, a: &Assets, at: (i32, i32)) {
    let before = m.top_id();
    let clicks = m.clicks();
    click(m, g, a, at);
    send(m, g, a, Event::Release { x: at.0, y: at.1 });
    assert_eq!(m.clicks(), clicks + 1, "the thumb is Widget_Test's press, so it clicks");
    for t in 1..l2_game::press::DELAYED_FRAMES as u32 {
        tick(m, g, a);
        assert_eq!(m.top_id(), before, "the castle thumb acted on tick {t}, before its twentieth");
    }
    tick(m, g, a);
    assert_eq!(m.clicks(), clicks + 1, "and the order itself is silent");
}

fn pixel(x: u8, y: u8) -> Option<(i32, i32)> {
    let probe = map::MapScreen::new();
    campaign::tile_centre(probe.viewport(), probe.zoom(), x as usize, y as usize)
}

fn run_until(
    m: &mut Machine,
    g: &mut Game,
    a: &Assets,
    what: &str,
    done: impl Fn(&Machine, &Game) -> bool,
) {
    for _ in 0..4_000 {
        if done(m, g) {
            return;
        }
        tick(m, g, a);
    }
    panic!("{what} never happened");
}

/// **Let every order on the map finish walking**, which is what a player does
/// between giving one and pressing End Turn.
///
/// `Units_Tick` runs on ordinary frames as well as inside a turn
/// (`turn::tick_units_only`, `docs/decisions.md` C115), and since
/// `Unit_StepOnce`'s sub-tile counter landed a unit takes 8 ticks to cross a
/// road tile and 32 to cross anything else. Nothing in the turn machine waits
/// on the human's armies,
/// the order is asserting a race
pub(crate) fn march(m: &mut Machine, g: &mut Game, a: &Assets) {
    run_until(m, g, a, "the march", |_, g| {
        !g.kingdom.campaign.units.iter().any(|(_, u)| u.moving)
    });
}

pub(crate) fn end_turn(m: &mut Machine, g: &mut Game, a: &Assets) {
    let before = g.kingdom.turn_count;
    press(m, g, a, 'e');
    run_until(m, g, a, "the turn", |_, g| g.kingdom.turn_count > before);
    for _ in 0..=l2_view::fade::PHASES {
        tick(m, g, a);
    }
}

/// The sidebar's **CASTLE** button, found by its name
/// so that reordering the strip does not silently point this at COURT.
fn castle_button() -> Rect {
    map::SIDEBAR_BUTTONS
        .iter()
        .find(|b| b.name == "CASTLE")
        .expect("the sidebar has a castle button")
        .rect()
}

/// A county's castle plot: a 2×2 block of `SETTLEMENT` tiles at terrain
/// `0x14`, placed inside the opening viewport so it can be clicked and marched
/// to. Returns its north-west tile.
fn plot(g: &mut Game, county: u8, at: (u8, u8)) -> (u8, u8) {
    for dy in 0..2u8 {
        for dx in 0..2u8 {
            let (x, y) = (at.0 + dx, at.1 + dy);
            g.kingdom.campaign.map.set_flags(x, y, flags::SETTLEMENT);
            g.kingdom.campaign.map.set_terrain(x, y, terrain::CASTLE_PLOT);
            g.kingdom.campaign.map.set_county(x, y, county);
        }
    }
    at
}

/// Two counties, one the player's and one an opponent's, on a map every tile of
/// which is walkable. The opponent holds two counties so that losing one does
/// not eliminate it and end the game — `tests/military.rs` learned that the
/// hard way and it is the same trap here.
pub(crate) fn world() -> (Game, Assets) {
    let mut g = Game::new(11);
    // **Tip screens: No**, the player's own switch. A new game's three tips
    // open on the campaign map and hold its input on screen `0x27`, which is
    // right and is `tests/tips.rs`'s subject, not this file's.
    g.prefs.tip_screens = false;
    g.kingdom.set_county_count(3);
    for id in 1..=3usize {
        let c = &mut g.kingdom.counties[id];
        c.population = 4_000;
        c.happiness = 90;
        c.grain = 60_000;
        c.herd = 400;
    }
    g.kingdom.counties[1].owner = 1;
    g.kingdom.counties[2].owner = 2;
    g.kingdom.counties[3].owner = 2;
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
        // Enough wood and stone in the store that a palisade is paid for on the
        // spot. A castle ordered without them is a separate test.
        g.kingdom.realms[realm].wood = 5_000;
        g.kingdom.realms[realm].stone = 5_000;
    }
    g.kingdom.realms[1].is_human = true;
    g.kingdom.realms[1].weapons = [200; 6];

    let mut m = CampaignMap::empty();
    for i in 0..MAP_TILES {
        let x = i % MAP_DIM;
        m.county[i] = if x < BORDER_1_2 { 1 } else { 2 };
    }
    g.kingdom.campaign.map = m;
    g.kingdom.campaign.mercenaries = MercenaryBands::init(2);
    g.player = 1;
    g.selected = 1;
    // **Animations off.** With them on — the original's default and ours —
    // `CastleBuild_Confirm` plays `castle<n>.smk` over the chooser before the map
    // comes back, and these tests are about the order, not the film. The film
    // is `tests/movies.rs`'s.
    g.prefs.animations = false;
    (g, Assets::placeholder())
}

fn on_the_map() -> (Game, Assets, Machine) {
    let (g, a) = world();
    (g, a, Machine::new(ScreenId::Campaign))
}

fn army_at(g: &mut Game, owner: u8, county: u8, men: i32, at: (u8, u8)) -> usize {
    let mut u = Unit::new(UnitKind::Army, owner, at.0, at.1);
    u.men = men;
    u.troops[TroopType::Peasant.index()] = men;
    u.county = county;
    u.home_county = county;
    u.owner_is_human = owner == 1;
    g.kingdom.campaign.units.spawn(u).expect("a free slot")
}

/// A tile whose whole 4×4 neighbourhood — from one west and one north to two
/// east and two south — is inside the opening viewport **and** in the given
/// county, so a 2×2 castle block placed on it has room around it for an army to
/// stand and for a test to click.
///
/// The campaign screen opens at `Map_InitMode`'s own scroll origin and these
/// tests never scroll, so [`pixel`]'s fresh probe is a valid ruler for the
/// machine's screen. Searched in ascending `(y, x)` so the answer is the same
/// every run.
fn visible_in(g: &Game, county: u8) -> (u8, u8) {
    for y in 2..60u8 {
        for x in 2..60u8 {
            let ok = (-1i32..=2).all(|dy| {
                (-1i32..=2).all(|dx| {
                    let (px, py) = ((x as i32 + dx) as u8, (y as i32 + dy) as u8);
                    g.kingdom.campaign.map.county_at(px, py) == county && pixel(px, py).is_some()
                })
            });
            if ok {
                return (x, y);
            }
        }
    }
    panic!("no visible 4x4 block in county {county}");
}

// ---------------------------------------------------------------------------
// 1. Building one
// ---------------------------------------------------------------------------


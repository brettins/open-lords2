
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

fn castle_button() -> Rect {
    map::SIDEBAR_BUTTONS
        .iter()
        .find(|b| b.name == "CASTLE")
        .expect("the sidebar has a castle button")
        .rect()
}

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

pub(crate) fn world() -> (Game, Assets) {
    let mut g = Game::new(11);
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
    l2_testkit::chain_neighbours!(g.kingdom);
    for realm in 1..=2usize {
        g.kingdom.realms[realm].in_play = true;
        g.kingdom.realms[realm].strength = 5;
        g.kingdom.realms[realm].gold = 20_000;
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



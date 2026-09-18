//! > **This paragraph used to say the AI never orders siege engines anywhere in
//! > the workspace.** That was a conclusion drawn from `order_engine`'s three
//! > callers all being the player's, and `order_engine` is the siege screen's
//! > `+` and `−` buttons — the original's AI does not press those either. The
//! > AI's path is `Siege_Prepare` (`0x004A7EB5`), reached from
//! > `Unit_ReachCastleBuilding` when an army *walks onto* the castle, and it
//! > has been implemented all along. `crates/l2-kingdom/tests/ai_siege.rs`
//! > travels that road; what was missing was a test that did, not the code.

mod siege_battle_tests;
pub use siege_battle_tests::*;

use l2_game::game::Assets;
use l2_game::input::{Event, Key, Rect};
use l2_game::screen::{Ctx, Machine, ScreenId};
use l2_game::screens::{battle, map};
use l2_game::Game;
use l2_kingdom::map::{flags, terrain, CampaignMap, MAP_DIM, MAP_TILES};
use l2_kingdom::unit::{TroopType, Unit, UnitKind};
use l2_kingdom::MercenaryBands;
use l2_view::campaign;

use l2_game::battlefield as bf;

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
    for _ in 0..8_000 {
        if done(m, g) {
            return;
        }
        tick(m, g, a);
        // **The message scroll is modal and never times out in single player**
        // — `Msg_Pump` clamps the timer to 1 — so a loop that only ticks stops
        // dead at the first thing the game tells the player. A siege raises
        // several. Closing it is a right release, which is
        // `0x0047685D/message-scroll-dismiss`; doing it as an `Event` rather
        // than by calling `message::dismiss` is the point, because it is the
        // player's own way out and this file drives nothing else by hand.
        if m.top_id() == Some(ScreenId::Message) {
            send(m, g, a, Event::RightClick { x: 320, y: 240 });
        }
    }
    panic!("{what} never happened; the screen is {:?}", m.top_id());
}

pub(crate) fn world() -> (Game, Assets) {
    let mut g = Game::new(11);
    g.kingdom.set_county_count(2);
    for id in 1..=2usize {
        let c = &mut g.kingdom.counties[id];
        c.population = 4_000;
        c.happiness = 90;
        c.grain = 60_000;
        c.herd = 400;
        c.neighbour_count = 1;
        c.neighbours[0] = if id == 1 { 2 } else { 1 };
    }
    g.kingdom.counties[1].owner = 1;
    g.kingdom.counties[2].owner = 2;
    for realm in 1..=2usize {
        g.kingdom.realms[realm].in_play = true;
        g.kingdom.realms[realm].strength = 5;
        g.kingdom.realms[realm].wood = 5_000;
        g.kingdom.realms[realm].stone = 5_000;
    }
    g.kingdom.realms[1].is_human = true;
    for realm in 1..=2usize {
        g.kingdom.realms[realm].gold = 20_000;
    }

    let mut m = CampaignMap::empty();
    for i in 0..MAP_TILES {
        m.county[i] = if i % MAP_DIM < BORDER_1_2 { 1 } else { 2 };
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

fn plot(g: &mut Game, county: u8, at: (u8, u8)) {
    for dy in 0..2u8 {
        for dx in 0..2u8 {
            let (x, y) = (at.0 + dx, at.1 + dy);
            g.kingdom.campaign.map.set_flags(x, y, flags::SETTLEMENT);
            g.kingdom.campaign.map.set_terrain(x, y, terrain::CASTLE_PLOT);
            g.kingdom.campaign.map.set_county(x, y, county);
        }
    }
}

/// `Units_Tick` runs on every tick the game is up —
/// `docs/decisions.md` C35, and the campaign screen's `update` is what calls it
/// — so an ordered army walks while the player sits on the map. That matters
/// here for a reason beyond fidelity: **ending a turn to move an army also runs
/// phase 2**, so a level-2 castle would be assaulted on the same turn the siege
/// was laid, and the setup would resolve the very battle the test is about to
/// watch.
///
/// `Unit_ReachCastleBuilding` (`0x004686A0`) is what lays the siege — the code-6
/// branch of the stepper, on terrain `0x15 … 0x19`.
fn lay_siege(m: &mut Machine, g: &mut Game, a: &Assets, besieger: usize, keep: (u8, u8), county: u8) {
    march(g, besieger, keep);
    run_until(m, g, a, "the siege", |_, g| {
        g.kingdom.campaign.units.get(besieger).is_some_and(|u| u.besieging_county == county)
    });
}

pub(crate) fn march(g: &mut Game, unit: usize, to: (u8, u8)) {
    let map = g.kingdom.campaign.map.clone();
    l2_kingdom::movement::order_move(
        &map,
        &mut g.kingdom.campaign.units,
        unit,
        to,
        l2_kingdom::movement::Routing::Direct,
    )
    .expect("a path one tile long");
}

fn castle_with_garrison(
    g: &mut Game,
    m: &mut Machine,
    a: &Assets,
    county: u8,
    castle_type: u8,
    men: i32,
) -> ((u8, u8), usize) {
    let keep = visible_in(g, county);
    plot(g, county, keep);
    g.kingdom.counties[county as usize].castle_type = castle_type;
    l2_kingdom::map::stamp_castle_terrain(&mut g.kingdom.campaign.map, county, castle_type);

    let owner = g.kingdom.counties[county as usize].owner;
    let garrison = army_at(g, owner, county, men, (keep.0 - 1, keep.1));
    march(g, garrison, keep);
    run_until(m, g, a, "the garrison walking in", |_, g| {
        g.kingdom.counties[county as usize].garrison_unit == garrison
    });
    (keep, garrison)
}

fn end_turns_until_the_assault(m: &mut Machine, g: &mut Game, a: &Assets) {
    for _ in 0..12 {
        let before = g.kingdom.turn_count;
        press(m, g, a, 'e');
        for _ in 0..8_000 {
            if m.top_id() == Some(ScreenId::BattlePrompt) {
                return;
            }
            if g.kingdom.turn_count > before {
                break;
            }
            tick(m, g, a);
        }
        for _ in 0..=l2_view::fade::PHASES {
            tick(m, g, a);
        }
    }
    panic!("the assault prompt never came; the screen is {:?}", m.top_id());
}

fn take_the_field(m: &mut Machine, g: &mut Game, a: &Assets) {
    assert_eq!(m.top_id(), Some(ScreenId::BattlePrompt), "a prompt should be up");
    click(m, g, a, on(battle::widget_rect(battle::TAKE_THE_FIELD)));
    assert_eq!(m.top_id(), Some(ScreenId::Battlefield), "the field, not the result");
    assert!(g.battle.as_ref().expect("a live battle").paused, "a battle starts paused");
    click(m, g, a, on(bf::Button::Pause.rect()));
    assert!(!g.battle.as_ref().expect("a live battle").paused);
}

fn watch_to_the_end(m: &mut Machine, g: &mut Game, a: &Assets, ticks: u32) {
    let mut n = 0;
    while n < ticks && g.battle.as_ref().is_some_and(|b| b.conclusion.is_none()) {
        tick(m, g, a);
        n += 1;
        if n % 50_000 == 0 {
            if let Some(b) = g.battle.as_ref() {
                eprintln!(
                    "  t={n} besieger={} garrison={} approach={} breach={} ramparts={} \
                     gate={} ditch={}",
                    b.runner.men_of_side(l2_sim::SIDE_B),
                    b.runner.men_of_side(l2_sim::SIDE_A),
                    b.runner.ai.approach_score,
                    b.runner.ai.breach_score,
                    b.runner.siege.ramparts_breached,
                    b.runner.siege.gate_breached,
                    b.runner.siege.moat_filled,
                );
            }
        }
    }
    assert!(
        g.battle.as_ref().is_some_and(|b| b.conclusion.is_some()),
        "the siege did not reach a conclusion in {ticks} frames",
    );
    eprintln!("the siege was decided after {n} frames");
    assert_eq!(g.battle.as_ref().map(|b| b.screen_id()), Some(0x2B), "the banner");
    send(m, g, a, Event::RightClick { x: 100, y: 300 });
    tick(m, g, a);
    assert_eq!(m.top_id(), Some(ScreenId::BattleResult), "the result screen follows");
}

/// The pixel of a battlefield cell in the **overview panel**, which is 160 × 160
/// at (480, 24) and two pixels a cell — `BattleMap_Click` (`0x00432443`).
fn overview_px(cell: (u8, u8)) -> (i32, i32) {
    (bf::OVERVIEW.x + cell.0 as i32 * 2, bf::OVERVIEW.y + cell.1 as i32 * 2)
}

/// Two played gestures — a left click on the overview panel, which is
/// `BattleMap_Click`'s order arm at battlefield scale, and the fourth button —
/// and a besieging player makes both. A formation ordered at the wall stops at
/// the wall; somebody has to tell it to go through the hole it has just made,
/// and `FUN_0047A76D` is what turns a formation into a mob hunting the last of
/// a garrison down.
fn press_the_assault_home(m: &mut Machine, g: &mut Game, a: &Assets) {
    let mut n = 0;
    while n < 200_000 {
        let through = g
            .battle
            .as_ref()
            .is_none_or(|b| b.conclusion.is_some() || b.runner.siege.gate_breached);
        if through {
            break;
        }
        tick(m, g, a);
        n += 1;
    }
    eprintln!("the wall was open after {n} frames");
    let target = overview_px(keep_centre(g));
    click(m, g, a, target);
    click(m, g, a, on(bf::Button::Charge.rect()));
    assert!(g.battle.as_ref().is_none_or(|b| b.charged), "DAT_0055322C, the charge latch");
}

fn keep_centre(g: &Game) -> (u8, u8) {
    let live = g.battle.as_ref().expect("a live battle");
    let dim = l2_sim::terrain::DIM;
    live.runner
        .field
        .cells
        .iter()
        .position(|c| c.flags & l2_sim::siege::FLAG_KEEP != 0)
        .map(|i| ((i % dim) as u8, (i / dim) as u8))
        .expect("every castle has one way in")
}

/// `FUN_0043BF07` in three events — `press_field`, `drag_to`, `release_field` —
/// and the box is the whole viewport, so what it picks is whatever the camera is
/// looking at.
fn box_select_the_viewport(m: &mut Machine, g: &mut Game, a: &Assets) {
    let v = bf::VIEW;
    click(m, g, a, (v.x + 2, v.y + 2));
    send(m, g, a, Event::Pointer { x: v.x + v.w - 2, y: v.y + v.h - 2 });
    send(m, g, a, Event::Release { x: v.x + v.w - 2, y: v.y + v.h - 2 });
}



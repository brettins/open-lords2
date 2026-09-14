//! **The films, played — every place `Smk_Play` is called, and every way out.**
//!
//! ```text
//! LORDS2_DIR="F:\games\Lords of the Realm II" cargo test -p l2-game --test movies
//! ```
//!
//! Two halves, and the split is the install. Without it every film fails to
//! open, which is still a real path — `Smk_Play`'s callers each have an arm for
//! it — so *which* film each trigger asks for, and where the machine goes when
//! it cannot have it, run everywhere. With it the films open, and the tests ask
//! what a player sees and hears: the gestures that end one, the tick it ends
//! on, where it is drawn, the palette the screen runs under, and the music bed
//! stopping for it and starting over after it.
//!
//! Everything goes through [`Machine::handle`] and [`Machine::update`], and the
//! sound through [`audio::Director::listen`] — the functions the game calls.
//!
//! **Ablations run on this file**, each named at its test: delete the line the
//! assertion is about and the test goes red.


use l2_game::audio::{self, Audio};
use l2_game::game::Assets;
use l2_game::input::{Event, Key};
use l2_game::message::{category, Record};
use l2_game::movie::{self, Film};
use l2_game::screen::{Ctx, Machine, ScreenId};
use l2_game::screens::{castle, setup};
use l2_game::Game;
use l2_kingdom::unit::{TroopType, Unit, UnitKind};

fn send(m: &mut Machine, g: &mut Game, a: &Assets, e: Event) {
    let mut ctx = Ctx { game: g, assets: a };
    m.handle(e, &mut ctx);
}

fn tick(m: &mut Machine, g: &mut Game, a: &Assets) {
    let mut ctx = Ctx { game: g, assets: a };
    m.update(&mut ctx);
}

fn centre(r: l2_game::input::Rect) -> (i32, i32) {
    (r.x + r.w / 2, r.y + r.h / 2)
}

/// The install, mounted, and its assets — or `None`.
fn installed() -> Option<(l2_mods::Platform, Assets)> {
    let dir = l2_testkit::install_dir()?;
    let platform = l2_mods::Platform::builder().base(&dir).build().ok()?;
    let assets = Assets::load(&platform.vfs).ok()?;
    if assets.films.is_empty() {
        return None;
    }
    Some((platform, assets))
}

macro_rules! install {
    () => {
        match installed() {
            Some(x) => x,
            None => l2_testkit::skip!("no install with films (the DOS release ships none)"),
        }
    };
}

mod triggers;
pub use triggers::*;
mod playback;
pub use playback::*;
mod audio_part;
pub use audio_part::*;

/// Five realms, realm 1 the human, as `tests/messages.rs` has them — **with
/// animations on**, which is the original's default and ours.
fn realms() -> Game {
    let mut g = Game::new(0xF11A);
    g.player = 1;
    g.kingdom.set_county_count(6);
    l2_testkit::chain_neighbours!(g.kingdom);
    for realm in 1..=5usize {
        g.kingdom.realms[realm].in_play = true;
        g.kingdom.realms[realm].strength = 3;
        g.kingdom.realms[realm].lord = realm as u8 - 1;
    }
    g.kingdom.realms[1].is_human = true;
    g.kingdom.realms[1].lord = 1;
    g.kingdom.year = movie::FIRST_YEAR + 1;
    assert!(g.prefs.animations, "animations default to on, as FUN_004AF35E sets them");
    g
}

fn ending(from: u8, group: u16) -> Record {
    Record { to: 0, from, group, category: category::ENDING, ..Record::default() }
}

fn film_on_top(m: &Machine) -> Option<Film> {
    match m.top_id() {
        Some(ScreenId::Movie(f)) => Some(f),
        _ => None,
    }
}

// =============================================================== no install

fn castle_world() -> (Game, Machine) {
    let mut g = Game::new(11);
    g.kingdom.set_county_count(2);
    l2_testkit::chain_neighbours!(g.kingdom);
    g.kingdom.counties[1].owner = 1;
    g.kingdom.realms[1].in_play = true;
    g.kingdom.realms[1].is_human = true;
    g.kingdom.realms[1].wood = 5_000;
    g.kingdom.realms[1].stone = 5_000;
    g.player = 1;
    g.selected = 1;
    let mut m = Machine::new(ScreenId::Campaign);
    m.push(ScreenId::Castle(1));
    (g, m)
}

/// **Press `g_castleBuildWidgets`' tick and let its twenty frames run.**
///
/// `CastleBuild_Confirm` (`0x00436B59`) is record 0 of `g_castleBuildWidgets`
/// and that record is `Widget_Test` kind 5, so the press only puts the thumb
/// down: the handler — and so the `Smk_Play` in its tail — runs on the
/// twentieth frame. `Widget_Test` (`0x0040DA1E`) is what counts them, from its
/// countdown over `+0x0D` of every record.
///
/// The release goes in where a player's does, straight after the press, which
/// is the whole point: it is answered by the chooser, nineteen frames before
/// there is a film to skip.
fn order_the_castle(m: &mut Machine, g: &mut Game, a: &Assets) {
    let ok = (castle::OK.x + 4, castle::OK.y + 4);
    send(m, g, a, Event::Click { x: ok.0, y: ok.1 });
    send(m, g, a, Event::Release { x: ok.0, y: ok.1 });
    assert_eq!(m.top_id(), Some(ScreenId::Castle(1)), "a kind-5 press must not act on the press");
    for t in 1..l2_game::press::DELAYED_FRAMES as u32 {
        tick(m, g, a);
        assert_eq!(m.top_id(), Some(ScreenId::Castle(1)), "the castle thumb acted on tick {t}");
    }
    tick(m, g, a);
}

fn army(g: &mut Game, owner: u8, county: u8, at: (u8, u8)) -> usize {
    let mut u = Unit::new(UnitKind::Army, owner, at.0, at.1);
    u.men = 300;
    u.troops[TroopType::Peasant.index()] = 300;
    u.county = county;
    u.home_county = county;
    u.owner_is_human = owner == 1;
    g.kingdom.campaign.units.spawn(u).expect("a free slot")
}

/// A battlefield whose battle has just been decided for `winner`.
fn decided(attacker_owner: u8, castle: Option<u8>, winner: l2_sim::Side) -> (Game, Machine) {
    let mut g = realms();
    let attacker = army(&mut g, attacker_owner, 1, (10, 10));
    let defender = army(&mut g, 3 - attacker_owner, 2, (11, 10));
    let runner = l2_game::engagement::begin_fight(&mut g.kingdom, attacker, defender, None, 7)
        .expect("two armies");
    let mut live = l2_game::battlefield::LiveBattle::new(runner, attacker, defender, 2, castle, 1, 1);
    live.mode = l2_game::battlefield::Mode::Outcome;
    live.conclusion = Some(l2_sim::runner::Conclusion { winner, cause: l2_sim::End::Annihilation });
    g.battle = Some(Box::new(live));
    let mut m = Machine::new(ScreenId::Campaign);
    m.push(ScreenId::Battlefield);
    (g, m)
}

fn decode_frame0(a: &Assets, name: &str) -> l2_smk::Decoder {
    let smk = a.films.open(name).expect("the film");
    let mut d = smk.decoder();
    d.next_frame(&smk).expect("frame 0");
    d
}

/// One event-loop tick: update, then listen, as `App::tick` runs them.
fn tick_and_listen(m: &mut Machine, g: &mut Game, a: &Assets, audio: &mut Audio, d: &mut audio::Director) {
    tick(m, g, a);
    d.listen(audio, m, g);
}

fn buffer(audio: &mut Audio) -> Vec<f32> {
    let mut b = vec![0f32; 2 * 2048];
    audio.mix(&mut b);
    b
}


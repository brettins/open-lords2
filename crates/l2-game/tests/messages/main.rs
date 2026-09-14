//! **The message scroll, played.**
//!
//! ```text
//! cargo test -p l2-game --test messages
//! ```
//!
//! Three questions, and they are the three the message window was blocking:
//!
//! * **can a player dismiss a message?** — `Msg_HandleInput` (`0x0047685D`),
//!   every screen's arm, which nothing in this workspace had;
//! * **can a player answer a lord?** — the two prompts `docs/arms.json` called
//! *"the only two places a person answers a lord,
//!   and both are unreachable"*;
//! * **can a player be told he has won?** — `docs/plan.md`'s mainline win, which
//!   ends *"the game is won when **that** is dismissed"* and had nothing to
//!   dismiss.
//!
//! Everything here goes through [`Machine::handle`] and [`Machine::update`] with
//! [`Event`] values. Nothing calls `message::dismiss`, sets `campaign.outcome`,
//! or reaches into the ring except to put a letter in it — which is what a rule
//! does, and the rules are `l2_kingdom`'s.
//!
//! > *"A rule with no way in is not a rule the game has."* — `docs/agents.md`


use l2_game::game::Assets;
use l2_game::input::Event;
use l2_game::message::{self, category, Prompt, Record};
use l2_game::screen::{Ctx, Machine, ScreenId};
use l2_game::Game;
use l2_kingdom::victory::Outcome;

// ---------------------------------------------------------------------- setup

fn send(m: &mut Machine, g: &mut Game, a: &Assets, e: Event) {
    let mut ctx = Ctx { game: g, assets: a };
    m.handle(e, &mut ctx);
}

fn tick(m: &mut Machine, g: &mut Game, a: &Assets) {
    let mut ctx = Ctx { game: g, assets: a };
    m.update(&mut ctx);
}

/// The middle of a widget, and
/// one that is off by a button does not.
fn on(at: (i32, i32)) -> Event {
    Event::Click { x: at.0 + Prompt::SIDE / 2, y: at.1 + Prompt::SIDE / 2 }
}

/// Five realms on a small map, realm 1 the human, nothing owned yet.
pub(crate) fn world() -> (Game, Assets, Machine) {
    let mut g = Game::new(0xB0A7);
    g.player = 1;
    g.kingdom.set_county_count(6);
    for id in 1..=6usize {
        let c = &mut g.kingdom.counties[id];
        c.population = 500;
        c.happiness = 70;
    }
    for realm in 1..=5usize {
        g.kingdom.realms[realm].in_play = true;
        g.kingdom.realms[realm].strength = 3;
        g.kingdom.realms[realm].gold = 2_000;
        g.kingdom.realms[realm].lord = realm as u8 - 1;
    }
    g.kingdom.realms[1].is_human = true;
    g.kingdom.realms[1].lord = 1;
    // **Animations off**, and the report names
    // read. With them on — the default — `Msg_DrawWindow` closes both on the
    // frame they open and plays a film instead; that branch is
    // `tests/movies.rs`'s.
    g.prefs.animations = false;
    (g, Assets::placeholder(), Machine::new(ScreenId::Campaign))
}

/// One `Msg_Enqueue`. The `to` is the local player or 0, because that is the
/// only kind of record that ever reaches a peer's ring.
fn post(g: &mut Game, r: Record) {
    let player = g.player;
    assert!(g.messages.enqueue(r, player), "the peer filter kept this record out");
}

fn notice(group: u16) -> Record {
    Record { to: 0, group, category: category::NOTICE, ..Record::default() }
}

/// Run the machine until the scroll is up, and say so if it never is.
fn open_the_scroll(m: &mut Machine, g: &mut Game, a: &Assets) {
    for _ in 0..8 {
        tick(m, g, a);
        if m.top_id() == Some(ScreenId::Message) {
            return;
        }
    }
    panic!("Msg_Pump never raised the window; the screen is {:?}", m.top_id());
}

macro_rules! painted {
    () => {{
        let Some(dir) = l2_testkit::install_dir() else {
            l2_testkit::skip!("no game install, so there is no artwork to draw with");
        };
        let platform =
            l2_mods::Platform::builder().base(&dir).build().expect("the install mounts");
        let assets = Assets::load(&platform.vfs).expect("assets load");
        let (mut g, _placeholder, m) = world();
        g.map_slot = 4;
        (g, assets, m)
    }};
}

mod dismissal_and_timing;
pub use dismissal_and_timing::*;
mod prompts_and_alliances;
pub use prompts_and_alliances::*;
mod outcomes_and_obituaries;
pub use outcomes_and_obituaries::*;
mod posting_and_sync;
pub use posting_and_sync::*;
mod painting_and_layout;
pub use painting_and_layout::*;
mod battle_drain;
pub use battle_drain::*;

fn painting(g: &mut Game, a: &Assets, m: &mut Machine) -> l2_view::Canvas {
    let mut canvas = l2_view::Canvas::screen();
    let ctx = Ctx { game: g, assets: a };
    m.draw(&ctx, &mut canvas);
    canvas
}

// ------------------------------------------------------- 1. dismiss a message


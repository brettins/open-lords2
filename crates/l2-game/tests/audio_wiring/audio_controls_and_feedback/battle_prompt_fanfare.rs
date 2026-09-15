//! **The drum roll when a battle is announced, on the path the player walks.**
//!
//! The report (2026-09-14): *"no drum roll when I try to attack a country"* —
//! *"when my army first reaches a country and I get the battle to be fought I
//! remember a drum roll of sorts"*.
//!
//! The site is `Battle_ChooseSettlement` (`0x004A6A30`),
//! `Sound_PlayFile("ff_batl.wav", 0, 0)` on the branch that raises screen
//! `0x12` — `docs/audio.json` `Battle_ChooseSettlement#1`. `Director::listen`
//! plays it when [`ScreenId::BattlePrompt`] arrives, and
//! `audio::tests::the_battle_prompt_speaks_the_take_its_choice_owner_picks`
//! covers that with a pushed stack. What that fixture cannot see is the turn:
//! a march into an enemy county raises the prompt from `MapScreen`'s own
//! update, and this drives exactly that.

#![allow(unused_imports)]
use super::*;
use super::routing::*;
use l2_game::audio::{self, Audio};
use l2_game::game::Assets;
use l2_game::input::{Event, Key};
use l2_game::screen::{Ctx, Machine, ScreenId};
use l2_game::Game;
use l2_kingdom::map::{CampaignMap, MAP_DIM, MAP_TILES};
use l2_kingdom::unit::{TroopType, Unit, UnitKind};
use l2_kingdom::MercenaryBands;

/// The x at which county 1 gives way to county 2, as `tests/military` divides
/// its map.
const BORDER: usize = 32;

/// Three counties in vertical bands, the player holding the first and an AI
/// the other two — `tests/military`'s `world`, which is the fixture the
/// prompt's own tests march across.
fn two_realms() -> (Game, Assets) {
    let mut g = Game::new(11);
    g.prefs.tip_screens = true;
    g.prefs.animations = true;
    g.kingdom.set_county_count(3);
    for id in 1..=3usize {
        let c = &mut g.kingdom.counties[id];
        c.population = 1_000;
        c.happiness = 90;
        c.grain = 500;
    }
    g.kingdom.counties[1].owner = 1;
    g.kingdom.counties[2].owner = 2;
    g.kingdom.counties[3].owner = 2;
    l2_testkit::chain_neighbours!(g.kingdom);
    for realm in 1..=2usize {
        g.kingdom.realms[realm].in_play = true;
        g.kingdom.realms[realm].strength = 5;
        g.kingdom.realms[realm].gold = 20_000;
    }
    g.kingdom.realms[1].is_human = true;
    let mut m = CampaignMap::empty();
    for i in 0..MAP_TILES {
        m.county[i] = if i % MAP_DIM < BORDER { 1 } else { 2 };
    }
    g.kingdom.campaign.map = m;
    g.kingdom.campaign.mercenaries = MercenaryBands::init(2);
    g.player = 1;
    g.selected = 1;
    (g, Assets::placeholder())
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

/// March into the enemy county and end the turn, ticking the machine and the
/// director together the way `main.rs`'s `tick` does — `machine.update` then
/// `Director::listen`, once a frame.
///
/// Returns what the one-shot buffer was asked for, and whether the prompt
/// came up at all.
fn a_march_into_the_enemy(platform: &l2_mods::Platform) -> (bool, Vec<String>) {
    let (mut game, assets) = two_realms();
    let mine = (BORDER as u8 - 1, 40);
    let theirs = (BORDER as u8, 40);
    let attacker = army_at(&mut game, 1, 1, 400, mine);
    army_at(&mut game, 2, 2, 200, theirs);
    let map = game.kingdom.campaign.map.clone();
    l2_kingdom::movement::order_move(
        &map,
        &mut game.kingdom.campaign.units,
        attacker,
        theirs,
        l2_kingdom::movement::Routing::Direct,
    )
    .expect("a path one tile long");

    // The stack `main.rs` builds, never a constructed one.
    let mut machine = Machine::new(APP_ROOT);
    machine.push(ScreenId::Campaign);
    let mut audio = Audio::headless(&platform.vfs);
    let mut director = audio::Director::new();
    director.listen(&mut audio, &machine, &game);

    send(&mut machine, &mut game, &assets, Event::KeyDown(Key::letter('e')));
    let mut opened = false;
    for _ in 0..400 {
        {
            let mut ctx = Ctx { game: &mut game, assets: &assets };
            machine.update(&mut ctx);
        }
        director.listen(&mut audio, &machine, &game);
        if machine.ids().contains(&ScreenId::BattlePrompt) {
            opened = true;
            break;
        }
    }
    // **And it is still sounding sixty frames later.** The one-shot buffer is
    // emptied by `Msg_Dismiss`'s `Sound_StopOneShot` (`0x00476768`), which
    // `Director::listen` makes on every change of the open message record; a
    // turn that ends with a letter still up would cut the fanfare a frame
    // after it started, which is what "no drum roll" sounds like. Headless
    // voices never finish on their own, so a stop is the only way this goes
    // false.
    let mut survived = true;
    for _ in 0..60 {
        {
            let mut ctx = Ctx { game: &mut game, assets: &assets };
            machine.update(&mut ctx);
        }
        director.listen(&mut audio, &machine, &game);
        survived &= audio.is_playing("ff_batl.wav");
    }
    (opened && survived, audio.heard().iter().map(|s| s.to_string()).collect())
}

/// **The fanfare sounds when the march raises the question.**
///
/// `Battle_ChooseSettlement#1` in `docs/audio.json`.
///
/// **Ablation, run:** drop the `audio.play_file(names::fanfare::BATTLE, ..)`
/// in `Director::listen`'s battle-prompt arm and this goes red with an empty
/// `heard`; the pushed-stack test in `audio::tests` goes red with it.
#[test]
fn marching_into_an_enemy_county_sounds_the_battle_fanfare() {
    let Some(dir) = l2_testkit::install_dir() else {
        l2_testkit::skip!("no game install, so no ff_batl.wav");
    };
    let platform = l2_mods::Platform::builder().base(&dir).build().expect("the install mounts");
    let (opened, heard) = a_march_into_the_enemy(&platform);
    assert!(opened, "the march never raised the battle prompt");
    assert!(
        heard.iter().any(|n| n == "ff_batl.wav"),
        "the battle question opened in silence; heard {heard:?}",
    );
}

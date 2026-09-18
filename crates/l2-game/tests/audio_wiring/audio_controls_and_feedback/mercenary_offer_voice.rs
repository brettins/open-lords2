//! gap of ours.** `L2.eng` group 16 is the twelve nationalities *on the
//! raise-army screen* (`docs/formats/eng.md` §5: consumers `Screen_RaiseArmy`,
//! `Screen_SplitArmyRows`, `UnitPanel_Draw`), not a message group;
//! `Msg_PlayVoice` (`0x004B35C1`) knows three bands and 16 is in none of them;
//!
//! hotspot 1, whose tail is `if (county.mercenaryOffer != 0)
//! FUN_004B3714(offer - 1)`. So the offer *arriving* is silent in the original
//!
//! `[V]` on group 16's consumers, `[D]` that no other site plays S016.

#![allow(unused_imports)]
use super::*;
use super::routing::*;
use l2_game::audio::{self, Audio};
use l2_game::game::Assets;
use l2_game::input::Event;
use l2_game::message;
use l2_game::screen::{Ctx, Machine, ScreenId};
use l2_game::Game;

fn a_season_then_the_army_button(
    platform: &l2_mods::Platform,
    band: u8,
    dismiss_first: bool,
) -> (bool, Vec<String>) {
    let assets = Assets::placeholder();
    let mut game = world();
    game.selected = 1;
    // `Mercenary_AdvanceAll` (`0x004ACA2B`) writes this cache at end of turn;
    // `County::mercenary_offer` is `+0x1AD`, 1..=12 or 0.
    game.kingdom.counties[1].mercenary_offer = band;

    let mut machine = Machine::new(APP_ROOT);
    machine.push(ScreenId::Campaign);
    let mut audio = Audio::headless(&platform.vfs);
    let mut director = audio::Director::new();

    let mut rec = message::Record::default();
    rec.group = 130;
    rec.category = message::category::COUNTY_NOTICE;
    rec.to = game.player;
    assert!(game.messages.enqueue(rec, game.player), "the record was accepted");
    for _ in 0..40 {
        let mut ctx = Ctx { game: &mut game, assets: &assets };
        machine.update(&mut ctx);
        director.listen(&mut audio, &machine, &game);
    }
    assert!(game.messages.is_open(), "the season's window never opened");

    if dismiss_first {
        // `Msg_Dismiss` (`0x00476768`) stops the one-shot on its way out.
        message::dismiss(&mut game);
        let mut ctx = Ctx { game: &mut game, assets: &assets };
        machine.update(&mut ctx);
        drop(ctx);
        director.listen(&mut audio, &machine, &game);
    }

    let b = l2_game::screens::map::SIDEBAR_BUTTONS[0];
    assert_eq!(b.name, "ARMY", "the first sidebar button is hotspot 1");
    let r = b.rect();
    send(&mut machine, &mut game, &assets, Event::Click { x: r.x + r.w / 2, y: r.y + r.h / 2 });
    {
        let mut ctx = Ctx { game: &mut game, assets: &assets };
        machine.update(&mut ctx);
    }
    let opened = machine.ids().contains(&ScreenId::RaiseArmy(1));
    director.listen(&mut audio, &machine, &game);
    (opened, audio.heard().iter().map(|s| s.to_string()).collect())
}

/// `FUN_004B3714(n)` indexes `s_S016_01_wav` (`0x004DF8B8`, `char[16][16]`) by
/// `offer - 1`, so band 1 — `l2_kingdom::mercenary::ROSTER[1]`, the Scottish
/// pikemen of the report — is `S016_01.wav`, and band 12, the Angevin knights,
/// is `S016_12.wav`.
#[test]
fn the_offer_is_announced_when_the_army_screen_opens_after_the_seasons_letter() {
    let Some(dir) = l2_testkit::install_dir() else {
        l2_testkit::skip!("no game install, so no S016 clips");
    };
    let platform = l2_mods::Platform::builder().base(&dir).build().expect("the install mounts");

    for (band, want) in [(1u8, "s016_01.wav"), (5, "s016_05.wav"), (12, "s016_12.wav")] {
        let (opened, spoken) = a_season_then_the_army_button(&platform, band, true);
        assert!(opened, "the ARMY button did not open the raise-army screen");
        assert!(
            spoken.iter().any(|n| n == want),
            "band {band} ({}) was not announced; heard {spoken:?}",
            l2_kingdom::mercenary::ROSTER[band as usize].nationality,
        );
    }
}

/// Half one: `Mercenary_AdvanceAll` (`0x004ACA2B`) writes `+0x1AD` and plays
/// no sound —, so a season that hands county 1 a
/// band is silent. Half two: with the window up the sidebar never sees the
/// click, so the screen the line hangs on does not open.
///
/// **Ablation, run:** move the mercenary arm off the `opened(RaiseArmy)` edge
/// onto the county's `mercenary_offer` alone and half one goes red.
#[test]
fn the_arrival_itself_is_silent_and_a_talking_window_swallows_the_line() {
    let Some(dir) = l2_testkit::install_dir() else {
        l2_testkit::skip!("no game install, so no S016 clips");
    };
    let platform = l2_mods::Platform::builder().base(&dir).build().expect("the install mounts");
    let assets = Assets::placeholder();

    let mut game = world();
    game.selected = 1;
    let mut bands = l2_kingdom::mercenary::MercenaryBands::init(14);
    let mut machine = Machine::new(APP_ROOT);
    machine.push(ScreenId::Campaign);
    let mut audio = Audio::headless(&platform.vfs);
    let mut director = audio::Director::new();
    let mut offered = 0u8;
    for _ in 0..8 {
        bands.advance(&mut game.kingdom.counties, 14, game.kingdom.options.quirks);
        offered = game.kingdom.counties[1].mercenary_offer;
        let mut ctx = Ctx { game: &mut game, assets: &assets };
        machine.update(&mut ctx);
        drop(ctx);
        director.listen(&mut audio, &machine, &game);
        if offered != 0 {
            break;
        }
    }
    assert!(offered != 0, "no band ever stood in county 1 in eight seasons");
    assert!(
        !audio.heard().iter().any(|n| n.starts_with("s016_")),
        "the offer announced itself with no screen open: {:?}",
        audio.heard(),
    );

    let (opened, heard) = a_season_then_the_army_button(&platform, 1, false);
    assert!(!opened, "the sidebar answered a click the message window was holding");
    assert!(
        !heard.iter().any(|n| n == "s016_01.wav"),
        "a band was announced with no raise-army screen: {heard:?}",
    );
}

fn tick(
    machine: &mut Machine,
    game: &mut Game,
    assets: &Assets,
    audio: &mut Audio,
    director: &mut audio::Director,
) {
    let mut ctx = Ctx { game, assets };
    machine.update(&mut ctx);
    drop(ctx);
    director.listen(audio, machine, game);
}

fn a_map_with_an_offer(
    platform: &l2_mods::Platform,
    band: u8,
) -> (Game, Machine, Audio, audio::Director) {
    let mut game = world();
    game.prefs.tip_screens = true;
    game.selected = 1;
    game.kingdom.counties[1].mercenary_offer = band;
    let mut machine = Machine::new(APP_ROOT);
    machine.push(ScreenId::Campaign);
    (game, machine, Audio::headless(&platform.vfs), audio::Director::new())
}

fn press_the_army_button(machine: &mut Machine, game: &mut Game, assets: &Assets) {
    let b = l2_game::screens::map::SIDEBAR_BUTTONS[0];
    assert_eq!(b.name, "ARMY", "the first sidebar button is hotspot 1");
    let r = b.rect();
    let (x, y) = (r.x + r.w / 2, r.y + r.h / 2);
    send(machine, game, assets, Event::Click { x, y });
    send(machine, game, assets, Event::Release { x, y });
}

/// `Tip_Show` (`0x00476DA9`) writes `g_screenId = 0x27` and `Screen_FrameInput`
/// answers the map only on `g_screenId == 0`, so the button is deaf until
/// `FUN_00476E21` puts the screen back. `[V]`; ours is `ScreenId::Tip` over
/// `Campaign`.
#[test]
fn a_tip_window_holds_the_army_button_and_the_band_is_announced_once_it_is_gone() {
    let Some(dir) = l2_testkit::install_dir() else {
        l2_testkit::skip!("no game install, so no S016 clips");
    };
    let platform = l2_mods::Platform::builder().base(&dir).build().expect("the install mounts");
    let assets = Assets::placeholder();

    for (band, want) in [(1u8, "s016_01.wav"), (12, "s016_12.wav")] {
        let (mut game, mut machine, mut audio, mut director) = a_map_with_an_offer(&platform, band);
        for _ in 0..40 {
            tick(&mut machine, &mut game, &assets, &mut audio, &mut director);
        }
        assert!(game.messages.is_open(), "the opening tip never posted; tips are on");
        press_the_army_button(&mut machine, &mut game, &assets);
        tick(&mut machine, &mut game, &assets, &mut audio, &mut director);
        assert!(
            !machine.ids().contains(&ScreenId::RaiseArmy(1)),
            "the sidebar answered a click the tip window was holding",
        );

        // `Msg_Dismiss` (`0x00476768`), then the same click.
        while game.messages.is_open() {
            message::dismiss(&mut game);
            tick(&mut machine, &mut game, &assets, &mut audio, &mut director);
        }
        press_the_army_button(&mut machine, &mut game, &assets);
        tick(&mut machine, &mut game, &assets, &mut audio, &mut director);
        assert!(machine.ids().contains(&ScreenId::RaiseArmy(1)), "the screen did not open");
        assert!(
            audio.heard().iter().any(|n| *n == want),
            "band {band} was not announced; heard {:?}",
            audio.heard(),
        );
    }
}

/// `Tip_Update` (`0x00476AA7`) posts group 209 on `g_screenId == 0x17` — the
/// screen the sidebar has just opened — once `DAT_004F0358` reaches zero, and
/// `Msg_PlayVoice` (`0x004B35C1`) is `Sound_StopOneShot(); Sound_PlayFile(…)`,
/// so `S209_01.wav` **stops** `S016_01.wav`. Measured here: 29 ticks after the
/// screen opens, which is the twenty-frame re-arm plus the window's own voice
/// timer, against a sentence three seconds long. The band's line is audible for
/// a syllable, once per run, and that is the silence reported.
///
/// **Both addresses are `[V]` and this is the original's shape**, so nothing is
/// changed to stop it; the test is here so that a change to either timer says
/// so out loud.
#[test]
fn the_raise_army_screens_own_tip_talks_over_the_band() {
    let Some(dir) = l2_testkit::install_dir() else {
        l2_testkit::skip!("no game install, so no S016 clips");
    };
    let platform = l2_mods::Platform::builder().base(&dir).build().expect("the install mounts");
    let assets = Assets::placeholder();
    let (mut game, mut machine, mut audio, mut director) = a_map_with_an_offer(&platform, 1);
    for _ in 0..40 {
        tick(&mut machine, &mut game, &assets, &mut audio, &mut director);
    }
    while game.messages.is_open() {
        message::dismiss(&mut game);
        tick(&mut machine, &mut game, &assets, &mut audio, &mut director);
    }
    press_the_army_button(&mut machine, &mut game, &assets);
    tick(&mut machine, &mut game, &assets, &mut audio, &mut director);
    assert!(audio.is_playing("s016_01.wav"), "the band was not announced at all");

    let mut cut = None;
    for i in 0..120 {
        tick(&mut machine, &mut game, &assets, &mut audio, &mut director);
        if !audio.is_playing("s016_01.wav") {
            cut = Some(i);
            break;
        }
    }
    assert_eq!(cut, Some(28), "the armoury tip cut the band off at a different tick");
    assert!(
        audio.heard().iter().any(|n| *n == "s209_01.wav"),
        "something other than the armoury tip took the buffer: {:?}",
        audio.heard(),
    );
}

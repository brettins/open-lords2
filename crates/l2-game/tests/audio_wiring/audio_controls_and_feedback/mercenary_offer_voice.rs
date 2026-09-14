//! **The mercenary offer's voice, on the path the player walks to it.**
//!
//! The report on build `EE0CB9233`: *"No VO for 'A band of scottish pikemen
//! are available for hire, my lord' with a mercenary."*
//!
//! gap of ours.** `L2.eng` group 16 is the twelve nationalities *on the
//! raise-army screen* (`docs/formats/eng.md` §5: consumers `Screen_RaiseArmy`,
//! `Screen_SplitArmyRows`, `UnitPanel_Draw`), not a message group;
//! `Msg_PlayVoice` (`0x004B35C1`) knows three bands and 16 is in none of them;
//!
//! hotspot 1, whose tail is `if (county.mercenaryOffer != 0)
//! FUN_004B3714(offer - 1)`. So the offer *arriving* is silent in the original
//!
//! `[V]` on group 16's consumers, `[D]` that no other site plays S016.
//!
//! `speech_and_panels.rs` has the button in isolation. These two are the turn
//! around it: a season's messages read and dismissed first, which is what the
//! player had on screen when he heard nothing.

#![allow(unused_imports)]
use super::*;
use super::routing::*;
use l2_game::audio::{self, Audio};
use l2_game::game::Assets;
use l2_game::input::Event;
use l2_game::message;
use l2_game::screen::{Ctx, Machine, ScreenId};
use l2_game::Game;

/// A season that ends with a message window up, then the ARMY button.
///
/// Returns everything the one-shot buffer was asked for, in order, so the
/// caller can say both *what* spoke and *whether the window's own voice ate
/// it*.
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

    // The season's own letter, posted while the map is up — the window the
    // player was reading when the band walked in.
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
    // `Audio::heard` is a *set*
    // ask it what it holds, never what is past an index taken earlier.
    (opened, audio.heard().iter().map(|s| s.to_string()).collect())
}

/// **The band is announced on the player's own path**, a season's letter read
/// and dismissed first
///
/// `FUN_004B3714(n)` indexes `s_S016_01_wav` (`0x004DF8B8`, `char[16][16]`) by
/// `offer - 1`, so band 1 — `l2_kingdom::mercenary::ROSTER[1]`, the Scottish
/// pikemen of the report — is `S016_01.wav`, and band 12, the Angevin knights,
/// is `S016_12.wav`.
///
/// **Ablation, run:** delete the `audio.play_file(name, true)` in
/// `Director::listen`'s mercenary arm and all three nationalities go red.
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

/// **Why the player heard silence: the arrival says nothing, and a letter
/// still up eats the click that would make it speak.**
///
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

    // A season in which the band really walks in — `Mercenary_AdvanceAll` on
    // the fourteen-county England start — with nobody touching a button.
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

    // The same click with the letter still up: the message window takes it,
    // the raise-army screen never opens, and nothing is said.
    let (opened, heard) = a_season_then_the_army_button(&platform, 1, false);
    assert!(!opened, "the sidebar answered a click the message window was holding");
    assert!(
        !heard.iter().any(|n| n == "s016_01.wav"),
        "a band was announced with no raise-army screen: {heard:?}",
    );
}

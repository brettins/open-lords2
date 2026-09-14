#![allow(unused_imports)]
use super::*;
use super::sound_settings_and_ui::*;
use super::map_and_castle_feedback::*;
use super::*;
use super::routing::*;
use super::ui_and_speech::*;
use l2_game::audio::{self, Audio, Scene};
use l2_game::game::Assets;
use l2_game::input::{Event, Key};
use l2_game::screen::{Ctx, Machine, ScreenId};
use l2_game::message;
use l2_game::screens::setup::SetupPage;
use l2_game::Game;

/// **The narrator has to stop when the window he is reading closes.**
///
/// A player reported it on build `EE0CB9233`: *"VO doesn't seem to stop when
/// the dialogue that produces it is closed, eg tutorial it will finish the
/// line."* `Msg_Dismiss` (`0x00476768`) is where the original does it, four
/// statements in and nowhere near the drawing:
///
/// ```c
/// if (g_messageGroup != 0xc2) Sound_StopOneShot();
/// ```
///
/// **Ablations, run:** delete the `audio.stop_one_shot()` in
/// `Director::listen`'s dismissal block and the first half goes red; drop the
/// `!= FOILED_AGAIN` guard and the second half does.
#[test]
fn closing_a_message_window_stops_the_narrator_unless_it_is_group_194() {
    let Some(dir) = l2_testkit::install_dir() else {
        l2_testkit::skip!("no game install, so no voice clip to cut off");
    };
    let platform = l2_mods::Platform::builder().base(&dir).build().expect("the install mounts");
    let assets = Assets::placeholder();

    // One window, opened for real and spoken, then dismissed. Answers whether
    // the one-shot buffer was still sounding afterwards.
    let speaks_on_after_dismissal = |group: u16| -> bool {
        let mut game = world();
        let mut machine = Machine::new(APP_ROOT);
        machine.push(ScreenId::Campaign);
        let mut audio = Audio::headless(&platform.vfs);
        let mut director = audio::Director::new();

        let mut rec = message::Record::default();
        rec.group = group;
        rec.category = message::category::COUNTY_NOTICE;
        rec.to = game.player;
        assert!(game.messages.enqueue(rec, game.player), "the record was accepted");

        // Run until the ten-tick schedule has put the line in the buffer.
        for _ in 0..40 {
            let mut ctx = Ctx { game: &mut game, assets: &assets };
            machine.update(&mut ctx);
            director.listen(&mut audio, &machine, &game);
        }
        // 194 is inside the diplomatic band, so its clip is a lord's take and
        // not `S194_01.wav`; `Msg_PlayVoice` is what knows that.
        let clip = l2_game::audio::names::message_voice(group, rec.variant)
            .expect("both groups have a clip")
            .to_ascii_lowercase();
        assert!(audio.is_playing(&clip), "{clip} never started; heard {:?}", audio.heard());
        assert!(audio.one_shot_busy(), "and it is the one-shot's occupant");

        // The OK button, by the route `Msg_Dismiss`'s commonest caller takes.
        message::dismiss(&mut game);
        let mut ctx = Ctx { game: &mut game, assets: &assets };
        machine.update(&mut ctx);
        director.listen(&mut audio, &machine, &game);
        audio.one_shot_busy()
    };

    // Group 130 is a plain notice with a system clip, and the same one
    // `a_message_window_speaks_ten_ticks_after_it_opens` uses.
    assert!(
        !speaks_on_after_dismissal(130),
        "the narrator read on past the window that produced him",
    );
    // **Group 194, *\"Foiled again.\"*** — the one group the original exempts.
    assert!(
        speaks_on_after_dismissal(message::group::FOILED_AGAIN),
        "group 194 is the exception Msg_Dismiss carves out and it was cut off",
    );
}

/// **`Opt_ToggleMusic` cuts the narrator too**, which is the half of that
/// function nobody would guess from the row's label:
///
/// ```c
/// if (g_optMusic == 0) { Music_Stop(0); Sound_StopOneShot(); }
/// ```
///
/// `turning_music_off_on_the_sounds_page_stops_the_music` covers the first
/// statement; this is the second. **Ablation, run:** delete the
/// `music_went_off` block and this goes red while that one stays green.
#[test]
fn turning_music_off_also_cuts_whoever_is_speaking() {
    let Some(dir) = l2_testkit::install_dir() else {
        l2_testkit::skip!("no game install, so no voice clip to cut off");
    };
    let platform = l2_mods::Platform::builder().base(&dir).build().expect("the install mounts");
    let assets = Assets::placeholder();
    let mut game = world();
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
    assert!(audio.one_shot_busy(), "the narrator never started: {:?}", audio.heard());

    game.prefs.music = false;
    director.listen(&mut audio, &machine, &game);
    assert!(!audio.one_shot_busy(), "Music: Off left the narrator talking");
    // And the window is still up, so this was the switch and not a dismissal.
    assert!(game.messages.is_open(), "the window closed, which would prove the wrong thing");
}

/// **The mercenary offer is announced, and by the button that announces it.**
///
/// The second defect of the pair: *"No VO for 'A band of scottish pikemen are
/// available for hire, my lord' with a mercenary."* `Sidebar_Button`
/// (`0x0043AE30`) hotspot 1 opens `g_screenId = 0x17` and then, and only when
/// the county has an offer, `FUN_004B3714(offer - 1)`.
///
/// Band 1 is `l2_kingdom::mercenary::ROSTER[1]` — the Scottish pikemen — so
/// the reported line is `S016_01.wav` exactly.
///
/// **Ablations, run:** remove the `mercenary_offer` gate and the no-offer case
/// goes red; remove the `Armoury` gate on the previous stack and the *Change*
/// case does.
#[test]
fn the_mercenary_offer_speaks_when_the_sidebar_opens_the_raise_army_screen() {
    let Some(dir) = l2_testkit::install_dir() else {
        l2_testkit::skip!("no game install, so no S016 clips");
    };
    let platform = l2_mods::Platform::builder().base(&dir).build().expect("the install mounts");
    let assets = Assets::placeholder();

    // The ARMY button on the county the player owns, with `band` standing in
    // its town. Answers what the audio layer decoded.
    let open_raise_army = |band: u8| -> Vec<String> {
        let mut game = world();
        game.selected = 1;
        game.kingdom.counties[1].mercenary_offer = band;
        let mut machine = Machine::new(APP_ROOT);
        machine.push(ScreenId::Campaign);
        let mut audio = Audio::headless(&platform.vfs);
        let mut director = audio::Director::new();
        director.listen(&mut audio, &machine, &game);

        let b = l2_game::screens::map::SIDEBAR_BUTTONS[0];
        assert_eq!(b.name, "ARMY", "the first sidebar button is hotspot 1");
        let r = b.rect();
        send(&mut machine, &mut game, &assets, Event::Click { x: r.x + r.w / 2, y: r.y + r.h / 2 });
        {
            let mut ctx = Ctx { game: &mut game, assets: &assets };
            machine.update(&mut ctx);
        }
        assert!(
            machine.ids().contains(&ScreenId::RaiseArmy(1)),
            "the button did not open the raise-army screen: {:?}",
            machine.ids(),
        );
        director.listen(&mut audio, &machine, &game);
        audio.heard().iter().map(|s| s.to_string()).collect()
    };

    let spoken = open_raise_army(1);
    assert!(
        spoken.iter().any(|n| n == "s016_01.wav"),
        "the Scottish pikemen were not announced; heard {spoken:?}",
    );
    let silent = open_raise_army(0);
    assert!(
        !silent.iter().any(|n| n.starts_with("s016_")),
        "a county with no offer announced one anyway: {silent:?}",
    );

    // **`Armoury_Button` id 2, *Change*, writes the same `g_screenId` and is
    // silent.** Ours is `Transition::Replace`, so the armoury is on the
    // previous tick's stack and that is what tells the two arrivals apart.
    let mut game = world();
    game.selected = 1;
    game.kingdom.counties[1].mercenary_offer = 1;
    let mut machine = Machine::new(APP_ROOT);
    machine.push(ScreenId::Campaign);
    machine.push(ScreenId::Armoury(1));
    let mut audio = Audio::headless(&platform.vfs);
    let mut director = audio::Director::new();
    director.listen(&mut audio, &machine, &game);
    let change = l2_game::screens::armoury::CHANGE_BOX;
    send(
        &mut machine,
        &mut game,
        &assets,
        Event::Click { x: change.x + change.w / 2, y: change.y + change.h / 2 },
    );
    {
        let mut ctx = Ctx { game: &mut game, assets: &assets };
        machine.update(&mut ctx);
    }
    assert!(
        machine.ids().contains(&ScreenId::RaiseArmy(1)) && !machine.ids().contains(&ScreenId::Armoury(1)),
        "Change did not replace the armoury with the raise-army screen: {:?}",
        machine.ids(),
    );
    director.listen(&mut audio, &machine, &game);
    assert!(
        !audio.heard().contains(&"s016_01.wav"),
        "arriving from the armoury announced the band; the original's Change button is silent",
    );
}

/// **The population panel's health line**, `Panel_OpenPopulation`
/// (`0x0043A8F2`) — the twin of `Panel_OpenRation`'s two arms, on the panel
/// next door.
///
/// The interesting half is the *table*: bands 3 and 4 share `S020_04.wav` at
/// `0x004E2058`, so a generated name would speak `S020_05.wav` — a real file —
/// for the healthiest county in the game and nothing would notice.
/// `tests/audio_install.rs` pins the table; this pins the wiring.
///
/// **Ablation, run:** index `POPULATION_HEALTH` by `band + 1` and the first
/// assertion goes red.
#[test]
fn the_population_panel_speaks_the_countys_health_band() {
    let Some(dir) = l2_testkit::install_dir() else {
        l2_testkit::skip!("no game install, so no S020 clips");
    };
    let platform = l2_mods::Platform::builder().base(&dir).build().expect("the install mounts");
    let assets = Assets::placeholder();

    let open_population = |band: u8| -> Vec<String> {
        let mut game = world();
        game.kingdom.counties[1].health_band = band;
        let mut machine = Machine::new(APP_ROOT);
        machine.push(ScreenId::Campaign);
        let mut audio = Audio::headless(&platform.vfs);
        let mut director = audio::Director::new();
        director.listen(&mut audio, &machine, &game);
        machine.push(ScreenId::County(1, l2_game::screens::county::Panel::Population));
        {
            let mut ctx = Ctx { game: &mut game, assets: &assets };
            machine.update(&mut ctx);
        }
        director.listen(&mut audio, &machine, &game);
        audio.heard().iter().map(|s| s.to_string()).collect()
    };

    assert!(open_population(0).iter().any(|n| n == "s020_01.wav"), "band 0");
    assert!(open_population(2).iter().any(|n| n == "s020_03.wav"), "band 2");
    // The duplicate, from the wiring's side.
    for band in [3, 4] {
        let heard = open_population(band);
        assert!(heard.iter().any(|n| n == "s020_04.wav"), "band {band}: {heard:?}");
        assert!(!heard.iter().any(|n| n == "s020_05.wav"), "band {band} spoke the next one along");
    }
}

/// **The information panel says what it is looking at** — `FUN_004B37BC`, the
/// last statement of both functions that open screen `0x04`.
///
/// Four of its five arms are the unit ladder, and the fifth is the ladder's
/// hole: **a merchant is silent**, because `Map_Click` sends a click on one to
/// the stall instead and the panel never opens on it.
/// asserts hardest, because the easy mistake is to fill it.
///
/// **And it is silence in the original, not a gap of ours** — it was reported
/// as one (*"picking a merchant says nothing, where a unit or a castle
/// speaks"*) and there is a third reading that settles it
/// failing to find an arm. `L2.eng` group 31 holds **five** unit descriptions;
/// indices 13 … 16 are these four, in exactly the order `S031_01` … `04`, and
/// index **12** is *"Merchants allow a county to buy needed supplies and raise
/// revenue by selling goods."* — the one member of the run with prose and no
/// recording. Only four `S031_*.wav` ship. `docs/audio.json` `FUN_004b37bc#1`.
///
/// **Ablations, run:** give `UnitKind::Merchant` a line and the silence goes
/// red; drop the owner test on the army arm and the *theirs* case does.
#[test]
fn the_information_panel_speaks_the_unit_it_opened_on() {
    let Some(dir) = l2_testkit::install_dir() else {
        l2_testkit::skip!("no game install, so no S031 clips");
    };
    let platform = l2_mods::Platform::builder().base(&dir).build().expect("the install mounts");
    let assets = Assets::placeholder();

    let open_info_on = |kind: l2_kingdom::UnitKind, owner: u8| -> Vec<String> {
        let mut game = world();
        let unit = l2_kingdom::unit::Unit::new(kind, owner, 10, 10);
        let id = game.kingdom.campaign.units.spawn(unit).expect("a free slot");
        let mut machine = Machine::new(APP_ROOT);
        machine.push(ScreenId::Campaign);
        let mut audio = Audio::headless(&platform.vfs);
        let mut director = audio::Director::new();
        director.listen(&mut audio, &machine, &game);
        machine.push(ScreenId::Info(l2_game::screens::info::Target::Unit(id)));
        {
            let mut ctx = Ctx { game: &mut game, assets: &assets };
            machine.update(&mut ctx);
        }
        director.listen(&mut audio, &machine, &game);
        audio.heard().iter().map(|s| s.to_string()).collect()
    };

    use l2_kingdom::UnitKind;
    // `world()`'s local player is realm 1.
    let mine = open_info_on(UnitKind::Army, 1);
    assert!(mine.iter().any(|n| n == "s031_04.wav"), "my own army: {mine:?}");
    let theirs = open_info_on(UnitKind::Army, 2);
    assert!(theirs.iter().any(|n| n == "s031_03.wav"), "somebody else's army: {theirs:?}");
    let transport = open_info_on(UnitKind::Transport, 1);
    assert!(transport.iter().any(|n| n == "s031_02.wav"), "a transport: {transport:?}");
    let mob = open_info_on(UnitKind::PeasantMob, 2);
    assert!(mob.iter().any(|n| n == "s031_01.wav"), "a peasant mob: {mob:?}");
    // **The hole.** `FUN_004B37BC` has `kind == 1`, `== 4` and `== 2` and no
    // arm for 3.
    let merchant = open_info_on(UnitKind::Merchant, 1);
    assert!(
        !merchant.iter().any(|n| n.starts_with("s031_")),
        "the merchant has no arm in the original's ladder: {merchant:?}",
    );
}



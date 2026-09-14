#![allow(unused_imports)]
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

/// **The Sounds page had no effect on anything audible.** `screens::options`
/// writes `game.prefs`; `Audio` kept a second copy of the same three flags that
/// nothing ever wrote. `App::listen` pushes one into the other, and this is
/// that push, driven the way the event loop drives it.
#[test]
fn turning_music_off_on_the_sounds_page_stops_the_music() {
    let Some(dir) = l2_testkit::install_dir() else {
        l2_testkit::skip!("no game install, so no .wav files to stop playing");
    };
    let platform = l2_mods::Platform::builder().base(&dir).build().expect("the install mounts");
    let mut game = world();
    let mut machine = Machine::new(APP_ROOT);
    machine.push(ScreenId::Campaign);
    let mut audio = Audio::headless(&platform.vfs);
    let mut director = audio::Director::new();
    macro_rules! listen {
        () => {
            director.listen(&mut audio, &machine, &game)
        };
    }

    listen!();
    assert_eq!(audio.music_name().as_deref(), Some("scroll1.wav"));

    // `Opt_ToggleMusic` (`0x004349A4`) — the Sounds page's first row.
    game.prefs.music = false;
    listen!();
    assert_eq!(audio.music_name(), None, "Music: Off left the track playing");
    let mut buf = vec![0f32; 512];
    audio.mix(&mut buf);
    assert!(buf.iter().all(|s| *s == 0.0), "and the mixer is still producing samples");

// And back on, which the original re-derives.
    game.prefs.music = true;
    listen!();
    assert_eq!(audio.music_name().as_deref(), Some("scroll1.wav"), "Music: On did not resume");
}

/// **The pointer click reaches a speaker, once per press — and not from a
/// hotspot.**
///
/// `tests/click.rs` asserts when [`Machine::clicks`] moves; this asserts that
/// [`audio::Director::listen`] turns the movement into `click3.wav` and nothing
/// else into it. `Widget_Test` (`0x0040DA1E`) is the only function in the game
/// whose click sound is live — the other two sites are dead code
/// (`docs/audio.json`) — and it sounds on the **initial press** of a kind-4 or
/// kind-5 widget only.
///
/// The held half is the one worth having: the buffer is drained with
/// [`Audio::mix`] until the click has finished, and then the arrow is held for
/// two seconds of ticks. A click on any repeat pulse would put `click3.wav`
/// back in the mixer, and `is_playing` would see it on that very tick.
///
/// **Ablations, run:** delete the `self.hear_the_click(..)` call in
/// `Director::listen` and the loud assertion goes red; make `hear_the_click`
/// play on `now != 0` and the held assertion does.
#[test]
fn a_widget_press_is_heard_once_and_a_hotspot_press_is_not() {
    let Some(dir) = l2_testkit::install_dir() else {
        l2_testkit::skip!("no game install, so no click3.wav");
    };
    let platform = l2_mods::Platform::builder().base(&dir).build().expect("the install mounts");
    let assets = Assets::placeholder();
    let mut audio = Audio::headless(&platform.vfs);
    let mut director = audio::Director::new();

    // **The sidebar, which is `Hotspot_Test` kind 1.** Each button on a fresh
    // machine, so that every one of them is pressed from the map.
    let mut opened = 0;
    for b in l2_game::screens::map::SIDEBAR_BUTTONS {
        let mut game = world();
        game.selected = 1;
        let mut machine = Machine::new(APP_ROOT);
        machine.push(ScreenId::Campaign);
        director.listen(&mut audio, &machine, &game);
        let r = b.rect();
        send(&mut machine, &mut game, &assets, Event::Click { x: r.x + r.w / 2, y: r.y + r.h / 2 });
        if machine.top_id() != Some(ScreenId::Campaign) {
            opened += 1;
        }
        director.listen(&mut audio, &machine, &game);
        assert!(!audio.heard().contains(&"click3.wav"), "{} is a hotspot and clicked", b.name);
    }
    assert!(opened >= 1, "no sidebar button opened anything, so the silence proves nothing");

    // **The tax arrow, which is `Widget_Test` kind 4.**
    let mut game = world();
    let mut machine = Machine::new(APP_ROOT);
    machine.push(ScreenId::Campaign);
    machine.push(ScreenId::County(1, l2_game::screens::county::Panel::Tax));
    macro_rules! tick {
        () => {{
            let mut ctx = Ctx { game: &mut game, assets: &assets };
            machine.update(&mut ctx);
            director.listen(&mut audio, &machine, &game);
        }};
    }
    tick!();
    let up = l2_game::screens::county::Panel::Tax.increase_button().expect("an up arrow");
    send(&mut machine, &mut game, &assets, Event::Click { x: up.x + up.w / 2, y: up.y + up.h / 2 });
    tick!();
    assert!(audio.heard().contains(&"click3.wav"), "the press was silent; heard {:?}", audio.heard());
    assert!(audio.is_playing("click3.wav"), "and it is sounding now");

    // Let it finish. A second of samples at a time, and no more than ten.
    let mut buf = vec![0f32; 44_100 * 2];
    for _ in 0..10 {
        if !audio.is_playing("click3.wav") {
            break;
        }
        audio.mix(&mut buf);
    }
    assert!(!audio.is_playing("click3.wav"), "click3.wav never finished");

    // Hold for two seconds of ticks.
    let mut steps = 0;
    for t in 0..125 {
        let before = game.kingdom.counties[1].tax_rate;
        tick!();
        if game.kingdom.counties[1].tax_rate != before {
            steps += 1;
        }
        assert!(
            !audio.is_playing("click3.wav"),
            "tick {t} of the hold put the click back in the mixer; the original plays it on the press only"
        );
    }
    assert!(steps >= 4, "the hold must have repeated for its silence to mean anything: {steps}");
    send(&mut machine, &mut game, &assets, Event::Release { x: up.x + up.w / 2, y: up.y + up.h / 2 });
    tick!();
    assert!(!audio.is_playing("click3.wav"), "letting go of the arrow clicked");
}

/// **The field brush's three sounds.** `FUN_00438B02` (`0x00438B02`) answers
/// every one of the five buttons and picks the slot off the terrain it is
/// about to paint: `0x13` → 4 `moo_2.wav`, `2` → 7 `wheat.wav`, and `1`, `0`
/// and `0x19` → 6 `fallow.wav`.
///
/// Driven through the panel's own handler at the pixels the hotspot table
/// gives, with [`audio::Director`] between the two ticks — the brush closes
/// the panel (`g_screenId = 0`), so the tile it painted is gone from the stack
/// by the tick that hears it.
///
/// **Ablation, run:** delete the `hear_the_brush` call in `Director::listen`
/// and all four arms go red.
#[test]
fn the_field_brush_sounds_what_it_paints() {
    use l2_game::screens::info::{Target, BRUSH_DIM, BRUSH_FIELD_X, BRUSH_ROW_Y, BRUSH_WASTE_X};
    use l2_kingdom::field::terrain;
    use l2_kingdom::map::flags;
    let Some(dir) = l2_testkit::install_dir() else {
        l2_testkit::skip!("no game install, so no fallow.wav");
    };
    let platform = l2_mods::Platform::builder().base(&dir).build().expect("the install mounts");
    let assets = Assets::placeholder();

    // brush x, the terrain the tile starts on, and what the original plays.
    let arms: [(i32, bool, u8, &str); 4] = [
        (BRUSH_FIELD_X[1], true, terrain::FALLOW, "wheat.wav"),
        (BRUSH_FIELD_X[2], true, terrain::FALLOW, "moo_2.wav"),
        (BRUSH_FIELD_X[0], true, terrain::GRAIN, "fallow.wav"),
        (BRUSH_WASTE_X[0], false, terrain::WASTE, "fallow.wav"),
    ];
    for (bx, field_menu, from, want) in arms {
        let mut audio = Audio::headless(&platform.vfs);
        let mut director = audio::Director::new();
        let mut game = world();
        // Twenty fields for county 1, which `world` gives the player. The flag
// `field::set_type` refuses a tile that is in no slot.
        let tile = {
            let map = &mut game.kingdom.campaign.map;
            let c = &mut game.kingdom.counties[1];
            for slot in 0..20u8 {
                let (x, y) = (10 + slot % 5, 20 + slot / 5);
                map.set_flags(x, y, flags::FARMLAND);
                map.terrain[l2_kingdom::map::index(x, y)] = from;
                // The panel takes the county off the *map*, not off the slot.
                map.county[l2_kingdom::map::index(x, y)] = 1;
                c.set_field_tile(slot as usize, Some(l2_kingdom::map::index(x, y)));
            }
            l2_kingdom::map::index(10, 20)
        };

        let mut machine = Machine::new(APP_ROOT);
        machine.push(ScreenId::Campaign);
        machine.push(ScreenId::Info(Target::Tile(tile)));
        // The panel's own arrival sound is `scroll1.wav`; none of the brush's
        // three is heard until a button is hit. (`heard` is a set, so this is
        // membership and not a sequence.)
        const BRUSH_WAVS: [&str; 3] = ["moo_2.wav", "wheat.wav", "fallow.wav"];
        director.listen(&mut audio, &machine, &game);
        assert!(
            !BRUSH_WAVS.iter().any(|w| audio.heard().contains(w)),
            "opening the panel painted nothing: {:?}",
            audio.heard()
        );

        let at = (bx + BRUSH_DIM / 2, BRUSH_ROW_Y + BRUSH_DIM / 2);
        send(&mut machine, &mut game, &assets, Event::Click { x: at.0, y: at.1 });
        send(&mut machine, &mut game, &assets, Event::Release { x: at.0, y: at.1 });
        assert_ne!(
            game.kingdom.campaign.map.terrain[tile], from,
            "the click at {at:?} never reached Field_SetType"
        );
        assert_eq!(machine.top_id(), Some(ScreenId::Campaign), "the brush closes the panel");

        director.listen(&mut audio, &machine, &game);
        let brushes: Vec<&str> =
            BRUSH_WAVS.into_iter().filter(|w| audio.heard().contains(w)).collect();
        assert_eq!(
            brushes,
            [want],
            "brush at x {bx} on the {} menu: all {:?}, terrain now {:#x}",
            if field_menu { "field" } else { "waste" },
            audio.heard(),
            game.kingdom.campaign.map.terrain[tile]
        );
    }
}

/// **The castle chooser's five pictures say their own names** —
/// `CastleBuild_Select` (`0x00436B22`), whose last statement is
/// `FUN_004B3940(g_uiHotspotId)`: `S071_02.wav + hotspot * 0x10`.
///
/// The selection is the screen's own field, so the screen reports the line on
/// `Game::spoken` and `Director::listen` plays it — the channel six other
/// screen-local `Sound_PlayFile` sites already use.
///
/// **Ablations, run:** delete the `ctx.game.spoken = …` in `castle.rs`'s click
/// arm and every level goes silent; index `PICKED_CASTLE` by anything but the
/// hotspot and the wrong file is asserted against.
#[test]
fn picking_a_castle_picture_speaks_that_castles_name() {
    use l2_game::screens::castle;
    let Some(dir) = l2_testkit::install_dir() else {
        l2_testkit::skip!("no game install, so no S071 lines");
    };
    let platform = l2_mods::Platform::builder().base(&dir).build().expect("the install mounts");
    let assets = Assets::placeholder();

    for level in 0..5usize {
        let mut audio = Audio::headless(&platform.vfs);
        let mut director = audio::Director::new();
        let mut game = world();
        let mut machine = Machine::new(APP_ROOT);
        machine.push(ScreenId::Campaign);
        machine.push(ScreenId::Castle(1));

        director.listen(&mut audio, &machine, &game);
        let want = l2_game::audio::names::speech::PICKED_CASTLE[level].to_ascii_lowercase();
        assert!(!audio.heard().contains(&want.as_str()), "opening the chooser picks nothing");

        let r = castle::type_rect(level);
        let at = (r.x + r.w / 2, r.y + r.h / 2);
        send(&mut machine, &mut game, &assets, Event::Click { x: at.0, y: at.1 });
        send(&mut machine, &mut game, &assets, Event::Release { x: at.0, y: at.1 });
        director.listen(&mut audio, &machine, &game);

        let spoken: Vec<&str> = l2_game::audio::names::speech::PICKED_CASTLE
            .iter()
            .map(|n| n.to_ascii_lowercase())
            .enumerate()
            .filter(|(_, n)| audio.heard().contains(&n.as_str()))
            .map(|(i, _)| l2_game::audio::names::speech::PICKED_CASTLE[i])
            .collect();
        assert_eq!(
            spoken,
            [l2_game::audio::names::speech::PICKED_CASTLE[level]],
            "picture {level} at {at:?}: heard {:?}",
            audio.heard()
        );
    }
}

/// **The wreck** — `dest_ind.wav`, all six `Sound_RestartSlot(3)` sites, heard
/// by `Director::hear_the_wreck` off the content plane it diffs.
///
/// One army, one foreign dwelling plot: `Unit_BurnDwelling` (`0x00468AE2`)
/// writes content `0x10` → `0x13` and the sound follows on the next listen.
///
/// **Ablations, run:** put the plot in the army's own county and the burn does
/// not fire, so neither does the sound (second arm); change a farmland tile's
/// crop stage instead and the plane has moved with nothing wrecked (third arm).
/// Deleting the `hear_the_wreck` call from `Director::listen` reds the first.
#[test]
fn wrecking_a_dwelling_sounds_and_an_ordinary_tile_change_does_not() {
    use l2_kingdom::map::{flags, terrain};
    let Some(dir) = l2_testkit::install_dir() else {
        l2_testkit::skip!("no game install, so no dest_ind.wav");
    };
    let platform = l2_mods::Platform::builder().base(&dir).build().expect("the install mounts");

    // county owner, whether the step burns, and whether it sounds.
    let arms: [(u8, bool, bool); 2] = [(2, true, true), (1, false, false)];
    for (county_owner, burns, sounds) in arms {
        let mut audio = Audio::headless(&platform.vfs);
        let mut director = audio::Director::new();
        let mut game = world();
        let mut machine = Machine::new(APP_ROOT);
        machine.push(ScreenId::Campaign);

        let army;
        {
            let k = &mut game.kingdom;
            for i in 0..l2_kingdom::map::MAP_TILES {
                k.campaign.map.county[i] = 2;
            }
            k.counties[2].owner = county_owner;
            k.counties[2].population = 400;
            k.campaign.map.set_flags(11, 10, flags::PLOT);
            k.campaign.map.set_terrain(11, 10, terrain::DWELLING);
            let mut u = l2_kingdom::Unit::new(l2_kingdom::UnitKind::Army, 1, 10, 10);
            u.men = 200;
            u.county = 2;
            u.path = vec![(11, 10)];
            army = k.campaign.units.spawn(u).expect("a slot for the army");
        }
        // Seed the plane; the first tick is silent by construction.
        director.listen(&mut audio, &machine, &game);
        assert!(!audio.heard().contains(&"dest_ind.wav"), "the first tick is silent");

        let k = &mut game.kingdom;
        l2_kingdom::movement::step(
            &mut k.campaign.map,
            &mut k.counties,
            &k.realms,
            &mut k.campaign.units,
            army,
        )
        .expect("the army steps at the plot");
        assert_eq!(
            game.kingdom.campaign.map.terrain_at(11, 10) == terrain::DWELLING_BURNT,
            burns,
            "county owner {county_owner}"
        );
        assert_eq!(game.kingdom.counties[2].population, if burns { 300 } else { 400 });

        director.listen(&mut audio, &machine, &game);
        assert_eq!(
            audio.heard().contains(&"dest_ind.wav"),
            sounds,
            "county owner {county_owner}: heard {:?}",
            audio.heard()
        );
    }

    // A crop growing moves the same plane and wrecks nothing.
    let mut audio = Audio::headless(&platform.vfs);
    let mut director = audio::Director::new();
    let mut game = world();
    let mut machine = Machine::new(APP_ROOT);
    machine.push(ScreenId::Campaign);
    game.kingdom.campaign.map.set_flags(11, 10, flags::FARMLAND);
    game.kingdom.campaign.map.set_terrain(11, 10, 4);
    director.listen(&mut audio, &machine, &game);
    game.kingdom.campaign.map.set_terrain(11, 10, 9);
    director.listen(&mut audio, &machine, &game);
    assert!(
        !audio.heard().contains(&"dest_ind.wav"),
        "a crop stage is not a wreck: {:?}",
        audio.heard()
    );
}

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


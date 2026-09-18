#![allow(unused_imports)]
use super::*;
use super::sound_settings_and_ui::*;
use super::speech_and_panels::*;
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

/// **The field brush's three sounds.** `FUN_00438B02` (`0x00438B02`) answers
/// every one of the five buttons and picks the slot off the terrain it is
/// about to paint: `0x13` → 4 `moo_2.wav`, `2` → 7 `wheat.wav`, and `1`, `0`
/// and `0x19` → 6 `fallow.wav`.
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
        let tile = {
            let map = &mut game.kingdom.campaign.map;
            let c = &mut game.kingdom.counties[1];
            for slot in 0..20u8 {
                let (x, y) = (10 + slot % 5, 20 + slot / 5);
                map.set_flags(x, y, flags::FARMLAND);
                map.terrain[l2_kingdom::map::index(x, y)] = from;
                map.county[l2_kingdom::map::index(x, y)] = 1;
                c.set_field_tile(slot as usize, Some(l2_kingdom::map::index(x, y)));
            }
            l2_kingdom::map::index(10, 20)
        };

        let mut machine = Machine::new(APP_ROOT);
        machine.push(ScreenId::Campaign);
        machine.push(ScreenId::Info(Target::Tile(tile)));
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

/// One army, one foreign dwelling plot: `Unit_BurnDwelling` (`0x00468AE2`)
/// writes content `0x10` → `0x13` and the sound follows on the next listen.
#[test]
fn wrecking_a_dwelling_sounds_and_an_ordinary_tile_change_does_not() {
    use l2_kingdom::map::{flags, terrain};
    let Some(dir) = l2_testkit::install_dir() else {
        l2_testkit::skip!("no game install, so no dest_ind.wav");
    };
    let platform = l2_mods::Platform::builder().base(&dir).build().expect("the install mounts");

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


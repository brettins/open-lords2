#![allow(unused_imports)]
use super::*;
use super::load_tests::*;
use super::*;
use super::roundtrip::*;
use super::isolation::*;
use super::autosave::*;
use std::path::PathBuf;
use l2_game::input::{Event, Key};
use l2_game::save::{self, LoadError};
use l2_game::saves;
use l2_game::screen::{Ctx, Machine, ScreenId};
use l2_game::screens::saveload::{Mode, SaveLoadScreen, Status};
use l2_game::turn;
use l2_game::{Assets, Game};
use l2_kingdom::tables::{Tables, Weather};
use l2_kingdom::{Kingdom, Options};


/// `main.rs` turns nothing off, so `g_optTipScreens` is set and
/// `Tip_Update` (`0x00476AA7`) runs every frame. Its last rung, the invasion
/// tip, is the **one arm with no `g_screenId` test**, so it fires over the save
/// box as readily as over the map; `Tip_Show` (`0x00476DA9`) then saves
/// `g_screenId` and writes `0x27`.
///
/// That pauses the count and nothing more: `SaveLoad_Tick` (`0x004AD9F0`) is
/// called from `Screen_HandleInput`'s `g_screenId == '5' || '6'` arm, so `0x27`
/// stops it, and `FUN_00476E21` puts `0x36` back when the tip is dismissed and
/// the count goes on. Ours seated the host on top of the box and never took it
/// off, so `DAT_0057D3C4` never reached zero and the box sat on *"Saving game.
/// Please wait."* for ever.
///
/// **Ablation, run:** drop the unseat arm from `Machine::seat_tip_host` and
/// the file never lands.
#[test]
fn a_tip_over_the_save_box_pauses_the_count_and_the_save_still_lands() {
    use l2_game::screens::saveload::{CONFIRM, WORK_TICKS};

    let own = Saves::new("tip-over-save");
    let (mut game, assets) = bare();
    game.prefs.tip_screens = true;
    // The box goes over the map: `Tip_Update`'s ladder is deaf off it
    // (`crate::tip::View::of`'s `in_play`), and `Tips::delay`, the re-arm
    // `FUN_00476A5D` sets, has to run down before the invasion arm is reached.
    let mut m = Machine::new(ScreenId::Campaign);
    for _ in 0..200 {
        let mut ctx = Ctx { game: &mut game, assets: &assets };
        m.update(&mut ctx);
    }
    // The map's own four tips go first; dismiss them, so the invasion is the
    // one that arrives with the box up. Each dismissal re-arms the delay.
    for _ in 0..400 {
        if m.ids() == vec![ScreenId::Campaign] && game.tips.delay() == 0 {
            break;
        }
        drive(&mut m, &mut game, &assets, &[Event::RightClick { x: 320, y: 240 }]);
        let mut ctx = Ctx { game: &mut game, assets: &assets };
        m.update(&mut ctx);
    }
    assert_eq!(m.ids(), vec![ScreenId::Campaign], "the map is clear");
    assert_eq!(game.tips.delay(), 0, "and the ladder is armed");
    m.push(ScreenId::SaveLoad(Mode::Save));
    let mut events: Vec<Event> = "TIPPED".chars().map(Event::Text).collect();
    events.push(click_widget(CONFIRM));
    events.push(release_widget(CONFIRM));
    drive(&mut m, &mut game, &assets, &events);

    // The tip goes up on the box's next tick — `Machine::update` runs
    // `run_tips` ahead of the top screen — and it goes up *over* the box.
    let player = game.player;
    let crossing = l2_kingdom::units_tick::Incursion { unit: 0, owner: player, county: 1 };
    game.tips.note_incursions(&[crossing], player);
    for _ in 0..=WORK_TICKS {
        let mut ctx = Ctx { game: &mut game, assets: &assets };
        m.update(&mut ctx);
    }
    assert!(m.ids().contains(&ScreenId::Tip), "the tip is up: {:?}", m.ids());
    assert!(own.files().is_empty(), "the host over the box stops the count, as 0x27 does");

    for _ in 0..40 {
        if !own.files().is_empty() {
            break;
        }
        drive(&mut m, &mut game, &assets, &[Event::RightClick { x: 320, y: 240 }]);
        let mut ctx = Ctx { game: &mut game, assets: &assets };
        m.update(&mut ctx);
    }
    assert_eq!(own.files(), vec![file("tipped")], "{:?}", m.ids());
    assert!(!m.ids().contains(&ScreenId::SaveLoad(Mode::Save)), "the box closed: {:?}", m.ids());
}

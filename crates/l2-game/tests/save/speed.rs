#![allow(unused_imports)]
use super::*;
use super::roundtrip::*;
use super::isolation::*;
use super::ui::*;
use std::path::PathBuf;
use std::time::Duration;
use l2_game::input::{Event, Key};
use l2_game::save::{self, LoadError};
use l2_game::saves;
use l2_game::screen::{Ctx, Machine, ScreenId};
use l2_game::screens::saveload::{Mode, SaveLoadScreen, Status};
use l2_game::turn;
use l2_game::{Assets, Game};
use l2_kingdom::tables::{Tables, Weather};
use l2_kingdom::{Kingdom, Options};

/// **The bound is the original's own wait, counted in ticks.**
/// `SaveLoad_Tick`'s `DAT_0057D3C4 = 0x96` (`004a0000.c:5773`) counts
/// iterations of `App_WinMain`'s unthrottled pump (`00400000.c:8256-8262`),
/// ~111,866 a second by `status.txt`: 1.3 ms. Ours cannot beat one
/// [`l2_game::TICK_MS`] tick. The screen counts ticks, so the test counts them
/// and a busy machine cannot fail it; four ticks of slack. `WORK_FRAMES` was
/// 150 ticks, 2.4 s.
///
/// **Ablation, run:** put `WORK_FRAMES` back in `SaveLoadScreen::begin` and
/// *"still on 'Please wait.'"* goes red at tick `WORK_TICKS + 4`.
#[test]
fn a_save_from_the_campaign_map_takes_the_wait_the_original_takes() {
    use l2_game::screens::saveload::{CONFIRM, WORK_TICKS};

    let own = Saves::new("speed");
    let (mut game, assets) = bare();
    game.prefs.tip_screens = false;
    let mut m = Machine::new(ScreenId::SaveLoad(Mode::Save));
    let mut events: Vec<Event> = "TIMED".chars().map(Event::Text).collect();
    events.push(click_widget(CONFIRM));
    events.push(release_widget(CONFIRM));

    let bound = WORK_TICKS as u32 + 4;
    drive(&mut m, &mut game, &assets, &events);
    let mut ticks = 0u32;
    while own.files().is_empty() {
        assert!(ticks < bound, "{ticks} ticks in and still on 'Please wait.'");
        let mut ctx = Ctx { game: &mut game, assets: &assets };
        m.update(&mut ctx);
        ticks += 1;
    }
    let waited = Duration::from_millis(l2_game::TICK_MS as u64) * ticks;
    assert!(waited < Duration::from_millis(100), "the player waits {waited:?}");
    assert_eq!(own.files(), vec![file("timed")], "the file the player asked for");
    assert_eq!(ticks, WORK_TICKS as u32, "the count is {WORK_TICKS} tick, not 0x96");
}

/// Where the time is not, asserted as work rather than wall time:
/// `save::encode` is 68 KB of a four-turn game, and `saves::write` puts those
/// bytes down once, to a temp file it renames.
///
/// **Ablation, run:** write `bytes` twice in `saves::write` and
/// *"the file is one encode"* goes red.
#[test]
fn the_encoder_and_the_write_are_not_the_cost() {
    let own = Saves::new("cost");
    let game = played(4);

    let bytes = save::encode(&game);
    assert!(bytes.len() < 128 * 1024, "encode of a four-turn game is {} bytes", bytes.len());

    let path = saves::write("cost", &game).expect("write");
    assert_eq!(std::fs::read(&path).expect("read back"), bytes, "the file is one encode");
    assert_eq!(own.files(), vec![file("cost")], "the temp file is renamed, not left");
}

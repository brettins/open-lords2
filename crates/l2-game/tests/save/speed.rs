#![allow(unused_imports)]
use super::*;
use super::roundtrip::*;
use super::isolation::*;
use super::ui::*;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use l2_game::input::{Event, Key};
use l2_game::save::{self, LoadError};
use l2_game::saves;
use l2_game::screen::{Ctx, Machine, ScreenId};
use l2_game::screens::saveload::{Mode, SaveLoadScreen, Status};
use l2_game::turn;
use l2_game::{Assets, Game};
use l2_kingdom::tables::{Tables, Weather};
use l2_kingdom::{Kingdom, Options};

/// **The bound is the original's own wait, rounded up to what we can pace.**
/// `SaveLoad_Tick`'s `DAT_0057D3C4 = 0x96` (`004a0000.c:5773`) counts
/// iterations of `App_WinMain`'s unthrottled pump (`00400000.c:8256-8262`),
/// which the install's `status.txt` measures at ~111,866 a second — 1.3 ms for
/// the whole count. Ours cannot beat one [`l2_game::TICK_MS`] tick, so the
/// budget is `WORK_TICKS` ticks plus the encode and the write, and four ticks
/// of slack for a loaded machine. Before this, `working = WORK_FRAMES` made it
/// 150 × 16 ms = 2.4 s.
///
/// **Ablation, run:** put `WORK_FRAMES` back in `SaveLoadScreen::begin` and
/// *"still on 'Please wait.'"* goes red on its way to 2.4 s.
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

    // `main.rs` waits `TICK_MS` between ticks, so the wait a player sees is
    // wall time and the test pays it.
    let tick = Duration::from_millis(l2_game::TICK_MS as u64);
    let budget = tick * (WORK_TICKS as u32 + 4);
    let started = Instant::now();
    drive(&mut m, &mut game, &assets, &events);
    let mut ticks = 0u32;
    while own.files().is_empty() {
        assert!(started.elapsed() < budget, "{ticks} ticks in and still on 'Please wait.'");
        std::thread::sleep(tick);
        let mut ctx = Ctx { game: &mut game, assets: &assets };
        m.update(&mut ctx);
        ticks += 1;
    }
    let waited = started.elapsed();
    assert!(waited < budget, "the save took {waited:?} of a {budget:?} budget");
    assert_eq!(own.files(), vec![file("timed")], "the file the player asked for");
    assert_eq!(ticks, WORK_TICKS as u32, "the count is {WORK_TICKS} tick, not 0x96");
}

/// Where the time is not: the encoder and the file. `save::encode` is 68 KB of
/// a four-turn game and `saves::write` adds a temp file and a rename.
#[test]
fn the_encoder_and_the_write_are_not_the_cost() {
    let _own = Saves::new("cost");
    let game = played(4);

    let t = Instant::now();
    let bytes = save::encode(&game);
    let encode = t.elapsed();

    let t = Instant::now();
    saves::write("cost", &game).expect("write");
    let write = t.elapsed();

    let tick = Duration::from_millis(l2_game::TICK_MS as u64);
    assert!(encode < tick, "encode of {} bytes took {encode:?}", bytes.len());
    assert!(write < tick, "encode and write took {write:?}");
}

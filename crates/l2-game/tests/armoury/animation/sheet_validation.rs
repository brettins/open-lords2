#![allow(unused_imports)]
use super::*;
use super::walker_logic::*;
use super::rendering::*;
use super::*;
use super::hit_map::*;
use super::rack::*;
use super::screenshots::*;
use std::path::PathBuf;
use l2_game::game::Assets;
use l2_game::input::{Event, Rect};
use l2_game::scenario;
use l2_game::screen::{Ctx, Machine, ScreenId};
use l2_game::screens::armoury;
use l2_game::Game;
use l2_kingdom::tables::{Tables, WEAPON_TYPE_COUNT};
use l2_view::Canvas;

/// The five item sheets are five different pictures, all of them present, and
/// all of them carrying the twenty-one frames the two screens index — six
/// weapons, eight portraits, and the six little icons the levy screen prints
/// its stocks beside.
#[test]
fn all_five_armoury_sheets_carry_the_frames_both_screens_ask_for() {
    let (_g, assets) = world!();
    let mut sizes: Vec<usize> = Vec::new();
    for name in armoury::ITEM_SHEETS {
        let sheet = assets.shell.sheet(name).unwrap_or_else(|| panic!("{name} is in the install"));
        for &(frame, ..) in &armoury::WALL {
            assert!(sheet.frame(frame).is_some(), "{name} has no wall frame {frame}");
        }
        for slot in 0..armoury::RACKS_DRAWN {
            let frame = armoury::RACKS[slot].0;
            assert!(sheet.frame(frame).is_some(), "{name} has no rack frame {frame}");
        }
        for i in 0..WEAPON_TYPE_COUNT {
            let frame = l2_game::screens::army::ICON_FRAME_BASE + i;
            assert!(sheet.frame(frame).is_some(), "{name} has no levy icon {frame}");
        }
        sizes.push(sheet.frame(0).map(|f| f.indices.len()).unwrap_or(0));
    }
    assert!(sizes.iter().all(|&n| n > 0), "a sheet decoded to nothing");

    // And one per weapon type for the rack panel.
    for name in armoury::WEAPON_SHEETS {
        let sheet = assets.shell.sheet(name).unwrap_or_else(|| panic!("{name} is in the install"));
        let f = sheet.frame(0).expect("frame 0");
        assert_eq!((f.width, f.height), (100, 100), "{name}'s picture is not 100 x 100");
    }
}

/// **The artwork counts the animation for us.** `Armoury_DrawWalker` reaches
/// frame `cycle + 0` walking in (eight), `pickup / 3 + 8` taking the weapon
/// down (five, `8 … 0x0C`) and `cycle + 0x0D` carrying it out (eight). That is
/// 21, and every one of the thirty `Trp_*.pl8` sheets holds exactly 21 frames.
///
/// The height is the second coincidence and it is a better one: the frames are
/// 158 tall and `Screen_Armoury` saves its four strips at height `0x9E`, which
/// **is** 158. A strip is exactly one soldier.
#[test]
fn every_walker_sheet_holds_exactly_the_frames_the_walk_reaches() {
    let (_g, assets) = world!();
    let last = armoury::CARRY_FIRST + armoury::WALK_PHASES as usize - 1;
    assert_eq!(last, 20, "the walk's highest frame index");

    let mut seen = 0;
    for colour in 0..armoury::WALKER_SHEETS.len() as u8 {
        for slot in 1..=WEAPON_TYPE_COUNT as u8 {
            let name = armoury::walker_sheet(colour, slot);
            let sheet = assets.shell.sheet(name).unwrap_or_else(|| panic!("{name} is installed"));
            let f = sheet.frame(last).unwrap_or_else(|| panic!("{name} has no frame {last}"));
            assert_eq!(f.height as i32, armoury::STRIP_H, "{name} is not one strip tall");
            assert!(
                sheet.frame(last + 1).is_none(),
                "{name} has a frame past the end of the walk",
            );
            seen += 1;
        }
    }
    assert_eq!(seen, 36, "six colours by six weapons, red twice");
}

/// **`Armtorch.pl8` is two torches and says so**: thirteen frames, then
/// thirteen more, which is `Armoury_DrawTorches`' `+ 0x0D` measured off the
/// file. Each `Arm_<weapon>.pl8` is 24, which is
/// `DAT_005AEA48`'s wrap.
#[test]
fn the_two_animation_counters_wrap_where_their_sheets_end() {
    let (_g, assets) = world!();
    let torch = assets.shell.sheet(armoury::TORCH_SHEET).expect("Armtorch.pl8");
    let n = armoury::TORCH_FRAMES as usize;
    assert_eq!(armoury::TORCH_SECOND, n, "the second torch begins where the first ends");
    let first = torch.frame(0).expect("frame 0");
    let second = torch.frame(n).expect("the second torch");
    assert_ne!(
        (first.width, first.height),
        (second.width, second.height),
        "the two blocks are 77 x 49 and 77 x 48 - a boundary an index off by one breaks",
    );
    assert!(torch.frame(n * 2).is_none(), "there is no third torch");

    for name in armoury::WEAPON_SHEETS {
        let sheet = assets.shell.sheet(name).expect("a weapon sheet");
        let last = armoury::WEAPON_FRAMES as usize - 1;
        assert!(sheet.frame(last).is_some(), "{name} has no frame {last}");
        assert!(sheet.frame(last + 1).is_none(), "{name} has a 25th frame");
    }
}


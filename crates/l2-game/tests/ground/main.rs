//! Every window in this game sits on one ground: `Ui_DrawBox` / `FUN_004093E0`,
//! a bordered box tiled out of `Panels.pl8`, with `Ui_DrawInsetRect` for the
//! recesses inside it and `Ui_DrawBoxInterior` for a well with no border. A
//! screen that draws its own filled rectangle instead looks *plausible* under
//! our interface palette — where `ink.panel` happens to be a parchment colour —
//! and is a hole under the game's own. That is `docs/decisions.md` C61's
//! armoury bug, and it is why every assertion in this file is against the
//! install
//!
//! Every test here has been ablated: the line the assertion names was deleted
//! or changed, and the test was watched going red. The specific ablations are
//! recorded at each test, because an ablation nobody wrote down is one nobody
//! will repeat.
//!
//! # One of them passed under ablation, and it is why [`pinned`] exists
//!
//! `the_diplomacy_menu_widget_is_system_frame_64` was written as
//! `frame_is_drawn(&c, &system, diplomacy::MENU_FRAME, …)`. Moving `MENU_FRAME`
//! from 64 to 65 **left it green**, because the painter and the test read the
//! same constant and moved together — `docs/agents.md`'s first way to ablate
//! wrongly, *computing the probe from the constant you are ablating*, met at
//! full speed by someone who had just read the paragraph about it.
//!
//! ## The shared box, `FUN_004093E0` / `Ui_DrawBox` — converted
//!
//! | our site | the original |
//! |---|---|
//! | `diplomacy.rs` the window | `Diplo_DrawScreen` `FUN_004093E0(0x10, 0x20, 0x1C, 0x1B)`, **set 1** |
//! | `battlefield.rs` the yes/no box | `Screen_ConfirmBox` `0x0040CCFA`, `FUN_004093E0(x−0x10, y−0x10, 0xE, 8)`, set 1 |
//! | `battlefield.rs` the outcome banner | `Screen_BattleOutcome` `0x00423241`, `FUN_004093E0(0x10, 0x90, 0x1C, 0x0A)`, set 1 |
//!
//! | our site | the original |
//! |---|---|
//! | `diplomacy.rs` a lord card | `Diplo_DrawLordCard` `Ui_DrawInsetRect(0x30, n*100+0x31, 0x52, 0x4E)` |
//! | `diplomacy.rs` the menu | **one** `Ui_DrawInsetRect(0xD0, 0x60, 0xE8, h)`, `h` per layout — *not one box per row, which is what we drew* |
//! | `diplomacy.rs` the thermometer | `Ui_DrawInsetRect(0x88, n*100+0x40, 10, 0x3F)`, and **only for an AI rival** |
//!
//! | our site | the original |
//! |---|---|
//! | `diplomacy.rs` the six menu widgets | `g_diploWidgets` `0x004DD940`, **`System.pl8` frame 64**, (400, 102+50n) |
//! | `battlefield.rs` yes / no | `g_confirmWidgets` `0x004DD310`, frames **29 and 31** — we drew 51 and 53, *the right sheet and the wrong frames* |
//!
//! | our site | why it stays |
//! |---|---|
//! | `map.rs` × 4 `None =>` arms | the no-chrome fallback beside a real `draw_box` / `draw_misc` / `draw_menu_bar_background` |
//! | `map.rs` the brush popup, its buttons | **removed**: the left click on a field opens screen `0x04`, which draws the original's brush — `docs/arms.json` `ours/brush-popup-on-the-map` |
//! | `map.rs` `draw_unit_banner` | ours, deliberately not the right column; `UnitPanel_Draw` `0x0041B19D` is screen `0x04` |
//! | `map.rs` the INDUSTRY / NOT DRAWN plate | an honest diagnostic |
//! | `village.rs`, `siege.rs`, `armoury.rs`, `army.rs`, `county.rs`, `info.rs`, `divide.rs` arrows | all `if !pen.system_frame(…)` fallbacks |
//! | `battle.rs` the OK button | fallback behind `c.draw_system(canvas, system::OK, …)` |
//! | `index.rs`, `menu.rs` | ours by design and they say so |
//!
//! `battlefield.rs`'s **overview viewport box**, its **selection markers** and
//! its **banner plates** (`draw_banners`) are still ours, and the battlefield's
//! own painter (`Screen_DrawBattlefield` `0x004233F7` and what `Widget_Draw`
//! puts over it) has not been read for them. They are also invisible to the
//! draw audit: no record in `tools/draws/screens.json` names `battlefield.rs`
//! as its `module`, so none of that file's marks is counted on either side of
//! the ratio.
//!
//! Found and removed this pass: `diplomacy.rs`'s **`"OK"`** at
//! `Ui_OkButton(0x1A8, 0x1A6, 0)` — the last of the three the draw audit had
//! catalogued. Also removed, and a *different* shape of the same fault: every
//! `L2.eng` string on that screen was `.to_uppercase()`d, which is a spelling
//! the game does not have, and the target's name was `L2.eng` group 7 — the
//! lord's **title** — where the painter draws `g_playerNames`.


use l2_formats::pl8::DecodedFrame;
use l2_game::game::Assets;
use l2_game::screen::{Ctx, Machine, ScreenId};
use l2_game::screens::{battlefield, diplomacy, divide};
use l2_game::Game;
use l2_view::chrome::panels;
use l2_view::sheet::Sheet;
use l2_view::Canvas;

struct Sheets {
    panels: Sheet,
    system: Sheet,
}

macro_rules! world {
    () => {{
        let Some(dir) = l2_testkit::install_dir() else {
            l2_testkit::skip!("no game install, so there is no artwork to sit a window on");
        };
        let platform =
            l2_mods::Platform::builder().base(&dir).build().expect("the install mounts");
        let assets = Assets::load(&platform.vfs).expect("assets load");
        let sheets = Sheets {
            panels: Sheet::new(platform.vfs.read("Panels.pl8").expect("Panels.pl8"))
                .expect("Panels.pl8 parses"),
            system: Sheet::new(platform.vfs.read("System.pl8").expect("System.pl8"))
                .expect("System.pl8 parses"),
        };
        (rivals(), assets, sheets)
    }};
}

mod pinned_part;
pub use pinned_part::*;
mod diplomacy_part;
pub use diplomacy_part::*;
mod army_and_confirm;
pub use army_and_confirm::*;

fn rivals() -> Game {
    let mut g = Game::new(5);
    g.kingdom.set_county_count(4);
    l2_testkit::chain_neighbours!(g.kingdom);
    for realm in 1..=3usize {
        let r = &mut g.kingdom.realms[realm];
        r.in_play = true;
        r.strength = 3;
        r.county_count = 1;
        r.gold = 4_000;
        r.lord = realm as u8 - 1;
        r.shield_index = realm as u8;
        g.kingdom.counties[realm].owner = realm as u8;
    }
    g.kingdom.realms[1].is_human = true;
    g.player = 1;
    g.selected = 1;
    g.kingdom.init_diplomacy();
    g
}

fn frame_of(screen: ScreenId, g: &mut Game, a: &Assets) -> Canvas {
    let mut c = Canvas::screen();
    let mut m = Machine::new(screen);
    let ctx = Ctx { game: g, assets: a };
    m.draw(&ctx, &mut c);
    c
}

fn px(c: &Canvas, x: i32, y: i32) -> u8 {
    c.at(x as usize, y as usize)
}

fn frame_is_drawn(canvas: &Canvas, sheet: &Sheet, index: usize, x: i32, y: i32, what: &str) {
    let f: DecodedFrame =
        sheet.frame(index).unwrap_or_else(|| panic!("{what}: frame {index:#x} is not in the sheet"));
    let mut opaque = 0usize;
    for row in 0..i32::from(f.height) {
        for col in 0..i32::from(f.width) {
            let i = (row * i32::from(f.width) + col) as usize;
            if !f.opaque[i] {
                continue;
            }
            opaque += 1;
            let (cx, cy) = (x + col, y + row);
            assert_eq!(
                px(canvas, cx, cy),
                f.indices[i],
                "{what}: ({cx}, {cy}) is {:#04x}, but frame {index:#x} pixel ({col}, {row}) is \
                 {:#04x}",
                px(canvas, cx, cy),
                f.indices[i],
            );
        }
    }
    assert!(opaque > 0, "{what}: frame {index:#x} is entirely transparent, so this asserts nothing");
}


/// `FUN_004093E0(0x10, 0x20, 0x1C, 0x1B)` in `Diplo_DrawScreen` (`0x00416CF3`).
const DIPLO_WINDOW_AT: (i32, i32) = (0x10, 0x20);
const DIPLO_OK_AT: (i32, i32) = (0x1A8, 0x1A6);
/// `Ui_DrawInsetRect(0x30, slot*100 + 0x31, 0x52, 0x4E)` in `Diplo_DrawLordCard`
/// (`0x004171EE`).
const CARD_AT: (i32, i32, i32, i32) = (0x30, 0x31, 0x52, 0x4E);
const CARD_PITCH: i32 = 100;
const MENU_INSET_AT: (i32, i32, i32, i32) = (0xD0, 0x60, 0xE8, 0xD0);
/// `g_diploWidgets` (`0x004DD940`) record 0: `(400, 102)`, frame **64**, size 32.
const MENU_WIDGET_AT: (i32, i32) = (400, 102);
const MENU_WIDGET_FRAME: usize = 64;
const OK_FRAME: usize = 0x33;
/// `Ui_DrawInsetRect` (`0x00403DEB`): `0x10` along the top and right edges,
/// `0x1F` along the bottom and left, and no fill between them.
const INSET_TOP_RIGHT: u8 = 0x10;
const INSET_BOTTOM_LEFT: u8 = 0x1F;
/// `Ui_DrawBoxInterior(0x18, 0x80, 0x1A, 0x12)` in `Screen_SplitArmyRows`
/// (`0x00419354`).
const DIVIDE_WELL_AT: (i32, i32, i32, i32) = (0x18, 0x80, 0x1A, 0x12);
const PANELS_CORNER_TL: usize = 0;
const PANELS_TEXTURE: usize = 0x34;
const PANELS_TEXTURE_DIM: usize = 12;
const PANELS_SET_B: usize = 0xCC;
const PANELS_CELL: i32 = 16;
/// `FUN_004093E0(g_confirmX − 0x10, g_confirmY − 0x10, 0xE, 8)` in
/// `Screen_ConfirmBox` (`0x0040CCFA`), and every one of `Ui_OpenConfirm`'s
/// thirteen call sites passes `(0xA0, 0xA0)`.
const CONFIRM_AT: (i32, i32, i32, i32) = (0xA0 - 0x10, 0xA0 - 0x10, 0xE, 8);
/// `g_confirmWidgets` (`0x004DD310`): frames 29 and 31 at `(64, 46)` and
/// `(112, 50)` off the box's own origin, 32 pixels square.
const CONFIRM_YES_AT: (i32, i32) = (0xA0 + 64, 0xA0 + 46);
const CONFIRM_NO_AT: (i32, i32) = (0xA0 + 112, 0xA0 + 50);
const CONFIRM_YES_FRAME: usize = 29;
const CONFIRM_NO_FRAME: usize = 31;


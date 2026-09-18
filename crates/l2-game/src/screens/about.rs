//! **About** — `Screen_About` (`0x0041543F`), `g_screenId` `0x25`.
//!
//! ```text
//! Screen_About():                                               0x0041543F
//!   FUN_004B1DE0()                        an empty body - 11 bytes, `return`
//!   DAT_004EB260 = 2;  DAT_004E65F8 = 1              the menu backdrop latch
//!   FUN_004093E0(0x60, 0xE0, 0x16, 0x09)          border set 1 at (96, 224),
//!                                        22 x 9 cells = 352 x 144; interior
//!                                        Ui_DrawBoxInterior(112, 240, 20, 7)
//!   Eng_DrawString(59, 0, 0x80, 0x0F4, heading)   "Lords 2."      (128, 244)
//!   Eng_DrawString(59, 2, 0x80, 0x110, body)      copyright       (128, 272)
//!   Eng_DrawString(59, 1, 0x80, 0x148, body)      version         (128, 328)
//!   Ui_OkButton(0x194, 0x146, 0)                  System frame 0x33 (404, 326)
//!   FUN_0045240A()                                 present the whole screen
//!   FUN_00452160(1)                                             the flip tick
//! ```
//!
//! `L2.eng` 59/1 is *"Release version  2.03"* — two spaces — and it is a
//! **string in the user's own data files**, not a resource in the executable
//! and not a constant in the code. So this screen reports the version of the
//! game whose `L2.eng` is installed, which is the right answer and is not one
//! we could have produced by knowing it. Our own build stamp is on the title
//! page and deliberately not here: this box is the original's.
//!
//! One writer: `Menu_About` (`0x00434D60`), twenty-three bytes, whose address
//! sits at `0x004DC414` as the fourth record of the **Help** drop-down's action
//! table — `L2.eng` group 3 index 7, *"About"*, under group 3 index 0,
//! *"Help"*. `Menu_OpenDropdown` restores `g_screenId` to the screen underneath
//! before dispatching, and the exit
//! arm still finds the right place to go back to. **Not dead code.**

use l2_view::Canvas;

use crate::input::{Event, Key, Rect};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::shell::{font, Pen};

/// `L2.eng` group 59 — three strings, which is the whole group.
pub const GROUP: usize = 59;

pub const TITLE: usize = 0;
pub const VERSION: usize = 1;
pub const COPYRIGHT: usize = 2;

/// `FUN_004093E0(0x60, 0xE0, 0x16, 0x09)` — border set 1, in 16-pixel cells.
pub const BOX_X: i32 = 0x60;
pub const BOX_Y: i32 = 0xE0;
pub const BOX_COLS: i32 = 0x16;
pub const BOX_ROWS: i32 = 0x09;
pub const BOX_SET: usize = 1;

pub const TEXT_X: i32 = 0x80;
pub const TITLE_Y: i32 = 0x0F4;
pub const COPYRIGHT_Y: i32 = 0x110;
pub const VERSION_Y: i32 = 0x148;

pub const OK: Rect = Rect::new(0x194, 0x146, 24, 24);
pub const OK_MODE: usize = 0;

pub struct AboutScreen;

impl AboutScreen {
    pub fn new() -> AboutScreen {
        AboutScreen
    }
}

impl Default for AboutScreen {
    fn default() -> AboutScreen {
        AboutScreen::new()
    }
}

impl Screen for AboutScreen {
    fn id(&self) -> ScreenId {
        ScreenId::About
    }

    fn title(&self, _ctx: &Ctx) -> String {
        "About — screen 0x25".into()
    }

    fn is_overlay(&self) -> bool {
        true
    }

    fn handle(&mut self, event: Event, _ctx: &mut Ctx) -> Transition {
        match event {
            // arm: 0x0042FF10/about-ok left-release
            Event::Click { x, y } if OK.contains(x, y) => Transition::Pop,
            // arm: 0x0042FF10/about-right right-release
            Event::RightClick { .. } => Transition::Pop,
            // arm: ours/about-keyboard-close key
            Event::KeyDown(Key::Escape) | Event::KeyDown(Key::Enter) => Transition::Pop,
            _ => Transition::Stay,
        }
    }

    fn draw(&mut self, ctx: &Ctx, canvas: &mut Canvas) {
        let a = &ctx.assets.shell;
        let pen = Pen {
            assets: a,
            ink: &ctx.assets.ink,
            chrome: ctx.assets.chrome.as_ref(),
            shadow: Some(font::SHADOW),
            caps: None,
        };
        pen.window(canvas, BOX_X, BOX_Y, BOX_COLS, BOX_ROWS, BOX_SET);
        let title = a.text(GROUP, TITLE).to_string();
        let copyright = a.text(GROUP, COPYRIGHT).to_string();
        let version = a.text(GROUP, VERSION).to_string();
        pen.heading(canvas, TEXT_X, TITLE_Y, &title, font::TEXT);
        pen.body(canvas, TEXT_X, COPYRIGHT_Y, &copyright, font::TEXT);
        pen.body(canvas, TEXT_X, VERSION_Y, &version, font::TEXT);
        pen.ok_button(canvas, OK.x, OK.y, OK_MODE);
    }
}

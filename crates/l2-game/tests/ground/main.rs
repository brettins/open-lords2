//! **The shared dialogue ground, against the real artwork.**
//!
//! ```text
//! LORDS2_DIR="F:\games\Lords of the Realm II" cargo test -p l2-game --test ground
//! ```
//!
//! Every window in this game sits on one ground: `Ui_DrawBox` / `FUN_004093E0`,
//! a bordered box tiled out of `Panels.pl8`, with `Ui_DrawInsetRect` for the
//! recesses inside it and `Ui_DrawBoxInterior` for a well with no border. A
//! screen that draws its own filled rectangle instead looks *plausible* under
//! our interface palette — where `ink.panel` happens to be a parchment colour —
//! and is a hole under the game's own. That is `docs/decisions.md` C61's
//! armoury bug, and it is why every assertion in this file is against the
//! install
//!
//! **The model is `l2-view`'s path-marker test**: specific opaque palette
//! indices at a named position, matched against the sheet the frame came from.
//! A canvas diff would pass on a garbage sprite or on the wrong frame of the
//! right sheet — and the wrong frame of the right sheet is
//! `battlefield.rs` was drawing for the yes/no pair.
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
//! The cure the document prescribes is *pin the literal from the oracle*, and
//! that is [`pinned`]: every number this file shares with the code it tests is
//! stated here as the literal in the decompiled painter, checked against the
//! constant once, and then **the literal is what the probes use**. A constant
//! that moves now fails in `pinned` by name, with the address of the function
//! that says otherwise.
//!
//! # The classification: which ground each of our own rectangles stands for
//!
//! **This table is worth more than the conversions below it.** A window
//! converted to the wrong ground looks *more* right and is *more* wrong, and it
//! is harder to find afterwards than the placeholder it replaced — so the
//! expensive half of this job is establishing, per site, what the original
//! Every row was read out of the decompiled painter named
//! in it. The rows marked *converted* are done; the rest are verified verdicts
//! nobody needs to re-derive.
//!
//! At the time of writing the tree holds **48** call sites of
//! `widget::panel` / `frame` / `button` across 13 screen modules — not the
//! "23 across 8" the campaign-map audit reported, which matches nothing in the
//! tree's history and should be treated as superseded by this list.
//!
//! ## The shared box, `FUN_004093E0` / `Ui_DrawBox` — converted
//!
//! | our site | the original |
//! |---|---|
//! | `diplomacy.rs` the window | `Diplo_DrawScreen` `FUN_004093E0(0x10, 0x20, 0x1C, 0x1B)`, **set 1** |
//! | `battlefield.rs` the yes/no box | `Screen_ConfirmBox` `0x0040CCFA`, `FUN_004093E0(x−0x10, y−0x10, 0xE, 8)`, set 1 |
//! | `battlefield.rs` the outcome banner | `Screen_BattleOutcome` `0x00423241`, `FUN_004093E0(0x10, 0x90, 0x1C, 0x0A)`, set 1 |
//!
//! ## `Ui_DrawInsetRect` — four lines and **no fill** — converted
//!
//! | our site | the original |
//! |---|---|
//! | `diplomacy.rs` a lord card | `Diplo_DrawLordCard` `Ui_DrawInsetRect(0x30, n*100+0x31, 0x52, 0x4E)` |
//! | `diplomacy.rs` the menu | **one** `Ui_DrawInsetRect(0xD0, 0x60, 0xE8, h)`, `h` per layout — *not one box per row, which is what we drew* |
//! | `diplomacy.rs` the thermometer | `Ui_DrawInsetRect(0x88, n*100+0x40, 10, 0x3F)`, and **only for an AI rival** |
//!
//! ## `Ui_DrawBoxInterior` — the parchment with no border — converted
//!
//! | our site | the original |
//! |---|---|
//! | `divide.rs` the rows well | `Screen_SplitArmyRows` `Ui_DrawBoxInterior(0x18, 0x80, 0x1A, 0x12)` |
//!
//! ## A `Widget_Draw` record's sheet frame, not a rectangle at all — converted
//!
//! | our site | the original |
//! |---|---|
//! | `diplomacy.rs` the six menu widgets | `g_diploWidgets` `0x004DD940`, **`System.pl8` frame 64**, (400, 102+50n) |
//! | `battlefield.rs` yes / no | `g_confirmWidgets` `0x004DD310`, frames **29 and 31** — we drew 51 and 53, *the right sheet and the wrong frames* |
//!
//! ## Ours on purpose, and correctly so — **do not convert**
//!
//! Every one of these is either a diagnostic of ours that says the engine is
//! incomplete, or a fallback that runs only when the sheet is missing. The
//! second kind is the large majority and is the reason the count of
//! `widget::*` sites overstates the problem badly: the shape
//! `if !pen.system_frame(…) { widget::… }` is already the right shape.
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
//! ## Not classified — a fresh reader starts here
//!
//! `battlefield.rs`'s **overview viewport box**, its **selection markers** and
//! its **banner plates** (`draw_banners`) are still ours, and the battlefield's
//! own painter (`Screen_DrawBattlefield` `0x004233F7` and what `Widget_Draw`
//! puts over it) has not been read for them. They are also invisible to the
//! draw audit: no record in `tools/draws/screens.json` names `battlefield.rs`
//! as its `module`, so none of that file's marks is counted on either side of
//! the ratio.
//!
//! # Text where the original draws none
//!
//! Found and removed this pass: `diplomacy.rs`'s **`"OK"`** at
//! `Ui_OkButton(0x1A8, 0x1A6, 0)` — the last of the three the draw audit had
//! catalogued. Also removed, and a *different* shape of the same fault: every
//! `L2.eng` string on that screen was `.to_uppercase()`d, which is a spelling
//! the game does not have, and the target's name was `L2.eng` group 7 — the
//! lord's **title** — where the painter draws `g_playerNames`.
//!
//! Still standing, and all of them behind a missing-sheet guard
//! front of the picture: `"+1" "-1" "NONE" "ALL"` (armoury), `"+" "-"` (siege,
//! county), `"<" ">"` (the ration slider), `"CLOSE"` (village), `"YES" "NO"`
//! (battlefield, army). Each is a fallback, so the honest verdict is *not an
//! invention* — but `tools/draws/screendraws.js` counts them all as English we
//! wrote, at 0.

mod pinned_part;
pub use pinned_part::*;
mod diplomacy_part;
pub use diplomacy_part::*;
mod army_and_confirm;
pub use army_and_confirm::*;

use l2_formats::pl8::DecodedFrame;
use l2_game::game::Assets;
use l2_game::screen::{Ctx, Machine, ScreenId};
use l2_game::screens::{battlefield, diplomacy, divide};
use l2_game::Game;
use l2_view::chrome::panels;
use l2_view::sheet::Sheet;
use l2_view::Canvas;

/// The install's own `Panels.pl8` and `System.pl8`, read straight
/// through [`l2_view::chrome::Chrome`] — the point is to compare what we drew
/// with what the file holds, and going through the same object that drew it
/// would be comparing the code with itself.
struct Sheets {
    panels: Sheet,
    system: Sheet,
}

macro_rules! world {
    () => {{
        // `install_dir` is named here
        // `crates/l2-testkit/tests/census.rs` files these seven under "install"
        // and not under the catch-all gate.
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

/// Realm 1 is the person and realms 2 and 3 are AI rivals, so the diplomacy
/// screen has two cards and its no-ally menu.
fn rivals() -> Game {
    let mut g = Game::new(5);
    g.kingdom.set_county_count(4);
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

/// [`Canvas::at`] takes `usize`; every coordinate in the painters is `i32`.
fn px(c: &Canvas, x: i32, y: i32) -> u8 {
    c.at(x as usize, y as usize)
}

/// **Every opaque pixel of `frame`, at `(x, y)`, is on the canvas.**
///
/// The `opaque > 0` line is not decoration: a frame index that decodes to
/// nothing would make this function assert *nothing at all* and pass, which is
/// `docs/agents.md`'s *a check that passes for an accidental reason*.
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

// ----------------------------------------------------------- the oracle's own
//
// Every number below is a literal argument in a decompiled painter, written out
// here
// the constant it is meant to be testing. `pinned` is where the two lists are
// joined, and it is the only test that mentions both.

/// `FUN_004093E0(0x10, 0x20, 0x1C, 0x1B)` in `Diplo_DrawScreen` (`0x00416CF3`).
const DIPLO_WINDOW_AT: (i32, i32) = (0x10, 0x20);
/// `Ui_OkButton(0x1A8, 0x1A6, 0)`, same function.
const DIPLO_OK_AT: (i32, i32) = (0x1A8, 0x1A6);
/// `Ui_DrawInsetRect(0x30, slot*100 + 0x31, 0x52, 0x4E)` in `Diplo_DrawLordCard`
/// (`0x004171EE`).
const CARD_AT: (i32, i32, i32, i32) = (0x30, 0x31, 0x52, 0x4E);
const CARD_PITCH: i32 = 100;
/// `Ui_DrawInsetRect(0xD0, 0x60, 0xE8, 0xD0)` — the `g_diploMenuState == 0` arm.
const MENU_INSET_AT: (i32, i32, i32, i32) = (0xD0, 0x60, 0xE8, 0xD0);
/// `g_diploWidgets` (`0x004DD940`) record 0: `(400, 102)`, frame **64**, size 32.
/// `tools/oracle/widgets.js widgets 4dd940 6`.
const MENU_WIDGET_AT: (i32, i32) = (400, 102);
const MENU_WIDGET_FRAME: usize = 64;
/// `Ui_OkButton`'s own frame — `System.pl8` `0x33`, the arrow into a hole.
const OK_FRAME: usize = 0x33;
/// `Ui_DrawInsetRect` (`0x00403DEB`): `0x10` along the top and right edges,
/// `0x1F` along the bottom and left, and no fill between them.
const INSET_TOP_RIGHT: u8 = 0x10;
const INSET_BOTTOM_LEFT: u8 = 0x1F;
/// `Ui_DrawBoxInterior(0x18, 0x80, 0x1A, 0x12)` in `Screen_SplitArmyRows`
/// (`0x00419354`).
const DIVIDE_WELL_AT: (i32, i32, i32, i32) = (0x18, 0x80, 0x1A, 0x12);
/// `Ui_DrawBoxBorder`'s layout inside `Panels.pl8`: corner 0, the interior field
/// at `0x34` as a 12 × 12 grid of 16-pixel cells, and `0xCC` added to every
/// border index for the second set.
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


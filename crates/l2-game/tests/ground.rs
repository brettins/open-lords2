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
//! install rather than against a placeholder.
//!
//! **The model is `l2-view`'s path-marker test**: specific opaque palette
//! indices at a named position, matched against the sheet the frame came from.
//! A canvas diff would pass on a garbage sprite or on the wrong frame of the
//! right sheet — and the wrong frame of the right sheet is exactly what
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
//! actually draws there. Every row was read out of the decompiled painter named
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
//! | `map.rs` the brush popup, its buttons | ours; the original has no such control — see `screens/map.rs`'s `brush` |
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
//! Still standing, and all of them behind a missing-sheet guard rather than in
//! front of the picture: `"+1" "-1" "NONE" "ALL"` (armoury), `"+" "-"` (siege,
//! county), `"<" ">"` (the ration slider), `"CLOSE"` (village), `"YES" "NO"`
//! (battlefield, army). Each is a fallback, so the honest verdict is *not an
//! invention* — but `tools/draws/screendraws.js` counts them all as English we
//! wrote, which is why that figure stands at 16 rather than at 0.

use l2_formats::pl8::DecodedFrame;
use l2_game::game::Assets;
use l2_game::screen::{Ctx, Machine, ScreenId};
use l2_game::screens::{battlefield, diplomacy, divide};
use l2_game::Game;
use l2_view::chrome::panels;
use l2_view::sheet::Sheet;
use l2_view::Canvas;

/// The install's own `Panels.pl8` and `System.pl8`, read straight rather than
/// through [`l2_view::chrome::Chrome`] — the point is to compare what we drew
/// with what the file holds, and going through the same object that drew it
/// would be comparing the code with itself.
struct Sheets {
    panels: Sheet,
    system: Sheet,
}

macro_rules! world {
    () => {{
        // `install_dir` is named here rather than behind a helper so that
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
// here rather than imported, so that no probe in this file can be computed from
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

/// **The join, and the reason every other test in this file uses literals.**
///
/// This is the only place where a constant from the crate meets the number the
/// decompilation carries. Ablating any of those constants fails *here*, by name,
/// with the painter's address in the message — instead of quietly moving a probe
/// along with the thing it was supposed to be probing.
///
/// It needs no install and is deliberately not gated: a constant that has drifted
/// away from the binary is wrong on a machine with no copy of the game too.
#[test]
fn pinned() {
    assert_eq!(
        (diplomacy::WINDOW.x, diplomacy::WINDOW.y),
        DIPLO_WINDOW_AT,
        "Diplo_DrawScreen 0x00416CF3: FUN_004093E0(0x10, 0x20, …)",
    );
    assert_eq!(diplomacy::WINDOW_COLS, 0x1C);
    assert_eq!(diplomacy::WINDOW_ROWS, 0x1B);
    assert_eq!(diplomacy::WINDOW_SET, 1, "FUN_004093E0 is Ui_DrawBoxBorder(1, …)");
    assert_eq!((diplomacy::OK.x, diplomacy::OK.y), DIPLO_OK_AT);
    assert_eq!(diplomacy::MENU_FRAME, MENU_WIDGET_FRAME, "g_diploWidgets record 0, +4");
    assert_eq!((diplomacy::menu_widget(0).x, diplomacy::menu_widget(0).y), MENU_WIDGET_AT);
    assert_eq!(
        (diplomacy::MENU_INSET_X, diplomacy::MENU_INSET_Y, diplomacy::MENU_INSET_W),
        (MENU_INSET_AT.0, MENU_INSET_AT.1, MENU_INSET_AT.2),
    );
    assert_eq!(
        diplomacy::Menu::NoAlly.inset_height(),
        Some(MENU_INSET_AT.3),
        "the g_diploMenuState == 0 arm's Ui_DrawInsetRect height",
    );
    for slot in 0..3usize {
        let r = diplomacy::card_rect(slot);
        assert_eq!(
            (r.x, r.y, r.w, r.h),
            (CARD_AT.0, slot as i32 * CARD_PITCH + CARD_AT.1, CARD_AT.2, CARD_AT.3),
            "Diplo_DrawLordCard 0x004171EE: card {slot}",
        );
    }
    let w = divide::rows_well();
    assert_eq!(
        (w.x, w.y, divide::ROWS_WELL_COLS, divide::ROWS_WELL_ROWS),
        DIVIDE_WELL_AT,
        "Screen_SplitArmyRows 0x00419354: Ui_DrawBoxInterior(0x18, 0x80, 0x1A, 0x12)",
    );
    assert_eq!(panels::CORNER_TL, PANELS_CORNER_TL);
    assert_eq!(panels::TEXTURE, PANELS_TEXTURE);
    assert_eq!(panels::TEXTURE_DIM, PANELS_TEXTURE_DIM);
    assert_eq!(panels::SET_B, PANELS_SET_B);
    assert_eq!(panels::CELL, PANELS_CELL);
    assert_eq!(l2_view::chrome::system::OK, OK_FRAME, "Ui_OkButton mode 0");
    assert_eq!(
        (battlefield::CONFIRM_BOX.x, battlefield::CONFIRM_BOX.y),
        (CONFIRM_AT.0, CONFIRM_AT.1),
    );
    assert_eq!((battlefield::CONFIRM_COLS, battlefield::CONFIRM_ROWS), (CONFIRM_AT.2, CONFIRM_AT.3));
    assert_eq!(battlefield::BOX_SET, 1);
    assert_eq!((battlefield::CONFIRM_YES.x, battlefield::CONFIRM_YES.y), CONFIRM_YES_AT);
    assert_eq!((battlefield::CONFIRM_NO.x, battlefield::CONFIRM_NO.y), CONFIRM_NO_AT);
    assert_eq!(battlefield::CONFIRM_YES_FRAME, CONFIRM_YES_FRAME, "g_confirmWidgets record 0");
    assert_eq!(battlefield::CONFIRM_NO_FRAME, CONFIRM_NO_FRAME, "g_confirmWidgets record 1");
    assert_ne!(
        battlefield::CONFIRM_YES_FRAME,
        OK_FRAME,
        "the yes button is not Ui_OkButton's close corner, which is what this screen drew",
    );
}

// ---------------------------------------------------------------- diplomacy

/// **The other lords sits on `Panels.pl8`, in border set 1.**
///
/// `Diplo_DrawScreen`'s first draw is `FUN_004093E0(0x10, 0x20, 0x1C, 0x1B)`,
/// and `FUN_004093E0` is `Ui_DrawBoxBorder(**1**, …)` — so the corner at the
/// box's origin is `Panels.pl8` frame `CORNER_TL + 0xCC` and not frame 0. This
/// painter drew a `widget::panel` there: a flat `ink.panel` fill with a
/// one-pixel `ink.border` round it.
///
/// The **set** is half the claim and it has bitten this project once already:
/// adding `0xCC` to an *interior* index sends it past 255 and into the banner
/// frames, which is how the custom-game screen's option boxes came out full of
/// shields. Asserting the corner rather than the interior is what makes the set
/// visible at all.
///
/// **Ablated** two ways: `pen.window(…, WINDOW_SET)` back to
/// `widget::panel(canvas, ink, WINDOW)` — every corner pixel becomes `ink.panel`
/// or `ink.border`; and `WINDOW_SET` from 1 to 0, which fails on the first
/// corner pixel that differs between the two sets.
#[test]
fn the_diplomacy_window_is_panels_pl8_in_border_set_one() {
    let (mut g, a, s) = world!();
    let c = frame_of(ScreenId::Diplomacy, &mut g, &a);
    frame_is_drawn(
        &c,
        &s.panels,
        PANELS_CORNER_TL + PANELS_SET_B,
        DIPLO_WINDOW_AT.0,
        DIPLO_WINDOW_AT.1,
        "the diplomacy window's top-left corner",
    );
}

/// **A lord card is `Ui_DrawInsetRect`: four lines and no fill.**
///
/// `Diplo_DrawLordCard` opens with `Ui_DrawInsetRect(0x30, slot*100 + 0x31,
/// 0x52, 0x4E)` — colour `0x10` along the top and right edges and `0x1F` along
/// the bottom and left, and *nothing in the middle*. This screen drew a filled
/// `widget::panel` there, which is the same shape as the armoury's black hole:
/// the fill is invisible under our own palette, where the interface's panel
/// colour is the parchment colour, and is a rectangle of black under
/// `Panels.pl8`'s.
///
/// The two probes are the **left** and **top** edges, and they are chosen so
/// nothing can overwrite them: the portrait is blitted at `(r.x + 1, r.y + 1)`,
/// one pixel inside, so a sprite of any size leaves column `r.x` and row `r.y`
/// alone. The colours `0x1F` and `0x10` are written out here as literals from
/// `Ui_DrawInsetRect` (`0x00403DEB`) rather than read from the code under test,
/// because `docs/agents.md`'s first way to ablate wrongly is computing the probe
/// from the constant being ablated.
///
/// **Ablated** by deleting `pen.inset(canvas, r)`: both probes become the
/// window's parchment.
#[test]
fn a_lord_card_is_an_inset_and_not_a_plate() {
    let (mut g, a, _s) = world!();
    let c = frame_of(ScreenId::Diplomacy, &mut g, &a);
    for slot in 0..2i32 {
        let (rx, ry, rw, rh) =
            (CARD_AT.0, slot * CARD_PITCH + CARD_AT.1, CARD_AT.2, CARD_AT.3);
        assert_eq!(
            px(&c, rx, ry + rh / 2),
            INSET_BOTTOM_LEFT,
            "card {slot}'s left edge at ({}, {}) is not Ui_DrawInsetRect's 0x1F",
            rx,
            ry + rh / 2,
        );
        assert_eq!(
            px(&c, rx + rw / 2, ry),
            INSET_TOP_RIGHT,
            "card {slot}'s top edge at ({}, {}) is not Ui_DrawInsetRect's 0x10",
            rx + rw / 2,
            ry,
        );
    }
}

/// **The menu has one recess behind all of it, and the height is the layout's.**
///
/// `Diplo_DrawScreen` draws `Ui_DrawInsetRect(0xD0, 0x60, 0xE8, h)` once, with
/// `h` `0xD0` on the no-ally layout, `0x130` when allied to this lord and `0xA0`
/// when allied elsewhere — and **nothing at all** on the fourth. This painter
/// drew a filled box *per menu row* instead, which is a shape the original does
/// not have anywhere on the screen.
///
/// The bottom edge is the probe because it is the only thing that moves when the
/// height is wrong, and a per-row box could never put `0x1F` there.
///
/// **Ablated** by returning `Some(0xE0)` from `Menu::inset_height`: the bottom
/// edge lands sixteen pixels lower and the probe reads the window's parchment.
#[test]
fn the_diplomacy_menu_has_one_recess_and_it_is_the_layouts_height() {
    let (mut g, a, _s) = world!();
    let c = frame_of(ScreenId::Diplomacy, &mut g, &a);
    // Nobody is allied in `rivals()`, so this is the no-ally layout, whose
    // recess is 0xD0 tall. Every number here is the painter's, not the crate's.
    let (ix, iy, _iw, ih) = MENU_INSET_AT;
    let bottom = iy + ih - 1;
    let x = ix + 4;
    assert_eq!(
        px(&c, x, bottom),
        INSET_BOTTOM_LEFT,
        "the recess's bottom edge at ({x}, {bottom})",
    );
    assert_eq!(px(&c, ix, iy + 4), INSET_BOTTOM_LEFT, "and its left edge");
}

/// **The six menu pictures are `System.pl8` frame 64**, which nothing in this
/// tree drew: the menu widgets were filled rectangles of ours.
///
/// The frame is not a guess. `g_diploWidgets` (`0x004DD940`) is six 24-byte
/// records and every one carries 64 at `+4`, read out of the executable by
/// `tools/oracle/widgets.js widgets 4dd940 6`.
///
/// **Ablated** by moving `MENU_FRAME` to 65 — and the first time, with the test
/// reading `diplomacy::MENU_FRAME` for its own expectation, **it stayed green**.
/// It now reads the pinned literal, and the same ablation turns this test *and*
/// [`pinned`] red. That is the case a canvas diff cannot see, and it took two
/// goes to build a test that could.
#[test]
fn the_diplomacy_menu_widget_is_system_frame_64() {
    let (mut g, a, s) = world!();
    let c = frame_of(ScreenId::Diplomacy, &mut g, &a);
    frame_is_drawn(
        &c,
        &s.system,
        MENU_WIDGET_FRAME,
        MENU_WIDGET_AT.0,
        MENU_WIDGET_AT.1,
        "the first menu widget",
    );
}

/// **The corner is `Ui_OkButton`'s picture and not the word OK.**
///
/// `Ui_OkButton(0x1A8, 0x1A6, 0)` draws `System.pl8` frame `0x33`, which decodes
/// to a cursor arrow pointing into a small black hole — among the darkest frames
/// in the sheet. This screen drew the two letters `OK` in our 5 × 7 debug font,
/// the last of the three `Ui_OkButton` inventions the draw audit found, and
/// `tools/draws/screens.json` named it in `literals_ours` until this pass.
///
/// **Ablated** by putting `widget::button(canvas, ink, OK, "OK", true)` back:
/// the frame's opaque pixels land on our own rectangle instead.
#[test]
fn the_diplomacy_corner_is_the_ok_picture_and_carries_no_letters() {
    let (mut g, a, s) = world!();
    let c = frame_of(ScreenId::Diplomacy, &mut g, &a);
    frame_is_drawn(
        &c,
        &s.system,
        OK_FRAME,
        DIPLO_OK_AT.0,
        DIPLO_OK_AT.1,
        "the diplomacy screen's Ui_OkButton",
    );
}

// ------------------------------------------------------------ army division

/// **The division rows sit on parchment, not on a hole.**
///
/// `Screen_SplitArmyRows`' first statement is `Ui_DrawBoxInterior(0x18, 0x80,
/// 0x1A, 0x12)` — the 12 × 12 texture field at `Panels.pl8` frame `0x34`, tiled,
/// with no border. This drew `fill_rect(well, ink.background)` with an outline of
/// ours over it: under our palette a dark plate that reads as deliberate, and
/// under the game's a black rectangle in the middle of the window.
///
/// The expected frame is `TEXTURE + col % 12 + (row % 12) * 12` at the well's
/// own origin, which is `Ui_DrawBoxInterior`'s tiling and not `Ui_DrawBox`'s —
/// the box insets its interior by a cell and this primitive does not, so
/// starting the tiling in the wrong place is a real way to be wrong here.
///
/// **Ablated** two ways: deleting the `pen.box_interior` call, which leaves the
/// window's own interior tiling underneath — a *different* phase of the same
/// texture, because `Ui_DrawBox` insets its interior by a cell and this
/// primitive does not, so the probe disagrees on the first cell; and narrowing
/// `ROWS_WELL_COLS` to 0x19, which turns [`pinned`] red as well.
#[test]
fn the_army_division_rows_sit_on_the_parchment_field() {
    let (mut g, a, s) = world!();
    let c = frame_of(ScreenId::Divide(0), &mut g, &a);
    let (wx, wy) = (DIVIDE_WELL_AT.0, DIVIDE_WELL_AT.1);
    // **The probes are the two bottom cell rows, and that is not arbitrary.**
    // The well is 26 × 18 cells and the eight troop rows are painted over the
    // top of it — nouns at x 0x18, icons at 0xA8 and 0x158, numbers at 0xD8 and
    // 0x188, arrow pairs at y 120 + 32n — so a probe in the upper two thirds
    // reads a glyph or a sprite rather than the ground. The first draft of this
    // test probed cell (0, 0) and read `0x10`, which is the body font's own
    // shadow colour under `Ui_DrawUnitNoun(2, 0x34, 0x18, 0x80)`: the parchment
    // was there all along and the probe was on top of the first row's word.
    for (col, row) in [(0i32, 16i32), (25, 16), (0, 17), (25, 17)] {
        let index = PANELS_TEXTURE
            + (col as usize % PANELS_TEXTURE_DIM)
            + (row as usize % PANELS_TEXTURE_DIM) * PANELS_TEXTURE_DIM;
        frame_is_drawn(
            &c,
            &s.panels,
            index,
            wx + col * PANELS_CELL,
            wy + row * PANELS_CELL,
            &format!("the division well's cell ({col}, {row})"),
        );
    }
}

// ------------------------------------------------------------- the yes/no box

/// **The confirmation box is the shared ground and the mailed hands.**
///
/// Two claims, and the second is the one a canvas diff would have missed.
///
/// `Screen_ConfirmBox` (`0x0040CCFA`) is three statements: `FUN_004093E0(x −
/// 0x10, y − 0x10, 0xE, 8)`, an `Eng_DrawString`, and the widget pass. The
/// ground was a `fill_rect(ink.background)` here.
///
/// And `g_confirmWidgets` (`0x004DD310`) carries frames **29 and 31** — a mailed
/// hand thumb up and thumb down. This drew `system::OK` and `system::OK + 2`,
/// which are the close corner and its neighbour: **the right sheet, the wrong
/// frames, at the right coordinates.** Nothing that compares two canvases of our
/// own could tell; only the sheet can.
///
/// **Ablated** by putting `system::OK` (51) back as `CONFIRM_YES_FRAME`, which
/// fails on the thumb's first opaque pixel and in [`pinned`]; and by moving
/// `BOX_SET` to 0, which fails on the corner and in [`pinned`].
#[test]
fn the_yes_no_box_is_the_shared_ground_and_the_thumb_pair() {
    let (mut g, a, s) = world!();
    let mut c = Canvas::screen();
    // The box is drawn by the battlefield's painter while a prompt is up, and
    // there is no battle here — so this draws the two calls directly, at the
    // module's own constants, which is what the painter passes them.
    let pen = l2_game::shell::Pen {
        assets: &a.shell,
        ink: &a.ink,
        chrome: a.chrome.as_ref(),
        shadow: Some(l2_game::shell::font::SHADOW),
        caps: None,
    };
    pen.window(
        &mut c,
        battlefield::CONFIRM_BOX.x,
        battlefield::CONFIRM_BOX.y,
        battlefield::CONFIRM_COLS,
        battlefield::CONFIRM_ROWS,
        battlefield::BOX_SET,
    );
    // The frames the painter passes, from the crate — so that a wrong frame in
    // the crate is drawn here and then caught below against the pinned literal.
    pen.system_frame(
        &mut c,
        battlefield::CONFIRM_YES_FRAME,
        battlefield::CONFIRM_YES.x,
        battlefield::CONFIRM_YES.y,
    );
    pen.system_frame(
        &mut c,
        battlefield::CONFIRM_NO_FRAME,
        battlefield::CONFIRM_NO.x,
        battlefield::CONFIRM_NO.y,
    );
    let _ = &mut g;

    frame_is_drawn(
        &c,
        &s.panels,
        PANELS_CORNER_TL + PANELS_SET_B,
        CONFIRM_AT.0,
        CONFIRM_AT.1,
        "the confirmation box's corner",
    );
    frame_is_drawn(
        &c,
        &s.system,
        CONFIRM_YES_FRAME,
        CONFIRM_YES_AT.0,
        CONFIRM_YES_AT.1,
        "the confirmation box's thumb up",
    );
    frame_is_drawn(
        &c,
        &s.system,
        CONFIRM_NO_FRAME,
        CONFIRM_NO_AT.0,
        CONFIRM_NO_AT.1,
        "the confirmation box's thumb down",
    );
}

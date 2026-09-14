#![allow(unused_imports)]
use super::*;
use super::helpers::*;
use super::ui::*;
use l2_view::Canvas;
use crate::input::{Event, Key, Rect};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::setup::SetupOptions;
use crate::shell::{self, font, Pen};
use crate::text::{self, TextField};

// ------------------------------------------------------------ menu geometry

/// A menu item on pages 1, 2 and 4: `FUN_00403EE4(x, y, 192, 24)` with the
/// label centred in the same 192 pixels, four below the top.
pub const ITEM_W: i32 = 0xC0;
pub const ITEM_H: i32 = 0x18;
/// Pages 1 and 2 put every item at the same x.
pub const ITEM_X: i32 = 0xE0;
/// And step them 36 apart from y = 91: `0x5B, 0x7F, 0xA3, 0xC7, 0xEB`.
pub const ITEM_Y: i32 = 0x5B;
pub const ITEM_STEP: i32 = 0x24;
/// The label sits five pixels into the recess.
pub(super) const ITEM_TEXT: i32 = 5;

pub fn item_rect(index: usize) -> Rect {
    Rect::new(ITEM_X, ITEM_Y + index as i32 * ITEM_STEP, ITEM_W, ITEM_H)
}

/// `FUN_004148E4`'s three `FUN_00403CF4` calls: the name field, the file list
/// and the status line, inside the one `FUN_00403EE4` recess at (112, 66).
pub const LOAD_OUTLINES: [Rect; 3] = [
    Rect::new(0x78, 0x4A, 0xC0, 0x20),
    Rect::new(0x78, 0x72, 0x160, 0xA4),
    Rect::new(0x78, 0x11E, 0x180, 0x1C),
];

/// **Say on the screen when the game's own fonts did not load.**
///
/// # A silent fallback is the worst shape of defect on this project
///
/// Every [`Pen`] method falls back to `l2_view::text`, our 5 × 7 debug font,
/// when the face it wants is `None` — per call
/// checkout that cannot read `Fntl2_14.pl8` renders the *entire* front end in a
/// squat all-capitals face, with every call site perfectly correct and nothing
/// anywhere to say why. A player reported exactly that:
///
/// > *"all caps of that font is ridiculous"*
///
/// and there was no way, from the picture, to tell it from a font we had
/// chosen. The code did the wrong thing, correctly, and said nothing.
///
/// `ShellAssets::load` complains to **stderr**, which is the right place for a
/// developer and no place at all for somebody who double-clicked an executable.
/// This is the same sentence on the first screen he sees, in the debug font
/// itself — so the banner is drawn in the very face it is complaining about,
/// which is the only self-evidencing form available: if you can read the
/// warning, you are looking at the font it warns about.
///
/// Nothing is drawn when the fonts are there, which is the ordinary case and
/// the one this must not clutter.
pub fn missing_fonts_banner(ctx: &Ctx, canvas: &mut Canvas) {
    let missing = ctx.assets.shell.missing_fonts();
    if missing.is_empty() {
        return;
    }
    let ink = &ctx.assets.ink;
    l2_view::text::draw(canvas, 4, 4, "THE GAME'S OWN FONTS DID NOT LOAD.", ink.bad);
    l2_view::text::draw(canvas, 4, 14, &format!("MISSING: {}", missing.join(", ")), ink.bad);
    l2_view::text::draw(canvas, 4, 24, "THIS IS OUR 5X7 DEBUG FONT, NOT THE GAME'S.", ink.bad);
}

/// Page 1's four items, as `L2.eng` group 11 indices — *"Single player"*,
/// *"Multiple players"*, *"Lords of Magic?"*, *"Exit game"*.
pub const TITLE_ITEMS: [usize; 4] = [2, 3, 47, 4];

/// **`FUN_0041EA14`: `Ui_DrawCentred(11, 0, 0x80, 0x1E, 0x180, &g_fontHeading,
/// 0x3F)`** — the game's own name, *"Lords of the Realm 2"*, on the page a
/// player reaches.
///
/// This was `0x20`, two pixels low, while the subtitle beside it at `0x3A` was
/// right — which is the shape a transcription takes when one argument is read
/// off the painter and its neighbour is not. `crate::screens::menu` draws a
/// title too and **is not reached by the application**; `main.rs` boots to
/// `ScreenId::Setup(SetupPage::Title)`, so this is the one that is seen.
pub const TITLE_X: i32 = 0x80;
pub const TITLE_Y: i32 = 0x1E;
pub const TITLE_W: i32 = 0x180;
/// The subtitle under it — `L2.eng` 11/1, *"The siege is on"*, in the **body**
/// font and **flat**: `FUN_0041EA14` sets `DAT_005AEA40` around this line alone
/// and leaves the heading above it embossed.
pub const SUBTITLE_Y: i32 = 0x3A;
/// Page 2's five — *"Play Now!"*, *"Load a game"*, *"Skirmish!"*,
/// *"Custom game"*, *"Back"*.
pub const OPTION_ITEMS: [usize; 5] = [6, 7, 19, 8, 9];

/// Page 4's two buttons, from `FUN_0041F01F`: *"Back"* at `(0x70, 0xD7)` and
/// *"Continue"* at `(0x150, 0xD7)`, both 192 × 24.
pub const SHIELD_BUTTONS: [(i32, i32, usize); 2] = [(0x70, 0xD7, 9), (0x150, 0xD7, 11)];

/// The five shields: `x = 0x70 + 0x58 i`, `y = 0x8C`, from `FUN_0041F1DD`.
/// `Pl8_DrawFrameHere(panels2, 0xCC, 0xD0, 0x48)` — the 224 x 32 recess the
/// name sits in, and `Ui_DrawText(&g_options, 0xD6, 0x50, …)` the text inside
/// it. Six pixels in and eight down from the plate's corner.
pub const NAME_PLATE_X: i32 = 0xD0;
pub const NAME_PLATE_Y: i32 = 0x48;
pub const NAME_X: i32 = 0xD6;
pub const NAME_Y: i32 = 0x50;

pub const SHIELD_X: i32 = 0x70;
pub const SHIELD_STEP: i32 = 0x58;
pub const SHIELD_Y: i32 = 0x8C;
/// The hit box `FUN_0041F1DD`'s frames occupy — roughly 60 × 65.
pub const SHIELD_W: i32 = 60;
pub const SHIELD_H: i32 = 65;

/// `DAT_004D5548` — the five shield hotspots' colours, read out of the user's
/// own executable and **the identity map**: `[_, 1, 2, 3, 4, 5]`, so hotspot
/// `i` is shield `i`.
///
/// It is written down because it is the only thing that
/// says the picker's left-to-right order *is* the shield numbering, and
/// `FUN_00432FAB` indexes it — which
/// is the shape of a table that could have been a permutation and is not.
/// `[V]`, `tools/maps/pe.js` at `0x004D5548`.
pub const SHIELD_OF_HOTSPOT: [u8; 6] = [1, 1, 2, 3, 4, 5];

/// Pages 5 and 6 share a geometry: `FUN_00403EE4(0x6E, 0x74, 0xA4, 0x18)` and
/// the same 164 × 24 again at `x = 0x16E`, with the label centred in 160
/// pixels from two inside each recess.
pub const PAIR_X: [i32; 2] = [0x6E, 0x16E];
pub(super) const PAIR_TEXT_X: [i32; 2] = [0x70, 0x170];
pub const PAIR_Y: i32 = 0x74;
pub const PAIR_W: i32 = 0xA4;
pub(super) const PAIR_TEXT_W: i32 = 0xA0;

// ------------------------------------------------- the custom game's tables

/// `0x004D3098` — twelve records of `(x, boxY, labelY)`, four columns of three.
///
/// The value box is `FUN_004093E0(x, boxY, 6, 3)`: a `Panels.pl8` box in border
/// set 1, 96 × 48 pixels, with the value centred in 96 from `x + 1` at
/// `boxY + 16`. The label is the wrapped group 102 string at `(x, labelY)`.
pub const OPTION_CELLS: [(i32, i32, i32); 12] = [
    (170, 280, 246),
    (170, 353, 337),
    (170, 426, 410),
    (290, 280, 246),
    (290, 353, 337),
    (290, 426, 410),
    (410, 280, 246),
    (410, 353, 337),
    (410, 426, 410),
    (530, 280, 246),
    (530, 353, 337),
    (530, 426, 410),
];

/// `0x004D3128` — each option's base index into `L2.eng` group 103.
pub const OPTION_BASE: [usize; 12] = [0, 2, 5, 9, 11, 15, 19, 25, 29, 34, 37, 44];

/// `0x004D3158` — the open drop-down: `(x, y, rows)`, `rows` being the item
/// count plus two. [`OPTION_COUNT`] is that minus two.
pub const OPTION_LIST: [(i32, i32, i32); 12] = [
    (170, 280, 4),
    (170, 353, 4),
    (170, 426, 6),
    (290, 280, 4),
    (290, 337, 6),
    (290, 378, 6),
    (410, 280, 8),
    (410, 337, 6),
    (410, 362, 7),
    (530, 280, 5),
    (530, 305, 9),
    (530, 426, 4),
];

/// How many values each option has: `OPTION_LIST[i].2 - 2`.
pub const OPTION_COUNT: [usize; 12] = [2, 2, 4, 2, 4, 4, 6, 4, 5, 3, 7, 2];

/// The value box is six cells wide and three tall.
pub(super) const OPTION_BOX_W: i32 = 6 * 16;
pub(super) const OPTION_BOX_H: i32 = 3 * 16;
/// And the label above it wraps at a hundred pixels.
pub(super) const OPTION_LABEL_W: i32 = 100;

/// Page 7's three buttons — *"Cancel"*, *"Start"*, *"Defaults"* — centred in
/// 76 pixels at `y = 0xC6`. Page 8 adds *"Load"* at `x = 399`.
pub const CUSTOM_BUTTONS: [(i32, usize); 4] = [(0xA5, 12), (0xF3, 13), (0x141, 14), (399, 15)];
pub const CUSTOM_BUTTON_Y: i32 = 0xC6;
pub(super) const CUSTOM_BUTTON_W: i32 = 0x4C;

/// The map list on the custom-game pages: `misc_sel.pl8` frame 0x10 at
/// `(0x1F0, 9)`, five rows of 16 pixels of group 101 from `(0x1F2, 0x8E)`, the
/// selected row filled 105 × 16 from `(0x1F0, 0x8D)`.
pub const MAP_LIST_X: i32 = 0x1F0;
pub(super) const MAP_LIST_TEXT_X: i32 = 0x1F2;
pub const MAP_LIST_Y: i32 = 0x8D;
pub const MAP_LIST_ROW: i32 = 0x10;
pub const MAP_LIST_ROWS: usize = 5;
pub(super) const MAP_LIST_W: i32 = 0x69;
/// Group 101 has sixty entries, one per map slot.
pub const MAP_COUNT: usize = 60;

/// `ScenarioList_Draw`'s scroll bar: three stacked fills at x = 604, from
/// y = 163, 20 wide, **44 pixels of track in total**.
///
/// The heights are `Pct(0x2C, PctOf(part, total))` for the three parts — above
/// the window, the window itself, below it — and the *thumb* is given whatever
/// the three roundings lost: `hb += 0x2C - ha - hb - hc`. A zero-height segment
/// is skipped.
pub const SCROLLBAR_X: i32 = 0x25C;
pub const SCROLLBAR_Y: i32 = 0xA3;
pub const SCROLLBAR_W: i32 = 0x14;
pub const SCROLLBAR_H: i32 = 0x2C;

/// `FUN_00410C71(0, 0x1F0, 9)` inside `ScenarioList_Draw` — the selected map's
/// 128 x 128 minimap, blitted at `(x - 2, y + 3)` like every other caller of
/// that helper.
pub const MAP_THUMB: (i32, i32) = (MAP_LIST_X - 2, 9 + 3);

/// `FUN_0042150B` → `FUN_00414E06`: the skirmish file list's outline, drawn
/// twice, and the corner close button.
pub const SKIRMISH_FILE_LIST: Rect = Rect::new(0x78, 0xAC, 0x160, 0xA5);
pub const SKIRMISH_FILE_OK: (i32, i32) = (0x1F8, 0x164);


#![allow(unused_imports)]
use super::*;
use super::screen::*;
use super::tests_part::*;
use l2_kingdom::industry::{self, CastleRefusal};
use l2_view::{text, Canvas};
use crate::press::{Press, Widget};
use crate::input::{Event, Key, Rect};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::shell::{self, font, Pen};

/// `FUN_00408FCB("cas_back.pl8", 0x1E0)` — a raw 640 × 480 raster read straight
/// into the display buffer. The shipped file is 307,224 bytes, which is
/// `640 * 480 + 24`. The five castle pictures the player points at are in it.
pub const BACKDROP: &str = "Cas_back.pl8";
/// `FUN_0040AE12("caspics.pl8", …)` then `Blit_Raster` — the big preview.
pub const PICS: &str = "Caspics.pl8";
pub const BITS: &str = "Cas_bits.pl8";

/// `DAT_004D2DD0` — the `caspics.pl8` frame for each selection, **one-based,
/// and 0 means no picture**. Selection 1, the motte and bailey, has none.
pub const PICTURE_FRAME: [usize; 5] = [1, 0, 2, 3, 4];

pub const PICTURE: Rect = Rect::new(0x9E, 0x14, 0x140, 200);

/// `DAT_004D2DE8`, `{frame, x, y}` five times — `cas_bits.pl8` frames 0…4, all
/// at **the same (19, 63)**: the name plate above the preview.
pub const NAME_PLATE: [(usize, i32, i32); 5] =
    [(0, 19, 63), (1, 19, 63), (2, 19, 63), (3, 19, 63), (4, 19, 63)];

/// `DAT_004D2E28` — `cas_bits.pl8` frames 5…9, one per selection, each over its
/// own strip in the row of five. **This is the only thing that marks the
/// selection**, and its x values land inside [`TYPE_BOUNDS`]' five rectangles.
pub const SELECTED_MARK: [(usize, i32, i32); 5] =
    [(5, 24, 325), (6, 103, 297), (7, 231, 292), (8, 307, 277), (9, 462, 285)];

pub const STONE_CAPTION: usize = 0x0A;
pub const WOOD_CAPTION: usize = 0x0B;
pub const CAPTION_X: i32 = 0x208;
pub const STONE_CAPTION_Y: i32 = 0x5C;
pub const WOOD_CAPTION_Y: i32 = 0x7C;
pub const MATERIAL_NUM_X: i32 = 0x230;
pub const STONE_NUM_Y: i32 = 0x60;
pub const WOOD_NUM_Y: i32 = 0x80;

pub const STANDING_MARK: usize = 0x0C;
/// `DAT_004D2E64`, indexed by the **standing castle type** 1…5, and the painter
/// subtracts ten from it. Slot 0 is never read: `castleType == 0` skips the
/// draw.
pub const STANDING_MARK_X: [i32; 6] = [0, 52, 153, 253, 370, 538];
pub const STANDING_MARK_DX: i32 = -10;
pub const STANDING_MARK_Y: i32 = 0x110;

/// `FUN_0040328E(71, sel + 1, 0x1F6, 0x18, 0xA0, 100, 0, 0, heading, 0x3F)` —
/// the castle's name, wrapped at 160 pixels in the **22-pixel** font, which
/// steps `0x18` a line.
pub const NAME_AT: (i32, i32) = (0x1F6, 0x18);
pub const NAME_WIDTH: i32 = 0xA0;
pub const NAME_LINE: i32 = 0x18;

pub const WILL_TAKE_AT: (i32, i32) = (0x20C, 0xB4);
pub const WORKFORCE_AT: (i32, i32) = (0x1F2, 0xC4);
pub const SEASON_AT: (i32, i32) = (0x1FC, 0xD4);
pub const TO_BUILD_AT: (i32, i32) = (0x20C, 0xE4);

pub const TAX_PLAQUE: (i32, i32, i32, i32) = (0x70, 0x1AC, 0x1A, 3);
pub const BOOSTS_TAX_AT: (i32, i32) = (0x90, 0x1B4);
pub const START_AT: (i32, i32) = (0xE0, 0x1C8);
pub const BARRACKS_PLAQUE: (i32, i32, i32, i32) = (8, 200, 8, 3);
pub const BARRACKS_AT: (i32, i32) = (0xC, 0xD2);
pub const GARRISON_AT: (i32, i32) = (0xC, 0xE2);

pub const CORNER_OK: Rect = Rect::new(640 - 0x1C, 480 - 0x1C, 24, 24);

pub const THUMB_UP: usize = 29;
pub const THUMB_DOWN: usize = 31;

/// `g_castleTypeWidgets` (`0x004DC818`) — the five picture strips, as
/// `(x1, y1, x2, y2)`
pub const TYPE_BOUNDS: [(i32, i32, i32, i32); 5] = [
    (17, 270, 95, 415),
    (96, 270, 209, 415),
    (210, 270, 290, 415),
    (291, 270, 414, 415),
    (415, 270, 618, 415),
];

pub fn type_rect(level: usize) -> Rect {
    let (x1, y1, x2, y2) = TYPE_BOUNDS[level.min(4)];
    Rect::new(x1, y1, x2 - x1 + 1, y2 - y1 + 1)
}

/// `g_castleBuildWidgets` (`0x004DDB80`) record 0 — the tick, hotspot 1.
///
/// **The table is exactly two records long**, which is what the count of 2 the
/// `0x1B` arm passes should be checked against: `0x004DDB80 + 2 * 24` is
/// `0x004DDBB0`, and that is the base `Screen_DrawWidgets`' `0x12` arm passes.
pub const OK: Rect = Rect::new(432, 440, 32, 32);
pub const CANCEL: Rect = Rect::new(472, 444, 32, 32);

/// `[V]` The thumb goes down on the press and the order is placed twenty
/// frames later. This screen answered a raw click, so it acted at once, drew no
/// pressed picture and played no click — the yes/no box's defect, a second
/// time, on a screen that
pub(super) fn widgets() -> [Widget; 2] {
    [
        Widget::new(OK, crate::arm!("0x00436B59/castle-build-confirm", Delayed)),
        Widget::new(CANCEL, crate::arm!("0x00436B59/castle-build-cancel", Delayed)),
    ]
}


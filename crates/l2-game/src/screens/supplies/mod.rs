//! **Send supplies** — `Screen_SendSupplies` (`0x0041AD5D`), `g_screenId`
//! `0x18`, with a per-frame overlay at `FUN_0041AEA2` (`0x0041AEA2`).
//!
//! # It goes to a county, not to an army
//!
//! The name reads like a supply run to a besieging force. It is not: the
//! destination is a **county**, picked off the kingdom minimap, and what
//! arrives is a `kind == 4` transport unit that walks there and unloads.
//! Nothing in the path names a `g_units` slot as a destination. `Transport_Deliver`
//! (`0x004296B5`) adds the cargo to the destination county's `grain` and `herd`
//! and destroys the unit.
//!
//! # The painter, address by address
//!
//! Two functions, because the numbers change without the frame changing.
//!
//! ```text
//! Screen_SendSupplies():                                        0x0041AD5D
//!   FUN_004050C0()                              the map's animation counters
//!   FUN_0045240A()                                    dirty the whole screen
//!   FUN_004093E0(0x40, 0x30, 0x16, 0x16)     border set 1 at (64, 48) 352x352
//!   FUN_00410A5D(county, 0x60, 0x68)     the 128 x 128 kingdom minimap raster
//!                                        blitted at (94, 107) - see below
//!   Eng_DrawString(33, 0, 0x50, 0x44, heading)   "Send supplies"   (80, 68)
//!   FUN_0040328E(33, 7, 0xF0, 0x70, 0xB0, 100, …, body)
//!                              "Click on a county to supply." (240, 112) w176
//!   Eng_DrawString(33, 1, 0xF0, 0x9E, body)      "Supply from:"   (240, 158)
//!   Eng_DrawString(100, scenario*20 + g_selectedCounty, 0xF0, 0xB0, body)
//!   Eng_DrawString(33, 2, 0xF0, 0xC2, body)      "To:"            (240, 194)
//!   Eng_DrawString(100, scenario*20 + DAT_00553C78, 0xF0, 0xD4, body)
//!   Eng_DrawString(33, 6, 0x60, 0x160, body)  "Dispatch shipment?" (96, 352)
//!
//! FUN_0041AEA2():                          every frame, before the widgets
//!   Ui_DrawBoxInterior(0x50, 0x100, 0x14, 5)              (80, 256) 320 x 80
//!   Ui_DrawInsetRect(0x50, 0x100, 0x140, 0x50)                       likewise
//!   Eng_DrawString(33, 3, 0x58, 0x10C, body)      "Grain"           (88, 268)
//!   Ui_DrawNumber(rec[0], ' ', " ", 0xA0, 0x10C)   grain left      (160, 268)
//!   Pl8_DrawFrame(Misc_cty, 0x21, 0xFA, 0x108)    grain sack      (250, 264)
//!   Ui_DrawNumber(rec[1], ' ', " ", 0x150, 0x10C)  in the cart    (336, 268)
//!   Eng_DrawString(33, 5, 0x58, 0x130, body)      "Cattle"          (88, 304)
//!   Ui_DrawNumber(rec[4], ' ', " ", 0xA0, 0x130)   herd left       (160, 304)
//!   Pl8_DrawFrame(Misc_cty, 0x26, 0xF8, 300)      cattle          (248, 300)
//!   Ui_DrawNumber(rec[5], ' ', " ", 0x150, 0x130)  in the cart    (336, 304)
//! ```
//!
//! The two icons are two pixels apart in x and four in y where the two text
//! rows are exactly 36 apart. That is the original's own untidiness and it is
//! reproduced, because a tidied layout is a layout nobody can check.
//!
//! # The shipment record
//!
//! `DAT_005678C0 + player * 0x18`, six `i32`, **three pairs of (left in the
//! county, in the cart)**:
//!
//! | offset | what |
//! |---|---|
//! | `+0x00` / `+0x04` | grain |
//! | `+0x08` / `+0x0C` | **sheep — written by nothing, ever** |
//! | `+0x10` / `+0x14` | cattle |
//!
//! **This screen is the fourth independent piece of evidence for the cut sheep
//! subsystem** (`docs/bugs.md` D20), and it is the most structural one yet:
//!
//! * `g_sendSuppliesWidgets` holds **a complete sheep row** — a minus/plus pair
//! at (216, 296) and (296, 296) with hotspot id 1 and the same two handlers —
//!   immediately after the six records every caller passes. `count = 6`, so it
//!   is never drawn and never tested.
//! * The two rows that survive carry hotspot ids **0 and 2, skipping 1**. The
//!   id is a row index scaled by eight into the record above, so the numbering
//!   itself is the fossil.
//! * `L2.eng` **33/4 is `Sheep`** and is drawn by nobody.
//! * `Transport_Spawn` still reads `+0x0C` into `unit.troops[1]`, a field
//!   nothing writes and `Transport_Deliver` never adds back — the cut wiring
//!   left connected at one end.
//! * The cattle row sits at y 300 and the orphan sheep row at y 296: the
//!   layout was re-spaced when the row came out.
//!
//! [`SHEEP_ROW`] carries the orphan so the absence is countable
//! merely missing.
//!
//! # The spinner step is one or ten
//!
//! ```c
//! FUN_0043B1CA  /* minus */  if (cart < 11) { if (cart) { cart--; left++; } }
//!                            else           {             cart -= 10; left += 10; }
//! FUN_0043B27A  /* plus  */  if (left < 11) { if (left) { left--; cart++; } }
//!                            else           {             left -= 10; cart += 10; }
//! ```
//!
//! So the pool you are drawing **from** decides the step, not the one you are
//! adding to: a count runs 1, 2, … 10, then jumps 20, 30 and comes back the
//! same way. The auto-repeat is `Widget_Test`'s
//! kind-4 press timer. The sum is conserved, so the cap is structural — you can
//! ship everything the county had when the screen opened and nothing more.
//!
//! # The two icon hotspots are an all-or-nothing toggle
//!
//! `0x004DC7E8`, two records tested at an offset of `(0x40, 0x30)`, sitting on
//! the two commodity pictures — **(254, 264)–(284, 293)** and
//! **(254, 294)–(284, 323)**. `FUN_0043B32A`: if the cart is empty put
//! everything in it, otherwise take everything out.
//!
//! # Dispatching
//!
//! `FUN_0043B04C`, the thumbs-up and thumbs-down at (320, 342) and (360, 346):
//!
//! * thumb **down** → leave, send nothing;
//! * thumb **up** with the destination still equal to the source → open
//!   `Ui_OpenConfirm(0xE, …)`, which is `L2.eng` group 10 index 14,
//!   ***"Quit? (no destination)."*** Its yes leaves **without sending**; its no
//!   returns here. The box exists because the screen opens with source and
//!   destination equal, so dispatching without touching the minimap is the
//!   no-destination case.
//! * thumb **up** with a real destination → `FUN_0043B145`, which is
//!   `Transport_Spawn` and then the county's ration, labour, crowding and
//!   estimate passes **twice**.
//!
//! **`Transport_Spawn` can silently drop the shipment.** It looks for a free
//! road tile and then any free open tile; if neither exists it spawns nothing
//! *and deducts nothing*. No message. That
//! is reproduced — see [`Dispatch::Nowhere`] — because a shipment that
//! evaporates with a warning is not the shipment the original loses.
//!
//! # The minimap's hit test is two pixels off its own picture
//!
//! `FUN_00410A5D` blits at **(94, 107)** and `FUN_0043B412` tests
//! `0x60 <= x < 0xE0 && 0x68 <= y < 0xE8` — **(96, 104)**. The same 2/3-pixel
//! disagreement `docs/screens-county.md` already records for `Minimap_Draw`,
//! here as well, from the same two constants. Reproduced.
//!
//! # How it is reached
//!
//! `Sidebar_Button` (`0x0043AE30`), hotspot id 3 — the third of the five
//! buttons in the strip at y 430. Gated on `county.owner == g_localPlayer`;
//! otherwise it enqueues a message instead. It seeds the record from
//! `county.grain` and `county.herd`, sets the destination equal to the source,
//! and plays `s033_01.wav`.

mod screen;
pub use screen::*;

use l2_view::Canvas;

use crate::input::{Event, Key, Rect};
use crate::press::{Press, Widget};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use l2_kingdom::Kingdom;

use crate::shell::{font, Face, Pen};

/// `L2.eng` group 33 — eight strings.
pub const GROUP: usize = 33;
pub const TITLE: usize = 0;
pub const FROM: usize = 1;
pub const TO: usize = 2;
pub const GRAIN: usize = 3;
/// **Never drawn.** See the module docs.
pub const SHEEP: usize = 4;
pub const CATTLE: usize = 5;
pub const DISPATCH: usize = 6;
pub const PROMPT: usize = 7;

/// `L2.eng` group 100 — county names, twenty per scenario.
pub const COUNTY_GROUP: usize = 100;
pub const COUNTY_STRIDE: usize = 20;

/// `FUN_004093E0(0x40, 0x30, 0x16, 0x16)`.
pub const BOX: (i32, i32, i32, i32, usize) = (0x40, 0x30, 0x16, 0x16, 1);

/// Where the minimap raster is **drawn**, and where it is **hit tested**. The
/// original disagrees with itself by two and three pixels and so does this.
pub const MINIMAP_DRAW: (i32, i32) = (94, 107);
pub const MINIMAP_HIT: Rect = Rect::new(0x60, 0x68, 0x80, 0x80);

pub const TITLE_AT: (i32, i32) = (0x50, 0x44);
pub const PROMPT_AT: (i32, i32, i32) = (0xF0, 0x70, 0xB0);
pub const FROM_AT: (i32, i32) = (0xF0, 0x9E);
pub const FROM_NAME_AT: (i32, i32) = (0xF0, 0xB0);
pub const TO_AT: (i32, i32) = (0xF0, 0xC2);
pub const TO_NAME_AT: (i32, i32) = (0xF0, 0xD4);
pub const DISPATCH_AT: (i32, i32) = (0x60, 0x160);

/// `Ui_DrawBoxInterior`/`Ui_DrawInsetRect(0x50, 0x100, 0x140, 0x50)`.
pub const WELL: Rect = Rect::new(0x50, 0x100, 0x140, 0x50);

/// One commodity row of the cart: label, the two numbers, and the icon.
pub struct Row {
    /// `L2.eng` group 33's label.
    pub label: usize,
    /// The hotspot id, which is also **the pair index into the record**.
    pub id: usize,
    pub label_at: (i32, i32),
    pub left_x: i32,
    pub cart_x: i32,
    /// `Misc_cty.pl8` frame, and where it goes.
    pub icon: usize,
    pub icon_at: (i32, i32),
    /// `FUN_0043B1CA` and `FUN_0043B27A`'s widget squares, 24 pixels.
    pub minus: Rect,
    pub plus: Rect,
    /// The icon's own hotspot, the all-or-nothing toggle.
    pub toggle: Rect,
}

/// The two rows the game passes, in table order.
pub const ROWS: [Row; 2] = [
    Row {
        label: GRAIN,
        id: 0,
        label_at: (0x58, 0x10C),
        left_x: 0xA0,
        cart_x: 0x150,
        icon: 0x21,
        icon_at: (0xFA, 0x108),
        minus: Rect::new(216, 264, 24, 24),
        plus: Rect::new(296, 264, 24, 24),
        toggle: Rect::new(254, 264, 30, 29),
    },
    Row {
        label: CATTLE,
        id: 2,
        label_at: (0x58, 0x130),
        left_x: 0xA0,
        cart_x: 0x150,
        icon: 0x26,
        icon_at: (0xF8, 300),
        minus: Rect::new(216, 300, 24, 24),
        plus: Rect::new(296, 300, 24, 24),
        toggle: Rect::new(254, 294, 30, 29),
    },
];

/// **The seventh and eighth widget records, which the game never passes.**
///
/// `(x, y)` of the minus and plus of the sheep row, hotspot id **1**, both
/// present in `g_sendSuppliesWidgets` at `0x004DD538` past the `count = 6` the
/// three call sites use. Kept as a value so the cut row is *countable*; nothing
/// draws or tests it, and a test asserts that.
pub const SHEEP_ROW: [(i32, i32); 2] = [(216, 296), (296, 296)];

/// The two thumbs. **Kind 5**: the press puts the gauntlet down and the handler
/// runs [`crate::press::DELAYED_FRAMES`] frames later.
pub const THUMB_UP: Rect = Rect::new(320, 342, 32, 32);
pub const THUMB_DOWN: Rect = Rect::new(360, 346, 32, 32);
pub const THUMB_UP_FRAME: usize = 29;
pub const THUMB_DOWN_FRAME: usize = 31;
/// `System.pl8` frames for the spinners: 27 minus, 25 plus, `+1` while held.
pub const MINUS_FRAME: usize = 27;
pub const PLUS_FRAME: usize = 25;

/// The step's own rule: **ten or fewer in the pool you draw from moves one,
/// more than ten moves ten.**
pub const BULK_ABOVE: i32 = 10;
pub const BULK_STEP: i32 = 10;

/// What one dispatch did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dispatch {
    /// Nothing yet.
    None,
    /// The thumb-down, or the confirm box's yes.
    Cancelled,
    /// `Ui_OpenConfirm(0xE, …)` — *"Quit? (no destination)."* is up.
    NoDestination,
    /// A transport is on the road, carrying `(grain, cattle)`.
    Sent(i32, i32),
    /// **`Transport_Spawn` found no free tile.** The cargo is not deducted and
    /// nothing is spawned.
    Nowhere,
}

/// One player's shipment record — three pairs
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Cart {
    /// `(left in the county, in the cart)`.
    pub grain: (i32, i32),
    /// `+0x08`/`+0x0C`. **Written by nothing in the original**, kept so that
    /// the record has the shape the original's does and the absence is a value.
    pub sheep: (i32, i32),
    pub cattle: (i32, i32),
}

impl Cart {
    /// `Sidebar_Button`'s seeding: the county's stores on the left, an empty
    /// cart on the right.
    pub fn open(grain: i32, herd: i32) -> Cart {
        Cart { grain: (grain, 0), sheep: (0, 0), cattle: (herd, 0) }
    }

    fn pair(&mut self, id: usize) -> &mut (i32, i32) {
        match id {
            0 => &mut self.grain,
            1 => &mut self.sheep,
            _ => &mut self.cattle,
        }
    }

    pub fn get(&self, id: usize) -> (i32, i32) {
        match id {
            0 => self.grain,
            1 => self.sheep,
            _ => self.cattle,
        }
    }

    /// `FUN_0043B1CA` — take from the cart, put back in the county. Its arm is
    /// declared on [`widgets`], beside the kind it is answered with.
    pub fn minus(&mut self, id: usize) {
        let p = self.pair(id);
        if p.1 <= BULK_ABOVE {
            if p.1 > 0 {
                p.1 -= 1;
                p.0 += 1;
            }
        } else {
            p.1 -= BULK_STEP;
            p.0 += BULK_STEP;
        }
    }

    /// `FUN_0043B27A` — take from the county, put in the cart. Its arm is
    /// declared on [`widgets`].
    pub fn plus(&mut self, id: usize) {
        let p = self.pair(id);
        if p.0 <= BULK_ABOVE {
            if p.0 > 0 {
                p.0 -= 1;
                p.1 += 1;
            }
        } else {
            p.0 -= BULK_STEP;
            p.1 += BULK_STEP;
        }
    }

    /// `FUN_0043B32A` — the icon. Everything in, or everything out.
    // arm: 0x0043B32A/supplies-all left-press
    pub fn toggle(&mut self, id: usize) {
        let p = self.pair(id);
        if p.1 == 0 {
            p.1 += p.0;
            p.0 = 0;
        } else {
            p.0 += p.1;
            p.1 = 0;
        }
    }
}

pub struct SuppliesScreen {
    from: u8,
    to: u8,
    cart: Cart,
    /// What the last dispatch did, for a test that wants to know without
    /// reading the kingdom.
    pub outcome: Dispatch,
    opened: bool,
    /// `g_sendSuppliesWidgets`' press timer and repeat counter. See
    /// [`WIDGETS`].
    press: Press,
}

/// **`g_sendSuppliesWidgets` (`0x004DD568`) as a table, with the kind byte each
/// record carries**, in the order `Screen_DrawWidgets` walks it.
///
/// Six kind-**4** spinners — the three rows' minus and plus, which therefore
/// auto-repeat — then the two kind-**5** thumbs at `0x004DD538`. Both kinds were
/// already the words `docs/arms.json` filed these four arms under; what was
/// missing was any code that behaved like them. `docs/decisions.md`
/// C148.
pub fn widgets() -> Vec<Widget> {
    let mut out = Vec::with_capacity(8);
    for row in &ROWS {
        out.push(Widget::new(row.minus, crate::arm!("0x0043B1CA/supplies-minus", Repeat)));
        out.push(Widget::new(row.plus, crate::arm!("0x0043B27A/supplies-plus", Repeat)));
    }
    out.push(Widget::new(THUMB_UP, crate::arm!("0x0043B04C/supplies-dispatch", Delayed)));
    out.push(Widget::new(THUMB_DOWN, crate::arm!("0x0043B04C/supplies-cancel", Delayed)));
    out
}

/// [`widgets`]' index of the thumb-up; the thumb-down is the one after it.
const THUMB_UP_INDEX: usize = ROWS.len() * 2;


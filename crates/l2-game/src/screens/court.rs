//! **The court** — `Court_Draw` (`0x00416925`), `g_screenId` `0x09`.
//!
//! # It is not a diplomacy screen
//!
//! It sits next to one in the sidebar and the two were filed together, but the
//! court is the **realm's own balance sheet**: the treasury, the three raw
//! materials, the six weapon stocks, this season's army bill and next season's
//! expected tax. Nothing on it concerns another lord. The one button leads to
//! the standings, `Screen_GreatestNoble` (`0x0041593B`), screen `0x20`.
//!
//! `L2.eng` group **70**, nine strings, verified as the literal argument at all
//! eight of the painter's `Eng_DrawString` sites:
//!
//! ```text
//! 0 Gold   1 Arms   2 Iron   3 Stone   4 Wood
//! 5 Court of   6 Greatest nobles.   7 Total army wages of
//! 8 Tax revenues expected,
//! ```
//!
//! **Index 1, *"Arms"*, is drawn by nothing.** The six weapon icons are laid
//! out with no heading over them, which is the original leaving a label behind
//! in the file rather than us failing to find it. [`ARMS`] names it so that the
//! next reader does not go looking.
//!
//! # The painter, address by address
//!
//! ```text
//! Court_Draw():                                                 0x00416925
//!   DAT_00568468 = 1                              publish the widget count
//!   FUN_004093E0(0x40, 0x30, 0x18, 0x16)     border set 1 at (64, 48), 384x352
//!   Ui_OkButton(0x198, 0x158, 0)                                 (408, 344)
//!   Eng_DrawString(70, 6, 0x88, 0x15C, body)   "Greatest nobles." (136, 348)
//!   Eng_DrawString(70, 5, 0x50, 0x44, heading) "Court of"          (80, 68)
//!   Ui_DrawText(playerName, pen + 0x52, 0x44, heading)     after the label
//!   Ui_DrawInsetRect(0x50, 0x68, 0x160, 0xE0)         (80, 104) 352 x 224
//!   Eng_DrawString(70, 0, 0x60, 0x72, heading)  "Gold"            (96, 114)
//!   Ui_DrawCount(realm.gold, 0, 0xE0, 0x72, heading)  + group 8 "Crowns."
//!   Eng_DrawString(70, 2, 0x60, 0x90, heading)  "Iron"            (96, 144)
//!   Ui_DrawNumber(realm.iron, ' ', " ", 0xE0, 0x90, heading)
//!   Pl8_DrawFrame(Misc_cty, 0x2C, 0x130, 0x90)         iron icon (304, 144)
//!   ... the same three lines for Stone (0x2D, y 174) and Wood (0x2E, y 204)
//!   for i in 0..6:
//!     Pl8_DrawFrame(Misc_cty, 0x30 + i, i * 0x38 + 100, 0xF0)     y 240
//!     Ui_DrawNumberRight(realm.weapons[i], ' ', " ",
//!                        i * 0x38 + 0x54, 0x10A, 0x38, body)      y 266
//!   Eng_DrawString(70, 7, 100, 0x124, body)   "Total army wages of" (100, 292)
//!   Ui_DrawCount(realm.wages, 0, pen + 100, 0x124, body)
//!   Eng_DrawString(70, 8, 100, 0x136, body) "Tax revenues expected," (100, 310)
//!   Ui_DrawCount(realm.field_0x15C, 0, pen + 100, 0x136, body)
//!   FUN_0045240A()
//! ```
//!
//! No `.pl8`, no palette, no clear: it is an overlay over whatever was beneath.
//!
//! # Four rows are drawn in the **heading** font
//!
//! `shells.rs` documented [`Shell::lines`] as *"the body lines, in the 14-pixel
//! font"* and filed indices 0, 2, 3 and 4 there. The painter draws all four
//! with `g_fontHeading`, the 22-pixel one; only index 6 is body font, and index
//! 6 is a **button caption** rather than a line at all. So the shell drew the
//! right words in the wrong size in the wrong role, which is the kind of thing
//! a table of five fields cannot say and a painter can.
//!
//! # `realm + 0x15C` was the one field nobody had
//!
//! *"Tax revenues expected,"* reads a realm word that is in neither
//! `docs/records.json`, `docs/kingdom.md` §2 nor `l2_kingdom::realm::Realm` —
//! all three stop before it. The whole corpus touches it **twice**: this
//! painter reads it, and `FUN_0044BA35` writes it, from the tail of
//! `Tax_RecomputePreview` (`0x0044B80B`):
//!
//! ```c
//! for (c = 1; c < 6; c++) g_realms[c].field_0x15c = 0;
//! for (n = 1; n <= g_countyCount; n++)
//!   if (g_counties[n].owner != 0)
//!     g_realms[g_counties[n].owner].field_0x15c += g_counties[n].taxShown;
//! ```
//!
//! So it is **the empire-wide sum of `County::tax_shown`**, recomputed whenever
//! a tax rate moves. We have every county's `tax_shown` already, so rather than
//! add a realm field that only one screen reads and only one function writes,
//! [`tax_expected`] sums it at the point of use — and says so. That is a
//! divergence from the original's storage and not from its arithmetic: the
//! original caches, we recompute, and the two cannot disagree because the
//! original's cache is invalidated on every write to the input.
//!
//! # The one widget, which the painter does not draw
//!
//! `Screen_DrawWidgets` and `Screen_HandleInput` each carry a `g_screenId == 9`
//! arm doing `Widget_Draw`/`Widget_Test(0, 0, &DAT_004DD928, DAT_00568468)`.
//! The table is one 24-byte record, read out of `Lords2.exe`:
//!
//! ```text
//! x = 336  y = 340  frame = 64  size = 32  handler = FUN_004351C4  kind = 5
//! ```
//!
//! `FUN_004351C4` is `g_screenId = 0x20; Score_RankRealms(); …` — **it opens
//! the standings**, and *"Greatest nobles."* at (136, 348) is its label.
//!
//! **Kind 5 is a deferred button.** `Widget_Test` plays a sound, sets a
//! twenty-frame timer and returns *consumed* without calling the handler; the
//! handler fires when the timer reaches zero. Every button on this screen and
//! on the diplomacy screen behaves that way. We fire immediately and record it
//! as ours — see [`DEFERRED_FRAMES`].
//!
//! # How it is reached
//!
//! `Sidebar_Button` (`0x0043AE30`), hotspot id 2, the rectangle **x 512…542,
//! y 430…458**, and it is **ungated** — unlike ids 1 and 3 there is no
//! ownership or turn test. `screens/index.rs` said it *"opens off a sidebar
//! button that needs a selected county"*; it does not.

use l2_view::Canvas;

use crate::input::{Event, Key, Rect};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::shell::{font, Pen};

/// `L2.eng` group 70.
pub const GROUP: usize = 70;

pub const GOLD: usize = 0;
/// **Never drawn.** The six weapon icons carry no heading.
pub const ARMS: usize = 1;
pub const IRON: usize = 2;
pub const STONE: usize = 3;
pub const WOOD: usize = 4;
pub const COURT_OF: usize = 5;
pub const NOBLES: usize = 6;
pub const WAGES: usize = 7;
pub const TAX_EXPECTED: usize = 8;

/// `L2.eng` group 8 index 0/1 — *"Crown."* / *"Crowns."*, which
/// `Ui_DrawCount(v, 0, …)` picks between.
pub const CROWN_NOUN: usize = 0;

/// `FUN_004093E0(0x40, 0x30, 0x18, 0x16)` — border set 1, in 16-pixel cells.
pub const BOX: (i32, i32, i32, i32, usize) = (0x40, 0x30, 0x18, 0x16, 1);

/// `Ui_DrawInsetRect(0x50, 0x68, 0x160, 0xE0)` — the well the stores sit in.
pub const WELL: Rect = Rect::new(0x50, 0x68, 0x160, 0xE0);

/// `Ui_OkButton(0x198, 0x158, 0)`.
pub const OK: Rect = Rect::new(0x198, 0x158, 24, 24);

/// The heading, and the player's name drawn immediately after it.
pub const HEADING_AT: (i32, i32) = (0x50, 0x44);
/// `Ui_DrawText(name, g_penAdvance + 0x52, 0x44, …)` — **`0x52`, not `0x50`**.
/// The name is nudged two pixels right of the label's own column.
pub const NAME_X: i32 = 0x52;

/// The four store rows: `(group index, label x, value x, y)`. Every one of
/// them is drawn in the **heading** font.
pub const STORE_LABEL_X: i32 = 0x60;
pub const STORE_VALUE_X: i32 = 0xE0;
pub const STORE_ICON_X: i32 = 0x130;
/// Gold, iron, stone, wood — 30 pixels apart, and gold has no icon.
pub const STORE_Y: [i32; 4] = [0x72, 0x90, 0xAE, 0xCC];
/// `Misc_cty.pl8` frames for iron, stone and wood.
pub const STORE_ICON_FRAME: [usize; 3] = [0x2C, 0x2D, 0x2E];

/// The six weapon columns: `x = i * 0x38 + 100` for the icon and
/// `i * 0x38 + 0x54` for the number, which is centred in a 56-pixel box.
pub const WEAPON_PITCH: i32 = 0x38;
pub const WEAPON_ICON_X0: i32 = 100;
pub const WEAPON_ICON_Y: i32 = 0xF0;
pub const WEAPON_NUM_X0: i32 = 0x54;
pub const WEAPON_NUM_Y: i32 = 0x10A;
pub const WEAPON_NUM_W: i32 = 0x38;
/// `Misc_cty.pl8` frame `0x30 + i`, in `Realm::weapons` order — crossbow, mace,
/// sword, pike, bow, armour. **[D]**: the painter's loop indexes both with one
/// `i`, so the frame order *is* the array order; the frames themselves have not
/// been decoded.
pub const WEAPON_FRAME0: usize = 0x30;

/// The two footer lines.
pub const FOOTER_X: i32 = 100;
pub const WAGES_Y: i32 = 0x124;
pub const TAX_Y: i32 = 0x136;

/// `g_courtWidgets` (`0x004DD928`) — one record, the *Greatest nobles* button.
pub const NOBLES_BUTTON: Rect = Rect::new(336, 340, 32, 32);
/// `System.pl8` frame 64, and 65 while it is held down.
pub const NOBLES_FRAME: usize = 64;

/// How long `Widget_Test`'s kind-5 press timer runs before the handler fires.
/// **We do not reproduce it** — see the module docs.
pub const DEFERRED_FRAMES: u8 = 0x14;

/// **`FUN_0044BA35`** — the empire-wide sum of `County::tax_shown`, which the
/// original caches in realm `+0x15C` and we recompute.
///
/// The original's loop skips a county with `owner == 0`; ours skips it by
/// asking for a match on `realm`, and realm 0 is not a realm.
pub fn tax_expected(ctx: &Ctx, realm: u8) -> i32 {
    let k = &ctx.game.kingdom;
    (1..=k.county_count)
        .filter(|&id| k.counties[id].owner == realm)
        .map(|id| k.counties[id].tax_shown)
        .sum()
}

pub struct CourtScreen;

impl CourtScreen {
    pub fn new() -> CourtScreen {
        CourtScreen
    }
}

impl Default for CourtScreen {
    fn default() -> CourtScreen {
        CourtScreen::new()
    }
}

impl Screen for CourtScreen {
    fn id(&self) -> ScreenId {
        ScreenId::Court
    }

    fn title(&self, _ctx: &Ctx) -> String {
        "The court — screen 0x09".into()
    }

    fn is_overlay(&self) -> bool {
        true
    }

    fn handle(&mut self, event: Event, _ctx: &mut Ctx) -> Transition {
        match event {
            // `Screen_FrameInput`'s epilogue, which runs on every screen but
            // `0x12`: a press inside the minimap raster selects that county,
            // recentres the map and sets `g_screenId = 0`. The shell wrapper
            // did this for all seven shells generically; graduating them lost
            // it, and the arm is per screen now because the screens are.
            //
            // Only the raster, not the column — see `screens/job.rs` at the
            // same arm.
            // arm: 0x0042FF10/minimap-under-the-court
            Event::Click { x, y } if l2_view::chrome::minimap_hit_area().contains(x, y) => {
                Transition::Pass
            }
            // `Widget_Test(0, 0, &g_courtWidgets, 1)` — the *Greatest nobles*
            // button, `FUN_004351C4`, which is `g_screenId = 0x20`.
            //
            // **`0x20` is not built**, so this is the arm and not the
            // destination: it is recorded `missing` rather than reproduced, and
            // the screen says so on itself rather than doing nothing when a
            // player presses a button that visibly exists.
            Event::Click { x, y } if NOBLES_BUTTON.contains(x, y) => Transition::Stay,
            // `Ui_OkButtonClicked()` — left release in the 24 x 24 corner box.
            // arm: 0x0042FF10/court-ok
            Event::Click { x, y } if OK.contains(x, y) => Transition::Pop,
            // `g_mouseRightReleased`, anywhere.
            // arm: 0x0042FF10/court-right
            Event::RightClick { .. } => Transition::Pop,
            // **Ours**: the arm has no keyboard test.
            // arm: ours/court-keyboard-close
            Event::KeyDown(Key::Escape) | Event::KeyDown(Key::Enter) => Transition::Pop,
            _ => Transition::Stay,
        }
    }

    fn draw(&mut self, ctx: &Ctx, canvas: &mut Canvas) {
        let a = &ctx.assets.shell;
        let ink = &ctx.assets.ink;
        let pen = Pen {
            assets: a,
            ink,
            chrome: ctx.assets.chrome.as_ref(),
            shadow: Some(font::SHADOW),
            caps: None,
        };
        let player = ctx.game.player;
        let realm = ctx.game.kingdom.realms.get(player as usize);

        pen.window(canvas, BOX.0, BOX.1, BOX.2, BOX.3, BOX.4);
        // The corner picture, then the caption that belongs to the widget.
        pen.ok_button(canvas, OK.x, OK.y, 0);
        pen.eng(canvas, GROUP, NOBLES, 0x88, 0x15C, font::TEXT);

        // "Court of" and the player's name after it, both in the 22-pixel font.
        let court_of = a.text(GROUP, COURT_OF).to_string();
        let w = pen.heading(canvas, HEADING_AT.0, HEADING_AT.1, &court_of, font::TEXT);
        // **The lord names are somebody else's.** The original draws
        // `g_playerNames + realm * 0x2C` here; nothing in this tree carries
        // them yet and `screens/battle.rs` has the same hole, so this is the
        // same stand-in it uses rather than a second invention.
        let name = format!("LORD {player}");
        // **`NAME_X - HEADING_AT.0`, not `NAME_X`.** The original is
        // `Ui_DrawText(name, g_penAdvance + 0x52, 0x44, heading)`, and
        // `g_penAdvance` is the width the label advanced — a *relative* number.
        // `Pen::heading` returns the **absolute** next x, which already has
        // `HEADING_AT.0` (`0x50`) in it, so adding `NAME_X` (`0x52`) put the
        // lord's name at `g_penAdvance + 0xA2`: **80 pixels right** of where the
        // game puts it. The two pixels the original nudges by are `0x52 - 0x50`.
        //
        // Third instance of one mistake in one afternoon — `Pen::count` and
        // `ratings.rs` had it too — and it is not carelessness: **every `Pen`
        // method returns an absolute x while every coordinate in the
        // decompilation beside `g_penAdvance` is relative**, so transcribing the
        // painter faithfully produces this bug. See the `Pen` docs; making it
        // unrepresentable wants a newtype and is `docs/decisions.md`
        // C110.
        pen.heading(canvas, w + (NAME_X - HEADING_AT.0), HEADING_AT.1, &name, font::TEXT);

        pen.inset(canvas, WELL);

        // Gold, then the three materials with their icons.
        let stores = [
            (GOLD, realm.map_or(0, |r| r.gold)),
            (IRON, realm.map_or(0, |r| r.iron)),
            (STONE, realm.map_or(0, |r| r.stone)),
            (WOOD, realm.map_or(0, |r| r.wood)),
        ];
        for (row, &(index, value)) in stores.iter().enumerate() {
            let y = STORE_Y[row];
            let label = a.text(GROUP, index).to_string();
            let w = pen.heading(canvas, STORE_LABEL_X, y, &label, font::TEXT);
            let _ = w;
            if row == 0 {
                // `Ui_DrawCount(gold, 0, …)` — the number and then group 8's
                // "Crown."/"Crowns.", which is why gold gets no icon.
                pen.count(canvas, STORE_VALUE_X, y, value, CROWN_NOUN, true, font::TEXT);
            } else {
                pen.number(canvas, STORE_VALUE_X, y, value, true, font::TEXT);
                pen.misc_frame(canvas, STORE_ICON_FRAME[row - 1], STORE_ICON_X, y);
            }
        }

        // The six weapon stocks, icon over number.
        for i in 0..l2_kingdom::tables::WEAPON_TYPE_COUNT {
            let x = i as i32 * WEAPON_PITCH;
            pen.misc_frame(canvas, WEAPON_FRAME0 + i, WEAPON_ICON_X0 + x, WEAPON_ICON_Y);
            let n = realm.map_or(0, |r| r.weapons[i]);
            // `Ui_DrawNumberRight` **centres** — `FUN_004025D7` is
            // `(width - textWidth) / 2`, the same helper `Ui_DrawCentred` uses.
            // `docs/symbols.json` calls it right-aligned and that is wrong for
            // every caller in the binary, not just this one.
            pen.number_centred(
                canvas,
                WEAPON_NUM_X0 + x,
                WEAPON_NUM_Y,
                WEAPON_NUM_W,
                n,
                font::TEXT,
            );
        }

        // The two footers: label, then value immediately after it.
        let wages = a.text(GROUP, WAGES).to_string();
        let w = pen.body(canvas, FOOTER_X, WAGES_Y, &wages, font::TEXT);
        pen.count(
            canvas,
            FOOTER_X + w,
            WAGES_Y,
            realm.map_or(0, |r| r.wages),
            CROWN_NOUN,
            false,
            font::TEXT,
        );

        let tax = a.text(GROUP, TAX_EXPECTED).to_string();
        let w = pen.body(canvas, FOOTER_X, TAX_Y, &tax, font::TEXT);
        pen.count(
            canvas,
            FOOTER_X + w,
            TAX_Y,
            tax_expected(ctx, player),
            CROWN_NOUN,
            false,
            font::TEXT,
        );

        // The widget the painter does not draw, and the note that its
        // destination is not built.
        pen.system_frame(canvas, NOBLES_FRAME, NOBLES_BUTTON.x, NOBLES_BUTTON.y);
        l2_view::text::draw(canvas, 4, 470, "0x20 THE STANDINGS IS NOT BUILT", ink.dim);
    }
}

//! **The court** — `Court_Draw` (`0x00416925`), `g_screenId` `0x09`.
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
//! *"Tax revenues expected,"* reads a realm word that is in neither
//! `docs/records.json`, `docs/kingdom.md` §2 nor `l2_kingdom::realm::Realm` —
//! all three stop before it. The whole corpus touches it **twice**: this
//! painter reads it, and `FUN_0044BA35` writes it, from the tail of
//! `Tax_RecomputePreview` (`0x0044B80B`):
//!
//! `Screen_DrawWidgets` and `Screen_HandleInput` each carry a `g_screenId == 9`
//! arm doing `Widget_Draw`/`Widget_Test(0, 0, &DAT_004DD928, DAT_00568468)`.
//!
//! ```text
//! x = 336  y = 340  frame = 64  size = 32  handler = FUN_004351C4  kind = 5
//! ```
//!
//! `FUN_004351C4` is `g_screenId = 0x20; Score_RankRealms(); …` — **it opens
//! the standings**, and *"Greatest nobles."* at (136, 348) is its label.
//!
//! # What the button does, in full — `FUN_004351C4` (`0x004351C4`)
//!
//! ```c
//! g_screenId = 0x20; g_redrawRequest = 1;
//! if (g_multiplayer == 0) FUN_00435211();        /* Score_RankAndRefreshAll */
//! else                    Net_SendCommand(0x3B, 0);
//! FUN_004B3994(DAT_0055CE7C);                   /* say the category's name */
//! ```
//!
//! **Three statements and the recount is the one that matters.** The page
//! reads five of its six score inputs out of the realm totals, and nothing
//! rebuilds them between AI turns, so without `FUN_00435211` the standings are
//! whatever the last AI turn left behind. Ours calls
//! [`crate::screens::nobles::recount`] on the same edge.
//!
//! The multiplayer arm sends network command `0x3B`, whose deferred action
//! `NetAct_RankRealms` (`0x00448422`) is the same recount on every peer. We
//! have no network game, and calling the single-player arm is what
//! [`crate::game::Game::multiplayer`] being false already means everywhere
//! else.
//!
//! `Sidebar_Button` (`0x0043AE30`), hotspot id 2, the rectangle **x 512…542,
//! y 430…458**, and it is **ungated** — unlike ids 1 and 3
//! ownership or turn test. `screens/index.rs` said it *"opens off a sidebar
//! button that needs a selected county"*; it does not.

use l2_view::Canvas;

use crate::input::{Event, Key, Rect};
use crate::press::{Press, Widget};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::shell::{font, Pen};

/// `L2.eng` group 70.
pub const GROUP: usize = 70;

pub const GOLD: usize = 0;
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

pub const WELL: Rect = Rect::new(0x50, 0x68, 0x160, 0xE0);

pub const OK: Rect = Rect::new(0x198, 0x158, 24, 24);

pub const HEADING_AT: (i32, i32) = (0x50, 0x44);
pub const NAME_X: i32 = 0x52;

pub const STORE_LABEL_X: i32 = 0x60;
pub const STORE_VALUE_X: i32 = 0xE0;
pub const STORE_ICON_X: i32 = 0x130;
pub const STORE_Y: [i32; 4] = [0x72, 0x90, 0xAE, 0xCC];
pub const STORE_ICON_FRAME: [usize; 3] = [0x2C, 0x2D, 0x2E];

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

pub const FOOTER_X: i32 = 100;
pub const WAGES_Y: i32 = 0x124;
pub const TAX_Y: i32 = 0x136;

/// `g_courtWidgets` (`0x004DD928`) — one record, the *Greatest nobles* button.
pub const NOBLES_BUTTON: Rect = Rect::new(336, 340, 32, 32);
pub const NOBLES_FRAME: usize = 64;

pub const DEFERRED_FRAMES: u8 = 0x14;

pub(super) fn widgets() -> [Widget; 1] {
    [Widget::new(NOBLES_BUTTON, crate::arm!("0x004351C4/court-greatest-nobles", Delayed))]
}

/// **`FUN_0044BA35`** — the empire-wide sum of `County::tax_shown`, which the
/// original caches in realm `+0x15C` and we recompute.
pub fn tax_expected(ctx: &Ctx, realm: u8) -> i32 {
    ctx.game.kingdom.tax_expected(realm)
}

pub struct CourtScreen {
    press: Press,
}

impl CourtScreen {
    pub fn new() -> CourtScreen {
        CourtScreen { press: Press::new() }
    }

    /// **`FUN_004351C4`** — the handler, twenty frames after the press.
    fn open_the_standings(&mut self, ctx: &mut Ctx) -> Transition {
        // `if (g_multiplayer == 0) FUN_00435211(); else Net_SendCommand(0x3B, 0);`
        crate::screens::nobles::recount(ctx.game);
        // `FUN_004B3994(DAT_0055CE7C)` — the category speaks its own name on
        // the way in, whether or not it changed. See
        // [`crate::game::Game::nobles_spoken`].
        ctx.game.nobles_spoken = ctx.game.nobles_spoken.wrapping_add(1);
        Transition::Push(ScreenId::Nobles)
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

    fn update(&mut self, ctx: &mut Ctx) -> Transition {
        if self.press.tick().next().is_some() {
            return self.open_the_standings(ctx);
        }
        Transition::Stay
    }

    fn take_clicks(&mut self) -> u8 {
        self.press.take_clicks()
    }

    fn take_redraw(&mut self) -> bool {
        self.press.take_redraw()
    }

    fn handle(&mut self, event: Event, _ctx: &mut Ctx) -> Transition {
        match event {
            // arm: 0x0042FF10/minimap-under-the-court left-press
            Event::Click { x, y } if l2_view::chrome::minimap_hit_area().contains(x, y) => {
                Transition::Pass
            }
            // `Widget_Test(0, 0, &g_courtWidgets, 1)` — the *Greatest nobles*
            // button, `FUN_004351C4`, which is `g_screenId = 0x20`.
            Event::Click { x, y } if NOBLES_BUTTON.contains(x, y) => {
                let fired = self.press.event(&widgets(), event);
                debug_assert!(fired.is_none(), "the court's one widget is kind 5");
                Transition::Stay
            }
            // arm: 0x0042FF10/court-ok left-release
            Event::Click { x, y } if OK.contains(x, y) => Transition::Pop,
            // arm: 0x0042FF10/court-right right-release
            Event::RightClick { .. } => Transition::Pop,
            // arm: ours/court-keyboard-close key
            Event::KeyDown(Key::Escape) | Event::KeyDown(Key::Enter) => Transition::Pop,
            Event::DoubleClick { .. }
            | Event::Release { .. }
            | Event::Pointer { .. }
            | Event::PointerLeft => {
                let fired = self.press.event(&widgets(), event);
                debug_assert!(fired.is_none(), "the court's one widget is kind 5");
                Transition::Stay
            }
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
        pen.ok_button(canvas, OK.x, OK.y, 0);
        pen.eng(canvas, GROUP, NOBLES, 0x88, 0x15C, font::TEXT);

        let court_of = a.text(GROUP, COURT_OF).to_string();
        let w = pen.heading(canvas, HEADING_AT.0, HEADING_AT.1, &court_of, font::TEXT);
        // `Ui_DrawText(&g_playerNames + realm * 0x2C, …)`. `Player_SetHuman`
        // (`0x0049BAE9`) copies the typed name in for the person and
        // `Eng_Seek(7, realm.lord)` names the AIs, which is exactly
        // [`super::message::lord_name`]'s pair of sources.
        let name = super::message::lord_name(ctx, player);
        // Third instance of one mistake in one afternoon — `Pen::count` and
        // `ratings.rs` had it too — and it is not carelessness: **every `Pen`
        // method returns an absolute x while every coordinate in the
        // decompilation beside `g_penAdvance` is relative**, so transcribing the
        // painter faithfully produces this bug. See the `Pen` docs; making it
        // unrepresentable wants a newtype and is `docs/decisions.md`
        // C110.
        pen.heading(canvas, w + (NAME_X - HEADING_AT.0), HEADING_AT.1, &name, font::TEXT);

        pen.inset(canvas, WELL);

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
            // `Court_Draw` (`0x00416925`) passes `&g_fontHeading` to every one:
            //
            // Their suffixes are `&DAT_004D3F8C`, `…90` and `…94`, each one
            // space, read out of the shipped exe. **[V]**
            let face = crate::shell::Face::Heading;
            if row == 0 {
                pen.count_in(face, canvas, STORE_VALUE_X, y, value, CROWN_NOUN, font::TEXT);
            } else {
                pen.number_in(face, canvas, STORE_VALUE_X, y, value, ' ', " ", font::TEXT);
                pen.misc_frame(canvas, STORE_ICON_FRAME[row - 1], STORE_ICON_X, y);
            }
        }

        for i in 0..l2_kingdom::tables::WEAPON_TYPE_COUNT {
            let x = i as i32 * WEAPON_PITCH;
            pen.misc_frame(canvas, WEAPON_FRAME0 + i, WEAPON_ICON_X0 + x, WEAPON_ICON_Y);
            let n = realm.map_or(0, |r| r.weapons[i]);
            // `Ui_DrawNumberRight` **centres** — `FUN_004025D7` is
            // `(width - textWidth) / 2`, the same helper `Ui_DrawCentred` uses.
            //
            // The lead is `' '` and the suffix is `&DAT_004D3F98`, which holds
            // one space — checked in the image
            // five sites on `Panel_Ration` pass an *empty* suffix and the
            // difference is two pixels of centring.
            //
            // `docs/decisions.md` C140. **[V]**
            pen.number_centred(
                canvas,
                WEAPON_NUM_X0 + x,
                WEAPON_NUM_Y,
                WEAPON_NUM_W,
                n,
                ' ',
                " ",
                font::TEXT,
            );
        }

        let wages = a.text(GROUP, WAGES).to_string();
        let w = pen.body(canvas, FOOTER_X, WAGES_Y, &wages, font::TEXT);
        pen.count(
            canvas,
            FOOTER_X + w,
            WAGES_Y,
            realm.map_or(0, |r| r.wages),
            CROWN_NOUN,
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
            font::TEXT,
        );

        let frame = if self.press.is_pressed(0) { NOBLES_FRAME + 1 } else { NOBLES_FRAME };
        pen.system_frame(canvas, frame, NOBLES_BUTTON.x, NOBLES_BUTTON.y);
    }
}

//! **The merchant** — `Screen_Merchant` (`0x00415FB7`), `g_screenId` `0x08`,
//! and the trade panel `Screen_TradeGoods` (`0x00416308`) / `Trade_DrawPanel`
//! (`0x0041635F`), `g_screenId` `0x0C`, both on `L2.eng` group 68.
//!
//! Two screens, because the original has two screen ids and the panel goes back
//! to the stall rather than replacing it. [`MerchantScreen`] is the stall you
//! pick a good on; [`TradeScreen`] is the one you agree a quantity on. The
//! rules behind both are [`l2_kingdom::trade`], which knows nothing about
//! either.
//!
//! # The stall
//!
//! ```text
//! File_ReadChunk("merchant.256", …)   Palette_Set
//! FUN_00408FCB("merchant.pl8", 0x1E0)                 the picture
//! File_ReadChunk("mercgrid.pl8", &g_villageGrid, 0x12D8)   the hit map
//! Ui_OkButton(g_screenStride - 0x1C, g_screenHeight - 0x1C, 1)
//! ```
//!
//! **`mercgrid.pl8` is not drawn. It is the hit test**, and that is the whole
//! design of this screen. `Merchant_StallHit` (`0x004357A6`) reads it as an
//! 80-wide byte map at eight pixels a cell —
//!
//! ```c
//! good = grid[(mouseX >> 3) + (mouseY >> 3) * 0x50];
//! if (good == 0) return 0;                 /* not on any ware */
//! if (!g_mouseLeftPressed) return 0;
//! FUN_0043530E();                          /* -> screen 0x0C on that good */
//! ```
//!
//! — so every ware painted on the stall carries its own good id in a parallel
//! image, and the picture and the click map are the same artwork twice. There
//! are no rectangles anywhere. Ours reads the same file the same way when it is
//! installed, and falls back to the plaque rectangles below when it is not.
//!
//! **The stall draws no text of its own, and the hover plaque is the whole of
//! its interface.** `Merchant_HoverPlaque` (`0x0041608B`): for the good under
//! the pointer it draws a 128 x 64 box at that good's own position out of a
//! fourteen-row table at `0x004D2B70`, the good's name centred in it, and
//! **both prices side by side** — the manual's *"30/60"*. [`STALL`] is that
//! table, read out of the executable rather than measured.
//!
//! This is the *"mouseover tooltip the original showed"* a player reported
//! missing, and it is **not** generic hover chrome: it is one function, on one
//! screen, with a throttle of its own. The shell that stood here drew `L2.eng`
//! 68 index 0 as a standing caption at (0x88, 0x68) and that was invented
//! twice over — `Screen_Merchant` calls `Eng_DrawString` **not once**, and
//! index 0 of a group is the *group's own label* rather than a drawn string
//! (`docs/formats/eng.md` §5). The position was the trade panel's heading
//! borrowed. Both are gone: the stall is the picture and the plaque.
//!
//! `DAT_0053EF54` is the plaque's throttle — set to 500 on entry and again
//! after every swap, decremented once per widget pass, and the swap only
//! happens at zero. Ours updates on the pointer event instead, because our
//! widget pass is not the original's frame and a count of ours would mean a
//! different length of time; the throttle is recorded rather than reproduced.
//!
//! **This screen's artwork says sheep and wool are not in the game, twice.**
//! [`STALL`] puts both at **(0, 0)** — no place for a plaque — and the shipped
//! `mercgrid.pl8` holds exactly twelve ids, with **neither 3 nor 5 in any of
//! its 4,800 cells**: there is nowhere on the stall to click for either. With
//! the missing `Merchant_Trade` branch and the price of zero that is four
//! independent sources, and these are the two made by the pictures rather than
//! by the code or the strings. Both goods are carried anyway, for the reason
//! `l2_kingdom::trade` carries them: the game demonstrating its own dead end
//! beats us deciding in advance that it does not exist.
//!
//! # The panel
//!
//! ```text
//! FUN_004093E0(0x30, 0x40, 0x22, 0x10)               the window, 544 x 256 at (48, 64)
//! Ui_OkButton(0x22C, 0x114, 0)
//! Ui_DrawBevelRect(0x44, 0x66, 0x32, 0x32)           the commodity icon's well
//! FUN_0040A682(good - 1, 0x45, 0x67)                 icontrad.pl8, frame good-1
//! 68/2 "Trading" | 68/6 "Selling" | 68/5 "Buying"  + the group 6 name  at (0x88, 0x68)
//! 68/9 "Buying price" + buy | 68/10 "Selling price" + sell             at (0x88, 0x88)
//! 68/12 "You have." + gold [+ 68/11 "and" + stock + icon] + 68/13 "my Lord."  at (0x58, 0xB0)
//! Ui_DrawInsetRect(0x50, 0xD0, 0x1D0, 0x4C)          the advice well
//!   qty == 0:  one wrapped paragraph of 68/16 … 68/19 at (0x60, 0xF6), width 0x1C0
//!   qty != 0:  the quantity at (0x60, 0xE0), 68/1 "We receive" or 68/8 "Total
//!              cost of" + crowns at (0x100, 0xE0), and 68/7 "Complete this" +
//!              68/14 "Purchase?" / 68/15 "Sale?" at (0x60, 0x100)
//! ```
//!
//! Six widgets at `0x004DD838`, drawn at an offset of (0x30, 0x50) — and **only
//! four of them exist until a quantity is chosen**: `Widget_Draw` is given a
//! count of 4 with nothing pending and 6 with something, so the tick and the
//! cross appear when there is something to agree to. That is `DAT_00553F58`,
//! and it is the panel's whole modality.
//!
//! # The three strings that are the rules stated in English
//!
//! The advice paragraph is chosen by a flag the arrows set, and each of its
//! four values is one of the model's own limits:
//!
//! * **68/18** *"You do not have enough crowns to buy, my Lord."* — the up
//!   arrow clamped **and the ceiling was zero**. The original's test is
//!   `if (maxQty < qty && (qty = maxQty, maxQty == 0))`, so it is not "you hit
//!   the limit", it is "you cannot afford even one".
//! * **68/16** *"You have no goods to sell, my Lord."* — the down arrow, with
//!   the floor at zero, the same way.
//! * **68/19** *"Buy ale for your county, as a gift for its people."* — the
//!   good is ale.
//! * **68/17** *"Use the up and down arrows to buy and sell goods."* —
//!   otherwise.
//!
//! # What is ours, and marked
//!
//! The arrows, the two limit buttons, the tick and the cross are the original's
//! six widgets at the original's six positions, drawn as our own buttons
//! because we do not have its button sheet frames 21, 23, 70, 72, 29 and 31
//! decoded as a set. **Keyboard** — the arrow keys, `M` and `S` for the two
//! limits, Enter and Escape — is entirely ours and says so on the screen: C21.

use l2_kingdom::trade::{self, Good, Order, Quote, Refusal};
use l2_view::{text, Canvas};

use crate::input::{Event, Key, Rect};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::shell::{self, font, Pen};
use crate::widget;

/// `L2.eng` group 68 — the merchant's own words, on both screens.
pub const GROUP: usize = 68;
/// `L2.eng` group 6 — the fourteen goods' names.
pub const GROUP_GOODS: usize = 6;

/// One row of the fourteen-row table at `0x004D2B70`, five ints apiece.
///
/// Read out of `Lords2.exe` rather than measured off a screenshot. The third
/// column is the good's own id, which is what makes the reading self-checking:
/// row `i` holds `i + 1` in it, fourteen for fourteen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StallRow {
    /// `+0x00`, `+0x04` — where the price plaque is drawn on `Merchant.pl8`.
    /// **(0, 0) for sheep and wool**, which have no place on the stall.
    pub x: i32,
    pub y: i32,
    /// `+0x08` — the good's own id, 1 … 14.
    pub good: usize,
    /// `+0x0C` — 0, 1 or 2. **Nothing in the binary reads it.** It is 0 for
    /// exactly sheep, ale and wool — the three goods with no `You have` line —
    /// and 1 or 2 for the rest with no pattern this project has explained.
    /// Carried rather than dropped, and marked `[I]` for whoever finds the
    /// reader.
    pub unread: i32,
    /// `+0x10` — the icon frame for the *"and N …"* clause on the trade panel,
    /// and **0 means the clause is not drawn at all**. Zero for sheep, ale and
    /// wool: you never hold any of the three.
    pub icon: usize,
}

/// `0x004D2B70`, the whole table.
pub const STALL: [StallRow; 14] = [
    StallRow { x: 100, y: 230, good: 1, unread: 1, icon: 14 },
    StallRow { x: 116, y: 114, good: 2, unread: 1, icon: 15 },
    StallRow { x: 0, y: 0, good: 3, unread: 0, icon: 0 },
    StallRow { x: 140, y: 170, good: 4, unread: 0, icon: 0 },
    StallRow { x: 0, y: 0, good: 5, unread: 0, icon: 0 },
    StallRow { x: 10, y: 340, good: 6, unread: 1, icon: 16 },
    StallRow { x: 160, y: 210, good: 7, unread: 2, icon: 17 },
    StallRow { x: 310, y: 100, good: 8, unread: 1, icon: 18 },
    StallRow { x: 440, y: 42, good: 9, unread: 1, icon: 22 },
    StallRow { x: 130, y: 356, good: 10, unread: 1, icon: 23 },
    StallRow { x: 180, y: 270, good: 11, unread: 2, icon: 20 },
    StallRow { x: 360, y: 290, good: 12, unread: 2, icon: 19 },
    StallRow { x: 240, y: 330, good: 13, unread: 1, icon: 21 },
    StallRow { x: 400, y: 350, good: 14, unread: 2, icon: 24 },
];

pub fn stall_row(good: Good) -> StallRow {
    STALL[good.id() - 1]
}

/// `FUN_004093E0(x, y, 8, 4)` — the plaque is eight cells by four, and a cell
/// is sixteen pixels.
pub const PLAQUE_W: i32 = 8 * 16;
pub const PLAQUE_H: i32 = 4 * 16;

/// Where a good can be clicked on the stall.
///
/// **This is our fallback, not the original's hit test.** The original reads
/// `mercgrid.pl8` — see the module docs — and so do we when the file is there;
/// this rectangle is what a machine with no game installed clicks instead, and
/// it is the plaque's own box so that what is drawn is what is clickable.
/// A good at (0, 0) — sheep and wool — gets an empty rectangle, so the fallback
/// cannot reach them either.
pub fn plaque(good: Good) -> Rect {
    let r = stall_row(good);
    if r.x == 0 && r.y == 0 {
        Rect::new(0, 0, 0, 0)
    } else {
        Rect::new(r.x, r.y, PLAQUE_W, PLAQUE_H)
    }
}

/// `Ui_OkButton(g_screenStride - 0x1C, g_screenHeight - 0x1C, 1)`.
pub const STALL_OK: Rect = Rect::new(640 - 0x1C, 480 - 0x1C, 24, 24);

// ---------------------------------------------------------------------------
// the trade panel's geometry, all of it out of `Trade_DrawPanel`
// ---------------------------------------------------------------------------

/// `FUN_004093E0(0x30, 0x40, 0x22, 0x10)`.
pub const PANEL: Rect = Rect::new(0x30, 0x40, 0x22 * 16, 0x10 * 16);
/// `Ui_OkButton(0x22C, 0x114, 0)`.
pub const PANEL_OK: Rect = Rect::new(0x22C, 0x114, 24, 24);
/// `Ui_DrawBevelRect(0x44, 0x66, 0x32, 0x32)` — the commodity icon's well.
pub const ICON_WELL: Rect = Rect::new(0x44, 0x66, 0x32, 0x32);
/// `Ui_DrawInsetRect(0x50, 0xD0, 0x1D0, 0x4C)` — the advice well.
pub const ADVICE_WELL: Rect = Rect::new(0x50, 0xD0, 0x1D0, 0x4C);
/// The wrapped paragraph's origin and width: `(0x60, 0xF6)`, `0x1C0`.
pub const ADVICE_X: i32 = 0x60;
pub const ADVICE_Y: i32 = 0xF6;
pub const ADVICE_W: i32 = 0x1C0;

/// `Widget_Draw(0x30, 0x50, &DAT_004DD838, …)` — every widget's stored position
/// is relative to this.
pub const WIDGET_ORIGIN: (i32, i32) = (0x30, 0x50);

/// The six widgets of `0x004DD838`, in table order, at their stored positions.
/// `size` is the record's `+6`, which is the square button's side.
const WIDGETS: [(i32, i32, i32); 6] = [
    (96, 136, 24),  // frame 21, up      -> FUN_00435339
    (120, 136, 24), // frame 23, down    -> FUN_0043543D
    (152, 136, 24), // frame 72, ceiling -> FUN_004355DB
    (176, 136, 24), // frame 70, floor   -> FUN_00435541
    (272, 168, 32), // frame 29, tick    -> FUN_00435286
    (312, 168, 32), // frame 31, cross   -> FUN_004352F2
];

fn widget_rect(i: usize) -> Rect {
    let (x, y, s) = WIDGETS[i];
    Rect::new(WIDGET_ORIGIN.0 + x, WIDGET_ORIGIN.1 + y, s, s)
}

pub fn up_button() -> Rect {
    widget_rect(0)
}
pub fn down_button() -> Rect {
    widget_rect(1)
}
pub fn ceiling_button() -> Rect {
    widget_rect(2)
}
pub fn floor_button() -> Rect {
    widget_rect(3)
}
pub fn confirm_button() -> Rect {
    widget_rect(4)
}
pub fn cancel_button() -> Rect {
    widget_rect(5)
}

/// Which of the four paragraphs the advice well shows.
///
/// The `Limit` cases are **not** "you reached the limit": the original sets the
/// flag only when the limit it clamped to was itself zero.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Advice {
    /// 68/18 — the ceiling is zero.
    CannotAfford,
    /// 68/16 — the floor is zero.
    NothingToSell,
    /// 68/19 — the good is ale.
    Ale,
    /// 68/17 — the arrows work; use them.
    UseTheArrows,
}

impl Advice {
    pub fn index(self) -> usize {
        match self {
            Advice::NothingToSell => 16,
            Advice::UseTheArrows => 17,
            Advice::CannotAfford => 18,
            Advice::Ale => 19,
        }
    }

    /// The fallback wording, for a machine with no `L2.eng`. Ours, and only
    /// reached when the game's own words are not installed.
    pub fn fallback(self) -> &'static str {
        match self {
            Advice::NothingToSell => "YOU HAVE NO GOODS TO SELL, MY LORD.",
            Advice::UseTheArrows => "USE THE UP AND DOWN ARROWS TO BUY AND SELL GOODS.",
            Advice::CannotAfford => "YOU DO NOT HAVE ENOUGH CROWNS TO BUY, MY LORD.",
            Advice::Ale => "BUY ALE FOR YOUR COUNTY, AS A GIFT FOR ITS PEOPLE.",
        }
    }
}

// ---------------------------------------------------------------------------
// the stall
// ---------------------------------------------------------------------------

/// `g_screenId` `0x08`. Owns the merchant being traded with — `DAT_00553C64`,
/// which the map click writes and which only this screen and its panel read.
pub struct MerchantScreen {
    /// The merchant unit. `DAT_00553C64`.
    unit: usize,
    /// The good under the pointer, 0 for none. `DAT_0056D8AC`.
    hover: usize,
    /// Ours: what the last trade did, so a completed trade says so.
    status: String,
}

impl MerchantScreen {
    pub fn new(unit: usize) -> MerchantScreen {
        MerchantScreen { unit, hover: 0, status: String::new() }
    }

    /// The merchant's morale, which is the only input to the markup.
    /// **100 for every merchant the shipped game creates**, and 0 for a unit
    /// that has gone — a merchant that walked away between the click and the
    /// draw prices everything at its floor of one crown rather than panicking.
    fn morale(&self, ctx: &Ctx) -> i32 {
        ctx.game.kingdom.campaign.units.get(self.unit).map_or(0, |u| u.morale)
    }

    /// The good the pointer is over, through `mercgrid.pl8` when it is loaded
    /// and through the plaque rectangles when it is not.
    fn pick(&self, ctx: &Ctx, x: i32, y: i32) -> Option<Good> {
        if let Some(id) = ctx.assets.shell.merchant_grid(x, y) {
            return Good::from_id(id as usize);
        }
        trade::ALL_GOODS.iter().copied().find(|&g| plaque(g).contains(x, y))
    }
}

impl Screen for MerchantScreen {
    fn id(&self) -> ScreenId {
        ScreenId::Merchant(self.unit)
    }

    fn title(&self, _ctx: &Ctx) -> String {
        "The merchant".to_string()
    }

    fn palette(&self) -> Option<&'static str> {
        Some("Merchant.256")
    }

    /// **Only two things close this screen**, and a click in the body is not
    /// one of them: `Ui_OkButtonClicked` (`0x0040E7E4`) hit-tests a 24 x 24 box
    /// at the corner on a *left release*, and `Screen_FrameInput` takes a right
    /// release as a dismissal. A player reported that our shell closed on any
    /// click anywhere, which is what a shell does and is why this is written
    /// down rather than left to the reader of the match arms.
    ///
    /// Escape is **ours**, and the screen says so in its own font.
    fn handle(&mut self, event: Event, ctx: &mut Ctx) -> Transition {
        match event {
            Event::KeyDown(Key::Escape) | Event::RightClick { .. } => Transition::Pop,
            Event::Pointer { x, y } => {
                self.hover = self.pick(&Ctx { game: ctx.game, assets: ctx.assets }, x, y)
                    .map_or(0, |g| g.id());
                Transition::Stay
            }
            Event::Click { x, y } => {
                if STALL_OK.contains(x, y) {
                    return Transition::Pop;
                }
                let read = Ctx { game: ctx.game, assets: ctx.assets };
                match self.pick(&read, x, y) {
                    // `FUN_004357A6` -> `FUN_0043530E`: the click on a ware is
                    // the whole of the navigation, and it carries the good.
                    Some(good) => {
                        self.hover = good.id();
                        Transition::Push(ScreenId::Trade(self.unit, good.id() as u8))
                    }
                    None => Transition::Stay,
                }
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
        if !shell::background(canvas, a, "Merchant.pl8") {
            canvas.clear(ink.background);
        }
        // **No caption.** `Screen_Merchant` draws no string at all; see the
        // module docs for what used to be here and why it was invented.

        // The hover plaque: the good's own box, its name, and both prices.
        if let Some(good) = Good::from_id(self.hover) {
            let r = plaque(good);
            if r.w > 0 {
                let q = trade::quote(&ctx.game.kingdom.tables, good, self.morale(ctx));
                widget::panel(canvas, ink, r);
                let name = {
                    let s = a.text(GROUP_GOODS, good.id());
                    if s.is_empty() { good.name() } else { s }
                };
                pen.body_centred(canvas, r.x, r.y + 0x10, PLAQUE_W, name, font::TEXT);
                // `Ui_DrawNumber(sell)` then the buy price after it, at
                // (x + 0x24, y + 0x22) — the manual's "30/60".
                text::draw(
                    canvas,
                    r.x + 0x24,
                    r.y + 0x22,
                    &format!("{}/{}", q.sell, q.buy),
                    ink.highlight,
                );
            }
        }

        // `Ui_OkButton(…, 1)`.
        let drawn = ctx.assets.chrome.as_ref().is_some_and(|c| {
            c.draw_system(canvas, l2_view::chrome::system::OK_ALT, STALL_OK.x, STALL_OK.y)
        });
        if !drawn {
            shell::button_recess(canvas, STALL_OK.x, STALL_OK.y, 24, 24);
        }

        // ---- ours ---------------------------------------------------------
        if !self.status.is_empty() {
            text::draw(canvas, 4, 458, &self.status, ink.text);
        }
        text::draw(
            canvas,
            4,
            470,
            "HOVER A WARE FOR ITS PRICES, CLICK IT TO TRADE - RIGHT-CLICK OR ESC LEAVES",
            ink.dim,
        );
    }
}

// ---------------------------------------------------------------------------
// the trade panel
// ---------------------------------------------------------------------------

/// `g_screenId` `0x0C`. One good, one quantity, one agreement.
pub struct TradeScreen {
    unit: usize,
    good: Good,
    /// `DAT_00554170` — signed: positive buys, negative sells.
    qty: i32,
    /// `DAT_0057C994` — 1 when the ceiling refused, -1 when the floor did, and
    /// only when the limit was itself zero.
    limit: i32,
    /// Ours: what happened, in our own words, under the window.
    status: String,
}

impl TradeScreen {
    pub fn new(unit: usize, good: u8) -> TradeScreen {
        TradeScreen {
            unit,
            good: Good::from_id(good as usize).unwrap_or(Good::Grain),
            qty: 0,
            limit: 0,
            status: String::new(),
        }
    }

    fn morale(&self, ctx: &Ctx) -> i32 {
        ctx.game.kingdom.campaign.units.get(self.unit).map_or(0, |u| u.morale)
    }

    fn quote(&self, ctx: &Ctx) -> Quote {
        trade::quote(&ctx.game.kingdom.tables, self.good, self.morale(ctx))
    }

    /// The county the merchant stands in — `g_selectedCounty`, which the map
    /// click set to the merchant's own county on the way here.
    fn county(&self, ctx: &Ctx) -> usize {
        ctx.game.selected as usize
    }

    /// `FUN_00428DAF`'s two clamps, recomputed on every frame the way the
    /// original recomputes them on every entry to this screen.
    fn limits(&self, ctx: &Ctx) -> (i32, i32) {
        let k = &ctx.game.kingdom;
        let realm = ctx.game.player as usize;
        let floor = trade::min_qty(&k.counties, &k.realms, self.good, realm, self.county(ctx));
        let ceiling = trade::max_buy(k.realms[realm].gold, self.quote(ctx).buy);
        (floor, ceiling)
    }

    /// `FUN_00435339` / `FUN_0043543D` — step, then clamp, and set the flag
    /// only when the limit clamped to was zero.
    fn step(&mut self, ctx: &Ctx, by: i32) {
        let (floor, ceiling) = self.limits(ctx);
        self.limit = 0;
        self.qty += by;
        if self.qty > ceiling {
            self.qty = ceiling;
            if ceiling == 0 {
                self.limit = 1;
            }
        }
        if self.qty < floor {
            self.qty = floor;
            if floor == 0 {
                self.limit = -1;
            }
        }
    }

    /// `FUN_004355DB` and `FUN_00435541` — the two buttons that go straight to
    /// a limit. Neither sets the flag.
    fn jump_to_limit(&mut self, ctx: &Ctx, ceiling: bool) {
        let (floor, top) = self.limits(ctx);
        self.limit = 0;
        self.qty = if ceiling { top } else { floor };
    }

    /// What the panel is about to charge or pay — `g_tradeCrowns`.
    fn crowns(&self, ctx: &Ctx) -> i32 {
        let q = self.quote(ctx);
        if self.qty < 0 {
            -self.qty * q.sell
        } else {
            self.qty * q.buy
        }
    }

    /// `Ale_PreviewGain` (`0x00435673`) — the same ladder as
    /// [`l2_kingdom::happiness::buy_ale`], run on the pending crown total so
    /// the panel can show the gain before the purchase.
    ///
    /// **Duplicated code in the original, not a shared helper** — a mod that
    /// changes one must change both. Ours calls the rule, so it cannot drift;
    /// that is a deliberate difference and it is the safe direction.
    fn ale_preview(&self, ctx: &Ctx) -> i32 {
        if self.good != Good::Ale || self.qty <= 0 {
            return 0;
        }
        let k = &ctx.game.kingdom;
        let mut county = k.counties[self.county(ctx)].clone();
        l2_kingdom::happiness::buy_ale(&k.tables, &mut county, self.crowns(ctx), k.options.quirks)
    }

    /// Which paragraph the advice well shows.
    fn advice(&self) -> Advice {
        if self.limit == 1 {
            Advice::CannotAfford
        } else if self.limit == -1 {
            Advice::NothingToSell
        } else if self.good == Good::Ale {
            Advice::Ale
        } else {
            Advice::UseTheArrows
        }
    }

    /// `FUN_00435286` — the tick. `Merchant_Trade`, then back to the stall.
    fn confirm(&mut self, ctx: &mut Ctx) -> Transition {
        let q = self.quote(&Ctx { game: ctx.game, assets: ctx.assets });
        let county = self.county(&Ctx { game: ctx.game, assets: ctx.assets });
        let order = Order {
            qty: self.qty,
            good: self.good,
            buy_price: q.buy,
            sell_price: q.sell,
            realm: ctx.game.player as usize,
            county,
        };
        match l2_kingdom::trade::trade(&mut ctx.game.kingdom, order) {
            Ok(r) if self.qty == 0 => {
                let _ = r;
                Transition::Pop
            }
            Ok(r) => {
                self.status = if r.ale_happiness > 0 {
                    format!("{} CROWNS OF ALE, +{} HAPPINESS", r.crowns, r.ale_happiness)
                } else if self.qty < 0 {
                    format!("SOLD {} FOR {} CROWNS", -self.qty, r.crowns)
                } else {
                    format!("BOUGHT {} FOR {} CROWNS", self.qty, r.crowns)
                };
                Transition::Pop
            }
            // The original refuses silently and returns to `0x08` anyway. Ours
            // stays and says which of the two guards fired, because a silent
            // refusal is a screen a player thinks is broken.
            Err(Refusal::NotEnoughGold) => {
                self.limit = 1;
                self.status = "THE TREASURY WILL NOT COVER IT".into();
                Transition::Stay
            }
            Err(Refusal::NotEnoughStock) => {
                self.limit = -1;
                self.status = "YOU DO NOT HOLD THAT MUCH".into();
                Transition::Stay
            }
        }
    }
}

impl Screen for TradeScreen {
    fn id(&self) -> ScreenId {
        ScreenId::Trade(self.unit, self.good.id() as u8)
    }

    fn title(&self, _ctx: &Ctx) -> String {
        format!("Trading {}", self.good.name())
    }

    /// `Screen_TradeGoods` re-loads `merchant.pl8` as its own backdrop, so it is
    /// a page rather than an inset — and it runs under the merchant's palette.
    fn palette(&self) -> Option<&'static str> {
        Some("Merchant.256")
    }

    fn handle(&mut self, event: Event, ctx: &mut Ctx) -> Transition {
        let read = Ctx { game: ctx.game, assets: ctx.assets };
        match event {
            Event::KeyDown(Key::Escape) | Event::RightClick { .. } => Transition::Pop,
            Event::KeyDown(Key::Enter) => self.confirm(ctx),
            Event::KeyDown(Key::Up) => {
                self.step(&read, 1);
                Transition::Stay
            }
            Event::KeyDown(Key::Down) => {
                self.step(&read, -1);
                Transition::Stay
            }
            Event::KeyDown(Key::Right) => {
                self.step(&read, 10);
                Transition::Stay
            }
            Event::KeyDown(Key::Left) => {
                self.step(&read, -10);
                Transition::Stay
            }
            Event::KeyDown(Key::Char('M')) => {
                self.jump_to_limit(&read, true);
                Transition::Stay
            }
            Event::KeyDown(Key::Char('S')) => {
                self.jump_to_limit(&read, false);
                Transition::Stay
            }
            Event::Click { x, y } => {
                if up_button().contains(x, y) {
                    self.step(&read, 1);
                } else if down_button().contains(x, y) {
                    self.step(&read, -1);
                } else if ceiling_button().contains(x, y) {
                    self.jump_to_limit(&read, true);
                } else if floor_button().contains(x, y) {
                    self.jump_to_limit(&read, false);
                } else if self.qty != 0 && confirm_button().contains(x, y) {
                    return self.confirm(ctx);
                } else if (self.qty != 0 && cancel_button().contains(x, y))
                    || PANEL_OK.contains(x, y)
                {
                    return Transition::Pop;
                }
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
        if !shell::background(canvas, a, "Merchant.pl8") {
            canvas.clear(ink.background);
        }
        pen.window(canvas, PANEL.x, PANEL.y, 0x22, 0x10, 1);

        // The commodity icon's well. `icontrad.pl8` frame `good - 1` goes in it;
        // that sheet is not one we load, so the well holds the good's name.
        canvas.fill_rect(ICON_WELL.x, ICON_WELL.y, ICON_WELL.w, ICON_WELL.h, ink.background);
        widget::frame(canvas, ICON_WELL, ink.border);

        let q = self.quote(ctx);
        let (floor, ceiling) = self.limits(ctx);
        let name = {
            let s = a.text(GROUP_GOODS, self.good.id());
            if s.is_empty() { self.good.name() } else { s }
        };

        // The heading: 68/2 "Trading" with nothing pending, 68/6 "Selling" or
        // 68/5 "Buying" with something, then the good's name after it.
        let head = if self.qty == 0 {
            2
        } else if self.qty < 0 {
            6
        } else {
            5
        };
        let mut x = 0x88;
        {
            let s = a.text(GROUP, head).to_string();
            let s = if s.is_empty() {
                ["", "", "TRADING", "", "", "BUYING", "SELLING"][head].to_string()
            } else {
                s
            };
            x = pen.heading(canvas, x, 0x68, &format!("{s} "), font::TEXT);
        }
        pen.heading(canvas, x, 0x68, name, font::TEXT);

        // The ale preview, beside the heading, when there is one.
        let ale = self.ale_preview(ctx);
        if ale > 0 {
            text::draw(canvas, 0x88, 0x78, &format!("+{ale} HAPPINESS"), ink.good);
        }

        // The price line: whichever of the two prices this direction uses.
        if self.qty != 0 {
            let (idx, price, fallback) = if self.qty < 1 {
                (10, q.sell, "SELLING PRICE")
            } else {
                (9, q.buy, "BUYING PRICE")
            };
            let label = {
                let s = a.text(GROUP, idx).to_string();
                if s.is_empty() { fallback.to_string() } else { s }
            };
            let x = pen.body(canvas, 0x88, 0x88, &format!("{label} "), font::TEXT);
            pen.body(canvas, x, 0x88, &format!("{price}"), font::TEXT);
        }

        // "You have. <gold> [and <stock> <icon>] my Lord."
        let gold = ctx.game.gold();
        let mut x = pen.body(canvas, 0x58, 0xB0, &format!("{} ", a.text(GROUP, 12)), font::TEXT);
        x = pen.body(canvas, x, 0xB0, &format!("{gold} "), font::TEXT);
        if stall_row(self.good).icon != 0 {
            x = pen.body(canvas, x, 0xB0, &format!("{} ", a.text(GROUP, 11)), font::TEXT);
            x = pen.body(canvas, x, 0xB0, &format!("{} {} ", -floor, name), font::TEXT);
        }
        pen.body(canvas, x, 0xB0, a.text(GROUP, 13), font::TEXT);

        // The advice well: one wrapped paragraph, or the pending trade.
        canvas.fill_rect(ADVICE_WELL.x, ADVICE_WELL.y, ADVICE_WELL.w, ADVICE_WELL.h, ink.background);
        widget::frame(canvas, ADVICE_WELL, ink.border);
        if self.qty == 0 {
            let advice = self.advice();
            let s = a.text(GROUP, advice.index()).to_string();
            let s = if s.is_empty() { advice.fallback().to_string() } else { s };
            pen.body_wrapped(canvas, ADVICE_X, ADVICE_Y, ADVICE_W, &s, font::TEXT);
        } else {
            let qty = self.qty.abs();
            let x = pen.body(canvas, 0x60, 0xE0, &format!("{qty} "), font::TEXT);
            pen.body(canvas, x, 0xE0, name, font::TEXT);
            // 68/1 "We receive" when selling, 68/8 "Total cost of" when buying.
            let idx = if self.qty < 1 { 1 } else { 8 };
            let x = pen.body(canvas, 0x100, 0xE0, &format!("{} ", a.text(GROUP, idx)), font::TEXT);
            pen.body(canvas, x, 0xE0, &format!("{}", self.crowns(ctx)), font::TEXT);
            // 68/7 "Complete this" + 68/14 "Purchase?" / 68/15 "Sale?"
            let x = pen.body(canvas, 0x60, 0x100, &format!("{} ", a.text(GROUP, 7)), font::TEXT);
            let ask = if self.qty < 0 { 15 } else { 14 };
            pen.body(canvas, x, 0x100, a.text(GROUP, ask), font::TEXT);
        }

        // The six widgets, at the original's six positions. The tick and the
        // cross exist only with something pending — `DAT_00553F58` is 4 or 6.
        widget::button(canvas, ink, up_button(), "+", self.qty < ceiling);
        widget::button(canvas, ink, down_button(), "-", self.qty > floor);
        widget::button(canvas, ink, ceiling_button(), "MAX", ceiling != 0);
        widget::button(canvas, ink, floor_button(), "ALL", floor != 0);
        if self.qty != 0 {
            widget::button(canvas, ink, confirm_button(), "OK", true);
            widget::button(canvas, ink, cancel_button(), "NO", false);
        }

        let drawn = ctx.assets.chrome.as_ref().is_some_and(|c| {
            c.draw_system(canvas, l2_view::chrome::system::OK, PANEL_OK.x, PANEL_OK.y)
        });
        if !drawn {
            shell::button_recess(canvas, PANEL_OK.x, PANEL_OK.y, 24, 24);
        }

        // ---- ours, below the original's window ---------------------------
        text::draw(canvas, PANEL.x, PANEL.y + PANEL.h + 6, &self.status, ink.text);
        text::draw(
            canvas,
            PANEL.x,
            PANEL.y + PANEL.h + 18,
            "UP/DOWN 1  LEFT/RIGHT 10  M MAX BUY  S SELL ALL  ENTER AGREES - KEYS ARE OURS",
            ink.dim,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The table is self-checking: row `i` carries good id `i + 1`, fourteen
    /// for fourteen. A misread row would show up here rather than as a plaque
    /// in the wrong place.
    #[test]
    fn the_stall_table_names_its_own_good_in_every_row() {
        for (i, row) in STALL.iter().enumerate() {
            assert_eq!(row.good, i + 1, "row {i}");
            assert_eq!(stall_row(Good::from_id(row.good).unwrap()), *row);
        }
        assert_eq!(STALL.len(), trade::ALL_GOODS.len());
    }

    /// **Sheep and wool have no place on the stall**, and nothing else is
    /// missing one. The third independent statement that the two goods are not
    /// in the game.
    #[test]
    fn exactly_sheep_and_wool_are_absent_from_the_stall() {
        for good in trade::ALL_GOODS {
            let placed = plaque(good).w > 0;
            assert_eq!(placed, good.tradeable(), "{good:?}");
        }
        assert!(plaque(Good::Sheep).w == 0 && plaque(Good::Wool).w == 0);
        assert!(plaque(Good::Ale).w > 0, "ale is bought here, so it is on the stall");
    }

    /// The `You have … and N …` clause is drawn only for a good you can hold,
    /// and that is exactly the goods with a non-zero icon: not sheep, not wool,
    /// and **not ale**, which is drunk rather than held.
    #[test]
    fn only_a_good_you_can_hold_gets_an_icon() {
        for good in trade::ALL_GOODS {
            let holdable = !matches!(good, Good::Sheep | Good::Ale | Good::Wool);
            assert_eq!(stall_row(good).icon != 0, holdable, "{good:?}");
            // and the unread third column agrees with it, row for row
            assert_eq!(stall_row(good).unread != 0, holdable, "{good:?}");
        }
    }

    /// Every plaque is on the screen, and **they overlap** — which is a fact
    /// about the original rather than a defect in the table.
    ///
    /// `FUN_0041608B` draws exactly one plaque at a time and erases the last
    /// one before drawing the next, so two goods may claim the same 128 x 64
    /// box and nothing ever shows both. Grain's at (100, 230) and stone's at
    /// (160, 210) do. That is why the plaque box is a poor hit test and
    /// `mercgrid.pl8` is the real one; the fallback below resolves an overlap
    /// in ascending good id, deterministically, and only ever runs on a machine
    /// with no artwork at all.
    #[test]
    fn the_plaques_fit_on_the_screen_and_overlapping_ones_resolve_in_id_order() {
        let placed: Vec<(Good, Rect)> = trade::ALL_GOODS
            .iter()
            .map(|&g| (g, plaque(g)))
            .filter(|(_, r)| r.w > 0)
            .collect();
        assert_eq!(placed.len(), 12, "fourteen goods less sheep and wool");
        for (g, r) in &placed {
            assert!(r.x >= 0 && r.y >= 0 && r.x + r.w <= 640 && r.y + r.h <= 480, "{g:?} {r:?}");
        }
        // The overlap is real and named, so nobody "fixes" the table later.
        let grain = plaque(Good::Grain);
        let stone = plaque(Good::Stone);
        assert!(grain.contains(stone.x, stone.y + stone.h - 1), "grain and stone overlap");
        // And the fallback picks the lower id where they do.
        let first = trade::ALL_GOODS
            .iter()
            .copied()
            .find(|&g| plaque(g).contains(stone.x, stone.y + stone.h - 1));
        assert_eq!(first, Some(Good::Grain));
    }

    /// The six widgets land inside the original's window, in two rows, and the
    /// tick and cross are the pair on the lower one.
    #[test]
    fn the_six_widgets_are_inside_the_panel() {
        for i in 0..6 {
            let r = widget_rect(i);
            assert!(PANEL.contains(r.x, r.y), "widget {i} is outside the window");
            assert!(PANEL.contains(r.x + r.w - 1, r.y + r.h - 1), "widget {i} runs off it");
        }
        assert_eq!(up_button().y, down_button().y);
        assert_eq!(confirm_button().y, cancel_button().y);
        assert!(confirm_button().y > up_button().y);
    }

    /// The four advice indices are the four strings, and no two share one.
    #[test]
    fn the_four_advice_paragraphs_are_four_distinct_strings() {
        let all = [Advice::CannotAfford, Advice::NothingToSell, Advice::Ale, Advice::UseTheArrows];
        let mut idx: Vec<usize> = all.iter().map(|a| a.index()).collect();
        idx.sort_unstable();
        idx.dedup();
        assert_eq!(idx, vec![16, 17, 18, 19]);
        for a in all {
            assert!(!a.fallback().is_empty());
        }
    }
}

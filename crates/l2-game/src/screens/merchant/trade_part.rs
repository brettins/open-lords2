#![allow(unused_imports)]
use super::*;
use super::merchant::*;
use l2_kingdom::trade::{self, Good, Order, Quote, Refusal};
use l2_view::{text, Canvas};
use crate::input::{Event, Key, Rect};
use crate::press::{Press, Widget};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::shell::{self, font, Pen};

/// `FUN_004093E0(0x30, 0x40, 0x22, 0x10)`.
pub const PANEL: Rect = Rect::new(0x30, 0x40, 0x22 * 16, 0x10 * 16);
/// `Ui_OkButton(0x22C, 0x114, 0)`.
pub const PANEL_OK: Rect = Rect::new(0x22C, 0x114, 24, 24);
/// `Ui_DrawBevelRect(0x44, 0x66, 0x32, 0x32)` — the commodity icon's well.
pub const ICON_WELL: Rect = Rect::new(0x44, 0x66, 0x32, 0x32);
/// `Sprite_WGenSprite(good - 1, 0x45, 0x67)` — one pixel in from the well.
pub const ICON_AT: (i32, i32) = (0x45, 0x67);

/// `Eng_DrawString(68, …, 0x88, 0x68, heading)` — the heading, 22-pixel font.
pub const HEADING_AT: (i32, i32) = (0x88, 0x68);
/// `Ui_DrawHappinessDelta(v, g_penAdvance + 0x90, 0x6E, …)` — the pen is the
/// heading's own, so the delta lands `0x90 - 0x88` beyond where the good's name
/// ended, eight pixels **into** the next line's row.
pub const ALE_DELTA_DX: i32 = 0x90 - HEADING_AT.0;
pub const ALE_DELTA_Y: i32 = 0x6E;
/// `Misc_cty.pl8` frame `0x17` — the happiness face `Ui_DrawHappinessDelta`
/// puts between the brackets. It is drawn two pixels above the text's y.
pub const HAPPINESS_FACE: usize = 0x17;
/// The closing bracket is a flat `0x14` past the face, whatever it measured.
pub const HAPPINESS_CLOSE_DX: i32 = 0x14;

/// `Eng_DrawString(68, 9 | 0xA, 0x88, 0x88, body)`.
pub const PRICE_AT: (i32, i32) = (0x88, 0x88);
/// `Eng_DrawString(68, 0x0C, 0x58, 0xB0, body)` — *"You have."*
pub const HAVE_AT: (i32, i32) = (0x58, 0xB0);
/// `Sprite_WGenSprite(icon, …, 0xAA)` — six pixels above [`HAVE_AT`]'s y.
pub const HAVE_ICON_Y: i32 = 0xAA;
/// `g_penAdvance = g_penAdvance + 0x24` after the icon — a **fixed** step that
/// ignores how wide the sprite was.
pub const HAVE_ICON_ADVANCE: i32 = 0x24;

/// `Ui_DrawInsetRect(0x50, 0xD0, 0x1D0, 0x4C)` — the advice well.
pub const ADVICE_WELL: Rect = Rect::new(0x50, 0xD0, 0x1D0, 0x4C);
/// `Ui_DrawBoxInterior(0x50, 0xD0, 0x1D, 5)` — the parchment under it, in
/// cells. **29 × 5 cells is 464 × 80 and the inset over it is 464 × 76**, so
/// the parchment is four pixels taller than the line that frames it.
pub const ADVICE_CELLS: (i32, i32) = (0x1D, 5);
/// `Ui_DrawNumber(|qty|, '@', "", 0x60, 0xE0, body)`.
pub const QTY_AT: (i32, i32) = (0x60, 0xE0);
/// `Eng_DrawString(68, 1 | 8, 0x100, 0xE0, body)`.
pub const TOTAL_AT: (i32, i32) = (0x100, 0xE0);
/// `Eng_DrawString(68, 7, 0x60, 0x100, body)` — *"Complete this"*.
pub const ASK_AT: (i32, i32) = (0x60, 0x100);
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

pub(super) fn widget_rect(i: usize) -> Rect {
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

/// **`DAT_004DD838` as a table: six kind-4 records**, and `DAT_00553F58` of
/// them live — four with nothing agreed, six with a quantity pending.
///
/// `node tools/oracle/kinds.js` files all six handlers under `widget 4`. `[V]`
/// So every one fires on the press, clicks, shows `base + 1`, and repeats while
/// held; this screen answered raw clicks, so none of them clicked, drew a
/// pressed picture or repeated — and the thumbs up and down at the bottom are
/// the same mailed hands as every yes/no in the game.
///
/// Each `arm!` is the marker and the kind. The index is the table's.
fn trade_widgets(pending: bool) -> Vec<Widget> {
    let mut out = vec![
        Widget::new(up_button(), crate::arm!("0x00435339/trade-more", Repeat)),
        Widget::new(down_button(), crate::arm!("0x0043543D/trade-less", Repeat)),
        Widget::new(ceiling_button(), crate::arm!("0x004355DB/trade-all", Repeat)),
        Widget::new(floor_button(), crate::arm!("0x00435541/trade-none", Repeat)),
    ];
    if pending {
        out.push(Widget::new(confirm_button(), crate::arm!("0x00435286/trade-agree", Repeat)));
        out.push(Widget::new(cancel_button(), crate::arm!("0x004352F2/trade-refuse", Repeat)));
    }
    out
}

/// **`if (DAT_00591554 < 0x2C)` — the repeat step from which the trade arrows
/// move ten at a time.** `FUN_00435339` and `FUN_0043543D` both read it, and it
/// is `0` on the press. `[V]` See [`crate::press::Press::repeat_step`].
pub const TRADE_FAST_STEP: u8 = 0x2C;

/// `Ui_DrawBevelRect` (`0x00403FDD`) — **four clipped lines and no fill**, and
/// it is [`crate::shell::inset_rect`] with its two colours the other way round:
/// `0x1F` along the top and right, `0x10` along the bottom and left. So an
/// inset reads as recessed and this reads as raised, under any palette that
/// puts a light and a dark at those two indices.
///
/// It lives here because this is the only screen in the
/// crate that draws one.
pub fn bevel_rect(canvas: &mut Canvas, x: i32, y: i32, w: i32, h: i32) {
    const TOP_RIGHT: u8 = 0x1F;
    const BOTTOM_LEFT: u8 = 0x10;
    if w < 1 || h < 1 {
        return;
    }
    canvas.fill_rect(x, y, w, 1, TOP_RIGHT);
    canvas.fill_rect(x + w - 1, y, 1, h, TOP_RIGHT);
    canvas.fill_rect(x, y + h - 1, w, 1, BOTTOM_LEFT);
    canvas.fill_rect(x, y, 1, h, BOTTOM_LEFT);
}

/// `Ui_DrawHappinessDelta` (`0x0041AC95`) — **four draws**: an opening bracket,
/// `Ui_DrawDelta` in mode 1, `Misc_cty.pl8` frame `0x17` and a closing bracket.
///
/// `Ui_DrawDelta(v, 1, "", "", …, 0x3F, 0xF9)` prints `+N` in the caller's
/// positive colour and `-N` in its negative one — and mode 1 means **a zero is
/// printed**, so the caller guards on
/// `g_alePreviewHappiness != 0` itself.
///
/// **On the trade screen it is the ale preview**, and only there: the whole
/// call is `if (good == 4 && g_alePreviewHappiness != 0)`. It reads
/// `g_alePreviewHappiness` (`0x0053E8C0`), which `Ale_PreviewGain`
/// (`0x00435673`) writes from the *pending* crown total — so the number beside
/// *"Ale"* is what the purchase would buy, before it is made, and it is capped
/// at `5 - county.aleHappinessGiven`. It is the one place a county field is
/// shown on a screen that is otherwise entirely about the realm's purse.
pub fn happiness_delta(pen: &Pen, canvas: &mut Canvas, value: i32, x: i32, y: i32) {
    // `Ui_DrawText("(", x, y, font, 0x3F)`, then `g_penAdvance -= 4`.
    let after = pen.body(canvas, x, y, "(", font::TEXT) - shell::TRAILING;
    let colour = if value < 0 { ASK_BUY_COLOUR } else { font::TEXT };
    let sign = if value < 0 { '-' } else { '+' };
    let after = pen.body(canvas, after, y, &format!("{sign}{}", value.abs()), colour);
    pen.misc_frame(canvas, HAPPINESS_FACE, after, y - 2);
    pen.body(canvas, after + HAPPINESS_CLOSE_DX, y, ")", font::TEXT);
}

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
    /// `DAT_004DD838`'s press timers and repeat counter. See [`trade_widgets`].
    press: Press,
}

impl TradeScreen {
    pub fn new(unit: usize, good: u8) -> TradeScreen {
        TradeScreen {
            unit,
            good: Good::from_id(good as usize).unwrap_or(Good::Grain),
            qty: 0,
            limit: 0,
            status: String::new(),
            press: Press::new(),
        }
    }

    /// One `DAT_004DD838` record's handler, from the press or from the repeat.
    fn fire(&mut self, ctx: &mut Ctx, widget: usize) -> Transition {
        let by = if self.press.repeat_step() < TRADE_FAST_STEP { 1 } else { 10 };
        match widget {
            0 => self.step(ctx, by),
            1 => self.step(ctx, -by),
            2 => self.jump_to_limit(ctx, true),
            3 => self.jump_to_limit(ctx, false),
            // `FUN_00435286`: `Merchant_Trade(…)`, then `g_screenId = 8`.
            4 => return self.confirm(ctx),
            // `FUN_004352F2`: `g_screenId = 8`.
            _ => return Transition::Pop,
        }
        Transition::Stay
    }

    fn morale(&self, ctx: &Ctx) -> i32 {
        ctx.game.kingdom.campaign.units.get(self.unit).map_or(0, |u| u.morale)
    }

    fn quote(&self, ctx: &Ctx) -> Quote {
        trade::quote(&ctx.game.kingdom.tables, self.good, self.morale(ctx))
    }

    /// `DAT_00554170`, the quantity on the panel: positive buys, negative sells.
    pub fn qty(&self) -> i32 {
        self.qty
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
    ///
    /// The up arrow is `S068_01.wav` and the down arrow `S068_02.wav`; see
    /// [`TradeScreen::crossed_into_buying`] for the guard both share.
    // sfx: FUN_00435339#1,FUN_0043543d#1
    fn step(&mut self, ctx: &mut Ctx, by: i32) {
        let (floor, ceiling) = self.limits(&Ctx { game: ctx.game, assets: ctx.assets });
        let before = self.qty;
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
        self.crossed_into_buying(ctx, before, if by > 0 {
            crate::audio::names::speech::TRADE_BUYING_UP
        } else {
            crate::audio::names::speech::TRADE_BUYING_DOWN
        });
    }

    /// **The tail all four quantity handlers share**, and the whole of what
    /// they do with sound:
    ///
    /// ```c
    /// if (0 < qty && oldQty < 1) { Sound_PlayFile(take, 1, 0); }
    /// ```
    ///
    /// `[V]` at `FUN_00435339`, `FUN_0043543D`, `FUN_00435541` and
    /// `FUN_004355DB`. It is the *crossing* and not the value: a player who
    /// holds the up arrow hears it once, on the step that turns a sale or a
    /// standstill into a purchase, and not again while the number climbs.
    ///
    /// **The down arrow's copy is very nearly dead and is not quite.** A step
    /// down cannot raise the quantity, so the guard can only be met when the
    /// clamp does it — the floor is above zero and the quantity was at or
/// below it. Written as the original writes it.
    ///
    /// A screen cannot reach the audio layer (`docs/netcode.md` D-3), so the
    /// line is reported on [`crate::game::Game::spoken`].
    fn crossed_into_buying(&self, ctx: &mut Ctx, before: i32, take: &'static str) {
        if self.qty > 0 && before < 1 {
            ctx.game.spoken = (ctx.game.spoken.0.wrapping_add(1), take);
        }
    }

    /// `FUN_004355DB` and `FUN_00435541` — the two buttons that go straight to
    /// a limit. Neither sets the flag.
    ///
    /// The ceiling button says `S068_01.wav` and the floor button
    /// `S068_02.wav`, on the same crossing the arrows use.
    // sfx: FUN_004355db#1,FUN_00435541#1
    fn jump_to_limit(&mut self, ctx: &mut Ctx, ceiling: bool) {
        let (floor, top) = self.limits(&Ctx { game: ctx.game, assets: ctx.assets });
        let before = self.qty;
        self.limit = 0;
        self.qty = if ceiling { top } else { floor };
        self.crossed_into_buying(ctx, before, if ceiling {
            crate::audio::names::speech::TRADE_BUYING_UP
        } else {
            crate::audio::names::speech::TRADE_BUYING_DOWN
        });
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
/// a page — and it runs under the merchant's palette.
    fn palette(&self) -> Option<&'static str> {
        Some("Merchant.256")
    }

    /// `Widget_Test`'s per-frame pass over `DAT_004DD838`: the arrows' ramp.
    fn update(&mut self, ctx: &mut Ctx) -> Transition {
        for widget in self.press.tick() {
            let t = self.fire(ctx, widget);
            if t != Transition::Stay {
                return t;
            }
        }
        Transition::Stay
    }

    /// `Widget_Test`'s `Sound_RestartSlot(1)`, carried up to the audio layer.
    fn take_clicks(&mut self) -> u8 {
        self.press.take_clicks()
    }

    /// A held arrow moved the quantity: `Screen_DrawWidgets`' `0x0C` arm runs
    /// `Trade_DrawPanel` every frame. See [`Press::take_redraw`].
    fn take_redraw(&mut self) -> bool {
        self.press.take_redraw()
    }

    fn handle(&mut self, event: Event, ctx: &mut Ctx) -> Transition {
        match event {
            Event::KeyDown(Key::Escape) | Event::RightClick { .. } => Transition::Pop,
            Event::KeyDown(Key::Enter) => self.confirm(ctx),
            Event::KeyDown(Key::Up) => {
                self.step(ctx, 1);
                Transition::Stay
            }
            Event::KeyDown(Key::Down) => {
                self.step(ctx, -1);
                Transition::Stay
            }
            Event::KeyDown(Key::Right) => {
                self.step(ctx, 10);
                Transition::Stay
            }
            Event::KeyDown(Key::Left) => {
                self.step(ctx, -10);
                Transition::Stay
            }
            Event::KeyDown(Key::Char('M')) => {
                self.jump_to_limit(ctx, true);
                Transition::Stay
            }
            Event::KeyDown(Key::Char('S')) => {
                self.jump_to_limit(ctx, false);
                Transition::Stay
            }
            // The six widgets through the hit test, each on its own kind — a
// double click is a press to kind 4.
            // widget and reads only the press here.
            Event::Click { .. } | Event::DoubleClick { .. } => {
                let table = trade_widgets(self.qty != 0);
                if let Some(i) = self.press.event(&table, event) {
                    return self.fire(ctx, i);
                }
                Transition::Stay
            }
            // **The corner picture, on the release.** `Screen_FrameInput`'s
            // `0x0C` arm is `Ui_OkButtonClicked()` (`0x0040E7E4`) → `g_screenId
            // = 8`, and that call opens `if (g_mouseLeftReleased == 0) return
            // 0;`. This panel closed on the press.
            // arm: 0x0042FF10/trade-ok left-release
            Event::Release { x, y } => {
                let fired = self.press.event(&trade_widgets(self.qty != 0), event);
                debug_assert!(fired.is_none(), "no trade widget is a release widget");
                if PANEL_OK.contains(x, y) {
                    return Transition::Pop;
                }
                Transition::Stay
            }
            Event::Pointer { .. } | Event::PointerLeft => {
                let fired = self.press.event(&trade_widgets(self.qty != 0), event);
                debug_assert!(fired.is_none(), "no trade widget is a release widget");
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
        // `FUN_00408FCB("merchant.pl8", 0x1E0)` — the stall again, as a
// backdrop, so the panel needs no clear.
        if !shell::background(canvas, a, BACKDROP) {
            canvas.clear(ink.background);
        }
        // `FUN_004093E0(0x30, 0x40, 0x22, 0x10)` — border **set 1**.
        pen.window(canvas, PANEL.x, PANEL.y, 0x22, 0x10, 1);
        // `Ui_OkButton(0x22C, 0x114, 0)` — mode 0, frame 0x33. The original
        // draws it here, second, and not at the end.
        pen.ok_button(canvas, PANEL_OK.x, PANEL_OK.y, 0);

        // `Ui_DrawBevelRect(0x44, 0x66, 0x32, 0x32)` — four lines and **no
        // fill**: exactly `Ui_DrawInsetRect` with its two colours swapped, so
        // it reads as raised. Filling it would paint a hole in the parchment,
        // which is the mistake `shell::inset_rect`'s docs record.
        bevel_rect(canvas, ICON_WELL.x, ICON_WELL.y, ICON_WELL.w, ICON_WELL.h);
        // `Sprite_WGenSprite(good - 1, 0x45, 0x67)` — `icontrad.pl8`.
        if let Some(f) = a.sheet(ICONS).and_then(|s| s.frame(self.good.id() - 1)) {
            canvas.blit(&f, ICON_AT.0, ICON_AT.1);
        }

        let q = self.quote(ctx);
        let (floor, ceiling) = self.limits(ctx);

        // `DAT_0058FE2C = 1` for everything down to "my Lord."
        let caps = pen.drop_caps();

        // The heading: 68/2 "Trading" with nothing pending, 68/6 "Selling" or
        // 68/5 "Buying" with something, then the group 6 name after it — both
        // in the **22-pixel** font.
        let head = if self.qty == 0 {
            TRADING
        } else if self.qty < 0 {
            SELLING
        } else {
            BUYING
        };
        let s = a.text(GROUP, head).to_string();
        let x = caps.heading(canvas, HEADING_AT.0, HEADING_AT.1, &s, font::TEXT);
        let name = a.text(GROUP_GOODS, self.good.id()).to_string();
        let x = caps.heading(canvas, x, HEADING_AT.1, &name, font::TEXT);

        // `if (good == 4 && g_alePreviewHappiness != 0)
        //     Ui_DrawHappinessDelta(v, g_penAdvance + 0x90, 0x6E, body, 0x3F, 0xF9)`
        // — a bracketed signed number with `Misc_cty.pl8` frame 0x17, the
        // happiness face, inside the brackets. It is drawn from the heading's
        // own pen, so it follows the good's name.
        let ale = self.ale_preview(ctx);
        if self.good == Good::Ale && ale != 0 {
            happiness_delta(&pen, canvas, ale, x + ALE_DELTA_DX, ALE_DELTA_Y);
        }

        // The price line, and **the price carries its noun**:
        // `Ui_DrawCount(price, 0, …)` is "N Crowns.", not a bare number.
        if self.qty != 0 {
            let (idx, price) =
                if self.qty < 1 { (SELLING_PRICE, q.sell) } else { (BUYING_PRICE, q.buy) };
            let x = caps.eng(canvas, GROUP, idx, PRICE_AT.0, PRICE_AT.1, font::TEXT);
            caps.count(canvas, x, PRICE_AT.1, price, CROWN_NOUN, font::TEXT);
        }

        // "You have." <gold Crowns.> ["and" <stock> <icon>] "my Lord."
        let gold = ctx.game.gold();
        let mut x = caps.eng(canvas, GROUP, YOU_HAVE, HAVE_AT.0, HAVE_AT.1, font::TEXT);
        x = caps.count(canvas, x, HAVE_AT.1, gold, CROWN_NOUN, font::TEXT);
        let icon = stall_row(self.good).icon;
        if icon != 0 {
            x = caps.eng(canvas, GROUP, AND, x, HAVE_AT.1, font::TEXT);
            // `Ui_DrawNumber(-DAT_0053E9E4, …)` and `DAT_0053E9E4` is **minus**
            // what the seller holds, so this prints the stock. `floor` is the
            // same number with the same sign as the original's.
            // `'@'` and `&DAT_004D3F80`, a NUL: the digits sat four left, and
            // the icon at `g_penAdvance + 0x58` landed right by the same
            // cancellation as everywhere else. **[V]**
            let body = shell::Face::Body;
            x = caps.number_in(body, canvas, x, HAVE_AT.1, -floor, '@', "", font::TEXT);
            // `Sprite_WGenSprite(STALL[good].icon, g_penAdvance + 0x58, 0xAA)`
            // — the good's own picture, **six pixels above the text baseline**,
            // then a flat `g_penAdvance += 0x24` regardless of its width.
            if let Some(f) = a.sheet(ICONS).and_then(|s| s.frame(icon)) {
                canvas.blit(&f, x, HAVE_ICON_Y);
            }
            x += HAVE_ICON_ADVANCE;
        }
        caps.eng(canvas, GROUP, MY_LORD, x, HAVE_AT.1, font::TEXT);

        // `Ui_DrawBoxInterior(0x50, 0xD0, 0x1D, 5)` then `Ui_DrawInsetRect` over
        // it — the parchment and then its four lines, in that order.
        pen.box_interior(canvas, ADVICE_WELL.x, ADVICE_WELL.y, ADVICE_CELLS.0, ADVICE_CELLS.1);
        pen.inset(canvas, ADVICE_WELL);

        if self.qty == 0 {
            let advice = self.advice();
            let s = a.text(GROUP, advice.index()).to_string();
            let s = if s.is_empty() { advice.fallback().to_string() } else { s };
            pen.body_wrapped(canvas, ADVICE_X, ADVICE_Y, ADVICE_W, &s, font::TEXT);
        } else {
            // 68/7 "Complete this" and then the question, **in two colours**.
            let x = pen.eng(canvas, GROUP, COMPLETE_THIS, ASK_AT.0, ASK_AT.1, font::TEXT);
            let (ask, colour) = if self.qty < 0 {
                (SALE_ASK, ASK_SELL_COLOUR)
            } else {
                (PURCHASE_ASK, ASK_BUY_COLOUR)
            };
            pen.eng(canvas, GROUP, ask, x, ASK_AT.1, colour);
            // `Ui_DrawNumber(|qty|, '@', "", 0x60, 0xE0)` — **the number alone.**
            // The good's name is not repeated here; ours used to add it. The
            // two suffixes are `&DAT_004D3F84` / `…88`, both NUL. **[V]**
            let (qx, qy) = QTY_AT;
            pen.number_in(shell::Face::Body, canvas, qx, qy, self.qty.abs(), '@', "", font::TEXT);
            // 68/1 "We receive" when selling, 68/8 "Total cost of" when buying,
            // then `Ui_DrawCount(g_tradeCrowns, 0, …)` — "N Crowns."
            let idx = if self.qty < 1 { WE_RECEIVE } else { TOTAL_COST_OF };
            let x = pen.eng(canvas, GROUP, idx, TOTAL_AT.0, TOTAL_AT.1, font::TEXT);
            pen.count(canvas, x, TOTAL_AT.1, self.crowns(ctx), CROWN_NOUN, font::TEXT);
        }

        // `Widget_Draw(0x30, 0x50, &DAT_004DD838, DAT_00553F58)` — four records
        // with nothing pending and six with something, and `System.pl8` carries
        // all six pictures.
        let shown = if self.qty == 0 { 4 } else { 6 };
        for (i, &frame) in WIDGET_FRAMES.iter().enumerate().take(shown) {
            let r = widget_rect(i);
            pen.system_frame(canvas, frame + usize::from(self.press.is_pressed(i)), r.x, r.y);
        }
        // The two clamps are read for the *arrows'* behaviour, not for their
        // pictures: `Widget_Draw` draws every record it is given whether or not
        // the handler would do anything.
        let _ = (floor, ceiling);

        // ---- ours, below the original's window, debug overlay only --------
        if ctx.game.prefs.debug_overlay {
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
}


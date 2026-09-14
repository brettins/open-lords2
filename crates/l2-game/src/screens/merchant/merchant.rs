#![allow(unused_imports)]
use super::*;
use super::trade_part::*;
use l2_kingdom::trade::{self, Good, Order, Quote, Refusal};
use l2_view::{text, Canvas};
use crate::input::{Event, Key, Rect};
use crate::press::{Press, Widget};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::shell::{self, font, Pen};

/// One row of the fourteen-row table at `0x004D2B70`, five ints apiece.
///
/// Read out of `Lords2.exe`. The third
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
/// Carried, and marked `[I]` for whoever finds the
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
pub const PLAQUE_CELLS: (i32, i32) = (8, 4);
pub const PLAQUE_W: i32 = PLAQUE_CELLS.0 * 16;
pub const PLAQUE_H: i32 = PLAQUE_CELLS.1 * 16;

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
/// draw prices everything at its floor of one crown.
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
/// down.
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
            // **`Ui_OkButtonClicked` (`0x0040E7E4`) is a RELEASE**, and this arm
            // tested it on the press. Its first statement is
            // `if (g_mouseLeftReleased == 0) return 0;`, and `Screen_FrameInput`'s
            // `0x08` arm is nothing but that call and the right release:
            //
            // ```c
            // if (g_mouseRightReleased == 0) { if (Ui_OkButtonClicked()) { g_screenId = 0; … } }
            // else { g_screenId = 0; … }
            // ```
            //
            // arm: 0x0042FF10/merchant-ok left-release
            Event::Release { x, y } => {
                if STALL_OK.contains(x, y) {
                    return Transition::Pop;
                }
                Transition::Stay
            }
            Event::Click { x, y } => {
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
        // `FUN_00408FCB("merchant.pl8", 0x1E0)`, and it is the whole screen.
        let have_backdrop = shell::background(canvas, a, BACKDROP);
        if !have_backdrop {
            canvas.clear(ink.background);
        }
        // **No caption.** `Screen_Merchant` draws no string at all; see the
        // module docs for what used to be here and why it was invented.

        // `Merchant_HoverPlaque` (`0x0041608B`), five draws: the box, the name
        // centred in it, and the two prices side by side.
        if let Some(good) = Good::from_id(self.hover) {
            let r = plaque(good);
            if r.w > 0 {
                let q = trade::quote(&ctx.game.kingdom.tables, good, self.morale(ctx));
                // `FUN_004093E0(x, y, 8, 4)` — border **set 1**, in cells.
                pen.window(canvas, r.x, r.y, PLAQUE_CELLS.0, PLAQUE_CELLS.1, 1);
                // `Ui_DrawCentred(6, good, x, y + 0x10, 0x80, body, 0x3F)`.
                pen.eng_centred(canvas, GROUP_GOODS, good.id(), r.x, r.y + 0x10, PLAQUE_W, font::TEXT);
                // `Ui_DrawNumber(sell, '@', "/", x + 0x24, y + 0x22, body)` and
                // then the buy price at `x + g_penAdvance + 0x24`, suffix "".
                // The suffix really is a bare `/`: the manual's *"30/60"*.
                let x = pen.body(canvas, r.x + 0x24, r.y + 0x22, &format!("{}/", q.sell), font::TEXT);
                pen.body(canvas, x, r.y + 0x22, &format!("{}", q.buy), font::TEXT);
            }
        }

        // `Ui_OkButton(stride - 0x1C, height - 0x1C, 1)` — mode 1, frame 0x10.
        pen.ok_button(canvas, STALL_OK.x, STALL_OK.y, 1);

        // ---- ours, debug overlay only ---------------------------------------
        if ctx.game.prefs.debug_overlay {
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
}

// ---------------------------------------------------------------------------
// the trade panel
// ---------------------------------------------------------------------------


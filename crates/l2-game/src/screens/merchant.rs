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
//! # The painter, address by address — and the stall makes **one** draw call
//!
//! ```text
//! Screen_Merchant(firstFrame):                                 0x00415FB7
//!   File_ReadChunk("merchant.256", &DAT_004EA8A0, 0x300)   the palette only
//!   FUN_00408FCB("merchant.pl8", 0x1E0)   the whole stall: 640 x 480 raw
//!   File_ReadChunk("mercgrid.pl8", &g_villageGrid, 0x12D8)   the hit map
//!   Ui_OkButton(stride - 0x1C, height - 0x1C, 1)    the corner OK (612, 452)
//!   DAT_00569500 = 0            <- the widget count, and nothing raises it
//!   g_merchantHoverDamper = 500
//!
//! Screen_DrawWidgets, 0x08 arm:                                0x004BA26E
//!   Merchant_HoverPlaque()                                     0x0041608B
//!     FUN_004B3EC0 / FUN_004B3F0A(sprite bank, 0x200)   save and restore a
//!                                     32 x 64 patch: the plaque's own erase
//!     FUN_004093E0(x, y, 8, 4)          border set 1, 128 x 64 at the good's
//!                                       own (x, y) out of g_goodsStall
//!     Ui_DrawCentred(6, good, x, y + 0x10, 0x80, body, 0x3F)   the name
//!     Ui_DrawNumber(sell,      '@', "/", x + 0x24,       y + 0x22, body)
//!     Ui_DrawNumber(sell + mk, '@', "",  x + pen + 0x24, y + 0x22, body)
//!   Widget_Draw(0, 0, &DAT_004DD808, DAT_00569500)   <- count is always 0
//! ```
//!
//! **The stall is a picture and nothing else.** `merchant.pl8` is not a sprite
//! sheet: the shipped file is 307,224 bytes, which is `640 * 480 + 24`, and
//! `FUN_00408FCB` reads it from offset `0x18` straight into the display buffer.
//! Every ware, every price plaque frame, every shelf is in that one raster. So
//! `Screen_Merchant`'s single `Ui_OkButton` really is the whole of its drawing,
//! the grid is nowhere on screen, and there is no missing painter to find.
//! `docs/draws.md` §3's *"an audit that walks painters reports a comfortable
//! number"* has an opposite here: an audit that walks painters reports **one**,
//! and one is the truth.
//!
//! # `DAT_004DD808` is two widgets that are drawn and tested with a count of 0
//!
//! `Screen_Merchant` writes `DAT_00569500 = 0` and **nothing in the binary ever
//! writes it again** — `xref.js touches DAT_00569500` returns three functions,
//! and the other two are `Screen_DrawWidgets` and `Screen_HandleInput` reading
//! it. The table itself is two records, bounded by `DAT_004DD838` at
//! `0x004DD808 + 2 * 24`:
//!
//! ```text
//! x = 256  y = 452  frame = 27  size = 24  handler = FUN_0043527B  kind 4  id 0
//! x = 288  y = 452  frame = 25  size = 24  handler = FUN_0043527B  kind 4  id 1
//! ```
//!
//! and **`FUN_0043527B` is `void f(void) { return; }`**, eleven bytes, pointed
//! at from exactly those two records and nowhere else. Two buttons on the
//! stall's bottom bar, left of the corner OK, whose handler was gutted and
//! whose count was set to zero. This is `g_sendSuppliesWidgets`' cut sheep row
//! in a second place, and a shorter story: there the records survive and the
//! count is short, here the count is zero and the handler is a stub. We draw
//! neither, which is what the original does. **C109.**
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
//! # The panel, address by address — and `Screen_TradeGoods` makes **zero**
//!
//! ```text
//! Screen_TradeGoods():                                         0x00416308
//!   Trade_BeginGood()                       the prices and the two clamps
//!   FUN_00408FCB("merchant.pl8", 0x1E0)     the stall again, as a backdrop
//!   File_ReadChunk("icontrad.pl8", DAT_004EABEC, 160000)   the sprite bank
//!   Trade_DrawPanel()
//!
//! Trade_DrawPanel():   (also the 0x0C widget arm, every frame)  0x0041635F
//!   FUN_004093E0(0x30, 0x40, 0x22, 0x10)     set 1, 544 x 256 at (48, 64)
//!   Ui_OkButton(0x22C, 0x114, 0)                                (556, 276)
//!   Ui_DrawBevelRect(0x44, 0x66, 0x32, 0x32)   the icon's well  (68, 102)
//!   Sprite_WGenSprite(good - 1, 0x45, 0x67)    icontrad frame   (69, 103)
//!   DAT_0058FE2C = 1                            <- drop-cap colour on
//!   Eng_DrawString(68, 2 | 6 | 5, 0x88, 0x68, heading)          (136, 104)
//!   Eng_DrawString(6, good, pen + 0x88, 0x68, heading)   the good's name
//!   if (good == 4 && alePreview != 0)
//!     Ui_DrawHappinessDelta(alePreview, pen + 0x90, 0x6E, body, 0x3F, 0xF9)
//!   if (qty != 0):
//!     Eng_DrawString(68, 10 | 9, 0x88, 0x88, body)             (136, 136)
//!     Ui_DrawCount(sell | buy, 0, pen + 0x88, 0x88, body)   N Crown(s).
//!   Eng_DrawString(68, 0x0C, 0x58, 0xB0, body)   "You have."   (88, 176)
//!   Ui_DrawCount(realm.gold, 0, pen + 0x58, 0xB0, body)
//!   if (STALL[good].icon != 0):
//!     Eng_DrawString(68, 0x0B, pen + 0x58, 0xB0, body)         "and"
//!     Ui_DrawNumber(stock, '@', "", pen + 0x58, 0xB0, body)
//!     Sprite_WGenSprite(STALL[good].icon, pen + 0x58, 0xAA)    (…, 170)
//!     g_penAdvance += 0x24
//!   Eng_DrawString(68, 0x0D, pen + 0x58, 0xB0, body)    "my Lord."
//!   DAT_0058FE2C = 0                            <- drop-cap colour off
//!   Ui_DrawBoxInterior(0x50, 0xD0, 0x1D, 5)   the well's parchment, 29 x 5
//!   Ui_DrawInsetRect(0x50, 0xD0, 0x1D0, 0x4C)   its four lines (80, 208)
//!   if (qty == 0):
//!     DAT_00553F58 = 4
//!     FUN_0040328E(68, 0x12 | 0x10 | 0x13 | 0x11, 0x60, 0xF6, 0x1C0, body)
//!   else:
//!     DAT_00553F58 = 6
//!     Eng_DrawString(68, 7, 0x60, 0x100, body)   "Complete this" (96, 256)
//!     Eng_DrawString(68, 0x0F, pen + 0x60, 0x100, body, 0xFC)    "Sale?"
//!     Eng_DrawString(68, 0x0E, pen + 0x60, 0x100, body, 0xF9)    "Purchase?"
//!     Ui_DrawNumber(|qty|, '@', "", 0x60, 0xE0, body)            (96, 224)
//!     Eng_DrawString(68, 1 | 8, 0x100, 0xE0, body)              (256, 224)
//!     Ui_DrawCount(g_tradeCrowns, 0, pen + 0x100, 0xE0, body)
//!   Widget_Draw(0x30, 0x50, &DAT_004DD838, DAT_00553F58)
//! ```
//!
//! **`Screen_TradeGoods` itself has no draw call at all.** All 33 are in
//! `Trade_DrawPanel`, which the painter calls once and `Screen_DrawWidgets`
//! calls again every frame — so the panel repaints itself over the backdrop
//! without the screen being redrawn, which is `docs/draws.md`'s send-supplies
//! finding in a second place.
//!
//! **The two colours on one line are the original's.** *"Purchase?"* is drawn
//! in `0xF9` and *"Sale?"* in `0xFC` — the only place in this module where the
//! colour carries meaning, and the reason [`ASK_BUY_COLOUR`] and
//! [`ASK_SELL_COLOUR`] are named rather than folded into `font::TEXT`.
//!
//! **`DAT_0058FE2C` is on for the middle third of the panel** and off for the
//! rest: the heading, the price line and the *"You have"* line are drawn with
//! the drop-capital colour and the advice well is not. `Pen::drop_caps` is that
//! flag.
//!
//! Six widgets at `0x004DD838`, drawn at an offset of (0x30, 0x50) — and **only
//! four of them exist until a quantity is chosen**: `Widget_Draw` is given a
//! count of 4 with nothing pending and 6 with something, so the tick and the
//! cross appear when there is something to agree to. That is `DAT_00553F58`,
//! written by `Trade_DrawPanel` itself in the two arms above, and it is the
//! panel's whole modality. **The table is exactly six records long** —
//! `0x004DD838 + 6 * 24` is `0x004DD8C8`, which is `g_armouryBuyWidgets` — so
//! unlike the merchant's own table there is no cut row behind this count.
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

/// `L2.eng` group 68 — the merchant's own words, and **only the trade panel
/// draws any of them**. `Screen_Merchant` calls `Eng_DrawString` not once.
///
/// **Verified against the words.** Group 68 is 20 strings and this module draws
/// fourteen of them; every index below was read out of the file and reads as
/// the fragment the panel assembles it into — 0 *"Click on a price to trade
/// with"*, 1 *"We receive"*, 2 *"Trading"*, 3 *"Buy"*, 4 *"Sell"*, 5
/// *"Buying"*, 6 *"Selling"*, 7 *"Complete this"*, 8 *"Total cost of"*, 9
/// *"Buying price"*, 0xA *"Selling price"*, 0xB *"and"*, 0xC *"You have."*,
/// 0xD *"my Lord."*, 0xE *"Purchase?"*, 0xF *"Sale?"*, 0x10…0x13 the four
/// advice paragraphs.
pub const GROUP: usize = 68;
/// `L2.eng` group 6 — the fourteen goods' names, indexed 1…14 by the good's own
/// id, with index 0 *"No goods"*. Both the stall plaque and the panel's heading
/// draw it. **[V]** against the words: 1 Grain … 14 Mail, in the order
/// [`l2_kingdom::trade::Good`] numbers them.
pub const GROUP_GOODS: usize = 6;

/// **Drawn by nothing.** Every `Eng_DrawString`/`FUN_0040328E` call in the
/// corpus with a literal group-68 index is one of 1, 2, 5, 6, 7, 8, 9, 0xA,
/// 0xB, 0xC, 0xD, 0xE, 0xF, 0x10, 0x11, 0x12, 0x13 — seventeen of the twenty,
/// **all seventeen in `Trade_DrawPanel`**. The three left over are this,
/// [`BUY_LABEL`] and [`SELL_LABEL`].
///
/// *"Click on a price to trade with"* is the stall's own instruction and
/// `Screen_Merchant` draws no string at all; like the court's *"Arms"* and
/// `L2.eng` 31/21 *"Morale"*, it is a label left in the file. Named so the next
/// reader does not go looking for the painter that shows it.
pub const CLICK_ON_A_PRICE: usize = 0;
/// **Drawn by nothing** — see [`CLICK_ON_A_PRICE`]. The arrows carry pictures
/// (`System.pl8` frames 21 and 23) instead of these two words.
pub const BUY_LABEL: usize = 3;
/// See [`BUY_LABEL`].
pub const SELL_LABEL: usize = 4;

pub const WE_RECEIVE: usize = 1;
pub const TRADING: usize = 2;
pub const BUYING: usize = 5;
pub const SELLING: usize = 6;
pub const COMPLETE_THIS: usize = 7;
pub const TOTAL_COST_OF: usize = 8;
pub const BUYING_PRICE: usize = 9;
pub const SELLING_PRICE: usize = 0xA;
pub const AND: usize = 0xB;
pub const YOU_HAVE: usize = 0xC;
pub const MY_LORD: usize = 0xD;
pub const PURCHASE_ASK: usize = 0xE;
pub const SALE_ASK: usize = 0xF;

/// `L2.eng` group 8 index 0/1 — *"Crown."* / *"Crowns."*. Every
/// `Ui_DrawCount(v, 0, …)` on the panel: the gold, the unit price and the
/// total. **[V]** against the words.
pub const CROWN_NOUN: usize = 0;

/// `Eng_DrawString(68, 0x0E, …, 0xF9)` — *"Purchase?"* is drawn in the
/// highlight colour and *"Sale?"* in `0xFC`. The two differ, and no other pair
/// on this screen does.
pub const ASK_BUY_COLOUR: u8 = 0xF9;
/// See [`ASK_BUY_COLOUR`]. `0xFC` has no name in `shell::font`; it appears at
/// this one call site.
pub const ASK_SELL_COLOUR: u8 = 0xFC;

/// `File_ReadChunk("icontrad.pl8", DAT_004EABEC, 160000)` — the trade panel's
/// sprite bank. Two things are drawn out of it: frame `good - 1` in the well,
/// and [`StallRow::icon`] beside the *"and N …"* clause.
pub const ICONS: &str = "Icontrad.pl8";
/// `FUN_00408FCB("merchant.pl8", 0x1E0)` — a raw 640 × 480 raster, read by
/// **both** screens. The shipped file is 307,224 bytes = `640 * 480 + 24`.
pub const BACKDROP: &str = "Merchant.pl8";

/// `System.pl8` frames for the six records of `DAT_004DD838`, in table order:
/// up, down, all, none, thumb up, thumb down.
pub const WIDGET_FRAMES: [usize; 6] = [21, 23, 72, 70, 29, 31];

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
/// ignores how wide the sprite actually was.
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

/// `Ui_DrawBevelRect` (`0x00403FDD`) — **four clipped lines and no fill**, and
/// it is [`crate::shell::inset_rect`] with its two colours the other way round:
/// `0x1F` along the top and right, `0x10` along the bottom and left. So an
/// inset reads as recessed and this reads as raised, under any palette that
/// puts a light and a dark at those two indices.
///
/// It lives here rather than in `shell` because this is the only screen in the
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
/// printed** rather than skipped, which is why the caller guards on
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
        // `FUN_00408FCB("merchant.pl8", 0x1E0)` — the stall again, as a
        // backdrop, which is why the panel needs no clear.
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
            caps.count(canvas, x, PRICE_AT.1, price, CROWN_NOUN, true, font::TEXT);
        }

        // "You have." <gold Crowns.> ["and" <stock> <icon>] "my Lord."
        let gold = ctx.game.gold();
        let mut x = caps.eng(canvas, GROUP, YOU_HAVE, HAVE_AT.0, HAVE_AT.1, font::TEXT);
        x = caps.count(canvas, x, HAVE_AT.1, gold, CROWN_NOUN, true, font::TEXT);
        let icon = stall_row(self.good).icon;
        if icon != 0 {
            x = caps.eng(canvas, GROUP, AND, x, HAVE_AT.1, font::TEXT);
            // `Ui_DrawNumber(-DAT_0053E9E4, …)` and `DAT_0053E9E4` is **minus**
            // what the seller holds, so this prints the stock. `floor` is the
            // same number with the same sign as the original's.
            x = caps.number(canvas, x, HAVE_AT.1, -floor, true, font::TEXT);
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
            // The good's name is not repeated here; ours used to add it.
            pen.number(canvas, QTY_AT.0, QTY_AT.1, self.qty.abs(), true, font::TEXT);
            // 68/1 "We receive" when selling, 68/8 "Total cost of" when buying,
            // then `Ui_DrawCount(g_tradeCrowns, 0, …)` — "N Crowns."
            let idx = if self.qty < 1 { WE_RECEIVE } else { TOTAL_COST_OF };
            let x = pen.eng(canvas, GROUP, idx, TOTAL_AT.0, TOTAL_AT.1, font::TEXT);
            pen.count(canvas, x, TOTAL_AT.1, self.crowns(ctx), CROWN_NOUN, true, font::TEXT);
        }

        // `Widget_Draw(0x30, 0x50, &DAT_004DD838, DAT_00553F58)` — four records
        // with nothing pending and six with something, and `System.pl8` carries
        // all six pictures.
        let shown = if self.qty == 0 { 4 } else { 6 };
        for (i, &frame) in WIDGET_FRAMES.iter().enumerate().take(shown) {
            let r = widget_rect(i);
            pen.system_frame(canvas, frame, r.x, r.y);
        }
        // The two clamps are read for the *arrows'* behaviour, not for their
        // pictures: `Widget_Draw` draws every record it is given whether or not
        // the handler would do anything, so there is no disabled frame.
        let _ = (floor, ceiling);

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

    /// **The six widget frames are the six pictures, in table order**, and the
    /// last two are the same pair every yes/no in the game draws — the castle
    /// chooser's `g_castleBuildWidgets` carries 29 and 31 at the same sizes.
    /// Pinned from `widgets.js`'s reading of `0x004DD838`, not computed.
    #[test]
    fn the_widget_frames_are_the_tables_own_and_end_in_the_mailed_hand() {
        assert_eq!(WIDGET_FRAMES, [21, 23, 72, 70, 29, 31]);
        assert_eq!(WIDGET_FRAMES[4], crate::screens::castle::THUMB_UP);
        assert_eq!(WIDGET_FRAMES[5], crate::screens::castle::THUMB_DOWN);
        // The four arrows are 24 pixels square and the pair is 32, which is
        // the record's `+6` and is why they cannot be drawn from one loop with
        // one size.
        for i in 0..4 {
            assert_eq!(widget_rect(i).w, 24, "record {i}");
        }
        assert_eq!(widget_rect(4).w, 32);
        assert_eq!(widget_rect(5).w, 32);
    }

    /// **Group 68 indices 0, 3 and 4 are drawn by nothing.**
    ///
    /// Seventeen of the twenty are drawn, all seventeen by `Trade_DrawPanel`.
    /// This asserts the module names all three of the leftovers, so that a
    /// later reader adding a *"Buy"* caption has to delete a constant that says
    /// the original has no call site for it. `L2.eng` 31/21 *"Morale"* is the
    /// same shape and cost a `[V]` in `docs/armies.md`.
    #[test]
    fn the_three_undrawn_group_68_strings_are_named() {
        let dead = [CLICK_ON_A_PRICE, BUY_LABEL, SELL_LABEL];
        assert_eq!(dead, [0, 3, 4]);
        let drawn = [
            WE_RECEIVE,
            TRADING,
            BUYING,
            SELLING,
            COMPLETE_THIS,
            TOTAL_COST_OF,
            BUYING_PRICE,
            SELLING_PRICE,
            AND,
            YOU_HAVE,
            MY_LORD,
            PURCHASE_ASK,
            SALE_ASK,
        ];
        for d in dead {
            assert!(!drawn.contains(&d), "{d} is both drawn and not drawn");
        }
        // Thirteen singles plus the four advice paragraphs is seventeen.
        let mut all: Vec<usize> = drawn.to_vec();
        all.extend([16, 17, 18, 19]);
        all.sort_unstable();
        all.dedup();
        assert_eq!(all.len(), 17);
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

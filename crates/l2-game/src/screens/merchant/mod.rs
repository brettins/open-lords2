//! **The merchant** — `Screen_Merchant` (`0x00415FB7`), `g_screenId` `0x08`,
//! and the trade panel `Screen_TradeGoods` (`0x00416308`) / `Trade_DrawPanel`
//! (`0x0041635F`), `g_screenId` `0x0C`, both on `L2.eng` group 68.
//!
//! Two screens, because the original has two screen ids and the panel goes back
//! to the stall. [`MerchantScreen`] is the stall you
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
//! **The stall is a picture.**
//! sheet: the shipped file is 307,224 bytes, which is `640 * 480 + 24`, and
//! `FUN_00408FCB` reads it from offset `0x18` straight into the display buffer.
//! Every ware, every price plaque frame, every shelf is in that one raster. So
//! `Screen_Merchant`'s single `Ui_OkButton` really is the whole of its drawing,
//! the grid is nowhere on screen.
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
//! table, read out of the executable.
//!
//! This is the *"mouseover tooltip the original showed"* a player reported
//! missing, and it is **not** generic hover chrome: it is one function, on one
//! screen, with a throttle of its own. The shell that stood here drew `L2.eng`
//! 68 index 0 as a standing caption at (0x88, 0x68) and that was invented
//! twice over — `Screen_Merchant` calls `Eng_DrawString` **not once**, and
//! index 0 of a group is the *group's own label*
//! (`docs/formats/eng.md` §5). The position was the trade panel's heading
//! borrowed. Both are gone: the stall is the picture and the plaque.
//!
//! `DAT_0053EF54` is the plaque's throttle — set to 500 on entry and again
//! after every swap, decremented once per widget pass, and the swap only
//! happens at zero. Ours updates on the pointer event instead, because our
//! widget pass is not the original's frame and a count of ours would mean a
//! different length of time; the throttle is recorded.
//!
//! **This screen's artwork says sheep and wool are not in the game, twice.**
//! [`STALL`] puts both at **(0, 0)** — no place for a plaque — and the shipped
//! `mercgrid.pl8` holds exactly twelve ids, with **neither 3 nor 5 in any of
//! its 4,800 cells**: there is nowhere on the stall to click for either. With
//! the missing `Merchant_Trade` branch and the price of zero that is four
//! independent sources, and these are the two made by the pictures
//! by the code or the strings. Both goods are carried anyway, for the reason
//! `l2_kingdom::trade` carries them: the game demonstrating its own dead end
//! beats us deciding in advance that it does not exist.
//!
//! # The panel, address by address — and `Screen_TradeGoods` makes **zero**
//!
//! ```text
//! Screen_TradeGoods():                                         0x00416308
//! Trade_BeginGood() the prices and the two clamps
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
//! [`ASK_SELL_COLOUR`] are named.
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
//! unlike the merchant's own table.
//!
//! # The three strings that are the rules stated in English
//!
//! The advice paragraph is chosen by a flag the arrows set, and each of its
//! four values is one of the model's own limits:
//!
//! * **68/18** *"You do not have enough crowns to buy, my Lord."* — the up
//! arrow clamped **and the ceiling was zero**. The original's test is
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

mod merchant;
pub use merchant::*;
mod trade;
pub use trade::*;

use l2_kingdom::trade::{self, Good, Order, Quote, Refusal};
use l2_view::{text, Canvas};

use crate::input::{Event, Key, Rect};
use crate::press::{Press, Widget};
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

#[cfg(test)]
mod tests {
    use super::*;

    /// The table is self-checking: row `i` carries good id `i + 1`, fourteen
    /// for fourteen. A misread row would show up here
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
/// and **not ale**, which is drunk.
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
/// about the original.
    ///
    /// `FUN_0041608B` draws exactly one plaque at a time and erases the last
    /// one before drawing the next, so two goods may claim the same 128 x 64
    /// box and nothing ever shows both. Grain's at (100, 230) and stone's at
    /// (160, 210) do. That is why the plaque box is a poor hit test and
    /// `mercgrid.pl8` is the real one; the fallback below resolves an overlap
    /// in ascending good id, deterministically,
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


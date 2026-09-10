//! **The army-division screen** — `Screen_ArmyDivision` (`0x004192B1`) and
//! `Screen_SplitArmyRows` (`0x00419354`), `g_screenId` `0x11`, `L2.eng` group
//! 17.
//!
//! # What the painter draws
//!
//! ```text
//! File_ReadChunk("icon_tmp.pl8", ...)
//! Ui_DrawBox(8, 0x30, 0x1C, 0x1A)                  the window, 448 x 416 at (8, 48)
//! Ui_OkButton(0x1AC, 0x1B4, 0)
//! Eng_DrawString(17, 0, 0x68, 0x44, heading)       "Army Division."
//! Eng_DrawString(17, 1, 0x78, 0x1AE)               "Split the army?"
//! Ui_DrawBoxInterior(0x18, 0x80, 0x1A, 0x12)       the rows' well
//! for t in 0..7:   y = 0x80 + t * 0x20
//!     Ui_DrawUnitNoun(2, 0x34 + t*2, 0x18, y)          the troop's plural
//!     misc_cty frame 0x2F + t at (0xA8, y - 8)          the troop's PICTURE
//!     Ui_DrawNumber(basket[t].chosen,    0xD8, y)       who stays
//!     misc_cty frame 0x2F + t at (0x158, y - 8)         the same picture again
//!     Ui_DrawNumber(basket[t].available, 0x188, y)      who leaves
//! row 7, only when the army carries a band:
//!     L2.eng 16/mercBand at (0x18, 0x180) and the troop noun at (0x58, 0x190)
//! the two "Total men" lines at y 0x184 with a band, 0x160 without
//! ```
//!
//! **The two columns are two words of one basket slot.** The parent's count is
//! `g_levyBasket[t].chosen` and the daughter's is `g_levyBasket[t].available` —
//! the field that means *"how many of this weapon the armoury has"* on the
//! raise-army screen. `docs/armies.md` §6.2 says the buffer is reused with
//! different field meanings and this is the confirmation; `l2_kingdom::divide`
//! models it as two arrays rather than as an alias, and says why.
//!
//! **Slot 7 is the mercenary band, and it is not a count.** It moves whole:
//! hotspot 7 on either handler swaps which column holds it, and `Army_Split`
//! carries `mercBand`, `mercMen`, `mercTroop` and the live table's `hiredBy`
//! across in one piece.
//!
//! # Every control on this screen was in the wrong place, and the painter is why
//!
//! **`Screen_FrameInput`'s `0x11` arm holds no verb at all** — three exits, all
//! of them to `g_screenId = 0x04`. The buttons are `Screen_HandleInput`'s
//! `Widget_Test(0, 0, &DAT_004DD388, DAT_0055321C)`, which is the *second* of
//! the three places input hides (`docs/arms.json`'s `_note`), and nothing had
//! read it. So this module derived its hit boxes from the painter above, put
//! them on the two columns of `Misc_cty` troop pictures at `0xA8` and `0x158`,
//! and left the real arrows at 256 and 288 with nothing over them. See
//! [`PARENT_BUTTON_X`].
//!
//! **It also had three buttons of its own** — SPLIT, DISBAND and CANCEL in our
//! own font at y 446 — where the table has a tick and a cross at (288, 420) and
//! (336, 424). CANCEL overlapped the tick, so **our cancel sat on the
//! original's confirm**. All three are gone: [`SPLIT_TICK`] is the split,
//! [`SPLIT_CROSS`] is the cancel, and the disband is
//! [`crate::screens::info`]'s, which is where `Panel_DisbandButton`
//! (`0x0043733A`) has always lived — record 1 of the information panel's
//! `g_infoUnitButtons`. `docs/arms.json` counts all three as inventions.
//!
//! # The gate that only the Readme states
//!
//! *"An army normally can only be split only at the start of its movement in a
//! turn."* — `FUN_004378B3` refuses with message `0x95` when `movesUsed >= 1`,
//! and the screen never opens. Ours opens and says why, because a screen that
//! silently will not appear is a screen a player thinks is broken.

use l2_kingdom::divide::{SplitBasket, SplitInto, SplitRefusal};
use l2_kingdom::unit::{ALL_TROOP_TYPES, TroopType};
use l2_view::{text, Canvas};

use crate::input::{Event, Key, Rect};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::shell::{font, Pen};
use crate::widget;

/// `L2.eng` group 17 — *"Army Division."* and *"Split the army?"*.
pub const GROUP: usize = 17;
/// `L2.eng` group 8's troop nouns begin at index `0x34`, two apiece: singular
/// then plural. `Ui_DrawUnitNoun(count, 0x34 + t*2)` picks between them.
pub const NOUN_BASE: usize = 0x34;

/// `Ui_DrawBox(8, 0x30, 0x1C, 0x1A)`.
pub const BOX_X: i32 = 8;
pub const BOX_Y: i32 = 0x30;
pub const BOX_COLS: i32 = 0x1C;
pub const BOX_ROWS: i32 = 0x1A;

pub fn window() -> Rect {
    Rect::new(BOX_X, BOX_Y, BOX_COLS * 16, BOX_ROWS * 16)
}

/// The rows: seven troop types, then the band, `0x20` apart from `0x80`.
pub const ROW_Y: i32 = 0x80;
pub const ROW_STEP: i32 = 0x20;
/// The mercenary band's row — index 7, at `0x80 + 7 * 0x20`.
pub const MERC_ROW: usize = 7;

pub fn row_y(row: usize) -> i32 {
    ROW_Y + row as i32 * ROW_STEP
}

/// The noun's x, and the two columns' icon and number positions.
///
/// **`0xA8` and `0x158` are where the painter draws the troop-type ICONS**, not
/// where the buttons are: `Pl8_DrawFrame(g_miscCtySheet, t + 0x2F, 0xA8, …)` and
/// the same at `0x158` are the same `Misc_cty` frames the information panel's
/// troop grid uses. See [`PARENT_BUTTON_X`] for the buttons, which are
/// somewhere else entirely.
pub const NOUN_X: i32 = 0x18;
pub const PARENT_ICON_X: i32 = 0xA8;
pub const PARENT_NUMBER_X: i32 = 0xD8;
pub const DAUGHTER_ICON_X: i32 = 0x158;
pub const DAUGHTER_NUMBER_X: i32 = 0x188;

/// `Ui_DrawBoxInterior(0x18, 0x80, 0x1A, 0x12)` — the well the rows sit in.
pub fn rows_well() -> Rect {
    Rect::new(0x18, 0x80, 0x1A * 16, 0x12 * 16)
}

/// `Ui_OkButton(0x1AC, 0x1B4, 0)`.
pub const OK: Rect = Rect::new(0x1AC, 0x1B4, 24, 24);

/// The **totals** row: `0x184` when the army carries a band, `0x160` when it
/// does not.
///
/// `0x160` is *exactly* row 7's own y — `0x80 + 7 * 0x20` — so an army with no
/// mercenaries does not leave a gap where the band would have been: the totals
/// move up into its place. With a band they sit four pixels below it. That
/// four-pixel offset is the whole of the layout difference, and it is why the
/// painter carries two literals rather than one plus a step.
pub fn totals_y(band: bool) -> i32 {
    if band {
        0x184
    } else {
        0x160
    }
}

/// **`g_splitWidgets` (`0x004DD388`) — this screen's real controls, and they
/// were not on this screen at all.**
///
/// `Screen_FrameInput`'s `0x11` arm is three exits and no verb; every button
/// here belongs to `Screen_HandleInput`, which is the second of the three
/// places input hides (`docs/arms.json`'s `_note`). The table is
/// `Widget_Test(0, 0, &DAT_004DD388, DAT_0055321C)` with the count
/// `Panel_SplitButton` sets: **16 records with no mercenary band, 18 with one**,
/// so the band's pair exists only when the army has a band.
///
/// ```text
///  #    x    y  frame  handler                 kind  id
///  0  288  420     29  Army_SplitConfirm          5   1   the tick: SPLIT
///  1  336  424     31  Army_SplitConfirm          5   0   the cross: back to 0x04
///  2  256  120     27  SplitScreen_ToParent       4   0   |  eight rows,
///  3  288  120     25  SplitScreen_ToDaughter     4   0   |  0x20 apart,
///  …                                                      |  ids 0 … 7
/// 17  288  344     25  SplitScreen_ToDaughter     4   7   |
/// ```
///
/// **The buttons are at x 256 and 288 and ours were at 168 and 344**, which are
/// [`PARENT_ICON_X`] and [`DAUGHTER_ICON_X`] — the two columns of `Misc_cty`
/// troop-type *pictures*. So our hit boxes sat on the artwork and the
/// original's live controls, in the 32-pixel gutter between the parent's number
/// and the daughter's icon, had nothing over them. The y is right and always
/// was: `row_y(row) - 8` is the table's `row * 0x20 + 0x78` exactly.
///
/// That is the fifth time a hit box on this project has been placed by reading
/// a painter rather than a table; `screens/map.rs`'s header carries the
/// standing warning and this is now one of its examples.
pub const BUTTON_DIM: i32 = 24;
pub const PARENT_BUTTON_X: i32 = 256;
pub const DAUGHTER_BUTTON_X: i32 = 288;

pub fn parent_button(row: usize) -> Rect {
    Rect::new(PARENT_BUTTON_X, row_y(row) - 8, BUTTON_DIM, BUTTON_DIM)
}

pub fn daughter_button(row: usize) -> Rect {
    Rect::new(DAUGHTER_BUTTON_X, row_y(row) - 8, BUTTON_DIM, BUTTON_DIM)
}

/// **The tick, `Army_SplitConfirm` with `g_uiHotspotId == 1`** — 32 square at
/// (288, 420), `System.pl8` frame 29.
pub const SPLIT_TICK: Rect = Rect::new(288, 420, 32, 32);
/// **The cross, the same handler with `g_uiHotspotId == 0`** — frame 31 at
/// (336, 424). It is the *first* statement of `Army_SplitConfirm`:
/// `g_screenId = 0; … if (g_uiHotspotId == 0) g_screenId = 4;`, so the cross
/// goes back to the information panel and splits nothing.
pub const SPLIT_CROSS: Rect = Rect::new(336, 424, 32, 32);

/// How many men a click moves. **Ours**: `SplitScreen_ToParent` moves exactly
/// one — `slot.available -= 1; slot.chosen += 1` — which is unusable for an
/// army of 800 without the key-repeat its widget table's kind 4 supplies and
/// our event loop does not deliver here yet. Ten is
/// [`l2_kingdom::divide`]'s own round number, and the arrow keys still move
/// one. `docs/arms.json` `ours/divide-click-moves-ten`.
///
/// **The band is the exception and it is the original's**: hotspot id 7 swaps
/// the whole band rather than one man of it, in both directions.
// arm: ours/divide-click-moves-ten
pub const CLICK_MEN: i32 = 10;

/// What the screen did before it closed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Divided {
    None,
    /// The daughter army is on the map, in this slot.
    Split(usize),
    /// The men went home: the county they joined and how many.
    ///
    /// **Nothing on this screen produces it any more.** The disband button was
    /// ours; the original's is record 1 of the information panel's
    /// `g_infoUnitButtons`, and it is [`crate::screens::info`]'s now. The
    /// variant is kept because the outcome is still a thing a caller may want
    /// to read and because deleting it would delete the evidence that a button
    /// was here — `docs/arms.json` `ours/divide-disband-button`.
    Disbanded(u8, i32),
    Refused,
}

/// Screen `0x11` for one army.
pub struct DivideScreen {
    /// `DAT_00553074` — the army being divided.
    unit: usize,
    basket: SplitBasket,
    /// Which row the keyboard is on.
    row: usize,
    pub outcome: Divided,
    status: String,
    seeded: bool,
}

impl DivideScreen {
    pub fn new(unit: usize) -> DivideScreen {
        DivideScreen {
            unit,
            basket: SplitBasket::default(),
            row: 0,
            outcome: Divided::None,
            status: String::new(),
            seeded: false,
        }
    }

    pub fn unit(&self) -> usize {
        self.unit
    }

    pub fn basket(&self) -> &SplitBasket {
        &self.basket
    }

    /// `FUN_004378B3`'s seeding: everyone in the parent's column, the band with
    /// them.
    fn seed(&mut self, ctx: &Ctx) {
        if self.seeded {
            return;
        }
        self.seeded = true;
        let Some(unit) = ctx.game.kingdom.campaign.units.get(self.unit) else {
            self.status = "NO SUCH ARMY".into();
            return;
        };
        self.basket = SplitBasket::seed(unit);
        // The Readme's rule, and message 0x95 = group 149 in the original.
        self.status = if unit.moves_used >= 1 {
            "THIS ARMY HAS ALREADY MARCHED THIS SEASON".into()
        } else {
            format!("{} MEN. MOVE THEM RIGHT TO SPLIT THEM OFF", unit.men)
        };
    }

    fn move_men(&mut self, row: usize, to_daughter: bool, n: i32) {
        if row == MERC_ROW {
            if self.basket.move_mercenaries(to_daughter) {
                self.status = "THE BAND MARCHES WHOLE OR NOT AT ALL".into();
            }
            return;
        }
        let Some(troop) = TroopType::from_index(row) else { return };
        let moved = if to_daughter {
            self.basket.to_daughter(troop, n)
        } else {
            self.basket.to_parent(troop, n)
        };
        if moved > 0 {
            self.status =
                format!("{} / {}", self.basket.parent_total(), self.basket.daughter_total());
        }
    }

    /// `FUN_00437AFB` with no destination county — the plain field split.
    fn split(&mut self, ctx: &mut Ctx) -> Transition {
        match ctx.game.split_army(self.unit, &self.basket, SplitInto::Field) {
            Ok(id) => {
                self.outcome = Divided::Split(id);
                Transition::Pop
            }
            Err(no) => {
                self.outcome = Divided::Refused;
                self.status = match no {
                    SplitRefusal::NotAnArmy => "THAT IS NOT YOUR ARMY".into(),
                    SplitRefusal::AlreadyMoved => "IT HAS ALREADY MARCHED THIS SEASON".into(),
                    SplitRefusal::Empty => "ONE SIDE IS EMPTY - NOTHING TO SPLIT".into(),
                    SplitRefusal::TooFew => "BOTH HALVES NEED 50 MEN".into(),
                    SplitRefusal::GarrisonFull(n) => format!("ONLY {n} WOULD FIT"),
                    SplitRefusal::NowhereToStand => "NOWHERE NEARBY TO STAND".into(),
                };
                Transition::Stay
            }
        }
    }

}

impl Screen for DivideScreen {
    fn id(&self) -> ScreenId {
        ScreenId::Divide(self.unit)
    }

    fn title(&self, _ctx: &Ctx) -> String {
        "Army division".to_string()
    }

    fn is_overlay(&self) -> bool {
        true
    }

    fn update(&mut self, ctx: &mut Ctx) -> Transition {
        let read = Ctx { game: ctx.game, assets: ctx.assets };
        self.seed(&read);
        Transition::Stay
    }

    fn handle(&mut self, event: Event, ctx: &mut Ctx) -> Transition {
        {
            let read = Ctx { game: ctx.game, assets: ctx.assets };
            self.seed(&read);
        }
        match event {
            // **`0x11` goes back to `0x04`, not to the map**, and all three of
            // its ways out say so: the turn-ended latch, the right release and
            // `Ui_OkButtonClicked` each write `g_screenId = 0x04`. Five of the
            // nineteen right-close arms step back one level rather than to the
            // campaign map — `0x0C` → `0x08`, `0x0D` → `0x0A`, `0x11` → `0x04`,
            // `0x17` → `0x0A`, `0x2A` → `0x29` — and a stack pop is that, so
            // long as the screen underneath is the one the original names.
            //
            // It was not: this screen was reached by `Transition::Replace` from
            // the information panel, so popping it landed on the campaign map.
            // [`crate::screens::info`] pushes now.
            //
            // **The right release was missing entirely** — this screen had no
            // right-button arm at all, so the button every other window in the
            // game closes with did nothing here.
            // arm: 0x0042FF10/back-one-rather-than-to-the-map
            Event::RightClick { .. } => Transition::Pop,
            // **Ours, and counted.** `Screen_HandleInput` names no key on this
            // screen and the window procedure has no `0x11` case, so every one
            // of these is an invention. Kept, because the arrows are the only
            // way to move one man at a time now that a click moves ten.
            // arm: ours/divide-keyboard
            Event::KeyDown(Key::Escape) => Transition::Pop,
            Event::KeyDown(Key::Up) => {
                self.row = self.row.saturating_sub(1);
                Transition::Stay
            }
            Event::KeyDown(Key::Down) => {
                self.row = (self.row + 1).min(MERC_ROW);
                Transition::Stay
            }
            Event::KeyDown(Key::Right) => {
                self.move_men(self.row, true, 1);
                Transition::Stay
            }
            Event::KeyDown(Key::Left) => {
                self.move_men(self.row, false, 1);
                Transition::Stay
            }
            Event::KeyDown(Key::Enter) => self.split(ctx),
            Event::Click { x, y } => {
                // **`SplitScreen_ToParent` (`0x00437D65`) and
                // `SplitScreen_ToDaughter` (`0x00437E9E`)** — sixteen widgets in
                // eight rows, `g_uiHotspotId` carrying the slot. Row 7 is the
                // mercenary band and swaps whole rather than by one, in the
                // function itself.
                // arm: 0x00437D65/divide-to-parent
                // arm: 0x00437E9E/divide-to-daughter
                for row in 0..=MERC_ROW {
                    if parent_button(row).contains(x, y) {
                        self.row = row;
                        self.move_men(row, true, CLICK_MEN);
                        return Transition::Stay;
                    }
                    if daughter_button(row).contains(x, y) {
                        self.row = row;
                        self.move_men(row, false, CLICK_MEN);
                        return Transition::Stay;
                    }
                }
                // arm: 0x00437AFB/divide-confirm
                if SPLIT_TICK.contains(x, y) {
                    return self.split(ctx);
                }
                // The cross is the same handler reading `g_uiHotspotId == 0`,
                // and it lands on `0x04` rather than on the map.
                // arm: 0x00437AFB/divide-cancel
                if SPLIT_CROSS.contains(x, y) {
                    return Transition::Pop;
                }
                // `Ui_OkButtonClicked()` — the corner picture, and the third of
                // this screen's three exits. It also writes `g_screenId = 0x04`.
                // arm: 0x0040E7E4/divide-ok
                if OK.contains(x, y) {
                    return Transition::Pop;
                }
                Transition::Stay
            }
            _ => Transition::Stay,
        }
    }

    fn draw(&mut self, ctx: &Ctx, canvas: &mut Canvas) {
        let ink = &ctx.assets.ink;
        let pen = Pen {
            assets: &ctx.assets.shell,
            ink,
            chrome: ctx.assets.chrome.as_ref(),
            shadow: Some(font::SHADOW),
            caps: None,
        };
        let w = window();
        pen.window(canvas, w.x, w.y, BOX_COLS, BOX_ROWS, 0);
        pen.eng(canvas, GROUP, 0, 0x68, 0x44, font::TEXT);
        pen.eng(canvas, GROUP, 1, 0x78, 0x1AE, font::TEXT);

        let well = rows_well();
        canvas.fill_rect(well.x, well.y, well.w, well.h, ink.background);
        widget::frame(canvas, well, ink.border);

        let band = self.basket.mercenaries;
        for (row, troop) in ALL_TROOP_TYPES.iter().enumerate() {
            let y = row_y(row);
            let (left, right) =
                (self.basket.parent[troop.index()], self.basket.daughter[troop.index()]);
            let noun = ctx.assets.shell.text(8, NOUN_BASE + row * 2 + 1).to_string();
            let label =
                if noun.is_empty() { troop.name().to_uppercase() } else { noun.to_uppercase() };
            text::draw(
                canvas,
                NOUN_X,
                y,
                &label,
                if row == self.row { ink.highlight } else { ink.dim },
            );
            widget::button(canvas, ink, parent_button(row), ">", left > 0);
            text::draw_right(canvas, PARENT_NUMBER_X + 32, y, &format!("{left}"), ink.text);
            widget::button(canvas, ink, daughter_button(row), "<", right > 0);
            text::draw_right(canvas, DAUGHTER_NUMBER_X + 32, y, &format!("{right}"), ink.text);
        }

        // Row 7 — the band, drawn only when there is one, exactly as the
        // painter's `bVar1` gates it.
        if let Some(m) = band {
            let y = row_y(MERC_ROW);
            let nationality = ctx.assets.shell.text(16, m.band as usize).to_string();
            let label = if nationality.is_empty() {
                l2_kingdom::mercenary::ROSTER[m.band as usize].nationality.to_uppercase()
            } else {
                nationality.to_uppercase()
            };
            text::draw(
                canvas,
                NOUN_X,
                y,
                &format!("{} {}", label, m.troop.name().to_uppercase()),
                if self.row == MERC_ROW { ink.highlight } else { ink.dim },
            );
            let (left, right) = if self.basket.mercenaries_leave {
                (0, m.men())
            } else {
                (m.men(), 0)
            };
            widget::button(canvas, ink, parent_button(MERC_ROW), ">", left > 0);
            text::draw_right(canvas, PARENT_NUMBER_X + 32, y, &format!("{left}"), ink.text);
            widget::button(canvas, ink, daughter_button(MERC_ROW), "<", right > 0);
            text::draw_right(canvas, DAUGHTER_NUMBER_X + 32, y, &format!("{right}"), ink.text);
        }

        // The two "Total men" lines, on the row the painter picks.
        let ty = totals_y(band.is_some());
        text::draw(canvas, NOUN_X, ty, "TOTAL MEN", ink.text);
        text::draw_right(
            canvas,
            PARENT_NUMBER_X + 32,
            ty,
            &format!("{}", self.basket.parent_total()),
            ink.highlight,
        );
        text::draw_right(
            canvas,
            DAUGHTER_NUMBER_X + 32,
            ty,
            &format!("{}", self.basket.daughter_total()),
            ink.highlight,
        );

        widget::button(canvas, ink, OK, "OK", false);

        // `Widget_Draw(0, 0, &g_splitWidgets, …)` records 0 and 1 — `System.pl8`
        // frames 29 and 31, the tick and the cross. **They are the original's,
        // and they replace three buttons of ours** that stood where the painter
        // draws nothing: SPLIT, DISBAND and CANCEL in our own font at y 446,
        // one of which overlapped this tick.
        if !pen.system_frame(canvas, 29, SPLIT_TICK.x, SPLIT_TICK.y) {
            widget::button(canvas, ink, SPLIT_TICK, "OK", false);
        }
        if !pen.system_frame(canvas, 31, SPLIT_CROSS.x, SPLIT_CROSS.y) {
            widget::button(canvas, ink, SPLIT_CROSS, "X", false);
        }
        // **Ours**: the original answers a refusal with a message scroll.
        text::draw(canvas, 16, 452, &self.status, ink.dim);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The eight rows are inside the well the painter draws them in, and the
    /// totals line is below the last of them.
    #[test]
    fn the_eight_rows_and_both_totals_sit_inside_the_rows_well() {
        let (well, w) = (rows_well(), window());
        for row in 0..=MERC_ROW {
            let y = row_y(row);
            assert!(y >= well.y && y < well.y + well.h, "row {row} at {y} is outside the well");
            // The buttons are drawn at `y - 8`, so row 0's pair **overhangs the
            // well's top edge by eight pixels**. That is the painter's own
            // layout — `Ui_DrawBoxInterior(0x18, 0x80, …)` and the sprites at
            // `0x80 - 8` — and asserting they were inside it is what caught it.
            for b in [parent_button(row), daughter_button(row)] {
                assert!(w.contains(b.x, b.y), "row {row}'s button is off the window");
                assert!(w.contains(b.x + b.w - 1, b.y + b.h - 1));
            }
        }
        assert_eq!(parent_button(0).y, well.y - 8, "the first row's button overhangs the well");
        // Without a band the totals take row 7's own y exactly; with one they
        // sit four pixels below it.
        assert_eq!(totals_y(false), row_y(MERC_ROW));
        assert_eq!(totals_y(true) - row_y(MERC_ROW), 0x24);
        assert!(totals_y(true) < well.y + well.h, "the totals run off the well");
    }

    /// **The buttons are `g_splitWidgets`' own records**, transcribed rather
    /// than derived from the painter, and this is where that is pinned.
    ///
    /// The literals come from `node tools/oracle/widgets.js widgets 4dd388 18`
    /// against the player's own `Lords2.exe`: records 2 … 17 are `(256 | 288,
    /// row * 0x20 + 0x78)`, 24 square, alternating `SplitScreen_ToParent` and
    /// `SplitScreen_ToDaughter`, and records 0 and 1 are the tick and the
    /// cross.
    ///
    /// It replaces a test that asserted the buttons were *left of their own
    /// numbers*, which they were — because they were sitting on the troop-type
    /// icons at `0xA8` and `0x158`, 88 pixels from where the game hit-tests
    /// them. A geometry check derived from a painter agrees with a hit box
    /// derived from the same painter, and neither knows about the table.
    #[test]
    fn the_row_buttons_are_the_widget_tables_own_geometry() {
        for row in 0..=MERC_ROW {
            let (p, d) = (parent_button(row), daughter_button(row));
            assert_eq!((p.x, p.w, p.h), (256, 24, 24), "row {row} parent");
            assert_eq!((d.x, d.w, d.h), (288, 24, 24), "row {row} daughter");
            assert_eq!(p.y, row as i32 * 0x20 + 0x78, "row {row} y");
            assert_eq!(d.y, p.y);
            // The gutter the table puts them in: right of the parent's number,
            // left of the daughter's icon.
            assert!(PARENT_NUMBER_X < p.x, "row {row}: the button covers its number");
            assert!(d.x + d.w <= DAUGHTER_ICON_X, "row {row}: the button covers the icon");
        }
    }

    /// **Nothing this screen hit-tests overlaps anything else it hit-tests.**
    ///
    /// The three buttons this replaced — SPLIT, DISBAND and CANCEL at y 446 —
    /// had a test of their own that *passed*: it asserted they were left of
    /// `OK`, calling `OK` *"the original's tick"*. `OK` is the corner picture at
    /// (428, 436); the tick is record 0 of `g_splitWidgets` at (288, 420), and
    /// CANCEL at (264, 446) 100 × 18 overlapped it in a 32 × 6 strip. **Our
    /// cancel sat on the original's confirm.** The check was right about the
    /// rectangle it named and the rectangle it named was not the one it meant.
    ///
    /// So this one enumerates every box the screen tests and compares them
    /// pairwise, which has no rectangle to name wrongly.
    #[test]
    fn no_two_hotspots_on_this_screen_overlap() {
        let mut boxes: Vec<(String, Rect)> = vec![
            ("tick".into(), SPLIT_TICK),
            ("cross".into(), SPLIT_CROSS),
            ("ok".into(), OK),
        ];
        for row in 0..=MERC_ROW {
            boxes.push((format!("parent {row}"), parent_button(row)));
            boxes.push((format!("daughter {row}"), daughter_button(row)));
        }
        for (i, (an, a)) in boxes.iter().enumerate() {
            assert!(
                a.x >= 0 && a.y >= 0 && a.x + a.w <= 640 && a.y + a.h <= 480,
                "{an} {a:?} is off the screen",
            );
            for (bn, b) in boxes.iter().skip(i + 1) {
                let hit = a.x < b.x + b.w && b.x < a.x + a.w && a.y < b.y + b.h && b.y < a.y + a.h;
                assert!(!hit, "{an} {a:?} overlaps {bn} {b:?}");
            }
        }
    }
}

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
//!     misc_cty frame 0x2F + t at (0xA8, y - 8)          the parent's button
//!     Ui_DrawNumber(basket[t].chosen,    0xD8, y)       who stays
//!     misc_cty frame 0x2F + t at (0x158, y - 8)         the daughter's button
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
//! # Disbanding is on this screen, and in the original it is not
//!
//! `Panel_DisbandButton` (`0x0043733A`) is a button on the **unit panel**, the
//! three-hotspot strip `Map_Click` opens over an army — move, split, disband.
//! We have no unit panel: our map click goes straight to move-order mode, which
//! is what `Panel_MoveButton` does with two of the three buttons unbuilt. So
//! the disband button is here, in our own font, below the original's window,
//! and it says so. The *rule* it drives is the original's, `L2.eng` group 145
//! included — [`l2_kingdom::divide::disband_county`] picks the county and
//! refuses exactly where `Panel_DisbandButton` refuses.
//!
//! # The gate that only the Readme states
//!
//! *"An army normally can only be split only at the start of its movement in a
//! turn."* — `FUN_004378B3` refuses with message `0x95` when `movesUsed >= 1`,
//! and the screen never opens. Ours opens and says why, because a screen that
//! silently will not appear is a screen a player thinks is broken.

use l2_kingdom::divide::{SplitBasket, SplitInto, SplitRefusal};
use l2_kingdom::unit::{ALL_TROOP_TYPES, TroopType};
use l2_kingdom::DisbandRefusal;
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

/// The noun's x, and the two columns' button and number positions.
pub const NOUN_X: i32 = 0x18;
pub const PARENT_BUTTON_X: i32 = 0xA8;
pub const PARENT_NUMBER_X: i32 = 0xD8;
pub const DAUGHTER_BUTTON_X: i32 = 0x158;
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

/// One column's button on one row. **The 24 × 24 box is ours** — the painter
/// draws a `misc_cty` sprite whose frame size is not in any table we have read
/// — and the origin is the painter's `(0xA8 | 0x158, y - 8)`.
pub const BUTTON_DIM: i32 = 24;

pub fn parent_button(row: usize) -> Rect {
    Rect::new(PARENT_BUTTON_X, row_y(row) - 8, BUTTON_DIM, BUTTON_DIM)
}

pub fn daughter_button(row: usize) -> Rect {
    Rect::new(DAUGHTER_BUTTON_X, row_y(row) - 8, BUTTON_DIM, BUTTON_DIM)
}

/// How many men a click moves. **Ours**: the original's button is one at a
/// time, which is unusable for an army of 800 without the key-repeat its
/// hotspot table supplies and our event loop does not deliver here yet. Ten is
/// [`l2_kingdom::divide`]'s own round number, and the arrow keys still move
/// one.
pub const CLICK_MEN: i32 = 10;

/// **Ours.** The window runs to y 464 and the screen is 480 tall, so there is
/// nowhere below it to stand: these sit in the band between the rows' well
/// (which ends at 416) and the window's own bottom edge, beside the original's
/// tick rather than over it. They are drawn in our 5 × 7 font and framed as our
/// own buttons, which is what keeps them distinguishable from the painter's.
pub const OURS_Y: i32 = 446;
pub fn split_button() -> Rect {
    Rect::new(16, OURS_Y, 116, 18)
}
pub fn disband_button() -> Rect {
    Rect::new(140, OURS_Y, 116, 18)
}
pub fn cancel_button() -> Rect {
    Rect::new(264, OURS_Y, 100, 18)
}

/// What the screen did before it closed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Divided {
    None,
    /// The daughter army is on the map, in this slot.
    Split(usize),
    /// The men went home: the county they joined and how many.
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

    /// `Panel_DisbandButton` then `Army_DisbandConfirm`.
    fn disband(&mut self, ctx: &mut Ctx) -> Transition {
        match ctx.game.disband_army(self.unit) {
            Ok((county, men)) => {
                self.outcome = Divided::Disbanded(county, men);
                Transition::Pop
            }
            Err(no) => {
                self.outcome = Divided::Refused;
                self.status = match no {
                    DisbandRefusal::NotAnArmy => "THAT IS NOT YOUR ARMY".into(),
                    // Message 0x91 = L2.eng group 145, and it states the rule.
                    DisbandRefusal::NowhereToGo => {
                        "MARCH IT TO A COUNTY YOU RULE BEFORE DISBANDING".into()
                    }
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
            Event::KeyDown(Key::Char('D')) => self.disband(ctx),
            Event::Click { x, y } => {
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
                if split_button().contains(x, y) {
                    return self.split(ctx);
                }
                if disband_button().contains(x, y) {
                    return self.disband(ctx);
                }
                if cancel_button().contains(x, y) || OK.contains(x, y) {
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

        // ---- ours -------------------------------------------------------
        widget::button(canvas, ink, split_button(), "SPLIT", false);
        widget::button(canvas, ink, disband_button(), "DISBAND", false);
        widget::button(canvas, ink, cancel_button(), "CANCEL", false);
        text::draw(canvas, 16, OURS_Y - 18, &self.status, ink.dim);
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

    /// The two columns' buttons never overlap, and neither reaches the numbers.
    #[test]
    fn the_two_columns_of_buttons_are_apart_and_clear_of_their_numbers() {
        for row in 0..=MERC_ROW {
            let (p, d) = (parent_button(row), daughter_button(row));
            assert!(p.x + p.w <= PARENT_NUMBER_X, "row {row}: the button covers its number");
            assert!(PARENT_NUMBER_X < d.x, "row {row}: the columns overlap");
            assert!(d.x + d.w <= DAUGHTER_NUMBER_X);
        }
    }

    /// Our three buttons are clear of everything the painter draws: below the
    /// rows' well, left of the original's tick, and inside the screen.
    #[test]
    fn our_own_buttons_do_not_sit_on_anything_the_painter_drew() {
        let (w, well) = (window(), rows_well());
        for r in [split_button(), disband_button(), cancel_button()] {
            assert!(r.y >= well.y + well.h, "{r:?} covers the troop rows");
            assert!(r.x + r.w <= OK.x, "{r:?} runs into the original's tick");
            assert!(r.y + r.h <= w.y + w.h, "{r:?} runs off the window");
            assert!(r.x + r.w <= 640 && r.y + r.h <= 480);
        }
        assert!(w.contains(OK.x, OK.y), "the tick is the original's and is inside it");
    }
}

//! **The army-division screen** — `Screen_ArmyDivision` (`0x004192B1`) and
//! `Screen_SplitArmyRows` (`0x00419354`), `g_screenId` `0x11`, `L2.eng` group
//! 17.
//!
//! # The painter, address by address
//!
//! **Four of the twenty-five draw calls are in `Screen_ArmyDivision`.** The
//! other twenty-one are in `Screen_SplitArmyRows`, which the painter calls once
//! *and* `Screen_DrawWidgets` calls again on **every frame** — so the rows are
//! redrawn continuously and the window behind them is not. Reading only the
//! painter gives you a window with a title and nothing in it.
//!
//! ```text
//! Screen_ArmyDivision():                                        0x004192B1
//!   File_ReadChunk("icon_tmp.pl8", DAT_004EABEC, 160000)
//!   Ui_DrawBox(8, 0x30, 0x1C, 0x1A)              the window, 448 x 416 at (8, 48)
//!   Ui_OkButton(0x1AC, 0x1B4, 0)                                 (428, 436)
//!   Sprite_WGenSprite(0x2B, 0x18, 0x40)     icon_tmp frame 43 at (24, 64)
//!   Eng_DrawString(17, 0, 0x68, 0x44, HEADING)  "Army Division."  (104, 68)
//!   Eng_DrawString(17, 1, 0x78, 0x1AE, body)    "Split the army?" (120, 430)
//!   Screen_SplitArmyRows()
//!
//! Screen_SplitArmyRows():                                       0x00419354
//!   band = basket[7].chosen != 0 || basket[7].available != 0
//!   Ui_DrawBoxInterior(0x18, 0x80, 0x1A, 0x12)   the rows' well, 416 x 288
//!   for t in 0..7:   y = 0x80 + t * 0x20                      128 … 320
//!     Ui_DrawUnitNoun(2, 0x34 + t*2, 0x18, y, body)  the troop's PLURAL — the
//!                                       count is the literal 2, always
//!     Pl8_DrawFrame(Misc_cty, 0x2F + t, 0xA8, y - 8)    the type's icon (168)
//!     Ui_DrawNumber(basket[t].chosen,    '@', 0xD8, y, body)   who stays (216)
//!     Pl8_DrawFrame(Misc_cty, 0x2F + t, 0x158, y - 8)   the same icon  (344)
//!     Ui_DrawNumber(basket[t].available, '@', 0x188, y, body)  who goes (392)
//!   if band:                              row 7, y = 0x160 = 352
//!     one number only, in the column that holds it:
//!       chosen != 0 -> Ui_DrawNumber(chosen,    0xD8,  y)
//!       else        -> Ui_DrawNumber(available, 0x188, y)
//!     Eng_DrawString(16, unit.mercBand, 0x18, y, body)     the nationality
//!     Ui_DrawUnitNoun(chosen, 0x34 + mercTroop*2, 0x58, y + 0x10, body)
//!                                   the troop noun on a SECOND line, indented
//!   Ui_DrawUnitNoun(1, 0x48, 0x18, totals)        group 8/72 "Total men"
//!   Ui_DrawNumber(DAT_00554468, 0xD8,  totals)    parent total
//!   Ui_DrawNumber(DAT_00554040, 0x188, totals)    daughter total
//!         totals = 0x184 with a band, 0x160 without
//! ```
//!
//! `Sprite_WGenSprite(0x2B, …)` reads `DAT_004EABEC`, which is the buffer this
//! painter has just read `icon_tmp.pl8` into — so it is **frame 43 of
//! `Icon_tmp.pl8`**, and `screens/info.rs` identified that frame independently
//! by rendering it: [`crate::screens::info::icon::SPLIT`], the split button on
//! the unit panel. Two readings from opposite directions agreeing, which is
//! what makes [`SPLIT_ICON`] `[V]` rather than a guess.
//!
//! # The rows are drawn by nothing you can click, and clicked on nothing drawn
//!
//! This was the finding. `DAT_004DD388` is **eighteen** 24-byte records and
//! `Widget_Draw`/`Widget_Test` are passed `DAT_0055321C`, which
//! `Panel_SplitButton` (`0x004378B3`) and `FUN_004376BB` both set to `0x10`
//! for an army with no mercenary band and `0x12` for one with. Sixteen or
//! eighteen — the table's own length, so nothing hides behind the count here.
//!
//! ```text
//!  0  (288, 420)  frame 29  Army_SplitConfirm   hotspot 1   the tick
//!  1  (336, 424)  frame 31  Army_SplitConfirm   hotspot 0   the cross
//!  2..17  pairs at y 120 + 32n, n = 0..7:
//!         (256, y)  frame 27  SplitScreen_ToParent    hotspot n
//!         (288, y)  frame 25  SplitScreen_ToDaughter  hotspot n
//! ```
//!
//! **The arrows are at x 256 and 288, between the two numbers** — not on the
//! `misc_cty` icons at 168 and 344, which are decoration naming the troop type.
//! This module used to put its own `>` and `<` buttons on the icons, 88 and 56
//! pixels from where the original tests, so every click landed on a picture and
//! nothing was where the game puts it. The arrow y is `row_y(n) - 8`, the same
//! eight-pixel overhang the icons have.
//!
//! **The tick and the cross are the split and the cancel**, and
//! `Army_SplitConfirm` (`0x00437AFB`) reads the hotspot id to tell them apart.
//! Our own SPLIT and CANCEL buttons were invented, and the CANCEL one sat *on
//! top of* the tick's hit box.
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
//!
//! # What is still ours here
//!
//! * **DISBAND.** `Panel_DisbandButton` is a button on the unit panel we have
//!   not built; see above.
//! * **The status line** under the window, in our 5 × 7 font, and the keyboard
//!   row highlight. Diagnostics, named as ours at the site.
//! * **[`CLICK_MEN`]** — the original's arrow moves *one* man.
//!
//! Nothing else on this screen is an invention any more, and no English word we
//! wrote ourselves is drawn where the original fetches an `L2.eng` string.
//!
//! # The denominator, for the draw audit
//!
//! **Twenty-three** call sites of the 26 pixel primitives — five in
//! `Screen_ArmyDivision` and eighteen in `Screen_SplitArmyRows` — plus sixteen
//! or eighteen widget records. We draw all twenty-three and all of them.
//!
//! `tools/audit/draws-B.json` reports 23 where `draws.js --count --tree` says
//! **25**, and the two pixels of difference are `Sprite_WGenSprite`: the script
//! counts the three `Blit_ClippedLeft`/`Right`/`Unclipped` sites inside it, and
//! the audit's denominator excludes blitters, so the sprite is one draw.
//!
//! Excluded: `File_ReadChunk("icon_tmp.pl8")`, `FUN_004B1DE0`,
//! `Gfx_MarkAllDirty`, `Gfx_MarkSpriteDirty` and `Blit_*`.
//!
//! **This screen draws no `L2.eng` group 31**, so it does not resource
//! `docs/armies.md`'s `[V]` on unit `+0x166` against 31/21 *"Morale"* — and it
//! is the screen a reader would expect to, because it is the one that takes an
//! army apart.

use l2_kingdom::divide::{SplitBasket, SplitInto, SplitRefusal};
use l2_kingdom::unit::{ALL_TROOP_TYPES, TroopType};
use l2_kingdom::DisbandRefusal;
use l2_view::{text, Canvas};

use crate::input::{Event, Key, Rect};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::shell::{font, Pen};
use crate::widget;

/// `L2.eng` group 17 — **the whole group, two strings, both drawn here.**
///
/// Verified against the words in the player's own file and not merely against
/// the indices existing, which is the check that passes on a wrong group:
/// `17/0` is *"Army Division."* and `17/1` is *"Split the army?"*.
pub const GROUP: usize = 17;
pub const TITLE: usize = 0;
pub const SPLIT_QUESTION: usize = 1;

/// `L2.eng` group 8's troop nouns begin at index `0x34`, two apiece: singular
/// then plural. `Ui_DrawUnitNoun(count, 0x34 + t*2)` picks between them —
/// **and this screen passes the literal `2` for every troop row**, so the seven
/// nouns are always plural however many men are in the column.
pub const NOUN_GROUP: usize = 8;
pub const NOUN_BASE: usize = 0x34;

/// `Ui_DrawUnitNoun(1, 0x48, …)` — group 8 index **72**, *"Total men"*. The
/// count is the literal 1, so the singular arm is taken and index 73 (the same
/// words) is never reached. Verified against the words.
pub const TOTAL_MEN_NOUN: usize = 0x48;

/// `L2.eng` group 16 — the twelve mercenary nationalities, indexed by the
/// army's `mercBand`. Verified against the words: `16/1` is *"Scottish"*.
/// Index 0 is *"No mercenaries in the army."* and the painter's `band` guard
/// means row 7 is never drawn for a band of 0, so index 0 is unreachable here.
pub const GROUP_NATIONALITY: usize = 16;

/// `Sprite_WGenSprite(0x2B, 0x18, 0x40)` — frame 43 of the `icon_tmp.pl8` this
/// painter has just loaded, in the window's top-left corner. It is the **split
/// button's own icon**, the same frame `screens/info.rs` identified by
/// rendering it. `[V]` from two directions.
pub const ICON_SHEET: &str = "Icon_tmp.pl8";
pub const SPLIT_ICON: usize = 0x2B;
pub const SPLIT_ICON_AT: (i32, i32) = (0x18, 0x40);

/// `Pl8_DrawFrame(g_miscCtySheet, 0x2F + t, …)` — the troop type's icon, drawn
/// **twice per row**, once in each column. `Misc_cty.pl8` frames `0x30`…`0x35`
/// are the six weapon icons the court draws (`screens/court.rs`), so `0x2F` is
/// the peasant and the seven run `0x2F`…`0x35`.
pub const ROW_ICON_BASE: usize = 0x2F;
pub const PARENT_ICON_X: i32 = 0xA8;
pub const DAUGHTER_ICON_X: i32 = 0x158;

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
pub const PARENT_NUMBER_X: i32 = 0xD8;
pub const DAUGHTER_NUMBER_X: i32 = 0x188;
/// The band's troop noun, on a second line indented to `0x58` — the only row
/// of the eight that is two lines tall.
pub const BAND_NOUN_X: i32 = 0x58;
pub const BAND_NOUN_DY: i32 = 0x10;

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

/// The arrow pair, `DAT_004DD388` records 2…17: **24 pixels square at x 256
/// and x 288**, on `row_y(row) - 8`, carrying the row as the hotspot id.
pub const BUTTON_DIM: i32 = 24;
pub const TO_PARENT_X: i32 = 256;
pub const TO_DAUGHTER_X: i32 = 288;
/// The button-sheet frames the two records carry. Frame 27 is the record whose
/// handler is `SplitScreen_ToParent` (`0x00437D65`) and 25 is
/// `SplitScreen_ToDaughter` (`0x00437E9E`); the pair is not one `widgets.js`
/// documents, so the *names* come from the handlers rather than from the
/// pictures, which is the direction that cannot be got backwards — see
/// `armoury.rs` on frames 68 and 66.
pub const TO_PARENT_FRAME: usize = 27;
pub const TO_DAUGHTER_FRAME: usize = 25;

/// `SplitScreen_ToDaughter` — one man out of the parent's column.
pub fn to_daughter_button(row: usize) -> Rect {
    Rect::new(TO_DAUGHTER_X, row_y(row) - 8, BUTTON_DIM, BUTTON_DIM)
}

/// `SplitScreen_ToParent` — one man back from the daughter's.
pub fn to_parent_button(row: usize) -> Rect {
    Rect::new(TO_PARENT_X, row_y(row) - 8, BUTTON_DIM, BUTTON_DIM)
}

/// The old name for [`to_daughter_button`], kept because
/// `crates/l2-game/tests/military.rs` calls it. It never named the widget: the
/// record's handler is `SplitScreen_ToDaughter`.
pub fn parent_button(row: usize) -> Rect {
    to_daughter_button(row)
}

/// The old name for [`to_parent_button`], kept for the same reason.
pub fn daughter_button(row: usize) -> Rect {
    to_parent_button(row)
}

/// `DAT_004DD388` records 0 and 1 — the tick and the cross, 32 pixels square,
/// both running `Army_SplitConfirm` (`0x00437AFB`) and told apart by the
/// hotspot id: 1 splits, 0 closes the screen. Frames 29 and 31 are the tick and
/// the cross, which `tools/oracle/widgets.js` documents and four other tables
/// in the binary agree with.
pub const CONFIRM_DIM: i32 = 32;
pub const CONFIRM_FRAME: usize = 29;
pub const CANCEL_FRAME: usize = 31;

/// The tick. Named `split_button` because that is what the tree already calls
/// it; it is a widget record, not a button of ours.
pub fn split_button() -> Rect {
    Rect::new(288, 420, CONFIRM_DIM, CONFIRM_DIM)
}

/// The cross.
pub fn cancel_button() -> Rect {
    Rect::new(336, 424, CONFIRM_DIM, CONFIRM_DIM)
}

/// `Widget_Draw(0, 0, &DAT_004DD388, DAT_0055321C)` — the count both
/// `Panel_SplitButton` and `FUN_004376BB` publish before setting the screen id:
/// `0x10` for an army with no band, `0x12` for one with. The table is eighteen
/// records long, so **the count reaches the end of it** and nothing is hidden
/// behind a short count here — unlike `g_sendSuppliesWidgets`.
pub const WIDGETS_NO_BAND: usize = 0x10;
pub const WIDGETS_WITH_BAND: usize = 0x12;
pub const WIDGET_RECORDS: usize = 18;

/// How many men a click moves. **Ours**: the original's button is one at a
/// time, which is unusable for an army of 800 without the key-repeat its
/// hotspot table supplies and our event loop does not deliver here yet. Ten is
/// [`l2_kingdom::divide`]'s own round number, and the arrow keys still move
/// one.
pub const CLICK_MEN: i32 = 10;

/// **Ours**, and the only one left: `Panel_DisbandButton` is a button on the
/// unit panel we have not built. It sits in the band between the rows' well
/// (which ends at 416) and the window's bottom edge, **left of x 256** so that
/// it is clear of both arrow columns and of the tick and the cross, and it is
/// drawn in our 5 × 7 font and framed as our own button so that it cannot be
/// mistaken for the painter's.
pub const OURS_Y: i32 = 446;
pub fn disband_button() -> Rect {
    Rect::new(16, OURS_Y, 116, 18)
}

/// **Ours.** One line of feedback, below the window entirely — the sixteen
/// pixels the 448 × 416 box leaves at the foot of a 480-line screen.
pub const STATUS_AT: (i32, i32) = (16, 466);

/// `Eng_DrawString(group, index, x, y, &g_fontHeading, 0x3F)`. [`Pen`] has
/// `eng` (body) and `eng_heading_centred` but no plain heading one, and adding
/// a method to `shell/mod.rs` is not this module's to do.
fn eng_heading(pen: &Pen, canvas: &mut Canvas, group: usize, index: usize, x: i32, y: i32) -> i32 {
    let s = pen.assets.text(group, index).to_string();
    pen.heading(canvas, x, y, &s, font::TEXT)
}

/// `Ui_DrawUnitNoun(count, index, x, y, body, 0x3F)` (`0x0041AC3E`) — group 8
/// at `index`, with our own troop name when `L2.eng` is not installed.
///
/// **It is not `Ui_DrawCount`.** That one takes the singular at ±1; this takes
/// it only at +1, so a column of −1 men would read *"-1 Pikemen"*. The caller
/// passes the index it has already chosen, because the painter's two call sites
/// choose it from different counts.
fn noun(
    pen: &Pen,
    canvas: &mut Canvas,
    index: usize,
    troop: &TroopType,
    count: i32,
    x: i32,
    y: i32,
) -> i32 {
    let s = pen.assets.text(NOUN_GROUP, index).to_string();
    let s = if !s.is_empty() {
        s
    } else if count == 1 {
        troop.name().to_string()
    } else {
        format!("{}s", troop.name())
    };
    pen.body(canvas, x, y, &s, font::TEXT)
}

/// One row's pair of arrow records, frames 27 and 25.
fn arrows(pen: &Pen, canvas: &mut Canvas, row: usize) {
    let (to_parent, to_daughter) = (to_parent_button(row), to_daughter_button(row));
    if !pen.system_frame(canvas, TO_PARENT_FRAME, to_parent.x, to_parent.y) {
        crate::widget::frame(canvas, to_parent, pen.ink.border);
    }
    if !pen.system_frame(canvas, TO_DAUGHTER_FRAME, to_daughter.x, to_daughter.y) {
        crate::widget::frame(canvas, to_daughter, pen.ink.border);
    }
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
                // `Widget_Test(0, 0, &DAT_004DD388, DAT_0055321C)` — records
                // 2…17, the hotspot id being the row.
                for row in 0..=MERC_ROW {
                    if to_daughter_button(row).contains(x, y) {
                        self.row = row;
                        self.move_men(row, true, CLICK_MEN);
                        return Transition::Stay;
                    }
                    if to_parent_button(row).contains(x, y) {
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
        let a = &ctx.assets.shell;
        let w = window();
        pen.window(canvas, w.x, w.y, BOX_COLS, BOX_ROWS, 0);

        // `Ui_OkButton(0x1AC, 0x1B4, 0)` — mode 0, System.pl8 frame 0x33.
        pen.ok_button(canvas, OK.x, OK.y, 0);

        // `Sprite_WGenSprite(0x2B, 0x18, 0x40)` out of the `icon_tmp.pl8` the
        // painter loads two lines earlier.
        if let Some(f) = a.sheet(ICON_SHEET).and_then(|s| s.frame(SPLIT_ICON)) {
            canvas.blit(&f, SPLIT_ICON_AT.0, SPLIT_ICON_AT.1);
        }

        // 17/0 is the **heading** font and 17/1 the body one.
        eng_heading(&pen, canvas, GROUP, TITLE, 0x68, 0x44);
        pen.eng(canvas, GROUP, SPLIT_QUESTION, 0x78, 0x1AE, font::TEXT);

        // `Ui_DrawBoxInterior(0x18, 0x80, 0x1A, 0x12)` — the parchment on its
        // own, with no border. `Screen_DrawWidgets` repaints it every frame,
        // which is what stops the numbers smearing.
        pen.box_interior(canvas, 0x18, 0x80, 0x1A, 0x12);

        let band = self.basket.mercenaries.filter(|m| m.men() > 0);
        for (row, troop) in ALL_TROOP_TYPES.iter().enumerate() {
            let y = row_y(row);
            let (left, right) =
                (self.basket.parent[troop.index()], self.basket.daughter[troop.index()]);

            // `Ui_DrawUnitNoun(2, 0x34 + t*2, …)` — the literal 2 means the
            // plural is drawn even for a column of one man.
            noun(&pen, canvas, NOUN_BASE + row * 2 + 1, troop, 2, NOUN_X, y);

            pen.misc_frame(canvas, ROW_ICON_BASE + row, PARENT_ICON_X, y - 8);
            pen.number(canvas, PARENT_NUMBER_X, y, left, true, font::TEXT);
            pen.misc_frame(canvas, ROW_ICON_BASE + row, DAUGHTER_ICON_X, y - 8);
            pen.number(canvas, DAUGHTER_NUMBER_X, y, right, true, font::TEXT);

            arrows(&pen, canvas, row);
            if row == self.row {
                // **Ours**: which row the keyboard is on. The original has no
                // keyboard here at all.
                canvas.fill_rect(NOUN_X - 4, y, 2, 14, ink.highlight);
            }
        }

        // Row 7 — the band. `bVar1` is `basket[7].chosen != 0 ||
        // basket[7].available != 0`, and the painter draws **one** number, in
        // whichever column holds it.
        if let Some(m) = band {
            let y = row_y(MERC_ROW);
            let (left, right) =
                if self.basket.mercenaries_leave { (0, m.men()) } else { (m.men(), 0) };
            if left != 0 {
                pen.number(canvas, PARENT_NUMBER_X, y, left, true, font::TEXT);
            } else {
                pen.number(canvas, DAUGHTER_NUMBER_X, y, right, true, font::TEXT);
            }
            let s = a.text(GROUP_NATIONALITY, m.band as usize).to_string();
            let s = if s.is_empty() {
                l2_kingdom::mercenary::ROSTER[m.band as usize].nationality.to_string()
            } else {
                s
            };
            pen.body(canvas, NOUN_X, y, &s, font::TEXT);
            // The noun is on a second line, indented, and the count it is
            // chosen by is the **parent's** on both arms of the painter.
            noun(
                &pen,
                canvas,
                NOUN_BASE + m.troop.index() * 2 + usize::from(left != 1),
                &m.troop,
                left,
                BAND_NOUN_X,
                y + BAND_NOUN_DY,
            );
            arrows(&pen, canvas, MERC_ROW);
            if self.row == MERC_ROW {
                canvas.fill_rect(NOUN_X - 4, y, 2, 14, ink.highlight);
            }
        }

        // `Ui_DrawUnitNoun(1, 0x48, …)` — group 8/72, and the two totals.
        let ty = totals_y(band.is_some());
        let s = a.text(NOUN_GROUP, TOTAL_MEN_NOUN).to_string();
        let s = if s.is_empty() { "Total men".to_string() } else { s };
        pen.body(canvas, NOUN_X, ty, &s, font::TEXT);
        pen.number(canvas, PARENT_NUMBER_X, ty, self.basket.parent_total(), true, font::TEXT);
        pen.number(canvas, DAUGHTER_NUMBER_X, ty, self.basket.daughter_total(), true, font::TEXT);

        // Records 0 and 1: the tick and the cross.
        let (yes, no) = (split_button(), cancel_button());
        if !pen.system_frame(canvas, CONFIRM_FRAME, yes.x, yes.y) {
            widget::frame(canvas, yes, ink.highlight);
        }
        if !pen.system_frame(canvas, CANCEL_FRAME, no.x, no.y) {
            widget::frame(canvas, no, ink.border);
        }

        // ---- ours -------------------------------------------------------
        // The disband button has no counterpart on this screen: it belongs to
        // `Panel_DisbandButton` on the unit panel, which we have not built.
        widget::button(canvas, ink, disband_button(), "DISBAND", false);
        text::draw(canvas, STATUS_AT.0, STATUS_AT.1, &self.status, ink.dim);
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
            for b in [to_parent_button(row), to_daughter_button(row)] {
                assert!(w.contains(b.x, b.y), "row {row}'s button is off the window");
                assert!(w.contains(b.x + b.w - 1, b.y + b.h - 1));
            }
        }
        assert_eq!(to_parent_button(0).y, well.y - 8, "the first row's button overhangs the well");
        // Without a band the totals take row 7's own y exactly; with one they
        // sit four pixels below it.
        assert_eq!(totals_y(false), row_y(MERC_ROW));
        assert_eq!(totals_y(true) - row_y(MERC_ROW), 0x24);
        assert!(totals_y(true) < well.y + well.h, "the totals run off the well");
    }

    /// **The arrows are between the two numbers, not on the two icons.** This
    /// is the assertion the old layout failed: it put the pair at `0xA8` and
    /// `0x158`, which are the `misc_cty` decorations, 88 and 56 pixels from
    /// where `Widget_Test` looks. The numbers here are pinned from the
    /// decompilation rather than computed off the constants they check.
    #[test]
    fn the_arrows_sit_between_the_two_numbers_where_the_widget_table_puts_them() {
        for row in 0..=MERC_ROW {
            let (p, d) = (to_parent_button(row), to_daughter_button(row));
            assert_eq!((p.x, d.x), (256, 288), "DAT_004DD388 records 2..17");
            assert_eq!(p.y, 120 + 32 * row as i32, "the table's own y for row {row}");
            assert_eq!(d.y, p.y);
            assert_eq!((p.w, p.h), (24, 24), "the records' size byte is 24");
            // Between the parent's number and the daughter's, and clear of the
            // troop icons in both columns.
            assert!(PARENT_NUMBER_X < p.x, "row {row}: the arrows cover the parent's number");
            assert!(d.x + d.w <= DAUGHTER_ICON_X, "row {row}: the arrows cover the second icon");
            assert!(PARENT_ICON_X + 24 <= p.x, "row {row}: the arrows cover the first icon");
        }
    }

    /// The tick and the cross are the original's records and our own button is
    /// clear of both of them. **The old CANCEL button of ours overlapped the
    /// tick**, so a click on the original's split control cancelled instead.
    #[test]
    fn the_confirm_pair_is_the_widget_tables_and_our_button_is_clear_of_it() {
        let (yes, no) = (split_button(), cancel_button());
        assert_eq!((yes.x, yes.y), (288, 420), "DAT_004DD388 record 0");
        assert_eq!((no.x, no.y), (336, 424), "record 1");
        assert!(yes.x + yes.w <= no.x, "the tick and the cross overlap");
        let (w, well) = (window(), rows_well());
        let ours = disband_button();
        assert!(ours.y >= well.y + well.h, "{ours:?} covers the troop rows");
        assert!(ours.y + ours.h <= w.y + w.h, "{ours:?} runs off the window");
        for r in [yes, no, OK] {
            assert!(
                ours.x + ours.w <= r.x || r.x + r.w <= ours.x || ours.y + ours.h <= r.y
                    || r.y + r.h <= ours.y,
                "our disband button overlaps {r:?}",
            );
        }
        assert!(w.contains(OK.x, OK.y), "the corner picture is the original's and is inside it");
        // And the status line is below the window entirely.
        assert!(STATUS_AT.1 >= w.y + w.h, "our status line is on top of the painter's window");
        assert!(STATUS_AT.1 + 7 <= 480);
    }

    /// The widget count reaches the end of the table, both ways — which is what
    /// `g_sendSuppliesWidgets` does not do and is why every table in this audit
    /// is decoded rather than trusted.
    #[test]
    fn the_widget_count_covers_the_whole_table() {
        assert_eq!(WIDGETS_WITH_BAND, WIDGET_RECORDS, "0x12 is the table's own length");
        assert_eq!(WIDGETS_NO_BAND, WIDGET_RECORDS - 2, "one arrow pair short: row 7");
        // Two confirm records plus one pair per row.
        assert_eq!(WIDGET_RECORDS, 2 + 2 * (MERC_ROW + 1));
    }
}

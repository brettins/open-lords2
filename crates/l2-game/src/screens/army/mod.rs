//! **The raise-army screen** — `Screen_RaiseArmy` (`0x00418653`), `g_screenId`
//! `0x17`, `L2.eng` group 69.
//!
//! # The name was wrong, and the name was the problem
//!
//! `screens/shells.rs` called this *"Hire mercenaries"* while
//! `docs/symbols.json` called its painter **`Screen_RaiseArmy`**. Both halves
//! matter and the shell table had the wrong one
//! screen in the game at all.** The mercenary offer is a block on *this*
//! screen, below the levy slider, and this screen is the only door to the
//! armoury a player has. Filing it as optional content is what let the most
//! gameplay-critical shell in the table sit there for weeks;
//! `docs/decisions.md` C45 records it as the fifth correction of the form *a
//! name is a claim*, and the first where the cost was priority.
//! wrong belief about a rule.
//!
//! # It is a window on the armoury, and that was the second finding
//!
//! ```c
//! else if (g_screenId == '\x17') { if (firstFrame == 1) Screen_Armoury(1);
//!                                  Screen_RaiseArmy(); }
//! ```
//!
//! **This screen has no background of its own.** `Screen_Draw` paints the
//! armoury under it and `Screen_Armoury` ends with `Palette_Set(armoury.256)`,
//! so the levy window is a `Ui_DrawBox` over a forge, in the armoury's palette
//! — not a popup over the campaign map in the campaign palette, which is what
//! this module used to answer and what a player was looking at when he called
//! it *"a weird popup"*. [`crate::screens::armoury::page`] is the painter both
//! screens share
//!
//! # What the painter draws, read out of it
//!
//! Two layouts, chosen by whether the county has a standing offer at `+0x1AD`.
//! Everything is placed relative to one local, which is `0x80` with an offer
//! and `0xA0` without — [`base`] — and the window is `rows + 1` cells tall for
//! `rows` of `0x10` or `0x0D`:
//!
//! ```text
//! Screen_RaiseArmy():                                            0x00418653
//!   DAT_00522F58 = offer ? (gold >= price ? 3 : 1) : 1   publish the widget count
//!   Ui_DrawBox(0x50, base - 0x10, 0x1E, rows + 1)
//!   Eng_DrawString(69, 0x10, 0x70, base - 8, body)     "Raising an army in"
//!   Eng_DrawString(100, scenario*20 + county, pen, base - 8)   + the county's name
//!   Ui_DrawInsetRect(0xC3, base + 0x28, 0x6F, 4)          the slider's well
//!   system 0x51 (0x80, base+0x10)   0x4E (0xB9, base+0x21)  the left icon and arrow
//!   system 0x50 (levyPercent + 0xC4, base + 0x1A)              THE KNOB
//!   system 0x4F (0x132, base+0x21)  0x52 (0x145, base+0x10) the right arrow and icon
//!   Ui_DrawNumber(population - levyMen, '@', 0x80,  base + 0x44)   who stays
//!   Ui_DrawNumber(levyMen,              '@', 0x145, base + 0x44)   who marches
//!   for i in 0..6:  weapon icon (0x70 + i*0x48, y - 4)
//!                   Ui_DrawNumber(realm.weapons[i], ' ', 0x90 + i*0x48, y)
//!                   where y = base + (rows - 4) * 0x10
//!   cost >= 1 -> 69/10 "Happiness will" (0x188, base+0x18)
//!                69/11 "be"            (0x188, base+0x30)
//!                Ui_DrawNumber(happiness - cost, pen + 0x188, base + 0x30)
//!   cost <  1 -> 69/12 "Happiness stays" and 69/13 "at" in the same two places,
//!                Ui_DrawNumber(happiness,        pen + 0x188, base + 0x30)
//!   Pl8_DrawFrame(system, 0x53, pen + 0x188, base + 0x30)   AFTER either number
//!   no offer:
//!     FUN_0040328E(69, 4, 0x80, base + 0x60, 400, 100, body)  wrapped, 400 wide
//! Ui_DrawInsetRect(0x70, base + 0x58, 0x1A0, 0x32)... and the well AFTER it
//!   an offer:
//!     Ui_DrawInsetRect(0x70, base + 0x58, 0x1A0, 0x60)
//!     Ui_DrawNumber(band.men, '@', 0x70, base + 0x60, HEADING)
//!     Eng_DrawString(16, band, pen + 0x70, base + 0x60, HEADING)  the nationality
//!     Ui_DrawUnitNoun(band.men, 0x34 + troop*2, pen + 0x70, …, HEADING)
//!     Ui_DrawNumber(band.price,  '@', 0x70,      base + 0x7C, body)
//!     Eng_DrawString(69, 0, pen + 0x70, …)               "crowns to hire."
//!     Ui_DrawNumber(band.men / 2, '@', pen + 0x70, …, body)
//!     Eng_DrawString(69, 1, pen + 0x70, …)          "crowns seasonal wages."
//!     gold < price -> FUN_0040328E(69, 3, 0x80, base + 0x94, 400, 100, body)
//!     otherwise    -> Eng_DrawString(69, 15, 0x72, base + 0x92, body)  "You have"
//!                     Ui_DrawCount(realm.gold, 0, pen + 0x72, base + 0x92, body)
//!                     Eng_DrawString(69, 2, 0x92, base + 0xA4, body)
//!                                                     "Hire mercenaries ?"
//!                     Eng_DrawString(18, hire ? 0 : 1, 0x1D0, base + 0x98, HEADING)
//!   Ui_DrawNumber(realm + 0x138, '@', 0x70, (rows-2)*0x10 + base + 4, body)
//!   Eng_DrawString(69, 14, pen + 0x70, the same row)      "Total weapons"
//!   Eng_DrawString(69,  9, 0x180,      the same row)      "Continue"
//! ```
//!
//! Thirty-eight draw calls, and **every `Eng_DrawString` group was checked
//! against the words**: 69 is the
//! raise-army/armoury group, 16 the nationalities, 18 *"Yes"*/*"No"*/*"Cancel"*
//! and 100 the county names. Group 16 is the trap this audit exists for — the
//! armoury was filed under it and 16/6 *does* exist, so an existence check
//! passes on a screen that is wrong. Here 16 really is the nationality table
//! and the index really is the band id.
//!
//! **`Pl8_DrawFrame(&g_systemSheet, 0x53, …)` is drawn on both happiness
//! branches**, after whichever number was printed, on `base + 0x30`. It is
//! outside the `if`/`else` in the painter and it was missing here entirely.
//! [`HAPPINESS_ICON`]; **what the picture is has not been established** — it is
//! the frame after the slider's five (`0x4E`…`0x52`) and nothing else in the
//! binary draws it.
//!
//! **`Ui_DrawCount` uses the heading font for the band's headline line** — the
//! count, the nationality and the troop noun at `base + 0x60` are all
//! `g_fontHeading`, and everything below them is `g_fontBody`. This module drew
//! the whole block in the body font.
//!
//! **`g_levyPercent + 0xC4` is the knob's x**, which closes with the slider
//! handler's `g_levyPercent = mouseX - 0xC4` over a 101-pixel track: the two
//! are exact inverses, so [`SLIDER_X`] `[V]`.
//! measurement.
//!
//! **The six weapon icons come out of the armoury's own sheet.** They are
//! `Pl8_DrawFrame(DAT_0056D5B8, i + 0x0F, …)`, and `DAT_0056D5B8` is the buffer
//! `Screen_Armoury` reads `arm_it_<colour>.pl8` into — frames 15…20 of the
//! twenty-one, the six 30 × 26 icons that sit below its troop portraits. This
//! module used to say *"the sheet is not one we load"*; it is, and it is
//! [`crate::screens::armoury::items_sheet`].
//!
//! **`realm + 0x138` is the realm's weapon total** and 69/14 is *"Total
//! weapons"* — `Realm_RecountWeapons` (`0x004487A9`) keeps it as the sum of the
//! six stocks, so we sum.
//!
//! **`Eng_DrawString(69, 9)` is *"Continue"*, and it labels the one widget on
//! this screen that goes anywhere.** See below.
//!
//! The mercenary block, when `+0x1AD` is set:
//!
//! ```text
//! Ui_DrawInsetRect(0x70, base + 0x58, 0x1A0, 0x60)
//! Ui_DrawNumber(band.men) + L2.eng 16/band + the troop noun   at (0x70, base + 0x60)
//! Ui_DrawNumber(band.price) + 69/0 "crowns to hire."          at (0x70, base + 0x7C)
//! Ui_DrawNumber(band.men / 2) + 69/1 "crowns seasonal wages." on the same line
//! gold < price  ->  69/3 "You cannot afford ..."              at (0x80, base + 0x94)
//! otherwise     ->  69/15 + the treasury, 69/2 "Hire mercenaries ?", and
//!                   L2.eng 18/0 or 18/1 — Yes or No — at (0x1D0, base + 0x98)
//! ```
//!
//! **`men / 2` is the seasonal wage this screen prints**, and it is not what
//! the treasury pays: `Wages_ForUnit` charges `men / 4`, and `g_mercWage` holds
//! a third number that nothing reads. `docs/armies.md` §6 says so; this is the
//! screen that shows the first of the three.
//!
//! # The three widgets, and the two that are a pair
//!
//! `DAT_004DD340` is **three** 24-byte records — the next table,
//! `DAT_004DD388`, begins at `0x004DD340 + 3 * 24` — so `DAT_00522F58`'s
//! maximum of 3 reaches the end of it and nothing hides behind the count here.
//! **`Screen_RaiseArmy`'s own prologue is what publishes the count**, before it
//! draws anything: 1 with no offer, 1 with an offer the treasury cannot meet,
//! 3 otherwise. `Screen_HandleInput` reads the same global, so the tick and the
//! cross are untestable as well as invisible on the poor branch.
//!
//! It is drawn and tested at an offset of `(0, 0x10)` when the county has an
//! offer and `(0, 0)` when it has not — **sixteen pixels down when there is an
//! offer** — and that offset is not decoration. The label the *Continue* button
//! belongs to, `69/9`, is on `(rows - 2) * 0x10 + base + 4`, which is 340
//! without an offer and **356** with one, because `rows` goes `0x0D → 0x10`
//! while `base` goes `0xA0 → 0x80`. The record's own y is 336 either way. So
//! the offset is exactly the distance the label moved, and the button stays
//! four pixels above its own caption on both layouts. [`widget_offset`], and
//! the test below pins both numbers.
//!
//! | x | y | frame | handler | |
//! |---:|---:|---:|---|---|
//! | 480 | 336 | 33 | `FUN_00435CBF` | **Continue** — `g_screenId = 0x0A` |
//! | 352 | 256 | 29 | `FUN_00435C89` | the tick: hire = yes |
//! | 400 | 260 | 31 | `FUN_00435C89` | the cross: hire = no |
//!
//! Only the first is always there; the tick and the cross appear when the
//! county has a band on offer **and the treasury can meet its price**
//! the `DAT_00522F58 = 3` branch. The *"Yes"* / *"No"* word this screen prints
//! at `(0x1D0, base + 0x98)` is a **read-out of the flag, not a button** — it
//! is 88 pixels to the right of the cross and nothing tests it. This module
//! used to call that rectangle *"the original's own hotspot"*; it is
//! [`hire_readout`] now, and [`hire_yes`] and [`hire_no`] are the two that
//! click.
//!
//!
//! `Screen_FrameInput`'s `0x17` arm is `Levy_SliderClick()`, then — if that did
//! not take the click — a **right release** to `g_screenId = 0x0A` or the
//! corner picture to the campaign map. `Army_RaiseConfirm` is not reachable
//! from this screen at all: it is *Create*, a hotspot on the **armoury**.
//!
//! So the AUTO-EQUIP, UNEQUIP ALL, RAISE and CANCEL buttons this module used to
//! carry — ours, drawn below the original's window and labelled as ours — are
//! gone, and every one of them has a home now. Equipping is the armoury's
//! `+`/`−`, raising is *Create*, and cancelling is the corner picture. The one
//! thing with no counterpart was AUTO-EQUIP: `Levy_AutoEquip`'s round-robin is
//! the **AI's** function and no button in the game runs it, so the
//! address this module printed for it (`0x004AAD5F`) named nothing — it falls
//! inside `Battle_AutoResolve`. [`l2_kingdom::LevyBasket::auto_equip`] keeps
//! the rule for the AI and this screen no longer offers it.
//!
//! # The denominator, for the draw audit
//!
//! **Thirty-eight** call sites of the 26 pixel primitives: thirty-seven in
//! `Screen_RaiseArmy`'s own body plus the one `Ui_DrawText` inside
//! `FUN_0040328E`, which the painter reaches from two places. We draw all
//! thirty-eight, and all one-or-three widget records.
//!
//! Excluded: the armoury page beneath (`Screen_Draw` paints it and it is
//! counted under `screens/armoury.rs`), `FUN_004B1DE0`, `Gfx_MarkAllDirty` and
//! the `Blit_*` family.
//!
//! Still ours, and only these: **the status line** under the window, and two
//! no-artwork placeholders — a four-letter troop name where a weapon icon
//! would be, and a filled rectangle for the slider knob. A `<n>%` read-out of
//! ours, which the original does not draw at all, was removed.
//!
//! **This screen draws no `L2.eng` group 31**, so it does not resource
//! `docs/armies.md`'s `[V]` on unit `+0x166` against 31/21 *"Morale"*.
//!
//! # Where the state lives
//!
//! Not here. The slider's percentage, the headcount, the happiness cost, the
//! basket and the hire flag are five globals in the original and one
//! [`crate::game::LevyOrder`] here, because the armoury reads and writes all
//! five and this screen is *replaced* on the way there.

mod raise_part;
pub use raise_part::*;
mod raise;
pub use raise::*;

use raise::*;

use l2_kingdom::mercenary::ROSTER;
use l2_kingdom::unit::TroopType;
use l2_view::{text, Canvas};

use crate::input::{Event, Key, Rect};
use crate::press::{Press, Widget};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::screens::armoury;
use crate::shell::{self, font, Pen};
use crate::widget;

/// `L2.eng` group 69 — this screen's own text, shared with the armoury.
///
/// **Verified against the words**, not against the indices existing. The
/// thirteen this screen draws, in the file's own order:
///
/// ```text
///  0 "crowns to hire."          1 "crowns seasonal wages."
///  2 "Hire mercenaries ?"       3 "You cannot afford to hire these mercenaries."
///  4 "There are no mercenaries currently available for hire in the county."
///  9 "Continue"               10 "Happiness will"     11 "be"
/// 12 "Happiness stays"        13 "at"                 14 "Total weapons"
/// 15 "You have"               16 "Raising an army in"
/// ```
///
/// 5 belongs to the rack panel and 6, 7, 8 to the armoury's three captions;
/// the group has seventeen strings and this screen and `armoury.rs` draw all
/// of them between them.
pub const GROUP: usize = 69;
pub const CROWNS_TO_HIRE: usize = 0;
pub const CROWNS_WAGES: usize = 1;
pub const HIRE_QUESTION: usize = 2;
pub const CANNOT_AFFORD: usize = 3;
pub const NO_MERCENARIES: usize = 4;
pub const CONTINUE: usize = 9;
pub const HAPPINESS_WILL: usize = 10;
pub const HAPPINESS_BE: usize = 11;
pub const HAPPINESS_STAYS: usize = 12;
pub const HAPPINESS_AT: usize = 13;
pub const TOTAL_WEAPONS: usize = 14;
pub const YOU_HAVE: usize = 15;
pub const RAISING_IN: usize = 16;

/// `L2.eng` group 16 — the twelve nationalities, indexed by band id.
/// **Verified against the words**: `16/1` is *"Scottish"*, `16/12` *"Angevin"*.
/// Index 0 is *"No mercenaries in the army."* and the painter's `offer != 0`
/// guard means this screen never reaches it. This is the group the armoury was
/// wrongly filed under, and 16/6 existing is why an existence check did not
/// catch that.
pub const GROUP_NATIONALITY: usize = 16;
/// `L2.eng` group 18 — *"Yes"*, *"No"*, *"Cancel"*. Index 0 is *"Yes"*: the
/// painter draws 18/1 when the hire flag is clear. Index 2 is not drawn here.
pub const GROUP_YESNO: usize = 18;
/// `L2.eng` group 8 — the noun table. `Ui_DrawCount(gold, 0)` takes 0/1
/// *"Crown."*/*"Crowns."*; `Ui_DrawUnitNoun(men, 0x34 + t*2)` the troop nouns.
pub const NOUN_GROUP: usize = 8;
pub const NOUN_BASE: usize = 0x34;
pub const CROWN_NOUN: usize = 0;

/// The window: `Ui_DrawBox(0x50, base - 0x10, 0x1E, rows + 1)`.
pub const BOX_X: i32 = 0x50;
pub const BOX_COLS: i32 = 0x1E;

/// **The slider track's left end in pixels.** `Pl8_DrawFrame(system, 0x50,
/// g_levyPercent + 0xC4, …)` draws the knob and `Levy_SliderClick` reads
/// `g_levyPercent = mouseX - 0xC4` back out of it, over 101 pixels for 0…100.
pub const SLIDER_X: i32 = 0xC4;
/// Where the track stops and the right arrow begins.
pub const SLIDER_RIGHT: i32 = 0x129;
/// `Levy_SliderClick`'s hit band: `0x80 <= x < 0x176`, and `base + 0x10 <= y <
/// base + 0x40`. The three zones inside it are decrement, track, increment.
pub const SLIDER_HIT_X: (i32, i32) = (0x80, 0x176);
pub const SLIDER_HIT_DY: (i32, i32) = (0x10, 0x40);

/// `Ui_DrawInsetRect(0xC3, base + 0x28, 0x6F, 4)` — the well the knob runs in.
pub const WELL_X: i32 = 0xC3;
pub const WELL_W: i32 = 0x6F;
pub const WELL_H: i32 = 4;

/// The six weapon icons: icon at `0x70 + i * 0x48`, stock number `0x20` right
/// of it, on the row `base + (rows - 4) * 0x10`. **They are display only** —
/// nothing on this screen tests them, and the racks that do are the armoury's.
pub const RACK_X: i32 = 0x70;
pub const RACK_STEP: i32 = 0x48;
pub const RACK_NUMBER_DX: i32 = 0x20;
/// `Pl8_DrawFrame(DAT_0056D5B8, i + 0x0F, …)` — frames 15…20 of
/// `arm_it_<colour>.pl8`, the armoury's own sheet.
pub const ICON_FRAME_BASE: usize = 0x0F;

/// `Eng_DrawString(69, 9, 0x180, …)`.
pub const CONTINUE_LABEL_X: i32 = 0x180;

/// `L2.eng` group 100 — the county names, **twenty per scenario**, indexed
/// `scenarioIndex * 0x14 + county`. `Screen_RaiseArmy` prints one after 69/16.
pub const COUNTY_NAMES: usize = 100;
pub const COUNTY_NAMES_STRIDE: usize = 0x14;

/// `System.pl8` frames the slider is built from, in the order the painter draws
/// them: the icon at each end, the arrow at each end, and the knob.
pub const SLIDER_LEFT_ICON: usize = 0x51;
pub const SLIDER_LEFT_ARROW: usize = 0x4E;
pub const SLIDER_KNOB: usize = 0x50;
pub const SLIDER_RIGHT_ARROW: usize = 0x4F;
pub const SLIDER_RIGHT_ICON: usize = 0x52;

/// `Pl8_DrawFrame(&g_systemSheet, 0x53, g_penAdvance + 0x188, base + 0x30)` —
/// drawn on **both** happiness branches, after the number.
///
/// **What the picture is has not been established.** It is the frame
/// immediately after the slider's five and no other function in the
/// decompiled corpus passes `0x53` to `Pl8_DrawFrame`
/// call site to read it against. Named.
pub const HAPPINESS_ICON: usize = 0x53;

/// The three records of `DAT_004DD340`, in table order: **Continue** (frame 33,
/// 24 pixels, `FUN_00435CBF`), then the tick (29) and the cross (31), both
/// `FUN_00435C89` and told apart by the hotspot id. The tick and the cross are
/// 32 pixels; Continue is 24.
pub const CONTINUE_FRAME: usize = 33;
pub const HIRE_YES_FRAME: usize = 29;
pub const HIRE_NO_FRAME: usize = 31;
/// `DAT_004DD340` is three records long, and `DAT_00522F58` is 1 or 3.
pub const WIDGET_RECORDS: usize = 3;
pub const WIDGETS_NO_OFFER: usize = 1;
pub const WIDGETS_UNAFFORDABLE: usize = 1;
pub const WIDGETS_AFFORDABLE: usize = 3;

/// `FUN_0040328E(group, index, x, y, 400, 100, 0, 0, …)` — the wrapped
/// paragraph's column. Both of this screen's two long sentences use it, and
/// drawing either as a single line runs it off the window.
pub const PARAGRAPH_W: i32 = 400;

/// `Ui_OkButton(g_screenStride - 0x1C, g_screenHeight - 0x70, 1)` — stashed by
/// `Screen_Armoury`, which is the painter that ran underneath, so it is the
/// armoury's corner picture that closes this screen too. Both go to the map.
pub const OK: Rect = armoury::OK;

/// What the last confirm did. Kept so that a caller or a test can ask this
/// screen what happened even though the button that raises an army is now the
/// armoury's — [`crate::screens::armoury::Raised`] is where it comes from.
pub use crate::screens::armoury::Raised;

#[cfg(test)]
mod tests {
    use super::*;

    /// The knob's x and the handler's arithmetic are exact inverses over the
    /// whole 101-pixel track, which is what makes both `[V]`.
    #[test]
    fn the_knob_and_the_slider_handler_are_inverses_over_the_whole_track() {
        for pct in 0..=100 {
            let knob = SLIDER_X + pct;
            assert!((SLIDER_X..SLIDER_RIGHT).contains(&knob), "{pct} lands off the track");
            assert_eq!(knob - SLIDER_X, pct);
        }
        assert_eq!(SLIDER_RIGHT - SLIDER_X, 101, "0 ... 100 inclusive");
    }

    /// The two layouts, and the fact that the taller one is the one with an
    /// offer on it — which is the opposite of what a reader guesses from the
    /// smaller base y.
    #[test]
    fn the_window_grows_downward_when_a_band_is_offering() {
        let (with, without) = (window(true), window(false));
        assert_eq!((with.x, with.y, with.w, with.h), (0x50, 0x70, 480, 272));
        assert_eq!((without.x, without.y, without.w, without.h), (0x50, 0x90, 480, 224));
        assert!(with.h > without.h);
        for r in [with, without] {
            assert!(r.x + r.w <= 640 && r.y + r.h <= 480, "off screen");
        }
    }

    /// Six icons, evenly pitched, none of them overlapping and all of them
    /// inside the window.
    #[test]
    fn the_six_weapon_icons_are_inside_the_window() {
        for on in [true, false] {
            let w = window(on);
            let y = rack_row(on);
            for i in 0..6i32 {
                let x = RACK_X + i * RACK_STEP;
                assert!(w.contains(x, y - 4), "icon {i} is outside the window");
                assert!(w.contains(x + RACK_NUMBER_DX + 20, y), "icon {i}'s number runs off it");
            }
        }
    }

    /// **The widgets are the original's, and the read-out is not one of them.**
    /// The word "Yes" sits 88 pixels right of the cross; if a future edit makes
    /// the read-out clickable, this is what says the original did not.
    #[test]
    fn the_hire_readout_is_not_where_the_tick_and_the_cross_are() {
        for on in [true, false] {
            let (yes, no, word) = (hire_yes(on), hire_no(on), hire_readout(on));
            assert!(yes.x + yes.w <= no.x, "the tick and the cross overlap");
            assert!(no.x + no.w <= word.x, "the read-out overlaps the cross");
            assert_eq!(word.x - (no.x + no.w), 32, "the gap the painter leaves");
        }
    }

    /// The Continue button is inside the original's window, is clear of the
/// slider's hit band, and moves with the table offset
    /// layout's base.
    #[test]
    fn the_continue_button_is_the_only_way_on_and_it_is_inside_the_window() {
        for on in [true, false] {
            let (w, c) = (window(on), continue_button(on));
            assert!(w.contains(c.x, c.y), "Continue is outside the window");
            assert!(w.contains(c.x + c.w - 1, c.y + c.h - 1), "Continue runs off it");
            assert!(
                c.y >= base(on) + SLIDER_HIT_DY.1 || c.x >= SLIDER_HIT_X.1,
                "Continue is inside the slider's hit band, which is tested first",
            );
        }
        // The two positions differ by exactly the table offset and nothing
        // else: the original moves the widget table, not the widget.
        assert_eq!(continue_button(true).y - continue_button(false).y, 0x10);
    }

    /// **The sixteen-pixel widget offset is the distance the caption moved.**
/// Both sides are pinned from the decompilation
    /// the constants they check: `DAT_004DD340` record 0 sits at y 336, the
    /// caption row is `(rows - 2) * 0x10 + base + 4`, and the difference is the
    /// offset `Screen_DrawWidgets` passes.
    #[test]
    fn the_widget_offset_is_exactly_how_far_the_continue_caption_moved() {
        assert_eq!(footer_row(false), 340, "(0x0D - 2) * 0x10 + 0xA0 + 4");
        assert_eq!(footer_row(true), 356, "(0x10 - 2) * 0x10 + 0x80 + 4");
        assert_eq!(footer_row(true) - footer_row(false), widget_offset(true));
        assert_eq!(widget_offset(false), 0);
        // And the button stays four pixels above its own caption either way.
        for on in [true, false] {
            assert_eq!(footer_row(on) - continue_button(on).y, 4);
        }
    }

    /// The count the painter's prologue publishes never runs off the end of the
    /// table — `g_sendSuppliesWidgets`'s eight records against a count of six
    /// is the failure this is the check for, in the other direction.
    #[test]
    fn the_widget_count_never_exceeds_the_tables_three_records() {
        for n in [WIDGETS_NO_OFFER, WIDGETS_UNAFFORDABLE, WIDGETS_AFFORDABLE] {
            assert!(n <= WIDGET_RECORDS, "DAT_00522F58 = {n} reads past DAT_004DD340");
        }
        assert_eq!(WIDGETS_AFFORDABLE, WIDGET_RECORDS, "the rich branch draws all three");
        assert_eq!(WIDGETS_NO_OFFER, 1, "and the other two draw Continue alone");
    }

    /// The corner picture is the armoury's, because the armoury is the painter
    /// that ran underneath and `Ui_OkButton` stashes only its last call.
    #[test]
    fn the_close_button_is_the_one_the_armoury_stashed() {
        assert_eq!(OK.x, 640 - 0x1C);
        assert_eq!(OK.y, 480 - 0x70);
        for on in [true, false] {
            let w = window(on);
            assert!(OK.x >= w.x + w.w || OK.y >= w.y + w.h, "the corner picture is under the box");
        }
    }
}



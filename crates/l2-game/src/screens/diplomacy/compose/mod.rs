#![allow(unused_imports)]

mod helper;
pub use helper::*;
mod compose;
pub use compose::*;

use super::*;
use super::main::*;
use super::tests::*;
use l2_kingdom::diplomacy::{group, Kind};
use l2_kingdom::realm::MAX_REALMS;
use l2_view::Canvas;
use crate::input::{Event, Key, Rect};
use crate::press::{Press, Widget};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::screens::message::lord_name;
use crate::shell::{font, Pen};
use crate::widget;

// ------------------------------------------------------------ the 0x1A screen

/// `FUN_00436372` — the gift stepper's step, and it is ten crowns whichever
/// button was pressed. Hotspot 1 adds, hotspot 0 subtracts.
pub const GIFT_STEP: i32 = 10;

/// `g_giftWidgets` (`0x004DD9D0`) — plus, minus, tick, cross.
pub const GIFT_MORE: Rect = Rect::new(184, 240, 32, 32);
pub const GIFT_LESS: Rect = Rect::new(216, 240, 32, 32);
pub const GIFT_SEND: Rect = Rect::new(288, 280, 32, 32);
pub const GIFT_CANCEL: Rect = Rect::new(324, 284, 32, 32);
/// `0x004DDA30` — the letter dialog's two.
pub const LETTER_SEND: Rect = Rect::new(288, 292, 32, 32);
pub const LETTER_CANCEL: Rect = Rect::new(324, 296, 32, 32);
/// `0x004DDA60` — the county dialog's two.
pub const COUNTY_SEND: Rect = Rect::new(320, 244, 32, 32);
pub const COUNTY_CANCEL: Rect = Rect::new(356, 248, 32, 32);

/// The button-sheet frames those records name. `System.pl8`
/// is the one `tools/oracle/widgets.js` anchors on this very table:
/// **frame 68 is plus** — its record carries hotspot id 1, and `FUN_00436372`
/// reads `if (id == 1) g_diploGold += 10` — and **66 is minus**.
pub const PLUS_FRAME: usize = 68;
pub const MINUS_FRAME: usize = 66;
/// 29 and 31 are the mailed hand, thumb up and thumb down. **Not a tick and a
/// cross** — `docs/screens-county.md` §4.2 decoded them.
pub const THUMB_UP_FRAME: usize = 29;
pub const THUMB_DOWN_FRAME: usize = 31;
/// Every record in the three slices is `size` 32.
pub const WIDGET_DIM: i32 = 32;

/// The three windows, `FUN_004093E0(x, y, cols, rows)` from the three painters.
/// `FUN_004093E0` is `Ui_DrawBoxBorder(**1**, …)` plus `Ui_DrawBoxInterior`
/// inset a cell, so the border set is 1 on all three.
pub const GIFT_WINDOW: Rect = Rect::new(0x40, 0xA0, 0x16 * 16, 0x0B * 16);
pub const LETTER_WINDOW: Rect = Rect::new(0x10, 0x90, 0x1C * 16, 0x0D * 16);
pub const COUNTY_WINDOW: Rect = Rect::new(0x30, 0x80, 0x18 * 16, 0x0F * 16);
pub const WINDOW_SET: usize = 1;

/// `Ui_OkButton(x, y, 0)` — **a third button on every one of the seven**, and
/// one this module drew nothing for until the draw-call audit counted them.
/// It is `System.pl8` frame `0x33`, the cursor-into-a-hole close picture, and
/// it sits to the right of the thumb pair inside the same window.
pub const GIFT_OK: Rect = Rect::new(0x178, 0x126, 24, 24);
pub const LETTER_OK: Rect = Rect::new(0x1A8, 0x136, 24, 24);
pub const COUNTY_OK: Rect = Rect::new(0x188, 0x146, 24, 24);

// ------------------------------------------- the compose dialogs' own strings
//
// Every one of these is a literal argument to an `Eng_DrawString` in
// `Diplo_DrawGiftGold`, `Diplo_DrawLetter` or `Diplo_DrawCountyRequest`, and
// the words are checked against `L2.eng` in `crates/l2-game/tests/shell/main.rs`.
// **The check is on existence and these indices are verified against the
// words**

/// 72/10 *"Send gift of gold to"*, with the target's name after it.
pub const GIFT_TO: usize = 10;
/// 72/23 *"Last gift was"*.
pub const LAST_GIFT: usize = 23;
/// 72/18 *"Gift of"*.
pub const GIFT_OF: usize = 18;
/// 72/17 *"Dispatch ?"* — the caption over the thumb pair, and **all three**
/// dialogs draw it, at three different places.
pub const DISPATCH: usize = 17;
/// 72/11 + (kind − 1): *"Give a compliment to"*, *"Insult"*, *"Ask for an
/// alliance with"*, *"End alliance with"*.
pub const LETTER_BASE: usize = 11;
/// 72/15 + k: *"Plead for help from"*, *"Plan strategic attack with"*.
pub const REQUEST_BASE: usize = 15;
/// 72/19 + k while no county is picked: *"Choose the county you want help
/// in."* / *"…attacked."*
pub const REQUEST_PROMPT: usize = 19;
/// 72/21 + k once one is, with the county's name after it.
pub const REQUEST_PICKED: usize = 21;
/// `Eng_DrawString(100, g_scenarioIndex * 0x14 + county, …)` — the county
/// names, twenty per scenario.
pub const COUNTY_NAME_GROUP: usize = 100;
/// `Ui_DrawCount(value, **0**, …)`, so `L2.eng` group 8 index 0 *"Crown."* or
/// index 1 *"Crowns."* — the noun the amount is drawn with.
pub const CROWN_NOUN: usize = 0;

/// `FUN_00417BD3`'s draft box: `Ui_DrawBoxInterior(0x20, 0xC0, 0x1A, 6)` and
/// `Ui_DrawInsetRect(0x20, 0xC0, 0x1A0, 0x60)` over the top of it, the same
/// 416 × 96 twice.
pub const LETTER_DRAFT: Rect = Rect::new(0x20, 0xC0, 0x1A0, 0x60);
/// `FUN_0040352F(draft, 0x30, 200, 0x180, body, 0x3F)` — the text inside it,
/// wrapped at 384 pixels from (48, 200).
pub const LETTER_DRAFT_TEXT: (i32, i32, i32) = (0x30, 200, 0x180);

/// `Edit_Begin(&g_diploLetterDraft + (kind - 1) * 200, 200, 10000, 0)` —
/// `Diplo_OpenCompliment` (`0x0043618B`) and its three siblings. The character
/// limit is **200**, the pixel limit 10,000, which no draft box reaches, so
/// only the character limit bites. **[V]**
pub const LETTER_MAX_LEN: usize = 200;
pub const LETTER_MAX_PIXELS: i32 = 10_000;

/// `Edit_Commit(g_diploLetterDraft + (kind - 1) * 200, 199)` — the harvest in
/// `Screen_HandleInput`'s `0x1A` arm. **199, one less than the field takes**,
/// so the two-hundredth character a person may type is the one a send drops.
/// **[V]**
pub const LETTER_COMMIT: usize = 199;

/// `Eng_CopyString(g_diploLetterDraft + k * 200, 0xE2, k, 200)` in
/// `Options_SetDefaults` (`0x004AE310`): the four drafts a game starts with,
/// **`L2.eng` group 226 indices 0…3**, each cut at the first character below
/// `0x20`. One consumer, so `CLAUDE.md` rule 6 makes it this screen's
/// vocabulary. **[V]**
pub const LETTER_DEFAULT_GROUP: usize = 226;

/// The same four transcribed, for an install with no `L2.eng`.
const LETTER_DEFAULT_OURS: [&str; 4] = [
    "Verily, your oppression of the weak and your flattery of the strong are worthy of emulation.  Pray, teach me more.",
    "You are ugly, and your mother dresses you funny.",
    "Sire, these are troubled times, I ask you to forget our past differences. We needs must fight together, to see off those that would do us harm.",
    "It pleases me to report that happier times seem to be upon us now my friend. We now no longer have need for our pact. I shall remember your loyalty ere I lift the crown.",
];

/// The draft a letter of `kind` (1…4) opens with.
///
/// The cut at the first control character is `Options_SetDefaults`' own loop,
/// and it **latches**: everything from the first byte under `0x20` onwards is
/// zeroed, not only that byte.
pub fn letter_default(ctx: &Ctx, kind: Kind) -> String {
    let k = (kind.byte() as usize).saturating_sub(1).min(3);
    let from_eng = ctx.assets.shell.text(LETTER_DEFAULT_GROUP, k);
    let s = if from_eng.is_empty() { LETTER_DEFAULT_OURS[k] } else { from_eng };
    s.split(|c: char| (c as u32) < 0x20).next().unwrap_or("").to_string()
}

/// `FUN_00410C71(county, 0x60, 0xB0)` blits the raster at `(x − 2, y + 3)`.
/// [`PICKER`] is the rectangle the hit test uses, which is the unadjusted one.
pub const PICKER_DRAW: (i32, i32) = (PICKER.x - 2, PICKER.y + 3);

/// Why a send was refused, each one an `L2.eng` group of its own with
/// *"Message not sent."* at index 0. `Diplo_SendClicked`'s order is preserved,
/// because the order decides which message a doubly-wrong county gets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// 240 — *"You did not select a county, my Lord."*
    NoCounty,
    /// 241 — *"Your requested county does not belong to anyone, my Lord."*
    Unowned,
    /// 242 — *"Your requested county does not belong to us, my Lord."* Kind 5
    /// only.
    NotOurs,
    /// 243 — *"Our county does not have an enemy in it, my Lord."* Kind 5 only.
    NoEnemy,
    /// 244 — *"This county is part of our alliance, my Lord."* Kind 6, and it
    /// is **also what a county of your own gets**, which is the wrong sentence
    /// for that case. Reproduced.
    Allied,
    /// 219 — *"You cannot ally with this player, my Lord, until they end their
    /// current treaty."* Kind 3, and **only against a human target**: the guard
    /// is `g_realms[target].isHuman != 0 && ally != 0`, so an AI that already
    /// has an ally is not refused here — the offer goes out and comes back as
    /// group 178.
    TargetAlreadyAllied,
}

impl Refusal {
    /// The `L2.eng` group this refusal is drawn from.
    pub fn group(self) -> u16 {
        match self {
            Refusal::NoCounty => group::NO_COUNTY_SELECTED,
            Refusal::Unowned => group::COUNTY_UNOWNED,
            Refusal::NotOurs => group::COUNTY_NOT_OURS,
            Refusal::NoEnemy => group::COUNTY_UNTHREATENED,
            Refusal::Allied => group::COUNTY_IS_ALLIED,
            Refusal::TargetAlreadyAllied => group::ALREADY_IN_ALLIANCE,
        }
    }
}

/// The rectangle `FUN_0043B4CB` hit-tests: 128 × 128 at (96, 176).
///
/// **The picture is two pixels left and three down of it.**
/// `FUN_00410C71(county, x, y)` blits the raster at `(x - 2, y + 3)` and then
/// `Minimap_DrawOverlay(county, x - 2, y + 3, 0)` on top, while the hit test
/// uses `(x, y)` unadjusted. That is the same disagreement the sidebar minimap
/// has between `Minimap_Draw` and `Minimap_Click` — `l2_view::chrome`'s
/// `MINIMAP_X` against `MINIMAP_HIT_X` — and it is the original's in both
/// places. The hit rectangle is what is reproduced here, because it is what
/// decides which county you picked.
pub const PICKER: Rect =
    Rect::new(0x60, 0xB0, l2_view::chrome::MINIMAP_DIM, l2_view::chrome::MINIMAP_DIM);


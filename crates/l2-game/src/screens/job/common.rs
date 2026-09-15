#![allow(unused_imports)]
use super::*;
use super::screen::*;
use super::blacksmith_part::*;
use super::bodies::*;
use super::castle::*;
use l2_kingdom::county::County;
use l2_kingdom::tables::{
    Commodity, Tables, JOB_CASTLE_BUILDING, JOB_CATTLE_FARMING, JOB_COUNT, JOB_FIELD_RECLAMATION,
    JOB_GRAIN_FARMING, JOB_IRON_MINING, JOB_NAMES, JOB_STONE_QUARRYING, JOB_WOOD_CUTTING,
};
use l2_view::chrome::system;
use l2_view::Canvas;
use crate::input::{Event, Key, Rect};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::shell::{count_noun, font, Face, Pen};

/// `FUN_00403CF4(0x40, 0x68, 0x32, 0x32, 0x3F)` — **a one-pixel outline, not a
/// recess.** The function is four `FUN_00403A8F` line calls in colour `0x3F`
/// round the given box, clipped to the screen.
///
/// lighting. `Sprite_WGenSprite(DAT_004D2974[job], 0x41, 0x69)` puts the job's
/// `iconvill.pl8` picture one pixel inside it.
pub(super) const ICON_BOX: Rect = Rect::new(64, 104, 50, 50);
pub(super) const ICON_BOX_INK: u8 = 0x3F;

/// `Sprite_WGenSprite(DAT_004D2974[job], 0x41, 0x69)` — the job's picture, one
/// pixel inside [`ICON_BOX`]. `Iconvill.pl8` is 17 frames of 48 × 48 and this
/// panel is its only consumer.
pub(super) const ICON_SHEET: &str = "Iconvill.pl8";
pub(super) const ICON_AT: (i32, i32) = (0x41, 0x69);

/// `DAT_004D2974`, the frame per job, **1-based in the original** and shifted
/// here. The last entry is the painter's own override: the table says 10 for
/// job 9 and `if (g_jobPanelJob == 9) local_c = 0x10;` says 16. Read out of the
/// shipped exe; the sheet's 17 frames are the check on the 16.
pub(super) const JOB_ICON: [usize; JOB_COUNT] = [0, 2, 3, 5, 6, 7, 8, 9, 16];


/// **`L2.eng` group 77** — the grain, herd and reclamation forecast lines.
pub const FORECAST_GROUP: usize = 77;
/// **`L2.eng` group 76** — the industry lines. `Panel_JobIndustry` draws 0…3,
/// `Panel_JobBlacksmith` 4…8.
pub const INDUSTRY_GROUP: usize = 76;
/// **`L2.eng` group 71** — castle selection and status; this body draws 6, 7,
/// 8, 11, 12, 16 and 17.
pub const CASTLE_GROUP: usize = 71;
/// **`L2.eng` group 22** — the seven fertility phrases, index
/// `(fertility + 100) / 29`.
pub const FERTILITY_GROUP: usize = 22;

pub const NOUN_SACK: usize = 2;
pub const NOUN_ANIMAL: usize = 4;
pub const NOUN_IRON: usize = 0x0C;
pub const NOUN_STONE: usize = 0x0E;
pub const NOUN_WOOD: usize = 0x10;
pub const NOUN_BUILDER: usize = 0x26;
pub const NOUN_SEASON: usize = 0x42;

pub(super) const BODY_INK: u8 = font::TEXT;
const DELTA_NEG: u8 = 0xF9;

/// Our transcription of group 77, for an install with no `L2.eng`.
const OURS_77: [&str; 31] = [
    "from",
    "to be sown, yielding",
    "in 4 seasons.",
    "harvested in",
    "sown in spring.",
    "Calf births expected",
    "Cow deaths expected",
    "Change due to farming",
    "Low herd crowding.",
    "Average herd crowding.",
    "Herd overcrowded.",
    "Massive overcrowding!!",
    "field being reclaimed",
    "fields being reclaimed",
    "Next field reclaimed in",
    "No field reclamation with 0 labourers",
    "gained last season, due to weather.",
    "lost last season, due to weather.",
    "Weather had no effect last season.",
    "No outside events affected the herd this season.",
    "died of disease.",
    "taken by wolves.",
    "had to be put down.",
    "born, over expectations.",
    "No outside factors affected stored grain.",
    "eaten by rats.",
    "found as surplus.",
    "Change due to eating",
    "Overall change",
    "extra deaths.",
    "extra births.",
];

const OURS_76: [&str; 9] = [
    "Working with an efficiency of",
    "will be produced next season.",
    "will be used by the blacksmiths.",
    "needed by castle builders.",
    "Serfs working at",
    "efficiency.",
    "Will produce",
    "next season.",
    "Smiths working.",
];

const OURS_75: [&str; 3] = [
    "Click on a weapon to change production.",
    "wood needed.",
    "iron needed.",
];

const OURS_71: [&str; 20] = [
    "Select a castle to build",
    "Wooden palisade.",
    "Motte and bailey.",
    "Norman keep.",
    "Stone castle.",
    "Royal castle.",
    "of stone needed,",
    "of wood needed.",
    "will take",
    "to build.",
    "Build this castle",
    "Barracks for",
    "troops.",
    "currently stationed here.",
    "View these troops?",
    "Start construction?",
    "Boosts tax revenues by",
    "No castle building in progress.",
    "Needed",
    "Enemy troops are barracked here.",
];

const OURS_22: [&str; 7] = [
    "Infertile - almost no production.",
    "Very poor fertility - mainly weeds.",
    "Poor fertility - crops grow less well.",
    "Average fertility - no effect on crops.",
    "Good fertility - crops are boosted.",
    "Very High fertility - many extra crops.",
    "Excellent fertility - bumper crop!",
];

/// Our transcription of one string these bodies draw. `screens::info` shares
/// group 71's, because `Castle_DrawStatusBlock` and `TileInfo_DrawCastle` draw
/// the same strings on two screens.
pub(crate) fn ours(group: usize, index: usize) -> &'static str {
    let table: &[&str] = match group {
        FORECAST_GROUP => &OURS_77,
        INDUSTRY_GROUP => &OURS_76,
        CASTLE_GROUP => &OURS_71,
        FERTILITY_GROUP => &OURS_22,
        SMITHY_GROUP => &OURS_75,
        COUNT_NOUN_GROUP => {
            return match index {
                2 => "Sack",
                3 => "Sacks",
                4 => "Animal",
                5 => "Animals",
                0x0C | 0x0E | 0x10 => "Tonne",
                0x0D | 0x0F | 0x11 => "Tonnes",
                // The six weapons, `DAT_004D29C8`'s indices and their plurals.
                0x12 => "Pike",
                0x13 => "Pikes",
                0x14 => "Bow",
                0x15 => "Bows",
                0x16 => "Mace",
                0x17 => "Maces",
                0x18 => "Crossbow",
                0x19 => "Crossbows",
                0x1A => "Sword",
                0x1B => "Swords",
                0x1C | 0x1D => "Armour",
                0x26 => "Builder",
                0x27 => "Builders",
                0x42 => "Season",
                0x43 => "Seasons",
                _ => "",
            }
        }
        _ => &[],
    };
    table.get(index).copied().unwrap_or("")
}

#[allow(clippy::too_many_arguments)]
pub(super) fn say(pen: &Pen, ctx: &Ctx, canvas: &mut Canvas, group: usize, index: usize, x: i32, y: i32) -> i32 {
    let s = eng(ctx, group, index, ours(group, index));
    pen.body(canvas, x, y, &s, BODY_INK)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn count(pen: &Pen, ctx: &Ctx, canvas: &mut Canvas, value: i32, noun: usize, x: i32, y: i32) -> i32 {
    let index = count_noun(value, noun);
    let s = eng(ctx, COUNT_NOUN_GROUP, index, ours(COUNT_NOUN_GROUP, index));
    pen.count_with_noun(Face::Body, canvas, x, y, value, &s, BODY_INK)
}

pub(super) fn zero(pen: &Pen, canvas: &mut Canvas, x: i32, y: i32) {
    pen.number_in(Face::Body, canvas, x, y, 0, '@', " ", BODY_INK);
}

pub(super) fn delta(pen: &Pen, canvas: &mut Canvas, value: i32, x: i32, y: i32) {
    if value == 0 {
        return;
    }
    let colour = if value < 0 { DELTA_NEG } else { BODY_INK };
    let next = pen.body(canvas, x, y, " ", colour);
    let (lead, shown) = if value < 0 { ('-', value.wrapping_neg()) } else { ('+', value) };
    pen.number_in(Face::Body, canvas, next, y, shown, lead, " ", colour);
}


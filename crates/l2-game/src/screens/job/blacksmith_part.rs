#![allow(unused_imports)]
use super::*;
use super::screen::*;
use super::common::*;
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

/// Our transcription of those nine pairs, for a machine with no `L2.eng`.
pub(super) const WORKER_NOUN: [(&str, &str); JOB_COUNT] = [
    ("FARMER", "FARMERS"),
    ("DAIRY MAID", "DAIRY MAIDS"),
    ("SERF", "SERFS"),
    ("BUILDER", "BUILDERS"),
    ("MINER", "MINERS"),
    ("QUARRIER", "QUARRIERS"),
    ("FORESTER", "FORESTERS"),
    ("BLACKSMITH", "BLACKSMITHS"),
    ("PEASANT", "PEASANTS"),
];

pub const BLACKSMITH: usize = 7;

pub(super) const BLACKSMITH_OK: Rect = Rect::new(0x1C0, 0x1C0, system::OK_DIM, system::OK_DIM);


/// **`DAT_004DCA10` — the six hotspots over the smithy picture**, `(x0, y0, x1,
/// y1)` in table order, which is also weapon-type order: crossbow, mace, sword,
/// pike, bow, armour.
///
/// `Screen_HandleInput` (`0x004BA9C8`) opens with them and nothing else on this
/// screen:
///
/// ```c
/// if (g_screenId == 0x0F && g_jobPanelJob == 8) {
///     if (DAT_0055CD78 != 0) g_redrawRequest = 2;
///     if (Hotspot_Test(0, 0x18, &DAT_004DCA10, 6)) return 1;
/// }
/// ```
///
/// **This is the only control on any of the nine job popups**, and it is
/// dispatched from `Screen_HandleInput`,
/// so an enumeration of the dispatcher scored it zero without it ever
/// appearing as a miss. `docs/decisions.md` C61's denominator note.
///
/// Every record's kind byte at `+0x0F` is **1** — [`Kind::Press`], the down
/// edge, no pressed picture and no repeat — and every one dispatches to
/// `FUN_0043A950`, which publishes `g_uiHotspotId` (0…5) and calls
/// `FUN_0043A997(g_selectedCounty, id)`. Read out of the player's own exe by
/// `node tools/oracle/widgets.js widgets 4dca10 6`; asserted against it in
/// `crates/l2-game/tests/arms.rs`.
pub const WEAPON_HOTSPOTS: [(i32, i32, i32, i32); l2_kingdom::tables::WEAPON_TYPE_COUNT] = [
    (354, 10, 479, 97),  // 0 crossbow
    (296, 63, 354, 198), // 1 mace
    (368, 97, 479, 219), // 2 sword
    (158, 53, 233, 225), // 3 pike
    (9, 38, 87, 211),    // 4 bow
    (233, 82, 296, 246), // 5 armour
];

/// `Hotspot_Test(0, 0x18, …)`'s two offsets, which are **added to every record**
/// before the test: `x0 + dx <= mx < x1 + dx`, half-open on both axes
/// (`Hotspot_Test`, `0x0040E3EE`).
pub const HOTSPOT_ORIGIN: (i32, i32) = (0, 0x18);

pub fn weapon_at(x: i32, y: i32) -> Option<usize> {
    let (dx, dy) = HOTSPOT_ORIGIN;
    WEAPON_HOTSPOTS.iter().position(|&(x0, y0, x1, y1)| {
        x0 + dx <= x && x < x1 + dx && y0 + dy <= y && y < y1 + dy
    })
}

const SMITHY_SHEET: &str = "Smithy.pl8";
const SMITHY_AT: (i32, i32) = (0, 0x18);

const HEARTH_SHEET: &str = "Hearth.pl8";
const HEARTH_FIRST: usize = 0x0B;

/// `DAT_004D29E0[weapon]` — how far the hearth picture is lifted off
/// [`HEARTH_FLOOR`]: `y = 0x1A8 - lift[weapon]`. Every one is a different height
/// because every weapon hangs differently. Read out of the shipped exe.
const HEARTH_LIFT: [i32; l2_kingdom::tables::WEAPON_TYPE_COUNT] = [136, 142, 119, 125, 124, 182];
const HEARTH_FLOOR: i32 = 0x1A8;

/// `g_spriteWidth = 0x1E; g_spriteHeight = 0x38; FUN_004B414A(0, 0x1A8, 0)` —
/// **a fill, not a sprite.** `FUN_004B414A` (`0x004B414A`) writes
/// `g_spriteWidth` dwords-times-four across and `g_spriteHeight` rows down from
/// `(x, y)`, so this is 480 × 56 of palette index 0 under the hearth.
const FLOOR_BAND: Rect = Rect::new(0, HEARTH_FLOOR, 0x1E * 16, 0x38);

/// `FUN_00413526` (`0x00413526`) — **the forge fire**, and the whole of
/// `Screen_DrawWidgets`'s `0x0F` arm:
///
/// ```c
/// if (g_pulse80 == 0) return;
/// DAT_004E59BC = (DAT_004E59BC + 1) % 11;
/// Pl8_DrawFrameClipped(scratch, DAT_004E59BC, 0x58, 0x9D);
/// Gfx_MarkSpriteDirty(0x58, 0x9D, 8, 8, 1);
/// ```
pub const FORGE_FRAMES: usize = 11;
pub(super) const FORGE_AT: (i32, i32) = (0x58, 0x9D);

const FOOTER_X: i32 = 0;
const FOOTER_Y: i32 = 0x180;
const FOOTER_COLS: i32 = 0x1E;
const FOOTER_ROWS: i32 = 6;

/// **`L2.eng` group 75, and `Panel_JobBlacksmith` is its only consumer in the
/// whole binary** — so it is this page's vocabulary
/// (`CLAUDE.md` rule 6, `docs/formats/eng.md` §5).
///
/// Index 0 is *"Click on a weapon to change production."*, drawn by
/// `Ui_DrawCentred(0x4B, 0, 0, 0x1CC, 0x1CC, body, 0x3F)` — **the sentence that
/// tells a player the picture is a control at all**, and the one a player said
/// was missing: *"I can't choose what type of weapon my blacksmiths are
/// making."* Indices 1 and 2, *"wood needed."* and *"iron needed."*, are in the
/// group and **nothing in the binary draws them**: the two cost figures below
/// take `&DAT_004D3E9C` and `&DAT_004D3EA0`, which are both the empty string,
/// and the icons carry the meaning instead.
pub const SMITHY_GROUP: usize = 75;
const SMITHY_LINE: (i32, i32, i32) = (0, 0x1CC, 0x1CC);

const SMITHY_TITLE_AT: (i32, i32) = (0x10, 0x186);
const SMITHY_ROW_WORKERS: i32 = 0x1A4;
const SMITHY_ROW_OUTPUT: i32 = 0x1B4;
const SMITHY_TEXT_X: i32 = 0x10;

/// `Ui_DrawInsetRect(0x140, 0x186, 0x8E, 0x20)` and the two `Pl8_DrawFrame`
/// icons and `Ui_DrawNumber`s inside it. `Misc_cty` frame `0x2C` is the iron
/// bar and `0x2E` the log; the numbers are `g_weaponCost[weapon]`'s **iron
/// first** (`&DAT_004D8994`, the pair's second word) and wood second
/// (`&g_weaponCost`, its first).
const COST_WELL: Rect = Rect::new(0x140, 0x186, 0x8E, 0x20);
const COST_IRON_ICON: (usize, i32, i32) = (0x2C, 0x146, 0x187);
const COST_IRON_AT: (i32, i32) = (0x16A, 0x18E);
const COST_WOOD_ICON: (usize, i32, i32) = (0x2E, 0x189, 0x18A);
const COST_WOOD_AT: (i32, i32) = (0x1B4, 0x18E);

/// `DAT_004D29C8[weapon]` — the group-8 **singular** noun index for what this
/// smithy makes: 24 Crossbow, 22 Mace, 26 Sword, 18 Pike, 20 Bow, 28 Armour.
///
/// `Ui_DrawCount` takes the pair's second string for anything but ±1, and
/// group 8 index 29 is *"Armour"* again, so armour never pluralises.
const WEAPON_NOUN: [usize; l2_kingdom::tables::WEAPON_TYPE_COUNT] = [24, 22, 26, 18, 20, 28];


/// **`Panel_JobBlacksmith` (`0x00413155`), call for call** — the one job that is
/// a full-screen page,
///
/// ```text
/// File_ReadChunk("smithy.pl8", scratch, 200000, 0)
/// Sprite_WGenSprite(0, 0, 0x18)
/// g_spriteWidth = 0x1E; g_spriteHeight = 0x38; FUN_004B414A(0, 0x1A8, 0)
/// File_ReadChunk("hearth.pl8", scratch, 200000, 0)
/// Pl8_DrawFrameClipped(scratch, weapon + 0xB, 0, 0x1A8 - DAT_004D29E0[weapon])
/// Ui_DrawBox(0, 0x180, 0x1E, 6)
/// Eng_DrawString(74, 8, 0x10, 0x186, heading, 0x3F)
/// Ui_DrawCentred(75, 0, 0, 0x1CC, 0x1CC, body, 0x3F)
/// if (advancedFarming) { N " " 76/4 eff "%" 76/5 }  else { N " " 76/8 }
/// 76/6  Ui_DrawCount(industry[2].next_season, DAT_004D29C8[weapon])  76/7
/// Ui_DrawInsetRect(0x140, 0x186, 0x8E, 0x20)
/// Pl8_DrawFrame(Misc_cty, 0x2C, 0x146, 0x187); Ui_DrawNumber(cost.iron, '@', "", 0x16A, 0x18E)
/// Pl8_DrawFrame(Misc_cty, 0x2E, 0x189, 0x18A); Ui_DrawNumber(cost.wood, '@', "", 0x1B4, 0x18E)
/// ```
///
/// **Both `Ui_DrawNumber` suffixes are the empty string** — `DAT_004D3E9C` and
/// `DAT_004D3EA0` are two NULs in a run of zero bytes —
/// *"wood needed."* and *"iron needed."* are in the file and on no screen.
pub(super) fn blacksmith(pen: &Pen, ctx: &Ctx, canvas: &mut Canvas, c: &County, forge: usize) {
    let a = &ctx.assets.shell;
    let face = crate::shell::Face::Body;
    let weapon = c.weapon_type.min(l2_kingdom::tables::WEAPON_TYPE_COUNT - 1);

    if let Some(f) = a.sheet(SMITHY_SHEET).and_then(|s| s.frame(0)) {
        canvas.blit(&f, SMITHY_AT.0, SMITHY_AT.1);
    } else {
        for &(x0, y0, x1, y1) in &WEAPON_HOTSPOTS {
            let (dx, dy) = HOTSPOT_ORIGIN;
            pen.outline(canvas, x0 + dx, y0 + dy, x1 - x0, y1 - y0, ctx.assets.ink.border);
        }
    }
    // `FUN_004B414A(0, 0x1A8, 0)` — 480 × 56 of palette index 0.
    canvas.fill_rect(FLOOR_BAND.x, FLOOR_BAND.y, FLOOR_BAND.w, FLOOR_BAND.h, 0);
    if let Some(f) = a.sheet(HEARTH_SHEET).and_then(|s| s.frame(HEARTH_FIRST + weapon)) {
        canvas.blit(&f, 0, HEARTH_FLOOR - HEARTH_LIFT[weapon]);
    }

    pen.window(canvas, FOOTER_X, FOOTER_Y, FOOTER_COLS, FOOTER_ROWS, 0);
    let title = eng(ctx, JOB_GROUP, BLACKSMITH + 1, JOB_NAMES[BLACKSMITH]);
    pen.heading(canvas, SMITHY_TITLE_AT.0, SMITHY_TITLE_AT.1, &title, COUNT_RIGHT);
    let (x, y, width) = SMITHY_LINE;
    let s = eng(ctx, SMITHY_GROUP, 0, ours(SMITHY_GROUP, 0));
    pen.body_centred(canvas, x, y, width, &s, BODY_INK);

    // The workers line. Advanced Farming splits it in two and names the
    // efficiency; plain says only how many smiths. Both lead `'@'`; the first
    // number's suffix is `" "` (`&DAT_004D3E90`, `&DAT_004D3E98`) and the
    // efficiency's is `"%"` (`&DAT_004D3E94`).
    let workers = i32::from(c.labour[BLACKSMITH]);
    let x = pen.number_in(
        face,
        canvas,
        SMITHY_TEXT_X,
        SMITHY_ROW_WORKERS,
        workers,
        '@',
        " ",
        BODY_INK,
    );
    if ctx.game.kingdom.options.advanced_farming {
        let x = say(pen, ctx, canvas, INDUSTRY_GROUP, 4, x, SMITHY_ROW_WORKERS);
        let efficiency = i32::from(c.industry[Commodity::Weapons.index()].efficiency);
        let x =
            pen.number_in(face, canvas, x, SMITHY_ROW_WORKERS, efficiency, '@', "%", BODY_INK);
        say(pen, ctx, canvas, INDUSTRY_GROUP, 5, x, SMITHY_ROW_WORKERS);
    } else {
        say(pen, ctx, canvas, INDUSTRY_GROUP, 8, x, SMITHY_ROW_WORKERS);
    }

    // *"Will produce"* N *"next season."* — the weapons record's own forecast at
    // `+0x2A8 + 2*0x18`, which is `Industry::next_season`.
    let x = say(pen, ctx, canvas, INDUSTRY_GROUP, 6, SMITHY_TEXT_X, SMITHY_ROW_OUTPUT);
    let made = c.industry[Commodity::Weapons.index()].next_season;
    let x = count(pen, ctx, canvas, made, WEAPON_NOUN[weapon], x, SMITHY_ROW_OUTPUT);
    say(pen, ctx, canvas, INDUSTRY_GROUP, 7, x, SMITHY_ROW_OUTPUT);

    pen.inset(canvas, COST_WELL);
    let row = ctx.game.kingdom.tables.weapon[weapon];
    pen.misc_frame(canvas, COST_IRON_ICON.0, COST_IRON_ICON.1, COST_IRON_ICON.2);
    pen.number_in(face, canvas, COST_IRON_AT.0, COST_IRON_AT.1, row.iron, '@', "", BODY_INK);
    pen.misc_frame(canvas, COST_WOOD_ICON.0, COST_WOOD_ICON.1, COST_WOOD_ICON.2);
    pen.number_in(face, canvas, COST_WOOD_AT.0, COST_WOOD_AT.1, row.wood, '@', "", BODY_INK);

    // `FUN_00413526` — the fire, out of the sheet the painter's second read
    // left in the scratch buffer.
    if let Some(f) = a.sheet(HEARTH_SHEET).and_then(|s| s.frame(forge % FORGE_FRAMES)) {
        canvas.blit(&f, FORGE_AT.0, FORGE_AT.1);
    }
}


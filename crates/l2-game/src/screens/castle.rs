//! **The castle chooser** — `Screen_CastleBuild` (`0x00419789`), `g_screenId`
//! `0x1B`, `L2.eng` group 71.
//!
//! It was one of the shells in [`crate::screens::shells`]: it drew a window and
//! did nothing. It is **the only place a player can order a castle**, and a
//! county with no castle cannot be besieged, so without it half the campaign
//! layer had no way in — `docs/decisions.md` C27's shape, and the reason
//! [`l2_kingdom::County::castle_degraded`] had no reachable writer.
//!
//! # The five buttons and the OK, from the widget tables
//!
//! Two tables, and neither had been decoded:
//!
//! ```text
//! g_castleTypeWidgets  0x004DC818  five kind-1 rectangles -> CastleBuild_Select
//!     (17,270)-(95,415)  (96,270)-(209,415)  (210,270)-(290,415)
//!     (291,270)-(414,415)  (415,270)-(618,415)          hotspot ids 0 … 4
//! g_castleBuildWidgets 0x004DDB80  two kind-5 sprites   -> CastleBuild_Confirm
//!     tick  frame 29 at (432,440)  hotspot 1   OK
//!     cross frame 31 at (472,444)  hotspot 0   cancel
//! ```
//!
//! **The five are the five castle pictures laid side by side**, and their widths
//! differ because the pictures do: 79, 114, 81, 124 and 204 pixels, tiling x 17
//! to 618 with no gap. So *"five buttons and an OK"* is literally a row of five
//! castles you point at. The selection is `DAT_0056D898`, a plain 0…4 that
//! [`crate::screens::map`]'s sidebar seeds from the county's own castle.
//!
//! # The painter, address by address
//!
//! Two functions, and **20 of the 22 draws are in the second one**, which
//! `Screen_Draw` never calls: `Screen_DrawWidgets`' `0x1B` arm runs
//! `Screen_CastleBuildPanel(); Widget_Draw(0, 0, &g_castleBuildWidgets, 2)`
//! every frame. An enumeration that read the painter would report two.
//!
//! ```text
//! Screen_CastleBuild(firstFrame):                              0x00419789
//!   File_ReadChunk("cas_back.256", &DAT_004EA8A0, 0x300)   the palette only
//!   FUN_00408FCB("cas_back.pl8", 0x1E0)      the backdrop: 640 x 480 raw
//!   Ui_OkButton(stride - 0x1C, height - 0x1C, 1)    the corner OK (612, 452)
//!   if (DAT_004D2DD0[sel] != 0):
//!     FUN_0040AE12("caspics.pl8", buf, DAT_004D2DD0[sel] - 1)
//!     Blit_Raster(buf, 0x9E, 0x14, 0x140, 200)   the big picture (158, 20)
//!   File_ReadChunk("cas_bits.pl8", g_villani2Sheet, 150000)
//!   DAT_005440B8 = 1;  Screen_CastleBuildPanel()
//!
//! Screen_CastleBuildPanel():   (only when DAT_005440B8)       0x004198AA
//!   Pl8_DrawFrame(cas_bits, DAT_004D2DE8[sel])   frame sel, (19, 63) — the plate
//!   Pl8_DrawFrame(cas_bits, DAT_004D2E28[sel])   frame 5+sel, over the chosen strip
//!   FUN_0040328E(71, sel + 1, 0x1F6, 0x18, 0xA0, heading)
//!                                  "Wooden palisade." … "Royal castle." (502, 24)
//!   Pl8_DrawFrame(cas_bits, 0x0A, 0x208, 0x5C)      the stone caption (520, 92)
//!   Ui_DrawNumber(stone, '@', "", 0x230, 0x60)                      (560, 96)
//!   Pl8_DrawFrame(cas_bits, 0x0B, 0x208, 0x7C)       the wood caption (520, 124)
//!   Ui_DrawNumber(wood,  '@', "", 0x230, 0x80)                      (560, 128)
//!   if (standing != 0):
//!     Pl8_DrawFrame(cas_bits, 0x0C, DAT_004D2E64[standing] - 10, 0x110)
//!   Eng_DrawString(71, 8, 0x20C, 0xB4, body)         "will take"    (524, 180)
//!   Ui_DrawCount(workforce, 0x26, 0x1F2, 0xC4, body)  N Builder(s)  (498, 196)
//!   Ui_DrawCount(1, 0x42, 0x1FC, 0xD4, body)          1 Season      (508, 212)
//!   Eng_DrawString(71, 9, 0x20C, 0xE4, body)         "to build."    (524, 228)
//!   Ui_DrawBox(0x70, 0x1AC, 0x1A, 3)          the tax plaque (112, 428) 416 x 48
//!   Eng_DrawString(71, 0x10, 0x90, 0x1B4)   "Boosts tax revenues by" (144, 436)
//!   Ui_DrawNumber(bonus, ' ', " %", pen + 0x90, 0x1B4)
//!   Eng_DrawString(71, 0xF, 0xE0, 0x1C8)      "Start construction?"  (224, 456)
//!   Ui_DrawBox(8, 200, 8, 3)             the barracks plaque (8, 200) 128 x 48
//!   Eng_DrawString(71, 0xB, 0xC, 0xD2)        "Barracks for"          (12, 210)
//!   Ui_DrawNumber(cap, '@', " ", 0xC, 0xE2)                           (12, 226)
//!   Eng_DrawString(71, 0xC, pen + 0xE, 0xE2)  "troops."
//! ```
//!
//! **`71/6` *"of stone needed,"* and `71/7` *"of wood needed."* are not on this
//! screen.** `docs/screens-county.md` §11.2 attaches them to the two
//! `Ui_DrawNumber` lines; the panel draws `cas_bits.pl8` frames `0x0A` and
//! `0x0B` there instead — the captions are **artwork**, at x 520 with the
//! number at x 560 after them. The two strings are alive elsewhere:
//! `Castle_DrawStatusBlock` (`0x0041DEDB`), the map-information panel's castle
//! block, is their only consumer. [`STONE_NEEDED`] and [`WOOD_NEEDED`] name
//! them here so the next reader does not go looking on this screen.
//!
//! **Nor is `71/0` *"Select a castle to build"*.** Grepping every
//! `Eng_DrawString`, `Ui_DrawCentred` and `FUN_0040328E` call with a **literal**
//! group-71 index finds 6, 7, 8, 9, 0xB, 0xC, 0xD, 0xE, 0xF, 0x10, 0x11, 0x12
//! and 0x13 — and neither 0 nor 0xA *"Build this castle"*. Index 0 of a group
//! is the group's own label (`docs/formats/eng.md` §5) and this heading is
//! painted into `cas_back.pl8`; 0xA is very likely a tooltip and is **[I]**.
//! Our shell drew index 0 as a caption of our own, which was an invention twice
//! over — the words and the font.
//!
//! **The stone and the wood are net of the castle already standing** — the panel
//! subtracts `g_castleMaterial[existing type]` before printing, which is the
//! same difference [`l2_kingdom::industry::order_castle`] charges, and it can
//! come out negative. It is printed as it comes.
//!
//! # `caspics.pl8` has four pictures for five castles  **[V]**
//!
//! `DAT_004D2DD0` is `{1, 0, 2, 3, 4}`, one-based with **0 meaning none**, and
//! the guard is `if (DAT_004D2DD0[sel] != 0)`. So selection 1, the motte and
//! bailey, blits **no big picture at all** and the backdrop shows through. The
//! shipped `Caspics.pl8` is 256,072 bytes, which is 72 of header and
//! `4 x 320 x 200` exactly — four rasters for four used slots, so the table and
//! the file close on each other and the gap is the original's, not a misread.
//!
//! # The five strips are hit rectangles over painted artwork
//!
//! `g_castleTypeWidgets` is tested with `Hotspot_Test`, never drawn: nothing in
//! either function paints the row of five castles at y 270…415. They are in
//! `cas_back.pl8`. What the panel *does* draw over them is one `cas_bits.pl8`
//! frame per selection — [`SELECTED_MARK`], frames 5…9 at five different
//! positions — and, when a castle already stands, frame `0x0C` at
//! [`STANDING_MARK_X`]`[type] - 10`.
//!
//! # The OK button's two refusals, and the one it does not have
//!
//! `CastleBuild_Confirm` (`0x00436B59`) has exactly two guards, both of which
//! close the screen with a message rather than staying on it:
//!
//! * the type picked is the one already standing — message `0x93`, `L2.eng`
//!   **147**: *"The castle in this county is already of the type you are
//!   proposing to change it to!!"*
//! * the type picked is **smaller** — message `0x122`, `L2.eng` **290**: *"Your
//!   current castle is stronger than the one you propose to upgrade to, my
//!   lord."*
//!
//! **There is no third guard.** You may order a royal castle with an empty
//! store; see [`l2_kingdom::industry::order_castle`] for what that buys you.

use l2_kingdom::industry::{self, CastleRefusal};
use l2_view::{text, Canvas};

use crate::input::{Event, Key, Rect};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::shell::{self, font, Pen};

// ---------------------------------------------------------------------------
// `L2.eng` group 71, and every index this screen draws
// ---------------------------------------------------------------------------

/// `L2.eng` group 71 — the literal first argument of all six `Eng_DrawString`
/// sites in `Screen_CastleBuildPanel` and of its one `FUN_0040328E`.
///
/// **Verified against the words, not against the indices existing.** Group 71
/// reads 0 *"Select a castle to build"*, 1…5 the five castle names in the order
/// [`l2_kingdom::tables::CASTLE_COST`] costs them, 6 *"of stone needed,"*,
/// 7 *"of wood needed."*, 8 *"will take"*, 9 *"to build."*, 0xB *"Barracks
/// for"*, 0xC *"troops."*, 0xF *"Start construction?"*, 0x10 *"Boosts tax
/// revenues by"*. Every one of those is a fragment of a sentence this panel
/// assembles, which is the check `docs/draws.md` §6 says existence is not.
pub const GROUP: usize = 71;

/// **Not drawn by this screen, and by nothing else in the binary.** The heading
/// is painted into `cas_back.pl8`; index 0 is the group's own label.
pub const TITLE: usize = 0;
/// 1…5, `FUN_0040328E(71, sel + 1, …)`.
pub const NAME_BASE: usize = 1;
/// **Not drawn here.** `Castle_DrawStatusBlock` (`0x0041DEDB`) draws it; this
/// panel puts `cas_bits.pl8` frame [`STONE_CAPTION`] in its place.
pub const STONE_NEEDED: usize = 6;
/// **Not drawn here** — see [`STONE_NEEDED`]; the artwork is [`WOOD_CAPTION`].
pub const WOOD_NEEDED: usize = 7;
pub const WILL_TAKE: usize = 8;
pub const TO_BUILD: usize = 9;
pub const BARRACKS_FOR: usize = 0xB;
pub const TROOPS: usize = 0xC;
pub const START_CONSTRUCTION: usize = 0xF;
pub const BOOSTS_TAX: usize = 0x10;

/// `Ui_DrawCount(workforce, 0x26, …)` — `L2.eng` group 8 `0x26`/`0x27`,
/// *"Builder"* / *"Builders"*. **[V]** against the words.
pub const BUILDER_NOUN: usize = 0x26;
/// `Ui_DrawCount(1, 0x42, …)` — group 8 `0x42`/`0x43`, *"Season"* /
/// *"Seasons"*, and the value is the **literal 1**: every castle takes one
/// season regardless of type, which is a rule stated only by this draw call.
pub const SEASON_NOUN: usize = 0x42;

// ---------------------------------------------------------------------------
// the three sheets
// ---------------------------------------------------------------------------

/// `FUN_00408FCB("cas_back.pl8", 0x1E0)` — a raw 640 × 480 raster read straight
/// into the display buffer. The shipped file is 307,224 bytes, which is
/// `640 * 480 + 24`. The five castle pictures the player points at are in it.
pub const BACKDROP: &str = "Cas_back.pl8";
/// `FUN_0040AE12("caspics.pl8", …)` then `Blit_Raster` — the big preview.
pub const PICS: &str = "Caspics.pl8";
/// `File_ReadChunk("cas_bits.pl8", g_villani2Sheet, 150000)` — the plates, the
/// selection marks and the two material captions.
pub const BITS: &str = "Cas_bits.pl8";

/// `DAT_004D2DD0` — the `caspics.pl8` frame for each selection, **one-based,
/// and 0 means no picture**. Selection 1, the motte and bailey, has none.
pub const PICTURE_FRAME: [usize; 5] = [1, 0, 2, 3, 4];

/// `Blit_Raster(buf, 0x9E, 0x14, 0x140, 200)`.
pub const PICTURE: Rect = Rect::new(0x9E, 0x14, 0x140, 200);

/// `DAT_004D2DE8`, `{frame, x, y}` five times — `cas_bits.pl8` frames 0…4, all
/// at **the same (19, 63)**: the name plate above the preview.
pub const NAME_PLATE: [(usize, i32, i32); 5] =
    [(0, 19, 63), (1, 19, 63), (2, 19, 63), (3, 19, 63), (4, 19, 63)];

/// `DAT_004D2E28` — `cas_bits.pl8` frames 5…9, one per selection, each over its
/// own strip in the row of five. **This is the only thing that marks the
/// selection**, and its x values land inside [`TYPE_BOUNDS`]' five rectangles.
pub const SELECTED_MARK: [(usize, i32, i32); 5] =
    [(5, 24, 325), (6, 103, 297), (7, 231, 292), (8, 307, 277), (9, 462, 285)];

/// `cas_bits.pl8` frame `0x0A` at (0x208, 0x5C) — the words *"of stone
/// needed,"* as **artwork**, which is why [`STONE_NEEDED`] is unused here.
pub const STONE_CAPTION: usize = 0x0A;
/// The same for the wood, frame `0x0B` at (0x208, 0x7C).
pub const WOOD_CAPTION: usize = 0x0B;
pub const CAPTION_X: i32 = 0x208;
pub const STONE_CAPTION_Y: i32 = 0x5C;
pub const WOOD_CAPTION_Y: i32 = 0x7C;
/// The two numbers, `Ui_DrawNumber(v, '@', "", 0x230, …)`.
pub const MATERIAL_NUM_X: i32 = 0x230;
pub const STONE_NUM_Y: i32 = 0x60;
pub const WOOD_NUM_Y: i32 = 0x80;

/// `cas_bits.pl8` frame `0x0C` — *the castle you already have*, drawn over its
/// strip only when one stands.
pub const STANDING_MARK: usize = 0x0C;
/// `DAT_004D2E64`, indexed by the **standing castle type** 1…5, and the painter
/// subtracts ten from it. Slot 0 is never read: `castleType == 0` skips the
/// draw.
pub const STANDING_MARK_X: [i32; 6] = [0, 52, 153, 253, 370, 538];
pub const STANDING_MARK_DX: i32 = -10;
pub const STANDING_MARK_Y: i32 = 0x110;

/// `FUN_0040328E(71, sel + 1, 0x1F6, 0x18, 0xA0, 100, 0, 0, heading, 0x3F)` —
/// the castle's name, wrapped at 160 pixels in the **22-pixel** font, which
/// steps `0x18` a line rather than `0x10`.
pub const NAME_AT: (i32, i32) = (0x1F6, 0x18);
pub const NAME_WIDTH: i32 = 0xA0;
pub const NAME_LINE: i32 = 0x18;

/// The workforce sentence, four calls on four lines.
pub const WILL_TAKE_AT: (i32, i32) = (0x20C, 0xB4);
pub const WORKFORCE_AT: (i32, i32) = (0x1F2, 0xC4);
pub const SEASON_AT: (i32, i32) = (0x1FC, 0xD4);
pub const TO_BUILD_AT: (i32, i32) = (0x20C, 0xE4);

/// `Ui_DrawBox(0x70, 0x1AC, 0x1A, 3)` — border set **0**, in 16-pixel cells, so
/// 416 × 48 at (112, 428).
pub const TAX_PLAQUE: (i32, i32, i32, i32) = (0x70, 0x1AC, 0x1A, 3);
pub const BOOSTS_TAX_AT: (i32, i32) = (0x90, 0x1B4);
pub const START_AT: (i32, i32) = (0xE0, 0x1C8);
/// `Ui_DrawBox(8, 200, 8, 3)` — 128 × 48 at (8, 200).
pub const BARRACKS_PLAQUE: (i32, i32, i32, i32) = (8, 200, 8, 3);
pub const BARRACKS_AT: (i32, i32) = (0xC, 0xD2);
pub const GARRISON_AT: (i32, i32) = (0xC, 0xE2);

/// `Ui_OkButton(g_screenStride - 0x1C, g_screenHeight - 0x1C, 1)` — the corner
/// picture, `System.pl8` frame `0x10`. It is drawn by `Screen_CastleBuild`
/// itself and is **not** one of the two widgets below.
pub const CORNER_OK: Rect = Rect::new(640 - 0x1C, 480 - 0x1C, 24, 24);

/// `System.pl8` frames 29 and 31 — the mailed hand, thumb up and thumb down.
/// Every yes/no pair in the game draws these two; `docs/screens-county.md` §4.2.
pub const THUMB_UP: usize = 29;
pub const THUMB_DOWN: usize = 31;

/// `g_castleTypeWidgets` (`0x004DC818`) — the five picture strips, as
/// `(x1, y1, x2, y2)` exactly as the kind-1 records hold them.
pub const TYPE_BOUNDS: [(i32, i32, i32, i32); 5] = [
    (17, 270, 95, 415),
    (96, 270, 209, 415),
    (210, 270, 290, 415),
    (291, 270, 414, 415),
    (415, 270, 618, 415),
];

/// One of the five, as a rectangle.
pub fn type_rect(level: usize) -> Rect {
    let (x1, y1, x2, y2) = TYPE_BOUNDS[level.min(4)];
    Rect::new(x1, y1, x2 - x1 + 1, y2 - y1 + 1)
}

/// `g_castleBuildWidgets` (`0x004DDB80`) record 0 — the tick, hotspot 1.
///
/// **The table is exactly two records long**, which is what the count of 2 the
/// `0x1B` arm passes should be checked against: `0x004DDB80 + 2 * 24` is
/// `0x004DDBB0`, and that is the base `Screen_DrawWidgets`' `0x12` arm passes.
/// So unlike `g_sendSuppliesWidgets` there is **no cut row here**.
pub const OK: Rect = Rect::new(432, 440, 32, 32);
/// …and record 1, the cross, hotspot 0.
pub const CANCEL: Rect = Rect::new(472, 444, 32, 32);

/// What the OK button did, for a caller that wants to know without reading the
/// county back.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CastleChoice {
    /// Still on the screen.
    None,
    /// The work is ordered.
    Ordered(u8),
    /// One of `CastleBuild_Confirm`'s two guards. The screen closes either way,
    /// which is the original's behaviour: the refusal is a message on the map,
    /// not a red light on the panel.
    Refused(CastleRefusal),
    /// The cross.
    Cancelled,
}

/// Screen `0x1B` for one county.
pub struct CastleScreen {
    county: u8,
    /// `DAT_0056D898` — the selected **level**, 0…4, which is `castleType - 1`.
    ///
    /// `None` until the player has clicked, because `Castle_OpenScreen`
    /// (`0x00436A88`) seeds it from the county and a screen cannot read the
    /// county at construction time. It is resolved on first use instead, which
    /// is the same value at the same moment.
    selected: Option<usize>,
    /// What the last click decided.
    pub choice: CastleChoice,
    /// The refusal line, for the status bar the map draws when we come back.
    pub message: Option<&'static str>,
}

impl CastleScreen {
    /// `Castle_OpenScreen` (`0x00436A88`) seeds the selection from the castle
    /// already standing, or 0 when there is none — so the screen opens showing
    /// what you have and the OK is refused until you move.
    pub fn new(county: u8) -> CastleScreen {
        CastleScreen { county, selected: None, choice: CastleChoice::None, message: None }
    }

    pub fn county(&self) -> u8 {
        self.county
    }

    /// The selection, resolved against the county the way `Castle_OpenScreen`
    /// seeds `DAT_0056D898`: the castle already standing, or 0 for a bare plot.
    pub fn level(&self, ctx: &Ctx) -> usize {
        self.selected.unwrap_or_else(|| {
            ctx.game.kingdom.counties[self.county as usize].castle_type.saturating_sub(1) as usize
        })
    }

    /// The castle type the selection names, 1..=5.
    pub fn castle_type(&self, ctx: &Ctx) -> u8 {
        self.level(ctx) as u8 + 1
    }

    /// `CastleBuild_Select` (`0x00436B22`) — `DAT_0056D898 = g_uiHotspotId`.
    /// It is a bare assignment: **there is no guard here at all**, so a player
    /// may select a castle smaller than the one he has and only learns
    /// otherwise from the OK button.
    pub fn select(&mut self, level: usize) {
        self.selected = Some(level.min(4));
    }

    /// The two numbers the panel prints, **net of the castle already there**.
    /// Either can be negative; the original prints what it computes.
    pub fn materials(&self, ctx: &Ctx) -> (i32, i32) {
        let t = &ctx.game.kingdom.tables;
        let (mut wood, mut stone) = industry::castle_cost(t, self.castle_type(ctx));
        let standing = ctx.game.kingdom.counties[self.county as usize].castle_type;
        if standing != 0 {
            let (had_wood, had_stone) = industry::castle_cost(t, standing);
            wood -= had_wood;
            stone -= had_stone;
        }
        (wood, stone)
    }

    /// `CastleBuild_Confirm`'s hotspot 1.
    fn confirm(&mut self, ctx: &mut Ctx) -> Transition {
        let want = self.castle_type(ctx);
        let t = ctx.game.kingdom.tables;
        let county = &mut ctx.game.kingdom.counties[self.county as usize];
        if let Some(why) = industry::castle_refusal(&t, county, want) {
            self.choice = CastleChoice::Refused(why);
            self.message = Some(match why {
                // `L2.eng` 147/1 and 290/1, the two the original sends.
                CastleRefusal::AlreadyBuilt => {
                    "THE CASTLE IN THIS COUNTY IS ALREADY OF THE TYPE YOU PROPOSE"
                }
                CastleRefusal::Downgrade => {
                    "YOUR CURRENT CASTLE IS STRONGER THAN THE ONE YOU PROPOSE, MY LORD"
                }
                CastleRefusal::NoSuchType => "THERE IS NO SUCH CASTLE",
            });
            return Transition::Pop;
        }
        let owner = county.owner as usize;
        let l2_kingdom::Kingdom { counties, realms, campaign, .. } = &mut ctx.game.kingdom;
        let Some(realm) = realms.get_mut(owner) else { return Transition::Pop };
        industry::order_castle(&t, &mut counties[self.county as usize], realm, want);
        // `Castle_Order` stamps the map in the same breath, and the scaffolding
        // is an obstacle from that moment: the plot stops being walkable and
        // starts being a castle. See [`l2_kingdom::map::stamp_castle_terrain`].
        l2_kingdom::map::stamp_castle_terrain(&mut campaign.map, self.county, want);
        self.choice = CastleChoice::Ordered(want);
        self.message = Some("CONSTRUCTION BEGINS");
        Transition::Pop
    }
}

impl Screen for CastleScreen {
    fn id(&self) -> ScreenId {
        ScreenId::Castle(self.county)
    }

    fn title(&self, _ctx: &Ctx) -> String {
        "Select a castle to build".to_string()
    }

    /// `File_ReadChunk("cas_back.256", …)` then `Palette_Set` — it is one of the
    /// six screens with a palette of its own.
    fn palette(&self) -> Option<&'static str> {
        Some("cas_back.256")
    }

    fn handle(&mut self, event: Event, ctx: &mut Ctx) -> Transition {
        match event {
            Event::KeyDown(Key::Escape) => {
                self.choice = CastleChoice::Cancelled;
                Transition::Pop
            }
            Event::KeyDown(Key::Enter) => self.confirm(ctx),
            Event::Click { x, y } => {
                if OK.contains(x, y) {
                    return self.confirm(ctx);
                }
                if CANCEL.contains(x, y) {
                    self.choice = CastleChoice::Cancelled;
                    return Transition::Pop;
                }
                // `Screen_FrameInput`'s `0x1B` arm: `Ui_OkButtonClicked()` →
                // `g_screenId = 0`, so the **corner** picture closes to the map
                // and orders nothing. It is a third way out that this screen
                // drew and did not answer.
                // arm: 0x0042FF10/castle-corner-ok
                if CORNER_OK.contains(x, y) {
                    self.choice = CastleChoice::Cancelled;
                    return Transition::Pop;
                }
                for level in 0..5 {
                    if type_rect(level).contains(x, y) {
                        self.select(level);
                        break;
                    }
                }
                Transition::Stay
            }
            _ => Transition::Stay,
        }
    }

    /// Every line here is one call site of `Screen_CastleBuild` or
    /// `Screen_CastleBuildPanel`, in the original's order, at the original's
    /// coordinate, through the original's fonts and sheets. Nothing on this
    /// screen is a caption of ours: the words it shows are `L2.eng` group 71
    /// and the pictures are `cas_back.pl8`, `caspics.pl8` and `cas_bits.pl8`.
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
        let level = self.level(ctx);
        let castle_type = level as u8 + 1;
        let standing = ctx.game.kingdom.counties[self.county as usize].castle_type;

        // `FUN_00408FCB("cas_back.pl8", 0x1E0)`. **The five castle pictures and
        // the "Select a castle to build" heading are in this image** — the
        // painter draws neither, so with no install there is nothing to draw
        // and the strips below say so instead.
        let have_backdrop = shell::background(canvas, a, BACKDROP);
        if !have_backdrop {
            canvas.clear(ink.background);
        }

        // `Ui_OkButton(stride - 0x1C, height - 0x1C, 1)`, mode 1 = frame 0x10.
        pen.ok_button(canvas, CORNER_OK.x, CORNER_OK.y, 1);

        // `if (DAT_004D2DD0[sel] != 0) Blit_Raster(caspics[n - 1], 0x9E, 0x14, …)`
        // — and for the motte and bailey it is zero, so nothing is blitted.
        if let Some(frame) = PICTURE_FRAME[level].checked_sub(1) {
            if let Some(f) = a.sheet(PICS).and_then(|s| s.frame(frame)) {
                canvas.blit_opaque(&f, PICTURE.x, PICTURE.y);
            }
        }

        // The two `cas_bits.pl8` plates: the name plate at (19, 63) and the
        // mark over the chosen strip.
        let bits = |canvas: &mut Canvas, frame: usize, x: i32, y: i32| {
            if let Some(f) = a.sheet(BITS).and_then(|s| s.frame(frame)) {
                canvas.blit(&f, x, y);
            }
        };
        let (frame, x, y) = NAME_PLATE[level];
        bits(canvas, frame, x, y);
        let (frame, x, y) = SELECTED_MARK[level];
        bits(canvas, frame, x, y);

        // `FUN_0040328E(71, sel + 1, 0x1F6, 0x18, 0xA0, 100, 0, 0, heading, 0x3F)`
        // — the castle's own name, wrapped at 160 pixels in the 22-pixel font,
        // which steps 0x18 a line and not 0x10.
        let name = a.text(GROUP, NAME_BASE + level).to_string();
        let mut line = NAME_AT.1;
        for part in heading_wrap(a, &name, NAME_WIDTH) {
            pen.heading(canvas, NAME_AT.0, line, &part, font::TEXT);
            line += NAME_LINE;
        }

        // The two materials: an artwork caption, then the number after it.
        // **`71/6` and `71/7` are not drawn here** — see the module docs.
        let t = &ctx.game.kingdom.tables;
        let (wood, stone) = self.materials(ctx);
        bits(canvas, STONE_CAPTION, CAPTION_X, STONE_CAPTION_Y);
        pen.number(canvas, MATERIAL_NUM_X, STONE_NUM_Y, stone, true, font::TEXT);
        bits(canvas, WOOD_CAPTION, CAPTION_X, WOOD_CAPTION_Y);
        pen.number(canvas, MATERIAL_NUM_X, WOOD_NUM_Y, wood, true, font::TEXT);

        // `if (castleType != 0) Pl8_DrawFrame(cas_bits, 0x0C, x[type] - 10, 0x110)`
        if standing != 0 {
            let x = STANDING_MARK_X[(standing as usize).min(5)] + STANDING_MARK_DX;
            bits(canvas, STANDING_MARK, x, STANDING_MARK_Y);
        }

        // "will take" / N Builders / 1 Season / "to build.", four lines.
        let work = industry::castle_workforce(t, castle_type);
        pen.eng(canvas, GROUP, WILL_TAKE, WILL_TAKE_AT.0, WILL_TAKE_AT.1, font::TEXT);
        pen.count(canvas, WORKFORCE_AT.0, WORKFORCE_AT.1, work, BUILDER_NOUN, true, font::TEXT);
        pen.count(canvas, SEASON_AT.0, SEASON_AT.1, 1, SEASON_NOUN, true, font::TEXT);
        pen.eng(canvas, GROUP, TO_BUILD, TO_BUILD_AT.0, TO_BUILD_AT.1, font::TEXT);

        // `Ui_DrawBox(0x70, 0x1AC, 0x1A, 3)` — border **set 0**, unlike the
        // court's and the trade panel's `FUN_004093E0`, which is set 1.
        pen.window(canvas, TAX_PLAQUE.0, TAX_PLAQUE.1, TAX_PLAQUE.2, TAX_PLAQUE.3, 0);
        let bonus = t.castle.tax_bonus_pct[level.min(t.castle.tax_bonus_pct.len() - 1)];
        let x = pen.eng(canvas, GROUP, BOOSTS_TAX, BOOSTS_TAX_AT.0, BOOSTS_TAX_AT.1, font::TEXT);
        // `Ui_DrawNumber(bonus, ' ', " %", …)` — a leading space, not the blank
        // glyph, and the per-cent sign is the suffix rather than the string's.
        pen.body(canvas, x, BOOSTS_TAX_AT.1, &format!(" {bonus} %"), font::TEXT);
        pen.eng(canvas, GROUP, START_CONSTRUCTION, START_AT.0, START_AT.1, font::TEXT);

        // `Ui_DrawBox(8, 200, 8, 3)`, then "Barracks for" / N "troops." — and
        // the number is on the **next line**, not after the label.
        pen.window(
            canvas,
            BARRACKS_PLAQUE.0,
            BARRACKS_PLAQUE.1,
            BARRACKS_PLAQUE.2,
            BARRACKS_PLAQUE.3,
            0,
        );
        let cap = industry::garrison_cap(t, castle_type);
        pen.eng(canvas, GROUP, BARRACKS_FOR, BARRACKS_AT.0, BARRACKS_AT.1, font::TEXT);
        let x = pen.number(canvas, GARRISON_AT.0, GARRISON_AT.1, cap, true, font::TEXT);
        // `Eng_DrawString(71, 0xC, g_penAdvance + 0xE, 0xE2, …)` — **`0xE`, two
        // pixels right of the number's own column**, the same nudge the court's
        // player name has.
        pen.eng(canvas, GROUP, TROOPS, x + 2, GARRISON_AT.1, font::TEXT);

        // `Widget_Draw(0, 0, &g_castleBuildWidgets, 2)` — the thumb up and the
        // thumb down, drawn from `Screen_DrawWidgets` rather than the painter.
        pen.system_frame(canvas, THUMB_UP, OK.x, OK.y);
        pen.system_frame(canvas, THUMB_DOWN, CANCEL.x, CANCEL.y);

        // ---- ours, and only when there is no artwork to point at -----------
        //
        // The five strips are `cas_back.pl8`'s pixels and `Hotspot_Test`'s
        // rectangles; nothing paints them. With no install there is nothing at
        // all in the row, so this names the five so the screen can be used —
        // and it is **our** font, deliberately, so a screenshot says which.
        if !have_backdrop {
            for strip in 0..5usize {
                let r = type_rect(strip);
                let colour = if strip == level { ink.highlight } else { ink.dim };
                text::draw(canvas, r.x + 4, r.y + 8, &format!("{}", strip + 1), colour);
            }
            text::draw(canvas, 4, 470, "CAS_BACK.PL8 IS NOT INSTALLED - STRIPS ARE OURS", ink.dim);
        }
    }
}

/// `FUN_0040328E`'s wrap, measured in the **heading** font.
///
/// [`Pen::wrap`] measures with the body font, and this one call site passes
/// `&g_fontHeading`; wrapping 22-pixel text against 14-pixel widths would put
/// too much on a line. Kept local because this is the only heading-font wrap in
/// the crate.
fn heading_wrap(a: &crate::shell::ShellAssets, s: &str, width: i32) -> Vec<String> {
    let measure = |t: &str| -> i32 {
        match &a.heading {
            Some(f) => f.width(t),
            None => l2_view::text::width(t),
        }
    };
    let mut out: Vec<String> = Vec::new();
    let mut line = String::new();
    for word in s.split_whitespace() {
        let next = if line.is_empty() { word.to_string() } else { format!("{line} {word}") };
        if !line.is_empty() && measure(&next) > width {
            out.push(std::mem::take(&mut line));
            line = word.to_string();
        } else {
            line = next;
        }
    }
    if !line.is_empty() {
        out.push(line);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **The five strips tile the row with no gap and no overlap**, which is
    /// what says the widget table was decoded at the right base address: a
    /// mis-aligned read would not produce five abutting rectangles.
    #[test]
    fn the_five_castle_strips_tile_the_row_exactly() {
        for level in 0..5 {
            let r = type_rect(level);
            assert_eq!(r.y, 270);
            assert_eq!(r.h, 146);
            assert!(r.w > 0);
        }
        for level in 0..4 {
            let a = type_rect(level);
            let b = type_rect(level + 1);
            assert_eq!(a.x + a.w, b.x, "strip {level} does not meet strip {}", level + 1);
        }
        assert_eq!(type_rect(0).x, 17);
        assert_eq!(type_rect(4).x + type_rect(4).w, 619);
    }

    /// **Three tables read at three base addresses agree with each other, five
    /// times each.**
    ///
    /// `g_castleTypeWidgets` (`0x004DC818`) holds the five hit rectangles;
    /// `DAT_004D2E28` holds the `cas_bits.pl8` selection mark for each
    /// selection; `DAT_004D2E64` holds the *"you already have this one"* mark
    /// for each standing castle type. Nothing in the binary relates them —
    /// they are read by two different functions in two different passes — and
    /// every one of the ten marks lands inside the strip it belongs to. A
    /// mis-aligned read of any of the three would put a mark in the wrong
    /// strip or off the row.
    ///
    /// The literals here are pinned from the decompilation rather than
    /// computed from the constants, so ablating a constant reddens the test.
    #[test]
    fn the_two_mark_tables_land_inside_the_five_strips() {
        let strips: [(i32, i32); 5] = [(17, 95), (96, 209), (210, 290), (291, 414), (415, 618)];
        for (i, &(x0, x1)) in strips.iter().enumerate() {
            // …and the pinned copy is the table's, so ablating TYPE_BOUNDS
            // reddens this too rather than only the two mark tables.
            assert_eq!((TYPE_BOUNDS[i].0, TYPE_BOUNDS[i].2), (x0, x1), "strip {i}");
            let (_, mx, my) = SELECTED_MARK[i];
            assert!((x0..=x1).contains(&mx), "selection mark {i} at x {mx} is not in {x0}..{x1}");
            assert!((270..=415).contains(&my), "selection mark {i} at y {my} is off the row");
            // The standing mark is indexed by castle **type**, so strip i is
            // type i + 1, and the painter subtracts ten before drawing.
            let sx = STANDING_MARK_X[i + 1] + STANDING_MARK_DX;
            assert!((x0..=x1).contains(&sx), "standing mark {i} at x {sx} is not in {x0}..{x1}");
        }
        assert_eq!(STANDING_MARK_X[0], 0, "slot 0 is never read: castleType 0 skips the draw");
    }

    /// **`caspics.pl8` has four big pictures for five castles**, and the file
    /// says so independently of the table: 256,072 bytes is 72 of header plus
    /// `4 * 320 * 200`, and [`PICTURE`] is 320 × 200.
    #[test]
    fn the_motte_and_bailey_alone_has_no_big_picture() {
        assert_eq!(PICTURE_FRAME, [1, 0, 2, 3, 4]);
        assert_eq!(PICTURE_FRAME.iter().filter(|&&f| f == 0).count(), 1);
        assert_eq!(PICTURE_FRAME[1], 0, "selection 1 is the motte and bailey");
        let mut used: Vec<usize> =
            PICTURE_FRAME.iter().filter(|&&f| f != 0).map(|&f| f - 1).collect();
        used.sort_unstable();
        assert_eq!(used, vec![0, 1, 2, 3], "four distinct frames, and no gap");
        assert_eq!((PICTURE.w, PICTURE.h), (320, 200));
    }

    /// The OK and the cancel are clear of the strips and of each other.
    #[test]
    fn the_two_buttons_are_clear_of_the_five() {
        for level in 0..5 {
            let r = type_rect(level);
            assert!(r.y + r.h <= OK.y, "strip {level} runs into the buttons");
        }
        // Both are read out of `g_castleBuildWidgets`, so a table re-read that moved
        // one onto the other should say so rather than compile away.
        assert!(
            core::hint::black_box(OK).x + OK.w <= CANCEL.x,
            "the tick and the cross overlap"
        );
    }
}

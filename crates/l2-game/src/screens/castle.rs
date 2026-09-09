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
//! # What it draws, out of `Screen_CastleBuildPanel` (`0x004198AA`)
//!
//! ```text
//! Blit_Raster(caspics.pl8[type], 0x9E, 0x14, 0x140, 200)   the big picture
//! FUN_0040328E(71, sel + 1, 0x1F6, 0x18)      "Wooden palisade." … "Royal castle."
//! Ui_DrawNumber(stone, '@', 0x230, 0x60)   + 71/6  "of stone needed,"
//! Ui_DrawNumber(wood,  '@', 0x230, 0x80)   + 71/7  "of wood needed."
//! Eng_DrawString(71, 8, 0x20C, 0xB4)              "will take"
//! Ui_DrawCount(workforce, 0x26, 0x1F2, 0xC4)      N man/men
//! Ui_DrawCount(1, 0x42, 0x1FC, 0xD4)              1 season(s)
//! Eng_DrawString(71, 9, 0x20C, 0xE4)              "to build."
//! Ui_DrawBox(0x70, 0x1AC, 0x1A, 3)                the tax plaque
//! Eng_DrawString(71, 0x10, 0x90, 0x1B4) + g_castleTaxBonus[sel]  "Boosts tax revenues by N%"
//! Eng_DrawString(71, 0xF, 0xE0, 0x1C8)            "Start construction?"
//! Ui_DrawBox(8, 200, 8, 3)                        the barracks plaque
//! Eng_DrawString(71, 0xB, 0xC, 0xD2) + cap + 71/0xC   "Barracks for N troops."
//! ```
//!
//! **The stone and the wood are net of the castle already standing** — the panel
//! subtracts `g_castleMaterial[existing type]` before printing, which is the
//! same difference [`l2_kingdom::industry::order_castle`] charges, and it can
//! come out negative. It is printed as it comes.
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
use l2_kingdom::tables::CASTLE_NAMES;
use l2_view::{text, Canvas};

use crate::input::{Event, Key, Rect};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::widget;

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
pub const OK: Rect = Rect::new(432, 440, 32, 32);
/// …and record 1, the cross, hotspot 0.
pub const CANCEL: Rect = Rect::new(472, 444, 32, 32);

/// `Blit_Raster(caspics.pl8[type], 0x9E, 0x14, 0x140, 200)`.
pub const PICTURE: Rect = Rect::new(0x9E, 0x14, 0x140, 200);

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

    fn draw(&mut self, ctx: &Ctx, canvas: &mut Canvas) {
        let ink = &ctx.assets.ink;
        canvas.clear(ink.background);

        // The big picture is `caspics.pl8` frame `DAT_004D2DD0[sel] - 1`, which
        // we have no sheet loaded for; the rectangle is the original's.
        widget::panel(canvas, ink, PICTURE);
        let level = self.level(ctx);
        let castle_type = level as u8 + 1;
        let name = CASTLE_NAMES[castle_type as usize].to_uppercase();
        text::draw(canvas, 0x1F6 - 180, 0x18, "SELECT A CASTLE TO BUILD", ink.highlight);
        text::draw(canvas, PICTURE.x + 8, PICTURE.y + 8, &name, ink.highlight);

        let t = &ctx.game.kingdom.tables;
        let (wood, stone) = self.materials(ctx);
        text::draw(canvas, 0x1D0, 0x60, &format!("{stone} OF STONE NEEDED,"), ink.text);
        text::draw(canvas, 0x1D0, 0x80, &format!("{wood} OF WOOD NEEDED."), ink.text);

        let work = industry::castle_workforce(t, castle_type);
        text::draw(canvas, 0x1B0, 0xB4, &format!("WILL TAKE {work} MEN"), ink.text);
        text::draw(canvas, 0x1B0, 0xC4, "1 SEASON TO BUILD.", ink.text);

        // The barracks plaque, `Ui_DrawBox(8, 200, 8, 3)` — 128 x 48 at (8, 200).
        let barracks = Rect::new(8, 200, 8 * 16, 3 * 16);
        widget::panel(canvas, ink, barracks);
        let cap = industry::garrison_cap(t, castle_type);
        text::draw(canvas, 0x0C, 0xD2, "BARRACKS FOR", ink.text);
        text::draw(canvas, 0x0C, 0xE2, &format!("{cap} TROOPS."), ink.text);

        // The tax plaque, `Ui_DrawBox(0x70, 0x1AC, 0x1A, 3)` — 416 x 48.
        let tax = Rect::new(0x70, 0x1AC, 0x1A * 16, 3 * 16);
        widget::panel(canvas, ink, tax);
        let bonus = t.castle.tax_bonus_pct[level.min(5)];
        text::draw(canvas, 0x90, 0x1B4, &format!("BOOSTS TAX REVENUES BY {bonus}%"), ink.text);
        text::draw(canvas, 0xE0, 0x1C8, "START CONSTRUCTION?", ink.text);

        // The five strips. The original blits the castle pictures here; ours
        // draws the frames at the widget table's own rectangles and names them,
        // and marks the one selected and the one already standing.
        let standing = ctx.game.kingdom.counties[self.county as usize].castle_type;
        for strip in 0..5usize {
            let r = type_rect(strip);
            let chosen = strip == level;
            widget::panel(canvas, ink, r);
            if chosen {
                widget::frame(canvas, r, ink.highlight);
            }
            let colour = if chosen { ink.highlight } else { ink.text };
            text::draw(canvas, r.x + 4, r.y + 8, &format!("{}", strip + 1), colour);
            if standing as usize == strip + 1 {
                text::draw(canvas, r.x + 4, r.y + 24, "HERE", ink.text);
            } else if (strip + 1) < standing as usize {
                text::draw(canvas, r.x + 4, r.y + 24, "LESS", ink.dim);
            }
        }

        widget::button(canvas, ink, OK, "OK", true);
        widget::button(canvas, ink, CANCEL, "X", true);
    }
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

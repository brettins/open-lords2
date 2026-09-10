//! **The map information panel** — `FUN_0041B032` (`0x0041B032`), `g_screenId`
//! `0x04`.
//!
//! # One screen, two painters, eleven layouts
//!
//! The shell table called it *"the map information panel"*, which is right and
//! is half the story: the painter is a **dispatcher** and everything it does is
//! choose between two others.
//!
//! ```c
//! void FUN_0041B032(void) {
//!   FUN_004B1DE0();                 /* an empty stub - 11 bytes, `return` */
//!   FUN_0045240A();                 /* dirty the whole 640 x 480          */
//!   if (g_pickedTileUnit == 0 && DAT_0052AFB4 == 0) FUN_0041BEFE();
//!   else                                            UnitPanel_Draw();
//!   FUN_00452160(1);
//!   DAT_004EB260 = 1;
//! }
//! ```
//!
//! * **`UnitPanel_Draw` (`0x0041B19D`)** — the unit half, four layouts by
//!   `unit.kind`: army, revolting peasants, merchant, transport.
//! * **`FUN_0041BEFE`** — the tile half, which is itself only a layout chooser:
//!   it computes the panel's top row, draws the frame and hands off to
//!   **`TileInfo_Draw` (`0x0041C208`)**. On farmland it also calls
//!   `FUN_0041C996`, **the field brush**.
//!
//! So `docs/screens.md`'s *"screen `0x04` is the field brush"* and
//! `docs/screens-county.md`'s *"the map information panel"* are both true and
//! both partial. The brush is one sub-case of the tile half.
//!
//! # `DAT_00553D2C` is a top row, and the panel's **bottom edge is pinned**
//!
//! Every y on this screen is `R * 16 + k` where `R` is `DAT_00553D2C`, and the
//! shell table's `0x04` row said so and stopped, refusing to invent a y. That
//! restraint was right and is now unnecessary, because the box arithmetic
//! closes:
//!
//! ```text
//! Ui_DrawBox(8, (R - C) * 16 + 32, 0x1C, (0x1B - R) + C)    the tile half
//! Ui_DrawBox(8,  R      * 16 + 32, 0x1C,  0x1B - R    )     the unit half
//! ```
//!
//! `top + height = 464` for **every** value of `R`, because the `R` terms
//! cancel. So `R` is the panel's top row in 16-pixel cells, the panel **grows
//! upward** as its content grows, and `DAT_005651C8` (`C`) is two extra cells
//! of head-room granted when the tile belongs to a county so the county's name
//! can be printed above the heading. [`Layout`] is that, and [`Layout::box_at`]
//! is the arithmetic.
//!
//! `R` takes eleven values across the two painters — 2, 5, 0x0A, 0x0C, 0x0E,
//! 0x0F, 0x10, 0x11, 0x12 — and [`Layout::ALL`] has every one with the
//! condition that produces it.
//!
//! # Three errors in the shell table's row, and one thing it had right
//!
//! > *"`FUN_0041B032` draws no `Ui_DrawBox`: it paints over the campaign map
//! > and its two halves place their own lines."*
//!
//! **False.** Both halves open with `Ui_DrawBox`, at `0x0041B1D9` and
//! `0x0041C1B0`. The comment reads like somebody looked at the dispatcher's own
//! 79 bytes — which contain no drawing at all — and concluded the callees did
//! not either. `window: None` follows from it and is wrong the same way.
//!
//! What the row **had right** was the refusal to place a line at a y it could
//! not derive. That is the discipline working, and the answer it was waiting
//! for is above.
//!
//! # And an error in `docs/screens-county.md`
//!
//! > *"…inside the branch that requires the unit to be an army of the local
//! > player's, so the county of origin shows for your own armies only."*
//!
//! **False.** `Eng_DrawString(31, 9, …)` — *"An army from"* — and the group-100
//! county name after it sit **before and outside** the ownership gate, inside
//! `else if (kind == 1)`. **Right-clicking an enemy army shows its county of
//! origin.** What the gate withholds is the inner inset, Formed and Wages, the
//! moves-left line, the three buttons, the troop grid and the mercenary line.
//!
//! # `L2.eng` 31/21, *"Morale"*, is dead text — and a `[V]` rested on it
//!
//! `docs/armies.md` §1 and `screens/map.rs` both list *"31/21 Morale"* among
//! *"the army info panel's own field labels, drawn by `UnitPanel_Draw` right
//! next to the offsets"*, and `armies.md` then rests the **[V]** on unit
//! `+0x166 morale` on it.
//!
//! **Nothing in `Lords2.exe` ever draws group 31 index 21.** Group 31 has
//! exactly two consumers — this painter and `FUN_0041B081` — and their
//! reachable index sets are `{0, 2, 5, 8, 9, 12…16, 20, 22, 23, 24, 25, 27…31}`
//! and `{17, 18}`. `+0x166` is written by `Merchant_SpawnAll` and read by
//! nothing on this panel. So 31/21 is dead text exactly like 31/26, which
//! `armies.md` §3.4 already flags, and **the `[V]` on the morale field needs a
//! different source.** [`DEAD_LABELS`] carries all ten.
//!
//! # The brush is 192 pixels lower than we draw it
//!
//! `screens/map.rs`'s `mod brush` has `ROW_Y: i32 = 184` with the comment
//! *"before its `g_uiPopupRow` offset"*. The original **always** applies that
//! offset and it always comes to `+192` — the field variant uses `R = 5` and
//! adds seven cells, `(5 + 7) * 16 = 192`; the waste variant uses `R = 0x0C`
//! and adds none, `12 * 16 = 192`. So the two variants land in the **same
//! absolute place**, the bevel is `(40, 368)–(424, 432)` and the buttons are at
//! **y 376…424**. [`BRUSH_ROW_Y`] is that number, and the x columns and the
//! 48-pixel button size `map.rs` already had are exact.
//!
//! # The three army buttons fire on press and the brush on release
//!
//! `g_infoUnitButtons` (`0x004DC560`) is **kind 1** — left *press* — and
//! `g_infoFieldBrush` (`0x004DC4D0`) is **kind 3** — left *release*. A real
//! behavioural difference between the two halves of one screen, reproduced.
//!
//! ```text
//! 0x004DC560  in the field                0x004DC5A8  garrisoned
//!   (48,352)  Panel_MoveButton  0x004371CE   (48,352)  FUN_004374C4  LEAVE THE CASTLE
//!   (112,352) Panel_DisbandButton 0x0043733A (112,352) Panel_DisbandButton
//!   (176,352) Panel_SplitButton 0x004378B3   (176,352) Panel_SplitButton
//! ```
//!
//! **`FUN_004374C4` is unnamed and it is a real action.** It finds a free tile
//! near the castle with `Map_FindFreeTileNear`, moves the army out, clears both
//! halves of the garrison link and starts a battle if the county was besieged —
//! or destroys the army if no tile is free. It is *leave the castle*, and a
//! name like `Army_LeaveCastle` is earned.
//!
//! # What is here and what is not
//!
//! The panel's **geometry, its layout ladder and its input arms** are here, and
//! the unit half draws every value it has state for. The tile half's eighty-odd
//! `L2.eng` group 30 descriptions are a table this module carries the shape of
//! and not the contents: [`TileKind`] is the ladder, and the strings come out
//! of the player's own `L2.eng`. `Icon_tmp.pl8` **is** loaded by
//! `crate::shell::ShellAssets` and until now no frame of it was drawn anywhere;
//! [`ICON`] is what changes that.

use l2_view::Canvas;

use crate::input::{Event, Key, Rect};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::shell::{font, Pen};

/// What the right click resolved to. **Part of the screen's identity**, because
/// the original keeps it in `g_pickedTileUnit`/`DAT_0056795C` and picks the
/// painter from it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    /// `g_pickedTileUnit != 0` — the unit half.
    Unit(usize),
    /// A tile index — the tile half.
    Tile(usize),
}

/// `L2.eng` group 31 — the unit panel's own strings, 32 of them.
pub const UNIT_GROUP: usize = 31;
/// `L2.eng` group 30 — the tile panel's, 88 of them.
pub const TILE_GROUP: usize = 30;
/// Group 100, county names, twenty per scenario.
pub const COUNTY_GROUP: usize = 100;

/// **The ten strings of group 31 that nothing in the binary draws**, with 21
/// among them.
///
/// Enumerated rather than asserted: group 31 has two consumers and their
/// reachable index sets were listed. 6 is the interesting one — it is *assigned*
/// to the heading local for an army and then explicitly suppressed by
/// `else if (local_20 != 6)`.
pub const DEAD_LABELS: [usize; 10] = [1, 3, 4, 6, 7, 10, 11, 19, 21, 26];

/// The panel's top row in 16-pixel cells, with the head-room the tile half
/// grants a county tile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Layout {
    /// `DAT_00553D2C`.
    pub row: i32,
    /// `DAT_005651C8` — 2 when the tile belongs to a county, else 0.
    pub headroom: i32,
}

impl Layout {
    /// `Ui_DrawBox(8, (row − headroom) * 16 + 32, 0x1C, (0x1B − row) + headroom)`
    /// — **and `top + height` is 464 for every one of them.**
    pub fn box_at(self) -> Rect {
        let top = (self.row - self.headroom) * 16 + 32;
        let rows = (0x1B - self.row) + self.headroom;
        Rect::new(8, top, 0x1C * 16, rows * 16)
    }

    /// Where a y written as `row * 16 + k` in the decompilation lands.
    pub fn y(self, k: i32) -> i32 {
        self.row * 16 + k
    }

    /// Every value the two painters take, with what produces it. The unit
    /// half's three never grant head-room; the tile half's grant it except for
    /// sea and a village.
    pub const ALL: [(&'static str, Layout); 11] = [
        ("army, yours", Layout { row: 2, headroom: 0 }),
        ("farmland, your county, a real field", Layout { row: 5, headroom: 2 }),
        ("castle degraded, your county", Layout { row: 0x0A, headroom: 2 }),
        ("farmland, your county, waste or reclaiming", Layout { row: 0x0C, headroom: 2 }),
        ("castle intact, no build in progress", Layout { row: 0x0E, headroom: 2 }),
        ("county town with a mercenary offer", Layout { row: 0x0F, headroom: 2 }),
        ("peasants, merchant or transport", Layout { row: 0x0F, headroom: 0 }),
        ("a village", Layout { row: 0x10, headroom: 0 }),
        ("the fallback: scrub, road, mountain, an enemy's anything", Layout { row: 0x11, headroom: 2 }),
        ("sea", Layout { row: 0x11, headroom: 0 }),
        ("army, not yours", Layout { row: 0x12, headroom: 0 }),
    ];
}

/// `Ui_OkButton(0x1AC, 0x1B6, 0)`, in **both** halves.
pub const OK: Rect = Rect::new(0x1AC, 0x1B6, 24, 24);

/// **`g_tilePanelWidgets` (`0x004DD640`) record 0 — *"View these troops?"***
///
/// `Widget_Test(8, 0x20, &DAT_004DD640, DAT_00568474)` and the matching
/// `Widget_Draw`, so the record's own `(336, 382)` lands at **(344, 414)**,
/// 24 square. `L2.eng` 71/14 is the caption above it.
///
/// The count is a **runtime global**: `TileInfo_DrawCastle` opens
/// `DAT_00568474 = (g_counties[g_pickedTileCounty].garrisonUnit != 0)`, so the
/// panel has one widget on a castle tile with a garrison and none anywhere
/// else. There is **no ownership gate** — the branch above it draws either
/// *"…currently stationed here."* for your own men or 71/19 *"Enemy troops are
/// barracked here."* for somebody else's, and offers the button either way.
pub const GARRISON_WIDGET: Rect = Rect::new(336 + 8, 382 + 0x20, 24, 24);
/// `L2.eng` group 71 — the castle block of the tile half. 14 is *"View these
/// troops?"*.
pub const CASTLE_GROUP: usize = 0x47;
pub const VIEW_THESE_TROOPS: usize = 0x0E;

/// The heading's column, and the icon's.
pub const HEADING_X: i32 = 0x28;
pub const HEADING_DY: i32 = 0x40;
pub const ICON_AT: (i32, i32) = (0x28, 0x60);
/// The wrapped description: `FUN_0040328E(group, i, 0x68, row*16 + 100, 0x120, …)`.
pub const BODY_X: i32 = 0x68;
pub const BODY_DY: i32 = 100;
pub const BODY_WRAP: i32 = 0x120;

/// `Icon_tmp.pl8` — **57 raw frames**, re-read from disk by both painters on
/// every repaint. `crate::shell::ShellAssets` already loads it and, until this
/// module, no frame of it was ever drawn.
pub const ICON_SHEET: &str = "Icon_tmp.pl8";

/// The five unit-kind icons, identified by rendering the frames.
pub mod icon {
    pub const MERCHANT: usize = 0x23;
    pub const TRANSPORT: usize = 0x24;
    pub const PEASANTS: usize = 0x25;
    pub const ENEMY_ARMY: usize = 0x26;
    pub const OWN_ARMY: usize = 0x27;
    /// The three action buttons: move, move-out-of-a-castle, disband, split.
    pub const MOVE: usize = 0x28;
    pub const SORTIE: usize = 0x29;
    pub const DISBAND: usize = 0x2A;
    pub const SPLIT: usize = 0x2B;
    /// The five brush buttons, and **the frame is not the brush id**:
    /// abandon 0 → `0x00`, reclaim `0x19` → `0x11`, fallow 1 → `0x01`,
    /// grain 2 → `0x0D`, pasture `0x13` → `0x16`. The lookup between them was
    /// not found in the binary; these are the frames the two brush painters
    /// pass literally.
    pub const BRUSH_ABANDON: usize = 0x00;
    pub const BRUSH_RECLAIM: usize = 0x11;
    pub const BRUSH_FALLOW: usize = 0x01;
    pub const BRUSH_GRAIN: usize = 0x0D;
    pub const BRUSH_PASTURE: usize = 0x16;
}

/// The unit half's per-kind constants: `(heading index, description index,
/// icon frame)`, and the row the panel opens at.
pub const MERCHANT: (usize, usize, usize) = (0, 12, icon::MERCHANT);
pub const PEASANTS: (usize, usize, usize) = (5, 13, icon::PEASANTS);
pub const TRANSPORT: (usize, usize, usize) = (2, 14, icon::TRANSPORT);
pub const ENEMY_ARMY: (usize, usize, usize) = (6, 15, icon::ENEMY_ARMY);
pub const OWN_ARMY: (usize, usize, usize) = (6, 16, icon::OWN_ARMY);

/// 31/9, *"An army from"* — **drawn for an enemy army too**, which is the
/// correction above.
pub const ARMY_FROM: usize = 9;
/// 31/20 *"Formed"*, 31/8 *"Wages"*, 31/22 *"moves left."* — all three inside
/// the ownership gate.
pub const FORMED: usize = 0x14;
pub const WAGES: usize = 8;
pub const MOVES_LEFT: usize = 0x16;
/// 31/23…26 the four supply states, 31/27…31 the five health states.
pub const SUPPLY0: usize = 0x17;
pub const HEALTH0: usize = 0x1B;
/// 16/0 *"No mercenaries in the army."*, 16/1…12 the nationalities.
pub const MERC_GROUP: usize = 16;

/// `local_c = max(0, 15 - movesUsed)` — the moves-left line, drawn **only when
/// the army is not garrisoned**.
pub const MOVE_ALLOWANCE: i32 = 15;

/// The three army buttons, 48 pixels square. `g_infoUnitButtons`
/// (`0x004DC560`) is **kind 1**: they fire on left *press*.
pub const BUTTON_DIM: i32 = 40;
pub const BUTTON_X: [i32; 3] = [48, 112, 176];
pub const BUTTON_DY: i32 = 352;

/// **The brush's absolute row**, which is 192 pixels below where `map.rs` puts
/// it. Both variants land here; see the module docs.
pub const BRUSH_ROW_Y: i32 = 376;
/// `Ui_DrawBevelRect(0x28, …, 0x180, 0x40)` — the bevel behind the buttons.
pub const BRUSH_BEVEL: Rect = Rect::new(40, 368, 384, 64);
/// The caption, `L2.eng` 30/52 — *"Click on an icon to alter field usage."*
pub const BRUSH_CAPTION_AT: (i32, i32, i32) = (48, 386, 192);
pub const BRUSH_CAPTION: usize = 52;
/// The three buttons of a live field and the two of waste, at their x columns.
/// `Hotspot_Test` is half-open and every box is 48 square.
pub const BRUSH_FIELD_X: [i32; 3] = [240, 304, 368];
pub const BRUSH_WASTE_X: [i32; 2] = [304, 368];
pub const BRUSH_DIM: i32 = 48;

/// What one brush button paints — the raw terrain the hotspot record carries.
pub const BRUSH_FIELD_ID: [u8; 3] = [1, 2, 0x13];
pub const BRUSH_WASTE_ID: [u8; 2] = [0x19, 0];

/// `TileInfo_Draw`'s ladder, as far as it selects a *layout*. The eighty-eight
/// group 30 strings are the player's data and not a table of ours.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TileKind {
    Road,
    Sea,
    Village,
    Mountain,
    Woodland,
    Farmland,
    CountyTown,
    Industry,
    Castle,
    Scrubland,
}

/// **The outermost pixel of a 640 × 480 screen**, which is what
/// `Map_EdgeScroll` calls an edge: `x == 0 || x == width - 1`, and the same for
/// `y`. See [`crate::screens::map::MapScreen::edge_direction`], which is the
/// same predicate on the screen that owns the scroll.
fn at_screen_edge(x: i32, y: i32) -> bool {
    x <= 0 || y <= 0 || x >= l2_view::canvas::WIDTH as i32 - 1 || y >= l2_view::canvas::HEIGHT as i32 - 1
}

pub struct InfoScreen {
    target: Target,
    /// One line of feedback about the last thing a button did. **Ours** — the
    /// original answers a refused disband with a message scroll we have not
    /// built.
    status: String,
}

impl InfoScreen {
    pub fn new(target: Target) -> InfoScreen {
        InfoScreen { target, status: String::new() }
    }

    pub fn target(&self) -> Target {
        self.target
    }

    /// The garrison this panel's tile would show, if its castle holds one.
    ///
    /// `FUN_00438ACC` is `g_pickedTileUnit = g_counties[g_pickedTileCounty]
    /// .garrisonUnit`, and `TileInfo_DrawCastle` is what decides the widget
    /// exists at all: a **castle tile** whose county has a garrison. Any owner.
    pub fn garrison(&self, ctx: &Ctx) -> Option<usize> {
        let Target::Tile(tile) = self.target else { return None };
        let map = &ctx.game.kingdom.campaign.map;
        if map.flags[tile] & l2_kingdom::map::flags::SETTLEMENT == 0
            || map.terrain[tile] <= l2_kingdom::map::terrain::CASTLE_PLOT
        {
            return None;
        }
        let county = map.county[tile] as usize;
        let unit = ctx.game.kingdom.counties.get(county).map_or(0, |c| c.garrison_unit);
        (unit != 0).then_some(unit)
    }

    /// The three army buttons, and which of the two tables they come from.
    ///
    /// `FUN_00437002` picks between `g_infoUnitButtons` (`0x004DC560`) and the
    /// garrisoned table (`0x004DC5A8`) on `unit.garrisonCounty`, behind three
    /// guards: a unit is picked, it is **kind 1**, and its owner is the local
    /// player. The two tables differ in one slot.
    fn unit_buttons(&self, ctx: &Ctx) -> Option<(usize, bool)> {
        let Target::Unit(id) = self.target else { return None };
        let u = ctx.game.kingdom.campaign.units.get(id)?;
        if u.kind != l2_kingdom::unit::UnitKind::Army || u.owner != ctx.game.player {
            return None;
        }
        Some((id, u.garrison_county != 0))
    }

    /// Which of the eleven layouts this panel is using.
    pub fn layout(&self, ctx: &Ctx) -> Layout {
        match self.target {
            Target::Unit(id) => {
                let k = &ctx.game.kingdom;
                match k.campaign.units.get(id) {
                    Some(u) if u.kind == l2_kingdom::unit::UnitKind::Army => {
                        if u.owner == ctx.game.player {
                            Layout { row: 2, headroom: 0 }
                        } else {
                            Layout { row: 0x12, headroom: 0 }
                        }
                    }
                    // Peasants, merchant and transport all take `0x0F`.
                    _ => Layout { row: 0x0F, headroom: 0 },
                }
            }
            // The tile half's full ladder needs the plane-0 flags and the
            // county's castle state; what is reproduced here is the two arms a
            // right click on the campaign map can actually reach today —
            // farmland of the player's own county, and everything else.
            Target::Tile(tile) => {
                let map = &ctx.game.kingdom.campaign.map;
                let county = map.county[tile];
                let mine = ctx
                    .game
                    .kingdom
                    .counties
                    .get(county as usize)
                    .is_some_and(|c| c.owner == ctx.game.player);
                if map.flags[tile] & l2_kingdom::map::flags::FARMLAND != 0 && mine {
                    let t = map.terrain[tile];
                    if t == 0 || t > 0x18 {
                        Layout { row: 0x0C, headroom: 2 }
                    } else {
                        Layout { row: 5, headroom: 2 }
                    }
                } else {
                    Layout { row: 0x11, headroom: 2 }
                }
            }
        }
    }

    /// Whether the field brush is offered: farmland, the player's own county,
    /// and not a tile the weather ruined this season.
    pub fn brush(&self, ctx: &Ctx) -> Option<&'static [u8]> {
        let Target::Tile(tile) = self.target else { return None };
        let map = &ctx.game.kingdom.campaign.map;
        if map.flags[tile] & l2_kingdom::map::flags::FARMLAND == 0 {
            return None;
        }
        let t = map.terrain[tile];
        if t == 0x17 || t == 0x18 {
            return None;
        }
        let county = map.county[tile];
        if !ctx.game.kingdom.counties.get(county as usize).is_some_and(|c| c.owner == ctx.game.player)
        {
            return None;
        }
        if t == 0 || t > 0x18 {
            Some(&BRUSH_WASTE_ID)
        } else {
            Some(&BRUSH_FIELD_ID)
        }
    }
}

impl Screen for InfoScreen {
    fn id(&self) -> ScreenId {
        ScreenId::Info(self.target)
    }

    fn title(&self, _ctx: &Ctx) -> String {
        match self.target {
            Target::Unit(id) => format!("Unit {id} — screen 0x04"),
            Target::Tile(t) => format!("Tile {t} — screen 0x04"),
        }
    }

    fn is_overlay(&self) -> bool {
        true
    }

    fn handle(&mut self, event: Event, ctx: &mut Ctx) -> Transition {
        let Event::Click { x, y } = event else {
            return match event {
                // The same button opens and closes it.
                // arm: 0x0042FF10/info-right-close
                Event::RightClick { .. } => Transition::Pop,
                // arm: ours/info-keyboard-close
                Event::KeyDown(Key::Escape) | Event::KeyDown(Key::Enter) => Transition::Pop,
                // **`Map_EdgeScroll` is the SECOND guard of the `0x04` arm and a
                // scroll CLOSES the panel**: pushing the pointer into the edge
                // of the screen with the information panel up puts you back on
                // the map. It is not a click at all, and nobody would guess it.
                //
                // ```c
                // if (Map_EdgeScroll()) { g_screenId = 0; FUN_0043CC56(); }
                // ```
                //
                // `Map_EdgeScroll` (`0x00432221`) returns 0 **at the far zoom**
                // — `if (g_battlePhase == 0 && g_mapZoom == 2) return 0;` — so
                // the gesture does nothing there, which is why this reads
                // [`crate::game::Game::map_zoom_far`] rather than closing on
                // any edge. Its other refusal, a message scroll being up, is
                // vacuous here: we have no message scroll.
                //
                // The *edge* is the outermost pixel of a 640 × 480 screen;
                // `main.rs` clamps a pointer in the letterbox border onto it,
                // so the gesture works at the edge of the window. That is
                // [`crate::screens::map::MapScreen::edge_direction`]'s own
                // argument and this is the same predicate.
                // arm: 0x0042FF10/info-edge-scroll-closes
                Event::Pointer { x, y } if !ctx.game.map_zoom_far && at_screen_edge(x, y) => {
                    Transition::Pop
                }
                _ => Transition::Stay,
            };
        };
        // **The campaign minimap is live under this panel**, and it was not:
        // `Screen_FrameInput`'s epilogue runs `Minimap_Click` on every press on
        // every screen id but `0x12`, and closes the management surface on a
        // hit. This screen swallowed the press instead, so the one control that
        // works from everywhere did not work from here.
        //
        // **Only the raster, not the column.** `0x04`'s arm does not run the
        // six sidebar guards — the village's and the four county panels' do,
        // and this one does not — so the sidebar, the county strip and the
        // split slider are all dead with the information panel up, and only the
        // 128 × 128 minimap is not. See `screens/court.rs` at the same arm and
        // `docs/arms.json` `0x0042FF10/minimap-closes-the-surface`, which is the
        // campaign map's half of it.
        // arm: 0x0042FF10/minimap-under-the-info-panel
        if l2_view::chrome::minimap_hit_area().contains(x, y) {
            return Transition::Pass;
        }
        // arm: 0x0042FF10/info-ok
        if OK.contains(x, y) {
            return Transition::Pop;
        }
        // `FUN_00438990` — the brush, on left **release**, and every button
        // closes the panel afterwards because `Field_SetType` sets
        // `g_screenId = 0`.
        // arm: 0x00438990/field-brush
        if let Some(ids) = self.brush(ctx) {
            let xs: &[i32] = if ids.len() == 3 { &BRUSH_FIELD_X } else { &BRUSH_WASTE_X };
            for (i, &bx) in xs.iter().enumerate() {
                let box_ = Rect::new(bx, BRUSH_ROW_Y, BRUSH_DIM, BRUSH_DIM);
                if box_.contains(x, y) {
                    if let Target::Tile(tile) = self.target {
                        let county = ctx.game.kingdom.campaign.map.county[tile] as usize;
                        let kind = match ids[i] {
                            0 => l2_kingdom::field::FieldType::Waste,
                            1 => l2_kingdom::field::FieldType::Fallow,
                            2 => l2_kingdom::field::FieldType::Grain,
                            0x13 => l2_kingdom::field::FieldType::Pasture,
                            _ => l2_kingdom::field::FieldType::Reclaiming,
                        };
                        let _ = ctx.game.kingdom.paint_field(county, tile, kind);
                    }
                    return Transition::Pop;
                }
            }
        }
        // **`FUN_00438A91` — the tile half's one widget.** It is tested between
        // the brush and the unit buttons, and it is the only control on this
        // screen that changes what the panel is *about* without leaving it:
        // `FUN_00438ACC` writes the county's garrison into `g_pickedTileUnit`
        // and calls the painter again, so the tile half becomes the unit half
        // in place. `g_screenId` never moves, which is why this is a mutation
        // of `self.target` and not a transition.
        //
        // `FUN_00438ACC` opens `if (g_mapZoom != 2)` and does nothing at the far
        // zoom, which is [`crate::game::Game::map_zoom_far`] here.
        // arm: 0x00438A91/info-garrison-widget
        if GARRISON_WIDGET.contains(x, y) && !ctx.game.map_zoom_far {
            if let Some(unit) = self.garrison(&Ctx { game: ctx.game, assets: ctx.assets }) {
                self.target = Target::Unit(unit);
                return Transition::Stay;
            }
        }
        // **`FUN_00437002` — the three army buttons, on left *press*** while the
        // brush above fires on release. Which three depends on
        // `unit.garrisonCounty`: `g_infoUnitButtons` (`0x004DC560`) in the field
        // and `0x004DC5A8` inside a castle, differing in the first slot only.
        if let Some((id, garrisoned)) = self.unit_buttons(&Ctx { game: ctx.game, assets: ctx.assets })
        {
            let l = self.layout(ctx);
            for (i, &bx) in BUTTON_X.iter().enumerate() {
                if !Rect::new(bx, l.y(BUTTON_DY - l.row * 16), BUTTON_DIM, BUTTON_DIM).contains(x, y)
                {
                    continue;
                }
                return match (i, garrisoned) {
                    // **`Panel_MoveButton` (`0x004371CE`) — the door to screen
                    // `0x10`.** Two statements: `g_screenId = 0` and
                    // `Map_BeginMoveSelection()`. Ours has no global to write,
                    // so the request goes on [`crate::game::Game`] and the map
                    // picks it up on its next tick; the pop is the `g_screenId
                    // = 0`.
                    //
                    // **The besieging case is not reproduced**: the original
                    // asks `L2.eng` 10/13 *"Lift the siege?"* through
                    // `Ui_OpenConfirm` first, and we have no confirm box. It is
                    // named in `docs/arms.json` rather than silently dropped.
                    // arm: 0x00437002/info-move
                    (0, false) => {
                        ctx.game.begin_move_order = Some(id);
                        Transition::Pop
                    }
                    // `FUN_004374C4` — **leave the castle**, the garrisoned
                    // table's first slot. Not built: see `docs/arms.json`
                    // `0x004374C4/info-leave-castle`.
                    (0, true) => {
                        self.status = "THE SORTIE IS NOT BUILT".into();
                        Transition::Stay
                    }
                    // **`Panel_DisbandButton` (`0x0043733A`)**, in both tables.
                    // It picks the county the men would join — the home county,
                    // or the one the army stands in when the home county has
                    // changed hands — and asks *"Disband army?"* only when that
                    // county is the owner's. Otherwise it raises message `0x91`,
                    // `L2.eng` group 145, and closes the panel.
                    //
                    // [`l2_kingdom::divide::disband_county`] is the two clauses
                    // and [`crate::game::Game::disband_army`] the whole of it,
                    // including the refusal. **The confirm box is not
                    // reproduced** — `Ui_OpenConfirm(6, …)` is `L2.eng` 10/6 —
                    // and neither is the message scroll, so the refusal is a
                    // status line of ours.
                    // arm: 0x00437002/info-disband
                    (1, _) => match ctx.game.disband_army(id) {
                        Ok((county, men)) => {
                            let name = super::county::county_name(&*ctx, county);
                            self.status = format!("{men} MEN WENT HOME TO {name}");
                            Transition::Pop
                        }
                        Err(_) => {
                            self.status =
                                "MARCH IT TO A COUNTY YOU RULE BEFORE DISBANDING".into();
                            Transition::Pop
                        }
                    },
                    // **`Panel_SplitButton` (`0x004378B3`)**, and it is a
                    // `Push` rather than a `Replace` because `0x11` goes
                    // **back to `0x04`**: every one of the division screen's
                    // three ways out — the turn-ended latch, the right release
                    // and the OK button — writes `g_screenId = 0x04` and not 0.
                    // That is `docs/arms.json`
                    // `0x0042FF10/back-one-rather-than-to-the-map`, whose note
                    // said ours reached the campaign map "because we have no
                    // unit panel to go back to". There is one now.
                    // arm: 0x004378B3/info-split
                    (2, _) => Transition::Push(ScreenId::Divide(id)),
                    _ => Transition::Stay,
                };
            }
        }
        Transition::Stay
    }

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
        let l = self.layout(ctx);
        let b = l.box_at();
        pen.window(canvas, b.x, b.y, b.w / 16, b.h / 16, 0);
        pen.ok_button(canvas, OK.x, OK.y, 0);

        let icon = |frame: usize, canvas: &mut Canvas| {
            if let Some(f) = a.sheet(ICON_SHEET).and_then(|s| s.frame(frame)) {
                canvas.blit(&f, ICON_AT.0, l.y(ICON_AT.1));
            }
        };

        match self.target {
            Target::Unit(id) => {
                let k = &ctx.game.kingdom;
                let Some(u) = k.campaign.units.get(id) else { return };
                use l2_kingdom::unit::UnitKind;
                let (heading, body, frame) = match u.kind {
                    UnitKind::Merchant => MERCHANT,
                    UnitKind::PeasantMob => PEASANTS,
                    UnitKind::Transport => TRANSPORT,
                    _ if u.owner == ctx.game.player => OWN_ARMY,
                    _ => ENEMY_ARMY,
                };
                icon(frame, canvas);
                if u.kind == UnitKind::Army {
                    // **Outside the ownership gate**: the name, "An army from"
                    // and the county. This is the correction the module docs
                    // record.
                    let w = pen.eng(canvas, UNIT_GROUP, ARMY_FROM, HEADING_X, l.y(0x4A), font::TEXT);
                    let name = super::county::county_name(ctx, u.home_county);
                    pen.body(canvas, HEADING_X + w, l.y(0x4A), &name, font::TEXT);
                } else {
                    // 31/6 is suppressed for an army by `local_20 != 6`; every
                    // other kind draws its heading here.
                    pen.eng(canvas, UNIT_GROUP, heading, HEADING_X, l.y(HEADING_DY), font::TEXT);
                }
                if u.kind != UnitKind::Army {
                    let s = a.text(UNIT_GROUP, body).to_string();
                    pen.body_wrapped(canvas, BODY_X, l.y(BODY_DY), BODY_WRAP, &s, font::TEXT);
                }
                if u.kind == UnitKind::Transport {
                    // `troops[0]` is grain and `troops[2]` cattle — group 8
                    // nouns `0x44` and `0x46`, and the two `Misc_cty` icons.
                    pen.misc_frame(canvas, 0x21, 0x38, l.y(0xA0));
                    pen.count(canvas, 0x60, l.y(0xA4), u.troops[0], 0x44, false, font::TEXT);
                    pen.misc_frame(canvas, 0x26, 0x104, l.y(0xA0));
                    pen.count(canvas, 0x12E, l.y(0xA4), u.troops[2], 0x46, false, font::TEXT);
                }
                if u.kind == UnitKind::Army && u.owner == ctx.game.player {
                    let w = pen.eng(canvas, UNIT_GROUP, FORMED, HEADING_X, l.y(0xA0), font::TEXT);
                    pen.number(canvas, HEADING_X + w, l.y(0xA0), u.year_formed, false, font::TEXT);
                    let w = pen.eng(canvas, UNIT_GROUP, WAGES, 0xF8, l.y(0xA0), font::TEXT);
                    pen.count(canvas, 0xF8 + w, l.y(0xA0), u.wages, 0, false, font::TEXT);
                    if u.garrison_county == 0 {
                        let left = (MOVE_ALLOWANCE - u.moves_used as i32).max(0);
                        let w = pen.number(canvas, 0xF8, l.y(0x170), left, true, font::TEXT);
                        pen.eng(canvas, UNIT_GROUP, MOVES_LEFT, 0xF8 + w, l.y(0x170), font::TEXT);
                    }
                    // The three buttons, and the sortie frame when garrisoned.
                    let first = if u.garrison_county == 0 { icon::MOVE } else { icon::SORTIE };
                    for (i, &frame) in [first, icon::DISBAND, icon::SPLIT].iter().enumerate() {
                        if let Some(f) = a.sheet(ICON_SHEET).and_then(|s| s.frame(frame)) {
                            canvas.blit(&f, BUTTON_X[i], l.y(BUTTON_DY - l.row * 16));
                        }
                    }
                    // The seven troop rows.
                    for t in 0..7usize {
                        let row = (t / 2) as i32 * 13;
                        let (cx, nx) = if t % 2 == 0 { (0x38, 0x58) } else { (0xF8, 0x118) };
                        pen.misc_frame(canvas, 0x2F + t, cx, l.y(0xBE + row));
                        pen.count(
                            canvas,
                            nx,
                            l.y(0xC2 + row),
                            u.troops[t],
                            0x34 + t * 2,
                            true,
                            font::TEXT,
                        );
                    }
                }
            }
            Target::Tile(tile) => {
                let map = &ctx.game.kingdom.campaign.map;
                // The county's name, centred over the box in the head-room the
                // layout granted.
                if l.headroom != 0 {
                    let name = super::county::county_name(ctx, map.county[tile]);
                    pen.heading_centred(canvas, 8, l.y(0x18), 0x1C0, &name, font::TEXT);
                }
                // The brush, if the tile is one of the player's fields.
                if let Some(ids) = self.brush(ctx) {
                    // `Ui_DrawBevelRect` - the reverse lighting of an inset.
                    crate::shell::button_recess(
                        canvas,
                        BRUSH_BEVEL.x,
                        BRUSH_BEVEL.y,
                        BRUSH_BEVEL.w,
                        BRUSH_BEVEL.h,
                    );
                    let xs: &[i32] = if ids.len() == 3 { &BRUSH_FIELD_X } else { &BRUSH_WASTE_X };
                    for (i, &bx) in xs.iter().enumerate() {
                        let frame = match ids[i] {
                            0 => icon::BRUSH_ABANDON,
                            1 => icon::BRUSH_FALLOW,
                            2 => icon::BRUSH_GRAIN,
                            0x13 => icon::BRUSH_PASTURE,
                            _ => icon::BRUSH_RECLAIM,
                        };
                        if let Some(f) = a.sheet(ICON_SHEET).and_then(|s| s.frame(frame)) {
                            canvas.blit(&f, bx, BRUSH_ROW_Y);
                        }
                    }
                    let s = a.text(TILE_GROUP, BRUSH_CAPTION).to_string();
                    pen.body_wrapped(
                        canvas,
                        BRUSH_CAPTION_AT.0,
                        BRUSH_CAPTION_AT.1,
                        BRUSH_CAPTION_AT.2,
                        &s,
                        font::TEXT,
                    );
                }
                // **`TileInfo_DrawCastle`'s widget** — 71/14 *"View these
                // troops?"* over `Widget_Draw(8, 0x20, &g_tilePanelWidgets, …)`,
                // whose count is 1 exactly when the county has a garrison.
                if self.garrison(ctx).is_some() {
                    pen.eng(
                        canvas,
                        CASTLE_GROUP,
                        VIEW_THESE_TROOPS,
                        BODY_X,
                        l.y(0xC4),
                        font::TEXT,
                    );
                    // `System.pl8` frame 25 is the tick, which is what the
                    // record's `+4` carries; our own button is the fallback for
                    // an install with no artwork.
                    if !pen.system_frame(canvas, 25, GARRISON_WIDGET.x, GARRISON_WIDGET.y) {
                        crate::widget::button(canvas, ink, GARRISON_WIDGET, "OK", true);
                    }
                }
                l2_view::text::draw(
                    canvas,
                    4,
                    470,
                    "TILE HALF: THE GROUP 30 LADDER IS NOT ALL DRAWN YET",
                    ink.dim,
                );
            }
        }
        // **Ours.** The original answers a refused disband with message `0x91`
        // on a scroll we have not built; this is the same sentence with nowhere
        // else to go.
        if !self.status.is_empty() {
            l2_view::text::draw(canvas, 12, 452, &self.status, ink.highlight);
        }
    }
}

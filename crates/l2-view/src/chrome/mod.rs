//! * **`MAPnn.PL8`** holds two 128 × 128 rasters per map slot, four slots to a
//!   file: a county id per pixel and a shading mask. The *colour* of the
//!   minimap is not in the file at all — it comes from an 8-bytes-per-realm
//!   ramp in `Lords2.exe` at `0x004D2900`, which is read out of the executable
//!   here.

mod minimap;
pub use minimap::*;
mod chrome;
pub use chrome::*;

use l2_formats::{DecodedFrame, Palette, Pl8};

use crate::canvas::{Canvas, Clip};
use crate::sheet::Sheet;


pub mod panels {
    pub const CORNER_TL: usize = 0;
    pub const CORNER_TR: usize = 1;
    pub const CORNER_BR: usize = 2;
    pub const CORNER_BL: usize = 3;
    pub const EDGE_TOP: usize = 4;
    pub const EDGE_BOTTOM: usize = 0x10;
    pub const EDGE_LEFT: usize = 0x1C;
    pub const EDGE_RIGHT: usize = 0x28;
    pub const EDGE_LEN: usize = 12;
    pub const TEXTURE: usize = 0x34;
    pub const TEXTURE_DIM: usize = 12;
    pub const STRIP: usize = 0xC4;
    pub const STRIP_LEN: usize = 8;
    pub const SET_B: usize = 0xCC;
    pub const CELL: i32 = 16;
    pub const STRIP_CELL: i32 = 24;
}


/// `g_miscCtySheet` (`0x005530C8`) holds this file, not `Misc_cty.pl8`, while a
/// battle is up — `DAT_0053F050` picks — so these indices and `misc_cty`'s are
/// two vocabularies over one slot. Each frame's own header carries the position
/// it is drawn at, and every one below agrees with its call site. **[V]**
pub mod misc_bat {
    pub const COLUMN: usize = 0;
    pub const BUTTONS: usize = 1;
    pub const COUNTS: usize = 2;
    pub const PAUSE_LIT: usize = 5;
    pub const RETREAT_LIT: usize = 6;
    pub const SHIELD: usize = 6;
}


pub mod misc_cty {
    pub const PANEL_TOP: usize = 54;
    pub const PANEL_OWN_A: usize = 55;
    pub const PANEL_OWN_B: usize = 66;
    pub const PANEL_OWN_C: usize = 56;
    pub const PANEL_FOREIGN: usize = 58;
    pub const PANEL_STATUS: usize = 57;
    pub const PANEL_END_TURN: usize = 59;
    pub const BANNER: usize = 0x55;
    pub const MINIMAP_SIDE: usize = 0x5C;
    pub const MINIMAP_SIDE_ACTIVE: usize = 0x5B;

    pub const SPLIT_THUMB: usize = 0x3D;
    pub const SPLIT_THUMB_IDLE: usize = 0x55;

    /// **The last row is six pairs, not four.** `FUN_004106C4` indexes both
    /// frames by a county byte at `+0x290` this project has not named, and the
    /// ringed run in the file is `0x4F` … `0x54` — six frames, matching
    /// `0x30` … `0x35` — so `n` reaches 5. That is a reading of how many frames
    /// exist, not of what the byte means, and none of the six is drawn yet.
    pub const RINGED_PAIRS: [(usize, usize); 9] = [
        (0x21, 0x4B),
        (0x26, 0x4C),
        (0x3F, 0x4D),
        (0x30, 0x4F),
        (0x31, 0x50),
        (0x32, 0x51),
        (0x33, 0x52),
        (0x34, 0x53),
        (0x35, 0x54),
    ];

    pub const CASTLE_PLAIN: usize = 0x40;
    pub const CASTLE_RINGED: usize = 0x4E;

    /// `FUN_00410502` (iron), `FUN_00410598` (stone) and `FUN_0041062E` (wood)
    /// are the same nine lines three times over: one `Pl8_DrawFrame` of a fixed
    /// frame, one `Ui_DrawDelta`, and the row counter. **They have no ringed
    /// twin and no shortfall frame** — only the blacksmith and the castle, of
    /// the five industry rows, react to their staffing.
    ///
    /// **`docs/draws-map.md` §2 names these three painters in the wrong
    /// order** — it has `0x00410502` as stone, `0x00410598` as wood and
    /// `0x0041062E` as iron. `CountyStrip_Draw`'s dispatch settles it: the
    /// right-hand list holds *labour slots*, and slot 4 (iron mining) calls
    /// `0x00410502`, slot 5 (stone quarrying) calls `0x00410598`, slot 6 (wood
    /// cutting) calls `0x0041062E`. `Unit_TrampleTile` agrees from the other
    /// side. `docs/decisions.md` C135.
    pub const INDUSTRY_IRON: usize = 0x2C;
    pub const INDUSTRY_STONE: usize = 0x2D;
    pub const INDUSTRY_WOOD: usize = 0x2E;
    pub const INDUSTRY_X: [i32; 3] = [600, 0x252, 0x252];

    /// `CountyStrip_DrawCastleIcon` picks one when the castle is still owed
    /// anything, and puts `L2.eng` 71/18 — *"Needed"* — under it:
    pub const CASTLE_NEEDS_STONE: usize = 0x19;
    pub const CASTLE_NEEDS_WOOD: usize = 0x1A;
    pub const CASTLE_NEEDS_BOTH: usize = 0x1B;

    pub const SHORTFALL_GRAIN: usize = 0x23;
    pub const SHORTFALL_CATTLE: usize = 0x29;

    pub const RING_COLOURS: [u8; 3] = [64, 65, 95];
}


/// `Res_LoadButtons` (`0x00499A1C`) loads `system2.pl8` for index 0 and
/// `system.pl8` for index 1 into the same 56,600-byte buffer, and the kingdom
/// screens ask for index 1 (`0x004BA3xx`, guarded on the in-game state being
/// 3). The two files are the same size and the same 84-frame layout, and the
/// difference is not cosmetic: **69 of `System2.pl8`'s 84 frames are entirely
/// index 0**, including every frame named below except [`OK`], while none of
/// `System.pl8`'s are. Loading `System2.pl8` draws the panels' arrows and
/// slider as nothing at all, which is how this was found.
///
/// Every button is a **normal/pressed pair**: `Widget_Draw` adds 1 to the frame
/// while the record's press timer at `+0x0D` is running, so the odd frame of
/// each pair is the pressed state. `docs/screens-county.md` §4.2.
pub mod system {
    /// The tax panel's up arrow, from `g_taxWidgets` (`0x004DD790`) record 0.
    pub const ARROW_UP: usize = 0x15;
    pub const ARROW_DOWN: usize = 0x17;
    pub const ARROW: i32 = 24;
    pub const OK: usize = 0x33;
    pub const OK_ALT: usize = 0x10;
    pub const OK_DIM: i32 = 24;
    /// The ration slider, from `Panel_RationSlider` (`0x00411FDE`).
    pub const SLIDER_CAP_LEFT: usize = 0x4A;
    pub const SLIDER_CAP_RIGHT: usize = 0x4B;
    pub const SLIDER_KNOB: usize = 0x4C;
    pub const SLIDER_CAP: i32 = 24;
    pub const SLIDER_KNOB_W: i32 = 10;
}

pub const PANEL_TOP_Y: i32 = 24;
pub const PANEL_MIDDLE_Y: i32 = 156;
pub const PANEL_OWN_B_Y: i32 = 250;
pub const PANEL_OWN_C_Y: i32 = 302;
pub const PANEL_STATUS_Y: i32 = 430;
pub const PANEL_END_TURN_Y: i32 = 460;

pub const MINIMAP_X: i32 = 478;
pub const MINIMAP_Y: i32 = 28;
pub const MINIMAP_HIT_X: i32 = 480;
pub const MINIMAP_HIT_Y: i32 = 25;
pub const MINIMAP_DIM: i32 = 128;
pub const MINIMAP_SIDE_X: i32 = MINIMAP_HIT_X + 0x83;
pub const MINIMAP_SIDE_Y: i32 = MINIMAP_HIT_Y + 7;
pub const MINIMAP_BADGE_X: i32 = MINIMAP_HIT_X + 5;
pub const MINIMAP_BADGE_Y: i32 = MINIMAP_HIT_Y + 5;


/// The realm colour ramp `Minimap_DrawOverlay` indexes, read out of
/// `Lords2.exe` at `0x004D2900`: eight bytes per realm colour, of which the
/// first four are used — `ramp[colour * 8 + (shade - 10)]`.
pub const MINIMAP_REALM_RAMP: [[u8; 4]; 6] = [
    [0x0A, 0x0B, 0x0C, 0x0D], // 0 — unowned: the shades unchanged
    [0x01, 0x0E, 0x0F, 0xF9], // 1
    [0x03, 0xF2, 0xF3, 0xFB], // 2
    [0x38, 0x35, 0x32, 0x2F], // 3
    [0x05, 0xF4, 0xF5, 0xFD], // 4
    [0x04, 0xFC, 0xF1, 0xF0], // 5
];
pub const MINIMAP_REALM_RAMP_VA: u32 = 0x004D_2900;

/// The **rating** ramp the three statistic overlays index, read out of
/// `Lords2.exe` at `0x004D28F8`: six bytes, **worst first**.
///
/// It is a *different* table from the realm ramp, not an extension of it: the
/// two are adjacent and the eight bytes between `0x004D28F8` and the realm
/// ramp's first row are this table plus two bytes nothing indexes.
pub const MINIMAP_RATING_RAMP: [u8; 6] = [0x0F, 0x15, 0xF3, 0x09, 0xF1, 0x05];
pub const MINIMAP_RATING_RAMP_VA: u32 = 0x004D_28F8;

pub const MINIMAP_SELECTED: u8 = 0x20;

pub const MINIMAP_SHADE_LO: u8 = 10;
pub const MINIMAP_SHADE_HI: u8 = 13;

/// **[V]**
///
/// A ten-byte table at `0x004DC1D0`, five `(pen, highlight)` pairs, and the
/// binary indexes it **from two bytes lower** so that the shield is 1-based:
///
/// `(&g_realmColour)[shieldIndex * 2]` with `g_realmColour` at `0x004DC1CE`.
///
/// The pen is stored in the save, at realm `+0x08`, and **nothing computes it
/// at draw time** — `CountyStrip_Draw` reads the byte.
///
/// written from this table, from the shield, by the two functions that hand a
/// realm its colour: `Realms_AssignLords` (`0x0049CAAA`) at new game and
/// `FUN_0042BA40` when a custom battle invents an opponent. Both do
///
/// ```c
/// g_realms[n].field_0x8 = (&g_realmColour)[g_realms[n].shieldIndex * 2];
/// g_realms[n].field_0x9 = (&DAT_004dc1cf)[g_realms[n].shieldIndex * 2];
/// ```
///
/// and `RefsTo` finds no other reader of the table and no other writer of the
/// field. So deriving the pen from the shield cannot get out of step with the
/// shield, which storing a second copy of it could. Checked against the user's
/// own saves: over the eleven fixture `.sav` files, realm `+0x08` equals this
/// table at `+0x0A` for **every realm in every one**, including the six where
/// realm 1 flies shield 5 — which is what separates "keyed by the shield" from
/// "keyed by the realm id".
pub const REALM_PEN: [[u8; 2]; 5] =
    [[0x0E, 0x0F], [0xFB, 0x0D], [0x3A, 0x20], [0x05, 0xFD], [0x04, 0xF0]];

pub const REALM_PEN_VA: u32 = 0x004D_C1D0;

/// **This does not clamp, and [`realm_colour`] does.** The clamp there is
/// `FUN_004171EE`'s, applied before using the byte as a *frame index*, where
/// A pen has an honest answer for "we do
/// not know this realm's colour", and it is not red: a zero shield that clamped
/// up to 1 would render as a plausible wrong colour and survive a canvas diff,
/// which is the failure that hid this bug. The caller falls back
/// visibly instead.
pub fn realm_pen(shield: u8) -> Option<u8> {
    REALM_PEN.get(shield.checked_sub(1)? as usize).map(|p| p[0])
}

/// The brighter of the pair, realm `+0x09`. Nothing in this tree draws with it
/// yet; it is here because it is the other half of the record and a reader who
/// finds one will want the other.
pub fn realm_pen_highlight(shield: u8) -> Option<u8> {
    REALM_PEN.get(shield.checked_sub(1)? as usize).map(|p| p[1])
}


pub struct Chrome {
    panels: Sheet,
    misc_cty: Sheet,
    misc_bat: Option<Sheet>,
    system: Option<Sheet>,
}

pub fn nearest(palette: &Palette, rgb: [u8; 3]) -> u8 {
    crate::ink::nearest(palette, rgb)
}

pub fn frame_size(sheet: &Sheet, index: usize) -> Option<(u16, u16)> {
    sheet.frame(index).map(|f: DecodedFrame| (f.width, f.height))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_panels_kit_partitions_its_frames_with_no_gap_and_no_overlap() {
        assert_eq!(panels::EDGE_TOP, panels::CORNER_BL + 1);
        assert_eq!(panels::EDGE_BOTTOM, panels::EDGE_TOP + panels::EDGE_LEN);
        assert_eq!(panels::EDGE_LEFT, panels::EDGE_BOTTOM + panels::EDGE_LEN);
        assert_eq!(panels::EDGE_RIGHT, panels::EDGE_LEFT + panels::EDGE_LEN);
        assert_eq!(panels::TEXTURE, panels::EDGE_RIGHT + panels::EDGE_LEN);
        assert_eq!(panels::TEXTURE, 4 + 4 * panels::EDGE_LEN);
        assert_eq!(panels::STRIP, panels::TEXTURE + panels::TEXTURE_DIM * panels::TEXTURE_DIM);
        assert_eq!(panels::SET_B, panels::STRIP + panels::STRIP_LEN);
        assert_eq!(panels::SET_B, 0xCC, "the offset Ui_DrawBoxBorder adds");
    }

    #[test]
    fn the_right_panel_frames_tile_the_column_exactly() {
        let (top, own_a, own_b, own_c, foreign, status, end) = (132, 94, 52, 128, 274, 30, 20);
        assert_eq!(PANEL_TOP_Y + top, PANEL_MIDDLE_Y);
        assert_eq!(PANEL_MIDDLE_Y + own_a, PANEL_OWN_B_Y);
        assert_eq!(PANEL_OWN_B_Y + own_b, PANEL_OWN_C_Y);
        assert_eq!(PANEL_OWN_C_Y + own_c, PANEL_STATUS_Y);
        assert_eq!(PANEL_MIDDLE_Y + foreign, PANEL_STATUS_Y, "the foreign layout ends level");
        assert_eq!(PANEL_STATUS_Y + status, PANEL_END_TURN_Y);
        assert_eq!(PANEL_END_TURN_Y + end, 480, "and the column reaches the bottom");
        assert_eq!(crate::campaign::PANEL_X + crate::campaign::PANEL_W, 640);
    }

    #[test]
    fn the_minimap_sits_flush_with_the_bottom_of_the_panels_top_frame() {
        assert_eq!(MINIMAP_Y + MINIMAP_DIM, PANEL_MIDDLE_Y);
        assert_eq!(MINIMAP_X, crate::campaign::PANEL_X);
        //...and the hit rectangle is offset from it
        assert_ne!((MINIMAP_X, MINIMAP_Y), (MINIMAP_HIT_X, MINIMAP_HIT_Y));
        assert_eq!(MINIMAP_HIT_X - MINIMAP_X, 2);
        assert_eq!(MINIMAP_Y - MINIMAP_HIT_Y, 3);
    }

    #[test]
    fn a_map_slot_names_its_file_and_its_two_frames() {
        assert_eq!(Minimap::file_for_slot(0), "Map01.pl8");
        assert_eq!(Minimap::frames_for_slot(0), (0, 1));
        assert_eq!(Minimap::file_for_slot(3), "Map01.pl8");
        assert_eq!(Minimap::frames_for_slot(3), (15, 16));
        assert_eq!(Minimap::file_for_slot(4), "Map02.pl8");
        assert_eq!(Minimap::file_for_slot(59), "Map15.pl8");
        assert_eq!(Minimap::frames_for_slot(59), (15, 16));
        for slot in 24..40 {
            let n = (slot >> 2) + 1;
            assert!((7..=10).contains(&n), "slot {slot} -> Map{n:02}");
        }
    }

    #[test]
    fn the_minimap_overlay_recolours_only_the_four_county_shades() {
        let n = (MINIMAP_DIM * MINIMAP_DIM) as usize;
        let mut m = Minimap { counties: vec![0; n], shades: vec![0; n] };
        for (i, s) in [10u8, 11, 12, 13, 63, 0].iter().enumerate() {
            m.shades[i] = *s;
            m.counties[i] = 3;
        }
        let mut c = Canvas::screen();
        c.clear(200);
        draw_minimap(&mut c, &m, 0, &MinimapTint::Owner(&|_| 2));

        fn at(c: &Canvas, i: i32) -> u8 {
            c.at((MINIMAP_X + i) as usize, MINIMAP_Y as usize)
        }
        assert_eq!([at(&c, 0), at(&c, 1), at(&c, 2), at(&c, 3)], MINIMAP_REALM_RAMP[2]);
        assert_eq!(at(&c, 4), 63, "the sea is drawn as the raster holds it");
        assert_eq!(at(&c, 5), 200, "index 0 is transparent");

        draw_minimap(&mut c, &m, 3, &MinimapTint::Owner(&|_| 2));
        assert_eq!(at(&c, 0), MINIMAP_SELECTED);
        assert_eq!(at(&c, 1), MINIMAP_REALM_RAMP[2][1], "the other three are untouched");
    }

    #[test]
    fn a_rating_overlay_paints_one_ramp_colour_over_all_four_shades() {
        let n = (MINIMAP_DIM * MINIMAP_DIM) as usize;
        let mut m = Minimap { counties: vec![0; n], shades: vec![0; n] };
        for (i, s) in [10u8, 11, 12, 13, 63, 0].iter().enumerate() {
            m.shades[i] = *s;
            m.counties[i] = 3;
        }
        fn at(c: &Canvas, i: i32) -> u8 {
            c.at((MINIMAP_X + i) as usize, MINIMAP_Y as usize)
        }
        let mut c = Canvas::screen();

        c.clear(200);
        draw_minimap(&mut c, &m, 0, &MinimapTint::Rating(&|_| Some(0)));
        assert_eq!(
            [at(&c, 0), at(&c, 1), at(&c, 2), at(&c, 3)],
            [MINIMAP_RATING_RAMP[0]; 4]
        );
        assert_eq!(at(&c, 4), 63, "the sea is still the raster's");
        assert_eq!(at(&c, 5), 200, "index 0 is still transparent");

        c.clear(200);
        draw_minimap(&mut c, &m, 0, &MinimapTint::Rating(&|_| Some(5)));
        assert_eq!([at(&c, 0), at(&c, 3)], [MINIMAP_RATING_RAMP[5]; 2]);
        assert_ne!(MINIMAP_RATING_RAMP[0], MINIMAP_RATING_RAMP[5]);

        for absent in [Some(6u8), None] {
            c.clear(200);
            draw_minimap(&mut c, &m, 0, &MinimapTint::Rating(&|_| absent));
            assert_eq!(
                [at(&c, 0), at(&c, 1), at(&c, 2), at(&c, 3)],
                MINIMAP_REALM_RAMP[0],
                "band {absent:?} must leave the picture alone"
            );
        }

        c.clear(200);
        draw_minimap(&mut c, &m, 3, &MinimapTint::Rating(&|_| Some(0)));
        assert_eq!(at(&c, 0), MINIMAP_SELECTED);
        assert_eq!(at(&c, 1), MINIMAP_RATING_RAMP[0]);
    }

    /// Every band any of the three ratings can produce is either a valid index
    /// into the six-entry ramp or the "draw nothing" 6 — **checked over the
    /// whole `u8` range**, because a six-entry table in this subsystem has run
    /// off its end once already (`docs/decisions.md` C50).
    #[test]
    fn no_rating_band_can_index_past_the_ramp() {
        let n = (MINIMAP_DIM * MINIMAP_DIM) as usize;
        let mut m = Minimap { counties: vec![0; n], shades: vec![10; n] };
        m.counties[0] = 3;
        let mut c = Canvas::screen();
        for band in 0..=255u8 {
            c.clear(200);
            draw_minimap(&mut c, &m, 0, &MinimapTint::Rating(&|_| Some(band)));
            let px = c.at(MINIMAP_X as usize, MINIMAP_Y as usize);
            if (band as usize) < MINIMAP_RATING_RAMP.len() {
                assert_eq!(px, MINIMAP_RATING_RAMP[band as usize]);
            } else {
                assert_eq!(px, 10, "band {band} must leave the raster's shade");
            }
        }
    }

    #[test]
    fn the_minimap_mode_buttons_map_to_modes_and_badges() {
        assert_eq!(MinimapMode::from_button(0), Some(MinimapMode::Labour));
        assert_eq!(MinimapMode::from_button(1), Some(MinimapMode::Food));
        assert_eq!(MinimapMode::from_button(2), Some(MinimapMode::Happiness));
        assert_eq!(MinimapMode::from_button(3), None, "button 4 is not a mode");
        assert_eq!(MinimapMode::default(), MinimapMode::Owner);
        assert!(!MinimapMode::Owner.is_rating());
        assert_eq!(MinimapMode::Owner.badge_frame(), None);
        assert_eq!(MinimapMode::Labour.badge_frame(), Some(0x5D));
        assert_eq!(MinimapMode::Food.badge_frame(), Some(0x5F));
        assert_eq!(MinimapMode::Happiness.badge_frame(), Some(0x5E));
    }

/// `FUN_004171EE`'s clamp, and the reason it lives here
    /// load path: colour 0 would index `Misc_cty` frame 85, which is 13 x 37
    /// and does not fit the 24-pixel menu bar, while 1..=5 land on the five
    /// 13 x 16 frames that do.
    #[test]
    fn a_realm_colour_is_clamped_to_the_five_frames_that_fit_the_bar() {
        assert_eq!(realm_colour(0), 1, "0 is not a colour");
        for c in 1..=5u8 {
            assert_eq!(realm_colour(c), c);
        }
        assert_eq!(realm_colour(6), 5);
        assert_eq!(realm_colour(255), 5);
        for raw in 0..=255u8 {
            let f = misc_cty::BANNER + realm_colour(raw) as usize;
            assert!((86..=90).contains(&f), "raw {raw} -> frame {f}");
        }
    }

    #[test]
    fn a_minimap_click_lands_on_the_county_under_it() {
        let n = (MINIMAP_DIM * MINIMAP_DIM) as usize;
        let mut m = Minimap { counties: vec![0; n], shades: vec![0; n] };
        m.counties[(7 * MINIMAP_DIM + 5) as usize] = 9;
        assert_eq!(m.county_at(MINIMAP_HIT_X + 5, MINIMAP_HIT_Y + 7), 9);
        assert_eq!(m.county_at(MINIMAP_HIT_X + 6, MINIMAP_HIT_Y + 7), 0);
        assert_eq!(m.county_at(MINIMAP_HIT_X - 1, MINIMAP_HIT_Y), 0, "outside is nothing");
        assert_eq!(m.county_at(MINIMAP_HIT_X, MINIMAP_HIT_Y + MINIMAP_DIM), 0);
        assert!(minimap_hit_area().contains(MINIMAP_HIT_X + 5, MINIMAP_HIT_Y + 7));
        assert!(!minimap_hit_area().contains(MINIMAP_HIT_X + MINIMAP_DIM, MINIMAP_HIT_Y));
    }
}


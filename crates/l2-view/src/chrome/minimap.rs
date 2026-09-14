#![allow(unused_imports)]
use super::*;
use super::chrome::*;
use l2_formats::{DecodedFrame, Palette, Pl8};
use crate::canvas::{Canvas, Clip};
use crate::sheet::Sheet;

/// The two 128 × 128 rasters `Minimap_Load` reads for one map slot.
pub struct Minimap {
    /// The county id under each minimap pixel. `Minimap_Click` reads this.
    pub counties: Vec<u8>,
    /// The shading mask. Only 0, 10..=13 and 63 occur in the shipped files:
    /// 10..=13 are land inside a county and get recoloured per owner, 63 is
    /// drawn as-is, and 0 is transparent.
    pub shades: Vec<u8>,
}

impl Minimap {
    /// `Minimap_Load`: `"map01.pl8" + (slot >> 2) * 0x10`, i.e. four map slots
    /// per file. The 11 files the game ships cover exactly the 44 used slots.
    pub fn file_for_slot(slot: usize) -> String {
        format!("Map{:02}.pl8", (slot >> 2) + 1)
    }

    /// The two frame indices for a slot: `(slot & 3) * 5` and `+ 1`.
    pub fn frames_for_slot(slot: usize) -> (usize, usize) {
        let base = (slot & 3) * 5;
        (base, base + 1)
    }

    pub fn load(bytes: &[u8], slot: usize) -> Result<Minimap, String> {
        let pl8 = Pl8::parse(bytes).map_err(|e| e.to_string())?;
        let (c, s) = Minimap::frames_for_slot(slot);
        let counties = pl8.decode(c).map_err(|e| e.to_string())?;
        let shades = pl8.decode(s).map_err(|e| e.to_string())?;
        let want = (MINIMAP_DIM * MINIMAP_DIM) as usize;
        if counties.indices.len() != want || shades.indices.len() != want {
            return Err(format!(
                "slot {slot}: frames {c}/{s} are {}x{} and {}x{}, not 128x128",
                counties.width, counties.height, shades.width, shades.height
            ));
        }
        Ok(Minimap { counties: counties.indices, shades: shades.indices })
    }

    /// The county at a minimap pixel, indexed as `Minimap_Click` does.
    pub fn county_at(&self, x: i32, y: i32) -> u8 {
        let (dx, dy) = (x - MINIMAP_HIT_X, y - MINIMAP_HIT_Y);
        if dx < 0 || dy < 0 || dx >= MINIMAP_DIM || dy >= MINIMAP_DIM {
            return 0;
        }
        self.counties[(dy * MINIMAP_DIM + dx) as usize]
    }
}

/// `g_minimapMode` (`0x0057A0C4`): what the minimap is coloured by.
///
/// The three statistic modes are the top three buttons of the strip beside the
/// minimap, and `Minimap_Draw` draws a badge for each in the minimap's top-left
/// corner — see [`MinimapMode::badge_frame`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MinimapMode {
    /// 0 — who owns each county, through [`MINIMAP_REALM_RAMP`].
    #[default]
    Owner,
    /// 1 — the labour rating, county `+0x03`. Badge: the peasant.
    Labour,
    /// 2 — the food rating, county `+0x02`. Badge: the loaf and cheese.
    Food,
    /// 3 — happiness, county `+0x01`. Badge: the heart.
    Happiness,
}

impl MinimapMode {
    /// `Minimap_ModeButton`'s hotspot id, 1…4.
    pub fn from_button(button: usize) -> Option<MinimapMode> {
        match button {
            0 => Some(MinimapMode::Labour),
            1 => Some(MinimapMode::Food),
            2 => Some(MinimapMode::Happiness),
            _ => None,
        }
    }

    /// The `Misc_cty` frame `Minimap_Draw` puts at (485, 30) while this mode is
    /// up — `0x5D` peasant, `0x5F` food, `0x5E` heart. **The order is not
    /// sequential**, and it is the original's: mode 2 draws `0x5F` and mode 3
    /// draws `0x5E`.
    pub fn badge_frame(self) -> Option<usize> {
        match self {
            MinimapMode::Owner => None,
            MinimapMode::Labour => Some(0x5D),
            MinimapMode::Food => Some(0x5F),
            MinimapMode::Happiness => Some(0x5E),
        }
    }

    pub fn is_rating(self) -> bool {
        self != MinimapMode::Owner
    }
}

/// How [`draw_minimap`] colours one county's land pixels.
pub enum MinimapTint<'a> {
    /// Mode 0: the county's realm colour, indexing [`MINIMAP_REALM_RAMP`].
    Owner(&'a dyn Fn(u8) -> u8),
    /// Modes 1…3: a rating band indexing [`MINIMAP_RATING_RAMP`].
    ///
    /// `None`, or a band of 6 or more, leaves the pixel as the raster drew it —
    /// which is what the original does, and is why a well-fed county shows
    /// nothing at all in food mode. See `l2_kingdom::county::MinimapBands`.
    Rating(&'a dyn Fn(u8) -> Option<u8>),
}

/// Clamp a realm's raw colour byte the way `FUN_004171EE` does before using it
/// as a frame index: 0 becomes 1, anything above 5 becomes 5.
///
/// The clamp belongs here and **not** on the load path. Realm `+0x0A` is stored
/// raw so that a misread offset reads back as zero and fails a test; clamping
/// on load would turn every realm into a plausible-looking colour 1.
pub fn realm_colour(raw: u8) -> u8 {
    raw.clamp(1, 5)
}

/// Draw the minimap: the shading raster recoloured per county
/// `Minimap_DrawOverlay` (`0x00410CBD`) does it.
///
/// A shade outside 10..=13 is left as the raster holds it, which is how the sea
/// (63) and the transparent surround (0) survive; the selected county's
/// brightest shade becomes [`MINIMAP_SELECTED`] **in every mode**, because the
/// original tests the selection before it dispatches on the mode.
///
/// `tint` is the mode: [`MinimapTint::Owner`] for the realm colours and
/// [`MinimapTint::Rating`] for the three statistic overlays.
pub fn draw_minimap(canvas: &mut Canvas, minimap: &Minimap, selected: u8, tint: &MinimapTint) {
    draw_minimap_at(canvas, minimap, (MINIMAP_X, MINIMAP_Y), selected, tint);
}

/// The same raster, at an origin the caller names.
///
/// **The send-supplies screen draws it somewhere else.** `FUN_00410A5D(county,
/// 0x60, 0x68)` is the two opening statements of `Minimap_Draw` with a
/// different origin — it blits at (94, 107)
/// — so the position is a parameter and [`draw_minimap`] is the sidebar's call
/// with the sidebar's constants.
pub fn draw_minimap_at(
    canvas: &mut Canvas,
    minimap: &Minimap,
    (ox, oy): (i32, i32),
    selected: u8,
    tint: &MinimapTint,
) {
    for y in 0..MINIMAP_DIM {
        for x in 0..MINIMAP_DIM {
            let i = (y * MINIMAP_DIM + x) as usize;
            let shade = minimap.shades[i];
            let county = minimap.counties[i];
            let (px, py) = (ox + x, oy + y);
            if shade == 0 {
                // Index 0 is transparent: the blitter leaves the panel behind.
                continue;
            }
            if !(MINIMAP_SHADE_LO..=MINIMAP_SHADE_HI).contains(&shade) {
                canvas.set(px as usize, py as usize, shade);
                continue;
            }
            let step = (shade - MINIMAP_SHADE_LO) as usize;
            let ink = if step == 0 && county != 0 && county == selected {
                MINIMAP_SELECTED
            } else {
                match tint {
                    MinimapTint::Owner(colour) => {
                        let c = colour(county).min(MINIMAP_REALM_RAMP.len() as u8 - 1);
                        MINIMAP_REALM_RAMP[c as usize][step]
                    }
                    // The original's guard is `band < 6`, and its three bands
                    // really do produce 6 — `goto` past the write, leaving the
                    // pixel the raster blitted. Reproduced literally rather
                    // than clamped: clamping would paint half the map purple.
                    MinimapTint::Rating(band) => match band(county) {
                        Some(b) if (b as usize) < MINIMAP_RATING_RAMP.len() => {
                            MINIMAP_RATING_RAMP[b as usize]
                        }
                        _ => shade,
                    },
                }
            };
            canvas.set(px as usize, py as usize, ink);
        }
    }
}

/// The rectangle a minimap click is tested against — `Minimap_Click`'s, not the
/// picture's.
pub const fn minimap_hit_area() -> Clip {
    Clip::new(
        MINIMAP_HIT_X,
        MINIMAP_HIT_Y,
        MINIMAP_HIT_X + MINIMAP_DIM,
        MINIMAP_HIT_Y + MINIMAP_DIM,
    )
}


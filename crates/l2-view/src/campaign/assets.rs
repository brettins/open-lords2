#![allow(unused_imports)]
use super::*;
use super::view::*;
use terrain::*;
use render::*;
use l2_formats::maps::{MapSlot, Plane, LATTICE_H, LATTICE_W, PLANE_DIM};
use crate::canvas::{Canvas, Clip, Tags};
use crate::sheet::Sheet;

/// The five tile banks **in every season**, the two sprite sheets and the flag
/// sheet at both zooms, decoded on demand.
///
/// # Why the banks are a pool and an index table
///
/// `Gfx_LoadCountyMode` does not hold four seasons at once: it frees the eight
/// buffers and reloads them from a different eight resource-table entries every
/// time the season turns. We hold all of them, because a `Sheet` decodes lazily
/// and re-reading five files on the season boundary would need the reader kept
/// alive for the life of the program.
///
/// That makes the repetition matter. The far zoom names the *same five files*
/// for all four seasons ([`FAR`]), so a naive `[[Sheet; 5]; 4]` per zoom would
/// hold four copies of `Base2a.pl8`. Interning by filename collapses those to
/// one and costs a string compare at load. Twenty-five names resolve to
/// **twenty** distinct files.
pub struct MapAssets {
    /// Every distinct bank file, decoded once. `Gfx_LoadCountyMode` repoints
    /// eight pointers at eight buffers; this is what they point into.
    banks: Vec<Sheet>,
    /// `[set][season slot][bank index]` → an index into `banks`, or `None` when
    /// that file would not load.
    bank_at: [[[Option<usize>; 5]; SEASONS]; 2],
    sprites: [Vec<Sheet>; 2],
    flags: [Option<Sheet>; 2],
}

impl MapAssets {
    /// Load through a caller-supplied reader, so this works equally against a
    /// plain directory and against the mod overlay's case-insensitive VFS.
    ///
    /// **Spring is required and the other three seasons are not.** Without
    /// spring's five banks; without summer's the map
    /// falls back on spring, which is a map that does not change with the year
    /// An install that is missing `Base1c.pl8` should still
    /// play, and a mod that ships one season should not have to ship four.
    ///
    /// The sprite and flag sheets are optional for the same reason: everything
    /// that draws from them falls back to a marker of ours, so a partial
    /// install still shows where its units are.
    pub fn load<F>(mut read: F) -> Result<MapAssets, String>
    where
        F: FnMut(&str) -> Result<Vec<u8>, String>,
    {
        let mut banks: Vec<Sheet> = Vec::new();
        let mut names: Vec<String> = Vec::new();
        let mut bank_at = [[[None; 5]; SEASONS]; 2];
        let mut sprites = [Vec::new(), Vec::new()];
        let mut flags = [None, None];
        for zoom in ZOOMS {
            for (season, set) in zoom.banks.iter().enumerate() {
                for (index, &name) in set.iter().enumerate() {
                    if let Some(i) = names.iter().position(|n| n == name) {
                        bank_at[zoom.set][season][index] = Some(i);
                        continue;
                    }
                    // Spring is the one the caller is entitled to an error
                    // about; a season that will not load is left `None` and
                    // resolves back to spring at draw time.
                    let sheet = match read(name).and_then(|b| {
                        Sheet::new(b).map_err(|e| format!("{name}: {e}"))
                    }) {
                        Ok(s) => s,
                        Err(e) if season == 0 => return Err(e),
                        Err(_) => continue,
                    };
                    names.push(name.to_string());
                    banks.push(sheet);
                    bank_at[zoom.set][season][index] = Some(banks.len() - 1);
                }
            }
            for name in zoom.sprites {
                if let Some(s) = read(name).ok().and_then(|b| Sheet::new(b).ok()) {
                    sprites[zoom.set].push(s);
                }
            }
            flags[zoom.set] = read(zoom.flags).ok().and_then(|b| Sheet::new(b).ok());
        }
        Ok(MapAssets { banks, bank_at, sprites, flags })
    }

    /// One tile bank, for a zoom and a `g_season` value.
    ///
    /// A season whose file would not load falls back to spring, which is
    /// `Gfx_LoadCountyMode`'s own behaviour for a season outside 1 … 4.
    pub fn bank(&self, zoom: &Zoom, season: u8, index: usize) -> Option<&Sheet> {
        let table = self.bank_at.get(zoom.set)?;
        let slot = season_slot(season);
        let at = table[slot].get(index).copied().flatten().or_else(|| {
            table[0].get(index).copied().flatten()
        })?;
        self.banks.get(at)
    }

/// How many distinct bank files were loaded. Twenty on a complete
    /// install: fifteen for the near zoom's four seasons and five for the far
    /// zoom's one.
    pub fn bank_files(&self) -> usize {
        self.banks.len()
    }

    /// Whether this season has artwork of its own, or is falling back on
    /// spring. A caller that wants to say "this install has no winter" can ask.
    pub fn has_season(&self, zoom: &Zoom, season: u8) -> bool {
        self.bank_at[zoom.set][season_slot(season)].iter().all(Option::is_some)
    }

    /// `g_spriteSheetA` (0) or `g_spriteSheetB` (1) for this zoom.
    pub fn sprite_sheet(&self, zoom: &Zoom, index: usize) -> Option<&Sheet> {
        self.sprites.get(zoom.set)?.get(index)
    }

    /// `g_flagsSheet` for this zoom.
    pub fn flag_sheet(&self, zoom: &Zoom) -> Option<&Sheet> {
        self.flags.get(zoom.set)?.as_ref()
    }
}


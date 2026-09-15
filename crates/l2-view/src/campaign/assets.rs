#![allow(unused_imports)]
use super::*;
use super::view::*;
use terrain::*;
use render::*;
use l2_formats::maps::{MapSlot, Plane, LATTICE_H, LATTICE_W, PLANE_DIM};
use crate::canvas::{Canvas, Clip, Tags};
use crate::sheet::Sheet;

pub struct MapAssets {
    banks: Vec<Sheet>,
    bank_at: [[[Option<usize>; 5]; SEASONS]; 2],
    sprites: [Vec<Sheet>; 2],
    flags: [Option<Sheet>; 2],
}

impl MapAssets {
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

    pub fn bank(&self, zoom: &Zoom, season: u8, index: usize) -> Option<&Sheet> {
        let table = self.bank_at.get(zoom.set)?;
        let slot = season_slot(season);
        let at = table[slot].get(index).copied().flatten().or_else(|| {
            table[0].get(index).copied().flatten()
        })?;
        self.banks.get(at)
    }

    pub fn bank_files(&self) -> usize {
        self.banks.len()
    }

    pub fn has_season(&self, zoom: &Zoom, season: u8) -> bool {
        self.bank_at[zoom.set][season_slot(season)].iter().all(Option::is_some)
    }

    pub fn sprite_sheet(&self, zoom: &Zoom, index: usize) -> Option<&Sheet> {
        self.sprites.get(zoom.set)?.get(index)
    }

    pub fn flag_sheet(&self, zoom: &Zoom) -> Option<&Sheet> {
        self.flags.get(zoom.set)?.as_ref()
    }
}


#![allow(unused_imports)]
use super::*;

use unit_frames::*;
use levy::*;
use game_methods::*;
use l2_formats::maps::{MapSet, MapSlot};
use l2_formats::Palette;
use l2_kingdom::county::{LABOUR_CEILING_IGNORED, MAX_COUNTIES};
use l2_kingdom::realm::MAX_REALMS;
use l2_kingdom::tables::JOB_IDLE_TOWNSFOLK;
use l2_kingdom::{Kingdom, SeasonReport};
use l2_mods::vfs::Vfs;
use l2_view::campaign::{self, MapAssets};
use l2_view::chrome::{Chrome, Minimap};
use l2_view::village::VillageArt;
use l2_view::Ink;
use crate::shell::ShellAssets;
use l2_kingdom::tables::MAX_TAX_RATE;
use l2_kingdom::county::MAX_RATION_SPLIT;

pub struct Assets {
    pub palette: Palette,
    pub ink: Ink,
    pub map: MapAssets,
    pub quirks: Quirks,
    pub wall_clock: Option<i64>,
    pub chrome: Option<Chrome>,
    pub village: Option<VillageArt>,
    /// `T32_bat1.pl8`, its palette and six colours of seven troop sheets — the
    /// battlefield's own artwork, loaded by `Battle_LoadAssets` (`0x004987B7`)
    /// and by nothing else.
    ///
    /// `None` on a partial install, and the battlefield then draws its own flat
    /// ground and a block for each man. That keeps every input arm testable
    /// without the install, and it is safe here for a reason the campaign map's
    /// hit test was not (`docs/decisions.md` C61): **every hotspot on this
    /// screen is a constant out of the binary**, not a consequence of the
    /// artwork,
    pub battle: Option<l2_view::scene::BattleAssets>,
    /// What the shell screens draw with: `L2.eng`, the two panel fonts, and
    /// the per-screen artwork the front end and the management screens load.
    pub shell: ShellAssets,
    pub films: crate::movie::FilmFiles,
    maps: Vec<u8>,
    minimap_files: Vec<Option<Vec<u8>>>,
}

impl Assets {
    pub fn load(vfs: &Vfs) -> Result<Assets, String> {
        let maps = vfs.read("L2_maps.dat").map_err(|e| format!("L2_maps.dat: {e}"))?;
        MapSet::parse(&maps).map_err(|e| format!("L2_maps.dat: {e}"))?;
        let palette = vfs
            .palette(campaign::PALETTE)
            .map_err(|e| format!("{}: {e}", campaign::PALETTE))?;
        let map = MapAssets::load(|name| vfs.read(name).map_err(|e| format!("{name}: {e}")))?;
        let chrome =
            Chrome::load(|name| vfs.read(name).map_err(|e| format!("{name}: {e}"))).ok();
        let village =
            VillageArt::load(|name| vfs.read(name).map_err(|e| format!("{name}: {e}"))).ok();
        let battle = l2_view::scene::BattleAssets::load(
            |name| vfs.read(name).map_err(|e| format!("{name}: {e}")),
            l2_view::figures::Colour::Red,
            l2_view::figures::Colour::Blue,
        )
        .ok();
        let minimap_files = (0..16)
            .map(|n| vfs.read(&Minimap::file_for_slot(n * 4)).ok())
            .collect();
        crate::castle::publish_from(|name| vfs.read(name).ok());
        crate::batfield::publish_from(|name| vfs.read(name).ok());
        Ok(Assets {
            ink: Ink::for_palette(&palette),
            quirks: Quirks::default(),
            wall_clock: None,
            palette,
            map,
            chrome,
            village,
            battle,
            shell: ShellAssets::load(vfs),
            films: crate::movie::FilmFiles::index(vfs),
            maps,
            minimap_files,
        })
    }

    pub fn slot(&self, index: usize) -> Option<MapSlot<'_>> {
        MapSet::parse(&self.maps).ok()?.slot(index).ok()
    }

    pub fn minimap(&self, slot: usize) -> Option<Minimap> {
        let bytes = self.minimap_files.get(slot >> 2)?.as_ref()?;
        Minimap::load(bytes, slot).ok()
    }

    pub fn placeholder() -> Assets {
        let mut palette_bytes = vec![0u8; Palette::FILE_LEN];
        for i in 0..256usize {
            let v = (i / 4) as u8;
            palette_bytes[i * 3] = v;
            palette_bytes[i * 3 + 1] = v;
            palette_bytes[i * 3 + 2] = v;
        }
        let palette = Palette::from_bytes(&palette_bytes).expect("768 bytes");

        let mut pl8 = vec![0u8; 8 + 16];
        pl8[2] = 1; // one frame
        pl8[8] = 2; // width
        pl8[10] = 2; // height
        pl8[12..16].copy_from_slice(&24u32.to_le_bytes());
        pl8.extend_from_slice(&[1, 2, 3, 4]);
        let map = MapAssets::load(|_| Ok(pl8.clone())).expect("a synthetic sheet parses");

        Assets {
            ink: Ink::for_palette(&palette),
            quirks: Quirks::default(),
            wall_clock: None,
            palette,
            map,
            chrome: None,
            village: None,
            battle: None,
            shell: ShellAssets::empty(),
            films: crate::movie::FilmFiles::default(),
            maps: vec![0u8; l2_formats::maps::SLOT_LEN],
            minimap_files: vec![None; 16],
        }
    }
}


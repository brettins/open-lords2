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

/// Everything the screens draw with. Not part of the world.
pub struct Assets {
    pub palette: Palette,
    pub ink: Ink,
    pub map: MapAssets,
    /// Presentation switches. See [`Quirks`].
    pub quirks: Quirks,
    /// **The shell's reading of the wall clock, in seconds since the Unix
    /// epoch — and the only one in the program.**
    ///
    /// `main.rs` samples `SystemTime` and projects it here once a tick, exactly
    /// as it projects [`Game::presentation_quirks`] into [`Assets::quirks`] on
    /// the line above; `crate::wallclock` turns it into the title screen's MST
    /// clock face and nothing else reads it. `None` everywhere the shell is not
    /// running — every test, and every headless driver —
    /// a clock has to be handed one and can never reach for it.
    ///
/// It is on [`Assets`] deliberately: `docs/netcode.md`
    /// D-5 forbids a wall clock in the simulation, and `Assets` is in no save,
    /// no digest and no `Kingdom`. `crate::wallclock` states the whole argument.
    pub wall_clock: Option<i64>,
    /// The original's interface artwork — `Panels.pl8` and `Misc_cty.pl8`.
    ///
    /// `None` when the install does not supply them, which is the placeholder
    /// case: every screen then falls back to its own flat panels, and looks it.
    /// That is deliberate — a stub that is visibly ours beats one that looks
    /// finished.
    pub chrome: Option<Chrome>,
    /// `vill.pl8`, `villtops.pl8` and `vill_gd8.pl8` — the village screen's own
    /// files, which no other screen loads.
    ///
    /// `None` on an install without them, and the village then draws its own
    /// ground and refuses to move anybody, because the grid that decides where
    /// a drop lands *is* one of those files.
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
    /// See [`crate::shell`].
    pub shell: ShellAssets,
    /// The install's 45 films, indexed and not read — see
    /// [`crate::movie::FilmFiles`]. Empty on an install without them, which is
    /// every DOS install: `Smk_Open` then fails and each caller's fail arm runs.
    pub films: crate::movie::FilmFiles,
    /// `L2_maps.dat` whole. A `MapSlot` borrows its file, so the bytes are kept
    /// and the slot is re-parsed on demand —
    /// decoding, and costs nothing.
    maps: Vec<u8>,
    /// The `MAPnn.PL8` files, by file number 1..=15, unparsed. Four map slots
    /// live in each and only one is ever wanted at a time, so they are decoded
    /// on demand by [`Assets::minimap`] and cached by the screen.
    minimap_files: Vec<Option<Vec<u8>>>,
}

impl Assets {
    /// Load through the mod overlay,
    /// or its own palette is picked up with no change to any drawing path.
    pub fn load(vfs: &Vfs) -> Result<Assets, String> {
        let maps = vfs.read("L2_maps.dat").map_err(|e| format!("L2_maps.dat: {e}"))?;
        MapSet::parse(&maps).map_err(|e| format!("L2_maps.dat: {e}"))?;
        let palette = vfs
            .palette(campaign::PALETTE)
            .map_err(|e| format!("{}: {e}", campaign::PALETTE))?;
        let map = MapAssets::load(|name| vfs.read(name).map_err(|e| format!("{name}: {e}")))?;
        // The chrome is optional: a partial install still starts, with our own
        // panels instead of the original's.
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
        // The game ships 11 of the 15 `MAPnn.PL8` names; the four it does not
        // are exactly the empty map slots 24..39 (`docs/screens.md` §3.1).
        let minimap_files = (0..16)
            .map(|n| vfs.read(&Minimap::file_for_slot(n * 4)).ok())
            .collect();
        // **`stnfield.pl8`, and it is not an asset this struct holds.** The
        // castle's layout is simulation, not artwork: `Battlefield_BuildCastle`
        // reads it in the middle of raising a battle, through no parameter at
        // all. `crate::castle` is that global; this is the one place with a
        // `Vfs` to fill it from.
        crate::castle::publish_from(|name| vfs.read(name).ok());
        // And `batfield.pl8` beside it, for the same reason:
        // `Battlefield_BuildRandom` reads the open field out of a process
        // global too. `crate::batfield`.
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

    /// The two 128 x 128 minimap rasters for a map slot, or `None` when the
    /// install has no `MAPnn.PL8` for it.
    pub fn minimap(&self, slot: usize) -> Option<Minimap> {
        let bytes = self.minimap_files.get(slot >> 2)?.as_ref()?;
        Minimap::load(bytes, slot).ok()
    }

    /// Assets with nothing in them: a grey ramp for a palette, one blank map
    /// slot, and five tile banks holding a single 2 x 2 frame.
    ///
    /// This is what lets the interface be tested on a machine with no copy of
    /// the game — every screen still lays out, every button is still where it
    /// is, and every assertion about *structure* still holds. Assertions about
    /// the shipped artwork need the install and live in the tests that skip
    /// without it.
    pub fn placeholder() -> Assets {
        // 256 greys, in the 6-bit range a `.256` file holds.
        let mut palette_bytes = vec![0u8; Palette::FILE_LEN];
        for i in 0..256usize {
            let v = (i / 4) as u8;
            palette_bytes[i * 3] = v;
            palette_bytes[i * 3 + 1] = v;
            palette_bytes[i * 3 + 2] = v;
        }
        let palette = Palette::from_bytes(&palette_bytes).expect("768 bytes");

        // The smallest legal PL8: one raw 2 x 2 frame.
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


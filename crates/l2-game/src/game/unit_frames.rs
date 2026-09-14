#![allow(unused_imports)]
use super::*;
use super::levy::*;
use super::game_methods::*;
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

/// **`+0x07` — the frame each unit's tick handler wrote, which is not the frame
/// its record would give now.**
///
/// `Army_Tick` (`0x0046521F`), `PeasantMob_Tick`, `Merchant_Tick` and
/// `Transport_Tick` all write the sprite frame from the unit's facing and walk
/// phase **first**, and only then call `Unit_Step`, which is what moves both.
/// `Map_DrawArmies` (`0x00408438`) reads the frame out of `+0x07` and the
/// position out of `+0x09` and `+0x149` as they stand. So on every tick a unit
/// steps, **the figure is drawn one walk phase behind where it is** — and on the
/// tick it turns a corner, it is drawn at the new heading's offset still facing
/// the old way. **[D]**, the order of two statements in four functions.
///
/// Our tick has nowhere to write `+0x07`, because the unit record is the world's
/// and a frame index is not (`docs/netcode.md`): two peers that disagreed about
/// which picture a merchant was showing would be playing the same game. So the
/// handlers' write is taken here, around the sweep, by [`Game::sweep_units`],
/// and read back by the painter through [`Game::unit_frame`].
///
/// **A held frame is believed only while nothing else has touched the unit.**
/// The original draws whatever the last tick wrote, even after a merge, a levy
/// or a garrison has changed the record under it — and on a fresh slot that is
/// frame 0, the merchant's first picture. That is not reproduced: the frame is
/// held with the pose the sweep left the unit in, and a unit whose pose has moved
/// since is drawn from its record as it stands.
#[derive(Debug, Clone)]
pub struct UnitFrames {
    held: [Option<HeldFrame>; l2_kingdom::unit::MAX_UNITS],
}

/// **Always equal.** Two games are the same game whatever picture a figure is
/// showing — the argument that keeps a frame index out of the lockstep digest
/// keeps it out of `Game`'s equality too. Without it a reloaded game, whose
/// first sweep has not run yet, is "different" from the one it was saved from
/// by exactly the frames `tests/save.rs` has no business comparing.
impl PartialEq for UnitFrames {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

impl Eq for UnitFrames {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct HeldFrame {
    frame: usize,
    pose: Pose,
}

/// Everything [`l2_kingdom::Unit::sprite_frame`] and the walk offset read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Pose {
    kind: l2_kingdom::UnitKind,
    size: usize,
    facing: u8,
    sub_tile: u8,
    at: (u8, u8),
}

impl Pose {
    fn of(u: &l2_kingdom::Unit) -> Pose {
        Pose { kind: u.kind, size: u.size_class(), facing: u.facing, sub_tile: u.sub_tile, at: u.tile() }
    }
}

impl Default for UnitFrames {
    fn default() -> Self {
        UnitFrames { held: [None; l2_kingdom::unit::MAX_UNITS] }
    }
}

impl UnitFrames {
    /// What every handler writes to `+0x07` on its way in, before `Unit_Step`.
    pub(super) fn written(
        units: &l2_kingdom::unit::Units,
    ) -> [Option<(l2_kingdom::UnitKind, usize)>; l2_kingdom::unit::MAX_UNITS] {
        let mut out = [None; l2_kingdom::unit::MAX_UNITS];
        for (id, u) in units.iter() {
            out[id] = Some((u.kind, u.sprite_frame(u.walk_phase())));
        }
        out
    }

    /// Keep those writes beside the pose the sweep left each unit in. A slot
    /// whose unit is not the kind it was on the way in is not the same unit.
    pub(super) fn hold(
        &mut self,
        written: [Option<(l2_kingdom::UnitKind, usize)>; l2_kingdom::unit::MAX_UNITS],
        units: &l2_kingdom::unit::Units,
    ) {
        self.held = [None; l2_kingdom::unit::MAX_UNITS];
        for (id, u) in units.iter() {
            if let Some((kind, frame)) = written[id] {
                if kind == u.kind {
                    self.held[id] = Some(HeldFrame { frame, pose: Pose::of(u) });
                }
            }
        }
    }

    pub(super) fn frame(&self, id: usize, unit: &l2_kingdom::Unit) -> usize {
        match self.held.get(id).copied().flatten() {
            Some(h) if h.pose == Pose::of(unit) => h.frame,
            _ => unit.sprite_frame(unit.walk_phase()),
        }
    }
}


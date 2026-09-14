#![allow(unused_imports)]
use super::*;
use super::layout::*;
use super::icons::*;
use crate::sheet::Sheet;
use crate::Canvas;

/// How many gates make one rung of the chain: 4 at 80 ms, 32 at 640 ms.
///
/// `Tick_Pulses` (`0x004BBC80`) counts *gates*, so this is the only honest
/// conversion from a rung's millisecond name to a period. Dividing the rung by
/// the tick length instead is the C179 error, and runs it 1.6 times fast.
pub fn gates_per_rung(period_ms: u32) -> u32 {
    (period_ms / GATE_MS).max(1)
}

impl AnimationClock {
    pub fn new() -> AnimationClock {
        AnimationClock::default()
    }

    /// Advance by one fixed tick of `tick_ms`, and say whether anything moved.
    ///
    /// A `false` is a screen that need not repaint — the same economy
    /// `MapScreen`'s flag phase makes.
    /// sixty repaints a second while a player thinks about it.
    pub fn tick(&mut self, tick_ms: u32) -> bool {
        let before = self.pulses;
        self.elapsed_ms += tick_ms;
        // `Tick_Pulses` (`0x004BBC80`): at most one gate a tick, and the
        // remainder is thrown away — `stamp = now`, not `stamp += 20`. C179.
        if self.elapsed_ms < GATE_MS {
            return false;
        }
        self.elapsed_ms = 0;
        self.gates += 1;
        if self.gates < PULSE80_GATES {
            return false;
        }
        // The fast pulse is the primary and the slow one is every second fast
        // one, so they cannot drift apart.
        self.gates = 0;
        self.pulses[0] += 1;
        if self.pulses[0].is_multiple_of(2) {
            self.pulses[1] += 1;
        }
        self.pulses != before
    }

    /// Which frame of `overlay` is showing.
    pub fn frame_of(&self, overlay: &Overlay) -> usize {
        let pulses = if overlay.period_ms == PULSE_FAST_MS { self.pulses[0] } else { self.pulses[1] };
        overlay.first + (pulses as usize % overlay.frames.max(1))
    }
}

impl VillageArt {
    /// `vill.pl8` is required; `villtops.pl8` is only drawn with *Advanced
    /// Farming* on and `vill_gd8.pl8` only decides where a drop lands, so an
    /// install missing either still gives a village that draws.
    pub fn load<F>(mut read: F) -> Result<VillageArt, String>
    where
        F: FnMut(&str) -> Result<Vec<u8>, String>,
    {
        let scene = Sheet::new(read("vill.pl8")?).map_err(|e| format!("vill.pl8: {e}"))?;
        let tops = read("villtops.pl8").ok().and_then(|b| Sheet::new(b).ok());
        let grid = read("vill_gd8.pl8")
            .ok()
            .filter(|b| b.len() >= GRID_DATA_OFFSET + GRID_LEN)
            .map(|b| b[GRID_DATA_OFFSET..GRID_DATA_OFFSET + GRID_LEN].to_vec())
            .unwrap_or_default();
        let animation_b = read("villani2.pl8").ok().and_then(|b| Sheet::new(b).ok());
        let animation_a = read("villani1.pl8").ok().and_then(|b| Sheet::new(b).ok());
        Ok(VillageArt { scene, tops, grid, animation_b, animation_a })
    }

    /// Whether the drop grid was found. Without it nothing can be dropped, and
    /// the screen says so.
    pub fn has_grid(&self) -> bool {
        self.grid.len() == GRID_LEN
    }

    /// `FUN_004398F5`: which cluster a point is over, 1-based, or 0.
    pub fn cluster_at(&self, x: i32, y: i32, top: i32) -> usize {
        if !self.has_grid() || x < SCENE_X || x >= SCENE_X + GRID_COLS as i32 * GRID_CELL {
            return 0;
        }
        if y < top || y >= top + GRID_ROWS as i32 * GRID_CELL {
            return 0;
        }
        let col = ((x - SCENE_X) / GRID_CELL) as usize;
        let row = ((y - top) / GRID_CELL) as usize;
        // The original clamps to 8,
        // as the last cluster.
        (self.grid[row * GRID_COLS + col] as usize).min(CLUSTER_COUNT)
    }

    /// The scene itself. Returns false when the frame will not decode, so the
    /// caller can draw its own ground.
    pub fn draw_scene(&self, canvas: &mut Canvas, top: i32) -> bool {
        match self.scene.frame(0) {
            Some(f) => {
                canvas.blit_opaque(&f, SCENE_X, top);
                true
            }
            None => false,
        }
    }

    /// `villtops.pl8` frame `weather`, drawn at y = 64 above an Advanced
    /// Farming scene.
    pub fn draw_tops(&self, canvas: &mut Canvas, weather: usize) -> bool {
        match self.tops.as_ref().and_then(|s| s.frame(weather)) {
            Some(f) => {
                canvas.blit_opaque(&f, SCENE_X, TOPS_Y);
                true
            }
            None => false,
        }
    }

    pub fn tops_frames(&self) -> usize {
        self.tops.as_ref().map_or(0, |s| s.frame_count())
    }

    /// **The quarry, the mine and the lumber camp**, drawn over the scene in
    /// `Village_Draw`'s own order and gated on `has_resource[industry]`.
    ///
    /// Returns how many were painted,
    /// none" from "the artwork is missing" —
    /// county with a mine draws one and the county with a quarry does not.
    ///
    /// This is the missing half of a defect a player reported as *"a county
    /// that clearly has iron has no iron mine in the town centre"*: the other
    /// half was that `has_resource`.
    /// every resource. `docs/decisions.md` C57.
    pub fn draw_resources(&self, canvas: &mut Canvas, has_resource: [bool; 4], top: i32) -> usize {
        let Some(sheet) = self.animation_b.as_ref() else { return 0 };
        let mut drawn = 0;
        for (industry, frame, x, y) in RESOURCE_BUILDINGS {
            if !has_resource[industry] {
                continue;
            }
            if let Some(f) = sheet.frame(frame) {
                canvas.blit(&f, x, top + y);
                drawn += 1;
            }
        }
        drawn
    }

    /// **`Village_Animate`'s six overlays**, in its own order, at the frames
    /// `clock` is currently showing.
    ///
    /// The three unconditional ones are drawn whatever the county holds; the
    /// other three are gated on `has_resource`.
    /// [`VillageArt::draw_resources`]'s are, and go **on top of** the buildings
    /// that function paints — the original calls `Village_Draw` once and
    /// `Village_Animate` every frame after it.
    ///
    /// Returns how many were painted,
    /// a missing sheet.
    pub fn draw_animations(
        &self,
        canvas: &mut Canvas,
        has_resource: [bool; 4],
        top: i32,
        clock: &AnimationClock,
    ) -> usize {
        let mut drawn = 0;
        for overlay in &OVERLAYS {
            if overlay.industry.is_some_and(|i| !has_resource[i]) {
                continue;
            }
            let sheet = if overlay.villani1 { &self.animation_a } else { &self.animation_b };
            let Some(sheet) = sheet.as_ref() else { continue };
            if let Some(f) = sheet.frame(clock.frame_of(overlay)) {
                canvas.blit(&f, overlay.at.0, top + overlay.at.1);
                drawn += 1;
            }
        }
        drawn
    }

    /// Whether `villani1.pl8` loaded. Only the iron mine's overlay needs it.
    pub fn has_villani1(&self) -> bool {
        self.animation_a.is_some()
    }
}


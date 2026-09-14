#![allow(unused_imports)]
use super::*;
use super::movement::*;
use super::*;

impl BattleRunner {
    /// `FUN_00479A71` (`0x00479A71`) — **clear one player's whole selection**,
    /// and the tail of every other selection verb.
    ///
    /// Sweeps figures 1 … 80 in index order and clears `selected` wherever it
    /// equals this player, leaving another player's selection alone.
    pub fn clear_selection(&mut self, owner: u8) {
        for f in self.sim.figures.iter_mut() {
            if f.selected == owner {
                f.selected = 0;
            }
        }
    }

    /// `FUN_00479B58` (`0x00479B58`) — **the rubber-band box**.
    ///
    /// The corners are normalised first (the original swaps them), then every
    /// cell of the closed rectangle is read and its occupant, if any, is marked.
    /// Two things it does that a modern box-select would not:
    ///
    /// * it marks the *occupant of a cell*, not a figure whose sprite overlaps
    /// the box —
    ///   picked, and one drawn outside and standing inside is;
    /// * it only sets `selected` on figures this player **owns**, but it sets
    ///   the drawing bit on everything in the box, friend or enemy. We keep only
    ///   the first half; the second is the highlight the renderer draws and
    ///   nothing in the rules reads it.
    ///
    /// **It does not clear first.** `FUN_0043C247` clears and then boxes, in
/// that order, so the box is always a fresh selection
    /// addition — see [`Self::pick_box`].
    pub fn select_box(&mut self, owner: u8, a: (u8, u8), b: (u8, u8)) {
        let (x0, x1) = (a.0.min(b.0), a.0.max(b.0));
        let (y0, y1) = (a.1.min(b.1), a.1.max(b.1));
        for y in y0..=y1.min(DIM as u8 - 1) {
            for x in x0..=x1.min(DIM as u8 - 1) {
                let Some(o) = self.occupant[y as usize * DIM + x as usize] else { continue };
                let sim = self.fighters[o as usize].sim;
                if self.sim.figures[sim].owner == owner {
                    self.sim.figures[sim].selected = owner;
                }
            }
        }
    }

    /// Put one figure into a player's selection.
    ///
    /// The original has no such entry point on the click path — a box is the
    /// only way a figure is picked — but `FUN_0043C910`, the control-group
    /// recall, restores a *stored selection* by copying ten bytes back over the
    /// live one, which is this, applied to a list.
    pub fn select_figure(&mut self, fighter: usize, owner: u8) {
        let Some(f) = self.fighters.get(fighter) else { return };
        let sim = f.sim;
        if self.sim.figures[sim].is_alive() && self.sim.figures[sim].owner == owner {
            self.sim.figures[sim].selected = owner;
        }
    }

    /// `FUN_00478F0B` (`0x00478F0B`) — **narrow the selection to one unit**.
    ///
    /// Finds the first selected figure's unit and drops everything not in it.
    /// It runs immediately after [`Self::regroup_selection`] on the committing
    /// path, where it is normally a no-op because the regroup has just put every
    /// selected figure in one unit — it earns its place when
    /// `BattleUnit_Alloc` had no slot left and the regroup silently did nothing.
    /// **A selection that cannot be split is truncated instead.**
    pub fn narrow_selection(&mut self, owner: u8) {
        let first = (0..self.fighters.len()).find(|&i| {
            let sim = self.fighters[i].sim;
            self.sim.figures[sim].is_alive() && self.sim.figures[sim].selected == owner
        });
        let Some(first) = first else { return };
        let keep = self.sim.figures[self.fighters[first].sim].unit;
        if keep == 0 {
            return;
        }
        for i in 0..self.fighters.len() {
            let sim = self.fighters[i].sim;
            let f = &mut self.sim.figures[sim];
            if f.selected == owner && f.unit != keep {
                f.selected = 0;
            }
        }
    }

    /// `FUN_0043C247` (`0x0043C247`) — **clear, box, and (on the committing
    /// call) regroup**, which is the whole of what a completed drag does.
    ///
    /// The original passes `param_2 = 1` on the release and `0` on every frame
    /// of the drag, and only the `1` runs `FUN_00478987`. So the selection is
    /// recomputed live while the box is being drawn and the *units are only
    /// rearranged once*, when the button comes up.
    pub fn pick_box(&mut self, owner: u8, a: (u8, u8), b: (u8, u8), commit: bool) {
        self.clear_selection(owner);
        self.select_box(owner, a, b);
        if commit {
            self.regroup_selection(owner);
            self.narrow_selection(owner);
        }
    }

    /// `FUN_0043C4C6` (`0x0043C4C6`) — **one click on one banner in the right
    /// column takes that figure out of the selection**, then regroups.
    ///
    /// Not "select this unit": the panel is a grid of the figures you already
    /// hold, and clicking one drops it.
    pub fn deselect_figure(&mut self, fighter: usize) {
        let Some(f) = self.fighters.get(fighter) else { return };
        let sim = f.sim;
        let owner = self.sim.figures[sim].owner;
        self.sim.figures[sim].selected = 0;
        self.regroup_selection(owner);
    }

    /// `FUN_00478987` (`0x00478987`) — **the regroup, and the reason selection
    /// is simulation state**.
    ///
    /// It asks one question: *is the selection exactly one whole unit?* If it
    /// is, nothing happens beyond resetting that unit's reform timer. If it is
    /// not — the player boxed half a unit, or figures from two — it **allocates
    /// a new unit** and moves every selected figure into it. From then on the
/// selection *is* a unit
    /// orders one.
    ///
/// Two details reproduced:
    ///
    /// * the new unit's category is written **inside** the move loop, so the
    ///   **last** selected figure decides whether the whole new unit is treated
    ///   as missile (1) or melee (3);
    /// * the base unit is the unit of the **lowest-numbered** selected figure,
    /// and the "is it exactly this unit" test is against that one alone.
    ///
    /// Returns the unit the player's selection now is — the original's
    /// `DAT_0053E984` — or 0 when nothing is selected.
    pub fn regroup_selection(&mut self, owner: u8) -> usize {
        if owner == 0 {
            return 0;
        }
        // The first selected figure, in index order, and its unit.
        let first = (0..self.fighters.len()).find(|&i| {
            let sim = self.fighters[i].sim;
            self.sim.figures[sim].is_alive() && self.sim.figures[sim].selected == owner
        });
        let Some(first) = first else { return 0 };
        let base = self.sim.figures[self.fighters[first].sim].unit as usize;

        // "The selection is not exactly unit `base`": some figure of `base` is
        // unselected, or some selected figure is not in `base`.
        let mut split = false;
        for i in 0..self.fighters.len() {
            let sim = self.fighters[i].sim;
            let f = &self.sim.figures[sim];
            if !f.is_alive() {
                continue;
            }
            if f.selected == 0 && f.unit as usize == base {
                split = true;
            }
            if f.selected == owner && f.unit as usize != base {
                split = true;
            }
        }
        if base != 0 {
            self.units.get_mut(base).reform = crate::unit::REFORM_ON_ORDER;
        }
        if !split {
            return base;
        }

        let human = self.units.get(base).human;
        let side = self.units.get(base).side;
        let Some(new) = self.units.create(owner, human, side, 3) else { return base };
        self.units.get_mut(new).reform = crate::unit::REFORM_ON_ORDER;
        let mut first_fig = 0u16;
        let mut last_fig = 0u16;
        let mut category = 3u8;
        for i in 0..self.fighters.len() {
            let sim = self.fighters[i].sim;
            if !self.sim.figures[sim].is_alive() || self.sim.figures[sim].selected != owner {
                continue;
            }
            if first_fig == 0 {
                first_fig = i as u16;
            }
            last_fig = i as u16;
            // The original writes this per figure, so the last one wins.
            category = if WEAPON_CLASS[self.fighters[i].troop.index()] == 0 { 3 } else { 1 };
            self.sim.figures[sim].unit = new as u16;
        }
        {
            let u = self.units.get_mut(new);
            u.first = first_fig;
            u.last = last_fig;
            u.category = category;
        }
        self.rebuild_units();
        new
    }

    /// How many of `owner`'s figures are picked — the original's
    /// `DAT_00553078`, which gates every order.
    pub fn selected_count(&self, owner: u8) -> usize {
        (0..self.fighters.len())
            .filter(|&i| {
                let sim = self.fighters[i].sim;
                self.sim.figures[sim].is_alive() && self.sim.figures[sim].selected == owner
            })
            .count()
    }

    /// The picked figures, low index first — the order the banner panel draws
    /// them in and the order `FUN_0043C2A9` hit-tests them in.
    pub fn selected_fighters(&self, owner: u8) -> Vec<usize> {
        (0..self.fighters.len())
            .filter(|&i| {
                let sim = self.fighters[i].sim;
                self.sim.figures[sim].is_alive() && self.sim.figures[sim].selected == owner
            })
            .collect()
    }

    pub fn is_selected(&self, fighter: usize) -> bool {
        self.fighters
            .get(fighter)
            .is_some_and(|f| self.sim.figures[f.sim].selected != 0)
    }

    /// Which player, if any, has this figure — for the renderer.
    pub fn selected_by(&self, fighter: usize) -> u8 {
        self.fighters.get(fighter).map_or(0, |f| self.sim.figures[f.sim].selected)
    }

    /// `FUN_0043C634` (`0x0043C634`) → `BattleUnit_Order` — **the click that
    /// gives an order**.
    ///
    /// The original orders exactly one unit, `DAT_0053E984`, because
    /// [`Self::regroup_selection`] has already made the selection be one unit.
    /// It is re-run here for the same reason: an order issued after figures have
    /// died has to be issued to whatever the selection is *now*.
    ///
    /// `woodland` is the original's fifth argument, `DAT_0053E874`. See
    /// [`Self::order_full`] — it is not "from a player" whatever
    /// `docs/symbols.json` calls it.
    pub fn order_selected(
        &mut self,
        owner: u8,
        x: u8,
        y: u8,
        target: Option<usize>,
        woodland: bool,
    ) -> bool {
        let unit = self.regroup_selection(owner);
        if unit == 0 {
            return false;
        }
        self.order_full(unit, x, y, target, woodland, Formation::Keep);
        true
    }

    /// `FUN_0043C77A` (`0x0043C77A`) — **the `H` and `V` keys**.
    ///
    /// Re-issues the player's current unit an order *at its own position* with a
    /// formation orientation, so the unit turns its rectangle without moving.
    /// The original reads `DAT_0053E984` directly and does **not** regroup
    /// first; it also plays the acknowledgement cry unconditionally, even when
    ///
    pub fn order_formation(&mut self, unit: usize, formation: Formation) {
        if unit == 0 || unit > MAX_UNITS || !self.units.get(unit).is_live() {
            return;
        }
        let (x, y) = {
            let u = self.units.get(unit);
            (u.x.clamp(0, DIM as i16 - 1) as u8, u.y.clamp(0, DIM as i16 - 1) as u8)
        };
        self.order_full(unit, x, y, None, false, formation);
    }

    /// `BattleUnit_Order` (`0x00479E90`) with **all six of its arguments**, as
    /// against [`Self::order_unit`], which is the AI's three.
    ///
    /// Three arms of the original that only a player's click can reach:
    ///
    /// * **`attackTarget`** — the enemy figure under the cursor. It is stored in
    ///   unit `+0x2C` and read by the order handlers; when the unit mixes
///   missile and melee figures the original *splits it*
    ///   it, which [`Self::regroup_selection`] already models on the selection
    ///   side.
    /// * **`woodland`** — `DAT_0053E874`, and the name in `docs/symbols.json`
    ///   is wrong. Its only writer is `Battle_UpdateHover`, which sets it when
    ///   the hovered cell's **surface byte is 15** and clears it otherwise, and
    ///   every AI call site passes a literal 0. Its effect is that a missile
    ///   unit of **side 0** ordered onto woodland has `Order_StopShortOfTarget`
///   applied. Reported as a correction.
    /// * **`facing`** — [`Formation`].
    pub fn order_full(
        &mut self,
        unit: usize,
        x: u8,
        y: u8,
        target: Option<usize>,
        woodland: bool,
        formation: Formation,
    ) {
        if unit == 0 || unit > MAX_UNITS || !self.units.get(unit).is_live() {
            return;
        }
        {
            let u = self.units.get_mut(unit);
            u.target_x = x as i16;
            u.target_y = y as i16;
            u.withdrawing = false;
            if u.in_melee {
                u.order_lock = crate::unit::ORDER_LOCK;
            }
            u.reform = crate::unit::REFORM_ON_ORDER;
        }
        // `unit.orderedTarget`, `+0x2C`: the enemy figure the player pointed at.
// Carried on the figures here, because that is
        // where this crate's handlers already look for a chased man.
        if let Some(t) = target {
            let members = self.members(unit);
            for m in members {
                let sim = self.fighters[m].sim;
                self.sim.figures[sim].target = Some(self.fighters[t].sim);
            }
        }
        if let Some(o) = formation.orientation() {
            self.units.get_mut(unit).orientation = o;
            // Non-zero facing puts the destination back where the unit is.
            let (hx, hy) = {
                let u = self.units.get(unit);
                (u.x, u.y)
            };
            let u = self.units.get_mut(unit);
            u.target_x = hx;
            u.target_y = hy;
        }
        // **`targetCell`, `+0x30` — the player's fire arrow.**
        // `BattleUnit_Order` (`0x00479E90`) clears it whenever `woodland` is
        // clear, and writes it only when the hovered cell is woodland, the unit
        // has a live missile figure, it is of side 0, no enemy is under the
        // cursor, and `BattleUnit_Classify` leaves `+0x08 < 5`. `[V]`.
        // `Order_StopShortOfTarget` and `Dest_FindReachableNear`, which the
        // same arm applies to the destination, are not built.
        let missiles_here = self
            .members(unit)
            .into_iter()
            .any(|m| WEAPON_CLASS[self.fighters[m].troop.index()] != 0);
        let u = self.units.get_mut(unit);
        u.target_cell = 0;
        if woodland && missiles_here && target.is_none() && u.side == SIDE_A && u.category < 5 {
            u.target_cell = fire::cell_byte_offset(x as i32, y as i32);
        }
        let (tx, ty) = (self.units.get(unit).target_x, self.units.get(unit).target_y);
        self.pour_on_order(unit, tx, ty);
        self.reform_unit(unit);
    }

    /// `FUN_0047A76D` (`0x0047A76D`) — **the charge button**, battle button 3.
    ///
    /// Every one of this player's figures whose troop type is under 7 — so no
    /// siege engine and no oil — goes into free pursuit and its unit is halted,
    /// which switches the every-500-frame reform off. It is **not**
    /// `Order_ChargeNearest` (`0x0048C8AF`), which is the AI's and also clears
    /// the withdraw flag and the figures' targets; this one does neither.
    ///
    /// The button that calls it is guarded by a once-per-battle latch
    /// (`DAT_0055322C`),
    pub fn charge_all(&mut self, owner: u8) {
        for i in 0..self.fighters.len() {
            let sim = self.fighters[i].sim;
            // `troopType < 7` in the original, which is exactly the four types
            // [`Troop::is_siege`] names: catapults, towers, rams and oil.
            if self.sim.figures[sim].owner != owner || self.fighters[i].troop.is_siege() {
                continue;
            }
            let unit = self.sim.figures[sim].unit as usize;
            if unit != 0 && unit <= MAX_UNITS {
                self.units.get_mut(unit).halted = true;
            }
            self.sim.figures[sim].state = State::Chasing;
        }
    }

    /// The figure standing on a cell, if any — what the hover and the box read.
    pub fn occupant_of(&self, x: u8, y: u8) -> Option<usize> {
        if x as usize >= DIM || y as usize >= DIM {
            return None;
        }
        self.occupant[y as usize * DIM + x as usize].map(|o| o as usize)
    }

    /// `BattleUnits_RebuildFromFigures` (`0x00488DFE`) — exposed because
    /// [`Self::regroup_selection`] has to run it after moving figures between
    /// units, and the tick loop's own call is a tick away.
    fn rebuild_units(&mut self) {
        self.units.rebuild_from_figures(&mut self.sim.figures);
    }
}



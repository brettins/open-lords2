use super::*;

impl BattleRunner {
    /// army A is raised with side 4, army B with side 0, and side 0 is the one
    /// that deploys at the `0x04` marker. **[V]**
    pub fn deploy(field: Battlefield, army_a: &[(Troop, u16)], army_b: &[(Troop, u16)]) -> Self {
        BattleRunner::deploy_armies(
            field,
            DEFAULT_SEED,
            Army { troops: army_a, owner: 1, human: false },
            Army { troops: army_b, owner: 2, human: true },
        )
    }

    pub fn deploy_muster(field: Battlefield, seed: u64, army_a: Muster, army_b: Muster) -> Self {
        BattleRunner::deploy_muster_on(field, seed, army_a, army_b, None, None)
    }

    pub fn deploy_siege(
        field: Battlefield,
        seed: u64,
        army_a: Muster,
        army_b: Muster,
        castle_level: u8,
    ) -> Self {
        BattleRunner::deploy_muster_on(field, seed, army_a, army_b, Some(castle_level), None)
    }

    pub fn deploy_siege_on_sheet(
        field: Battlefield,
        tables: &crate::CastleTables,
        seed: u64,
        army_a: Muster,
        army_b: Muster,
        castle_level: u8,
    ) -> Self {
        BattleRunner::deploy_muster_on(
            field,
            seed,
            army_a,
            army_b,
            Some(castle_level),
            Some(tables),
        )
    }

    fn deploy_muster_on(
        field: Battlefield,
        seed: u64,
        army_a: Muster,
        army_b: Muster,
        castle_level: Option<u8>,
        tables: Option<&crate::CastleTables>,
    ) -> Self {
        let session = Session::default();
        Self::deploy_prepared(field, seed, army_a, army_b, castle_level, tables, session, |_| {})
    }

    /// Any of the three fields, opened from the last battle's [`Session`].
    #[allow(clippy::too_many_arguments)]
    pub fn deploy_in_session(
        field: Battlefield,
        tables: Option<&crate::CastleTables>,
        seed: u64,
        army_a: Muster,
        army_b: Muster,
        castle_level: Option<u8>,
        session: Session,
    ) -> Self {
        Self::deploy_prepared(field, seed, army_a, army_b, castle_level, tables, session, |_| {})
    }

    /// What `Battle_Start` (`0x004778A0`) left in `g_battleApproachLane` and
    /// `g_battleRallyGroup`.
    pub fn session(&self) -> Session {
        Session {
            multiplayer: self.ai.multiplayer,
            lane: self.ai.approach_lane,
            rally: self.ai.rally_group,
            markers: (
                self.ai_field.enemy_end.map(|s| s.map(narrow)),
                self.ai_field.rally.map(|s| s.map(|g| g.map(narrow))),
            ),
        }
    }

    /// `prep` runs on the built field before either side is raised.
    pub(crate) fn deploy_prepared(
        field: Battlefield,
        seed: u64,
        army_a: Muster,
        army_b: Muster,
        castle_level: Option<u8>,
        tables: Option<&crate::CastleTables>,
        session: Session,
        prep: impl FnOnce(&mut BattleRunner),
    ) -> Self {
        let sized = battle_size_class(&army_a, &army_b);
        let class = MEN_PER_FIGURE_TABLE[sized];
        let mpf_a = side_scale(army_a.men(), class);
        let mpf_b = side_scale(army_b.men(), class);
        let mut field = field;
        // Battlefield_BuildRandom 0x0047AAA3 00470000.c:4672-4727 and BuildFromSkr :4944-4982 write them.
        field.keep_unwritten(session.markers);
        let mut runner = BattleRunner::empty(field, seed);
        let prev = (session.lane, session.rally);
        runner.ai = Ai::new_for_battle(seed, session.multiplayer, prev);
        runner.men_per_figure = [mpf_b as u16, mpf_a as u16];
        runner.size_class = sized as u8;
        if let Some(level) = castle_level {
            runner.siege = crate::siege::SiegeState::castle(level);
            runner.ai.is_siege = true;
            runner.ai_field = match tables {
                Some(t) => crate::castle::ai_field(&runner.field, level, t, runner.ai.approach_lane),
                None => crate::siege::our_castle_ai_field(&runner.field, level),
            };
            runner.ai.approach_score = crate::siege::approach_score_at_build(&runner.field);
            runner.ai.ramparts_breached = runner.siege.ramparts_breached as i32;
            // `Battlefield_BuildCastle` (`0x0047C4BA`):5265-5272, on
            // `g_units[g_battleArmyA]` — army A, the aggressor, is `army_a`.
            // One `Rand_Advance` (`0x00404A46`) on the A stream, taken here
            // because `Battle_Start` (`0x004778A0`) builds the castle before
            // `Battle_UpdateStrengthAdvantage`, [`Ai::rng`]'s other consumer.
            // `Ai::rng` is a Pcg32 stand-in for that stream, so the count of
            // advances is **[V]** and the value **[D]** — `docs/netcode.md`
            // D-3a. **[D]**: neither the draw nor the flag happens on the
            // stand-in castle, where `our_castle_ai_field` invents
            // `castle_ref` — the cell the flag's readers aim at — so a 1 there
            // would send the storming party at a fabricated point.
            if tables.is_some() {
                let basket = |t: Troop| {
                    army_a.troops.iter().filter(|(k, _)| *k == t).map(|(_, n)| *n).sum::<u32>()
                };
                let draw = runner.ai.rng.next_u32();
                runner.ai_field.orientation = crate::castle::orientation(
                    draw,
                    basket(Troop::BatteringRams),
                    basket(Troop::SiegeTowers),
                    basket(Troop::Catapults),
                );
            }
        }
        prep(&mut runner);
        runner.raise_men(&army_a, mpf_a, SIDE_B, false);
        runner.raise_men(&army_b, mpf_b, SIDE_A, castle_level.is_some());
        runner.settle();
        runner
    }

    pub fn deploy_armies(field: Battlefield, seed: u64, army_a: Army, army_b: Army) -> Self {
        assert_ne!(army_a.owner, 0, "owner 0 is the original's free-slot marker");
        assert_ne!(army_b.owner, 0, "owner 0 is the original's free-slot marker");
        let mut runner = BattleRunner::empty(field, seed);
        runner.raise(army_a, SIDE_B);
        runner.raise(army_b, SIDE_A);
        runner.settle();
        runner
    }

    pub(super) fn empty(field: Battlefield, seed: u64) -> Self {
        let blocked: Vec<bool> = field.cells.iter().map(|c| c.impassable()).collect();
        let ai_field = ai_field_for(&field);
        BattleRunner {
            sim: Battle::new(),
            field,
            fighters: Vec::new(),
            units: Units::new(),
            ai: Ai::new(seed),
            ai_field,
            positions: Vec::new(),
            occupant: vec![None; DIM * DIM],
            missiles: crate::missile::Missiles::new(),
            blocked,
            withdrawn: None,
            men_per_figure: [MEN_PER_FIGURE, MEN_PER_FIGURE],
            siege: crate::siege::SiegeState::field(),
            wall_missile_latch: 0,
            size_class: 0,
            wood_fire: false,
            humans_in_woods: 0,
            tick: 0,
        }
    }

    pub(super) fn settle(&mut self) {
        self.sync_positions();
        self.units.rebuild_from_figures(&mut self.sim.figures);
        self.ai.count_men(&self.sim.figures);
        for u in 1..=MAX_UNITS {
            if self.units.get(u).is_live() {
                let BattleRunner { units, sim, positions, .. } = self;
                units.recentre(u, &sim.figures, positions);
            }
        }
    }

    /// `BattleUnit_Order` (`0x00479E90`) is the entry point for a click. Only
    /// the part this crate can honour is here — the destination and the
    /// re-issue to the figures. The original also runs `Dest_FindReachableNear`
    /// on the cell and `Order_StopShortOfTarget` for a missile unit, and sets a
    /// 64-frame order lock when the unit is already in melee; the lock is
    /// reproduced because `BattleUnit_JoinMelee` reads it.
    pub fn order_unit(&mut self, unit: usize, x: u8, y: u8) {
        if unit == 0 || unit > MAX_UNITS || !self.units.get(unit).is_live() {
            return;
        }
        {
            let u = self.units.get_mut(unit);
            u.target_x = x as i16;
            u.target_y = y as i16;
            u.withdrawing = false;
            // > It was set by nothing here, which left one figure state
            // > permanently out of a player's reach. `Formation_SendFigure`
            // > refuses to re-issue a destination to a figure **in state 9**
            // > *unless the gate is set* — that is what stops a reform pulling
            // > men out of the ditch every 500 frames — so with the gate never
            // > set, a man ordered to fill a moat in could never be ordered to
            // > do anything else for the rest of the battle. `[V]` — the
            // > original sets it unconditionally at `0x004A...`'s order tail and
            // > clears it again only for a *non-human* unit under one arm.
            u.reform_gate = true;
            if u.in_melee {
                u.order_lock = crate::unit::ORDER_LOCK;
            }
        }
        self.pour_on_order(unit, x as i16, y as i16);
        self.clear_straff(unit);
        self.reform_unit(unit);
    }

    pub fn order_side(&mut self, side: Side, x: u8, y: u8) {
        for u in 1..=MAX_UNITS {
            if self.units.get(u).is_live() && self.units.get(u).side == side {
                self.order_unit(u, x, y);
            }
        }
    }

    pub fn unit_of(&self, fighter: usize) -> usize {
        self.sim.figures[self.fighters[fighter].sim].unit as usize
    }

    pub fn home(&self, side: Side) -> (u8, u8) {
        if side == SIDE_A {
            self.field.home_side0
        } else {
            self.field.home_side4
        }
    }

    /// `Deploy_SlotForUnit` (`0x0048169E`): the `n`-th unit of a side takes the
    /// `n`-th of that side's twelve marker slots.
    ///
    /// The two tables are adjacent in the original's `.data` — side 0's twelve
    /// at `0x00553150` and side 4's twelve at `0x005531B0` — and the routine
    /// **does not bound-check the ordinal**.
    pub(super) fn deploy_slot(&self, ordinal: usize, side: Side) -> (u8, u8) {
        let base = if side == SIDE_A { 0 } else { 12 };
        let idx = (base + ordinal).min(23);
        if idx < 12 {
            self.field.deploy_side0[idx]
        } else {
            self.field.deploy_side4[idx - 12]
        }
    }

    /// `FUN_0046E70E` — the placement search `BattleMan_Create` uses.
    pub(super) fn free_cell_near(&self, x: i32, y: i32) -> Option<(u8, u8)> {
        for radius in 0..20i32 {
            let x0 = (x - radius).max(0);
            let y0 = (y - radius).max(0);
            let x1 = (x + radius + 1).min(DIM as i32);
            let y1 = (y + radius + 1).min(DIM as i32);
            for cy in y0..y1 {
                for cx in x0..x1 {
                    let i = cy as usize * DIM + cx as usize;
                    if self.occupant[i].is_none() && !self.blocked[i] {
                        return Some((cx as u8, cy as u8));
                    }
                }
            }
        }
        None
    }

    fn sync_positions(&mut self) {
        self.positions.clear();
        self.positions
            .extend(self.fighters.iter().map(|f| (f.x, f.y)));
    }

    /// `DAT_0053F028` / `DAT_00553C58` — **a side's living men, and the number
    /// the player is looking at.**
    ///
    /// `Battle_CountMenByType` recomputes both every frame, and
    /// `00420000.c:1072` draws them side by side on the battle HUD with
    /// `Ui_DrawNumberRight`. So the two counters that decide the battle are the
    /// two numbers on the screen, which is as good a confirmation as this layer
    /// offers. **[V]**
    pub fn men_of_side(&self, side: Side) -> u32 {
        self.sim
            .figures
            .iter()
            .filter(|f| f.side == side && f.is_alive() && f.troop.index() < 7)
            .map(|f| f.men as u32)
            .sum()
    }

    /// **Is the battle over, and who holds the field** — `FUN_00477DFC`
    /// (`0x00477DFC`), the per-frame outcome test.
    ///
    /// ```c
    /// if (DAT_0056d5c8 == 0) {
    ///     if      (menA < 1) { g_battleLoser = g_battleArmyB; }  /* B holds the field */
    ///     else if (menB < 1) { g_battleLoser = g_battleArmyA; }
    ///     else if (siege)    { ...three more arms... }
    /// } else {                                    /* a side withdrew */
    ///     g_battleLoser = (A.owner == withdrawer) ? B : A;
    /// }
    /// ```
    ///
    /// — remembering that `g_battleLoser` holds the **winner**, which this is a
    /// fourth site to confirm: `FUN_00478419` maps
    /// `g_localPlayer == g_units[g_battleLoser].owner` onto `L2.eng` group 82's
    /// *"won"* pair.
    pub fn conclusion(&self) -> Option<Conclusion> {
        if let Some(side) = self.withdrawn {
            return Some(Conclusion { winner: other_side(side), cause: End::Withdrawal });
        }
        if self.men_of_side(SIDE_B) < 1 {
            return Some(Conclusion { winner: SIDE_A, cause: End::Annihilation });
        }
        if self.men_of_side(SIDE_A) < 1 {
            return Some(Conclusion { winner: SIDE_B, cause: End::Annihilation });
        }
        if self.siege.is_siege {
            // `00470000.c:3344-3369`: the failed-assault arm runs after the
            // break-in arm in the same frame, and its `g_battleLoser` stands.
            if self.ai.breach_score == 0
                && self.ai.siege_engine_count == 0
                && self.siege.failed_assault_ends()
            {
                return Some(Conclusion { winner: SIDE_A, cause: End::AssaultFailed });
            }
            // `:3344-3347`: `g_castleEntered` credits `g_battleArmyA`, side 4.
            if self.siege.broke_in {
                return Some(Conclusion { winner: SIDE_B, cause: End::BrokeIn });
            }
        }
        None
    }

    fn assault_repulsed(&mut self) {
        if !self.siege.is_siege
            || self.siege.failed_assault_ends()
            || self.ai.breach_score != 0
            || self.ai.siege_engine_count != 0
        {
            return;
        }
        self.ai.breach_score = ASSAULT_REPEAT_SCORE;
        self.ai.approach_score = ASSAULT_REPEAT_SCORE;
    }

    /// A side leaves the field — `DAT_0056D5C8` and `DAT_005656F8`.
    pub fn withdraw(&mut self, side: Side) {
        self.withdrawn = Some(side);
    }

    pub fn withdrawal(&self) -> Option<Side> {
        self.withdrawn
    }

    /// `g_battleWithdrawal = 0` — `Battle_Resume` (`0x00477C89`),
    /// `00470000.c:3220`. Neither `0x0056D5C8` nor `0x005656F8` is in
    /// `g_saveBlocks` (`0x004DE960`), so a load cannot carry a withdrawal.
    pub fn clear_withdrawal(&mut self) {
        self.withdrawn = None;
    }

    pub fn men_per_figure(&self, side: Side) -> u16 {
        self.men_per_figure[usize::from(side != SIDE_A)]
    }

    /// `g_battleSizeClass`, read by the selection marker `FUN_004BE070`.
    pub fn size_class(&self) -> u8 {
        self.size_class
    }

    /// **The casualty readback.** Living men of `side`, by
    /// [`Troop::index`] — the eleven counts a `g_units` record carries at
    /// `+0x16C`.
    pub fn survivors(&self, side: Side) -> [u32; 11] {
        let mut out = [0u32; 11];
        for f in &self.sim.figures {
            if f.side == side && f.is_alive() && !f.mercenary {
                out[f.troop.index()] += f.men as u32;
            }
        }
        out
    }

    /// The band's living men — `Battle_WriteBackCasualties` (`0x0047F474`):
    /// `if (mercenary) mercMen += men` in place of the troop slot.
    pub fn band_survivors(&self, side: Side) -> u32 {
        let alive = |f: &&crate::Figure| f.side == side && f.is_alive() && f.mercenary;
        self.sim.figures.iter().filter(alive).map(|f| f.men as u32).sum()
    }

    pub fn men(&self, side: Side) -> u32 {
        self.sim.men(side)
    }

    pub fn is_alive(&self, i: usize) -> bool {
        self.sim.figures[self.fighters[i].sim].is_alive()
    }

    /// **The body has been cleared away** — the corpse state has counted
    /// `+0x173` out and the original frees the record here. Ours keeps the
    /// index, so nothing may draw it from this frame on.
    ///
    /// This is what stops a spent pot of oil holding its pouring picture for
    /// the rest of the battle: `FUN_0047A814` puts it in state 2, and state 2
    /// runs out. `docs/battle.md` §17.8.
    pub fn corpse_gone(&self, i: usize) -> bool {
        let f = &self.fighters[i];
        f.anim == Motion::Dying && f.corpse >= f.corpse_frames()
    }

    pub fn living(&self, side: Side) -> usize {
        self.sim.living(side)
    }

    pub fn is_decided(&self) -> bool {
        self.sim.is_decided()
    }

    pub fn step(&mut self) {
        self.sync_positions();
        self.units.rebuild_from_figures(&mut self.sim.figures);
        if self.siege.is_siege {
            self.recount_siege();
            self.assault_repulsed();
        }

        let mut before = [(0i16, 0i16); MAX_UNITS + 1];
        for (u, slot) in before.iter_mut().enumerate().skip(1) {
            let unit = self.units.get(u);
            *slot = (unit.target_x, unit.target_y);
        }

        let reform = {
            let BattleRunner { units, sim, positions, ai_field, ai, .. } = self;
            ai::update_all_units(units, &mut sim.figures, positions, ai_field, ai)
        };
        if let Some(side) = self.ai.withdrawal {
            self.withdrawn = Some(side);
        }

        let ordered: Vec<usize> = before
            .iter()
            .enumerate()
            .skip(1)
            .filter(|&(u, &was)| {
                let unit = self.units.get(u);
                unit.is_live() && (unit.target_x, unit.target_y) != was
            })
            .map(|(u, _)| u)
            .collect();
        for u in ordered {
            let (tx, ty) = (self.units.get(u).target_x, self.units.get(u).target_y);
            self.pour_on_order(u, tx, ty);
            // The AI's action routines reach the figures through
            // `BattleUnit_Order`, which clears `movStraff` before it reforms.
            self.clear_straff(u);
            self.reform_unit(u);
        }
        for u in reform {
            self.reform_unit(u);
        }

        // `Battle_UpdateAllMen` (`0x004822ED`): `targeted` `:893-894` and
        // `firing` `:896-897`, each counted down to zero.
        for f in self.sim.figures.iter_mut() {
            f.targeted = f.targeted.saturating_sub(1);
            f.firing = f.firing.saturating_sub(1);
        }

        self.humans_in_woods = 0;
        for i in 0..self.fighters.len() {
            self.update_man(i);
            self.step_one(i);
        }
        self.update_missiles();
        // **`FUN_004859E5`, the wood fire.** `Battle_Frame` runs it after the
        // unit sweep; this runner's unit sweep is at the top of the frame, so
        // it runs here, after the missiles, which is the nearest place with
        // the original's neighbours on both sides.
        if self.wood_fire {
            let touched = fire::spread_woodland(&mut self.field, &mut self.missiles, &mut self.wood_fire);
            for c in touched {
                self.sync_cell(c);
            }
        }
        self.sim.step();
        for i in 0..self.fighters.len() {
            if !self.is_alive(i) && self.fighters[i].anim != Motion::Dying {
                let f = &self.fighters[i];
                let cell = f.y as usize * DIM + f.x as usize;
                if self.occupant[cell] == Some(i as u16) {
                    self.occupant[cell] = None;
                }
                self.fighters[i].anim = Motion::Dying;
                self.fighters[i].phase = 0;
                self.fighters[i].corpse = 0;
                self.fighters[i].path.clear();
            } else if self.fighters[i].anim == Motion::Dying {
                // The corpse state's own count, `+0x173`. It stops at the
                // bound instead of freeing the slot — see [`Fighter::corpse`].
                let limit = self.fighters[i].corpse_frames();
                if self.fighters[i].corpse < limit {
                    self.fighters[i].corpse += 1;
                }
            }
        }
        self.tick += 1;
    }

    pub fn run(&mut self, ticks: u32) {
        for _ in 0..ticks {
            self.step();
        }
    }

    /// * `g_attackersOnWall` (`0x00553E64`) — live side-4 figures standing on
    ///   surface 5. Every defender handler tests it, at 1, 2, 3, 4 and 6.
    ///
    /// * `g_siegeEngineCount` (`0x00553FF0`) — live figures of troop type 7, 8
    ///   or 9. Three attacker handlers will not move onto the castle objective
    ///   while it is zero, and `Battle_CheckOutcome` ends the battle when it
    /// and the breach score are both zero.
    fn recount_siege(&mut self) {
        let mut on_wall = 0;
        let mut engines = 0;
        // `Battle_CountMenByType` (`0x00481B9A`), 00480000.c:739 and :790 —
        // `if (troopType == 9) bVar1 = true;` … `if (!bVar1) DAT_004eeb6c = 0;`.
        // The orientation outlives the coin only while a ram does.
        let mut rams = 0;
        for f in self.fighters.iter() {
            if !self.sim.figures[f.sim].is_alive() {
                continue;
            }
            if f.troop.index() >= 7 && f.troop.index() <= 9 {
                engines += 1;
            }
            if f.troop == Troop::BatteringRams {
                rams += 1;
            }
            if f.side == SIDE_B
                && self.field.cells[f.y as usize * DIM + f.x as usize].surface
                    == crate::siege::SURFACE_BAILEY
            {
                on_wall += 1;
            }
        }
        self.ai.attackers_on_wall = on_wall;
        self.ai.siege_engine_count = engines;
        self.ai.ramparts_breached = self.siege.ramparts_breached as i32;
        if rams == 0 {
            self.ai_field.orientation = 0;
        }
    }


    pub(super) fn members(&self, unit: usize) -> Vec<usize> {
        (0..self.fighters.len())
            .filter(|&i| {
                let f = &self.sim.figures[self.fighters[i].sim];
                f.is_alive() && f.unit as usize == unit
            })
            .collect()
    }

    /// `Formation_PickGeometry` (`0x004819CC`): the member with the highest
    /// [`TYPE_PRIORITY`] dictates footprint and figures per row.
    fn pick_geometry(&self, members: &[usize]) -> (i32, i32) {
        let mut best = (0i32, Troop::Peasants);
        for &m in members {
            let t = self.fighters[m].troop;
            if TYPE_PRIORITY[t.index()] > best.0 {
                best = (TYPE_PRIORITY[t.index()], t);
            }
        }
        (FOOTPRINT[best.1.index()], ROW_MAX[best.1.index()])
    }

    /// **a unit whose `+0x09` is 1 forms two columns whatever its troop type
    /// would give**. `Formation_ComputeRect` (`0x0048A1C9`) is one line —
    /// `if (unit.field_0x9 == 1) g_formationCols = 2;` — and it is the only
    /// effect the `V` key has. **[V]**
    pub(super) fn unit_geometry(&self, unit: usize, members: &[usize]) -> (i32, i32) {
        let (footprint, cols) = self.pick_geometry(members);
        if self.units.get(unit).orientation == 1 {
            return (footprint, 2);
        }
        (footprint, cols)
    }

}

fn narrow((x, y): (i16, i16)) -> (u8, u8) {
    (x as u8, y as u8)
}

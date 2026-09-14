use super::*;

impl BattleRunner {
    /// Deploy two armies onto a battlefield: army A is the AI, army B is the
    /// player.
    ///
    /// `army_a` is side 4 and `army_b` is side 0, matching `Battle_InitArmies`:
    /// army A is raised with side 4, army B with side 0, and side 0 is the one
    /// that deploys at the `0x04` marker. **[V]**
    ///
    /// Army B is marked human-controlled, and that is not cosmetic. It decides
    /// two things the whole AI turns on: which units get an order handler at
    /// all, and the denominator of the strength advantage — see [`Army::human`].
    pub fn deploy(field: Battlefield, army_a: &[(Troop, u16)], army_b: &[(Troop, u16)]) -> Self {
        BattleRunner::deploy_armies(
            field,
            DEFAULT_SEED,
            Army { troops: army_a, owner: 1, human: false },
            Army { troops: army_b, owner: 2, human: true },
        )
    }

    /// **The campaign's entry point.** Deploy two armies given as real men,
    /// choosing the men-per-figure scale the way `Battle_InitArmies` does.
    ///
    /// `docs/battle.md` §5.1: the size class comes from the two totals
    /// *together*, then each side may halve it again if it would otherwise draw
    /// fewer than nine figures. Army A takes side 4 and army B side 0, and in a
    /// campaign battle **army B is the defender** (§4.3).
    ///
    /// Unlike [`Self::deploy_armies`], every figure carries its real share of
    /// men and the **last figure of each unit carries the remainder**
    /// `BattleUnit_Create`'s own rounding. That is what makes the survivors
    /// readable back out as campaign troop counts: the totals start exactly at
    /// the counts that went in.
    ///
    /// ```
    /// # use l2_sim::runner::{blank_field, BattleRunner, Muster};
    /// # use l2_sim::{Troop, SIDE_A, SIDE_B};
    /// let a = [(Troop::Peasants, 128u32), (Troop::Swordsmen, 25), (Troop::Archers, 25)];
    /// let b = [(Troop::Peasants, 122u32), (Troop::Archers, 60)];
    /// let r = BattleRunner::deploy_muster(
    ///     blank_field(),
    ///     l2_sim::runner::DEFAULT_SEED,
    ///     Muster { troops: &a, owner: 1, human: true },
    ///     Muster { troops: &b, owner: 6, human: false },
    /// );
    /// // 178 + 182 = 360, which is size class 1: eight men a figure.
    /// assert_eq!(r.men_per_figure(SIDE_B), 8);
    /// // And no man is lost or invented in the raising.
    /// assert_eq!(r.men(SIDE_B), 178);
    /// assert_eq!(r.men(SIDE_A), 182);
    /// ```
    pub fn deploy_muster(field: Battlefield, seed: u64, army_a: Muster, army_b: Muster) -> Self {
        BattleRunner::deploy_muster_on(field, seed, army_a, army_b, None, None)
    }

    /// **Deploy a siege** — army A besieging a castle of `castle_level` held by
    /// army B.
    ///
    /// Three things change, and they are the three the original changes:
    /// `g_battleIsSiege`, the castle level `Battle_CheckOutcome` reads, and the
    /// two one-shot category latches that give a garrison's first two missile
    /// units categories **9 and 10** instead of 1.
    /// ladder, the raise order, the deployment slots — is what a field battle
    /// does.
    ///
    /// The battlefield is the caller's. [`crate::siege::our_castle`] builds one
/// and says in its name that the layout is ours.
    pub fn deploy_siege(
        field: Battlefield,
        seed: u64,
        army_a: Muster,
        army_b: Muster,
        castle_level: u8,
    ) -> Self {
        BattleRunner::deploy_muster_on(field, seed, army_a, army_b, Some(castle_level), None)
    }

    /// **Deploy a siege of the castle the layout file describes** — the same
    /// call with `Battlefield_ReadStructureLayer`'s tables instead of the ring
    /// [`crate::siege::our_castle_ai_field`] invents.
    ///
    /// `field` should be [`crate::castle::build`]'s and `tables`
    /// [`crate::castle::tables`]'s, from the same [`crate::CastleSheet`].
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
        let total = army_a.men() + army_b.men();
        let class = MEN_PER_FIGURE_TABLE[size_class(total)];
        let mpf_a = side_scale(army_a.men(), class);
        let mpf_b = side_scale(army_b.men(), class);
        let mut runner = BattleRunner::empty(field, seed);
        runner.men_per_figure = [mpf_b as u16, mpf_a as u16];
        // `g_battleSizeClass`. The original's ladder is fed the total **plus
        // `scale × siegeEngines`**; `size_class` is fed the total alone, as it
// was before this read it, and that difference is noted.
        // closed here.
        runner.size_class = size_class(total) as u8;
        if let Some(level) = castle_level {
            runner.siege = crate::siege::SiegeState::castle(level);
            runner.ai.is_siege = true;
            runner.ai_field = match tables {
                Some(t) => crate::castle::ai_field(&runner.field, level, t),
                None => crate::siege::our_castle_ai_field(&runner.field, level),
            };
            // **A fresh siege opens with the approach score at 500 on a dry
            // castle and 0 on a moated one** —
            // [`crate::siege::APPROACH_SCORE_START`] has the three writes and
            // why the pair decides whether a besieger ever assaults at all.
            runner.ai.approach_score = crate::siege::approach_score_at_build(&runner.field);
            runner.ai.ramparts_breached = runner.siege.ramparts_breached as i32;
        }
        // Side 4 first, then side 0 — the order fixes figure indices, and figure
        // indices are the simulation order.
        runner.raise_men(&army_a, mpf_a, SIDE_B);
        runner.raise_men(&army_b, mpf_b, SIDE_A);
        runner.settle();
        runner
    }

    /// Deploy with full control over owners, control and the AI's seed.
    pub fn deploy_armies(field: Battlefield, seed: u64, army_a: Army, army_b: Army) -> Self {
        assert_ne!(army_a.owner, 0, "owner 0 is the original's free-slot marker");
        assert_ne!(army_b.owner, 0, "owner 0 is the original's free-slot marker");
        let mut runner = BattleRunner::empty(field, seed);
        // Side 4 first, then side 0. The order fixes figure indices, and figure
        // indices are the simulation order.
        runner.raise(army_a, SIDE_B);
        runner.raise(army_b, SIDE_A);
        runner.settle();
        runner
    }

    /// A battlefield with the arrays allocated and nothing on it.
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

    /// `Battle_InitArmies`'s tail: the census and the rebuild, before the first
    /// frame runs.
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

    /// The player's order: send a unit to a cell.
    ///
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
            // **`g_battleUnits[unit].field_0x13 = 1`** — the reform gate, set by
            // `BattleUnit_Order` on every order and cleared by `BattleUnit_Reform`
            // once it has been honoured.
            //
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
        self.reform_unit(unit);
    }

    /// Order every unit of a side at a cell. What a player would do with a
    /// select-all and a click
    /// watchable from the first frame.
    pub fn order_side(&mut self, side: Side, x: u8, y: u8) {
        for u in 1..=MAX_UNITS {
            if self.units.get(u).is_live() && self.units.get(u).side == side {
                self.order_unit(u, x, y);
            }
        }
    }

    /// The unit a fighter belongs to, or 0.
    pub fn unit_of(&self, fighter: usize) -> usize {
        self.sim.figures[self.fighters[fighter].sim].unit as usize
    }

    /// A side's own deployment marker: the `0x04` cell for side 0, `0x0F` for
    /// side 4.
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
    /// twelve units deploys its thirteenth unit on the *enemy's* first
    /// deployment slot, which is reachable: oil holds one figure per unit.
    /// Reproduced. Past the twenty-fourth entry the original reads memory we
    /// have not identified, so there we clamp instead of inventing bytes.
    fn deploy_slot(&self, ordinal: usize, side: Side) -> (u8, u8) {
        let base = if side == SIDE_A { 0 } else { 12 };
        let idx = (base + ordinal).min(23);
        if idx < 12 {
            self.field.deploy_side0[idx]
        } else {
            self.field.deploy_side4[idx - 12]
        }
    }

    /// `Battle_RaiseSide` (`0x0047FEA7`) and `BattleUnit_Create` (`0x00480662`).
    ///
    /// Each troop type is split into units of at most
    /// [`MAX_FIGURES_PER_UNIT`] figures, one unit per deployment slot, and each
    /// unit's figures are laid out in a rectangle around that slot by
    /// [`formation::offset_x`] / [`formation::offset_y`].
    fn raise(&mut self, army: Army, side: Side) {
        let men: Vec<(Troop, u32)> = army
            .troops
            .iter()
            .map(|&(t, figures)| (t, figures as u32 * MEN_PER_FIGURE as u32))
            .collect();
        self.raise_men(
            &Muster { troops: &men, owner: army.owner, human: army.human },
            MEN_PER_FIGURE as u32,
            side,
        );
    }

    /// `Battle_RaiseSide` proper: men in, units and figures out.
    ///
    /// `docs/battle.md` §5.2. The eleven troop types are walked in
    /// [`RAISE_ORDER`]; each is cut into units of at most
    /// `MAX_FIGURES_PER_UNIT[t] * men_per_figure` **men**, and each unit into
    /// `ceil(unitMen / men_per_figure)` figures with the last figure taking the
/// remainder.
    fn raise_men(&mut self, army: &Muster, men_per_figure: u32, side: Side) {
        assert_ne!(army.owner, 0, "owner 0 is the original's free-slot marker");
        let mpf = men_per_figure.max(1);
        let mut counts = [0u32; 11];
        for &(t, n) in army.troops {
            counts[t.index()] += n;
        }
        let mut ordinal = 0usize;
        for &troop in RAISE_ORDER.iter() {
            let count = counts[troop.index()];
            if count == 0 {
                continue;
            }
            // A siege engine is one figure whatever the scale, so its count is
            // multiplied up before the split and divided back out by it.
            let mut left = if troop.index() > 6 { count * mpf } else { count };
            let per_unit = MAX_FIGURES_PER_UNIT[troop.index()] as u32;
            let footprint = FOOTPRINT[troop.index()];
            while left > 0 {
                let unit_men = left.min(per_unit * mpf);
                let figures = unit_men.div_ceil(mpf) as usize;
                // `BattleUnit_Create`'s eleven-way ladder, plus the two
                // one-shot latches: in a **siege**, on side **0**, the first
                // missile unit raised takes category 9 and the second takes
                // category 10. Those two categories exist nowhere else, and
                // their handlers — one of which does nothing but count — are
                // two of the fourteen this makes reachable.
                let mut category = CATEGORY_OF_TROOP[troop.index()];
                if self.siege.is_siege && side == SIDE_A && category == 1 && self.wall_missile_latch < 2
                {
                    self.wall_missile_latch += 1;
                    category = 8 + self.wall_missile_latch;
                }
                let Some(unit) = self.units.create(army.owner, army.human, side, category) else {
                    return;
                };
                let slot = self.deploy_slot(ordinal, side);
                let rows = formation::rows_for(troop, figures);
                for i in 0..figures {
// The last figure takes what is left
                    // complement — `BattleUnit_Create`'s own rounding.
                    let men = if i + 1 == figures {
                        (unit_men - (figures as u32 - 1) * mpf) as u16
                    } else {
                        mpf as u16
                    };
                    let Some(sim) = self.sim.add(troop, side, men) else {
                        // The original truncates at 80 figures and keeps
                        // allocating empty units for the rest of the army; we
                        // stop, because an empty unit is a unit the AI would
                        // then dispatch on.
                        return;
                    };
                    let dx = formation::offset_x(footprint, rows, i as i32);
                    let dy = formation::offset_y(footprint, rows, i as i32, side);
                    let Some((x, y)) =
                        self.free_cell_near(slot.0 as i32 + dx, slot.1 as i32 + dy)
                    else {
                        return;
                    };
                    {
                        let f = &mut self.sim.figures[sim];
                        f.unit = unit as u16;
                        f.owner = army.owner;
                        f.owner_is_human = army.human;
                        // The strength band is measured against the **size
                        // class's** full figure, not against what this figure
                        // was given — the last figure of a unit takes a
                        // remainder and is not thereby a weak one.
                        f.full_men = mpf as u16;
                    }
                    // `BattleMan_Create` sets the facing from the *row*, not
                    // from the side: north of the halfway line a figure faces
                    // south and vice versa. **[D]**
                    let facing = if y < 41 { 4 } else { 0 };
                    self.occupant[y as usize * DIM + x as usize] = Some(self.fighters.len() as u16);
                    self.fighters.push(Fighter {
                        sim,
                        troop,
                        side,
                        x,
                        y,
                        target: (x, y),
                        facing,
                        progress: Progress::default(),
                        anim: Motion::Idle,
                        phase: ((sim as u32 * 9 + x as u32 * 16) & 0x3F) as u8,
                        facing_drawn: facing,
                        fidget: 0,
                        // `BattleMan_Create`: `((index * 9 + x * 16) & 0x3F) +
                        // 0xB4` — 180 … 243 frames. §14.6.
                        fidget_period: (((sim as u32 * 9 + x as u32 * 16) & 0x3F) + 0xB4) as u8,
                        path: Vec::new(),
                        barred: 0,
                        hold: 0,
                        reroutes: 0,
                        moat_cell: None,
                        moat_load: 0,
                        polar: 0,
                        corpse: 0,
                    });
                }
                left -= unit_men;
                ordinal += 1;
            }
        }
    }

    /// `FUN_0046E70E` — the placement search `BattleMan_Create` uses.
    ///
    /// Not a spiral: it scans the whole `(2r+1)²` box clipped to the map, rows
    /// top to bottom and columns left to right, and takes the first cell with
    /// no occupant and no `0x90` flag. Since radius `r-1` has already been
    /// scanned and rejected, that is the topmost-then-leftmost free cell in the
    /// box. Radii 0…19; `None` when the whole neighbourhood is full.
    fn free_cell_near(&self, x: i32, y: i32) -> Option<(u8, u8)> {
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
    ///
    /// **Troop types 7 … 10 are excluded** — the original's loop is
    /// `if (troopType < 7)`, so catapults, towers, rams and oil are worth no
    /// men and a side reduced to siege engines has already lost.
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
    /// `None` while it continues. The whole of the non-siege rule is the first
    /// two arms of that function:
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
    ///
    /// **A field battle ends only when one side is annihilated or withdraws.**
    ///
    /// siege arms — the escape tile, *assault repulsed, repeat*, and *siege
    /// lifted* — are deliberately not here: sieges are out of scope, and
    /// `Battlefield_BuildCastle` has not been implemented,
    /// crate cannot be one.
    pub fn conclusion(&self) -> Option<Conclusion> {
        if let Some(side) = self.withdrawn {
            return Some(Conclusion { winner: other_side(side), cause: End::Withdrawal });
        }
        // The original tests A first,
        // the same frame is won by B. Reproduced.
        if self.men_of_side(SIDE_B) < 1 {
            return Some(Conclusion { winner: SIDE_A, cause: End::Annihilation });
        }
        if self.men_of_side(SIDE_A) < 1 {
            return Some(Conclusion { winner: SIDE_B, cause: End::Annihilation });
        }
        if self.siege.is_siege {
            // **Arm three: the besieger got in.** A side-4 figure reached a
            // `0x08` cell, and that alone wins the siege — the garrison need
            // not be touched.
            if self.siege.broke_in {
                return Some(Conclusion { winner: SIDE_B, cause: End::BrokeIn });
            }
            // **Arms four and five, which are one test with two answers.** No
            // breach and no engines left: a small castle can still be stormed,
            // so the scores are reset and the battle carries on; a big one
            // cannot, and the besieger has lost.
            if self.ai.breach_score == 0 && self.ai.siege_engine_count == 0 {
                if self.siege.castle_level >= ASSAULT_REPEATS_BELOW_LEVEL {
                    return Some(Conclusion { winner: SIDE_A, cause: End::AssaultFailed });
                }
                // *Assault repulsed, repeat* is handled in [`Self::step`],
                // which is where a state change belongs; this test is `&self`.
            }
        }
        None
    }

    /// **Assault repulsed, repeat.** `Battle_CheckOutcome`'s arm four, which is
    /// the only place in the whole outcome test that *changes* something
    /// instead of ending the battle: with no breach and no engines left, a
    /// castle below level 3 has the breach and approach scores **reset to 4**
    /// and the fighting goes on.
    ///
    /// It reads as the besiegers regrouping for another go, and it is why a
/// palisade cannot be defended by destroying the siege engines.
    fn assault_repulsed(&mut self) {
        if !self.siege.is_siege
            || self.siege.castle_level >= ASSAULT_REPEATS_BELOW_LEVEL
            || self.ai.breach_score != 0
            || self.ai.siege_engine_count != 0
        {
            return;
        }
        self.ai.breach_score = ASSAULT_REPEAT_SCORE;
        self.ai.approach_score = ASSAULT_REPEAT_SCORE;
    }

    /// A side leaves the field — `DAT_0056D5C8` and `DAT_005656F8`.
    ///
    /// The original raises this in exactly one place,
    /// `UnitOrder_SiegeAttKnight`: an all-knight AI besieger facing an
/// unbreached wall gives up. It is a lever here because
    /// the only *rule* that pulls it is a siege one, and because a player's
    /// withdrawal has to enter the model somewhere.
    ///
    /// It outranks annihilation: the original tests the flag before it looks at
    /// either men counter.
    pub fn withdraw(&mut self, side: Side) {
        self.withdrawn = Some(side);
    }

    /// Men one figure of `side` stands for — [`size_class`]'s answer for this
    /// battle, or four for a skirmish.
    pub fn men_per_figure(&self, side: Side) -> u16 {
        self.men_per_figure[usize::from(side != SIDE_A)]
    }

    /// **The casualty readback.** Living men of `side`, by
    /// [`Troop::index`] — the eleven counts a `g_units` record carries at
    /// `+0x16C`.
    ///
    /// This is the whole of "the battle hands its result back": the campaign
    /// wrote eleven counts in, the battle killed some of the men standing for
    /// them, and this reads what is left in the same eleven slots. It is exact
/// because [`Self::deploy_muster`] gives every
    /// figure its real share of men.
    pub fn survivors(&self, side: Side) -> [u32; 11] {
        let mut out = [0u32; 11];
        for f in &self.sim.figures {
            if f.side == side && f.is_alive() {
                out[f.troop.index()] += f.men as u32;
            }
        }
        out
    }

    /// Living men of `side` — the sum of [`Self::survivors`].
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

    /// Advance one tick. See the module doc for the order and why it is that
    /// order.
    pub fn step(&mut self) {
        self.sync_positions();
        self.units.rebuild_from_figures(&mut self.sim.figures);
        if self.siege.is_siege {
            self.recount_siege();
            self.assault_repulsed();
        }

        // A destination the handlers are about to overwrite. Comparing before
        // and after is how this driver notices an order: the handlers in
        // `l2-sim::ai` write `target_x/target_y` where the original's action
        // routines write them *and* call `BattleUnit_Order`, which is what
        // re-issues the figures.
        let mut before = [(0i16, 0i16); MAX_UNITS + 1];
        for (u, slot) in before.iter_mut().enumerate().skip(1) {
            let unit = self.units.get(u);
            *slot = (unit.target_x, unit.target_y);
        }

        let reform = {
            let BattleRunner { units, sim, positions, ai_field, ai, .. } = self;
            ai::update_all_units(units, &mut sim.figures, positions, ai_field, ai)
        };
        // `g_battleWithdrawal`, raised inside an order handler by the one thing
        // in the binary that raises it — see [`ai::Ai::withdrawal`]. The
        // original writes a global that `Battle_CheckOutcome` reads directly;
        // it is carried onto the runner so [`Self::conclusion`] has one place
        // to look whether the AI or a caller pulled the lever.
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
            // `BattleUnit_Order`'s oil loop runs before it re-issues anybody.
            let (tx, ty) = (self.units.get(u).target_x, self.units.get(u).target_y);
            self.pour_on_order(u, tx, ty);
            self.reform_unit(u);
        }
        for u in reform {
            self.reform_unit(u);
        }

        // `Battle_UpdateAllMen` ages `targeted` by one per frame, which is what
// makes free pursuit spread out.
        for f in self.sim.figures.iter_mut() {
            f.targeted = f.targeted.saturating_sub(1);
        }

        self.humans_in_woods = 0;
        for i in 0..self.fighters.len() {
            self.update_man(i);
            self.step_one(i);
        }
        // `Missile_UpdateAll`, immediately after `Battle_UpdateAllMen` and
        // before anything else — which is the original's order, and it matters:
        // the arrows loosed this tick take their eight launch steps inside
        // `step_one`, and every arrow already in the air moves against the
        // occupancy this tick's movement produced.
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
        // Damage, casualties and death all happen here, in melee.rs.
        self.sim.step();
        // A figure that died this tick lets go of its cell and starts falling.
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

    /// The two counters `Battle_UpdateAllMen` recounts **every frame**, and
    /// which every siege handler branches on.
    ///
    /// * `g_attackersOnWall` (`0x00553E64`) — live side-4 figures standing on
    ///   surface 5. Every defender handler tests it, at 1, 2, 3, 4 and 6.
    /// * `g_siegeEngineCount` (`0x00553FF0`) — live figures of troop type 7, 8
    ///   or 9. Three attacker handlers will not move onto the castle objective
    ///   while it is zero, and `Battle_CheckOutcome` ends the battle when it
    /// and the breach score are both zero.
    ///
    /// The other two — the approach and breach scores — are *accumulators*
/// and are raised where the wall comes down.
    fn recount_siege(&mut self) {
        let mut on_wall = 0;
        let mut engines = 0;
        for f in self.fighters.iter() {
            if !self.sim.figures[f.sim].is_alive() {
                continue;
            }
            if f.troop.index() >= 7 && f.troop.index() <= 9 {
                engines += 1;
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
        // `_DAT_0055307C`, which lives in [`crate::SiegeState`] because the
        // county carries it between assaults, mirrored where the order
        // handlers can read it. See [`crate::Ai::ramparts_breached`] for why
        // it is not the moat flag.
        self.ai.ramparts_breached = self.siege.ramparts_breached as i32;
    }

    // -- reforming ----------------------------------------------------------

    /// Live fighter indices belonging to a unit, ascending.
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

    /// [`Self::pick_geometry`] with `Formation_ComputeRect`'s override on top:
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

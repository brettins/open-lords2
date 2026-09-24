#![allow(unused_imports)]
use super::*;
use super::queries::*;
use super::stepping::*;
use super::super::*;

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
            engagement_budget: Some(self.ai.engagement_budget as u8),
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
        if let Some(b) = session.engagement_budget {
            runner.ai.engagement_budget = i32::from(b);
        }
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

    pub(crate) fn empty(field: Battlefield, seed: u64) -> Self {
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

    pub(crate) fn settle(&mut self) {
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

}

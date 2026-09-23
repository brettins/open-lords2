use super::*;

impl BattleRunner {
    /// `Battle_RaiseSide` (`0x0047FEA7`) and `BattleUnit_Create` (`0x00480662`).
    pub(super) fn raise(&mut self, army: Army, side: Side) {
        let men: Vec<(Troop, u32)> = army
            .troops
            .iter()
            .map(|&(t, figures)| (t, figures as u32 * MEN_PER_FIGURE as u32))
            .collect();
        self.raise_men(
            &Muster { troops: &men, owner: army.owner, human: army.human, mercenaries: None },
            MEN_PER_FIGURE as u32,
            side,
        );
    }

    pub(super) fn raise_men(&mut self, army: &Muster, men_per_figure: u32, side: Side) {
        assert_ne!(army.owner, 0, "owner 0 is the original's free-slot marker");
        let mpf = men_per_figure.max(1);
        let mut counts = [0u32; 11];
        for &(t, n) in army.troops {
            counts[t.index()] += n;
        }
        let mut ordinal = 0usize;
        for &troop in RAISE_ORDER.iter() {
            let count = counts[troop.index()];
            let left = if troop.index() > 6 { count * mpf } else { count };
            if !self.raise_troop(army, troop, left, false, mpf, side, &mut ordinal) {
                return;
            }
        }
        // `Battle_RaiseSide` (`0x0047FEA7`): after the eleven, `mercMen` of
        // `mercTroop` is split the same way and raised with `mercenary = 1`.
        if let Some((troop, men)) = army.mercenaries {
            self.raise_troop(army, troop, men, true, mpf, side, &mut ordinal);
        }
    }

    /// One `while (local_8 != 0)` loop of `Battle_RaiseSide` (`0x0047FEA7`):
    /// units of at most `maxFigures * g_menPerFigure` men. `false` when full.
    #[allow(clippy::too_many_arguments)]
    fn raise_troop(
        &mut self,
        army: &Muster,
        troop: Troop,
        mut left: u32,
        mercenary: bool,
        mpf: u32,
        side: Side,
        ordinal: &mut usize,
    ) -> bool {
        let per_unit = MAX_FIGURES_PER_UNIT[troop.index()] as u32;
        let footprint = FOOTPRINT[troop.index()];
        while left > 0 {
            let unit_men = left.min(per_unit * mpf);
            let figures = unit_men.div_ceil(mpf) as usize;
            let mut category = CATEGORY_OF_TROOP[troop.index()];
            if self.siege.is_siege && side == SIDE_A && category == 1 && self.wall_missile_latch < 2 {
                self.wall_missile_latch += 1;
                category = 8 + self.wall_missile_latch;
            }
            let Some(unit) = self.units.create(army.owner, army.human, side, category) else {
                return false;
            };
            let slot = self.deploy_slot(*ordinal, side);
            let rows = formation::rows_for(troop, figures);
            for i in 0..figures {
                let men = if i + 1 == figures {
                    (unit_men - (figures as u32 - 1) * mpf) as u16
                } else {
                    mpf as u16
                };
                let Some(sim) = self.sim.add(troop, side, men) else {
                    return false;
                };
                let dx = formation::offset_x(footprint, rows, i as i32);
                let dy = formation::offset_y(footprint, rows, i as i32, side);
                let Some((x, y)) = self.free_cell_near(slot.0 as i32 + dx, slot.1 as i32 + dy)
                else {
                    return false;
                };
                {
                    let f = &mut self.sim.figures[sim];
                    f.unit = unit as u16;
                    f.owner = army.owner;
                    f.owner_is_human = army.human;
                    f.full_men = mpf as u16;
                    // `BattleUnit_Create` (`0x00480662`, `00480000.c:260-264`).
                    f.mercenary = mercenary;
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
                    fidget_period: (((sim as u32 * 9 + x as u32 * 16) & 0x3F) + 0xB4) as u8,
                    // `[D]`: `BattleMan_Create` (`0x0046E4C8`,
                    // `00460000.c:8262-8296`) takes the first slot with
                    // `owner == 0` and writes twelve fields, `+0x11` not
                    // among them, so a recycled slot keeps the old horse
                    // frame. Harmless — the first walk, stand or strike
                    // tick overwrites it before any state-9 read.
                    horse_frame: 0,
                    path: Vec::new(),
                    barred: 0,
                    hold: 0,
                    reroutes: 0,
                    moat_cell: None,
                    moat_load: 0,
                    polar: 0,
                    corpse: 0,
                    delay: 0,
                });
            }
            left -= unit_men;
            *ordinal += 1;
        }
        true
    }
}

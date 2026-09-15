
use crate::troop::{Troop, TroopStats, TroopTable};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Attacking,
    Defending,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Idle,
    Melee,
    Dead,
    Chasing,
    Shooting,
    FillingMoat,
}

/// [`State`] is the original's state byte and decides what the figure will do
/// next; this is the reduction of it that the original's animation handlers key
/// off — one handler per variant, and each writes the frame index to figure
/// `+0x10`:
///
/// | variant | handler | poses |
/// |---|---|---|
/// | `Idle` | `Anim_StandA2` `0x004872AE` | the figure index `& 7`, with 6 → 1 and 7 → 2 (`00480000.c:2873`), plus the fidget |
/// | `Walking` | `Anim_WalkA2` `0x00486D83` | 0 … 5, one every 4 ticks over 24 |
/// | `Attacking` | `Anim_StrikeA2` `0x00486249` | 6 …, from the strike cycle |
/// | `Shooting` | `Anim_DrawBowA2` `0x0048804A` | 10 … 12, archers and crossbowmen |
/// | `Dying` | `Anim_CollapseA2` `0x00487CE4` | `8N+0 … 8N+5` |
/// | `Shovelling` | `Anim_DyingA2` `0x00487908` | `8N+6 …` |
///
/// **Corrected.** This comment had `0x00486249` as idle/walk and `0x00486D83`
/// as attacking; `docs/battle.md` §14.5 has the five agreements that put them
/// the other way round, and `l2_view::figures` drew the two bands swapped
/// because of it — a fighting man marched and a marching man swung.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Motion {
    Idle,
    Walking,
    Attacking,
    /// `BattleMan_FireMissile` ends `if (target != 0) Anim_DrawBow();`
    /// (`00480000.c:1531`), so the draw is held every tick a target is held,
    /// and the pose comes from a curve indexed by `reload_counter`.
    Shooting,
    /// **Dead** — state 2, `BattleMan_StateDead` (`0x004830E9`), whose whole
    /// body is `Anim_Collapse(); if (0x50 < ++field_0x173) Destroy();`. The six
    /// frames of falling over come off the *death timer*, not `animPhase`.
    Dying,
    /// **Filling the moat** — state 9, `BattleMan_StateFillMoat`
    /// (`0x00483FE1`), the **only** caller of `Anim_Dying` → `Anim_DyingA2`
    /// (`0x00487908`) in the binary: a living man bent double over a shovel,
    /// twelve frames at `8N+6`.
    Shovelling,
}

pub type Side = u8;
pub const SIDE_A: Side = 0;
pub const SIDE_B: Side = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Figure {
    pub troop: Troop,
    pub side: Side,
    pub men: u16,
    pub hits: u16,
    pub recovery_counter: i32,
    pub state: State,
    pub role: Role,
    pub opponent: Option<usize>,
    pub exchange: i32,
    /// The heavy blow lands **once per figure for the whole battle**. `+0x18C`
    /// (`0x0055460C`) has exactly three absolute references in the executable:
    ///
    /// `BattleUnit_Create` zeroes it (`0x00480D47`) and `Melee_Tick`
    /// (`0x00494908`) reads and sets it (`0x00494BE7`, `0x00494C46`). Nothing
    /// resets it per exchange. **[V]** — a computed pointer would not appear in
    /// an absolute-reference count.
    pub blow_used: bool,
    pub owner_is_human: bool,
    pub owner: u8,
    pub unit: u16,
    /// Figure record `+0x15`: this figure was hit since the last unit rebuild.
    pub was_hit: bool,
    /// Figure record `+0x16`: who hit it. The rebuild looks up *that figure's
    /// unit* and stores it as the victim unit's remembered attacker. This is
    /// the only way a unit ever acquires a target it did not walk into or find
    /// by proximity —.
    pub hit_by: Option<usize>,
    pub target: Option<usize>,
    /// Figure record `+0x175`, the debug panel's own label **`targeted`**: how
    /// many men are currently running at this one, times two.
    pub targeted: u8,
    pub stats: TroopStats,
    pub hits_per_casualty: u16,
    /// Figure record `+0x180` — **the reload counter**, which
    /// `BattleMan_FireMissile` counts up once a tick.
    pub reload_counter: u16,
    pub full_men: u16,
    /// Figure record `+0x09`, and the debug panel's own label **`selected`**:
    ///
    /// A player index
    /// stores — `FUN_00479B58` writes `selected = param_1` and `FUN_00479A71`
    /// clears only the figures whose `selected` equals the player being cleared,
    /// so two players can hold disjoint selections in the same battle at the
    /// same time.
    ///
    /// **Selection is simulation state, not interface state**
    /// modelling choice: `FUN_00478987` (`0x00478987`) walks the selection and
    /// *allocates a new unit* for it whenever the picked figures are not exactly
    /// one whole unit. A box drawn round half a unit therefore **splits** that
    /// unit in the original, which changes what every later order applies to and
    /// what the AI's own sweeps see. It has to be in this crate, and it has to
    /// be in the lockstep digest.
    pub selected: u8,
}

impl Figure {
    pub fn new(troop: Troop, side: Side, men: u16) -> Self {
        Figure::with_table(&TroopTable::DEFAULT, troop, side, men)
    }

    pub fn with_table(table: &TroopTable, troop: Troop, side: Side, men: u16) -> Self {
        let stats = table.stats(troop);
        Figure {
            troop,
            side,
            men,
            hits: 0,
            recovery_counter: 0,
            state: State::Idle,
            role: Role::Defending,
            opponent: None,
            exchange: stats.exchange as i32,
            blow_used: false,
            owner_is_human: false,
            owner: 0,
            unit: 0,
            was_hit: false,
            hit_by: None,
            target: None,
            targeted: 0,
            stats,
            hits_per_casualty: table.hits_per_casualty(troop),
            reload_counter: 0,
            full_men: men,
            selected: 0,
        }
    }

    /// **[V]** on the three fractions; the table itself is not in this tree, and
    /// deriving it from the scale is what makes an arbitrary
    /// [`Figure::full_men`] work.
    pub fn strength_band(&self) -> u8 {
        let full = self.full_men.max(1) as u32;
        let men = self.men as u32;
        if men * 4 >= full * 3 {
            0
        } else if men * 2 >= full {
            1
        } else if men * 16 >= full * 3 {
            2
        } else {
            3
        }
    }

    pub fn stats(&self) -> TroopStats {
        self.stats
    }

    pub fn is_alive(&self) -> bool {
        self.state != State::Dead && self.men > 0
    }

    pub fn armour(&self) -> u16 {
        match (self.troop, self.owner_is_human) {
            (Troop::Oil, true) => 25,
            _ => self.stats().armour,
        }
    }

    pub fn take_hits_from(&mut self, amount: u16, by: usize) -> u16 {
        self.was_hit = true;
        self.hit_by = Some(by);
        self.take_hits(amount)
    }

    pub fn take_hits(&mut self, amount: u16) -> u16 {
        let threshold = self.hits_per_casualty;
        self.hits = self.hits.saturating_add(amount);
        let mut killed = 0;
        while self.hits >= threshold && self.men > 0 {
            self.hits -= threshold;
            self.men -= 1;
            killed += 1;
        }
        if self.men == 0 {
            self.state = State::Dead;
            self.opponent = None;
        }
        killed
    }
}

//! A battle figure: one drawn man standing for several real soldiers.
//!
//! The original has two levels — *units* are what the player orders, *figures*
//! are what is drawn and what fights. Combat happens entirely between figures.
//! See `docs/battle.md` §1 and §2.

use crate::troop::{Troop, TroopStats, TroopTable};

/// Which figure of a melee pair is currently swinging. The roles swap when the
/// attacker's exchange counter runs out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Attacking,
    Defending,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    /// Standing, looking for something to fight. The original's state 0.
    Idle,
    /// Locked in a melee duel with `opponent`. The original's **state 4**, and
    /// the state [`crate::unit::Units::rebuild_from_figures`] reads to set a
    /// unit's in-melee flag — which thirteen of the seventeen order handlers
    /// refuse to run under.
    Melee,
    /// All its men are gone.
    Dead,
    /// Free pursuit: the figure picks its own victim and ignores its unit's
    /// destination. The original's **state 8**, and what `Order_ChargeNearest`
    /// puts a whole unit into. `docs/battle-ai.md` §4.1 — *a charged unit stops
    /// being a formation*.
    Chasing,
    /// Closing to shoot a chosen figure. The original's **state 17**, set by
    /// `Order_ShootAtUnit`.
    Shooting,
    /// Filling in the moat. The original's **state 9**, tracked here only
    /// because the unit rebuild reads it; nothing in this crate drives it.
    FillingMoat,
}

/// Sides are numbered 0 and 4 in the original, not 0 and 1 — side 0 deploys at
/// the `0x04` terrain marker and side 4 at `0x0F`. Kept as the original's
/// numbering so state read out of a live game compares directly.
pub type Side = u8;
pub const SIDE_A: Side = 0;
pub const SIDE_B: Side = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Figure {
    pub troop: Troop,
    pub side: Side,
    /// Real soldiers this drawn figure represents. At zero the figure dies.
    pub men: u16,
    /// Accumulated damage. At `hits_per_casualty` one man dies and the
    /// remainder carries over — damage is never wasted.
    pub hits: u16,
    /// Ticks until this figure can be struck again. Counts down; while positive
    /// the figure takes no melee damage. This is the game's only melee defence.
    pub recovery_counter: i32,
    pub state: State,
    pub role: Role,
    /// Index of the figure this one is duelling.
    pub opponent: Option<usize>,
    /// Blows left before the roles swap.
    pub exchange: i32,
    /// The heavy blow lands once. In the original this flag is set and, in the
    /// paths that were traced, never cleared — which would mean one heavy blow
    /// per figure per battle. `docs/battle.md` flags that as unresolved, so it
    /// is reproduced faithfully and marked here rather than quietly "fixed".
    pub blow_used: bool,
    /// Owner is a human player. Only observable effect found is that human-owned
    /// oil gets less armour, which is real in the original but unexplained.
    ///
    /// The battle AI reads it for a second reason: `Battle_UpdateStrengthAdvantage`
    /// splits the whole battlefield into human men and AI men on this byte, and
    /// the one number the entire AI turns on is the ratio between them.
    pub owner_is_human: bool,
    /// Which player owns this figure; `0` is "no owner".
    ///
    /// A unit's owner byte is copied straight from its figures, and
    /// `Enemy_NearestUnit` tells friend from foe by comparing **owners**, not
    /// sides — so two AI players are enemies to each other.
    pub owner: u8,
    /// The unit this figure belongs to, `0` for none.
    ///
    /// `l2-sim` does not raise units for you; [`crate::unit::Units`] is the
    /// array and whoever builds an army fills this in. Nothing in the melee or
    /// missile model reads it — only the AI does.
    pub unit: u16,
    /// Figure record `+0x15`: this figure was hit since the last unit rebuild.
    /// Set by whatever damages it, and **consumed** by
    /// [`crate::unit::Units::rebuild_from_figures`], which is what makes a
    /// unit's grudge last frames rather than blows.
    pub was_hit: bool,
    /// Figure record `+0x16`: who hit it. The rebuild looks up *that figure's
    /// unit* and stores it as the victim unit's remembered attacker. This is
    /// the only way a unit ever acquires a target it did not walk into or find
    /// by proximity — there is no threat assessment anywhere.
    pub hit_by: Option<usize>,
    /// The figure this one is chasing or shooting at, set by the two order
    /// actions that reach past the unit into its figures
    /// (`Order_ChargeNearest`, `Order_ShootAtUnit`).
    pub target: Option<usize>,
    /// This figure's combat constants, **copied in at construction** from the
    /// [`TroopTable`] in force.
    ///
    /// Carried per figure rather than looked up per blow, and that is the seam
    /// that makes the numbers data: melee and missile code reads `f.stats`,
    /// never a constant, so whatever table built the figure is the table the
    /// whole battle runs on. It also means a table cannot change under a
    /// running battle, which a lockstep peer very much needs.
    pub stats: TroopStats,
    /// Hits absorbed before one man dies, from the same table.
    pub hits_per_casualty: u16,
}

impl Figure {
    /// A figure using [`TroopTable::DEFAULT`].
    pub fn new(troop: Troop, side: Side, men: u16) -> Self {
        Figure::with_table(&TroopTable::DEFAULT, troop, side, men)
    }

    /// A figure using a supplied table — the modded path.
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
            stats,
            hits_per_casualty: table.hits_per_casualty(troop),
        }
    }

    pub fn stats(&self) -> TroopStats {
        self.stats
    }

    pub fn is_alive(&self) -> bool {
        self.state != State::Dead && self.men > 0
    }

    /// Missile defence. The one place `armour` is used, and the one place the
    /// human-owner asymmetry shows up.
    pub fn armour(&self) -> u16 {
        match (self.troop, self.owner_is_human) {
            (Troop::Oil, true) => 25,
            _ => self.stats().armour,
        }
    }

    /// [`take_hits`](Self::take_hits), and raise the was-hit flag naming the
    /// figure that did it.
    ///
    /// Separate from `take_hits` on purpose: the flag is what the unit rebuild
    /// turns into a unit's fifty-frame grudge, so damage that should *not*
    /// create a grudge — a test, a heavy blow being re-applied — still has a
    /// way to be dealt.
    pub fn take_hits_from(&mut self, amount: u16, by: usize) -> u16 {
        self.was_hit = true;
        self.hit_by = Some(by);
        self.take_hits(amount)
    }

    /// Apply damage, converting whole multiples of the kill threshold into
    /// casualties. Returns how many men died.
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

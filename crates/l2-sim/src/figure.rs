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

/// What a figure is *visibly* doing this tick: standing, walking, swinging,
/// drawing a bow, or falling.
///
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
///
/// It lives here because the simulation is what *chooses* it — the renderer
/// only turns it into a frame index, and two peers that disagree about it would
/// draw different battles from the same state. It is presentation all the same
/// and stays out of the lockstep digest (`docs/netcode.md`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Motion {
    Idle,
    Walking,
    Attacking,
    /// Drawing a bow. **The whole reload, not a window before the shot:**
    /// `BattleMan_FireMissile` ends `if (target != 0) Anim_DrawBow();`
    /// (`00480000.c:1531`), so the draw is held every tick a target is held,
    /// and the pose comes from a curve indexed by `reload_counter`.
    ///
    /// Troops with no bow never enter it, and there is no fallback if one
    /// does: `Anim_DrawBowA2` has one formula for every `troopType < 7` — no
    /// per-troop stride and no knight arm — and `l2_view::figures` returns
    /// `drawbow::frame` for every troop to match, past the end of a pikeman's
    /// sheet exactly as the binary does.
    Shooting,
    /// **Dead** — state 2, `BattleMan_StateDead` (`0x004830E9`), whose whole
    /// body is `Anim_Collapse(); if (0x50 < ++field_0x173) Destroy();`. The six
    /// frames of falling over come off the *death timer*, not `animPhase`.
    Dying,
    /// **Filling the moat** — state 9, `BattleMan_StateFillMoat`
    /// (`0x00483FE1`), the **only** caller of `Anim_Dying` → `Anim_DyingA2`
    /// (`0x00487908`) in the binary: a living man bent double over a shovel,
    /// twelve frames at `8N+6`.
    ///
    /// **It is not the corpse pose**, and collapsing the two is what drew a
    /// living shoveller in the frames every dead man wears — the player's
    /// *"a dead sprite but he was still alive"*, 2026-09-14.
    Shovelling,
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
    /// The heavy blow lands **once per figure for the whole battle**. `+0x18C`
    /// (`0x0055460C`) has exactly three absolute references in the executable:
    /// `BattleUnit_Create` zeroes it (`0x00480D47`) and `Melee_Tick`
    /// (`0x00494908`) reads and sets it (`0x00494BE7`, `0x00494C46`). Nothing
    /// resets it per exchange. **[V]** — a computed pointer would not appear in
    /// an absolute-reference count.
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
    /// unit's grudge last frames.
    pub was_hit: bool,
    /// Figure record `+0x16`: who hit it. The rebuild looks up *that figure's
    /// unit* and stores it as the victim unit's remembered attacker. This is
    /// the only way a unit ever acquires a target it did not walk into or find
    /// by proximity —.
    pub hit_by: Option<usize>,
    /// The figure this one is chasing or shooting at, set by the two order
    /// actions that reach past the unit into its figures
    /// (`Order_ChargeNearest`, `Order_ShootAtUnit`).
    pub target: Option<usize>,
    /// Figure record `+0x175`, the debug panel's own label **`targeted`**: how
    /// many men are currently running at this one, times two.
    ///
    /// `Melee_ChooseChaseTarget` adds it to the distance score and adds 2 to
    /// the figure it picks; `Battle_UpdateAllMen` counts it back down by one
    /// each frame. That makes free pursuit a **load balancer**
    /// focus-fire rule — `docs/battle-ai.md` §3.3.
    pub targeted: u8,
    /// This figure's combat constants, **copied in at construction** from the
    /// [`TroopTable`] in force.
    ///
    /// Carried per figure
    /// that makes the numbers data: melee and missile code reads `f.stats`,
    /// never a constant, so whatever table built the figure is the table the
    /// whole battle runs on. It also means a table cannot change under a
    /// running battle, which a lockstep peer very much needs.
    pub stats: TroopStats,
    /// Hits absorbed before one man dies, from the same table.
    pub hits_per_casualty: u16,
    /// Figure record `+0x180` — **the reload counter**, which
    /// `BattleMan_FireMissile` counts up once a tick.
    ///
    /// Ten ticks before it reaches the weapon's reload interval the figure
    /// acquires a target; when it passes the interval it looses a missile and
    /// resets. A figure with no missile weapon never touches it.
    pub reload_counter: u16,
    /// **The full complement one figure of this battle stands for** — the
    /// men-per-figure scale, not this figure's own men.
    ///
    /// `BattleMan_RecomputeStrength` compares a figure's men against three
    /// thresholds loaded from `g_strengthBandTable` *by battlefield size class*,
    /// so the comparison is against the scale
    /// figure started with. It is carried per figure because a figure carries
    /// everything else it is judged by, and because the two sides of one battle
    /// can be on different scales (`docs/battle.md` §5.1).
    ///
    /// Defaults to the figure's own men, which is right for the skirmish case
    /// where every figure is full; [`crate::runner::BattleRunner`] overwrites it
    /// with the side's scale when it raises an army.
    pub full_men: u16,
    /// Figure record `+0x09`, and the debug panel's own label **`selected`**:
    /// **which player has this figure picked**, `0` for nobody.
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
            targeted: 0,
            stats,
            hits_per_casualty: table.hits_per_casualty(troop),
            reload_counter: 0,
            full_men: men,
            selected: 0,
        }
    }

    /// **The strength band, 0 … 3** — `BattleMan_RecomputeStrength`,
    /// `docs/battle.md` §5.3.
    ///
    /// A figure that has lost men fights and shoots worse. The three thresholds
    /// are 75 %, 50 % and 3/16 of a full figure, which is exactly what
    /// `g_strengthBandTable` holds at every size class the ladder can produce:
    /// `3, 2, 1` of four men, `12, 8, 3` of sixteen, `768, 512, 192` of 1,024.
    /// **[V]** on the three fractions; the table itself is not in this tree, and
    /// deriving it from the scale is what makes an arbitrary
    /// [`Figure::full_men`] work.
    ///
    /// ```
    /// # use l2_sim::{Figure, Troop, SIDE_A};
    /// let mut f = Figure::new(Troop::Archers, SIDE_A, 4);
    /// assert_eq!(f.strength_band(), 0);
    /// f.men = 3; assert_eq!(f.strength_band(), 0, "three of four is still 75 %");
    /// f.men = 2; assert_eq!(f.strength_band(), 1);
    /// f.men = 1; assert_eq!(f.strength_band(), 2);
    /// ```
    ///
    /// **Only the missile path reads it.** [`crate::melee`] still swings at band
    /// 0 — its own comment says so — so wiring the melee column of
    /// `g_meleeAttackTable` in is outstanding work and not something this method
    /// quietly did.
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

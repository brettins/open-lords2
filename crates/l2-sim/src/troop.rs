//! Troop types and their combat constants.
//!
//! The numbers come from `docs/battle.md` §6.1, read out of the original's
//! per-type tick handlers and its melee attack table. They are facts about how
//! the 1996 engine behaves, reimplemented here rather than copied.
//!
//! Every value is an integer. Nothing in this crate uses floating point, because
//! a lockstep simulation has to be bit-identical across machines — see
//! `docs/netcode.md`.

/// The eleven troop slots, in the order the army record stores them.
///
/// Mirrors `l2_formats::Troop`; kept separate so the simulation does not depend
/// on file-format ordering staying put.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Troop {
    Peasants,
    Crossbowmen,
    Macemen,
    Swordsmen,
    Pikemen,
    Archers,
    Knights,
    Catapults,
    SiegeTowers,
    BatteringRams,
    Oil,
}

pub const ALL_TROOPS: [Troop; 11] = [
    Troop::Peasants,
    Troop::Crossbowmen,
    Troop::Macemen,
    Troop::Swordsmen,
    Troop::Pikemen,
    Troop::Archers,
    Troop::Knights,
    Troop::Catapults,
    Troop::SiegeTowers,
    Troop::BatteringRams,
    Troop::Oil,
];

impl Troop {
    /// Siege engines are never chosen as melee targets, and deal no melee
    /// damage. The original's target search skips them outright.
    pub fn is_siege(self) -> bool {
        matches!(
            self,
            Troop::Catapults | Troop::SiegeTowers | Troop::BatteringRams | Troop::Oil
        )
    }

    /// Hits needed to kill one man. The single constant the whole damage model
    /// turns on.
    pub fn hits_per_casualty(self) -> u16 {
        if self.is_siege() {
            160
        } else {
            100
        }
    }

    pub fn stats(self) -> TroopStats {
        use Troop::*;
        // melee_attack is indexed by strength band 0..=3, best band first.
        match self {
            Peasants =>      TroopStats::new([5, 4, 3, 2],    6,   0,  0,  40),
            Crossbowmen =>   TroopStats::new([5, 4, 3, 2],    8,   0, 12,  40),
            Macemen =>       TroopStats::new([15, 12, 10, 6], 12, 300, 12,  80),
            Swordsmen =>     TroopStats::new([15, 12, 10, 6], 12, 100, 35,  80),
            Pikemen =>       TroopStats::new([10, 8, 6, 5],   30,   0, 35,  80),
            Archers =>       TroopStats::new([5, 4, 3, 2],     6,   0,  0,  40),
            Knights =>       TroopStats::new([20, 15, 11, 6], 16, 200, 25, 120),
            Catapults =>     TroopStats::new([0; 4],          20,   0, 33,   0),
            SiegeTowers =>   TroopStats::new([0; 4],          15,   0, 35,   0),
            BatteringRams => TroopStats::new([0; 4],          30,   0, 50,   0),
            // Armour is 40, or 25 when the owner is human. That asymmetry is
            // real in the original but its intent was never established, so it
            // is applied explicitly at the call site rather than hidden here.
            Oil =>           TroopStats::new([0; 4],           8,   0, 40,   0),
        }
    }
}

/// Per-type combat constants.
///
/// Note what is *not* here: there is no separate melee defence value. A figure's
/// melee defence is its `recovery` — the interval between blows it suffers is
/// its own recovery counter, so a slow-recovering figure is struck rarely.
/// `armour` applies to missiles only and is never read during a melee exchange.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TroopStats {
    /// Damage per blow, by strength band 0..=3.
    pub melee_attack: [u16; 4],
    /// Ticks between blows suffered. This *is* melee defence.
    pub recovery: u16,
    /// Landed once per exchange, and large.
    pub heavy_blow: u16,
    /// Flat subtraction, missiles only.
    pub armour: u16,
    /// Blows before the attacker and defender roles swap.
    pub exchange: u16,
}

impl TroopStats {
    const fn new(
        melee_attack: [u16; 4],
        recovery: u16,
        heavy_blow: u16,
        armour: u16,
        exchange: u16,
    ) -> Self {
        TroopStats { melee_attack, recovery, heavy_blow, armour, exchange }
    }

    /// Attack for a strength band, clamped rather than panicking on a bad band.
    pub fn attack(&self, band: u8) -> u16 {
        self.melee_attack[(band as usize).min(3)]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn siege_engines_take_more_hits_to_kill() {
        assert_eq!(Troop::Peasants.hits_per_casualty(), 100);
        assert_eq!(Troop::Catapults.hits_per_casualty(), 160);
        for t in ALL_TROOPS {
            assert_eq!(t.hits_per_casualty(), if t.is_siege() { 160 } else { 100 });
        }
    }

    #[test]
    fn siege_engines_have_no_melee_attack() {
        for t in ALL_TROOPS.iter().copied().filter(|t| t.is_siege()) {
            assert_eq!(t.stats().melee_attack, [0; 4], "{t:?} should not fight in melee");
            assert_eq!(t.stats().exchange, 0);
        }
    }

    /// The printed manual gives no numbers, but it does rank things. These are
    /// the five rankings it states, asserted against the table — the strongest
    /// independent check available on values read out of a decompiler.
    #[test]
    fn the_table_matches_every_ranking_the_manual_states() {
        let atk = |t: Troop| t.stats().attack(0);
        let rec = |t: Troop| t.stats().recovery;

        // "pikemen's hand-to-hand attack is less than macemen, swordsmen and knights"
        assert!(atk(Troop::Pikemen) < atk(Troop::Macemen));
        assert!(atk(Troop::Pikemen) < atk(Troop::Swordsmen));
        assert!(atk(Troop::Pikemen) < atk(Troop::Knights));

        // "pikemen's defence value is relatively high" - longest recovery of all.
        let longest = ALL_TROOPS.iter().map(|t| rec(*t)).max().unwrap();
        assert_eq!(rec(Troop::Pikemen), longest);

        // "macemen are good attackers but weak defenders"
        let heaviest = ALL_TROOPS.iter().map(|t| t.stats().heavy_blow).max().unwrap();
        assert_eq!(Troop::Macemen.stats().heavy_blow, heaviest);
        assert!(Troop::Macemen.stats().armour < Troop::Swordsmen.stats().armour);

        // "knights have the highest attack"
        let best = ALL_TROOPS.iter().map(|t| atk(*t)).max().unwrap();
        assert_eq!(atk(Troop::Knights), best);

        // "archers are nearly useless against swordsmen and knights"
        assert_eq!(Troop::Archers.stats().heavy_blow, 0);
        assert_eq!(Troop::Archers.stats().armour, 0);
        assert!(atk(Troop::Archers) < atk(Troop::Swordsmen));
    }

    #[test]
    fn strength_bands_never_improve_with_a_worse_band() {
        for t in ALL_TROOPS {
            let s = t.stats();
            for b in 1..4 {
                assert!(
                    s.melee_attack[b] <= s.melee_attack[b - 1],
                    "{t:?} band {b} should not exceed band {}",
                    b - 1
                );
            }
            // Out-of-range bands clamp rather than panic.
            assert_eq!(s.attack(9), s.melee_attack[3]);
        }
    }
}

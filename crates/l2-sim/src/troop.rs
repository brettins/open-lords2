
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
    pub const fn index(self) -> usize {
        self as usize
    }

    pub const fn is_siege(self) -> bool {
        matches!(
            self,
            Troop::Catapults | Troop::SiegeTowers | Troop::BatteringRams | Troop::Oil
        )
    }

    pub fn hits_per_casualty(self) -> u16 {
        TroopTable::DEFAULT.hits_per_casualty(self)
    }

    pub fn stats(self) -> TroopStats {
        TroopTable::DEFAULT.stats(self)
    }
}

/// This type exists so the numbers can arrive from somewhere other than this
/// file. `docs/decisions.md` C11 is the reason: every constant the original
/// turns on lives in `Lords2.exe`, so modding the 1996 game means patching a
/// binary, and our engine has to carry the same constants as data that can be
/// handed to it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TroopTable {
    pub stats: [TroopStats; ALL_TROOPS.len()],
    pub hits_per_casualty: [u16; ALL_TROOPS.len()],
}

impl TroopTable {
    pub const DEFAULT: TroopTable = TroopTable {
        stats: [
            /* Peasants      */ TroopStats::new([5, 4, 3, 2],     6,   0,  0,  40),
            /* Crossbowmen   */ TroopStats::new([5, 4, 3, 2],     8,   0, 12,  40),
            /* Macemen       */ TroopStats::new([15, 12, 10, 6], 12, 300, 12,  80),
            /* Swordsmen     */ TroopStats::new([15, 12, 10, 6], 12, 100, 35,  80),
            /* Pikemen       */ TroopStats::new([10, 8, 6, 5],   30,   0, 35,  80),
            /* Archers       */ TroopStats::new([5, 4, 3, 2],     6,   0,  0,  40),
            /* Knights       */ TroopStats::new([20, 15, 11, 6], 16, 200, 25, 120),
            /* Catapults     */ TroopStats::new([0; 4],          20,   0, 33,   0),
            /* SiegeTowers   */ TroopStats::new([0; 4],          15,   0, 35,   0),
            /* BatteringRams */ TroopStats::new([0; 4],          30,   0, 50,   0),
            /* Oil           */ TroopStats::new([0; 4],           8,   0, 40,   0),
        ],
        hits_per_casualty: [100, 100, 100, 100, 100, 100, 100, 160, 160, 160, 160],
    };

    pub fn stats(&self, troop: Troop) -> TroopStats {
        self.stats[troop.index()]
    }

    pub fn hits_per_casualty(&self, troop: Troop) -> u16 {
        self.hits_per_casualty[troop.index()]
    }
}

impl Default for TroopTable {
    fn default() -> Self {
        TroopTable::DEFAULT
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TroopStats {
    pub melee_attack: [u16; 4],
    pub recovery: u16,
    /// Landed **once per figure for the whole battle**, and large — maceman
    /// 300, knight 200, swordsman 100, everyone else 0. `TroopTick_Maceman`
    /// (`0x00482789`) stores it at `0x004827D5`, `66 c7 80 18 46 55 00 2c 01`
    /// — `mov word [eax + 0x554618], 0x12C`, the figure's `+0x198`.
    pub heavy_blow: u16,
    pub armour: u16,
    pub exchange: u16,
}

impl TroopStats {
    pub const fn new(
        melee_attack: [u16; 4],
        recovery: u16,
        heavy_blow: u16,
        armour: u16,
        exchange: u16,
    ) -> Self {
        TroopStats { melee_attack, recovery, heavy_blow, armour, exchange }
    }

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

    #[test]
    fn the_table_matches_every_ranking_the_manual_states() {
        let atk = |t: Troop| t.stats().attack(0);
        let rec = |t: Troop| t.stats().recovery;

        assert!(atk(Troop::Pikemen) < atk(Troop::Macemen));
        assert!(atk(Troop::Pikemen) < atk(Troop::Swordsmen));
        assert!(atk(Troop::Pikemen) < atk(Troop::Knights));

        let longest = ALL_TROOPS.iter().map(|t| rec(*t)).max().unwrap();
        assert_eq!(rec(Troop::Pikemen), longest);

        let heaviest = ALL_TROOPS.iter().map(|t| t.stats().heavy_blow).max().unwrap();
        assert_eq!(Troop::Macemen.stats().heavy_blow, heaviest);
        assert!(Troop::Macemen.stats().armour < Troop::Swordsmen.stats().armour);

        let best = ALL_TROOPS.iter().map(|t| atk(*t)).max().unwrap();
        assert_eq!(atk(Troop::Knights), best);

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
            assert_eq!(s.attack(9), s.melee_attack[3]);
        }
    }
}

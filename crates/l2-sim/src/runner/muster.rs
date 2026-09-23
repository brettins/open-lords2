use super::*;

impl Muster<'_> {
    /// `menTotal`: types 0..6 plus the band (`Mercenary_Hire` `0x004AC7F3`;
    /// `Army_WithdrawCasualties` sums `t < 7`, docs/armies.md). **[V]**
    pub fn men(&self) -> u32 {
        let men = self.troops.iter().filter(|(t, _)| !t.is_siege()).map(|(_, n)| *n);
        men.sum::<u32>() + self.mercenaries.map_or(0, |(_, n)| n)
    }

    /// Battle_InitArmies 0x0047EFEE 00470000.c:6499-6504: counts of types 7..10.
    pub fn engines(&self) -> u32 {
        self.troops.iter().filter(|(t, _)| t.is_siege()).map(|(_, n)| *n).sum()
    }
}

pub fn size_class(total_men: u32) -> usize {
    SIZE_CLASS_LADDER.iter().position(|&b| total_men < b).unwrap_or(8)
}

/// `g_siegeScaleLadder` (`0x004D9618`), read from Lords2.exe: (5,4) (9,8)
/// (17,16) (33,32) (65,64) (129,128) (257,256) (513,512), else 1024. **[V]**
pub const SIEGE_SCALE_LADDER: [(u32, u32); 8] =
    [(5, 4), (9, 8), (17, 16), (33, 32), (65, 64), (129, 128), (257, 256), (513, 512)];

/// Battle_InitArmies (`0x0047EFEE`) 00470000.c:6505-6506: each engine
/// counts as `Table_Lookup(men / 76, g_siegeScaleLadder, 1024)` men. **[V]**
pub fn battle_size_class(a: &Muster, b: &Muster) -> usize {
    let men = a.men() + b.men();
    let per = SIEGE_SCALE_LADDER.iter().find(|&&(t, _)| men / 76 < t).map_or(1024, |&(_, v)| v);
    size_class(men + per * (a.engines() + b.engines()))
}

/// It is applied **once**; `docs/battle.md` lists the reachable results as
/// 4, 8, 16, 32 or 64, which is one halving from 8 … 128 and no more. **[D]**
/// on the once.
pub fn side_scale(side_men: u32, men_per_figure: u32) -> u32 {
    if men_per_figure > 7 && side_men / men_per_figure < 9 {
        (men_per_figure / 2).max(4)
    } else {
        men_per_figure
    }
}

#[cfg(test)]
mod size_tests {
    use super::*;

    /// 300 men, 2 rams: 300 / 76 = 3 -> 4 men an engine -> 308 -> class 1.
    /// Ablation: engines as men (302) or without the term (300) -> class 0.
    #[test]
    fn a_siege_engine_counts_as_the_ladders_men_when_sizing() {
        let a = [(Troop::Peasants, 200u32), (Troop::BatteringRams, 2)];
        let b = [(Troop::Peasants, 100u32)];
        let m = |troops| Muster { troops, owner: 1, human: false, mercenaries: None };
        assert_eq!((m(&a).men(), m(&a).engines()), (200, 2), "menTotal leaves engines out");
        assert_eq!(battle_size_class(&m(&a), &m(&b)), 1);
        let r = BattleRunner::deploy_muster(blank_field(), DEFAULT_SEED, m(&a), m(&b));
        assert_eq!(r.battle_size_class(), 1);
        assert_eq!(r.men_per_figure(SIDE_B), 8);
    }
}


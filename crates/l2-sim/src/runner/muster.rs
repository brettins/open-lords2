use super::*;

impl Muster<'_> {
    pub fn men(&self) -> u32 {
        self.troops.iter().map(|(_, n)| *n).sum()
    }
}

pub fn size_class(total_men: u32) -> usize {
    SIZE_CLASS_LADDER.iter().position(|&b| total_men < b).unwrap_or(8)
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


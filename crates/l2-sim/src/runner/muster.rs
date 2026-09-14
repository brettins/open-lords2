use super::*;

impl Muster<'_> {
    pub fn men(&self) -> u32 {
        self.troops.iter().map(|(_, n)| *n).sum()
    }
}

/// `Table_Lookup(total, g_sizeClassLadder, 8)`: the first threshold the total
/// does not reach, or 8.
///
/// ```
/// # use l2_sim::runner::{size_class, MEN_PER_FIGURE_TABLE};
/// // docs/battle.md §5.4 derives USER.SKR map 0 independently: 600 v 450 men,
/// // size class 2, 16 men a figure.
/// assert_eq!(size_class(600 + 450), 2);
/// assert_eq!(MEN_PER_FIGURE_TABLE[size_class(1050)], 16);
/// // …and the blank template, 150 v 150, is class 0 at four men a figure.
/// assert_eq!(MEN_PER_FIGURE_TABLE[size_class(300)], 4);
/// ```
pub fn size_class(total_men: u32) -> usize {
    SIZE_CLASS_LADDER.iter().position(|&b| total_men < b).unwrap_or(8)
}

/// The **per-side** refinement of §5.1: a side that would draw fewer than nine
/// figures halves the scale,
/// large one. Floor of four men, the smallest figure the ladder produces.
///
/// The original's condition is `sideTotal / menPerFigure < 9 && menPerFigure > 7`.
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


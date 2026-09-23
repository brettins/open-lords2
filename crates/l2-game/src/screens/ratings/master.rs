//! The Battle Master's book — `FUN_0042CC4A` (`0x0042CC4A`), `FUN_0042D179`
//! (`0x0042D179`) and the two halves of `score.dat`, `FUN_0042D216` and
//! `FUN_0042D2DC`.
//!
//! ```text
//! FUN_0042D179():                       the defaults, no file
//!   DAT_0051FD00[i] := DAT_004D49C8[i]     500 400 300 200 100 80 60 40 20 10
//!   DAT_0051FD28[i] := 0
//!   Eng_Seek(7, DAT_004D49F0[i]); copy 0x20 -> DAT_0051FBC0 + i*0x20
//! FUN_0042D216():  read  score.dat  0xA0 scores, 0x140 names, rank, battles, total
//! FUN_0042D2DC():  write score.dat  the same five, in the same order
//! ```
use super::*;

/// `DAT_004D49C8`, ten `int`s at file offset `0xD3BC8`. **[V]**
pub const DEFAULT_SCORES: [i32; TOP] = [500, 400, 300, 200, 100, 80, 60, 40, 20, 10];

/// `DAT_004D49F0` — the `Eng_Seek(7, …)` index of each default name.
pub const DEFAULT_NAME_ENG: [usize; TOP] = [2, 1, 3, 4, 1, 2, 3, 2, 1, 4];

/// `L2.eng` group 7, the four lord titles `FUN_0042D179` seeks into.
pub const GROUP_LORDS: usize = 7;

/// The ten rows `Screen_BattleMasterRank` draws from `0x0051FBC0` /
/// `0x0051FD00`, and the ten `FUN_0042CC4A` inserts into.
pub const TOP: usize = 10;

/// `Screen_BattleMasterRank` draws **five** of `0x0051FD28`; `FUN_0042CC4A`
/// inserts into ten of it. Both numbers are the original's.
pub const MINE_SHOWN: usize = 5;

/// `FUN_0042CC4A`'s `local_14`: the highest rank `DAT_00568988` battles allow.
/// Thresholds `3, 5, 7, 9, 0xB, 0x15, 0x1F, 0x29, 0x33, 0x4C, 0x65, 0x97`.
pub const RANK_CAP: [(i32, i32); 12] = [
    (3, 0),
    (5, 1),
    (7, 2),
    (9, 3),
    (0xB, 4),
    (0x15, 5),
    (0x1F, 6),
    (0x29, 7),
    (0x33, 8),
    (0x4C, 9),
    (0x65, 10),
    (0x97, 11),
];

/// `FUN_0042CC4A`'s `local_10`, added to `average / 100` before the cap.
pub const RANK_BONUS: [(i32, i32); 12] = [
    (3, 0),
    (5, 0),
    (7, 0),
    (9, 0),
    (0xB, 0),
    (0x15, 0),
    (0x1F, 1),
    (0x29, 1),
    (0x33, 1),
    (0x4C, 1),
    (0x65, 2),
    (0x97, 2),
];

/// `L2.eng` group 38 — *"Rank of Private"* … *"Rank of Supreme commander"*.
pub const RANKS: usize = 12;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Entry {
    pub name: String,
    pub score: i32,
}

/// The five things `score.dat` holds, plus the two scores the last battle left
/// in `DAT_0053E9B4` and `DAT_0057C95C`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BattleMaster {
    /// `DAT_0051FBC0` (names) and `DAT_0051FD00` (scores), one table.
    pub top: Vec<Entry>,
    /// `DAT_0051FD28` — the local player's own scores, ten kept, five drawn.
    pub mine: [i32; TOP],
    /// `DAT_0053E9E0`, the `L2.eng` group 38 index.
    pub rank: usize,
    /// `DAT_00568988`, battles counted.
    pub battles: i32,
    /// `DAT_00568214`, every local score ever added up.
    pub total: i32,
    /// `DAT_0053E9B4` and `DAT_0057C95C`, the last battle's two scores.
    pub last: Ratings,
    /// Which `l2_sim` side `DAT_00553EFC` — the local army record — is.
    pub local_side: u8,
}

impl Default for BattleMaster {
    fn default() -> BattleMaster {
        BattleMaster {
            top: Vec::new(),
            mine: [0; TOP],
            rank: 0,
            battles: 0,
            total: 0,
            last: Ratings::default(),
            local_side: l2_sim::SIDE_A,
        }
    }
}

/// `FUN_0042CC4A`'s ladder: the first threshold the count is under, and its
/// final `else` — cap `0xB`, bonus 3 — when it is under none.
fn step(table: &[(i32, i32); 12], battles: i32, past_the_end: i32) -> i32 {
    table.iter().find(|(limit, _)| battles < *limit).map_or(past_the_end, |&(_, v)| v)
}

impl BattleMaster {
    /// `FUN_0042D179` — the table with no `score.dat`: ten scores out of the
    /// executable and ten names out of `L2.eng` group 7. `FUN_0042D216`'s
    /// missing-file limb leaves it standing (`score_dat.rs`).
    pub fn seed_defaults(&mut self, assets: &crate::shell::ShellAssets) {
        if !self.top.is_empty() {
            return;
        }
        self.top = (0..TOP)
            .map(|i| Entry {
                name: assets.text(GROUP_LORDS, DEFAULT_NAME_ENG[i]).to_string(),
                score: DEFAULT_SCORES[i],
            })
            .collect();
    }

    /// **`FUN_0042CC4A`** — what a finished skirmish does to the book.
    ///
    /// `DAT_00568988 += 1; DAT_00568214 += local; DAT_0055CE8C = total/battles`,
    /// then `DAT_0053E9E0 = average/100 + bonus`, floored at 0 and capped.
    /// The dedupe then the three sorted inserts, in the original's order.
    pub fn record(&mut self, local_name: &str, foe_name: &str, mine: i32, theirs: i32) {
        self.battles += 1;
        self.total += mine;
        let average = self.total / self.battles;
        let cap = step(&RANK_CAP, self.battles, 11);
        self.rank = (average / 100 + step(&RANK_BONUS, self.battles, 3)).clamp(0, cap) as usize;

        // `if (DAT_0057C95C < DAT_0053E9B4)` picks **whose** name the dedupe
        // reads, and a hit returns before any insert happens.
        let (who, score) =
            if theirs < mine { (local_name, mine) } else { (foe_name, theirs) };
        if self.top.iter().any(|e| e.score == score && e.name == who) {
            return;
        }

        insert(&mut self.mine, mine);
        self.insert_top(local_name, mine);
        self.insert_top(foe_name, theirs);
    }

    /// One of `FUN_0042CC4A`'s two identical tails: find the first row this
    /// score beats, shift the rest down, drop the tenth.
    fn insert_top(&mut self, name: &str, score: i32) {
        let Some(at) = self.top.iter().position(|e| e.score < score) else { return };
        self.top.insert(at, Entry { name: name.to_string(), score });
        self.top.truncate(TOP);
    }

    /// `L2.eng` 38's index, and `Screen_BattleMasterRank` hands it to
    /// `Ui_DrawCentred(0x26, DAT_0053E9E0, …)` unclamped.
    pub fn rank_index(&self) -> usize {
        self.rank.min(RANKS - 1)
    }
}

fn insert(list: &mut [i32; TOP], score: i32) {
    let Some(at) = list.iter().position(|&s| s < score) else { return };
    for i in (at + 1..TOP).rev() {
        list[i] = list[i - 1];
    }
    list[at] = score;
}

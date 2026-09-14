//! **The skirmish setup — `g_setupPage` 12, and the file box 13 over it.**
//!
//! A skirmish is one battle with no kingdom behind it: `Skirmish_Setup`
//! (`0x0042B7F7`) pairs realm `g_localPlayer` against `DAT_0056D5CC`, fills
//! army records `g_battleArmyA` = 1 and `g_battleArmyB` = 2 out of the troops
//! table, raises `DAT_0057A0F0` — the flag that makes `Battle_Start` skip every
//! write-back to a campaign that is not there — and leaves the page up until
//! *Go*.
//!
//! The page answers seven things, and this is all of them:
//!
//! | arm | what |
//! |---|---|
//! | `0x0043D929` | one of the six visible battle rows |
//! | `0x0043D9CD` | the list's two scroll arrows, hotspot ∓1 |
//! | `0x0043DC1D` | one of the four categories, hotspot **is** `DAT_0053E91C` |
//! | `0x0043DD83` | swap attacker and defender |
//! | `0x0043DAF3` | the handicap seesaw, hotspot 1 left, 2 right |
//! | `0x0043DA9E` | *Cust.* / *Norm.* — `DAT_0056899C` |
//! | `0x0043DDF4` | the `.skr` name field, which opens page 13 |
//!
//! and page 13 answers one, `0x00434174`, a row of the file list.

use l2_sim::Troop;

/// `g_troopsRowBase` (`0x004D49B8`) — the first table row of each category,
/// and with it the row counts `FUN_0043DC1D` sets `DAT_0053F0D4` to: ten
/// random field battles, ten more, fifteen castles, twenty from a `.skr`.
/// **[V]** from the exe.
pub const ROW_BASE: [usize; 4] = [0, 10, 20, 35];

/// `g_troopStrengthWeight` (`0x004D4B98`) — **eleven** entries, not seven.
/// `Army_StrengthScore` and `Battle_InitArmies` stop at the seven men types;
/// `Skirmish_FillArmies` runs the whole width, so a skirmish's strength counts
/// the four engines at 100, 100, 100 and 150. **[V]**, read out of the exe;
/// `l2_kingdom::unit::TROOP_STRENGTH_WEIGHT` is the first seven of it.
pub const STRENGTH_WEIGHT: [i32; 11] = [2, 16, 8, 13, 9, 13, 22, 100, 100, 100, 150];

/// How many of the list's rows are on screen at once — `FUN_00421005`'s loop.
pub const ROWS_SHOWN: usize = 6;

/// How many battles a category offers. `FUN_0043DC1D` writes `DAT_0053F0D4`.
pub fn rows_in(kind: usize) -> usize {
    match kind {
        3 => 20,
        2 => 15,
        _ => 10,
    }
}

/// The eleven counts of one army, as `Skirmish_FillArmies` (`0x0042BF46`)
/// leaves them: the row copied out of the table, then the two derived numbers
/// the same function recomputes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SkirmishArmy {
    /// Army record `+0x04` … — eleven `short`s, seven of men and four of
    /// engines.
    pub counts: [i16; 11],
    /// Army record `+0x00` (`DAT_0052F218`): the sum of the **first seven**.
    pub men: i32,
    /// `DAT_0051FBBC` for the local army and `DAT_0051FAD0` for the other —
    /// the autocalc strength the page prints under each name.
    pub strength: i32,
}

impl SkirmishArmy {
    /// The same, for a test that has no table.
    pub fn from_counts_for_test(counts: [i16; 11]) -> Self {
        Self::from_counts(counts)
    }

    fn from_counts(counts: [i16; 11]) -> Self {
        let men = counts[..7].iter().map(|&c| c as i32).sum();
        let strength =
            counts.iter().zip(STRENGTH_WEIGHT).map(|(&c, w)| c as i32 * w).sum();
        SkirmishArmy { counts, men, strength }
    }
}

/// **`g_troopsTable` (`0x00516AC0`) — `short[35][2][5][11]`.**
///
/// Side outer at stride `0x6E`, difficulty inner at stride `0x16`
/// `Troops_Load` (`0x0042AC0C`) parses `TROOPS2.ENG` (the local player
/// attacking) or `TROOPS3.ENG` (defending) into it. Rows 35…54 are written
/// instead by `FUN_0042D4AA` when a `.skr` file is loaded, which scales the
/// file's own counts by 5/3, 4/3, 1, 2/3 and 1/3 across the five difficulty
/// columns.
///
/// The parse is not here: this is the shape the page and the fill read, and a
/// checkout with no install has it empty.
#[derive(Clone, Debug, Default)]
pub struct TroopsTable {
    pub rows: Vec<[[[i16; 11]; 5]; 2]>,
}

impl TroopsTable {
    /// `(&g_troopsTable)[(rowBase[kind] + row) * 0xDC + side * 0x6E + diff * 0x16]`.
    pub fn counts(&self, kind: usize, row: usize, side: usize, diff: u8) -> [i16; 11] {
        let i = ROW_BASE.get(kind).copied().unwrap_or(0) + row;
        match self.rows.get(i) {
            Some(r) => r[side.min(1)][(diff as usize).min(4)],
            None => [0; 11],
        }
    }
}

/// The page's own state. Every field is one of the original's globals.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Skirmish {
    /// `DAT_0053E91C` — 0 and 1 field battles, 2 castles, 3 a `.skr` file.
    pub kind: usize,
    /// `DAT_0053F64C`, the first row drawn.
    pub top: usize,
    /// `DAT_0053E9A8`, which of the six drawn rows is lit.
    pub slot: usize,
    /// `DAT_0056D590` — the row itself, `slot + top`, and the one the fill
    /// reads. It is a separate global and it does **not** always agree with
    /// the other two; see [`Skirmish::scroll`].
    pub row: usize,
    /// `DAT_0053EF5C == g_localPlayer`. The attacker's half of the table is
    /// side 1.
    pub local_attacks: bool,
    /// Realm `+0x24` for realms 1 and 2 — `DAT_00553DA4` and `DAT_00553DD0`,
    /// the difficulty column each army is drawn from.
    pub difficulty: [u8; 2],
    /// `DAT_0056899C` — whether *Cust.* has turned the two musters into the
    /// custom pickers. The picture that goes with it is not built.
    pub custom: bool,
    /// `DAT_0053F5FC`, the chosen `.skr` file. Page 13 writes it.
    pub file: Option<String>,
}

impl Default for Skirmish {
    /// **`FUN_0042B919` (`0x0042B919`)**, fifteen assignments, and these are
    /// the eight of them that live here: category 0, row 0, scroll 0, slot 0,
    /// not custom, `DAT_0053EF5C = 1` — realm 1, which in single player is the
    /// local player, so he attacks — and both handicaps at 2, the middle of
    /// the three the seesaw allows.
    fn default() -> Self {
        Skirmish {
            kind: 0,
            top: 0,
            slot: 0,
            row: 0,
            local_attacks: true,
            difficulty: [2, 2],
            custom: false,
            file: None,
        }
    }
}

impl Skirmish {
    /// **One of the six rows** — `FUN_0043D929`, whose whole body is guarded by
    /// `g_uiHotspotId != DAT_0053E9A8`: clicking the lit row is not a click.
    pub fn pick_row(&mut self, slot: usize) -> bool {
        if slot == self.slot || slot >= ROWS_SHOWN {
            return false;
        }
        self.slot = slot;
        self.row = slot + self.top;
        true
    }

    /// **The two arrows** — `FUN_0043D9CD`, hotspot −1 and +1.
    ///
    /// The clamp is an `else`: when the scroll runs off either end the
    /// original stops at the end **and does not recompute `DAT_0056D590`**, so
    /// the lit row and the row that will be fought disagree until the next
    /// click. Kept, because it is what the game does.
    pub fn scroll(&mut self, delta: i32) {
        let last = rows_in(self.kind).saturating_sub(ROWS_SHOWN) as i32;
        let top = self.top as i32 + delta;
        if top > last {
            self.top = last.max(0) as usize;
        } else if top < 0 {
            self.top = 0;
        } else {
            self.top = top as usize;
            self.row = self.slot + self.top;
        }
    }

    /// **One of the four categories** — `FUN_0043DC1D`, where the hotspot id
    /// *is* the category. Hotspot 3 with no `.skr` loaded falls back to 0.
    /// The row, the slot and the scroll all go back to zero.
    pub fn choose_kind(&mut self, kind: usize) {
        self.kind = if kind == 3 && self.file.is_none() { 0 } else { kind };
        self.top = 0;
        self.slot = 0;
        self.row = 0;
    }

    /// **Swap attacker and defender** — `FUN_0043DD83`. It reloads the troops
    /// table, because which side the local player is on picks the *file*:
    /// `TROOPS2.ENG` attacking, `TROOPS3.ENG` defending (`Troops_Load`).
    pub fn swap_sides(&mut self) {
        self.local_attacks = !self.local_attacks;
    }

    /// **The handicap seesaw** — `FUN_0043DAF3`. Hotspot 1 takes one column
    /// off realm 1 and gives one to realm 2, hotspot 2 the other way, and both
    /// are clamped to 1…3 — so the outer columns 0 and 4 of the table are
    /// unreachable from this page. `FUN_0043DBAC` is the other difficulty arm
    /// and it cycles one realm 0…4; it belongs to the multiplayer page.
    pub fn handicap(&mut self, hotspot: usize) {
        let (a, b): (i32, i32) = match hotspot {
            1 => (-1, 1),
            2 => (1, -1),
            _ => return,
        };
        self.difficulty[0] = (self.difficulty[0] as i32 + a).clamp(1, 3) as u8;
        self.difficulty[1] = (self.difficulty[1] as i32 + b).clamp(1, 3) as u8;
    }

    /// ***Cust.*** — `FUN_0043DA9E`, which flips the flag and puts both
    /// handicaps back to 2.
    pub fn toggle_custom(&mut self) {
        self.custom = !self.custom;
        self.difficulty = [2, 2];
    }

    /// **A row of page 13's file list** — `FUN_00434174`. Taking a name sets
    /// the category to 3, loads the file and returns to page 12.
    pub fn choose_file(&mut self, name: &str) {
        if self.file.as_deref() == Some(name) {
            return;
        }
        self.file = Some(name.to_string());
        self.kind = 3;
        self.top = 0;
        self.slot = 0;
        self.row = 0;
    }

    /// Which half of the table each army is drawn from — `local_c` in
    /// `Skirmish_FillArmies`, computed once per army as
    /// `DAT_0053EF5C == owner`.
    pub fn side_of(&self, local: bool) -> usize {
        usize::from(self.local_attacks == local)
    }

    /// **`Skirmish_FillArmies` (`0x0042BF46`)**, both armies, in its order.
    ///
    /// Returns `(local, other)`. The army records they land in are
    /// `DAT_00553EFC` and `DAT_00553F34`, which `FUN_0042BA40` sets to
    /// `(2, 1)` when the local player is the attacker and `(1, 2)` when he is
    /// not — [`Skirmish::slots`].
    pub fn fill_armies(&self, table: &TroopsTable) -> (SkirmishArmy, SkirmishArmy) {
        let mine = table.counts(self.kind, self.row, self.side_of(true), self.difficulty[0]);
        let theirs = table.counts(self.kind, self.row, self.side_of(false), self.difficulty[1]);
        (SkirmishArmy::from_counts(mine), SkirmishArmy::from_counts(theirs))
    }

    /// `FUN_0042BA40`'s two army slots for `(local, other)`: the attacker is
    /// `g_battleArmyA` = 1 and the defender `g_battleArmyB` = 2.
    pub fn slots(&self) -> (usize, usize) {
        if self.local_attacks { (1, 2) } else { (2, 1) }
    }

    /// A seed that is a pure function of the page — no clock and no counter,
    /// `docs/netcode.md`. The original takes the field from a walking playlist
    /// cursor instead; [`crate::batfield::field`] is the other end of this.
    pub fn seed(&self) -> u64 {
        let d = self.difficulty;
        let n = (self.kind as u64) << 32
            | (self.row as u64) << 16
            | (d[0] as u64) << 8
            | d[1] as u64
            | u64::from(self.local_attacks) << 40;
        n.wrapping_mul(0x9E37_79B9_7F4A_7C15)
    }
}

/// The eleven counts as the simulation wants them — the tail of
/// [`crate::engagement::muster_with`], which drops the empty types.
pub fn muster(counts: &[i16; 11]) -> Vec<(Troop, u32)> {
    (0..11)
        .filter(|&t| counts[t] > 0)
        .map(|t| (l2_sim::ALL_TROOPS[t], counts[t] as u32))
        .collect()
}

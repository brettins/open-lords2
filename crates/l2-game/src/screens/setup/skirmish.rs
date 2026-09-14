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
/// **[V]** — read out of `Lords2.exe` at `0x004D49B8` (file offset
/// `0xD2BB8`); `docs/symbols.md` still calls the four inferred.
pub const ROW_BASE: [usize; 4] = [0, 10, 20, 35];

/// `g_troopStrengthWeight` (`0x004D4B98`) — **eleven** entries, not seven.
/// `Army_StrengthScore` and `Battle_InitArmies` stop at the seven men types;
/// `Skirmish_FillArmies` runs the whole width, so a skirmish's strength counts
/// the four engines at 100, 100, 100 and 150. **[V]**, read out of the exe;
/// `l2_kingdom::unit::TROOP_STRENGTH_WEIGHT` is the first seven of it.
pub const STRENGTH_WEIGHT: [i32; 11] = [2, 16, 8, 13, 9, 13, 22, 100, 100, 100, 150];

/// `DAT_004D4B58` — the castle level of each of the fifteen castle battles.
/// `FUN_0043D929` (`00430000.c:7950`) reads it by the chosen row into
/// `DAT_0057C910` (`g_castleLevel`) whenever the category is 2, and
/// `FUN_0043DC1D` (`00430000.c:8098`) puts entry 0 back when the category is
/// picked. **[V]**, fifteen `int`s read out of `Lords2.exe` at `0x004D4B58`
/// (file offset `0xD2D58`); the sixteenth is padding before
/// `g_troopStrengthWeight`.
pub const CASTLE_LEVEL: [u8; 15] = [0, 0, 0, 0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1];

/// `DAT_004D4B18` — which of the same fifteen has a drawbridge.
/// `Siege_LowerDrawbridge` (`0x00496B9F`, `00490000.c:2672`) reads it by the
/// same row in place of its `g_castleLevel >= 3` guard once `DAT_0057A0F0` is
/// up. **[V]** from `Lords2.exe` at `0x004D4B18` (file offset `0xD2D18`): 1 at
/// rows 5, 6 and 9…13, where `docs/symbols.md` 0x00496B9F names 5, 6 and 9
/// only. Nothing of ours reads it yet.
pub const CASTLE_DRAWBRIDGE: [bool; 15] = [
    false, false, false, false, false, true, true, false, false, true, true, true, true, true,
    false,
];

/// How many of the list's rows are on screen at once — `FUN_00421005`'s loop.
pub const ROWS_SHOWN: usize = 6;

/// How many battles a category offers — the value `FUN_0043DC1D`
/// (`0x0043DC1D`, `00430000.c:8081-8089`) writes into `DAT_0053F0D4`. That
/// global is the *only* reader afterwards, and nothing else writes it, so a
/// page-13 pick that sets the category to 3 leaves the old count standing:
/// [`Skirmish::rows`] stores it rather than deriving it.
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
    /// Eleven `short`s from `0x0052F21C` — seven of men and four of engines.
    pub counts: [i16; 11],
    /// The army record's `menTotal` at `+0x168` (`0x0052F218`, the word before
    /// the counts): the sum of the **first seven**. `Skirmish_FillArmies`
    /// (`0x0042BF46`) recomputes it. **[V]** from the record's header.
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
    /// `DAT_0053F0D4`, how many rows the list has. Only `FUN_0043DC1D` writes
    /// it (`00430000.c:8081-8089`), so it is stored and not derived: after
    /// page 13 sets the category to 3 the previous category's count stands.
    pub rows: usize,
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
            rows: 10,
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
        if slot == self.slot {
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
        let last = self.rows.saturating_sub(ROWS_SHOWN) as i32;
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
    /// The row count, the row, the slot and the scroll all follow.
    pub fn choose_kind(&mut self, kind: usize) {
        self.kind = if kind == 3 && self.file.is_none() { 0 } else { kind };
        self.rows = rows_in(self.kind);
        self.top = 0;
        self.slot = 0;
        self.row = 0;
    }

    /// **Swap attacker and defender** — `FUN_0043DD83`, which moves
    /// `DAT_0053EF5C` between `g_localPlayer` and `DAT_0056D5CC`
    /// (`00430000.c:8120-8126`).
    ///
    /// **[D]** the original then calls `Troops_Load` (`0x0042AC0C`), because
    /// which side the local player is on picks the *file*: `TROOPS2.ENG`
    /// attacking, `TROOPS3.ENG` defending. The parse is not built and the
    /// table is handed in, so ours cannot reload; the sides swap and the
    /// counts stay whichever file the caller supplied.
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

    /// **A row of page 13's file list** — `FUN_00434174`
    /// (`00430000.c:1371-1376`). Taking a name writes `DAT_0053F5FC` and the
    /// category, loads the file and returns to page 12. It writes **nothing
    /// else**: `DAT_0053F64C`, `DAT_0053E9A8`, `DAT_0056D590` and
    /// `DAT_0053F0D4` all keep the previous category's values, so the list is
    /// still scrolled where it was and still as long as it was.
    ///
    /// Returns whether the page changed — choosing the name already chosen
    /// returns 0 and stays on page 13 (`00430000.c:1364-1367`).
    pub fn choose_file(&mut self, name: &str) -> bool {
        if self.file.as_deref() == Some(name) {
            return false;
        }
        self.file = Some(name.to_string());
        self.kind = 3;
        true
    }

    /// `DAT_0053EF5C == 1`, the test `FUN_004209C1` (`00420000.c:246`) puts
    /// the role captions on — realm 1 attacking, not the local player
    /// attacking. `FUN_0042B919` starts `DAT_0053EF5C` at 1 and `FUN_0043DD83`
    /// moves it between `g_localPlayer` and `DAT_0056D5CC`, so on this page,
    /// where the local player is realm 1 and the opponent is not, the two
    /// tests coincide. **Inferred** — nothing here reads a realm number.
    pub fn local_realm(&self) -> u8 {
        1
    }

    /// `DAT_0057C910` for the chosen row — `FUN_0043D929`
    /// (`00430000.c:7949-7952`) writes it out of [`CASTLE_LEVEL`], and only
    /// for category 2. The row indexes the table; it is not the level.
    pub fn castle_level(&self) -> Option<u8> {
        (self.kind == 2).then(|| CASTLE_LEVEL.get(self.row).copied().unwrap_or(0))
    }

    /// `g_localPlayer` as this page has it: realm **1**. `Skirmish_Setup`
    /// (`0x0042B7F7`) pairs it against `DAT_0056D5CC` and nothing on page 12
    /// moves it. **Inferred** — no realm number is stored here.
    pub fn realm1_attacks(&self) -> bool {
        self.local_attacks
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

    /// `FUN_0042BA40`'s two army records for `(local, other)` —
    /// `DAT_00553EFC` and `DAT_00553F34`, `00420000.c:4665-4673`. When
    /// `DAT_0053EF5C == g_localPlayer` the local player takes record **2**
    /// (`g_battleArmyB`) and the opponent record 1 (`g_battleArmyA`); the
    /// other branch is the other way round. So `g_battleArmyA` always holds
    /// the side the local player is *not* on when he attacks.
    pub fn slots(&self) -> (usize, usize) {
        if self.local_attacks { (2, 1) } else { (1, 2) }
    }

    /// **[D]** a seed that is a pure function of the page — no clock and no
    /// counter, `docs/netcode.md`. The original has no seed here: the field's
    /// identity comes from the category and the row through `FUN_0042B9C4`
    /// (`0x0042B9C4`, `00420000.c:4608-4619`), which picks the builder, and
    /// `Battlefield_BuildRandom` draws on the global PRNG.
    /// [`crate::batfield::field`] is the other end of this.
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

//! `TROOPS.ENG` / `TROOPS2.ENG` / `TROOPS3.ENG` — `Troops_Load`
//! (`0x0042AC0C`, `00420000.c:4317-4503`), the skirmish page's army table.
//!
//! Skip to the first `*`, take every decimal run and ignore everything else,
//! then post-process. `docs/formats/eng.md` §3 is the format; this is the
//! reader, and [`Troops::load`] is the whole function including the three
//! passes after the parse that the text file's own difficulty rows lose to.

use crate::{Error, Result};

pub const ROWS: usize = 0x23;
pub const DIFFS: usize = 5;
pub const SIDES: usize = 2;
pub const SLOTS: usize = 11;

/// The first of the four siege columns — `Ca To Ra Oi`, the ones
/// `00420000.c:4419` clamps and `00420000.c:4480` leaves unscaled.
pub const SIEGE_SLOT: usize = 7;

/// `if (9 < x) x = 9` on slots 7…10, in every one of the ten sub-arrays
/// (`00420000.c:4418-4452`).
pub const SIEGE_MAX: i16 = 9;

/// The column the file is authoritative for: `DAT_00516AEC` is side 0's and
/// `DAT_00516B5A` side 1's, both `+0x2C`/`+0x9A` off `g_troopsTable`.
pub const NORMAL: usize = 2;

/// `Pct(normal, p)` per difficulty — `00420000.c:4481-4499`, where 0 and 4 take
/// `Pct(x, 0x10)` and 1 and 3 take `Pct(x, 8)`, added for 0 and 1 and
/// subtracted for 3 and 4.
pub const SCALE_PERCENT: [i32; DIFFS] = [116, 108, 100, 92, 84];

pub const TOKENS: usize = ROWS * (1 + DIFFS * SIDES * SLOTS);

/// The file as written: `[row][difficulty][side][slot]`, which is the order the
/// tokens arrive in and **not** the order they are stored in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawTroops {
    /// `g_troopsAdvantage` (`0x0051FAE0`), clamped to 0…10 as it is read
    /// (`00420000.c:4392-4397`).
    pub advantage: [i16; ROWS],
    pub counts: Box<[[[[i16; SLOTS]; SIDES]; DIFFS]; ROWS]>,
}

/// `g_troopsTable` (`0x00516AC0`) after `Troops_Load` has finished with it:
/// `short[35][2][5][11]`, side outer at stride `0x6E` and difficulty inner at
/// `0x16`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Troops {
    pub advantage: [i16; ROWS],
    pub rows: Box<[[[[i16; SLOTS]; DIFFS]; SIDES]; ROWS]>,
}

/// Which of the three files `Troops_Load` opens (`00420000.c:4349-4369`), and
/// the two globals are traced: `g_multiplayer` first, then
/// `DAT_0053EF5C == g_localPlayer` — whether the local player is the
/// **attacker**. `docs/formats/eng.md` §3.3 had this open.
pub fn file_for(multiplayer: bool, local_attacks: bool) -> &'static str {
    match (multiplayer, local_attacks) {
        (true, _) => "TROOPS.ENG",
        (false, true) => "TROOPS2.ENG",
        (false, false) => "TROOPS3.ENG",
    }
}

/// The token scan alone (`00420000.c:4372-4416`).
pub fn parse_raw(bytes: &[u8]) -> Result<RawTroops> {
    let start = bytes.iter().position(|&b| b == b'*').ok_or(Error::TroopsNoMarker)?;
    let mut tokens: Vec<i16> = Vec::with_capacity(TOKENS);
    let mut i = start;
    while i < bytes.len() {
        if !bytes[i].is_ascii_digit() {
            i += 1;
            continue;
        }
        let from = i;
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            i += 1;
        }
        let text = String::from_utf8_lossy(&bytes[from..i]).into_owned();
        // `FUN_00401666` is `atoi`; a run that does not fit a `short` is not
        // something any shipped file has, so it is refused rather than wrapped.
        match text.parse::<i16>() {
            Ok(v) => tokens.push(v),
            Err(_) => return Err(Error::TroopsBadToken(text)),
        }
    }
    if tokens.len() != TOKENS {
        return Err(Error::TroopsTokenCount { found: tokens.len(), expected: TOKENS });
    }

    let mut raw = RawTroops {
        advantage: [0; ROWS],
        counts: Box::new([[[[0i16; SLOTS]; SIDES]; DIFFS]; ROWS]),
    };
    let mut t = tokens.into_iter();
    for row in 0..ROWS {
        raw.advantage[row] = t.next().unwrap_or(0).clamp(0, 10);
        for d in 0..DIFFS {
            for s in 0..SIDES {
                for c in 0..SLOTS {
                    raw.counts[row][d][s][c] = t.next().unwrap_or(0);
                }
            }
        }
    }
    Ok(raw)
}

impl Troops {
    /// The whole of `Troops_Load`: the scan, the siege clamp, the overwrite of
    /// the four dead difficulty columns with Normal, and the re-derivation.
    pub fn load(bytes: &[u8]) -> Result<Troops> {
        let raw = parse_raw(bytes)?;
        let mut out =
            Troops { advantage: raw.advantage, rows: Box::new([[[[0i16; SLOTS]; DIFFS]; SIDES]; ROWS]) };
        for row in 0..ROWS {
            for side in 0..SIDES {
                for diff in 0..DIFFS {
                    for slot in 0..SLOTS {
                        let mut v = raw.counts[row][diff][side][slot];
                        // `00420000.c:4418-4452`, and it runs before the
                        // overwrite, so it bites on every column the file wrote.
                        if slot >= SIEGE_SLOT && v > SIEGE_MAX {
                            v = SIEGE_MAX;
                        }
                        out.rows[row][side][diff][slot] = v;
                    }
                }
                // `00420000.c:4455-4477` — the four other columns become
                // Normal, and `00420000.c:4480-4500` then scales slots 0…6.
                let normal = out.rows[row][side][NORMAL];
                for diff in 0..DIFFS {
                    for slot in 0..SLOTS {
                        let n = normal[slot] as i32;
                        let scaled = if slot < SIEGE_SLOT {
                            // `Pct(x, p)` (`0x00404D6B`) is `x * p / 100`, and
                            // the engine adds or subtracts that delta to Normal
                            // rather than scaling Normal: `n + n*16/100`.
                            let delta = n * (SCALE_PERCENT[diff] - 100).abs() / 100;
                            if SCALE_PERCENT[diff] >= 100 { n + delta } else { n - delta }
                        } else {
                            n
                        };
                        out.rows[row][side][diff][slot] = scaled as i16;
                    }
                }
            }
        }
        Ok(out)
    }

    /// One muster: `g_troopsTable[row][side][difficulty]`, out of range
    /// answering zeroes rather than reading past the array as the original does.
    pub fn counts(&self, row: usize, side: usize, diff: usize) -> [i16; SLOTS] {
        match self.rows.get(row) {
            Some(r) => r[side.min(SIDES - 1)][diff.min(DIFFS - 1)],
            None => [0; SLOTS],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn synthetic(normal_attacker: [i16; SLOTS]) -> Vec<u8> {
        let mut s = String::from("header with no digits\n*Map one\n");
        for row in 0..ROWS {
            s.push_str("05\n");
            for d in 0..DIFFS {
                for side in 0..SIDES {
                    for slot in 0..SLOTS {
                        let v = if row == 0 && d == NORMAL && side == 0 {
                            normal_attacker[slot]
                        } else {
                            0
                        };
                        s.push_str(&format!("{v:03}  "));
                    }
                    s.push('\n');
                }
            }
        }
        s.into_bytes()
    }

    #[test]
    fn the_token_arithmetic_closes() {
        assert_eq!(TOKENS, 35 * 111);
        assert_eq!(TOKENS, 3885);
    }

    #[test]
    fn the_file_order_is_difficulty_before_side_and_the_table_is_the_other_way() {
        let t = Troops::load(&synthetic([250, 75, 0, 0, 0, 0, 0, 0, 0, 0, 0])).unwrap();
        assert_eq!(t.counts(0, 0, NORMAL)[0], 250);
        assert_eq!(t.counts(0, 1, NORMAL)[0], 0);
        assert_eq!(t.advantage[0], 5);
    }

    /// `00420000.c:4481-4499` — 116/108/100/92/84 on slots 0…6, and the
    /// delta is `Pct(Normal, p)`, so 250 goes to 290 and not to 289.
    #[test]
    fn the_four_dead_columns_are_rederived_from_normal() {
        let t = Troops::load(&synthetic([250, 75, 0, 0, 0, 0, 0, 0, 0, 0, 0])).unwrap();
        assert_eq!(t.counts(0, 0, 0)[0], 290);
        assert_eq!(t.counts(0, 0, 1)[0], 270);
        assert_eq!(t.counts(0, 0, 3)[0], 230);
        assert_eq!(t.counts(0, 0, 4)[0], 210);
        // 75 * 8 / 100 == 6, not 6.5 rounded.
        assert_eq!(t.counts(0, 0, 1)[1], 81);
        assert_eq!(t.counts(0, 0, 3)[1], 69);
    }

    /// The siege columns are clamped to nine and then not scaled at all.
    #[test]
    fn the_siege_columns_are_clamped_and_left_flat() {
        let t = Troops::load(&synthetic([0, 0, 0, 0, 0, 0, 0, 40, 3, 0, 0])).unwrap();
        for diff in 0..DIFFS {
            assert_eq!(t.counts(0, 0, diff)[SIEGE_SLOT], SIEGE_MAX);
            assert_eq!(t.counts(0, 0, diff)[SIEGE_SLOT + 1], 3);
        }
    }

    #[test]
    fn which_file_is_which() {
        assert_eq!(file_for(false, true), "TROOPS2.ENG");
        assert_eq!(file_for(false, false), "TROOPS3.ENG");
        assert_eq!(file_for(true, true), "TROOPS.ENG");
    }

    #[test]
    fn a_file_with_no_marker_or_too_few_numbers_is_refused() {
        assert!(matches!(Troops::load(b"no marker here"), Err(Error::TroopsNoMarker)));
        assert!(matches!(
            Troops::load(b"*1 2 3"),
            Err(Error::TroopsTokenCount { found: 3, .. })
        ));
    }
}

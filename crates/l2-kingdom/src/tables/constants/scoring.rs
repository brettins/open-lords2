#![allow(unused_imports)]
use super::*;
use super::demographics::*;
use super::agriculture::*;
use super::castle_and_tax::*;
use super::jobs_and_goods::*;
use super::ai_and_grants::*;
use super::military_and_movement::*;


/// The gold brackets in `Score_RankRealms` (`0x0049AA0E`) — the one term of the
/// score whose meaning is unambiguous. `docs/kingdom.md` §8.3.
///
/// **`[V]`, from the bytes.** It reads as three thresholds paying 50, 100 and
/// 200, and this function used to implement that. It is not what the binary
/// does. `0x0049AED1` onwards, disassembled by hand:
#[inline]
pub fn score_gold_bracket(gold: i32) -> i32 {
    if gold > 2_000 {
        50
    } else {
        0
    }
}

/// The six weights `Score_RankRealms` applies to six realm fields **none of
/// which `docs/kingdom.md` §8.3 could identify**. They are expressed as a
/// numerator and a denominator so that `+0x10 / 10` and `+0x54 / 5` are not
/// silently turned into rounded multiplications.
///
/// `(numerator, denominator)` for realm fields `+0x60, +0x10, +0x0C, +0x58,
/// +0x54, +0x4C` in that order.
pub const SCORE_WEIGHTS: [(i32, i32); 6] = [(10, 1), (1, 10), (2, 1), (2, 1), (1, 5), (50, 1)];

/// `docs/kingdom.md` §8.3 says **none** of the six was identified. Five now
/// are, from `FUN_0049D1E0` — the AI turn's fourteenth step, which recomputes
/// exactly these realm fields once a turn:
///
/// | offset | weight | what `FUN_0049D1E0` writes there |
/// |---|---|---|
/// | `+0x60` | x10 | `PctOf(ownedCounties, g_countyCount)` — **share of the map**, 0..100 |
/// | `+0x10` | /10 | total population over the realm's counties |
/// | `+0x0C` | x2 | mean happiness over the realm's counties |
/// | `+0x58` | x2 | mean health meter over the realm's counties |
/// | `+0x54` | /5 | total men over the realm's armies |
/// | `+0x4C` | x50 | **castles held** — see below |
///
/// All six are identified, and the score reads as *castles above everything
/// else, then territory, then people, then how well they are doing, then the
/// army*. `[V]`
///
/// `+0x4C` was the last, and was found from the other end. A player described
/// the standings screen — swords and flags at differing heights, each with a
/// voice-over — and `L2.eng` group 35 is exactly that list: *Most counties,
/// Most castles, Most troops, Most crowns, Happiest people, Most people,
/// Greatest noble, undecided.* Five already had an input; **Most castles** did
/// not, and `+0x4C` had no name.
///
/// `0x0053FB70 - 0x0053F9B0 = 0x1C0`, which is `castleType`. So it counts the
/// realm's counties holding a castle, and it carries **more weight than the
/// other five combined** — a real statement about what this game thinks winning
/// is.
pub const SCORE_INPUT_OFFSETS: [u16; 6] = [0x60, 0x10, 0x0C, 0x58, 0x54, 0x4C];

/// The slot of [`SCORE_INPUT_OFFSETS`] that holds `+0x4C`, the castle count.
///
/// **It is the one slot [`crate::realm::Realm::sync_score_inputs`] must not
/// touch**, because in the original it is not one of `Realm_UpdateTotals`'
/// writes: `Castle_BuildTick` (`0x004508DE`) owns it, alone. Verified
/// exhaustively — every instruction in `Lords2.exe` whose
/// operand mentions `g_realms + 0x4C` is one of seven, and they are
/// `Castle_BuildTick` twice (`0x0045090E` clears, `0x00450CB1` increments),
/// `Game_SetupRealmsAndCounties` once (`0x0049C14C`, the initial clear),
/// `Score_RankRealms` three times and one painter.
pub const SCORE_INPUT_CASTLES: usize = 5;

pub const SCORE_INPUT_NAMES: [&str; 6] = [
    "share of the map, percent",
    "total population",
    "mean county happiness",
    "mean county health",
    "total men under arms",
    "castles held",
];

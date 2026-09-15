#![allow(unused_imports)]
use super::*;

use codec::*;
use domain::*;
use crate::county::{ChangeReason, County, Industry, MAX_COUNTIES, MAX_INFLOW_SOURCES, MAX_NEIGHBOURS};
use crate::kingdom::{History, HistoryEntry, Kingdom, Options};
use crate::phase::{Phase, TurnMachine};
use crate::realm::{Realm, MAX_REALMS};
use crate::tables::{
    Tables, Weather, HISTORY_COUNTIES, HISTORY_SEASONS, JOB_COUNT, WEAPON_TYPE_COUNT,
};
use l2_net::canonical::{Canonical, CodecError, Decode, Encode, Reader};

pub const MAGIC: [u8; 8] = *b"L2KSAVE\x01";

/// * 7 — **the grain year got a memory**: `fields_grain_sown` (county `+0x202`)
///   and `sow_shortfall` (`+0x1A7`). `Grain_Grow` and `Grain_Harvest` scale the
///   standing crop by the grain fields still standing *against the fields that
/// were sown*,
///   half-grown crop with no basis to measure it against. A version 5 save has
///   both missing, and defaulting `fields_grain_sown` to 0 would silently mean
///   "no fields were sown" — it is a different game.
///
/// * 9 — **the AI's farming style and its weapon rota**: county `farm_style`
///   (`+0x1FE`) and realm `weapon_rota` (`+0x6C`). Neither is derivable from the
///   rest of the file. An *unowned* county's style is whichever lord held it
/// last and `AI_ManageFields(0)` only reads it,
/// say how a county that has changed hands should be farmed; and AI step 12
/// advances the rota cursor once per county,
///   restarts every AI's weapon programme from the top. See
///   [`crate::ai_farm`].
///
///   It cannot stop two branches picking the same number, because neither can
///   see the other. What it *can* do is fail the instant they are merged —
///   which is where all three of these were caught by hand, twice by an
///   integrator reading a doc comment. A merge that takes one branch's `VERSION`
///   and both branches' changelog entries now goes red
///   number that means two different layouts.
///
/// * 10 — **sieges** (`crate::siege`). The unit grew the three siege-engine
/// build records and the seasons countdown; `County::castle_degraded` grew
/// from a `bool` to the three-valued byte it always was
///   `castle_ruined` and `castle_level_left` beside it. `Unit::defence_mark`
///   joins them: it existed before and was in neither the save nor the
///   checksum, which is C30's shape exactly, and it is the byte that decides
///   whether winning a battle also wins the county. An older save has no siege
///   in it, but it also cannot say what its `castle_degraded` bytes meant — a
/// `true` could be either 1 or 2 and the two fight different castles — so
///   this is a refusal.
///
///   **Three branches found `defence_mark` missing, independently, within a
///   day.** The sharpest statement of it is the unit-import branch's: entry 8
///   added `cargo_county`, the *other* meaning of the same `+0x167` byte, and
///   left the field beside it unwritten. Two meanings sharing one offset, one
///   encoded and one not, added in the same neighbourhood by different hands.
///
/// **C30 for the third time,
///   it**: `every_field_of_the_state_is_furnished` in `tests/save.rs` derives
///   the field list from the struct definitions, and these twelve were its
///   first run's output. `tests/save_gap.rs`, which pinned eleven of them, is
///   deleted with them — the gap it described is closed.
///
/// * 12 — **`Options::exploration` and `Options::time_limit`**, the last two of
///   the six *rule* options the setup screen sets. Both are globals the
///   original's own `Save_Write` stores (`g_optExploration` `0x0053F264`,
///   `g_optTimeLimit` `0x0053F26C`) and both were being read out of a `.sav` by
///   `l2_formats::save::Globals` and then dropped on the floor, because
///   `Options` had nowhere to put them. Neither is read by a rule in this
///   crate — exploration's behaviour is unimplemented (`docs/mechanics.md`) and
/// a wall clock is not a rule — but a game that was started with a four
///   minute turn limit and no fog is a different game from one that was not,
///   and a save that cannot say which is a save that guesses.
///
/// * 13 — **the merchant's books**: `County::purse` and the four `Realm` trade
///   accumulators (`trade_spent_a`/`_b`, `trade_received_a`/`_b`). All five are
///   written by [`crate::trade::trade`] and by nothing else, and until the
///   merchant screen existed no code path could make any of them non-zero — so
/// they were absent from the record, the save and the lockstep digest at once,
///   which is C30's shape for the fifth time. A trade produces them now, and an
///   unowned county's purse in particular is *simulation* state: the AI trades
///   out of it for every county nobody owns, so two peers that disagreed about a
///   purse would deal different goods and every checksum they exchanged would
///   still agree.
///
/// * 14 — **the castle's build record**: `County::castle_percent`,
///   `castle_work_left`, `castle_work_total`, `castle_stone_owed`,
///   `castle_stone_total`, `castle_wood_owed` and `castle_wood_total`, in place
///   of the single `castle_progress` counter this crate invented. County
/// `+0x1C4` and `+0x1CC … +0x1E0` in the original
///   a version is that **the model changed with them**:
///
///   **And a third branch, same day, same version:** **`Options::quirks`**, the bitfield saying which of the original's
///   defects this game reproduces (`docs/bugs.md`, `docs/decisions.md` C62).
///
/// * 15 — **`County::siege_scars`**, the six values county `+0x1E4` … `+0x1F1`
///   holds between two assaults on the same castle: the moat cells filled in,
///   the wall damage, the two progress scores, the ramparts down and whether
///   the gate is open. [`crate::siege::record_castle_damage`] writes them at
///   the end of a fought siege and [`crate::siege::scars_for_assault`] hands
///   them back to the next one.
///
/// * 16 — **the grain row's three forecasts**, [`crate::county::County`]'s
///   `grain_sown_expected`, `grain_grown_expected` and `grain_change_expected`,
///   which are `Grain_LabourEstimate`'s tail (`0x0044D374`). Twelve bytes a
/// county over 17 slots — **and the reclamation row's two**,
///   `reclaim_fields_finishing` and `reclaim_seasons_to_next` (county `+0x20C`
///   and `+0x214`), which are `Field_ReclaimEstimate`'s tail (`0x0044C278`).
///
///   A player: *"Sidebar doesn't show grain being planted as a negative
///   number."* These are the numbers that say so — in Spring the change is
///   `−sown − eaten` — and until this version nothing in the workspace computed
///   any of them. `docs/decisions.md` C123.
///
/// * 17 — **the sub-tile counter**: [`crate::unit::Unit`]'s `sub_tile`,
///   `sub_frame` and `at_tile_edge`, which are `Unit_StepOnce`'s `+0x149`,
///   `+0x14A` and `+0x14B` bit 0. Three bytes a unit over 150 slots, +450.
///
///   They are how far across its current tile a walking unit is, and until
///   this version nothing in the workspace had them: a unit entered a tile on
/// **every** tick instead of every eighth (road) or thirty-second (open
///   ground). A player: *"the merchants don't move right when you click End
///   Turn, and then… move insanely fast."* `docs/decisions.md` **C134**.
///
/// * 18 — **the industry rows' forecast**,
///   [`crate::county::Industry::next_season`], which is
///   `Industry_LabourEstimate`'s tail (`0x0044F318`). Four bytes on each of
///   four records a county, over 17 slots: **+272**.
///
///   Entry 16's twin, found the same way and one function along: a search loop
///   we ported and a tail we did not. The five sidebar industry rows stood
///   behind a box of ours reading `INDUSTRY / NOT DRAWN` on the reading that
///   *"three of them are flat icons"* — they are, and each still draws a
///   `Ui_DrawDelta` of this number, which `L2.eng` group 220 calls *"Wood
///   produced next season"*.
///
///   Carried for entry 16's own reason and no stronger
/// one: nothing re-runs the estimate round on load,
///   blank row a player would see. `docs/decisions.md`
///   C136.
///
/// * 19 — **the county's merchant stall**: [`crate::county::County::merchant_count`]
///   (`+0x1A4`), `merchant_unit` (`+0x1A5`) and `merchant_visits` (`+0x1A0`),
///   nine bytes a county over 17 slots: **+153**.
///
///   `County_RecountMerchants` (`0x00451061`) writes all three once a season
///   and [`crate::phase::Pass::CountyRecountMerchants`] is the pass. Carried
///   for the reason that made this whole change
///   necessary: **the first thing a loaded game does is turn phase 1**, which
///   runs the neutral counties' farming pass, which asks `merchant_count`
/// whether the county may buy food — and the recount does not run again
///   until the *end* of that turn. A defaulted stall is a season of unowned
///   counties that cannot shop, which is exactly the defect being fixed.
///
///   `docs/decisions.md` C149.
///
/// * 20 — **what the weather and the random events did to the grain and the
///   herd last season**: [`crate::county::County`]'s `grain_weather_change`
///   (`+0x24C`), `grain_event_change` (`+0x278`), `herd_weather_change`
///   (`+0x270`) and `herd_event_change` (`+0x274`). Sixteen bytes a county over
///   17 slots: **+272**.
///
/// `Grain_SeasonTick` and `Herd_SeasonTick` store all four and the grain and
///   cattle panels print them under `L2.eng` group 77 — *"eaten by rats."*,
///   *"taken by wolves."*, *"gained last season, due to weather."* — and nothing
///   in the workspace had them: `docs/stored-fields.json` carried them as
///   excluded, *"not a field of County"*, which was true and was the gap.
///
/// * 21 — **realm `+0xF4` and `+0xF8`**, [`crate::realm::Realm::tax_ledger`]:
///
///   `Tax_CollectAll`'s (`0x0044B59B`) third accumulator pair, credited with
///   every owned county's take beside the treasury. Eight bytes a realm over six
///   slots: **+48**. No rule in the original reads either — the realm block is
///   stored by `Save_Write` and compared by `Sync_CompareState`, and that is
///   the whole of their use — so they are carried for entry 13's reason about
///   the trade pair: state the original keeps is state two peers must agree on.
///
///   [`crate::county::County::event_population_swing`], county `+0x2F8`, four
///   bytes a county over 17 slots: **+68**.
///
///   `Population_UpdateAll` (`0x00449EF3`) writes it in every county every
///   season, and `Msg_DrawWindow` prints it in *Plague* and *Wedding fever*'s
///   letters before *"extra deaths."* / *"extra births."* It is the one output
///   of that pass nothing else re-derives: the percentage it came from is
/// cleared inside the same season,
///   Carried for entry 16's reason — it feeds no
///   rule. `docs/decisions.md` C169.
///
///   `Options::exploration` has been carried since entry 12 and **read by
///   nothing**: the switch was honest, the fog did not exist. It exists now,
/// and the plane it draws is not derivable from anything else in this file —
///   it is the record of every tile a realm's armies have stood within six of,
/// every county it has held and the one it started in. `docs/decisions.md`
///   C172.
///
/// * 24 — **realm `+0x2A`**, [`crate::realm::Realm::peak_counties`]: the most
///   counties a realm has ever held. One byte a realm over six slots: **+6**.
///
///   `County_ChangeOwner` (`0x004A72FE`) reads it to choose which of its capture
///   letters the local player is sent and then raises it; nothing else reads
///   it. Refused: a defaulted peak of 0 sends *"Bravo!!
///
/// * 25 — **the sown grain fields still standing**,
///   [`crate::county::County::fields_grain_standing`] (`+0x206`): four bytes a
///   county over 17 slots, **+68**.
///
///   The divisor of the wheat picture. `Grain_SeasonTick` bands the crop by
///   this byte to choose which of four frames every grain tile draws, and
///   `County_DestroyField` steps it down; it is not `+0x202` and is not
///   derivable from it. `docs/decisions.md` C195.
///
/// * 26 — **the industry ramp's own input**,
///   [`crate::county::Industry::last_efficiency`] (county `+0x29C`): four bytes
///   a commodity over four commodities over 17 slots, **+272**.
///
///   `Industry_EfficiencyRamp` (`0x0044F248`) ramps from `+0x29C`, not from the
///   `+0x294` production multiplies by, and only `Industry_Produce`
///   (`0x0044EA92`) copies one to the other. That is what lets
///   `Industry_LabourEstimate` write `+0x294` on every refresh without
///   compounding, which is the ramp C136 left unported.
///
///   **Refusal.** With *Advanced Farming* on the two bytes differ from the
///   first labour move of the season, and a defaulted `+0x29C` would either
///   restart a mine's ramp at its base or advance it by a refresh.
///
/// * 27 — **the two field cursors**, [`crate::county::County::pasture_cursor`]
///   (`+0x15A`) and [`crate::county::County::blight_cursor`] (`+0x15B`): one
///   byte each a county over 17 slots, **+34**.
///
///   `Map_ResolvePick` (`0x0046D5FE`) sets `DAT_005651BC` on
///   `(tile.bank & 0x1C) == 4`, and that global is the *whole* of
///   `TileInfo_Draw`'s (`0x0041C208`) mountain-or-woodland test on a `0x08`
///   rough tile. Nothing else in the record answers it.
///
/// * 30 — **the ration shadow pair**,
///   [`crate::county::County::grain_eaten_shadow`] (`+0x18C`) and
///   [`crate::county::County::herd_eaten_shadow`] (`+0x190`): four bytes each a
///   county over 17 slots, **+136**.
///
///   `Ration_Apply` (`0x0044DF5F`) does not debit a store; `Ration_ApplyAll`
/// (`0x0044BF04`) copies `+0x178`/`+0x17C` into this pair, and the debit is
/// the opening line of `Grain_SeasonTick` (`0x0044C8AE`) and the third of
///   `Herd_SeasonTick` (`0x0044D60D`), eleven and twelve passes later.
///
///   A save taken between `RationApply` and `GrainSeasonTick` — every autosave
///   inside a season advance — would reload with the season's food never paid
/// for, and the pair is not derivable from `+0x178`/`+0x17C`, which
///   [`crate::ration::preview`] has by then overwritten with next season's
///   forecast (`docs/decisions.md` C20).
pub const VERSION: u32 = 30;

pub const HEADER_LEN: usize = 8 + 4 + 8 + 4;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoadError {
    NotASave,
    UnsupportedVersion { found: u32, supported: u32 },
    RulesetMismatch { save: u64, supplied: u64 },
    TruncatedBody { declared: usize, actual: usize },
    Corrupt { expected: u64, actual: u64 },
    Malformed(CodecError),
    CorruptHistory { head: usize, tail: usize, len: usize },
    CountyCount(u32),
    UnitCount(u32),
    MapSize(u32),
    BandCount(u32),
}

impl core::fmt::Display for LoadError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            LoadError::NotASave => write!(f, "not a lords2 kingdom save"),
            LoadError::UnsupportedVersion { found, supported } => write!(
                f,
                "save format version {found}; this build reads {supported}. \
                 Refusing rather than guessing at the layout"
            ),
            LoadError::RulesetMismatch { save, supplied } => write!(
                f,
                "the save was written under ruleset {save:#018x} and was handed \
                 {supplied:#018x}; load the mod set it was made with"
            ),
            LoadError::TruncatedBody { declared, actual } => {
                write!(f, "the body declares {declared} bytes and {actual} follow")
            }
            LoadError::Corrupt { expected, actual } => {
                write!(f, "checksum {actual:#018x}, expected {expected:#018x}")
            }
            LoadError::Malformed(e) => write!(f, "{e}"),
            LoadError::CorruptHistory { head, tail, len } => {
                write!(f, "history ring head {head}, tail {tail}, len {len}")
            }
            LoadError::CountyCount(n) => write!(f, "{n} counties, and the array holds 16"),
            LoadError::UnitCount(n) => {
                write!(f, "{n} unit slots, and the array holds {}", crate::unit::MAX_UNITS)
            }
            LoadError::MapSize(n) => {
                write!(f, "a map plane of {n} tiles, and the map is {}", crate::map::MAP_TILES)
            }
            LoadError::BandCount(n) => write!(
                f,
                "{n} mercenary bands in play, and there are {}",
                crate::mercenary::MERCENARY_BANDS
            ),
        }
    }
}

impl std::error::Error for LoadError {}

impl From<CodecError> for LoadError {
    fn from(e: CodecError) -> LoadError {
        LoadError::Malformed(e)
    }
}



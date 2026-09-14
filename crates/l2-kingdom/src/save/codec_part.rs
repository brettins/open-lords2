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

/// Eight bytes, so a file command can name the format and a truncated file
/// fails on the magic.
pub const MAGIC: [u8; 8] = *b"L2KSAVE\x01";

/// The format version. **Bump it whenever the byte layout below changes**, and
/// never reinterpret an unknown one.
///
/// * 1 — the first layout.
/// * 2 — the county grew four herd fields (`docs/kingdom.md` §13)
/// ruleset fingerprint grew the tax-happiness table, the herd table and the
/// cattle-farming job slot. Both halves of the file moved,
///   save is refused.
/// * 3 — the **campaign layer** (`docs/armies.md`): the 151-slot unit array,
/// the map's three tile planes, the twelve mercenary bands and the realms'
///   army-name counters, plus three new county fields. A version 2 save has no
///   armies in it and no way to say so.
/// * 4 — **fields became tile-derived** (`crate::field`): the county carries
/// its twenty field tiles and the two counts `County_RecountFields` fills
///   besides the three we already had. A version 3 save records the counts but
///   not the tiles they were counted from, so its fields could never be
/// repainted;
///   why this is a refusal and not a default.
/// * 5 — **four county fields that were never written at all**:
///   `labour_wanted`, `labour_useful`, `labour_share` and `industry_share`.
///   Found by the *game* save's round trip over the England turn-one position
///   (`crates/l2-game/tests/save/main.rs`): a decoded kingdom compared equal on its
///   checksum and unequal on `PartialEq`, because the four were absent from the
///   `Encode` impl below and therefore absent from the hash as well. Two of
///   them — `labour_useful` and `labour_share` — are what the labour allocator
///   allocates *from*, so this was a hole in the lockstep checksum
///   (`docs/netcode.md` §5) and not only in the save. A version 4 save has the
///   four missing and no way to say what they held.
/// * 6 — `Options::fight_humans_only_byte`. It was a parameter threaded through
/// `l2-game`'s `engagement::resolve` while the save and the battle seam were
///   being written in parallel branches; now that both have landed it is where
///   it belongs, in the kingdom's own options, and therefore in the save and in
/// the lockstep checksum. A version 5 save does not carry it
///   changes whether a battle is fought or auto-resolved — so this is a refusal
///   on the same grounds as version 5.
/// * 7 — **the grain year got a memory**: `fields_grain_sown` (county `+0x202`)
///   and `sow_shortfall` (`+0x1A7`). `Grain_Grow` and `Grain_Harvest` scale the
///   standing crop by the grain fields still standing *against the fields that
/// were sown*,
///   half-grown crop with no basis to measure it against. A version 5 save has
///   both missing, and defaulting `fields_grain_sown` to 0 would silently mean
///   "no fields were sown" — it is a different game.
///
///   **This arrived as its own version 6.** Two branches bumped 5 → 6 in
/// parallel, each adding different fields,
///   of their version 6s — so it is 7. A version 6 save is a real thing that
///   exists (it carries `fight_humans_only_byte` and *not* these two), which is
///   so the number had to move
///   together.
/// * 8 — **the things that move now move** (`crate::units_tick`): the six
///   merchant trade routes `g_merchantRoutes`, the peasant mobs' shared
///   destination cursor, and a transport's cargo county on the unit record.
///   All three are state a turn *reads and writes* — a version 7 save would
///   load with six empty routes and every merchant would stand still for ever,
/// which is a silently different game
///
/// **And it happened again,
///   arrived as its own version 7 from a third parallel branch, was read off
///   the changelog on merge, and moved to 8. That is now twice in one day, so
/// the note above should be taken as a standing hazard
///   anecdote: **the version number is the one field in this file that two
///   branches will always collide on**, because every branch that changes the
///   layout has to touch it and none of them can see the others. A check like
///   `tools/decisions/corrections.js` — which catches exactly this for
///   correction numbers — is the fix, and it does not exist for this constant.
/// * 9 — **the AI's farming style and its weapon rota**: county `farm_style`
///   (`+0x1FE`) and realm `weapon_rota` (`+0x6C`). Neither is derivable from the
///   rest of the file. An *unowned* county's style is whichever lord held it
/// last and `AI_ManageFields(0)` only reads it,
/// say how a county that has changed hands should be farmed; and AI step 12
/// advances the rota cursor once per county,
///   restarts every AI's weapon programme from the top. See
///   [`crate::ai_farm`].
///
///   **Three times in one day, and this one is the third.** This arrived as its
///   own version 5, was moved to 7 on one merge and to 9 on the next, both
///   times by reading this changelog. The check the two entries above ask for
///   now exists: `the_version_is_ahead_of_its_own_changelog` in
///   `tests/save.rs` reads this file, scans the comment for its `* N —`
///   entries, and fails unless [`VERSION`] is greater than every one of them
/// and the numbering has no gap and no repeat.
///
///   It cannot stop two branches picking the same number, because neither can
///   see the other. What it *can* do is fail the instant they are merged —
///   which is where all three of these were caught by hand, twice by an
///   integrator reading a doc comment. A merge that takes one branch's `VERSION`
///   and both branches' changelog entries now goes red
///   number that means two different layouts.
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
///   It was invisible for version 5's reason: every unit that had ever
///   existed was one a test built by hand, and a hand-built unit carries 0 in
///   it. It surfaced the moment `l2-scenario` began importing `g_units` from
///   a save — `battle-during.sav` slot 6 carries a 1 — and it surfaced the
///   same way version 5's four did: a round trip equal on the checksum and
///   unequal on `PartialEq`.
///
///   **And a fourth collision, on the same day as the other three.** This
///   arrived as its own version 7, was rebased to 9, and is 10 here because the
///   entry above took 9 first. The standing hazard above is now the rule rather
///   than the exception: assume the number has moved under you. This is the
///   first collision the check above would have caught on its own — a merge
///   taking one branch's `VERSION` and both branches' entries leaves a repeat,
/// and the repeat is what it tests for.
/// * 11 — **the diplomacy record**: `Realm`'s
///   `offer_pending`, `ally_candidate`, `ally`, the six-slot `pairs` block,
///   `target_county`, `taunt_timer`, `taunt_stage`, `war_target`,
///   `offer_timer`, `crowned_once` and `voice_rotation`. (The twelfth field its
///   census found, `Unit::defence_mark`, went in with the sieges at entry 10 —
///   the two branches found it independently and within a day of each other.)
///   Every one of them is simulation state — `pairs` is what
/// `docs/diplomacy.md` §1 *is*: standing, alliance, grudge, at-war and the
///   gift history between every pair of realms — and none of them reached
///   these bytes, so none of them reached the lockstep digest either. Two peers
///   could diverge on the whole diplomatic state of a game and every checksum
///   they exchanged would agree.
///
/// **C30 for the third time,
///   it**: `every_field_of_the_state_is_furnished` in `tests/save.rs` derives
///   the field list from the struct definitions, and these twelve were its
///   first run's output. `tests/save_gap.rs`, which pinned eleven of them, is
///   deleted with them — the gap it described is closed.
///
///   A version 10 save has all eleven missing. **Refusal**,
///   and `pairs` is why: a defaulted pair block is not "no diplomacy", it is
///   every alliance broken, every grudge forgotten and every standing reset to
///   the same number, which is a different game silently resumed.
///
///   **And this one is the fourth collision, caught by the check
///   an integrator.** It was written as version 8, then 9, and
///   `the_version_is_ahead_of_its_own_changelog` — added by the branch above,
///   independently and for the same reason — failed the merge both times and
///   named the duplicate.
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
///   **Refusal.** A version 11 save carries neither, and
///   defaulting both to off would silently claim a setting the file never made.
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
///   **Refusal**, on the same reasoning as entry 12: a
///   version 12 save was written by a build that could not trade, so all five
///   really are zero in it — but a save is not the place to be right by
/// accident
///   default nobody checked.
///
///   *This entry was written as 13 with `VERSION` at 12 on `main`. Per the
///   standing hazard above, assume the number has moved: a merge that finds 13
/// taken renumbers this entry and the constant together.*
/// * 14 — **the castle's build record**: `County::castle_percent`,
///   `castle_work_left`, `castle_work_total`, `castle_stone_owed`,
///   `castle_stone_total`, `castle_wood_owed` and `castle_wood_total`, in place
///   of the single `castle_progress` counter this crate invented. County
/// `+0x1C4` and `+0x1CC … +0x1E0` in the original
///   a version is that **the model changed with them**:
///   the materials are drawn down season by season,
///   the work counts down, and `castle_building` holds the
///   castle you *had*
///   (`County::castle_building`). A version 13 save's `castle_progress` cannot
///   be translated into any of them — the same number means "work done" there
///   and nothing here — and its `castle_building` byte means the opposite of
///   what this build would read.
///
///   **Refusal**, and this one is not conservatism:
///   `castle_degraded` had **no reachable writer** before this version, so
///   every version 13 save in existence has it zero in every county and a
///   castle nobody could have started. There is nothing to preserve.
///
///   **Two branches bumped to 14 on the same day with different contents, and
///   they are one version.** Neither had shipped, so no
///   save exists that has one set of fields and not the other; numbering them
///   separately would invent a save nobody can hold and a translation nobody
///   can test. Both are refusals in any case.
///
///   **And, from a second branch that bumped to the same version on the same's war plan** ([`crate::ai_army`]): `Unit`'s `mission` and
///   day, the AI
///   `mission_county`, and `Realm`'s `muster_county`, `raid_county`,
/// `muster_timer`, `threat_realm`, `attack_county`, `raid_timer` and the
///   four-slot `want`.
///
///   Every one of them is a *standing order that persists between turns*, which
///   is exactly what a save is for. `mission` is the sharpest: it is the byte
/// AI step 11 dispatches on,
///   army as an attacker — **including the garrisons**, which would then
///   discover their county was still theirs, do nothing, and be marched out of
///   their castles the first time anything took one. The realm fields are
///   softer and no less real: a reloaded game would restart every lord's muster
///   and raid counters at zero, forget which county each realm musters from,
///   and forget the county its main army is currently marching on.
///
///   **Refusal**, on the same reasoning as entries 12 and
///   13: a version 13 save was written by a build whose AI raised no armies at
///   all, so the fields really are zero in it — but zero is a *meaningful*
///   value for `mission` (it is the one the dispatcher normalises), and a save
///   is not the place to be right by accident.
///
///   *Written as 14 with `VERSION` at 13 on `main`. Per the standing hazard
///   above, assume the number has moved.*
///
///   **And a third branch, same day, same version:** **`Options::quirks`**, the bitfield saying which of the original's
///   defects this game reproduces (`docs/bugs.md`, `docs/decisions.md` C62).
///   It changes what the simulation computes, so it is state: a save that did
///   not carry it would resume a fixed game as a faithful one, and a lockstep
///   peer that never exchanged it would desync.
///
///   **This is the only version bump the quirk set will ever cost**, which is
///   why the field is a `u64` bitfield, and why a
///   set bit means *fixed*: adding a quirk next month
///   sets a bit that is already being written as zero, and zero already means
///   the original's behaviour. `docs/bugs.md` §6.3 asked that the bump be paid
///   once; this is how it is paid once.
///
///   **Refusal**, as for 12 and 13 — and here the default
///   would even have been right, since a version 13 save was written by a build
///   that had no quirks and so was faithful. It is refused anyway, because *"the
///   default happens to be correct this time"* is the reasoning that makes the
///   next widening wrong.
///
/// * 15 — **`County::siege_scars`**, the six values county `+0x1E4` … `+0x1F1`
///   holds between two assaults on the same castle: the moat cells filled in,
///   the wall damage, the two progress scores, the ramparts down and whether
///   the gate is open. [`crate::siege::record_castle_damage`] writes them at
///   the end of a fought siege and [`crate::siege::scars_for_assault`] hands
///   them back to the next one.
///
///   **They are state, not a report.** A besieger thrown off a half-wrecked
///   castle comes back to a half-wrecked castle, and a save that dropped them
///   would quietly rebuild the walls over a load. Two of the six also carry the
/// repair the county is *already* paying for,
///   with them defaulted would show a castle mid-repair with no reason for it.
///
///   **Refusal**, on the standing reasoning — and here the
///   default would have been right for a different reason worth writing down:
///   a version 14 save was written by a build in which the accumulators had no
///   writer at all, so the scars really were zero. It is refused anyway,
///   because a version 14 save could also have `castle_degraded == 2` from
///   nothing, which is a state this version cannot produce.
///
///   *Written as 15 with `VERSION` at 14 on `main`. Per the standing hazard
///   above, assume the number has moved.*
///
///   **And, from a second branch that took the same version the same evening, **the diplomatic inbox** ([`crate::diplomacy`]): six realms of five
///   `g_diploInbox` slots, the outstanding pay-for-help price and county, and
///   the generator the three bargaining replies roll.
///
///   The eleven fields entry 11 added are a realm's *opinions*; these are the
///   letters in flight. `Diplo_Post` moves a gift's gold the moment it is
/// posted and the reply arrives a turn later,
///   that dropped the slot would take the money and never answer — and a
///   pay-for-help prompt reloaded without its price would ask for nothing.
///
///   **Refusal**, and here for a reason none of the earlier
/// entries had: the pair block *is* carried by a version 14 save,
///   defaulted inbox would produce a kingdom that looks entirely coherent — the
///   standings, alliances and grudges all present and correct — with the mail
///   silently thrown away. A wrong load that looks right is the one to refuse.
/// * 16 — **the grain row's three forecasts**, [`crate::county::County`]'s
///   `grain_sown_expected`, `grain_grown_expected` and `grain_change_expected`,
///   which are `Grain_LabourEstimate`'s tail (`0x0044D374`). Twelve bytes a
/// county over 17 slots — **and the reclamation row's two**,
///   `reclaim_fields_finishing` and `reclaim_seasons_to_next` (county `+0x20C`
///   and `+0x214`), which are `Field_ReclaimEstimate`'s tail (`0x0044C278`).
///   Twenty bytes a county over 17 slots: **+340**.
///
///   A player: *"Sidebar doesn't show grain being planted as a negative
///   number."* These are the numbers that say so — in Spring the change is
///   `−sown − eaten` — and until this version nothing in the workspace computed
///   any of them. `docs/decisions.md` C123.
///
///   **This entry is the first one where the default would have been harmless,
///   and it is refused anyway — but for a weaker reason than the others, and
///   the difference is worth stating.** Entries 12 … 15
///   refuse because a defaulted load produces *a state this version cannot
///   otherwise produce*, or one that changes what the simulation computes. These
///   three do neither: they are **derived**, recomputed from scratch by every
///   estimate round, and nothing anywhere reads them except the painter. A
///   version 15 save loaded with them zeroed would show one blank produce row
///   until the turn ended and then be correct for ever.
///
///   They are carried anyway for one reason: **nothing re-runs the estimate
///   round on load**, so "until the turn ended" is a real interval a player
///   would see, and a blank row is the exact defect they exist to fix. The
///   format has no widening mechanism — `decode` is `if version != VERSION
///   { Err }` and always has been — so carrying them *is* a bump, and there was
///   never a third option to weigh.
///
///   The rule this sharpens: **refuse when a default is a
///   state the writer could not have produced, or when it feeds the
///   simulation.** A derived display field is neither, and if the format ever
///   grows widening, this is the entry that should take it.
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
///   **Refusal, and this one is squarely inside the rule
///   entry 16 sharpened**: a defaulted load feeds the simulation directly. A
///   version 16 save taken mid-turn has units part-way across tiles; zeroing
///   the counter hands each of them up to thirty-one ticks of free progress or
///   takes it away, which moves *which tick* an army arrives on, which moves
/// which of two armies reaches a castle first. It is not a display field and
///   it is not derived.
///
///   *Written as 17 with `VERSION` at 16 on `main`. Per the standing hazard
///   above, assume the number has moved.*
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
///   **The fifth collision, and caught by a reader again — git put the two
/// entries in one conflict hunk and the integrator read them.** The sub-tile
///   counter above arrived as its own 17 on one branch and this arrived as its
///   own 17 on another. Both entries are kept and this one is renumbered,
///   which is what the standing hazard says to do.
///   `the_version_is_ahead_of_its_own_changelog` is the backstop for the case
///   that is *not* a conflict: two branches touching different parts of this
///   comment merge clean and leave the repeat, and that is the shape the check
///   exists for. It has still never been the thing that caught one.
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
///   `docs/decisions.md` C149.
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
///   Refused on entry 16's weaker ground: they are display
///   figures nothing reads back, but nothing recomputes them on load either, so
///   a defaulted load prints *"no effect"* for a whole season in which the
///   original prints a number.
///
///   *Written as 20 with `VERSION` at 19 on `main`. Per the standing hazard
///   above, assume the number has moved.*
///
/// * 21 — **realm `+0xF4` and `+0xF8`**, [`crate::realm::Realm::tax_ledger`]:
///   `Tax_CollectAll`'s (`0x0044B59B`) third accumulator pair, credited with
///   every owned county's take beside the treasury. Eight bytes a realm over six
///   slots: **+48**. No rule in the original reads either — the realm block is
///   stored by `Save_Write` and compared by `Sync_CompareState`, and that is
///   the whole of their use — so they are carried for entry 13's reason about
///   the trade pair: state the original keeps is state two peers must agree on.
///
///   **And a pass index moved under the same number.**
///   [`crate::phase::Pass::AiManageFarms`] is `Season_Advance`'s first call and
///   went in at position 0 of `SEASON_PIPELINE`, which shifts every index
///   `l2_game::save` writes a season report's pass list as. `l2_game::save`
/// decodes this body **before** its own prefix,
///   refused by this check before a shifted index is ever read.
///
///   **Refusal**: a version 19 save was written by a build
///   that never credited the pair, so its zeros are a state that build produced
///   and this one cannot — any realm that has collected a season's tax holds a
///   non-zero ledger here.
///
///   *Written as 20 with `VERSION` at 19 on `main`, and with another queued
///   branch known to be taking 20. Per the standing hazard above, assume the
/// number has moved: a merge that finds 20 taken renumbers this entry and the
///   constant together.* It merged as 21, after the mercenaries' 20.
///
/// * 22 — **the population event's swing**:
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
///   *Written as 20 with `VERSION` at 19 on `main`, while another queued branch
///   was also taking 20. Per the standing hazard above, assume the number has
/// moved.* It merged as 22, after the mercenaries' 20 and the tax ledger's 21.
///
/// * 23 — **the fog of war's seen plane**, [`crate::explore::Explored`]: one
///   byte a tile, bit `r` for realm `r`, **+4,096**. The original keeps it as
///   tile record `+2` bit `0x20` and `Save_Write` stores it with the rest of
///   `g_tiles`, which is the first block of every `.sav`.
///
///   `Options::exploration` has been carried since entry 12 and **read by
///   nothing**: the switch was honest, the fog did not exist. It exists now,
/// and the plane it draws is not derivable from anything else in this file —
///   it is the record of every tile a realm's armies have stood within six of,
/// every county it has held and the one it started in. `docs/decisions.md`
///   C172.
///
/// **Refusal**
///   wrong: a version 19 save loaded with no tile seen and
///   the option on is a player's own county blacked out. A default of *every*
///   tile seen is the other wrong answer — a fog that lifted over a load.
///
///   *Written as 20 with `VERSION` at 19 on `main` — and a queued branch was
///   already known to take 20. Per the standing hazard above, this entry is the
///   one that renumbers, to 21, at whichever merge comes second.* It merged as 23, after
///   entries 20, 21 and 22.
///
/// * 24 — **realm `+0x2A`**, [`crate::realm::Realm::peak_counties`]: the most
///   counties a realm has ever held. One byte a realm over six slots: **+6**.
///   `County_ChangeOwner` (`0x004A72FE`) reads it to choose which of its capture
///   letters the local player is sent and then raises it; nothing else reads
///   it. Refused: a defaulted peak of 0 sends *"Bravo!!
///   … an excellent start"* for a county won back in the fortieth season.
///
///   *Written as 22 with `VERSION` at 21 on `main`. Per the standing hazard
///   above, assume the number has moved.* It merged as 24, after entries 22 and 23.
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
///   **Refusal**, under entry 16's rule: a defaulted load
///   feeds the simulation. A zero here makes the next trampled grain field
///   take no crop and repaint nothing different, and makes every grain tile of
///   the county draw the bare-crop picture until the next sowing.
///
///   *Written as 21 on its own branch, behind four entries that had all queued
///   for 20 or 21.* It merged as 25, after entries 21 through 24.
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
///   *Written as 26 with `VERSION` at 25 on `main`. Per the standing hazard
///   above, assume the number has moved.*
///
/// * 27 — **the two field cursors**, [`crate::county::County::pasture_cursor`]
///   (`+0x15A`) and [`crate::county::County::blight_cursor`] (`+0x15B`): one
///   byte each a county over 17 slots, **+34**.
///
///   `County_EnsurePasture`'s two sweeps share the first and
///   `Weather_UpdateAll`'s blight walks the second. Both advance before they
///   read, so the cursor decides *which* field a cattle purchase eats and which
///   one a flood ruins.
///
///   **Refusal**, under entry 16's rule: a defaulted load feeds the simulation.
///   Two saves of the same position would blight different fields.
///
///   *Written as 27 with `VERSION` at 26 on `main`. Per the standing hazard
///   above, assume the number has moved.*
///
/// * 28 — **the map's bank plane**, [`crate::map::CampaignMap::bank`] (tile
///   record `+2`): one byte a tile over 4,096 tiles, **+4,096**.
///
///   `Map_ResolvePick` (`0x0046D5FE`) sets `DAT_005651BC` on
///   `(tile.bank & 0x1C) == 4`, and that global is the *whole* of
///   `TileInfo_Draw`'s (`0x0041C208`) mountain-or-woodland test on a `0x08`
///   rough tile. Nothing else in the record answers it.
///
///   **Refusal**, under entry 16's rule: a defaulted load feeds a rule. A zero
///   bank calls every mountain in the kingdom a wood.
///
///   *Written as 28 with `VERSION` at 27 on `main`. Per the standing hazard
///   above, assume the number has moved.*
///
/// * 29 — **no field moved**: [`crate::phase::Pass::ArmyRecountTroops`] was
///   inserted into [`crate::phase::SEASON_PIPELINE`] before
///   `LabourAllocateAgain`, so every later pass *index* shifted and a season
///   report written by 28 would name the wrong passes.
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
///   **Refusal**, under entry 16's rule: a defaulted load feeds the simulation.
///   A save taken between `RationApply` and `GrainSeasonTick` — every autosave
///   inside a season advance — would reload with the season's food never paid
/// for, and the pair is not derivable from `+0x178`/`+0x17C`, which
///   [`crate::ration::preview`] has by then overwritten with next season's
///   forecast (`docs/decisions.md` C20).
///
///   *Written as 30 with `VERSION` at 29 on `main`. Per the standing hazard
///   above, assume the number has moved.*
pub const VERSION: u32 = 30;

/// The header: magic, version, ruleset fingerprint, and the body length.
pub const HEADER_LEN: usize = 8 + 4 + 8 + 4;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoadError {
    /// The first eight bytes are not [`MAGIC`].
    NotASave,
    /// A version this build does not know. **A refusal, not a guess.**
    UnsupportedVersion { found: u32, supported: u32 },
    /// The save was written under a different ruleset. The rules are the mod
    /// layer's to supply; this only reports that they differ.
    RulesetMismatch { save: u64, supplied: u64 },
    /// The body's declared length does not match what followed it.
    TruncatedBody { declared: usize, actual: usize },
    /// The body decoded but its checksum does not match the header's.
    Corrupt { expected: u64, actual: u64 },
    /// The bytes ran out, or a tag byte named nothing.
    Malformed(CodecError),
    /// `head`, `tail` and `len` do not describe a ring of
    /// [`HISTORY_SEASONS`] seasons.
    CorruptHistory { head: usize, tail: usize, len: usize },
    /// A county count larger than the array.
    CountyCount(u32),
    /// The unit array is not [`crate::unit::MAX_UNITS`] slots.
    UnitCount(u32),
    /// The map's planes are not [`crate::map::MAP_TILES`] bytes each.
    MapSize(u32),
    /// More mercenary bands in play than there are bands.
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



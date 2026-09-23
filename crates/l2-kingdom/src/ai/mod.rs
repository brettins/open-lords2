//! `AI_RunTurnStep` (`0x0049A581`) picks the next in-play realm round-robin and
//! dispatches on that realm's `+0x00`. `docs/kingdom.md` §12 calls the handlers
//! *"the single largest remaining piece of the kingdom layer"* and records that
//! **none was decompiled**. All fourteen are decompiled now, and [`AiStep`]
//! names them with their addresses.
//!
//! | step | address | what it does | here? |
//! |---:|---|---|---|
//! | *0* | `0x0049B42B` | recount realm strength, check elimination, rank realms | [`begin_realm_turn`] |
//! | 1 | `0x004A277D` | `Diplo_AnswerInbox` — answer the five pending diplomatic messages | [`crate::diplomacy::answer_inbox`] |
//! | 2 | `0x004A0C1D` | `AI_Diplomacy` — heal standing, age the grudge, court an ally | [`crate::diplomacy::ai_diplomacy`] |
//! | 3 | `0x0049D638` | **`AI_SetTaxRates`** — set tax rates, grant resources | [`set_tax_rates`], [`grant_resources`] |
//! | 4 | `0x0049E1BF` | work out what the realm wants to buy | [`crate::ai_army::resource_wants`] |
//! | 5 | `0x0049DD01` | **`Ai_ManageCountyFarms`** — order fields, then the lord's farming style | [`crate::ai_farm::manage_county_farms`] |
//! | 6 | `0x0049EDC7` | **`AI_BuildCastles`** — order the biggest castle the treasury clears | [`build_castles`] |
//! | 7 | `0x0049F93D` | three army-management passes | [`crate::Kingdom::run_ai_armies`] |
//! | 8 | `0x0049F96C` | **nothing: the function is empty** | [`AiStep::is_empty`] |
//! | 9 | `0x0049F977` | raise the main army and aim it | [`crate::Kingdom::run_ai_raise_army`] |
//! | 10 | `0x004A0015` | send out the raiding force | [`crate::Kingdom::run_ai_raid`] |
//! | 11 | `0x004A5667` | walk every army towards its target | [`crate::Kingdom::run_ai_move_armies`] |
//! | 12 | `0x0049E77D` | **`AI_ChooseIndustry`** — the weapon rota
//! | 13 | `0x004A13A6` | **`AI_Taunt`** — gloat at the human when winning | [`taunt`] |
//! | 14 | `0x0049D1E0` | **recompute the realm's totals** | [`update_realm_totals`] |
//!
//! This table used to give step 5 as *"`AI_ManageFields` (`0x0049DD01`)"*.
//!
//! **Those are two different functions and the mistake hid two whole
//! allocators.** `Ai_ManageCountyFarms` (`0x0049DD01`) is step 5 and dispatches
//! styles 0, 1 and 9 into three allocators; `AI_ManageFields` (`0x0049DFC6`) has
//! exactly one caller, `AI_ManageFields(0)` in turn phase 1, and dispatches
//! styles 0 and 1 into **two others**. There are five allocators in all
//! one previously described as *"the AI's farming style"* is the one no AI realm
//! can reach. [`crate::ai_farm`] carries the correction and all five.
//!
//!    The realm finishes when `aiStep >= 15 + 2 * realmIndex` *and*
//!    `FUN_004A4E3D(1, realm)` returns 0 — a call that sets every one of the
//! realm's idle armies moving and reports whether it found any. So a realm
//!    with armies still to launch keeps stepping past the threshold, doing
//!    nothing at each stop, until they are all away.
//!
//! `0049A933 CMP [aiStep],ECX / JL` with `ECX = 2*realm + 15`, then
//! `CALL 0x004A4E3D / TEST EAX,EAX / JNZ`, then `MOV [aiStep],0x3E7` — and
//! `0049A985 INC dword ptr [aiStep]` sits at the target both jumps skip to, so
//! it runs either way. `docs/decisions.md` C13: the artefact is the evidence.
//!
//! `AI_SetTaxRates` (`0x0049D638`) does two things and `docs/kingdom.md` §8.2
//! gives neither in full. Its **four tax ladders** are
//! [`crate::tables::AiTable::tax_ladder_neutral`] and
//! [`crate::tables::AiTable::tax_ladders`]; its **resource grants** are tiered
//! by the realm's county count, which the document does not mention. Both are
//! implemented, and both are read out of the [`Tables`] the caller hands in
//! — the ladders are `if`/`else if` chains in the
//! original

mod economy;
mod steps;
pub use economy::*;
pub use steps::*;

#[cfg(test)]
use crate::county::County;
#[cfg(test)]
use crate::realm::{Realm, AI_STEP_DONE};
#[cfg(test)]
use crate::tables::{
    tax_rate_for, Tables,
};

pub const AI_HANDLER_COUNT: i32 = 14;

#[cfg(test)]
mod tests;



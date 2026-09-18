#![allow(unused_imports)]
use super::*;
use super::engine::*;
use super::types::*;
use super::ai_part::*;
use super::tests::*;
use l2_kingdom::ai::{self, AiStep};
use l2_kingdom::conquest::Attack;
use l2_kingdom::phase::{Phase, PhaseWait};
use l2_kingdom::units_tick::{Contact, Encounter};
use l2_kingdom::victory::Outcome;
use l2_kingdom::{Kingdom, SeasonReport};
use crate::engagement::{self, Answer, BattleReport, SiegePhase};
use crate::game::Game;

pub fn resolve_battle(game: &mut Game, encounter: Encounter) -> Option<Encounter> {
    let attack = Attack::Battle { attacker: encounter.mover, defender: encounter.occupant };
    let seed = battle_seed(&game.kingdom, encounter);
    let answer = game.field_policy;
    match engagement::resolve(&mut game.kingdom, attack, encounter.county, answer, seed) {
        Some(_) => None,
        None => Some(encounter),
    }
}

pub(super) fn battle_seed(kingdom: &Kingdom, e: Encounter) -> u64 {
    let mut z = (kingdom.turn_count as u64)
        .wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ (e.mover as u64) << 32
        ^ (e.occupant as u64) << 8
        ^ e.county as u64;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// * **Phase 1** — `docs/kingdom.md` §3.1 says the neutral counties get their tax
///   rates and their fields set once a turn, on the neutral ladder, and realm
///   **0** is how the original addresses them: `AI_SetTaxRates(0)` then
///   `AI_ManageFields(0)` — which is `0x0049DFC6`, the *unowned* counties' pass,
///   and not the AI realms' step 5. See `l2_kingdom::ai_farm`.
///
/// > Movement is not confined to a phase: `Units_Tick` runs on every tick of
/// > the turn (`docs/decisions.md` C35), so an army can reach a castle, take a
/// > county or lose a battle *during phase 4*, after the step 0s have all run.
pub(super) fn begin_phase(game: &mut Game, phase: Phase) {
    game.kingdom.begin_unit_phase(phase);
    match phase {
        // **Phase 2 is sieges, and this is where they run.** The variant keeps
        // the name `ArmyMovement` because five files spell it; what it does was
        // corrected in `docs/decisions.md` C35 and it is `Siege_StartPhase`, a
        // cursor over `Siege_BuildTick`, and `Siege_LaunchAssault` for every
        // army whose engines came ready.
        Phase::ArmyMovement => {
            let phase = SiegePhase::begin(&mut game.kingdom);
            if let Some(p) = game.turn.as_mut() {
                p.siege = Some(phase);
            }
        }
        Phase::NeutralCounties => {
            game.kingdom.run_ai_tax_rates(0);
            // > It used to pass `l2_kingdom::ai_farm::NoMarket`, with the
            // > reason written at the type: *"
            // > style's opening shopping cascade is refused and the county
            // > farms what it already has."* **The stated reason was false and
            // > nothing had checked it.** `Ai_BuyGood`'s stall gate is county
            // > `+0x1A4`, which every fixture carries non-zero wherever a
            // > merchant is standing, and its purse is `+0x1F4`, which they
            // > carry at 186 … 436. The behavioural differential put a number
            // > on the refusal: **50 sacks of grain, per unowned county, per
            // > season, for ever.** `docs/decisions.md` C149.
            game.kingdom.run_neutral_farms_at_the_stall();
        }
        Phase::PlayersTurn => {
            if !game.kingdom.turn.players_turn_open {
                for id in 1..l2_kingdom::realm::MAX_REALMS {
                    step_zero(game, id as u8);
                }
            }
        }
        _ => {}
    }
}


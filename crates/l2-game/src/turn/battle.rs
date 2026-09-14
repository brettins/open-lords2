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

/// **The battle seam.** Two enemy armies have met, and somebody has to fight
/// them.
///
/// `l2-kingdom` reports the pair and stops the mover; it cannot resolve the
/// fight because resolving it means `l2-sim`, and `docs/plan.md`'s dependency
/// rule is one way. [`crate::engagement`] is the only module in the workspace
/// that depends on both, and this is the campaign's way in.
///
/// > This was written as a marked stub — the mover landed while battle
/// > resolution was being built on another branch, and an unfought pair went
/// > into [`TurnOutcome::pending_battles`] so that filling the seam in would
/// > change a test. Both have landed,
/// > handoff is a real call now. `pending_battles` stays, because there is
/// > still one case that does not resolve: see below.
///
/// # The player *is* asked now, and this is not the door it happens at
///
/// This used to carry a note saying `end_turn` had no screen to raise *"will
/// you take the field?"* on and therefore answered [`Answer::Decline`], and
/// that when the map screen could raise the prompt *"this is the one line that
/// changes"*. It was not one line,
/// change belonged: a prompt has to **suspend the turn**, because the campaign
/// is left mid-tick with two armies on one tile while the player thinks. So the
/// interactive path is [`begin_turn`] and [`answer_battle`], and this function
/// is what remains for a caller resolving one encounter by itself — a test, or
/// anything outside the turn loop. It answers the game's standing
/// [`Game::field_policy`](crate::game::Game::field_policy).
///
/// # The seed is state, never a clock
///
/// `docs/netcode.md`: two peers fight the same battle or they are not playing
/// the same game. The seed is mixed from the turn counter, the two unit slots
/// and the county — all of which both peers agree on —
/// [`l2_kingdom::Kingdom::rng`], because drawing would advance the generator
/// the weather and the event deck share and make a turn with a battle in it
/// roll different weather from the same turn without one.
///
/// Returns the encounter **unfought** if it could not be resolved, which is
/// then reported.
pub fn resolve_battle(game: &mut Game, encounter: Encounter) -> Option<Encounter> {
    let attack = Attack::Battle { attacker: encounter.mover, defender: encounter.occupant };
    let seed = battle_seed(&game.kingdom, encounter);
    let answer = game.field_policy;
    match engagement::resolve(&mut game.kingdom, attack, encounter.county, answer, seed) {
        Some(_) => None,
        None => Some(encounter),
    }
}

/// A battle's seed, from simulation state both peers already agree on.
///
/// Splitmix64's finaliser over a mix of the turn counter, the two slots and the
/// county. Any deterministic mix would do; what matters is that no term is a
/// clock, an address or an iteration order.
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

/// The work a phase does on its first call.
///
/// * **Phase 1** — `docs/kingdom.md` §3.1 says the neutral counties get their tax
///   rates and their fields set once a turn, on the neutral ladder, and realm
///   **0** is how the original addresses them: `AI_SetTaxRates(0)` then
///   `AI_ManageFields(0)` — which is `0x0049DFC6`, the *unowned* counties' pass,
///   and not the AI realms' step 5. See `l2_kingdom::ai_farm`.
/// * **Phase 4** — `AI_RunTurnStep`'s **step 0** for every realm. See
///   [`step_zero`], which is the prologue in full and which runs for the human
///   too.
/// * **Phases 3, 5 and 6** originate the game's own move orders, and that is
///   `l2-kingdom`'s to do because it is a rule about units:
///   [`Kingdom::begin_unit_phase`] is the transport re-target, the peasant-mob
///   cursor and `Merchant_AdvanceAll`. **Phase 2 is sieges and originates
///   nothing** — see [`l2_kingdom::units_tick`].
///
/// # The one difference from the original,
/// gone
///
/// `AI_RunTurnStep` interleaves: realm 1 takes step 0, then realm 2 takes step 0,
/// and by the time realm 5 reaches its own step 0 the earlier realms have taken
/// several of their fourteen.
/// *later in the same phase*. Here all five step 0s happen at the top of the
/// phase instead.
///
/// > This note used to end: *"The two are the same as long as nothing inside
/// > phase 4 takes a county or destroys an army — which is true today, because
/// > both happen in phase 2 — and this is the note to read when that stops
/// > being true."* **It has stopped being true, and this is that reading.**
/// > Movement is not confined to a phase: `Units_Tick` runs on every tick of
/// > the turn (`docs/decisions.md` C35), so an army can reach a castle, take a
/// > county or lose a battle *during phase 4*, after the step 0s have all run.
/// >
/// > What that costs is bounded: a realm
/// > eliminated by a battle inside phase 4 is recounted at the end of the turn
/// > by [`Game::rank_realms`](crate::game::Game::rank_realms)
/// > mid-phase, so it is noticed one phase later than the original notices it.
/// > Nothing between the two reads the elimination. Interleaving the step 0s
/// > properly is the fix, and it belongs with whoever next touches the AI
/// > dispatcher.
pub(super) fn begin_phase(game: &mut Game, phase: Phase) {
    game.kingdom.begin_unit_phase(phase);
    match phase {
        // **Phase 2 is sieges, and this is where they run.** The variant keeps
        // the name `ArmyMovement` because five files spell it; what it does was
        // corrected in `docs/decisions.md` C35 and it is `Siege_StartPhase`, a
        // cursor over `Siege_BuildTick`, and `Siege_LaunchAssault` for every
        // army whose engines came ready.
        //
        // `engagement::run_siege_phase` has been that pump end to end since it
        // was written and **nothing called it outside its own tests** — a
        // besieging army in a played turn built nothing and never assaulted.
        // This is the call.
        //
        // **The pump is loaded here and run by the tick loop**, one assault at
        // a time, so that each assault can stop and ask the player. Running it
        // to exhaustion in this call — which is what it used to do — is what
        // made a siege the one battle nobody could be asked about.
        Phase::ArmyMovement => {
            let phase = SiegePhase::begin(&mut game.kingdom);
            if let Some(p) = game.turn.as_mut() {
                p.siege = Some(phase);
            }
        }
        Phase::NeutralCounties => {
            game.kingdom.run_ai_tax_rates(0);
            // The unowned counties farm too, by whichever lord held them last —
            // **and they shop**, which is `AI_ManageFields(0)`'s first act and
            // the half of the pass this line used to refuse.
            //
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
            //
            // `Kingdom::run_neutral_farms` builds the stall from the counties'
            // own merchant fields and the unit array.
            // reads `Ai_BuyGood` makes.
            game.kingdom.run_neutral_farms_at_the_stall();
        }
        Phase::PlayersTurn => {
            // Once per turn, and the interactive frames may have run it already
            // when they opened the phase — `turn::open_players_turn`. A second
            // pass would clear `offer_pending` under the diplomacy that had
            // just set it.
            if !game.kingdom.turn.players_turn_open {
                for id in 1..l2_kingdom::realm::MAX_REALMS {
                    step_zero(game, id as u8);
                }
            }
        }
        _ => {}
    }
}


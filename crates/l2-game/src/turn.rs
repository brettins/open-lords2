//! Ending a turn: driving `l2-kingdom`'s phase machine all the way round.
//!
//! `Turn_Tick` (`0x0049A010`) dispatches on `g_turnPhase` every frame and
//! `Turn_AdvancePhase` wraps 7 back to 1. `l2-kingdom` models that as
//! [`TurnMachine`](l2_kingdom::phase::TurnMachine) and deliberately stops
//! there: four of the seven phases wait on units *moving*, and units are not
//! that crate's state, so the caller answers the wait. This module is that
//! caller.
//!
//! # What the spine has to supply
//!
//! Two things, and neither is in `l2-kingdom` because neither is a rule:
//!
//! 1. **The waits.** Phases 2, 3, 5 and 6 wait for armies, transports, peasant
//!    mobs and merchants to stop moving. None of those exist yet, so they are
//!    settled the moment they start, and `settled = true` says exactly that.
//!    When campaign units land, this is the one place that changes.
//!
//! 2. **The AI's turn.** Phase 4 will not end until every realm's `aiStep`
//!    reaches its threshold, and nothing was calling [`l2_kingdom::ai::run_step`]
//!    at all — the crate exposes the machine and the handlers and had no
//!    driver. [`drive_ai`] is the driver: one step per realm per tick, realms in
//!    ascending index order, dispatching the four handlers `l2-kingdom`
//!    implements and skipping the ten whose state lives elsewhere.
//!
//! # Determinism
//!
//! Nothing here reads a clock, allocates a decision, or iterates anything but
//! an ascending index range. A turn is a pure function of the kingdom it
//! started from: the same save ended twice produces the same numbers, which is
//! what `tests/turn.rs` asserts by running one twice.

use l2_kingdom::ai::{self, AiStep};
use l2_kingdom::phase::Phase;
use l2_kingdom::{Kingdom, SeasonReport};

use crate::game::Game;

/// How many `Turn_Tick` calls one whole turn may take before we conclude the
/// machine is not going to come round.
///
/// A real turn takes far fewer — `tests/turn.rs` pins the actual number — and
/// this exists only so a bug in a wait condition is a diagnosable stop rather
/// than a hang in the event loop.
pub const MAX_TICKS: u32 = 512;

/// What one end-of-turn did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TurnOutcome {
    pub report: SeasonReport,
    /// How many phase ticks it took. Interesting only as a check on the
    /// machine; nothing branches on it.
    pub ticks: u32,
}

/// Run the phase machine until the end-of-season pipeline has run once.
///
/// Returns `None` only if [`MAX_TICKS`] is reached, which would be a bug in a
/// wait condition rather than something a player can cause.
pub fn end_turn(game: &mut Game) -> Option<TurnOutcome> {
    for (id, realm) in game.kingdom.realms.iter().enumerate() {
        game.gold_last[id] = realm.gold;
    }

    let mut ticks = 0;
    // The AI's resource grant runs once a turn. See `run_handler`.
    let mut granted = false;
    while ticks < MAX_TICKS {
        ticks += 1;
        let phase = game.kingdom.turn.phase;
        // `step == 0` is the machine's own "this is the first call of this
        // phase", which is when the original's phase handlers kick off work.
        if game.kingdom.turn.step == 0 {
            begin_phase(&mut game.kingdom, phase);
        }
        if phase == Phase::PlayersTurn {
            drive_ai(&mut game.kingdom, &mut granted);
        }
        // Every wait that is not the AI's is answered `true`: the units those
        // phases wait on do not exist yet.
        let (_, report) = game.kingdom.tick(true);
        if let Some(report) = report {
            game.turns_played += 1;
            game.last_report = Some(report.clone());
            return Some(TurnOutcome { report, ticks });
        }
    }
    None
}

/// The work a phase does on its first call.
///
/// Only phase 1 has any, here: `docs/kingdom.md` §3.1 says the neutral counties
/// get their tax rates and their fields set once a turn, on the neutral ladder,
/// and realm **0** is how the original addresses them —
/// `AI_SetTaxRates(0)` then `AI_ManageFields(0)`.
fn begin_phase(kingdom: &mut Kingdom, phase: Phase) {
    if phase == Phase::NeutralCounties {
        kingdom.run_ai_tax_rates(0);
        ai::manage_fields(&mut kingdom.counties, kingdom.county_count, 0);
    }
}

/// One step of every AI realm that has not finished its turn.
///
/// Realms are walked in ascending index order and each takes exactly one step
/// per tick, which is what the original's per-frame dispatcher does. The step
/// to run is the counter's value *before* the increment, so
/// [`ai::pending_step`] is read first and [`ai::run_step`] second.
///
/// `launched_army` is passed `false`: it asks whether the realm found an idle
/// army to start moving, and there are no armies. That is the pure-economy
/// turn `l2-kingdom` documents, not a simplification invented here.
pub fn drive_ai(kingdom: &mut Kingdom, granted: &mut bool) {
    for id in 1..kingdom.realms.len() {
        if kingdom.realms[id].turn_done() {
            continue;
        }
        let step = ai::pending_step(&kingdom.realms[id], id);
        let _finished = ai::run_step(&mut kingdom.realms[id], id, false);
        if let Some(step) = step {
            run_handler(kingdom, id as u8, step, granted);
        }
    }
}

/// Dispatch one AI handler.
///
/// Four of the fourteen are implemented in `l2-kingdom`; the other ten drive
/// armies, merchants, diplomacy and map tiles, which no crate owns yet. They
/// are skipped rather than stubbed, so nothing here pretends to be a rule.
fn run_handler(kingdom: &mut Kingdom, realm: u8, step: AiStep, granted: &mut bool) {
    match step {
        AiStep::SetTaxRates => {
            kingdom.run_ai_tax_rates(realm);
            // **The grant runs once a turn, not once per realm**, and that is a
            // real difference from the original worth writing down.
            // `AI_SetTaxRates(realm)` grants *that* realm its gold and its
            // counties their free people, herd and grain;
            // `l2_kingdom::ai::grant_resources` instead walks every AI realm
            // and every AI-owned county in one pass. Calling it inside each
            // realm's step 3 would hand every realm four turns' worth of gold.
            // Running it once, at the first step 3 of the turn, puts it where
            // the original puts it and produces the original's totals.
            //
            // On the shipped scenario the difficulty is 0 and every grant
            // multiplies by it, so nothing moves either way; the distinction
            // matters the moment somebody starts a harder game.
            if !*granted {
                kingdom.run_ai_grants();
                *granted = true;
            }
        }
        AiStep::ManageFields => {
            ai::manage_fields(&mut kingdom.counties, kingdom.county_count, realm);
        }
        AiStep::UpdateTotals => update_totals(kingdom, realm),
        // An empty function in the shipped binary. Named, and it does nothing
        // here for the same reason it does nothing there.
        AiStep::Nothing => {}
        _ => {}
    }
}

/// `armies` and `total_men` are zero because the unit array does not exist yet.
/// They land in realm `+0x2C` and `+0x54`, and `+0x54` is a score input, so the
/// scores this produces are the economy's part of the score and not the whole.
fn update_totals(kingdom: &mut Kingdom, realm: u8) {
    ai::update_realm_totals(
        &mut kingdom.realms[realm as usize],
        &kingdom.counties,
        kingdom.county_count,
        realm,
        0,
        0,
    );
}


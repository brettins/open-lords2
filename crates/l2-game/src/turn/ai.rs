#![allow(unused_imports)]
use super::*;
use super::engine::*;
use super::types::*;
use super::battle::*;
use super::tests::*;
use l2_kingdom::ai::{self, AiStep};
use l2_kingdom::conquest::Attack;
use l2_kingdom::phase::{Phase, PhaseWait};
use l2_kingdom::units_tick::{Contact, Encounter};
use l2_kingdom::victory::Outcome;
use l2_kingdom::{Kingdom, SeasonReport};
use crate::engagement::{self, Answer, BattleReport, SiegePhase};
use crate::game::Game;

/// **`AI_RunTurnStep`'s step-0 prologue (`0x0049A581`), for one realm.**
///
/// ```c
/// if (g_realms[r].strength != 0) {          /* the outer guard, read BEFORE the recount */
///     if (g_realms[r].aiStep == 0) {
///         Realm_RecountStrength(r);         /* 0x0049B42B, and Score_RankRealms inside it */
///         Realm_UpdateTotals(r);            /* 0x0049D1E0 — FUN_0049d1e0 */
///         g_realms[r].offerPending = 0;     /* +0x1C */
///         g_realms[r].aiStep = 1;
///     }
///     if ((g_realms[r].isHuman == 0) && (aiStep < 999)) { ...the fourteen handlers... }
/// }
/// ```
///
/// **The `isHuman` test guards the handlers and the counter's increment,
/// this.** `Turn_BeginPlayersTurn` (`0x0049B6D3`) writes `aiStep = 0` into every
/// realm — 999 only for a realm at zero strength —
/// 0 exactly like an AI one and then stops, its counter parked at 1 until
/// `Turn_End` (`0x0043AC23`) writes 999.
///
/// # What this fixes,
///
/// This used to be `game.recount_realm(id)` alone — the first of the prologue's
/// three writes. `Realm_UpdateTotals` is **the only thing in the original that
/// fills the six score inputs**, and its other production caller here is AI step
/// 14, which a human realm never reaches ([`l2_kingdom::ai::begin_turn`] marks a
/// human done before step 0). So the human's `share_of_map_pct`,
/// `population_total`, `mean_happiness`, `mean_health` and `total_men` all stayed
/// at zero for the whole game and `Score_RankRealms` scored the human on the gold
/// bracket alone: a flat **50** against the original's 576, 590, 1333 and 1334
/// across `crates/l2-game/tests/differential.rs`' four pairs, with `rank`
/// inverted for both realms out of the same hole.
///
/// **`offer_pending` was set by [`l2_kingdom::diplomacy`] and cleared by nothing**
/// — `grep -rn offer_pending crates/` found one writer and no reset —
/// that once courted somebody was refused as a candidate by
/// `pick_ally_candidate` for the rest of the game. This is the clear the original
/// does, at the line the original does it.
///
/// [`l2_kingdom::ai::begin_realm_turn`] is this prologue's `l2-kingdom` half and
/// has never had a production caller; it is left as the crate's own statement of
/// the rule, and this is the call site.
fn step_zero(game: &mut Game, realm: u8) {
// The original reads the guard before the recount,
    // recount *eliminates* still gets its totals rebuilt and its offer cleared.
    if game.kingdom.realms[realm as usize].strength == 0 {
        return;
    }
    game.recount_realm(realm);
    update_totals(&mut game.kingdom, realm);
    game.kingdom.realms[realm as usize].offer_pending = false;
}

/// One step of every AI realm that has not finished its turn.
///
/// Realms are walked in ascending index order and each takes exactly one step
/// per tick, which is what the original's per-frame dispatcher does. The step
/// to run is the counter's value *before* the increment, so
/// [`ai::pending_step`] is read first and [`ai::run_step`] second.
///
/// > `launched_army` **used to be passed `false`**, on the grounds that "it
/// > asks whether the realm found an idle army to start moving, and there are
/// > no armies". There are armies now,
/// > `FUN_004A4E3D(1, realm)`, which is
/// > [`Kingdom::realm_units_moving`](l2_kingdom::Kingdom::realm_units_moving).
/// > It matters because it is half of the finish test — `docs/armies.md` §3.2:
/// > a realm is done only when its `aiStep` has passed `15 + 2 × realmIndex`
/// > **and** none of its armies is still walking. A realm marching on somebody
/// > keeps taking steps.
pub fn drive_ai(kingdom: &mut Kingdom, granted: &mut bool) {
    for id in 1..kingdom.realms.len() {
        if kingdom.realms[id].turn_done() {
            continue;
        }
        let marching = kingdom.realm_units_moving(l2_kingdom::UnitKind::Army, id as u8);
        let step = ai::pending_step(&kingdom.realms[id], id);
        let _finished = ai::run_step(&mut kingdom.realms[id], id, marching);
        if let Some(step) = step {
            run_handler(kingdom, id as u8, step, granted);
        }
    }
}

/// Dispatch one AI handler.
///
/// **All fourteen run.** One of them is empty in the shipped binary and is
/// empty here for the same reason.
///
/// > This used to read *"twelve of the fourteen; the other two are the
/// > diplomacy steps, which need a `l2_kingdom::diplomacy` that does not exist
/// > yet"*. It exists.
///
/// > This used to read: *"Four of the fourteen are implemented in
/// > `l2-kingdom`; the other ten drive armies, merchants, diplomacy and map
/// > tiles, which no crate owns yet."* `l2-kingdom` has owned a unit model
/// > since the day that was written — `unit.rs`, `movement.rs`, `levy.rs`,
/// > `map.rs` — so the sentence had stopped being a decision and become a
/// > description of a world that had gone. `docs/plan.md` §2.4;
/// > `l2_kingdom::ai_army` is the five steps it was blocking.
fn run_handler(kingdom: &mut Kingdom, realm: u8, step: AiStep, granted: &mut bool) {
    match step {
        // The two diplomacy steps. Their letters are dropped here for the same
        // reason the taunt's are — `l2-game` has no letter screen yet — and
        // everything they change about the simulation has already happened by
        // the time they return: a standing moved, an alliance formed or broken,
        // a war target named. Those four fields are what AI step 10 and the
        // *assist ally* mission read, and until `l2_kingdom::diplomacy` existed
        // nothing wrote any of them. `docs/decisions.md` C62.
        AiStep::Diplomacy => {
            kingdom.run_ai_inbox(realm);
        }
        AiStep::ConsiderWar => {
            kingdom.run_ai_diplomacy(realm);
        }
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
            // On the England turn-one scenario the difficulty is 0 and every grant
            // multiplies by it, so nothing moves either way; the distinction
            // matters the moment somebody starts a harder game.
            if !*granted {
                kingdom.run_ai_grants();
                *granted = true;
            }
        }
        AiStep::ResourceWants => kingdom.run_ai_resource_wants(realm),
        // **At the stall.** This passed `NoMarket`, so an AI lord's counties
        // never bought a sack or a head: `Ai_FarmStyleArable`/`Grazing`/`Mixed`
        // (`0x004A4052`, `0x004A42E3`, `0x004A440F`) each open with
        // `Ai_BuyGood` lines, and `Ai_BuyGood` (`0x004A4B12`) pays an owned
        // county's bill out of `g_realms[owner].gold`. The same pass runs again
        // at the head of the season — `l2_kingdom::Pass::AiManageFarms`.
        AiStep::ManageFields => {
            kingdom.run_ai_farms_at_the_stall(realm);
        }
        AiStep::BuildCastles => {
            kingdom.run_ai_castles(realm);
        }
        // The four army steps. Their reports — which garrisons went up, which
        // counties were written off, which garrisons were turned out of a lost
        // castle — are dropped here for the same reason the taunt's letters
        // are: `l2-game` has nowhere to show them yet. Everything they change
        // about the simulation has already happened by the time they return.
        AiStep::ManageArmies => {
            kingdom.run_ai_armies(realm);
        }
        AiStep::RaiseArmy => {
            kingdom.run_ai_raise_army(realm);
        }
        AiStep::SendUnit => {
            kingdom.run_ai_raid(realm);
        }
        AiStep::MoveArmies => {
            kingdom.run_ai_move_armies(realm);
        }
        AiStep::ChooseIndustry => kingdom.run_ai_industry(realm),
// The letters are dropped: a realm-to-realm taunt is
        // not a season message and `l2-game` has no letter screen yet. The
        // realm's timer, stage and voice rotation still advance, which is the
        // whole of the step's effect on the simulation.
        AiStep::Taunt => {
            kingdom.run_ai_taunt(realm);
        }
        AiStep::UpdateTotals => update_totals(kingdom, realm),
        // An empty function in the shipped binary. Named, and it does nothing
        // here for the same reason it does nothing there.
        //
// **
        // wildcard was what let the two diplomacy steps sit undispatched
        // without the compiler having anything to say. A fifteenth handler
// would now be a compile error.
        AiStep::Nothing => {}
    }
}

/// > `armies` and `total_men` **used to be zero**, "because the unit array does
/// > not exist yet". It does, and they come from it:
/// > [`Units::realm_totals`](l2_kingdom::Units::realm_totals) counts the
/// > realm's armies and their men. They land in realm `+0x2C` and `+0x54`, and
/// > `+0x54` is a score input — so this is the difference between a score that
/// > is the economy's part and a score that is the whole, and every realm's
/// > rank moves the first turn anybody raises an army.
fn update_totals(kingdom: &mut Kingdom, realm: u8) {
    let (armies, total_men) = kingdom.campaign.units.realm_totals(realm);
    ai::update_realm_totals(
        &mut kingdom.realms[realm as usize],
        &kingdom.counties,
        kingdom.county_count,
        realm,
        armies,
        total_men,
    );
}


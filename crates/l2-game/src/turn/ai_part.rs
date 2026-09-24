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
pub(super) fn step_zero(game: &mut Game, realm: u8) {
    // 00490000.c:4920-4921: `strength != 0` then `aiStep == 0` guard the prologue [V].
    let r = &game.kingdom.realms[realm as usize];
    if r.strength == 0 || r.ai_step != 0 {
        return;
    }
    game.recount_realm(realm);
    update_totals(&mut game.kingdom, realm);
    game.kingdom.realms[realm as usize].offer_pending = false;
}

/// > `launched_army` **used to be passed `false`**, on the grounds that "it
/// > asks whether the realm found an idle army to start moving, and there are
/// > no armies". There are armies now,
/// > `FUN_004A4E3D(1, realm)`, which is
/// > [`Kingdom::realm_units_moving`](l2_kingdom::Kingdom::realm_units_moving).
///
/// **Open phase 4 on the frame the person gets control** —
/// `Turn_AdvancePhase`'s (`0x0049CE51`) call to `Turn_BeginPlayersTurn`
/// (`0x0049B6D3`), followed by `AI_RunTurnStep`'s (`0x0049A581`) step-0
/// prologue for every realm.
///
/// `Turn_Tick` (`0x0049A010`) runs phase 4's arm **every frame**, and its arm is
/// `AI_RunTurnStep(); if (Turn_AllRealmsDone()) Turn_AdvancePhase();` with no
/// test on the person at all: the AI realms take their steps while he is still
/// deciding, and `Turn_End` (`0x0043AC23`) writing 999 into his own `aiStep` is
/// the last thing the phase waits for. Our machine parks between turns instead
/// (`g_turnPhase = 1`, the instant the original's autosave is taken), so phase 4
/// used to be opened *and run to the end* inside the wind-on End Turn starts —
/// which is the whole of the report *"the AI seems to take its turn at End Turn
/// instead of at the start of the turn"*. This opens it on the map's idle
/// frames instead, and [`TurnMachine::players_turn_open`](l2_kingdom::phase::TurnMachine)
/// stops the wind-on opening it again.
pub fn open_players_turn(game: &mut Game) {
    if game.kingdom.turn.players_turn_open {
        return;
    }
    // Turn_BeginPlayersTurn has one caller, Turn_AdvancePhase's 7 -> 1 wrap (setup.rs) [V].
    for id in 1..l2_kingdom::realm::MAX_REALMS {
        let fresh = game.kingdom.realms[id].ai_step == 0;
        step_zero(game, id as u8);
        let realm = &mut game.kingdom.realms[id];
        if fresh && realm.strength != 0 && realm.is_human {
            realm.ai_step = 1;
            // 0x0049A581 steps the human too; the isHuman guard leaves it the prologue alone.
            l2_net::trace!("ai", 0x0049A581, "AI_RunTurnStep", realm = id, step = 0, done = 0);
        }
    }
    game.ai_granted = false;
    game.kingdom.turn.players_turn_open = true;
}

/// **`Turn_End` (`0x0043AC23`)** — the sidebar's bottom button writes 999 into
/// the local player's `aiStep`, and that is the only thing phase 4 is still
/// waiting for once the AI realms have finished.
pub fn end_players_turn(game: &mut Game) {
    for realm in game.kingdom.realms.iter_mut() {
        if realm.is_human {
            realm.ai_step = l2_kingdom::AI_STEP_DONE;
        }
    }
}

/// Every seated human has pressed End Turn: `aiStep == 999`, as `Turn_End` writes it.
pub fn every_seat_ready(game: &Game) -> bool {
    game.seats.iter().all(|&(_, realm)| {
        let r = game.kingdom.realms.get(realm as usize);
        r.is_none_or(|r| !r.in_play || !r.is_human || r.ai_step >= l2_kingdom::AI_STEP_DONE)
    })
}

pub fn tick_ai_frame(game: &mut Game) {
    open_players_turn(game);
    let mut granted = game.ai_granted;
    let mut letters = Vec::new();
    drive_ai(&mut game.kingdom, &mut granted, &mut letters);
    game.ai_granted = granted;
    post_letters(game, &letters);
}

/// Every letter the two diplomacy steps compose goes on the ring —
/// `Msg_Enqueue` (`0x00472BC5`), which is [`crate::message::MessageQueue::post`]
/// and drops what is not addressed to this seat.
pub fn post_letters(game: &mut Game, letters: &[l2_kingdom::diplomacy::Letter]) {
    let player = game.player;
    for letter in letters {
        game.messages.post(*letter, player);
    }
}

pub fn drive_ai(
    kingdom: &mut Kingdom,
    granted: &mut bool,
    letters: &mut Vec<l2_kingdom::diplomacy::Letter>,
) {
    // AI_RunTurnStep 0x0049A581: one realm a call, the first with strength != 0 from the
    // cursor DAT_0057C944, which moves one past it and wraps 5 -> 1 [V].
    let last = (kingdom.realms.len() - 1).min(5) as u8;
    let mut id = kingdom.turn.ai_cursor;
    for _ in 0..last {
        id = kingdom.turn.ai_cursor;
        kingdom.turn.ai_cursor = if id >= last { 1 } else { id + 1 };
        // 00490000.c:4919 `while (g_realms[uVar1].strength == 0)` [V].
        if kingdom.realms[id as usize].strength != 0 {
            break;
        }
    }
    let id = id as usize;
    let Some(realm) = kingdom.realms.get_mut(id).filter(|r| r.strength != 0) else { return };
    // `if (aiStep == 0) { …prologue…; aiStep = 1; }` then the handler in the same call [V];
    // our prologue ran in `open_players_turn`.
    if realm.ai_step == 0 {
        realm.ai_step = 1;
    }
    // 00490000.c:4970: `2*realm + 15 <= aiStep && FUN_004A4E3D(1, realm) == 0`; the call,
    // which starts ordered armies, is short-circuited below the threshold [V].
    let r = &kingdom.realms[id];
    // 00490000.c:4920 `strength != 0` is the filter above; 4927 `isHuman == 0 && aiStep < 999` [V].
    let at_end = !r.is_human && r.ai_step < l2_kingdom::AI_STEP_DONE
        && r.ai_step >= ai::done_threshold(id);
    let marching = at_end && kingdom.realm_units_moving(l2_kingdom::UnitKind::Army, id as u8);
    let step = ai::pending_step(&kingdom.realms[id], id);
    let _finished = ai::run_step(&mut kingdom.realms[id], id, marching);
    if let Some(step) = step {
        run_handler(kingdom, id as u8, step, granted, letters);
    }
}

fn run_handler(
    kingdom: &mut Kingdom,
    realm: u8,
    step: AiStep,
    granted: &mut bool,
    letters: &mut Vec<l2_kingdom::diplomacy::Letter>,
) {
    match step {
        // `Diplo_AnswerInbox` (`0x004A277D`) ends each of its six answers on
        // `Msg_Enqueue`, so the reply to a letter the person sent reaches him on
        // the recipient's next AI turn. These two returns used to be dropped
        // here, with a comment reading *"`l2-game` has no letter screen yet"* —
        // it has one (`crate::screens::message`), and that stale justification
        // is `docs/decisions.md` C113's shape exactly.
        AiStep::Diplomacy => {
            letters.extend(kingdom.run_ai_inbox(realm));
        }
        // `AI_Diplomacy` (`0x004A0C1D`): `Msg_Enqueue` of 0xB5 (grudge break)
        // and 0xB4 (alliance offer), decomp 004a0000.c:408, :431.
        AiStep::ConsiderWar => {
            letters.extend(kingdom.run_ai_diplomacy(realm));
        }
        AiStep::SetTaxRates => {
            kingdom.run_ai_tax_rates(realm);
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
        // `AI_Taunt` (`0x004A13A6`): `Msg_Enqueue(realm, to, 0xC1|0xC0, variant,
        // '', 0, 0, 0)`, decomp 004a0000.c:502, :521.
        AiStep::Taunt => {
            letters.extend(kingdom.run_ai_taunt(realm).into_iter().map(|t| {
                l2_kingdom::diplomacy::Letter {
                    from: t.from,
                    to: t.to,
                    group: t.group,
                    variant: t.variant,
                    category: l2_kingdom::diplomacy::category::LETTER,
                    county: 0,
                    payload: 0,
                }
            }));
        }
        AiStep::UpdateTotals => update_totals(kingdom, realm),
        AiStep::Nothing => {}
    }
}

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


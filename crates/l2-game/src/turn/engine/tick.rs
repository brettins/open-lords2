#![allow(unused_imports)]
use super::*;
use super::control::*;
use super::battle::*;
use super::*;
use super::types::*;
use super::battle::*;
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

/// `Unit_EnterOccupiedTile`'s rung 3 is *"same owner — merge, but only on an
/// explicit merge order"*,
/// army walks over its own. Ours stops (`Contact::Blocked`'s own note). The
/// question it stops on is the original's: `Map_ConfirmMoveOrder`
/// (`0x004A9252`) raises `L2.eng` 10/5 *"Combine armies?"* off
/// `g_hoverMergeUnit` and `MoveOrder_ConfirmCombine` (`0x004A975D`) merges into
/// the standing unit. **When** we ask is ours — [`Game::combine_ask`].
pub(super) fn ask_combine(game: &mut Game, contacts: &[Contact]) {
    if game.combine_ask.is_some() {
        return;
    }
    let units = &game.kingdom.campaign.units;
    game.combine_ask = contacts.iter().find_map(|c| {
        let Contact::Blocked { mover, occupant } = *c else { return None };
        let (m, o) = (units.get(mover)?, units.get(occupant)?);
        let mine = |u: &l2_kingdom::Unit| {
            u.owner == game.player && u.kind == l2_kingdom::unit::UnitKind::Army
        };
        (mine(m) && mine(o)).then_some((mover, occupant))
    });
}

pub(super) fn raise_idle_battle(game: &mut Game, e: Encounter) {
    use l2_kingdom::battle::Settlement;
    let settlement = l2_kingdom::battle::settlement(
        &game.kingdom.campaign.units,
        e.mover,
        e.occupant,
        game.kingdom.options.fight_humans_only_byte,
    );
    if settlement == Settlement::Silently || game.turn.is_some() {
        let attack = Attack::Battle { attacker: e.mover, defender: e.occupant };
        let seed = battle_seed(&game.kingdom, e);
        let answer = game.field_policy;
        let report = engagement::resolve(&mut game.kingdom, attack, e.county, answer, seed);
        record(game, report);
        return;
    }
    game.turn = Some(TurnProgress { idle: true, ..TurnProgress::default() });
    let q = Question { county: e.county, ..question_for(game, e.mover, e.occupant, None) };
    if settlement == Settlement::Prompt {
        game.turn.as_mut().expect("just installed").question = Some(q);
        return;
    }
    let answer = game.field_policy;
    settle_question(game, q, answer);
}

/// **The sortie** — `FUN_00437535`'s tail, the body of `Army_LeaveCastle`
/// (`0x004374C4`):
///
/// `Battle_BeginFromCampaign` (`0x004A7158`) takes the marching garrison as
/// `g_battleArmyA` and the besieger as `g_battleArmyB`; the caller then
/// **overwrites `g_battleCounty`** with the castle's county.
pub(crate) fn raise_sortie(game: &mut Game, garrison: usize, besieger: usize, county: u8) {
    raise_idle_battle(game, Encounter { mover: garrison, occupant: besieger, county });
}

pub(super) fn run_phase_tick(game: &mut Game) {
    let phase = game.kingdom.turn.phase;
    if phase == Phase::PlayersTurn {
        let mut granted = game.turn.as_ref().is_some_and(|p| p.granted);
        drive_ai(&mut game.kingdom, &mut granted);
        if let Some(p) = game.turn.as_mut() {
            p.granted = granted;
        }
    }
    let (_, report) = game.kingdom.tick(settled(&game.kingdom, phase));

    let moved = game.sweep_units();
    // `DAT_00553210`, the invasion tip's flag. See `crate::tip`.
    game.tips.note_incursions(&moved.incursions, game.player);
    crate::arrival::post(game, &moved.posted);
    let Some(p) = game.turn.as_mut() else { return };
    p.steps += moved.stepped;
    p.stage = Stage::Tail;
    p.tail = Some(Tail { battle: moved.battle(), contacts: moved.contacts, report });
}

pub(super) fn pump_siege(game: &mut Game) {
    let Some(mut phase) = game.turn.as_mut().and_then(|p| p.siege.take()) else { return };
    let Some(assault) = phase.next(&mut game.kingdom) else {
        return;
    };
    // `Turn_Tick`'s phase-2 arm: `DAT_0055403C = 0; Siege_LaunchAssault(…)`.
    game.turn_clock.assault_launched();
    let seed = siege_seed(&game.kingdom);
    let level = match assault {
        l2_kingdom::siege::Assault::Battle { castle_level, .. } => Some(castle_level),
        _ => None,
    };
    let question = SiegePhase::settlement(&game.kingdom, assault).and_then(|(a, d, s)| {
        (s == l2_kingdom::battle::Settlement::Prompt).then(|| question_for(game, a, d, level))
    });
    match question {
        Some(q) => {
            let p = game.turn.as_mut().expect("pump runs inside a turn");
            p.siege = Some(phase);
            p.pending_assault = Some(assault);
            p.question = Some(q);
        }
        None => {
            let report = phase.settle(&mut game.kingdom, assault, game.field_policy, seed);
            let p = game.turn.as_mut().expect("pump runs inside a turn");
            p.siege = Some(phase);
            record(game, report);
        }
    }
}

fn settled(kingdom: &Kingdom, phase: Phase) -> bool {
    match phase.wait() {
        PhaseWait::Units(kind) => !kingdom.units_moving(kind),
        PhaseWait::Sieges => true,
        PhaseWait::Steps(_) | PhaseWait::AllRealmsDone | PhaseWait::Immediate => true,
    }
}

pub(crate) fn siege_seed(kingdom: &Kingdom) -> u64 {
    let mut z = (kingdom.turn_count as u64)
        .wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ 0x5165_6765_0000_0002;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}



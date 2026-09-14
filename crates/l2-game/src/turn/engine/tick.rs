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

/// **The halt where the original walks on** — [`Contact::Blocked`] with the
/// player's own army standing on the tile.
///
/// `Unit_EnterOccupiedTile`'s rung 3 is *"same owner — merge, but only on an
/// explicit merge order"*,
/// army walks over its own. Ours stops (`Contact::Blocked`'s own note). The
/// question it stops on is the original's: `Map_ConfirmMoveOrder`
/// (`0x004A9252`) raises `L2.eng` 10/5 *"Combine armies?"* off
/// `g_hoverMergeUnit` and `MoveOrder_ConfirmCombine` (`0x004A975D`) merges into
/// the standing unit. **When** we ask is ours — [`Game::combine_ask`].
///
/// Only the local player's own two armies: an AI's pair merges with no prompt
/// (`Contact::Merged`), and nobody may be asked about another realm's units.
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

/// `Battle_ChooseSettlement` for a battle raised outside a turn. See
/// [`tick_units_only`].
pub(super) fn raise_idle_battle(game: &mut Game, e: Encounter) {
    use l2_kingdom::battle::Settlement;
    let settlement = l2_kingdom::battle::settlement(
        &game.kingdom.campaign.units,
        e.mover,
        e.occupant,
        game.kingdom.options.fight_humans_only_byte,
    );
    if settlement == Settlement::Silently || game.turn.is_some() {
        // **Nobody's but the lords'**,
        // screen, no report, the autocalc and on with the frame. Through
        // [`record`] like every other battle, so the losing realm is recounted
        // here too —
        // itself has nowhere to go and `record` drops it,
        // the half that must not be dropped with it.
        //
// The `game.turn.is_some()` half is a guard: the map
        // screen does not run this sweep while a turn is in flight, and
        // clobbering a suspended turn with an idle one would lose a season.
        let attack = Attack::Battle { attacker: e.mover, defender: e.occupant };
        let seed = battle_seed(&game.kingdom, e);
        let answer = game.field_policy;
        let report = engagement::resolve(&mut game.kingdom, attack, e.county, answer, seed);
        record(game, report);
        return;
    }
    // A person is in it. Suspend the campaign the way a turn's battle does, and
    // mark the suspension as *not a turn* so that answering it winds nothing on.
    game.turn = Some(TurnProgress { idle: true, ..TurnProgress::default() });
    let q = Question { county: e.county, ..question_for(game, e.mover, e.occupant, None) };
    if settlement == Settlement::Prompt {
        game.turn.as_mut().expect("just installed").question = Some(q);
        return;
    }
    // `Settlement::Reported` — *Fight humans only?* is on and the other side is
    // the AI's. The autocalc runs and the player is **told** on screen `0x13`
// The report used to be dropped here.
    let answer = game.field_policy;
    settle_question(game, q, answer);
}

/// **The sortie** — `FUN_00437535`'s tail, the body of `Army_LeaveCastle`
/// (`0x004374C4`):
///
/// ```c
/// if (unit.besiegedBy && Battle_BeginFromCampaign(unit, unit.besiegedBy))
///     g_battleCounty = county;
/// ```
///
/// `Battle_BeginFromCampaign` (`0x004A7158`) takes the marching garrison as
/// `g_battleArmyA` and the besieger as `g_battleArmyB`; the caller then
/// **overwrites `g_battleCounty`** with the castle's county.
/// county here is the one left and not the occupant's.
///
/// The button is pressed on an ordinary frame, so this is
/// [`raise_idle_battle`]'s gate — `Battle_ChooseSettlement` either way.
pub(crate) fn raise_sortie(game: &mut Game, garrison: usize, besieger: usize, county: u8) {
    raise_idle_battle(game, Encounter { mover: garrison, occupant: besieger, county });
}

/// The phase's own tick and the unit sweep that follows it, up to the point
/// where a battle may interrupt.
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

    // `Units_Tick`, immediately after `Turn_Tick` and outside the phase
    // machine entirely. See the module documentation.
    // `Game::sweep_units` also holds the frame each tick handler writes first.
    let moved = game.sweep_units();
    // `DAT_00553210`, the invasion tip's flag. See `crate::tip`.
    game.tips.note_incursions(&moved.incursions, game.player);
    // The same sweep's letters, the same door as `tick_units_only`'s.
    crate::arrival::post(game, &moved.posted);
    let Some(p) = game.turn.as_mut() else { return };
    p.steps += moved.stepped;
    p.stage = Stage::Tail;
    p.tail = Some(Tail { battle: moved.battle(), contacts: moved.contacts, report });
}

/// One assault of phase 2, asked about or settled.
pub(super) fn pump_siege(game: &mut Game) {
    let Some(mut phase) = game.turn.as_mut().and_then(|p| p.siege.take()) else { return };
    let Some(assault) = phase.next(&mut game.kingdom) else {
        // The cursor ran off the end; phase 2 is done and the tick carries on.
        return;
    };
    // `Turn_Tick`'s phase-2 arm: `DAT_0055403C = 0; Siege_LaunchAssault(…)`.
    // The flag is the one the turn timer's restart waits on.
    game.turn_clock.assault_launched();
    let seed = siege_seed(&game.kingdom);
    // A refusal is not a battle and cannot be asked about: the siege has
    // already been lifted by `siege::assault` and there is nothing to fight.
    let level = match assault {
        l2_kingdom::siege::Assault::Battle { castle_level, .. } => Some(castle_level),
        _ => None,
    };
    let question = SiegePhase::settlement(&game.kingdom, assault).and_then(|(a, d, s)| {
        (s == l2_kingdom::battle::Settlement::Prompt).then(|| question_for(game, a, d, level))
    });
    match question {
        Some(q) => {
            // Park the assault on the question so the answer settles this one.
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

/// Answer the current phase's wait.
///
/// Three of the four unit phases ask the unit array directly. Phase 4's wait is
/// the AI's and `Kingdom::tick` overrides whatever is passed for it.
///
/// **Phase 2 is answered `true` here because [`begin_phase`] has already run
/// the whole of it.** `Turn_Tick`'s phase-2 arm is a pump — validate, build,
/// assault, repeat until the cursor comes up empty — and
/// [`crate::engagement::run_siege_phase`] runs that pump to exhaustion in one
/// call. The two agree on every number because
/// the cursor only ever advances and no other phase runs between its steps; the
/// difference is that ours does not spread the sieges over as many `Turn_Tick`
/// calls, which nothing outside the phase can observe. See the comment on
/// [`begin_phase`]'s `ArmyMovement` arm.
fn settled(kingdom: &Kingdom, phase: Phase) -> bool {
    match phase.wait() {
        PhaseWait::Units(kind) => !kingdom.units_moving(kind),
        PhaseWait::Sieges => true,
        PhaseWait::Steps(_) | PhaseWait::AllRealmsDone | PhaseWait::Immediate => true,
    }
}

/// The seed for the siege phase's assaults, from state both peers agree on.
///
/// The same rule as [`battle_seed`]: the turn counter and nothing that is a
/// clock, an address or an iteration order. `run_siege_phase` adds the round
/// number to it,
/// same phase run twice fights the same two.
pub(crate) fn siege_seed(kingdom: &Kingdom) -> u64 {
    let mut z = (kingdom.turn_count as u64)
        .wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ 0x5165_6765_0000_0002;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}



//! Ending a turn: driving `l2-kingdom`'s phase machine all the way round.
//!
//! `Turn_Tick` (`0x0049A010`) dispatches on `g_turnPhase` every frame and
//! `Turn_AdvancePhase` wraps 7 back to 1. `l2-kingdom` models that as
//! [`TurnMachine`](l2_kingdom::phase::TurnMachine) and deliberately stops
//! there: **three** of the seven phases wait on units moving, one waits on
//! sieges, one on the AI, and the machine answers none of them itself. The
//! caller does. This module is that caller.
//!
//! > The line above used to say *"four of the seven phases wait on units
//! > moving, and units are not that crate's state"*. Both halves have since
//! > stopped being true. Units **are** `l2-kingdom`'s state — `Campaign` lives
//! > inside `Kingdom` because two lockstep peers have to agree about where an
//! > army stands — and phase 2 waits on sieges rather than armies
//! > (`docs/decisions.md` C35). What is left on this side of the seam is not
//! > ownership of the units; it is the two things below that genuinely are not
//! > rules: what a phase's wait *means* to an application, and who fights a
//! > battle.
//!
//! # What the spine has to supply
//!
//! Three things, and none of them is in `l2-kingdom` because none of them is a
//! rule:
//!
//! 1. **The waits, and the mover they wait on.** This used to read:
//!
//!    > *"Phases 2, 3, 5 and 6 wait for armies, transports, peasant mobs and
//!    > merchants to stop moving. None of those exist yet, so they are settled
//!    > the moment they start, and `settled = true` says exactly that."*
//!
//!    They exist now. Four of the seven phases were no-ops, so nothing on the
//!    campaign map ever moved during a played turn: two units could never meet,
//!    a merchant never walked its route, an enemy army never trampled a
//!    resource site, and `disabled_seasons` never counted down. That is what
//!    this module now does, through [`l2_kingdom::units_tick`].
//!
//!    **The one thing that changed shape while doing it**: the mover is *not*
//!    inside a phase. `Units_Tick` is a sibling of `Turn_Tick` in the original's
//!    frame loop, called immediately after it and never reading `g_turnPhase`,
//!    so every unit that is walking takes a step on **every** tick of the turn.
//!    The phases only originate the game's own orders — transports in 3, mobs
//!    in 5, merchants in 6 — and then wait for them to stop. Phase 2
//!    originates nothing and waits on sieges, not armies; `docs/kingdom.md`
//!    §3.1 said otherwise and is corrected (`docs/decisions.md` C35).
//!
//! 2. **The battle.** When two enemy armies meet, `l2-kingdom` reports the pair
//!    and refuses to fight them — it cannot depend on `l2-sim` and must not.
//!    [`resolve_battle`] hands them to [`crate::engagement`], which is the only
//!    module in the workspace that depends on both. A played turn therefore
//!    produces a battle *result*: one of the two armies is gone, the county may
//!    have changed hands, and a levied defence has walked home.
//!
//! 3. **The AI's turn.** Phase 4 will not end until every realm's `aiStep`
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
//! what `tests/turn.rs` asserts by running one twice. The unit sweep keeps that
//! property because it walks slots 1 … 150 in order and stops at the first
//! battle — `docs/netcode.md` §5's rule, and the reason the stop is reproduced
//! rather than tidied away.

use l2_kingdom::ai::{self, AiStep};
use l2_kingdom::conquest::Attack;
use l2_kingdom::phase::{Phase, PhaseWait};
use l2_kingdom::units_tick::{Contact, Encounter, UnitsTick};
use l2_kingdom::victory::Outcome;
use l2_kingdom::{Kingdom, SeasonReport};

use crate::engagement::{self, Answer};
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
    /// `DAT_0053F0C4` after this turn: whether the game is now over, and how.
    /// [`Outcome::InPlay`] almost always. See [`crate::victory`].
    pub outcome: Outcome,
    /// How many tiles were entered by anything, over the whole turn. Zero on a
    /// turn where nobody had anywhere to go — and the number that was
    /// necessarily zero while four phases did nothing.
    pub steps: usize,
    /// Everything the unit sweep reported, in the order it happened. Battles,
    /// captures and blocked moves.
    pub contacts: Vec<Contact>,
    /// Battles that were raised and **not fought** — normally empty, because
    /// [`resolve_battle`] fights them. An entry here means
    /// [`crate::engagement::resolve`] declined the pair, which it does when
    /// either slot is no longer a unit. Reported rather than swallowed.
    pub pending_battles: Vec<Encounter>,
}

impl TurnOutcome {
    /// Counties that changed hands during the turn.
    pub fn captures(&self) -> impl Iterator<Item = (usize, u8)> + '_ {
        self.contacts.iter().filter_map(|c| match c {
            Contact::Castle { unit, county, outcome: l2_kingdom::conquest::Attack::Captured } => {
                Some((*unit, *county))
            }
            _ => None,
        })
    }
}

/// Run the phase machine until the end-of-season pipeline has run once.
///
/// Returns `None` only if [`MAX_TICKS`] is reached, which would be a bug in a
/// wait condition rather than something a player can cause.
///
/// # The shape of one tick
///
/// The original's frame loop is `Turn_Tick(); Units_Tick();` — two calls, in
/// that order, and this is them. `Turn_Tick` runs the phase's first-step work,
/// evaluates the wait and may advance; `Units_Tick` then steps every unit that
/// is walking, whatever phase that left current. Doing it the other way round
/// would let a unit ordered by a phase take its first step before the phase had
/// finished ordering the rest of them, which is a lockstep difference and not a
/// cosmetic one.
pub fn end_turn(game: &mut Game) -> Option<TurnOutcome> {
    for (id, realm) in game.kingdom.realms.iter().enumerate() {
        game.gold_last[id] = realm.gold;
    }

    let mut ticks = 0;
    let mut steps = 0;
    let mut contacts: Vec<Contact> = Vec::new();
    let mut pending_battles: Vec<Encounter> = Vec::new();
    // The AI's resource grant runs once a turn. See `run_handler`.
    let mut granted = false;
    while ticks < MAX_TICKS {
        ticks += 1;
        let phase = game.kingdom.turn.phase;
        // `step == 0` is the machine's own "this is the first call of this
        // phase", which is when the original's phase handlers kick off work.
        if game.kingdom.turn.step == 0 {
            begin_phase(game, phase);
        }
        if phase == Phase::PlayersTurn {
            drive_ai(&mut game.kingdom, &mut granted);
        }
        let (_, report) = game.kingdom.tick(settled(&game.kingdom, phase));

        // `Units_Tick`, immediately after `Turn_Tick` and outside the phase
        // machine entirely. See the module documentation.
        let moved = game.kingdom.tick_units();
        steps += moved.stepped;
        hand_off_battles(game, &moved, &mut pending_battles);
        contacts.extend(moved.contacts.iter().copied());

        if let Some(report) = report {
            game.turns_played += 1;
            game.last_report = Some(report.clone());
            // `Turn_Tick`'s phase 7 calls `Score_RankRealms` after
            // `Season_Advance` — one of its five callers, and the one that can
            // crown a survivor at the end of a season in which nobody died.
            // `Pass::ScoreRank` inside the pipeline has already ranked; this
            // adds the leader/trailer scan that the pass deliberately does not
            // own, because the pass does not know who the local player is.
            //
            // It runs *after* the unit sweep and the battles, which is the
            // order that matters now that a turn can destroy an army: a realm
            // whose last army died this turn is eliminated by this call rather
            // than surviving until the next one.
            game.rank_realms();
            let outcome = game.campaign.settle(game.player);
            return Some(TurnOutcome {
                report,
                ticks,
                outcome,
                steps,
                contacts,
                pending_battles,
            });
        }
    }
    None
}

/// Answer the current phase's wait.
///
/// Three of the four unit phases ask the unit array directly. Phase 2 asks
/// about **sieges**, which nothing in this build can create, so it is answered
/// `true` — and that is a real "there are none", not the old "none of this
/// exists". Phase 4's wait is the AI's and `Kingdom::tick` overrides whatever
/// is passed for it.
fn settled(kingdom: &Kingdom, phase: Phase) -> bool {
    match phase.wait() {
        PhaseWait::Units(kind) => !kingdom.units_moving(kind),
        // Sieges are out of scope. When they land this is the line that asks
        // `Siege_TickPhase` whether the cursor sweep came up empty.
        PhaseWait::Sieges => true,
        PhaseWait::Steps(_) | PhaseWait::AllRealmsDone | PhaseWait::Immediate => true,
    }
}

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
/// > change a test rather than pass either way. Both have landed, and the
/// > handoff is a real call now. `pending_battles` stays, because there is
/// > still one case that does not resolve: see below.
///
/// # The player is not asked, and that is a UI gap rather than a rule
///
/// `engagement::resolve` takes an [`Answer`] and consults it only when
/// [`l2_kingdom::battle::settlement`] returns `Prompt` — the original's *"will
/// you take the field?"*. `end_turn` has no screen to ask on, so it answers
/// [`Answer::Decline`], which is a real branch of the original and not an
/// invention: declining runs the autocalc and shows the report, so it is a way
/// out of *watching* the battle rather than out of fighting it. When the map
/// screen can raise the prompt, this is the one line that changes.
///
/// # The seed is state, never a clock
///
/// `docs/netcode.md`: two peers fight the same battle or they are not playing
/// the same game. The seed is mixed from the turn counter, the two unit slots
/// and the county — all of which both peers agree on — rather than drawn from
/// [`l2_kingdom::Kingdom::rng`], because drawing would advance the generator
/// the weather and the event deck share and make a turn with a battle in it
/// roll different weather from the same turn without one.
///
/// Returns the encounter **unfought** if it could not be resolved, which is
/// then reported rather than swallowed.
pub fn resolve_battle(game: &mut Game, encounter: Encounter) -> Option<Encounter> {
    let attack = Attack::Battle { attacker: encounter.mover, defender: encounter.occupant };
    let seed = battle_seed(&game.kingdom, encounter);
    match engagement::resolve(&mut game.kingdom, attack, encounter.county, Answer::Decline, seed) {
        Some(_) => None,
        None => Some(encounter),
    }
}

/// A battle's seed, from simulation state both peers already agree on.
///
/// Splitmix64's finaliser over a mix of the turn counter, the two slots and the
/// county. Any deterministic mix would do; what matters is that no term is a
/// clock, an address or an iteration order.
fn battle_seed(kingdom: &Kingdom, e: Encounter) -> u64 {
    let mut z = (kingdom.turn_count as u64)
        .wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ (e.mover as u64) << 32
        ^ (e.occupant as u64) << 8
        ^ e.county as u64;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

fn hand_off_battles(game: &mut Game, moved: &UnitsTick, pending: &mut Vec<Encounter>) {
    if let Some(e) = moved.battle() {
        if let Some(unfought) = resolve_battle(game, e) {
            pending.push(unfought);
        }
    }
}

/// The work a phase does on its first call.
///
/// * **Phase 1** — `docs/kingdom.md` §3.1 says the neutral counties get their tax
///   rates and their fields set once a turn, on the neutral ladder, and realm
///   **0** is how the original addresses them: `AI_SetTaxRates(0)` then
///   `AI_ManageFields(0)` — which is `0x0049DFC6`, the *unowned* counties' pass,
///   and not the AI realms' step 5. See `l2_kingdom::ai_farm`.
/// * **Phase 4** — `AI_RunTurnStep`'s **step 0** for every realm: recount its
///   strength, eliminate it if that comes out zero, and rank. See
///   [`Game::recount_realm`](crate::game::Game::recount_realm), and note that it
///   runs for the human too.
/// * **Phases 3, 5 and 6** originate the game's own move orders, and that is
///   `l2-kingdom`'s to do because it is a rule about units:
///   [`Kingdom::begin_unit_phase`] is the transport re-target, the peasant-mob
///   cursor and `Merchant_AdvanceAll`. **Phase 2 is sieges and originates
///   nothing** — see [`l2_kingdom::units_tick`].
///
/// # The one difference from the original, and the condition it rested on has
/// gone
///
/// `AI_RunTurnStep` interleaves: realm 1 takes step 0, then realm 2 takes step 0,
/// and by the time realm 5 reaches its own step 0 the earlier realms have taken
/// several of their fourteen. So a realm that realm 2's turn destroys is noticed
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
/// > What that costs is bounded and is not a divergence in the numbers: a realm
/// > eliminated by a battle inside phase 4 is recounted at the end of the turn
/// > by [`Game::rank_realms`](crate::game::Game::rank_realms) rather than
/// > mid-phase, so it is noticed one phase later than the original notices it.
/// > Nothing between the two reads the elimination. Interleaving the step 0s
/// > properly is the fix, and it belongs with whoever next touches the AI
/// > dispatcher.
fn begin_phase(game: &mut Game, phase: Phase) {
    game.kingdom.begin_unit_phase(phase);
    match phase {
        Phase::NeutralCounties => {
            game.kingdom.run_ai_tax_rates(0);
            // The unowned counties farm too, by whichever lord held them last.
            // `NoMarket` is the merchant seam: there is no stall yet, so every
            // style's opening shopping cascade is refused and the county farms
            // what it already has. See `l2_kingdom::ai_farm`.
            game.kingdom.run_neutral_farms(&mut l2_kingdom::ai_farm::NoMarket);
        }
        Phase::PlayersTurn => {
            for id in 1..l2_kingdom::realm::MAX_REALMS {
                game.recount_realm(id as u8);
            }
        }
        _ => {}
    }
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
/// > no armies". There are armies now, and the argument is the real predicate:
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
            // On the England turn-one scenario the difficulty is 0 and every grant
            // multiplies by it, so nothing moves either way; the distinction
            // matters the moment somebody starts a harder game.
            if !*granted {
                kingdom.run_ai_grants();
                *granted = true;
            }
        }
        AiStep::ManageFields => {
            kingdom.run_ai_farms(realm, &mut l2_kingdom::ai_farm::NoMarket);
        }
        AiStep::BuildCastles => {
            kingdom.run_ai_castles(realm);
        }
        AiStep::ChooseIndustry => kingdom.run_ai_industry(realm),
        // The letters are dropped rather than shown: a realm-to-realm taunt is
        // not a season message and `l2-game` has no letter screen yet. The
        // realm's timer, stage and voice rotation still advance, which is the
        // whole of the step's effect on the simulation.
        AiStep::Taunt => {
            kingdom.run_ai_taunt(realm);
        }
        AiStep::UpdateTotals => update_totals(kingdom, realm),
        // An empty function in the shipped binary. Named, and it does nothing
        // here for the same reason it does nothing there.
        AiStep::Nothing => {}
        _ => {}
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


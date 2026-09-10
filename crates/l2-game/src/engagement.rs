//! **The seam.** An army reaches an enemy county, a battle is fought, and the
//! result comes back onto the campaign map.
//!
//! Two crates that had never met. `l2-kingdom` takes counties by arithmetic and
//! *reports* the battle case — [`l2_kingdom::conquest::Attack::Battle`] names
//! two unit slots and stops. `l2-sim` fights battles and has never been told
//! where an army comes from. This module is the only place in the workspace
//! that can join them, because it is the only crate that depends on both, and
//! `docs/plan.md`'s one-way dependency rule says it must stay that way: neither
//! simulation learns the other exists.
//!
//! # The whole path, and where each piece lives
//!
//! ```text
//! movement::march              l2-kingdom  the army walks
//! conquest::attack_county      l2-kingdom  a defender is found or levied
//! battle::settlement           l2-kingdom  autocalc? a report? or ask?
//!   ├── battle::auto_resolve   l2-kingdom  strength, a ratio, a percentage
//!   └── fight (here)           l2-sim      figures, cells, ticks, casualties
//! battle::return_to_campaign   l2-kingdom  the county, the moves, the loser
//! battle::disband_defence      l2-kingdom  the levy walks home
//! ```
//!
//! **Only one of those seven steps needs `l2-sim` at all**, which is why the
//! other six are in `l2-kingdom` where the campaign can reach them without a
//! battle simulation present. An AI-versus-AI war runs entirely without this
//! module.
//!
//! # What the original decides, and what we decide
//!
//! `FUN_004A6A30` chooses between three settlements and this module honours it
//! exactly — see [`l2_kingdom::battle::Settlement`]. What the original does
//! *inside* a fought battle that we do not:
//!
//! * **The battlefield is blank.** `Battlefield_BuildRandom` (`0x004AAA3`)
//!   builds one from the campaign tile the armies are standing on and is
//!   `[I]`-level unread; [`l2_sim::runner::blank_field`] is used instead, so
//!   terrain plays no part yet.
//! * **Nobody clicks.** A human side gets no AI order handler in the original
//!   either, so a battle with an unattended player would stand still forever.
//!   [`fight`] issues the one order a player always issues — every unit at the
//!   enemy's end of the field — and lets the AI side think for itself.
//!
//! > This list used to carry a fourth entry: *"Missiles do not fly. `l2-sim`
//! > resolves a missile hit but nothing drives reload and flight, so an army of
//! > archers fights as an army of men with bows they do not use. This is the
//! > single largest reason a fought battle here and a fought battle there would
//! > not agree."* It was, and it is closed. `l2-sim` flies them, and the
//! > measurement that said so was the fixture: the same position that was won
//! > by the player with 56 men of 178 is now lost by him, which is what the
//! > saved game records. `tests/seam.rs` asserts the verdict rather than
//! > excusing it.
//! * **Mercenaries lose their band.** `FUN_0047F474` tells a mercenary figure
//!   from a levied one by a flag on the figure record; `l2_sim::Figure` has no
//!   such flag, so a band that goes into a fought battle comes out folded into
//!   its troop type. The autocalc path scales the band correctly.

use l2_kingdom::battle::{self, Aftermath, Settlement, Verdict};
use l2_kingdom::conquest::Attack;
use l2_kingdom::kingdom::Kingdom;
use l2_kingdom::unit::TROOP_TYPES;
use l2_sim::runner::{blank_field, BattleRunner, Muster};
use l2_sim::{End, Troop, SIDE_A, SIDE_B};

/// How the battle was actually settled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Resolution {
    /// `FUN_004AAD07`: strength scores, a ratio and a survival percentage.
    Autocalc,
    /// `l2-sim` ran it to a conclusion — [`BattleRunner::conclusion`], which is
    /// `FUN_00477DFC`'s field arms.
    Fought { ticks: u32, cause: End },
    /// `l2-sim` ran it and it was **still going** at [`MAX_TICKS`], so the
    /// winner was settled on the men left standing.
    ///
    /// This is not an outcome the original has. A field battle there ends only
    /// by annihilation or withdrawal and has no clock at all, so reaching this
    /// means our simulation stalled — two sides that cannot find each other, or
    /// a melee that cannot resolve. It is a distinct variant rather than a flag
    /// precisely so that a caller cannot mistake it for a real result, and so a
    /// test can assert it never happens.
    Stalled { ticks: u32 },
}

/// How long a fought battle may run before we give up on it.
///
/// **The original has no such limit.** `FUN_00477DFC` ends a field battle on
/// one condition — a side's men reaching zero — or on a withdrawal, and it
/// waits as long as that takes. This exists because our simulation can stall
/// where the original would not, and [`Resolution::Stalled`] is how it says so
/// rather than quietly inventing a winner. Twelve thousand ticks is several
/// times the longest battle `l2-sim`'s own tests produce.
pub const MAX_TICKS: u32 = 12_000;

/// Ticks between checks that the battle is over. One thought-cycle of the
/// battle AI is 200 frames, so a hundred is fine grain.
const CHECK_EVERY: u32 = 100;

/// Everything one battle did, from the prompt to the county.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BattleReport {
    /// Which of the original's three settlements this battle qualified for.
    pub settlement: Settlement,
    /// How it was in fact settled — a [`Settlement::Prompt`] the player
    /// declined is [`Resolution::Autocalc`].
    pub resolution: Resolution,
    pub verdict: Verdict,
    pub aftermath: Aftermath,
    /// Men a dissolved county defence walked back into its county.
    pub defenders_returned: i32,
    /// The two armies' men before and after, for a caller drawing the
    /// after-battle roster (screen `0x13` draws exactly this).
    pub attacker_men: (i32, i32),
    pub defender_men: (i32, i32),
    /// The county the battle was fought in — the besieged one for an assault.
    pub county: u8,
    /// Whether this was a siege assault. Three of `L2.eng` group 82's seven
    /// banners are unreachable without it, and so is the branch in
    /// [`battle::return_to_campaign`].
    pub is_siege: bool,
    /// The two realms, **captured before the loser was destroyed**.
    ///
    /// [`Verdict`] names two unit *slots* and [`battle::return_to_campaign`]
    /// empties the loser's, so by the time a screen reads this report the only
    /// place the loser's realm still exists is here. It is what
    /// [`BattleReport::outcome`] needs, and without it three of the seven
    /// banners cannot be chosen.
    pub attacker_owner: u8,
    pub defender_owner: u8,
    /// **The two rosters, before and after** — the seven troop counts each side
    /// took onto the field and the seven it brought off.
    ///
    /// `FUN_004224E7` is the painter both battle screens share, and mode 1 —
    /// the one screen `0x13` passes — prints the *before* count in parentheses
    /// beside the *after* count on every one of its seven rows. It reads the
    /// before counts out of `DAT_00568420` / `DAT_0056843C`, two arrays screen
    /// `0x12` filled on its way past. This is those two arrays, kept on the
    /// report rather than in a global, because the losing record is gone by the
    /// time anybody draws them.
    pub attacker_roster: (Roster, Roster),
    pub defender_roster: (Roster, Roster),
    /// **What the assault did to the castle** — the six numbers
    /// `Siege_RecordCastleDamage` (`0x004784CA`) bills the repair from, kept on
    /// the report so a screen can say *"the walls are breached"* without
    /// re-reading a county that may since have changed hands.
    ///
    /// All zeroes for a field battle, and all zeroes for a siege that was
    /// **calculated** rather than fought: the accumulators only move while men
    /// are shovelling and shot is landing.
    pub castle_damage: l2_sim::CastleDamage,
}

/// One army's seven campaign troop counts — peasant, crossbowman, maceman,
/// swordsman, pikeman, archer, knight, in the order every table in the game
/// agrees on.
pub type Roster = [i32; TROOP_TYPES];

impl BattleReport {
    /// The realm that held the field.
    pub fn winner_owner(&self) -> u8 {
        if self.verdict.attacker_won {
            self.attacker_owner
        } else {
            self.defender_owner
        }
    }

    /// The realm that did not.
    pub fn loser_owner(&self) -> u8 {
        if self.verdict.attacker_won {
            self.defender_owner
        } else {
            self.attacker_owner
        }
    }

    /// **Which of `L2.eng` group 82's seven heading/body pairs this battle
    /// draws** for `local_player` — [`l2_kingdom::battle::outcome`], which is
    /// `FUN_00478419`.
    ///
    /// It is a method rather than a field because the answer depends on who is
    /// looking: the same battle is *won* to one peer, *lost* to the other and
    /// [`Outcome::Bystander`](l2_kingdom::battle::Outcome::Bystander) to a
    /// third. A field on the report would have to pick one of them, and in a
    /// lockstep game every peer holds the same report.
    pub fn outcome(&self, local_player: u8) -> l2_kingdom::battle::Outcome {
        battle::outcome(
            self.verdict,
            self.is_siege,
            local_player,
            self.winner_owner(),
            self.loser_owner(),
        )
    }
}

/// Whether the player took the field, when they were asked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Answer {
    /// *"Will you take the field?"* — yes. `Battle_Start`.
    TakeTheField,
    /// No. `FUN_0043B622` runs the autocalc and shows the report, so declining
    /// is not a way out of the battle: it is a way out of *watching* it.
    Decline,
}

/// **Resolve a battle the campaign has just produced, end to end.**
///
/// `attack` is what [`l2_kingdom::conquest::attack_county`] returned; anything
/// but [`Attack::Battle`] yields `None`, because nothing was fought.
///
/// `answer` is consulted only when [`battle::settlement`] returns
/// [`Settlement::Prompt`] — the other two settlements never reach a screen and
/// the player is not asked. `seed` seeds the battle AI's generator and must be
/// derived from simulation state, never from a clock: two lockstep peers fight
/// the same battle or they are not playing the same game.
///
/// `g_optFightHumansOnly` comes from `kingdom.options`. It was a parameter here
/// while the save and the battle seam were being written in parallel branches —
/// adding a field to [`l2_kingdom::kingdom::Options`] means adding it to the
/// save and to the lockstep checksum, and neither branch could do that to the
/// other's file. Both have landed, so it lives where it belongs.
pub fn resolve(
    kingdom: &mut Kingdom,
    attack: Attack,
    county: u8,
    answer: Answer,
    seed: u64,
) -> Option<BattleReport> {
    let Attack::Battle { attacker, defender } = attack else { return None };
    resolve_battle(kingdom, attacker, defender, county, None, answer, seed, None)
}

/// **Settle a battle the player has already watched.**
///
/// [`resolve`] runs the whole battle between two statements; this takes one that
/// a [`crate::battlefield::LiveBattle`] has been stepping a tick at a time and
/// does the rest — the write-back, the verdict and the aftermath — from exactly
/// where it stopped. Everything after the fight is the same code, which is the
/// point: a played battle and a headless one differ in *who supplied the ticks*
/// and in nothing else.
pub fn resolve_fought(
    kingdom: &mut Kingdom,
    attacker: usize,
    defender: usize,
    county: u8,
    castle_level: Option<u8>,
    seed: u64,
    runner: BattleRunner,
) -> Option<BattleReport> {
    resolve_battle(
        kingdom,
        attacker,
        defender,
        county,
        castle_level,
        Answer::TakeTheField,
        seed,
        Some(runner),
    )
}

/// **Resolve a siege assault** — [`l2_kingdom::siege::assault`]'s
/// [`Assault::Battle`](l2_kingdom::siege::Assault::Battle), fought or
/// auto-resolved.
///
/// The only differences from [`resolve`] are the three the original makes, and
/// each of them is one argument:
///
/// * the **castle level** goes to [`battle::auto_resolve`], which multiplies
///   the *defender's* strength by [`battle::CASTLE_STRENGTH_PERCENT`]. The
///   table has been in that function since the seam landed with nothing to
///   pass it; this is what passes it.
/// * `Army_PrepareForBattle` fills the four battle-only troop slots — the
///   besieger's engines and the garrison's oil — and they go into the muster
///   rather than into the campaign record, because the original zeroes them
///   again the instant the battle ends.
/// * `g_battleIsSiege` reaches [`battle::return_to_campaign`], where it decides
///   which half of the siege link is cleared, and reaches
///   [`battle::outcome`], where it picks four of the seven `L2.eng` group 82
///   banners.
///
/// **The `county` is the besieged one**, taken from the besieger's own
/// `besieging_county`, exactly as `Siege_LaunchAssault` takes it.
pub fn resolve_siege(
    kingdom: &mut Kingdom,
    assault: l2_kingdom::siege::Assault,
    answer: Answer,
    seed: u64,
) -> Option<BattleReport> {
    let l2_kingdom::siege::Assault::Battle { attacker, defender, castle_level } = assault else {
        return None;
    };
    let county = kingdom.campaign.units.get(attacker)?.besieging_county;
    resolve_battle(kingdom, attacker, defender, county, Some(castle_level), answer, seed, None)
}

/// **Turn phase 2, end to end** — `Turn_Tick`'s `g_turnPhase == 2` arm.
///
/// ```c
/// if (step == 1) Siege_StartPhase();
/// if (step % 100 == 2) {
///     if (Siege_TickPhase() == 0) Turn_AdvancePhase();
///     else                        Siege_LaunchAssault(g_siegeCursor);
/// }
/// ```
///
/// **Phase 2 is sieges and nothing else.** It is called *"army movement"* in
/// `docs/kingdom.md` §3.1 and there is no movement in it: `Siege_StartPhase`,
/// a cursor, and the assaults it yields. Armies move in phase 4, under whoever
/// is driving that realm.
///
/// The cursor is deliberately not advanced past an army that assaulted: the
/// assault clears the siege link one way or another, so the next
/// [`l2_kingdom::siege::build_tick`] on that slot reports nothing and the
/// cursor moves on by itself. Reproduced, and it is what makes the loop
/// terminate.
///
/// `answer` decides what a human does when asked to take the field, and `seed`
/// must come from simulation state — a battle is lockstep state like any other.
/// Returns one report per assault fought, in cursor order.
pub fn run_siege_phase(kingdom: &mut Kingdom, answer: Answer, seed: u64) -> Vec<BattleReport> {
    let mut phase = SiegePhase::begin(kingdom);
    let mut reports = Vec::new();
    while let Some(assault) = phase.next(kingdom) {
        if let Some(report) = phase.settle(kingdom, assault, answer, seed) {
            reports.push(report);
        }
    }
    reports
}

/// **Turn phase 2, one assault at a time** — the pump [`run_siege_phase`] runs
/// to exhaustion in a loop, exposed so a caller that has a screen can stop
/// between two assaults and ask the player about each.
///
/// It is the same code either way: `run_siege_phase` *is* a `while let` over
/// this, so the two cannot drift apart and the phase's tests cover both.
///
/// **[`SiegePhase::next`] has already launched the assault it hands back.**
/// `Siege_LaunchAssault` breaks the siege link whichever way the assault goes,
/// so a caller that takes a value from `next` and never settles it has lifted a
/// siege and fought nothing. The pair is not optional.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SiegePhase {
    cursor: l2_kingdom::siege::SiegeCursor,
    /// Which assault of this phase the next one is, mixed into its seed so two
    /// assaults in one phase do not fight the same battle.
    round: usize,
}

impl SiegePhase {
    /// `Siege_StartPhase`: break the pairs that no longer agree and seed the
    /// cursor.
    pub fn begin(kingdom: &mut Kingdom) -> SiegePhase {
        let Kingdom { counties, campaign, .. } = kingdom;
        SiegePhase { cursor: l2_kingdom::siege::start_phase(counties, &mut campaign.units), round: 0 }
    }

    /// How many live besieging armies the phase started with — `g_siegeCount`.
    pub fn count(&self) -> u32 {
        self.cursor.count
    }

    /// The next assault, or `None` once the cursor has run off the end.
    ///
    /// The cursor only ever advances, so a loop over this cannot spin: at worst
    /// it looks at every unit slot once and yields at most one assault a slot.
    pub fn next(&mut self, kingdom: &mut Kingdom) -> Option<l2_kingdom::siege::Assault> {
        if self.round > l2_kingdom::MAX_UNITS {
            return None;
        }
        let army = {
            let Kingdom { campaign, .. } = kingdom;
            l2_kingdom::siege::tick_phase(&mut self.cursor, &mut campaign.units)?
        };
        let Kingdom { counties, campaign, .. } = kingdom;
        Some(l2_kingdom::siege::assault(counties, &mut campaign.units, army))
    }

    /// Settle what [`SiegePhase::next`] handed back, and advance the cursor
    /// past it.
    ///
    /// A refusal is not a battle: message `0x119`, the siege is lifted, and
    /// `siege::assault` has already done it — so this answers `None`.
    ///
    /// Whatever happened, the slot must not be looked at again with the same
    /// state: the original relies on the link being gone, and an assault that
    /// was refused has had it broken too. That is the `cursor.at += 1`, and it
    /// is what makes the phase terminate.
    pub fn settle(
        &mut self,
        kingdom: &mut Kingdom,
        assault: l2_kingdom::siege::Assault,
        answer: Answer,
        seed: u64,
    ) -> Option<BattleReport> {
        let report = resolve_siege(kingdom, assault, answer, seed.wrapping_add(self.round as u64));
        self.cursor.at += 1;
        self.round += 1;
        report
    }

    /// Whether the assault `next` handed back is one a human is in — the
    /// question [`l2_kingdom::battle::settlement`] answers, asked before the
    /// battle rather than inside it.
    pub fn settlement(
        kingdom: &Kingdom,
        assault: l2_kingdom::siege::Assault,
    ) -> Option<(usize, usize, Settlement)> {
        let l2_kingdom::siege::Assault::Battle { attacker, defender, .. } = assault else {
            return None;
        };
        let s = battle::settlement(
            &kingdom.campaign.units,
            attacker,
            defender,
            kingdom.options.fight_humans_only_byte,
        );
        Some((attacker, defender, s))
    }
}

#[allow(clippy::too_many_arguments)]
fn resolve_battle(
    kingdom: &mut Kingdom,
    attacker: usize,
    defender: usize,
    county: u8,
    castle_level: Option<u8>,
    answer: Answer,
    seed: u64,
    fought: Option<BattleRunner>,
) -> Option<BattleReport> {
    let before = |k: &Kingdom, id: usize| k.campaign.units.get(id).map_or(0, |u| u.men);
    let (attacker_before, defender_before) = (before(kingdom, attacker), before(kingdom, defender));
    // Read now, not later: `return_to_campaign` destroys the loser's record, so
    // after it there is no realm to read off the losing slot at all.
    let owner_of = |k: &Kingdom, id: usize| k.campaign.units.get(id).map_or(0, |u| u.owner);
    let (attacker_owner, defender_owner) = (owner_of(kingdom, attacker), owner_of(kingdom, defender));
    let roster = |k: &Kingdom, id: usize| k.campaign.units.get(id).map_or([0; TROOP_TYPES], |u| u.troops);
    let (a_before, d_before) = (roster(kingdom, attacker), roster(kingdom, defender));

    let settlement = battle::settlement(
        &kingdom.campaign.units,
        attacker,
        defender,
        kingdom.options.fight_humans_only_byte,
    );
    let take_the_field = settlement == Settlement::Prompt && answer == Answer::TakeTheField;

    // **The repair bill, read before the runner is consumed.**
    // `Siege_RecordCastleDamage` (`0x004784CA`) is billed from the battle's own
    // accumulators, and [`conclude_fight`] takes the runner by value, so the six
    // numbers are lifted off it here.
    let mut castle_damage = fought.as_ref().map(|r| r.castle_damage()).unwrap_or_default();

    let (verdict, resolution) = if let Some(runner) = fought {
        // **The player watched it.** Everything after this point is the same
        // code the headless path runs; only the ticks came from somewhere else.
        conclude_fight(kingdom, attacker, defender, runner)
    } else if take_the_field {
        let (runner, verdict, resolution) =
            fight(kingdom, attacker, defender, castle_level, seed)?;
        castle_damage = runner;
        (verdict, resolution)
    } else {
        let verdict =
            battle::auto_resolve(&mut kingdom.campaign.units, attacker, defender, castle_level)?;
        (verdict, Resolution::Autocalc)
    };

    let attacker_after = before(kingdom, attacker);
    let defender_after = before(kingdom, defender);
    // Read **before** `return_to_campaign` empties the loser's record — after
    // it, the losing roster is seven zeros however the battle went, and the
    // screen would be unable to tell a wiped army from a missing one.
    let (a_after, d_after) = (roster(kingdom, attacker), roster(kingdom, defender));

    // `g_battleWithdrawal` is raised in exactly one place in the original —
    // `UnitOrder_SiegeAttKnight` — and `l2-sim` reports it the same way, as the
    // cause of the conclusion. Nothing else can set it, which is the whole
    // point of C31.
    let withdrawal = matches!(resolution, Resolution::Fought { cause: End::Withdrawal, .. });

    // **Bill the repair, before the county can change hands.**
    // `Siege_RecordCastleDamage` (`0x004784CA`) has exactly **one** caller and
    // it is not `Battle_ReturnToCampaign`, whatever `docs/symbols.md` says —
    // it is `FUN_004782C5`, the outcome banner's frame counter, at the moment
    // the 5,000 frames run out:
    //
    // ```c
    // if (screen == '+' && ++DAT_00568470 > 5000) {
    //     if (!skirmish && !multiplayer) Siege_RecordCastleDamage();
    //     if (choiceOwner == 1 && !skirmish) Battle_WriteBackCasualties();
    //     if (!skirmish) Battle_ReturnToCampaign(1);
    // }
    // ```
    //
    // Three rules come out of that call site and each of them is visible:
    //
    // * **Only a battle somebody watched to its end bills a repair.**
    //   `Battle_Decline` and the retreat/autocalc button both leave for the
    //   report screen without ever reaching this counter, so a player who
    //   knocks a wall down and then presses Autocalc un-knocks it down — the
    //   same un-doing the casualty write-back suffers on that path, and for the
    //   same reason.
    // * **It runs before the write-back and before the return**, so the county
    //   is billed while it still belongs to the defender and the *conqueror*
    //   inherits both the wreck and the bill. That ordering is why this block
    //   sits above `return_to_campaign` rather than below it.
    // * **`g_multiplayer` skips it entirely**, which cannot be right and is not
    //   reproduced: a peer that billed and a peer that did not would hold
    //   different counties. `docs/netcode.md` — the original's sync is the
    //   defect being replaced, not a model.
    if let Some(level) = castle_level {
        if let Some(c) = kingdom.counties.get_mut(county as usize) {
            l2_kingdom::siege::record_castle_damage(c, level, to_scars(castle_damage));
        }
    }

    let aftermath = {
        let Kingdom { counties, realms, campaign, options, tables, .. } = kingdom;
        battle::return_to_campaign(
            tables,
            counties,
            realms,
            &mut campaign.units,
            &mut campaign.names,
            verdict,
            county,
            castle_level.is_some(),
            withdrawal,
            options.difficulty,
        )
    };

    // **`Realm_RecountStrength` (`0x0049B42B`) on the loser's realm** is the
    // last thing both of `Battle_ReturnToCampaign`'s branches do, and it is
    // *not* here: it needs the county count and the local player, and neither
    // is a battle rule. [`Aftermath::loser_owner`] carries the argument out to
    // [`crate::turn`], which has a [`crate::game::Game`] and does it — the same
    // division as the diplomatic offence, which `l2-kingdom` reports rather
    // than applies.

    // A loser that **withdrew** is still standing, and it walked off having
    // paid `Army_WithdrawCasualties`. The two `after` numbers above were read
    // before that charge, so re-read the survivor: everything else is either
    // untouched or a record that no longer exists.
    let (attacker_after, defender_after, a_after, d_after) =
        if aftermath.withdrawal_casualties.is_some() && aftermath.loser_siege_lifted {
            (
                before(kingdom, attacker),
                before(kingdom, defender),
                roster(kingdom, attacker),
                roster(kingdom, defender),
            )
        } else {
            (attacker_after, defender_after, a_after, d_after)
        };

    let Kingdom { counties, realms, campaign, options, tables, .. } = kingdom;
    // `Defence_Disband` runs **after** the return, at the end of screen `0x13`
    // and immediately after `Battle_ReturnToCampaign(0)` on the silent path. A
    // defence that lost has already been destroyed and this finds nothing; a
    // defence that won is still standing and walks home.
    let defenders_returned = battle::disband_defence(
        tables,
        counties,
        realms,
        &mut campaign.units,
        &mut campaign.names,
        defender,
        options.difficulty,
    );

    // `Diplo_Offend(loserOwner, winnerOwner, 20)` — `Battle_ReturnToCampaign`
    // (`0x004AB383`) calls it inline and `battle::return_to_campaign` reports
    // it instead, because `l2-kingdom` had no diplomacy layer when that was
    // written. It has one now, and this is where the report is spent.
    //
    // **Losing a battle is the single largest thing that moves an AI's
    // opinion**, and until this line existed nothing in a played game moved one
    // at all: forty turns of England left every AI-to-AI standing saturated at
    // +30 by `AI_Diplomacy`'s heal, with no war target anywhere on the map.
    if let Some((loser, winner)) = aftermath.offence {
        l2_kingdom::diplomacy::offend(realms, loser, winner, battle::BATTLE_OFFENCE as i8);
    }

    Some(BattleReport {
        settlement,
        resolution,
        verdict,
        aftermath,
        defenders_returned,
        attacker_men: (attacker_before, attacker_after),
        defender_men: (defender_before, defender_after),
        county,
        is_siege: castle_level.is_some(),
        attacker_owner,
        defender_owner,
        attacker_roster: (a_before, a_after),
        defender_roster: (d_before, d_after),
        castle_damage,
    })
}

/// **The one place the two simulations' castle records meet.**
///
/// `l2_sim::CastleDamage` and `l2_kingdom::siege::SiegeScars` are the same six
/// county fields written twice, in two crates that must not see each other —
/// `docs/plan.md`'s one-way rule. This module is the only crate that depends on
/// both, so this is the only place the conversion can live, and it is a
/// field-for-field copy so that a reader can check it at a glance.
fn to_scars(d: l2_sim::CastleDamage) -> l2_kingdom::siege::SiegeScars {
    l2_kingdom::siege::SiegeScars {
        moat_filled: d.moat_filled,
        wall_damage: d.wall_damage,
        breach_score: d.breach_score,
        approach_score: d.approach_score,
        ramparts_breached: d.ramparts_breached,
        gate_open: d.gate_open,
    }
}

/// The other direction — [`l2_kingdom::siege::scars_for_assault`]'s answer, put
/// back into a battle that is opening on the same castle.
fn from_scars(s: l2_kingdom::siege::SiegeScars) -> l2_sim::CastleDamage {
    l2_sim::CastleDamage {
        moat_filled: s.moat_filled,
        wall_damage: s.wall_damage,
        breach_score: s.breach_score,
        approach_score: s.approach_score,
        ramparts_breached: s.ramparts_breached,
        gate_open: s.gate_open,
    }
}

/// **Raise both campaign records into `l2-sim`, run the battle, and write the
/// survivors back.**
///
/// `Battle_InitArmies` raises army A with side 4 and army B with side 0, and
/// the attacker is A at every one of the original's call sites — so the
/// *defender* is side 0, the side that deploys at the `0x04` marker.
/// [`BattleRunner::deploy_muster`] holds that convention and picks the
/// men-per-figure scale from the two totals.
///
/// The battle ends where `FUN_00477DFC` ends it — one side's men reaching
/// zero, or a withdrawal — and only then is the result written back.
/// **`FUN_0047F474` runs on the far side of the outcome banner's 5,000-tick
/// settle**, not at the moment the battle is decided; nothing changes while it
/// counts, so the settle is skipped here and
/// [`l2_sim::runner::SETTLE_TICKS`] carries the number for a caller that is
/// pacing a screen.
///
/// Writing back is that function: both records' eleven counts are zeroed and
/// rebuilt from the surviving figures, and the total follows the counts rather
/// than being scaled.
fn fight(
    kingdom: &mut Kingdom,
    attacker: usize,
    defender: usize,
    castle_level: Option<u8>,
    seed: u64,
) -> Option<(l2_sim::CastleDamage, Verdict, Resolution)> {
    let mut runner = begin_fight(kingdom, attacker, defender, castle_level, seed)?;
    // The original's frame loop asks `FUN_00477DFC` every frame; asking every
    // hundredth costs at most ninety-nine ticks of a battle that is already
    // over, and no rule reads the tick count.
    while runner.tick < MAX_TICKS {
        runner.run(CHECK_EVERY);
        if runner.conclusion().is_some() {
            break;
        }
    }
    let damage = runner.castle_damage();
    let (verdict, resolution) = conclude_fight(kingdom, attacker, defender, runner);
    Some((damage, verdict, resolution))
}

/// **Raise the battle and stop**, so that somebody else can supply the ticks.
///
/// This is `Battle_Start` (`0x004778A0`) minus the screen: the two musters, the
/// battlefield, and the one order a human side always issues on the first frame
/// because `Battle_UpdateAllUnits` runs no handler for it.
///
/// A battle a player watches and a battle nobody watches begin **here, in the
/// same call, with the same seed**, which is what makes it possible to assert
/// that giving no orders reproduces the headless verdict exactly.
pub fn begin_fight(
    kingdom: &mut Kingdom,
    attacker: usize,
    defender: usize,
    castle_level: Option<u8>,
    seed: u64,
) -> Option<BattleRunner> {
    // `Army_PrepareForBattle` — the four battle-only troop slots, produced here
    // rather than stored, because the original zeroes them again the moment the
    // battle is over (`Army_ClearBattleSlots`).
    let (a_extra, d_extra) = match castle_level {
        Some(level) => (
            l2_kingdom::siege::prepare_besieger(kingdom.campaign.units.get(attacker)?),
            l2_kingdom::siege::prepare_garrison(level),
        ),
        None => Default::default(),
    };
    let a_troops = muster_with(kingdom, attacker, a_extra)?;
    let d_troops = muster_with(kingdom, defender, d_extra)?;
    let (a_owner, a_human) = {
        let u = kingdom.campaign.units.get(attacker)?;
        (u.owner, u.owner_is_human)
    };
    let (d_owner, d_human) = {
        let u = kingdom.campaign.units.get(defender)?;
        (u.owner, u.owner_is_human)
    };

    let a = Muster { troops: &a_troops, owner: a_owner, human: a_human };
    let d = Muster { troops: &d_troops, owner: d_owner, human: d_human };
    let mut runner = match castle_level {
        // **The castle layout is ours, not the original's**, and
        // `l2_sim::siege::our_castle` says so in its name. See its module
        // header: `Battlefield_BuildCastle` reads a raster we have not read.
        Some(level) => BattleRunner::deploy_siege(
            l2_sim::siege::our_castle(level),
            seed,
            a,
            d,
            level,
        ),
        None => BattleRunner::deploy_muster(blank_field(), seed, a, d),
    };

    // **What the last siege on this castle left.** `FUN_004787A4` is the last
    // statement but one of `Battlefield_BuildCastle`, so it runs *after* the
    // fresh scores and overwrites them — which is why this sits below
    // `deploy_siege` rather than being an argument to it.
    if castle_level.is_some() {
        let besieged = kingdom.campaign.units.get(attacker)?.besieging_county;
        if let Some(c) = kingdom.counties.get_mut(besieged as usize) {
            let scars = l2_kingdom::siege::scars_for_assault(c);
            runner.restore_castle_damage(from_scars(scars));
        }
    }

    // A human side gets no order handler — `Battle_UpdateAllUnits` guards on
    // it — so without this it stands where it deployed until the other side
    // walks into it. This is the click a player makes on the first frame.
    for (side, human) in [(SIDE_B, a_human), (SIDE_A, d_human)] {
        if human {
            let enemy = runner.home(l2_sim::runner::other_side(side));
            runner.order_side(side, enemy.0, enemy.1);
        }
    }

    Some(runner)
}

/// **The tail of a fought battle**: the write-back and the verdict.
///
/// `FUN_0047F474` rebuilds both campaign records from the figures still
/// standing, and the winner is the side the conclusion names. The stall arm is
/// ours — the original has no clock — and it is a separate [`Resolution`]
/// variant so that nobody can mistake an invented winner for a won battle.
pub fn conclude_fight(
    kingdom: &mut Kingdom,
    attacker: usize,
    defender: usize,
    runner: BattleRunner,
) -> (Verdict, Resolution) {
    let conclusion = runner.conclusion();

    // `FUN_0047F474` — the write-back. Both sides, all eleven slots, rebuilt
    // from what is still standing.
    write_back(kingdom, attacker, runner.survivors(SIDE_B));
    write_back(kingdom, defender, runner.survivors(SIDE_A));

    // Army A is side 4 and army B is side 0 — `Battle_InitArmies`.
    let (winner_side, resolution) = match conclusion {
        Some(c) => (c.winner, Resolution::Fought { ticks: runner.tick, cause: c.cause }),
        // Our stall, not the original's. The larger force holds the field; the
        // variant says the number was invented rather than won.
        None => {
            let side = if runner.men_of_side(SIDE_B) > runner.men_of_side(SIDE_A) {
                SIDE_B
            } else {
                SIDE_A
            };
            (side, Resolution::Stalled { ticks: runner.tick })
        }
    };
    let verdict = if winner_side == SIDE_B {
        Verdict::a_won(attacker, defender)
    } else {
        Verdict::b_won(attacker, defender)
    };
    (verdict, resolution)
}

/// A campaign record's seven counts as `l2-sim` troops, plus its mercenary band
/// folded into its own troop type.
///
/// The eleven-column order is shared: `docs/battle.md` §4.1 has the campaign
/// record, `TROOPS*.ENG` and the `.skr` army record all agreeing on
/// peasant, crossbowman, maceman, swordsman, pikeman, archer, knight — which is
/// [`TroopType`]'s order and [`Troop`]'s alike. The four battle-only slots
/// (`+0x16C + t*2` for `t` 7…10) hold siege engines and oil and are filled by
/// `Army_PrepareForBattle` on the siege path only, which is out of scope.
#[cfg(test)]
fn muster_of(kingdom: &Kingdom, id: usize) -> Option<Vec<(Troop, u32)>> {
    muster_with(kingdom, id, l2_kingdom::siege::BattleEngines::default())
}

/// The same, plus the four battle-only slots a siege fills. `extra` is empty
/// for a field battle, so the two paths are one function.
fn muster_with(
    kingdom: &Kingdom,
    id: usize,
    extra: l2_kingdom::siege::BattleEngines,
) -> Option<Vec<(Troop, u32)>> {
    let u = kingdom.campaign.units.get(id)?;
    let mut counts = [0u32; 11];
    for (slot, men) in counts.iter_mut().zip(u.troops.iter()) {
        *slot = (*men).max(0) as u32;
    }
    if let Some(band) = u.mercenaries {
        counts[band.troop.index()] += band.men().max(0) as u32;
    }
    // Troop types 7…10. **These are counts of engines, not of men**: one
    // catapult is one figure whatever the men-per-figure scale, and
    // `BattleRunner::raise_men` multiplies them back up for exactly that
    // reason.
    for (slot, count) in extra.counts().into_iter().enumerate() {
        counts[7 + slot] = count.max(0) as u32;
    }
    debug_assert_eq!(TROOP_TYPES, 7, "the campaign record carries seven of the eleven");
    Some(
        (0..11)
            .filter(|&t| counts[t] > 0)
            .map(|t| (l2_sim::ALL_TROOPS[t], counts[t]))
            .collect(),
    )
}

/// `FUN_0047F474` for one side: zero all eleven counts, refill the seven the
/// campaign carries from the surviving figures, and rebuild the total by
/// summing.
fn write_back(kingdom: &mut Kingdom, id: usize, survivors: [u32; 11]) {
    let Some(u) = kingdom.campaign.units.get_mut(id) else { return };
    for (slot, left) in u.troops.iter_mut().zip(survivors.iter()) {
        *slot = *left as i32;
    }
    // The band cannot be told from the line it was folded into, so it is
    // released rather than guessed at. See this module's header.
    u.mercenaries = None;
    u.men = u.troops.iter().sum();
}

/// **The two crates index the same column the same way**, kept honest at
/// compile time rather than by a comment.
///
/// `l2_kingdom::TroopType` and `l2_sim::Troop` are separate enums in separate
/// crates that never see each other, and [`muster_of`] and [`write_back`] cross
/// between them by index alone. If either is ever reordered this stops
/// building, which is the only way that mistake gets caught: a swapped pair
/// would compile, run, and quietly turn every archer into a swordsman.
/// The column order both crates index by, written out once so that a
/// reordering of either enum fails here and names the column, rather than
/// quietly turning every archer into a swordsman.
///
/// `docs/battle.md` §4.1: the campaign record's `+0x16C`, `TROOPS*.ENG`'s
/// columns and the `.skr` army record all agree on it — three independent
/// sources, which is why it is safe to cross between the crates by index at
/// all.
#[cfg(test)]
const COLUMN_ORDER: [(&str, &str); TROOP_TYPES] = [
    ("Peasant", "Peasants"),
    ("Crossbowman", "Crossbowmen"),
    ("Maceman", "Macemen"),
    ("Swordsman", "Swordsmen"),
    ("Pikeman", "Pikemen"),
    ("Archer", "Archers"),
    ("Knight", "Knights"),
];

#[test]
fn the_two_crates_number_the_troop_types_identically() {
    for (t, (kingdom, sim)) in COLUMN_ORDER.iter().enumerate() {
        assert_eq!(l2_kingdom::unit::ALL_TROOP_TYPES[t].index(), t);
        assert_eq!(l2_sim::ALL_TROOPS[t].index(), t);
        assert_eq!(
            format!("{:?}", l2_kingdom::unit::ALL_TROOP_TYPES[t]),
            *kingdom,
            "l2-kingdom column {t}"
        );
        assert_eq!(format!("{:?}", l2_sim::ALL_TROOPS[t]), *sim, "l2-sim column {t}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use l2_kingdom::unit::{TroopType, Unit, UnitKind};

    fn kingdom_with(
        attacker: &[(TroopType, i32)],
        defender: &[(TroopType, i32)],
        defender_owner: u8,
        defender_human: bool,
    ) -> (Kingdom, usize, usize) {
        let mut k = Kingdom::new(1);
        k.counties[3].owner = 0;
        k.counties[3].population = 546;
        k.counties[3].happiness = 79;
        k.realms[1].in_play = true;
        k.realms[1].is_human = true;

        let mut a = Unit::new(UnitKind::Army, 1, 33, 17);
        a.owner_is_human = true;
        for &(t, n) in attacker {
            a.troops[t.index()] = n;
        }
        a.men = a.troops.iter().sum();
        a.moves_used = 8;
        let ai = k.campaign.units.spawn(a).unwrap();

        let mut d = Unit::new(UnitKind::Army, defender_owner, 36, 17);
        d.owner_is_human = defender_human;
        d.home_county = 3;
        d.defence_mark = l2_kingdom::conquest::RAISED;
        d.move_allowance = 0;
        for &(t, n) in defender {
            d.troops[t.index()] = n;
        }
        d.men = d.troops.iter().sum();
        let di = k.campaign.units.spawn(d).unwrap();
        (k, ai, di)
    }

    /// **The fixture battle, through the seam.** `battle-before.sav`'s player
    /// army against the militia county 3 levies, resolved the way the original
    /// resolves a battle the player does not fight — and every number the saved
    /// game holds afterwards comes out.
    #[test]
    fn the_fixture_battle_runs_from_the_campaign_and_returns_the_saves_numbers() {
        let (mut k, a, d) = kingdom_with(
            &[(TroopType::Peasant, 128), (TroopType::Swordsman, 25), (TroopType::Archer, 25)],
            &[(TroopType::Peasant, 122), (TroopType::Archer, 60)],
            l2_kingdom::levy::OWNERLESS,
            false,
        );

        let report = resolve(
            &mut k,
            Attack::Battle { attacker: a, defender: d },
            3,
            Answer::Decline,
            1,
        )
        .expect("a battle");

        // A person is in it, so the original would have asked.
        assert_eq!(report.settlement, Settlement::Prompt);
        assert_eq!(report.resolution, Resolution::Autocalc, "declining is the autocalc");

        // **The player lost.**
        assert!(!report.verdict.attacker_won);
        assert_eq!(report.verdict.winner(), d);
        assert_eq!(report.attacker_men, (178, 0));
        assert_eq!(report.defender_men, (182, 36), "the ladder's second rung, 20%");

        // The county did **not** change hands, and both armies are gone.
        assert_eq!(report.aftermath.county_taken_by, None);
        assert_eq!(k.counties[3].owner, 0, "county 3 is still neutral");
        assert!(k.campaign.units.get(a).is_none(), "the attacker was destroyed");
        assert!(k.campaign.units.get(d).is_none(), "the defence was dissolved");

        // …and the survivors went back into the county: 546 -> 582.
        assert_eq!(report.defenders_returned, 36);
        assert_eq!(k.counties[3].population, 582);
    }

    /// The same battle **fought** rather than calculated: `l2-sim` really does
    /// run from campaign records, and the result really does come back.
    ///
    /// The *winner* is deliberately not asserted. `l2-sim` does not fly
    /// missiles yet, so sixty archers fight as sixty men with bows they never
    /// draw, and pinning the outcome here would pin that gap in place. What is
    /// asserted is that the seam is whole: figures were raised at the right
    /// scale, men died, the survivors landed back in the campaign records, one
    /// army was destroyed and the county's books balance either way.
    #[test]
    fn taking_the_field_runs_the_real_simulation_and_hands_the_result_back() {
        let (mut k, a, d) = kingdom_with(
            &[(TroopType::Peasant, 128), (TroopType::Swordsman, 25), (TroopType::Archer, 25)],
            &[(TroopType::Peasant, 122), (TroopType::Archer, 60)],
            l2_kingdom::levy::OWNERLESS,
            false,
        );

        let report = resolve(
            &mut k,
            Attack::Battle { attacker: a, defender: d },
            3,
            Answer::TakeTheField,
            l2_sim::runner::DEFAULT_SEED,
        )
        .expect("a battle");

        let Resolution::Fought { ticks, cause } = report.resolution else {
            panic!("taking the field must fight it to a conclusion: {:?}", report.resolution);
        };
        assert!(ticks > 0);
        // The only way a field battle ends by itself. If this ever reads
        // `Withdrawal` something pulled a lever nothing should be pulling.
        assert_eq!(cause, End::Annihilation);
        eprintln!(
            "fought: {ticks} ticks, {cause:?}, attacker {:?}, defender {:?}, winner {}",
            report.attacker_men,
            report.defender_men,
            if report.verdict.attacker_won { "attacker" } else { "defender" }
        );

        // Men died, and nobody was invented.
        let (a0, a1) = report.attacker_men;
        let (d0, d1) = report.defender_men;
        assert_eq!((a0, d0), (178, 182));
        assert!(a1 < a0 || d1 < d0, "a battle in which nobody died is not a battle");
        assert!(a1 >= 0 && d1 >= 0 && a1 <= a0 && d1 <= d0);

        // The loser is gone and the winner is the one still standing.
        let loser = report.verdict.loser();
        assert!(k.campaign.units.get(loser).is_none());

        // The county's books balance whichever way it went: a levied defence
        // that won walked its survivors home, and one that lost returned
        // nobody.
        if report.verdict.attacker_won {
            assert_eq!(report.defenders_returned, 0);
            assert_eq!(k.counties[3].population, 546);
            assert_eq!(k.counties[3].owner, 1, "a marked defence loses the county");
        } else {
            assert_eq!(k.counties[3].population, 546 + report.defenders_returned);
            assert_eq!(k.counties[3].owner, 0);
        }
    }

    /// Two AI armies never reach a screen and never reach `l2-sim` — the
    /// campaign settles them and moves on. This is the path an AI-versus-AI war
    /// takes, and it must not depend on a battle simulation being present.
    #[test]
    fn an_ai_battle_is_settled_silently_whatever_the_player_would_have_answered() {
        for answer in [Answer::TakeTheField, Answer::Decline] {
            let (mut k, a, d) = kingdom_with(
                &[(TroopType::Knight, 100)],
                &[(TroopType::Peasant, 40)],
                2,
                false,
            );
            k.campaign.units.get_mut(a).unwrap().owner = 3;
            k.campaign.units.get_mut(a).unwrap().owner_is_human = false;

            let report =
                resolve(&mut k, Attack::Battle { attacker: a, defender: d }, 3, answer, 1)
                    .unwrap();
            assert_eq!(report.settlement, Settlement::Silently);
            assert_eq!(report.resolution, Resolution::Autocalc);
            assert!(report.verdict.attacker_won);
            assert_eq!(k.counties[3].owner, 3, "and the AI took the county");
        }
    }

    /// The battle simulation is deterministic through the seam: the same
    /// kingdom fought twice with the same seed reaches the same numbers.
    /// `docs/netcode.md` — two lockstep peers fight the same battle or they are
    /// not playing the same game.
    #[test]
    fn the_same_battle_fought_twice_gives_the_same_answer() {
        let run = || {
            let (mut k, a, d) = kingdom_with(
                &[(TroopType::Swordsman, 90), (TroopType::Peasant, 60)],
                &[(TroopType::Pikeman, 80), (TroopType::Maceman, 40)],
                2,
                false,
            );
            k.campaign.units.get_mut(d).unwrap().defence_mark = 0;
            let r = resolve(
                &mut k,
                Attack::Battle { attacker: a, defender: d },
                3,
                Answer::TakeTheField,
                0xC0FF_EE01,
            )
            .unwrap();
            (r.attacker_men, r.defender_men, r.verdict.attacker_won, r.resolution)
        };
        assert_eq!(run(), run());
    }

    /// The men-per-figure ladder is chosen from the two armies together, and
    /// the raising loses nobody: the counts that go in are the counts that come
    /// out when nothing has happened yet.
    #[test]
    fn a_campaign_army_is_raised_at_the_scale_the_two_totals_choose() {
        let (k, a, d) = kingdom_with(
            &[(TroopType::Peasant, 128), (TroopType::Swordsman, 25), (TroopType::Archer, 25)],
            &[(TroopType::Peasant, 122), (TroopType::Archer, 60)],
            6,
            false,
        );
        let at = muster_of(&k, a).unwrap();
        let dt = muster_of(&k, d).unwrap();
        let runner = BattleRunner::deploy_muster(
            blank_field(),
            1,
            Muster { troops: &at, owner: 1, human: true },
            Muster { troops: &dt, owner: 6, human: false },
        );
        // 178 + 182 = 360, over the ladder's first break at 305: class 1.
        assert_eq!(runner.men_per_figure(SIDE_B), 8);
        assert_eq!(runner.men_per_figure(SIDE_A), 8);
        assert_eq!(runner.survivors(SIDE_B)[TroopType::Peasant.index()], 128);
        assert_eq!(runner.survivors(SIDE_B)[TroopType::Swordsman.index()], 25);
        assert_eq!(runner.survivors(SIDE_A)[TroopType::Archer.index()], 60);
        assert_eq!(runner.men(SIDE_B) + runner.men(SIDE_A), 360, "nobody lost in the raising");
    }

    // --- sieges ------------------------------------------------------------

    /// A besieging army with its engines built, a garrison in a castle, and
    /// the assault taken all the way back onto the campaign map.
    fn siege_kingdom(castle_type: u8, engines: [i16; 3]) -> (Kingdom, usize, usize) {
        // Chosen so that the castle bonus is the *only* thing that decides it:
        // 200 peasants and 150 knights score 3,720, and 100 archers with 50
        // pikemen score 1,770 — which the five castle percentages lift to
        // 2,832 / 3,540 / 4,425 / 5,664 / 7,080. The attacker clears the first
        // two and nothing above them.
        let (mut k, a, d) = kingdom_with(
            &[(TroopType::Peasant, 200), (TroopType::Knight, 150)],
            &[(TroopType::Archer, 100), (TroopType::Pikeman, 50)],
            2,
            false,
        );
        k.counties[3].owner = 2;
        k.counties[3].castle_type = castle_type;
        k.counties[3].garrison_unit = d;
        k.realms[2].in_play = true;

        let du = k.campaign.units.get_mut(d).unwrap();
        du.garrison_county = 3;
        du.defence_mark = 0;
        du.besieged_by = a as u8;
        let au = k.campaign.units.get_mut(a).unwrap();
        au.besieging_county = 3;
        for (slot, ordered) in au.engines.iter_mut().zip(engines) {
            slot.ordered = ordered;
            slot.percent = 100;
        }
        (k, a, d)
    }

    /// **The castle level reaches the autocalc, and it changes who wins.**
    ///
    /// The same two armies, the same seed, five castles: the besieger takes a
    /// palisade and is thrown off a royal castle, and the only thing that
    /// differs is [`l2_kingdom::battle::CASTLE_STRENGTH_PERCENT`]. Until this
    /// existed the table was in `auto_resolve` with nothing to pass it.
    #[test]
    fn the_castle_level_reaches_the_autocalc_and_decides_the_siege() {
        let mut won = Vec::new();
        for castle_type in 1..=5u8 {
            let (mut k, a, d) = siege_kingdom(castle_type, [2, 0, 0]);
            let assault = l2_kingdom::siege::assault(&k.counties, &mut k.campaign.units, a);
            assert!(matches!(
                assault,
                l2_kingdom::siege::Assault::Battle { castle_level, .. }
                    if castle_level == castle_type - 1
            ));
            let report = resolve_siege(&mut k, assault, Answer::Decline, 1).expect("a siege");
            assert_eq!(report.resolution, Resolution::Autocalc);
            won.push(report.verdict.attacker_won);
            if report.verdict.attacker_won {
                assert_eq!(k.counties[3].owner, 1, "the castle fell and the county with it");
                assert_eq!(k.counties[3].garrison_unit, 0);
                assert!(k.campaign.units.get(d).is_none());
                assert_eq!(k.campaign.units.get(a).unwrap().besieging_county, 0);
            } else {
                assert_eq!(k.counties[3].owner, 2, "a castle that holds keeps its county");
                assert_eq!(k.campaign.units.get(d).unwrap().besieged_by, 0, "the siege is over");
            }
        }
        assert_eq!(
            won,
            vec![true, true, false, false, false],
            "the same army takes the two smallest castles and no others"
        );
    }

    /// **The engines and the oil reach the battle**, which is the whole of
    /// `Army_PrepareForBattle` — and neither survives it.
    #[test]
    fn the_besiegers_engines_and_the_garrisons_oil_are_raised_and_then_gone() {
        let (k, a, d) = siege_kingdom(5, [2, 1, 1]);
        let engines = l2_kingdom::siege::prepare_besieger(k.campaign.units.get(a).unwrap());
        let oil = l2_kingdom::siege::prepare_garrison(4);
        assert_eq!(engines.counts(), [2, 1, 1, 0]);
        assert_eq!(oil.counts(), [0, 0, 0, 6], "a royal castle gets six pots");

        let at = muster_with(&k, a, engines).unwrap();
        let dt = muster_with(&k, d, oil).unwrap();
        assert!(at.iter().any(|&(t, n)| t == l2_sim::Troop::Catapults && n == 2));
        assert!(dt.iter().any(|&(t, n)| t == l2_sim::Troop::Oil && n == 6));

        // And the campaign record never learns about them: `Unit::troops` is
        // seven columns and the four battle slots are produced on the way in.
        assert_eq!(k.campaign.units.get(a).unwrap().troops.len(), TROOP_TYPES);
    }

    /// A siege **fought** rather than calculated: the castle is on the field,
    /// the siege order tables are the ones being dispatched, and the result
    /// comes back onto the campaign map.
    ///
    /// The winner is deliberately not asserted, for the same reason the field
    /// version does not assert one — missiles do not fly yet, so a fought
    /// battle here and a fought battle there would not agree.
    #[test]
    fn a_fought_siege_puts_a_castle_on_the_field_and_returns_a_result() {
        let (mut k, a, d) = siege_kingdom(4, [2, 2, 1]);
        k.campaign.units.get_mut(d).unwrap().owner_is_human = true;
        let assault = l2_kingdom::siege::assault(&k.counties, &mut k.campaign.units, a);
        let report =
            resolve_siege(&mut k, assault, Answer::TakeTheField, 0xB01D).expect("a siege");
        assert!(
            matches!(report.resolution, Resolution::Fought { .. } | Resolution::Stalled { .. }),
            "taking the field runs l2-sim: {:?}",
            report.resolution
        );
        // Whoever won, the siege link is gone on both sides afterwards.
        for id in [a, d] {
            if let Some(u) = k.campaign.units.get(id) {
                assert_eq!(u.besieging_county, 0);
                assert_eq!(u.besieged_by, 0);
            }
        }
    }

    /// **A besieger that gives up — the whole withdrawal path, on the road a
    /// player can actually reach.**
    ///
    /// `UnitOrder_SiegeAttKnight` (`0x0048D9CE`) is the only writer of
    /// `g_battleWithdrawal` in the binary: an AI besieger whose whole force is
    /// knights, in front of a wall nothing has breached, stops trying. It is
    /// the only lever in the game that reaches
    /// [`l2_kingdom::battle::withdraw_casualties`], and until the clause was
    /// added to `l2-sim` neither existed — which is why the campaign had never
    /// implemented the half of `Battle_ReturnToCampaign` behind it.
    ///
    /// The catapult is not decoration: with no siege engine at all
    /// `Battle_CheckOutcome`'s *assault repulsed* arm fires first and the
    /// question never gets asked. One engine keeps `g_siegeEngineCount`
    /// non-zero, the breach score stays 0, and the knights think.
    #[test]
    fn an_all_knight_ai_besieger_withdraws_and_is_charged_for_it() {
        let (mut k, a, d) = siege_kingdom(2, [1, 0, 0]);
        {
            // Swap the roles round: the besieger is the AI's and the castle is
            // the player's, so the battle is one the player is asked about and
            // the AI's own knights are the only figures the census sees.
            let au = k.campaign.units.get_mut(a).unwrap();
            au.owner = 2;
            au.owner_is_human = false;
            au.troops = [0; TROOP_TYPES];
            au.troops[TroopType::Knight.index()] = 400;
            au.men = 400;
            let du = k.campaign.units.get_mut(d).unwrap();
            du.owner = 1;
            du.owner_is_human = true;
        }
        k.counties[3].owner = 1;

        let assault = l2_kingdom::siege::assault(&k.counties, &mut k.campaign.units, a);
        let report = resolve_siege(&mut k, assault, Answer::TakeTheField, 0x5A11).expect("a siege");

        assert_eq!(
            report.resolution,
            Resolution::Fought { ticks: report_ticks(&report), cause: End::Withdrawal },
            "the knights gave up: {:?}",
            report.resolution
        );
        assert!(!report.verdict.attacker_won, "a withdrawal hands the field to the other side");

        // **`Army_WithdrawCasualties` was charged**, and it is half the line.
        assert_eq!(report.aftermath.withdrawal_casualties, Some(200));
        assert!(!report.aftermath.loser_destroyed, "two hundred knights is over fifty");
        assert!(report.aftermath.loser_siege_lifted);
        let survivor = k.campaign.units.get(a).expect("it walked off the field");
        assert_eq!(survivor.troops[TroopType::Knight.index()], 200);
        assert_eq!(survivor.men, 200);
        assert_eq!(survivor.besieging_county, 0, "and it lost the siege, not its life");
        assert_eq!(k.counties[3].owner, 1, "the castle held");
        // The report's "after" numbers are the ones the retreat left, not the
        // ones the army walked onto the field with.
        assert_eq!(report.attacker_men, (400, 200));
    }

    /// The ticks of whatever the report says, so the assertion above can name
    /// the *cause* without pinning the length of the battle.
    fn report_ticks(r: &BattleReport) -> u32 {
        match r.resolution {
            Resolution::Fought { ticks, .. } | Resolution::Stalled { ticks } => ticks,
            Resolution::Autocalc => 0,
        }
    }

    /// **A whole turn phase 2**, from the link check to the county changing
    /// hands — which is the thing a player could not do before today.
    #[test]
    fn one_turn_phase_two_builds_the_engines_assaults_and_takes_the_county() {
        let (mut k, a, d) = siege_kingdom(1, [0, 0, 0]);
        // The player orders one catapult: 200 man-seasons over 350 men is one.
        l2_kingdom::siege::order_engine(
            &mut k.campaign.units,
            a,
            l2_kingdom::siege::Engine::Catapult,
            1,
        );
        assert_eq!(k.campaign.units.get(a).unwrap().siege_seasons_left, 1);

        let reports = run_siege_phase(&mut k, Answer::Decline, 7);
        assert_eq!(reports.len(), 1, "one siege, one assault");
        let report = &reports[0];
        assert!(report.verdict.attacker_won, "a palisade against 350 men");
        assert_eq!(k.counties[3].owner, 1, "the county changed hands");
        assert_eq!(k.counties[3].garrison_unit, 0);
        assert!(k.campaign.units.get(d).is_none());
        assert_eq!(k.campaign.units.get(a).unwrap().engines[0].percent, 100);

        // And a second phase 2 finds nothing to do.
        assert!(run_siege_phase(&mut k, Answer::Decline, 8).is_empty());
    }

    /// A siege whose engines are not ready yet is **not** assaulted, and the
    /// phase leaves it building.
    #[test]
    fn a_siege_still_building_survives_the_phase_untouched() {
        let (mut k, a, d) = siege_kingdom(5, [0, 0, 0]);
        // Two rams: 800 man-seasons over 350 men is three seasons.
        for _ in 0..2 {
            l2_kingdom::siege::order_engine(
                &mut k.campaign.units,
                a,
                l2_kingdom::siege::Engine::BatteringRam,
                1,
            );
        }
        assert_eq!(k.campaign.units.get(a).unwrap().siege_seasons_left, 3);

        for expected in [2u8, 1, 0] {
            let reports = run_siege_phase(&mut k, Answer::Decline, 3);
            if expected == 0 {
                assert_eq!(reports.len(), 1, "the last season assaults");
            } else {
                assert!(reports.is_empty(), "still building");
                assert_eq!(k.campaign.units.get(a).unwrap().siege_seasons_left, expected);
                assert_eq!(k.counties[3].owner, 2, "the castle still stands");
                assert!(k.campaign.units.get(d).is_some());
            }
        }
    }

    /// **The gate, from the phase's own side.** A besieger that orders nothing
    /// against a stone castle is not stalled and does not fight: its siege is
    /// lifted and it is free to march away.
    #[test]
    fn a_besieger_with_no_engines_against_a_big_castle_is_released_rather_than_stalled() {
        let (mut k, a, d) = siege_kingdom(4, [0, 0, 0]);
        assert_eq!(k.campaign.units.get(a).unwrap().siege_seasons_left, 0, "nothing to build");
        let reports = run_siege_phase(&mut k, Answer::Decline, 1);
        assert!(reports.is_empty(), "no battle was fought");
        assert_eq!(k.campaign.units.get(a).unwrap().besieging_county, 0, "the siege was lifted");
        assert_eq!(k.campaign.units.get(d).unwrap().besieged_by, 0);
        assert_eq!(k.counties[3].owner, 2);
    }

    /// The banner the outcome screen shows is one of the four siege pairs, and
    /// which one depends on **both** questions.
    #[test]
    fn a_siege_picks_one_of_the_four_siege_banners_and_not_a_field_one() {
        let (mut k, a, _d) = siege_kingdom(1, [1, 0, 0]);
        let assault = l2_kingdom::siege::assault(&k.counties, &mut k.campaign.units, a);
        let report = resolve_siege(&mut k, assault, Answer::Decline, 1).unwrap();
        // A is the besieger (realm 1, the player) and B the garrison (realm 2).
        let (winner_owner, loser_owner) =
            if report.verdict.attacker_won { (1, 2) } else { (2, 1) };
        let banner = battle::outcome(report.verdict, true, 1, winner_owner, loser_owner);
        assert!(
            matches!(
                banner,
                battle::Outcome::SiegeWon
                    | battle::Outcome::SiegeLost
                    | battle::Outcome::SiegeLifted
                    | battle::Outcome::CastleLost
            ),
            "a siege never shows a field banner: {banner:?}"
        );
    }
}

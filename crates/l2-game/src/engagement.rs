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
//! * **Missiles do not fly.** `l2-sim` resolves a missile hit but nothing
//!   drives reload and flight (`l2-sim`'s own module docs say so), so an army
//!   of archers fights as an army of men with bows they do not use. This is
//!   the single largest reason a fought battle here and a fought battle there
//!   would not agree, and it is why [`resolve`]'s fixture test uses the
//!   autocalc path, which does close exactly.
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
    let before = |k: &Kingdom, id: usize| k.campaign.units.get(id).map_or(0, |u| u.men);
    let (attacker_before, defender_before) = (before(kingdom, attacker), before(kingdom, defender));

    let settlement = battle::settlement(
        &kingdom.campaign.units,
        attacker,
        defender,
        kingdom.options.fight_humans_only_byte,
    );
    let take_the_field = settlement == Settlement::Prompt && answer == Answer::TakeTheField;

    let (verdict, resolution) = if take_the_field {
        fight(kingdom, attacker, defender, seed)?
    } else {
        // Sieges are out of scope, so no castle level is ever passed. When they
        // arrive this is the one argument that changes.
        let verdict = battle::auto_resolve(&mut kingdom.campaign.units, attacker, defender, None)?;
        (verdict, Resolution::Autocalc)
    };

    let attacker_after = before(kingdom, attacker);
    let defender_after = before(kingdom, defender);

    let Kingdom { counties, realms, campaign, options, tables, .. } = kingdom;
    let aftermath = battle::return_to_campaign(
        tables,
        counties,
        realms,
        &mut campaign.units,
        &mut campaign.names,
        verdict,
        county,
        false,
        options.difficulty,
    );
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

    Some(BattleReport {
        settlement,
        resolution,
        verdict,
        aftermath,
        defenders_returned,
        attacker_men: (attacker_before, attacker_after),
        defender_men: (defender_before, defender_after),
    })
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
    seed: u64,
) -> Option<(Verdict, Resolution)> {
    let a_troops = muster_of(kingdom, attacker)?;
    let d_troops = muster_of(kingdom, defender)?;
    let (a_owner, a_human) = {
        let u = kingdom.campaign.units.get(attacker)?;
        (u.owner, u.owner_is_human)
    };
    let (d_owner, d_human) = {
        let u = kingdom.campaign.units.get(defender)?;
        (u.owner, u.owner_is_human)
    };

    let mut runner = BattleRunner::deploy_muster(
        blank_field(),
        seed,
        Muster { troops: &a_troops, owner: a_owner, human: a_human },
        Muster { troops: &d_troops, owner: d_owner, human: d_human },
    );

    // A human side gets no order handler — `Battle_UpdateAllUnits` guards on
    // it — so without this it stands where it deployed until the other side
    // walks into it. This is the click a player makes on the first frame.
    for (side, human) in [(SIDE_B, a_human), (SIDE_A, d_human)] {
        if human {
            let enemy = runner.home(l2_sim::runner::other_side(side));
            runner.order_side(side, enemy.0, enemy.1);
        }
    }

    // The original's frame loop asks `FUN_00477DFC` every frame; asking every
    // hundredth costs at most ninety-nine ticks of a battle that is already
    // over, and no rule reads the tick count.
    let mut conclusion = None;
    while runner.tick < MAX_TICKS {
        runner.run(CHECK_EVERY);
        conclusion = runner.conclusion();
        if conclusion.is_some() {
            break;
        }
    }

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
    Some((verdict, resolution))
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
fn muster_of(kingdom: &Kingdom, id: usize) -> Option<Vec<(Troop, u32)>> {
    let u = kingdom.campaign.units.get(id)?;
    let mut counts = [0u32; TROOP_TYPES];
    for (slot, men) in counts.iter_mut().zip(u.troops.iter()) {
        *slot = (*men).max(0) as u32;
    }
    if let Some(band) = u.mercenaries {
        counts[band.troop.index()] += band.men().max(0) as u32;
    }
    Some(
        (0..TROOP_TYPES)
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
}

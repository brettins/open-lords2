#![allow(unused_imports)]
use super::*;
use super::helpers::*;
use super::scenarios::*;
use std::collections::BTreeMap;
use l2_game::game::Game;
use l2_kingdom::report::Message;
use l2_kingdom::tables::Tables;
use l2_kingdom::{Kingdom, UnitKind};


#[test]
fn a_hundred_turns_of_england() {
    let save = l2_testkit::england!();
    let mut game =
        l2_game::scenario::from_save(&save, Tables::DEFAULT).expect("the fixture loads");
    scoreline(&game.kingdom, "England, turn 1");
    let census = play(&mut game, TURNS, "England");
    scoreline(&game.kingdom, "England, turn 100");
    census.print("England, 100 turns");

    // **The revolt chain, which is what this file was written to find.** Before
    // `County_RaiseRevolt` (`0x004AC185`) landed, a hundred turns of England
    // raised twenty `Revolt` messages and nothing else: no mob, no debit, no
    // `County_MakeIndependent`. The person's county sat at population **zero**
    // for seventy-eight turns while the realm stayed in play.
    //
    // Each assertion names one link of the chain, and each was ablated:
    //
    // | delete | which goes red |
    // |---|---|
    // | `unrest::raise_revolt`'s `units.spawn` | the mob |
    // | `Kingdom::unrest_update`'s `make_county_independent` | the elimination |
    // | the `Message::Revolt` push | the revolt |
    assert!(census.fired("REVOLT"), "a hundred turns and nobody revolted");
    assert!(
        census.fired("a peasant mob on the map"),
        "a revolt was reported and no kind-2 unit exists — `County_RaiseRevolt` \
         raised no mob"
    );
    assert!(
        census.fired("a realm eliminated"),
        "the unplayed person's county revolted and the realm stayed in play — \
         `County_MakeIndependent` did not run, so `Realm_RecountStrength` never \
         saw a zero"
    );
    assert!(
        census.fired("THE GAME WAS LOST"),
        "the person's realm was eliminated and `g_gameOutcome` never became 11"
    );

    assert!(
        census.max_counties >= 4,
        "the largest realm after {TURNS} turns holds {} counties — the war is not \
         moving the map",
        census.max_counties
    );
    assert!(census.fired("a battle"), "a hundred turns of five realms and no battle");
}

/// They say *"a hundred years of England still contains a bankrupt lord"*, and
/// a change to any economic rule can move that legitimately. If one goes red,
/// read the census this test prints before assuming a defect: the question to
/// ask is whether the rule became **unreachable**, which is C27's failure and a
/// real one,
///
/// `Unit_StepOnce`'s sub-tile counter (`docs/decisions.md` **C134**)
/// made a unit take 8 ticks to cross a road tile and 32 to cross open ground,
/// where every earlier build crossed one a tick. Tiles a *season* did not
/// change — the move allowance is the budget and it is untouched — but the
/// tick a unit arrives on did, and over four hundred turns that moves the
/// board. Measured, both ways, on this fixture:
///
/// * **The mutiny re-fires because the counter wraps.** `Wages_PayAll`
/// (`0x004ACBD4`) sets stage 5 → 0 at the mutiny, so a
///   realm that never pays loses its armies every six seasons for ever.
///
/// * **A tax rate ≥ 20 is on no ladder at all.** `AI_SetTaxRates`' four ladders
///   top out at 15 ([`l2_kingdom::tables::AI_TAX_LADDERS`]). The rates above 19
///   come from `FUN_0049F431`, AI step 7's abandon pass, which sets 32, 28, 23
///   or 35 by the lord's personality on a county it has decided it cannot hold
///   — [`l2_kingdom::tables::AI_PERSONALITY_ABANDON_TAX_RATE`], `[V]`.
///
/// `C184` moved the trajectory again and `SECESSION` stopped firing.
///
/// `Realm_SecedeIsolatedCounties` (`0x0044AE3C`) takes a county only from a
/// realm holding two or more contiguity blocks, and contiguity is the county
/// neighbour list at `+0x5C` and nothing else (`docs/kingdom.md` §6.1). On this
/// fixture — 14 counties, 39 undirected edges — **county 2 is the only cut
/// vertex**: county 1's list holds nothing but county 2, and removing any other
/// county leaves the rest connected. So the pass can fire here only when a realm
/// holds county 1 and something past a county 2 that is not its own — enemy or
/// neutral alike, the partition being same-owner adjacency. That is a fact about
/// where the armies went, not about a rule.
#[test]
fn four_hundred_turns_of_england_reaches_the_rules_nothing_else_can() {
    let save = l2_testkit::england!();
    let mut game =
        l2_game::scenario::from_save(&save, Tables::DEFAULT).expect("the fixture loads");
    let census = play(&mut game, TURNS_LONG, "England x600");
    scoreline(&game.kingdom, "England, turn 600");
    census.print("England, 600 turns");

    assert!(
        census.fired("bankruptcy"),
        "a hundred and fifty years and no lord ever missed a wage bill — the \
         bankruptcy ladder has no way in at all, which is `docs/decisions.md` \
         C27's shape"
    );
    assert!(
        census.fired("bankruptcy: desertion"),
        "the bankruptcy counter reached {} and never 2 — the desertion arm of \
         `Wages_PayAll` has never executed",
        census.max_bankrupt_stage
    );
    // `Wages_PayAll` (`0x004ACBD4`) wraps stage 5 to 0, so a realm that
    // pays mutinies every six seasons.
    assert!(
        census.fired("bankruptcy: MUTINY"),
        "the bankruptcy counter reached {} and never wrapped — the mutiny arm of \
         `Wages_PayAll` has never executed",
        census.max_bankrupt_stage
    );
    // No tax ladder goes above 15. A rate this high is `FUN_0049F431`, AI step
    // 7's abandon pass, stripping a county it has given up on.
    assert!(
        census.fired("a tax rate the empire term can see (>= 20)"),
        "no county was ever taxed at 20 or more, so `TAX_HAPPINESS_OTHER`'s first row is \
         the only one the happiness term has ever used; max rate {}",
        census.max_tax_rate
    );
    assert!(
        census.max_blocks <= 1,
        "a realm ended a turn holding {} contiguity blocks — \
         `Realm_SecedeIsolatedCounties` did not take the outlying one, so the \
         pass is not running at all, which is `docs/decisions.md` C27's shape",
        census.max_blocks
    );
}

/// **`Realm_SecedeIsolatedCounties` (`0x0044AE3C`) out of a played turn.**
///
/// `crates/l2-kingdom/tests/secession.rs` drives the pass over synthetic chains;
/// nothing drove it through `l2_game::turn::end_turn` on the real map except the
/// run above, by accident. One county going raises `L2.eng` group 127.
#[test]
fn a_realm_cut_in_two_loses_the_far_half_through_the_turn_machine() {
    let Some(mut game) = england_cut_in_two() else {
        l2_testkit::skip!("no England fixture");
    };
    let realm = game.kingdom.counties[1].owner;
    let census = play(&mut game, 2, "a realm cut in two");
    census.print("a realm cut in two, 2 turns");

    assert!(
        census.fired("SECESSION"),
        "realm {realm} held county 1 and county 3 with county 2 between them and \
         kept both — `Realm_SecedeIsolatedCounties` did not run, or \
         `Territory_BuildBlocks` joined two counties that are not neighbours"
    );
    let kept: Vec<usize> =
        [1usize, 3].into_iter().filter(|&id| game.kingdom.counties[id].owner == realm).collect();
    assert_eq!(kept.len(), 1, "exactly one of the two blocks is kept, and it kept {kept:?}");
    let gone = if kept[0] == 1 { 3 } else { 1 };
    assert_eq!(
        game.kingdom.counties[gone].owner, 0,
        "and the other declared independence rather than changing hands"
    );
    assert!(
        game.kingdom.counties[gone].industry.iter().all(|i| !i.enabled),
        "every industry switched off"
    );
    assert_eq!(
        census.max_blocks, 1,
        "and nobody is left holding two blocks once the pass has run"
    );
}


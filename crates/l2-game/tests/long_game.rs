//! **The hundred-turn game.** `docs/plan.md` §2.5 and §0 row 4.
//!
//! `tests/ai_war.rs` plays forty turns and asks whether the AI is *playing*.
//! This file plays enough turns for the late-game rules to fire at all, and
//! asks a different question: **which rules have now run, and what breaks when
//! they do.**
//!
//! Everything here is our own arithmetic agreeing with itself —
//! oracle for turn 100 of England and there will not be one until a person
//! plays it (`docs/plan.md` §5 item 1). So the assertions are of two kinds only,
//!
//!
//! * **Invariants** — statements the original's own functions maintain, which
//!   are true whatever the numbers come out as. A labour split that no longer
//!   sums to the population is a defect at any turn count.
//! * **Reachability** — *a rule fired at all*. `docs/decisions.md` C27: a rule
//!   had a way in.
//!
//! Nothing here asserts a *value*, because nothing here could justify one.

use std::collections::BTreeMap;

use l2_game::game::Game;
use l2_kingdom::report::Message;
use l2_kingdom::tables::Tables;
use l2_kingdom::{Kingdom, UnitKind};

/// A hundred turns is twenty-five years. `docs/plan.md` §2.5 calls it "the
/// hundred-turn game" and that is the number.
const TURNS: usize = 100;

/// The long game's horizon — a hundred and fifty years.
///
/// **It was 400 and is 600**, because the first bankruptcy on this fixture
/// moved from turn 144 to turn 480 when units started taking the original's
/// number of ticks to cross a tile. See
/// [`four_hundred_turns_of_england_reaches_the_rules_nothing_else_can`] for the
/// measurement and for what it does *not* buy back.
const TURNS_LONG: usize = 600;

// ---------------------------------------------------------------------------
// The census: which late-game rules have fired, and when they first did.
// ---------------------------------------------------------------------------

/// One rule,
#[derive(Debug, Default)]
struct Census {
    first: BTreeMap<&'static str, usize>,
    count: BTreeMap<&'static str, usize>,
    /// The largest number of counties any one realm ever held.
    max_counties: u8,
    /// The largest tax rate any county ever carried.
    max_tax_rate: i32,
    /// The largest treasury any realm ever held.
    max_gold: i32,
    /// The largest bankruptcy stage reached.
    max_bankrupt_stage: u8,
    /// The most contiguity blocks any realm held at the **end** of a turn.
    /// `Realm_SecedeIsolatedCounties` runs inside the season, so 2 here means
    /// the pass did not run. `docs/kingdom.md` §6.1.
    max_blocks: usize,
}

impl Census {
    fn fire(&mut self, what: &'static str, turn: usize) {
        self.first.entry(what).or_insert(turn);
        *self.count.entry(what).or_insert(0) += 1;
    }

    fn fired(&self, what: &str) -> bool {
        self.first.contains_key(what)
    }

    /// Everything the pass reads out of one turn's messages and one turn's
    /// state. Kept in one place so the England run
    /// measure the same things.
    fn observe(&mut self, k: &Kingdom, messages: &[Message], turn: usize) {
        for m in messages {
            match m {
                Message::UnrestWarning { .. } => self.fire("unrest warning", turn),
                Message::UnrestRising { .. } => self.fire("unrest rising", turn),
                Message::Revolt { .. } => self.fire("REVOLT", turn),
                Message::Drought { .. } => self.fire("drought", turn),
                Message::Flooding { .. } => self.fire("flooding", turn),
                Message::Bankrupt { stage, action, .. } => {
                    self.max_bankrupt_stage = self.max_bankrupt_stage.max(*stage);
                    self.fire("bankruptcy", turn);
                    self.fire(
                        match action {
                            l2_kingdom::industry::BankruptcyAction::None => "bankruptcy: none",
                            l2_kingdom::industry::BankruptcyAction::MercenariesDesert => {
                                "bankruptcy: mercenaries desert"
                            }
                            l2_kingdom::industry::BankruptcyAction::Warned => "bankruptcy: warned",
                            l2_kingdom::industry::BankruptcyAction::Desertion => {
                                "bankruptcy: desertion"
                            }
                            l2_kingdom::industry::BankruptcyAction::LastWarning => {
                                "bankruptcy: last warning"
                            }
                            l2_kingdom::industry::BankruptcyAction::Mutiny => "bankruptcy: MUTINY",
                        },
                        turn,
                    );
                }
                Message::CountySeceded { .. } => self.fire("SECESSION", turn),
                Message::LandsDivide { .. } => self.fire("lands divide", turn),
                Message::ArmyStarving { stage, .. } => {
                    self.fire("army starving", turn);
                    if *stage >= 5 {
                        self.fire("army perishes", turn);
                    }
                }
                Message::CastleBuilt { .. } => self.fire("castle built", turn),
                Message::Event { .. } => self.fire("random event", turn),
            }
        }

        for id in 1..=k.county_count {
            let c = &k.counties[id];
            self.max_tax_rate = self.max_tax_rate.max(c.tax_rate);
            if c.tax_rate > 0 {
                self.fire("a non-zero tax rate", turn);
            }
            if c.tax_rate >= 20 {
                self.fire("a tax rate the empire term can see (>= 20)", turn);
            }
            if c.tax_hap_other != 0 {
                self.fire("TAX_HAPPINESS_OTHER non-zero", turn);
            }
            if c.fields_waste > 0 {
                self.fire("wasteland", turn);
            }
            if c.fields_reclaiming > 0 {
                self.fire("reclamation under way", turn);
            }
            if c.castle_type > 0 && c.owner != 0 {
                self.fire("an owned castle", turn);
            }
            if c.unrest > 0 {
                self.fire("an unrest counter above zero", turn);
            }
            if c.purse != 0 && c.owner == 0 {
                self.fire("an unowned county with a purse", turn);
            }
        }

        // `Realm_SecedeIsolatedCounties` (`0x0044AE3C`) leaves every realm one
        // block a season. Two after a turn means the pass did not run; no
        // `SECESSION` means only that nobody was ever split.
        let blocks = l2_kingdom::territory::build_blocks(&k.counties, k.county_count);
        for r in 1..l2_kingdom::MAX_REALMS {
            let held = blocks.of_realm(r as u8).count();
            self.max_blocks = self.max_blocks.max(held);
            if held > 1 {
                self.fire("a realm left cut in two", turn);
            }
        }

        for r in 1..l2_kingdom::MAX_REALMS {
            let realm = &k.realms[r];
            if !realm.in_play {
                self.fire("a realm eliminated", turn);
                continue;
            }
            self.max_counties = self.max_counties.max(realm.county_count);
            self.max_gold = self.max_gold.max(realm.gold);
            if realm.county_count > 1 {
                self.fire("a realm holding more than one county", turn);
            }
            if realm.county_count >= 4 {
                self.fire("a realm holding four or more counties", turn);
            }
            if realm.tax_hap_empire != 0 {
                self.fire("Realm::tax_hap_empire non-zero", turn);
            }
            if realm.ally != 0 {
                self.fire("an alliance", turn);
            }
            if realm.gold >= 10_000 {
                self.fire("a treasury over 10,000", turn);
            }
            if realm.gold < 0 {
                self.fire("a negative treasury", turn);
            }
            for other in 1..l2_kingdom::MAX_REALMS {
                let p = realm.pair(other as u8);
                if p.at_war {
                    self.fire("a declared war", turn);
                }
                if p.standing <= -20 {
                    self.fire("a standing at or below -20", turn);
                }
            }
        }

        for (_, u) in k.campaign.units.iter() {
            match u.kind {
                UnitKind::Army => self.fire("an army on the map", turn),
                UnitKind::Merchant => self.fire("a merchant on the map", turn),
                UnitKind::Transport => self.fire("a transport on the map", turn),
                UnitKind::PeasantMob => self.fire("a peasant mob on the map", turn),
            }
        }
    }

    fn print(&self, label: &str) {
        eprintln!("--- {label}: what fired, and on which turn ---");
        for (what, turn) in &self.first {
            eprintln!("  turn {turn:>4}  x{:<6} {what}", self.count[what]);
        }
        eprintln!(
            "  max counties held by one realm: {}   max tax rate: {}   max treasury: {}   \
             max bankruptcy stage: {}   max blocks held by one realm: {}",
            self.max_counties,
            self.max_tax_rate,
            self.max_gold,
            self.max_bankrupt_stage,
            self.max_blocks
        );
    }
}

// ---------------------------------------------------------------------------
// The invariants: things the original's own passes maintain, at every turn.
// ---------------------------------------------------------------------------

/// Everything that must be true of a kingdom between two turns, whatever the
/// numbers are. Returns the first violation as a sentence.
///
/// **Each clause names the original function that maintains it**,
/// says which subsystem to look at
fn invariant(k: &Kingdom) -> Result<(), String> {
    for id in 1..=k.county_count {
        let c = &k.counties[id];
        // `Labour_Allocate` (`0x0045C6C6`) deals every peasant to one of nine
        // jobs, twice a season. The nine records summing to the population is
        // the invariant the whole record layout was proved from
        // (`docs/rules.md` §2).
        let dealt: i32 = c.labour.iter().sum();
        if dealt != c.population {
            return Err(format!(
                "county {id}: {dealt} peasants are in jobs and {} live there — `Labour_Allocate` \
                 did not deal everybody",
                c.population
            ));
        }
        if c.population < 0 {
            return Err(format!("county {id}: population {}", c.population));
        }
        // `Happiness_UpdateAll` clamps 0..=100 on every write.
        if !(0..=100).contains(&c.happiness) {
            return Err(format!("county {id}: happiness {} is outside 0..=100", c.happiness));
        }
        if !(0..=100).contains(&c.health_meter) {
            return Err(format!("county {id}: health {} is outside 0..=100", c.health_meter));
        }
        // `Tax_CollectAll` reads the rate; `Screen_Tax`'s slider is capped at
        // 50 (`docs/rules.md` §3).
        if !(0..=50).contains(&c.tax_rate) {
            return Err(format!("county {id}: tax rate {} is outside 0..=50", c.tax_rate));
        }
        // `Field_Recount` (`docs/kingdom.md` §7.2) recounts the five field
        // classes off the twenty map tiles, so they cannot exceed twenty.
        let fields =
            c.fields_fallow + c.fields_cattle + c.fields_grain + c.fields_waste + c.fields_reclaiming;
        if fields > l2_kingdom::county::MAX_FIELDS as i32 {
            return Err(format!(
                "county {id}: {fields} fields counted and a county has {}",
                l2_kingdom::county::MAX_FIELDS
            ));
        }
        if c.grain < 0 {
            return Err(format!("county {id}: grain {}", c.grain));
        }
        if c.herd < 0 {
            return Err(format!("county {id}: herd {}", c.herd));
        }
        // A county cannot be owned by a realm that has been eliminated —
        // `Realm_RecountStrength` takes a realm out of play only when it holds
        // nothing.
        if c.owner != 0 && !k.realms[c.owner as usize].in_play {
            return Err(format!(
                "county {id} is owned by realm {}, which is out of play",
                c.owner
            ));
        }
    }

    for r in 1..l2_kingdom::MAX_REALMS {
        let realm = &k.realms[r];
        // `Realm_Recount` (`0x0049B3AC`) rewrites `county_count` from a sweep
        // of the counties,
        let held = (1..=k.county_count).filter(|&id| k.counties[id].owner == r as u8).count();
        if realm.county_count as usize != held {
            return Err(format!(
                "realm {r}: county_count is {} and {held} counties name it — `Realm_Recount` is \
                 not running",
                realm.county_count
            ));
        }
        if realm.in_play && held == 0 && realm.strength == 0 {
            return Err(format!("realm {r} is in play with strength 0"));
        }
        // **`Diplo_ReconcileAlliances` (`0x004A1847`) runs every turn — it does
        // now — and it guarantees less than this check used to demand.**
        //
        // Its loop `continue`s on `strength == 0` before it looks at that
        // realm's `ally` byte at all, so **a dead realm's `ally` is stale by
        // construction**: realm 2 dies pointing at realm 5, realm 5's own
        // pairing is dropped on the next pass, and realm 2 goes on naming 5 for
        // ever. Reading a corpse's byte as an assertion about the living is
        // what this check was doing, and it is why it fired.
        //
        // The two halves it still asserts, both of which the original really
        // does maintain, are about **realms in play**:
        //
        // * an in-play realm's pairing is symmetric — the reconcile pass writes
        //   `ally` back on the partner (it *repairs* a one-sided pairing rather
        //   than dropping it, which is `l2_kingdom::diplomacy`'s own correction
        //   to `docs/diplomacy.md` §4.1);
        // * an in-play realm is not allied to a **lower-numbered** dead realm,
        // because the `handled` array the loop is filling can only ever see
        //   indices below the one being walked. A *higher*-numbered dead
        //   partner survives, and that asymmetry is the original's — see
        //   `docs/bugs.md` and `reconcile_alliances`, where it is reproduced
        //   deliberately. Asserting it away here would be asserting our own
        //   repair of a defect we chose to keep.
        if realm.in_play && realm.ally != 0 {
            let other = &k.realms[realm.ally as usize];
            if other.ally != r as u8 {
                return Err(format!(
                    "realm {r} is allied to {} and {} is allied to {}",
                    realm.ally, realm.ally, other.ally
                ));
            }
            if !other.in_play && (realm.ally as usize) < r {
                return Err(format!(
                    "realm {r} is allied to {}, which is out of play and below it — \
                     `Diplo_ReconcileAlliances` drops exactly this pairing",
                    realm.ally
                ));
            }
        }
        for other in 1..l2_kingdom::MAX_REALMS {
            let s = realm.pair(other as u8).standing;
            if !(l2_kingdom::diplomacy::STANDING_MIN..=l2_kingdom::diplomacy::STANDING_MAX)
                .contains(&s)
            {
                return Err(format!(
                    "realm {r}'s standing towards {other} is {s} — a write site did not clamp"
                ));
            }
        }
    }

    // Every unit sits in a county that exists and belongs to a realm in play.
    for (idx, u) in k.campaign.units.iter() {
        if u.county as usize > k.county_count {
            return Err(format!(
                "unit {idx} ({:?}) is in county {}, and the map has {}",
                u.kind, u.county, k.county_count
            ));
        }
        if u.men < 0 {
            return Err(format!("unit {idx} ({:?}) has {} men", u.kind, u.men));
        }
    }
    Ok(())
}

/// Play `turns` turns, checking the invariant after each and censusing what
/// fired. Returns the census, or panics naming the turn that broke.
fn play(game: &mut Game, turns: usize, label: &str) -> Census {
    let mut census = Census::default();
    if let Err(why) = invariant(&game.kingdom) {
        panic!("{label}: broken before a single turn was played: {why}");
    }
    for turn in 1..=turns {
        let Some(outcome) = l2_game::turn::end_turn(game) else {
            panic!("{label}: the turn machine stopped to ask a question on turn {turn}");
        };
        match outcome.outcome {
            l2_kingdom::victory::Outcome::InPlay => {}
            l2_kingdom::victory::Outcome::Won => census.fire("THE GAME WAS WON", turn),
            l2_kingdom::victory::Outcome::Lost => census.fire("THE GAME WAS LOST", turn),
        }
        if !outcome.battles.is_empty() {
            census.fire("a battle", turn);
        }
        for c in outcome.captures() {
            let _ = c;
            census.fire("a county changed hands", turn);
        }
        let messages =
            game.last_report.as_ref().map(|r| r.messages.clone()).unwrap_or_default();
        census.observe(&game.kingdom, &messages, turn);
        if let Err(why) = invariant(&game.kingdom) {
            census.print(label);
            panic!("{label}: turn {turn} (year {}) broke an invariant: {why}", game.kingdom.year);
        }
    }
    census
}

fn scoreline(k: &Kingdom, label: &str) {
    eprintln!("--- {label}, year {} ---", k.year);
    for r in 1..l2_kingdom::MAX_REALMS {
        let realm = &k.realms[r];
        let (mut pop, mut grain, mut hap) = (0, 0, 0);
        let (mut fields_grain, mut fields_waste, mut herd, mut crop) = (0, 0, 0, 0);
        let mut held = 0;
        let mut ids = Vec::new();
        for id in 1..=k.county_count {
            if k.counties[id].owner == r as u8 {
                held += 1;
                ids.push(id);
                pop += k.counties[id].population;
                grain += k.counties[id].grain;
                hap += k.counties[id].happiness;
                fields_grain += k.counties[id].fields_grain;
                fields_waste += k.counties[id].fields_waste;
                herd += k.counties[id].herd;
                crop += k.counties[id].crop.iter().sum::<i32>();
            }
        }
        let armies = k
            .campaign
            .units
            .iter()
            .filter(|(_, u)| u.owner == r as u8 && u.kind == UnitKind::Army)
            .count();
        let men: i32 = k
            .campaign
            .units
            .iter()
            .filter(|(_, u)| u.owner == r as u8 && u.kind == UnitKind::Army)
            .map(|(_, u)| u.men)
            .sum();
        eprintln!(
            "  realm {r} human={} in_play={} counties {held:>2} {ids:?} pop {pop:>7} \
             grain {grain:>7} crop {crop:>7} herd {herd:>5} grain fields {fields_grain:>3} \
             waste {fields_waste:>2} gold {:>7} happiness {:>3} armies {armies:>2} ({men} men) \
             tax_hap_empire {}",
            realm.is_human,
            realm.in_play,
            realm.gold,
            if held > 0 { hap / held } else { 0 },
            realm.tax_hap_empire,
        );
    }
    let unowned = (1..=k.county_count).filter(|&id| k.counties[id].owner == 0).count();
    eprintln!("  {unowned} of {} counties are nobody's", k.county_count);
}

// ---------------------------------------------------------------------------
// The runs.
// ---------------------------------------------------------------------------

/// **A hundred turns of England.** The real position,
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

    // **The multi-county rules are reachable without a dealt board.** This is
    // `docs/plan.md` §2.5's whole complaint: every fixture is turn one with one
    // county each. Left to run, England now produces a realm holding four.
    assert!(
        census.max_counties >= 4,
        "the largest realm after {TURNS} turns holds {} counties — the war is not \
         moving the map",
        census.max_counties
    );
    assert!(census.fired("a battle"), "a hundred turns of five realms and no battle");
}

/// **Four hundred turns of England — a century of play, in 0.8 seconds.**
///
/// It runs because it is the only thing in the workspace that can reach three
/// rules at all. `docs/plan.md` §2.5 says the late game has no oracle; this is
/// not an oracle, but it is the difference between a rule that is *unchecked*
/// and one that has *never executed*:
///
/// | rule | first fires on | reachable anywhere else? |
/// |---|---:|---|
/// | secession — `Realm_SecedeIsolatedCounties` | never, now — see below | **yes, on purpose**: [`a_realm_cut_in_two_loses_the_far_half_through_the_turn_machine`] |
/// | bankruptcy, through to the mutiny | turn 144 | no |
/// | a tax rate above 19, so `TAX_HAPPINESS_OTHER` is not its first row | turn 136 | no |
///
/// **Read the assertions below as trajectory assertions and not as invariants.**
/// They say *"a hundred years of England still contains a bankrupt lord"*, and
/// a change to any economic rule can move that legitimately. If one goes red,
/// read the census this test prints before assuming a defect: the question to
/// ask is whether the rule became **unreachable**, which is C27's failure and a
/// real one,
///
/// # It has gone red once, and this is the reading it asked for
///
/// `Unit_StepOnce`'s sub-tile counter (`docs/decisions.md` **C134**)
/// made a unit take 8 ticks to cross a road tile and 32 to cross open ground,
/// where every earlier build crossed one a tick. Tiles a *season* did not
/// change — the move allowance is the budget and it is untouched — but the
/// tick a unit arrives on did, and over four hundred turns that moves the
/// board. Measured, both ways, on this fixture:
///
/// | | before | after |
/// |---|---:|---:|
/// | battles in 400 turns | 65 | 30 |
/// | *THE GAME WAS WON* | turn 212 | never |
/// | first bankruptcy | turn 144 | turn 480 |
/// | mutiny (stage 5) | turn 151 | turn 103 |
/// | a tax rate ≥ 20 | turn 136 | turn 78 |
///
/// **The mechanism is interception.** Half as many battles, because an army
/// sent at an enemy now spends most of a season walking
/// moved by the time it arrives — which is what the original does, at the
/// original's speed. Realm 3 runs away with the map but cannot catch realm 2's
/// last 42-man army, so nobody wins, so the endgame collapse that used to
/// bankrupt the losers never happens inside four hundred turns.
///
/// So: **late, not unreachable** — bankruptcy and its desertion arm still fire,
/// at 480 and 486, and the horizon here is [`TURNS_LONG`].
///
/// # The last two rows read `not in 1200`, and they were wrong
///
/// They were one build's trajectory written down as a property of the fixture.
/// On this one the mutiny fires 3 times from turn 103 and a tax rate ≥ 20 is
/// carried 25 times from turn 78. Both are the binary's rules, and neither
/// needs the dealt board this said they needed:
///
/// * **The mutiny re-fires because the counter wraps.** `Wages_PayAll`
/// (`0x004ACBD4`) sets stage 5 → 0 at the mutiny, so a
///   realm that never pays loses its armies every six seasons for ever.
/// * **A tax rate ≥ 20 is on no ladder at all.** `AI_SetTaxRates`' four ladders
///   top out at 15 ([`l2_kingdom::tables::AI_TAX_LADDERS`]). The rates above 19
///   come from `FUN_0049F431`, AI step 7's abandon pass, which sets 32, 28, 23
///   or 35 by the lord's personality on a county it has decided it cannot hold
///   — [`l2_kingdom::tables::AI_PERSONALITY_ABANDON_TAX_RATE`], `[V]`.
///
/// So both assertions are back, as trajectory assertions like the two above.
///
/// # Red a second time, and that assertion was wrong
///
/// `C184` moved the trajectory again and `SECESSION` stopped firing.
/// `Realm_SecedeIsolatedCounties` (`0x0044AE3C`) takes a county only from a
/// realm holding two or more contiguity blocks, and contiguity is the county
/// neighbour list at `+0x5C` and nothing else (`docs/kingdom.md` §6.1). On this
/// fixture — 14 counties, 39 undirected edges — **county 2 is the only cut
/// vertex**: county 1's list holds nothing but county 2, and removing any other
/// county leaves the rest connected. So the pass can fire here only when a realm
/// holds county 1 and something past a county 2 that is not its own — enemy or
/// neutral alike, the partition being same-owner adjacency. That is a fact about
/// where the armies went, not about a rule.
///
/// The reachability claim therefore moved to
/// [`a_realm_cut_in_two_loses_the_far_half_through_the_turn_machine`], which
/// deals the cut. What stays here is the invariant: nobody is left holding two
/// blocks.
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
    // The invariant, not the trajectory: `SECESSION` firing needs this map's
    // one cut vertex to be dealt, so its absence says nothing about the pass.
    assert!(
        census.max_blocks <= 1,
        "a realm ended a turn holding {} contiguity blocks — \
         `Realm_SecedeIsolatedCounties` did not take the outlying one, so the \
         pass is not running at all, which is `docs/decisions.md` C27's shape",
        census.max_blocks
    );
}

/// **A realm cut in two, dealt on purpose.** County 1's neighbour list holds
/// nothing but county 2, so handing county 1's owner a second county past it
/// splits the realm into `{1}` and `{3}`. County 2 need not be an enemy's:
/// `Territory_ExtendBlock` joins through `County_IsNeighbour` (`0x00467E2C`) on
/// **same-owner** adjacency,
/// kept depends on populations the season moves, so nothing below names one.
fn england_cut_in_two() -> Option<Game> {
    let save = match l2_testkit::england_turn1() {
        l2_testkit::FixtureState::Ready(s) => *s,
        _ => return None,
    };
    let mut game =
        l2_game::scenario::from_save(&save, Tables::DEFAULT).expect("the fixture loads");
    let owner_of_1 = game.kingdom.counties[1].owner;
    assert_ne!(owner_of_1, 0, "the fixture's county 1 is owned by somebody");
    assert_eq!(game.kingdom.counties[3].owner, 0, "and its county 3 is neutral");
    game.kingdom.counties[3].owner = owner_of_1;
    l2_kingdom::conquest::recount_realm_counties(&game.kingdom.counties, &mut game.kingdom.realms);
    Some(game)
}

/// **`Realm_SecedeIsolatedCounties` (`0x0044AE3C`) out of a played turn.**
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
    // `County_MakeIndependent` leaves it a *consistent* neutral county, which is
    // the half of the rule a test that only reads `owner` would miss.
    assert!(
        game.kingdom.counties[gone].industry.iter().all(|i| !i.enabled),
        "every industry switched off"
    );
    assert_eq!(
        census.max_blocks, 1,
        "and nobody is left holding two blocks once the pass has run"
    );
}

/// **A realm holding many counties**, which is the position `docs/plan.md`
/// §2.5 says the project has no evidence about at all.
///
/// The board is dealt by hand — realm 2 takes six of England's fourteen
/// person takes four — and then the rules run. **Nothing here asserts a value**,
/// because a dealt board is not an oracle for anything; what it does is make
/// the multi-county rules *reachable*, which is what §2.5 asks for.
fn england_with_an_empire() -> Option<Game> {
    let save = match l2_testkit::england_turn1() {
        l2_testkit::FixtureState::Ready(s) => *s,
        _ => return None,
    };
    let mut game =
        l2_game::scenario::from_save(&save, Tables::DEFAULT).expect("the fixture loads");
    // Realm 2 takes the west, the person the east; the rest stay neutral.
    for (id, owner) in [(1u8, 2u8), (2, 2), (3, 2), (4, 2), (5, 2), (6, 2), (11, 1), (12, 1), (13, 1), (14, 1)]
    {
        game.kingdom.counties[id as usize].owner = owner;
    }
    // A tax rate the empire term can see: `TAX_HAPPINESS_OTHER` is
    // flat zero from rate 0 to 19 (`docs/decisions.md` C26), so every fixture
    // this project has exercises the one value of the input that cannot
    // distinguish the table from a constant.
    for id in 1..=game.kingdom.county_count {
        if game.kingdom.counties[id].owner != 0 {
            game.kingdom.counties[id].tax_rate = 30;
        }
    }
    l2_kingdom::conquest::recount_realm_counties(&game.kingdom.counties, &mut game.kingdom.realms);
    Some(game)
}

#[test]
fn a_hundred_turns_of_an_empire_taxed_at_thirty() {
    let Some(mut game) = england_with_an_empire() else {
        l2_testkit::skip!("no England fixture");
    };
    scoreline(&game.kingdom, "empire, turn 1");
    let census = play(&mut game, TURNS, "empire");
    scoreline(&game.kingdom, "empire, turn 100");
    census.print("empire, 100 turns");

    // **Reachability, not values.** Each of these is a rule that had no way in
    // before this file existed; the assertion is that it has one now,
    // message says which rule stayed dark.
    for what in [
        "a realm holding more than one county",
        "a realm holding four or more counties",
        "a tax rate the empire term can see (>= 20)",
        "TAX_HAPPINESS_OTHER non-zero",
        "Realm::tax_hap_empire non-zero",
    ] {
        assert!(census.fired(what), "after {TURNS} turns of a six-county empire, `{what}` never happened");
    }
}

/// **All 44 shipped maps, twenty turns each.** `docs/decisions.md` C26's
/// warning applied to the map
/// forty-four, and `tests/newgame.rs` only takes *one* turn in each.
#[test]
fn every_shipped_map_survives_twenty_turns() {
    let Some(dir) = l2_testkit::install_dir() else {
        l2_testkit::skip!("no game install, so no L2_maps.dat");
    };
    let platform = l2_mods::Platform::builder().base(&dir).build().expect("the install mounts");
    let assets = l2_game::game::Assets::load(&platform.vfs).expect("assets load");
    let bytes = l2_testkit::read_install("L2_maps.dat").expect("the install has it");
    let set = l2_formats::maps::MapSet::parse(&bytes).expect("L2_maps.dat parses");

    let mut played = 0;
    let mut worst: Vec<String> = Vec::new();
    for slot in set.used_slots() {
        if slot >= 60 {
            continue;
        }
        let seats = set.slot(slot).unwrap().player_start_count();
        let lords = seats.min(5).max(1);
        let settings =
            l2_game::setup::SetupOptions::new().commit(1, l2_kingdom::Quirks::default());
        let settings = l2_game::setup::Settings { ai_lords: lords as i32 - 1, ..settings };
        // **A different colour on every map**, cycling 1 … 5 across the 44.
        // The shield moves which realm flies which colour *and which lord sits
        // behind it*,
        // one arrangement `docs/rules.md` §7a's first row describes and leave
        // the other four rows never simulated at all.
        let shield = (slot % 5 + 1) as u8;
        let mut game = l2_game::scenario::new_game(
            &assets,
            slot,
            &settings,
            1,
            shield,
            l2_game::scenario::SEED,
            Tables::DEFAULT,
        )
        .unwrap_or_else(|e| panic!("slot {slot}: {e}"));
        settings.apply_to(&mut game);
        game.kingdom.start_new_game();
        let census = play(&mut game, 20, &format!("map slot {slot}"));
        let alive = (1..l2_kingdom::MAX_REALMS)
            .filter(|&r| game.kingdom.realms[r].in_play)
            .count();
        worst.push(format!(
            "slot {slot:>2}: {:>3} counties, {lords} lords, {alive} alive after 20 turns, \
             max held {}",
            game.kingdom.county_count, census.max_counties
        ));
        played += 1;
    }
    for line in &worst {
        eprintln!("  {line}");
    }
    assert_eq!(played, 44, "every shipped map should have been played");
}

/// **A hundred turns is also the best determinism test available.**
/// `docs/netcode.md`: the simulation must be a pure function of its seed and
/// its inputs. Two runs of the same hundred turns must agree bit for bit, and a
/// game saved at turn 50 and reloaded must produce the same turn 100 as one
/// played straight through.
#[test]
fn a_hundred_turns_is_the_same_hundred_however_it_is_reached() {
    let save = l2_testkit::england!();
    let fresh =
        || l2_game::scenario::from_save(&save, Tables::DEFAULT).expect("the fixture loads");
    let digest = |game: &Game| {
        let mut c = l2_net::Canonical::hashing();
        l2_net::Encode::encode(&game.kingdom, &mut c);
        c.finish()
    };

    let mut straight = fresh();
    for _ in 0..TURNS {
        l2_game::turn::end_turn(&mut straight).expect("the machine comes round");
    }

    // 1. The same hundred turns, twice.
    let mut again = fresh();
    for _ in 0..TURNS {
        l2_game::turn::end_turn(&mut again).expect("the machine comes round");
    }
    assert_eq!(
        digest(&straight).hash,
        digest(&again).hash,
        "two identical hundred-turn games diverged"
    );

    // 2. Fifty turns, a save, a load, and fifty more. This is the assertion
    //    `tests/save.rs` makes over ten seasons, at ten times the length — and
    //    it is the one that catches a field the codec drops, because fifty more
    //    turns of divergence is a long time for a wrong value to stay invisible.
    let mut halved = fresh();
    for _ in 0..TURNS / 2 {
        l2_game::turn::end_turn(&mut halved).expect("the machine comes round");
    }
    let bytes = l2_game::save::encode(&halved);
    let mut reloaded = l2_game::save::decode(&bytes, Tables::DEFAULT).expect("our own save loads");
    for _ in 0..TURNS / 2 {
        l2_game::turn::end_turn(&mut reloaded).expect("the machine comes round");
    }
    assert_eq!(
        digest(&straight).hash,
        digest(&reloaded).hash,
        "a game saved at turn {} and reloaded reached a different turn {TURNS}",
        TURNS / 2
    );
}

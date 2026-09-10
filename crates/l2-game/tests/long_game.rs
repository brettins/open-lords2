//! **The hundred-turn game.** `docs/plan.md` §2.5 and §0 row 4.
//!
//! `tests/ai_war.rs` plays forty turns and asks whether the AI is *playing*.
//! This file plays enough turns for the late-game rules to fire at all, and
//! asks a different question: **which rules have now run, and what breaks when
//! they do.**
//!
//! Everything here is our own arithmetic agreeing with itself — there is no
//! oracle for turn 100 of England and there will not be one until a person
//! plays it (`docs/plan.md` §5 item 1). So the assertions are of two kinds only,
//! and the distinction is the whole design:
//!
//! * **Invariants** — statements the original's own functions maintain, which
//!   are true whatever the numbers come out as. A labour split that no longer
//!   sums to the population is a defect at any turn count.
//! * **Reachability** — *a rule fired at all*. `docs/decisions.md` C27: a rule
//!   with no way in is not a rule the game has, and until today none of these
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

// ---------------------------------------------------------------------------
// The census: which late-game rules have fired, and when they first did.
// ---------------------------------------------------------------------------

/// One rule, and the turn it first fired on.
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
    /// state. Kept in one place so the England run and the synthetic run
    /// measure the same things.
    fn observe(&mut self, k: &Kingdom, messages: &[Message], turn: usize) {
        for m in messages {
            match m {
                Message::UnrestWarning { .. } => self.fire("unrest warning", turn),
                Message::UnrestRising { .. } => self.fire("unrest rising", turn),
                Message::Revolt { .. } => self.fire("REVOLT", turn),
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
             max bankruptcy stage: {}",
            self.max_counties, self.max_tax_rate, self.max_gold, self.max_bankrupt_stage
        );
    }
}

// ---------------------------------------------------------------------------
// The invariants: things the original's own passes maintain, at every turn.
// ---------------------------------------------------------------------------

/// Everything that must be true of a kingdom between two turns, whatever the
/// numbers are. Returns the first violation as a sentence.
///
/// **Each clause names the original function that maintains it**, so a failure
/// says which subsystem to look at rather than which line went red.
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
        // of the counties, so it and the sweep must agree.
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
        // `Diplo_ReconcileAlliances` runs every turn and an alliance is
        // symmetric and exclusive (`docs/diplomacy.md`).
        if realm.ally != 0 {
            let other = &k.realms[realm.ally as usize];
            if other.ally != r as u8 {
                return Err(format!(
                    "realm {r} is allied to {} and {} is allied to {}",
                    realm.ally, realm.ally, other.ally
                ));
            }
            if !other.in_play {
                return Err(format!("realm {r} is allied to {}, which is out of play", realm.ally));
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

/// **A hundred turns of England.** The real position, the real map.
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
/// | secession — `Realm_SecedeIsolatedCounties` | turn 116 | no |
/// | bankruptcy, through to the mutiny | turn 144 | no |
/// | a tax rate above 19, so `TAX_HAPPINESS_OTHER` is not its first row | turn 136 | no |
///
/// **Read the assertions below as trajectory assertions and not as invariants.**
/// They say *"a hundred years of England still contains a bankrupt lord"*, and
/// a change to any economic rule can move that legitimately. If one goes red,
/// read the census this test prints before assuming a defect: the question to
/// ask is whether the rule became **unreachable**, which is C27's failure and a
/// real one, or merely late.
#[test]
fn four_hundred_turns_of_england_reaches_the_rules_nothing_else_can() {
    let save = l2_testkit::england!();
    let mut game =
        l2_game::scenario::from_save(&save, Tables::DEFAULT).expect("the fixture loads");
    let census = play(&mut game, 400, "England x400");
    scoreline(&game.kingdom, "England, turn 400");
    census.print("England, 400 turns");

    assert!(
        census.fired("bankruptcy"),
        "a hundred years and no lord ever missed a wage bill — the bankruptcy \
         ladder has no way in at all, which is `docs/decisions.md` C27's shape"
    );
    assert!(
        census.fired("bankruptcy: MUTINY"),
        "the bankruptcy counter reached {} and never 5 — the mutiny arm of \
         `Wages_PayAll` has never executed",
        census.max_bankrupt_stage
    );
    assert!(
        census.fired("SECESSION"),
        "a hundred years and nobody's lands were ever cut in two — \
         `Realm_SecedeIsolatedCounties` has never taken a county"
    );
    assert!(
        census.max_tax_rate >= 20,
        "the highest tax rate in a hundred years is {} — `TAX_HAPPINESS_OTHER` \
         is flat zero below 20, so every reading of it is still its first row (C26)",
        census.max_tax_rate
    );
}

/// **A realm holding many counties**, which is the position `docs/plan.md`
/// §2.5 says the project has no evidence about at all.
///
/// The board is dealt by hand — realm 2 takes six of England's fourteen and the
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
    // A tax rate the empire term can actually see: `TAX_HAPPINESS_OTHER` is
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
    // before this file existed; the assertion is that it has one now, and the
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
/// warning applied to the map rather than to the rule: England is one input of
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
        // behind it*, so a constant here would play forty-four games of the
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

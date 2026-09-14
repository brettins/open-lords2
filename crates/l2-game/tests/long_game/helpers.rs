#![allow(unused_imports)]
use super::*;
use super::england::*;
use super::scenarios::*;
use std::collections::BTreeMap;
use l2_game::game::Game;
use l2_kingdom::report::Message;
use l2_kingdom::tables::Tables;
use l2_kingdom::{Kingdom, UnitKind};

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


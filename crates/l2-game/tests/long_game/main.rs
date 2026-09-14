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

mod helpers;
pub use helpers::*;
mod england;
pub use england::*;
mod scenarios;
pub use scenarios::*;

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
    pub(crate) fn fire(&mut self, what: &'static str, turn: usize) {
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


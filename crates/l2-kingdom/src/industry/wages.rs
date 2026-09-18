#![allow(unused_imports)]
use super::*;
use super::production::*;
use super::castles::*;
use super::ui::*;
use crate::county::County;
use crate::math::pct;
use crate::realm::Realm;
use crate::report::Message;
use crate::tables::{Commodity, Tables, RESOURCE_LIMIT_UNLIMITED, WEAPON_TYPE_COUNT};
use l2_net::{Quirk, Quirks};

/// `Wages_PayAll` (`0x004ACBD4`) sums [`Realm::wage_for_unit`] over a realm's
/// units.
///
/// `men` is campaign unit `+0x168`, the field `docs/battle.md` §4.1 identified
/// as the total over troop types 0..=6. Units live in `g_units`, which is not
/// this crate's, so the caller supplies the per-unit totals in a stable order.
pub fn compute_wages(t: &Tables, realm: &Realm, unit_men: &[i32], difficulty: u8) -> i32 {
    let mut total: i64 = 0;
    for &men in unit_men {
        total += realm.wage_for_unit(t, men, difficulty) as i64;
    }
    total as i32
}

/// `had_mercenaries` is `FUN_004AD230`'s return: it dismisses the realm's
/// mercenaries as a side effect and reports whether there were any. Armies are
/// not this crate's, so the caller answers,
/// [`BankruptcyAction`] tells the caller what to do to them.
pub fn pay(
    realm: &mut Realm,
    id: u8,
    had_mercenaries: bool,
    out: &mut Vec<Message>,
) -> BankruptcyAction {
    if realm.gold >= realm.wages {
        realm.gold -= realm.wages;
        realm.bankrupt_stage = 0;
        return BankruptcyAction::None;
    }
    let action = match realm.bankrupt_stage {
        0 => {
            realm.bankrupt_stage = 1;
            if had_mercenaries {
                BankruptcyAction::MercenariesDesert
            } else {
                BankruptcyAction::Warned
            }
        }
        1..=3 => {
            realm.bankrupt_stage += 1;
            BankruptcyAction::Desertion
        }
        4 => {
            realm.bankrupt_stage = 5;
            BankruptcyAction::LastWarning
        }
        _ => {
            realm.bankrupt_stage = 0;
            BankruptcyAction::Mutiny
        }
    };
    out.push(Message::Bankrupt { realm: id, stage: realm.bankrupt_stage, action });
    action
}


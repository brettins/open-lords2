#![allow(unused_imports)]
use super::*;
use super::standing::*;
use super::inbox::*;
use super::ai::*;
use super::tests::*;
use crate::realm::{Realm, MAX_REALMS};
use crate::tables::Tables;
use l2_net::Pcg32;

/// Kind 0 — `Diplo_ReplyGift` (`0x004A29B2`).
///
/// | gift | reply | standing |
/// |---|---|---:|
/// | `< best + T/2` | group 173 | **−8** |
/// | `< best + T` | group 172 | **+5** |
/// | `>= best + T` | group 171 | **+10** |
pub fn reply_gift(
    realms: &mut [Realm],
    tables: &Tables,
    me: u8,
    them: u8,
    gold: i32,
) -> Vec<Letter> {
    if !realms[them as usize].is_human {
        return Vec::new();
    }
    let Some(p) = tables.ai_personality(realms[me as usize].lord) else { return Vec::new() };
    let best = realms[me as usize].pair(them).best_gift;
    let increment = p.gift_increment;
    let (group, delta) = if gold < best + increment / 2 {
        (group::GIFT_CONTEMPTUOUS, -8)
    } else if gold < best + increment {
        (group::GIFT_GRUDGING, 5)
    } else {
        (group::GIFT_PLEASED, 10)
    };
    if best < gold {
        realms[me as usize].pair_mut(them).best_gift = gold;
    }
    move_standing(realms, me, them, delta);
    vec![speak(realms, me, them, group, category::LETTER, 0)]
}

/// Kind 1 — `Diplo_ReplyCompliment` (`0x004A2CD6`).
///
/// On `compliments_from`, which [`post`] incremented before this ran and which
/// **nothing ever resets**: 1 → group 174 and **+15**, 2 → group 175 and
/// **+8**, 3 or more → group 176 and **−4**.
///
/// So compliments are worth +23 in total, once, and cost 4 apiece for ever
/// after — and group 176 says the same in words: *"You are boring me now with
/// your groveling letters. Do not send me any more."*
pub fn reply_compliment(realms: &mut [Realm], me: u8, them: u8) -> Vec<Letter> {
    if !realms[them as usize].is_human {
        return Vec::new();
    }
    let n = realms[me as usize].pair(them).compliments_from;
    let (group, delta) = match n {
        0 | 1 => (group::COMPLIMENT_FIRST, 15),
        2 => (group::COMPLIMENT_SECOND, 8),
        _ => (group::COMPLIMENT_ENOUGH, -4),
    };
    move_standing(realms, me, them, delta);
    vec![speak(realms, me, them, group, category::LETTER, 0)]
}

/// Kind 2 — `Diplo_ReplyInsult` (`0x004A2F61`). Breaks any alliance between the
/// two, costs **−20**, and replies with group 177.
pub fn reply_insult(realms: &mut [Realm], me: u8, them: u8) -> Vec<Letter> {
    if !realms[them as usize].is_human {
        return Vec::new();
    }
    break_alliance(realms, me, them);
    move_standing(realms, me, them, -20);
    vec![speak(realms, me, them, group::INSULT, category::LETTER, 0)]
}

/// Kind 3 — `Diplo_ReplyAllianceOffer` (`0x004A30F6`).
pub fn reply_alliance_offer(
    realms: &mut [Realm],
    diplomacy: &mut Diplomacy,
    me: u8,
    them: u8,
    realms_active: usize,
) -> Vec<Letter> {
    let standing = realms[me as usize].pair(them).standing;
    let group = if realms[me as usize].pair(them).at_war {
        group::ALLIANCE_AT_WAR
    } else if realms_active < 3 {
        group::ALLIANCE_REFUSED
    } else if realms[me as usize].ally != 0 {
        group::ALLIANCE_HAVE_ALLY
    } else if realms[them as usize].ally != 0 {
        group::ALLIANCE_REFUSED
    } else if standing >= 11 {
        group::ALLIANCE_ACCEPTED
    } else if standing < -10 {
        group::ALLIANCE_REFUSED
    } else if diplomacy.dice.rand7b() < standing as i32 * 3 + 45 {
        group::ALLIANCE_ACCEPTED
    } else {
        group::ALLIANCE_REFUSED
    };
    if realms[me as usize].offer_timer != 0 {
        realms[me as usize].offer_timer -= 1;
    }
    match group {
        group::ALLIANCE_REFUSED => move_standing(realms, me, them, -2),
        group::ALLIANCE_ACCEPTED => {
            move_standing(realms, me, them, 4);
            form_alliance(realms, me, them);
        }
        _ => move_standing(realms, me, them, -1),
    }
    vec![speak(realms, me, them, group, category::LETTER, 0)]
}

/// Kind 4 — `Diplo_ReplyAllianceEnd` (`0x004A3475`). Breaks the alliance, costs
/// **−15**, and **sends nothing at all**: terminating an alliance from the
/// diplomacy screen is silent.
///
/// The *"Broken alliance."* text, group 182, is not this — it comes from
/// [`offend`] when an *act* breaks one.
pub fn reply_alliance_end(realms: &mut [Realm], me: u8, them: u8) -> Vec<Letter> {
    break_alliance(realms, me, them);
    move_standing(realms, me, them, -15);
    Vec::new()
}

/// Kind 5 — `Diplo_ReplyHelpRequest` (`0x004A3551`).
///
/// ```text
/// if they are not my ally                            refuse
/// else if my mean population < personality[+0x30]    refuse
/// else if standing < 10                              refuse
/// else if rand7B < standing * 4                      accept, and march for free
/// else                                               "pay me"
/// ```
///
/// > **`personality +0x30` is a population floor, not a treasury one.**
///
/// > `docs/diplomacy.md` §3.5 calls it *"a treasury floor … below which an ally
/// > will not move at all"*; the comparison is against realm `+0x14`, the mean
/// > population of the realm's counties, which is
/// > [`crate::realm::Realm::population_mean`]. §8.4's own note that the same
/// > field is *"also a county-population floor at step 9"* is the
/// > corroboration: it is a population floor in both places.
///
/// A refusal adds **+1 grudge** — the lord resents being asked. Accepting calls
/// [`pay_for_help`] with a price of **zero**, so the ally's `target_county`
/// becomes the county and it marches. *Pay me* sets [`Diplomacy::help_price`]
/// to `personality[+0x0C] × helpPriceMultiple` and raises a category-10 prompt;
/// clicking it runs [`pay_for_help`] for real.
pub fn reply_help_request(
    realms: &mut [Realm],
    diplomacy: &mut Diplomacy,
    tables: &Tables,
    me: u8,
    them: u8,
    county: u8,
) -> Vec<Letter> {
    reply_request(
        realms,
        diplomacy,
        tables,
        me,
        them,
        county,
        RequestShape {
            refused: group::HELP_REFUSED,
            agreed: group::HELP_AGREED,
            priced: group::HELP_PRICED,
            odds: 4,
            grudge: 1,
        },
    )
}

/// Kind 6 — `Diplo_ReplyAttackRequest` (`0x004A38DB`). The same shape as
/// [`reply_help_request`] with three constants moved: groups 186/187/188, a
/// **+2** grudge on refusal, and **half** the acceptance odds (`standing × 2`).
pub fn reply_attack_request(
    realms: &mut [Realm],
    diplomacy: &mut Diplomacy,
    tables: &Tables,
    me: u8,
    them: u8,
    county: u8,
) -> Vec<Letter> {
    reply_request(
        realms,
        diplomacy,
        tables,
        me,
        them,
        county,
        RequestShape {
            refused: group::ATTACK_REFUSED,
            agreed: group::ATTACK_AGREED,
            priced: group::ATTACK_PRICED,
            odds: 2,
            grudge: 2,
        },
    )
}

struct RequestShape {
    refused: u16,
    agreed: u16,
    priced: u16,
    odds: i32,
    grudge: u8,
}

fn reply_request(
    realms: &mut [Realm],
    diplomacy: &mut Diplomacy,
    tables: &Tables,
    me: u8,
    them: u8,
    county: u8,
    shape: RequestShape,
) -> Vec<Letter> {
    let Some(p) = tables.ai_personality(realms[me as usize].lord) else { return Vec::new() };
    let standing = realms[me as usize].pair(them).standing as i32;
    let group = if realms[me as usize].ally != them {
        shape.refused
    } else if realms[me as usize].population_mean < p.help_population_floor {
        shape.refused
    } else if standing < 10 {
        shape.refused
    } else if diplomacy.dice.rand7b() < standing * shape.odds {
        shape.agreed
    } else {
        shape.priced
    };
    if group == shape.refused {
        let pair = realms[me as usize].pair_mut(them);
        pair.grudge = pair.grudge.wrapping_add(shape.grudge);
        return vec![speak(realms, me, them, group, category::LETTER, county)];
    }
    if group == shape.agreed {
        let letter = speak(realms, me, them, group, category::LETTER, county);
        pay_for_help(realms, me, them, county, 0);
        return vec![letter];
    }
    diplomacy.help_county = county;
    diplomacy.help_price =
        p.help_price * realms[me as usize].pair(them).help_price_multiple as i32;
    vec![speak(realms, me, them, group, category::PAY_PROMPT, county)]
}

/// `Diplo_PayForHelp(ally, payer, county, price)` (`0x004A1B29`).
pub fn pay_for_help(realms: &mut [Realm], ally: u8, payer: u8, county: u8, price: i32) {
    if price > realms[payer as usize].gold {
        return;
    }
    realms[payer as usize].gold -= price;
    realms[ally as usize].gold += price;
    let p = realms[ally as usize].pair_mut(payer);
    p.help_price_multiple = p.help_price_multiple.wrapping_add(1);
    p.standing = p.standing.wrapping_sub(4).max(STANDING_MIN);
    realms[ally as usize].target_county = county;
}


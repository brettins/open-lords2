//! **What an army says when it crosses into a county** — the posting half of
//! `Unit_EnterCounty` (`0x004ABB36`) and all of `County_GreetArmy`
//! (`0x004ABF77`).
//!
//! A player on build `73DF34969`: *"county did not give me a message when I
//! moved an army into it."* Both functions had been read — `docs/armies.md`
//! §2.5 carries the greeting table — and [`crate::units_tick`] reproduced the
//! one line of `Unit_EnterCounty` the invasion tip reads. Neither of its
//! `Msg_Enqueue` calls had been built. These are they. `[V]`:
//!
//! ```c
//! void Unit_EnterCounty(int unit, uint county) {
//!     g_units[unit].county = county;
//!     County_GreetArmy(unit, county);
//!     if (county.owner == g_localPlayer && g_selectedCounty == county) DAT_005530d0 = 1;
//!     if (county.owner != unit.owner) {
//!         if (unit.owner == g_localPlayer) DAT_00553210 = 1;       /* the invasion tip */
//!         if (county.owner != 0 && unit.destCounty == county) {
//!             Msg_Enqueue(unit.owner, county.owner, 0xAA,
//!                         realm[unit.owner].voiceRotation - 4 + realm[unit.owner].lord * 4,
//!                         1, county, 0, 0);
//!             if (3 < ++realm[unit.owner].voiceRotation) realm[unit.owner].voiceRotation = 0;
//!         }
//!     }
//! }
//! ```
//!
//! | the county the army crosses into | letter | category | to |
//! |---|---|---|---|
//! | neutral | `L2.eng` 130…134, by mood and by the army's size — [`greeting`] | 2 | the army's owner |
//! | another lord's, **and the army's destination** | 170, one of sixteen lord-flavoured taunts | 1 | the county's owner |
//! | another lord's, marched through | nothing | | |
//! | the army's own | nothing | | |
//!
//! **Nothing else.** `Unit_EnterCounty` has no test for a siege, an army
//! standing in the county, or a castle. What happens at a town or a castle is
//! `Unit_Step`'s — `Army_AttackCounty` and `Unit_ReachCastleBuilding` — and the
//! letter a capture posts is `County_ChangeOwner`'s,
//! [`crate::conquest::Capture`]. `Army_Tick` (`0x0046521F`) is the only caller,
//! so a merchant, a cart or a mob of peasants says nothing; the mob's own
//! crossing function (`FUN_004ABD0F`, groups 154…156) is a different rule.

use crate::county::{County, MAX_COUNTIES};
use crate::diplomacy::{category, Letter};
use crate::industry::pct_of;
use crate::realm::{Realm, MAX_REALMS};
use crate::unit::Unit;

/// `L2.eng` 130 — *"The people are wretched, my liege…"*. Message id `0x82`.
pub const GROUP_WRETCHED: u16 = 0x82;
pub const GROUP_WELCOME: u16 = 0x83;
pub const GROUP_NO_NOTICE: u16 = 0x84;
pub const GROUP_UNACCEPTABLE: u16 = 0x85;
pub const GROUP_OUTRAGE: u16 = 0x86;
pub const GROUP_INVASION: u16 = 0xAA;

/// `Msg_DrawWindow`'s category 2 — the portrait panel with `L2.eng` 109/1,
/// *"An envoy from"*, over the county's name. Every greeting is posted in it.
pub const CATEGORY_ENVOY: u8 = 2;

/// **`County_GreetArmy` (`0x004ABF77`)** — a neutral county's reply to an army
/// walking in. `[V]`:
pub fn greeting(county: u8, c: &County, unit: &Unit) -> Option<Letter> {
    if c.owner != 0 {
        return None;
    }
    let pct = if unit.men < c.population { pct_of(unit.men, c.population) } else { 101 };
    let group = if c.happiness < 10 {
        GROUP_WRETCHED
    } else if c.happiness < 30 {
        GROUP_WELCOME
    } else if pct < 101 {
        if pct < 50 {
            if pct < 20 {
                GROUP_OUTRAGE
            } else {
                GROUP_UNACCEPTABLE
            }
        } else {
            GROUP_NO_NOTICE
        }
    } else {
        GROUP_WELCOME
    };
    Some(Letter {
        from: 0,
        to: unit.owner,
        group,
        variant: 0,
        category: CATEGORY_ENVOY,
        county,
        payload: 0,
    })
}

pub fn enter_county(
    counties: &[County; MAX_COUNTIES],
    realms: &mut [Realm; MAX_REALMS],
    unit: &Unit,
    county: u8,
) -> Option<Letter> {
    let c = counties.get(county as usize)?;
    if let Some(greeting) = greeting(county, c, unit) {
        return Some(greeting);
    }
    if c.owner == unit.owner || c.owner == 0 || unit.dest_county != county {
        return None;
    }
    let realm = realms.get_mut(unit.owner as usize)?;
    let variant = (realm.voice_rotation as u32)
        .wrapping_sub(4)
        .wrapping_add(realm.lord as u32 * 4) as u8;
    realm.voice_rotation = realm.voice_rotation.wrapping_add(1);
    if realm.voice_rotation > 3 {
        realm.voice_rotation = 0;
    }
    Some(Letter {
        from: unit.owner,
        to: c.owner,
        group: GROUP_INVASION,
        variant,
        category: category::LETTER,
        county,
        payload: 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::unit::UnitKind;

    fn counties() -> [County; MAX_COUNTIES] {
        core::array::from_fn(|_| County::new())
    }

    fn army(owner: u8, men: i32) -> Unit {
        let mut u = Unit::new(UnitKind::Army, owner, 10, 10);
        u.men = men;
        u
    }

    /// **Every band of the ladder, and both edges of each.** A thousand people;
    /// the numbers are typed
    /// ablating a threshold cannot move a probe with it.
    ///
    /// Ablation: swap `pct < 20` for `pct < 21` and the 20-man row goes red;
    /// drop the happiness tests and the first three do.
    #[test]
    fn a_neutral_county_answers_by_its_mood_first_and_the_armys_size_second() {
        let cases: [(i32, i32, u16, &str); 10] = [
            (9, 5_000, 0x82, "wretched, however big the army"),
            (10, 5_000, 0x83, "unhappy is a welcome"),
            (29, 10, 0x83, "and still a welcome for ten men"),
            (30, 1_000, 0x83, "as many men as people is not below it: 101, a welcome"),
            (30, 999, 0x84, "99%"),
            (30, 500, 0x84, "50%"),
            (30, 499, 0x85, "49%"),
            (30, 200, 0x85, "20%"),
            (30, 199, 0x86, "19% is an outrage"),
            (77, 1, 0x86, "the shipped happiness and a scout"),
        ];
        for (happiness, men, group, why) in cases {
            let mut c = counties();
            c[2].population = 1_000;
            c[2].happiness = happiness;
            let l = greeting(2, &c[2], &army(3, men)).expect("a neutral county always answers");
            assert_eq!(l.group, group, "{why}");
            assert_eq!((l.from, l.to, l.county, l.category, l.variant), (0, 3, 2, 2, 0), "{why}");
        }
    }

    #[test]
    fn a_county_somebody_owns_does_not_greet() {
        let mut c = counties();
        c[2].owner = 4;
        c[2].happiness = 5;
        assert_eq!(greeting(2, &c[2], &army(3, 100)), None);
    }

    /// Ablation: delete `unit.dest_county != county` and the passing-through
    /// line goes red.
    #[test]
    fn a_lord_writes_only_to_the_county_he_is_marching_on() {
        let mut c = counties();
        c[1].owner = 1;
        c[2].owner = 2;
        let mut realms: [Realm; MAX_REALMS] = core::array::from_fn(|_| Realm::new());
        realms[2].lord = 3;
        realms[2].voice_rotation = 1;

        let mut u = army(2, 300);
        u.dest_county = 5;
        assert_eq!(enter_county(&c, &mut realms, &u, 1), None, "passing through");
        assert_eq!(realms[2].voice_rotation, 1, "and nothing was posted, so nothing advanced");
        assert_eq!(enter_county(&c, &mut realms, &u, 2), None, "his own county");

        u.dest_county = 1;
        let l = enter_county(&c, &mut realms, &u, 1).expect("the destination");
        assert_eq!((l.from, l.to, l.group, l.category, l.county), (2, 1, 170, 1, 1));
        assert_eq!(l.variant, 9, "lord 3, rotation 1: 3 * 4 + 1 - 4");
        assert_eq!(realms[2].voice_rotation, 2);
    }

    #[test]
    fn the_invasion_letter_advances_the_invaders_rotation_through_four_takes() {
        let mut c = counties();
        c[1].owner = 1;
        let mut realms: [Realm; MAX_REALMS] = core::array::from_fn(|_| Realm::new());
        realms[2].lord = 4;
        let mut u = army(2, 300);
        u.dest_county = 1;
        let variants: Vec<u8> = (0..5)
            .map(|_| enter_county(&c, &mut realms, &u, 1).expect("a letter").variant)
            .collect();
        assert_eq!(variants, vec![12, 13, 14, 15, 12]);
        assert_eq!(realms[2].voice_rotation, 1);
    }

    #[test]
    fn a_neutral_destination_is_greeted_not_invaded() {
        let mut c = counties();
        c[2].population = 1_000;
        c[2].happiness = 50;
        let mut realms: [Realm; MAX_REALMS] = core::array::from_fn(|_| Realm::new());
        let mut u = army(1, 300);
        u.dest_county = 2;
        let l = enter_county(&c, &mut realms, &u, 2).expect("a greeting");
        assert_eq!(l.group, 0x85);
        assert_eq!(realms[1].voice_rotation, 0);
    }
}

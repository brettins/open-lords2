//! Step 4 is the one that could not have been guessed. The 36 is not a
//! parameter anywhere: it falls out of the strength weights, a ratio of 112 %
//! and the second rung of a ten-pair ladder read out of `Lords2.exe` at
//! `0x004DE710`. Four independent numbers have to be right for it to come out,
//! and the saved game says 36.

mod conquest_part;
pub use conquest_part::*;
mod battle;
pub use battle::*;
mod realm;
pub use realm::*;

use l2_formats::save::Save;
use l2_game::engagement::{self, Answer, Resolution};
use l2_kingdom::conquest::{self, Attack};
use l2_kingdom::kingdom::Kingdom;
use l2_kingdom::map::{flags, CampaignMap, MAP_DIM};
use l2_kingdom::unit::{TroopType, Unit, UnitKind, TROOP_TYPES};

/// `g_units`, `0x0052F0B0`, stride `0x1A4` — `docs/armies.md` §1.
const UNIT_BASE: u32 = 0x0052_F0B0;
const UNIT_STRIDE: u32 = 0x1A4;

const COUNTY: u8 = 3;
const ATTACKER_SLOT: u32 = 5;
const DEFENCE_SLOT: u32 = 6;

struct SavedUnit {
    owner: u8,
    kind: u8,
    home_county: u8,
    men: i32,
    troops: [i32; TROOP_TYPES],
    defence_mark: u8,
    move_allowance: u8,
    morale: u8,
}

fn unit_at(save: &Save, slot: u32) -> Option<SavedUnit> {
    let base = UNIT_BASE + slot * UNIT_STRIDE;
    let owner = save.u8_at(base).ok()?;
    if owner == 0 {
        return None;
    }
    let mut troops = [0i32; TROOP_TYPES];
    for (t, slot) in troops.iter_mut().enumerate() {
        let lo = save.u8_at(base + 0x16C + t as u32 * 2).ok()? as i32;
        let hi = save.u8_at(base + 0x16D + t as u32 * 2).ok()? as i32;
        *slot = lo | (hi << 8);
    }
    Some(SavedUnit {
        owner,
        kind: save.u8_at(base + 0x08).ok()?,
        home_county: save.u8_at(base + 0x11).ok()?,
        men: save.i32_at(base + 0x168).ok()?,
        troops,
        defence_mark: save.u8_at(base + 0x167).ok()?,
        move_allowance: save.u8_at(base + 0x154).ok()?,
        morale: save.u8_at(base + 0x166).ok()?,
    })
}

fn map_of_one_county(county: u8) -> CampaignMap {
    let mut m = CampaignMap::empty();
    for i in 0..MAP_DIM * MAP_DIM {
        m.county[i] = county;
        m.flags[i] = flags::ROAD;
    }
    m
}

fn before(save: &Save) -> (Kingdom, usize) {

    let c = save.county(COUNTY as usize).expect("county 3");
    assert!(c.is_county(), "county 3 is a real county");
    assert_eq!(c.owner, 0, "county 3 is neutral before the battle");

    let a = unit_at(save, ATTACKER_SLOT).expect("the player's army");
    assert_eq!(a.kind, 1, "slot 5 is an army");
    assert!(unit_at(save, DEFENCE_SLOT).is_none(), "nothing has been levied yet");

    let mut k = Kingdom::new(1);
    k.county_count = 5;
    k.campaign.map = map_of_one_county(COUNTY);
    k.counties[COUNTY as usize].owner = 0;
    k.counties[COUNTY as usize].population = c.population;
    k.counties[COUNTY as usize].happiness = c.happiness as i32;
    k.counties[COUNTY as usize].anchor_x = c.anchor_x;
    k.counties[COUNTY as usize].anchor_y = c.anchor_y;
    k.realms[1].in_play = true;
    k.realms[1].is_human = true;
    k.options.difficulty = 0;

    let mut u = Unit::new(UnitKind::Army, a.owner, c.anchor_x, c.anchor_y);
    u.owner_is_human = true;
    u.county = COUNTY;
    u.troops = a.troops;
    u.men = a.men;
    let id = k.campaign.units.spawn(u).expect("a slot");
    (k, id)
}


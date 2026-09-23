
mod paths;
pub use paths::*;
mod trample;
pub use trample::*;
mod burning;
pub use burning::*;

use super::*;
use crate::map::flags;
use crate::unit::Unit;

fn open_map() -> CampaignMap {
    let mut m = CampaignMap::empty();
    for i in 0..MAP_TILES {
        m.county[i] = 1;
    }
    m
}

fn blank() -> ([County; MAX_COUNTIES], [Realm; MAX_REALMS]) {
    (core::array::from_fn(|_| County::new()), core::array::from_fn(|_| Realm::new()))
}

/// **`County_DestroyField` (`0x00469E5B`) charges against `+0x206` and
/// steps it down**
#[test]
fn a_wrecked_grain_field_charges_and_steps_down_the_fields_still_standing() {
    let mut map = open_map();
    let mut c = County::new();
    c.fields_grain = 4;
    c.fields_grain_standing = 4;
    c.crop[1] = 400;
    map.set_flags(3, 3, flags::FARMLAND);
    map.set_terrain(3, 3, 7);
    assert_eq!(destroy_field(&mut c, &mut map, 3, 3), 100);
    assert_eq!((c.crop[1], c.fields_grain, c.fields_grain_standing), (300, 3, 3));
    assert_eq!(map.terrain_at(3, 3), 0);

    // More grain painted than was sown: `fieldsGrain > +0x206`, so the share
    // is 0 and `+0x206` stays — the original's `else 0`.
    let mut c = County::new();
    c.fields_grain = 6;
    c.fields_grain_standing = 4;
    c.crop[1] = 400;
    map.set_terrain(3, 3, 7);
    assert_eq!(destroy_field(&mut c, &mut map, 3, 3), 0);
    assert_eq!((c.crop[1], c.fields_grain, c.fields_grain_standing), (400, 5, 4));

    let mut c = County::new();
    c.fields_grain = 2;
    map.set_terrain(3, 3, 7);
    assert_eq!(destroy_field(&mut c, &mut map, 3, 3), 0);
    assert_eq!(c.fields_grain, 2, "the grain arm is guarded on +0x206");
    assert_eq!(map.terrain_at(3, 3), 0, "and the repaint is not guarded at all");
}

fn army_at(units: &mut Units, owner: u8, x: u8, y: u8) -> usize {
    let mut u = Unit::new(UnitKind::Army, owner, x, y);
    u.men = 200;
    u.county = 1;
    units.spawn(u).unwrap()
}



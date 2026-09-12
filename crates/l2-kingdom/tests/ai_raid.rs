//! **A raiding party can see the crop it was sent for, in every season.**
//!
//! ```text
//! cargo test -p l2-kingdom --test ai_raid
//! ```
//!
//! Needs no game install and no fixture.
//!
//! # Why this file exists
//!
//! `Ai_FindStandingCropTile` (`0x004A689D`) is what AI step 10's raiding party
//! (mission 7) is aimed at, and its tile test is a literal:
//!
//! ```c
//! if (g_tiles[t].county == county && (g_tiles[t].flags & 0x20) != 0 &&
//!     2 < g_tiles[t].content && g_tiles[t].content < 0x17) { … nearest wins … }
//! /* else: the county anchor */
//! ```
//!
//! **[V]**, read off the decompilation. **Terrain `2` is excluded**, and `2` is
//! what `Grain_SeasonTick`'s repaint leaves on a grain tile whose banded crop
//! is nothing. So the raid finder and the wheat picture read the same byte, and
//! a bug in the picture is a bug in the AI's war.
//!
//! That is exactly what C124 shipped. It banded `crop[2]` in all four seasons;
//! `crop[2]` is cleared at the top of every season and refilled only by the
//! harvest, so a sown field sat at terrain `2` from sowing until Winter and
//! **no raid could see a crop for three seasons in four**. The picture was the
//! reported symptom; this was the silent one.
//!
//! # What was asserting it before, and why that was not enough
//!
//! Nothing deliberately. `ai_war.rs`'s forty-turn runs were the only witness:
//! correcting the band changed where the raiders went, which changed who
//! conquered whom, which moved one long-run assertion. A rule that can only be
//! observed as a shifted trajectory is a rule nobody can ablate — `C184`'s
//! finding, in the same shape. So the claim is dealt here instead: sow a
//! county, tick it through a year, and ask the finder what it sees.
//!
//! # Ablations
//!
//! | ablated | this file |
//! |---|---|
//! | the band reads `crop[2]` in Spring, Summer and Autumn — C124's word | **red**, on all three |
//! | `Aim::StandingCrop`'s `terrain > 2` | **red** — the finder stops excluding bare ground |
//! | sowing does not write `+0x206` | **red** — the divisor is 0 and every band is 2 |

use l2_kingdom::ai_army::{aim_tile, Aim};
use l2_kingdom::county::County;
use l2_kingdom::map::{flags, CampaignMap};
use l2_kingdom::tables::{Season, Tables, Weather};
use l2_kingdom::{land, Quirks, MAX_COUNTIES};

const T: &Tables = &Tables::DEFAULT;
const Q: Quirks = Quirks::FAITHFUL;
const COUNTY: u8 = 3;

/// The county's six grain tiles, all in one row, and its anchor far away from
/// them so that the fallback is unmistakable.
const GRAIN: [(u8, u8); 6] = [(20, 20), (21, 20), (22, 20), (23, 20), (24, 20), (25, 20)];
const ANCHOR: (u8, u8) = (40, 40);
/// Where the raiding party stands: nearest to the first grain tile.
const RAIDER: (u8, u8) = (18, 20);

/// County 3, six fields laid to grain, 200 sacks in store and hands enough to
/// work them — the state `Grain_Sow` needs to sow a full crop.
fn a_county_about_to_be_sown() -> ([County; MAX_COUNTIES], CampaignMap) {
    let mut counties: [County; MAX_COUNTIES] = core::array::from_fn(|_| County::new());
    let c = &mut counties[COUNTY as usize];
    c.owner = 1;
    c.fields_grain = GRAIN.len() as i32;
    c.grain = 200;
    c.labour[T.job.grain_farming] = 10_000;
    c.weather = Weather::Cloudy;
    c.anchor_x = ANCHOR.0;
    c.anchor_y = ANCHOR.1;

    let mut map = CampaignMap::empty();
    for &(x, y) in &GRAIN {
        map.set_county(x, y, COUNTY);
        map.set_flags(x, y, flags::FARMLAND);
        map.set_terrain(x, y, 2); // bare, as a field is before it is sown
    }
    map.set_county(ANCHOR.0, ANCHOR.1, COUNTY);
    (counties, map)
}

/// One season of the county's grain pass, repaint included — the two calls
/// `Kingdom::grain_season_tick` makes in this order.
fn tick(counties: &mut [County; MAX_COUNTIES], map: &mut CampaignMap, season: Season) {
    land::grain_season_tick(T, &mut counties[COUNTY as usize], season, true, Q);
    land::grain_repaint_fields(COUNTY as usize, &counties[COUNTY as usize], season, map);
}

/// **The crop a county sows is a tile the raid finder accepts, in all four
/// seasons** — and under C124's word it was invisible in three of them.
///
/// The expected terrain bytes are `FUN_0044CF6F`'s literals, not a call into
/// our own bander: 60 sacks of seed at twelve sacks a sack is 720 over six
/// fields, 120 a field, which is past the 81 threshold, so band `11` — and
/// `11` is inside the finder's `3 … 22`.
#[test]
fn a_sown_field_is_a_tile_the_ai_raid_finder_can_see_all_year() {
    let (mut counties, mut map) = a_county_about_to_be_sown();

    // Before the sowing there is nothing to raid, and the finder says so by
    // falling back to the anchor. This is the control: it is the answer C124's
    // reading gave in Spring, Summer and Autumn as well.
    assert_eq!(
        aim_tile(&map, &counties, RAIDER, COUNTY, Aim::StandingCrop),
        ANCHOR,
        "bare fields are terrain 2, which `2 < content` excludes"
    );

    for season in [Season::Spring, Season::Summer, Season::Autumn, Season::Winter] {
        tick(&mut counties, &mut map, season);
        let c = &counties[COUNTY as usize];
        assert_eq!(c.fields_grain_standing, 6, "{season:?}: `+0x206`, the divisor");
        for &(x, y) in &GRAIN {
            assert_eq!(
                map.terrain_at(x, y),
                11,
                "{season:?}: 720 sacks over 6 fields is 120 a field, past `FUN_0044CF6F`'s 81"
            );
        }
        assert_eq!(
            aim_tile(&map, &counties, RAIDER, COUNTY, Aim::StandingCrop),
            GRAIN[0],
            "{season:?}: the raid is aimed at the nearest standing crop, not the anchor"
        );
    }
}

/// **And a raid that trampled the crop makes the rest of it look richer**,
/// because `County_DestroyField` (`0x00469E5B`) steps `+0x206` down and the
/// band divides by it. One pass of the same chain, end to end: sow, trample
/// one field, repaint.
///
/// Literals: `PctOf(1, 6)` is 16, and 16% of 720 truncates to 115, so 605
/// sacks stand on five fields — 121 a field, still the top band. The tile that
/// was trampled goes to terrain `0` and drops out of the finder's range, so the
/// raid's next aim is the tile after it.
#[test]
fn trampling_a_field_steps_the_divisor_down_and_the_tile_out_of_the_finders_range() {
    let (mut counties, mut map) = a_county_about_to_be_sown();
    tick(&mut counties, &mut map, Season::Spring);

    let lost = l2_kingdom::movement::destroy_field(
        &mut counties[COUNTY as usize],
        &mut map,
        GRAIN[0].0,
        GRAIN[0].1,
    );
    assert_eq!(lost, 115, "a sixth of 720, truncated");
    let c = &counties[COUNTY as usize];
    assert_eq!((c.crop[1], c.fields_grain, c.fields_grain_standing), (605, 5, 5));
    assert_eq!(map.terrain_at(GRAIN[0].0, GRAIN[0].1), 0, "the tile is bare ground now");

    assert_eq!(
        aim_tile(&map, &counties, RAIDER, COUNTY, Aim::StandingCrop),
        GRAIN[1],
        "terrain 0 is outside `2 < content`, so the next field is the nearest crop"
    );

    // And the picture on the five that are left is banded by the five.
    land::grain_repaint_fields(COUNTY as usize, &counties[COUNTY as usize], Season::Summer, &mut map);
    for &(x, y) in &GRAIN[1..] {
        assert_eq!(map.terrain_at(x, y), 11, "605 over 5 is 121 a field");
    }
}

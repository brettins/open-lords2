#![allow(unused_imports)]
use super::*;
use super::reset::*;
use super::lords::*;
use super::scenario::*;
use l2_formats::maps::{MapSlot, Plane, PLANE_DIM};
use l2_kingdom::county::{MAX_COUNTIES, MAX_COUNTY_ID, MAX_FIELDS, MAX_NEIGHBOURS};
use l2_kingdom::map::{CampaignMap, MAP_TILES};
use l2_kingdom::merchant::{self, MerchantRoutes, ROUTES, ROUTE_SLOTS};
use l2_kingdom::mercenary::MercenaryBands;
use l2_kingdom::realm::MAX_REALMS;
use l2_kingdom::tables::{Weather, JOB_COUNT};
use l2_kingdom::unit::{Unit, UnitKind, Units};
use l2_kingdom::Options;
use crate::{Clock, CountyState, IndustryState, RealmState, Scenario};

/// The whole of `Map_InitScenario`, in its order.
pub fn build(slot: &MapSlot<'_>, setup: &NewGame) -> Result<MapWorld, MapError> {
    let mut w = load_planes(slot)?;
    adjacency(&mut w);
    place_sites(&mut w);
    place_starting_fields(&mut w, setup.options.difficulty);
    collect_field_tiles(&mut w);
    pick_merchant_starts(&mut w);
    Ok(w)
}

/// `Map_LoadPlanes` (`0x00467770`) whole: the copy, the county count, and the
/// plane-4 dispatch.
///
/// **The dispatch is two tables and the flag bit picks which.** `[V]` and it
/// was written the other way round until `docs/decisions.md` C25:
///
/// ```c
/// if (marker != 0) {
///     if ((flags & 0x40) == 0) { if (flags & 0x80) PlayerStart_Record(county, marker); }
///     else                     Merchant_RouteAppend(county, marker);
/// }
/// ```
///
/// so a marker on the **county town** extends a trade route and a marker on the
/// **castle** is a player start — which is what both of those are.
fn load_planes(slot: &MapSlot<'_>) -> Result<MapWorld, MapError> {
    let tiles = Tiles::from_slot(slot);

    let mut county_count = 0usize;
    let mut player_start = [0u8; 6];
    let mut player_start_count = 0usize;
    let mut routes = MerchantRoutes::none();
    let mut rows = [[0u8; ROUTE_SLOTS]; ROUTES];

    for y in 0..PLANE_DIM {
        for x in 0..PLANE_DIM {
            let i = y * PLANE_DIM + x;
            let county = tiles.county[i];
            // `if ((county < 0x11) && (g_countyCount < county))` — 32 is in the
// plane and is not a county
            if county < 0x11 && county_count < county as usize {
                county_count = county as usize;
            }
            let marker = slot.at(Plane::Marker, x, y);
            if marker == 0 {
                continue;
            }
            if tiles.flags[i] & bit::TOWN != 0 {
                // `Merchant_RouteAppend(county, marker)`: the first free cell
                // of row `marker - 1`. A marker of 0 cannot reach here.
                let row = marker as usize - 1;
                if let Some(cells) = rows.get_mut(row) {
                    if let Some(cell) = cells.iter_mut().find(|c| **c == 0) {
                        *cell = county;
                    }
                }
            } else if tiles.flags[i] & bit::SITE != 0 {
                // `PlayerStart_Record(county, marker)`. The counter counts
                // *writes*, not distinct markers — see the note on
                // `player_start_count` below.
                if let Some(cell) = player_start.get_mut(marker as usize) {
                    *cell = county;
                }
                player_start_count += 1;
            }
        }
    }

    if county_count == 0 {
        return Err(MapError::NoCounties);
    }
    if county_count > MAX_COUNTY_ID as usize {
        return Err(MapError::CountyCount(county_count));
    }
    for (row, cells) in rows.iter().enumerate() {
        routes.set_row(row, *cells);
    }
    // **The counter and the table disagree on a map that repeats a marker.**
    // `PlayerStart_Record` increments on every write, so two castle tiles
    // carrying the same marker count twice and store once.
    // `l2_formats::maps::MapSlot::player_start_count` counts distinct markers,
    // which is what the table can hold; the seat count the *screen* shows comes
    // from there. This one reproduces the original's own arithmetic so that a
// map where they differ is visible — no shipped
    // map does, which `tests/newgame.rs` asserts over all 44.
    Ok(MapWorld {
        tiles,
        county_count,
        player_start,
        player_start_count,
        routes,
        merchant_start: [0; ROUTES],
        neighbours: vec![Vec::new(); MAX_COUNTIES],
        town_tile: vec![0; MAX_COUNTIES],
        anchor: vec![(0, 0); MAX_COUNTIES],
        castle_tile: vec![0; MAX_COUNTIES],
        castle: vec![(0, 0); MAX_COUNTIES],
        dwelling_plots: vec![[0; 4]; MAX_COUNTIES],
        industry_site: vec![[0; 4]; MAX_COUNTIES],
        has_resource: vec![[false; 4]; MAX_COUNTIES],
        field_tiles: vec![[0; MAX_FIELDS]; MAX_COUNTIES],
        farm_tile_count: vec![0; MAX_COUNTIES],
    })
}

/// The four 4-adjacent neighbours of a tile, in `FUN_0046C080`'s order —
/// north, east, south, west — with the edge reading as county 0.
fn four_neighbours(x: usize, y: usize) -> [Option<usize>; 4] {
    [
        if y > 0 { Some((y - 1) * PLANE_DIM + x) } else { None },
        if x + 1 < PLANE_DIM { Some(y * PLANE_DIM + x + 1) } else { None },
        if y + 1 < PLANE_DIM { Some((y + 1) * PLANE_DIM + x) } else { None },
        if x > 0 { Some(y * PLANE_DIM + x - 1) } else { None },
    ]
}

/// `FUN_00467CA6` — county `+0x5C`, the zero-terminated adjacency list.
///
/// **Ascending by discovery, not sorted.** The original appends in the scan's
/// own order (`y` outer, `x` inner, then N/E/S/W), stops at sixteen entries,
/// and counts them into `+0x5B`. Reproducing the order matters: the list is
/// walked by index in half a dozen places and two peers must walk it the same
/// way (`docs/netcode.md` D-4).
fn adjacency(w: &mut MapWorld) {
    let n = w.county_count;
    for y in 0..PLANE_DIM {
        for x in 0..PLANE_DIM {
            let i = y * PLANE_DIM + x;
            let c = w.tiles.county[i] as usize;
            if c == 0 || c >= 0x12 {
                continue;
            }
            for slot in four_neighbours(x, y) {
                let other = slot.map(|t| w.tiles.county[t]).unwrap_or(0);
                if other == 0 || other as usize >= 0x12 || other as usize == c {
                    continue;
                }
                let list = &mut w.neighbours[c];
                if list.len() >= MAX_NEIGHBOURS || list.contains(&other) {
                    continue;
                }
                list.push(other);
            }
        }
    }
    // A county id above the count cannot be a county; the original's clamp is
    // `< 0x12` and the shipped maps never exercise the difference.
    for c in 0..MAX_COUNTIES {
        if c == 0 || c > n {
            w.neighbours[c].clear();
        } else {
            w.neighbours[c].retain(|&o| o as usize <= n);
        }
    }
}

/// `Counties_PlaceSites` (`0x00468D4F`) — five passes per county, and **the
/// order is load-bearing**.
///
/// The town goes first because the anchor it leaves is what the blacksmith
/// measures distance from; the resource sites go before the castle because they
/// stamp a terrain onto the Town-bank `0x80` tiles, and the castle search is
/// *"a `0x80` tile with no terrain yet"* — which is exactly the tiles the
/// resource sites did not take. Reorder them and the county's mine becomes its
/// castle.
fn place_sites(w: &mut MapWorld) {
    for county in 1..=w.county_count {
        let Some((town, anchor)) = find_town_tile(w, county) else { continue };
        w.town_tile[county] = town;
        w.anchor[county] = anchor;
        w.dwelling_plots[county] = find_dwelling_plots(w, county);
        place_resource_sites(w, county);
        place_blacksmith(w, county);
        if let Some((tile, xy)) = find_castle_tile(w, county) {
            w.castle_tile[county] = tile;
            w.castle[county] = xy;
        }
    }
}

/// `County_FindTownTile` (`0x00467FD1`).
///
/// Returns the block's **north-west** tile and the county anchor, which is its
/// **south-east** one — the original stores the fourth match's `x`/`y` and
/// returns the first match's offset, and those are not the same tile.
fn find_town_tile(w: &mut MapWorld, county: usize) -> Option<(usize, (u8, u8))> {
    let mut found = 0;
    let mut first = 0usize;
    for y in 0..PLANE_DIM {
        for x in 0..PLANE_DIM {
            let i = y * PLANE_DIM + x;
            if w.tiles.county[i] as usize != county
                || w.tiles.flags[i] & bit::TOWN == 0
                || w.tiles.content[i] != 0
            {
                continue;
            }
            if found == 0 {
                first = i;
                w.tiles.bank[i] |= BANK_OVERLAY;
                w.tiles.stamp_2x2(i, FRAME_VILLAGE_SMALL, BANK_TOWN, 0);
            }
            if found == 2 {
                w.tiles.bank[i] |= BANK_OVERLAY;
            }
            if found == 3 {
                return Some((first, (x as u8, y as u8)));
            }
            found += 1;
        }
    }
    None
}

/// `County_FindDwellingPlots` (`0x00468C41`) — the four `0x10` tiles, their
/// terrain frames saved so razing one can put the ground back.
///
/// **Four is the storage, not a rule.** The original's counter has no bound and
/// the four `i32` slots at county `+0x80` end exactly on `fieldProgress`,
/// fifth plot in one county would corrupt a field's reclamation. All 434
/// counties of the 44 shipped maps have exactly four
/// (`docs/formats/maps-layers.md` §2.3); this one drops the fifth
/// reproducing the overrun, because the overrun is a memory bug and not a rule.
fn find_dwelling_plots(w: &mut MapWorld, county: usize) -> [usize; 4] {
    let mut plots = [0usize; 4];
    let mut n = 0;
    for i in 0..MAP_TILES {
        if w.tiles.flags[i] & bit::PLOT == 0 || w.tiles.county[i] as usize != county {
            continue;
        }
        if n < plots.len() {
            plots[n] = i;
        }
        w.tiles.saved_frame[i] = w.tiles.frame[i];
        w.tiles.content[i] = 0;
        n += 1;
    }
    plots
}

/// `County_PlaceResourceSites` (`0x00468E61`) — **where a county's resources
/// come from is the map, not the county record.**
///
/// Three of the four industries: a Town-bank tile of this county drawing frame
/// 30 is the iron mine, frame 0 is the quarry and frame 20 is the forest. The
/// fourth, weapons, is [`place_blacksmith`].
///
/// **The stone test carries an `iron == 0` guard and it is not symmetric.** The
/// scan is row-major and iron is tested first at every tile,
/// quarry comes *before* its mine gets both and one whose mine comes first gets
/// only the mine. That is the original's arithmetic and it is why iron and
/// stone are complementary in thirteen of England's fourteen counties rather
/// than in all of them.
fn place_resource_sites(w: &mut MapWorld, county: usize) {
    for i in 0..MAP_TILES {
        if w.tiles.county[i] as usize != county || w.tiles.bank_layer(i) != BANK_TOWN {
            continue;
        }
        let frame = w.tiles.frame[i];
        if frame == FRAME_MINE {
            set_site(w, county, i, IND_IRON, TERRAIN_IRON);
        }
        if frame == FRAME_QUARRY && !w.has_resource[county][IND_IRON] {
            set_site(w, county, i, IND_STONE, TERRAIN_STONE);
        }
        if frame == FRAME_FOREST {
            set_site(w, county, i, IND_WOOD, TERRAIN_WOOD);
        }
    }
}

fn set_site(w: &mut MapWorld, county: usize, tile: usize, record: usize, terrain: u8) {
    w.has_resource[county][record] = true;
    w.industry_site[county][record] = tile;
    w.tiles.content[tile] = terrain;
    w.tiles.bank[tile] |= BANK_OVERLAY;
}

/// `FUN_0046C147` — the flag byte of one 4-neighbour, classified.
///
/// The boundary bit is masked off first,
/// border is still a field; then two corrections fold the mountain and the
/// woodland into one bit and the reserved plot into nothing when the bank
/// disagrees. Only bit `0x20` is read by the one caller here, and the mask is
/// what makes `0x22` count.
fn neighbour_class(w: &MapWorld, tile: Option<usize>) -> u8 {
    let Some(t) = tile else { return 0 };
    let raw = w.tiles.flags[t];
    if raw == 0 {
        return 0;
    }
    let layer = w.tiles.bank_layer(t);
    let mut r = raw & !bit::BOUNDARY;
    if r == 0x08 && layer != BANK_MTNS {
        r = 0x10;
    }
    if r == 0x10 && layer != BANK_ROADS {
        r = 0;
    }
    r
}

/// `Dist_Chebyshev` (`0x00404F4C`).
fn chebyshev(a: (u8, u8), b: (u8, u8)) -> i32 {
    let dx = (a.0 as i32 - b.0 as i32).abs();
    let dy = (a.1 as i32 - b.1 as i32).abs();
    dx.max(dy)
}

/// `County_PlaceBlacksmith` (`0x0046902A`) — **the weapons site is derived, not
/// authored, so every county has one.**
///
/// The pick is the county's *plain* tile — flags exactly zero, so not a road,
/// not a field, not a boundary — that is 4-adjacent to one of the county's own
/// farm fields and nearest the town anchor. Ties go to the first in row-major
/// order, because the comparison is strict.
///
/// It **adds** [`bit::SITE`] to a tile the file had no flag on at all, so this
/// is one of the two places where the runtime flags plane is not the file's.
/// The other is a razed field. A reader diffing our flags against
/// `L2_maps.dat`'s should expect exactly `county_count` extra `0x80` tiles.
fn place_blacksmith(w: &mut MapWorld, county: usize) {
    let anchor = w.anchor[county];
    let mut best_tile = 0usize;
    let mut best = 0x40i32;
    for y in 0..PLANE_DIM {
        for x in 0..PLANE_DIM {
            let i = y * PLANE_DIM + x;
            if w.tiles.county[i] as usize != county || w.tiles.flags[i] != 0 {
                continue;
            }
            let n = four_neighbours(x, y);
            for slot in n {
                let same_county = slot.map(|t| w.tiles.county[t] as usize) == Some(county);
                if !same_county {
                    continue;
                }
                if neighbour_class(w, slot) & bit::FARM == 0 {
                    continue;
                }
                let d = chebyshev(anchor, (x as u8, y as u8));
                if d < best {
                    best = d;
                    best_tile = i;
                }
            }
        }
    }
    // `if ((local_14 != 0) && (flags[local_14] == 0))` — tile 0 is the "none"
    // encoding, so the north-west corner of the map can never be a blacksmith.
    if best_tile == 0 || w.tiles.flags[best_tile] != 0 {
        return;
    }
    w.has_resource[county][IND_WEAPONS] = true;
    w.industry_site[county][IND_WEAPONS] = best_tile;
    w.tiles.flags[best_tile] |= bit::SITE;
    w.tiles.content[best_tile] = TERRAIN_WEAPONS;
    w.tiles.bank[best_tile] = (w.tiles.bank[best_tile] & 0xE3) | BANK_TOWN | BANK_OVERLAY;
    w.tiles.frame[best_tile] = 10;
}

/// `County_FindCastleTile` (`0x00468121`) — the bare plot, and **only** the
/// bare plot.
///
/// Finds the county's 2×2 of `0x80` tiles that no resource site took, stamps
/// terrain `0x14` on all four and saves their terrain frames. Raising a castle
/// on the plot is `FUN_0046826C`, which is keyed on the castle's level and its
/// build percentage, and is not done here.
///
/// Returns the block's **north-west** tile, which is both the returned offset
/// and the stored `x`/`y` — the opposite of [`find_town_tile`], where they are
/// different tiles.
fn find_castle_tile(w: &mut MapWorld, county: usize) -> Option<(usize, (u8, u8))> {
    let mut found = 0;
    let mut first = None;
    for y in 0..PLANE_DIM {
        for x in 0..PLANE_DIM {
            let i = y * PLANE_DIM + x;
            if w.tiles.county[i] as usize != county
                || w.tiles.flags[i] & bit::SITE == 0
                || w.tiles.content[i] != 0
            {
                continue;
            }
            if found == 0 {
                first = Some((i, (x as u8, y as u8)));
                w.tiles.bank[i] |= BANK_OVERLAY;
            }
            w.tiles.saved_frame[i] = w.tiles.frame[i];
            w.tiles.content[i] = TERRAIN_CASTLE_PLOT;
            if found == 3 {
                return first;
            }
            found += 1;
        }
    }
    // Fewer than four: the original returns 0 and the county has no castle
    // tile, but the tiles it did find keep the stamp. Reproduced.
    None
}

/// `Map_PlaceStartingFields` (`0x00467A36`) — every farm tile's opening crop.
///
/// The ladder is *per county*, on the tile's ordinal within it, and the whole
/// of the difficulty setting's effect on the land is here:
///
/// | difficulty | pasture | fallow | wild |
/// |---|---|---|---|
/// | 0 easiest | first 8 | the rest | — |
/// | 1 | first 6 | 2 more | the rest |
/// | 2 | first 4 | 2 more | the rest |
/// | 3 hardest | first 4 | — | the rest |
///
/// The frame is `base + ((storedFrame + 0xB0) & 3)`, which keeps the tile's own
/// variant: every crop state is four consecutive frames and `& 3` picks the
/// same one of the four. `FUN_0046D7F4` then recomputes the identical number
/// from the other side (`docs/formats/maps-layers.md` §5.5), which is a
/// redundancy in the original and not a second rule.
fn place_starting_fields(w: &mut MapWorld, difficulty: u8) {
    let mut ordinal = [0i32; 0x20];
    for c in w.farm_tile_count.iter_mut() {
        *c = 0;
    }
    for y in 0..PLANE_DIM {
        for x in 0..PLANE_DIM {
            let i = y * PLANE_DIM + x;
            let county = w.tiles.county[i] as usize;
            if county == 0 || county >= 0x12 || w.tiles.flags[i] & bit::FARM == 0 {
                continue;
            }
            if let Some(c) = w.farm_tile_count.get_mut(county) {
                *c = c.wrapping_add(1);
            }
            let nth = ordinal[county];
            ordinal[county] += 1;
            let (content, base) = match difficulty {
                3 if nth < 4 => (0x14u8, 104u8),
                3 => (0, 80),
                2 if nth < 4 => (0x14, 104),
                2 if nth < 6 => (1, 84),
                2 => (0, 80),
                1 if nth < 6 => (0x14, 104),
                1 if nth < 8 => (1, 84),
                1 => (0, 80),
                _ if nth < 8 => (0x14, 104),
                _ => (1, 84),
            };
            let variant = w.tiles.frame[i].wrapping_add(0xB0) & 3;
            w.tiles.frame[i] = base + variant;
            w.tiles.content[i] = content;
            // `FUN_0046D7F4`'s bank half, which the frame half above already
            // agrees with: the roads layer, bit 0x01 set, bit 0x80 cleared and
            // then set again for a terrain in 0x0F..0x17 — which pasture is
            // and neither fallow nor wild is.
            let mut bank = ((w.tiles.bank[i] | 1) & 0xE3) | BANK_ROADS;
            bank &= 0x7F;
            if content > 0x0E && content < 0x17 {
                bank |= BANK_OVERLAY;
            }
            w.tiles.bank[i] = bank;
        }
    }
}

/// `County_CollectFieldTiles` (`0x0046DA4B`) — `g_countyFieldTiles`, and **the
/// twenty-slot table is a hard limit that razes what will not fit.**
///
/// This is the function that makes twenty fields per county a *fact about the
/// map*. A county with twenty-one farm tiles
/// loses the twenty-first outright: terrain 0, frame 6, **flags zeroed** and
/// the bank put back to base — it stops being farmland at all and becomes
/// plain grass, before the first season runs.
///
/// `[D]` on the consequence, `[V]` on the code: the razing branch is the `else`
/// of *"is there a free slot"*, and the four writes are literal.
fn collect_field_tiles(w: &mut MapWorld) {
    for row in w.field_tiles.iter_mut() {
        *row = [0; MAX_FIELDS];
    }
    for i in 0..MAP_TILES {
        let county = w.tiles.county[i] as usize;
        if county == 0 || county >= 0x12 || w.tiles.flags[i] & bit::FARM == 0 {
            continue;
        }
        let placed = w
            .field_tiles
            .get_mut(county)
            .and_then(|row| row.iter_mut().find(|slot| **slot == 0))
            .map(|slot| *slot = i as u16)
            .is_some();
        if !placed {
            w.tiles.content[i] = 0;
            w.tiles.frame[i] = FRAME_GRASS;
            w.tiles.flags[i] = 0;
            w.tiles.bank[i] &= 0xE3;
        }
    }
}


#![allow(unused_imports)]
use super::*;
use super::build_part::*;
use super::sites::*;
use super::setup::*;
use super::scenario::*;
use super::tests_part::*;
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

/// `Merchant_PickStartCounties` (`0x004291B3`) — one start county per trade
/// route, each different from the ones already handed out.
///
/// **The dedup walk is odd and it is reproduced exactly**, because a merchant
/// that starts in the wrong county walks the wrong route for the rest of the
/// game. Candidate 0 is the route's first town; after that it tries cells
/// 2, 4, 6 … and, on running off the end of the row, cells 3, 5, 7 …; **cell 1
/// is never tried**; and after five retries it stores whatever it has, taken or
/// not. `Merchant_StartCountyTaken` returns 1 for county 0 as well, because
/// unset entries are 0 —
/// `Merchant_SpawnAll` stops dead at the first zero.
fn pick_merchant_starts(w: &mut MapWorld) {
    let taken = |starts: &[u8; ROUTES], c: u8| starts.iter().any(|&s| s == c);
    let mut starts = [0u8; ROUTES];
    for row in 0..ROUTES {
        let cells = *w.routes.row(row);
        let mut cursor = 0usize;
        let mut candidate = cells[0];
        let mut tries = 0;
        while taken(&starts, candidate) {
            tries += 1;
            if tries >= 6 {
                break;
            }
            candidate = cells.get(cursor + 2).copied().unwrap_or(0);
            cursor += 2;
            if candidate == 0 {
                cursor = 1;
            }
        }
        starts[row] = candidate;
    }
    w.merchant_start = starts;
}

// ---------------------------------------------------------------- the seating

/// `FUN_00497E65` (`0x00497E65`) — **the start table is shuffled, and that is
/// why which realm you play is different every game.**
///
/// `docs/environment.md` records the fact from the other end: the England
/// turn-one fixture's fingerprint deliberately excludes the realm→county
/// assignment, because two independently created saves of the same map
/// disagree about it. This is the code that disagrees. `Game_NewGame` calls it
/// between `Mercenary_Init` and `PlayerStart_Compact`, and it re-deals the
/// live entries of `g_playerStartTable` into each other's slots:
///
/// ```c
/// for (i = 1; i <= live; i++) {
///     pos = (rand & 3) + 1 + i;  if (pos > live) pos = 1;
///     for (tries = 0; tries < live; tries++) {
///         if (table[pos] == 0) { table[pos] = src[i]; break; }
///         if (++pos > live) pos = 1;
///     }
/// }
/// ```
///
/// **`[D]`, and the function had no name until now.** Its second half deals a
/// 48-entry table into `DAT_0057CAE0` and fills `DAT_00553080` with a value per
/// entry (`0`, `100`, or `6 + 5n`); neither destination has been traced and
/// neither is a player start, so neither is reproduced.
///
/// **The generator is ours and the shape is the original's**, for the reason
/// `l2_game::scenario::SEED` gives: the original draws from two 31-bit LFSRs
/// whose state no save records, and reproducing its *stream* is impossible from
/// a file. What is reproduced is the deal — a random offset, then a linear
/// probe forward for a free slot, wrapping at the live count.
fn shuffle_starts(w: &MapWorld, seed: u64) -> Vec<u8> {
    // `local_ec`: how many of entries 1..5 the map filled.
    let src: Vec<u8> = (1..w.player_start.len()).map(|m| w.player_start[m]).collect();
    let live = src.iter().filter(|&&c| c != 0).count();
    if live == 0 {
        return Vec::new();
    }
    let mut rng = l2_kingdom::Pcg32::from_seed(seed);
    let mut table = vec![0u8; live + 1];
    for i in 1..=live {
        let mut pos = (rng.next_u32() & 3) as usize + 1 + i;
        if pos > live {
            pos = 1;
        }
        for _ in 0..live {
            if table[pos] == 0 {
                table[pos] = src[i - 1];
                break;
            }
            pos += 1;
            if pos > live {
                pos = 1;
            }
        }
    }
    // The probe can leave an entry unplaced only if every slot was full, which
// needs more sources than slots; there are
    debug_assert!(table[1..].iter().all(|&c| c != 0), "the deal placed every start");
    table[1..].to_vec()
}

/// `PlayerStart_Compact` (`0x0049BC5F`) — the start counties the live realms
/// get, realm 1 first.
///
/// The original bubbles entries whose *slot number* is above the live realm
/// count out of a six-entry table, repeatedly, until none is left. Every
/// populated entry's slot number is its own index (`PlayerStart_Record` writes
/// both),
/// result is the first `lords` entries of the table as
/// [`shuffle_starts`] left it. Written that way, with
/// the equivalence stated here and checked over all 44 shipped maps in
/// `tests/newgame.rs`.
fn start_counties(w: &MapWorld, lords: usize, seed: u64) -> Result<Vec<u8>, MapError> {
    let seats = shuffle_starts(w, seed);
    if seats.is_empty() {
        return Err(MapError::NoPlayerStarts);
    }
    if lords > seats.len() {
        return Err(MapError::TooManyLords { lords, seats: seats.len() });
    }
    for (i, &c) in seats.iter().take(lords).enumerate() {
        if c as usize > w.county_count {
            return Err(MapError::StartCounty { marker: i + 1, county: c });
        }
    }
    Ok(seats)
}


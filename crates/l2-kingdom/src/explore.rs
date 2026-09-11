//! **Exploration** — the fog of war's seen bits, and every rule that sets them.
//!
//! `g_optExploration` (`0x0053F264`) is the *Exploration* row of the advanced
//! options, and `L2.eng` group 218 index 3 is the game's own specification of
//! it: *"When Exploration is turned on, the world outside your county is
//! blacked out. It is gradually revealed as your armies move through and
//! conquer new counties."*
//!
//! # Where the original keeps it: tile record `+2`, bit `0x20`
//!
//! **[V]**, from the decompilation, every reader and every writer:
//!
//! | writer | address | what it sets |
//! |---|---|---|
//! | `FUN_0046DF51` | `0x0046DF51` | **clears** bit `0x20` on all 4,096 tiles — from `Map_InitScenario` (`0x004676E0`), straight after `Map_LoadPlanes` |
//! | `FUN_0046E067(x, y, r)` | `0x0046E067` | the `(2r+1)²` square round `(x, y)`, clipped to the map |
//! | `FUN_0046DFD5(county)` | `0x0046DFD5` | `FUN_0046E067(x, y, 1)` on every tile of the county — the county **and a one-tile border** |
//! | `Unit_Step` | `0x00465D28` | radius **6** at the unit's tile, every time the loop finds it on a tile centre — `kind == 1` (an army) and `owner == g_localPlayer` only |
//! | `Army_Create` | `0x004A9A9A` | radius **6** at the new army's tile, `realm == g_localPlayer` only |
//! | `County_ChangeOwner` | `0x004A72FE` | `FUN_0046DFD5(county)` when `newOwner == g_localPlayer`, its first statement |
//! | `Game_SetupRealmsAndCounties` | `0x0049BD99` | `FUN_0046DFD5(g_playerStartTable[g_localPlayer * 2])`, its last statement |
//! | `FUN_00469370` | `0x00469370` | re-sets the bit on a county's twenty field tiles after a network snapshot overwrote their bank bytes, when the county is the local player's |
//!
//! Every other write to the byte masks with `0xE3`, `0x7F`, `0xFE` or `0xBF`
//! or ORs in another bit, so **nothing but `FUN_0046DF51` ever clears a seen
//! tile** — a county lost stays explored. The only wholesale assignment is
//! `Map_LoadPlanes`' load from the map file, which `FUN_0046DF51` follows.
//!
//! **None of the writers tests `g_optExploration`.** The bits are kept whether
//! the option is on or off, and the England turn-one fixture is the proof from
//! the data rather than the code: it was saved with the option off, and its
//! seen bits are exactly the local player's county and its one-tile border, bit
//! for bit (`crates/l2-scenario/tests/explored.rs`). So turning the option on in the
//! middle of a game blacks out what the armies have *not* seen, not the whole
//! world.
//!
//! **And none of the readers is a rule.** The option and the bit are read
//! together in seven painters and nowhere else — `Map_RenderIso`,
//! `Map_RenderAlignedRow`, `Map_RenderOffsetRow`, `Map_DrawTile`,
//! `Map_DrawTileApex`, `Sprite_TopIt`, `Map_DrawArmies`, plus the dead
//! `FUN_00406BBA` — and the option alone by `Screen_AdvancedOptions`' Yes/No.
//! No input arm, no AI step and no simulation pass reads either. **The AI lords
//! see everything.**
//!
//! # One seen bit per realm, where the original has one per machine
//!
//! Every writer above is guarded on `g_localPlayer` — a value that differs
//! between the machines of a network game — so the original's seen plane is a
//! different array on every peer. That cannot be lockstep state
//! (`docs/netcode.md` §3), and it cannot be left out of the save either, because
//! nothing can rebuild it: it is the history of where the armies walked.
//!
//! So this keeps **one bit per realm per tile**, and every writer sets the bit
//! of the realm its original sets the bit for *when that realm is the local
//! player*. The plane a viewer draws is exactly the original's for that viewer;
//! the other realms' bits are the planes the other machines would hold, and
//! nothing reads them. Every peer computes all of them identically. That is the
//! one divergence in representation, and it changes no picture.
//!
//! A unit owned by no realm — the ownerless militia `Army_Create` writes as
//! owner 6, the merchants — can never be `g_localPlayer`, and reveals nothing.

use crate::map::{index, CampaignMap, MAP_DIM, MAP_TILES};
use crate::realm::MAX_REALMS;
use l2_net::canonical::{Canonical, Encode};

/// `Unit_Step` and `Army_Create` both pass **6** to `FUN_0046E067`: an army
/// sees a 13 × 13 square.
pub const ARMY_SIGHT: i32 = 6;

/// `FUN_0046DFD5` passes **1** for every tile of the county: a county is seen
/// with a one-tile border round it.
pub const COUNTY_BORDER: i32 = 1;

/// The seen bits: one byte per tile, bit `r` for realm `r`.
///
/// `MAX_REALMS` is 6 and realm 0 is never a realm, so bits 1 … 5 are used and
/// bits 0, 6 and 7 are always clear.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Explored {
    seen: Vec<u8>,
}

impl Default for Explored {
    fn default() -> Self {
        Explored::new()
    }
}

/// The bit a realm's seen flag occupies, or `None` for anything that is not a
/// realm — 0, and the owner 6 that ownerless units and merchants carry.
fn realm_bit(realm: u8) -> Option<u8> {
    if realm == 0 || realm as usize >= MAX_REALMS {
        None
    } else {
        Some(1 << realm)
    }
}

impl Explored {
    /// **`FUN_0046DF51` (`0x0046DF51`)** — nothing seen by anyone. What
    /// `Map_InitScenario` leaves after it loads a map.
    pub fn new() -> Explored {
        Explored { seen: vec![0; MAP_TILES] }
    }

    /// `FUN_0046DF51` again, on a plane that already has bits in it.
    pub fn clear(&mut self) {
        self.seen.iter_mut().for_each(|b| *b = 0);
    }

    /// Has `realm` seen this tile? `false` for anything that is not a realm and
    /// for a tile off the map.
    pub fn is_seen(&self, realm: u8, tile: usize) -> bool {
        match (realm_bit(realm), self.seen.get(tile)) {
            (Some(bit), Some(b)) => b & bit != 0,
            _ => false,
        }
    }

    /// Mark one tile seen by `realm`. What `l2-scenario` uses to carry a saved
    /// game's bank bit `0x20` across, which is the local player's alone.
    pub fn set_seen(&mut self, realm: u8, tile: usize) {
        if let (Some(bit), Some(b)) = (realm_bit(realm), self.seen.get_mut(tile)) {
            *b |= bit;
        }
    }

    /// How many tiles `realm` has seen.
    pub fn count(&self, realm: u8) -> usize {
        (0..MAP_TILES).filter(|&t| self.is_seen(realm, t)).count()
    }

    /// **`FUN_0046E067(x, y, r)` (`0x0046E067`)** — the `(2r+1)²` square
    /// centred on `(x, y)`, clipped to the 64 × 64 map.
    ///
    /// ```c
    /// x -= r;  y -= r;  w = h = 2*r + 1;
    /// if (x < 0) { w += x; x = 0; } else if (0x40 < w + x) w = 0x40 - x;
    /// if (y < 0) { h += y; y = 0; } else if (0x40 < h + y) h = 0x40 - y;
    /// for (row = y; row < y + h; row++) for (col = x; col < x + w; col++)
    ///     g_tiles[row * 64 + col].bank |= 0x20;
    /// ```
    ///
    /// The clip is a plain intersection for every radius the game passes (1
    /// and 6), which is what the ranges below compute.
    pub fn reveal_square(&mut self, realm: u8, x: i32, y: i32, radius: i32) {
        let Some(bit) = realm_bit(realm) else { return };
        let dim = MAP_DIM as i32;
        let (x0, x1) = ((x - radius).max(0), (x + radius).min(dim - 1));
        let (y0, y1) = ((y - radius).max(0), (y + radius).min(dim - 1));
        for row in y0..=y1 {
            for col in x0..=x1 {
                self.seen[index(col as u8, row as u8)] |= bit;
            }
        }
    }

    /// **`FUN_0046DFD5(county)` (`0x0046DFD5`)** — every tile whose county
    /// byte is `county`, each with its one-tile border. The original walks the
    /// map in index order and ORs, so the order cannot matter.
    pub fn reveal_county(&mut self, realm: u8, map: &CampaignMap, county: u8) {
        if realm_bit(realm).is_none() {
            return;
        }
        for tile in 0..MAP_TILES {
            if map.county[tile] == county {
                let (x, y) = crate::map::coords(tile);
                self.reveal_square(realm, x as i32, y as i32, COUNTY_BORDER);
            }
        }
    }

    /// The plane as bytes, tile order — what the save writes.
    pub fn as_bytes(&self) -> &[u8] {
        &self.seen
    }

    /// Replace the plane from [`Explored::as_bytes`]' form. `false`, and
    /// nothing changed, when the length is not one byte a tile.
    pub fn copy_from_bytes(&mut self, bytes: &[u8]) -> bool {
        if bytes.len() != MAP_TILES {
            return false;
        }
        self.seen.copy_from_slice(bytes);
        true
    }
}

impl Encode for Explored {
    fn encode(&self, out: &mut Canonical) {
        out.raw(&self.seen);
    }
}

/// **The test every painter makes**, and the only one:
///
/// ```c
/// if (g_optExploration == 1 && (g_tiles[t].bank & 0x20) == 0)  /* hidden */
/// ```
///
/// `exploration` is `Options::exploration` and `viewer` is the realm whose
/// screen this is — `g_localPlayer`.
pub fn hides(exploration: bool, explored: &Explored, viewer: u8, tile: usize) -> bool {
    exploration && !explored.is_seen(viewer, tile)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_armys_square_is_thirteen_on_a_side_and_clipped_at_the_edge() {
        let mut e = Explored::new();
        e.reveal_square(1, 30, 30, ARMY_SIGHT);
        assert_eq!(e.count(1), 169);
        assert!(e.is_seen(1, index(24, 24)) && e.is_seen(1, index(36, 36)));
        assert!(!e.is_seen(1, index(23, 30)) && !e.is_seen(1, index(37, 30)));

        let mut corner = Explored::new();
        corner.reveal_square(1, 0, 0, ARMY_SIGHT);
        assert_eq!(corner.count(1), 49, "seven by seven survives the clip");
        let mut far = Explored::new();
        far.reveal_square(1, 63, 63, ARMY_SIGHT);
        assert_eq!(far.count(1), 49);
    }

    #[test]
    fn a_realms_bit_is_its_own_and_a_non_realm_sees_nothing() {
        let mut e = Explored::new();
        e.reveal_square(2, 10, 10, 1);
        assert!(e.is_seen(2, index(10, 10)));
        assert!(!e.is_seen(1, index(10, 10)), "realm 1 saw nothing");
        e.reveal_square(0, 40, 40, 6);
        e.reveal_square(crate::levy::OWNERLESS, 40, 40, 6);
        assert_eq!(e.count(0) + e.count(crate::levy::OWNERLESS), 0);
        assert!((0..MAP_TILES).all(|t| e.as_bytes()[t] & !0b0011_1110 == 0));
    }

    #[test]
    fn a_county_is_seen_with_a_one_tile_border() {
        let mut map = CampaignMap::empty();
        for y in 20..=22u8 {
            for x in 20..=23u8 {
                map.set_county(x, y, 5);
            }
        }
        let mut e = Explored::new();
        e.reveal_county(3, &map, 5);
        // A 4 × 3 block grown by one on every side is 6 × 5.
        assert_eq!(e.count(3), 30);
        assert!(e.is_seen(3, index(19, 19)) && e.is_seen(3, index(24, 23)));
        assert!(!e.is_seen(3, index(18, 21)) && !e.is_seen(3, index(25, 21)));
    }

    #[test]
    fn the_painters_test_is_the_option_and_the_bit_together() {
        let mut e = Explored::new();
        e.set_seen(1, 100);
        assert!(!hides(false, &e, 1, 5), "the option off hides nothing");
        assert!(hides(true, &e, 1, 5));
        assert!(!hides(true, &e, 1, 100));
        assert!(hides(true, &e, 2, 100), "another realm's bit is not the viewer's");
    }
}

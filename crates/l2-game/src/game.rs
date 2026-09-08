//! The world, and the assets drawn from it.
//!
//! # One `Game`, borrowed by the screens
//!
//! `docs/plan.md`: *"A `Game` holds the kingdom, the active battle if any, and
//! the screen stack. Screens borrow it; they do not each keep a copy of the
//! world."* [`Game`] is that state, and it is a **plain struct of plain
//! fields** — fixed arrays, small integers, and types that are themselves plain
//! (`l2_kingdom::Kingdom` is arrays of counties and realms). No handle, no
//! index into a texture table, no `Rc`, nothing that only means something while
//! this process is running. That is what lets somebody serialise it later
//! without rewriting it first.
//!
//! [`Assets`] is deliberately *not* part of it. Decoded sprite sheets and a
//! palette are what the machine happens to have loaded, not what the world is,
//! and putting them in the same struct is how a save file ends up with a
//! tile-set in it.

use l2_formats::maps::{MapSet, MapSlot};
use l2_formats::Palette;
use l2_kingdom::county::MAX_COUNTIES;
use l2_kingdom::realm::MAX_REALMS;
use l2_kingdom::tables::RATION_LEVEL_COUNT;
use l2_kingdom::{Kingdom, SeasonReport};
use l2_mods::vfs::Vfs;
use l2_view::campaign::{self, MapAssets};
use l2_view::Ink;

/// The highest tax rate the interface will set.
///
/// **The original's own limit is not established.** The rate is a percentage
/// (`take = Pct(Pct(population, base), taxRate)`), the AI's four ladders top
/// out at 15, and no clamp on the player's slider has been found in the
/// binary. So this is the arithmetic bound, not a reading of `Lords2.exe`, and
/// it is marked as such rather than being quietly asserted as the game's rule.
pub const MAX_TAX_RATE: i32 = 100;

/// Everything the screens draw with. Not part of the world.
pub struct Assets {
    pub palette: Palette,
    pub ink: Ink,
    pub map: MapAssets,
    /// `L2_maps.dat` whole. A `MapSlot` borrows its file, so the bytes are kept
    /// and the slot is re-parsed on demand — which is bounds arithmetic, not
    /// decoding, and costs nothing.
    maps: Vec<u8>,
}

impl Assets {
    /// Load through the mod overlay, so a mod that supplies its own `Base2a.pl8`
    /// or its own palette is picked up with no change to any drawing path.
    pub fn load(vfs: &Vfs) -> Result<Assets, String> {
        let maps = vfs.read("L2_maps.dat").map_err(|e| format!("L2_maps.dat: {e}"))?;
        MapSet::parse(&maps).map_err(|e| format!("L2_maps.dat: {e}"))?;
        let palette = vfs
            .palette(campaign::PALETTE)
            .map_err(|e| format!("{}: {e}", campaign::PALETTE))?;
        let map = MapAssets::load(|name| vfs.read(name).map_err(|e| format!("{name}: {e}")))?;
        Ok(Assets { ink: Ink::for_palette(&palette), palette, map, maps })
    }

    pub fn slot(&self, index: usize) -> Option<MapSlot<'_>> {
        MapSet::parse(&self.maps).ok()?.slot(index).ok()
    }

    /// Assets with nothing in them: a grey ramp for a palette, one blank map
    /// slot, and five tile banks holding a single 2 x 2 frame.
    ///
    /// This is what lets the interface be tested on a machine with no copy of
    /// the game — every screen still lays out, every button is still where it
    /// is, and every assertion about *structure* still holds. Assertions about
    /// the shipped artwork need the install and live in the tests that skip
    /// without it.
    pub fn placeholder() -> Assets {
        // 256 greys, in the 6-bit range a `.256` file holds.
        let mut palette_bytes = vec![0u8; Palette::FILE_LEN];
        for i in 0..256usize {
            let v = (i / 4) as u8;
            palette_bytes[i * 3] = v;
            palette_bytes[i * 3 + 1] = v;
            palette_bytes[i * 3 + 2] = v;
        }
        let palette = Palette::from_bytes(&palette_bytes).expect("768 bytes");

        // The smallest legal PL8: one raw 2 x 2 frame.
        let mut pl8 = vec![0u8; 8 + 16];
        pl8[2] = 1; // one frame
        pl8[8] = 2; // width
        pl8[10] = 2; // height
        pl8[12..16].copy_from_slice(&24u32.to_le_bytes());
        pl8.extend_from_slice(&[1, 2, 3, 4]);
        let map = MapAssets::load(|_| Ok(pl8.clone())).expect("a synthetic sheet parses");

        Assets {
            ink: Ink::for_palette(&palette),
            palette,
            map,
            maps: vec![0u8; l2_formats::maps::SLOT_LEN],
        }
    }
}

/// The world.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Game {
    pub kingdom: Kingdom,
    /// `g_localPlayer` — the realm this machine drives.
    pub player: u8,
    /// The map slot the scenario runs on: `g_scenarioIndex >> 2`, since the
    /// low two bits pick the tile-set's season variant.
    pub map_slot: usize,
    /// The county under the cursor's last click, or 0 for none. County ids are
    /// 1-based in the original, so 0 is a usable "nothing".
    pub selected: u8,
    /// County `+0x6C`, `+0x6D` — each county's anchor tile, which is where its
    /// marker is drawn. Two arrays rather than an array of pairs: index order
    /// is the only order anything here is ever walked in.
    pub anchor_x: [u8; MAX_COUNTIES],
    pub anchor_y: [u8; MAX_COUNTIES],
    /// Each realm's treasury as it stood before the last end-of-turn, so the
    /// interface can show which way the money went.
    pub gold_last: [i32; MAX_REALMS],
    /// What the last season did. Plain data: passes, messages and revolts.
    pub last_report: Option<SeasonReport>,
    /// How many turns this session has ended. `Kingdom::turn_count` is the
    /// game's own counter and starts at 1 in the shipped save; this one counts
    /// what the player did.
    pub turns_played: u32,
}

impl Game {
    /// An empty world. The scenario loader fills it; nothing else should
    /// construct a half-populated one.
    pub fn new(seed: u64) -> Game {
        Game {
            kingdom: Kingdom::new(seed),
            player: 1,
            map_slot: 0,
            selected: 0,
            anchor_x: [0; MAX_COUNTIES],
            anchor_y: [0; MAX_COUNTIES],
            gold_last: [0; MAX_REALMS],
            last_report: None,
            turns_played: 0,
        }
    }

    /// The player's treasury.
    pub fn gold(&self) -> i32 {
        self.kingdom.realms.get(self.player as usize).map_or(0, |r| r.gold)
    }

    /// What the treasury did over the last end-of-turn.
    pub fn gold_change(&self) -> i32 {
        self.gold() - self.gold_last.get(self.player as usize).copied().unwrap_or(0)
    }

    /// Counties held by a realm, counted in index order.
    pub fn owned_by(&self, realm: u8) -> usize {
        self.kingdom
            .county_ids()
            .filter(|&id| self.kingdom.counties[id].owner == realm)
            .count()
    }

    pub fn is_county(&self, id: u8) -> bool {
        id >= 1 && (id as usize) <= self.kingdom.county_count
    }

    /// Whether the player may give this county orders. Setting another realm's
    /// taxes is not a thing the interface refuses for tidiness; it is not the
    /// player's county.
    pub fn is_players(&self, id: u8) -> bool {
        self.is_county(id) && self.kingdom.counties[id as usize].owner == self.player
    }

    /// Select a county, or clear the selection with 0. An id that is not a
    /// county on this map is refused rather than stored.
    pub fn select(&mut self, id: u8) -> bool {
        if id == 0 {
            self.selected = 0;
            return true;
        }
        if !self.is_county(id) {
            return false;
        }
        self.selected = id;
        true
    }

    /// Set a county's tax rate, clamped. Returns false, and changes nothing,
    /// for a county the player does not hold.
    pub fn set_tax_rate(&mut self, id: u8, rate: i32) -> bool {
        if !self.is_players(id) {
            return false;
        }
        self.kingdom.counties[id as usize].tax_rate = rate.clamp(0, MAX_TAX_RATE);
        true
    }

    /// Set a county's wanted ration level, clamped to the six the table holds.
    ///
    /// It writes `rationWanted` (`+0x15E`), never `rationAchieved` (`+0x15D`):
    /// what the player asks for and what the county's stores could actually
    /// feed are different fields, and only the season pipeline decides the
    /// second one.
    pub fn set_ration(&mut self, id: u8, level: i32) -> bool {
        if !self.is_players(id) {
            return false;
        }
        self.kingdom.counties[id as usize].ration_wanted =
            level.clamp(0, RATION_LEVEL_COUNT as i32 - 1);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn two_counties() -> Game {
        let mut g = Game::new(7);
        g.kingdom.set_county_count(2);
        g.kingdom.counties[1].owner = 1;
        g.kingdom.counties[2].owner = 2;
        g
    }

    #[test]
    fn only_the_players_own_counties_take_orders() {
        let mut g = two_counties();
        assert!(g.set_tax_rate(1, 9));
        assert_eq!(g.kingdom.counties[1].tax_rate, 9);

        assert!(!g.set_tax_rate(2, 9), "county 2 belongs to another realm");
        assert_eq!(g.kingdom.counties[2].tax_rate, 0, "and it is unchanged");
        assert!(!g.set_ration(2, 5));
        assert!(!g.set_tax_rate(9, 1), "and 9 is not a county at all");
    }

    #[test]
    fn orders_are_clamped_to_the_ranges_the_rules_have() {
        let mut g = two_counties();
        g.set_tax_rate(1, -40);
        assert_eq!(g.kingdom.counties[1].tax_rate, 0);
        g.set_tax_rate(1, 10_000);
        assert_eq!(g.kingdom.counties[1].tax_rate, MAX_TAX_RATE);

        let achieved = g.kingdom.counties[1].ration_achieved;
        g.set_ration(1, 99);
        assert_eq!(g.kingdom.counties[1].ration_wanted, RATION_LEVEL_COUNT as i32 - 1);
        g.set_ration(1, -3);
        assert_eq!(g.kingdom.counties[1].ration_wanted, 0);
        assert_eq!(
            g.kingdom.counties[1].ration_achieved, achieved,
            "what the player asks for (+0x15E) is not what the county managed to feed (+0x15D)"
        );
    }

    #[test]
    fn selection_refuses_ids_that_are_not_counties_on_this_map() {
        let mut g = two_counties();
        assert!(g.select(2));
        assert_eq!(g.selected, 2);
        assert!(!g.select(3), "county 3 is past g_countyCount");
        assert_eq!(g.selected, 2, "and the refusal leaves the old selection alone");
        assert!(g.select(0));
        assert_eq!(g.selected, 0);
    }

    #[test]
    fn counting_owners_walks_the_map_rather_than_trusting_the_realm_record() {
        let mut g = two_counties();
        g.kingdom.realms[1].county_count = 99; // a stale record
        assert_eq!(g.owned_by(1), 1);
        assert_eq!(g.owned_by(0), 0);
    }
}

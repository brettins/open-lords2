//! Reading the shipped scenario, against a real install.
//!
//! ```text
//! LORDS2_DIR="F:\games\Lords of the Realm II" cargo test -p l2-game
//! ```
//!
//! Skips rather than fails when unset, like `l2-view`'s install tests. **No
//! window is opened and no process is started**; every check reads bytes.
//!
//! The numbers asserted here are `docs/kingdom.md` §9's — the eight independent
//! predictions it landed against this file — plus the correction
//! `docs/plan.md`'s review made to §9's own county count: the save holds
//! **five** owned counties, one for each of realms 1 to 5, and the human realm
//! owns county 8 alone.

use std::env;
use std::path::PathBuf;

use l2_game::game::Assets;
use l2_game::scenario;
use l2_kingdom::tables::{Tables, Weather};
use l2_mods::Platform;

fn install() -> Option<PathBuf> {
    env::var("LORDS2_DIR").ok().map(PathBuf::from).filter(|d| d.is_dir())
}

fn platform(dir: &PathBuf) -> Platform {
    Platform::builder().base(dir).build().expect("the install mounts")
}

macro_rules! skip_without_install {
    () => {
        match install() {
            Some(d) => d,
            None => {
                eprintln!("LORDS2_DIR not set - skipping");
                return;
            }
        }
    };
}

/// The correction. `docs/kingdom.md` §9 and `l2-kingdom`'s reproduction test
/// both say four counties owned by one realm; the bytes say five owned by five
/// different realms, at indices 1, 4, 8, 11 and 13, with owners 5, 4, 1, 3 and
/// 2. Nine are unowned and there are fourteen in all.
#[test]
fn the_shipped_save_holds_five_owned_counties_one_for_each_realm() {
    let dir = skip_without_install!();
    let game = scenario::load(&platform(&dir).vfs, Tables::DEFAULT).expect("the save loads");

    assert_eq!(game.kingdom.county_count, 14);
    let owners: Vec<(usize, u8)> = game
        .kingdom
        .county_ids()
        .map(|id| (id, game.kingdom.counties[id].owner))
        .filter(|(_, owner)| *owner != 0)
        .collect();
    assert_eq!(owners, vec![(1, 5), (4, 4), (8, 1), (11, 3), (13, 2)]);

    assert_eq!(game.player, 1, "g_localPlayer");
    assert_eq!(game.owned_by(game.player), 1, "the human realm owns county 8 alone");
    assert_eq!(game.owned_by(0), 9, "and nine counties are unclaimed");
    assert_eq!(game.selected, 8, "so the game opens on the player's one county");
}

/// §9 points 1, 2 and 3: the clock, the county count and the two options that
/// force every county to Cloudy with zero fertility.
#[test]
fn the_clock_and_the_options_are_read_out_of_the_save() {
    let dir = skip_without_install!();
    let game = scenario::load(&platform(&dir).vfs, Tables::DEFAULT).expect("the save loads");
    let k = &game.kingdom;

    assert_eq!((k.season, k.season_next, k.year, k.turn_count), (4, 1, 1268, 1));
    assert_eq!(k.turn.phase, l2_kingdom::phase::Phase::NeutralCounties, "g_turnPhase = 1");
    assert_eq!(k.options.difficulty, 0);
    assert!(!k.options.advanced_farming);
    assert!(!k.options.armies_eat);
    assert_eq!(game.map_slot, 0, "g_scenarioIndex 0 is England, slot 0");

    for id in k.county_ids() {
        assert_eq!(k.counties[id].weather, Weather::Cloudy, "county {id}");
        assert_eq!(k.counties[id].fertility, 0, "county {id}");
    }
}

/// §9 point 5 and the health chain of point 7, on the two cases the map has:
/// an owned county stores happiness 72 = 65 + 5 + 1 + 1, an unowned one stores
/// 77 with a `+5` from the unowned bonus, and both are at health 67, band 3.
#[test]
fn the_happiness_and_health_the_save_stores_are_the_ones_the_rules_predict() {
    let dir = skip_without_install!();
    let game = scenario::load(&platform(&dir).vfs, Tables::DEFAULT).expect("the save loads");
    let k = &game.kingdom;

    for id in k.county_ids() {
        let c = &k.counties[id];
        assert_eq!(c.happiness_last, 65, "county {id}");
        assert_eq!((c.shown_tax, c.shown_ration, c.shown_health), (5, 1, 1), "county {id}");
        assert_eq!((c.health_meter, c.health_band), (67, 3), "county {id}");
        if c.owner == 0 {
            assert_eq!((c.happiness, c.shown_events), (77, 5), "unowned county {id}");
            assert_eq!((c.population, c.births, c.deaths), (456, 84, 45), "county {id}");
        } else {
            assert_eq!((c.happiness, c.shown_events), (72, 0), "owned county {id}");
            assert_eq!((c.population, c.births, c.deaths), (435, 63, 45), "county {id}");
        }
        assert_eq!(c.pop_last, 417, "county {id}");
    }
}

/// The five realms, their lords and their treasuries, straight out of
/// `g_realms`.
#[test]
fn the_five_realms_are_in_play_with_a_thousand_crowns_each() {
    let dir = skip_without_install!();
    let game = scenario::load(&platform(&dir).vfs, Tables::DEFAULT).expect("the save loads");

    let lords: Vec<u8> = (1..=5).map(|r| game.kingdom.realms[r].lord).collect();
    assert_eq!(lords, vec![0, 1, 2, 4, 3], "lord 0 is the human; the AI lords are 1, 2, 4, 3");
    for r in 1..=5usize {
        assert!(game.kingdom.realms[r].in_play, "realm {r}");
        assert_eq!(game.kingdom.realms[r].gold, 1000, "realm {r}");
        assert_eq!(game.kingdom.realms[r].county_count, 1, "realm {r}");
    }
    assert!(game.kingdom.realms[1].is_human);
    for r in 2..=5usize {
        assert!(!game.kingdom.realms[r].is_human, "realm {r} is an AI");
    }
}

/// **Two independently authored files agreeing, id for id.**
///
/// The adjacency a game runs on is the list the save stores at county `+0x5C`.
/// `L2_maps.dat`'s county plane is a second, entirely separate description of
/// the same geography, and deriving adjacency from it — a tile of one county
/// 4-adjacent to a tile of another — reproduces the stored list exactly for all
/// fourteen counties. Neither file was written with the other in mind, so this
/// would fail loudly if the county plane were misread, if the map slot were the
/// wrong one, or if the save's neighbour list were being read at the wrong
/// offset.
///
/// What it does **not** establish: 8-way adjacency gives the same answer on
/// this map, so the agreement confirms the two readings rather than the shape
/// of the adjacency rule.
#[test]
fn adjacency_derived_from_the_map_matches_the_list_stored_in_the_save() {
    let dir = skip_without_install!();
    let platform = platform(&dir);
    let assets = Assets::load(&platform.vfs).expect("assets load");
    let game = scenario::load(&platform.vfs, Tables::DEFAULT).expect("the save loads");

    let stored: Vec<Vec<u8>> = game
        .kingdom
        .county_ids()
        .map(|id| game.kingdom.counties[id].neighbours().to_vec())
        .collect();
    let counts: Vec<usize> = stored.iter().map(|n| n.len()).collect();
    assert_eq!(counts, vec![1, 3, 4, 3, 3, 6, 5, 3, 3, 7, 5, 4, 4, 3], "the save's own counts");

    let slot = assets.slot(game.map_slot).expect("slot 0");
    let derived = scenario::adjacency_from_map(&slot, game.kingdom.county_count);
    for id in game.kingdom.county_ids() {
        let mut mine = stored[id - 1].clone();
        mine.sort_unstable();
        assert_eq!(derived[id], mine, "county {id}");
    }
    assert_eq!(derived[2], vec![1, 3, 7], "and the lists are ids, not just counts");

    // **The stored order is not ascending**, and that is a fact about the file
    // rather than a problem: county 2's list reads 3, 7, 1 — the order the
    // map loader happened to find them in. It is preserved as it is, because
    // it is the order the original's migration walks, and because it comes
    // from the file it is the same order on every peer. Sorting it here would
    // have been a silent change to a rule's input.
    assert_eq!(stored[1], vec![3, 7, 1], "county 2's neighbours, in the file's own order");

    // Adjacency is symmetric.
    for id in game.kingdom.county_ids() {
        for &n in &stored[id - 1] {
            assert!(
                stored[n as usize - 1].contains(&(id as u8)),
                "county {id} lists {n} but not the other way round"
            );
        }
    }
}

/// The map the save names decodes, holds fourteen counties, and the slot is
/// `g_scenarioIndex` **unshifted**.
///
/// **This test cannot, on its own, distinguish the two readings**, and saying
/// so is the point: the shipped index is 0, and `0 >> 2` is also 0. An earlier
/// revision shifted it right by two on the belief that the low bits named a
/// season variant of the tile set. What settles it is elsewhere —
/// `Map_LoadLattice(slot)` seeks `slot * 0x80C1`, the whole slot stride, and
/// `L2.eng` group 101's sixty strings name slots 0..=59 one for one
/// (`docs/screens.md` §3.1). The identity is asserted here so that a
/// re-introduced shift fails the moment anyone loads a save from any map but
/// the first.
#[test]
fn the_map_slot_the_save_names_is_the_fourteen_county_england() {
    let dir = skip_without_install!();
    let platform = platform(&dir);
    let assets = Assets::load(&platform.vfs).expect("assets load");
    let game = scenario::load(&platform.vfs, Tables::DEFAULT).expect("the save loads");

    let exe = platform.vfs.read("Lords2.exe").expect("Lords2.exe");
    let bytes = platform.vfs.read("lastturn.sav").expect("lastturn.sav");
    let save = l2_formats::Save::open(&exe, &bytes).expect("the save opens");
    let index = save.globals().expect("the globals block").scenario_index;
    assert_eq!(index, 0, "the shipped save is England, slot 0");
    assert_eq!(game.map_slot as i32, index, "the slot is the index, not the index >> 2");

    let slot = assets.slot(game.map_slot).expect("slot 0");
    assert!(!slot.is_empty());
    assert_eq!(slot.county_count(), game.kingdom.county_count);
}

/// The realm colour byte at `+0x0A`, which picks a realm's banner in the menu
/// bar and its ramp on the minimap.
///
/// `FUN_004171EE` clamps it to 1..=5 before using it as a frame index, and the
/// clamp matters: `Misc_cty` frame `0x55 + 0` is 13 x 37 and would overflow the
/// 24-pixel menu bar, while 1..=5 land on the five 13 x 16 frames that fit.
#[test]
fn every_realm_in_the_save_flies_a_colour_the_banner_frames_have() {
    let dir = skip_without_install!();
    let game = scenario::load(&platform(&dir).vfs, Tables::DEFAULT).expect("the save loads");
    for id in 1..game.kingdom.realms.len() {
        let c = game.realm_colour[id];
        assert!((1..=5).contains(&c), "realm {id} flies colour {c}");
    }
    // Realm 0 is never a realm and keeps the sentinel `Game::new` seeded.
    assert_eq!(game.realm_colour[0], 0);
    // Five realms, five colours, no two the same — which is what makes the
    // banners in the menu bar tell them apart.
    let mut seen = game.realm_colour[1..].to_vec();
    seen.sort_unstable();
    seen.dedup();
    assert_eq!(seen.len(), 5, "the five realms fly five different colours");
}

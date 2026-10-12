#![allow(unused_imports)]
use super::*;
use l2_formats::maps::Plane;
use l2_game::scenario::{new_game_seed, SEED};

/// `L2.eng` group 101's eighteenth name, and `Campaign_LoadEntry`'s first map.
const QUAINTVILLE: usize = 17;

/// The two `0x80`-tile markers of slot 17, from `L2_maps.dat`: marker 1 is
/// county 2 at tile x 31, marker 2 is county 4 at tile x 46.
fn starts(assets: &Assets) -> Vec<(u8, u8, usize)> {
    let slot = assets.slot(QUAINTVILLE).expect("slot 17");
    let mut out = Vec::new();
    for y in 0..64 {
        for x in 0..64 {
            let marker = slot.at(Plane::Marker, x, y);
            if marker != 0 && slot.flags_at(x, y) & l2_formats::maps::flags::SETTLEMENT != 0 {
                out.push((marker, slot.county_at(x, y), x));
            }
        }
    }
    out.sort();
    out
}

/// The county realm 1 holds, for one seed.
fn human_county(assets: &Assets, seed: u64) -> usize {
    let base = l2_game::setup::SetupOptions::new().commit(1, l2_kingdom::Quirks::default());
    let settings = l2_game::setup::Settings { ai_lords: 1, ..base };
    let tables = Game::new(SEED).kingdom.tables;
    let game = scenario::new_game(assets, QUAINTVILLE, &settings, 1, 1, seed, tables)
        .expect("Quaintville builds");
    game.kingdom
        .county_ids()
        .find(|&id| game.kingdom.counties[id].owner == game.player)
        .expect("the human holds a county")
}

/// **Player report, 2026-09-17: the human always started on the east side of
/// Quaintville.**
///
/// `Game_SetupRealmsAndCounties` (`0x0049BD99`) seats realm `i` on
/// `g_playerStartTable[i * 2]` and the human is realm 1 (`FUN_004978AD`:
/// `g_localPlayer = 1`). `PlayerStart_Shuffle` (`0x00497E65`) deals entry `i`
/// to `pos = (rand & 3) + 1 + i`, wrapping at the live count: with two live
/// starts entry 1 reaches slot 1 unless `rand & 3 == 0`, so the human takes
/// marker 1's county in three games of four. The original re-rolls that deal
/// every game off its live LFSR state; ours dealt from one fixed seed, and
/// `SEED`'s first draw is the one-in-four arm.
#[test]
fn the_human_takes_quaintvilles_western_start_three_games_in_four() {
    let assets = assets!();
    let starts = starts(&assets);
    assert_eq!(starts.len(), 2, "slot 17 seats two, and the campaign spends one on the AI");
    let west = starts[0].1 as usize;
    let east = starts[1].1 as usize;
    // Tile x 31 against 46, and x - y is -15 against +4, so the marker-1
    // county is west of the marker-2 county on either reading of the lattice.
    assert!(starts[0].2 < starts[1].2, "marker 1's tile is the western one");
    assert_eq!((west, east), (2, 4), "counties 2 and 4 are what the two markers name");

    let mut in_the_west = 0;
    for seed in 0..100u64 {
        let got = human_county(&assets, seed);
        assert!(got == west || got == east, "seed {seed}: county {got} is not a start county");
        in_the_west += usize::from(got == west);
    }
    assert!(
        (65..=85).contains(&in_the_west),
        "the deal's three-in-four, and it was {in_the_west} of 100",
    );

    // The report itself: the seed *Start* used to pass forever.
    assert_eq!(human_county(&assets, SEED), east, "SEED is the one-in-four arm");
    assert_eq!(new_game_seed(None, None), SEED, "no clock, so tests keep the fixed world");
    assert_ne!(new_game_seed(Some(1), None), new_game_seed(Some(2), None), "a new deal");
}

/// D-5. Ablation: `new_game_seed`'s `(Some(s), _) => s.seed` arm.
#[test]
fn a_network_game_deals_from_start_not_the_clock() {
    let start = l2_net::Start { seed: 0x5EED, roster: Default::default(), unseated_gold: 0, setup: vec![], save: vec![] };
    assert_eq!(new_game_seed(Some(1), Some(&start)), 0x5EED);
    assert_eq!(new_game_seed(Some(2), Some(&start)), 0x5EED);
}

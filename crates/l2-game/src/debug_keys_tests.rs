use super::*;

fn game() -> Game {
    Game::new(1)
}

/// Ablation: drop the `screen == Some(0x09)` test and the second half fails.
#[test]
fn f2_cycles_the_mode_everywhere_and_flips_food_bands_only_on_court() {
    let mut g = game();
    let modes: Vec<u8> = (0..3)
        .map(|_| {
            key_down(&mut g, Key::F2, Some(0x00));
            g.debug_keys.panel_mode
        })
        .collect();
    assert_eq!(modes, [1, 2, 0]);
    assert!(!g.debug_keys.food_bands, "0x71 off screen 9 leaves DAT_00553E60");
    key_down(&mut g, Key::F2, Some(0x09));
    assert!(g.debug_keys.food_bands);
    key_down(&mut g, Key::F2, Some(0x09));
    assert!(!g.debug_keys.food_bands);
}

/// Ablation: `< LAST_MAN` to `<=` and the ceiling moves.
#[test]
fn the_arrows_step_the_debug_man_through_0_to_0x50() {
    let mut g = game();
    key_down(&mut g, Key::Left, None);
    assert_eq!(g.debug_keys.selected_man, 0);
    for _ in 0..0x60 {
        key_down(&mut g, Key::Right, None);
    }
    assert_eq!(g.debug_keys.selected_man, 0x50);
}

/// Ablation: drop the `if dk.debug` guard on F3 or F4 and the first half fails.
#[test]
fn f3_and_f4_answer_only_with_the_debug_flag() {
    let mut g = game();
    let pop = g.kingdom.counties[3].population;
    key_down(&mut g, Key::F3, None);
    key_down(&mut g, Key::F4, None);
    assert!(!g.debug_keys.panel_on);
    assert_eq!(g.kingdom.counties[3].population, pop);
    g.debug_keys.debug = true;
    key_down(&mut g, Key::F3, None);
    key_down(&mut g, Key::F4, None);
    assert!(g.debug_keys.panel_on);
    assert_eq!(g.kingdom.counties[3].population, pop + 1, "g_battlePhase == 0 arm");
}

/// `DAT_00553E60` set: `Minimap_ComputeBands` spreads achieved 0..=3 over 1..=4.
#[test]
fn the_food_debug_branch_bands_by_ration_achieved() {
    let mut c = game().kingdom.counties[1].clone();
    c.ration_wanted = 0;
    let bands: Vec<u8> = (0..6)
        .map(|r| {
            c.ration_achieved = r;
            c.minimap_bands_with(true).food
        })
        .collect();
    assert_eq!(bands, [1, 2, 3, 4, 5, 5]);
    assert_eq!(c.minimap_bands().food, 6);
}

/// CNEW-net-local. Ablation: `!game.net.lockstep` on F4.
#[test]
fn f4_changes_nothing_in_a_network_game() {
    let mut g = game();
    g.debug_keys.debug = true;
    g.net.lockstep = true;
    let pop = g.kingdom.counties[3].population;
    key_down(&mut g, Key::F4, None);
    assert_eq!(g.kingdom.counties[3].population, pop);
}

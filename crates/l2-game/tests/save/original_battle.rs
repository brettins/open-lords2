//! A save taken mid-battle resumes it: `FUN_004976A1` (`0x004976A1`),
//! `g_battlePhase` 2 -> screen 0x29. Ablation: drop the `saved_battle::resume`
//! call in `scenario::from_save` and the first assertion fails.
use l2_formats::save::Save;
use l2_kingdom::tables::Tables;

/// `g_battleMen` (`0x00554480`, stride `0x1B0`): owner `+0x2C`, x/y `+0x20`/`+0x22`.
fn saved_men(save: &Save) -> Vec<(u8, u8)> {
    (1..=80u32)
        .map(|i| 0x0055_4480 + i * 0x1B0)
        .filter(|&r| save.u8_at(r + 0x2C).unwrap() != 0)
        .map(|r| (save.i16_at(r + 0x20).unwrap() as u8, save.i16_at(r + 0x22).unwrap() as u8))
        .collect()
}

#[test]
fn a_battle_saved_on_the_field_is_on_the_field_after_the_load() {
    for name in ["battle-during.sav", "siege-sieging.sav", "noorders-start.sav"] {
        let save: Save = l2_testkit::fixture!(name);
        let game = l2_game::scenario::from_save(&save, Tables::DEFAULT).expect("loads");
        let live = game.battle.as_ref().unwrap_or_else(|| panic!("{name}: no battle"));
        let r = &live.runner;
        let ours: Vec<(u8, u8)> = r
            .fighters
            .iter()
            .filter(|f| r.sim.figures[f.sim].is_alive())
            .map(|f| (f.x, f.y))
            .collect();
        assert_eq!(ours, saved_men(&save), "{name}: every live man where the save put him");
    }
}

#[test]
fn a_save_taken_on_the_map_loads_no_battle() {
    let save: Save = l2_testkit::fixture!("battle-before.sav");
    let game = l2_game::scenario::from_save(&save, Tables::DEFAULT).expect("loads");
    assert!(game.battle.is_none());
}

/// DAT_0053F084 comes back from the save: Battle_CheckOutcome 0x00477DFC's film column.
/// Ablation: drop the `films.battle` line in `scenario::from_save`.
#[test]
fn the_battle_film_column_is_the_saves() {
    let save: Save = l2_testkit::fixture!("siege-sieging.sav");
    let game = l2_game::scenario::from_save(&save, Tables::DEFAULT).expect("loads");
    assert_eq!(game.films.battle, 1);
}

/// FUN_004782C5 0x004782C5 `00470000.c:3452-3459`: a loaded battle ends on screen 0x13 too.
/// Ablation: drop the `game.turn.is_none()` arm in `turn::finish_battle` and this is `Stuck`.
#[test]
fn a_battle_loaded_from_a_save_ends_on_its_report() {
    use l2_game::turn::{self, TurnStep};
    let save: Save = l2_testkit::fixture!("battle-during.sav");
    let mut game = l2_game::scenario::from_save(&save, Tables::DEFAULT).expect("loads");
    assert!(matches!(turn::finish_battle(&mut game), TurnStep::Report(_)));
    assert!(matches!(turn::dismiss_report(&mut game), TurnStep::Running));
    assert!(game.battle.is_none() && turn::pending_report(&game).is_none());
}

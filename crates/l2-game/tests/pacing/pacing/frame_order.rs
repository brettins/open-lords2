use super::*;

fn turn_state(game: &l2_game::Game) -> (u8, u32, u8) {
    let t = &game.kingdom.turn;
    (t.phase as u8, t.step, t.ai_cursor)
}

fn end_turn(game: &mut l2_game::Game, machine: &mut Machine, assets: &Assets) {
    let mut ctx = Ctx { game, assets };
    machine.handle(l2_game::screens::map::END_TURN_BUTTON.click(), &mut ctx);
}

/// FUN_00476E21 0x00476E21 restores g_screenId inside Msg_Dismiss, so the next FUN_004BB978
/// gate is open: the frame after a tip's dismissal ticks. Ablate: seat_tip_host before ticks_withheld.
#[test]
fn the_frame_after_a_tip_is_dismissed_runs_a_tick() {
    let mut game = england!();
    game.prefs.tip_screens = true;
    let mut machine = Machine::new(ScreenId::Campaign);
    let assets = Assets::placeholder();
    end_turn(&mut game, &mut machine, &assets);
    for _ in 0..GIVE_UP {
        if game.tips.hosting() {
            break;
        }
        let mut ctx = Ctx { game: &mut game, assets: &assets };
        machine.update(&mut ctx);
    }
    assert!(game.tips.hosting(), "a tip came up during the turn");
    l2_game::message::dismiss(&mut game);
    let held = turn_state(&game);
    {
        let mut ctx = Ctx { game: &mut game, assets: &assets };
        machine.update(&mut ctx);
    }
    assert_ne!(turn_state(&game), held, "Turn_Tick ran");
}

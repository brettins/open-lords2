use super::*;
use l2_game::screens::confirm::Ask;

/// `Battle_RetreatButton` 0x0043BA29 is `Ui_OpenConfirm(11, 0xA0, 0xA0, FUN_0043BB68)` on a
/// siege, and `Ui_OpenConfirm` 0x0040E6F2 ends `FUN_004B36C0(prompt)`: screen `0x1E` speaks
/// 10/11. Ablated by restoring `self.confirm = live.press_retreat()` in the battlefield:
/// no `Confirm` on top and nothing spoken.
#[test]
fn the_battle_s_retreat_opens_the_one_confirm_box_and_it_speaks() {
    let (mut g, a, mut m) = on_the_map();
    let (keep, _garrison) = castle_with_garrison(&mut g, &mut m, &a, 1, 2, 200);
    let besieger = army_at(&mut g, 2, 2, 600, (keep.0 - 1, keep.1 + 1));
    lay_siege(&mut m, &mut g, &a, besieger, keep, 1);
    end_turns_until_the_assault(&mut m, &mut g, &a);
    take_the_field(&mut m, &mut g, &a);

    let before = g.spoken.0;
    click(&mut m, &mut g, &a, on(bf::Button::Retreat.rect()));
    assert_eq!(m.top_id(), Some(ScreenId::Confirm(Ask::Surrender)));
    tick(&mut m, &mut g, &a);
    assert_ne!(g.spoken.0, before, "FUN_004B36C0 fired");
    assert_eq!(g.spoken.1, l2_game::audio::names::speech::CONFIRM_PROMPT[11]);

    let men = g.battle.as_ref().expect("a live battle").runner.tick;
    tick(&mut m, &mut g, &a);
    let live = g.battle.as_ref().expect("still a battle");
    assert!(live.runner.tick > men, "Battle_Frame steps the men under 0x1E (g_screenId < 0x2B)");

    click(&mut m, &mut g, &a, on(l2_game::screens::battlefield::CONFIRM_NO));
    for _ in 0..20 {
        tick(&mut m, &mut g, &a);
    }
    assert_eq!(m.top_id(), Some(ScreenId::Battlefield), "the cross returns to the field");
    assert!(!g.battle.as_ref().expect("still a battle").autocalc);
}

use super::*;
use l2_game::screens::confirm::Ask;

/// `there` becomes a type-2 castle of county 1, owned by realm 1 (Map_HoverUnitTarget's
/// `flags & 0x80 && content > 0x14`, `004a0000.c:3759`).
fn castle_next_door(g: &mut Game) -> ((u8, u8), (u8, u8)) {
    let (here, there) = adjacent_pair(|x| x < 30);
    let t = there.1 as usize * MAP_DIM + there.0 as usize;
    let map = &mut g.kingdom.campaign.map;
    (map.flags[t], map.terrain[t], map.county[t]) = (map.flags[t] | flags::SETTLEMENT, 0x16, 1);
    g.kingdom.counties[1].owner = 1;
    g.kingdom.counties[1].castle_type = 2;
    (here, there)
}

fn ask_to_garrison(g: &mut Game, a: &Assets, m: &mut Machine, men: i32) -> usize {
    let (here, there) = castle_next_door(g);
    let id = army_at(g, 1, 1, men, here);
    click(m, g, a, pixel(here.0, here.1).unwrap());
    click(m, g, a, pixel(there.0, there.1).unwrap());
    id
}

/// Map_ConfirmMoveOrder 0x004A9252 `Ui_OpenConfirm(7, …, MoveOrder_ConfirmGarrison)`, and
/// Ui_OpenConfirm's FUN_004B36C0 speaks 10/7. Ablated by dropping the `garrison_ask` push in
/// `confirm_move_order`: the army marches with no box.
#[test]
fn a_march_onto_your_own_castle_asks_garrison_castle_and_yes_orders_it() {
    let (mut g, a, mut m) = on_the_map();
    let id = ask_to_garrison(&mut g, &a, &mut m, 50);
    assert!(matches!(m.top_id(), Some(ScreenId::Confirm(Ask::Garrison { unit, county: 1, .. })) if unit == id));
    assert!(g.kingdom.campaign.units.get(id).is_some_and(|u| !u.moving), "nothing ordered yet");
    let before = g.spoken.0;
    tick(&mut m, &mut g, &a);
    assert_ne!(g.spoken.0, before);
    assert_eq!(g.spoken.1, l2_game::audio::names::speech::CONFIRM_PROMPT[7]);
    press_and_wait(&mut m, &mut g, &a, on(l2_game::screens::battlefield::CONFIRM_YES));
    assert!(g.kingdom.campaign.units.get(id).is_some_and(|u| u.moving), "Unit_OrderMove");
}

/// MoveOrder_ConfirmGarrison 0x004A9372 `:3913`: more men than room raises 0xA6 with the unit,
/// category 0x11, the county and g_hoverTileOffset `(y*64+x)*8`. Ablated by ordering the
/// move in place of the `0xA6` arm.
#[test]
fn a_garrison_too_big_for_the_castle_offers_the_split_message() {
    let (mut g, a, mut m) = on_the_map();
    let cap = l2_kingdom::industry::garrison_cap(&g.kingdom.tables, 2);
    let id = ask_to_garrison(&mut g, &a, &mut m, cap + 1);
    let Some(ScreenId::Confirm(Ask::Garrison { dest, .. })) = m.top_id() else { panic!("no box") };
    press_and_wait(&mut m, &mut g, &a, on(l2_game::screens::battlefield::CONFIRM_YES));
    assert!(g.kingdom.campaign.units.get(id).is_some_and(|u| !u.moving), "not ordered");
    let msg = g.messages.open().copied().or_else(|| {
        tick(&mut m, &mut g, &a);
        g.messages.open().copied()
    });
    let rec = msg.expect("a message");
    assert_eq!((rec.group, rec.category, rec.county, rec.variant as usize), (0xA6, 0x11, 1, id));
    assert_eq!(rec.payload, ((dest.1 as i32) * 64 + dest.0 as i32) * 8);
}

/// `:3933`: a garrison under siege refuses ahead of every other rung with 0x121.
#[test]
fn a_besieged_castle_refuses_with_message_289() {
    let (mut g, a, mut m) = on_the_map();
    let (_, there) = castle_next_door(&mut g);
    let keep = army_at(&mut g, 1, 1, 10, there);
    g.kingdom.campaign.units.get_mut(keep).unwrap().besieged_by = 9;
    g.kingdom.counties[1].garrison_unit = keep;
    g.kingdom.campaign.units.get_mut(keep).unwrap().garrison_county = 1;
    let (here, _) = adjacent_pair(|x| x < 30);
    let id = army_at(&mut g, 1, 1, 20, here);
    click(&mut m, &mut g, &a, pixel(here.0, here.1).unwrap());
    click(&mut m, &mut g, &a, pixel(there.0, there.1).unwrap());
    press_and_wait(&mut m, &mut g, &a, on(l2_game::screens::battlefield::CONFIRM_YES));
    tick(&mut m, &mut g, &a);
    assert!(g.kingdom.campaign.units.get(id).is_some_and(|u| !u.moving));
    assert_eq!(g.messages.open().map(|r| r.group), Some(0x121));
}

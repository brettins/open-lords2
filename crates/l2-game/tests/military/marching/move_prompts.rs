use super::*;
use l2_game::screens::battlefield::CONFIRM_YES;
use l2_game::screens::confirm::Ask;

/// `there` becomes a tile of county 2, owned by realm 2, with `flag` and `terrain`.
fn foreign_next_door(g: &mut Game, flag: u8, terrain: u8) -> ((u8, u8), (u8, u8)) {
    let (here, there) = adjacent_pair(|x| x < 30);
    let t = there.1 as usize * MAP_DIM + there.0 as usize;
    let map = &mut g.kingdom.campaign.map;
    (map.flags[t], map.terrain[t], map.county[t]) = (flag, terrain, 2);
    g.kingdom.counties[2].owner = 2;
    (here, there)
}

fn order(g: &mut Game, a: &Assets, m: &mut Machine, here: (u8, u8), there: (u8, u8)) -> usize {
    let id = army_at(g, 1, 1, 30, here);
    click(m, g, a, pixel(here.0, here.1).unwrap());
    click(m, g, a, pixel(there.0, there.1).unwrap());
    id
}

fn moving(g: &Game, id: usize) -> bool {
    g.kingdom.campaign.units.get(id).is_some_and(|u| u.moving)
}

fn message(m: &mut Machine, g: &mut Game, a: &Assets) -> Option<u16> {
    tick(m, g, a);
    g.messages.open().map(|r| r.group)
}

/// Map_HoverUnitTarget `004a0000.c:3749` + Map_ConfirmMoveOrder `:3869` Ui_OpenConfirm(4, …,
/// MoveOrder_Confirm). Ablated by dropping the `h.village` rung in `pick`: no box.
#[test]
fn a_march_onto_a_foreign_village_asks_slaughter_villagers_and_yes_orders_it() {
    let (mut g, a, mut m) = on_the_map();
    let (here, there) = foreign_next_door(&mut g, flags::PLOT | flags::FARMLAND, 5);
    let id = order(&mut g, &a, &mut m, here, there);
    assert!(matches!(m.top_id(), Some(ScreenId::Confirm(Ask::Slaughter { unit, .. })) if unit == id));
    assert!(!moving(&g, id), "nothing ordered yet");
    tick(&mut m, &mut g, &a);
    assert_eq!(g.spoken.1, l2_game::audio::names::speech::CONFIRM_PROMPT[4]);
    press_and_wait(&mut m, &mut g, &a, on(CONFIRM_YES));
    assert!(moving(&g, id), "MoveOrder_Confirm → Unit_OrderMove");
}

/// `:3753` County_TileIsField + `:3865` Ui_OpenConfirm(10, …). Ablated by dropping the
/// `h.field` rung in `pick`.
#[test]
fn a_march_onto_a_foreign_field_asks_destroy_field() {
    let (mut g, a, mut m) = on_the_map();
    let (here, there) = foreign_next_door(&mut g, flags::FARMLAND, 5);
    let id = order(&mut g, &a, &mut m, here, there);
    assert!(matches!(m.top_id(), Some(ScreenId::Confirm(Ask::DestroyField { unit, .. })) if unit == id));
    tick(&mut m, &mut g, &a);
    assert_eq!(g.spoken.1, l2_game::audio::names::speech::CONFIRM_PROMPT[10]);
}

/// County_TileIsField 0x00469E0E: content 0x17 and up is not a field. Ablated by widening the
/// `(2..0x17)` range in `hover`.
#[test]
fn farmland_past_content_0x16_is_not_asked_about() {
    let (mut g, a, mut m) = on_the_map();
    let (here, there) = foreign_next_door(&mut g, flags::FARMLAND, 0x17);
    let id = order(&mut g, &a, &mut m, here, there);
    assert_eq!(m.top_id(), Some(ScreenId::Campaign));
    assert!(moving(&g, id));
}

/// `:3853` Ui_OpenConfirm(8, …, MoveOrder_ConfirmSiege); `:3968` a garrisoned castle is ordered.
/// Ablated by dropping the `h.siege` rung in `pick`.
#[test]
fn a_march_onto_a_foreign_castle_asks_besiege_castle_and_yes_orders_it() {
    let (mut g, a, mut m) = on_the_map();
    let (here, there) = foreign_next_door(&mut g, flags::SETTLEMENT, 0x16);
    let keep = army_at(&mut g, 2, 2, 10, (0, 0));
    g.kingdom.campaign.units.get_mut(keep).unwrap().garrison_county = 2;
    g.kingdom.counties[2].garrison_unit = keep;
    let id = order(&mut g, &a, &mut m, here, there);
    assert!(matches!(m.top_id(), Some(ScreenId::Confirm(Ask::Siege { county: 2, .. }))));
    tick(&mut m, &mut g, &a);
    assert_eq!(g.spoken.1, l2_game::audio::names::speech::CONFIRM_PROMPT[8]);
    press_and_wait(&mut m, &mut g, &a, on(CONFIRM_YES));
    assert!(moving(&g, id));
}

/// MoveOrder_ConfirmSiege `:3961`: no garrison and castleDegraded 0 → 0x11D, nothing ordered.
/// Ablated by ordering the move in place of the `keep == 0` arm.
#[test]
fn besieging_a_deserted_castle_refuses_with_message_285() {
    let (mut g, a, mut m) = on_the_map();
    let (here, there) = foreign_next_door(&mut g, flags::SETTLEMENT, 0x16);
    let id = order(&mut g, &a, &mut m, here, there);
    press_and_wait(&mut m, &mut g, &a, on(CONFIRM_YES));
    assert!(!moving(&g, id));
    assert_eq!(message(&mut m, &mut g, &a), Some(0x11D));
}

/// MoveOrder_ConfirmSiege `:3975`: the garrison already besieged → 0x113.
#[test]
fn besieging_a_castle_already_under_siege_refuses_with_message_275() {
    let (mut g, a, mut m) = on_the_map();
    let (here, there) = foreign_next_door(&mut g, flags::SETTLEMENT, 0x16);
    let keep = army_at(&mut g, 2, 2, 10, (0, 0));
    g.kingdom.campaign.units.get_mut(keep).unwrap().besieged_by = 9;
    g.kingdom.counties[2].garrison_unit = keep;
    let id = order(&mut g, &a, &mut m, here, there);
    press_and_wait(&mut m, &mut g, &a, on(CONFIRM_YES));
    assert!(!moving(&g, id));
    assert_eq!(message(&mut m, &mut g, &a), Some(0x113));
}

/// MoveOrder_Confirm `:4031`: a foreign county town (`flags & 0x40`, `:3745`) whose castle holds a
/// garrison refuses with 0x11E. Ablated by dropping `town_refusal` in `confirm_move_order`.
#[test]
fn marching_on_a_garrisoned_county_town_refuses_with_message_286() {
    let (mut g, a, mut m) = on_the_map();
    let (here, there) = foreign_next_door(&mut g, flags::CASTLE, 0);
    let keep = army_at(&mut g, 2, 2, 10, (0, 0));
    g.kingdom.counties[2].garrison_unit = keep;
    let id = order(&mut g, &a, &mut m, here, there);
    assert_eq!(m.top_id(), Some(ScreenId::Campaign));
    assert!(!moving(&g, id));
    assert_eq!(message(&mut m, &mut g, &a), Some(0x11E));
}

/// Widget_Test kind 5 `Sound_RestartSlot(1)` on g_confirmWidgets (Screen_HandleInput
/// `004b0000.c:7311`). Ablated by dropping ConfirmScreen's `take_clicks`.
#[test]
fn a_thumb_on_the_confirm_box_clicks() {
    let (mut g, a, mut m) = on_the_map();
    let (here, there) = foreign_next_door(&mut g, flags::FARMLAND, 5);
    order(&mut g, &a, &mut m, here, there);
    let before = m.clicks();
    click(&mut m, &mut g, &a, on(CONFIRM_YES));
    assert_eq!(m.clicks(), before.wrapping_add(1));
}

use super::*;

/// Panel_SplitButton 0x004378B3 00430000.c:4288-4293: a marched army is refused at the
/// button with 0x95, the panel closes to the map and the next map click is latched.
/// Ablate: the `(2, _) if … moves_used >= 1` arm in the info panel.
#[test]
fn a_marched_armys_split_is_refused_at_the_button() {
    let (mut g, a, mut m) = on_the_map();
    let (here, _) = adjacent_pair(|x| x < 30);
    let id = army_at(&mut g, 1, 1, 300, here);
    g.kingdom.campaign.units.get_mut(id).unwrap().moves_used = 1;

    right_click(&mut m, &mut g, &a, pixel(here.0, here.1).unwrap());
    click(&mut m, &mut g, &a, info_button(2));

    assert_eq!(m.top_id(), Some(ScreenId::Campaign), "g_screenId = 0; 0x11 never opens");
    g.messages.pull();
    assert_eq!(g.messages.open().map(|r| r.group), Some(0x95), "scroll 149 says why");
    assert!(g.map_click_latch, "DAT_0052afa4 = 1");
}

/// FUN_00438B02 0x00438B02 00430000.c:4707 `DAT_0052afa4 = 0` after Field_SetType.
/// Ablate: `ctx.game.map_click_latch = false;` in the brush arm.
#[test]
fn the_field_brush_clears_the_map_click_latch() {
    use l2_game::screens::info::{Target, BRUSH_DIM, BRUSH_FIELD_X, BRUSH_ROW_Y};
    let (mut g, a, mut m) = on_the_map();
    let tile = {
        let map = &mut g.kingdom.campaign.map;
        let c = &mut g.kingdom.counties[1];
        for slot in 0..20u8 {
            let (x, y) = (10 + slot % 5, 20 + slot / 5);
            map.set_flags(x, y, flags::FARMLAND);
            map.terrain[l2_kingdom::map::index(x, y)] = l2_kingdom::field::terrain::FALLOW;
            map.county[l2_kingdom::map::index(x, y)] = 1;
            c.set_field_tile(slot as usize, Some(l2_kingdom::map::index(x, y)));
        }
        l2_kingdom::map::index(10, 20)
    };
    m.push(ScreenId::Info(Target::Tile(tile)));
    g.map_click_latch = true;
    click(&mut m, &mut g, &a, (BRUSH_FIELD_X[1] + BRUSH_DIM / 2, BRUSH_ROW_Y + BRUSH_DIM / 2));
    assert_eq!(m.top_id(), Some(ScreenId::Campaign), "the brush closes the panel");
    assert!(!g.map_click_latch);
}

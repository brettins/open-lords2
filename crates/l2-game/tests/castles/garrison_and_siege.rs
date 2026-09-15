#![allow(unused_imports)]
use super::*;
use super::construction::*;
use l2_game::game::Assets;
use l2_game::input::{Event, Key, Rect};
use l2_game::screen::{Ctx, Machine, ScreenId};
use l2_game::screens::{castle, map};
use l2_game::Game;
use l2_kingdom::map::{flags, terrain, CampaignMap, MAP_DIM, MAP_TILES};
use l2_kingdom::unit::{TroopType, Unit, UnitKind};
use l2_kingdom::MercenaryBands;
use l2_view::campaign;

#[test]
fn marching_an_army_onto_your_own_castle_puts_it_inside() {
    let (mut g, a, mut m) = on_the_map();
    let here = visible_in(&g, 1);
    plot(&mut g, 1, here);
    g.kingdom.counties[1].castle_type = 2; // a motte and bailey, 200 men
    l2_kingdom::map::stamp_castle_terrain(&mut g.kingdom.campaign.map, 1, 2);

    let outside = (here.0 - 1, here.1);
    let id = army_at(&mut g, 1, 1, 150, outside);

    click(&mut m, &mut g, &a, pixel(outside.0, outside.1).unwrap());
    click(&mut m, &mut g, &a, pixel(here.0, here.1).unwrap());
    assert!(g.kingdom.campaign.units.get(id).is_some_and(|u| u.moving), "ordered from the map");

    march(&mut m, &mut g, &a);
    end_turn(&mut m, &mut g, &a);

    assert_eq!(g.kingdom.counties[1].garrison_unit, id, "the county holds the link");
    let unit = g.kingdom.campaign.units.get(id).expect("still there");
    assert_eq!(unit.garrison_county, 1, "and so does the unit");
    assert_eq!((unit.x, unit.y), here, "teleported onto the block");
}

#[test]
fn a_castle_refuses_more_men_than_it_can_barrack() {
    let (mut g, a, mut m) = on_the_map();
    let here = visible_in(&g, 1);
    plot(&mut g, 1, here);
    g.kingdom.counties[1].castle_type = 1; // a palisade: 150 men
    l2_kingdom::map::stamp_castle_terrain(&mut g.kingdom.campaign.map, 1, 1);

    let outside = (here.0 - 1, here.1);
    let id = army_at(&mut g, 1, 1, 151, outside);
    click(&mut m, &mut g, &a, pixel(outside.0, outside.1).unwrap());
    click(&mut m, &mut g, &a, pixel(here.0, here.1).unwrap());
    end_turn(&mut m, &mut g, &a);

    assert_eq!(g.kingdom.counties[1].garrison_unit, 0, "151 men will not fit in 150");
    let unit = g.kingdom.campaign.units.get(id).expect("still there");
    assert_eq!(unit.garrison_county, 0);
    assert_ne!((unit.x, unit.y), here, "and it did not move onto the block");
}


#[test]
fn an_army_that_marches_onto_an_enemy_castle_besieges_it_and_the_turn_settles_the_assault() {
    let (mut g, a, mut m) = on_the_map();
    let keep = visible_in(&g, 2);
    plot(&mut g, 2, keep);
    g.kingdom.counties[2].castle_type = 1;
    l2_kingdom::map::stamp_castle_terrain(&mut g.kingdom.campaign.map, 2, 1);

    let garrison = army_at(&mut g, 2, 2, 40, (keep.0 - 1, keep.1));
    {
        let map = g.kingdom.campaign.map.clone();
        l2_kingdom::movement::order_move(
            &map,
            &mut g.kingdom.campaign.units,
            garrison,
            keep,
            l2_kingdom::movement::Routing::Direct,
        )
        .expect("one tile");
    }
    end_turn(&mut m, &mut g, &a);
    assert_eq!(
        g.kingdom.counties[2].garrison_unit, garrison,
        "the enemy castle is manned, and by a march rather than by an assignment",
    );

    assert!(
        !l2_kingdom::conquest::can_be_entered(
            &g.kingdom.counties,
            &g.kingdom.campaign.units,
            2,
            1
        ),
        "a castle plus a garrison shuts the county",
    );

    let camp = (keep.0 - 1, keep.1 + 1);
    let besieger = army_at(&mut g, 1, 1, 800, camp);
    click(&mut m, &mut g, &a, pixel(camp.0, camp.1).unwrap());
    click(&mut m, &mut g, &a, pixel(keep.0, keep.1).unwrap());
    assert!(g.kingdom.campaign.units.get(besieger).is_some_and(|u| u.moving), "ordered");

    // **The march happens while he watches it, and it is the march that lays
    // the siege** — `Unit_ReachCastleBuilding`, on the frame the army reaches
    // the keep. Phase 2 is what *storms* the palisade, and it runs at the top
    // of the turn: an army still walking when End Turn is pressed has missed
    // it. See [`march`], and `docs/decisions.md` C115 for why the order walks
    // on ordinary frames at all.
    march(&mut m, &mut g, &a);
    assert_eq!(
        g.kingdom.campaign.units.get(besieger).map(|u| u.besieging_county),
        Some(2),
        "the march laid the siege",
    );

    press(&mut m, &mut g, &a, 'e');
    run_until(&mut m, &mut g, &a, "the assault prompt", |m, _| {
        m.top_id() == Some(ScreenId::BattlePrompt)
    });

    click(
        &mut m,
        &mut g,
        &a,
        on(l2_game::screens::battle::widget_rect(l2_game::screens::battle::DECLINE)),
    );
    assert_eq!(m.top_id(), Some(ScreenId::BattleResult), "then the result");
    click(&mut m, &mut g, &a, on(l2_game::screens::battle::ok_rect()));
    run_until(&mut m, &mut g, &a, "the rest of the turn", |m, _| {
        m.top_id() == Some(ScreenId::Campaign)
    });

    assert!(
        g.kingdom.campaign.units.get(garrison).is_none(),
        "the garrison was destroyed by the assault",
    );
    assert_eq!(g.kingdom.counties[2].garrison_unit, 0, "and the county's link with it is gone");
    assert!(
        g.kingdom
            .campaign
            .units
            .get(besieger)
            .is_none_or(|u| u.besieging_county == 0),
        "the siege is over either way",
    );
    assert!(
        l2_kingdom::conquest::can_be_entered(
            &g.kingdom.counties,
            &g.kingdom.campaign.units,
            2,
            1
        ),
        "and the door the siege existed to open is open",
    );
}


/// The three appearances are `Castle_StampTile`'s three arms, and driving all
/// three is deliberate — a frame table exercised at one input is C26's shape.
#[test]
fn a_castle_is_stamped_onto_the_map_and_changes_picture_as_it_goes_up() {
    use l2_kingdom::map::{castle_stamp, CASTLE_BANK_BYTE};

    let built = castle_stamp(3, 0, 100).expect("a keep");
    assert_eq!(built.bank, CASTLE_BANK_BYTE);
    assert_eq!(built.terrain, 0x17, "a Norman keep's content byte");
    assert_eq!(built.frames, [0x58, 0x5A, 0x59, 0x5B], "0x50 + 2*4, plus [0, 2, 1, 3]");

    let early = castle_stamp(3, 1, 49).expect("a keep");
    assert_eq!(early.frames[0], 0x30);
    let half = castle_stamp(3, 1, 50).expect("a keep");
    assert_eq!(half.frames[0], 0x44);
    assert_eq!(half.terrain, early.terrain, "the content byte does not move with the work");

    assert_ne!(early.frames, half.frames);
    assert_ne!(half.frames, built.frames);
    assert_eq!(castle_stamp(3, 2, 10).unwrap().frames, early.frames);

    assert!(castle_stamp(0, 0, 0).is_none(), "a bare plot draws what the file holds");

    let (mut g, a, mut m) = on_the_map();
    let here = visible_in(&g, 1);
    plot(&mut g, 1, here);
    let bare = {
        let ctx = Ctx { game: &mut g, assets: &a };
        map::MapScreen::town_graphics(&ctx).get(here.0 as usize, here.1 as usize)
    };
    assert_eq!(bare, None, "an empty plot overrides nothing");

    click(&mut m, &mut g, &a, on(castle_button()));
    click(&mut m, &mut g, &a, on(castle::type_rect(2)));
    press_and_wait(&mut m, &mut g, &a, on(castle::OK));
    let scaffold = {
        let ctx = Ctx { game: &mut g, assets: &a };
        map::MapScreen::town_graphics(&ctx).get(here.0 as usize, here.1 as usize)
    };
    assert_eq!(
        scaffold,
        Some((CASTLE_BANK_BYTE, 0x30)),
        "the moment it is ordered there is scaffolding on the map",
    );
    let quads: Vec<Option<(u8, u8)>> = {
        let ctx = Ctx { game: &mut g, assets: &a };
        let o = map::MapScreen::town_graphics(&ctx);
        (0..2)
            .flat_map(|dy| (0..2).map(move |dx| (dx, dy)))
            .map(|(dx, dy)| o.get(here.0 as usize + dx, here.1 as usize + dy))
            .collect()
    };
    assert_eq!(
        quads,
        vec![
            Some((CASTLE_BANK_BYTE, 0x30)),
            Some((CASTLE_BANK_BYTE, 0x32)),
            Some((CASTLE_BANK_BYTE, 0x31)),
            Some((CASTLE_BANK_BYTE, 0x33)),
        ],
        "the 2x2 block, with Map_StampBlock's [0, 2, 1, 3] quadrant offsets",
    );
}


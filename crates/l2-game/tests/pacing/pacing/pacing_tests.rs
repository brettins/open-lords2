#![allow(unused_imports)]
use super::*;

use super::*;
use l2_game::audio::{self, Audio};
use l2_game::game::Assets;
use l2_game::input::{Event, Key};
use l2_game::scenario;
use l2_game::screen::{Ctx, Machine, ScreenId};
use l2_kingdom::tables::Tables;
use l2_kingdom::UnitKind;

#[test]
fn a_merchant_takes_at_least_eight_frames_to_cross_one_tile() {
    let mut game = england!();
    let mut machine = Machine::new(ScreenId::Campaign);
    let slots = merchant_slots(&game);
    let trail = watch_a_turn(&mut game, &mut machine);

    let mut measured = 0;
    for (i, &slot) in slots.iter().enumerate() {
        let on = entries(&trail[i]);
        for pair in on.windows(2) {
            let gap = pair[1] - pair[0];
            assert!(
                gap >= FEWEST_TICKS_PER_TILE,
                "merchant {slot} entered a tile on frame {} and the next on frame {} — \
                 {gap} frames apart, and the fewest `Unit_StepOnce` allows is \
                 {FEWEST_TICKS_PER_TILE}",
                pair[0],
                pair[1],
            );
            measured += 1;
        }
    }
    assert!(
        measured >= 6,
        "only {measured} tile-to-tile gaps were measured across six merchants, \
         which is too few to have tested anything",
    );
}

/// **Ablated**: making `cross_sub_tile` return `true` unconditionally gives a
/// 47-frame turn containing an 11-tile leg and this goes red.
#[test]
fn a_turn_lasts_at_least_as_long_as_the_longest_march_in_it() {
    let mut game = england!();
    let mut machine = Machine::new(ScreenId::Campaign);
    let slots = merchant_slots(&game);
    let trail = watch_a_turn(&mut game, &mut machine);
    let frames = trail[0].len() as u32;

    let (longest, walker) = slots
        .iter()
        .enumerate()
        .map(|(i, &slot)| (entries(&trail[i]).len() as u32, slot))
        .max()
        .expect("six merchants");
    assert!(longest > 2, "no merchant walked far enough to time: {longest} tiles");

    let floor = FEWEST_TICKS_PER_TILE * (longest - 1);
    eprintln!("the turn took {frames} frames; merchant {walker} walked {longest} tiles");
    assert!(
        frames >= floor,
        "the turn was {frames} frames and merchant {walker} crossed {longest} tiles \
         inside it — {floor} frames is the least that can take, so the tiles were \
         being entered faster than `Unit_StepOnce` enters them",
    );
}

/// **Ablated**: returning `TurnStep::Running` from `advance` only every other
/// tick — two ticks to a frame — halves the frame count and this goes red.
#[test]
fn a_frame_of_a_turn_is_exactly_one_turn_tick() {
    let mut stepped = england!();
    let mut machine = Machine::new(ScreenId::Campaign);
    let trail = watch_a_turn(&mut stepped, &mut machine);
    let frames = trail[0].len() as u32;

    let mut at_once = england!();
    let outcome = l2_game::turn::end_turn(&mut at_once).expect("the machine comes round");

    assert_eq!(
        frames,
        outcome.ticks - 1,
        "the turn took {} ticks and {frames} frames; a frame runs one tick, and \
         the odd one out is the tick `begin_turn` runs inside the keystroke",
        outcome.ticks,
    );

    // And the load-bearing half of C59/C60 restated at this door
    // the turn machine's: spreading a turn over frames is a display change.
    for (id, u) in stepped.kingdom.campaign.units.iter() {
        let other = at_once.kingdom.campaign.units.get(id).map(|o| o.tile());
        assert_eq!(Some(u.tile()), other, "unit {id} ended the turn somewhere else");
    }
    assert_eq!(stepped.gold(), at_once.gold());
    assert_eq!(stepped.kingdom.season, at_once.kingdom.season);
}

/// `Battle_Frame` (`0x004B99C0`) ends its inner loop with
///
/// ```c
/// if ((g_battlePhase == 0) && (ticksDue != 0)) { FUN_0040490d(); Turn_Tick(); Units_Tick(); }
/// ```
///
/// and there is **no `g_screenId` test on it** — `[V]`, read whole. The message
/// scroll is not a screen in the original at all (`g_messageGroup`, painted
/// over whatever `g_screenId` is)
/// treating the campaign map as the screen while a letter is up.
#[test]
fn a_letter_open_over_the_map_does_not_stop_the_turn() {
    use l2_game::message::{category, Record};

    let mut game = england!();
    let mut machine = Machine::new(ScreenId::Campaign);
    let assets = Assets::placeholder();
    let before = game.kingdom.turn_count;

    {
        let mut ctx = Ctx { game: &mut game, assets: &assets };
        machine.handle(Event::KeyDown(Key::letter('e')), &mut ctx);
    }
    assert!(l2_game::turn::turn_in_flight(&game), "End Turn started a turn");

    for _ in 0..4 {
        let mut ctx = Ctx { game: &mut game, assets: &assets };
        machine.update(&mut ctx);
    }
    let posted = Record {
        to: game.player,
        from: 0,
        group: 0x72,
        variant: 0,
        category: category::NOTICE,
        county: 1,
        spare: 0,
        payload: 0,
    };
    assert!(game.messages.enqueue(posted, game.player), "the letter is this player's");
    {
        let mut ctx = Ctx { game: &mut game, assets: &assets };
        machine.update(&mut ctx);
    }
    assert_eq!(machine.top_id(), Some(ScreenId::Message), "the scroll is up");

    let mut with_the_letter_up = 0;
    for _ in 0..GIVE_UP {
        {
            let mut ctx = Ctx { game: &mut game, assets: &assets };
            machine.update(&mut ctx);
        }
        if machine.top_id() == Some(ScreenId::Message) {
            with_the_letter_up += 1;
        }
        if game.kingdom.turn_count > before {
            assert!(
                with_the_letter_up > 20,
                "the turn came round, but the scroll was up for only \
                 {with_the_letter_up} of its frames — it closed, so nothing was tested",
            );
            return;
        }
    }
    panic!(
        "the turn never came round in {GIVE_UP} frames with a letter open over the map; \
         the scroll was up for {with_the_letter_up} of them",
    );
}

/// **The pacing, heard.** One hoofbeat per tile per unit —
/// `Unit_MoveInFacing` (`0x00466D84`), which plays a sound as its first
/// statement after unlinking the unit from the tile it is leaving.
///
/// **Ablated**: removing the `hear_the_march` call from `Director::listen`
/// leaves `merchant.wav` unheard and this goes red.
#[test]
fn a_merchant_crossing_a_tile_is_audible() {
    let Some(dir) = l2_testkit::install_dir() else {
        l2_testkit::skip!("no game install, so no .wav files to open");
    };
    let platform = l2_mods::Platform::builder().base(&dir).build().expect("the install mounts");
    let assets = Assets::load(&platform.vfs).expect("assets load");
    let mut game = england!();
    let mut machine = Machine::new(ScreenId::Campaign);
    let mut audio = Audio::headless(&platform.vfs);
    let mut director = audio::Director::new();

    {
        let mut ctx = Ctx { game: &mut game, assets: &assets };
        machine.handle(Event::KeyDown(Key::letter('e')), &mut ctx);
    }
    let before = game.kingdom.turn_count;
    for _ in 0..GIVE_UP {
        {
            let mut ctx = Ctx { game: &mut game, assets: &assets };
            machine.update(&mut ctx);
        }
        director.listen(&mut audio, &machine, &game);
        if game.kingdom.turn_count > before {
            break;
        }
    }

    let heard = audio.heard();
    assert!(
        heard.contains(&"merchant.wav"),
        "a turn in which six merchants walked ten tiles each opened {heard:?} and \
         not merchant.wav — `Unit_MoveInFacing`'s call site is not reached",
    );
}

/// Nothing in the original takes the interpolation off a cart (C213, C184):
///
/// `Unit_StepOnce` (`0x0046634D`) has no kind test — `+0x14A` past the
/// road-keyed divider (0 on road, 3 off), then `+0x149 += 2` and the tile is
/// entered at 16 — and `Map_DrawArmies` (`0x00408438`) reads its six 8 x 16
/// offset tables (`l2_view::campaign::walk_offset`) **before** its first kind
/// comparison. So: 8 ticks a road tile, 32 an open one, and a non-zero walk
/// offset on the ticks in between. The numbers are typed; see the module note.
#[test]
fn a_merchant_crosses_sub_tiles_at_the_road_keyed_rate_and_is_drawn_between_tiles() {
    use l2_kingdom::map::{flags, CampaignMap, MAP_TILES};
    use l2_kingdom::unit::Unit;
    use l2_kingdom::{movement, Kingdom};
    use l2_view::campaign;

    const TICKS_PER_OPEN_TILE: u32 = 32;

    let mut k = Kingdom::new(0x2E5);
    assert!(k.set_county_count(1));
    let mut map = CampaignMap::empty();
    for i in 0..MAP_TILES {
        map.county[i] = 1;
    }
    for x in 0..64u8 {
        map.set_flags(x, 10, flags::ROAD);
    }
    k.campaign.map = map;
    k.counties[1].population = 500;
    k.counties[1].happiness = 70;

    let mut carts = Vec::new();
    for (y, dest) in [(10u8, (16u8, 10u8)), (20, (13, 20))] {
        let mut cart = Unit::new(UnitKind::Merchant, 6, 10, y);
        cart.county = 1;
        let id = k.campaign.units.spawn(cart).expect("a slot");
        movement::order_move(&k.campaign.map, &mut k.campaign.units, id, dest, movement::Routing::Direct)
            .expect("a route");
        carts.push(id);
    }

    let mut trail: Vec<Vec<((u8, u8), u8, u8)>> = vec![Vec::new(); carts.len()];
    for _ in 0..600 {
        k.tick_units();
        for (i, &id) in carts.iter().enumerate() {
            let u = k.campaign.units.get(id).expect("a merchant is never lost");
            trail[i].push((u.tile(), u.sub_tile, u.facing));
        }
    }

    let zoom = campaign::NEAR;
    for (i, (expected, fewest)) in
        [(FEWEST_TICKS_PER_TILE, 4usize), (TICKS_PER_OPEN_TILE, 2)].into_iter().enumerate()
    {
        let path: Vec<(u8, u8)> = trail[i].iter().map(|&(t, _, _)| t).collect();
        let on = entries(&path);
        assert!(
            on.len() >= fewest,
            "cart {} walked {} tiles and {fewest} are needed to time it",
            carts[i],
            on.len(),
        );
        for pair in on.windows(2) {
            assert_eq!(
                pair[1] - pair[0],
                expected,
                "cart {} entered a tile on tick {} and the next on tick {}; \
                 `Unit_StepOnce` takes {expected} ticks over this ground",
                carts[i],
                pair[0],
                pair[1],
            );
        }

        let mut between = 0;
        for tick in (on[0] as usize)..(*on.last().expect("entries") as usize) {
            let (_, sub_tile, facing) = trail[i][tick];
            assert!(sub_tile < 16, "`+0x149` never reaches 16 without entering a tile: {sub_tile}");
            if sub_tile == 0 || sub_tile == 15 {
                continue;
            }
            assert_ne!(
                campaign::walk_offset(&zoom, facing, sub_tile),
                (0, 0),
                "cart {} is {sub_tile}/16 across its tile on tick {tick} and drawn on the centre",
                carts[i],
            );
            between += 1;
        }
        assert!(between >= 20, "only {between} ticks caught mid-tile for cart {}", carts[i]);
    }
}



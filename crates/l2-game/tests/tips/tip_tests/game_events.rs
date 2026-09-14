#![allow(unused_imports)]
use super::*;
use super::screen_tips::*;
use super::input_and_layout::*;
use super::audio_and_narration::*;
use super::*;
use std::collections::BTreeMap;
use l2_game::audio::{self, Audio};
use l2_game::game::Assets;
use l2_game::input::Event;
use l2_game::message::{self, Frame};
use l2_game::screen::{Ctx, Machine, ScreenId};
use l2_game::screens::options::{self, Page, Setting};
use l2_game::tip::{self, Tips, View};
use l2_game::Game;
use l2_kingdom::units_tick::Incursion;

/// **The switch silences the tips, and every flip re-arms them.**
/// `Opt_ToggleTipScreens` is `g_optTipScreens = !g_optTipScreens;
/// FUN_00476A5D();` — the reset runs on the flip off as well as on.
///
/// Ablation: delete `ctx.game.tips.reset()` in `options::toggle` and the last
/// two assertions go red.
#[test]
fn the_tip_screens_switch_silences_them_and_every_flip_rearms_them() {
    let (mut g, a, mut m) = campaign();
    g.prefs.tip_screens = false;
    assert_eq!(ticks_until_posted(&mut m, &mut g, &a, 300), None, "Tip screens: No");
    assert_eq!(g.tips.delay(), 20, "and Tip_Update does not even count down");

    let flip = |g: &mut Game| {
        let mut ctx = Ctx { game: g, assets: &a };
        options::toggle(Setting::TipScreens, &mut ctx);
    };
    flip(&mut g);
    assert!(g.prefs.tip_screens);
    assert_eq!(ticks_until_posted(&mut m, &mut g, &a, 100), Some(21));
    assert!(g.tips.shown(200));

    send(&mut m, &mut g, &a, right_click());
    for _ in 0..5 {
        tick(&mut m, &mut g, &a);
    }
    assert_eq!(g.tips.delay(), 15);
    flip(&mut g);
    assert!(!g.prefs.tip_screens);
    assert!(!g.tips.shown(200), "the flip OFF clears g_tipShown too");
    assert_eq!(g.tips.delay(), 20);
}

/// **The invasion flag**: set for the local player's army only, answered by the
/// ladder's last arm on a screen no other arm names — and cleared before
/// `Tip_Show` is asked, so on screen `0x27` the tip is lost.
/// `docs/bugs.md` B101.
#[test]
fn the_local_players_incursion_raises_the_invasion_tip_and_screen_0x27_swallows_it() {
    let mut t = armed();
    t.note_incursions(&[Incursion { unit: 3, owner: 2, county: 4 }], 1);
    assert!(!t.invaded(), "another realm's army is not g_localPlayer's");
    t.note_incursions(&[Incursion { unit: 3, owner: 1, county: 4 }], 1);
    assert!(t.invaded());
    assert_eq!(tip::update(&mut t, &view(0x04)), Some(211), "\"Invasions:\"");
    assert!(!t.invaded(), "consumed by the arm");

    let mut g = world();
    g.tips = armed();
    assert!(tip::show(&mut g, 200), "a tip is up: g_screenId is 0x27");
    g.tips.note_incursions(&[Incursion { unit: 1, owner: 1, county: 2 }], 1);
    assert_eq!(tip::tick(&mut g, &view(0x27)), None, "Tip_Show refuses on 0x27");
    assert!(!g.tips.invaded(), "and the flag was cleared before it asked");
    assert!(!g.tips.shown(211), "so the tip waits for the next crossing");
}

/// **The flag reaches the tips from a real march.** The simulation reports the
/// crossing; `turn::tick_units_only` is the door the frame loop uses.
///
/// Ablation: delete `game.tips.note_incursions` in `tick_units_only`.
#[test]
fn an_army_marching_out_of_its_own_county_sets_the_flag() {
    use l2_kingdom::map::{flags, CampaignMap, MAP_DIM, MAP_TILES};
    let mut g = world();
    let mut map = CampaignMap::empty();
    for i in 0..MAP_TILES {
        map.county[i] = if i % MAP_DIM < 32 { 1 } else { 2 };
    }
    for x in 0..64u8 {
        map.set_flags(x, 10, flags::ROAD);
    }
    g.kingdom.campaign.map = map;
    g.kingdom.counties[1].owner = 1;
    g.kingdom.realms[1].in_play = true;
    g.kingdom.realms[1].is_human = true;

    let mut u = l2_kingdom::unit::Unit::new(l2_kingdom::UnitKind::Army, 1, 30, 10);
    u.men = 100;
    u.troops[0] = 100;
    u.county = 1;
    u.owner_is_human = true;
    let id = g.kingdom.campaign.units.spawn(u).expect("a free slot");
    l2_kingdom::movement::order_move(
        &g.kingdom.campaign.map,
        &mut g.kingdom.campaign.units,
        id,
        (34, 10),
        l2_kingdom::movement::Routing::Direct,
    )
    .expect("a road the whole way");

    for _ in 0..200 {
        if !g.kingdom.units_moving(l2_kingdom::UnitKind::Army) {
            break;
        }
        l2_game::turn::tick_units_only(&mut g);
    }
    assert_eq!(g.kingdom.campaign.units.get(id).map(|u| u.county), Some(2), "it crossed");
    assert!(g.tips.invaded(), "DAT_00553210: the local player's army entered a county it does not own");
}

// ------------------------------------------------------ with the player's game


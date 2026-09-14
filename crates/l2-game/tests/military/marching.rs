#![allow(unused_imports)]
use super::*;
use super::battle_part::*;
use super::raising::*;
use super::division::*;
use l2_game::game::Assets;
use l2_game::input::{Event, Key};
use l2_game::screen::{Ctx, Machine, ScreenId};
use l2_game::battlefield as bf;
use l2_game::screens::{armoury, army, battle, divide, info, map};
use l2_game::Game;
use l2_kingdom::map::{flags, CampaignMap, MAP_DIM, MAP_TILES};
use l2_kingdom::unit::{TroopType, Unit, UnitKind};
use l2_kingdom::MercenaryBands;
use l2_view::campaign;

// ---------------------------------------------------------------------------
// 2. March
// ---------------------------------------------------------------------------

/// The two clicks: one selects, one orders. Both land on the map,
/// is never left.
#[test]
fn two_clicks_on_the_map_select_an_army_and_send_it_marching() {
    let (mut g, a, mut m) = on_the_map();
    let (here, there) = adjacent_pair(|x| x < 30);
    let id = army_at(&mut g, 1, 1, 300, here);

    click(&mut m, &mut g, &a, pixel(here.0, here.1).unwrap());
    assert_eq!(m.top_id(), Some(ScreenId::Campaign), "selecting an army does not leave the map");
    assert!(g.kingdom.campaign.units.get(id).is_some_and(|u| !u.moving), "not ordered yet");

    click(&mut m, &mut g, &a, pixel(there.0, there.1).unwrap());
    let unit = g.kingdom.campaign.units.get(id).expect("still there");
    assert!(unit.moving, "Unit_OrderMove writes moveState = 2");
    assert_eq!(unit.dest, Some(there));
    assert!(!unit.path.is_empty(), "and a path to walk");
    assert_eq!(m.top_id(), Some(ScreenId::Campaign), "and the map is still what is on screen");
}

/// **A second click on the selected army ends the selection and orders
/// nothing — and it is a refused destination.**
///
/// The distinction is the whole of what was wrong here. We had an explicit
/// "clicking the same army again deselects it" branch in `click_unit`, which
/// was ours: while move-order mode is up, `g_screenId` is `0x10` and
/// `Map_Click` is not reachable at all, so the original never sees a second
/// click on an army *as* a click on an army. It sees a destination, and
/// `Map_HoverUnitTarget` has already cleared `g_moveOrderAvailable` for that
/// tile because the flood fill's raw distance there is 1 — the army is
/// standing on it. `Map_ConfirmMoveOrder` returns without writing anything,
/// and `Screen_FrameInput` had already put the screen back to `0`.
///
/// Same outcome, different mechanism,
/// **every** tile the fill did not reach behaves this way.
#[test]
fn clicking_the_selected_army_again_ends_the_selection_and_orders_nothing() {
    let (mut g, a, mut m) = on_the_map();
    let (here, there) = adjacent_pair(|x| x < 30);
    let id = army_at(&mut g, 1, 1, 300, here);
    click(&mut m, &mut g, &a, pixel(here.0, here.1).unwrap());
    click(&mut m, &mut g, &a, pixel(here.0, here.1).unwrap());
    assert!(
        g.kingdom.campaign.units.get(id).is_some_and(|u| !u.moving),
        "a destination the fill never reached is not an order",
    );
    // And the selection is gone,
    //
    click(&mut m, &mut g, &a, pixel(there.0, there.1).unwrap());
    assert!(
        g.kingdom.campaign.units.get(id).is_some_and(|u| !u.moving),
        "the deselected army took no order",
    );
}

/// **The right button is how an army is deselected, and it was missing.**
///
/// A player, one minute after reporting that the march preview only appears
/// after the click: *"you cannot deselect an army."* Both were the same gap.
/// `Screen_FrameInput`'s `0x10` arm has exactly four clauses, and this is one
/// of them: `if (g_mouseRightReleased != 0) { g_screenId = 0; g_redrawRequest
/// = 2; }`. We had the right button bound to screen `0`'s arm — the
/// information panel — with no test for the mode, so it opened a panel where
/// the original cancels.
#[test]
fn the_right_button_deselects_an_army_and_does_not_open_the_information_panel() {
    let (mut g, a, mut m) = on_the_map();
    let (here, there) = adjacent_pair(|x| x < 30);
    let id = army_at(&mut g, 1, 1, 300, here);

    let (hx, hy) = pixel(here.0, here.1).unwrap();
    click(&mut m, &mut g, &a, (hx, hy));

    send(&mut m, &mut g, &a, Event::RightClick { x: hx, y: hy });
    assert_eq!(
        m.top_id(),
        Some(ScreenId::Campaign),
        "the information panel did not open over the selection",
    );

    // The selection is gone,
    // on open ground is no longer a destination: it is a click on plain
    // ground, which does nothing at all.
    click(&mut m, &mut g, &a, pixel(there.0, there.1).unwrap());
    assert!(
        g.kingdom.campaign.units.get(id).is_some_and(|u| !u.moving),
        "a deselected army takes no orders",
    );

    // And with nothing selected the same gesture reaches screen 0x04, which
// is what makes the arm above a *mode* test.
    send(&mut m, &mut g, &a, Event::RightClick { x: hx, y: hy });
    assert!(
        matches!(m.top_id(), Some(ScreenId::Info(_))),
        "with nothing picked the right button still opens the information panel",
    );
}

/// **An unreachable destination is an accepted order with nothing in it**, and
/// that distinction is `docs/armies.md` §2.3's own correction.
///
/// `Move_ExtractPath` returns *success* with a zero-length path when the greedy
/// descent never reached the destination, so `Unit_OrderMove` writes the
/// destination, sets `moveState = 2` and copies an empty path. The army is
/// ordered and stands still. Only a dead end in the descent returns 0 and
/// leaves the record untouched.
///
/// A reimplementation that treated "no path" as a refusal would leave
/// `moveState` at 0,
/// lockstep difference, not a cosmetic one.
///
/// **But a human click on the map cannot reach that state, and this test used
/// to say it could.** `Map_ConfirmMoveOrder` opens with `if
/// (g_moveOrderAvailable != 1) return;`, and `Map_HoverUnitTarget` clears that
/// flag whenever the flood fill's raw distance at the hovered tile is below 2
/// — which is every tile the fill never reached. So the empty-path order is
/// real and is what the AI, the network command and the phase machine produce;
/// **from the map it is unreachable, because a gate stands in front of it that
/// we had not implemented.** The order is asserted where it happens,
/// in `l2-kingdom`,
#[test]
fn an_unreachable_destination_is_ordered_with_an_empty_path_and_the_army_stands() {
    let (mut g, a, mut m) = on_the_map();
    let (here, there) = adjacent_pair(|x| x > 26 && x < 30);
    let id = army_at(&mut g, 1, 1, 300, here);
    // Wall the army in on all eight sides.
    for dy in -1i32..=1 {
        for dx in -1i32..=1 {
            if dx == 0 && dy == 0 {
                continue;
            }
            g.kingdom.campaign.map.set_flags(
                (here.0 as i32 + dx) as u8,
                (here.1 as i32 + dy) as u8,
                flags::IMPASSABLE,
            );
        }
    }
    let at = (
        g.kingdom.campaign.units.get(id).unwrap().x,
        g.kingdom.campaign.units.get(id).unwrap().y,
    );

    // The screen refuses first: two clicks place no order at all, because the
    // hover gate never lit.
    click(&mut m, &mut g, &a, pixel(here.0, here.1).unwrap());
    click(&mut m, &mut g, &a, pixel(there.0, there.1).unwrap());
    assert!(
        g.kingdom.campaign.units.get(id).is_some_and(|u| !u.moving),
        "g_moveOrderAvailable was never set, so Map_ConfirmMoveOrder returned",
    );

    // `Unit_OrderMove` itself,
    // call, accepts it — and that is the part that must not drift.
    let steps = l2_kingdom::movement::order_move(
        &g.kingdom.campaign.map,
        &mut g.kingdom.campaign.units,
        id,
        there,
        l2_kingdom::movement::Routing::Direct,
    );
    assert_eq!(steps, Some(0), "success, with nothing in the buffer");
    let unit = g.kingdom.campaign.units.get(id).expect("still there");
    assert!(unit.moving, "the order was accepted");
    assert_eq!(unit.dest, Some(there), "and it names the tile that was asked for");
    assert!(unit.path.is_empty(), "with no path to walk");

    // And a whole turn of ticking moves it nowhere.
    end_turn(&mut m, &mut g, &a);
    let unit = g.kingdom.campaign.units.get(id).expect("still there");
    assert_eq!((unit.x, unit.y), at, "the army stood exactly where it was");
    assert_eq!(unit.moves_used, 0, "and spent nothing standing there");
}

/// **Clicking your own besieging army opens the siege screen
/// orders** — `Map_Click`'s own branch,
/// `0x1D` from the map.
#[test]
fn clicking_a_besieging_army_opens_the_siege_screen() {
    let (mut g, a, mut m) = on_the_map();
    let (camp, _) = adjacent_pair(|x| x < 30);
    g.kingdom.counties[2].castle_type = 2;
    let garrison = army_at(&mut g, 2, 2, 120, (camp.0, camp.1 + 2));
    g.kingdom.campaign.units.get_mut(garrison).unwrap().garrison_county = 2;
    g.kingdom.counties[2].garrison_unit = garrison;

    let id = army_at(&mut g, 1, 1, 400, camp);
    g.kingdom.campaign.units.get_mut(id).unwrap().besieging_county = 2;
    g.kingdom.campaign.units.get_mut(garrison).unwrap().besieged_by = id as u8;

    click(&mut m, &mut g, &a, pixel(camp.0, camp.1).unwrap());
    assert_eq!(
        m.top_id(),
        Some(ScreenId::Siege(id)),
        "a besieging army is a siege, not a march order",
    );
}

/// **`Siege_ValidateLink` runs before the branch is chosen**,
/// whose target garrison has gone gets its link cleared *by the click* and
/// lands on the move branch in the same call.
#[test]
fn a_besieger_whose_garrison_has_gone_takes_orders_instead_of_opening_the_siege() {
    let (mut g, a, mut m) = on_the_map();
    let (camp, there) = adjacent_pair(|x| x < 30);
    let id = army_at(&mut g, 1, 1, 400, camp);
    // Besieging a county that has no garrison at all any more.
    g.kingdom.campaign.units.get_mut(id).unwrap().besieging_county = 2;
    g.kingdom.counties[2].garrison_unit = 0;

    click(&mut m, &mut g, &a, pixel(camp.0, camp.1).unwrap());
    assert_eq!(m.top_id(), Some(ScreenId::Campaign), "no siege screen: the link was stale");
    assert_eq!(
        g.kingdom.campaign.units.get(id).unwrap().besieging_county,
        0,
        "and the click is what cleared it",
    );
    click(&mut m, &mut g, &a, pixel(there.0, there.1).unwrap());
    assert!(g.kingdom.campaign.units.get(id).is_some_and(|u| u.moving), "orders taken");
}

/// **The sortie**: a besieged garrison told to leave fights the besieger.
///
/// `Army_LeaveCastle`'s body `FUN_00437535` (`0x004374C4`) ends
/// `if (unit.besiegedBy && Battle_BeginFromCampaign(unit, unit.besiegedBy))
/// g_battleCounty = county;` — the marching garrison is `g_battleArmyA`, the
/// besieger `g_battleArmyB`,
/// occupant's. It used to march out onto its besieger and fight nothing.
#[test]
fn a_garrison_that_marches_out_onto_its_besieger_raises_the_battle_prompt() {
    let (mut g, a, mut m) = on_the_map();
    let (keep, camp) = adjacent_pair(|x| x < 30);
    g.kingdom.counties[1].castle_type = 2;

    let garrison = army_at(&mut g, 1, 1, 200, keep);
    g.kingdom.campaign.units.get_mut(garrison).unwrap().garrison_county = 1;
    g.kingdom.counties[1].garrison_unit = garrison;

    let besieger = army_at(&mut g, 2, 2, 300, camp);
    g.kingdom.campaign.units.get_mut(besieger).unwrap().besieging_county = 1;
    g.kingdom.campaign.units.get_mut(garrison).unwrap().besieged_by = besieger as u8;

    let out = g.leave_castle(garrison);
    assert!(
        matches!(out, l2_kingdom::conquest::LeftCastle::Marched { sortie: Some(b), .. }
            if b == besieger),
        "she marched, carrying the besieger: {out:?}",
    );

    let q = l2_game::turn::pending_question(&g).expect("the sortie is on the table");
    assert_eq!((q.attacker, q.defender), (garrison, besieger), "the marcher attacks");
    assert_eq!(q.county, 1, "g_battleCounty is the county left");
    assert!(!q.is_siege, "a field battle: Battle_BeginFromCampaign clears g_battleIsSiege");

    run_until(&mut m, &mut g, &a, "the sortie prompt", |m, _| {
        m.top_id() == Some(ScreenId::BattlePrompt)
    });
    assert!(
        g.kingdom.campaign.units.get(garrison).is_some()
            && g.kingdom.campaign.units.get(besieger).is_some(),
        "and nothing is resolved while the question stands",
    );
}

/// **The halt that asks.** A march whose route runs onto your own army stops
/// on `Entry::Occupied` — the original's tile stacks and ours does not — and
/// the map puts up `Ui_OpenConfirm(5, …)`, `L2.eng` group 10 index 5
/// *"Combine armies?"*, the question `Map_ConfirmMoveOrder` (`0x004A9252`)
/// raises off `g_hoverMergeUnit`. Yes is `Army_Combine` (`0x004AA181`) **into
/// the standing army**, the direction `MoveOrder_ConfirmCombine`
/// (`0x004A975D`) fixes with `Unit_OrderMove`'s fifth argument.
#[test]
fn a_march_onto_your_own_army_asks_to_combine_and_yes_merges_them() {
    let (mut g, a, mut m) = on_the_map();
    let (here, there) = adjacent_pair(|x| x < 30);
    let mover = army_at(&mut g, 1, 1, 120, here);
    let standing = army_at(&mut g, 1, 1, 200, there);
    assert!(g.order_unit_move(mover, there).is_some_and(|n| n > 0), "the march is ordered");

    run_until(&mut m, &mut g, &a, "the combine question", |_, g| g.combine_ask.is_some());
    assert_eq!(g.combine_ask, Some((mover, standing)), "the halted pair, mover first");

    press_and_wait(&mut m, &mut g, &a, on(l2_game::screens::battlefield::CONFIRM_YES));
    assert!(g.kingdom.campaign.units.get(mover).is_none(), "the mover's slot is gone");
    assert_eq!(
        g.kingdom.campaign.units.get(standing).map(|u| u.men),
        Some(320),
        "and the men are in the army that was standing there",
    );
}

/// **No is the halt that was there before the question** — both armies stand,
/// and nothing is asked again.
#[test]
fn a_march_onto_your_own_army_answered_no_leaves_both_armies_standing() {
    let (mut g, a, mut m) = on_the_map();
    let (here, there) = adjacent_pair(|x| x < 30);
    let mover = army_at(&mut g, 1, 1, 120, here);
    let standing = army_at(&mut g, 1, 1, 200, there);
    g.order_unit_move(mover, there).expect("the march is ordered");

    run_until(&mut m, &mut g, &a, "the combine question", |_, g| g.combine_ask.is_some());

    press_and_wait(&mut m, &mut g, &a, on(l2_game::screens::battlefield::CONFIRM_NO));
    assert!(g.combine_ask.is_none(), "the question is answered and does not come back");
    assert_eq!(
        g.kingdom.campaign.units.get(mover).map(|u| (u.x, u.y)),
        Some(here),
        "the mover stands where Entry::Occupied stopped it",
    );
    assert_eq!(g.kingdom.campaign.units.get(standing).map(|u| u.men), Some(200), "and so does she");
}

/// **The end of the verb: march onto an enemy county's town and take it.**
///
/// Everything is ordered from the map and nothing else is touched. Ending the
/// turn walks the army, `Unit_TryEnterTile` returns code 5 on the `0x40` tile
/// and `Army_AttackCounty` changes the owner. `docs/armies.md` §2.2: *"stepping
/// onto a county's town is how a county is taken."*
#[test]
fn an_army_ordered_from_the_map_takes_an_undefended_county_when_the_turn_ends() {
    let (mut g, a, mut m) = on_the_map();
    // The border: county 1's last column and county 2's first, both in view.
    let (here, town) = adjacent_pair(|x| x as usize == BORDER_1_2 - 1);
    assert_eq!(g.kingdom.campaign.map.county_at(here.0, here.1), 1);
    assert_eq!(g.kingdom.campaign.map.county_at(town.0, town.1), 2);
    g.kingdom.campaign.map.set_flags(town.0, town.1, flags::CASTLE);
    g.kingdom.counties[2].population = 10; // below the defence floor
    let id = army_at(&mut g, 1, 1, 600, here);

    click(&mut m, &mut g, &a, pixel(here.0, here.1).unwrap());
    click(&mut m, &mut g, &a, pixel(town.0, town.1).unwrap());
    assert!(
        g.kingdom.campaign.units.get(id).is_some_and(|u| u.moving),
        "the order was placed from the map",
    );
    // **Taking it writes to the player** — `County_ChangeOwner`'s capture
    // letter, category `0x0D`, `tests/arrival.rs`'s subject. The turn is wound
    // by the map's `update`, which does not run under the scroll, so the turn is
    // ended the way a player ends it: closing each letter as it comes.
    let before = g.kingdom.turn_count;
    press(&mut m, &mut g, &a, 'e');
    let mut letters = Vec::new();
    for _ in 0..2_000 {
        if g.kingdom.turn_count > before {
            break;
        }
        if m.top_id() == Some(ScreenId::Message) {
            letters.extend(g.messages.open().copied());
            send(&mut m, &mut g, &a, Event::RightClick { x: 320, y: 240 });
        }
        tick(&mut m, &mut g, &a);
    }
    assert!(g.kingdom.turn_count > before, "the turn happened");
    for _ in 0..=l2_view::fade::PHASES {
        tick(&mut m, &mut g, &a);
    }
    assert!(
        letters.iter().any(|r| r.category == l2_game::message::category::CAPTURE && r.county == 2),
        "the player was told he took county 2: {letters:?}",
    );
    assert_eq!(
        g.kingdom.counties[2].owner, 1,
        "the county changed hands without the player leaving the map",
    );
    assert_eq!(g.owned_by(1), 2, "and the realm's holding grew");
    assert_eq!(
        m.top_id(),
        Some(ScreenId::Campaign),
        "the map is still what is on screen: raise, march, take, all from here",
    );
}

// ---------------------------------------------------------------------------
// A turn takes time, and so does a march
// ---------------------------------------------------------------------------

/// **An army the player orders walks away while he is watching it.**
///
/// The defect a player reported as *"can't seem to move my army"* had two
/// halves and this is the structural one. `Units_Tick` has one call site in the
/// original and it is the **frame loop**, not a phase (`docs/decisions.md`
/// C35), so it runs whenever the game is up — including all the time the human
/// is looking at the map. Ours only ran it from inside a turn, so an order was
/// accepted,
/// not move until the turn was ended. From the player's chair that is
/// indistinguishable from the order having been ignored.
///
/// So: order a march,
/// to be somewhere else — with no turn ended.
#[test]
fn an_army_ordered_from_the_map_walks_while_the_player_watches() {
    let (mut g, a, mut m) = on_the_map();
    let (here, there) = adjacent_pair(|x| x < 30);
    let id = army_at(&mut g, 1, 1, 300, here);

    click(&mut m, &mut g, &a, pixel(here.0, here.1).unwrap());
    click(&mut m, &mut g, &a, pixel(there.0, there.1).unwrap());
    assert!(g.kingdom.campaign.units.get(id).is_some_and(|u| u.moving), "ordered");
    assert_eq!(g.kingdom.campaign.units.get(id).map(|u| u.tile()), Some(here));

    let before = g.kingdom.turn_count;
    for _ in 0..8 {
        tick(&mut m, &mut g, &a);
    }
    assert_eq!(g.kingdom.turn_count, before, "no turn was ended");
    assert_eq!(
        g.kingdom.campaign.units.get(id).map(|u| u.tile()),
        Some(there),
        "the army walked to the tile it was sent to, without the turn being ended",
    );
}

/// **A unit's walk is bounded by its own allowance, not by how long the player
/// sits there.**
///
/// The other side of the change above: `Units_Tick` running every frame must
/// not hand out free movement. `moveAllowance - movesUsed` is the budget and
/// `Pass::UnitsResetMoves` at the end of the season refills it,
/// leaves the map open for a thousand frames gets exactly the same march as one
/// who ends the turn immediately.
#[test]
fn watching_the_map_does_not_give_an_army_extra_movement() {
    let (mut g, a, mut m) = on_the_map();
    let (here, _) = adjacent_pair(|x| x < 30);
    let id = army_at(&mut g, 1, 1, 300, here);
    // Far enough that the allowance runs out first, and still on the grid: an
    // army has fifteen points and open ground costs three a tile, so five tiles
    // is the whole season's march and this asks for six times that. The opening
    // viewport only shows tiles with a high `y`, so the room is northwards.
    let far = (here.0, here.1.saturating_sub(30));
    assert!(here.1 - far.1 > 20, "the destination is well out of one season's reach");
    g.order_unit_move(id, far).expect("a path across open ground");
    let allowance = g.kingdom.campaign.units.get(id).map_or(0, |u| u.move_allowance);

    for _ in 0..1_000 {
        tick(&mut m, &mut g, &a);
    }
    let u = g.kingdom.campaign.units.get(id).expect("still there");
    assert!(!u.moving, "the army ran out of moves and stopped");
    assert!(
        u.moves_used <= allowance,
        "a thousand frames spent {} of an allowance of {allowance}",
        u.moves_used,
    );
    assert_ne!(u.tile(), far, "and it did not arrive: the season's budget is the bound");
}

/// **One press never both selects an army and orders it.**
///
/// The original needs `g_moveOrderClickGuard` — forty frames of deadness — for
/// this, because `Screen_FrameInput` polls the mouse button's *level* every
/// frame, so one physical press is read as a click on every frame it is held
/// down and the press that opened move-order mode would otherwise be read again
/// as the press that confirms the destination.
///
/// **Ours needs no guard, and this is why**: `Event::Click` is edge-triggered —
/// `main.rs` synthesises exactly one per `WindowEvent::MouseInput{Pressed}` —
/// and `Map_Click`'s army branch `return`s, so the selecting click cannot fall
/// through to `Map_ConfirmMoveOrder` in the same call. The guard is a
/// consequence of a polled input model we do not have. This asserts the
/// property the guard exists to protect,
/// would mean nothing here.
#[test]
fn the_click_that_selects_an_army_never_also_orders_it() {
    let (mut g, a, mut m) = on_the_map();
    let (here, _) = adjacent_pair(|x| x < 30);
    let id = army_at(&mut g, 1, 1, 300, here);

    click(&mut m, &mut g, &a, pixel(here.0, here.1).unwrap());
    let u = g.kingdom.campaign.units.get(id).expect("still there");
    assert!(!u.moving, "the selecting click did not also place an order");
    assert_eq!(u.dest, None, "and named no destination");
    assert!(u.path.is_empty());
    assert_eq!(u.moves_used, 0, "and cost nothing");

    // Repeating the same press — which is what a held button looks like to a
    // polled reader — orders nothing either, by the original's own route: the
    // tile is the army's own, the fill's distance there is `START_DISTANCE`,
    // and `Map_ConfirmMoveOrder` returns on `g_moveOrderAvailable != 1`.
    click(&mut m, &mut g, &a, pixel(here.0, here.1).unwrap());
    let u = g.kingdom.campaign.units.get(id).expect("still there");
    assert!(!u.moving, "and neither did the second one");
}

/// **The screen goes dark, and it goes dark at the season boundary.**
///
/// A player: *"the merchants move and then it fades out then in which hides the
/// season change visuals just abruptly changing"*, and *"the screen doesn't go
/// dark"* when it did not. `FUN_004B0CB4`'s two call sites are `Turn_Tick`'s
/// phase 7 with `rawFlag = 1` right after `Season_Advance`, and `FUN_0049A3E6`
/// with `0` after the seasonal art is reloaded — so the order is: units walk,
/// season advances, fade down, art swaps in the dark, fade up.
///
/// This asserts that order through the one thing a screen exposes about it,
/// `Screen::fade`: nothing while the phases run, then a full down-and-up that
/// bottoms out exactly once.
#[test]
fn the_end_of_a_turn_fades_the_screen_down_and_back_up() {
    use l2_game::screen::Screen;
    let (mut g, a) = world();
    let mut s = map::MapScreen::new();

    assert_eq!(s.fade(), None, "no fade before the turn");
    let mut ctx = Ctx { game: &mut g, assets: &a };
    s.handle(Event::KeyDown(Key::letter('e')), &mut ctx);
    assert_eq!(s.fade(), None, "and none while the phase machine is still running");

    let mut seen: Vec<u8> = Vec::new();
    let mut season_at = None;
    let before = g.kingdom.turn_count;
    for n in 0..2_000u32 {
        let mut ctx = Ctx { game: &mut g, assets: &a };
        // Both halves of a frame, in `Machine::update`'s order: `wind_turn` is
        // `Battle_Frame`'s `Turn_Tick(); Units_Tick();` and `update` is
        // `Screen_FrameInput`'s. The fade lives in the first.
        s.wind_turn(&mut ctx);
        s.update(&mut ctx);
        if season_at.is_none() && g.kingdom.turn_count > before {
            season_at = Some(n);
        }
        match s.fade() {
            Some(p) => seen.push(p),
            None if season_at.is_some() && !seen.is_empty() => break,
            None => {}
        }
    }

    assert!(season_at.is_some_and(|n| n > 4), "the season advanced, and took frames doing it");
    assert_eq!(seen.first(), Some(&0), "the fade starts at full brightness");
    assert_eq!(
        seen.len(),
        l2_view::fade::PHASES as usize,
        "one phase per tick, all the way down and back up",
    );
    assert!(seen.windows(2).all(|w| w[1] == w[0] + 1), "and strictly in order");
    assert_eq!(
        seen.iter().filter(|&&p| l2_view::fade::is_darkest(p)).count(),
        1,
        "it bottoms out exactly once",
    );
    assert_eq!(s.fade(), None, "and the light is fully back afterwards");
}

/// **The map stops taking orders while the turn is being wound on.**
///
/// Every hotspot on the map writes to state the phase machine is in the middle
/// of reading. A click that landed mid-turn would race it, so it is refused —
/// and pointer motion is *not*, because a frozen cursor would look like a hang
///
#[test]
fn the_map_refuses_orders_while_the_turn_is_running() {
    let (mut g, a, mut m) = on_the_map();
    let (here, there) = adjacent_pair(|x| x < 30);
    let id = army_at(&mut g, 1, 1, 300, here);

    press(&mut m, &mut g, &a, 'e');
    tick(&mut m, &mut g, &a);
    assert!(l2_game::turn::turn_in_flight(&g), "a turn is in flight");

    click(&mut m, &mut g, &a, pixel(here.0, here.1).unwrap());
    click(&mut m, &mut g, &a, pixel(there.0, there.1).unwrap());
    assert_eq!(
        g.kingdom.campaign.units.get(id).and_then(|u| u.dest),
        None,
        "no order was taken from a click during the turn",
    );
    // Pointer motion still arrives, so the map can still scroll under the
    // cursor while the season winds on.
    send(&mut m, &mut g, &a, Event::Pointer { x: 4, y: 4 });
    assert_eq!(m.top_id(), Some(ScreenId::Campaign));
}

// ---------------------------------------------------------------------------
// The shell table
// ---------------------------------------------------------------------------

/// Both screens have left the shell table, which is the measure of progress the
/// table's own documentation names.
#[test]
fn the_two_screens_have_graduated_out_of_the_shell_table() {
    use l2_game::screens::shells;
    assert!(shells::find(0x17).is_none(), "the raise-army screen is implemented");
    assert!(shells::find(0x11).is_none(), "the army-division screen is implemented");
    assert!(
        !shells::SHELLS.iter().any(|s| s.name.to_lowercase().contains("mercenaries")),
        "and nothing in the table still claims there is a mercenaries screen",
    );
}

/// **Two armies that overlap are stacked by where they stand, not by their
/// array slots.**
///
/// `FUN_00405487` (`0x00405487`) walks the screen lattice and calls
/// `Map_DrawArmies` (`0x00408438`) once per *cell*; the painter's whole body is
/// a walk of that one tile's unit list. So the pass is ordered by lattice row
/// then column, and a figure on a lower row covers one behind it. Ours iterated
/// `g_units` instead, so the lower id won — see
/// [`map::units_in_paint_order`](l2_game::screens::map::units_in_paint_order).
///
/// A sprite is taller than its tile and is anchored on the tile's bottom
/// vertex, so this is visible whenever two armies are within a row or two of
/// each other, which on a 64 × 64 map is often.
///
/// **Ablation.** Drop the `sort_unstable` in `units_in_paint_order` and the
/// first assertion goes red: the fixture's slots are deliberately the reverse
/// of its rows.
#[test]
fn armies_are_painted_down_the_lattice_and_not_up_the_unit_array() {
    let (mut g, _a) = world();
    // `tile_to_cell(x, y) = (x + y + 1, (x - y + 64) >> 1)`, so the lattice row
    // is `x + y`. **The slots are the reverse of the depth on purpose**: the
    // frontmost army is spawned first, so an array walk paints it under the two
    // behind it and the map decides nothing.
    let front = army_at(&mut g, 1, 1, 100, (6, 6)); // row 13
    let middle = army_at(&mut g, 1, 1, 100, (5, 5)); // row 11
    let sharing = army_at(&mut g, 1, 1, 100, (5, 5)); // row 11, the same tile
    let back = army_at(&mut g, 1, 1, 100, (4, 4)); // row 9
    assert!(front < middle && middle < sharing && sharing < back, "slots descend with depth");

    let order = map::units_in_paint_order(&g);
    assert_eq!(
        order,
        vec![back, middle, sharing, front],
        "the pass is lattice row, then column, then the tile's own list"
    );

    // The other direction: march the hindmost army past the others and it is
    // painted last, though its slot is unchanged.
    g.kingdom.campaign.units.get_mut(back).expect("the army").y = 9; // row 14
    assert_eq!(
        map::units_in_paint_order(&g).last().copied(),
        Some(back),
        "an army that marched to the front row is painted last"
    );
}


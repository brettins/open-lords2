#![allow(unused_imports)]
use super::*;
use super::repeating_gestures::*;
use super::button_kinds::*;
use super::corner_closing::*;
use super::*;
use l2_game::game::Assets;
use l2_game::input::{Event, Rect};
use l2_game::press::{self, Kind, Press, Widget};
use l2_game::screen::{Ctx, Machine, ScreenId};
use l2_game::screens::county::{self, Panel};
use l2_game::Game;

/// `App_WndProc` (`0x004B29BE`) answers `WM_LBUTTONDBLCLK` (`0x203`) with
/// `DAT_004EADA1 |= 1` and nothing else; only `0x201` sets the down bit. So
/// `g_mouseLeftDown` is clear for as long as the second press is held, and
/// `Widget_Test`'s hold branch returns at `if (g_mouseLeftDown == 0) return
/// 0;`. `[V]`
///
/// **Ablation, run:** delete `self.release()` from the `Kind::Repeat` arm of
/// `Press::event`'s double click and the repeat count goes red.
#[test]
fn a_double_click_on_a_repeating_button_fires_once_and_does_not_hold() {
    let r = Rect::new(0, 0, 24, 24);
    for kind in [Kind::Repeat, Kind::Held] {
        let mut p = Press::new();
        assert_eq!(p.event(&[Widget::new(r, kind)], Event::DoubleClick { x: 4, y: 4 }), Some(0));
        let repeats: usize = (0..200).map(|_| p.tick().count()).sum();
        assert_eq!(repeats, 0, "{kind:?}: a double click leaves g_mouseLeftDown clear");
    }
}

/// **A double click on a prompt's thumb answers it; anywhere else it passes
/// down.** `Msg_HandleInput` (`0x0047685D`) runs `Widget_Test` on the open
/// prompt's table — kind 4, pressed or double-clicked — and its 48 × 48 corner
/// opens `if (g_mouseLeftPressed == 0) { uVar1 = 0; }`. `[V]` This screen passed
/// every double click down, prompt or not.
#[test]
fn a_double_click_on_a_prompt_thumb_answers_it() {
    let (mut g, a) = world();
    g.prefs.tip_screens = false;
    let mut m = Machine::new(ScreenId::Campaign);
    let mut rec = l2_game::message::Record::default();
    rec.group = 247;
    rec.category = l2_game::message::category::PAY_PROMPT;
    rec.to = g.player;
    assert!(g.messages.enqueue(rec, g.player));
    tick(&mut m, &mut g, &a);
    assert_eq!(m.top_id(), Some(ScreenId::Message), "the pump opened the prompt");

    let [_, no] = l2_game::message::Prompt::PayForHelp.widgets();
    let side = l2_game::message::Prompt::SIDE;
    send(&mut m, &mut g, &a, Event::DoubleClick { x: no.0 + side / 2, y: no.1 + side / 2 });
    assert_eq!(m.top_id(), Some(ScreenId::Campaign), "the double click declined and closed it");
    assert_eq!(m.clicks(), 1, "Widget_Test's press, so it clicks");
}

/// `Ration_SliderClick` (`0x0043A379`) has two doors: the arrows want
/// `g_mouseLeftPressed || g_mouseLeftDoubleClick`,
/// `g_mouseLeftDown && g_mouseInputChanged`. `App_WndProc` (`0x004B29BE`)
/// answers `WM_LBUTTONDBLCLK` with `DAT_004EADA1 |= 1` and nothing else — only
/// `WM_LBUTTONDOWN` sets the down bit — so after a double click the button is
/// not down and moving the pointer moves nothing.
///
/// **Ablation, run:** put `self.slider_held = true;` back in the
/// `Event::DoubleClick` arm of `CountyScreen::handle` and the second assertion
/// goes red — the pointer drags the split to the far end.
#[test]
fn a_double_click_on_the_ration_slider_does_not_leave_a_drag_running() {
    use l2_game::screens::county::split_track;
    let (mut g, a) = world();
    g.prefs.tip_screens = false;
    let mut m = Machine::new(ScreenId::County(1, Panel::Ration));
    let track = split_track();
    let (low, high) = (track.x + 10, track.x + 90);
    let y = track.y + track.h / 2;

    send(&mut m, &mut g, &a, Event::DoubleClick { x: low, y });
    assert_eq!(g.kingdom.counties[1].ration_split, 10, "the double click still jumps");

    send(&mut m, &mut g, &a, Event::Pointer { x: high, y });
    assert_eq!(
        g.kingdom.counties[1].ration_split, 10,
        "a double click must not leave the drag latched on",
    );

    send(&mut m, &mut g, &a, Event::Click { x: low, y });
    send(&mut m, &mut g, &a, Event::Pointer { x: high, y });
    assert_eq!(g.kingdom.counties[1].ration_split, 90, "and the press does drag");
}

/// `[V]` This screen dropped it.
///
/// **Ablation, run:** disable the supplies screen's `Event::DoubleClick` arm and
/// the click count goes red at 0 — the double click reached nothing, so nothing
/// went down and nothing will leave.
#[test]
fn a_double_click_on_the_supplies_thumb_down_leaves_twenty_ticks_later() {
    use l2_game::screens::supplies::THUMB_DOWN;
    let (mut g, a) = world();
    g.prefs.tip_screens = false;
    let mut m = Machine::new(ScreenId::Campaign);
    m.push(ScreenId::Supplies(1));
    let at = on(THUMB_DOWN);
    send(&mut m, &mut g, &a, Event::DoubleClick { x: at.0, y: at.1 });
    assert_eq!(m.clicks(), 1, "kind 5's press clicks");
    for t in 1..press::DELAYED_FRAMES as u32 {
        tick(&mut m, &mut g, &a);
        assert_eq!(m.top_id(), Some(ScreenId::Supplies(1)), "left on tick {t}");
    }
    tick(&mut m, &mut g, &a);
    assert_eq!(m.top_id(), Some(ScreenId::Campaign), "FUN_0043B04C's cancel, on the twentieth tick");
}

/// **A double click on the information panel's garrison widget turns the tile
/// half into the unit half.** `0x04`'s arm tests the corner and the brush
/// (releases), then `FUN_00438A91`'s `Widget_Test` kind 4, then the unit
/// buttons (`Hotspot_Test` kind 1); only the widget reads the double click.
///
/// `[V]` This screen dropped it.
#[test]
fn a_double_click_on_the_garrison_widget_opens_the_garrison() {
    use l2_game::screens::info::{Target, GARRISON_WIDGET};
    let (mut g, a) = world();
    g.prefs.tip_screens = false;
    let tile = l2_kingdom::map::index(20, 20);
    g.kingdom.campaign.map.county[tile] = 1;
    g.kingdom.campaign.map.terrain[tile] = l2_kingdom::map::terrain::CASTLE_PLOT + 1;
    g.kingdom.campaign.map.flags[tile] |= l2_kingdom::map::flags::SETTLEMENT;
    g.kingdom.counties[1].garrison_unit = 7;
    g.map_zoom_far = false;
    let mut m = Machine::new(ScreenId::Campaign);
    m.push(ScreenId::Info(Target::Tile(tile)));
    let at = on(GARRISON_WIDGET);
    send(&mut m, &mut g, &a, Event::DoubleClick { x: at.0, y: at.1 });
    assert_eq!(m.top_id(), Some(ScreenId::Info(Target::Unit(7))), "FUN_00438ACC ran on the double click");
}


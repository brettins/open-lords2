#![allow(unused_imports)]
use super::*;
use super::repeating_gestures::*;
use super::button_kinds::*;
use super::double_click::*;
use super::*;
use l2_game::game::Assets;
use l2_game::input::{Event, Rect};
use l2_game::press::{self, Kind, Press, Widget};
use l2_game::screen::{Ctx, Machine, ScreenId};
use l2_game::screens::county::{self, Panel};
use l2_game::Game;

/// `Screen_FrameInput`'s `0x1B` arm is `Ui_OkButtonClicked()` (`0x0040E7E4`),
/// whose first statement is `if (g_mouseLeftReleased == 0) return 0;`. The
/// marker in `castle.rs` said `left-release` and the code read `Event::Click`.
///
/// **Ablation, run:** move the `CORNER_OK.contains` test back into the
/// `Event::Click` arm and the first assertion goes red.
#[test]
fn the_castle_choosers_corner_closes_on_the_release() {
    use l2_game::screens::castle::CORNER_OK;
    let (mut g, a) = world();
    g.prefs.tip_screens = false;
    let mut m = Machine::new(ScreenId::Campaign);
    m.push(ScreenId::Castle(1));
    let opened = m.top_id();
    let at = on(CORNER_OK);
    send(&mut m, &mut g, &a, Event::Click { x: at.0, y: at.1 });
    assert_eq!(m.top_id(), opened, "the press does nothing: the corner reads g_mouseLeftReleased");
    send(&mut m, &mut g, &a, Event::Release { x: at.0, y: at.1 });
    assert_eq!(m.top_id(), Some(ScreenId::Campaign), "and the release closes it");
}

/// `Screen_FrameInput`'s `0x17` arm asks `Levy_SliderClick()` (`0x00435CEF`)
/// first — which cannot answer a release: its arrow branches want
/// `g_mouseLeftPressed || g_mouseLeftDoubleClick` and its track branch
/// `g_mouseLeftDown` — and then `Ui_OkButtonClicked()`.
///
/// **Ablation, run:** move the `OK.contains` test back into the `Event::Click`
/// arm and the first assertion goes red.
#[test]
fn the_raise_army_screens_corner_closes_on_the_release() {
    use l2_game::screens::army::OK;
    let (mut g, a) = world();
    g.prefs.tip_screens = false;
    let mut m = Machine::new(ScreenId::Campaign);
    m.push(ScreenId::RaiseArmy(1));
    let opened = m.top_id();
    let at = on(OK);
    send(&mut m, &mut g, &a, Event::Click { x: at.0, y: at.1 });
    assert_eq!(m.top_id(), opened, "the press does nothing");
    send(&mut m, &mut g, &a, Event::Release { x: at.0, y: at.1 });
    assert_eq!(m.top_id(), Some(ScreenId::Campaign), "and the release closes it");
}


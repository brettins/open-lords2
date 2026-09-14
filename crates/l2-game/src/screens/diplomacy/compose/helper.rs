#![allow(unused_imports)]
use super::*;
use super::compose::*;
use super::*;
use super::main::*;
use super::tests::*;
use l2_kingdom::diplomacy::{group, Kind};
use l2_kingdom::realm::MAX_REALMS;
use l2_view::Canvas;
use crate::input::{Event, Key, Rect};
use crate::press::{Press, Widget};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::screens::message::lord_name;
use crate::shell::{font, Pen};
use crate::widget;

/// Where the caret lands inside the wrapped draft — the pen position
/// `FUN_0040352F` hands back to `FUN_00417BD3` as `g_caretX`, `g_caretY`.
///
/// **[I] on the column.** Our wrap splits on whitespace and drops it, so a
/// caret standing on a run of spaces is drawn at the end of the word before
/// them; the original measures the spaces. Elsewhere the two agree.
fn caret_pen(pen: &Pen, text: &str, caret: usize, x: i32, y: i32, width: i32) -> (i32, i32) {
    use crate::text::Metrics;
    let m = crate::text::FontMetrics::of(pen.assets);
    let line_h = pen.assets.body.as_ref().map_or(16, |f| f.line);
    let head: String = text.chars().take(caret).collect();
    let rows = pen.wrap(&head, width);
    let row = rows.len().saturating_sub(1) as i32;
    let last: Vec<char> = rows.last().map_or_else(Vec::new, |s| s.chars().collect());
    (x + m.width(&last), y + row * line_h)
}

/// `Diplo_SendClicked` (`0x00436408`)'s validation, as a function of the state
/// it reads. Separated from the click so that the ladder can be tested at every
/// rung — `docs/decisions.md` C26 is exactly about a rule with one fixture.
pub fn refusal(ctx: &Ctx, target: u8, kind: Kind, county: u8) -> Option<Refusal> {
    let me = ctx.game.player;
    if matches!(kind, Kind::AskHelp | Kind::AskAttack) {
        if county == 0 {
            return Some(Refusal::NoCounty);
        }
        let c = ctx.game.kingdom.counties.get(county as usize)?;
        if c.owner == 0 {
            return Some(Refusal::Unowned);
        }
        if kind == Kind::AskHelp {
            if c.owner != me {
                return Some(Refusal::NotOurs);
            }
            // County `+0x19C` — the enemy-troop count the panel draws. Asking
            // for help in a county nothing is threatening is refused.
            if c.enemy_troops == 0 {
                return Some(Refusal::NoEnemy);
            }
        } else {
            if c.owner == me {
                return Some(Refusal::Allied);
            }
            if ctx.game.kingdom.realms.get(me as usize).map_or(0, |r| r.ally) == c.owner {
                return Some(Refusal::Allied);
            }
        }
    }
    if kind == Kind::OfferAlliance {
        let t = ctx.game.kingdom.realms.get(target as usize)?;
        if t.is_human && t.ally != 0 {
            return Some(Refusal::TargetAlreadyAllied);
        }
    }
    None
}

/// The county under a pixel of the compose dialog's map, or `None`.
///
/// **`None` covers two different things**:
/// outside the rectangle, and inside it on a pixel whose county is 0. Both
/// return 0 from `FUN_0043B4CB`, and 0 means *"not consumed"* — so a click on
/// the sea inside the map falls through to the corner button behind it.
pub fn county_at_picker(ctx: &Ctx, x: i32, y: i32) -> Option<u8> {
    if !PICKER.contains(x, y) {
        return None;
    }
    let minimap = ctx.assets.minimap(ctx.game.map_slot)?;
    let dim = l2_view::chrome::MINIMAP_DIM;
    let (dx, dy) = (x - PICKER.x, y - PICKER.y);
    let county = *minimap.counties.get((dy * dim + dx) as usize)?;
    (county != 0).then_some(county)
}


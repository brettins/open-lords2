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

/// `Ui_DrawText` (00400000.c:1125) takes the caret while `FUN_0040352F` draws the
/// whole buffer: the pen at the character whose index is `g_editCaret`. None
/// matched, `FUN_00417BD3` (00410000.c:2919) falls back to the last line's
/// `g_penAdvance`, +4 (`0x00402637`), and `DAT_005CD4F8`, +0x10 per line drawn
/// (00400000.c:1720): one line below the last. [V]
pub(super) fn caret_pen(pen: &Pen, text: &str, caret: usize, x: i32, y: i32, width: i32) -> (i32, i32) {
    use crate::text::Metrics;
    let m = crate::text::FontMetrics::of(pen.assets);
    let line_h = pen.assets.body.as_ref().map_or(16, |f| f.line);
    let rows = pen.wrap_buffer(text, width);
    let mut at = 0;
    for (row, line) in rows.iter().enumerate() {
        let chars: Vec<char> = line.chars().collect();
        // `if (0x1f < local_c)` guards the capture
        if caret >= at && caret < at + chars.len() && chars[caret - at] > '\u{1f}' {
            return (x + m.width(&chars[..caret - at]), y + row as i32 * line_h);
        }
        at += chars.len();
    }
    let last: Vec<char> = rows.last().map_or_else(Vec::new, |s| s.chars().collect());
    (x + m.width(&last) + 4, y + rows.len() as i32 * line_h)
}

#[cfg(test)]
mod caret_tests {
    use super::*;

    fn pen_of(a: &crate::game::Assets) -> Pen<'_> {
        Pen { assets: &a.shell, ink: &a.ink, chrome: None, shadow: None, caps: None }
    }

    /// Ablated by restoring the wrap of the text before the caret (3f5e13b8): "aaaa bb"
    /// fits one line, the caret stays on row 0 and this goes red. Verified.
    #[test]
    fn a_caret_mid_word_follows_its_word_to_the_next_line() {
        let a = crate::game::Assets::placeholder();
        let pen = pen_of(&a);
        let rows = pen.wrap_buffer("aaaa bbbb", 44);
        assert_eq!(rows, vec!["aaaa".to_string(), " bbbb".to_string()]);
        assert_eq!(caret_pen(&pen, "aaaa bbbb", 7, 0x30, 200, 44), (0x30 + 12, 216));
    }

    /// Ablated by dropping the fallback's `+ 4` and row below: the caret sits
    /// after "bbbb" on row 1 and this goes red. Verified.
    #[test]
    fn a_caret_at_the_end_falls_back_one_line_below() {
        let a = crate::game::Assets::placeholder();
        let pen = pen_of(&a);
        assert_eq!(caret_pen(&pen, "aaaa bbbb", 9, 0x30, 200, 44), (0x30 + 24, 232));
        assert_eq!(caret_pen(&pen, "", 0, 0x30, 200, 44), (0x30 + 4, 216));
    }
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


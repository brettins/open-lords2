#![allow(unused_imports)]
use super::*;
use super::main::*;
use super::rows::*;
use super::*;
use super::render_helpers::*;
use super::*;
use super::layout::*;
use super::draw::*;
use l2_kingdom::tables::{
    HEALTH_BAND_NAMES, JOB_IDLE_TOWNSFOLK, RATION_LEVEL_COUNT, RATION_NAMES,
};
use l2_view::chrome::{self, misc_cty, system};
use l2_view::{text, Canvas};
use crate::game::{MAX_RATION_SPLIT, MAX_TAX_RATE};
use crate::input::{Event, Key, Rect};
use crate::press::{Press, Widget};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::shell::{font, Pen, TRAILING};
use crate::widget;

pub(crate) fn draw_strip_icon(ctx: &Ctx, canvas: &mut Canvas, frame: usize, x: i32, y: i32, label: &str) {
    let drawn = ctx.assets.chrome.as_ref().is_some_and(|ch| ch.draw_misc(canvas, frame, x, y));
    if !drawn {
        text::draw(canvas, x, y + 8, label, ctx.assets.ink.dim);
    }
}


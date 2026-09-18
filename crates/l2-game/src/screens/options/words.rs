#![allow(unused_imports)]
use super::*;
use super::page::*;
use super::screen::*;
use l2_view::Canvas;
use crate::game::PRESENTATION;
use crate::input::{Event, Key, Rect};
use crate::press::{Kind, Press, Widget};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::shell::{self, font, Pen};

/// A row's state word comes from one of two `L2.eng` groups, and one panel uses
/// both.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Words {
    YesNo,
    OnOff,
}

impl Words {
    pub fn group(self) -> usize {
        match self {
            Words::YesNo => GROUP_YES_NO,
            Words::OnOff => GROUP_ON_OFF,
        }
    }

    pub fn index(self, on: bool) -> usize {
        usize::from(!on)
    }
}


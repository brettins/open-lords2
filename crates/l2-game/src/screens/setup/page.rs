#![allow(unused_imports)]
use super::*;
use super::helpers_part::*;
use super::constants_part::*;
use super::ui_part::*;
use super::screen::*;
use helpers::*;
use constants::*;
use ui::*;
use l2_view::Canvas;
use crate::input::{Event, Key, Rect};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::setup::SetupOptions;
use crate::shell::{self, font, Pen};
use crate::text::{self, TextField};

/// `g_setupPage` (`0x005530F0`). The thirteen values `FUN_0041E7E1` switches on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SetupPage {
    Title,
    Options,
    Load,
    Shield,
    Campaign,
    GameType,
    Custom,
    CustomMulti,
    Dropdown,
    NoCd,
    SkirmishMulti,
    Skirmish,
    SkirmishFile,
}

impl SetupPage {
    pub fn number(self) -> u8 {
        match self {
            SetupPage::Title => 1,
            SetupPage::Options => 2,
            SetupPage::Load => 3,
            SetupPage::Shield => 4,
            SetupPage::Campaign => 5,
            SetupPage::GameType => 6,
            SetupPage::Custom => 7,
            SetupPage::CustomMulti => 8,
            SetupPage::Dropdown => 9,
            SetupPage::NoCd => 10,
            SetupPage::SkirmishMulti => 11,
            SetupPage::Skirmish => 12,
            SetupPage::SkirmishFile => 13,
        }
    }

    pub const ALL: [SetupPage; 13] = [
        SetupPage::Title,
        SetupPage::Options,
        SetupPage::Load,
        SetupPage::Shield,
        SetupPage::Campaign,
        SetupPage::GameType,
        SetupPage::Custom,
        SetupPage::CustomMulti,
        SetupPage::Dropdown,
        SetupPage::NoCd,
        SetupPage::SkirmishMulti,
        SetupPage::Skirmish,
        SetupPage::SkirmishFile,
    ];

    /// Which full-screen background `FUN_0041E61D` reads for this page.
    ///
    /// **[D]** The chain is: page 10 and pages below 7 take `gateway.256` and
    /// `gateway.pl8` with `panels2.pl8` on top; pages 7, 8 and 9 take
    /// `custom.256` and `custom.pl8`; pages 11 and up take `skirmish.256` and
    /// either `skirmish.pl8` or `skircust.pl8`. Page 9 draws the page beneath
    /// it first and so inherits whichever that was.
    pub fn background(self) -> &'static str {
        match self {
            SetupPage::Custom | SetupPage::CustomMulti | SetupPage::Dropdown => "Custom.pl8",
            SetupPage::SkirmishMulti | SetupPage::Skirmish | SetupPage::SkirmishFile => {
                "Skirmish.pl8"
            }
            _ => "Gateway.pl8",
        }
    }

    pub fn palette(self) -> &'static str {
        match self {
            SetupPage::Custom | SetupPage::CustomMulti | SetupPage::Dropdown => "Custom.256",
            SetupPage::SkirmishMulti | SetupPage::Skirmish | SetupPage::SkirmishFile => {
                "Skirmish.256"
            }
            _ => "Gateway.256",
        }
    }
}


#![allow(unused_imports)]
use super::*;
use super::hits::*;
use super::items::*;
use super::launch::*;
use super::super::*;
use super::super::*;
use super::super::screen::*;
use super::super::input::*;
use super::super::*;
use super::super::helpers::*;
use super::super::constants::*;
use l2_view::Canvas;
use crate::input::{Event, Key, Rect};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::setup::SetupOptions;
use crate::shell::{self, font, Pen};
use crate::text::{self, TextField};
use crate::screens::setup::skirmish::ROWS_SHOWN;

impl SetupScreen {
    pub fn new(page: SetupPage) -> SetupScreen {
        SetupScreen {
            page,
            selected: 0,
            under: SetupPage::Custom,
            open: 0,
            options: SetupOptions::new(),
            shield: 0,
            map_top: 0,
            map_row: 0,
            map: 0,
            player_starts: 5,
            map_read: false,
            unhonoured: Vec::new(),
            failure: None,
            campaign: false,
            via_skirmish: false,
            network: false,
            autosave: false,
            track: crate::victory::Track::First,
            name: begin_name(text::DEFAULT_PLAYER_NAME),
            saved_name: text::DEFAULT_PLAYER_NAME.to_string(),
            prefs_read: false,
            load: None,
            load_opener: SetupPage::Options,
            load_list_hit: false,
            skirmish: Default::default(),
            troops: Default::default(),
            troops_file: None,
            banner: 0,
            banner_ms: 0,
            skr_text: Vec::new(),
            skirmish_files: Vec::new(),
            skirmish_file_top: 0,
            clock_minute: None,
            clock_redraw: false,
            press: crate::press::Press::new(),
        }
    }

    pub fn name(&self) -> String {
        self.name.commit(text::PLAYER_NAME_LEN)
    }

    pub fn name_field(&self) -> &text::TextField {
        &self.name
    }

    pub fn options(&self) -> &SetupOptions {
        &self.options
    }

    pub fn map(&self) -> usize {
        self.map
    }

    pub fn shield(&self) -> u8 {
        SHIELD_OF_HOTSPOT[(self.shield + 1).min(5)]
    }

    pub fn player_starts(&self) -> usize {
        self.player_starts
    }

    pub(crate) fn read_map(&mut self, ctx: &Ctx) {
        self.map_read = true;
        let Some(slot) = ctx.assets.slot(self.map) else { return };
        let seats = slot.player_start_count();
        if seats == 0 {
            return;
        }
        self.player_starts = seats;
        self.options.set_nobles_from_map(seats);
    }

    pub fn page(&self) -> SetupPage {
        self.page
    }

    /// The value of option `i`, as an index into `L2.eng` group 103.
    pub fn option_value(&self, i: usize) -> usize {
        OPTION_BASE[i] + self.options.get(i)
    }

    /// Everything but *Nobles* shows its whole run. *Nobles* is shortened to
    /// the map's seat count — `FUN_00433999`: `DAT_00553FB4 =
    /// g_playerStartCount - 1` when the map seats fewer than five — which is
    /// how the original stops a person asking for more lords than the map has
    /// castles for.
    pub(crate) fn rows(&self, i: usize) -> usize {
        if i == crate::setup::option::NOBLES {
            SetupOptions::nobles_rows_for_map(self.player_starts)
        } else {
            OPTION_COUNT[i]
        }
    }

}

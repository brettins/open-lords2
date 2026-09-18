#![allow(unused_imports)]
use super::*;
use super::helpers_part::*;
use super::constants_part::*;
use super::ui_part::*;
use super::page::*;
use helpers::*;
use constants::*;
use ui::*;
use l2_view::Canvas;
use crate::input::{Event, Key, Rect};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::setup::SetupOptions;
use crate::shell::{self, font, Pen};
use crate::text::{self, TextField};


pub struct SetupScreen {
    pub(crate) page: SetupPage,
    /// Which item the pointer or the keyboard is on — `DAT_00553114`, the
    /// global the painters compare against to pick `0xF9` over `0x3F`.
    pub(crate) selected: usize,
    /// Which page the drop-down is open over: `DAT_00553E5C`.
    pub(crate) under: SetupPage,
    /// Which of the twelve options is open: `DAT_0055306C`.
    pub(crate) open: usize,
    /// This used to be a bare `[usize; 12]` with a comment saying nothing read
    /// it. It is [`SetupOptions`] — `0x0053F288 … 0x0053F2B4`, the block
    /// `Setup_SetOption` writes — and pressing *Start* puts it through
    /// [`SetupOptions::commit`] and applies the result. `crate::setup` is what
    /// each of the twelve does.
    pub(crate) options: SetupOptions,
    /// Which of the five shields page 4 has picked. Realm `+0x0A` in the
    /// original, one-based there and zero-based here because this is an index
    /// into the five frame pairs and nothing else yet.
    pub(crate) shield: usize,
    pub(crate) map_top: usize,
    pub(crate) map: usize,
    pub(crate) player_starts: usize,
    pub(crate) map_read: bool,
    /// What the last *Start* could not honour, as `L2.eng` group 102 indices —
    /// [`crate::setup::Settings::unhonoured`]. Drawn under the grid, in our own
    /// font. `docs/decisions.md` C21.
    pub(crate) unhonoured: Vec<usize>,
    pub(crate) failure: Option<String>,
    /// A player reported *"I can't type my name in the start menu?"*, and this
    /// is the field they were looking for. Both arms that open page 4 —
    /// `FUN_00432B05`'s hotspot 2 and `FUN_00432CC8`'s hotspots 3 and 5 — run
    /// `Edit_Begin(&g_options, 0x10, 0xC0, 0)` before anything else, so the
    /// field is seeded with the name you already have, sixteen characters, one
    /// hundred and ninety-two pixels, free text. [`crate::text`] is the engine
    /// and `docs/arms.json`'s `text` group is the inventory.
    pub(crate) name: crate::text::TextField,
    /// **`DAT_0057D320` — whether *Continue* on page 4 starts a campaign.**
    ///
    /// Page 4 is reached from four places and the button at the bottom of it
    /// means something different depending on which. This is the flag the
    /// original uses to tell them apart, and it is written by every one of
    /// those arms: `Setup_ChooseCampaign` (`0x00433461`) sets it to **1**, and
    /// `FUN_00432B05` (page 1) and three arms of `FUN_00432CC8` (page 2) set it
    /// to **0**. [`SetupScreen::continue_pressed`] is what reads it.
    ///
    /// Without it, our page 4 started the map list's slot whichever way the
    /// person had arrived — so *Play Now!* built England instead of
    /// Quaintville. `docs/decisions.md` C117.
    pub(crate) campaign: bool,
    /// **A `Save_RotateAndWrite` is owed** — `Game_NewGame`'s own call to it
    /// (`0x00497E2B`), raised when *Start* has built a world. See
    /// [`crate::screen::Screen::take_autosave`].
    pub(crate) autosave: bool,
    /// `g_campaignTrack` (`DAT_0053F640`) — which of the two campaigns page 5
    /// chose. `Setup_ChooseCampaign` stores the hotspot here.
    pub(crate) track: crate::victory::Track,
    pub(crate) saved_name: String,
    pub(crate) clock_minute: Option<i64>,
    pub(crate) skirmish: skirmish::Skirmish,
    /// `g_troopsTable`, as `Troops_Load` (`0x0042AC0C`) would have filled it.
    pub(crate) troops: skirmish::TroopsTable,
    /// The `.skr` files page 13 lists — `DAT_004E8790`, 0x41 bytes a name,
    /// counted by `DAT_004EB25C`. Nothing scans a directory for them yet.
    pub(crate) skirmish_files: Vec<String>,
    /// `g_fileListTop` (`0x004EA1A0`) — the index page 13's ten rows start at.
    pub(crate) skirmish_file_top: usize,
    pub(crate) clock_redraw: bool,
    /// The 320 ms pulse of `Hotspot_Test` (`0x0040E3EE`) kind 2, which five of
    /// the records in the widget table at `0x004DCF68` are. `held_kind` says
    /// which five.
    pub(crate) press: crate::press::Press,
}


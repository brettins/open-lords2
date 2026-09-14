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

// --------------------------------------------------------------- the screen

pub struct SetupScreen {
    page: SetupPage,
    /// Which item the pointer or the keyboard is on — `DAT_00553114`, the
    /// global the painters compare against to pick `0xF9` over `0x3F`.
    selected: usize,
    /// Which page the drop-down is open over: `DAT_00553E5C`.
    under: SetupPage,
    /// Which of the twelve options is open: `DAT_0055306C`.
    open: usize,
    /// **The twelve settings, and they now reach the game.**
    ///
    /// This used to be a bare `[usize; 12]` with a comment saying nothing read
    /// it. It is [`SetupOptions`] — `0x0053F288 … 0x0053F2B4`, the block
    /// `Setup_SetOption` writes — and pressing *Start* puts it through
    /// [`SetupOptions::commit`] and applies the result. `crate::setup` is what
    /// each of the twelve does.
    options: SetupOptions,
    /// Which of the five shields page 4 has picked. Realm `+0x0A` in the
    /// original, one-based there and zero-based here because this is an index
    /// into the five frame pairs and nothing else yet.
    shield: usize,
    /// The top row of the map list, and the selected map — `g_scenarioIndex`,
    /// which *is* the map slot (`Game::map_slot`).
    ///
    /// **Zero is a real value here and it is where the list sits untouched**,
    /// which is England. A campaign does not read this: `Campaign_LoadEntry`
    /// *writes* it, from the campaign row, the same way it writes the global —
/// so [`SetupScreen::start_campaign`] sets it
    /// slot beside the world it just built.
    map_top: usize,
    map: usize,
    /// `g_playerStartCount` for [`SetupScreen::map`] — how many lords that map
    /// seats. `Map_LoadPlanes` recomputes it every time the scenario changes
    /// and three call sites then push it into the *Nobles* drop-down, so it is
/// cached beside the map.
    ///
    /// **Five until a map has been read**, which is what an install without
    /// `L2_maps.dat` leaves it at: the full drop-down, and no seat count
    /// invented from nothing.
    player_starts: usize,
    /// Whether [`SetupScreen::player_starts`] has been read for
    /// [`SetupScreen::map`] yet. The file is [`Ctx`]'s and the constructor has
    /// no `Ctx`, so the first read happens on the first tick.
    map_read: bool,
    /// What the last *Start* could not honour, as `L2.eng` group 102 indices —
    /// [`crate::setup::Settings::unhonoured`]. Drawn under the grid, in our own
    /// font. `docs/decisions.md` C21.
    unhonoured: Vec<usize>,
    /// **Why the last *Start* did not start**, if it did not.
    ///
    /// A slot that will not build a world — no `L2_maps.dat`, an empty
    /// template, more lords than seats — leaves the game untouched and says so
    /// under the grid. Silence would look exactly like a button that works.
    failure: Option<String>,
    /// **The lord's name, being typed.** `g_editBuffer` while page 4 is up.
    ///
    /// A player reported *"I can't type my name in the start menu?"*, and this
    /// is the field they were looking for. Both arms that open page 4 —
    /// `FUN_00432B05`'s hotspot 2 and `FUN_00432CC8`'s hotspots 3 and 5 — run
    /// `Edit_Begin(&g_options, 0x10, 0xC0, 0)` before anything else, so the
    /// field is seeded with the name you already have, sixteen characters, one
    /// hundred and ninety-two pixels, free text. [`crate::text`] is the engine
    /// and `docs/arms.json`'s `text` group is the inventory.
    ///
/// It lives here because our page 4 is
    /// reachable from three places and a field rebuilt on each of them would
    /// lose what was typed; [`SetupScreen::go`] does the `Edit_Begin` at
    /// exactly the moments the original does.
    name: crate::text::TextField,
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
    campaign: bool,
    /// **A `Save_RotateAndWrite` is owed** — `Game_NewGame`'s own call to it
    /// (`0x00497E2B`), raised when *Start* has built a world. See
    /// [`crate::screen::Screen::take_autosave`].
    autosave: bool,
    /// `g_campaignTrack` (`DAT_0053F640`) — which of the two campaigns page 5
    /// chose. `Setup_ChooseCampaign` stores the hotspot here.
    track: crate::victory::Track,
    /// The persisted `g_options` name — what the field is seeded *from*, and
    /// where a commit goes back to.
    ///
    /// `g_options` is one 0x468-byte block the original `fread`s and `fwrite`s
    /// whole, and byte 0 begins a 31-byte name. We have no settings file yet,
    /// so this is that byte run and nothing else, defaulted the way
    /// `Options_SetDefaults` defaults it.
    saved_name: String,
    /// **Which minute the drawn clock says** — [`crate::wallclock::minute`] of
    /// the reading the last tick saw, or `None` before the first.
    ///
    /// A still screen costs nothing here: [`Machine::update`] only repaints
    /// when [`Screen::take_redraw`] says so, and this is what makes it say so
/// **once a minute**. It is a cached
    /// *picture* fact, not a clock — the reading itself is handed in through
    /// `Assets` and is never read from the system by anything in this crate.
    clock_minute: Option<i64>,
    /// Whether the minute turned since the last paint.
    clock_redraw: bool,
}


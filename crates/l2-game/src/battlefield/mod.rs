//! `docs/screens-county.md` §1 lists four ids — `0x28`, `0x29`, `0x2A` and
//! `0x2B` — and the input audit (`docs/decisions.md` C61) counted their arms as
//! one group of 49. **`0x28` is unreachable.** Every immediate write of
//! `g_screenId` in the shipped binary was enumerated — 212 `mov byte ptr
//! [0x004EAC50], imm8` sites covering `0x00` … `0x45` — and `0x28` is not among
//! them, while `0x29`, `0x2A` and `0x2B` all are; no decompiled function assigns
//! it either, and the only indirect writes restore a value `g_screenId` already
//! held. It has a live `Screen_FrameInput` arm and a live `Screen_Draw` arm and
//! neither can run. **[V]** `docs/bugs.md` D37.
//!
//! | id | what | where its arm lives |
//! |---|---|---|
//! | `0x29` | the field | `Screen_FrameInput` `0x0042FF10`, arm `')'` |
//! | `0x2A` | **the drag**, entered by pressing on the field | arm `'*'` |
//! | `0x2B` | the outcome banner | arm `'+'` |
//!
//! `Battle_Start` (`0x004778A0`) writes `DAT_0053F238 = 0xFFFFFFFF` before it
//! raises `g_screenId = 0x29`, and battle button 0 (`FUN_0043B9A1`) toggles that
//! word with a bitwise NOT — so it flips between `-1` and `0` and the first
//! thing a player does is unpause. While it is set, `FUN_0043C57D` refuses to
//! issue an order and `Battle_Frame` paints `L2.eng` group 32 index 0 across the
//! bottom of the field. **[V]**
//!
//! | y | what | the original |
//! |---|---|---|
//! | 24 … 183 | the 80 × 80 overview at 2 px a cell | `BattleMap_Click` `0x00432443` |
//! | 185 … 404 | the banners of the figures you hold | `FUN_0043C2A9` `0x0043C2A9`, table `0x004D31F4` |
//! | 448 … 480 | five buttons | `FUN_004329A4` `0x004329A4`, table `0x004DC710` |

mod logic;
pub use logic::*;
mod view;
pub use view::*;

mod tests;
pub use tests::*;

use l2_sim::runner::{BattleRunner, Conclusion, Formation};
use l2_sim::terrain::DIM;

use crate::input::Rect;

/// `Battle_LoadAssets` (`0x004987B7`) hands the tile renderer
/// `FUN_004BC020(…, 0x50, 0x50, 8, 0, 0x18, 0xF, 0xE, 0x20)`: an 80 × 80 array
/// of 8-byte cells, origin `(0, 24)`, **15 × 14 tiles of 32 pixels**. **[V]**
pub const TILE: i32 = 32;
pub const VIEW_COLS: i32 = 15;
pub const VIEW_ROWS: i32 = 14;
pub const VIEW: Rect = Rect::new(0, 24, VIEW_COLS * TILE, VIEW_ROWS * TILE);

/// `BattleMap_Click` (`0x00432443`): x 480 … 639, y 24 … 183, two pixels a cell
/// over the whole 80 × 80 field. **[V]**
pub const OVERVIEW: Rect = Rect::new(0x1E0, 0x18, 160, 160);

/// `FUN_004329A4` (`0x004329A4`): `Hotspot_Test(0x1E0, 0x1C0, &DAT_004DC710, 5)`
/// — five 32 × 32 buttons in a row at (480, 448). The table's records are
/// `(0,0)…(128,0)` with size 32, so the strip is exactly the panel's width.
///
/// **[V]**
pub const BUTTON_ORIGIN: (i32, i32) = (0x1E0, 0x1C0);
pub const BUTTON_SIZE: i32 = 32;
pub const BUTTON_COUNT: usize = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Button {
    /// `FUN_0043B9A1` — **pause**, and the battle starts in it.
    Pause,
    /// `FUN_0043BA29` — `Ui_OpenConfirm(12)` *"Retreat from field?"*, or
    /// `Ui_OpenConfirm(11)` *"Surrender castle?"* when this is a siege and the
    /// local player is the garrison. Either answer runs `FUN_0043BE65`, which is
    /// the autocalc.
    Retreat,
    /// `FUN_0043BBE7` — siege only, garrison only, castle level 3 or more,
    /// once: `FUN_00496B9F` clears a 7 × 4 patch of cells flagged `0x40` and
    /// adds 4 to both siege scores. On a field battle it enqueues message
    /// `0x6E` and does nothing else.
    Sally,
    /// `FUN_0043BD02` → `FUN_0047A76D` — **charge**, once per battle
    /// (`DAT_0055322C`).
    Charge,
    /// `FUN_0043BD67` — `Ui_OpenConfirm(9)` *"Autocalc battle?"*.
    Autocalc,
}

impl Button {
    pub const ALL: [Button; BUTTON_COUNT] =
        [Button::Pause, Button::Retreat, Button::Sally, Button::Charge, Button::Autocalc];

    pub fn rect(self) -> Rect {
        let i = Button::ALL.iter().position(|&b| b == self).unwrap_or(0) as i32;
        Rect::new(BUTTON_ORIGIN.0 + i * BUTTON_SIZE, BUTTON_ORIGIN.1, BUTTON_SIZE, BUTTON_SIZE)
    }

    pub fn label(self) -> &'static str {
        match self {
            Button::Pause => "PAUSE",
            Button::Retreat => "RETR",
            Button::Sally => "GATE",
            Button::Charge => "CHRG",
            Button::Autocalc => "AUTO",
        }
    }

    pub fn at(x: i32, y: i32) -> Option<Button> {
        Button::ALL.into_iter().find(|b| b.rect().contains(x, y))
    }
}

/// The three banner layouts of `DAT_004D31F4`, chosen by how many figures the
/// player holds — `FUN_0043C2A9`'s ladder: under 13 takes the first, under 19
/// the second, otherwise the third. **[V]**, read out of the table.
///
/// The record is 28 bytes from `DAT_004D31F0`: `frame, x, y, w, h` and two
/// zeroes. `FUN_004B1DEB` hit-tests `(x, y, w, h)`, a plain
/// `x ≤ mx < x + w, y ≤ my < y + h`; `FUN_004238B8` (`0x004238B8`) blits
/// `Misc_bat` frame `record.frame + troopType` at `(x, y)`. **[V]**, the 80
/// records read out of `Lords2.exe` at file offset `0xD13F0`.
pub struct BannerLayout {
    pub origin: (i32, i32),
    pub size: (i32, i32),
    pub pitch: (i32, i32),
    pub cols: usize,
    pub slots: usize,
    pub frame: usize,
}

pub const BANNERS_FEW: BannerLayout = BannerLayout {
    origin: (488, 189),
    size: (45, 50),
    pitch: (53, 55),
    cols: 3,
    slots: 12,
    frame: 13,
};
pub const BANNERS_SOME: BannerLayout = BannerLayout {
    origin: (488, 186),
    size: (45, 35),
    pitch: (53, 37),
    cols: 3,
    slots: 18,
    frame: 24,
};
pub const BANNERS_MANY: BannerLayout = BannerLayout {
    origin: (484, 185),
    size: (22, 18),
    pitch: (26, 19),
    cols: 6,
    slots: 50,
    frame: 35,
};

impl BannerLayout {
    /// `FUN_0043C2A9`'s three-way choice.
    pub fn for_count(picked: usize) -> &'static BannerLayout {
        if picked < 13 {
            &BANNERS_FEW
        } else if picked < 19 {
            &BANNERS_SOME
        } else {
            &BANNERS_MANY
        }
    }

    pub fn rect(&self, slot: usize) -> Rect {
        let (col, row) = (slot % self.cols, slot / self.cols);
        Rect::new(
            self.origin.0 + col as i32 * self.pitch.0,
            self.origin.1 + row as i32 * self.pitch.1,
            self.size.0,
            self.size.1,
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollDir {
    N,
    NE,
    E,
    SE,
    S,
    SW,
    W,
    NW,
}

impl ScrollDir {
    /// `Map_ScrollStep` (`0x00431F59`)'s `g_battlePhase == 2` half: **one cell**
    /// per step, against the campaign map's two rows and one column. **[V]**
    pub fn delta(self) -> (i32, i32) {
        match self {
            ScrollDir::N => (0, -1),
            ScrollDir::NE => (1, -1),
            ScrollDir::E => (1, 0),
            ScrollDir::SE => (1, 1),
            ScrollDir::S => (0, 1),
            ScrollDir::SW => (-1, 1),
            ScrollDir::W => (-1, 0),
            ScrollDir::NW => (-1, -1),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Field,
    Drag,
    Outcome,
}

/// What `Battle_UpdateHover` (`0x0047ED9B`) recomputes every frame.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Hover {
    pub on_field: bool,
    /// The cell under the pointer — `DAT_00553F88` / `DAT_00553F94`.
    pub cell: (u8, u8),
    pub surface: u8,
    /// `DAT_0053E874`: the hovered cell's surface is **15**. Passed to
    /// `BattleUnit_Order` as its fifth argument, which `docs/symbols.json` calls
    /// `fromPlayer` and which is nothing of the kind — see
    /// [`BattleRunner::order_full`].
    pub woodland: bool,
    pub friendly: Option<usize>,
    pub friendly_picked: Option<usize>,
    pub enemy: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cursor {
    Arrow,
    Move,
    Attack,
    Select,
}

/// A drag in flight: `_DAT_0055CE60` / `_DAT_0055CE64` (the anchor **cell**) and
/// `DAT_0057A0F4` / `DAT_0057A0E8` (the anchor **pixel**).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Drag {
    pub anchor_cell: (u8, u8),
    pub anchor_px: (i32, i32),
    pub px: (i32, i32),
}

/// `FUN_00479CF7` (`0x00479CF7`) classifies a finished drag into three cases,
/// and its answer is what makes a click different from a box.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DragKind {
    Box,
    Pick,
    Nothing,
}

/// `FUN_00479CF7`'s threshold: a movement under 25 pixels on **both** axes is a
/// click
pub const DRAG_SLOP: i32 = 0x19;

/// **One `Sound_PlayTroopCry(class)`** (`0x00499CB1`) — a player's men
/// answering a selection or an order.
///
/// The original plays it from inside the arm; ours cannot, because a screen
/// cannot reach the audio layer (`docs/netcode.md` D-3). So the arm appends one
/// of these to [`LiveBattle::cries`] and `audio::Director` plays them after
/// the tick. What is recorded is exactly the two numbers the original's call
/// reads: the argument, and `DAT_0055408C` at that moment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cry {
    /// `DAT_0055408C` — see [`LiveBattle::cry_troop`].
    pub troop: u8,
    pub class: u8,
}

pub mod cry {
    pub const SELECTED: u8 = 0;
    pub const ORDERED: u8 = 1;
    pub const ATTACK: u8 = 2;
    /// `_M` — an order onto `g_battleHoverSurface == 2`. `[V]` for the test;
    /// `[I]` that the letter means *moat*, from surface 2 being the moat
    /// (`docs/battle.md` §3.2).
    pub const MOAT: u8 = 3;
    pub const MOAT_SURFACE: u8 = 2;
}

/// The nine numbered control groups, `DAT_00553400`, stride `0x18`, zeroed by
/// `Battle_Start`.
///
/// The window procedure (`0x004B29BE`) routes virtual keys `0x31` … `0x39` to
/// `FUN_0043C910` (recall) or, while `VK_CONTROL` is held (`DAT_004DF3A8`), to
/// `FUN_0043C885` (store). **[V]** Ten slots are cleared and only nine are
/// reachable.
pub const GROUPS: usize = 9;

/// `DAT_00568470`, counted up once a frame by `Battle_CheckOutcome` and by
/// `FUN_004782C5`; past 5000 the outcome banner gives way. A right release on
/// `0x2B` sets it to **5001**, which is the skip.
pub const OUTCOME_FRAMES: u32 = 5000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiveBattle {
    pub runner: BattleRunner,
    pub attacker: usize,
    pub defender: usize,
    pub county: u8,
    pub castle_level: Option<u8>,
    pub owner: u8,
    pub choice_owner: u8,
    /// `DAT_0053F238` — **and it starts set**. See the module header.
    pub paused: bool,
    /// `DAT_0055CE70` / `DAT_005678A0`, seeded `(0x20, 0x21)` by `Battle_Start`.
    pub cam: (i32, i32),
    pub mode: Mode,
    pub drag: Option<Drag>,
    pub hover: Hover,
    pub pointer: (i32, i32),
    pub pointer_in: bool,
    /// `DAT_00553400`: nine saved selections, each a list of figure indices.
    pub groups: [Option<Vec<usize>>; GROUPS],
    /// `DAT_0055322C` — the charge button fires once per battle.
    pub charged: bool,
    /// `DAT_0052AF9C` — the sally likewise.
    pub sallied: bool,
    /// `DAT_0053E984`, the unit the player's selection currently *is*.
    pub current_unit: usize,
    /// `DAT_00568470`.
    pub outcome_ticks: u32,
    /// **`DAT_0057A0F0` — this battle is a skirmish.** `Skirmish_Setup`
    /// (`0x0042B7F7`) raises it and `FUN_0043D649` (*Back*) lowers it. The end
    /// of a battle reads it three times, to skip the castle-damage record, the
    /// casualty write-back and the return to the campaign — there is no
    /// campaign behind a skirmish for any of the three to reach. Ours is
    /// raised by `SetupScreen::go_skirmish` and **read nowhere yet**: none of
    /// the three ends of a battle is built.
    pub skirmish: bool,
    pub conclusion: Option<Conclusion>,
    /// The player asked for the rest of the battle to be calculated — the
    /// Retreat and Autocalc buttons both land here, because both of them run
    /// `FUN_0043BE65`, which is `Battle_AutoResolve`.
    pub autocalc: bool,
    pub scroll_speed: i32,
    pub scroll_wait: u32,
    pub redraw: bool,
    pub cries: Vec<Cry>,
}

/// `g_optScrollSpeed`'s default, written by the options-defaults routine at
/// `0x004AE310`.
pub const DEFAULT_SCROLL_SPEED: i32 = 60;


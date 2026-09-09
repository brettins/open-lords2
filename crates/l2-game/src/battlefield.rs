//! **The live battle** — the state `Lords2.exe` keeps in globals while
//! `g_battlePhase == 2`, and every verb a player has on it.
//!
//! The drawing is [`crate::screens::battlefield`]; this is the state and the
//! rules, so that a test can play a battle with no window and no artwork.
//!
//! # What the battlefield's screens actually are
//!
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
//! So the battlefield is three screens:
//!
//! | id | what | where its arm lives |
//! |---|---|---|
//! | `0x29` | the field | `Screen_FrameInput` `0x0042FF10`, arm `')'` |
//! | `0x2A` | **the drag**, entered by pressing on the field | arm `'*'` |
//! | `0x2B` | the outcome banner | arm `'+'` |
//!
//! `0x2A` is a mode here rather than a [`crate::screen::ScreenId`] of its own —
//! see [`Mode`] — because pushing and popping a screen per drag would put a
//! stack operation on a pointer motion. Every arm of it is still reproduced
//! individually and marked.
//!
//! # A battle starts **paused**
//!
//! `Battle_Start` (`0x004778A0`) writes `DAT_0053F238 = 0xFFFFFFFF` before it
//! raises `g_screenId = 0x29`, and battle button 0 (`FUN_0043B9A1`) toggles that
//! word with a bitwise NOT — so it flips between `-1` and `0` and the first
//! thing a player does is unpause. While it is set, `FUN_0043C57D` refuses to
//! issue an order and `Battle_Frame` paints `L2.eng` group 32 index 0 across the
//! bottom of the field. **[V]**
//!
//! # The right column, top to bottom
//!
//! | y | what | the original |
//! |---|---|---|
//! | 24 … 183 | the 80 × 80 overview at 2 px a cell | `BattleMap_Click` `0x00432443` |
//! | 185 … 404 | the banners of the figures you hold | `FUN_0043C2A9` `0x0043C2A9`, table `0x004D31F4` |
//! | 448 … 480 | five buttons | `FUN_004329A4` `0x004329A4`, table `0x004DC710` |

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
/// **[V]**
pub const BUTTON_ORIGIN: (i32, i32) = (0x1E0, 0x1C0);
pub const BUTTON_SIZE: i32 = 32;
pub const BUTTON_COUNT: usize = 5;

/// The five buttons, in table order.
///
/// Every one of them is guarded by `g_battleChoiceOwner`, and three of the five
/// want it to be **1** — the local player owns the take-the-field choice —
/// rather than merely non-zero. A player watching somebody else's battle can
/// pause it and charge, and cannot retreat or autocalc it.
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
/// Each entry of the table is 28 bytes and its first four `i32`s are
/// `(x, y, w, h)` for `FUN_004B1DEB`, which is a plain
/// `x ≤ mx < x + w, y ≤ my < y + h`.
pub struct BannerLayout {
    pub origin: (i32, i32),
    pub size: (i32, i32),
    pub pitch: (i32, i32),
    pub cols: usize,
    pub slots: usize,
}

/// Twelve big banners, 3 × 4 at (488, 189), 45 × 50, pitched 53 × 55.
pub const BANNERS_FEW: BannerLayout =
    BannerLayout { origin: (488, 189), size: (45, 50), pitch: (53, 55), cols: 3, slots: 12 };
/// Eighteen medium banners, 3 × 6 at (488, 186), 45 × 35, pitched 53 × 37.
pub const BANNERS_SOME: BannerLayout =
    BannerLayout { origin: (488, 186), size: (45, 35), pitch: (53, 37), cols: 3, slots: 18 };
/// Fifty small banners, 6 × 9 at (484, 185), 22 × 18, pitched 26 × 19 — and
/// **fifty is the click loop's own bound**, `if (0x31 < local_c) break`.
pub const BANNERS_MANY: BannerLayout =
    BannerLayout { origin: (484, 185), size: (22, 18), pitch: (26, 19), cols: 6, slots: 50 };

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

/// The pointer's eight compass directions, `g_mapScrollDir` 0 … 7.
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

/// Which of the two field screens is up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// `g_screenId == 0x29`.
    Field,
    /// `g_screenId == 0x2A` — the left button is down and a box is being drawn.
    Drag,
    /// `g_screenId == 0x2B` — the battle is over and the banner is up.
    Outcome,
}

/// What `Battle_UpdateHover` (`0x0047ED9B`) recomputes every frame.
///
/// The five globals it writes are the whole of what the cursor ladder and two
/// of the three order arms read. It runs **after** the cursor is chosen in
/// `Battle_Frame`, so the original's battle pointer is one frame behind the
/// pointer; ours is not, and that is a deliberate difference recorded here
/// rather than reproduced — a one-frame-stale cursor is a defect of the
/// original's frame order, not a rule.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Hover {
    /// `g_battleHoverOnField`.
    pub on_field: bool,
    /// The cell under the pointer — `DAT_00553F88` / `DAT_00553F94`.
    pub cell: (u8, u8),
    /// `g_battleHoverSurface`, cell byte `+7`.
    pub surface: u8,
    /// `DAT_0053E874`: the hovered cell's surface is **15**. Passed to
    /// `BattleUnit_Order` as its fifth argument, which `docs/symbols.json` calls
    /// `fromPlayer` and which is nothing of the kind — see
    /// [`BattleRunner::order_full`].
    pub woodland: bool,
    /// `g_battleHoverFriendly`: one of yours, and **nothing is selected**.
    pub friendly: Option<usize>,
    /// `g_battleHoverFriendlyPicked`: one of yours while something *is*
    /// selected. The two are exclusive by construction.
    pub friendly_picked: Option<usize>,
    /// `g_battleHoverEnemy`: not yours, something is selected, and at least one
    /// selected figure is not a siege engine.
    pub enemy: Option<usize>,
}

/// The cursor kinds `Battle_Frame`'s ladder passes to `Cursor_Set`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cursor {
    /// 0 — the plain arrow.
    Arrow,
    /// 4 — the move order.
    Move,
    /// 5 — the attack.
    Attack,
    /// 6 — "this man can be picked".
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
    /// Moved 25 pixels or more on either axis: a real box.
    Box,
    /// Barely moved, over one of your own men: pick him.
    Pick,
    /// Barely moved, over nothing: the original returns 0 and the release does
    /// nothing at all — **it does not clear the selection**.
    Nothing,
}

/// `FUN_00479CF7`'s threshold: a movement under 25 pixels on **both** axes is a
/// click rather than a drag.
pub const DRAG_SLOP: i32 = 0x19;

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

/// Everything the battle keeps outside `l2-sim`: the camera, the pause, the
/// selection's UI, the groups and the outcome timer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiveBattle {
    pub runner: BattleRunner,
    /// The two campaign unit slots, so the conclusion can be written back.
    pub attacker: usize,
    pub defender: usize,
    pub county: u8,
    pub castle_level: Option<u8>,
    /// `g_localPlayer`'s realm, which is who every arm here acts for.
    pub owner: u8,
    /// `g_battleChoiceOwner`: 1 when the local player owns the take-the-field
    /// choice. Three of the five buttons want exactly 1.
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
    /// Set once `Battle_CheckOutcome` would have raised `0x2B`.
    pub conclusion: Option<Conclusion>,
    /// The player asked for the rest of the battle to be calculated — the
    /// Retreat and Autocalc buttons both land here, because both of them run
    /// `FUN_0043BE65`, which is `Battle_AutoResolve`.
    pub autocalc: bool,
    /// `g_optScrollSpeed`, and the ticks left before the camera may move again.
    /// See [`scroll_interval_ticks`].
    pub scroll_speed: i32,
    pub scroll_wait: u32,
    pub redraw: bool,
}

impl LiveBattle {
    /// `Battle_Start` (`0x004778A0`)'s tail, in the order it writes: the groups
    /// are cleared, the pause word is set to `0xFFFFFFFF`, the screen becomes
    /// `0x29`, `g_battlePhase` becomes 2, input is armed (`DAT_00568964 = 1`)
    /// and the camera is put at `(0x20, 0x21)`.
    pub fn new(
        runner: BattleRunner,
        attacker: usize,
        defender: usize,
        county: u8,
        castle_level: Option<u8>,
        owner: u8,
        choice_owner: u8,
    ) -> LiveBattle {
        LiveBattle {
            runner,
            attacker,
            defender,
            county,
            castle_level,
            owner,
            choice_owner,
            paused: true,
            cam: (0x20, 0x21),
            mode: Mode::Field,
            drag: None,
            hover: Hover::default(),
            pointer: (VIEW.w / 2, VIEW.y + VIEW.h / 2),
            pointer_in: false,
            groups: Default::default(),
            charged: false,
            sallied: false,
            current_unit: 0,
            outcome_ticks: 0,
            conclusion: None,
            autocalc: false,
            scroll_speed: DEFAULT_SCROLL_SPEED,
            scroll_wait: 0,
            redraw: true,
        }
    }

    /// `g_screenId` as the original would hold it.
    pub fn screen_id(&self) -> u8 {
        match self.mode {
            Mode::Field => 0x29,
            Mode::Drag => 0x2A,
            Mode::Outcome => 0x2B,
        }
    }

    pub fn is_siege(&self) -> bool {
        self.castle_level.is_some()
    }

    /// The camera, clamped so the viewport never leaves the 80 × 80 field —
    /// `FUN_0047ED34`, which `Map_ScrollStep` and both centring paths call.
    fn clamp_cam(&mut self) {
        self.cam.0 = self.cam.0.clamp(0, DIM as i32 - VIEW_COLS);
        self.cam.1 = self.cam.1.clamp(0, DIM as i32 - VIEW_ROWS);
    }

    /// The cell a viewport pixel lands on — `FUN_004BC2A5` / `FUN_004BC2C5`,
    /// which are `(px − origin) / tileSize` and nothing else.
    pub fn cell_at(&self, x: i32, y: i32) -> (u8, u8) {
        let cx = (self.cam.0 + (x - VIEW.x) / TILE).clamp(0, DIM as i32 - 1);
        // The original clamps only y, and only against 0x4F. Reproduced: x is
        // not clamped there because the viewport test has already bounded it.
        let cy = (self.cam.1 + (y - VIEW.y) / TILE).clamp(0, DIM as i32 - 1);
        (cx as u8, cy as u8)
    }

    /// The two rounding rules `FUN_0043C247`'s box uses, which are **not** the
    /// same as [`Self::cell_at`]: the near corner rounds up when it is more than
    /// three quarters of the way into a tile (`FUN_004BC2E5`, `FUN_004BC3AC`)
    /// and the far corner rounds down when it is less than a quarter of the way
    /// in (`FUN_004BC346`, `FUN_004BC40D`). So the box snaps to whole cells with
    /// a quarter-tile of tolerance at each edge. **[V]**
    fn box_corners(&self, a: (i32, i32), b: (i32, i32)) -> ((u8, u8), (u8, u8)) {
        let (x0, x1) = (a.0.min(b.0), a.0.max(b.0));
        let (y0, y1) = (a.1.min(b.1), a.1.max(b.1));
        let near = |p: i32, origin: i32| {
            let d = p - origin;
            let mut c = d.div_euclid(TILE);
            if d.rem_euclid(TILE) >= TILE / 4 * 3 {
                c += 1;
            }
            c.max(0)
        };
        let far = |p: i32, origin: i32, bound: i32| {
            let d = p - origin;
            let mut c = d.div_euclid(TILE);
            if d.rem_euclid(TILE) < TILE / 4 {
                c -= 1;
            }
            c.min(bound - 1)
        };
        let lx = self.cam.0 + near(x0, VIEW.x);
        let ly = self.cam.1 + near(y0, VIEW.y);
        let hx = self.cam.0 + far(x1, VIEW.x, VIEW_COLS);
        let hy = self.cam.1 + far(y1, VIEW.y, VIEW_ROWS);
        let c = |v: i32| v.clamp(0, DIM as i32 - 1) as u8;
        ((c(lx), c(ly)), (c(hx), c(hy)))
    }

    // ------------------------------------------------------------------ hover

    /// `Battle_UpdateHover` (`0x0047ED9B`), once a frame.
    ///
    /// One clause is reproduced *corrected* rather than as found, and it is
    /// flagged here because it is the only place in this file that departs from
    /// the binary. The original's count of selected non-siege figures indexes
    /// the figure array by `g_curBattleMan` — a **different global**, left over
    /// from whatever sweep ran last and normally sitting one record past the end
    /// of the array — instead of by its own loop variable. Verified at the
    /// instruction level (`a1 f8 e8 53 00` = `mov eax,[g_curBattleMan]` where
    /// the two clauses either side use `mov eax,[ebp-4]`). `docs/bugs.md` B69
    /// records it; the byte it reads is in zeroed BSS, so the clause is true in
    /// practice and the corrected reading is the one that matches play.
    ///
    /// // arm: 0x0047ED9B/hover
    pub fn update_hover(&mut self) {
        let (x, y) = self.pointer;
        self.hover = Hover::default();
        if !self.pointer_in || !VIEW.contains(x, y) {
            return;
        }
        let cell = self.cell_at(x, y);
        let picked = self.runner.selected_count(self.owner);
        let picked_troops = self
            .runner
            .selected_fighters(self.owner)
            .into_iter()
            .filter(|&f| !self.runner.fighters[f].troop.is_siege())
            .count();
        let surface = self.runner.field.at(cell.0 as usize, cell.1 as usize).surface;
        self.hover = Hover {
            on_field: true,
            cell,
            surface,
            woodland: surface == 15,
            ..Hover::default()
        };
        let Some(fig) = self.runner.occupant_of(cell.0, cell.1) else { return };
        let owner = self.runner.fighters[fig].side;
        let mine = self.runner.selected_by(fig) == self.owner
            || self.runner.sim.figures[self.runner.fighters[fig].sim].owner == self.owner;
        let _ = owner;
        if mine {
            if picked > 0 {
                self.hover.friendly_picked = Some(fig);
            } else {
                self.hover.friendly = Some(fig);
            }
        } else if picked > 0 && picked_troops > 0 {
            self.hover.enemy = Some(fig);
        }
    }

    /// `Battle_Frame`'s cursor ladder (`0x004B99C0`), the arm for
    /// `0x28 ≤ g_screenId ≤ 0x2A`.
    ///
    /// Seven leaves, and the order matters: an enemy under the pointer beats
    /// everything, `0x2A` is always the plain arrow, and with a selection in
    /// hand a pointer *outside* the field still shows the move cursor as long as
    /// it is above y 184 — which is the overview panel, where a click really
    /// does order.
    ///
    /// // arm: 0x004B99C0/cursor
    pub fn cursor(&self) -> Cursor {
        if self.mode == Mode::Drag {
            return Cursor::Arrow;
        }
        if self.hover.enemy.is_some() {
            return Cursor::Attack;
        }
        let picked = self.runner.selected_count(self.owner);
        if picked == 0 {
            return if self.hover.friendly.is_some() { Cursor::Select } else { Cursor::Arrow };
        }
        if self.hover.friendly.is_some() || self.hover.friendly_picked.is_some() {
            return Cursor::Select;
        }
        if self.hover.on_field {
            return Cursor::Move;
        }
        if self.pointer.1 < 0xB8 {
            Cursor::Move
        } else {
            Cursor::Arrow
        }
    }

    // ------------------------------------------------------------- the camera

    /// `Map_EdgeScroll` (`0x00432221`) — the pointer against the outermost pixel
    /// of the screen. Unlike the campaign map's, the battle's is never disabled
    /// by a zoom: the `g_mapZoom == 2` early-out is guarded on
    /// `g_battlePhase == 0`.
    pub fn edge_direction(&self) -> Option<ScrollDir> {
        if !self.pointer_in {
            return None;
        }
        let (x, y) = self.pointer;
        let (w, e) = (x <= 0, x >= 639);
        let (n, s) = (y <= 0, y >= 479);
        match (n, e, s, w) {
            (true, false, false, false) => Some(ScrollDir::N),
            (true, true, false, false) => Some(ScrollDir::NE),
            (false, true, false, false) => Some(ScrollDir::E),
            (false, true, true, false) => Some(ScrollDir::SE),
            (false, false, true, false) => Some(ScrollDir::S),
            (false, false, true, true) => Some(ScrollDir::SW),
            (false, false, false, true) => Some(ScrollDir::W),
            (true, false, false, true) => Some(ScrollDir::NW),
            _ => None,
        }
    }

    /// `Map_ScrollStep` (`0x00431F59`)'s battle half, through
    /// `Map_ScrollThrottle`'s interval.
    ///
    /// The throttle is the campaign map's, minus the one clause that is not:
    /// `Map_ScrollThrottle` adds 2 to its quotient when `g_screenId == 0x10`,
    /// and the battlefield is not `0x10`, so the battle scrolls at the plain
    /// `((100 − speed) / 10) × 12 + 2` milliseconds. The quantisation to whole
    /// ticks is ours and is the same one `screens/map.rs` documents —
    /// `docs/decisions.md` C60. Without it the battle camera would move 62 cells
    /// a second.
    ///
    /// // arm: 0x00432221/battle-edge-scroll
    pub fn edge_scroll(&mut self) -> bool {
        let Some(dir) = self.edge_direction() else {
            self.scroll_wait = 0;
            return false;
        };
        if self.scroll_wait > 0 {
            self.scroll_wait -= 1;
            return false;
        }
        self.scroll_wait = scroll_interval_ticks(self.scroll_speed);
        let (dx, dy) = dir.delta();
        let before = self.cam;
        self.cam = (self.cam.0 + dx, self.cam.1 + dy);
        self.clamp_cam();
        self.redraw |= self.cam != before;
        self.cam != before
    }

    /// `FUN_0043C910`'s tail and `BattleMap_Click`'s else-arm both do this:
    /// put the *top-left* of the viewport seven cells above and left of a cell,
    /// which is not quite the centre of a 15 × 14 viewport and is reproduced as
    /// found.
    fn look_at(&mut self, cell: (u8, u8)) {
        self.cam = (cell.0 as i32 - 7, cell.1 as i32 - 7);
        self.clamp_cam();
        self.redraw = true;
    }

    // ------------------------------------------------------------ the buttons

    /// `FUN_0043B9A1` (`0x0043B9A1`) — **the pause button**.
    ///
    /// `DAT_0053F238 = ~DAT_0053F238` toggles between 0 and −1, and the branch
    /// underneath it — `if (DAT_0053F238 == 1) { … Sound_PlayFile("s032_01.wav") }`
    /// — can therefore never be taken. The pause sound in the shipped game is
    /// dead code. `docs/bugs.md` D38.
    ///
    /// // arm: 0x0043B9A1/pause
    pub fn press_pause(&mut self) -> bool {
        if self.choice_owner == 0 {
            return false;
        }
        self.paused = !self.paused;
        self.redraw = true;
        true
    }

    /// `FUN_0043BA29` (`0x0043BA29`) — **retreat**, or **surrender** when this
    /// is a siege and the local player owns army B.
    ///
    /// It opens `Ui_OpenConfirm(12)` or `Ui_OpenConfirm(11)`; both callbacks
    /// reach `FUN_0043BE65`, which is `Battle_AutoResolve` plus the return to
    /// the campaign. The prompt index is what tells them apart to the player and
    /// nothing downstream reads which was answered except `DAT_005653F4`.
    ///
    /// // arm: 0x0043BA29/retreat
    pub fn press_retreat(&mut self) -> Option<usize> {
        if self.choice_owner != 1 {
            return None;
        }
        Some(if self.is_siege() { 11 } else { 12 })
    }

    /// `FUN_0043BBE7` (`0x0043BBE7`) — the siege gate.
    ///
    /// Returns the `L2.eng` message the original enqueues when it refuses, or
    /// `None` when it acts. **The rules behind `FUN_00496B9F` are the siege
    /// agent's**; this arm is the button and its four guards, which are the
    /// battlefield's.
    ///
    /// // arm: 0x0043BBE7/sally
    pub fn press_sally(&mut self, garrison_is_local: bool) -> Result<(), u16> {
        if self.choice_owner == 0 {
            return Err(0);
        }
        if !self.is_siege() {
            return Err(0x6E);
        }
        if !garrison_is_local || self.castle_level.unwrap_or(0) < 3 {
            return Err(0x6F);
        }
        if self.sallied {
            return Err(0x9D);
        }
        self.sallied = true;
        self.redraw = true;
        Ok(())
    }

    /// `FUN_0043BD02` (`0x0043BD02`) → `FUN_0047A76D` (`0x0047A76D`) — **the
    /// charge**, and it is available exactly once in a battle.
    ///
    /// // arm: 0x0043BD02/charge
    pub fn press_charge(&mut self) -> bool {
        if self.choice_owner == 0 || self.charged {
            return false;
        }
        self.charged = true;
        self.runner.charge_all(self.owner);
        self.redraw = true;
        true
    }

    /// `FUN_0043BD67` (`0x0043BD67`) — `Ui_OpenConfirm(9)`, *"Autocalc
    /// battle?"*.
    ///
    /// // arm: 0x0043BD67/autocalc
    pub fn press_autocalc(&mut self) -> Option<usize> {
        if self.choice_owner != 1 {
            return None;
        }
        Some(9)
    }

    /// What the two confirm boxes do when they are answered yes: `FUN_0043BE65`
    /// (`0x0043BE65`), which is `Battle_AutoResolve` and the return to the
    /// campaign. Declining a battle and giving up on one you are watching are
    /// the same code.
    pub fn confirm_autocalc(&mut self) {
        self.autocalc = true;
    }

    // ---------------------------------------------------------- the selection

    /// `FUN_0043BF07` (`0x0043BF07`), the press half: **begin a drag**.
    ///
    /// Guarded on the pointer being over the field and the screen not already
    /// being `0x2A`. It records both the anchor cell and the anchor pixel — the
    /// cell is what the box is made of and the pixel is what
    /// [`DragKind`] is measured in — and it sets the debug panel's figure to
    /// whatever is standing under the press, which is the one thing this arm
    /// does that a player never sees.
    ///
    /// // arm: 0x0043BF07/begin-drag
    pub fn press_field(&mut self, x: i32, y: i32) -> bool {
        if self.mode != Mode::Field || !VIEW.contains(x, y) {
            return false;
        }
        let cell = self.cell_at(x, y);
        self.drag = Some(Drag { anchor_cell: cell, anchor_px: (x, y), px: (x, y) });
        self.mode = Mode::Drag;
        self.redraw = true;
        true
    }

    /// `FUN_0043BF07`'s middle branch: **the box while the button is down**.
    ///
    /// The original re-runs the whole selection every frame the pointer moves —
    /// `FUN_0043C247(player, 0, …)` — so what is highlighted follows the box
    /// live. The `0` is what stops it regrouping units on every pointer motion.
    /// It is skipped entirely while the battle is paused in a multiplayer game,
    /// which is the one clause here that has no single-player effect.
    ///
    /// // arm: 0x0043BF07/drag-update
    pub fn drag_to(&mut self, x: i32, y: i32) -> bool {
        let Some(d) = self.drag.as_mut() else { return false };
        if d.px == (x, y) {
            return false;
        }
        d.px = (x, y);
        let (a, b) = (d.anchor_px, d.px);
        let (lo, hi) = self.box_corners(a, b);
        self.runner.pick_box(self.owner, lo, hi, false);
        self.redraw = true;
        true
    }

    /// `FUN_00479CF7` (`0x00479CF7`) — which of the three things a release is.
    fn drag_kind(&self, d: Drag, x: i32, y: i32) -> DragKind {
        if (d.anchor_px.0 - x).abs() >= DRAG_SLOP || (d.anchor_px.1 - y).abs() >= DRAG_SLOP {
            return DragKind::Box;
        }
        if self.hover.friendly.is_some() || self.hover.friendly_picked.is_some() {
            return DragKind::Pick;
        }
        DragKind::Nothing
    }

    /// `FUN_0043BF07`'s release branch: **commit the selection**, and leave
    /// `0x2A`.
    ///
    /// Three outcomes, and the third is the one a modern game would get wrong:
    ///
    /// * a real box commits it with `FUN_0043C247(player, 1, …)`, which clears,
    ///   boxes, **regroups the units** and narrows;
    /// * a click on one of your own men nudges the box out by eight pixels in
    ///   each direction and commits *that* — the original literally rewrites the
    ///   anchor and the pointer, so a click selects a 16-pixel square and
    ///   therefore usually one man;
    /// * a click on nothing does **nothing**. It does not clear the selection.
    ///   Clearing is the right button's job and this is why.
    ///
    /// **The return value is the ladder, and it is load-bearing.** `true` means
    /// the guard consumed the release, and `Screen_FrameInput` then `goto`s past
    /// `FUN_0043C57D`. The third case returns `false` — `FUN_0043BF07` falls
    /// through to `uVar1 = 0` — and *that is how a click on empty ground with a
    /// selection becomes an order*: the press opened the drag, the release moved
    /// nothing and hit nobody, the drag arm declined, and the order arm behind it
    /// fired. Getting this backwards would make a finished box also issue an
    /// order at the corner it was released on.
    ///
    /// // arm: 0x0043BF07/commit-drag
    pub fn release_field(&mut self, x: i32, y: i32) -> bool {
        let Some(d) = self.drag.take() else { return false };
        self.mode = Mode::Field;
        self.redraw = true;
        match self.drag_kind(d, x, y) {
            DragKind::Box => {
                let (lo, hi) = self.box_corners(d.anchor_px, (x, y));
                self.runner.pick_box(self.owner, lo, hi, true);
            }
            DragKind::Pick => {
                // `DAT_0057A0F4 -= 8; DAT_0057A0E8 -= 8; g_mouseX += 8;
                //  g_mouseY += 8;` then the same commit.
                let a = (d.anchor_px.0 - 8, d.anchor_px.1 - 8);
                let b = (x + 8, y + 8);
                let (lo, hi) = self.box_corners(a, b);
                self.runner.pick_box(self.owner, lo, hi, true);
            }
            // Declined, so the order arm behind this one gets the release.
            DragKind::Nothing => return false,
        }
        self.current_unit = self.runner.regroup_selection(self.owner);
        true
    }

    /// `FUN_0043BF07`'s **double-click** clause, and the reason
    /// `crate::input`'s claim that `Village_DoubleClick` is the only reader of
    /// `g_mouseLeftDoubleClick` was false.
    ///
    /// The test is `(g_mouseLeftReleased || g_mouseLeftDoubleClick) &&
    /// g_screenId == 0x2A`, so a double click **commits an open drag exactly as
    /// a release would**. It is not a verb of its own; it exists because Windows
    /// sends `WM_LBUTTONDBLCLK` instead of the second `WM_LBUTTONDOWN`, so
    /// without this clause the second click of a fast double click would leave
    /// the drag open for ever.
    ///
    /// // arm: 0x0043BF07/double-click-commits
    pub fn double_click_field(&mut self, x: i32, y: i32) -> bool {
        if self.mode != Mode::Drag {
            return false;
        }
        self.release_field(x, y)
    }

    /// `FUN_0043C57D` (`0x0043C57D`) → `FUN_0043C634` — **the order**.
    ///
    /// Five guards, all of them refusals rather than fallbacks: the pointer must
    /// be on the field, it must **not** be over one of your own men (either
    /// hover flag blocks it, which is what makes clicking a friend a selection
    /// and never a destination), the button must have been *released*, something
    /// must be selected, and the battle must not be paused.
    ///
    /// // arm: 0x0043C57D/order
    pub fn order_at(&mut self, x: i32, y: i32) -> bool {
        if !VIEW.contains(x, y) || !self.hover.on_field {
            return false;
        }
        if self.hover.friendly_picked.is_some() || self.hover.friendly.is_some() {
            return false;
        }
        if self.runner.selected_count(self.owner) == 0 || self.paused {
            return false;
        }
        let cell = self.cell_at(x, y);
        let target = self.hover.enemy;
        let woodland = self.hover.woodland;
        let ok = self.runner.order_selected(self.owner, cell.0, cell.1, target, woodland);
        if ok {
            self.current_unit = self.runner.regroup_selection(self.owner);
            self.redraw = true;
        }
        ok
    }

    /// `FUN_0043C2A9` (`0x0043C2A9`), the left half: **a click on a banner drops
    /// that figure from the selection**.
    ///
    /// The banners are laid out by [`BannerLayout::for_count`] and walked in
    /// figure-index order, so slot *n* is the *n*-th figure you hold. **Only the
    /// first fifty are clickable**, whatever the layout and however many you
    /// hold: the loop breaks at `0x31 < local_c`.
    ///
    /// // arm: 0x0043C2A9/banner-drop
    pub fn click_banner(&mut self, x: i32, y: i32) -> bool {
        let picked = self.runner.selected_fighters(self.owner);
        let layout = BannerLayout::for_count(picked.len());
        for (slot, &fig) in picked.iter().enumerate() {
            if slot > 0x31 {
                break;
            }
            if layout.rect(slot).contains(x, y) {
                self.runner.deselect_figure(fig);
                self.current_unit = self.runner.regroup_selection(self.owner);
                self.redraw = true;
                return true;
            }
        }
        false
    }

    /// `FUN_0043C2A9`'s right half → `FUN_0043C55C` (`0x0043C55C`) — **the right
    /// button clears the whole selection**, and it is the arm the audit's
    /// "right-click exits" habit would have replaced with a way out of the
    /// battle.
    ///
    /// It is refused inside the overview panel — `x ≥ 0x1E1 && 0x18 ≤ y ≤ 0xB7`
    /// — and note the off-by-one: the panel starts at `0x1E0` and the guard
    /// tests `0x1E1`, so **a right click on the panel's leftmost column
    /// deselects**. Reproduced.
    ///
    /// // arm: 0x0043C2A9/right-deselect
    pub fn right_deselect(&mut self, x: i32, y: i32) -> bool {
        if x >= 0x1E1 && (0x18..=0xB7).contains(&y) {
            return false;
        }
        self.runner.clear_selection(self.owner);
        self.current_unit = 0;
        self.redraw = true;
        true
    }

    // ------------------------------------------------------- the overview map

    /// `BattleMap_Click` (`0x00432443`) — the 160 × 160 panel at two pixels a
    /// cell.
    ///
    /// It is reached from `Screen_FrameInput`'s **epilogue**, after every arm,
    /// on any screen but `0x12` — so it is live on `0x29`, `0x2A` and `0x2B`
    /// alike. Two behaviours in one hit test:
    ///
    /// * left button, something selected, no oil selected, not paused →
    ///   **order to that cell**, at battlefield scale, from a 2-pixel click;
    /// * anything else, including the right button → **look there**.
    ///
    /// // arm: 0x00432443/overview
    pub fn click_overview(&mut self, x: i32, y: i32, right: bool) -> bool {
        if !OVERVIEW.contains(x, y) {
            return false;
        }
        let cell = (((x - OVERVIEW.x) / 2) as u8, ((y - OVERVIEW.y) / 2) as u8);
        let orderable = !right && self.runner.selected_count(self.owner) > 0 && !self.paused;
        if orderable {
            self.runner.order_selected(self.owner, cell.0, cell.1, None, false);
            self.current_unit = self.runner.regroup_selection(self.owner);
        } else {
            self.look_at(cell);
        }
        self.redraw = true;
        true
    }

    // ---------------------------------------------------------------- the keys

    /// `FUN_0043C885` (`0x0043C885`) — **Ctrl and a digit stores the
    /// selection**.
    ///
    /// // arm: 0x0043C885/store-group
    pub fn store_group(&mut self, digit: u8) -> bool {
        let Some(slot) = group_slot(digit) else { return false };
        self.groups[slot] = Some(self.runner.selected_fighters(self.owner));
        true
    }

    /// `FUN_0043C910` (`0x0043C910`) — **a digit recalls it, and moves the
    /// camera to it**.
    ///
    /// The camera jump is part of the arm, not a convenience: the original ends
    /// the function by scanning for the first selected figure and putting the
    /// viewport's corner seven cells above and left of it.
    ///
    /// // arm: 0x0043C910/recall-group
    pub fn recall_group(&mut self, digit: u8) -> bool {
        let Some(slot) = group_slot(digit) else { return false };
        let Some(members) = self.groups[slot].clone() else { return false };
        self.runner.clear_selection(self.owner);
        for f in &members {
            self.runner.select_figure(*f, self.owner);
        }
        self.current_unit = self.runner.regroup_selection(self.owner);
        if let Some(&first) = self.runner.selected_fighters(self.owner).first() {
            let f = &self.runner.fighters[first];
            self.look_at((f.x, f.y));
        }
        self.redraw = true;
        true
    }

    /// `FUN_0043C77A` (`0x0043C77A`) — **`H` and `V`**.
    ///
    /// Line and column. The original reads `DAT_0053E984` straight — it does not
    /// regroup first — so pressing `H` after men have died reforms whatever unit
    /// the selection last became, which may no longer be the whole selection.
    /// Reproduced, including the missing regroup.
    ///
    /// // arm: 0x0043C77A/formation
    pub fn key_formation(&mut self, formation: Formation) -> bool {
        if self.paused || self.current_unit == 0 {
            return false;
        }
        self.runner.order_formation(self.current_unit, formation);
        self.redraw = true;
        true
    }

    // ------------------------------------------------------------ the outcome

    /// `Battle_CheckOutcome` (`0x00477DFC`) raises `0x2B` and then counts
    /// `DAT_00568470` up to 5000 before it returns to the campaign.
    ///
    /// // arm: 0x00477DFC/outcome-timer
    pub fn tick_outcome(&mut self) -> bool {
        if self.mode != Mode::Outcome {
            return false;
        }
        self.outcome_ticks += 1;
        self.outcome_ticks > OUTCOME_FRAMES
    }

    /// `Screen_FrameInput`'s `0x2B` arm — **a right release skips the banner**,
    /// by setting the counter one past its limit.
    ///
    /// // arm: 0x0042FF10/skip-outcome
    pub fn skip_outcome(&mut self) -> bool {
        if self.mode != Mode::Outcome {
            return false;
        }
        self.outcome_ticks = OUTCOME_FRAMES + 1;
        true
    }

    /// One simulation tick, unless the battle is paused or over.
    ///
    /// `Battle_Frame` runs the whole update chain regardless and the pause
    /// stops it much further in; the observable difference is none, and doing it
    /// here keeps `l2-sim` free of a flag that is an interface state.
    pub fn tick(&mut self) {
        if self.mode == Mode::Outcome {
            self.tick_outcome();
            return;
        }
        if self.paused {
            return;
        }
        self.runner.step();
        self.redraw = true;
        if self.conclusion.is_none() {
            if let Some(c) = self.runner.conclusion() {
                self.conclusion = Some(c);
                self.mode = Mode::Outcome;
                self.outcome_ticks = 0;
            }
        }
    }

    pub fn take_redraw(&mut self) -> bool {
        std::mem::take(&mut self.redraw)
    }
}

/// `Map_ScrollThrottle` (`0x004320D1`) in whole simulation ticks.
///
/// `((100 − speed) / 10) × 12 + 2` milliseconds, rounded to the nearest tick and
/// never below one; speed 0 never scrolls. Identical to `screens/map.rs`'s,
/// without the `+= 2` that function adds for `g_screenId == 0x10`.
pub fn scroll_interval_ticks(speed: i32) -> u32 {
    const TICK_MS: u32 = 16;
    let q = (100 - speed.clamp(0, 100)) / 10;
    if q >= 10 {
        return u32::MAX;
    }
    let ms = (q * 12 + 2) as u32;
    ((ms + TICK_MS / 2) / TICK_MS).max(1)
}

/// `g_optScrollSpeed`'s default, written by the options-defaults routine at
/// `0x004AE310`.
pub const DEFAULT_SCROLL_SPEED: i32 = 60;

/// Virtual keys `0x31` … `0x39` are groups 0 … 8. `0x30` is not one: the
/// original tests `0x30 < key && key < 0x3A`.
fn group_slot(digit: u8) -> Option<usize> {
    if (b'1'..=b'9').contains(&digit) {
        Some((digit - b'1') as usize)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_five_buttons_tile_the_bottom_of_the_panel_and_nothing_else() {
        let mut last = BUTTON_ORIGIN.0;
        for b in Button::ALL {
            let r = b.rect();
            assert_eq!(r.x, last, "{b:?} does not abut its neighbour");
            assert_eq!((r.y, r.w, r.h), (0x1C0, 32, 32));
            last = r.x + r.w;
        }
        assert_eq!(last, 640, "the five buttons end at the right edge");
        assert_eq!(Button::at(0x1E0, 0x1C0), Some(Button::Pause));
        assert_eq!(Button::at(639, 479), Some(Button::Autocalc));
        assert_eq!(Button::at(0x1DF, 0x1C0), None, "one pixel left of the strip");
        assert_eq!(Button::at(0x1E0, 0x1BF), None, "one pixel above it");
    }

    /// The three banner layouts, straight out of `DAT_004D31F4`: the first
    /// three rectangles of each, the slot counts, and that none of them
    /// overlaps the overview panel above or the buttons below.
    #[test]
    fn the_banner_layouts_match_the_table_and_stay_between_the_panels() {
        assert_eq!(BannerLayout::for_count(0).slots, 12);
        assert_eq!(BannerLayout::for_count(12).slots, 12);
        assert_eq!(BannerLayout::for_count(13).slots, 18);
        assert_eq!(BannerLayout::for_count(18).slots, 18);
        assert_eq!(BannerLayout::for_count(19).slots, 50);
        assert_eq!(BannerLayout::for_count(80).slots, 50);

        // Table entries 0, 1, 3 of the twelve-slot layout.
        assert_eq!(BANNERS_FEW.rect(0), Rect::new(488, 189, 45, 50));
        assert_eq!(BANNERS_FEW.rect(1), Rect::new(541, 189, 45, 50));
        assert_eq!(BANNERS_FEW.rect(3), Rect::new(488, 244, 45, 50));
        // Entries 12, 13, 15 of the table are slots 0, 1, 3 of the second.
        assert_eq!(BANNERS_SOME.rect(0), Rect::new(488, 186, 45, 35));
        assert_eq!(BANNERS_SOME.rect(1), Rect::new(541, 186, 45, 35));
        assert_eq!(BANNERS_SOME.rect(3), Rect::new(488, 223, 45, 35));
        // Entries 30, 31, 36 are slots 0, 1, 6 of the third.
        assert_eq!(BANNERS_MANY.rect(0), Rect::new(484, 185, 22, 18));
        assert_eq!(BANNERS_MANY.rect(1), Rect::new(510, 185, 22, 18));
        assert_eq!(BANNERS_MANY.rect(6), Rect::new(484, 204, 22, 18));

        for l in [&BANNERS_FEW, &BANNERS_SOME, &BANNERS_MANY] {
            let last = l.rect(l.slots - 1);
            assert!(l.rect(0).y > OVERVIEW.y + OVERVIEW.h - 1, "banners start under the overview");
            assert!(last.y + last.h <= 0x1C0, "banners end above the buttons");
        }
    }

    #[test]
    fn only_the_nine_digit_keys_are_control_groups() {
        assert_eq!(group_slot(b'1'), Some(0));
        assert_eq!(group_slot(b'9'), Some(8));
        assert_eq!(group_slot(b'0'), None, "0x30 is below the original's bound");
        assert_eq!(group_slot(b'A'), None);
    }

    #[test]
    fn the_viewport_is_fifteen_by_fourteen_tiles_at_the_top_left() {
        assert_eq!(VIEW, Rect::new(0, 24, 480, 448));
        assert!(!VIEW.contains(480, 100), "the panel is not the field");
        assert!(!VIEW.contains(100, 23), "nor the menu bar");
        assert!(VIEW.contains(479, 471));
    }

    /// **The hit test and the picture agree about where the field is.**
    ///
    /// `docs/decisions.md` C61's other lesson: every campaign-map test ran on
    /// `Assets::placeholder`, the one configuration in which a broken hit test
    /// and the picture agree. Here they are two modules — `l2-view`'s renderer
    /// has its own copy of `Battle_LoadAssets`' geometry — and if they ever part
    /// company a click would land on a different cell from the one under the
    /// pointer, at every zoom and with any artwork. This needs no install
    /// because both sides are constants out of the binary; that is what makes it
    /// safe to test without one.
    #[test]
    fn our_hit_test_and_the_renderer_read_the_same_geometry() {
        use l2_view::scene;
        assert_eq!(TILE, scene::TILE);
        assert_eq!(VIEW_COLS as usize, scene::VIEW_COLS);
        assert_eq!(VIEW_ROWS as usize, scene::VIEW_ROWS);
        assert_eq!(VIEW.x, scene::ORIGIN_X);
        assert_eq!(VIEW.y, scene::ORIGIN_Y);
        assert_eq!(VIEW.w, scene::VIEW_COLS as i32 * scene::TILE);
        assert_eq!(VIEW.h, scene::VIEW_ROWS as i32 * scene::TILE);
    }

    /// The camera the renderer is handed is the one the hit test converts from,
    /// clamp included — a camera clamped on one side and not the other is the
    /// same defect one step later.
    #[test]
    fn the_camera_clamps_the_same_way_on_both_sides() {
        for (x, y) in [(-5, -5), (0, 0), (40, 40), (200, 200)] {
            let ours = (
                x.clamp(0, DIM as i32 - VIEW_COLS),
                y.clamp(0, DIM as i32 - VIEW_ROWS),
            );
            let theirs = l2_view::scene::Camera::clamped(x, y);
            assert_eq!((ours.0 as usize, ours.1 as usize), (theirs.x, theirs.y), "at {x},{y}");
        }
    }
}

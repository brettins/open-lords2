#![allow(unused_imports)]
use super::*;
use super::commands::*;
use super::field::*;
use super::groups::*;
use super::super::*;
use super::super::*;
use super::super::view::*;
use tests::*;
use l2_sim::runner::{BattleRunner, Conclusion, Formation};
use l2_sim::terrain::DIM;
use crate::input::Rect;

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
            charged: 0,
            sallied: false,
            current_unit: 0,
            outcome_ticks: 0,
            order_mark: (0, 0),
            order_mark_ticks: 0,
            skirmish: false,
            conclusion: None,
            autocalc: false,
            scroll_wait: 0,
            redraw: true,
            cries: Vec::new(),
            deferred: false,
            outbox: Vec::new(),
            awaiting: Default::default(),
        }
    }

    /// **`DAT_0055408C`** — the troop type most of the local player's picked
    /// figures belong to, which is the `unit` `Sound_PlayTroopCry` indexes by.
    ///
    /// `Battle_CountMenByType` (`0x00481B9A`) zeroes eleven counts, adds one per
    /// figure whose `selected == g_localPlayer`, and keeps the first troop whose
    /// count is **strictly** greater than the best so far — so a tie goes to the
    /// lower troop index and nobody picked is 0, peasants. `[V]`. It is rerun by
    /// `Battle_Frame` every frame and by the commit, so the value a cry reads is
    /// the current selection's.
    ///
    /// One difference, `[D]`: the census counts every figure with a non-zero
    /// owner, which includes a dead one still lying on the field, and ours
    /// counts the living.
    pub fn cry_troop(&self) -> u8 {
        let mut count = [0usize; 11];
        for f in self.runner.selected_fighters(self.owner) {
            count[self.runner.fighters[f].troop.index()] += 1;
        }
        let mut best = 0;
        for (t, &n) in count.iter().enumerate() {
            if count[best] < n {
                best = t;
            }
        }
        best as u8
    }

    /// **`DAT_0053E8BC`** — the fourth of the five panel counters
    /// `BattleUnits_RegroupSelection` (`0x00478987`) writes: how many picked
    /// figures are **boiling oil**, troop type 10. The five buckets are types
    /// 7, 8, 9, 10 and *everything else*, and **all five are zeroed when more
    /// than one bucket is non-empty** — so the counter is non-zero only for a
    /// selection that is nothing but oil. That is what makes the overview
    /// panel's "no oil selected" guard (`BattleMap_Click`, `0x00432443`) mean
    /// *this selection is a pot*: a pot is ordered by pouring downhill, not by
    /// a click on a map two pixels to the cell. `[V]`
    pub(super) fn oil_selected(&self) -> bool {
        let mut bucket = [false; 5];
        for f in self.runner.selected_fighters(self.owner) {
            let i = self.runner.fighters[f].troop.index();
            bucket[if (7..=10).contains(&i) { i - 7 } else { 4 }] = true;
        }
        bucket[3] && bucket.iter().filter(|&&b| b).count() == 1
    }

    pub(crate) fn cry(&mut self, class: u8) {
        let troop = self.cry_troop();
        self.cries.push(Cry { troop, class });
    }

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
        let cy = (self.cam.1 + (y - VIEW.y) / TILE).clamp(0, DIM as i32 - 1);
        (cx as u8, cy as u8)
    }

    /// The two rounding rules `FUN_0043C247`'s box uses, which are **not** the
    /// same as [`Self::cell_at`]: the near corner rounds up when it is more than
    /// three quarters of the way into a tile (`FUN_004BC2E5`, `FUN_004BC3AC`)
    /// and the far corner rounds down when it is less than a quarter of the way
    /// in (`FUN_004BC346`, `FUN_004BC40D`). So the box snaps to whole cells with
    /// a quarter-tile of tolerance at each edge. **[V]**
    pub(crate) fn box_corners(&self, a: (i32, i32), b: (i32, i32)) -> ((u8, u8), (u8, u8)) {
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


    /// `Battle_UpdateHover` (`0x0047ED9B`), once a frame.
    ///
    /// // arm: 0x0047ED9B/hover hover
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
    /// // arm: 0x004B99C0/cursor hover
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
    /// `Map_ScrollThrottle` adds 2 to its quotient when `g_screenId == 0x10`,
    /// and the battlefield is not `0x10`, so the battle scrolls at the plain
    /// `((100 − speed) / 10) × 12 + 2` milliseconds. The quantisation to whole
    /// ticks is ours and is the same one `screens/map/mod.rs` documents —
    /// `docs/decisions.md` C60. Without it the battle camera would move 62 cells
    /// a second.
    ///
    /// // arm: 0x00432221/battle-edge-scroll hover-at-edge
    pub fn edge_scroll(&mut self, scroll_speed: i32) -> bool {
        let Some(dir) = self.edge_direction() else {
            self.scroll_wait = 0;
            return false;
        };
        if self.scroll_wait > 0 {
            self.scroll_wait -= 1;
            return false;
        }
        // Map_ScrollStep 0x00431F59 in g_battlePhase 2 -> Map_ScrollThrottle 0x004BBBE3 reads g_optScrollSpeed.
        self.scroll_wait = scroll_interval_ticks(scroll_speed);
        let (dx, dy) = dir.delta();
        let before = self.cam;
        self.cam = (self.cam.0 + dx, self.cam.1 + dy);
        self.clamp_cam();
        self.redraw |= self.cam != before;
        self.cam != before
    }

    /// `FUN_0043C910`'s tail and `BattleMap_Click`'s else-arm both do this:
    pub(crate) fn look_at(&mut self, cell: (u8, u8)) {
        self.cam = (cell.0 as i32 - 7, cell.1 as i32 - 7);
        self.clamp_cam();
        self.redraw = true;
    }


}

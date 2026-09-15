#![allow(unused_imports)]
use super::*;

use super::*;
use super::view::*;
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
            charged: false,
            sallied: false,
            current_unit: 0,
            outcome_ticks: 0,
            skirmish: false,
            conclusion: None,
            autocalc: false,
            scroll_speed: DEFAULT_SCROLL_SPEED,
            scroll_wait: 0,
            redraw: true,
            cries: Vec::new(),
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
    fn oil_selected(&self) -> bool {
        let mut bucket = [false; 5];
        for f in self.runner.selected_fighters(self.owner) {
            let i = self.runner.fighters[f].troop.index();
            bucket[if (7..=10).contains(&i) { i - 7 } else { 4 }] = true;
        }
        bucket[3] && bucket.iter().filter(|&&b| b).count() == 1
    }

    /// Append one [`Cry`] for the current selection.
    fn cry(&mut self, class: u8) {
        let troop = self.cry_troop();
        self.cries.push(Cry { troop, class });
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
    /// One clause is reproduced *corrected* rather than faithfully, and it is
    /// flagged here because it is the only place in this file that departs from
    /// the binary. The original's count of selected non-siege figures indexes
    /// the figure array by `g_curBattleMan` — a **different global**, left over
    /// from whatever sweep ran last and normally sitting one record past the end
    /// of the array — instead of by its own loop variable. Verified at the
    /// instruction level (`a1 f8 e8 53 00` = `mov eax,[g_curBattleMan]` where
    /// the two clauses either side use `mov eax,[ebp-4]`). `docs/bugs.md` B100
    /// records it; the byte it reads is in zeroed BSS, so the clause is true in
    /// practice and the corrected reading is the one that matches play.
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
    /// Seven leaves, and the order matters: an enemy under the pointer beats
    /// everything, `0x2A` is always the plain arrow, and with a selection in
    /// hand a pointer *outside* the field still shows the move cursor as long as
    /// it is above y 184 — which is the overview panel, where a click really
    /// does order.
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
    /// ticks is ours and is the same one `screens/map/mod.rs` documents —
    /// `docs/decisions.md` C60. Without it the battle camera would move 62 cells
    /// a second.
    ///
    /// // arm: 0x00432221/battle-edge-scroll hover-at-edge
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
    /// // arm: 0x0043B9A1/pause left-press
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
    /// // arm: 0x0043BA29/retreat left-press
    pub fn press_retreat(&mut self) -> Option<usize> {
        if self.choice_owner != 1 {
            return None;
        }
        Some(if self.is_siege() { 11 } else { 12 })
    }

    /// `FUN_0043BBE7` (`0x0043BBE7`) — **lower the drawbridge**, the garrison's
    /// own button.
    ///
    /// ```c
    /// if (g_battleChoiceOwner == 0) return;
    /// if (!g_battleIsSiege)                          Msg_Enqueue(0x6E);   /* Sieges only! */
    /// else if (localPlayer != units[armyB].owner)    Msg_Enqueue(0x6F);   /* No drawbridge! */
    /// else if (g_castleLevel < 3)                    Msg_Enqueue(0x6F);
    /// else if (DAT_0052AF9C != 0)                    Msg_Enqueue(0x9D);   /* Drawbridge is down. */
    /// else if (!g_multiplayer) FUN_00496B9F(); else Net_SendCommand(0x45, 0);
    /// ```
    ///
    /// **The three messages are what name the verb**: `L2.eng` group 110
    /// *"Sieges only!"*, group 111 *"No drawbridge!"* and group 157
    /// *"Drawbridge is down."* — so `FUN_00496B9F` lowers a drawbridge and
    /// nothing else. The hand-off this was built from called it siege-engine
    /// placement; it is not, and `docs/decisions.md` `C79`
    /// records how the mistake was caught.
    ///
    /// **`army B` is the garrison**, `docs/battle.md` §4.3 — so this is a
    /// defender's verb, and it is why a besieged human being unable to reach the
    /// battlefield at all made the whole button dead. See
    /// `crate::turn`'s `choice_owner`.
    ///
    /// Returns the `L2.eng` message the original enqueues when it refuses, or
    /// `Ok` when the bridge went down. `sallied` is set **only when the routine
    ///
    /// means a level-3 castle whose layout carries no `0x40` cell leaves the
    /// button live.
    ///
    /// // arm: 0x0043BBE7/sally left-press
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
        // The order enters the simulation here, and only here.
        // display flag: it rewrites cell flags and surfaces, so two peers that
        // disagreed about it would be pathing through different castles.
        if !self.runner.lower_drawbridge() {
            // No `0x40` cell on the field. The original's latch is inside the
            // search's `if`, so the button is not spent either.
            return Err(0x6F);
        }
        self.sallied = true;
        self.redraw = true;
        Ok(())
    }

    /// `FUN_0043BD02` (`0x0043BD02`) → `FUN_0047A76D` (`0x0047A76D`) — **the
    /// charge**, and it is available exactly once in a battle.
    ///
    /// // arm: 0x0043BD02/charge left-press
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
    /// // arm: 0x0043BD67/autocalc left-press
    pub fn press_autocalc(&mut self) -> Option<usize> {
        if self.choice_owner != 1 {
            return None;
        }
        Some(9)
    }

    /// What the two confirm boxes do when they are answered yes: `FUN_0043BE65`
    /// (`0x0043BE65`), and the return to the
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
    /// // arm: 0x0043BF07/begin-drag left-press
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
    /// **`Battle_ClassifyDrag` runs first, and kind 0 declines.** The held
    /// branch of `FUN_0043BF07` is
    /// `iVar1 = Battle_ClassifyDrag(); if (iVar1 == 0) return 0; else if
    /// (g_mouseInputChanged == 0) return 1; else commit(0)`. So while the
    /// gesture is still under 25 pixels on both axes and over nobody, a drag in
    /// flight **touches the selection at all** — a player's hand shake between
    /// the press and the release leaves what he already holds standing. Ours
    /// re-boxed on every pixel of motion, which cleared the selection under the
    /// press and left the release with nothing to order: the move order a
    /// player aimed at empty ground went missing unless his mouse never moved.
    ///
    /// // arm: 0x0043BF07/drag-update drag
    pub fn drag_to(&mut self, x: i32, y: i32) -> bool {
        let Some(mut d) = self.drag else { return false };
        let moved = d.px != (x, y);
        d.px = (x, y);
        self.drag = Some(d);
        // The two declines differ in the original (`00430000.c:6944-6951`) and
        // ours must too: kind 0 returns **0**, so the arm behind gets the
        // motion; an unchanged input returns **1**, consumed and uncommitted.
        // `!moved` stands in for `g_mouseInputChanged`, which is any input at
        // all and not only the pointer. `[D]`
        if self.drag_kind(d, x, y) == DragKind::Nothing {
            return false;
        }
        if !moved {
            return true;
        }
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
    /// anchor and the pointer, so a click selects a 16-pixel square and
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
    /// // arm: 0x0043BF07/commit-drag left-release
    pub fn release_field(&mut self, x: i32, y: i32) -> bool {
        let Some(d) = self.drag.take() else { return false };
        self.mode = Mode::Field;
        self.redraw = true;
        let picked = match self.drag_kind(d, x, y) {
            DragKind::Box => {
                // **`if (DAT_00553078 != 0)` wraps the whole kind-1 commit** —
                // `Battle_DragSelect` (`0x00430000`, `00430000.c:6906`). With
                // nothing held at the release the box commits **nothing**: no
                // clear, no re-box, and (the regroup living inside
                // `Battle_CommitSelection`'s `commit == 1`,
                // `00430000.c:7009`) no regroup, no narrow, no recount. It
                // still returns 1, so the order arm behind never sees it. The
                // count is the one standing at the release — the held branch
                // re-boxes live, but only for a gesture that already classified
                // as something. `[V]`
                if self.runner.selected_count(self.owner) == 0 {
                    return true;
                }
                let (lo, hi) = self.box_corners(d.anchor_px, (x, y));
                self.runner.pick_box(self.owner, lo, hi, true);
                false
            }
            DragKind::Pick => {
                // `DAT_0057A0F4 -= 8; DAT_0057A0E8 -= 8; g_mouseX += 8;
                //  g_mouseY += 8;` then the same commit.
                let a = (d.anchor_px.0 - 8, d.anchor_px.1 - 8);
                let b = (x + 8, y + 8);
                let (lo, hi) = self.box_corners(a, b);
                self.runner.pick_box(self.owner, lo, hi, true);
                true
            }
            // Declined, so the order arm behind this one gets the release.
            DragKind::Nothing => return false,
        };
        self.current_unit = self.runner.regroup_selection(self.owner);
        // **The men answer.** Both committing arms call `Sound_PlayTroopCry(0)`
        // after the commit, so the troop is the new selection's. The box arm
        // guards it on `DAT_00553078 != 0` — something is held — and the pick
        // arm does not guard it at all. `[V]`
        if picked {
            // sfx: Battle_DragSelect#2
            self.cry(cry::SELECTED);
        } else if self.runner.selected_count(self.owner) != 0 {
            // sfx: Battle_DragSelect#1
            self.cry(cry::SELECTED);
        }
        true
    }

    /// `FUN_0043BF07`'s **double-click** clause, and the reason
    /// `crate::input`'s claim that `Village_DoubleClick` is the only reader of
    /// `g_mouseLeftDoubleClick` was false.
    ///
    /// The test is `(g_mouseLeftReleased || g_mouseLeftDoubleClick) &&
    /// g_screenId == 0x2A`, so a double click **commits an open drag exactly
    /// as a release would**. It exists because Windows
    /// sends `WM_LBUTTONDBLCLK` instead of the second `WM_LBUTTONDOWN`, so
    /// without this clause the second click of a fast double click would leave
    /// the drag open for ever.
    ///
    /// // arm: 0x0043BF07/double-click-commits double-click
    pub fn double_click_field(&mut self, x: i32, y: i32) -> bool {
        if self.mode != Mode::Drag {
            return false;
        }
        self.release_field(x, y)
    }

    /// `FUN_0043C57D` (`0x0043C57D`) → `FUN_0043C634` — **the order**.
    ///
    /// Five guards, all of them refusals
    /// be on the field, it must **not** be over one of your own men (either
    /// hover flag blocks it, which is what makes clicking a friend a selection
    /// and never a destination), the button must have been *released*, something
    /// must be selected, and the battle must not be paused.
    ///
    /// // arm: 0x0043C57D/order left-release
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
        // **`FUN_0043C634` cries before it orders**, from the hover it was
        // handed: an enemy under the pointer first, then surface 2, then
        // anything else. `[V]`
        if self.hover.enemy.is_some() {
            // sfx: Battle_OrderSelection#3
            self.cry(cry::ATTACK);
        } else if self.hover.surface == cry::MOAT_SURFACE {
            // sfx: Battle_OrderSelection#1
            self.cry(cry::MOAT);
        } else {
            // sfx: Battle_OrderSelection#2
            self.cry(cry::ORDERED);
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
    /// // arm: 0x0043C2A9/banner-drop left-press
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
    /// // arm: 0x0043C2A9/right-deselect right-release
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
    /// // arm: 0x00432443/overview left-press
    pub fn click_overview(&mut self, x: i32, y: i32, right: bool) -> bool {
        if !OVERVIEW.contains(x, y) {
            return false;
        }
        let cell = (((x - OVERVIEW.x) / 2) as u8, ((y - OVERVIEW.y) / 2) as u8);
        let orderable = !right
            && self.runner.selected_count(self.owner) > 0
            && !self.oil_selected()
            && !self.paused;
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
    /// // arm: 0x0043C885/store-group key
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
    /// // arm: 0x0043C910/recall-group key
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
    /// **It acts while the battle is paused.** `[V]`, from both of its guards:
    /// the window procedure's `WM_CHAR` arm asks `g_battlePhase == 2 &&
    /// DAT_0057A0CC == 0`, and `FUN_0043C77A` itself asks `DAT_00553C6C == 0 &&
    /// g_appPhase == 3`. Neither is the pause word, `DAT_0053F238`, which
    /// `Battle_OrderClicked` and `BattleMap_Click` do test — so a click cannot
    /// order a paused battle and `H` can, and a player can set a line or a
    /// column before the fighting starts. This used to refuse while paused, a
    /// guard of ours copied from the click.
    ///
    /// // arm: 0x0043C77A/formation key
    pub fn key_formation(&mut self, formation: Formation) -> bool {
        // **The cry comes first**, with no unit to turn and while paused:
        // `Sound_PlayTroopCry(1)` is the first statement under the two guards.
        // `[V]`.
        // sfx: Battle_FormationKey#1
        self.cry(cry::ORDERED);
        // Not a guard of the original's: it orders `DAT_0053E984` whatever it
        // holds. Unit 0 is no unit, and `BattleRunner::order_formation` refuses
        // it too; this only keeps "nothing was turned" out of the redraw.
        if self.current_unit == 0 {
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
    /// // arm: 0x00477DFC/outcome-timer timer
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
    /// // arm: 0x0042FF10/skip-outcome right-release
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



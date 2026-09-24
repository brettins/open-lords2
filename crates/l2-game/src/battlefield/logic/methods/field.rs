#![allow(unused_imports)]
use super::*;
use super::view::*;
use super::commands::*;
use super::groups::*;
use super::super::*;
use super::super::*;
use super::super::view::*;
use tests::*;
use l2_sim::runner::{BattleRunner, Conclusion, Formation};
use l2_sim::terrain::DIM;
use crate::input::Rect;

impl LiveBattle {
    /// `FUN_0043BF07` (`0x0043BF07`), the press half: **begin a drag**.
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
        //
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
        self.act(BattleOp::Pick { lo, hi, commit: false });
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
    /// * a real box commits it with `FUN_0043C247(player, 1, …)`, which clears,
    ///   boxes, **regroups the units** and narrows;
    /// * a click on one of your own men nudges the box out by eight pixels in
    ///   each direction and commits *that* — the original literally rewrites the
    /// anchor and the pointer, so a click selects a 16-pixel square and
    ///   therefore usually one man;
    /// * a click on nothing does **nothing**. It does not clear the selection.
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
                let follow = Follow::Cry { class: cry::SELECTED, if_held: true };
                self.act_then(BattleOp::Pick { lo, hi, commit: true }, follow);
                false
            }
            DragKind::Pick => {
                // `DAT_0057A0F4 -= 8; DAT_0057A0E8 -= 8; g_mouseX += 8;
                //  g_mouseY += 8;` then the same commit.
                let a = (d.anchor_px.0 - 8, d.anchor_px.1 - 8);
                let b = (x + 8, y + 8);
                let (lo, hi) = self.box_corners(a, b);
                let follow = Follow::Cry { class: cry::SELECTED, if_held: false };
                self.act_then(BattleOp::Pick { lo, hi, commit: true }, follow);
                true
            }
            DragKind::Nothing => return false,
        };
        // **The men answer.** Both committing arms call `Sound_PlayTroopCry(0)`
        // after the commit, so the troop is the new selection's. The box arm
        // guards it on `DAT_00553078 != 0` — something is held — and the pick
        // arm does not guard it at all. `[V]`
        if self.deferred {
            return true;
        }
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
    /// // arm: 0x0043BF07/double-click-commits double-click
    pub fn double_click_field(&mut self, x: i32, y: i32) -> bool {
        if self.mode != Mode::Drag {
            return false;
        }
        self.release_field(x, y)
    }

    /// `FUN_0043C57D` (`0x0043C57D`) → `FUN_0043C634` — **the order**.
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
        let cell = self.cell_at(x, y);
        self.order_selection(cell)
    }

    /// Battle_OrderSelection 0x0043C634, from the field and from BattleMap_Click 0x00432443
    /// (`00430000.c:241`); both hand it the hover of the moment. [V]
    fn order_selection(&mut self, cell: (u8, u8)) -> bool {
        l2_net::trace!("call", 0x0043C634, "Battle_OrderSelection", at = format!("{cell:?}"));
        // **`FUN_0043C634` cries before it orders**, from the hover it was
        // handed: an enemy under the pointer first, then surface 2, then
        // anything else. `[V]`
        let class = if self.hover.enemy.is_some() {
            // sfx: Battle_OrderSelection#3
            cry::ATTACK
        } else if self.hover.surface == cry::MOAT_SURFACE {
            // sfx: Battle_OrderSelection#1
            cry::MOAT
        } else {
            // sfx: Battle_OrderSelection#2
            cry::ORDERED
        };
        if !self.deferred {
            self.cry(class);
        }
        self.mark_order(cell);
        let target = self.hover.enemy;
        let woodland = self.hover.woodland;
        self.act_then(BattleOp::Order { cell, target, woodland }, Follow::Cry { class, if_held: false })
    }

    /// `FUN_0043C2A9` (`0x0043C2A9`), the left half: **a click on a banner drops
    /// that figure from the selection**.
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
                self.act(BattleOp::Drop { figure: fig });
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
    /// // arm: 0x0043C2A9/right-deselect right-release
    pub fn right_deselect(&mut self, x: i32, y: i32) -> bool {
        if x >= 0x1E1 && (0x18..=0xB7).contains(&y) {
            return false;
        }
        self.act(BattleOp::Clear);
        true
    }


    /// `BattleMap_Click` (`0x00432443`) — the 160 × 160 panel at two pixels a
    /// cell.
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
            if !self.order_selection(cell) {
                self.current_unit = 0;
            }
        } else {
            self.look_at(cell);
        }
        self.redraw = true;
        true
    }

    /// Battle_OrderSelection 0x0043C634: `DAT_00553F84 = 0x1E` (10 in multiplayer),
    /// `DAT_00553F14 = x; DAT_00553F18 = y;` before BattleUnit_Order. [V]
    fn mark_order(&mut self, cell: (u8, u8)) {
        self.order_mark = cell;
        self.order_mark_ticks = if self.deferred { 10 } else { 0x1E };
    }
}

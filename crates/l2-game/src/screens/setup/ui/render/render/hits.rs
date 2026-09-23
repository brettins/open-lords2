#![allow(unused_imports)]
use super::*;
use super::layout::*;
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
    pub(crate) fn hotspots(&self) -> Vec<(Rect, Action)> {
        let mut v = Vec::new();
        match self.page {
            SetupPage::Title => {
                for (i, _) in TITLE_ITEMS.iter().enumerate() {
                    v.push((item_rect(i), Action::Item(i)));
                }
            }
            SetupPage::Options => {
                for (i, _) in OPTION_ITEMS.iter().enumerate() {
                    v.push((item_rect(i), Action::Item(i)));
                }
            }
            SetupPage::Shield => {
                for i in 0..5 {
                    v.push((
                        Rect::new(SHIELD_X + i * SHIELD_STEP, SHIELD_Y, SHIELD_W, SHIELD_H),
                        Action::Item(i as usize),
                    ));
                }
                for (i, (x, y, _)) in SHIELD_BUTTONS.iter().enumerate() {
                    v.push((Rect::new(*x, *y, ITEM_W, ITEM_H), Action::Item(5 + i)));
                }
            }
            SetupPage::Campaign | SetupPage::GameType => {
                for (i, x) in PAIR_X.iter().enumerate() {
                    v.push((Rect::new(*x, PAIR_Y, PAIR_W, ITEM_H), Action::Item(i)));
                }
            }
            SetupPage::Custom | SetupPage::CustomMulti => {
                for (i, &(x, y, _)) in OPTION_CELLS.iter().enumerate() {
                    v.push((Rect::new(x, y, OPTION_BOX_W, OPTION_BOX_H), Action::Open(i)));
                }
                for row in 0..MAP_LIST_ROWS {
                    let y = MAP_LIST_Y + row as i32 * MAP_LIST_ROW;
                    v.push((
                        Rect::new(MAP_LIST_X, y, MAP_LIST_W, MAP_LIST_ROW),
                        Action::Map(row),
                    ));
                }
                let n = if self.page == SetupPage::Custom { 3 } else { 4 };
                for (i, (x, _)) in CUSTOM_BUTTONS.iter().take(n).enumerate() {
                    // [I] The painter centres the caption in 76 pixels at
                    // y = 0xC6 and registers no rectangle of its own; the
                    // hit box is that caption's box, four pixels above it.
                    v.push((
                        Rect::new(*x, CUSTOM_BUTTON_Y - 4, CUSTOM_BUTTON_W, ITEM_H),
                        Action::Item(i),
                    ));
                }
            }
            SetupPage::Dropdown => {
                let n = self.rows(self.open);
                let (x, y, _) = OPTION_LIST[self.open];
                let y = self.dropdown_y(y, n);
                for i in 0..n {
                    v.push((
                        Rect::new(x, y + 16 + i as i32 * 16, OPTION_BOX_W, 16),
                        Action::Choose(i),
                    ));
                }
            }
            SetupPage::SkirmishFile => self.skirmish_file_arms(&mut v),
            // Screen_HandleInput 0x004BA9C8, 004b0000.c:7355-7362: page 3 is the box's table. [V]
            SetupPage::Load => {}
            SetupPage::NoCd => {
                v.push((Rect::new(0, 0, 640, 480), Action::Item(0)));
            }
            SetupPage::Skirmish | SetupPage::SkirmishMulti => self.skirmish_arms(&mut v),
        }
        v
    }

    /// `FUN_0041FDD6` shifts the *Nobles* drop-down up by one row per item so
    /// that a list opened from the bottom row of the grid still fits on the
    /// screen. It is the only option that gets the treatment.
    ///
    /// **`rows` is the item count, `DAT_00553FB4`** — the same number the
    /// painter loops over, not the geometry table's count-plus-two. That
    /// matters now that the count can be shortened: on a map that seats three
    /// lords the list is two rows and rides two rows lower
    /// original's does, because both read the one variable.
    pub(crate) fn dropdown_y(&self, y: i32, rows: usize) -> i32 {
        if self.open == crate::setup::option::NOBLES {
            y - (rows as i32 - 1) * 16
        } else {
            y
        }
    }

    pub(crate) fn at(&self, x: i32, y: i32) -> Option<(usize, Action)> {
        self.hotspots()
            .into_iter()
            .enumerate()
            .find(|(_, (r, _))| r.contains(x, y))
            .map(|(i, (_, a))| (i, a))
    }

    pub(crate) fn count(&self) -> usize {
        self.hotspots().len()
    }

    pub(crate) fn activate(&mut self, ctx: &mut Ctx) -> Transition {
        let Some(&(_, action)) = self.hotspots().get(self.selected) else {
            return Transition::Stay;
        };
        self.act(action, ctx)
    }

    /// The page graph, read out of `FUN_00432B05` (page 1) and `FUN_00432CC8`
    /// (page 2).
    ///
    /// **[D], ** Those
    /// handlers branch on `g_uiHotspotId`, and the ids do not run in the order
    /// the painter draws the items: on page 1 id 3 sets the quit flag and id 4
    /// plays `lom.smk`, while the painter draws *"Lords of Magic?"* third and
    /// *"Exit game"* fourth. **The table says why** — `node
    /// tools/oracle/widgets.js widgets 4dcb48 4` gives its third record hotspot
    /// id 4 and its fourth id 3, so the records are in drawing order and the
    /// ids are not, and both readings were right. The
    /// destinations below are keyed to the **captions**, which are [V], not to
    /// the ids.
    pub(crate) fn act(&mut self, action: Action, ctx: &mut Ctx) -> Transition {
        match action {
            Action::Item(i) => self.item(i, ctx),
            Action::Open(i) => {
                self.under = self.page;
                self.open = i;
                self.page = SetupPage::Dropdown;
                self.selected = self.options.get(i);
                Transition::Stay
            }
            Action::Choose(v) => {
                // `FUN_00433A23`: `Setup_SetOption(open, row - 1)`, then back to
                // the page underneath.
                self.options.set(self.open, v);
                self.page = self.under;
                self.selected = 0;
                Transition::Stay
            }
            Action::Map(row) => {
                // `FUN_00433905`: the row sets `g_scenarioIndex`, the planes are
                // loaded, and the seat count that comes out of them sets
                // *Nobles*. All three, or the lord count is left claiming a
                // number the new map cannot seat.
                self.map = (self.map_top + row).min(MAP_COUNT - 1);
                self.read_map(ctx);
                Transition::Stay
            }
            Action::Skirmish(arm) => self.skirmish_arm(arm, ctx),
        }
    }

    /// Page 12's five arms and page 13's one, each of which ends the same way
    /// in the original: `Skirmish_FillArmies` (`0x0042BF46`) and a redraw.
    fn skirmish_arm(&mut self, arm: SkirmishArm, ctx: &Ctx) -> Transition {
        match arm {
            SkirmishArm::Row(r) => {
                self.skirmish.pick_row(r);
            }
            SkirmishArm::Scroll(d) => self.skirmish.scroll(d),
            SkirmishArm::FileScroll(d) => self.scroll_skirmish_files(d),
            SkirmishArm::Count(col, row) => self.custom_count(col, row),
            SkirmishArm::Cycle(realm) => self.skirmish.cycle_difficulty(realm),
            SkirmishArm::Kind(k) => self.skirmish.choose_kind(k),
            // `00430000.c:8120-8126` — the move of `DAT_0053EF5C` and then
            // `Troops_Load`, which is the whole point of the arm: the table
            // the page shows comes out of a different file on each side.
            SkirmishArm::Sides => {
                self.skirmish.swap_sides();
                self.load_troops(ctx);
            }
            SkirmishArm::Handicap(h) => self.skirmish.handicap(h),
            // `FUN_0043DDF4` opens page 13 only on `0 < DAT_005653E8`
            // (`00430000.c:8140`) — the count of `.skr` files found. With
            // none, the field is not a button.
            SkirmishArm::OpenFiles => {
                if self.skirmish_files.is_empty() {
                    return Transition::Stay;
                }
                return self.go(SetupPage::SkirmishFile);
            }
            SkirmishArm::File(r) => {
                // `FUN_00434174` ignores a row past the end of the list, and
                // the row that is the name already chosen **returns 0 and
                // stays on page 13** (`00430000.c:1364-1367`) — only a new
                // name writes `g_setupPage`.
                let name = self.skirmish_files.get(self.skirmish_file_top + r).cloned();
                if let Some(name) = name {
                    if self.skirmish.choose_file(&name) {
                        // `00430000.c:1372` — `FUN_0042D4AA` on the way out.
                        self.load_skr(ctx, &name);
                        return self.go(SetupPage::Skirmish);
                    }
                }
            }
        }
        Transition::Stay
    }

}

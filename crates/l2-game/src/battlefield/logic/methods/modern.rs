//! The Modern scheme's input (C272, `docs/battle-controls.md` §3, §4, §6). `[I]` ours: the original
//! has one scheme, `Screen_FrameInput`'s `0x29` arms, which Classic still runs untouched. Selection is
//! a list of battle-unit ids on the seat: presentation, never sent, `figure.selected` never written.
//! Every order is one canonical [`BattleOp::Command`].

#![allow(unused_imports)]
use super::*;
use super::view::*;
use super::commands::*;
use super::field::*;
use super::groups::*;
use super::super::*;
use super::super::view::*;
use tests::*;
use l2_sim::runner::{BattleRunner, Conclusion, Formation};
use l2_sim::terrain::DIM;
use crate::game::BattleControls;
use crate::input::{Key, Rect};

impl LiveBattle {
    pub fn is_modern(&self) -> bool {
        self.controls == BattleControls::Modern
    }

    /// The scheme is read once, when the battle opens, and a Modern seat tells the sim so.
    pub fn latch_controls(&mut self, controls: BattleControls) {
        if !self.latched {
            self.latched = true;
            self.controls = controls;
        }
    }

    /// An empty `Command`: this realm's units answer a touch, a hit or a near enemy (Q271 `sc2`).
    pub(crate) fn announce_modern(&mut self) {
        if self.is_modern() && !self.announced {
            self.announced = true;
            self.act(BattleOp::Command { units: vec![], verb: Verb::Stop, at: (0, 0), target: None, queue: false });
        }
    }

    /// The men this seat holds: Classic's picked figures, or every man of the selected units.
    pub fn picked_fighters(&self) -> Vec<usize> {
        if !self.is_modern() {
            return self.runner.selected_fighters(self.owner);
        }
        (0..self.runner.fighters.len())
            .filter(|&f| {
                let fig = &self.runner.sim.figures[self.runner.fighters[f].sim];
                fig.is_alive() && fig.owner == self.owner && self.sel.contains(&(fig.unit as u8))
            })
            .collect()
    }

    pub fn picked_count(&self) -> usize {
        if self.is_modern() { self.picked_fighters().len() } else { self.runner.selected_count(self.owner) }
    }

    /// One tile per picked figure (Classic), or per selected unit with its men (Modern):
    /// `(a figure to draw, men)`. `FUN_0043C2A9`'s column.
    pub fn banner_tiles(&self) -> Vec<(usize, u32)> {
        let men = |f: usize| u32::from(self.runner.sim.figures[self.runner.fighters[f].sim].men);
        if !self.is_modern() {
            return self.runner.selected_fighters(self.owner).into_iter().map(|f| (f, men(f))).collect();
        }
        let held = self.picked_fighters();
        self.live_sel()
            .into_iter()
            .filter_map(|u| {
                let own: Vec<usize> = held.iter().copied().filter(|&f| self.runner.unit_of(f) == u as usize).collect();
                Some((*own.first()?, own.iter().map(|&f| men(f)).sum()))
            })
            .collect()
    }

    pub(super) fn live_sel(&self) -> Vec<u8> {
        let live = |u: &u8| {
            let unit = self.runner.units.get(*u as usize);
            unit.is_live() && unit.owner == self.owner
        };
        self.sel.iter().copied().filter(live).collect()
    }

    fn unit_of_man(&self, f: usize) -> Option<u8> {
        let fig = &self.runner.sim.figures[self.runner.fighters.get(f)?.sim];
        (fig.is_alive() && fig.owner == self.owner && (1..=80).contains(&fig.unit)).then_some(fig.unit as u8)
    }

    /// Every unit with a man on a cell of the box.
    fn units_in(&self, lo: (u8, u8), hi: (u8, u8)) -> Vec<u8> {
        let mut found = Vec::new();
        for y in lo.1..=hi.1 {
            for x in lo.0..=hi.0 {
                let unit = self.runner.occupant_of(x, y).and_then(|f| self.unit_of_man(f));
                if let Some(u) = unit.filter(|u| !found.contains(u)) {
                    found.push(u);
                }
            }
        }
        found
    }

    /// `LeftPress`: attack-move when armed, else the start of a click or a box. Classic's drag record.
    pub fn m_press(&mut self, x: i32, y: i32, shift: bool) -> bool {
        if self.mode != Mode::Field || !VIEW.contains(x, y) {
            return false;
        }
        self.last_group = None;
        if std::mem::take(&mut self.armed) {
            let cell = self.cell_at(x, y);
            return self.issue(Verb::AttackMove, cell, None, shift);
        }
        self.press_field(x, y)
    }

    /// The box only follows the pointer; nothing is sent while it is drawn.
    pub fn m_drag_to(&mut self, x: i32, y: i32) -> bool {
        let Some(d) = self.drag.as_mut() else { return false };
        d.px = (x, y);
        self.redraw = true;
        true
    }

    /// A click picks the whole unit under the pointer, a box every unit with a man inside it.
    /// Empty ground deselects (SC2). `[I]` ours, C272.
    pub fn m_release(&mut self, x: i32, y: i32, shift: bool) -> bool {
        let Some(d) = self.drag.take() else { return false };
        self.mode = Mode::Field;
        self.redraw = true;
        let boxed = (d.anchor_px.0 - x).abs() >= DRAG_SLOP || (d.anchor_px.1 - y).abs() >= DRAG_SLOP;
        let found = if boxed {
            let (lo, hi) = self.box_corners(d.anchor_px, (x, y));
            self.units_in(lo, hi)
        } else {
            let under = self.hover.friendly.or(self.hover.friendly_picked);
            under.and_then(|f| self.unit_of_man(f)).into_iter().collect()
        };
        let mut sel = self.live_sel();
        if !shift {
            sel.clear();
        }
        for u in found.iter().copied() {
            match sel.iter().position(|&s| s == u) {
                Some(i) if shift && !boxed => {
                    sel.remove(i);
                }
                Some(_) => {}
                None => sel.push(u),
            }
        }
        self.sel = sel;
        if !found.is_empty() && !self.deferred {
            self.cry(cry::SELECTED);
        }
        true
    }

    /// Double-click: every unit of the clicked troop type in the view. `[I]` ours, C272.
    pub fn m_double_click(&mut self, shift: bool) -> bool {
        let Some(f) = self.hover.friendly.or(self.hover.friendly_picked) else { return false };
        let troop = self.runner.fighters[f].troop;
        let cam = self.cam;
        let on_screen = |fi: usize| {
            let m = &self.runner.fighters[fi];
            (cam.0..cam.0 + VIEW_COLS).contains(&(m.x as i32)) && (cam.1..cam.1 + VIEW_ROWS).contains(&(m.y as i32))
        };
        let mut sel = if shift { self.live_sel() } else { Vec::new() };
        for fi in 0..self.runner.fighters.len() {
            if self.runner.fighters[fi].troop == troop && on_screen(fi) {
                if let Some(u) = self.unit_of_man(fi).filter(|u| !sel.contains(u)) {
                    sel.push(u);
                }
            }
        }
        self.sel = sel;
        self.redraw = true;
        true
    }

    /// F2: the whole army.
    pub fn select_all(&mut self) -> bool {
        self.sel = (1..=80u8).filter(|&u| self.runner.units.get(u as usize).owner == self.owner).collect();
        self.redraw = true;
        true
    }

    /// A tile in the right column: Shift drops that unit, plain keeps only it.
    pub fn m_click_banner(&mut self, x: i32, y: i32, shift: bool) -> bool {
        let tiles = self.banner_tiles();
        let layout = BannerLayout::for_count(tiles.len());
        let Some(slot) = (0..tiles.len().min(layout.slots)).find(|&s| layout.rect(s).contains(x, y)) else {
            return false;
        };
        let unit = self.runner.unit_of(tiles[slot].0) as u8;
        self.sel = if shift { self.live_sel().into_iter().filter(|&u| u != unit).collect() } else { vec![unit] };
        self.redraw = true;
        true
    }

    /// The overview: left looks (or attack-moves when armed), right commands.
    pub fn m_overview(&mut self, x: i32, y: i32, right: bool, shift: bool) -> bool {
        if !OVERVIEW.contains(x, y) {
            return false;
        }
        let cell = (((x - OVERVIEW.x) / 2) as u8, ((y - OVERVIEW.y) / 2) as u8);
        if right {
            self.issue(Verb::Move, cell, None, shift);
        } else if std::mem::take(&mut self.armed) {
            self.issue(Verb::AttackMove, cell, None, shift);
        } else {
            self.look_at(cell);
        }
        self.redraw = true;
        true
    }

    /// Right on the field: move, or attack the enemy under the pointer. Armed, it only disarms.
    pub fn m_right_click(&mut self, x: i32, y: i32, shift: bool) -> bool {
        self.last_group = None;
        if self.m_overview(x, y, true, shift) {
            return true;
        }
        if !VIEW.contains(x, y) || !self.hover.on_field {
            return false;
        }
        if std::mem::take(&mut self.armed) {
            return true;
        }
        let (cell, target) = (self.cell_at(x, y), self.hover.enemy);
        self.issue(Verb::Move, cell, target, shift)
    }

    /// One `BattleOp::Command` for the selection. `false` when nothing is held.
    pub(super) fn issue(&mut self, verb: Verb, at: (u8, u8), target: Option<usize>, queue: bool) -> bool {
        let units = self.live_sel();
        if units.is_empty() {
            return false;
        }
        // `Battle_OrderSelection` 00430000.c:7296-7297: an enemy under the pointer cries attack, an order
        // onto surface 2 (the moat) cries `_M`, else the plain order cry. The ordered cell's own surface.
        let moat = matches!(verb, Verb::Move | Verb::AttackMove)
            && self.runner.field.at(at.0 as usize, at.1 as usize).surface == cry::MOAT_SURFACE;
        let class = match (target.is_some(), moat) {
            (true, _) => cry::ATTACK,
            (false, true) => cry::MOAT,
            (false, false) => cry::ORDERED,
        };
        if !self.deferred {
            self.cry(class);
        }
        if matches!(verb, Verb::Move | Verb::AttackMove) {
            self.mark_order(at);
        }
        self.act_then(BattleOp::Command { units, verb, at, target, queue }, Follow::Cry { class, if_held: false });
        self.redraw = true;
        true
    }
}

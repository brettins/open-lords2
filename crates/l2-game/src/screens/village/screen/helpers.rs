#![allow(unused_imports)]
use super::*;
use super::screen::*;
use super::*;
use l2_kingdom::county::County;
use l2_kingdom::tables::{JOB_IDLE_TOWNSFOLK, JOB_NAMES};
use l2_view::village::{self as vill, ICONS_PER_CLUSTER};
use l2_view::{text, Canvas};
use crate::input::{Event, Key, Rect};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::widget;

impl VillageScreen {
    pub fn new(county: u8) -> VillageScreen {
        VillageScreen {
            county,
            phase: Phase::Idle,
            anchor: None,
            pointer: (0, 0),
            drag_cluster: 0,
            selected: [false; ICONS_PER_CLUSTER],
            drag_count: 0,
            pending_click: None,
            status: String::new(),
            clock: vill::AnimationClock::new(),
            animated: false,
        }
    }

    pub fn county(&self) -> u8 {
        self.county
    }

    pub fn phase(&self) -> Phase {
        self.phase
    }

    pub fn drag_cluster(&self) -> usize {
        self.drag_cluster
    }

    pub fn drag_count(&self) -> i32 {
        self.drag_count
    }

    /// `Ui_OkButton(0x180, g_villageTopY + 0x118, 1)`.
    pub fn ok_button(top: i32) -> Rect {
        Rect::new(vill::OK_X, top + vill::OK_DY, l2_view::chrome::system::OK_DIM, l2_view::chrome::system::OK_DIM)
    }

    /// `g_villageTopY`: 64, or 132 with *Advanced Farming*, which is what makes
    /// room for `villtops.pl8` above the scene.
    pub fn top_y(ctx: &Ctx) -> i32 {
        if ctx.game.kingdom.options.advanced_farming {
            vill::SCENE_Y_ADVANCED
        } else {
            vill::SCENE_Y
        }
    }

    fn county_ref<'a>(&self, ctx: &'a Ctx) -> Option<&'a County> {
        ctx.game.kingdom.counties.get(self.county as usize)
    }

    /// Which labour slot each of the eight clusters is, for this county.
    pub fn slots(c: &County) -> [usize; vill::CLUSTER_COUNT] {
        let (quarry, mine) = Self::resources(c);
        let mut out = [0usize; vill::CLUSTER_COUNT];
        for (cluster, slot) in out.iter_mut().enumerate() {
            *slot = vill::slot_for_cluster(cluster, quarry, mine);
        }
        out
    }

    /// `+0x2DD` and `+0x2AD` — the quarry and the mine, which are commodity
    /// records 3 and 1. **The resource comes from the map**, not from the
    /// county record: `County_PlaceResourceSites` reads plane 2 on `Town`-bank
    /// tiles at load and sets these bytes.
    fn resources(c: &County) -> (bool, bool) {
        (c.industry[3].has_resource, c.industry[1].has_resource)
    }

    /// All four `has_resource` bytes, in commodity order, for the three
    /// buildings [`l2_view::village::VillageArt::draw_resources`] paints.
    pub fn resource_flags(c: &County) -> [bool; 4] {
        [
            c.industry[0].has_resource,
            c.industry[1].has_resource,
            c.industry[2].has_resource,
            c.industry[3].has_resource,
        ]
    }

    /// `Village_RebuildIcons` for the whole village.
    pub fn icons(c: &County) -> [ClusterIcons; vill::CLUSTER_COUNT] {
        let slots = Self::slots(c);
        let mut out = [[0u8; ICONS_PER_CLUSTER]; vill::CLUSTER_COUNT];
        for (cluster, icons) in out.iter_mut().enumerate() {
            let slot = slots[cluster];
            let (main, other) = vill::icon_counts(
                c.labour[slot],
                c.labour_wanted[slot],
                c.labour_useful[slot],
                c.pop_band,
            );
            *icons = vill::cluster_icons(cluster, vill::ICON_VALUE[slot], main, other);
        }
        out
    }

    /// The band's rectangle, as two inclusive corners in either order — which
    /// is how `Village_BoxSelect` normalises it.
    pub fn band(&self) -> Option<(i32, i32, i32, i32)> {
        let (ax, ay) = self.anchor?;
        let (px, py) = self.pointer;
        Some((ax.min(px), ay.min(py), ax.max(px), ay.max(py)))
    }

    /// **`Village_DrawBand` (`0x00412795`)** — the rubber band's own outline,
    /// clamped the way the original clamps it.
    ///
    /// The whole body is `if (screen == 0x05)`: the guard reads
    /// `g_screenId == 5 || g_screenId == 6 || g_screenId == 2` and then
    /// `g_screenId != 2 && (FUN_004120E0(), g_screenId != 6)`, so `0x02` and
    /// `0x06` fall out and only the band state draws. Then, with
    /// `(x0, w)` and `(y0, h)` normalised from `DAT_005530FC` / `DAT_005530F4`
    /// and the live pointer:
    ///
    /// ```c
    /// if      (x0 < 0)                  { w += x0; x0 = 0; }
    /// else if (0x1FF < x0 + w)          { w = 0x200 - x0; }
    /// if      (y0 < g_villageTopY)      { h -= g_villageTopY - y0; y0 = g_villageTopY; }
    /// else if (g_villageTopY + 0x178 <= y0 + h) { h = (g_villageTopY + 0x178) - y0; }
    /// FUN_00403cf4(x0, y0, w, h, 0x20);
    /// ```
    ///
    /// Both branches are `else if`,
    /// never clamped on the right. That is the original's, kept.
    ///
    /// `FUN_00403CF4` is the four-line rectangle outline — it draws top,
    /// bottom, left and right through `FUN_00403A8F` in one colour — and the
    /// colour here is the literal `0x20`, which is `rgb(255, 255, 255)` in
    /// `Base01.256`, the palette `Screen_DrawCampaign` leaves set.
    fn band_rect(&self, top: i32) -> Option<Rect> {
        let (x0, y0, x1, y1) = self.band()?;
        let (mut x, mut w) = (x0, x1 - x0 + 1);
        let (mut y, mut h) = (y0, y1 - y0 + 1);
        if x < 0 {
            w += x;
            x = 0;
        } else if x + w > vill::BAND_X_MAX {
            w = vill::BAND_X_MAX + 1 - x;
        }
        if y < top {
            h -= top - y;
            y = top;
        } else if y + h >= top + vill::BAND_H {
            h = top + vill::BAND_H - y;
        }
        (w > 0 && h > 0).then(|| Rect::new(x, y, w, h))
    }

    /// `Village_BoxSelect` (`0x0043958A`).
    // arm: 0x0043958A/box-select drag
    pub(super) fn box_select(&mut self, ctx: &Ctx) {
        self.selected = [false; ICONS_PER_CLUSTER];
        self.drag_cluster = 0;
        self.drag_count = 0;
        let Some(c) = self.county_ref(ctx) else { return };
        let Some((x0, y0, x1, y1)) = self.band() else { return };
        let top = Self::top_y(ctx);
        let icons = Self::icons(c);

        for cluster in 0..vill::CLUSTER_COUNT {
            let (bx0, by0, bx1, by1) = vill::cluster_band_box(cluster, top);
            if x0 > bx1 || bx0 > x1 || y0 > by1 || by0 > y1 {
                continue;
            }
            for slot in 0..ICONS_PER_CLUSTER {
                let value = icons[cluster][slot];
                // 0 is an empty slot and 1 is a worker the job wants and has
                // not got. Neither is a person you can pick up.
                if value == 0 || value == vill::ICON_SHORTFALL {
                    continue;
                }
                let (px, py) = vill::icon_hit_point(cluster, slot, top);
                if px < x0 || px > x1 || py < y0 || py > y1 {
                    continue;
                }
                if self.drag_cluster != 0 && self.drag_cluster != cluster + 1 {
                    // A band across two clusters selects nothing at all.
                    self.drag_cluster = 0;
                    self.selected = [false; ICONS_PER_CLUSTER];
                    self.drag_count = 0;
                    return;
                }
                self.drag_cluster = cluster + 1;
                self.selected[slot] = true;
                self.drag_count += 1;
            }
        }
    }

    fn clear_drag(&mut self) {
        self.phase = Phase::Idle;
        self.anchor = None;
        self.drag_cluster = 0;
        self.drag_count = 0;
        self.selected = [false; ICONS_PER_CLUSTER];
    }

    /// `FUN_004399B0`: the drop. Returns true if anything moved.
    fn drop_on(&mut self, ctx: &mut Ctx, x: i32, y: i32) -> bool {
        let top = Self::top_y(ctx);
        let Some(art) = ctx.assets.village.as_ref() else {
            self.status = "NO VILL_GD8.PL8 - NOTHING TO DROP ON".into();
            return false;
        };
        let target = art.cluster_at(x, y, top);
        if target == 0 {
            return false;
        }
        if target == self.drag_cluster {
            // Dropped where it came from: the original just redraws.
            self.clear_drag();
            self.status = "PUT BACK".into();
            return false;
        }
        let Some(c) = self.county_ref(&*ctx) else { return false };
        let slots = Self::slots(c);
        let (from, to) = (slots[self.drag_cluster - 1], slots[target - 1]);
        let icons = self.drag_count;
        let moved = ctx.game.move_labour(self.county, from, to, icons);
        self.clear_drag();
        self.status = if moved > 0 {
            format!("{moved} TO {}", JOB_NAMES[to].to_uppercase())
        } else {
            "NOBODY MOVED".into()
        };
        moved > 0
    }

    /// A click that never became a drag: the job popup for whatever cluster it
    /// landed on. `FUN_0043A123`, whose one refusal is cluster 0 in a county
    /// with neither a quarry nor a mine.
    fn job_under(&self, ctx: &Ctx, x: i32, y: i32) -> Option<usize> {
        let top = Self::top_y(ctx);
        let cluster = ctx.assets.village.as_ref()?.cluster_at(x, y, top);
        if cluster == 0 {
            return None;
        }
        let c = self.county_ref(ctx)?;
        let (quarry, mine) = Self::resources(c);
        vill::cluster_is_clickable(cluster - 1, quarry, mine)
            .then(|| vill::slot_for_cluster(cluster - 1, quarry, mine))
    }

    /// Whether a press here arms the band at all — `FUN_004393EB`'s guard.
    fn in_band_area(ctx: &Ctx, x: i32, y: i32) -> bool {
        let top = Self::top_y(ctx);
        (0..=vill::BAND_X_MAX).contains(&x) && y >= top && y < top + vill::BAND_H
    }

    /// `Village_DoubleClick` (`0x00439DF0`)'s own guard, which is **not**
    /// [`VillageScreen::in_band_area`]: the band arms from x 0, the double
    /// click only from x `0x40`, the left edge of the picture. Returns the
    /// cluster it landed on, 1-based, or `None`.
    fn double_click_cluster(&self, ctx: &Ctx, x: i32, y: i32) -> Option<usize> {
        let top = Self::top_y(ctx);
        if x < vill::SCENE_X || x > vill::BAND_X_MAX || y < top || y >= top + vill::BAND_H {
            return None;
        }
        let cluster = ctx.assets.village.as_ref()?.cluster_at(x, y, top);
        (cluster != 0).then_some(cluster)
    }

    /// The double click, once a cluster is known — `FUN_00439EDB`.
    fn balance(&mut self, ctx: &mut Ctx, cluster: usize) {
        let moved = if cluster == vill::IDLE_CLUSTER {
            ctx.game.balance_all_labour(self.county)
        } else {
            ctx.game.balance_labour(self.county, cluster, true)
        };
        self.status = if moved > 0 { format!("{moved} REASSIGNED") } else { "NOTHING TO DO".into() };
    }

    /// How many ticks a click waits before it counts as a click and not the
    /// first half of a double one.
    ///
    /// **The original's number is 300 milliseconds** — the frame poll at
    /// `0x004B2D5A` compares `timeGetTime() - DAT_004E59F8` against `300` and
    /// only then sets `DAT_004EABF0`, the flag `Village_ClickJob` opens the job
    /// popup on. Ours is in *ticks*, because nothing below `main.rs` is allowed
    /// to read a clock (`docs/netcode.md`); at the 16 ms tick that file fixes,
    /// nineteen ticks is 304 ms.
    ///
    /// It is why the job popup opens a fraction after the button comes up
    ///, and that delay is not an accident of ours: without it
    /// there is nowhere for the double click to happen.
    pub const CLICK_SETTLE_TICKS: u32 = 19;

    /// One fixed simulation tick in milliseconds — `main::TICK`, and the same
    /// constant `screens::map::TICK_MS` carries for the same reason.
    ///
    /// **A constant, not a clock.** [`l2_view::village::AnimationClock`] is
    /// told how long a tick is; it never asks how long one took.
    pub const TICK_MS: u32 = 16;

    /// Whether this event is one the village's arm hands to the sidebar.
    ///
    /// **Left button and pointer only.** The six guards are all left-button
    /// hit tests, and the right button is the village's own third way out
    /// (`g_mouseRightReleased` → `g_screenId = 0`), so it must not pass. The
    /// double click must not pass either: `Village_DoubleClick` is read in
    /// exactly one place in the whole binary and this arm is it.
    fn belongs_to_the_sidebar(event: Event) -> bool {
        // The six guards are the *same six* the four county panels open with,
        // so the predicate is one function for both:
        // [`crate::screens::belongs_to_the_right_column`].
        crate::screens::belongs_to_the_right_column(event)
    }
}

impl VillageScreen {
    /// The eight clusters, and the icons in them.
    fn draw_clusters(
        &self,
        ctx: &Ctx,
        canvas: &mut Canvas,
        c: &County,
        top: i32,
        have_scene: bool,
    ) {
        let ink = &ctx.assets.ink;
        let icons = VillageScreen::icons(c);
        let slots = VillageScreen::slots(c);
        for cluster in 0..vill::CLUSTER_COUNT {
            for slot in 0..ICONS_PER_CLUSTER {
                let value = icons[cluster][slot];
                if value == 0 {
                    continue;
                }
                // `Village_DrawCluster`: the frame is the stored value less
                // one, and one more again while the icon is selected.
                let lit = self.drag_cluster == cluster + 1 && self.selected[slot];
                let frame = value as usize - 1 + usize::from(lit);
                let (x, y) = vill::icon_position(cluster, slot, top);
                let drawn = ctx
                    .assets
                    .chrome
                    .as_ref()
                    .is_some_and(|ch| ch.draw_misc(canvas, frame, x, y));
                if !drawn {
                    // OURS: a lozenge where the peasant goes.
                    let colour = match value {
                        vill::ICON_SHORTFALL => ink.bad,
                        vill::ICON_SURPLUS => ink.dim,
                        _ if lit => ink.highlight,
                        _ => ink.good,
                    };
                    canvas.fill_rect(x + 4, y + 12, 8, 18, colour);
                }
            }
            // OURS, and only when the artwork is missing: the original tells
            // the clusters apart by what is painted under them.
            if !have_scene {
                let (ox, oy) = vill::cluster_origin(cluster, top);
                let name = if slots[cluster] == JOB_IDLE_TOWNSFOLK {
                    "IDLE"
                } else {
                    JOB_NAMES[slots[cluster]]
                };
                text::draw(canvas, ox, oy - 30, &name.to_uppercase(), ink.dim);
            }
        }
    }
}



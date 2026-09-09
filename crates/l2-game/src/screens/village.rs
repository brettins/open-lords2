//! The village — where peasants are assigned, and the only order a player
//! gives every turn in every county.
//!
//! # What the original does
//!
//! `Village_Draw` (`0x00412143`) is screen `0x02`, one of the thirty-nine cases
//! in `Screen_Draw`, and it is **an inset over the campaign map** — a picture
//! painted into a rectangle, with the map, the menu bar and the county sidebar
//! left showing around it.
//!
//! This module said the opposite until a player opened the game, clicked the
//! town square and reported *"it opened up a dialog… still being able to see
//! the map around it and the rest of the screen"*. He was right and the
//! inference was wrong; `docs/decisions.md` C22 records how. The code says so
//! three ways:
//!
//! * **`Village_Draw` never clears.** Its first act on a reload is
//!   `FUN_004050C0`, which is an animation counter and then `FUN_004CFB08` —
//!   and `FUN_004CFB08` calls **`Map_DrawFrame`**. The village *repaints the
//!   campaign map* and then blits itself on top of it.
//! * **The picture is 363 x 320 at (64, `g_villageTopY`)**, and
//!   `g_villageTopY` is 64, or 132 with *Advanced Farming*. Nothing paints
//!   above it, beside it or below it.
//! * **The band it manages is 480 x 320 at (0, `g_villageTopY`).**
//!   `Village_Draw` ends by saving that rectangle and `FUN_004120E0` restores
//!   it, which is how the peasants are redrawn during a drag without repainting
//!   the map. The arithmetic is [`l2_view::village::BAND_W`]'s: the copy moves
//!   `g_spriteWidth` **dwords** a row — 0x78 x 4 = 480 bytes — and then skips
//!   `0xA0` = 160 more, and 480 + 160 is exactly the 640-byte screen stride.
//!
//! **The county sidebar starts at x = 478 and the menu bar ends at y = 23.**
//! Neither is inside anything the village touches, which is precisely what the
//! player saw.
//!
//! Standing on the picture are **eight clusters** of up to twenty-five peasant
//! icons each — seven jobs and *Idle townsfolk*, with iron and stone sharing
//! cluster 0 because a county's mine and its quarry are painted at the same
//! spot. One icon is `ceil(population / 25)` people, which is why a cluster has
//! twenty-five slots: `+0xB8` and the grid are the same fact seen twice.
//!
//! # The gesture, which is three screens in the original
//!
//! `Screen_HandleInput` treats the drag as its own screen ids, and reading them
//! is how the gesture is pinned down rather than guessed:
//!
//! | id | what | leaves when |
//! |---|---|---|
//! | `0x02` | the village, idle | `FUN_004393EB` sees the pointer **9 pixels** from where the button went down → `0x05` |
//! | `0x05` | the rubber band | `FUN_00439541` sees the button **released**: `0x06` if anything is selected, back to `0x02` if not |
//! | `0x06` | carrying the selection | `FUN_004399B0` sees the next **press**, and drops there |
//!
//! So it is press, drag, release, then a second click — not drag-and-drop. A
//! press that never travels nine pixels is a *click*, and a click opens the job
//! popup for whatever cluster it landed on (`FUN_0043A123`).
//!
//! Two rules inside the band are worth naming because neither is obvious:
//!
//! * **A band that reaches into two clusters selects nothing at all.**
//!   `Village_BoxSelect` abandons the whole selection the moment a second
//!   cluster contributes an icon. There is no partial answer.
//! * **Shortfall icons cannot be picked up.** The box-selector skips icon value
//!   1 explicitly, because those figures stand for workers the job *wants* and
//!   has not got. They are drawn and they are not there.
//!
//! # Where a drop lands, which is a file
//!
//! Not a rectangle. `FUN_004398F5` looks the pointer up in an 8-pixel grid that
//! turns out to be `vill_gd8.pl8` — see [`l2_view::village`], where the three
//! numbers that close it are set out. So the drop targets are painted, and this
//! screen reads them rather than inventing them; on an install without the file
//! it refuses to move anybody and says so.
//!
//! # What is ours, and says so
//!
//! * **The font**, as everywhere else: the original draws `Fntl2_14.pl8` and
//!   `Fntl2_22.pl8` and we draw our own 5 x 7.
//! * **The caption and the keyboard.** The original's village has no text on it
//!   at all beyond the two *Advanced Farming* lines, and no keyboard route into
//!   anything. A caption naming the county and Escape as a way out are ours —
//!   and they are drawn **inside the picture**, because everything outside it
//!   belongs to whatever is underneath.
//! * **The animations.** `Village_Animate` steps six counters and redraws
//!   overlays from `villani1`/`villani2` — smoke, water, a cart. None of it is
//!   here; the scene is still.
//! * **`Map_DrawFrame`.** The original repaints the map itself, from inside the
//!   village's own painter. Here the map is the screen underneath on the stack
//!   and [`crate::screen::Machine::draw`] paints it first, because a screen that
//!   drew another screen would be the one thing `screens/mod.rs` forbids.

use l2_kingdom::county::County;
use l2_kingdom::tables::{JOB_IDLE_TOWNSFOLK, JOB_NAMES};
use l2_view::village::{self as vill, ICONS_PER_CLUSTER};
use l2_view::{text, Canvas};

use crate::input::{Event, Key, Rect};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::widget;

/// Which of the original's three screen ids the drag is in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    /// `0x02` — nothing held.
    Idle,
    /// `0x05` — the band is being drawn.
    Band,
    /// `0x06` — a selection is being carried, waiting for somewhere to put it.
    Carry,
}

/// One cluster's icons, as `Village_RebuildIcons` leaves them: the value stored
/// is the `Misc_cty` frame **plus one**, and 0 is an empty slot.
pub type ClusterIcons = [u8; ICONS_PER_CLUSTER];

pub struct VillageScreen {
    county: u8,
    phase: Phase,
    /// Where the button went down, while it is still only a press.
    anchor: Option<(i32, i32)>,
    /// The live pointer, which is the band's other corner.
    pointer: (i32, i32),
    /// The cluster the selection came from, **1-based**; 0 for none. The
    /// original's `g_villageDragCluster`, and 1-based for the same reason: 0
    /// has to mean "nowhere".
    drag_cluster: usize,
    /// `g_peasantIconSelected` — which of the source cluster's twenty-five
    /// slots the band caught.
    selected: [bool; ICONS_PER_CLUSTER],
    /// `g_villageDragCount`, in icons rather than people.
    drag_count: i32,
    /// A click that has landed and is **waiting to find out whether it is half
    /// of a double click** — `DAT_004E65E8`, and the position it recorded in
    /// `DAT_004EAC04` / `DAT_004EAC08`. The counter is ticks remaining; at zero
    /// the click settles and opens the job popup, which is `DAT_004EABF0`.
    pending_click: Option<(i32, i32, u32)>,
    /// **Ours**: what just happened, for a player who cannot see a cursor
    /// change.
    status: String,
}

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

    /// `Village_BoxSelect` (`0x0043958A`).
    fn box_select(&mut self, ctx: &Ctx) {
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
    /// rather than on it, and that delay is not an accident of ours: without it
    /// there is nowhere for the double click to happen.
    pub const CLICK_SETTLE_TICKS: u32 = 19;

    /// Whether this event is one the village's arm hands to the sidebar.
    ///
    /// **Left button and pointer only.** The six guards are all left-button
    /// hit tests, and the right button is the village's own third way out
    /// (`g_mouseRightReleased` → `g_screenId = 0`), so it must not pass. The
    /// double click must not pass either: `Village_DoubleClick` is read in
    /// exactly one place in the whole binary and this arm is it.
    fn belongs_to_the_sidebar(event: Event) -> bool {
        let x = match event {
            Event::Click { x, .. } | Event::Release { x, .. } | Event::Pointer { x, .. } => x,
            _ => return false,
        };
        x >= l2_view::campaign::PANEL_X
    }
}

impl Screen for VillageScreen {
    fn id(&self) -> ScreenId {
        ScreenId::Village(self.county)
    }

    fn title(&self, _ctx: &Ctx) -> String {
        format!("Village of county {}", self.county)
    }

    fn handle(&mut self, event: Event, ctx: &mut Ctx) -> Transition {
        // **The sidebar stays live with the village open**, and it is the arm
        // itself that says so rather than an inference from the inset's size.
        // `Screen_FrameInput`'s `g_screenId == 0x02` ladder runs six guards
        // before it reaches a single village verb:
        //
        // ```c
        // if (Minimap_ModeButtonClicked()   ||   /* FUN_0043292d, the four minimap modes */
        //     Sidebar_ButtonClicked()       ||   /* FUN_00432967, the six sidebar buttons */
        //     CountyStrip_Click()           ||
        //     Labour_SplitSliderDrag()      ||   /* FUN_00439122, the farm/industry split */
        //     CountyStrip_JobClick()        ||
        //     FUN_00439079()) goto done;         /* consumed: no village verb runs */
        // /* only now: Ui_OkButtonClicked, Village_BandStart, Village_DoubleClick, ... */
        // ```
        //
        // All six are the **campaign map's** right-hand column: `FUN_0043292d`
        // is `Hotspot_Test(0x262, 0x20, &g_minimapModeButtons, 4)` and
        // `FUN_00432967` is `Hotspot_Test(0x1DE, 0x1AE, &g_sidebarButtons, 6)`,
        // and every one of the six hit-tests `x >= 0x1DE` — 478, which is
        // [`campaign::PANEL_X`]. So the rule is exactly *"the column at x >=
        // 478 keeps working"*, and nothing else does: `Map_Click` is **not** in
        // this ladder, so a click on the terrain round the inset does nothing.
        //
        // **The two drag states do not do this.** The `0x05` (banding) and
        // `0x06` (carrying) arms test no sidebar guard at all — they run
        // `Village_BandRelease` / `Village_Drop` and nothing else — so the
        // sidebar goes dead for the duration of a peasant drag and comes back
        // when it ends. That is [`Phase::Idle`] below, and it is the kind of
        // detail that reads as intermittent to a player and gets "fixed" into
        // uniformity by mistake. C57.
        //
        // A player checked it against the original: *"The slider does indeed
        // still work with town square open and causes no issues."* It did not
        // work in ours, because [`crate::screen::Machine`] offered input to the
        // top screen and stopped. [`Transition::Pass`] is what that cost.
        if self.phase == Phase::Idle && Self::belongs_to_the_sidebar(event) {
            return Transition::Pass;
        }
        match event {
            // **The right button leaves the village**, which is the third of
            // `Screen_FrameInput`'s three ways out of screen `0x02`: a right release
            // sets `g_screenId = 0` outright. Escape is ours and does the same,
            // except that mid-drag it cancels the drag instead — the original
            // has no key here at all.
            Event::RightClick { .. } => return Transition::Pop,
            Event::KeyDown(Key::Escape) => {
                if self.phase == Phase::Idle {
                    return Transition::Pop;
                }
                self.clear_drag();
                self.pending_click = None;
                self.status = "CANCELLED".into();
            }
            Event::KeyDown(Key::Enter) => return Transition::Pop,
            Event::Pointer { x, y } => {
                self.pointer = (x, y);
                if self.phase == Phase::Band {
                    self.box_select(&*ctx);
                } else if self.phase == Phase::Idle {
                    if let Some((ax, ay)) = self.anchor {
                        // The nine-pixel dead zone: below it this is still a
                        // click, and a click opens the job popup.
                        if (x - ax).abs() >= vill::DRAG_DEAD_ZONE
                            || (y - ay).abs() >= vill::DRAG_DEAD_ZONE
                        {
                            self.phase = Phase::Band;
                            self.box_select(&*ctx);
                        }
                    }
                }
            }
            // **`Village_DoubleClick` (`0x00439DF0`) is its own input arm**, and
            // it is the only reader of the double-click flag in the whole
            // binary. It sits *between* `Village_BandStart` and
            // `Village_ClickJob` in `Screen_FrameInput`'s screen-`0x02` ladder,
            // which is the order kept here: a drag in progress wins, then the
            // double click, then — only once it has settled — the job popup.
            Event::DoubleClick { x, y } => {
                self.pointer = (x, y);
                // The pending single click is cancelled outright: the original
                // clears `DAT_004E65E8` the moment `DAT_004EABC5` is set, in
                // the poll itself, so the popup never opens behind the move.
                self.pending_click = None;
                if self.phase != Phase::Idle {
                    return Transition::Stay;
                }
                if let Some(cluster) = self.double_click_cluster(&*ctx, x, y) {
                    self.balance(ctx, cluster - 1);
                }
            }
            Event::Click { x, y } => {
                self.pointer = (x, y);
                if self.phase == Phase::Carry {
                    self.drop_on(ctx, x, y);
                    return Transition::Stay;
                }
                if VillageScreen::ok_button(VillageScreen::top_y(&*ctx)).contains(x, y) {
                    return Transition::Pop;
                }
                if VillageScreen::in_band_area(&*ctx, x, y) {
                    self.anchor = Some((x, y));
                }
            }
            Event::Release { x, y } => {
                self.pointer = (x, y);
                match self.phase {
                    Phase::Band => {
                        // `FUN_00439541`: something selected means carry it,
                        // nothing means the band was for nothing.
                        if self.drag_cluster != 0 {
                            self.phase = Phase::Carry;
                            self.status = format!("{} PICKED UP", self.drag_count);
                        } else {
                            self.clear_drag();
                        }
                    }
                    Phase::Idle => {
                        // A press that never travelled nine pixels is a click,
                        // and a click opens the job popup — but **not yet**.
                        // The original arms `DAT_004E65E8` here and opens the
                        // popup only when 300 ms have gone by without a second
                        // press (`Village_ClickJob` reads `DAT_004EABF0`, which
                        // is that timer expiring). See
                        // [`VillageScreen::CLICK_SETTLE_TICKS`]; `update` is
                        // where it lands. Ownership is not tested: you cannot
                        // reach the village of a county you do not hold in the
                        // first place.
                        if self.anchor.take().is_some() {
                            self.pending_click = Some((x, y, Self::CLICK_SETTLE_TICKS));
                        }
                    }
                    Phase::Carry => {}
                }
            }
            _ => {}
        }
        Transition::Stay
    }

    /// The pending click's clock, and the one thing on this screen that happens
    /// without an event arriving.
    fn update(&mut self, ctx: &mut Ctx) -> Transition {
        let Some((x, y, left)) = self.pending_click else { return Transition::Stay };
        if left > 1 {
            self.pending_click = Some((x, y, left - 1));
            return Transition::Stay;
        }
        self.pending_click = None;
        match self.job_under(&*ctx, x, y) {
            Some(job) => Transition::Push(ScreenId::Job(self.county, job)),
            None => Transition::Stay,
        }
    }

    /// **The village is an inset.** `Village_Draw` never clears — it repaints
    /// the campaign map and blits over it — so the map screen underneath is
    /// painted first by [`crate::screen::Machine::draw`].
    fn is_overlay(&self) -> bool {
        true
    }

    fn draw(&mut self, ctx: &Ctx, canvas: &mut Canvas) {
        let ink = &ctx.assets.ink;
        let top = VillageScreen::top_y(ctx);
        // **No clear.** Everything below paints inside the picture at
        // (64, top) or inside the 480 x 320 band the original saves and
        // restores around it; the menu bar, the county sidebar and the map
        // either side belong to whatever is underneath.

        let art = ctx.assets.village.as_ref();
        let drew = art.is_some_and(|a| a.draw_scene(canvas, top));
        if !drew {
            // OURS: a flat field, for an install with no vill.pl8.
            canvas.fill_rect(vill::SCENE_X, top, vill::SCENE_W, vill::SCENE_H, ink.panel);
            widget::frame(
                canvas,
                Rect::new(vill::SCENE_X, top, vill::SCENE_W, vill::SCENE_H),
                ink.border,
            );
        }
        if let Some(c) = self.county_ref(ctx) {
            if ctx.game.kingdom.options.advanced_farming {
                art.map(|a| a.draw_tops(canvas, c.weather.index() as usize));
            }
            self.draw_clusters(ctx, canvas, c, top, drew);
        }

        let ok = VillageScreen::ok_button(top);
        let drawn = ctx.assets.chrome.as_ref().is_some_and(|ch| {
            ch.draw_system(canvas, l2_view::chrome::system::OK_ALT, ok.x, ok.y)
        });
        if !drawn {
            widget::button(canvas, ink, ok, "CLOSE", false);
        }

        // OURS. `Village_BandStart`'s hit region is read out of the binary and
        // is wider than the picture — x 0 … 0x1FF, y top … top + 0x178 — but
        // nothing in the decompiled corpus was found *drawing* the band, so the
        // outline is ours and so is its colour. It can therefore reach outside
        // the 480 x 320 region the original saves and restores; that costs
        // nothing here because the map underneath is repainted every frame,
        // where the original would have had to restore it.
        if let Some((x0, y0, x1, y1)) = self.band().filter(|_| self.phase == Phase::Band) {
            widget::frame(canvas, Rect::new(x0, y0, x1 - x0 + 1, y1 - y0 + 1), ink.highlight);
        }

        // OURS: the original's village carries no text at all. Ours goes
        // **inside the picture**, in the two rows at the top and bottom of it,
        // because everything outside belongs to the screen underneath.
        let mid = vill::SCENE_X + vill::SCENE_W / 2;
        let caption = match self.phase {
            Phase::Carry => format!("CARRYING {} - CLICK A JOB", self.drag_count),
            _ => format!("VILLAGE OF COUNTY {}", self.county),
        };
        text::draw_centred(canvas, mid, top + 4, &caption, ink.text);
        let mut line = top + vill::SCENE_H - 12;
        let mut say = |canvas: &mut Canvas, s: &str, colour: u8| {
            text::draw_centred(canvas, mid, line, s, colour);
            line -= 12;
        };
        if ctx.assets.village.as_ref().is_none_or(|a| !a.has_grid()) {
            say(canvas, "NO DROP GRID - VILL_GD8.PL8 MISSING", ink.bad);
        }
        if !ctx.game.is_players(self.county) {
            say(canvas, "NOT YOURS", ink.bad);
        }
        if !self.status.is_empty() {
            say(canvas, &self.status, ink.dim);
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::{Assets, Game};
    use l2_kingdom::tables::{JOB_CATTLE_FARMING, JOB_WOOD_CUTTING};

    fn world() -> (Game, Assets) {
        let mut g = Game::new(7);
        g.kingdom.set_county_count(2);
        g.kingdom.counties[1].owner = 1;
        g.kingdom.counties[1].population = 435;
        g.kingdom.counties[1].pop_band = (435 - 1) / 25 + 1;
        g.kingdom.counties[1].labour[JOB_CATTLE_FARMING] = 218;
        g.kingdom.counties[1].labour_wanted[JOB_CATTLE_FARMING] = 302;
        g.kingdom.counties[1].labour_useful[JOB_CATTLE_FARMING] = 302;
        g.kingdom.counties[1].labour[JOB_WOOD_CUTTING] = 217;
        g.kingdom.counties[1].labour_wanted[JOB_WOOD_CUTTING] = -1;
        g.kingdom.counties[1].labour_useful[JOB_WOOD_CUTTING] = 100_000;
        (g, Assets::placeholder())
    }

    /// County 1 of the shipped save, which is understaffed on cattle: 218
    /// dairy maids of a wanted 302, at 18 people an icon.
    ///
    /// **This is the test the two new words exist for.** Without them the
    /// cattle cluster is thirteen identical icons; with them it is thirteen
    /// people and five ghosts, and the ghosts are the interface's entire way of
    /// saying "this job is short".
    #[test]
    fn an_understaffed_job_draws_its_shortfall_and_a_healthy_one_does_not() {
        let (g, _) = world();
        let c = &g.kingdom.counties[1];
        let icons = VillageScreen::icons(c);
        // Cluster 2 is cattle farming, cluster 4 is wood cutting.
        assert_eq!(vill::CLUSTER_TO_SLOT[2], JOB_CATTLE_FARMING);
        assert_eq!(vill::CLUSTER_TO_SLOT[4], JOB_WOOD_CUTTING);

        let cattle = icons[2];
        let ghosts = cattle.iter().filter(|&&v| v == vill::ICON_SHORTFALL).count();
        let people = cattle.iter().filter(|&&v| v != 0 && v != vill::ICON_SHORTFALL).count();
        assert_eq!(people, 13, "218 of 435 people at 18 to an icon");
        assert_eq!(ghosts, 5, "and 84 more wanted is five more icons");

        let wood = icons[4];
        assert_eq!(wood.iter().filter(|&&v| v == vill::ICON_SHORTFALL).count(), 0);
        assert_eq!(wood.iter().filter(|&&v| v == vill::ICON_SURPLUS).count(), 0);
        assert_eq!(
            wood.iter().filter(|&&v| v != 0).count(),
            13,
            "an unbounded ceiling never makes anyone surplus"
        );
    }

    /// A band that reaches into two clusters selects nothing, which is the
    /// original refusing to guess which one you meant.
    #[test]
    fn a_band_across_two_clusters_selects_nothing_at_all() {
        let (mut g, a) = world();
        let mut s = VillageScreen::new(1);
        let ctx = Ctx { game: &mut g, assets: &a };
        let top = VillageScreen::top_y(&ctx);

        // One cluster: the cattle cluster's own box.
        let (bx0, by0, bx1, by1) = vill::cluster_band_box(2, top);
        s.anchor = Some((bx0, by0));
        s.pointer = (bx1, by1);
        s.box_select(&ctx);
        assert_eq!(s.drag_cluster, 3, "cluster 2, 1-based");
        assert!(s.drag_count > 0);

        // The whole picture: every cluster at once, and therefore none.
        s.anchor = Some((0, top));
        s.pointer = (639, top + vill::SCENE_H);
        s.box_select(&ctx);
        assert_eq!(s.drag_cluster, 0);
        assert_eq!(s.drag_count, 0);
    }

    /// The gesture, end to end: press, travel nine pixels, release, click.
    #[test]
    fn the_drag_is_press_band_release_then_a_second_click() {
        let (mut g, a) = world();
        let mut s = VillageScreen::new(1);
        let top = {
            let ctx = Ctx { game: &mut g, assets: &a };
            VillageScreen::top_y(&ctx)
        };
        let (bx0, by0, bx1, by1) = vill::cluster_band_box(2, top);

        let send = |s: &mut VillageScreen, g: &mut Game, e: Event| {
            let mut ctx = Ctx { game: g, assets: &a };
            s.handle(e, &mut ctx)
        };

        send(&mut s, &mut g, Event::Click { x: bx0, y: by0 });
        assert_eq!(s.phase(), Phase::Idle, "a press is not yet a drag");
        send(&mut s, &mut g, Event::Pointer { x: bx0 + 1, y: by0 + 1 });
        assert_eq!(s.phase(), Phase::Idle, "and one pixel is not either");
        send(&mut s, &mut g, Event::Pointer { x: bx1, y: by1 });
        assert_eq!(s.phase(), Phase::Band, "nine is");
        assert!(s.drag_count() > 0);

        let picked = s.drag_count();
        send(&mut s, &mut g, Event::Release { x: bx1, y: by1 });
        assert_eq!(s.phase(), Phase::Carry, "released with a selection: carry it");
        assert_eq!(s.drag_count(), picked);
        assert_eq!(
            g.kingdom.counties[1].labour[JOB_CATTLE_FARMING], 218,
            "and nothing has moved yet"
        );
    }

    /// A band that catches nobody puts the screen back where it started rather
    /// than leaving it carrying an empty selection.
    #[test]
    fn a_band_over_empty_ground_leaves_nothing_to_carry() {
        let (mut g, a) = world();
        let mut s = VillageScreen::new(1);
        let mut ctx = Ctx { game: &mut g, assets: &a };
        s.handle(Event::Click { x: 0, y: VillageScreen::top_y(&ctx) }, &mut ctx);
        s.handle(Event::Pointer { x: 20, y: VillageScreen::top_y(&ctx) + 20 }, &mut ctx);
        assert_eq!(s.phase(), Phase::Band);
        s.handle(Event::Release { x: 20, y: VillageScreen::top_y(&ctx) + 20 }, &mut ctx);
        assert_eq!(s.phase(), Phase::Idle);
        assert_eq!(s.drag_cluster(), 0);
    }

    /// Moving peasants is `icons * popBand`, clamped to what the job holds —
    /// so emptying a job never takes more people out of it than were in it.
    #[test]
    fn an_icon_is_pop_band_people_and_the_source_job_is_the_ceiling() {
        let (mut g, _) = world();
        let band = g.kingdom.counties[1].pop_band;
        assert_eq!(band, 18);
        assert_eq!(g.move_labour(1, JOB_CATTLE_FARMING, JOB_WOOD_CUTTING, 3), 54);
        assert_eq!(g.kingdom.counties[1].labour[JOB_CATTLE_FARMING], 164);
        assert_eq!(g.kingdom.counties[1].labour[JOB_WOOD_CUTTING], 271);

        // Thirteen icons is 234 people and only 164 are there.
        assert_eq!(g.move_labour(1, JOB_CATTLE_FARMING, JOB_WOOD_CUTTING, 13), 164);
        assert_eq!(g.kingdom.counties[1].labour[JOB_CATTLE_FARMING], 0);
        assert_eq!(
            g.kingdom.counties[1].labour.iter().sum::<i32>(),
            435,
            "and the nine still sum to the population"
        );

        assert_eq!(g.move_labour(2, JOB_CATTLE_FARMING, JOB_WOOD_CUTTING, 1), 0, "not your county");
        assert_eq!(g.move_labour(1, JOB_CATTLE_FARMING, JOB_CATTLE_FARMING, 1), 0, "nowhere to go");
    }

    /// The OK button is inside the picture in both scene positions, and the
    /// band area covers the whole picture.
    #[test]
    fn the_ok_button_and_the_band_area_sit_where_the_picture_is() {
        for top in [vill::SCENE_Y, vill::SCENE_Y_ADVANCED] {
            let ok = VillageScreen::ok_button(top);
            assert!(ok.x >= vill::SCENE_X, "{top}: the corner is on the picture");
            assert!(ok.x + ok.w <= vill::SCENE_X + vill::SCENE_W, "{top}");
            assert!(ok.y + ok.h <= top + vill::SCENE_H, "{top}");
            assert!(top + vill::BAND_H >= top + vill::SCENE_H, "the band covers the scene");
        }
    }
}

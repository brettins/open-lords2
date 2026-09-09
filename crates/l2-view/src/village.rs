//! The village — the county's own picture, and where peasants are moved.
//!
//! Screen `0x02`. `Village_Draw` (`0x00412143`) is one of the thirty-nine cases
//! in `Screen_Draw`, and it is **an inset over the campaign map**, not a page:
//! it repaints the map — `FUN_004050C0` → `FUN_004CFB08` → `Map_DrawFrame` —
//! and then blits a 363 x 320 picture over it. Nothing clears the screen, so
//! the menu bar, the county sidebar and a band of map either side of the
//! picture stay visible, which is how a player describes it as a dialogue.
//! Floating over *that* is the job popup (screen `0x0F`).
//!
//! This module said the opposite until a player opened the game and looked.
//! `docs/decisions.md` C22 records the inference and why it was wrong.
//!
//! Everything in this module is layout and artwork. The county's state stays in
//! `l2-kingdom` and the state machine in `l2-game`; what is here is the
//! geometry, which is the part read out of `Lords2.exe` and its files.
//!
//! # The picture, and what stays visible around it
//!
//! ```text
//!  x=0                                                478       640
//!  y=0   ┌──────────────────────────────────────────────┬─────────┐
//!        │                  menu bar                    │         │  none of this
//!  y=24  ├────────┬───────────────────────────┬─────────┤ county  │  is touched
//!        │        │                           │         │ sidebar │
//!  y=64  │  map   │      vill.pl8 frame 0     │   map   │         │
//!        │        │   363 x 320 at (64, 64)   │         │         │
//!        │        │  eight clusters of icons  │         │         │
//!  y=384 ├────────┴───────────────────────────┴─────────┤         │
//!        │                    map                       │         │
//!  y=480 └──────────────────────────────────────────────┴─────────┘
//!          |<------------- the 480 x 320 band ---------->|
//!                     saved and restored at (0, 64)
//! ```
//!
//! `g_villageTopY` is **64**, or **132** with *Advanced Farming* on, which is
//! what makes room for `villtops.pl8` — six 363 x 70 frames, one per weather,
//! drawn at y = 64 above the scene. **`[V]`**: `villtops.pl8` has exactly six
//! frames and `L2.eng` group 66 has exactly six weather names, and
//! `Village_Draw` indexes both with the same county byte.
//!
//! # The drop grid, which is a file
//!
//! Peasants are dropped by pointing at the *picture*, not at a rectangle.
//! `FUN_004398F5` reads
//!
//! ```c
//! (&DAT_00542CF8)[((x - 0x40) >> 3) + ((y - g_villageTopY) >> 3) * 0x2D]
//! ```
//!
//! and `0x00542CF8` is `0x00542CE0 + 0x18` — the pixel data of preload entry
//! 12, **`vill_gd8.pl8`**, whose "what it is" column in `docs/screens-county.md`
//! §4 was blank. It closes three ways: `0x2D` is 45 and the tested x range
//! `0x40 … 0x1A8` is 360 pixels, which is 45 cells of 8; the y range is 320
//! pixels, which is 40 cells of 8; and **the shipped file is 1,824 bytes**,
//! which is a 24-byte header plus 45 x 40 exactly. The byte is the cluster,
//! 1-based, clamped to 8. **`[V]`**
//!
//! # The icons
//!
//! One icon is `ceil(population / 25)` people, which is why a cluster has
//! twenty-five slots. `Misc_cty.pl8` frames 0 … 0x16 are the icons — twenty-
//! three 16 x 32 frames that `docs/screens-county.md` §9 guessed were "almost
//! certainly the top menu bar", marked `[I]` and never checked. They are not.
//! **`[V]`**: [`ICON_VALUE`] is the shipped table at `0x004D6808`, its nine
//! entries are nine (normal, highlighted) pairs, and the four frames it never
//! names — 5, 6, 11 and 12 — are exactly the four frames in that range of the
//! shipped file that are **2 x 2 stubs** rather than 16 x 32 icons. Nineteen
//! icons, nineteen 16 x 32 frames, nothing left over.

use crate::sheet::Sheet;
use crate::Canvas;

/// The eight peasant clusters. Seven jobs and *Idle townsfolk*: iron and stone
/// **share** cluster 0, because a county's mine and its quarry are drawn at the
/// same spot — `villani2.pl8` frames 0x2B and 0x28, both at `(0x4C, top + 0x0C)`.
///
/// This used to name `Misc_cty.pl8`. The frames were right and the file was
/// wrong; [`RESOURCE_BUILDINGS`] has `Village_Draw`'s three calls verbatim.
pub const CLUSTER_COUNT: usize = 8;

/// Twenty-five icon slots to a cluster, one twenty-fifth of the county each.
pub const ICONS_PER_CLUSTER: usize = 25;

/// `g_jobClusterOrigins` (`0x004D85A8`) — eight `{i32 x, i32 y}` pairs, before
/// the `(0x40, g_villageTopY)` offset every one of them is drawn at.
pub const ORIGINS: [(i32, i32); CLUSTER_COUNT] =
    [(22, 50), (147, 30), (271, 26), (268, 188), (117, 200), (15, 245), (146, 120), (19, 122)];

/// `g_peasantIconOffsets` (`0x004D85E8`) — a 5 x 5 grid, 16 pixels apart in x
/// and 12 in y, its rows staggered 0, 4, 8, 0, 4.
pub const ICON_OFFSETS: [(i32, i32); ICONS_PER_CLUSTER] = [
    (0, 0),
    (16, 0),
    (32, 0),
    (48, 0),
    (64, 0),
    (4, 12),
    (20, 12),
    (36, 12),
    (52, 12),
    (68, 12),
    (8, 24),
    (24, 24),
    (40, 24),
    (56, 24),
    (72, 24),
    (0, 36),
    (16, 36),
    (32, 36),
    (48, 36),
    (64, 36),
    (4, 48),
    (20, 48),
    (36, 48),
    (52, 48),
    (68, 48),
];

/// `g_jobClusterToSlot` (`0x004D6780`) — which labour slot each cluster is.
///
/// Cluster 0 reads **stone**, and [`slot_for_cluster`] overrides it to iron for
/// a county with a mine and no quarry. That single case is what pins industry 1
/// as iron and industry 3 as stone (`docs/screens-county.md` §6.4).
pub const CLUSTER_TO_SLOT: [usize; CLUSTER_COUNT] = [5, 0, 1, 3, 6, 7, 8, 2];

/// The cluster that is *Idle townsfolk*, and the one special case in three of
/// the icon builders: its icons are always drawn in the surplus frame, because
/// idle people **are** the surplus.
pub const IDLE_CLUSTER: usize = 6;

/// **Ten** cluster numbers, because `Village_BalanceAll` (`0x00439EDB`) loops
/// `for (i = 0; i < 10; i++)` over an eight-entry table.
///
/// It is an over-read and it is reproduced on purpose — `docs/bugs.md`. The two
/// words past `g_jobClusterToSlot` are the head of `DAT_004D67A0`, and they are
/// **4** and **6**, read out of the shipped `Lords2.exe` at `0x004D67A0` by
/// `crates/l2-view/tests/install.rs`. So a double click on the idle cluster
/// also balances slot 4, *Iron mining* — which no cluster 0 … 7 reaches unless
/// the county's cluster 0 has been overridden to it — and balances slot 6,
/// *Wood cutting*, a second time. Whether the original meant to reach iron or
/// merely ran off the end of its table, the effect is that the gesture covers
/// **all nine slots**, and a faithful reimplementation has to loop the same ten.
pub const CLUSTER_TO_SLOT_BALANCE: [usize; 10] = [5, 0, 1, 3, 6, 7, 8, 2, 4, 6];
/// Where the ten words above start, so a test can read them back out of the
/// user's own executable rather than trusting this table.
pub const CLUSTER_TO_SLOT_VA: u32 = 0x004D_6780;

/// `Village_Draw`: `FUN_0040A682(0, 0x40, g_villageTopY)`.
pub const SCENE_X: i32 = 64;
/// `g_villageTopY` with *Advanced Farming* off.
pub const SCENE_Y: i32 = 64;
/// `g_villageTopY` with it on — 68 pixels lower, which is where `villtops.pl8`
/// goes.
pub const SCENE_Y_ADVANCED: i32 = 132;
/// `villtops.pl8` is drawn at this y whichever mode is on.
pub const TOPS_Y: i32 = 64;

/// `vill.pl8` frame 0, out of the shipped file's own frame table.
pub const SCENE_W: i32 = 363;
pub const SCENE_H: i32 = 320;

/// The band `Village_Draw` saves and `FUN_004120E0` restores: **480 x 320 at
/// (0, `g_villageTopY`)**, and the outer bound of everything the village may
/// dirty.
///
/// **`[V]`, and the numbers do not read at face value.** `Village_Draw` sets
/// `g_drawX = 0`, `g_drawY = g_villageTopY`, `g_spriteWidth = 0x78` and
/// `g_spriteHeight = 0x140`, then calls `FUN_004B3F0A(buffer, 0xA0)`. That
/// function copies **dwords**: it advances the framebuffer pointer
/// `g_spriteWidth` times as an `undefined4 *` — 0x78 x 4 = **480 bytes**, one
/// byte a pixel — and then adds `0xA0` = 160 more to reach the next row.
/// `480 + 160 = 640`, which is the screen stride exactly, so the width is 480
/// pixels and not 120. Its twin `FUN_004B3EC0` copies the other way.
///
/// So the village's reach stops at **x = 480** and at
/// `g_villageTopY + 320`. The menu bar (y 0 … 23) and all but the first two
/// columns of the county sidebar (x 478 … 639) are outside it — 480 is a dword
/// boundary and 478 is not, which is the whole of why it overshoots by two.
/// The picture itself is narrower still — 363 wide from x = 64 — so a strip of
/// campaign map shows on both sides of it even inside the band.
pub const BAND_X: i32 = 0;
pub const BAND_W: i32 = 480;
/// `g_spriteHeight`, which is a plain row count.
pub const BAND_H_SAVED: i32 = 320;
/// What `FUN_004B3F0A`'s second argument leaves for the rest of the row.
pub const BAND_ROW_REMAINDER: i32 = 0xA0;
/// The screen stride the two must add up to.
pub const SCREEN_STRIDE: i32 = 640;

/// `vill_gd8.pl8`: 45 columns and 40 rows of 8 x 8 pixels, covering
/// x `0x40 … 0x1A8` and y `top … top + 0x140`.
pub const GRID_COLS: usize = 45;
pub const GRID_ROWS: usize = 40;
pub const GRID_CELL: i32 = 8;
pub const GRID_LEN: usize = GRID_COLS * GRID_ROWS;
/// The 24 bytes of `vill_gd8.pl8` before its single frame's data.
const GRID_DATA_OFFSET: usize = 0x18;

/// `Village_DrawCluster`: an icon is drawn at `origin + offset + this`.
pub const ICON_DX: i32 = -2;
pub const ICON_DY: i32 = -24;

/// `Ui_OkButton(0x180, g_villageTopY + 0x118, 1)`.
pub const OK_X: i32 = 384;
pub const OK_DY: i32 = 280;

/// `FUN_004393EB`: a press inside x `0 … 0x1FF`, y `top … top + 0x178` arms the
/// rubber band, and the band only *starts* once the pointer has moved this far
/// from where it went down. Below it the gesture is a click, which opens the
/// job popup instead.
pub const DRAG_DEAD_ZONE: i32 = 9;
pub const BAND_X_MAX: i32 = 0x1FF;
pub const BAND_H: i32 = 0x178;

/// `g_peasantIcons` values, from `0x004D6808`, indexed by **labour slot**.
///
/// The stored byte is the frame *plus one*; `Village_DrawCluster` draws
/// `value - 1`, and `+ 1` again while the icon is selected — so each entry
/// names a (normal, highlighted) pair. Slot 8 is 2, which is the same pair the
/// surplus icons use.
pub const ICON_VALUE: [u8; 9] = [4, 8, 10, 14, 16, 18, 20, 22, 2];

/// The icon for a worker the job **wants and has not got**. Never selectable —
/// `Village_BoxSelect` skips value 1 explicitly, because those people are not
/// there to be moved.
pub const ICON_SHORTFALL: u8 = 1;
/// The icon for a worker **past the job's useful ceiling**, and for every idle
/// townsman.
pub const ICON_SURPLUS: u8 = 2;

/// `DAT_004D6830` — the order the twenty-five slots of a single-state cluster
/// fill in. A permutation of 1 ..= 25, so the icons appear scattered over the
/// grid rather than in reading order.
pub const FILL_ORDER: [u8; ICONS_PER_CLUSTER] = [
    10, 22, 6, 19, 11, 25, 5, 15, 2, 23, 9, 14, 1, 16, 7, 18, 4, 17, 3, 20, 13, 21, 8, 24, 12,
];

/// `DAT_004D6850` and `DAT_004D6860` — the two orders a cluster showing **both**
/// states fills in, the second offset ten slots along.
///
/// They partition the grid exactly: the first has thirteen real thresholds and
/// lands in slots 0 … 12, the second has twelve and lands in slots 13 … 24, and
/// 13 + 12 = 25 with no slot claimed twice. `99` is a threshold no count
/// reaches.
pub const FILL_ORDER_MAIN: [u8; 15] = [8, 5, 2, 7, 10, 6, 3, 1, 4, 9, 11, 12, 13, 99, 99];
pub const FILL_ORDER_OTHER: [u8; 15] = [99, 99, 99, 12, 10, 11, 8, 5, 4, 7, 9, 3, 1, 2, 6];
/// Where `FILL_ORDER_OTHER`'s slots start. `FUN_00451A5D` writes them through
/// `g_peasantIcons + 10`.
const OTHER_BASE: usize = 10;
/// Below thirteen of each, the two orders above are used; at or above it the
/// cluster fills from both ends instead.
const MIXED_LIMIT: i32 = 13;

/// `Job_SlotForCluster` (`0x004517CA`).
///
/// Cluster 0 is the stone quarry unless the county has **no** quarry and **does**
/// have a mine, in which case the same spot is the mine.
pub fn slot_for_cluster(cluster: usize, has_quarry: bool, has_mine: bool) -> usize {
    let slot = CLUSTER_TO_SLOT_BALANCE[cluster.min(CLUSTER_TO_SLOT_BALANCE.len() - 1)];
    if cluster == 0 && !has_quarry && has_mine {
        4
    } else {
        slot
    }
}

/// `FUN_0045183A`: whether clicking a cluster opens its job popup.
///
/// Only cluster 0 can refuse, and only for a county with neither a quarry nor a
/// mine. **The cluster is still drawn** — `Village_DrawPeasants` loops 0 … 7
/// with no test at all — which is the binary agreeing that a county without a
/// mine still shows the slot; it simply has nobody in it.
pub fn cluster_is_clickable(cluster: usize, has_quarry: bool, has_mine: bool) -> bool {
    cluster != 0 || has_quarry || has_mine
}

/// Where cluster `c`'s icon grid starts, for a scene whose top is `top`.
pub fn cluster_origin(cluster: usize, top: i32) -> (i32, i32) {
    let (x, y) = ORIGINS[cluster.min(CLUSTER_COUNT - 1)];
    (x + SCENE_X, y + top)
}

/// Where one icon of one cluster is drawn.
pub fn icon_position(cluster: usize, slot: usize, top: i32) -> (i32, i32) {
    let (ox, oy) = cluster_origin(cluster, top);
    let (dx, dy) = ICON_OFFSETS[slot.min(ICONS_PER_CLUSTER - 1)];
    (ox + dx + ICON_DX, oy + dy + ICON_DY)
}

/// The point `Village_BoxSelect` tests an icon at — the icon's middle, ten
/// pixels right and nine down from where it is drawn.
pub fn icon_hit_point(cluster: usize, slot: usize, top: i32) -> (i32, i32) {
    let (x, y) = icon_position(cluster, slot, top);
    (x + 10, y + 9)
}

/// `Village_BoxSelect`'s per-cluster bounding box: a band reaches into a
/// cluster if it overlaps `origin.x + 0x40 … + 0x98` by `origin.y + top - 0x14
/// … + 0x3C`.
pub fn cluster_band_box(cluster: usize, top: i32) -> (i32, i32, i32, i32) {
    let (x, y) = ORIGINS[cluster.min(CLUSTER_COUNT - 1)];
    (x + 0x40, y + top - 0x14, x + 0x98, y + top + 0x3C)
}

fn ceil_div(a: i32, b: i32) -> i32 {
    if b <= 0 {
        return 0;
    }
    a / b + i32::from(a % b != 0)
}

/// How many icons a cluster shows, and in which of the two "wrong" states.
///
/// `Village_RebuildIcons` (`0x0045161E`), exactly. The second number is
/// **negative for a shortfall** and positive for a surplus, which is how one
/// integer carries both and how the caller knows which frame to use.
pub fn icon_counts(workers: i32, wanted: i32, useful: i32, pop_band: i32) -> (i32, i32) {
    if pop_band <= 0 {
        return (0, 0);
    }
    let other = if workers < wanted {
        -ceil_div(wanted - workers, pop_band)
    } else if useful < workers {
        let mut n = (workers - useful) / pop_band;
        // The original rounds up only when the ceiling is zero — a job the
        // county cannot do at all, where every worker is surplus and the part
        // icon still has to appear.
        if useful == 0 && workers % pop_band != 0 {
            n += 1;
        }
        n
    } else {
        0
    };
    let mut normal = ceil_div(workers, pop_band);
    if useful < workers {
        normal -= other;
    }
    (normal, other)
}

/// One cluster's twenty-five icon values, 0 for an empty slot.
///
/// `FUN_004518A5` and the two fillers under it. `value` is the cluster's own
/// icon from [`ICON_VALUE`]; `main` and `other` come from [`icon_counts`].
pub fn cluster_icons(cluster: usize, value: u8, main: i32, other: i32) -> [u8; ICONS_PER_CLUSTER] {
    let mut icons = [0u8; ICONS_PER_CLUSTER];
    if main == 0 && other == 0 {
        return icons;
    }
    let state = if other < 1 { ICON_SHORTFALL } else { ICON_SURPLUS };
    if other == 0 {
        fill_one(&mut icons, cluster, value, main);
    } else if main == 0 {
        fill_one(&mut icons, cluster, state, other.abs());
    } else {
        fill_two(&mut icons, cluster, value, state, main, other.abs());
    }
    icons
}

/// `FUN_004519CA`: one state, scattered over all twenty-five slots.
fn fill_one(icons: &mut [u8; ICONS_PER_CLUSTER], cluster: usize, value: u8, count: i32) {
    let value = if cluster == IDLE_CLUSTER { ICON_SURPLUS } else { value };
    for (i, &threshold) in FILL_ORDER.iter().enumerate() {
        if i32::from(threshold) <= count {
            icons[i] = value;
        }
    }
}

/// `FUN_00451A5D`: two states at once.
fn fill_two(
    icons: &mut [u8; ICONS_PER_CLUSTER],
    cluster: usize,
    main_value: u8,
    other_value: u8,
    main: i32,
    other: i32,
) {
    let (main_value, other_value) = if cluster == IDLE_CLUSTER {
        (ICON_SURPLUS, ICON_SURPLUS)
    } else {
        (main_value, other_value)
    };
    if main < MIXED_LIMIT && other < MIXED_LIMIT {
        for i in 0..FILL_ORDER_MAIN.len() {
            if i32::from(FILL_ORDER_OTHER[i]) <= other {
                icons[i + OTHER_BASE] = other_value;
            }
            if i32::from(FILL_ORDER_MAIN[i]) <= main {
                icons[i] = main_value;
            }
        }
        return;
    }
    // Too many of either to scatter: fill from the front with one state and
    // back-fill the empty tail with the other.
    for slot in icons.iter_mut().take(main.clamp(0, ICONS_PER_CLUSTER as i32) as usize) {
        *slot = main_value;
    }
    let mut i = ICONS_PER_CLUSTER as i32 - 1;
    while i > ICONS_PER_CLUSTER as i32 - 1 - other && i >= 0 && icons[i as usize] == 0 {
        icons[i as usize] = other_value;
        i -= 1;
    }
}

// ------------------------------------------------------------------- artwork

/// **The three buildings `Village_Draw` paints on top of the scene**, in the
/// order it paints them, each one gated on the county having that resource.
///
/// `[V]` — `Village_Draw` (`0x00412143`), read out of the corpus verbatim:
///
/// ```c
/// if (county.industry[0].hasResource) Pl8_DrawFrame(villani2, 0x29, 0xac, top + 0xe5);
/// if (county.industry[3].hasResource) Pl8_DrawFrame(villani2, 0x28, 0x4c, top + 0x0c);
/// if (county.industry[1].hasResource) Pl8_DrawFrame(villani2, 0x2b, 0x4c, top + 0x0c);
/// ```
///
/// Three things it settles, all of which had been guessed at:
///
/// * **The sheet is `villani2.pl8`, not `Misc_cty.pl8`.** This module's own
///   doc comment on [`CLUSTER_COUNT`] named `Misc_cty` frames `0x2B` and `0x28`
///   and it was wrong about the file; the frame numbers were right.
/// * **The lumber camp is a third building.** Wood is industry 0 and it has a
///   building of its own, at the bottom right of the scene — the pairing that
///   was documented was only the two that share a spot.
/// * **The mine is drawn *after* the quarry at the same position.** A county
///   holding both would show the mine, and no county on any shipped map holds
///   both — see `crates/l2-scenario/tests/import.rs`, where iron and stone come
///   out complementary in thirteen of England's fourteen counties and absent in
///   the fourteenth.
pub const RESOURCE_BUILDINGS: [(usize, usize, i32, i32); 3] = [
    // (industry record, villani2 frame, x, y relative to g_villageTopY)
    (0, 0x29, 0xAC, 0xE5),
    (3, 0x28, 0x4C, 0x0C),
    (1, 0x2B, 0x4C, 0x0C),
];

/// The animated overlay `Village_Animate` (`0x00412421`) puts on each of the
/// same three buildings: `(industry, sheet is villani1, first frame, frame
/// count, x, y from `g_villageTopY`)`.
///
/// **Not drawn yet** — this crate has no animation clock and `docs/netcode.md`
/// keeps one out of the simulation — but read out of the binary at the same
/// time as [`RESOURCE_BUILDINGS`] so the next person does not have to find it
/// again. The counters are `DAT_004d2944` (0…7), `DAT_004d2930` (0…6) and
/// `DAT_004d2938` (0…0x11), each incremented once a frame and wrapped.
pub const RESOURCE_ANIMATIONS: [(usize, bool, usize, usize, i32, i32); 3] = [
    (0, false, 7, 8, 0xF3, 0x104),
    (3, false, 0, 7, 0xA3, 0x1B),
    (1, true, 0, 18, 0xA4, 0x0C),
];

/// The village's own files, none of which any other screen loads.
pub struct VillageArt {
    scene: Sheet,
    tops: Option<Sheet>,
    grid: Vec<u8>,
    /// `villani2.pl8` — the quarry, the mine and the lumber camp, and the
    /// animated overlays for two of the three.
    animation_b: Option<Sheet>,
}

impl VillageArt {
    /// `vill.pl8` is required; `villtops.pl8` is only drawn with *Advanced
    /// Farming* on and `vill_gd8.pl8` only decides where a drop lands, so an
    /// install missing either still gives a village that draws.
    pub fn load<F>(mut read: F) -> Result<VillageArt, String>
    where
        F: FnMut(&str) -> Result<Vec<u8>, String>,
    {
        let scene = Sheet::new(read("vill.pl8")?).map_err(|e| format!("vill.pl8: {e}"))?;
        let tops = read("villtops.pl8").ok().and_then(|b| Sheet::new(b).ok());
        let grid = read("vill_gd8.pl8")
            .ok()
            .filter(|b| b.len() >= GRID_DATA_OFFSET + GRID_LEN)
            .map(|b| b[GRID_DATA_OFFSET..GRID_DATA_OFFSET + GRID_LEN].to_vec())
            .unwrap_or_default();
        let animation_b = read("villani2.pl8").ok().and_then(|b| Sheet::new(b).ok());
        Ok(VillageArt { scene, tops, grid, animation_b })
    }

    /// Whether the drop grid was found. Without it nothing can be dropped, and
    /// the screen says so rather than guessing at rectangles.
    pub fn has_grid(&self) -> bool {
        self.grid.len() == GRID_LEN
    }

    /// `FUN_004398F5`: which cluster a point is over, 1-based, or 0.
    pub fn cluster_at(&self, x: i32, y: i32, top: i32) -> usize {
        if !self.has_grid() || x < SCENE_X || x >= SCENE_X + GRID_COLS as i32 * GRID_CELL {
            return 0;
        }
        if y < top || y >= top + GRID_ROWS as i32 * GRID_CELL {
            return 0;
        }
        let col = ((x - SCENE_X) / GRID_CELL) as usize;
        let row = ((y - top) / GRID_CELL) as usize;
        // The original clamps to 8 rather than rejecting, so a stray byte reads
        // as the last cluster.
        (self.grid[row * GRID_COLS + col] as usize).min(CLUSTER_COUNT)
    }

    /// The scene itself. Returns false when the frame will not decode, so the
    /// caller can draw its own ground.
    pub fn draw_scene(&self, canvas: &mut Canvas, top: i32) -> bool {
        match self.scene.frame(0) {
            Some(f) => {
                canvas.blit_opaque(&f, SCENE_X, top);
                true
            }
            None => false,
        }
    }

    /// `villtops.pl8` frame `weather`, drawn at y = 64 above an Advanced
    /// Farming scene.
    pub fn draw_tops(&self, canvas: &mut Canvas, weather: usize) -> bool {
        match self.tops.as_ref().and_then(|s| s.frame(weather)) {
            Some(f) => {
                canvas.blit_opaque(&f, SCENE_X, TOPS_Y);
                true
            }
            None => false,
        }
    }

    pub fn tops_frames(&self) -> usize {
        self.tops.as_ref().map_or(0, |s| s.frame_count())
    }

    /// **The quarry, the mine and the lumber camp**, drawn over the scene in
    /// `Village_Draw`'s own order and gated on `has_resource[industry]`.
    ///
    /// Returns how many were painted, so a caller can tell "this county has
    /// none" from "the artwork is missing" — and so a test can assert the
    /// county with a mine draws one and the county with a quarry does not.
    ///
    /// This is the missing half of a defect a player reported as *"a county
    /// that clearly has iron has no iron mine in the town centre"*: the other
    /// half was that `has_resource` was never imported and every county claimed
    /// every resource. `docs/decisions.md` C57.
    pub fn draw_resources(&self, canvas: &mut Canvas, has_resource: [bool; 4], top: i32) -> usize {
        let Some(sheet) = self.animation_b.as_ref() else { return 0 };
        let mut drawn = 0;
        for (industry, frame, x, y) in RESOURCE_BUILDINGS {
            if !has_resource[industry] {
                continue;
            }
            if let Some(f) = sheet.frame(frame) {
                canvas.blit(&f, x, top + y);
                drawn += 1;
            }
        }
        drawn
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The two mixed-state fill orders partition the twenty-five slots exactly:
    /// thirteen real thresholds in the first, twelve in the second, no slot
    /// claimed by both, and none left over. That is the check that the ten-slot
    /// offset in `FUN_00451A5D` is an offset and not an overrun.
    #[test]
    fn the_two_fill_orders_partition_the_grid_with_nothing_over() {
        let main: Vec<usize> =
            (0..15).filter(|&i| FILL_ORDER_MAIN[i] != 99).collect();
        let other: Vec<usize> =
            (0..15).filter(|&i| FILL_ORDER_OTHER[i] != 99).map(|i| i + OTHER_BASE).collect();
        assert_eq!(main.len(), 13);
        assert_eq!(other.len(), 12);
        assert_eq!(main.iter().max(), Some(&12), "the first order stops at slot 12");
        assert_eq!(other.iter().min(), Some(&13), "and the second starts at 13");
        let mut all: Vec<usize> = main.iter().chain(other.iter()).copied().collect();
        all.sort_unstable();
        assert_eq!(all, (0..25).collect::<Vec<_>>(), "every slot exactly once");

        // And each is a permutation of 1..=n, so every count from 1 to the
        // limit lights exactly one more icon.
        let mut a: Vec<u8> = FILL_ORDER_MAIN.iter().copied().filter(|&v| v != 99).collect();
        a.sort_unstable();
        assert_eq!(a, (1..=13).collect::<Vec<u8>>());
        let mut b: Vec<u8> = FILL_ORDER_OTHER.iter().copied().filter(|&v| v != 99).collect();
        b.sort_unstable();
        assert_eq!(b, (1..=12).collect::<Vec<u8>>());
    }

    /// The single-state order is a permutation of 1 ..= 25, which is what makes
    /// "n workers" and "n icons" the same statement.
    #[test]
    fn the_single_state_fill_order_is_a_permutation_of_all_twenty_five() {
        let mut v = FILL_ORDER.to_vec();
        v.sort_unstable();
        assert_eq!(v, (1..=25).collect::<Vec<u8>>());
        for count in 0..=25i32 {
            let mut icons = [0u8; ICONS_PER_CLUSTER];
            fill_one(&mut icons, 0, 7, count);
            assert_eq!(
                icons.iter().filter(|&&v| v != 0).count() as i32,
                count,
                "{count} workers should light {count} icons"
            );
        }
    }

    /// The icon arithmetic, at the three states the job popup colours.
    #[test]
    fn a_job_shows_a_shortfall_a_surplus_or_neither() {
        // Eighteen workers of a wanted twenty-five, four people to an icon:
        // five icons of people and two of the seven missing.
        assert_eq!(icon_counts(18, 25, 25, 4), (5, -2));
        // Comfortably inside both bounds: no second state at all.
        assert_eq!(icon_counts(18, 10, 25, 4), (5, 0));
        // Past the ceiling: the surplus is split off the normal count rather
        // than added to it, so the cluster still shows five icons in total.
        assert_eq!(icon_counts(18, 0, 10, 4), (3, 2));
        // A job the county cannot do at all rounds the surplus up, so a part
        // icon is still drawn.
        assert_eq!(icon_counts(18, 0, 0, 4), (0, 5));
        assert_eq!(icon_counts(0, 0, 0, 4), (0, 0));
        assert_eq!(icon_counts(18, 25, 25, 0), (0, 0), "a county with no people has no icons");
    }

    /// A cluster never shows more icons than it has slots, at any staffing a
    /// county could reach: one icon is `ceil(pop / 25)` people, so twenty-five
    /// icons is the whole county by construction.
    #[test]
    fn no_staffing_overflows_a_clusters_twenty_five_slots() {
        for pop in [1i32, 25, 26, 100, 435, 2000] {
            let band = (pop - 1) / 25 + 1;
            for workers in [0, 1, pop / 3, pop / 2, pop] {
                for &(wanted, useful) in
                    &[(-1, 0), (-1, 100_000), (pop, pop), (pop / 2, pop / 2), (0, 0)]
                {
                    let (main, other) = icon_counts(workers, wanted, useful, band);
                    let icons = cluster_icons(0, ICON_VALUE[0], main, other);
                    assert!(
                        icons.iter().filter(|&&v| v != 0).count() <= ICONS_PER_CLUSTER,
                        "pop {pop} workers {workers} bounds {wanted}..{useful}"
                    );
                }
            }
        }
    }

    /// Idle townsfolk are always drawn in the surplus icon, whatever their
    /// slot's own icon would be — the one cluster with a special case, in all
    /// three of the original's fillers.
    #[test]
    fn the_idle_cluster_always_draws_the_surplus_icon() {
        let mut icons = [0u8; ICONS_PER_CLUSTER];
        fill_one(&mut icons, IDLE_CLUSTER, ICON_VALUE[8], 5);
        assert!(icons.iter().filter(|&&v| v != 0).all(|&v| v == ICON_SURPLUS));
        let mut icons = [0u8; ICONS_PER_CLUSTER];
        fill_two(&mut icons, IDLE_CLUSTER, ICON_VALUE[0], ICON_SHORTFALL, 5, 5);
        assert!(icons.iter().filter(|&&v| v != 0).all(|&v| v == ICON_SURPLUS));
    }

    /// Cluster 0 is the one that changes job with the county, and the one that
    /// refuses to open a popup when the county has neither resource.
    #[test]
    fn cluster_zero_is_the_quarry_the_mine_or_nothing() {
        assert_eq!(slot_for_cluster(0, true, false), 5, "a quarry");
        assert_eq!(slot_for_cluster(0, false, true), 4, "a mine");
        assert_eq!(slot_for_cluster(0, true, true), 5, "both, and stone wins");
        assert_eq!(slot_for_cluster(0, false, false), 5, "neither, and it is dead");
        assert!(!cluster_is_clickable(0, false, false));
        assert!(cluster_is_clickable(0, false, true));
        for c in 1..CLUSTER_COUNT {
            assert!(cluster_is_clickable(c, false, false), "cluster {c} never refuses");
            assert_eq!(slot_for_cluster(c, false, false), CLUSTER_TO_SLOT[c]);
        }
        // The eight clusters cover eight of the nine slots; the ninth is
        // whichever of iron and stone the county does not have.
        let mut slots = CLUSTER_TO_SLOT.to_vec();
        slots.sort_unstable();
        assert_eq!(
            slots,
            vec![0, 1, 2, 3, 5, 6, 7, 8],
            "iron is the one with no cluster of its own"
        );
    }

    /// Every icon of every cluster lands inside the picture.
    #[test]
    fn every_icon_of_every_cluster_is_inside_the_village_picture() {
        for top in [SCENE_Y, SCENE_Y_ADVANCED] {
            for c in 0..CLUSTER_COUNT {
                for s in 0..ICONS_PER_CLUSTER {
                    let (x, y) = icon_position(c, s, top);
                    assert!(x >= 0 && x + 16 <= 640, "cluster {c} icon {s} at x {x}");
                    assert!(y >= 0 && y + 32 <= 480, "cluster {c} icon {s} at y {y}");
                }
            }
        }
    }
}

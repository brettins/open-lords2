//! Screen `0x02`. `Village_Draw` (`0x00412143`) is one of the thirty-nine cases
//! in `Screen_Draw`, and it is **an inset over the campaign map**, not a page:
//!
//! it repaints the map — `FUN_004050C0` → `FUN_004CFB08` → `Map_DrawFrame` —
//! and then blits a 363 x 320 picture over it. Nothing clears the screen, so
//! the menu bar, the county sidebar and a band of map either side of the
//! picture stay visible, which is how a player describes it as a dialogue.
//!
//! `docs/decisions.md` C22 records the inference and why it was wrong.
//!
//! `g_villageTopY` is **64**, or **132** with *Advanced Farming* on, which is
//! what makes room for `villtops.pl8` — six 363 x 70 frames, one per weather,
//! drawn at y = 64 above the scene. **`[V]`**: `villtops.pl8` has exactly six
//! frames and `L2.eng` group 66 has exactly six weather names, and
//! `Village_Draw` indexes both with the same county byte.
//!
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
//! twenty-five slots. `Misc_cty.pl8` frames 0 … 0x16 are the icons — twenty-
//! three 16 x 32 frames that `docs/screens-county.md` §9 guessed were "almost
//! certainly the top menu bar", marked `[I]` and never checked. They are not.
//!
//! **`[V]`**: [`ICON_VALUE`] is the shipped table at `0x004D6808`, its nine
//! entries are nine (normal, highlighted) pairs, and the four frames it never
//! names — 5, 6, 11 and 12 — are exactly the four frames in that range of the
//! shipped file that are **2 x 2 stubs**.

mod layout;
pub use layout::*;
mod icons;
pub use icons::*;
mod art;
pub use art::*;

use crate::sheet::Sheet;
use crate::Canvas;

pub const CLUSTER_COUNT: usize = 8;

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
pub const CLUSTER_TO_SLOT: [usize; CLUSTER_COUNT] = [5, 0, 1, 3, 6, 7, 8, 2];

pub const IDLE_CLUSTER: usize = 6;

/// **Ten** cluster numbers, because `Village_BalanceAll` (`0x00439EDB`) loops
/// `for (i = 0; i < 10; i++)` over an eight-entry table.
///
/// It is an over-read and it is reproduced on purpose — `docs/bugs.md`. The two
/// words past `g_jobClusterToSlot` are the head of `DAT_004D67A0`, and they are
/// **4** and **6**, read out of the shipped `Lords2.exe` at `0x004D67A0` by
/// `crates/l2-view/tests/install/main.rs`.
pub const CLUSTER_TO_SLOT_BALANCE: [usize; 10] = [5, 0, 1, 3, 6, 7, 8, 2, 4, 6];
pub const CLUSTER_TO_SLOT_VA: u32 = 0x004D_6780;

/// `Village_Draw`: `FUN_0040A682(0, 0x40, g_villageTopY)`.
pub const SCENE_X: i32 = 64;
pub const SCENE_Y: i32 = 64;
pub const SCENE_Y_ADVANCED: i32 = 132;
pub const TOPS_Y: i32 = 64;

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
///
/// `480 + 160 = 640`, which is the screen stride exactly, so the width is 480
/// pixels and not 120. Its twin `FUN_004B3EC0` copies the other way.
pub const BAND_X: i32 = 0;
pub const BAND_W: i32 = 480;
pub const BAND_H_SAVED: i32 = 320;
/// What `FUN_004B3F0A`'s second argument leaves for the rest of the row.
pub const BAND_ROW_REMAINDER: i32 = 0xA0;
pub const SCREEN_STRIDE: i32 = 640;

pub const GRID_COLS: usize = 45;
pub const GRID_ROWS: usize = 40;
pub const GRID_CELL: i32 = 8;
pub const GRID_LEN: usize = GRID_COLS * GRID_ROWS;
const GRID_DATA_OFFSET: usize = 0x18;

pub const ICON_DX: i32 = -2;
pub const ICON_DY: i32 = -24;

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
pub const ICON_VALUE: [u8; 9] = [4, 8, 10, 14, 16, 18, 20, 22, 2];

pub const ICON_VALUE_VA: u32 = 0x004D_6808;

pub const ICON_SHORTFALL: u8 = 1;
pub const ICON_SURPLUS: u8 = 2;

/// `DAT_004D6830` — the order the twenty-five slots of a single-state cluster
/// fill in. A permutation of 1 ..= 25, so the icons appear scattered over the
/// grid.
pub const FILL_ORDER: [u8; ICONS_PER_CLUSTER] = [
    10, 22, 6, 19, 11, 25, 5, 15, 2, 23, 9, 14, 1, 16, 7, 18, 4, 17, 3, 20, 13, 21, 8, 24, 12,
];

/// `DAT_004D6850` and `DAT_004D6860` — the two orders a cluster showing **both**
/// states fills in, the second offset ten slots along.
pub const FILL_ORDER_MAIN: [u8; 15] = [8, 5, 2, 7, 10, 6, 3, 1, 4, 9, 11, 12, 13, 99, 99];
pub const FILL_ORDER_OTHER: [u8; 15] = [99, 99, 99, 12, 10, 11, 8, 5, 4, 7, 9, 3, 1, 2, 6];
/// Where `FILL_ORDER_OTHER`'s slots start. `FUN_00451A5D` writes them through
/// `g_peasantIcons + 10`.
const OTHER_BASE: usize = 10;
const MIXED_LIMIT: i32 = 13;


/// `[V]` — `Village_Draw` (`0x00412143`), read out of the corpus verbatim:
pub const RESOURCE_BUILDINGS: [(usize, usize, i32, i32); 3] = [
    (0, 0x29, 0xAC, 0xE5),
    (3, 0x28, 0x4C, 0x0C),
    (1, 0x2B, 0x4C, 0x0C),
];

/// One animated overlay of `Village_Animate` (`0x00412421`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Overlay {
    pub industry: Option<usize>,
    pub villani1: bool,
    pub first: usize,
    pub frames: usize,
    pub at: (i32, i32),
    pub period_ms: u32,
}

/// **Every overlay `Village_Animate` (`0x00412421`) draws, in its own order.**
///
/// **[V]** — read out of the corpus verbatim.
///
/// * **`villani1.pl8` is the iron mine's animation, and that is its only use in
///   the binary.** It is loaded by `Village_Draw` beside `villani2.pl8` and
///   read by exactly one blit — this one. Nothing else in the corpus touches
///   `DAT_0053E918`, the buffer it goes into. That answers a standing question
///   in this module: the file is not spare, it is one eighteen-frame loop.
///
///   `Village_Animate` also steps `DAT_004D2934` (0 … 0x14) and `DAT_004D2948`
///   (0 … 0x0F), and `RefsTo` finds no other reader of either address in the
///   whole executable. Two animations were cut and their clocks were left
///   running. `docs/bugs.md` has the row.
pub const OVERLAYS: [Overlay; 6] = [
    Overlay { industry: None, villani1: false, first: 0x19, frames: 8, at: (0x50, 0xAF), period_ms: PULSE_SLOW_MS },
    Overlay { industry: None, villani1: false, first: 0x21, frames: 7, at: (0x72, 0xC3), period_ms: PULSE_FAST_MS },
    Overlay { industry: None, villani1: false, first: 0x0F, frames: 10, at: (0x16E, 0x125), period_ms: PULSE_SLOW_MS },
    Overlay { industry: Some(0), villani1: false, first: 7, frames: 8, at: (0xF3, 0x104), period_ms: PULSE_SLOW_MS },
    Overlay { industry: Some(3), villani1: false, first: 0, frames: 7, at: (0xA3, 0x1B), period_ms: PULSE_SLOW_MS },
    Overlay { industry: Some(1), villani1: true, first: 0, frames: 18, at: (0xA4, 0x0C), period_ms: PULSE_SLOW_MS },
];

/// **The village's animation clock, in milliseconds — and where the numbers
/// come from.** **[V]**
///
/// `Village_Animate` does not count frames. It reads two booleans,
/// `DAT_00591510` and `DAT_0057D3AC`, and steps its counters only when they are
/// set. `FUN_004BBC80` is where they are set, once per frame, and it is a pure
/// pulse generator:
///
/// ```c
/// DAT_00591510 = 0; DAT_0057d3ac = 0;  /* …and six more, cleared every call */
/// now = timeGetTime();
/// if (0x13 < (int)(now - last) || (int)(now - last) < 0) { ticks++; last = now; }
/// if (3 < ticks) {                      /* every 4 x 20 ms = 80 ms */
///   ticks = 0;
///   DAT_00591510 = 1;
///   if (1 < ++half) { half = 0; DAT_0057d3ac = 1; }   /* every 2 x 80 ms = 160 ms */
///   /* …and /4, /8, /13, /16, /24, /32 for six other subscribers */
/// }
/// ```
///
/// So the base is **20 ms**, `DAT_00591510` fires at **80 ms** and
/// `DAT_0057D3AC` at **160 ms**. Five of the six overlays are on the slow
/// pulse and one — the second unconditional one — is on the fast one.
///
/// The same two pulses drive `FUN_004071A0`, the campaign map's waving flag,
/// which is where they can be checked against something already drawn.
pub const PULSE_FAST_MS: u32 = 80;
/// The 160 ms pulse — `DAT_0057D3AC`, two of the fast one.
pub const PULSE_SLOW_MS: u32 = 160;

/// **The gate at the head of `Tick_Pulses` (`0x004BBC80`), and it throws its
/// remainder away.**
///
/// ```c
/// now = timeGetTime();
/// if (0x13 < (int)(now - stamp) || (int)(now - stamp) < 0) {
///     DAT_005AEB2C++;  DAT_0058FCB0 = 1;  stamp = now;     /* now, not +20 */
/// }
/// ```
///
/// `docs/decisions.md` C179, which found it in the armoury walker.
pub const GATE_MS: u32 = 20;

pub const PULSE80_GATES: u32 = PULSE_FAST_MS / GATE_MS;

/// Their periods, for anyone who goes looking: `DAT_004D2934` wraps at `0x14`
/// (21 frames) on the slow pulse and `DAT_004D2948` at `0x0F` (16 frames) on
/// the fast one. Kept here because "we did not find a consumer" is a claim
/// somebody will want to re-check, and the addresses are the way to do it.
pub const DEAD_COUNTER_PERIODS: [(u32, usize); 2] = [(PULSE_SLOW_MS, 21), (PULSE_FAST_MS, 16)];

/// It is deliberately not `Copy`: a clock that can be duplicated is a clock
/// that gets stepped twice. `docs/decisions.md` C63.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AnimationClock {
    elapsed_ms: u32,
    gates: u32,
    pulses: [u32; 2],
}

pub struct VillageArt {
    scene: Sheet,
    tops: Option<Sheet>,
    grid: Vec<u8>,
    animation_b: Option<Sheet>,
    /// `villani1.pl8` — **the iron mine's eighteen-frame loop, and nothing
    /// else.** `Village_Draw` loads it into `DAT_0053E918` and the single
    /// `Pl8_DrawFrame` in `Village_Animate` is the only read of that buffer in
    /// the executable. See [`OVERLAYS`].
    animation_a: Option<Sheet>,
}

#[cfg(test)]
mod tests {
    use super::*;

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

        let mut a: Vec<u8> = FILL_ORDER_MAIN.iter().copied().filter(|&v| v != 99).collect();
        a.sort_unstable();
        assert_eq!(a, (1..=13).collect::<Vec<u8>>());
        let mut b: Vec<u8> = FILL_ORDER_OTHER.iter().copied().filter(|&v| v != 99).collect();
        b.sort_unstable();
        assert_eq!(b, (1..=12).collect::<Vec<u8>>());
    }

    /// **`Tick_Pulses` (`0x004BBC80`) throws its remainder away**: it fires on
    /// the first tick 20 ms after the last and then sets `stamp = now`. At the
    /// 16 ms tick a gate is two ticks, so the 80 ms pulse is eight ticks and
    /// the 160 ms one sixteen — not five and ten. C179.
    #[test]
    fn the_pulse_chain_drops_its_remainder_at_every_rung() {
        const TICK_MS: u32 = 16;
        let mut clock = AnimationClock::new();
        let mut first = [0u32; 2];
        for n in 1..=240u32 {
            clock.tick(TICK_MS);
            for k in 0..2 {
                if first[k] == 0 && clock.pulses[k] == 1 {
                    first[k] = n;
                }
            }
        }
        assert_eq!(first, [8, 16], "80 ms is eight ticks at a 16 ms tick, 160 ms is sixteen");
        assert_eq!(clock.pulses, [30, 15], "240 ticks, against the 48 and 24 a carry gives");

        let mut slow = AnimationClock::new();
        assert!(!slow.tick(1000), "a 1000 ms tick is one gate and no pulse");
        assert_eq!(slow.pulses, [0, 0]);

        let mut exact = AnimationClock::new();
        for _ in 0..PULSE80_GATES {
            exact.tick(GATE_MS);
        }
        assert_eq!(exact.pulses, [1, 0], "four 20 ms gates are one 80 ms pulse");
    }

    /// The rungs the campaign map's wheels ride, in gates. Four rungs, the same
    /// chain: `g_pulse80`, `g_pulse160`, `DAT_0057D3C8`, `DAT_0058FD08`.
    #[test]
    fn every_rung_is_its_milliseconds_in_twenty_millisecond_gates() {
        assert_eq!(
            [80, 160, 320, 640].map(gates_per_rung),
            [4, 8, 16, 32],
            "a rung counts gates; dividing it by the tick length is the C179 error"
        );
    }

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

    #[test]
    fn a_job_shows_a_shortfall_a_surplus_or_neither() {
        assert_eq!(icon_counts(18, 25, 25, 4), (5, -2));
        assert_eq!(icon_counts(18, 10, 25, 4), (5, 0));
        assert_eq!(icon_counts(18, 0, 10, 4), (3, 2));
        assert_eq!(icon_counts(18, 0, 0, 4), (0, 5));
        assert_eq!(icon_counts(0, 0, 0, 4), (0, 0));
        assert_eq!(icon_counts(18, 25, 25, 0), (0, 0), "a county with no people has no icons");
    }

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

    #[test]
    fn the_idle_cluster_always_draws_the_surplus_icon() {
        let mut icons = [0u8; ICONS_PER_CLUSTER];
        fill_one(&mut icons, IDLE_CLUSTER, ICON_VALUE[8], 5);
        assert!(icons.iter().filter(|&&v| v != 0).all(|&v| v == ICON_SURPLUS));
        let mut icons = [0u8; ICONS_PER_CLUSTER];
        fill_two(&mut icons, IDLE_CLUSTER, ICON_VALUE[0], ICON_SHORTFALL, 5, 5);
        assert!(icons.iter().filter(|&&v| v != 0).all(|&v| v == ICON_SURPLUS));
    }

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
        let mut slots = CLUSTER_TO_SLOT.to_vec();
        slots.sort_unstable();
        assert_eq!(
            slots,
            vec![0, 1, 2, 3, 5, 6, 7, 8],
            "iron is the one with no cluster of its own"
        );
    }

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


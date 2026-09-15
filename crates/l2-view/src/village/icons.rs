#![allow(unused_imports)]
use super::*;
use super::layout::*;
use super::art::*;
use crate::sheet::Sheet;
use crate::Canvas;

fn ceil_div(a: i32, b: i32) -> i32 {
    if b <= 0 {
        return 0;
    }
    a / b + i32::from(a % b != 0)
}

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
pub(crate) fn fill_one(icons: &mut [u8; ICONS_PER_CLUSTER], cluster: usize, value: u8, count: i32) {
    let value = if cluster == IDLE_CLUSTER { ICON_SURPLUS } else { value };
    for (i, &threshold) in FILL_ORDER.iter().enumerate() {
        if i32::from(threshold) <= count {
            icons[i] = value;
        }
    }
}

/// `FUN_00451A5D`: two states at once.
pub(super) fn fill_two(
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
    for slot in icons.iter_mut().take(main.clamp(0, ICONS_PER_CLUSTER as i32) as usize) {
        *slot = main_value;
    }
    let mut i = ICONS_PER_CLUSTER as i32 - 1;
    while i > ICONS_PER_CLUSTER as i32 - 1 - other && i >= 0 && icons[i as usize] == 0 {
        icons[i as usize] = other_value;
        i -= 1;
    }
}


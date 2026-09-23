

#[macro_use]
#[path = "../common/mod.rs"]
mod common;

mod view_tests;
mod interaction_tests;
mod structures_tests;
mod fog_and_march_tests;

use common::*;

use l2_game::screens::map::MapScreen;

fn pick_counts(screen: &MapScreen) -> [usize; 17] {
    let mut counts = [0usize; 17];
    let clip = screen.map_clip();
    for y in clip.y0..clip.y1 {
        for x in clip.x0..clip.x1 {
            let id = screen.county_at(x, y) as usize;
            if id < 17 {
                counts[id] += 1;
            }
        }
    }
    counts
}

fn pixel_of(screen: &MapScreen, county: u8) -> Option<(i32, i32)> {
    let clip = screen.map_clip();
    (clip.y0..clip.y1)
        .flat_map(|y| (clip.x0..clip.x1).map(move |x| (x, y)))
        .find(|&(x, y)| screen.county_at(x, y) == county)
}

fn visible_counties(screen: &MapScreen) -> usize {
    pick_counts(screen)[1..=14].iter().filter(|&&c| c > 0).count()
}


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
//! Neither is inside anything the village touches.
//! player saw.
//!
//! Standing on the picture are **eight clusters** of up to twenty-five peasant
//! icons each — seven jobs and *Idle townsfolk*, with iron and stone sharing
//! cluster 0 because a county's mine and its quarry are painted at the same
//! spot. One icon is `ceil(population / 25)` people.
//! twenty-five slots: `+0xB8` and the grid are the same fact seen twice.
//!
//! # The gesture, which is three screens in the original
//!
//! `Screen_HandleInput` treats the drag as its own screen ids, and reading them
//! is how the gesture is pinned down:
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
//! cluster contributes an icon.
//! * **Shortfall icons cannot be picked up.** The box-selector skips icon value
//!   1 explicitly, because those figures stand for workers the job *wants* and
//!   has not got. They are drawn and they are not there.
//!
//! # Where a drop lands, which is a file
//!
//! Not a rectangle. `FUN_004398F5` looks the pointer up in an 8-pixel grid that
//! turns out to be `vill_gd8.pl8` — see [`l2_view::village`], where the three
//! numbers that close it are set out. So the drop targets are painted, and this
//! screen reads them; on an install without the file
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

mod screen;
pub use screen::*;

use l2_kingdom::county::County;
use l2_kingdom::tables::{JOB_IDLE_TOWNSFOLK, JOB_NAMES};
use l2_view::village::{self as vill, ICONS_PER_CLUSTER};
use l2_view::{text, Canvas};

use crate::input::{Event, Key, Rect};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::widget;

/// **The rubber band's colour, `FUN_00412795`'s literal fifth argument.**
///
/// `FUN_00403cf4(x, y, w, h, 0x20)`, and `0x20` in `Base01.256` — the palette
/// `Screen_DrawCampaign` sets and the village never replaces,
/// `Village_Draw` paints over the campaign screen — is
/// `rgb(255, 255, 255)`. Read out of the player's own install; the file is one
/// 768-byte table of 6-bit VGA triples.
const BAND_INK: u8 = 0x20;

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
    pub(super) phase: Phase,
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
    /// `g_villageDragCount`.
    drag_count: i32,
    /// A click that has landed and is **waiting to find out whether it is half
    /// of a double click** — `DAT_004E65E8`, and the position it recorded in
    /// `DAT_004EAC04` / `DAT_004EAC08`. The counter is ticks remaining; at zero
    /// the click settles and opens the job popup, which is `DAT_004EABF0`.
    pending_click: Option<(i32, i32, u32)>,
    /// **Ours**: what just happened, for a player who cannot see a cursor
    /// change.
    status: String,
    /// **`Village_Animate`'s counters — display state and nothing else.**
    ///
    /// It lives on the screen, not in `Game` and not in `Kingdom`, and that is
    /// deliberate: `docs/netcode.md` D-12 forbids the simulation learning
    /// anything from a clock, and nothing in the lockstep digest or in a save
    /// may depend on which frame of the smoke is showing. Two clients whose
    /// villages are on different animation frames are not desynchronised.
    /// Closing the village and reopening it starts the loop again, which is
    /// also what the original does — `Village_Draw`'s reload path resets the
    /// counters through `FUN_004050C0`.
    clock: vill::AnimationClock,
    /// Whether the last tick moved an animation,
    /// repaint. The same economy `MapScreen`'s flag phase makes.
    animated: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::{Assets, Game};
    use l2_kingdom::tables::{JOB_CATTLE_FARMING, JOB_WOOD_CUTTING};

    pub(super) fn world() -> (Game, Assets) {
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


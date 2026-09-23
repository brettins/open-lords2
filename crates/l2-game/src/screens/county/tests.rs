    use super::*;

    fn inside(outer: Rect, inner: Rect) -> bool {
        inner.x >= outer.x
            && inner.y >= outer.y
            && inner.x + inner.w <= outer.x + outer.w
            && inner.y + inner.h <= outer.y + outer.h
    }

    #[test]
    fn every_panel_window_is_on_screen_and_holds_its_own_ok_button() {
        for p in PANELS {
            let w = p.window();
            assert!(w.x >= 0 && w.y >= 0, "{p:?} starts on screen");
            assert!(w.x + w.w <= 640 && w.y + w.h <= 480, "{p:?} ends on screen: {w:?}");
            assert!(inside(w, p.ok_button()), "{p:?}: the corner is outside its window {w:?}");
        }
    }

    #[test]
    fn only_the_two_order_panels_have_arrows_and_both_pairs_are_inside_them() {
        for p in PANELS {
            match p {
                Panel::Tax | Panel::Ration => {
                    let up = p.increase_button().expect("an up arrow");
                    let down = p.decrease_button().expect("a down arrow");
                    assert!(inside(p.window(), up), "{p:?} up arrow {up:?} escapes its window");
                    assert!(inside(p.window(), down), "{p:?} down arrow escapes its window");
                    assert!(up.x < down.x, "{p:?}: up is the left of the pair");
                    assert_eq!(up.y, down.y, "{p:?}: and they are on one line");
                    assert!(
                        down.x - up.x >= system::ARROW,
                        "{p:?}: the pair may touch but must not overlap"
                    );
                }
                _ => {
                    assert!(p.increase_button().is_none(), "{p:?} takes no orders");
                    assert!(p.decrease_button().is_none(), "{p:?} takes no orders");
                }
            }
        }
        assert_eq!(
            Panel::Tax.decrease_button().unwrap().x - Panel::Tax.increase_button().unwrap().x,
            32,
            "g_taxWidgets records 0 and 1 are at x 192 and 224"
        );
        assert_eq!(
            Panel::Ration.decrease_button().unwrap().x
                - Panel::Ration.increase_button().unwrap().x,
            24,
            "g_rationWidgets records 0 and 1 are at x 344 and 368, flush"
        );
    }

    #[test]
    fn the_strip_quadrants_fit_the_plate_and_leave_the_thermometer_unclickable() {
        let rects: Vec<Rect> = PANELS.iter().map(|p| p.strip_hotspot()).collect();
        for (i, a) in rects.iter().enumerate() {
            assert!(inside(STRIP, *a), "quadrant {i} {a:?} escapes the plate {STRIP:?}");
            for (j, b) in rects.iter().enumerate().skip(i + 1) {
                let over =
                    a.x < b.x + b.w && b.x < a.x + a.w && a.y < b.y + b.h && b.y < a.y + a.h;
                assert!(!over, "quadrants {i} and {j} overlap: {a:?} {b:?}");
            }
        }
        for x in THERMOMETER.0..THERMOMETER.0 + THERMOMETER_W {
            for r in &rects {
                assert!(!r.contains(x, 200), "thermometer column {x} is clickable in {r:?}");
            }
        }
        for r in &rects {
            assert!(r.w > 40 && r.h > 20, "{r:?} is too small to click");
        }
    }

    #[test]
    fn the_split_sliders_caps_track_and_knob_travel_agree() {
        assert_eq!(SLIDER_CAP_LEFT_X + system::SLIDER_CAP, SLIDER_TRACK_X, "left cap meets track");
        assert_eq!(SLIDER_TRACK_X + SLIDER_TRACK_W, SLIDER_CAP_RIGHT_X, "track meets right cap");
        let centre = |v: i32| SLIDER_KNOB_ORIGIN + v + system::SLIDER_KNOB_W / 2;
        assert_eq!(centre(0), SLIDER_TRACK_X + 1, "at 0 the knob centres on the track's start");
        assert_eq!(
            centre(MAX_RATION_SPLIT),
            SLIDER_TRACK_X + SLIDER_TRACK_W + 1,
            "at 100 it centres on the track's end"
        );
        assert!(inside(Panel::Ration.window(), split_track()), "the track is inside the panel");
    }

    #[test]
    fn no_panel_window_reaches_the_county_strip() {
        for p in PANELS {
            let w = p.window();
            assert!(
                w.x + w.w <= STRIP.x,
                "{p:?} at {w:?} overlaps the strip column at x = {}",
                STRIP.x
            );
        }
    }

    /// `Panel_RationSlider` draws the knob at `0xD8` and the caps at `0xDC`;
    /// `Ration_SliderClick` (`0x0043A379`) hit-tests `(200, 0xDC, 24, 24)`,
    /// `(0x145, 0xDC, 24, 24)` and `(0xE0, 0xDC, 0x66, 24)`. The literals here
/// are pinned from the decompilation
    /// constants, which is the difference between a test and a restatement.
    #[test]
    fn the_split_sliders_controls_are_four_pixels_below_its_knob() {
        assert_eq!(SLIDER_KNOB_Y, 0xD8, "Pl8_DrawFrame(System, 0x4C, …, 0xD8)");
        assert_eq!(SLIDER_CTRL_Y, 0xDC, "Rect_Contains(200, 0xDC, 0x18, 0x18)");
        for r in [split_down_button(), split_up_button(), split_track()] {
            assert_eq!(r.y, 0xDC, "every hit box in Ration_SliderClick is at 0xDC");
            assert_eq!(r.h, 0x18);
        }
        assert_eq!(split_down_button().x, 200);
        assert_eq!(split_up_button().x, 0x145, "0x145, one pixel right of the cap");
        assert_eq!(split_track().x, 0xE0);
        assert_eq!(split_track().w, 0x66);
    }

    #[test]
    fn armies_eat_is_the_one_option_that_changes_a_panels_shape() {
        assert_eq!(Panel::Ration.box_cells_for(false), (0x80, 0x60, 0x12, 0x0F));
        assert_eq!(Panel::Ration.box_cells_for(true), (0x80, 0x60, 0x12, 0x11));
        assert_eq!(Panel::Ration.ok_button_for(false).y, 0x0F * 0x10 + 0x44);
        assert_eq!(Panel::Ration.ok_button_for(true).y, 0x11 * 0x10 + 0x44);
        for p in [Panel::Population, Panel::Happiness, Panel::Tax] {
            assert_eq!(p.box_cells_for(false), p.box_cells_for(true), "{p:?}");
            assert_eq!(p.ok_button_for(false), p.ok_button_for(true), "{p:?}");
        }
    }

    #[test]
    fn the_ration_panels_numbers_centre_under_their_icons() {
        const ICON_X: [i32; 3] = [224, 284, 344];
        const ICON_W: [i32; 3] = [36, 37, 23];
        for i in 0..3 {
            let centre = FOOD_COL_X[i] + FOOD_COL_W / 2;
            let icon_centre = ICON_X[i] + ICON_W[i] / 2;
            assert!(
                (centre - icon_centre).abs() <= 4,
                "column {i}: number centres at {centre}, icon at {icon_centre}"
            );
            assert!(FOOD_COL_X[i] < ICON_X[i], "column {i} would be left of its icon");
        }
    }

    /// The five `L2.eng` groups these panels draw, and the indices that are
    /// **not** drawn — because a constant that names an absence is the only
    /// thing that stops the next reader re-discovering it.
    #[test]
    fn the_groups_are_the_painters_literal_arguments() {
        assert_eq!((g73::GROUP, g85::GROUP, g86::GROUP, g87::GROUP), (73, 85, 86, 87));
        assert_eq!(g61::GROUP, 61, "the strip's captions are group 61, not the tax panel's");
        assert_eq!((GROUP_HEALTH_BANDS, GROUP_RATION_LEVELS), (20, 21));
        assert_eq!(CROWN_NOUN, 0, "L2.eng 8/0 and 8/1, Crown. / Crowns.");

        assert_eq!(g86::TITLE_NEVER_DRAWN.index, 0);
        for l in [g86::RATE, g86::PEOPLE_PAY, g86::THIS_COUNTY, g86::OTHER_COUNTIES] {
            assert_ne!(l.index, g86::TITLE_NEVER_DRAWN.index);
        }
        assert_eq!(g73::PEOPLE.index, 9);
        for i in g87::UNDRAWN {
            for l in [g87::TITLE, g87::WANTED, g87::ACHIEVED, g87::HEALTH, g87::EATEN, g87::FED] {
                assert_ne!(l.index, i, "87/{i} is in UNDRAWN and also drawn");
            }
            assert_ne!(g87::FORAGING.index, i);
        }
    }

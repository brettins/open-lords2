//! Input, as the screens see it.
//!
//! The point of this module is that it names **no** windowing type. `winit`'s
//! `KeyCode`, its modifiers and its device ids stop at `main.rs`, which
//! translates them into the handful of things a screen can actually respond to.
//! Two things follow from that: a test can deliver a click without opening a
//! window, and swapping the windowing library later is a change to one file.
//!
//! Nothing here carries a timestamp. A screen that could ask *when* an event
//! arrived is a screen that could feed the clock into the simulation, and
//! `docs/netcode.md` does not allow that.

/// **Window coordinates to canvas pixels**, and it is a pure function of two
/// integers so that it can be checked without a window.
///
/// The picture is 640 × 480 and the window is whatever the player dragged it
/// to. We scale by a **whole number** — a 1996 sprite game at a fractional
/// scale gets pixels of two different widths in the same row, which is visible
/// on every diagonal in the tile art — and centre what is left, so a window
/// that is not an exact multiple has black borders. At the size the bug was
/// reported from, 1898 × 1562, the scale is 2 and the picture is 1280 × 960
/// with 309 pixels of border on each side and 301 above and below.
///
/// **The border is not a dead zone.** A position outside the picture is clamped
/// to the nearest canvas pixel rather than discarded, and that is the whole of
/// the fix for *"I can't scroll the map"*: the original edge-scrolls while the
/// cursor sits on the outermost pixel of the screen, it ran full-screen so its
/// screen edge was its window edge, and clamping restores that relationship at
/// any window size. Push the cursor into the black band and the game sees
/// column 0 or column 639.
pub mod window {
    /// The canvas the game always draws, whatever the window is doing.
    pub const CANVAS_W: u32 = 640;
    pub const CANVAS_H: u32 = 480;

    /// The whole-number factor the picture is drawn at: the largest that fits,
    /// never less than one — the same arithmetic `pixels`' integer scaling mode
    /// does, and the same one the F5 window snap uses to pick a size.
    pub fn scale(window_w: u32, window_h: u32) -> u32 {
        (window_w / CANVAS_W).min(window_h / CANVAS_H).max(1)
    }

    /// Where the picture's top-left corner sits in the window. Negative when
    /// the window is smaller than one whole scale, which is the one case where
    /// the picture is cropped rather than bordered.
    pub fn origin(window_w: u32, window_h: u32) -> (i32, i32) {
        let s = scale(window_w, window_h) as i32;
        (
            (window_w as i32 - CANVAS_W as i32 * s) / 2,
            (window_h as i32 - CANVAS_H as i32 * s) / 2,
        )
    }

    /// The canvas pixel a window position lands on, **clamped into the canvas**.
    pub fn to_canvas(window_w: u32, window_h: u32, x: f64, y: f64) -> (i32, i32) {
        let s = scale(window_w, window_h) as i32;
        let (ox, oy) = origin(window_w, window_h);
        let px = (x.floor() as i32 - ox).div_euclid(s);
        let py = (y.floor() as i32 - oy).div_euclid(s);
        (px.clamp(0, CANVAS_W as i32 - 1), py.clamp(0, CANVAS_H as i32 - 1))
    }

    /// The window size F5 snaps to: the picture at a whole scale and nothing
    /// else, so there are no borders at all.
    ///
    /// The key is the original's. With `g_optFullScreen` clear the game draws
    /// the caption *"(F5 key re-sizes window to 640x480)"*, so it shipped a key
    /// that snaps the window to a whole scale; **ours snaps to the largest that
    /// fits rather than always to 1×**, which is the one thing here that is not
    /// the original's and is written down as such.
    pub fn snapped(window_w: u32, window_h: u32) -> (u32, u32) {
        let s = scale(window_w, window_h);
        (CANVAS_W * s, CANVAS_H * s)
    }
}

/// The keys the interface uses. Deliberately a short list: a key that no screen
/// handles has no business having a name here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Key {
    Escape,
    Enter,
    Space,
    /// Only the save screen's name field reads it. It is here rather than being
    /// folded into [`Key::Char`] because a text field has to tell "the player
    /// typed a backspace" from "the player typed a character", and no other
    /// screen in this crate has a text field at all.
    Backspace,
    Up,
    Down,
    Left,
    Right,
    /// A printable character, already folded to uppercase so a screen never
    /// has to match both cases.
    Char(char),
    /// The same, with **Control** held.
    ///
    /// A separate variant rather than a modifier field, because in the original
    /// it is a separate *dispatch*, not a qualifier: the window procedure
    /// (`0x004B29BE`) latches `VK_CONTROL` into `DAT_004DF3A8` on key-down and
    /// clears it on key-up, and its `0x31` … `0x39` arm is
    /// `if (DAT_004DF3A8 == 0) FUN_0043C910(key); else FUN_0043C885(key);` —
    /// **two different functions**, recall a control group and store one. A flag
    /// on [`Key::Char`] would invite a screen to handle the digit and then
    /// branch, which is the shape that produces a modifier that half-works.
    ///
    /// Only the battlefield reads it, and only for the nine digits.
    CtrlChar(char),
}

impl Key {
    pub fn letter(c: char) -> Key {
        Key::Char(c.to_ascii_uppercase())
    }

    pub fn ctrl_letter(c: char) -> Key {
        Key::CtrlChar(c.to_ascii_uppercase())
    }
}

/// Something the player did. Canvas coordinates, not window coordinates: the
/// window may be any size, and the game is always 640 x 480.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    /// A key went down. Key *repeat* is delivered as further presses, which is
    /// what makes holding an arrow key step a value.
    KeyDown(Key),
    /// The pointer moved to a canvas pixel.
    ///
    /// **Clamped into the canvas, not dropped.** The window is scaled by an
    /// integer factor and centred, so unless it is an exact multiple of
    /// 640 × 480 there are black borders around the picture and the pointer can
    /// be in one of them. `main.rs` reports the nearest canvas pixel rather
    /// than nothing at all, which is what makes the edge of the *window* behave
    /// like the edge of the *screen* — and the original's edge scroll is a
    /// gesture against the edge of the screen.
    Pointer { x: i32, y: i32 },
    /// The pointer left the window.
    ///
    /// Not a windowing detail leaking through: without it, a screen holding the
    /// last reported position cannot tell "still at the left edge" from "gone",
    /// and the map would scroll for ever after the cursor walked off it.
    PointerLeft,
    /// The left button went down at a canvas pixel.
    Click { x: i32, y: i32 },
    /// The **right** button came up at a canvas pixel.
    ///
    /// A separate event rather than a button field on [`Event::Click`], because
    /// in this game the two buttons do unrelated jobs rather than the same job
    /// with a modifier: the right button *dismisses* an overlay — the game says
    /// so itself, in `L2.eng` group 12 index 0, *"Click Right to Exit"* — and on
    /// the campaign map it *opens* one. A flag would invite every hit test to
    /// run for both buttons and then branch, which is the shape that produces a
    /// right-click that half-works.
    ///
    /// **Release, not press.** The original's dispatcher reads `DAT_004E6900`,
    /// the right button *released this frame*, in all fifty-odd places it acts
    /// on the right button; the press flag beside it (`DAT_004EABE0`) is read
    /// five times and never to dismiss anything. Right double-clicks are
    /// computed and never read at all.
    RightClick { x: i32, y: i32 },
    /// The left button was **double-clicked** at a canvas pixel.
    ///
    /// **This is the original's own event, not a convenience.** `Lords2.exe`'s
    /// window procedure (`0x004B29BE`) handles message `0x203`
    /// — `WM_LBUTTONDBLCLK` — by setting bit 0 of `DAT_004EADA1`, and the frame
    /// poll at `0x004B2D5A` turns that into `DAT_004EABC5`. So the double click
    /// is a *different verb* from the click, dispatched from a different flag,
    /// and folding it into two [`Event::Click`]s would lose the distinction the
    /// game makes.
    ///
    /// **Corrected.** This paragraph said the flag was "read by exactly one
    /// input arm: `Village_DoubleClick` (`0x00439DF0`)", and the input audit
    /// (`docs/decisions.md` C61) falsified it without looking for it. There are
    /// **two** readers, and the second is on the battlefield:
    /// `FUN_0043BF07` (`0x0043BF07`) tests
    /// `(g_mouseLeftReleased || g_mouseLeftDoubleClick) && g_screenId == 0x2A`,
    /// so a double click **commits an open selection box** exactly as a release
    /// would. That is not a second verb — it is the same verb reached a second
    /// way, and it exists precisely because of the sentence below: without it,
    /// the second click of a fast double click would leave the drag open with no
    /// press to end it. `Hotspot_Test` (`0x0040E3EE`) also reads the flag for
    /// its kind-2 widgets, which is a third reader and not an arm.
    ///
    /// **Windows sends it instead of the second press**, not as well as it: the
    /// second `WM_LBUTTONDOWN` never arrives, which is why the original's
    /// button-down flag stays clear through a double click and why `main.rs`
    /// delivers this in place of the second [`Event::Click`]. The
    /// [`Event::Release`] that ends it still arrives, as `WM_LBUTTONUP` does.
    DoubleClick { x: i32, y: i32 },
    /// The left button came **up** at a canvas pixel.
    ///
    /// Added for the village, and it is not a convenience. The original's
    /// peasant drag is a three-state machine on `g_screenId` — 0x02 idle, 0x05
    /// while the band is being drawn, 0x06 while the selection is being carried
    /// — and the transition out of 0x05 is `FUN_00439541`, which fires **when
    /// the button is released**, not when it is next pressed. A screen that
    /// only ever hears about presses cannot tell a drag from two clicks, and
    /// would have had to invent a gesture the original does not have.
    Release { x: i32, y: i32 },
}

/// A rectangle in canvas coordinates, and the hit test that goes with it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl Rect {
    pub const fn new(x: i32, y: i32, w: i32, h: i32) -> Rect {
        Rect { x, y, w, h }
    }

    /// Half-open on both axes, so two rectangles that share an edge never both
    /// claim the same pixel.
    pub fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.x && x < self.x + self.w && y >= self.y && y < self.y + self.h
    }

    pub fn centre_x(&self) -> i32 {
        self.x + self.w / 2
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_rectangle_is_half_open_so_neighbours_do_not_overlap() {
        let a = Rect::new(0, 0, 10, 4);
        let b = Rect::new(10, 0, 10, 4);
        assert!(a.contains(0, 0));
        assert!(a.contains(9, 3));
        assert!(!a.contains(10, 0), "the right edge belongs to the next rectangle");
        assert!(b.contains(10, 0));
        assert!(!a.contains(-1, 0));
        assert!(!a.contains(0, 4));
    }

    /// **The transform, at the window size the bug was reported from.**
    ///
    /// 1898 × 1562 is more than 2 × 640 and less than 3 × 640, so the picture is
    /// 1280 × 960 centred with wide borders. The three cases that matter are the
    /// three the coordinator asked for, and the middle one is the bug: a
    /// pointer *in the border* must still reach the edge column, or the black
    /// band eats the gesture.
    #[test]
    fn a_pointer_in_the_letterbox_border_still_lands_on_the_edge_of_the_canvas() {
        use window::*;
        let (w, h) = (1898u32, 1562u32);
        assert_eq!(scale(w, h), 2, "1898/640 is 2, and 1562/480 is 3");
        assert_eq!(origin(w, h), (309, 301), "(1898 - 1280) / 2, (1562 - 960) / 2");

        // 1. The content's own edges.
        assert_eq!(to_canvas(w, h, 309.0, 781.0).0, 0, "the picture's left column");
        assert_eq!(to_canvas(w, h, 1588.0, 781.0).0, 639, "and its right one");

        // 2. The border. **This is the case that was broken.**
        assert_eq!(to_canvas(w, h, 0.0, 781.0).0, 0, "the far left of the window");
        assert_eq!(to_canvas(w, h, 1897.0, 781.0).0, 639, "the far right");
        assert_eq!(to_canvas(w, h, 949.0, 0.0).1, 0, "the top");
        assert_eq!(to_canvas(w, h, 949.0, 1561.0).1, 479, "the bottom");

        // 3. One pixel inside the content is not an edge, at either end.
        assert_eq!(to_canvas(w, h, 311.0, 781.0).0, 1);
        assert_eq!(to_canvas(w, h, 1586.0, 781.0).0, 638);

        // And the middle of the window is the middle of the canvas.
        assert_eq!(to_canvas(w, h, 949.0, 781.0), (320, 240));
    }

    /// The scale is whole and never zero, the snap leaves no border at all, and
    /// snapping is idempotent — pressing F5 twice does not shrink the window
    /// again.
    #[test]
    fn the_whole_number_scale_holds_at_every_window_size_worth_naming() {
        use window::*;
        for (w, h, want) in [
            (640, 480, 1),
            (639, 479, 1),   // smaller than 1x: still 1, and the picture crops
            (1, 1, 1),
            (1280, 960, 2),
            (1898, 1562, 2), // the player's window
            (1920, 1080, 2), // 1080p: three columns fit but only two rows
            (1920, 1440, 3),
            (3840, 2160, 4),
        ] {
            assert_eq!(scale(w, h), want, "{w}x{h}");
            let (sw, sh) = snapped(w, h);
            assert_eq!((sw, sh), (640 * want, 480 * want));
            assert_eq!(origin(sw, sh), (0, 0), "a snapped window has no border");
            assert_eq!(snapped(sw, sh), (sw, sh), "and snapping it again changes nothing");
        }
    }

    /// Every window position lands on the canvas, and every canvas pixel is
    /// reachable — which is what makes the edge of the *window* behave like the
    /// edge of the *screen* the original scrolled from.
    #[test]
    fn the_transform_is_total_and_onto() {
        use window::*;
        let (w, h) = (1898u32, 1562u32);
        let mut seen_x = vec![false; 640];
        for x in 0..w {
            let (px, py) = to_canvas(w, h, x as f64, 781.0);
            assert!((0..640).contains(&px) && (0..480).contains(&py), "window x {x} -> {px}");
            seen_x[px as usize] = true;
        }
        assert!(seen_x.iter().all(|&s| s), "every canvas column is reachable");
    }

    #[test]
    fn letters_are_folded_so_a_screen_matches_one_case() {
        assert_eq!(Key::letter('e'), Key::Char('E'));
        assert_eq!(Key::letter('E'), Key::Char('E'));
    }
}

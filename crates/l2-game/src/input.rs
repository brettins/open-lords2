
pub mod window {
    pub const CANVAS_W: u32 = 640;
    pub const CANVAS_H: u32 = 480;

    pub fn scale(window_w: u32, window_h: u32) -> u32 {
        (window_w / CANVAS_W).min(window_h / CANVAS_H).max(1)
    }

    pub fn origin(window_w: u32, window_h: u32) -> (i32, i32) {
        let s = scale(window_w, window_h) as i32;
        (
            (window_w as i32 - CANVAS_W as i32 * s) / 2,
            (window_h as i32 - CANVAS_H as i32 * s) / 2,
        )
    }

    pub fn to_canvas(window_w: u32, window_h: u32, x: f64, y: f64) -> (i32, i32) {
        let s = scale(window_w, window_h) as i32;
        let (ox, oy) = origin(window_w, window_h);
        let px = (x.floor() as i32 - ox).div_euclid(s);
        let py = (y.floor() as i32 - oy).div_euclid(s);
        (px.clamp(0, CANVAS_W as i32 - 1), py.clamp(0, CANVAS_H as i32 - 1))
    }

    pub fn snapped(window_w: u32, window_h: u32) -> (u32, u32) {
        let s = scale(window_w, window_h);
        (CANVAS_W * s, CANVAS_H * s)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Key {
    Escape,
    Enter,
    Space,
    Backspace,
    Up,
    Down,
    Left,
    Right,
    /// `VK_HOME` — `Edit_Home` (`0x00401CFC`). Caret to the start.
    Home,
    /// `VK_END` — `Edit_End` (`0x00401D11`). Caret to the end.
    ///
    /// The original's arm does two things, not one: `Edit_End` *and*
    /// `Chat_Close` (`0x004360F2`). The second half is multiplayer chat and is
    /// not reproduced.
    End,
    /// `VK_INSERT` — `Edit_ToggleInsert` (`0x00401CA3`), which is `g_editInsert
    /// ^= 1`.
    Insert,
    /// `VK_DELETE` — `Edit_Delete` (`0x00401C93` → `0x00401DC8`). The tail
    /// shifts left over the caret.
    Delete,
    Char(char),
/// A separate variant, because in the original
    /// it is a separate *dispatch*, not a qualifier: the window procedure
    /// (`0x004B29BE`) latches `VK_CONTROL` into `DAT_004DF3A8` on key-down and
    /// clears it on key-up, and its `0x31` … `0x39` arm is
    /// `if (DAT_004DF3A8 == 0) FUN_0043C910(key); else FUN_0043C885(key);` —
    /// **two different functions**, recall a control group and store one. A flag
    /// on [`Key::Char`] would invite a screen to handle the digit and then
    /// branch, which is the shape that produces a modifier that half-works.
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    /// **`WM_KEYDOWN` (`0x100`)**, and the original's window procedure
    /// (`0x004B29BE`) dispatches it on virtual-key codes: the editing keys, the
    /// nine control-group digits, the function keys and Escape.
    KeyDown(Key),
    Text(char),
    Pointer { x: i32, y: i32 },
    PointerLeft,
    Click { x: i32, y: i32 },
/// A separate event, because
/// in this game the two buttons do unrelated jobs
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
    ///
    /// `FUN_0043BF07` (`0x0043BF07`) tests
    /// `(g_mouseLeftReleased || g_mouseLeftDoubleClick) && g_screenId == 0x2A`,
/// so a double click **commits an open selection box**
/// would. It is the same verb reached a second
/// way, and it exists because of the sentence below: without it,
    /// the second click of a fast double click would leave the drag open with no
    /// press to end it. `Hotspot_Test` (`0x0040E3EE`) also reads the flag for
    /// its kind-2 widgets, which is a third reader and not an arm.
    DoubleClick { x: i32, y: i32 },
/// Added for the village. The original's
    /// peasant drag is a three-state machine on `g_screenId` — 0x02 idle, 0x05
    /// while the band is being drawn, 0x06 while the selection is being carried
    /// — and the transition out of 0x05 is `FUN_00439541`, which fires **when
    /// the button is released**, not when it is next pressed. A screen that
/// only ever hears about presses cannot tell a drag from two clicks, and
    /// would have had to invent a gesture the original does not have.
    Release { x: i32, y: i32 },
    /// **`g_mouseRightPressed` (`0x004EABE0`)**, the down edge beside
    /// [`Event::RightClick`]'s `g_mouseRightReleased` (`0x004E6900`). `[V]`
    /// `docs/symbols.md`: the released flag is what all forty-eight
    /// *click-right-to-exit* arms read, and this one is read five times and
    /// never to dismiss anything —.
    ///
    /// **The arm that needed it** is `Screen_FrameInput`'s epilogue
    /// (`0x0042FF10`), whose guard is `g_mouseLeftPressed || g_mouseRightPressed`
    /// over the minimap raster: the two buttons do the *same* job there, on the
    /// same edge, and answering the right half with the release would fire it
    /// one edge late. See [`crate::screens::tip`].
    RightPress { x: i32, y: i32 },
}

/// `App_WndProc` (`0x004B29BE`) keeps one bit for the left button —
/// `DAT_004EABC2 & 1` — and three messages touch it:
///
/// ```c
/// case 0x201: DAT_004EABC2 |= 1;        /* WM_LBUTTONDOWN   */
/// case 0x202: DAT_004EABC2 &= 0xFE;     /* WM_LBUTTONUP     */
/// case 0x203: DAT_004EADA1 |= 1;        /* WM_LBUTTONDBLCLK */
/// ```
///
/// The frame poll (`0x004B2CF7`) then derives the two edges from a change in
/// that bit and from nothing else:
///
/// ```c
/// DAT_004EABBC = g_mouseLeftDown;                 /* last frame's */
/// g_mouseLeftDown = (DAT_004EABC2 & 1) != 0;
/// if (DAT_004EABBC != g_mouseLeftDown) {
///     if (g_mouseLeftDown) g_mouseLeftPressed = 1; else g_mouseLeftReleased = 1;
/// }
/// ```
///
/// So **the `WM_LBUTTONUP` that ends a double click raises no release**: the
/// bit, clearing it changes nothing, and
/// edge. `[V]` `docs/input.md` §5 said so and said we still delivered one; this
/// is the type that stops it, and it lives here so a
/// test can drive it without a window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LeftButton {
    /// `DAT_004EABC2 & 1`.
    down: bool,
}

impl LeftButton {
    pub const fn new() -> LeftButton {
        LeftButton { down: false }
    }

    pub fn is_down(&self) -> bool {
        self.down
    }

    pub fn pressed(&mut self, x: i32, y: i32) -> Event {
        self.down = true;
        Event::Click { x, y }
    }

    ///. Windows sends this *instead of* the second
    pub fn double_clicked(&mut self, x: i32, y: i32) -> Event {
        Event::DoubleClick { x, y }
    }

    pub fn released(&mut self, x: i32, y: i32) -> Option<Event> {
        core::mem::take(&mut self.down).then_some(Event::Release { x, y })
    }
}

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

    #[test]
    fn a_pointer_in_the_letterbox_border_still_lands_on_the_edge_of_the_canvas() {
        use window::*;
        let (w, h) = (1898u32, 1562u32);
        assert_eq!(scale(w, h), 2, "1898/640 is 2, and 1562/480 is 3");
        assert_eq!(origin(w, h), (309, 301), "(1898 - 1280) / 2, (1562 - 960) / 2");

        assert_eq!(to_canvas(w, h, 309.0, 781.0).0, 0, "the picture's left column");
        assert_eq!(to_canvas(w, h, 1588.0, 781.0).0, 639, "and its right one");

        assert_eq!(to_canvas(w, h, 0.0, 781.0).0, 0, "the far left of the window");
        assert_eq!(to_canvas(w, h, 1897.0, 781.0).0, 639, "the far right");
        assert_eq!(to_canvas(w, h, 949.0, 0.0).1, 0, "the top");
        assert_eq!(to_canvas(w, h, 949.0, 1561.0).1, 479, "the bottom");

        assert_eq!(to_canvas(w, h, 311.0, 781.0).0, 1);
        assert_eq!(to_canvas(w, h, 1586.0, 781.0).0, 638);

        assert_eq!(to_canvas(w, h, 949.0, 781.0), (320, 240));
    }

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

    /// **Ablation, run:** make `released` return `Some` unconditionally — which
    /// is what `main.rs` did — and the fourth assertion goes red with a
    /// `Release` the original never raises.
    #[test]
    fn the_button_up_that_ends_a_double_click_raises_no_release() {
        let mut b = LeftButton::new();
        assert_eq!(b.pressed(10, 20), Event::Click { x: 10, y: 20 });
        assert!(b.is_down());
        assert_eq!(b.released(10, 20), Some(Event::Release { x: 10, y: 20 }));
        assert!(!b.is_down());
        assert_eq!(b.double_clicked(10, 20), Event::DoubleClick { x: 10, y: 20 });
        assert!(!b.is_down(), "0x203 sets DAT_004EADA1, never the down bit");
        assert_eq!(b.released(10, 20), None, "no edge, so no g_mouseLeftReleased");
    }

    #[test]
    fn a_release_with_no_press_behind_it_is_not_an_edge() {
        let mut b = LeftButton::new();
        assert_eq!(b.released(0, 0), None);
        b.pressed(0, 0);
        assert!(b.released(0, 0).is_some());
        assert_eq!(b.released(0, 0), None, "and the second up changes nothing");
    }

    #[test]
    fn letters_are_folded_so_a_screen_matches_one_case() {
        assert_eq!(Key::letter('e'), Key::Char('E'));
        assert_eq!(Key::letter('E'), Key::Char('E'));
    }
}

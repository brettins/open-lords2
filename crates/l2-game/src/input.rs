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

/// The keys the interface uses. Deliberately a short list: a key that no screen
/// handles has no business having a name here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Key {
    Escape,
    Enter,
    Space,
    Up,
    Down,
    Left,
    Right,
    /// A printable character, already folded to uppercase so a screen never
    /// has to match both cases.
    Char(char),
}

impl Key {
    pub fn letter(c: char) -> Key {
        Key::Char(c.to_ascii_uppercase())
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
    Pointer { x: i32, y: i32 },
    /// The left button went down at a canvas pixel.
    Click { x: i32, y: i32 },
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

    #[test]
    fn letters_are_folded_so_a_screen_matches_one_case() {
        assert_eq!(Key::letter('e'), Key::Char('E'));
        assert_eq!(Key::letter('E'), Key::Char('E'));
    }
}

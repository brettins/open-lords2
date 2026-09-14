//! **`Gfx_MarkDirty`'s accumulator** — the original's one repaint rectangle.
//!
//! `Gfx_MarkDirty` (`0x00452306`) is not a list.
//! rectangle, grown by every marker in the frame and blitted once:
//!
//! ```c
//! void Gfx_MarkDirty(int left, int top, int right, int bottom) {
//!   if (g_dirtyCount < 1) g_dirtyCount = 1;
//!   if (left < g_dirtyLeft)     g_dirtyLeft   = left;
//!   if (g_dirtyRight < right)   g_dirtyRight  = right;
//!   if (top < g_dirtyTop)       g_dirtyTop    = top;
//!   if (g_dirtyBottom < bottom) g_dirtyBottom = bottom;
//!   /* clamps */
//! }
//! ```
//!
//! `Gfx_Present` (`0x00452160`) blits only `if (0 < g_dirtyCount)`
//! and then decrements it, resetting the rectangle to its empty sentinel when
//! the count reaches zero. So **drawing is never gated** — every painter draws
//! into the back buffer whenever it is called — and what a mark decides is
//! whether the frame reaches the screen at all. **[V]**, all three bodies read.
//!
//! `Gfx_MarkAllDirty` (`0x0045240A`) is one line, `Gfx_MarkDirty(0, 0, 0x27F,
//! 0x1DF)`: the whole 640 × 480 frame, and eighty-six callers use it as *"I
//! repainted the page."* `Gfx_MarkSpriteDirty` (`0x004527A6`) is the other
//! marker, in cells: `Gfx_MarkDirty(x, y, w*0x10 + x, h*0x10 + y)`, plus
//! `if (1 < level) g_dirtyCount = 2`
//!
//! **The clamp at the bottom is the original's own defect and is kept**: the
//! last guard reads `if (0x27F < g_dirtyBottom) g_dirtyBottom = 0x1DF`, testing
//! the bottom against the *width*. A mark between 0x1E0 and 0x27F therefore
//! survives past the last row. Nothing reaches it — `Gfx_MarkAllDirty` passes
//! 0x1DF and the sprite marker's callers are all on screen —
//! picture; it is reproduced because `docs/bugs.md` keeps defects that cannot
//! be told from the rule by looking at one frame.
//!
//! Our canvas is still presented whole. This type records *what the original
//! would have blitted*
//! names, which is the half that is a specification.

/// The screen the original marks against: `Gfx_MarkAllDirty`'s literals.
pub const SCREEN_RIGHT: i32 = 0x27F;
pub const SCREEN_BOTTOM: i32 = 0x1DF;

/// `g_dirtyCount`, `g_dirtyLeft`, `g_dirtyTop`, `g_dirtyRight`, `g_dirtyBottom`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Dirty {
    count: u8,
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

impl Default for Dirty {
    fn default() -> Dirty {
        Dirty::EMPTY
    }
}

impl Dirty {
    /// `Gfx_Present`'s reset arm: left and top to the last column and row,
    /// right and bottom to zero, so the first mark of the next frame wins every
    /// comparison.
    pub const EMPTY: Dirty =
        Dirty { count: 0, left: SCREEN_RIGHT, top: SCREEN_BOTTOM, right: 0, bottom: 0 };

    /// `Gfx_MarkDirty(left, top, right, bottom)` — `0x00452306`.
    pub fn mark(&mut self, left: i32, top: i32, right: i32, bottom: i32) {
        self.count = self.count.max(1);
        self.left = self.left.min(left);
        self.right = self.right.max(right);
        self.top = self.top.min(top);
        self.bottom = self.bottom.max(bottom);
        self.left = self.left.max(0);
        self.top = self.top.max(0);
        self.right = self.right.min(SCREEN_RIGHT);
        // The original's own test: the bottom is compared against 0x27F, not
        // 0x1DF, and only then clamped to 0x1DF. Kept.
        if self.bottom > SCREEN_RIGHT {
            self.bottom = SCREEN_BOTTOM;
        }
    }

    /// `Gfx_MarkAllDirty()` — `0x0045240A`, the whole frame.
    pub fn mark_all(&mut self) {
        self.mark(0, 0, SCREEN_RIGHT, SCREEN_BOTTOM);
    }

    /// `Gfx_MarkSpriteDirty(x, y, wCells, hCells, level)` — `0x004527A6`, a
    /// rectangle in 16-pixel cells. `level > 1` asks for two presents.
    pub fn mark_sprite(&mut self, x: i32, y: i32, w_cells: i32, h_cells: i32, level: u8) {
        let (x, y) = (x.max(0), y.max(0));
        self.mark(x, y, w_cells * 0x10 + x, h_cells * 0x10 + y);
        if level > 1 {
            self.count = 2;
        }
    }

    /// What `Gfx_Present` would blit this frame, and the count it leaves
    /// behind: `Some((left, top, right, bottom))` while `0 < g_dirtyCount`, and
    /// the rectangle is cleared on the last one.
    pub fn take(&mut self) -> Option<(i32, i32, i32, i32)> {
        if self.count == 0 {
            return None;
        }
        let r = (self.left, self.top, self.right, self.bottom);
        self.count -= 1;
        if self.count == 0 {
            *self = Dirty::EMPTY;
        }
        Some(r)
    }

    /// Whether anything is waiting to be presented — `0 < g_dirtyCount`.
    pub fn pending(&self) -> bool {
        self.count > 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two marks are one rectangle, and `Gfx_Present` blits it once.
    #[test]
    fn marks_union_into_one_rectangle() {
        let mut d = Dirty::EMPTY;
        assert_eq!(d.take(), None);
        d.mark(0x10, 0x20, 0x30, 0x40);
        d.mark(0x08, 0x60, 0x20, 0x70);
        assert_eq!(d.take(), Some((0x08, 0x20, 0x30, 0x70)));
        assert_eq!(d.take(), None, "the count is one, so the second present has nothing");
    }

    /// `Gfx_MarkAllDirty` = `Gfx_MarkDirty(0, 0, 0x27F, 0x1DF)`.
    #[test]
    fn mark_all_is_the_whole_frame() {
        let mut d = Dirty::EMPTY;
        d.mark_all();
        assert_eq!(d.take(), Some((0, 0, SCREEN_RIGHT, SCREEN_BOTTOM)));
    }

    /// `Gfx_MarkSpriteDirty`'s cells, and its `level > 1` second present.
    #[test]
    fn sprite_marks_cells_and_level_two_presents_twice() {
        let mut d = Dirty::EMPTY;
        d.mark_sprite(0x58, 0x9D, 8, 8, 1);
        assert_eq!(d.take(), Some((0x58, 0x9D, 0xD8, 0x11D)));
        assert_eq!(d.take(), None);

        let mut d = Dirty::EMPTY;
        d.mark_sprite(0x58, 0x9D, 8, 8, 2);
        assert_eq!(d.take(), Some((0x58, 0x9D, 0xD8, 0x11D)));
        assert_eq!(d.take(), Some((0x58, 0x9D, 0xD8, 0x11D)), "level 2 blits the flip as well");
        assert_eq!(d.take(), None);
    }

    /// **The original's clamp defect, kept.** The last guard tests the bottom
    /// against 0x27F and clamps to 0x1DF,
    /// row — survives, and one of 0x280 is pulled back onto it.
    #[test]
    fn the_bottom_clamp_tests_the_width() {
        let mut d = Dirty::EMPTY;
        d.mark(0, 0, 0x10, 0x200);
        assert_eq!(d.take().map(|r| r.3), Some(0x200), "0x200 is not > 0x27F, so it stands");

        let mut d = Dirty::EMPTY;
        d.mark(0, 0, 0x10, 0x280);
        assert_eq!(d.take().map(|r| r.3), Some(SCREEN_BOTTOM));
    }
}

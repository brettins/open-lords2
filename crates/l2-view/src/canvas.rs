//! An indexed 640x480 framebuffer and the two blitters the original uses.
//!
//! The canvas holds **palette indices**, not colours. That is the whole point:
//! the endgame for this crate is a pixel-for-pixel diff against `Lords2.exe`'s
//! own framebuffer, and the original thinks in indices. Colour only appears at
//! the very last step, when `present` expands indices through a `.256` palette.
//!
//! Two blitters, because the original has two and they differ:
//!
//! * **Sprites** skip palette index 0 — `Blit_Unclipped` (`0x004B43B1`) tests
//!   each byte and leaves the destination alone when it is zero.
//! * **Terrain tiles** do not. `Battlefield_Draw32` (`0x004BCBDC`) reaches
//!   `FUN_004B5333`, a straight copy. **[V]** and, as it happens, unobservable
//!   on a field battlefield: no frame of `T32_bat1.pl8` contains a single index-0
//!   pixel, checked over all 252 frames. The distinction is kept because it is
//!   real, not because it currently changes a pixel.

use l2_formats::DecodedFrame;

pub const WIDTH: usize = 640;
pub const HEIGHT: usize = 480;

/// The game's transparent palette index. An unpainted canvas reads as "nothing
/// drawn here" rather than as a colour.
pub const TRANSPARENT: u8 = 0;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Canvas {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<u8>,
}

impl Canvas {
    pub fn new(width: usize, height: usize) -> Self {
        Canvas { width, height, pixels: vec![TRANSPARENT; width * height] }
    }

    /// The original's screen size.
    pub fn screen() -> Self {
        Canvas::new(WIDTH, HEIGHT)
    }

    pub fn clear(&mut self, index: u8) {
        self.pixels.fill(index);
    }

    #[inline]
    pub fn at(&self, x: usize, y: usize) -> u8 {
        self.pixels[y * self.width + x]
    }

    #[inline]
    pub fn set(&mut self, x: usize, y: usize, index: u8) {
        let (w, h) = (self.width, self.height);
        if x < w && y < h {
            self.pixels[y * w + x] = index;
        }
    }

    /// How many pixels differ from `other`. The measurement the eventual
    /// differential test against the original is built on.
    pub fn diff_count(&self, other: &Canvas) -> usize {
        self.pixels.iter().zip(other.pixels.iter()).filter(|(a, b)| a != b).count()
    }

    /// Count of pixels holding a given index. Cheap and enough to assert that
    /// something was drawn without pinning exact artwork.
    pub fn count(&self, index: u8) -> usize {
        self.pixels.iter().filter(|&&p| p == index).count()
    }

    /// Blit honouring index-0 transparency. Sprites.
    pub fn blit(&mut self, frame: &DecodedFrame, ox: i32, oy: i32) {
        self.blit_inner(frame, ox, oy, true)
    }

    /// Blit every pixel, transparent index included. Terrain tiles.
    pub fn blit_opaque(&mut self, frame: &DecodedFrame, ox: i32, oy: i32) {
        self.blit_inner(frame, ox, oy, false)
    }

    fn blit_inner(&mut self, frame: &DecodedFrame, ox: i32, oy: i32, mask: bool) {
        let (fw, fh) = (frame.width as i32, frame.height as i32);
        for y in 0..fh {
            let cy = oy + y;
            if cy < 0 || cy >= self.height as i32 {
                continue;
            }
            for x in 0..fw {
                let cx = ox + x;
                if cx < 0 || cx >= self.width as i32 {
                    continue;
                }
                let src = (y * fw + x) as usize;
                if mask && !frame.opaque[src] {
                    continue;
                }
                self.pixels[cy as usize * self.width + cx as usize] = frame.indices[src];
            }
        }
    }

    /// A filled rectangle, clipped. Used for the panel background and for
    /// markers a sprite sheet does not supply.
    pub fn fill_rect(&mut self, x: i32, y: i32, w: i32, h: i32, index: u8) {
        for yy in y.max(0)..(y + h).min(self.height as i32) {
            for xx in x.max(0)..(x + w).min(self.width as i32) {
                self.pixels[yy as usize * self.width + xx as usize] = index;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(w: u16, h: u16, fill: u8) -> DecodedFrame {
        let n = w as usize * h as usize;
        DecodedFrame {
            width: w,
            height: h,
            indices: vec![fill; n],
            opaque: vec![fill != 0; n],
        }
    }

    #[test]
    fn a_new_canvas_is_entirely_the_transparent_index() {
        let c = Canvas::screen();
        assert_eq!(c.pixels.len(), WIDTH * HEIGHT);
        assert_eq!(c.count(TRANSPARENT), WIDTH * HEIGHT);
    }

    #[test]
    fn the_sprite_blitter_leaves_index_zero_alone_and_the_tile_blitter_does_not() {
        let mut c = Canvas::new(4, 4);
        c.clear(7);
        c.blit(&frame(2, 2, 0), 0, 0);
        assert_eq!(c.count(7), 16, "transparent pixels must not be written");

        c.blit_opaque(&frame(2, 2, 0), 0, 0);
        assert_eq!(c.count(0), 4, "the tile blitter writes index 0");
        assert_eq!(c.count(7), 12);
    }

    #[test]
    fn blits_clip_on_every_edge_rather_than_panicking() {
        let mut c = Canvas::new(8, 8);
        c.blit(&frame(4, 4, 3), -2, -2);
        assert_eq!(c.count(3), 4, "only the bottom-right quarter lands");
        c.clear(0);
        c.blit(&frame(4, 4, 3), 6, 6);
        assert_eq!(c.count(3), 4, "only the top-left quarter lands");
        c.clear(0);
        c.blit(&frame(4, 4, 3), 100, 100);
        assert_eq!(c.count(3), 0, "entirely off-canvas draws nothing");
    }

    #[test]
    fn diff_count_is_zero_only_for_identical_canvases() {
        let a = Canvas::new(4, 4);
        let mut b = Canvas::new(4, 4);
        assert_eq!(a.diff_count(&b), 0);
        b.set(2, 2, 9);
        assert_eq!(a.diff_count(&b), 1);
    }
}

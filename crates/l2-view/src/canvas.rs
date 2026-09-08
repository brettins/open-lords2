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

/// A parallel plane of identifiers the same size as a [`Canvas`]: *what* was
/// drawn at each pixel, rather than what colour it came out.
///
/// This exists because picking a county off an isometric map cannot be done by
/// inverting the projection. Tiles are diamonds drawn back to front and they
/// overlap, so which tile a pixel belongs to is decided by the draw order, not
/// by geometry. Stamping an id while the tile is blitted records exactly that
/// decision, at one byte per pixel.
///
/// Id 0 means "nothing stamped here", which is also why county ids are 1-based
/// in the original.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tags {
    pub width: usize,
    pub height: usize,
    pub ids: Vec<u8>,
}

impl Tags {
    pub fn new(width: usize, height: usize) -> Self {
        Tags { width, height, ids: vec![0; width * height] }
    }

    pub fn screen() -> Self {
        Tags::new(WIDTH, HEIGHT)
    }

    pub fn clear(&mut self) {
        self.ids.fill(0);
    }

    /// The id at a pixel, or 0 outside the plane.
    #[inline]
    pub fn at(&self, x: i32, y: i32) -> u8 {
        if x < 0 || y < 0 || x as usize >= self.width || y as usize >= self.height {
            return 0;
        }
        self.ids[y as usize * self.width + x as usize]
    }

    pub fn count(&self, id: u8) -> usize {
        self.ids.iter().filter(|&&i| i == id).count()
    }

    /// Every distinct non-zero id present, ascending — ascending because a
    /// caller that walks it must not depend on hash or insertion order
    /// (`docs/netcode.md` §3).
    pub fn ids_present(&self) -> Vec<u8> {
        let mut seen = [false; 256];
        for &i in &self.ids {
            seen[i as usize] = true;
        }
        (1..=255u8).filter(|&i| seen[i as usize]).collect()
    }
}

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

    /// Blit a sprite and stamp `id` into `tags` at every pixel it actually
    /// painted. Transparent pixels leave both planes alone, so the tag plane
    /// records the shape of the sprite rather than of its bounding box.
    pub fn blit_tagged(&mut self, frame: &DecodedFrame, ox: i32, oy: i32, tags: &mut Tags, id: u8) {
        self.blit_full(frame, ox, oy, true, Some((tags, id)))
    }

    /// Expand the indexed canvas through a palette into RGBA. The one place
    /// colour appears at all.
    ///
    /// `rgba` holds four bytes per pixel and is written until either it or the
    /// canvas runs out. This lives here, rather than in the windowing layer, so
    /// that the final image can be asserted on without a GPU — and so this
    /// crate needs no presentation dependency to produce one.
    pub fn to_rgba(&self, palette: &l2_formats::Palette, rgba: &mut [u8]) {
        for (px, &idx) in rgba.chunks_exact_mut(4).zip(self.pixels.iter()) {
            let [r, g, b] = palette.rgb(idx);
            px[0] = r;
            px[1] = g;
            px[2] = b;
            px[3] = 0xff;
        }
    }

    fn blit_inner(&mut self, frame: &DecodedFrame, ox: i32, oy: i32, mask: bool) {
        self.blit_full(frame, ox, oy, mask, None)
    }

    fn blit_full(
        &mut self,
        frame: &DecodedFrame,
        ox: i32,
        oy: i32,
        mask: bool,
        mut tags: Option<(&mut Tags, u8)>,
    ) {
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
                if let Some((tags, id)) = tags.as_mut() {
                    let (tw, th) = (tags.width, tags.height);
                    if (cx as usize) < tw && (cy as usize) < th {
                        tags.ids[cy as usize * tw + cx as usize] = *id;
                    }
                }
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

    /// The tag plane must record what was *painted*, not what was *offered*.
    /// A sprite with a transparent hole in it leaves the id under the hole
    /// alone, or picking would claim pixels the player can see through.
    #[test]
    fn tags_are_stamped_only_where_the_sprite_actually_painted() {
        let mut c = Canvas::new(4, 4);
        let mut tags = Tags::new(4, 4);

        // A 2x2 frame with one transparent pixel at its top-left.
        let mut f = frame(2, 2, 5);
        f.indices[0] = 0;
        f.opaque[0] = false;

        c.blit_tagged(&f, 0, 0, &mut tags, 9);
        assert_eq!(tags.count(9), 3, "the transparent pixel stamps nothing");
        assert_eq!(tags.at(0, 0), 0);
        assert_eq!(tags.at(1, 0), 9);
        assert_eq!(c.count(5), 3);

        // Later draws win, which is what makes back-to-front order the
        // picking rule.
        c.blit_tagged(&frame(2, 2, 6), 1, 1, &mut tags, 4);
        assert_eq!(tags.at(1, 1), 4);
        assert_eq!(tags.ids_present(), vec![4, 9]);
    }

    #[test]
    fn tags_clip_with_the_blit_and_never_stamp_off_plane() {
        let mut c = Canvas::new(4, 4);
        let mut tags = Tags::new(4, 4);
        c.blit_tagged(&frame(4, 4, 3), -2, -2, &mut tags, 7);
        assert_eq!(tags.count(7), 4);
        assert_eq!(tags.at(-1, 0), 0, "off-plane reads as nothing");
        assert_eq!(tags.at(99, 99), 0);
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

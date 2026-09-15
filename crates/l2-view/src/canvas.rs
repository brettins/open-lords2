//! * **Sprites** skip palette index 0 — `Blit_Unclipped` (`0x004B43B1`) tests
//!   each byte and leaves the destination alone when it is zero.
//!
//! * **Terrain tiles** do not. `Battlefield_Draw32` (`0x004BCBDC`) reaches
//!   `FUN_004B5333`, a straight copy. **[V]** and, as it happens, unobservable
//!   on a field battlefield: no frame of `T32_bat1.pl8` contains a single index-0
//!   pixel, checked over all 252 frames. The distinction is kept because it is
//! real

use l2_formats::DecodedFrame;

pub const WIDTH: usize = 640;
pub const HEIGHT: usize = 480;

pub const TRANSPARENT: u8 = 0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Clip {
    pub x0: i32,
    pub y0: i32,
    pub x1: i32,
    pub y1: i32,
}

impl Clip {
    pub const WHOLE: Clip = Clip { x0: i32::MIN, y0: i32::MIN, x1: i32::MAX, y1: i32::MAX };

    pub const fn new(x0: i32, y0: i32, x1: i32, y1: i32) -> Clip {
        Clip { x0, y0, x1, y1 }
    }

    pub const fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.x0 && x < self.x1 && y >= self.y0 && y < self.y1
    }

    pub const fn width(&self) -> i32 {
        self.x1 - self.x0
    }

    pub const fn height(&self) -> i32 {
        self.y1 - self.y0
    }
}

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

    pub fn ids_present(&self) -> Vec<u8> {
        let mut seen = [false; 256];
        for &i in &self.ids {
            seen[i as usize] = true;
        }
        (1..=255u8).filter(|&i| seen[i as usize]).collect()
    }
}

/// **`FUN_004B1310`'s table** — every colour to one of eight dark shades,
/// `0x3F - (((r + g + b) / 3) >> 3)`.
///
/// animated arm of `Screen_BattleOutcome` are its three callers that matter
/// here. **`[I]` on the scale**: the formula is read off the decompilation, and
/// it reads `g_paletteRgb`, which holds a `.256` file's **6-bit** values — so
/// this converts our 8-bit palette back before it averages. Read as 8-bit the
/// same line would spread the result over 32 shades instead of 8.
pub fn shade_table(palette: &l2_formats::Palette) -> [u8; 256] {
    let mut t = [0u8; 256];
    for (i, slot) in t.iter_mut().enumerate() {
        let [r, g, b] = palette.rgb(i as u8);
        let six = |v: u8| v as u32 * 63 / 255;
        let lum = (six(r) + six(g) + six(b)) / 3;
        *slot = 0x3F - (lum >> 3) as u8;
    }
    t
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

    pub fn diff_count(&self, other: &Canvas) -> usize {
        self.pixels.iter().zip(other.pixels.iter()).filter(|(a, b)| a != b).count()
    }

    pub fn count(&self, index: u8) -> usize {
        self.pixels.iter().filter(|&&p| p == index).count()
    }

    pub fn blit(&mut self, frame: &DecodedFrame, ox: i32, oy: i32) {
        self.blit_inner(frame, ox, oy, true)
    }

    pub fn blit_opaque(&mut self, frame: &DecodedFrame, ox: i32, oy: i32) {
        self.blit_inner(frame, ox, oy, false)
    }

    pub fn blit_tagged(&mut self, frame: &DecodedFrame, ox: i32, oy: i32, tags: &mut Tags, id: u8) {
        self.blit_full(frame, ox, oy, true, Some((tags, id)), Clip::WHOLE)
    }

    pub fn blit_clipped(&mut self, frame: &DecodedFrame, ox: i32, oy: i32, clip: Clip) {
        self.blit_full(frame, ox, oy, true, None, clip)
    }

    pub fn blit_clipped_tagged(
        &mut self,
        frame: &DecodedFrame,
        ox: i32,
        oy: i32,
        clip: Clip,
        tags: &mut Tags,
        id: u8,
    ) {
        self.blit_full(frame, ox, oy, true, Some((tags, id)), clip)
    }

    pub fn to_rgba(&self, palette: &l2_formats::Palette, rgba: &mut [u8]) {
        for (px, &idx) in rgba.chunks_exact_mut(4).zip(self.pixels.iter()) {
            let [r, g, b] = palette.rgb(idx);
            px[0] = r;
            px[1] = g;
            px[2] = b;
            px[3] = 0xff;
        }
    }

    /// `Smk_Open` (`0x0042DA18`) and `Smk_PlayLoop` (`0x0042DBC7`) hand the
    /// original's `SmackToBuffer` the game's own 640 × 480 back buffer.
    pub fn blit_raster(&mut self, pixels: &[u8], width: usize, ox: i32, oy: i32, y_scale: usize) {
        if width == 0 || y_scale == 0 {
            return;
        }
        let rows = pixels.len() / width;
        for row in 0..rows {
            let src = &pixels[row * width..(row + 1) * width];
            for rep in 0..y_scale {
                let cy = oy + (row * y_scale + rep) as i32;
                if cy < 0 || cy >= self.height as i32 {
                    continue;
                }
                let x0 = ox.max(0);
                let x1 = (ox + width as i32).min(self.width as i32);
                if x0 >= x1 {
                    continue;
                }
                let dst = cy as usize * self.width;
                let from = (x0 - ox) as usize;
                self.pixels[dst + x0 as usize..dst + x1 as usize]
                    .copy_from_slice(&src[from..from + (x1 - x0) as usize]);
            }
        }
    }

    /// Every pixel through a 256-entry table. `FUN_004B1310`'s second loop;
    /// [`shade_table`] is its first.
    pub fn remap(&mut self, table: &[u8; 256]) {
        for p in self.pixels.iter_mut() {
            *p = table[*p as usize];
        }
    }

    fn blit_inner(&mut self, frame: &DecodedFrame, ox: i32, oy: i32, mask: bool) {
        self.blit_full(frame, ox, oy, mask, None, Clip::WHOLE)
    }

    fn blit_full(
        &mut self,
        frame: &DecodedFrame,
        ox: i32,
        oy: i32,
        mask: bool,
        mut tags: Option<(&mut Tags, u8)>,
        clip: Clip,
    ) {
        let (fw, fh) = (frame.width as i32, frame.height as i32);
        for y in 0..fh {
            let cy = oy + y;
            if cy < 0 || cy >= self.height as i32 || cy < clip.y0 || cy >= clip.y1 {
                continue;
            }
            for x in 0..fw {
                let cx = ox + x;
                if cx < 0 || cx >= self.width as i32 || cx < clip.x0 || cx >= clip.x1 {
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
    fn tags_are_stamped_only_where_the_sprite_actually_painted() {
        let mut c = Canvas::new(4, 4);
        let mut tags = Tags::new(4, 4);

        let mut f = frame(2, 2, 5);
        f.indices[0] = 0;
        f.opaque[0] = false;

        c.blit_tagged(&f, 0, 0, &mut tags, 9);
        assert_eq!(tags.count(9), 3, "the transparent pixel stamps nothing");
        assert_eq!(tags.at(0, 0), 0);
        assert_eq!(tags.at(1, 0), 9);
        assert_eq!(c.count(5), 3);

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

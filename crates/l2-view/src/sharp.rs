//! C259's sharp text: a plane at [`SCALE`] x beside the 640 x 480 one. A logical
//! pixel whose block is `on` presents from `hi`; any later write to it turns it
//! off, so whatever is drawn over text covers it. Presentation only.

use crate::canvas::Canvas;

pub const SCALE: usize = 3;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sharp {
    pub hi: Vec<u8>,
    pub on: Vec<bool>,
}

impl Canvas {
    pub fn enable_sharp(&mut self) {
        let (w, h) = (self.width, self.height);
        self.sharp = Some(Box::new(Sharp { hi: vec![0; w * h * SCALE * SCALE], on: vec![false; w * h] }));
    }

    pub fn is_sharp(&self) -> bool {
        self.sharp.is_some()
    }

    fn clipped(&self, x0: i32, y0: i32, x1: i32, y1: i32) -> (usize, usize, usize, usize) {
        let cx = |v: i32| v.clamp(0, self.width as i32) as usize;
        let cy = |v: i32| v.clamp(0, self.height as i32) as usize;
        (cx(x0), cy(y0), cx(x1), cy(y1))
    }

    /// Every block in the rect not already `on` takes its logical pixel, tripled.
    pub fn sharp_seed(&mut self, x0: i32, y0: i32, x1: i32, y1: i32) {
        let (x0, y0, x1, y1) = self.clipped(x0, y0, x1, y1);
        let w = self.width;
        let Some(s) = self.sharp.as_mut() else { return };
        for y in y0..y1 {
            for x in x0..x1 {
                if s.on[y * w + x] {
                    continue;
                }
                let v = self.pixels[y * w + x];
                for dy in 0..SCALE {
                    let row = (y * SCALE + dy) * w * SCALE + x * SCALE;
                    s.hi[row..row + SCALE].fill(v);
                }
            }
        }
    }

    pub fn sharp_claim(&mut self, x0: i32, y0: i32, x1: i32, y1: i32) {
        let (x0, y0, x1, y1) = self.clipped(x0, y0, x1, y1);
        let w = self.width;
        let Some(s) = self.sharp.as_mut() else { return };
        for y in y0..y1 {
            s.on[y * w + x0..y * w + x1].fill(true);
        }
    }

    /// One physical pixel, where its block is `on`.
    pub fn sharp_set(&mut self, px: i32, py: i32, index: u8) {
        let w = self.width;
        let (pw, ph) = ((w * SCALE) as i32, (self.height * SCALE) as i32);
        let Some(s) = self.sharp.as_mut() else { return };
        if px < 0 || py < 0 || px >= pw || py >= ph {
            return;
        }
        let (px, py) = (px as usize, py as usize);
        if s.on[(py / SCALE) * w + px / SCALE] {
            s.hi[py * w * SCALE + px] = index;
        }
    }

    #[inline]
    pub(crate) fn touch(&mut self, i: usize) {
        if let Some(s) = self.sharp.as_mut() {
            s.on[i] = false;
        }
    }

    /// The plane copied from `other`, and every block off.
    pub fn copy_plane(&mut self, other: &Canvas) {
        self.pixels.copy_from_slice(&other.pixels);
        if let Some(s) = self.sharp.as_mut() {
            s.on.fill(false);
        }
    }

    /// The physical index at (px, py) of a [`SCALE`] x picture.
    #[cfg(test)]
    fn physical(&self, px: usize, py: usize) -> u8 {
        let (x, y) = (px / SCALE, py / SCALE);
        let i = y * self.width + x;
        match &self.sharp {
            Some(s) if s.on[i] => s.hi[py * self.width * SCALE + px],
            _ => self.pixels[i],
        }
    }

    /// The picture at `k` x the logical size. Art is each logical pixel `k` x `k`;
    /// a block that is `on` is its [`SCALE`] x [`SCALE`] patch of `hi`, averaged
    /// to `k` x `k` by area (a copy at `k` = [`SCALE`]).
    pub fn to_rgba_at(&self, palette: &l2_formats::Palette, k: usize, rgba: &mut [u8]) {
        let k = k.max(1);
        let lut: Vec<[u8; 4]> = (0..=255u8).map(|i| { let [r, g, b] = palette.rgb(i); [r, g, b, 0xff] }).collect();
        let (lw, w) = (self.width, self.width * k);
        // Output o of a block spans [SCALE*o, SCALE*o + SCALE); source s spans [k*s, k*s + k).
        let taps: Vec<Vec<(usize, u32)>> = (0..k)
            .map(|o| {
                let (a, b) = (SCALE * o, SCALE * o + SCALE);
                (a / k..b.div_ceil(k).min(SCALE)).map(|s| (s, (b.min(k * s + k) - a.max(k * s)) as u32)).collect()
            })
            .collect();
        let n = (SCALE * SCALE) as u32;
        for y in 0..self.height {
            for x in 0..lw {
                let i = y * lw + x;
                let on = self.sharp.as_ref().is_some_and(|s| s.on[i]);
                if !on {
                    let c = lut[self.pixels[i] as usize];
                    for dy in 0..k {
                        let row = ((y * k + dy) * w + x * k) * 4;
                        for px in rgba[row..row + k * 4].chunks_exact_mut(4) {
                            px.copy_from_slice(&c);
                        }
                    }
                    continue;
                }
                let hi = &self.sharp.as_ref().unwrap().hi;
                let src = |sx: usize, sy: usize| lut[hi[(y * SCALE + sy) * lw * SCALE + x * SCALE + sx] as usize];
                for (oy, ys) in taps.iter().enumerate() {
                    for (ox, xs) in taps.iter().enumerate() {
                        let mut acc = [0u32; 3];
                        for &(sy, wy) in ys {
                            for &(sx, wx) in xs {
                                let c = src(sx, sy);
                                for ch in 0..3 {
                                    acc[ch] += c[ch] as u32 * wx * wy;
                                }
                            }
                        }
                        let at = ((y * k + oy) * w + x * k + ox) * 4;
                        rgba[at..at + 4].copy_from_slice(&[(acc[0] / n) as u8, (acc[1] / n) as u8, (acc[2] / n) as u8, 0xff]);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_write_over_claimed_text_turns_its_block_off() {
        let mut c = Canvas::new(4, 4);
        c.enable_sharp();
        c.clear(5);
        c.sharp_seed(0, 0, 4, 4);
        c.sharp_claim(0, 0, 4, 4);
        c.sharp_set(4, 4, 9);
        assert_eq!(c.physical(4, 4), 9);
        c.set(1, 1, 7);
        assert_eq!(c.physical(4, 4), 7, "the overlay must cover the sharp text");
        assert_eq!(c.physical(0, 0), 5);
    }
}

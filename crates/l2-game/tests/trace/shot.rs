//! trace-run.ps1 `snap`/`shot` on our side: the presented frame as `<name>.ours.shot<k>.bmp`,
//! the file `tools/oracle/pixel-diff.js` compares with the original's `<name>.orig.shot<k>.bmp`.

use std::path::PathBuf;

use crate::Run;

/// Where shots go: the trace's directory and the scenario name; `next` numbers them from 0.
pub struct Shots {
    pub dir: PathBuf,
    pub name: String,
    pub next: u32,
}

impl Run {
    /// The canvas through `Machine::present`'s palette and fade; the file's name, or "" with no sink.
    pub(crate) fn shot(&mut self) -> String {
        let Some(s) = self.shots.as_mut() else { return String::new() };
        let file = format!("{}.ours.shot{}.bmp", s.name, s.next);
        s.next += 1;
        let (w, h) = (self.canvas.width, self.canvas.height);
        let mut rgba = vec![0u8; w * h * 4];
        self.machine.present(&self.assets, &self.canvas, &mut rgba);
        std::fs::write(s.dir.join(&file), bmp24(&rgba, w, h)).expect("the shot writes");
        file
    }
}

/// A bottom-up 24-bit BMP of RGBA pixels.
fn bmp24(rgba: &[u8], w: usize, h: usize) -> Vec<u8> {
    let row = (w * 3 + 3) & !3;
    let size = 54 + row * h;
    let mut out = Vec::with_capacity(size);
    out.extend_from_slice(b"BM");
    for v in [size as u32, 0, 54, 40, w as u32, h as u32] {
        out.extend_from_slice(&v.to_le_bytes());
    }
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&24u16.to_le_bytes());
    out.extend_from_slice(&[0; 24]);
    for y in (0..h).rev() {
        for p in rgba[y * w * 4..(y + 1) * w * 4].chunks(4) {
            out.extend_from_slice(&[p[2], p[1], p[0]]);
        }
        out.resize(out.len() + row - w * 3, 0);
    }
    out
}

pub fn fnv(bytes: &[u8]) -> u32 {
    bytes.iter().fold(0x811C_9DC5u32, |h, &b| (h ^ b as u32).wrapping_mul(0x0100_0193))
}

#[test]
fn a_shot_is_a_bottom_up_bgr_bitmap() {
    let rgba = [1, 2, 3, 255, 4, 5, 6, 255];
    let b = bmp24(&rgba, 1, 2);
    assert_eq!(b.len(), 54 + 2 * 4);
    assert_eq!(&b[54..57], &[6, 5, 4], "the last row first, blue first");
    assert_eq!(&b[58..61], &[3, 2, 1]);
}

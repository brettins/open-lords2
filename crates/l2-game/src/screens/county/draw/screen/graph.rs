//! Ui_HistoryGraph 0x004156A7, the population and happiness bar chart.

use l2_kingdom::kingdom::History;
use l2_view::{Canvas, Clip};

use crate::screen::Ctx;
use crate::shell::Pen;

pub const GRAPHS_SHEET: &str = "Graphs.pl8";

/// 0x004D29F8: (g_historyLength <, frame) pairs, default 0x16. [V] read from the exe.
const FRAME_LADDER: [(i32, usize); 10] =
    [(10, 2), (20, 4), (30, 6), (40, 8), (50, 10), (60, 12), (80, 14), (100, 16), (130, 18), (200, 20)];
/// 0x004D2AC0 indexed by the frame: bar pitch. [V] read from the exe.
const PITCH: [i32; 23] = [0, 0, 40, 40, 20, 20, 12, 12, 10, 10, 8, 8, 6, 6, 5, 5, 4, 4, 3, 3, 2, 2, 1];
/// 0x004D2A48: (peak <, divisor) pairs, default 100. [V] read from the exe.
const DIV_LADDER: [(i32, i32); 15] = [
    (101, 1), (200, 2), (300, 3), (400, 4), (500, 5), (600, 6), (800, 8), (1000, 10),
    (1200, 12), (1500, 15), (2000, 20), (2500, 25), (3000, 30), (4000, 40), (5000, 50),
];

/// Table_Lookup 0x00404E4B: the first pair whose threshold exceeds x, else dflt.
fn lookup<T: Copy>(x: i32, pairs: &[(i32, T)], dflt: T) -> T {
    pairs.iter().find(|&&(t, _)| x < t).map_or(dflt, |&(_, v)| v)
}

/// Mode 0 reads +0x00 (u32 population), mode 1 +0x04 as a byte, unsigned.
pub fn values(history: &History, county: usize, mode: u8) -> Vec<i32> {
    let read = |e: &l2_kingdom::kingdom::HistoryEntry| match mode {
        0 => e.population,
        _ => e.happiness as u8 as i32,
    };
    history.county(county).iter().map(read).collect()
}

/// One bar: (frame, x, top, height). Returns the bars and the peak.
pub fn bars(x: i32, y: i32, values: &[i32]) -> (Vec<(usize, i32, i32, i32)>, i32) {
    let a = lookup(values.len() as i32, &FRAME_LADDER, 0x16);
    let pitch = PITCH[a];
    let peak = values.iter().fold(0, |m, &v| m.max(v));
    let div = lookup(peak, &DIV_LADDER, 100);
    let out = values
        .iter()
        .enumerate()
        .map(|(i, &v)| {
            let h = v / div;
            // `(local_10 & 1) == 0` -> frame a + 1, else a.
            let frame = if i & 1 == 0 { a + 1 } else { a };
            (frame, pitch * i as i32 + pitch / 2 + x + 1, y - h + 0x9A, h)
        })
        .collect();
    (out, peak)
}

/// Returns the peak, which Panel_Population prints.
pub fn draw(pen: &Pen, ctx: &Ctx, canvas: &mut Canvas, county: usize, mode: u8) -> i32 {
    let (x, y) = (0x20, 0x54);
    // Ui_DrawInsetRect(x, y, 0x192, 0x9B)
    pen.inset(canvas, crate::input::Rect::new(0x20, 0x54, 0x192, 0x9B));
    let sheet = ctx.assets.shell.sheet(GRAPHS_SHEET);
    // Sprite_WGenSprite(mode == 0 ? 0 : 1, x + 1, y + 1)
    if let Some(f) = sheet.and_then(|s| s.frame(if mode == 0 { 0 } else { 1 })) {
        canvas.blit(&f, 0x21, 0x55);
    }
    let (bars, peak) = bars(x, y, &values(&ctx.game.kingdom.history, county, mode));
    for (frame, bx, top, h) in bars {
        // Sprite_WGenHSprite's 4th argument is `100 - h` (push at 0x004158CA):
        // g_spriteHeight -= it, so only the top h rows are drawn.
        if let Some(f) = sheet.and_then(|s| s.frame(frame)) {
            canvas.blit_clipped(&f, bx, top, Clip::new(i32::MIN, top, i32::MAX, top + h));
        }
    }
    peak
}

#[cfg(test)]
mod tests {
    use super::*;
    use l2_kingdom::county::County;

    #[test]
    fn bars_follow_the_three_ladders() {
        // Five turns: frame 2, pitch 40. Peak 300: divisor 4, not 3 (`x < 300` fails).
        let (b, peak) = bars(0x20, 0x54, &[0, 50, 150, 99, 300]);
        assert_eq!(peak, 300);
        assert_eq!(b[0], (3, 0x20 + 1 + 20, 0x54 + 0x9A, 0), "even bars are frame a + 1");
        assert_eq!(b[1], (2, 0x20 + 1 + 60, 0x54 + 0x9A - 12, 12));
        assert_eq!(b[4], (3, 0x20 + 1 + 180, 0x54 + 0x9A - 75, 75));
        let (b, peak) = bars(0, 0, &vec![6000; 250]);
        assert_eq!((peak, b[1].0, b[1].1, b[1].3), (6000, 0x16, 1 + 1, 60), "both defaults");
    }

    #[test]
    fn happiness_is_read_as_an_unsigned_byte() {
        let mut counties = vec![County::default(); 17];
        counties[3].population = 700;
        counties[3].happiness = -1;
        let mut h = History::new();
        h.record(&counties);
        assert_eq!(values(&h, 3, 0), vec![700]);
        assert_eq!(values(&h, 3, 1), vec![255], "mode 1 reads +0x04 as a byte");
    }
}

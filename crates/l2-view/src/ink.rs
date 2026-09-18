
use l2_formats::Palette;

pub fn nearest(palette: &Palette, rgb: [u8; 3]) -> u8 {
    let mut best = 0u8;
    let mut best_d = i32::MAX;
    for i in 0..=255u8 {
        let e = palette.rgb(i);
        let d = (0..3)
            .map(|c| {
                let delta = e[c] as i32 - rgb[c] as i32;
                delta * delta
            })
            .sum::<i32>();
        if d < best_d {
            best_d = d;
            best = i;
        }
    }
    best
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ink {
    pub background: u8,
    pub panel: u8,
    pub border: u8,
    pub text: u8,
    pub dim: u8,
    pub highlight: u8,
    pub good: u8,
    pub bad: u8,
    /// The county strip used this for the *Sovereign land of …* lines and a
    /// player reported the result: *"the counties seem to have the right
    /// colours … but the text doesn't match that."* `docs/decisions.md` C112.
    pub realm: [u8; 6],
}

impl Ink {
    pub fn for_palette(p: &Palette) -> Ink {
        Ink {
            background: nearest(p, [0, 0, 0]),
            panel: nearest(p, [40, 32, 24]),
            border: nearest(p, [120, 100, 72]),
            text: nearest(p, [255, 255, 255]),
            dim: nearest(p, [168, 152, 128]),
            highlight: nearest(p, [255, 216, 64]),
            good: nearest(p, [80, 224, 80]),
            bad: nearest(p, [232, 64, 48]),
            realm: [
                nearest(p, [128, 128, 128]), // unowned
                nearest(p, [224, 32, 32]),   // 1
                nearest(p, [48, 96, 232]),   // 2
                nearest(p, [232, 224, 48]),  // 3
                nearest(p, [32, 32, 32]),    // 4
                nearest(p, [176, 64, 200]),  // 5
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ramp() -> Palette {
        let mut bytes = vec![0u8; Palette::FILE_LEN];
        for i in 0..256usize {
            let v = (i / 4) as u8; // 0..63
            bytes[i * 3] = v;
            bytes[i * 3 + 1] = v;
            bytes[i * 3 + 2] = v;
        }
        Palette::from_bytes(&bytes).unwrap()
    }

    #[test]
    fn nearest_finds_the_closest_entry_and_breaks_ties_at_the_lowest_index() {
        let p = ramp();
        assert_eq!(nearest(&p, [0, 0, 0]), 0);
        assert_eq!(nearest(&p, [255, 255, 255]), 252, "the first entry that is full white");
        for _ in 0..4 {
            assert_eq!(nearest(&p, [1, 1, 1]), 0);
        }
    }

    #[test]
    fn a_grey_ramp_cannot_distinguish_the_realm_colours_but_a_colour_palette_can() {
        let grey = Ink::for_palette(&ramp());
        assert_eq!(grey.text, 252);
        assert_eq!(grey.background, 0);

        let mut bytes = vec![0u8; Palette::FILE_LEN];
        let wanted: [[u8; 3]; 6] = [
            [32, 32, 32],
            [56, 8, 8],
            [12, 24, 58],
            [58, 56, 12],
            [8, 8, 8],
            [44, 16, 50],
        ];
        for (i, w) in wanted.iter().enumerate() {
            bytes[i * 3..i * 3 + 3].copy_from_slice(w);
        }
        let p = Palette::from_bytes(&bytes).unwrap();
        let ink = Ink::for_palette(&p);
        let mut seen = ink.realm.to_vec();
        seen.sort_unstable();
        seen.dedup();
        assert_eq!(seen.len(), 6, "six realm colours must land on six different entries");
    }
}

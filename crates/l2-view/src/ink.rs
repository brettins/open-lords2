//! Named interface colours, resolved against whatever palette is loaded.
//!
//! The canvas holds palette indices, and a `.256` palette is the game's, not
//! ours: there is no index that means "white" everywhere. Hard-coding an index
//! would be a claim about a shipped file that nobody has checked, and it would
//! break the moment a mod supplied a different palette.
//!
//! So the interface names the colours it wants in RGB and asks the palette for
//! its nearest entry. The search is integer, exhaustive and first-match-wins,
//! so it is deterministic and gives the same index on every machine.
//!
//! **This is the interface's own colour scheme, not the original's.** Nothing
//! here is a reading of `Lords2.exe`; the original's chrome is drawn from
//! `.pl8` artwork we do not yet compose.

use l2_formats::Palette;

/// The palette entry closest to an RGB triple, by squared distance in integer
/// arithmetic.
///
/// Ties go to the lowest index. That matters more than it looks: a palette with
/// duplicate entries — and the shipped ones have many — must still resolve to
/// one stable index, or two machines drawing the same interface would disagree
/// pixel for pixel.
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

/// The interface's colours, once resolved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ink {
    /// Behind everything.
    pub background: u8,
    /// A panel or bar sitting over the map.
    pub panel: u8,
    /// A panel's edge, and a button's.
    pub border: u8,
    /// Body text.
    pub text: u8,
    /// Labels and units — text that is not the number you came to read.
    pub dim: u8,
    /// The selected item: a menu row, a county outline, a focused button.
    pub highlight: u8,
    /// A number that moved the way the player wants, and one that did not.
    pub good: u8,
    pub bad: u8,
    /// One per realm, index 0 being "unowned". Presentation only — which lord
    /// flies which colour in the original is not established here.
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

    /// A palette built from an explicit ramp, so the expected answers are
    /// arithmetic rather than a reading of a shipped file.
    fn ramp() -> Palette {
        let mut bytes = vec![0u8; Palette::FILE_LEN];
        for i in 0..256usize {
            // 6-bit VGA values, which is what a .256 holds.
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
        // Entries 0..3 are all pure black in this ramp: the tie must resolve
        // to the lowest, every time.
        for _ in 0..4 {
            assert_eq!(nearest(&p, [1, 1, 1]), 0);
        }
    }

    #[test]
    fn a_grey_ramp_cannot_distinguish_the_realm_colours_but_a_colour_palette_can() {
        // On a grey ramp every hue collapses to its luminance-ish nearest, and
        // that is a property of the palette, not a bug in the resolver.
        let grey = Ink::for_palette(&ramp());
        assert_eq!(grey.text, 252);
        assert_eq!(grey.background, 0);

        // A palette with real colours in it separates them.
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

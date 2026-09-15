//! **[V]** `Glyph_Draw` (`0x00402A14`) is four lines of arithmetic:
//!
//! and `FUN_004014F0`, which measures a string, indexes the *same* table from
//! `0x004D71D0` with the raw character — `0x004D71F0 - 0x20`, the same bytes.
//!
//! The mapping is self-checking, which is what makes it **[V]**
//! **[D]**: it sends `'a'` to frame 0 and `'A'` to frame 26 in `Fntl2_14.pl8`,
//! and the frames it sends `'g'`, `'j'`, `'p'`, `'q'`, `'y'` to are
//! frames in that file that are four pixels taller than their neighbours.
//!
//! `Ui_DrawText` (`0x00402637`) draws each glyph at `y - 1` in one shadow
//! colour, at `y + 1` in another, and then at `y` in the real one. Text on
//! these screens is embossed, not flat, and a reimplementation that draws it
//! once looks subtly wrong everywhere at the same time.
//!
//! **[D]** and then two globals switch it off again:
//!
//! * `DAT_005AEA40` — when non-zero the whole emboss is skipped and the glyph
//!   is drawn once, at `y`, in its own colour. The front end sets it around
//!   every menu item, every button caption and every body line, and clears it
//!   for the heading. So on those pages **the heading is embossed and nothing
//!   else is**, which is not what a reimplementation would guess.
//!
//! * `DAT_0058FE2C` — when non-zero, characters `0x41 … 0x5A`, which is `A`
//! through `Z` and nothing else, are drawn in **colour 1** instead of the
//! caller's. The setup pages set it around their heading and the conquest
//!   screen sets it around everything it draws. It is a drop-capital effect
//!   applied to every capital in the line.

mod glyph;
pub use glyph::*;

use l2_formats::DecodedFrame;
use l2_view::sheet::Sheet;
use l2_view::Canvas;

pub const GLYPH_MAP_VA: u32 = 0x004D_71F0;

pub const GLYPH_MAP_BASE: u8 = 0x20;

/// What a character with no glyph advances **when it is drawn**. **[V]**
///
/// `Ui_DrawText` (`0x00402637`) looks the character up before it draws
/// anything, and for an empty entry it hard-codes the advance and never calls
/// the blitter:
///
/// So `' '`, `'@'` and every other unmapped character move the pen four pixels
/// and paint nothing. That is what makes `'@'` an invisible sign column that
/// still holds its place — `Ui_DrawCount` (`0x0041AB67`) opens every count with
/// one, and `Ui_DrawNumber`'s 62 `'@'` call sites use it to line up a column.
///
/// **This comment used to name `Glyph_Draw` as the mechanism**, and said it
/// "adds nothing at all for a zero entry" — which is true of `Glyph_Draw`
/// (`0x00402A14`, `return 0`) and beside the point, because `Ui_DrawText` does
/// not reach it for those characters. Read literally, the old sentence said
/// `'@'` advances zero, which is the opposite of its own conclusion.
///
/// **The measure disagrees with the draw.** `FUN_004014F0` (`0x004014F0`)
/// charges 4 for `0x20` alone and **nothing** for any other empty entry,
/// string holding `'@'` draws four pixels wider than it measures. [`Font::width`]
/// used to charge 4 for both; it now charges what the measure charges. That is
/// only visible under centring, and no centred draw in this crate passes `'@'`
/// today. `docs/decisions.md` C155.
pub const SPACE_ADVANCE: i32 = 4;

pub const SHADOW: (u8, u8) = (0x10, 0x1F);

pub const SHADOW_GATEWAY: (u8, u8) = (0x36, 0x2C);

/// **The grey emboss — `DAT_0058FE9C`'s pair, and the third mode of
/// `Ui_DrawText`.** **[V]**
///
/// The module doc above describes two emboss modes. There are four, and this is
/// the one that had been missed. When `DAT_0058FE9C` is non-zero the function
/// ignores the screen-id pair entirely and draws `0x3F` above and `0x26` below:
///
/// ```c
/// else {                       /* DAT_0058fe9c != 0 */
///   g_drawY = y - 1; DAT_0057d3bc = 0x3f; Glyph_Draw(font, c);
///   g_drawY = y + 1; DAT_0057d3bc = 0x26; Glyph_Draw(font, c);
///   g_drawY = y;     DAT_0057d3bc = colour; Glyph_Draw(font, c);
/// }
/// ```
///
/// **Three functions set it and no more:** `CountyStrip_Draw` around the three
/// *Sovereign land of …* lines, `Screen_Armoury`, and `FUN_004180F6`. It is
/// cleared again immediately after each.
pub const SHADOW_GREY: (u8, u8) = (0x3F, 0x26);

/// **The fourth mode: a drop shadow**, and [`Font::draw_dropped`] draws it. **[V]**
///
/// When `g_dropShadow` (`DAT_005AEB90`) is non-zero — and `DAT_005AEA40` and
/// `g_embossGrey` are both zero, which `Ui_DrawText` tests first — each glyph is
/// drawn twice
///
/// ```c
/// g_drawY = y + 1; g_drawX = g_drawX + 1; DAT_0057d3bc = 0x3f; Glyph_Draw(font, c);
/// g_drawX = g_drawX - 1; g_drawY = y;     DAT_0057d3bc = colour; Glyph_Draw(font, c);
/// ```
///
/// `FUN_004100AF`, `FUN_0041023A`, `FUN_004103C5`, `FUN_00410502`,
/// `FUN_00410598`, `FUN_0041062E`, `FUN_004106C4` and
/// `CountyStrip_DrawCastleIcon`, each `g_dropShadow = 1` on entry and `0` on the
/// way out. `CountyStrip_Draw` clears `DAT_005AEA40` before it calls any of
/// them, so every `&g_font10` draw in the image is a dropped one — and so are
/// the castle cell's two `&g_fontSmall` captions. The two farm rows that set
/// `DAT_005AEA40 = 1` do so *after* their delta, around the store,
/// the store is flat and the forecast above it is not.
pub const DROP_SHADOW_COLOUR: u8 = 0x3F;

pub const TEXT: u8 = 0x3F;
pub const HIGHLIGHT: u8 = 0xF9;
pub const DISABLED: u8 = 0x20;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Style {
    pub colour: u8,
    /// The emboss pair, or `None` for flat — `DAT_005AEA40 != 0`.
    pub shadow: Option<(u8, u8)>,
    /// The colour `A` … `Z` are drawn in instead — `DAT_0058FE2C != 0`, which
    /// always means colour 1.
    pub caps: Option<u8>,
}

impl Style {
    pub fn new(colour: u8) -> Style {
        Style { colour, shadow: Some(SHADOW), caps: None }
    }

    fn colour_of(&self, c: char) -> u8 {
        match self.caps {
            Some(k) if c.is_ascii_uppercase() => k,
            _ => self.colour,
        }
    }
}

pub struct Font {
    sheet: Sheet,
    pub line: i32,
    raises_accents: bool,
}

pub const BODY: &str = "Fntl2_14.pl8";
pub const HEADING: &str = "Fntl2_22.pl8";
/// `Fntl2_9.pl8` — `g_fontSmall` (`0x005C9A90`). **[V]** Nine `.text`
/// references: the loader, six in `CountyStrip_Draw`, two in
/// `CountyStrip_DrawCastleIcon` — and one outside the strip,
/// `Screen_DrawEndTurn`'s `Ui_DrawCentred(4, 0, 0x1DE, 0x1CE, 0xA2, &g_fontSmall,
/// 0x16)`. Our build stamp is drawn in it too, and that is ours.
pub const SMALL: &str = "Fntl2_9.pl8";
/// **`Font_10.pl8` — `g_font10` (`0x005AEBA0`), the fifth face, and the one a
/// player looks at every turn.** **[V]**
///
/// `Res_LoadStatic`'s record 5. Its nine `.text` references outside the loader
/// are all on the county strip's jobs plate: the seven `Ui_DrawDelta` forecasts
/// and the reclamation figure in `FUN_004100AF` … `FUN_004106C4`, and the
/// castle's seasons in `CountyStrip_DrawCastleIcon`.
///
/// **It is a numeral face wearing the full layout.** The file has 108 frame
/// records,
/// [`GLYPH_MAP`] sends no character past the end of it — so it is read through
/// the same table, at the same base, as the other four, and
/// `Glyph_Draw` (`0x00402A14`) has no other table to read it through. What is
/// different is the frames: `1`…`9`, `0` (frames 52…61) and the punctuation
/// strip (62…78, `! " % * ( ) - + = : ; ' ? / , .`) are real ten-pixel glyphs,
/// and **all 52 letters and the accented tail are 2 × 2 stubs** — 81 of them,
/// most wholly transparent. `Glyph_Draw` draws a stub like any frame and
/// advances three.
///
/// So a letter "drawn" in this face paints nothing, or a speck (`'e'` is a
/// solid 2 × 2 block of `0x10`), and moves the pen three. That is the original's
/// behaviour too, and it never meets it: every string the nine call sites build
/// is a lead (`' '`, `'@'`, `'+'` or `'-'`), digits, and a suffix that is one
/// space — `0x004D3D40` … `0x004D3D84`, read out of the image. The words beside
/// those numbers (*"Seasons"*, *"Needed"*) are `&g_fontSmall`.
pub const TEN: &str = "Font_10.pl8";
/// **`Fnt_8.pl8` — `g_font8` (`0x005CBFB0`), the fourth face.** **[V]**
///
/// `Res_LoadStatic` (`0x00499859`) walks thirteen `{char name[16]; u32 size}`
/// records at `g_preloadTable` (`0x004D9F48`) and hands record `n` to
/// `File_ReadChunk` with a buffer chosen by `n`; record **3**, at `0x004D9F84`,
/// is `"fnt_8.pl8"` with a size of 5,200, and `n == 3` selects `&g_font8`
/// (`mov [ebp-4], 0x005CBFB0` at `0x004998ED`). Records 4…7 are `SMALL`,
/// `TEN`, `BODY` and `HEADING` in that order.
///
/// gap.** All 86 references to `0x005CBFB0` in `.text` were enumerated from
/// the bytes: one is that loader line and the other 85 are inside six
/// functions — `Net_DrawDebugOverlay` (`0x00423BA4`), `BattleDebug_Panel`
/// (`0x00424992`), `FUN_00425314`, `FUN_00425487`, `FUN_0042563C` and
/// `FUN_00425799` — every one a developer read-out (`" divergances"`,
/// `" Dchk"`, `"FIGURE"`, `"GROUP"`, `" p1 rank"`). It is loaded so that the
/// face exists when one of those is reproduced, and so the complaint about a
/// broken install names every file the original preloads.
pub const EIGHT: &str = "Fnt_8.pl8";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_map_sends_letters_where_the_font_files_say_it_does() {
        assert_eq!(GLYPH_MAP[('a' as usize) - 0x20], 1);
        assert_eq!(GLYPH_MAP[('A' as usize) - 0x20], 27);
        assert_eq!(GLYPH_MAP[('z' as usize) - 0x20], 26);
        assert_eq!(GLYPH_MAP[('Z' as usize) - 0x20], 52);
        assert_eq!(GLYPH_MAP[('1' as usize) - 0x20], 53);
        assert_eq!(GLYPH_MAP[('9' as usize) - 0x20], 61);
        assert_eq!(GLYPH_MAP[('0' as usize) - 0x20], 62);
    }

    #[test]
    fn a_space_and_an_at_sign_have_no_glyph() {
        assert_eq!(GLYPH_MAP[0], 0, "' '");
        assert_eq!(GLYPH_MAP[('@' as usize) - 0x20], 0, "'@' - the blank sign column");
        assert_eq!(GLYPH_MAP[('$' as usize) - 0x20], 0);
    }

    #[test]
    fn the_map_covers_every_byte_from_0x20() {
        assert_eq!(GLYPH_MAP.len(), 0x100 - 0x20);
        assert_eq!(GLYPH_MAP_BASE, 0x20);
        assert_eq!(GLYPH_MAP.iter().copied().max().unwrap(), 106);
        assert_eq!(&GLYPH_MAP[0xBF..0xC2], &[106, 106, 106]);
    }

    #[test]
    fn the_raise_is_by_character_code_and_not_by_picture() {
        assert_eq!(GLYPH_MAP[0x86 - 0x20], GLYPH_MAP['a' as usize - 0x20]);
        assert_eq!(GLYPH_MAP[0x87 - 0x20], GLYPH_MAP[0x80 - 0x20]);
        assert!(accent_raised('\u{86}') && !accent_raised('a'));
        assert!(accent_raised('\u{87}') && !accent_raised('\u{80}'));
        for (c, raised) in [
            (0x80, false), (0x81, true), (0x8D, true), (0x8E, false),
            (0x92, false), (0x93, true), (0x97, true), (0x98, false),
            (0x9F, false), (0xA0, true), (0xA4, true), (0xA5, false),
        ] {
            assert_eq!(accent_raised(char::from_u32(c).unwrap()), raised, "{c:#04X}");
        }
        assert!(!accent_raised('\u{1F}') && !accent_raised('\u{100}'));
    }
}


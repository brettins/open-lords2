//! What a *shell* screen needs: the original's artwork, the original's strings,
//! the original's fonts, and the handful of primitives its painters call.
//!
//! # What a shell is, and what it is not
//!
//! Twenty-nine screens are identified in `docs/screens-county.md` §1 and five
//! are implemented. The rest are shells: each one loads **the `.pl8` its
//! painter loads** and draws **the `L2.eng` group its painter draws**, at the
//! coordinates read out of that painter, with nothing behind it. A shell is a
//! real surface with no logic, not a mock-up.
//!
//! The distinction matters because this project has thrown an invented
//! interface away once already — `screens/county.rs` opens by admitting it used
//! to be a made-up full-screen page listing twenty-two fields, built because
//! nobody had looked at what the original drew. Nothing in this module invents
//! a layout. Where a coordinate is unknown it says so and places the
//! thing plainly, so that a reader can tell a gap from a guess.
//!
//! # Every screen has its own palette
//!
//! The management screens run under the campaign palette. The front end does
//! not: `gateway.256`, `merchant.256`, `armoury.256`, `cas_back.256`,
//! `custom.256`, `skirmish.256` and `score1.256` are each read into the display
//! palette by the screen that wants them (`File_ReadChunk("gateway.256",
//! 0x004EA8A0, 0x300)` then `Palette_Set`). A canvas of palette indices is
//! meaningless without knowing which one, so [`Screen::palette`] names it and
//! the presenter asks the top screen.
//!
//! [`Screen::palette`]: crate::screen::Screen::palette

mod assets;
pub use assets::*;
mod drawing;
pub use drawing::*;

pub mod eng;
pub mod font;

use std::collections::BTreeMap;

use l2_formats::Palette;
use l2_mods::vfs::Vfs;
use l2_view::sheet::Sheet;
use l2_view::Canvas;

pub use eng::Eng;
pub use font::Font;

/// The full-screen background sheets, one 640 × 480 frame each, plus the
/// smaller sheets a shell draws on top of one.
///
/// Every name here is a literal in the painter that loads it. The list is
/// eager because it is small — the six full-screen backgrounds are 300 KB each
/// and [`Sheet`] decodes lazily, so what this costs is the read, not the
/// decode — and because a lazy cache would need interior mutability on a path
/// that `draw` is only allowed to see through `&`.
pub const SHEETS: &[&str] = &[
    // 0x1C the conquest screen and 0x1F pages 1..6, 10
    "Gateway.pl8",
    "Panels2.pl8",
    // 0x1F pages 7..9 — the custom game
    "Custom.pl8",
    // 0x1F pages 11..13 — skirmish
    "Skirmish.pl8",
    "Skircust.pl8",
    // the setup mode's icon sheet, g_miscCtySheet
    "Misc_sel.pl8",
    // 0x08 the merchant, and 0x0C trade goods, which draws over it
    "Merchant.pl8",
    "Mercgrid.pl8",
    "Icontrad.pl8",
    // 0x0A / 0x0D the armoury. `Arm_it_<c>.pl8` is the one sheet the armoury
    // and the raise-army screen share — the weapons on the walls, the eight
    // troop portraits along the bottom, and the six little weapon icons the
    // levy screen prints its stocks beside. Which of the five is loaded is the
    // realm's `shield_index`; see [`crate::screens::armoury::items_sheet`].
    "Armoury.pl8",
    "Arm_grid.pl8",
    "Arm_it_r.pl8",
    "Arm_it_y.pl8",
    "Arm_it_k.pl8",
    "Arm_it_p.pl8",
    "Arm_it_b.pl8",
    // 0x0D, one per weapon type: a 24-frame 100 x 100 animation of the weapon
    // being made. `Armoury_LoadScreen` reads exactly one of them, chosen by
    // `DAT_00553F20`, the rack the player clicked.
    "Arm_cros.pl8",
    "Arm_mace.pl8",
    "Arm_swor.pl8",
    "Arm_pike.pl8",
    "Arm_bow.pl8",
    "Arm_mail.pl8",
    // 0x0A / 0x0D, the room in motion: two guttering torches and the soldier
    // who walks over and takes the weapon down off the wall. `Armtorch.pl8` is
    // 26 frames — thirteen per torch — and each `Trp_<weapon>_<colour>.pl8` is
    // 21 of 89 × 158: eight walking in, five taking it down, eight carrying it
    // out. See [`crate::screens::armoury::Walker`].
    //
    // **Thirty sheets, 3.3 MB, and the original reads exactly one of them.**
    // `FUN_004AABD8` picks it by the local player's shield colour and the
    // weapon just assigned, at the moment the walk starts — which is a read
    // during a click, and this list is eager because a lazy cache would need
    // interior mutability on a path `draw` may only see through `&`. What that
    // costs is the read: `Sheet` decodes lazily, so a colour
    // never turned into pixels.
    "Armtorch.pl8",
    "Trp_xb_r.pl8",
    "Trp_ma_r.pl8",
    "Trp_sw_r.pl8",
    "Trp_pi_r.pl8",
    "Trp_ar_r.pl8",
    "Trp_kn_r.pl8",
    "Trp_xb_y.pl8",
    "Trp_ma_y.pl8",
    "Trp_sw_y.pl8",
    "Trp_pi_y.pl8",
    "Trp_ar_y.pl8",
    "Trp_kn_y.pl8",
    "Trp_xb_k.pl8",
    "Trp_ma_k.pl8",
    "Trp_sw_k.pl8",
    "Trp_pi_k.pl8",
    "Trp_ar_k.pl8",
    "Trp_kn_k.pl8",
    "Trp_xb_p.pl8",
    "Trp_ma_p.pl8",
    "Trp_sw_p.pl8",
    "Trp_pi_p.pl8",
    "Trp_ar_p.pl8",
    "Trp_kn_p.pl8",
    "Trp_xb_b.pl8",
    "Trp_ma_b.pl8",
    "Trp_sw_b.pl8",
    "Trp_pi_b.pl8",
    "Trp_ar_b.pl8",
    "Trp_kn_b.pl8",
    // 0x0B the other lords
    "Faces.pl8",
    // 0x1B castle building
    "Cas_back.pl8",
    "Caspics.pl8",
    "Cas_bits.pl8",
    // 0x2E the ratings
    "Score1.pl8",
    // 0x20 the standings: the page, and the six-frame banner sheet on it —
    // five 51 x 92 shields and one 23 x 60 tab marker.
    "Grtnoble.pl8",
    "Flags.pl8",
    // 0x1D siege preparations
    "Sgeplans.pl8",
    // 0x11 army division
    "Icon_tmp.pl8",
    // 0x0F job 8 — the blacksmith page. `Panel_JobBlacksmith` (`0x00413155`)
    // `File_ReadChunk`s both into the same scratch buffer, one after the other,
// so the forge fire is drawn out of whatever the *second* read
    // left there. `Smithy.pl8` is one frame of 480 x 400; `Hearth.pl8` is 17 —
    // eleven 69 x 52 fire frames and six hearths, one per weapon.
    "Smithy.pl8",
    "Hearth.pl8",
    // 0x0F, every job — `Panel_JobDetail`'s icon recess. 17 frames of 48 x 48.
    "Iconvill.pl8",
];

/// The `.256` files those screens set as the display palette.
pub const PALETTES: &[&str] = &[
    "Gateway.256",
    "Custom.256",
    "Skirmish.256",
    "Misc_sel.256",
    "Merchant.256",
    "Armoury.256",
    "Cas_back.256",
    "Score1.256",
    "Grtnoble.256",
    // 0x29 … 0x2B, the battlefield. Not a painter's own read: `Res_LoadStatic`
    // (`0x00499859`) preloads it into `0x00568EE0` as record 2 of
    // `g_preloadTable`, and `Screen_DrawBattlefield` (`0x004233F7`) sets it
    // with `Palette_Set(0x568EE0)`. **It was missing from this list** while
    // `BattlefieldScreen::palette` named it, so the lookup failed and the
    // presenter drew a battle in `base01.256` — *"blue grainy madness"*.
    //
    "T32_bat1.256",
    // And the siege arm, `Palette_Set(0x5675A0)` = `t32_stn1.256`, record 1 of
    // the same table. This list said it was NOT PORTED because it *"belongs
    // with `t32_stn1.pl8`"* and we drew every battle from `T32_bat1.pl8` — but
    // the palette colours the whole screen, not only the tiles, so withholding
    // it painted a siege's walls, men, banners and panel in the field's
    // colours (C200). C201 then gave the siege its own tiles, so the two now
    // arrive together. **Both castle families use this one palette**: there is
    // no `t32_wod1.256` in the install or in that table, and it serves
    // `t32_stn1.pl8` and `t32_wod1.pl8` alike — see `l2_view::scene::Ground`.
    "T32_stn1.256",
];

/// The artwork and text a shell screen draws with.
///
/// Everything is optional: a partial install, or a machine with no copy of the
/// game at all, still lays every screen out — with our own flat panels and
/// blank labels, and looking it. `docs/plan.md`'s rule that a stub should be
/// visibly ours applies here too.
pub struct ShellAssets {
    pub eng: Option<Eng>,
    pub body: Option<Font>,
    pub heading: Option<Font>,
    pub small: Option<Font>,
    /// `Fnt_8.pl8`, `g_font8` — see [`font::EIGHT`] for where the original
    /// loads it and why nothing here draws with it yet.
    pub eight: Option<Font>,
    /// `Font_10.pl8`, `g_font10` — the county strip's numbers. A numeral face:
    /// see [`font::TEN`] for why nothing but a number may be drawn in it.
    pub ten: Option<Font>,
    sheets: BTreeMap<String, Sheet>,
    palettes: BTreeMap<String, Palette>,
    /// `mercgrid.pl8`'s 80 x 60 byte map, with its 24-byte header stripped.
    /// Empty when the file is not installed. See [`ShellAssets::merchant_grid`].
    merchant_grid: Vec<u8>,
    /// `arm_grid.pl8`'s, the same shape and read by the same arithmetic.
    /// See [`ShellAssets::armoury_grid`].
    armoury_grid: Vec<u8>,
}

/// `mercgrid.pl8` is an **80 x 60 grid of eight-pixel cells over the whole
/// screen**, one byte a cell holding the good id under it.
///
/// `File_ReadChunk("mercgrid.pl8", &g_villageGrid, 0x12D8, 0)` reads the file
/// whole — 4,824 bytes, which is these 4,800 cells plus a 24-byte `.pl8`
/// header — and `FUN_004357A6` then indexes it as
/// `grid[(x >> 3) + (y >> 3) * 0x50]`. It shares its buffer with the village's
/// own drop grid (`vill_gd8.pl8`, 45 x 40, 1,824 bytes) and with the armoury's
/// (`arm_grid.pl8`, also 4,824), so the buffer is named for the
/// village and read by three unrelated screens.
pub const MERCHANT_GRID_COLS: usize = 80;
pub const MERCHANT_GRID_ROWS: usize = 60;
pub const MERCHANT_GRID_CELL: i32 = 8;
pub const MERCHANT_GRID_LEN: usize = MERCHANT_GRID_COLS * MERCHANT_GRID_ROWS;
/// The bytes of a `.pl8` before its single frame's data.
pub const GRID_HEADER: usize = 24;

impl ShellAssets {
}
/// A pen: the two fonts, the emboss colours the screen wants, and a fallback.
///
/// Every shell screen draws through one of these so that the *same* painter
/// works on a full install and on a machine with no copy of the game. With the
/// fonts present it is the original's text, embossed the original's way; with
/// them absent it is `l2_view::text`'s 5 × 7 font in the palette-resolved
/// interface colours, which lays out in the same places and is obviously ours.
#[derive(Clone, Copy)]
pub struct Pen<'a> {
    pub assets: &'a ShellAssets,
    pub ink: &'a l2_view::Ink,
    /// `Panels.pl8` and the button sheet, when the install has them. Several
    /// shells draw a `Ui_DrawBox` from that kit over their own background.
    pub chrome: Option<&'a l2_view::chrome::Chrome>,
    /// `Ui_DrawText`'s two shadow colours — [`font::SHADOW`] on most screens,
    /// [`font::SHADOW_GATEWAY`] on the setup and conquest pages — or `None`
    /// for flat text, which is `DAT_005AEA40 != 0`.
    pub shadow: Option<(u8, u8)>,
    /// `DAT_0058FE2C`: the colour `A` … `Z` are drawn in instead of the
    /// caller's. Always 1 where the original sets it.
    pub caps: Option<u8>,
}

/// **Four pixels of trailing space after every string**, and it is the last
/// statement of `Ui_DrawText` (`0x00402637`): `g_penAdvance = g_penAdvance + 4;`.
///
/// It is the gap between the two halves of every sentence the original builds
/// out of pieces — *"Raising an army in"* and the county's name, a number and
/// its noun — and none of `L2.eng`'s strings carries a trailing space of its
/// own, so without it the two halves touch. `[V]`
pub const TRAILING: i32 = 4;

/// **`L2.eng` group 8 is the noun table**, and `Ui_DrawCount`'s second argument
/// is an index into it. Seventy-four strings in singular/plural pairs: 0/1
/// *"Crown."*, 2/3 *"Sack."*, 4/5 *"Animal."*, `0x34 + t * 2` the seven troop
/// types, 68 *"Grain"*, 70/71 *"Cow."*, 72/73 *"Total men"*.
pub const COUNT_NOUN_GROUP: usize = 8;

/// **Which face a call site names** — the `font` argument `Ui_DrawText` and
/// everything above it take: `&g_fontBody` (`Fntl2_14.pl8`), `&g_fontHeading`
/// (`Fntl2_22.pl8`) or `&g_font8` (`Fnt_8.pl8`).
///
/// They are not interchangeable, and the choice is the call site's, not the
/// helper's: `Ui_DrawCount` is body at 78 of its 80 call sites and heading at
/// the other two, and `Court_Draw` puts every store value in heading beside a
/// heading label where we had drawn them in body. A helper that hard-codes one
/// face is a helper that draws some screen in the wrong one.
///
/// **The original preloads five faces, not three or four** — `Res_LoadStatic`'s
/// records 3…7 are `fnt_8`, `fntl2_9`, `font_10`, `fntl2_14`, `fntl2_22` — and
/// all five are loaded. Two have no variant here, because no `Pen` caller names
/// them: `&g_fontSmall` and `&g_font10` are drawn only on the county strip (and
/// `&g_fontSmall` once more, in `Screen_DrawEndTurn`), which draws through
/// `ShellAssets::small` and `ShellAssets::ten` directly. See [`font::TEN`] for
/// why the fifth face is a numeral face. Counted from the bytes, not the
/// decompilation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Face {
    Body,
    Heading,
    /// `g_font8`. **No screen of ours names it**: every call site in the
    /// original is one of six developer read-outs — [`font::EIGHT`].
    Eight,
}

/// `L2.eng` group 26, `Ui_DrawYear`'s era: index 0 *"BC"*, index 1 *"AD"*.
pub const YEAR_GROUP: usize = 26;
pub const YEAR_BC: usize = 0;
pub const YEAR_AD: usize = 1;

/// `Ui_DrawCount`'s lead, which it does not take from its caller: always
/// `'@'`, the blank sign column. `0x0041AB67`. **[V]**
pub const COUNT_LEAD: char = '@';

/// `Ui_DrawCount`'s suffix: `&DAT_004D41F4`, a NUL, so the empty string. Read
/// out of the shipped `Lords2.exe` at file offset `0xD23F4`. **[V]**
pub const COUNT_SUFFIX: &str = "";

/// Which of `Ui_DrawCount`'s singular/plural pair a value takes.
///
/// **`|value| == 1`, not `value == 1`**, and this is a free function so that
/// the rule can be asserted without a canvas, a font or an install. It is the
/// exact ladder at `0x0041AB67`:
///
/// ```c
/// if (value == 1)       Eng_DrawString(8, unitIndex,     ...);
/// else if (value == -1) Eng_DrawString(8, unitIndex,     ...);
/// else                  Eng_DrawString(8, unitIndex + 1, ...);
/// ```
///
/// Three arms where two would do, because **minus one is singular**. We had
/// only the first, so `-1` drew the plural — *"−1 Sacks."* where the original
/// writes *"−1 Sack."* It is reachable: the trade screen's quantity is signed,
/// and the map information panel draws
/// `Ui_DrawCount(-g_counties[c].field_0x24C, 2, ...)` with the sign negated at
/// the call site. Found by the draw-call audit reading the *primitive* rather
/// than the screens that call it, which is the argument for auditing the leaves
/// — `docs/draws.md` §7. **[V]**
pub fn count_noun(value: i32, noun: usize) -> usize {
    if value == 1 || value == -1 {
        noun
    } else {
        noun + 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_sheet_and_palette_name_is_unique_once_folded() {
        let mut seen: Vec<String> = SHEETS.iter().map(|n| key(n)).collect();
        seen.sort();
        let before = seen.len();
        seen.dedup();
        assert_eq!(seen.len(), before, "two SHEETS entries fold to the same key");
        let mut seen: Vec<String> = PALETTES.iter().map(|n| key(n)).collect();
        seen.sort();
        let before = seen.len();
        seen.dedup();
        assert_eq!(seen.len(), before);
    }

    /// **A palette a screen names is a palette something loads.** The
    /// battlefield named `T32_bat1.256` for as long as it existed and this list
    /// never carried it, so the presenter's `shell.palette(name)` found nothing,
    /// fell through to `base01.256`, and a player saw the battle in the
    /// campaign's colours. It needs no install: both halves are names.
    ///
    /// **Both of the screen's palettes**
    /// names: `Screen_DrawBattlefield` chooses on `g_battleIsSiege`, and the
    /// siege arm went unloaded for as long as it was a comment, falling back
    /// to `base01.256`
    ///
/// The loop walks `Ground::ALL`, so a
    /// ground added later cannot bring a palette nobody loads: that is C201's
    /// form, kept over C200's two-name list.
    ///
    /// Ablation, run: delete `"T32_bat1.256"` or `"T32_stn1.256"` above — red,
    /// *"… is named by the battlefield and loaded by nobody"*.
    #[test]
    fn the_battlefields_palette_is_one_the_shell_loads() {
        let name = crate::screen::ScreenId::Battlefield.build().palette();
        let name = name.expect("the battlefield names a palette of its own");
        assert_eq!(name, l2_view::scene::TILE_PALETTE, "a fresh screen is a field battle");
        assert!(
            PALETTES.iter().any(|p| key(p) == key(name)),
            "{name} is named by the battlefield and loaded by nobody"
        );
        for g in l2_view::scene::Ground::ALL {
            assert!(
                PALETTES.iter().any(|p| key(p) == key(g.palette())),
                "{g:?}'s palette {} is named by the battlefield and loaded by nobody",
                g.palette()
            );
        }
    }

    #[test]
    fn an_empty_shell_answers_everything_without_panicking() {
        let a = ShellAssets::empty();
        assert!(!a.has_artwork());
        assert_eq!(a.text(11, 0), "");
        assert!(a.sheet("Gateway.pl8").is_none());
        assert!(a.palette("Gateway.256").is_none());
        let mut c = Canvas::screen();
        assert!(!background(&mut c, &a, "Gateway.pl8"));
        assert_eq!(c.count(0), 640 * 480, "and it drew nothing at all");
    }

    /// **All five faces are announced, and `Fntl2_9.pl8` was the one that was
    /// not.** The complaint checked `body` and `heading`; `small` draws the
    /// build stamp and the county strip, and its absence was silent. `ten`
    /// draws every number on the jobs plate, and joins the list with it.
    ///
    /// Ablated by deleting the `small` arm of `missing_fonts`: the list comes
    /// back with two names and this goes red naming the third. Verified.
    #[test]
    fn a_bare_shell_names_every_font_it_could_not_load() {
        let missing = ShellAssets::empty().missing_fonts();
        assert_eq!(
            missing,
            vec![font::BODY, font::HEADING, font::SMALL, font::EIGHT, font::TEN],
            "every face a Pen or a painter falls back from has to be in this list"
        );
    }

    #[test]
    fn the_recess_lights_its_top_and_right_and_shades_its_bottom_and_left() {
        let mut c = Canvas::new(20, 10);
        button_recess(&mut c, 2, 2, 10, 6);
        assert_eq!(c.at(2, 2), 0x28, "the top-left corner belongs to the left edge");
        assert_eq!(c.at(6, 2), 0x35, "top");
        assert_eq!(c.at(11, 4), 0x35, "right");
        assert_eq!(c.at(6, 7), 0x28, "bottom");
        assert_eq!(c.at(2, 4), 0x28, "left");
        assert_eq!(c.at(6, 4), 0, "and the middle is left alone");
    }
}


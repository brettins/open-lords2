//! Page 12's second half: the words it draws, the banner that moves and the
//! muster the *Cust.* flag puts in place of the description.
#![allow(unused_imports)]
use super::*;
use l2_game::screen::{Ctx, Screen};
use l2_game::screens::setup::skirmish::TroopsTable;
use l2_game::screens::setup::{SetupPage, SetupScreen};
use l2_view::Canvas;

fn battle(s: &SetupScreen, game: &mut Game, assets: &Assets, row: usize) -> (String, String) {
    let ctx = Ctx { game, assets };
    let t = s.battle_text(&ctx, row);
    (t.name, t.title)
}

fn paint(s: &mut SetupScreen, game: &mut Game, assets: &Assets) -> Canvas {
    let mut canvas = Canvas::new(640, 480);
    let ctx = Ctx { game, assets };
    s.draw(&ctx, &mut canvas);
    canvas
}

/// **`(DAT_0053E91C * 10 + row) * 3`**, `00420000.c:396` and `:432`. The stride
/// is ten *whatever the category's row count is*, so the fifteen castles start
/// at entry 20 and not at entry 30. Ablating the `10` in
/// `l2_formats::battles::ROWS_PER_CATEGORY` to the category's own row count
/// puts category 2 row 0 on `Squigglewood`.
#[test]
fn the_list_rows_and_the_heading_come_out_of_battles_eng() {
    let (mut game, assets) = world!();
    if assets.shell.battles.is_empty() {
        l2_testkit::skip!("no BATTLES.ENG in this install");
    }
    let mut s = SetupScreen::new(SetupPage::Skirmish);
    assert_eq!(
        battle(&s, &mut game, &assets, 0),
        ("Three Bridges".to_string(), "A Bridge Too Far".to_string())
    );
    // Category 1 row 0 is entry 10, `Baffled`; category 2 row 0 is entry 20
    // and row 14 entry 34, the last castle.
    s.skirmish_mut().choose_kind(1);
    assert_eq!(battle(&s, &mut game, &assets, 0).0, "Baffled");
    s.skirmish_mut().choose_kind(2);
    assert_eq!(battle(&s, &mut game, &assets, 0).0, "The Palisade");
    assert_eq!(battle(&s, &mut game, &assets, 14).0, "Expanded Keep");
}

/// **`FUN_0042AA90`** turns every byte under `0x20` into a NUL and
/// **`FUN_0042AB15`** counts only the runs that are not empty, so the shipped
/// file is 171 non-empty lines and exactly 57 triples with nothing over.
#[test]
fn battles_eng_is_fifty_seven_whole_triples() {
    let (_game, assets) = world!();
    if assets.shell.battles.is_empty() {
        l2_testkit::skip!("no BATTLES.ENG in this install");
    }
    assert_eq!(assets.shell.battles.len(), 171);
    assert_eq!(assets.shell.battles.count(), 57);
}

/// **`FUN_00420D40`** (`00420000.c:4761-4765`): one frame per 100 ms, twelve of
/// them, and `Gfx_MarkAllDirty` on each step. Ablate the wrap and frame 12 is
/// asked of a sheet that has eleven past `0x21`.
#[test]
fn the_banner_steps_one_frame_per_hundred_milliseconds_and_wraps_at_twelve() {
    let (mut game, assets) = world!();
    let mut s = SetupScreen::new(SetupPage::Skirmish);
    let per_frame = 100 / l2_game::press::TICK_MS + 1;
    assert_eq!(s.banner_frame(), 0, "DAT_004D31E8 starts at 0");
    for _ in 0..per_frame {
        tick(&mut s, &mut game, &assets);
    }
    assert_eq!(s.banner_frame(), 1);
    assert!(s.take_redraw(), "each step calls Gfx_MarkAllDirty");
    for _ in 0..per_frame * 11 {
        tick(&mut s, &mut game, &assets);
    }
    assert_eq!(s.banner_frame(), 0, "0xB < n puts it back to 0");
}

/// **`FUN_0043DD83` ends with `Troops_Load`** (`00430000.c:8126`), and which
/// side the local player is on picks the *file*. Ablate the `load_troops` call
/// in the `Sides` arm and the page keeps `TROOPS2.ENG` after the swap.
#[test]
fn swapping_sides_reloads_the_troops_file() {
    let (mut game, assets) = world!();
    if assets.shell.troops(false, true).is_none() {
        l2_testkit::skip!("no TROOPS2.ENG in this install");
    }
    let mut s = SetupScreen::new(SetupPage::Skirmish);
    tick(&mut s, &mut game, &assets);
    assert_eq!(s.troops_file(), Some("TROOPS2.ENG"), "realm 1 attacks on the way in");
    // Table row 10 — category 1's first — is the first row the two files
    // disagree on; rows 0…9 are identical in both.
    let cell = |s: &SetupScreen| s.troops().counts(1, 0, 0, 2);
    let before = cell(&s);
    // `FUN_0043DD83`'s own rectangle, the left muster.
    click(&mut s, &mut game, &assets, 90, 280);
    assert_eq!(s.troops_file(), Some("TROOPS3.ENG"), "defending, the table is the other file");
    let reparsed = TroopsTable::from_file(assets.shell.troops(false, false).expect("TROOPS3.ENG"));
    assert_eq!(cell(&s), reparsed.counts(1, 0, 0, 2), "and it is that file, parsed");
    assert_ne!(before, cell(&s), "the two files are not the same table");
}

/// **`Troops_Load`'s three passes** (`00420000.c:4418-4500`): the siege columns
/// are clamped to 9, the four dead difficulty columns are overwritten with
/// Normal, and slots 0…6 are re-derived at 116/108/92/84 per cent. Ablate the
/// re-derivation and every handicap shows the same army.
#[test]
fn the_shipped_table_is_normal_plus_or_minus_the_engines_curve() {
    let (_game, assets) = world!();
    let Some(t) = assets.shell.troops(false, true) else {
        l2_testkit::skip!("no TROOPS2.ENG in this install");
    };
    let mut checked = 0;
    for row in 0..l2_formats::troops::ROWS {
        for side in 0..2 {
            let normal = t.counts(row, side, 2);
            for slot in 0..7 {
                let n = normal[slot] as i32;
                assert_eq!(t.counts(row, side, 0)[slot] as i32, n + n * 16 / 100);
                assert_eq!(t.counts(row, side, 4)[slot] as i32, n - n * 16 / 100);
                checked += usize::from(n != 0);
            }
            for slot in 7..11 {
                assert!(normal[slot] <= 9, "row {row} slot {slot} is clamped");
                assert_eq!(t.counts(row, side, 0)[slot], normal[slot], "engines are flat");
            }
        }
    }
    assert!(checked > 100, "only {checked} non-zero Normal entries; the file did not parse");
}

/// **`FUN_0042130F`** draws **eleven** rows of counts when the category is a
/// castle and **seven** otherwise (`00420000.c:5361-5365`), because the four
/// siege columns are only offered for a siege. The eighth row's band is the
/// ablation: fix the count at seven and the two paints are the same.
#[test]
fn the_custom_muster_grows_four_rows_for_a_siege() {
    let (mut game, assets) = world!();
    if !assets.shell.has_artwork() {
        l2_testkit::skip!("no artwork, so nothing is drawn in the game's own font");
    }
    let mut s = SetupScreen::new(SetupPage::Skirmish);
    tick(&mut s, &mut game, &assets);
    // *Cust.* — `FUN_0043DA9E`, the middle caption at y 0x1B8.
    click(&mut s, &mut game, &assets, 0x207 + 0x1C, 0x1B8 + 6);
    assert!(s.skirmish().custom, "DAT_0056899C is up");
    let field = paint(&mut s, &mut game, &assets);
    // The castle strip, which the four siege rows belong to.
    click(&mut s, &mut game, &assets, 470, 291 + 15);
    assert_eq!(s.skirmish().kind, 2);
    let siege = paint(&mut s, &mut game, &assets);
    assert!(
        row_ink(&siege, 0x6C + 7 * 0x1E) > row_ink(&field, 0x6C + 7 * 0x1E),
        "the eighth count row is drawn for a castle and not for a field battle"
    );
}

/// **`FUN_0041E61D`** (`00410000.c:4674-4679`) reads `skirmish.256` for pages
/// 11 and up either way and swaps only the `.pl8`.
#[test]
fn the_custom_flag_swaps_the_background_and_not_the_palette() {
    for page in [SetupPage::SkirmishMulti, SetupPage::Skirmish, SetupPage::SkirmishFile] {
        assert_eq!(page.background_custom(false), "Skirmish.pl8");
        assert_eq!(page.background_custom(true), "Skircust.pl8");
        assert_eq!(page.palette(), "Skirmish.256");
    }
    assert_eq!(SetupPage::Custom.background_custom(true), "Custom.pl8");
    assert_eq!(SetupPage::Title.background_custom(true), "Gateway.pl8");
}

/// **`Files_Enumerate("*.skr", 1)`** and **`FUN_0042D4AA`**
/// (`00420000.c:5257-5287`): the scan fills the page-13 list, and taking a name
/// lays the file's twenty musters into rows 35…54 at 5/3, 4/3, 1, 2/3 and 1/3.
/// Ablate the `load_skr` call on the pick and the row stays all zeroes.
#[test]
fn a_skr_pick_lays_its_twenty_musters_into_rows_thirty_five_and_up() {
    let (mut game, assets) = world!();
    if assets.shell.skirmish_files.is_empty() {
        l2_testkit::skip!("no .skr file in this install");
    }
    let name = assets.shell.skirmish_files[0].clone();
    let skr = assets.shell.skr(&name).expect("the scan found a file that parses");
    let mut s = SetupScreen::new(SetupPage::Skirmish);
    tick(&mut s, &mut game, &assets);
    assert!(s.troops().rows.len() >= 35, "Troops_Load fills 35 rows");

    // `FUN_0043DDF4`, the `.skr` field, then `FUN_00434174`, row 0.
    click(&mut s, &mut game, &assets, 560, 399);
    assert_eq!(s.page(), SetupPage::SkirmishFile);
    click(&mut s, &mut game, &assets, 200, 0xB0 + 8);
    assert_eq!(s.skirmish().kind, 3, "FUN_00434174 sets the category");
    assert_eq!(s.troops().rows.len(), 55, "twenty more rows");

    let want = skr.army(0, l2_formats::Side::Attacker).expect("map 0").counts;
    let normal = s.troops().counts(3, 0, 0, 2);
    let hardest = s.troops().counts(3, 0, 0, 4);
    for slot in 0..11 {
        assert_eq!(normal[slot] as u32, want[slot], "Normal is the file's own count");
        assert_eq!(hardest[slot] as u32, want[slot] / 3, "Very hard is a third of it");
    }
    // The names come from the file's text table, not from BATTLES.ENG.
    assert_eq!(battle(&s, &mut game, &assets, 0).0, skr.text(0).unwrap().name);
}

/// **`FUN_0042B9C4`** (`00420000.c:4608-4619`) sends category 3 to
/// `Battlefield_BuildFromSkr` (`0x0047B8B2`) and categories 0 and 1 to
/// `Battlefield_BuildRandom`'s skirmish branch, whose frame is `DAT_0056D590`
/// out of `batfiel2.pl8`, `+10` for category 1 (`00470000.c:4487-4493`).
/// Ablate either and *Go* raises the campaign's seed-picked field instead.
#[test]
fn the_field_go_raises_is_the_row_and_not_the_seed() {
    let (mut game, assets) = world!();
    if assets.shell.skirmish_files.is_empty() {
        l2_testkit::skip!("no .skr file in this install");
    }
    let mut s = SetupScreen::new(SetupPage::Skirmish);
    tick(&mut s, &mut game, &assets);

    let row0 = field_of(&s, &mut game, &assets);
    s.skirmish_mut().pick_row(3);
    let row3 = field_of(&s, &mut game, &assets);
    assert_ne!(row0, row3, "the row picks the field, and the seed is the same");

    // Category 1 row 0 is frame 10, not frame 0.
    s.skirmish_mut().choose_kind(1);
    assert_ne!(field_of(&s, &mut game, &assets), row0, "+10 is a different frame");

    // And a `.skr` pick raises that file's own layer.
    let name = assets.shell.skirmish_files[0].clone();
    let layer = assets.shell.skr(&name).unwrap().terrain(0).unwrap().to_vec();
    click(&mut s, &mut game, &assets, 560, 399);
    click(&mut s, &mut game, &assets, 200, 0xB0 + 8);
    assert_eq!(s.skirmish().kind, 3);
    assert_eq!(field_of(&s, &mut game, &assets), l2_sim::terrain::build(&layer, 1).cells);
}

fn field_of(s: &SetupScreen, game: &mut Game, assets: &Assets) -> Vec<l2_sim::terrain::Cell> {
    let ctx = Ctx { game, assets };
    s.skirmish_field(&ctx, 1).cells
}

/// How many pixels of the two count columns' band carry one of the two ink
/// colours `FUN_0042130F` uses — `0x20` for a count that is not zero, `0x3F`
/// for one that is and for every strength weight.
fn row_ink(canvas: &Canvas, y: i32) -> usize {
    let mut n = 0;
    for dy in 0..16i32 {
        for x in 0x37..0xBAi32 {
            let p = canvas.at(x as usize, (y + dy) as usize);
            if p == 0x3F || p == 0x20 {
                n += 1;
            }
        }
    }
    n
}

#![allow(unused_imports)]
use super::*;
use super::labels::*;
use super::*;
use super::strip_and_sidebar::*;
use super::panels::*;
use super::drawing_and_emboss::*;
use super::produce_and_pastures::*;
use common::*;
use l2_game::game::Assets;
use l2_game::game::MAX_TAX_RATE;
use l2_game::input::Event;
use l2_game::input::Key;
use l2_game::screen::Ctx;
use l2_game::screen::Machine;
use l2_game::screen::Screen;
use l2_game::screen::ScreenId;
use l2_game::screen::Transition;
use l2_game::screens::county::{self as county};
use l2_game::screens::county::CountyScreen;
use l2_game::screens::county::Panel;
use l2_game::screens::map;
use l2_game::screens::map::MapScreen;
use l2_game::screens::village::{self as village_screen};
use l2_game::screens::village::VillageScreen;
use l2_game::shell::font;
use l2_game::Game;
use l2_view::campaign;
use l2_view::Canvas;

/// **`Panel_Ration`'s five centred numbers, at the x its own arithmetic gives.**
///
/// The painter is `Ui_DrawNumberRight(value, ' ', "", x, y, 0x40, body, 0x3F)`
/// five times, and that function ends in `FUN_004025D7`:
///
/// ```c
/// local_c = (width - FUN_004014F0(buffer, font)) / 2;
/// if (local_c < 0) local_c = 0;
/// Ui_DrawText(buffer, local_c + x, y, font, colour);
/// ```
///
/// So the **whole buffer** — `lead + digits + suffix` — is centred in the
/// 64-pixel column, and the digits then begin one space-advance further right
/// because the lead is a space. `FUN_004014F0` charges four pixels for a space
/// wherever it sits and trims nothing, so the suffix is inside the measure.
///
/// # The suffix is what is under test, and the expectation does not mention it
///
/// `Panel_Ration`'s five suffix arguments are `&DAT_004D3E04`, `…08`, `…0C`,
/// `…10` and `…14` — five addresses inside a run of zero bytes in `.data` that
/// ends where `"villani1.pl8"` begins, so **every one of them is the empty
/// string**. Fifteen of the image's other `Ui_DrawNumberRight` sites pass a
/// single space instead, and [`Pen::number_centred`] used to build `" {v} "`
/// for all twenty.
///
/// The expectation below is `format!(" {value}")` — the **lead**, a different
/// literal from the one being ablated — so restoring the trailing space moves
/// the picture two pixels left and leaves this assertion exactly where it is.
/// That is the trap `docs/agents.md` calls *compute the probe from the constant
/// you are ablating*, avoided by building the probe from the argument that is
/// not in question. Measured on the England fixture: 505 in column 2 lands at
/// **343** with the empty suffix and at **341** with a one-space suffix.
/// `docs/decisions.md` C140.
#[test]
fn the_ration_panels_five_numbers_centre_where_panel_ration_centres_them() {
    let (mut game, assets) = world!();
    let c = &game.kingdom.counties[8];
    let (by_grain, by_meat, by_dairy) =
        l2_kingdom::ration::people_fed(&game.kingdom.tables, c);
    let (grain_eaten, herd_eaten) = (c.grain_eaten, c.herd_eaten);
    // The fixture's own numbers, so a changed fixture fails here and not in the
    // geometry below. County 8's herd feeds 505 and the county eats nothing.
    assert_eq!((by_grain, by_meat, by_dairy), (0, 0, 505));
    assert_eq!((grain_eaten, herd_eaten), (0, 0));

    let mut screen = CountyScreen::new(8, Panel::Ration);
    let canvas = draw(&mut screen, &mut game, &assets);

    // `Ui_DrawNumberRight(…, x, y, 0x40, …)`: the five (x, y, value) triples
    // transcribed from `Panel_Ration` at `0x00411B72`, in its own order.
    let sites = [
        (0xD0, 0x134, grain_eaten),
        (0xD0, 0x11E, by_grain),
        (0x10A, 0x134, herd_eaten),
        (0x10A, 0x11E, by_meat),
        (0x144, 0x11E, by_dairy),
    ];
    const WIDTH: i32 = 0x40;
    let lead_advance = body_width(&assets, " ");
    for (x, y, value) in sites {
        let string = format!(" {value}");
        let offset = ((WIDTH - body_width(&assets, &string)) / 2).max(0);
        let digits = format!("{value}");
        // Search only inside the column, because "0" is drawn in several other
        // places on this panel and a whole-canvas search would find one of them.
        let box_h = 20;
        let window = crop(&canvas, x, y, WIDTH + 8, box_h);
        assert_eq!(
            find_body(&window, &assets, &digits, font::TEXT),
            Some((offset + lead_advance, 0)),
            "Ui_DrawNumberRight({value}, ' ', \"\", {x:#X}, {y:#X}, 0x40): the buffer is \
             {string:?}, centred in 64, and the digits start one space into it"
        );
    }

    // And the whole-canvas position of the one number that is unambiguous, as a
    // hard integer: 0x144 + (0x40 - 34) / 2 + 4.
    assert_eq!(
        find_body(&canvas, &assets, "505", font::TEXT),
        Some((343, 286)),
        "the dairy column, at the x Panel_Ration's arithmetic produces"
    );
}

/// **The foraging line's label, which `Ui_DrawText`'s own trailing gap places.**
///
/// `Panel_Ration`'s `Armies Eat` tail is three statements:
///
/// ```c
/// g_penAdvance = 0;
/// Ui_DrawNumber(county+0x19C + county+0x198, ' ', "", 0x88, 0x150, body, 0x3F);
/// Eng_DrawString(87, 8, g_penAdvance + 0x88, 0x150, body, 0x3F);
/// ```
///
/// `Ui_DrawText` ends with `g_penAdvance += 4`, which is
/// [`l2_game::shell::TRAILING`] and which `Pen::body` already adds to its
/// return. The `" {men} "` this used to build therefore charged the gap twice
/// and put *"are foraging"* four pixels right of where `Eng_DrawString` lands
/// it. The transcription in the comment above the line had the suffix right —
/// `' ', ""` — and the line below it did not use it,
/// `docs/agents.md`'s *a correct explanation sitting directly above the
/// omission it describes*. `docs/decisions.md` C140.
#[test]
fn the_foraging_label_starts_one_trailing_gap_after_its_number() {
    let (mut game, assets) = world!();
    game.kingdom.options.armies_eat = true;
    let c = &game.kingdom.counties[8];
    let men = c.friendly_troops + c.enemy_troops;

    let mut screen = CountyScreen::new(8, Panel::Ration);
    let canvas = draw(&mut screen, &mut game, &assets);

    let label = assets.shell.text(87, 8).to_string();
    assert!(!label.is_empty(), "L2.eng 87/8 is the foraging caption");
    // `Ui_DrawNumber(men, ' ', "", 0x88, 0x150)` then the label at
    // `g_penAdvance + 0x88`, and `g_penAdvance` is the buffer's width plus four.
    let expected = 0x88 + body_width(&assets, &format!(" {men}")) + l2_game::shell::TRAILING;
    assert_eq!(
        find_body(&canvas, &assets, &label, font::TEXT),
        Some((expected, 0x150)),
        "87/8 belongs at g_penAdvance + 0x88 for a buffer of \" {men}\", not \" {men} \""
    );
}

/// **The grain row's sowing loss, from the brush to the pixel.**
///
/// > *"Sidebar doesn't show grain being planted as a negative number."*
///
/// [`the_cattle_row_draws_its_forecast_with_a_sign`] proved `Ui_DrawDelta`'s
/// three arms on the row whose data path was finished first, and it says so in
/// its own doc: *"this is the **cattle** row … which therefore proves the
/// drawing half before the expensive half lands on it."* The expensive half
/// landed — `Grain_LabourEstimate`'s tail, `docs/decisions.md` C123 — and
/// nothing joined the two ends. This is the join.
///
/// **It writes no county field.** The cattle test assigns
/// `herd_change_expected` directly, which is right for a test about
/// `Ui_DrawDelta` and says nothing about whether anything fills it. Here the
/// only inputs are a granary, the map brush and `County_RefreshEstimates`, so
/// what is asserted is the whole road: `Field_SetType` → `Labour_Allocate` →
/// `Grain_LabourEstimate`'s tail → `FUN_0041023A` → `Ui_DrawDelta`. That is
/// `docs/agents.md`'s *a field is only tested if something a test reads was
/// written by something the game runs*, and it is the distinction that let the
/// hole exist while eleven village tests passed.
///
/// Two claims:
///
/// 1. **the number the simulation computed is the number on the plate**, in
///    `colourNeg` (`0xF9`) with a `'-'` lead, and in no other colour;
/// 2. **it is on the produce plate** - *"is it
///    drawn"* and *"can it be seen"* are different claims (`docs/agents.md`).
///
/// Ablations, both run, and they fail at **different** assertions, which is the
/// join working: deleting the `strip_delta` call in `county::draw_produce_rows`
/// fails claim 1 at the glyph search (*"the grain row draws `-20 `"*), and
/// deleting the `grain_preview` call in `l2_kingdom::field::refresh_estimates`
/// fails one line earlier, at the simulation's own `shown < 0` with `0`. A test
/// that only did the second half could not tell those two apart.
#[test]
fn the_grain_row_draws_its_sowing_loss_from_the_brush_to_the_pixel() {
    let (mut game, assets) = world!();
    // `&g_font10` — the face every number on the jobs plate is drawn in.
    let Some(f) = assets.shell.ten.as_ref() else {
        l2_testkit::skip!("no Font_10.pl8, so the jobs plate has no numbers");
    };
    // `colourNeg` and `colourPos`, typed from `FUN_0041023A`'s call site rather
    // than imported from the constants under test.
    const NEG: u8 = 0xF9;
    const POS: u8 = 0xFA;

    let county = (1..=game.kingdom.county_count)
        .find(|&id| game.kingdom.counties[id].owner == game.player)
        .expect("the local player holds a county");
    game.select(county as u8);

    // A granary, then the brush. Seed first: the grain ceiling is a search over
    // `Grain_Sow`, and a county with an empty store is told it has no use for a
    // farmer.
    game.kingdom.counties[county].grain = 10_000;
    let tiles: Vec<usize> = game
        .kingdom
        .field_tiles(county)
        .into_iter()
        .filter(|&(_, kind)| kind == l2_kingdom::field::FieldType::Fallow)
        .map(|(tile, _)| tile)
        .collect();
    assert!(!tiles.is_empty(), "county {county} has fallow fields to paint");
    for tile in tiles {
        game.kingdom
            .paint_field(county, tile, l2_kingdom::field::FieldType::Grain)
            .expect("a fallow field takes the grain brush");
    }

    // The position faces Spring, which is the sowing turn. Nothing else is set.
    assert_eq!(game.kingdom.season_next, 1, "the England position faces Spring");
    let shown = game.kingdom.counties[county].grain_change_expected;
    assert!(shown < 0, "the simulation forecasts a sowing loss: {shown}");

    let canvas = draw(&mut MapScreen::new(), &mut game, &assets);
    // `Ui_DrawNumber` writes the sign into `g_numberBuffer[0]` and the suffix
    // after the digits, and both are inside what gets measured — so the string
    // searched for is the call site's, `'-'` + digits + `" "`. C140.
    let wanted = format!("-{} ", shown.abs());
    let at = find_font_text(&canvas, f, &wanted, NEG)
        .unwrap_or_else(|| panic!("the grain row draws {wanted:?} in {NEG:#04x}"));
    // 2 — on the produce plate. `Ui_DrawDelta(…, 0x204, y + 0x139, …)`, and the
    // grain row is the second of the farm list, so it is at or below the
    // cattle row's own delta at (478, 302).
    assert!(
        at.0 >= 478 && at.1 >= 302,
        "the grain delta belongs on the produce plate, not at {at:?}",
    );
    // And not in the positive colour, which is the half a swapped pair would
    // survive.
    assert!(
        find_font_text(&canvas, f, &wanted, POS).is_none(),
        "a negative delta is 0xF9, not 0xFA",
    );
}

/// **The reclamation row's two numbers, and they are two different routines.**
///
/// > *"The figure is missing in the sidebar — it draws the serf reclaiming, but
/// > not the +1 I'm used to."*
///
/// `Field_ReclaimEstimate` (`0x0044C278`) is the third of the three tails
/// (`docs/decisions.md` C129), and the row it feeds is the only produce row
/// with **two** figures, drawn by two different functions at two different
/// offsets:
///
/// ```c
/// Ui_DrawDelta (county +0x20C, 0, " ", " ", 0x204, y + 0x133, …, 0xFA, 0xF9);
/// if (county +0x214) Ui_DrawNumber(county +0x214, ' ', " ", 0x20A, y + 0x143, …, 0xFA);
/// ```
///
/// Three claims, and the first is the one that makes this worth a test of its
/// Own copy, not the grain row's:
///
/// 1. **The two land sixteen pixels apart in `y` and are anchored differently
///    in `x`** — `0x204`/`0x133` against `0x20A`/`0x143`. Reading only
///    `CountyStrip_Draw` would give one figure; the offsets are the evidence
/// there are two. The countdown's digits sit on its own `x` and the delta's
///    do **not**, because `Ui_DrawDelta` draws a `" "` prefix first and places
///    the number at `x + g_penAdvance` — so the two are asserted differently on
/// purpose, and the delta's half is C127 on this row.
/// 2. **The delta is a count of fields, not of work**, so a gang with enough
///    labour for two finished fields draws `+2` and not `+1`.
/// 3. **The countdown is drawn only when it is non-zero** — the original's own
/// `if`, so a county reclaiming nothing shows a bare icon.
///
/// The row is the second of the farm list here (cattle, then reclamation, with
/// no grain), so the pitch is `0x3C` and both `y`s carry one row of it. That is
/// Asserted: a wrong pitch would move both figures
/// together and claim 1 would still hold.
///
/// Ablation, run: deleting the `ten_number` call (then `strip_number`) fails claim 1's second half
/// while the delta stays exactly where it is, which is the pair the two-routine
/// claim needs.
#[test]
fn the_reclamation_row_draws_both_of_its_figures_where_the_call_sites_put_them() {
    let (mut game, assets) = world!();
    // `&g_font10` — the face every number on the jobs plate is drawn in.
    let Some(f) = assets.shell.ten.as_ref() else {
        l2_testkit::skip!("no Font_10.pl8, so the jobs plate has no numbers");
    };
    const POS: u8 = 0xFA;

    let county = (1..=game.kingdom.county_count)
        .find(|&id| game.kingdom.counties[id].owner == game.player)
        .expect("the local player holds a county");
    game.select(county as u8);

    // Two fields one season's work from done, and a gang big enough for both.
    let per = game.kingdom.tables.field.reclaim_per_season;
    let full = game.kingdom.tables.field.progress_max;
    let slots: Vec<usize> = (0..l2_kingdom::MAX_FIELDS)
        .filter(|&s| game.kingdom.counties[county].field_tile(s).is_some())
        .collect();
    for &s in slots.iter().take(2) {
        let tile = game.kingdom.counties[county].field_tile(s).expect("a tile");
        game.kingdom.campaign.map.terrain[tile] = l2_kingdom::field::terrain::RECLAIM_FIRST;
        game.kingdom.counties[county].field_progress[s] = (full - per) as u16;
    }
    game.kingdom.counties[county].labour[l2_kingdom::tables::JOB_FIELD_RECLAMATION] = per * 2;
    game.kingdom.refresh_estimates(county);
    // The cattle row is above this one and would otherwise put its own signed
    // number on the plate; silenced so that a match below is this row's.
    // `Ui_DrawDelta` in mode 0 draws nothing at all for zero.
    game.kingdom.counties[county].herd_change_expected = 0;

    let c = &game.kingdom.counties[county];
    assert_eq!(c.reclaim_fields_finishing, 2, "two gangs' worth finishes two fields");
    assert_eq!(c.reclaim_seasons_to_next, 1, "and the nearest is one season away");
    // The farm list is cattle then reclamation — no grain, this position sows
    // none — so the reclamation row is row 1 at the wide pitch.
    let rows = county::farm_rows(c);
    assert_eq!(rows, vec![1, 2], "cattle then reclamation");
    let pitch = county::farm_pitch(rows.len());
    assert_eq!(pitch, 0x3C, "two rows are drawn at the wide pitch");

    let canvas = draw(&mut MapScreen::new(), &mut game, &assets);

    // 1 and 2 — the delta, at `0x204 + the prefix's advance`, `y + 0x133`.
    let delta = find_font_text(&canvas, f, "+2 ", POS).expect("the reclamation row draws +2");
    assert_eq!(delta.1, pitch + 0x133, "the delta sits on the row's 0x133 line");
    // 1 and 3 — the countdown, sixteen pixels below it and six to the right,
    // It is `Ui_DrawNumber` with a `' '` lead.
    let countdown = find_font_text(&canvas, f, " 1 ", POS).expect("and the seasons countdown");
    assert_eq!(countdown, (0x20A, pitch + 0x143), "the countdown is its own call site");
    assert_eq!(
        countdown.1 - delta.1,
        0x10,
        "two figures, two lines: {delta:?} and {countdown:?}",
    );
    // **And the delta's digits are not at its `x`.** `Ui_DrawDelta` draws the
    // `" "` prefix at `0x204`, lets it advance the pen, and places the number
    // at `x + g_penAdvance` — so a match at `0x204` itself would mean the
    // advance had been dropped, which is C127 on this row. The countdown is a
    // Plain `Ui_DrawNumber` does sit on its own `x`; the two
    // assertions are different shapes.
    assert!(
        delta.0 > 0x204,
        "the delta's prefix advances the pen before the number: {delta:?}",
    );

    // 3 — and a county reclaiming nothing draws neither.
    for &s in slots.iter().take(2) {
        let tile = game.kingdom.counties[county].field_tile(s).expect("a tile");
        game.kingdom.campaign.map.terrain[tile] = l2_kingdom::field::terrain::FALLOW;
    }
    game.kingdom.refresh_estimates(county);
    game.kingdom.counties[county].herd_change_expected = 0;
    let canvas = draw(&mut MapScreen::new(), &mut game, &assets);
    assert!(find_font_text(&canvas, f, "+2 ", POS).is_none(), "nothing reclaiming, no delta");
    assert!(find_font_text(&canvas, f, " 1 ", POS).is_none(), "and no countdown either");
}



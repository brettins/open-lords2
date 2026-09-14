#![allow(unused_imports)]
use super::*;
use super::top_bar::*;
use super::panels::*;
use super::png_part::*;
use std::path::PathBuf;
use l2_game::game::Assets;
use l2_game::input::{Event, Key};
use l2_game::screen::{Ctx, Screen};
use l2_game::screens::map::MapScreen;
use l2_game::screens::setup::{SetupPage, SetupScreen};
use l2_game::shell::font::{self, Font, Style};
use l2_game::{scenario, Game};
use l2_kingdom::tables::Tables;
use l2_mods::Platform;
use l2_view::Canvas;

/// `Ui_DrawText`: a glyph-less lead character advances `local_14 = 4`.
const LEAD: i32 = 4;
/// `" "` as a suffix, the same advance.
const SPACE: i32 = 4;
/// `Ui_DrawText`'s last line, `g_penAdvance + 4`.
const TRAILER: i32 = 4;

fn own_county(game: &Game) -> u8 {
    (1..=game.kingdom.county_count as u8)
        .find(|&id| game.is_players(id))
        .expect("the local player holds a county")
}

/// [`find_on_row`], but only at or right of `from` — for a number that could
/// otherwise be found as the prefix of a longer number earlier on its row.
fn find_on_row_from(
    canvas: &Canvas,
    f: &Font,
    s: &str,
    colour: u8,
    y: i32,
    from: i32,
) -> Option<i32> {
    let mut probe = Canvas::new(canvas.width - from as usize, canvas.height);
    for py in 0..canvas.height {
        for px in from as usize..canvas.width {
            probe.set(px - from as usize, py, canvas.at(px, py));
        }
    }
    find_on_row(&probe, f, s, colour, y).map(|x| x + from)
}

/// **The castle's garrison is `"@40 "`, and *"troops."* is chained off it, so
/// both moved.**
///
/// ```c
/// /* Screen_CastleBuildPanel, 0x004198AA */
/// Ui_DrawNumber(g_castleGarrisonCap[sel], '@', &DAT_004D4160, 0xC, 0xE2, &g_fontBody, 0x3F);
/// Eng_DrawString(0x47, 0xC, g_penAdvance + 0xE, 0xE2, &g_fontBody, 0x3F);
/// ```
///
/// `DAT_004D4160` is one space. The old `Pen::number(…, true)` drew `"40 "`: the
/// space right by accident, the lead missing — digits **and** noun four left.
///
/// Ablated twice, each red on its own assertion:
/// * the old `pen.body(…, &format!("{cap} "))` restored at the call site — the
///   digits are found at **12** where 16 is expected, the defect as it shipped;
/// * suffix `" "` → `""` — the digits stay at 16 and the noun is found at **51**
///   where 55 is expected.
#[test]
fn the_castle_s_garrison_and_its_noun_both_start_one_sign_column_right() {
    use l2_game::screens::castle::CastleScreen;
    let (mut game, assets) = world!();
    let body = assets.shell.body.as_ref().expect("Fntl2_14.pl8");
    let county = own_county(&game);
    let mut screen = CastleScreen::new(county);
    let cap = {
        let ctx = Ctx { game: &mut game, assets: &assets };
        l2_kingdom::industry::garrison_cap(&ctx.game.kingdom.tables, screen.castle_type(&ctx))
    };
    let canvas = draw(&mut screen, &mut game, &assets);

    const X: i32 = 0x0C; // Ui_DrawNumber(cap, '@', " ", 0xC, 0xE2, …)
    const Y: i32 = 0xE2;
    const NOUN_X: i32 = 0x0E; // Eng_DrawString(71, 0xC, g_penAdvance + 0xE, …)

    let digits = cap.to_string();
    assert_eq!(
        find_on_row(&canvas, body, &digits, font::TEXT, Y),
        Some(X + LEAD),
        "the garrison's digits are not one sign column right of 0x0C"
    );
    let noun = assets.shell.text(71, 0x0C).to_string();
    assert!(!noun.is_empty(), "L2.eng 71/12 is the garrison's noun");
    assert_eq!(
        find_on_row(&canvas, body, &noun, font::TEXT, Y),
        Some(NOUN_X + LEAD + body.width(&digits) + SPACE + TRAILER),
        "{noun:?} is not at g_penAdvance + 0xE after \"@{digits} \""
    );
}

/// **The mercenary's price line: both numbers moved and neither noun did.**
///
/// ```c
/// /* Screen_RaiseArmy, 0x00418653 */
/// g_penAdvance = 0;
/// Ui_DrawNumber(price, '@', &DAT_004D40C8, 0x70, base + 0x7C, &g_fontBody, 0x3F);
/// Eng_DrawString(0x45, 0, g_penAdvance + 0x70, base + 0x7C, &g_fontBody, 0x3F);
/// Ui_DrawNumber(men / 2, '@', &DAT_004D40CC, g_penAdvance + 0x70, …);
/// Eng_DrawString(0x45, 1, g_penAdvance + 0x70, …);
/// ```
///
/// Both suffixes are NUL. The old `"{n} "` lost four at the front and added four
/// at the back, so the nouns landed right and the digits did not —
/// the nouns are asserted as well: a fix that adds the lead and keeps the
/// invented space moves them.
///
/// Ablated twice, each red on its own assertion:
/// * the price put back to the old `pen.body(…, &format!("{} ", price))` — found
///   at **112** where 116 is expected;
/// * the price's suffix `""` → `" "` — the price stays at 116 and *"crowns to
///   hire."* is found at **164** where 160 is expected.
#[test]
fn the_mercenary_price_line_moves_its_numbers_and_not_its_nouns() {
    use l2_game::screens::army::{self, RaiseArmyScreen};
    let (mut game, assets) = world!();
    let body = assets.shell.body.as_ref().expect("Fntl2_14.pl8");
    let county = own_county(&game);
    const BAND: u8 = 3;
    game.kingdom.counties[county as usize].mercenary_offer = BAND;
    let rules = &l2_kingdom::mercenary::ROSTER[BAND as usize];
    let mut screen = RaiseArmyScreen::new(county);
    let canvas = draw(&mut screen, &mut game, &assets);

    const X: i32 = 0x70;
    let y = army::base(true) + 0x7C;

    let price = rules.price.to_string();
    assert_eq!(
        find_on_row(&canvas, body, &price, font::TEXT, y),
        Some(X + LEAD),
        "the price is not one sign column right of 0x70"
    );
    let hire = assets.shell.text(0x45, 0).to_string();
    let hire_x = X + LEAD + body.width(&price) + TRAILER;
    assert_eq!(
        find_on_row_from(&canvas, body, &hire, font::TEXT, y, X),
        Some(hire_x),
        "{hire:?} is not at g_penAdvance + 0x70 after \"@{price}\""
    );
    let wages = (rules.men / 2).to_string();
    let wages_x = hire_x + body.width(&hire) + TRAILER + LEAD;
    assert_eq!(
        find_on_row_from(&canvas, body, &wages, font::TEXT, y, hire_x),
        Some(wages_x),
        "the wages are not one sign column after {hire:?}"
    );
    let seasonal = assets.shell.text(0x45, 1).to_string();
    assert_eq!(
        find_on_row_from(&canvas, body, &seasonal, font::TEXT, y, wages_x),
        Some(wages_x + body.width(&wages) + TRAILER),
        "{seasonal:?} is not at g_penAdvance + 0x70 after \"@{wages}\""
    );
}

/// **Every `&g_fontHeading` line of `UnitPanel_Draw` (`0x0041B19D`) is in the
/// heading face, where the call site puts it, inside the panel's own box — and
/// is not in the body face.**
///
/// We drew three of them in `Fntl2_14.pl8` through `Pen::eng` (the merchant's
/// and peasants' 31/0 and 31/5, and the transport's 31/2 at the others' place)
/// and five not at all: the transport's county, an army's name, and the
/// mercenary line's three pieces.
///
/// ```c
/// Ui_DrawBox(8, R * 0x10 + 0x20, 0x1c, 0x1b - R);
/// /* transport */  Eng_DrawString(0x1f, 2, 0x18, R * 0x10 + 0x30, &g_fontHeading, 0x3f);
///                  Eng_DrawString(100, unit[+0x167] + scen * 0x14, g_penAdvance + 0x18, …);
/// /* merchant, peasants */ Eng_DrawString(0x1f, local_20, 0x28, R * 0x10 + 0x40, …);
/// /* army */       Eng_DrawString(owner + 0x5d, unit.nameIndex, 0x28, R * 0x10 + 0x30, …);
/// /* own army */   Eng_DrawString(0x10, 0, 0x38, R * 0x10 + 0x130, …);         /* no band */
///                  Ui_DrawNumber(mercMen, '@', &DAT_004d422c, 0x38, R * 0x10 + 0x130, …);
///                  Eng_DrawString(0x10, mercBand, g_penAdvance + 0x38, …);
///                  Ui_DrawUnitNoun(mercMen, mercTroop * 2 + 0x34, g_penAdvance + 0x38, …);
/// ```
///
/// Every `R` and every position below is a literal out of those lines and
/// `FUN_0041BEFE`'s ladder; the only computed parts are the widths of strings
/// measured in the face the call names.
///
/// Ablated, each red here on its own:
/// * the merchant's heading back to `Face::Body` — *"Merchant."* is not in
///   `Fntl2_22.pl8` at (40, 304);
/// * the army-name draw deleted — *"The Foxes."* is not at (40, 80);
/// * `TRANSPORT_HEADING_AT` → `(0x28, 0x30)` — *"Supplies for"* found at 40, not 24;
/// * the mercenary count's `Face::Heading` → `Face::Body` — `"40"` is not at (60, 336).
#[test]
fn every_unit_panel_heading_is_in_the_heading_face_inside_its_box() {
    use l2_game::screens::info::{InfoScreen, Target};
    use l2_kingdom::unit::{Mercenaries, TroopType, Unit, UnitKind};
    let (mut game, assets) = world!();
    let body = assets.shell.body.as_ref().expect("Fntl2_14.pl8");
    let heading = assets.shell.heading.as_ref().expect("Fntl2_22.pl8");
    let county = own_county(&game);
    let player = game.player;
    let enemy = if player == 1 { 2 } else { 1 };
    let slot = game.map_slot;
    let text = |g: usize, i: usize| assets.shell.text(g, i).to_string();

    let unit = |kind: UnitKind, owner: u8| {
        let mut u = Unit::new(kind, owner, 10, 10);
        u.men = 100;
        u.county = county;
        u.home_county = county;
        u.owner_is_human = owner == player;
        u
    };
    let mut transport = unit(UnitKind::Transport, player);
    transport.cargo_county = county;
    let mut named = unit(UnitKind::Army, player);
    named.name_index = 3;
    let mut hired = unit(UnitKind::Army, player);
    hired.name_index = 4;
    hired.mercenaries = Some(Mercenaries { band: 3, troop: TroopType::Archer, men: 40 });
    let mut foe = unit(UnitKind::Army, enemy);
    foe.name_index = 7;

    let cases: Vec<(&str, Unit, i32)> = vec![
        ("merchant", unit(UnitKind::Merchant, 0), 0x0F),
        ("peasants", unit(UnitKind::PeasantMob, 0), 0x0F),
        ("transport", transport, 0x0F),
        ("own army, no band", named, 2),
        ("own army, a band", hired, 2),
        ("enemy army", foe, 0x12),
    ];
    for (label, u, row) in cases {
        let kind = u.kind;
        let id = game.kingdom.campaign.units.spawn(u).expect("a free slot");
        let mut panel = InfoScreen::new(Target::Unit(id));
        let canvas = draw(&mut panel, &mut game, &assets);

        let lines: Vec<(String, i32, i32)> = match (label, kind) {
            (_, UnitKind::Merchant) => vec![(text(0x1F, 0), 0x28, row * 16 + 0x40)],
            (_, UnitKind::PeasantMob) => vec![(text(0x1F, 5), 0x28, row * 16 + 0x40)],
            (_, UnitKind::Transport) => {
                let y = row * 16 + 0x30;
                let t = text(0x1F, 2);
                let name_x = 0x18 + heading.width(&t) + TRAILER;
                vec![(t, 0x18, y), (text(100, slot * 20 + county as usize), name_x, y)]
            }
            ("own army, no band", _) => vec![
                (text(0x5D + player as usize, 3), 0x28, row * 16 + 0x30),
                (text(0x10, 0), 0x38, row * 16 + 0x130),
            ],
            ("own army, a band", _) => {
                let y = row * 16 + 0x130;
                let band = text(0x10, 3);
                let band_x = 0x38 + LEAD + heading.width("40") + TRAILER;
                // Archer is troop 5, and 40 is plural: 0x34 + 5 * 2 + 1.
                let noun_x = band_x + heading.width(&band) + TRAILER;
                vec![
                    (text(0x5D + player as usize, 4), 0x28, row * 16 + 0x30),
                    ("40".to_string(), 0x38 + LEAD, y),
                    (band, band_x, y),
                    (text(8, 0x34 + 5 * 2 + 1), noun_x, y),
                ]
            }
            _ => vec![(text(0x5D + enemy as usize, 7), 0x28, row * 16 + 0x30)],
        };

        // `Ui_DrawBox(8, R * 0x10 + 0x20, 0x1C, 0x1B - R)`.
        let (left, top) = (8, row * 16 + 0x20);
        let (right, bottom) = (left + 0x1C * 16, top + (0x1B - row) * 16);
        for (s, x, y) in &lines {
            assert!(!s.is_empty(), "{label}: an L2.eng line the painter draws is empty");
            assert_eq!(
                find_on_row(&canvas, heading, s, font::TEXT, *y),
                Some(*x),
                "{label}: {s:?} is not in Fntl2_22.pl8 at ({x}, {y})",
            );
            assert!(
                *x >= left && x + heading.width(s) <= right && *y >= top && y + heading.height(s) <= bottom,
                "{label}: {s:?} at ({x}, {y}) is outside the panel's box ({left}, {top})-({right}, {bottom})",
            );
            // On the call site's own row: the troop grid draws the same group 8
            // nouns in body further up, which is right and is not this line.
            if !s.chars().all(|c| c.is_ascii_digit()) {
                assert_eq!(
                    find_on_row(&canvas, body, s, font::TEXT, *y),
                    None,
                    "{label}: {s:?} is on row {y} in the body face",
                );
            }
        }
        if label == "enemy army" {
            // The mercenary line is inside the ownership gate.
            let none = text(0x10, 0);
            assert_eq!(find_in(&canvas, heading, &none, font::TEXT), None, "{label}: {none:?}");
        }
    }
}

/// **The unit panel's *"Formed"* line is `Ui_DrawYear(…, style 0)`, which ends
/// in `L2.eng` 26/1 *"AD"* — and we drew the bare number.**
///
/// ```c
/// /* UnitPanel_Draw */
/// g_penAdvance = 0;
/// Eng_DrawString(0x1F, 0x14, 0x28, row * 0x10 + 0xA0, &g_fontBody, 0x3F);
/// Ui_DrawYear(unit.yearFormed, g_penAdvance + 0x28, row * 0x10 + 0xA0, 0);
/// /* Ui_DrawYear, 0x0041A900, style 0, year >= 0: */
/// Ui_DrawNumber(year, ' ', &DAT_004D41D8, x, y, &g_fontBody, 0x3F);
/// Eng_DrawString(0x1A, 1, x + g_penAdvance, y, &g_fontBody, 0x3F);
/// ```
///
/// `DAT_004D41D8` is one space, and the row is 2 for an army of the local
/// player's (`FUN_0041BEFE`). `CLAUDE.md` rule 6.
///
/// Ablated: the call site's style `0` → `3`, the bare number the panel used to
/// draw — *"AD"* is not found on the row (`None` where `Some(159)` is expected).
#[test]
fn the_unit_panel_says_the_year_an_army_was_formed_in_ad() {
    use l2_game::screens::info::{InfoScreen, Target};
    use l2_kingdom::unit::{Unit, UnitKind};
    let (mut game, assets) = world!();
    let body = assets.shell.body.as_ref().expect("Fntl2_14.pl8");
    let county = own_county(&game);
    const FORMED: i32 = 1271;
    let mut u = Unit::new(UnitKind::Army, game.player, 10, 10);
    u.men = 100;
    u.county = county;
    u.home_county = county;
    u.owner_is_human = true;
    u.year_formed = FORMED as _;
    let id = game.kingdom.campaign.units.spawn(u).expect("a free slot");
    let mut panel = InfoScreen::new(Target::Unit(id));
    let canvas = draw(&mut panel, &mut game, &assets);

    const ROW: i32 = 2;
    let y = ROW * 0x10 + 0xA0;
    let formed = assets.shell.text(0x1F, 0x14).to_string();
    let ad = assets.shell.text(0x1A, 1).to_string();
    assert_eq!(ad, "AD", "L2.eng 26/1");

    let year_x = 0x28 + body.width(&formed) + TRAILER;
    let digits = FORMED.to_string();
    assert_eq!(
        find_on_row(&canvas, body, &digits, font::TEXT, y),
        Some(year_x + LEAD),
        "the year is not one space right of g_penAdvance + 0x28"
    );
    assert_eq!(
        find_on_row_from(&canvas, body, &ad, font::TEXT, y, year_x),
        Some(year_x + LEAD + body.width(&digits) + SPACE + TRAILER),
        "{ad:?} is not after \" {digits} \" on the Formed line"
    );
}

// ------------------------------------------------ the far zoom's box of words

/// **The box at the far zoom was empty, and the original fills it.**
///
/// `Screen_DrawCampaign`'s (`0x0040F5FD`) zoom-2 arm, whole:
///
/// ```c
/// Ui_DrawBox(0, 0x19C, 0x1E, 4);
/// DAT_0058FE2C = 1;  g_penAdvance = 0;
/// Eng_DrawString(0x65, g_scenarioIndex, 0x40, 0x1A8, &g_fontHeading, 0x3F);
/// Eng_DrawString(0x22, 0,  g_penAdvance + 0x50, 0x1A8, &g_fontHeading, 0x3F);
/// Ui_DrawYear(g_year,      g_penAdvance + 0x60, 0x1A8, 1);
/// DAT_0058FE2C = 0;
/// Eng_DrawString(0x22, 1, 0x50, 0x1C6, &g_fontBody, 0x3F);
/// ```
///
/// We drew the box and, inside it, a status line of our own — which C173 then
/// gated behind the debug overlay, leaving the box blank. Group 34 has exactly
/// one consumer in the binary and it is this arm, so its two strings *are* this
/// box's vocabulary (`CLAUDE.md` rule 6), and the second of them is the game
/// saying what the far zoom is for.
///
/// **Ablated, one draw at a time:** removing any of the four turns exactly one
/// of the assertions below red.
#[test]
fn the_far_zoom_box_carries_the_map_name_the_year_and_the_instruction() {
    let (mut game, assets) = world!();
    let heading = assets.shell.heading.as_ref().expect("Fntl2_22.pl8 is in the install");
    let body = assets.shell.body.as_ref().expect("Fntl2_14.pl8 is in the install");
    let mut screen = MapScreen::new();
    // `Map_ToggleZoom` — the box exists only at zoom 2.
    {
        let mut ctx = Ctx { game: &mut game, assets: &assets };
        screen.handle(Event::KeyDown(Key::Char('Z')), &mut ctx);
    }
    let canvas = draw(&mut screen, &mut game, &assets);

    // The three heading draws run with `DAT_0058FE2C` set: capitals come out in
    // colour 1 whatever the caller passed,
    // mode as well as the face.
    let caps = Style { colour: font::TEXT, shadow: Some(font::SHADOW), caps: Some(1) };

    let name = assets.shell.text(101, game.map_slot).to_string();
    assert!(!name.is_empty(), "L2.eng group 101 has the sixty map names");
    let (nx, ny) = find_styled(&canvas, heading, &name, &caps)
        .unwrap_or_else(|| panic!("{name:?} is not in the far-zoom box"));
    assert_eq!((nx, ny), (0x40, 0x1A8), "Eng_DrawString(0x65, g_scenarioIndex, 0x40, 0x1A8)");

    let label = assets.shell.text(34, 0).to_string();
    assert_eq!(label, "Year", "L2.eng 34/0");
    let (lx, ly) = find_styled(&canvas, heading, &label, &caps).expect("{label:?} is not drawn");
    assert_eq!(ly, 0x1A8, "the label shares the map name's row");
    assert!(lx > nx + heading.width(&name), "the label is at {lx}, not after the name");

    // `Ui_DrawYear(…, 1)` puts group 26's era *before* the digits, both in the
    // heading face, and lifts an AD year by one pixel — `year < 0 ? y : y - 1`.
    // Finding the digits is the claim that the year was drawn.
    let digits = format!(" {} ", game.kingdom.year);
    let (yx, yy) = find_styled(&canvas, heading, &digits, &caps).expect("the year is not drawn");
    assert_eq!(yy, 0x1A8 - 1, "an AD year sits one pixel above its row");
    assert!(yx > lx, "the year is at {yx} and the label at {lx}");

    // And the instruction, in the body face with the drop capitals off again.
    let advice = assets.shell.text(34, 1).to_string();
    assert!(!advice.is_empty(), "L2.eng 34/1");
    let flat = Style { colour: font::TEXT, shadow: Some(font::SHADOW), caps: None };
    assert_eq!(
        find_styled(&canvas, body, &advice, &flat),
        Some((0x50, 0x1C6)),
        "Eng_DrawString(0x22, 1, 0x50, 0x1C6, &g_fontBody, 0x3F): {advice:?}"
    );
}

/// **`Screen_BattleMasterRatings` (`0x00421707`) draws each score in
/// `&g_fontHeading`** — the one heading-face figure on the page — and we drew it
/// in body. The lead and suffix, `' '` and one space, were already right.
///
/// ```c
/// Ui_DrawText(&g_playerNames[p], 0xD8, 0x6E, &g_fontBody, 0x3F);
/// Eng_DrawString(0x25, 4, g_penAdvance + 0xD8, 0x6E, &g_fontBody, 0x3F);
/// Ui_DrawNumber(score, ' ', &DAT_004D43B4, g_penAdvance + 0xEC, 0x69, &g_fontHeading, 0x3F);
/// ```
///
/// The name is `g_playerNames[realm]`, which this test writes so the pen
/// advance in front of the score is a known width; it is measured, not assumed.
///
/// Ablated: `Face::Heading` → `Face::Body` at the call site — the score is not
/// found on row `0x69` in the heading face (`None` where `Some(422)` is expected).
#[test]
fn the_battle_master_score_is_in_the_heading_face() {
    use l2_game::screens::ratings::{self, Ratings};
    let (mut game, assets) = world!();
    let body = assets.shell.body.as_ref().expect("Fntl2_14.pl8");
    let heading = assets.shell.heading.as_ref().expect("Fntl2_22.pl8");
    // The block is keyed by `g_localPlayer`'s realm and draws his name, so the
    // advance in front of the score is that name's width.
    game.player_names[Ratings::default().realms.0 as usize] =
        l2_game::text::PlayerName::new("PLAYER 1");
    let mut screen = ratings::RatingsScreen::new();
    let canvas = draw(&mut screen, &mut game, &assets);

    const NAME_X: i32 = 0xD8;
    const SCORE_X: i32 = 0xEC;
    const SCORE_Y: i32 = 0x69;
    let (mine, _) = ratings::score(&Ratings::default());
    let scored = assets.shell.text(0x25, 4).to_string();
    let advance = body.width("PLAYER 1") + TRAILER + body.width(&scored) + TRAILER;
    assert_eq!(
        find_on_row_from(&canvas, heading, &mine.to_string(), font::TEXT, SCORE_Y, NAME_X),
        Some(SCORE_X + advance + LEAD),
        "the score is not in the heading face at g_penAdvance + 0xEC, 0x69"
    );
}

/// **The fourth thing *Army foraging* gates is the unit panel's own lines**, and
/// ours drew none of them — not even the army's body line.
///
/// `UnitPanel_Draw` (`0x0041B19D`), the `kind == 1` arm: with `g_optArmiesEat`
/// off the body is one line at `row * 0x10 + 0x70`; with it on the body moves to
/// `+0x5E` and the supply state (31/23…26) and the starvation band
/// (31/27 + `+0x155`, red once the counter leaves zero) follow at `+0x72` and
/// `+0x86`. `docs/armies.md` §3.4 tabulates the supply strings.
///
/// Ablated: dropping the `armies_eat` branch leaves the body at `+0x70` with the
/// option on — the supply line is `None` at `+0x72`.
#[test]
fn army_foraging_moves_the_unit_panels_body_line_and_adds_two() {
    use l2_game::screens::info::{InfoScreen, Target};
    use l2_kingdom::unit::{Unit, UnitKind};
    let (mut game, assets) = world!();
    let body = assets.shell.body.as_ref().expect("Fntl2_14.pl8");
    let county = own_county(&game);
    let mut u = Unit::new(UnitKind::Army, game.player, 10, 10);
    u.men = 100;
    u.county = county;
    u.home_county = county;
    u.owner_is_human = true;
    u.starvation = 2;
    let id = game.kingdom.campaign.units.spawn(u).expect("a free slot");

    // `FUN_0041BEFE` puts a local player's army on row 2.
    const ROW: i32 = 2;
    let line = |i: usize| assets.shell.text(0x1F, i).to_string();
    // 31/16 is an own army's body, 31/23 "Fed in your county.", 31/29 the
    // third starvation band.
    let (text, fed, starving) = (line(0x10), line(0x17), line(0x1B + 2));
    assert!(!fed.is_empty() && !starving.is_empty(), "L2.eng 31/23 and 31/29");

    game.kingdom.options.armies_eat = false;
    let canvas = draw(&mut InfoScreen::new(Target::Unit(id)), &mut game, &assets);
    assert_eq!(
        find_on_row(&canvas, body, &text, font::TEXT, ROW * 0x10 + 0x70),
        Some(0x68),
        "with foraging off the army's body line is not at row * 0x10 + 0x70"
    );
    assert_eq!(
        find_on_row(&canvas, body, &fed, font::TEXT, ROW * 0x10 + 0x72),
        None,
        "the supply line is drawn with foraging off"
    );

    game.kingdom.options.armies_eat = true;
    let canvas = draw(&mut InfoScreen::new(Target::Unit(id)), &mut game, &assets);
    assert_eq!(
        find_on_row(&canvas, body, &text, font::TEXT, ROW * 0x10 + 0x5E),
        Some(0x68),
        "with foraging on the body line has not moved up to row * 0x10 + 0x5E"
    );
    assert_eq!(
        find_on_row(&canvas, body, &fed, font::TEXT, ROW * 0x10 + 0x72),
        Some(0x68),
        "{fed:?} is not the supply line at row * 0x10 + 0x72"
    );
    assert_eq!(
        find_on_row(&canvas, body, &starving, font::TEXT, ROW * 0x10 + 0x86),
        None,
        "the starvation line is drawn at 0x3F with the counter at 2"
    );
    assert_eq!(
        find_on_row(&canvas, body, &starving, 0xF9, ROW * 0x10 + 0x86),
        Some(0x68),
        "{starving:?} is not the starvation line in 0xF9 at row * 0x10 + 0x86"
    );
}

#[test]
#[ignore]
fn shoot() {
    let (mut game, assets) = world!();

    let mut screen = MapScreen::new();
    let canvas = draw(&mut screen, &mut game, &assets);
    save(&canvas, &assets.palette, "menubar");

    // The three drop-downs, over the map, for the row-pitch measurement.
    for menu in 0..3usize {
        let mut m = l2_game::screen::Machine::new(l2_game::screen::ScreenId::Campaign);
        m.push(l2_game::screen::ScreenId::MenuBar(menu));
        let mut canvas = Canvas::screen();
        let ctx = Ctx {
            game: &mut game,
            assets: &assets,
        };
        m.draw(&ctx, &mut canvas);
        save(&canvas, &assets.palette, &format!("dropdown_{menu}"));
    }

    for page in [SetupPage::Title, SetupPage::Options] {
        let mut screen = SetupScreen::new(page);
        let canvas = draw(&mut screen, &mut game, &assets);
        let p = assets
            .shell
            .palette("Gateway.256")
            .unwrap_or(&assets.palette);
        save(&canvas, p, &format!("setup_{}", page.number()));
    }
}

fn save(canvas: &Canvas, palette: &l2_formats::Palette, name: &str) {
    let mut rgba = vec![0u8; 640 * 480 * 4];
    canvas.to_rgba(palette, &mut rgba);
    let rgb: Vec<u8> = rgba
        .chunks_exact(4)
        .flat_map(|p| [p[0], p[1], p[2]])
        .collect();
    let dir = std::env::var("L2_SHOT_DIR").unwrap_or_else(|_| "out".to_string());
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(format!("{dir}/{name}.png"), png::encode(640, 480, &rgb)).unwrap();
}


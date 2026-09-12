//! **The standings** — `Screen_GreatestNoble` (`0x0041593B`), screen `0x20`,
//! and the court button that opens it.
//!
//! A player reported the *Greatest nobles* button in the treasury view as
//! dead. It was: the button was drawn, `court.rs` answered it with
//! `Transition::Stay`, and screen `0x20` did not exist. So this file has two
//! halves and the first one is the smaller:
//!
//! * **the button answers** — a kind-5 press, the handler twenty ticks later,
//!   the recount `FUN_00435211` runs before the page is drawn, and the page on
//!   the stack;
//! * **the page is right** — its geometry against the player's own
//!   `Lords2.exe` and `Flags.pl8`, and the seven scoring rules of
//!   `FUN_00415E42` and the ranking of `FUN_00415BDC` against values built by
//!   hand.
//!
//! The rules are tested by hand-built realms rather than from a fixture on
//! purpose: `docs/decisions.md` C26 — every fixture is turn one with one
//! county each, so **every category is a five-way tie** and the only line the
//! page would ever show from one is *"undecided."*. A test driven by the
//! fixture alone would assert the page's least interesting state and call the
//! screen covered.

use l2_game::game::Assets;
use l2_game::input::Event;
use l2_game::screen::{Ctx, Machine, ScreenId};
use l2_game::screens::nobles::{self, NoblesScreen};
use l2_game::Game;
use l2_kingdom::realm::Realm;
use l2_kingdom::tables::SCORE_INPUT_CASTLES;

// ---------------------------------------------------------------- the rules

/// Five realms in play, every field zero, so a test can set the one it means.
fn five_realms() -> Vec<Realm> {
    let mut realms = vec![Realm::new(); l2_kingdom::MAX_REALMS];
    for (i, r) in realms.iter_mut().enumerate() {
        r.in_play = i != 0;
        r.strength = if i == 0 { 0 } else { 1 };
        r.shield_index = i as u8;
    }
    realms
}

/// **`FUN_00415E42`, category by category**, and it is undocumented anywhere
/// else: the seven rules were read out of the decompilation for this branch.
///
/// Every category takes a different realm field, so one realm carrying a
/// distinct number in each is enough to prove none of the seven is reading its
/// neighbour's.
#[test]
fn each_category_reads_its_own_realm_field() {
    let mut realms = five_realms();
    let r = &mut realms[1];
    r.county_count = 7;
    r.score_inputs[SCORE_INPUT_CASTLES] = 3;
    r.total_men = 900;
    r.gold = 4_000;
    r.mean_happiness = 55;
    r.population_total = 12_345;
    r.rank = 2;

    let at = |c: usize| nobles::value(&realms[1], c, 1300);
    assert_eq!(at(nobles::COUNTIES), 7, "+0x29");
    assert_eq!(at(nobles::CASTLES), 3, "+0x4C");
    assert_eq!(at(nobles::TROOPS), 900, "+0x54");
    assert_eq!(at(nobles::CROWNS), 4_000, "+0x118");
    assert_eq!(at(nobles::HAPPINESS), 55, "+0x0C");
    assert_eq!(at(nobles::PEOPLE), 12_345, "+0x10");
    // `6 - rank`, so the top of the table scores 5 and the bottom 1.
    assert_eq!(at(nobles::GREATEST_NOBLE), 4, "6 - rank");
}

/// **A realm out of play scores zero in every category**, which is the
/// function's first statement and the reason a dead realm's pole is not drawn.
#[test]
fn a_realm_out_of_play_scores_nothing_anywhere() {
    let mut realms = five_realms();
    realms[1].in_play = false;
    realms[1].gold = 9_999;
    realms[1].county_count = 12;
    for c in 0..nobles::CATEGORIES {
        assert_eq!(nobles::value(&realms[1], c, 1300), 0, "category {c}");
    }
}

/// **The castle count is read as a byte** — `(uint)(byte)field_0x4c`. A 256th
/// castle reads as none.
///
/// Ablation, run: drop the `as u8` in [`nobles::value`] and this returns 256.
#[test]
fn the_castle_count_is_truncated_to_a_byte_as_the_original_truncates_it() {
    let mut realms = five_realms();
    realms[1].score_inputs[SCORE_INPUT_CASTLES] = 256;
    assert_eq!(nobles::value(&realms[1], nobles::CASTLES, 1300), 0);
    realms[1].score_inputs[SCORE_INPUT_CASTLES] = 257;
    assert_eq!(nobles::value(&realms[1], nobles::CASTLES, 1300), 1);
}

/// **The overall standing is dead until 1270** — `if (g_year < 0x4F6) return
/// 2;`, for every realm, so the page reads *"Greatest noble, undecided."* for
/// the first two years of every game whatever the ranks say.
#[test]
fn the_greatest_noble_is_undecided_until_1270() {
    let mut realms = five_realms();
    for (i, r) in realms.iter_mut().enumerate().skip(1) {
        r.rank = i as u8;
    }

    for year in [1268, 1269] {
        for r in 1..l2_kingdom::MAX_REALMS {
            assert_eq!(
                nobles::value(&realms[r], nobles::GREATEST_NOBLE, year),
                nobles::GREATEST_NOBLE_EARLY,
                "year {year}, realm {r}",
            );
        }
        let s = nobles::rank(&realms, nobles::GREATEST_NOBLE, year);
        assert!(s.all_level, "year {year}: every realm is level, so nothing leads");
        assert!(!s.decided(), "year {year}: the line reads `undecided.`");
    }

    // And the year it comes alive, with ranks that are not all equal.
    let s = nobles::rank(&realms, nobles::GREATEST_NOBLE, 1270);
    assert!(!s.all_level, "1270: the ranks differ, so the bars differ");
    assert!(s.decided(), "1270: rank 1 leads outright");
    assert_eq!(s.leader, 1, "realm 1 holds rank 1");
}

/// **The bars are a percentage of the leader**, clamped 0…100 — not of a
/// maximum and not of the sum. The leader's pole is always full height.
#[test]
fn the_bars_are_each_realms_share_of_the_leaders_score() {
    let mut realms = five_realms();
    realms[1].gold = 1_000;
    realms[2].gold = 500;
    realms[3].gold = 250;
    realms[4].gold = 0;
    realms[5].gold = 1;

    let s = nobles::rank(&realms, nobles::CROWNS, 1300);
    assert_eq!(s.leader, 1);
    assert!(s.decided(), "one realm is ahead on its own");
    assert_eq!(s.pct[1], 100);
    assert_eq!(s.pct[2], 50);
    assert_eq!(s.pct[3], 25);
    assert_eq!(s.pct[4], 0);
    // `PctOf(1, 1000)` is `1 * 100 / 1000`, which is integer zero — a realm
    // with a crown in the bank is drawn exactly as one with nothing.
    assert_eq!(s.pct[5], 0, "integer division, and the original's too");
}

/// **A tie for the lead is won by the highest realm index** — the loop is
/// `if (best <= v)`, not `<`.
///
/// It never shows on screen, because the same function then calls the category
/// tied and the line prints *"undecided."* instead of a name. It is reproduced
/// because [`nobles::Standings::leader`] is what *would* be printed, and a
/// leader chosen the other way would be a different name the day either flag
/// stops being set.
#[test]
fn a_tie_for_the_lead_goes_to_the_highest_realm_index() {
    let mut realms = five_realms();
    realms[1].gold = 100;
    realms[4].gold = 100;
    realms[2].gold = 10;

    let s = nobles::rank(&realms, nobles::CROWNS, 1300);
    assert_eq!(s.leader, 4, "`best <= v` keeps the later realm");
    assert!(s.tied_at_top, "and the page says so rather than naming it");
    assert!(!s.all_level, "realm 2 is behind, so this is not the level case");
    assert!(!s.decided());
}

/// **Level is not the same as tied**, and the original draws them differently:
/// a level category sets every in-play bar to **50**, a tie at the top leaves
/// the real percentages standing. Both print *"undecided."*.
#[test]
fn a_level_category_draws_every_bar_at_fifty() {
    let mut realms = five_realms();
    for r in realms.iter_mut().skip(1) {
        r.county_count = 3;
    }

    let s = nobles::rank(&realms, nobles::COUNTIES, 1300);
    assert!(s.all_level);
    assert!(!s.tied_at_top, "the tie test is only reached when the category is NOT level");
    for r in 1..l2_kingdom::MAX_REALMS {
        assert_eq!(s.pct[r], nobles::LEVEL_BAR_PCT, "realm {r}");
    }

    // The ablation the arrangement is about: make one realm differ and the
    // bars stop being 50 even though two of the others still match.
    realms[5].county_count = 4;
    let s = nobles::rank(&realms, nobles::COUNTIES, 1300);
    assert!(!s.all_level);
    assert_eq!(s.pct[5], 100);
    assert_eq!(s.pct[1], 75);
    assert!(!s.tied_at_top, "nobody else matches the leader");
    assert!(s.decided(), "so the page names realm 5");
}

/// **A realm out of play does not make a category level**, and does not tie
/// it: both loops skip it. Four realms all on 3 counties with a dead fifth is
/// still level.
#[test]
fn a_dead_realm_is_not_counted_level_or_tied() {
    let mut realms = five_realms();
    for r in realms.iter_mut().skip(1) {
        r.county_count = 3;
    }
    realms[5].in_play = false;
    realms[5].county_count = 0;

    let s = nobles::rank(&realms, nobles::COUNTIES, 1300);
    assert!(s.all_level, "the four live realms agree; the dead one is not asked");
}

// ------------------------------------------------------- the geometry, gated

/// **The seven tabs, the three lookup tables and the banner sheet, out of the
/// player's own copy of the game.**
///
/// `g_nobleTabs` (`0x004DC890`) is seven 24-byte `Hotspot_Test` records; the
/// three tables at `0x004D2B20`, `0x004D2B40` and `0x004D2B58` are the tab
/// marker's x, the column x and **which realm stands in which column**, which
/// is not the realm order.
///
/// Ablation, run: change one entry of [`nobles::COLUMN_REALM`] and the third
/// assertion fails naming the slot.
#[test]
fn the_pages_geometry_is_the_exes_own_tables() {
    let exe = l2_testkit::executable!();

    // `Hotspot_Test` records: `{x0, y0, x1, y1}` as four `i16`, the handler at
    // `+0x08`, the kind at `+0x0F` and `g_uiHotspotId` at `+0x10`.
    let t = l2_testkit::pe::Table::at(&exe, 0x004D_C890);
    for i in 0..nobles::CATEGORIES {
        let s = |k: usize| t.u16_at(i * 12 + k) as i16 as i32;
        let (x0, y0, x1, y1) = (s(0), s(1), s(2), s(3));
        let rect = nobles::TABS[i];
        assert_eq!((rect.x, rect.y), (x0, y0), "tab {i} origin");
        assert_eq!((rect.w, rect.h), (x1 - x0, y1 - y0), "tab {i} size");
        let handler = (0..4).fold(0u32, |a, k| a | (t.u8_at(i * 24 + 8 + k) as u32) << (8 * k));
        assert_eq!(handler, 0x0043_524E, "tab {i} is FUN_0043524E");
        assert_eq!(t.u8_at(i * 24 + 0x0F), 1, "tab {i} is Hotspot_Test kind 1");
        assert_eq!(t.u8_at(i * 24 + 0x10) as usize, i, "tab {i} publishes id {i}");
    }

    // `g_nobleTabX` — the marker's x per category, eight pixels inside the tab.
    let t = l2_testkit::pe::Table::at(&exe, 0x004D_2B20);
    for i in 0..nobles::CATEGORIES {
        assert_eq!(t.i32_at(i), nobles::TAB_X[i], "marker x {i}");
        assert_eq!(nobles::TAB_X[i] - nobles::TABS[i].x, 8, "marker {i} sits 8 inside its tab");
    }

    // `g_nobleColumnX` — entries **1…5**; entry 0 is a zero the loop never
    // reads, which is why this is offset by one.
    let t = l2_testkit::pe::Table::at(&exe, 0x004D_2B40);
    for (slot, &x) in nobles::COLUMN_X.iter().enumerate() {
        assert_eq!(t.i32_at(slot + 1), x, "column x, slot {}", slot + 1);
    }

    // `g_nobleColumnRealm` — and the point of asserting it is that it is not
    // 1, 2, 3, 4, 5: the original seats realm 1 in the middle.
    let t = l2_testkit::pe::Table::at(&exe, 0x004D_2B58);
    for (slot, &realm) in nobles::COLUMN_REALM.iter().enumerate() {
        assert_eq!(t.i32_at(slot + 1) as u8, realm, "column realm, slot {}", slot + 1);
    }
    assert_ne!(
        nobles::COLUMN_REALM,
        [1, 2, 3, 4, 5],
        "the seating order is the table's, not the realm order",
    );
}

/// **`Flags.pl8` has six frames and the sixth is not a sixth realm.**
///
/// `docs/draws.md` recorded frame 5 as *"for the leader"*. It is 23 × 60 where
/// the five banners are 51 × 92, and the painter indexes it by the category.
/// The dimensions are the evidence that it is a different thing: a sixth
/// banner would be 51 × 92 too.
#[test]
fn the_banner_sheet_holds_five_banners_and_one_marker() {
    let dir = l2_testkit::install!();
    let bytes = match std::fs::read(dir.join("Flags.pl8")) {
        Ok(b) => b,
        Err(e) => l2_testkit::skip!("Flags.pl8: {e}"),
    };
    let pl8 = l2_formats::Pl8::parse(&bytes).expect("Flags.pl8 parses");
    assert_eq!(pl8.frames.len(), 6, "five banners and one marker");
    for (i, f) in pl8.frames.iter().take(5).enumerate() {
        assert_eq!((f.width, f.height), (51, 92), "banner {i}");
    }
    let marker = &pl8.frames[nobles::MARKER_FRAME];
    assert_eq!((marker.width, marker.height), (23, 60), "the tab marker is its own size");
    // The banner's height is what puts the pole's top where it is: the flag is
    // drawn at `top + 0x2D` and the pole starts at `top + 0x89`.
    assert_eq!(
        nobles::POLE_TOP - nobles::FLAG_TOP,
        pl8.frames[0].height as i32,
        "the pole begins exactly at the banner's bottom edge",
    );
}

/// **`L2.eng` group 35 is this screen's whole vocabulary** — `CLAUDE.md` rule
/// 6, and this painter is its only consumer in the binary.
///
/// Seven categories and one *"undecided."*, and the test asserts the words
/// rather than the count so that a group that shifted would be caught.
#[test]
fn the_page_draws_its_words_out_of_group_35() {
    let dir = l2_testkit::install!();
    let bytes = match std::fs::read(dir.join("L2.eng")) {
        Ok(b) => b,
        Err(e) => l2_testkit::skip!("L2.eng: {e}"),
    };
    let eng = l2_game::shell::Eng::parse(bytes).expect("L2.eng parses");
    let want = [
        "Most counties,",
        "Most castles,",
        "Most troops,",
        "Most crowns,",
        "Happiest people,",
        "Most people,",
        "Greatest noble,",
        "undecided.",
    ];
    assert_eq!(eng.group(nobles::GROUP).len(), want.len(), "group 35 is eight strings");
    for (i, w) in want.iter().enumerate() {
        assert_eq!(eng.text(nobles::GROUP, i), *w, "35/{i}");
    }
    assert_eq!(nobles::UNDECIDED, want.len() - 1, "the last one is the refusal");
}

// ------------------------------------------------------------- the behaviour

fn bare_world() -> (Game, Assets) {
    let mut game = Game::new(5);
    game.prefs.tip_screens = false;
    (game, Assets::placeholder())
}

/// **The court's button opens the page** — `FUN_004351C4`, and it is kind 5,
/// so the press does not do it: the press starts the twenty-frame timer and
/// `Screen::update` runs the handler.
///
/// Ablation, run: answer the click with `Transition::Push` directly and the
/// first assertion fails, because the page would already be up.
#[test]
fn the_greatest_nobles_button_opens_the_standings_twenty_ticks_later() {
    let (mut game, assets) = bare_world();
    let mut m = Machine::new(ScreenId::Campaign);
    m.push(ScreenId::Court);

    let b = l2_game::screens::court::NOBLES_BUTTON;
    {
        let mut c = Ctx { game: &mut game, assets: &assets };
        m.handle(Event::Click { x: b.x + 4, y: b.y + 4 }, &mut c);
    }
    assert_eq!(m.top_id(), Some(ScreenId::Court), "the press only starts the timer");

    let mut opened_at = None;
    for tick in 1..=64u32 {
        let mut c = Ctx { game: &mut game, assets: &assets };
        m.update(&mut c);
        if m.top_id() == Some(ScreenId::Nobles) {
            opened_at = Some(tick);
            break;
        }
    }
    assert_eq!(
        opened_at,
        Some(l2_game::screens::court::DEFERRED_FRAMES as u32),
        "the handler runs on the twentieth frame, as `Widget_Test` kind 5 does",
    );
    assert_eq!(m.ids(), vec![ScreenId::Campaign, ScreenId::Court, ScreenId::Nobles]);
}

/// **The button runs the recount before the page is drawn** —
/// `FUN_00435211`, `Score_RankRealms()` then `Realm_UpdateTotals(r)` for
/// r in 1..=5.
///
/// This is the statement that would be easiest to drop and hardest to notice:
/// without it the page shows whatever the last AI turn left in the realm
/// totals. The test leaves a realm with a stale zero and asserts the button
/// fills it in.
///
/// Ablation, run: remove the `recount` call from `open_the_standings` and the
/// county count stays 0.
#[test]
fn the_button_rebuilds_the_realm_totals_the_page_reads() {
    let (mut game, assets) = bare_world();
    // Two counties for realm 1 and one for realm 2, with the realm records
    // left as a fresh game leaves them: zero.
    game.kingdom.county_count = 3;
    for (id, owner) in [(1u8, 1u8), (2, 1), (3, 2)] {
        game.kingdom.counties[id as usize].owner = owner;
        game.kingdom.counties[id as usize].population = 100;
    }
    for r in 1..=2usize {
        game.kingdom.realms[r].in_play = true;
        game.kingdom.realms[r].strength = 1;
        game.kingdom.realms[r].county_count = 0;
    }

    let mut m = Machine::new(ScreenId::Campaign);
    m.push(ScreenId::Court);
    let b = l2_game::screens::court::NOBLES_BUTTON;
    {
        let mut c = Ctx { game: &mut game, assets: &assets };
        m.handle(Event::Click { x: b.x + 4, y: b.y + 4 }, &mut c);
    }
    for _ in 0..l2_game::screens::court::DEFERRED_FRAMES {
        let mut c = Ctx { game: &mut game, assets: &assets };
        m.update(&mut c);
    }
    assert_eq!(m.top_id(), Some(ScreenId::Nobles));
    assert_eq!(game.kingdom.realms[1].county_count, 2, "Realm_UpdateTotals ran");
    assert_eq!(game.kingdom.realms[2].county_count, 1);
    assert_eq!(game.kingdom.realms[1].population_total, 200);
}

/// **The seven tabs pick the category and ask for the name to be spoken** —
/// `FUN_0043524E`, which is `DAT_0055CE7C = g_uiHotspotId; g_redrawRequest =
/// 2; FUN_004B3994(g_uiHotspotId);`.
///
/// Two things are asserted that a simpler wiring would fail: the id comes from
/// **which** tab was hit rather than from a running index, and the counter
/// moves even when the tab pressed is the one already showing — because the
/// original's call is unconditional and a diff on the category would swallow
/// that press.
#[test]
fn each_tab_selects_its_own_category_and_speaks_it() {
    let (mut game, assets) = bare_world();
    let mut m = Machine::new(ScreenId::Nobles);

    for (i, tab) in nobles::TABS.iter().enumerate() {
        let before = game.nobles_spoken;
        let mut c = Ctx { game: &mut game, assets: &assets };
        m.handle(Event::Click { x: tab.centre_x(), y: tab.y + 4 }, &mut c);
        assert_eq!(game.nobles_category as usize, i, "tab {i} selects category {i}");
        assert_eq!(game.nobles_spoken, before + 1, "tab {i} asks to be spoken");
        assert_eq!(m.top_id(), Some(ScreenId::Nobles), "a tab does not leave the page");
    }

    // The same tab again: the category does not move and the voice still does.
    let tab = nobles::TABS[nobles::CATEGORIES - 1];
    let before = game.nobles_spoken;
    {
        let mut c = Ctx { game: &mut game, assets: &assets };
        m.handle(Event::Click { x: tab.centre_x(), y: tab.y + 4 }, &mut c);
    }
    assert_eq!(game.nobles_category as usize, nobles::CATEGORIES - 1);
    assert_eq!(game.nobles_spoken, before + 1, "FUN_0043524E plays unconditionally");

    // A press just below the row does nothing — the ablation that stops the
    // arm being "anything in the bottom third of the screen".
    let before = (game.nobles_category, game.nobles_spoken);
    {
        let mut c = Ctx { game: &mut game, assets: &assets };
        m.handle(Event::Click { x: tab.centre_x(), y: nobles::TAB_Y1 + 4 }, &mut c);
    }
    assert_eq!((game.nobles_category, game.nobles_spoken), before, "outside the row, nothing");
}

/// **The two ways out**, which is the whole of `Screen_FrameInput`'s `0x20`
/// ladder — `Ui_OkButtonClicked` in the corner box, and a right release
/// anywhere.
///
/// The original sets `g_screenId = 0` — the map — and ours pops onto the court
/// the button was pressed on. `docs/arms.json`
/// `0x0042FF10/standings-ok` records the difference and why it is deliberate.
#[test]
fn the_corner_picture_and_a_right_click_leave_the_page() {
    let (mut game, assets) = bare_world();

    for (what, event) in [
        ("the corner picture", Event::Click { x: nobles::OK.x + 4, y: nobles::OK.y + 4 }),
        ("a right release", Event::RightClick { x: 320, y: 240 }),
    ] {
        let mut m = Machine::new(ScreenId::Campaign);
        m.push(ScreenId::Court);
        m.push(ScreenId::Nobles);
        let mut c = Ctx { game: &mut game, assets: &assets };
        m.handle(event, &mut c);
        assert_eq!(m.top_id(), Some(ScreenId::Court), "{what} leaves the page");
    }

    // A left press anywhere that is not the corner box and not a tab keeps it
    // up: the arm consults `Ui_OkButtonClicked` and the tab table, nothing
    // else. The ratings screen next door closes on *any* press, and this one
    // must not be given that behaviour by accident.
    let mut m = Machine::new(ScreenId::Campaign);
    m.push(ScreenId::Nobles);
    let mut c = Ctx { game: &mut game, assets: &assets };
    m.handle(Event::Click { x: 320, y: 240 }, &mut c);
    assert_eq!(m.top_id(), Some(ScreenId::Nobles), "a press in the middle does nothing");
}

/// **The page does not swallow the campaign minimap**, which is
/// `Screen_FrameInput`'s epilogue and belongs to every screen but `0x12`.
#[test]
fn the_page_passes_the_minimap_down() {
    let (mut game, assets) = bare_world();
    let hit = l2_view::chrome::minimap_hit_area();
    let mut m = Machine::new(ScreenId::Campaign);
    m.push(ScreenId::Nobles);
    let mut c = Ctx { game: &mut game, assets: &assets };
    m.handle(Event::Click { x: hit.x0 + 40, y: hit.y0 + 40 }, &mut c);
    assert_eq!(m.top_id(), Some(ScreenId::Campaign), "the map is revealed");
    assert_eq!(m.depth(), 1);
}

/// **The page paints the poles at the height the percentages say**, with no
/// artwork at all — which is what makes this checkable on a machine with no
/// copy of the game.
///
/// The pole is four one-pixel columns in colours `0x10`, `0x12`, `0x14`,
/// `0x12`, from `(100 - pct) * 2 + 0x89` down to `0x158`. A leader's pole is
/// therefore 208 pixels and a realm on nothing has 8.
#[test]
fn the_poles_are_drawn_to_the_height_the_percentages_say() {
    let (mut game, assets) = bare_world();
    for r in 1..l2_kingdom::MAX_REALMS {
        game.kingdom.realms[r].in_play = true;
        game.kingdom.realms[r].strength = 1;
        game.kingdom.realms[r].shield_index = r as u8;
    }
    game.kingdom.realms[1].gold = 1_000;
    game.kingdom.realms[2].gold = 500;
    game.kingdom.realms[3].gold = 0;
    game.kingdom.realms[4].gold = 0;
    game.kingdom.realms[5].gold = 0;
    game.nobles_category = nobles::CROWNS as u8;

    let mut screen = NoblesScreen::new();
    let mut canvas = l2_view::Canvas::screen();
    {
        let ctx = Ctx { game: &mut game, assets: &assets };
        <NoblesScreen as l2_game::screen::Screen>::draw(&mut screen, &ctx, &mut canvas);
    }

    // How tall is the lit column of realm `r`'s pole?
    let height = |realm: u8| {
        let slot = nobles::COLUMN_REALM.iter().position(|&x| x == realm).expect("a column");
        let x = nobles::COLUMN_X[slot] + nobles::POLE[2].0;
        (0..480).filter(|&y| canvas.at(x as usize, y) == nobles::POLE[2].1).count() as i32
    };
    let expect = |pct: i32| nobles::POLE_BOTTOM - ((100 - pct) * nobles::PIXELS_PER_PERCENT + nobles::POLE_TOP) + 1;

    assert_eq!(height(1), expect(100), "the leader's pole is full height");
    assert_eq!(height(2), expect(50), "half the leader's crowns is half the pole");
    assert_eq!(height(3), expect(0), "nothing in the bank is still a pole");

    // And a realm out of play is not drawn at all.
    game.kingdom.realms[5].in_play = false;
    let mut canvas = l2_view::Canvas::screen();
    {
        let ctx = Ctx { game: &mut game, assets: &assets };
        <NoblesScreen as l2_game::screen::Screen>::draw(&mut screen, &ctx, &mut canvas);
    }
    let slot = nobles::COLUMN_REALM.iter().position(|&x| x == 5).expect("a column");
    let x = (nobles::COLUMN_X[slot] + nobles::POLE[2].0) as usize;
    assert_eq!(
        (0..480).filter(|&y| canvas.at(x, y) == nobles::POLE[2].1).count(),
        0,
        "a realm out of play has no pole",
    );
}

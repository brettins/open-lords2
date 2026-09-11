//! The click-through demo: the front end, the thirteen setup pages, the
//! conquest screen and the shells.
//!
//! Two kinds of test, and the split is the point.
//!
//! * **Structure**, which needs no game: every page lays out, every hotspot is
//!   where the painter puts it, and the navigation graph reaches every screen
//!   and comes back. These run on a bare checkout.
//! * **Fidelity**, which needs the install: `L2.eng` really does say what the
//!   painters' `(group, index)` pairs claim, the two fonts really do map
//!   characters the way `g_glyphWidths` says, and that table really is the
//!   ninety-six bytes at `0x004D71F0` in the user's own `Lords2.exe`. Those
//!   skip without `LORDS2_DIR`.
//!
//! The second kind is what makes the first kind mean anything. A shell that
//! draws group 11 index 0 is only worth having if group 11 index 0 is
//! *"Lords of the Realm 2"*.

use std::path::PathBuf;

use l2_game::game::Assets;
use l2_game::input::{Event, Key};
use l2_game::screen::{Ctx, Machine, Screen, ScreenId};
use l2_game::screens::county::Panel;
use l2_game::screens::conquest::ConquestScreen;
use l2_game::screens::setup::{self, SetupPage};
use l2_game::screens::court::CourtScreen;
use l2_game::screens::ratings::RatingsScreen;
use l2_game::shell::{font, Eng};
use l2_game::Game;
use l2_view::Canvas;

fn install() -> Option<PathBuf> {
    l2_testkit::install_dir()
}

fn eng() -> Option<Eng> {
    let dir = install()?;
    Eng::parse(std::fs::read(dir.join("L2.eng")).ok()?).ok()
}

/// A world and assets with no game behind them. Every structural test below
/// runs against this, which is what proves the layout does not depend on the
/// install being present.
fn bare() -> (Game, Assets) {
    (Game::new(1), Assets::placeholder())
}

// ------------------------------------------------------------- the machine

#[test]
fn the_front_end_is_the_setup_screens_first_page() {
    let (mut game, assets) = bare();
    let mut m = Machine::new(ScreenId::Setup(SetupPage::Title));
    assert_eq!(m.ids(), vec![ScreenId::Setup(SetupPage::Title)]);

    // "Single player" opens "Your options"; that is one screen changing its
    // own page, not a push, exactly as `g_setupPage` is one screen.
    let mut ctx = Ctx { game: &mut game, assets: &assets };
    m.handle(click(setup_item(0)), &mut ctx);
    assert_eq!(m.ids(), vec![ScreenId::Setup(SetupPage::Options)]);
    assert_eq!(m.depth(), 1, "a page change must not grow the stack");
}

#[test]
fn back_walks_the_page_graph_the_original_has() {
    let (mut game, assets) = bare();
    let mut m = Machine::new(ScreenId::Setup(SetupPage::Title));
    let mut ctx = Ctx { game: &mut game, assets: &assets };

    // Title -> Options -> Custom game -> back to Options -> back to Title.
    m.handle(click(setup_item(0)), &mut ctx);
    assert_eq!(m.top_id(), Some(ScreenId::Setup(SetupPage::Options)));
    m.handle(click(setup_item(3)), &mut ctx);
    assert_eq!(m.top_id(), Some(ScreenId::Setup(SetupPage::Custom)));
    // "Cancel" is the first of the custom page's bottom buttons.
    m.handle(click(custom_button(0)), &mut ctx);
    assert_eq!(m.top_id(), Some(ScreenId::Setup(SetupPage::Options)));
    m.handle(click(setup_item(4)), &mut ctx);
    assert_eq!(m.top_id(), Some(ScreenId::Setup(SetupPage::Title)));
}

#[test]
fn a_drop_down_opens_over_its_page_and_puts_the_value_back() {
    let (mut game, assets) = bare();
    let mut m = Machine::new(ScreenId::Setup(SetupPage::Custom));
    let mut ctx = Ctx { game: &mut game, assets: &assets };

    // Option 4 is Difficulty, which has four values.
    let (x, y, _) = setup::OPTION_CELLS[4];
    m.handle(click((x + 8, y + 8)), &mut ctx);
    assert_eq!(m.top_id(), Some(ScreenId::Setup(SetupPage::Dropdown)));

    // Its third value, "hard": the list starts one cell into the box.
    let (lx, ly, _) = setup::OPTION_LIST[4];
    m.handle(click((lx + 8, ly + 16 + 2 * 16 + 8)), &mut ctx);
    assert_eq!(m.top_id(), Some(ScreenId::Setup(SetupPage::Custom)));
    assert_eq!(
        m.ids().len(),
        1,
        "the drop-down is a page of the same screen, not a screen of its own"
    );
}

#[test]
fn every_screen_the_index_lists_opens_over_it_draws_and_closes_again() {
    let (mut game, assets) = bare();
    for id in every_screen() {
        let mut m = Machine::new(ScreenId::Index);
        m.push(id);
        assert_eq!(m.depth(), 2);

        let mut canvas = Canvas::screen();
        {
            let mut ctx = Ctx { game: &mut game, assets: &assets };
            m.draw(&ctx, &mut canvas);
            // Escape backs out. The setup screen is thirteen pages behind one
            // `ScreenId`, so it takes at most two presses — one to the title
            // page, one off the screen — and every other screen takes one.
            for _ in 0..3 {
                if m.top_id() == Some(ScreenId::Index) {
                    break;
                }
                m.handle(Event::KeyDown(Key::Escape), &mut ctx);
            }
        }
        assert!(canvas.count(0) < 640 * 480, "{id:?} drew nothing at all");
        assert_eq!(m.top_id(), Some(ScreenId::Index), "{id:?} would not close");
        assert!(!m.should_quit());
    }
}

/// Every id the demo can build, which is also every row of the index.
fn every_screen() -> Vec<ScreenId> {
    let mut v = vec![
        ScreenId::Campaign,
        ScreenId::County(1, Panel::Tax),
        ScreenId::Conquest,
        ScreenId::Index,
    ];
    v.extend(SetupPage::ALL.iter().map(|p| ScreenId::Setup(*p)));
    // The last seven shells, now seven screens.
    v.extend([
        ScreenId::About,
        ScreenId::Court,
        ScreenId::Diplomacy,
        ScreenId::Supplies(1),
        ScreenId::Ratings,
        ScreenId::Info(l2_game::screens::info::Target::Tile(0)),
        ScreenId::Info(l2_game::screens::info::Target::Unit(1)),
    ]);
    v
}

#[test]
fn a_popup_is_drawn_over_what_was_underneath() {
    let (mut game, assets) = bare();
    game.kingdom.set_county_count(2);

    // The court is an overlay; the battle master ratings, which load their own
    // 640 × 480 background, are not. Those are the two kinds of screen this
    // test is about.
    //
    // **This test outlived four of its own examples and then the whole
    // table.** The merchant stood here, then the armoury, then castle
    // building, then the court - every whole-picture shell the table had -
    // and the note here said that when the last one graduated this should go
    // red and be deleted deliberately rather than quietly pass over an empty
    // set. It went red. This is that deliberate rewrite.
    //
    // What it asserted was never really about shells. It is that **an overlay
    // does not clear what is underneath it** - the property
    // `docs/decisions.md` C22 was written about, and the reason
    // `Machine::draw` walks back to the last non-overlay screen. So it names
    // two graduated screens instead: the court, which paints over the map,
    // and the Battle Master ratings, which load their own 640 x 480 page.
    // The claim outlives its examples, which is what a claim is for.
    assert!(CourtScreen::new().is_overlay(), "the court paints over what opened it");
    assert!(!RatingsScreen::new().is_overlay(), "the ratings screen is a page of its own");

    let mut m = Machine::new(ScreenId::Index);
    m.push(ScreenId::Court);
    assert_eq!(m.depth(), 2);
    let mut alone = Machine::new(ScreenId::Index);
    let (mut over, mut under) = (Canvas::screen(), Canvas::screen());
    {
        let ctx = Ctx { game: &mut game, assets: &assets };
        m.draw(&ctx, &mut over);
        alone.draw(&ctx, &mut under);
    }

    assert!(over.diff_count(&under) > 0, "the popup must have drawn something");
    // The index's own title line is at y = 10 and the court's window starts at
    // y = 48, so the line above it must survive the popup.
    let untouched = (0..640).filter(|&x| over.at(x as usize, 10) == under.at(x as usize, 10)).count();
    assert_eq!(untouched, 640, "a popup must not clear what was underneath it");
}

#[test]
fn every_setup_page_and_every_shell_draws_without_the_game() {
    let (mut game, assets) = bare();
    for id in every_screen() {
        let mut screen = id.build();
        let mut canvas = Canvas::screen();
        screen.draw(&Ctx { game: &mut game, assets: &assets }, &mut canvas);
        let ctx = Ctx { game: &mut game, assets: &assets };
        assert!(!screen.title(&ctx).is_empty(), "{id:?} has no window title");
    }
}

#[test]
fn the_conquest_screen_says_all_three_of_the_things_it_can_say() {
    let (mut game, assets) = bare();
    let mut s = ConquestScreen::new();
    let mut seen = Vec::new();
    for _ in 0..3 {
        seen.push(s.outcome());
        let mut ctx = Ctx { game: &mut game, assets: &assets };
        s.handle(Event::KeyDown(Key::Enter), &mut ctx);
    }
    seen.sort_by_key(|o| format!("{o:?}"));
    seen.dedup();
    assert_eq!(seen.len(), 3, "the three branches of FUN_0041E1DD");
}

// ------------------------------------------------------- against the install

#[test]
fn l2_eng_says_what_every_screen_in_the_table_claims_it_says() {
    let Some(e) = eng() else {
        eprintln!("skipping: no game install");
        return;
    };
    // The front end. If these three are right, page 1 is the front end and
    // `docs/screens-county.md`'s old row for 0x1C was wrong.
    assert_eq!(e.get(11, 0), Some("Lords of the Realm 2"));
    assert_eq!(e.get(11, 2), Some("Single player"));
    assert_eq!(e.get(11, 4), Some("Exit game"));
    assert_eq!(e.get(11, 5), Some("Your options"));
    assert_eq!(e.get(11, 6), Some("Play Now!"));

    // The conquest screen, which is what 0x1C actually is.
    assert_eq!(e.get(36, 0), Some("Congratulations!!"));
    assert_eq!(e.get(36, 1), Some("You have conquered"));
    assert_eq!(e.get(36, 4), Some("You have lost."));

    // The custom game's twelve options and the values they index.
    assert_eq!(e.get(102, 0), Some("Advanced Farming"));
    assert_eq!(e.get(102, 11), Some("Fight?"));
    assert_eq!(e.get(103, 0), Some("off"));
    assert_eq!(e.get(103, 45), Some("all"));
    assert_eq!(e.group(103).len(), 46, "the twelve runs cover 45 of these");

    // **And every index the last seven screens draw is a string that exists.**
    //
    // This walked `SHELLS` until the table emptied. The claim it was making -
    // *a group and index this engine draws must be a group and index the
    // player's own `L2.eng` has* - is worth more than the table it walked, so
    // it now names the seven modules' own constants. Two of those constants
    // are the finding rather than the check: group 70 index 1 (*"Arms"*) and
    // group 37 index 1 (*"Before"*) exist in the file and are drawn by
    // **nothing in the binary**, which is why they are asserted present here
    // and appear in no painter.
    use l2_game::screens::{about, court, diplomacy, ratings, supplies};

    for i in [about::TITLE, about::VERSION, about::COPYRIGHT] {
        assert!(e.get(about::GROUP, i).is_some(), "about: group 59 has no {i}");
    }
    assert_eq!(e.get(about::GROUP, about::TITLE), Some("Lords 2."));
    assert_eq!(e.group(about::GROUP).len(), 3, "group 59 is three strings and no more");

    for i in [
        court::GOLD,
        court::ARMS,
        court::IRON,
        court::STONE,
        court::WOOD,
        court::COURT_OF,
        court::NOBLES,
        court::WAGES,
        court::TAX_EXPECTED,
    ] {
        assert!(e.get(court::GROUP, i).is_some(), "court: group 70 has no {i}");
    }
    assert_eq!(e.get(court::GROUP, court::COURT_OF), Some("Court of"));
    assert_eq!(e.get(court::GROUP, court::ARMS), Some("Arms"), "and nothing draws it");

    // **Every row the screen can draw, taken from the screen rather than from a
    // list beside it.** This used to name eight constants belonging to a stub;
    // the stub was replaced by the real screen in the same merge and its
    // constants went with it. Asking `Menu::rows()` means a menu that gains a
    // row is covered here without anyone remembering — the difference between a
    // test that checks the screen and one that checks a copy of what it was.
    let rows: std::collections::BTreeSet<usize> = [
        diplomacy::Menu::NoAlly,
        diplomacy::Menu::Allied,
        diplomacy::Menu::AlliedElsewhere,
        diplomacy::Menu::Dispatched,
    ]
    .iter()
    .flat_map(|m| m.rows().iter().copied())
    .collect();
    assert!(rows.len() >= 8, "only {} distinct rows across the four menus", rows.len());
    for i in rows {
        assert!(e.get(diplomacy::GROUP, i).is_some(), "diplomacy: group 72 has no {i}");
    }
    assert_eq!(e.get(diplomacy::GROUP, 0), Some("Diplomacy."), "which is the screen's name");

    for i in [
        supplies::TITLE,
        supplies::FROM,
        supplies::TO,
        supplies::GRAIN,
        supplies::SHEEP,
        supplies::CATTLE,
        supplies::DISPATCH,
        supplies::PROMPT,
    ] {
        assert!(e.get(supplies::GROUP, i).is_some(), "supplies: group 33 has no {i}");
    }
    assert_eq!(e.get(supplies::GROUP, supplies::SHEEP), Some("Sheep"), "and nothing draws it");

    for i in [ratings::HEADING, ratings::BEFORE, ratings::KILLED, ratings::KILLS, ratings::SCORED]
    {
        assert!(e.get(ratings::GROUP, i).is_some(), "ratings: group 37 has no {i}");
    }
    assert_eq!(e.get(ratings::GROUP, ratings::BEFORE), Some("Before"), "and nothing draws it");

    // **The compose dialogs' own strings, checked against the words and not
    // only for existence.** `docs/draws.md` §6: the armoury was filed under
    // group 16 and group 16 index 6 exists, so an existence check passed on a
    // screen that was wrong. Every index below is a literal argument to an
    // `Eng_DrawString` in `Diplo_DrawGiftGold` (`0x00417960`),
    // `Diplo_DrawLetter` (`0x00417AEF`) or `Diplo_DrawCountyRequest`
    // (`0x00417CEF`), and the sentence it makes is the sentence the dialog is.
    assert_eq!(e.get(diplomacy::GROUP, diplomacy::GIFT_TO), Some("Send gift of gold to"));
    assert_eq!(e.get(diplomacy::GROUP, diplomacy::LAST_GIFT), Some("Last gift was"));
    assert_eq!(e.get(diplomacy::GROUP, diplomacy::GIFT_OF), Some("Gift of"));
    assert_eq!(e.get(diplomacy::GROUP, diplomacy::DISPATCH), Some("Dispatch ?"));
    // 72/11 + kind - 1, the four letters, in `g_diploKind` order.
    let letters: Vec<&str> =
        (0..4).map(|k| e.get(diplomacy::GROUP, diplomacy::LETTER_BASE + k).unwrap()).collect();
    assert_eq!(
        letters,
        vec![
            "Give a compliment to",
            "Insult",
            "Ask for an alliance with",
            "End alliance with"
        ],
        "g_diploKind 1..=4"
    );
    // 72/15 + k, and the prompt pair that follows it.
    assert_eq!(e.get(diplomacy::GROUP, diplomacy::REQUEST_BASE), Some("Plead for help from"));
    assert_eq!(
        e.get(diplomacy::GROUP, diplomacy::REQUEST_BASE + 1),
        Some("Plan strategic attack with")
    );
    for k in 0..2 {
        assert!(e.get(diplomacy::GROUP, diplomacy::REQUEST_PROMPT + k).is_some());
        assert!(e.get(diplomacy::GROUP, diplomacy::REQUEST_PICKED + k).is_some());
    }
    // `Ui_DrawCount(value, 0, …)`: group 8's crown pair, singular then plural.
    // The **plural** is what a zero amount takes, which is the half a
    // reimplementation gets wrong.
    assert_eq!(e.get(l2_game::shell::COUNT_NOUN_GROUP, diplomacy::CROWN_NOUN), Some("Crown."));
    assert_eq!(e.get(l2_game::shell::COUNT_NOUN_GROUP, diplomacy::CROWN_NOUN + 1), Some("Crowns."));

    // **The front end, page by page.** Every one of these is the literal
    // argument to a `Ui_DrawCentred` or `Eng_DrawString` in the thirteen
    // painters, so the words are the check and not the existence.
    assert_eq!(e.get(setup::GROUP, 1), Some("\"The siege is on\""), "page 1's subtitle");
    let title: Vec<&str> =
        setup::TITLE_ITEMS.iter().map(|&i| e.get(setup::GROUP, i).unwrap()).collect();
    assert_eq!(
        title,
        vec!["Single player", "Multiple players", "Lords of Magic?", "Exit game"],
        "page 1, in FUN_0041EAA3's own order"
    );
    let options: Vec<&str> =
        setup::OPTION_ITEMS.iter().map(|&i| e.get(setup::GROUP, i).unwrap()).collect();
    assert_eq!(
        options,
        vec!["Play Now!", "Load a game", "Skirmish!", "Custom game", "Back"],
        "page 2, in FUN_0041ECE6's own order"
    );
    assert_eq!(e.get(setup::GROUP, 10), Some("Choose your title and your shield."), "page 4");
    // Page 4's two buttons, then pages 7 and 8's four, from FUN_0041F01F and
    // FUN_0041F6C7 / FUN_0041F77A.
    assert_eq!(e.get(setup::GROUP, 9), Some("Back"));
    assert_eq!(e.get(setup::GROUP, 11), Some("Continue"));
    let buttons: Vec<&str> =
        setup::CUSTOM_BUTTONS.iter().map(|&(_, i)| e.get(setup::GROUP, i).unwrap()).collect();
    assert_eq!(buttons, vec!["Cancel", "Start", "Defaults", "Load"], "pages 7 and 8");
    // Page 5 and page 6 share group 39 and differ only in which pair they draw.
    assert_eq!(e.get(setup::GROUP_EXPANSION, 0), Some("Expansion pack installed, choose:-"));
    assert_eq!(e.get(setup::GROUP_EXPANSION, 4), Some("Original campaign"), "page 5");
    assert_eq!(e.get(setup::GROUP_EXPANSION, 5), Some("The new campaign"), "page 5");
    assert_eq!(e.get(setup::GROUP_EXPANSION, 1), Some("Full game"), "page 6, host only");
    assert_eq!(e.get(setup::GROUP_EXPANSION, 2), Some("Skirmish"), "page 6, host only");
    assert_eq!(
        e.get(setup::GROUP_EXPANSION, 3),
        Some("Please wait while the session creator decides what type of game to play."),
        "page 6 as a joiner sees it - the arm this engine does not draw"
    );
    // Page 3 and page 13, and **the string page 3 used to draw and must not**:
    // 40/8 belongs to the skirmish file box, 40/5 is page 3's heading, and the
    // status line under page 3's list is 40/2, drawn only mid-load.
    assert_eq!(e.get(setup::GROUP_FILE, 5), Some("Loading a game."), "page 3's heading");
    assert_eq!(e.get(setup::GROUP_FILE, 2), Some("Loading game. Please wait."), "and its status");
    assert_eq!(e.get(setup::GROUP_FILE, 6), Some("Click on a skirmish file to load."), "page 13");
    assert_eq!(e.get(setup::GROUP_FILE, 8), Some("Right click to exit."), "page 13, not page 3");
    // Pages 11, 12 and 13's three captions, and the one this engine cannot
    // reach: 39 "Norm." replaces 38 "Cust." while DAT_0056899C is set.
    assert_eq!(e.get(setup::GROUP, 36), Some("Go"));
    assert_eq!(e.get(setup::GROUP, 37), Some("Back"));
    assert_eq!(e.get(setup::GROUP, 38), Some("Cust."));
    assert_eq!(e.get(setup::GROUP, 39), Some("Norm."), "the arm on DAT_0056899C");
    // Page 10, whose five indices are 16, 17, 18, 48, 49 - not 16..=20.
    assert_eq!(e.get(setup::GROUP, 16), Some("No Lords of the Realm CD"));
    assert_eq!(e.get(setup::GROUP, 48), Some("Siege Pack"), "drawn in colour 1, not 0x3F");
    assert!(e.get(setup::GROUP, 49).unwrap().contains("Siege pack CD"));
    // The scenario list's rows come out of group 101, sixty map names.
    assert_eq!(e.get(setup::GROUP_MAPS, 0), Some("England"));
    assert_eq!(e.group(setup::GROUP_MAPS).len(), setup::MAP_COUNT, "one name per map slot");
}

#[test]
fn the_option_runs_name_the_values_a_player_of_the_game_would_recognise() {
    let Some(e) = eng() else {
        eprintln!("skipping: no game install");
        return;
    };
    let values = |i: usize| -> Vec<&str> {
        (0..setup::OPTION_COUNT[i])
            .map(|v| e.get(103, setup::OPTION_BASE[i] + v).unwrap())
            .collect()
    };
    assert_eq!(values(0), vec!["off", "on"], "Advanced Farming");
    assert_eq!(values(2), vec!["two", "three", "four", "five"], "Nobles - never one");
    assert_eq!(values(4), vec!["easy", "normal", "hard", "impossible"], "Difficulty");
    assert_eq!(values(8), vec!["100", "500", "1000", "2500", "5000"], "Crowns");
    assert_eq!(values(11), vec!["humans", "all"], "Fight?");
    // The one string nothing reaches.
    assert_eq!(e.get(103, 4), Some("one"));
}

#[test]
fn the_glyph_map_is_the_table_in_the_users_own_executable() {
    let Some(dir) = install() else {
        eprintln!("skipping: no game install");
        return;
    };
    let exe = std::fs::read(dir.join("Lords2.exe")).expect("Lords2.exe");
    // The image base is 0x400000 and there is no ASLR, so a virtual address is
    // a section offset away from a file offset. `.data` is found by walking the
    // section table rather than by hard-coding the delta.
    let pe = u32::from_le_bytes(exe[0x3C..0x40].try_into().unwrap()) as usize;
    let nsec = u16::from_le_bytes(exe[pe + 6..pe + 8].try_into().unwrap()) as usize;
    let opt = pe + 24;
    let opt_size = u16::from_le_bytes(exe[pe + 20..pe + 22].try_into().unwrap()) as usize;
    let base = u32::from_le_bytes(exe[opt + 28..opt + 32].try_into().unwrap());
    let want = font::GLYPH_MAP_VA - base;
    let mut file_off = None;
    for i in 0..nsec {
        let o = opt + opt_size + i * 40;
        let va = u32::from_le_bytes(exe[o + 12..o + 16].try_into().unwrap());
        let vsize = u32::from_le_bytes(exe[o + 8..o + 12].try_into().unwrap());
        let roff = u32::from_le_bytes(exe[o + 20..o + 24].try_into().unwrap());
        if want >= va && want < va + vsize {
            file_off = Some((roff + (want - va)) as usize);
        }
    }
    let at = file_off.expect("0x004D71F0 is in a section");
    assert_eq!(
        &exe[at..at + 128],
        &font::GLYPH_MAP[..],
        "g_glyphWidths has moved, or the transcription is wrong"
    );
}

#[test]
fn the_fonts_send_descenders_to_the_frames_that_have_them() {
    let Some(dir) = install() else {
        eprintln!("skipping: no game install");
        return;
    };
    let bytes = std::fs::read(dir.join(font::BODY)).expect("Fntl2_14.pl8");
    let pl8 = l2_formats::Pl8::parse(&bytes).expect("the body font parses");
    let frame = |c: char| {
        let e = font::GLYPH_MAP[c as usize - 0x20];
        assert_ne!(e, 0, "{c} has no glyph");
        &pl8.frames[e as usize - 1]
    };
    // The five letters with descenders are taller than the five without, in
    // the same file, under the same map. Nothing but the right mapping does
    // that.
    for d in "gjpqy".chars() {
        for n in "acemn".chars() {
            assert!(
                frame(d).height > frame(n).height,
                "'{d}' should hang below '{n}'"
            );
        }
    }
    // And an uppercase letter is taller again than a lowercase one without an
    // ascender.
    assert!(frame('A').height > frame('a').height);
}

/// Lowercase letters that sit *on* the baseline in all three fonts.
///
/// `g j p q y` descend everywhere. `h` is left out because it descends in
/// `Fntl2_22.pl8` and nowhere else — that typeface gives it a tail, and its
/// frame is 22 rows against 18 for the x-height letters, the same as the real
/// descenders. Excluding one letter costs nothing; pretending it is flat would
/// make this test lie about which font is wrong.
const ON_THE_BASELINE: &str = "abcdefiklmnorstuvwxz";

/// **`Res_LoadStatic` (`0x00499859`) preloads five faces, and records 3 and 5
/// are the two this workspace did not load.** **[V]**
///
/// `g_preloadTable` (`0x004D9F48`) is thirteen `{char name[16]; u32 size}`
/// records, and the loader hands record `n` to `File_ReadChunk` with a buffer
/// picked by `n`. The instruction that picks `&g_font8` for `n == 3` is
/// `mov dword [ebp-4], 0x005CBFB0` at `0x004998ED` — seven bytes, asserted here,
/// because an address in a comment is a claim and these are the bytes.
///
/// Ablated: `font::EIGHT` → `"Font_10.pl8"` — record 3 reads `"fnt_8.pl8"` and
/// the constant does not. (The same ablation also made
/// [`every_font_puts_its_lowercase_on_one_baseline`] panic with a lowercase
/// letter that draws nothing — which was the reason `Font_10.pl8` was not loaded
/// blindly, and is now explained by
/// [`font_10_is_a_numeral_face_read_through_the_shared_table`].)
#[test]
fn the_preload_table_names_every_face_and_record_3_is_g_font8() {
    let exe = l2_testkit::executable!();
    let record = |n: u32| -> String {
        let off = l2_testkit::pe::va_to_offset(&exe, 0x004D_9F48 + n * 0x14).expect("in .data");
        let name = &exe[off..off + 16];
        let end = name.iter().position(|&b| b == 0).unwrap_or(16);
        String::from_utf8_lossy(&name[..end]).to_ascii_lowercase()
    };
    assert_eq!(record(3), font::EIGHT.to_ascii_lowercase(), "g_font8");
    assert_eq!(record(4), font::SMALL.to_ascii_lowercase(), "g_fontSmall");
    assert_eq!(record(5), font::TEN.to_ascii_lowercase(), "g_font10");
    assert_eq!(record(6), font::BODY.to_ascii_lowercase(), "g_fontBody");
    assert_eq!(record(7), font::HEADING.to_ascii_lowercase(), "g_fontHeading");

    let at = l2_testkit::pe::va_to_offset(&exe, 0x0049_98ED).expect("in .text");
    assert_eq!(
        &exe[at..at + 7],
        &[0xC7, 0x45, 0xFC, 0xB0, 0xBF, 0x5C, 0x00],
        "Res_LoadStatic's record-3 arm is mov [ebp-4], &g_font8"
    );
}

/// **The measure and the draw disagree about `'@'`, as the original's do.**
///
/// `FUN_004014F0` (`0x004014F0`) charges 4 for `0x20` and nothing for any other
/// empty `g_glyphWidths` entry; `Ui_DrawText` (`0x00402637`) advances
/// `local_14 = 4` for all of them. `Font::width` charged 4 for `'@'`.
///
/// Ablated: `Font::width`'s `None => 0` arm → `SPACE_ADVANCE` — the first
/// assertion goes red by exactly four, 43 against 39.
#[test]
fn the_measure_charges_the_blank_sign_column_nothing_and_the_draw_charges_four() {
    let Some(dir) = install() else {
        eprintln!("skipping: no game install");
        return;
    };
    let bytes = std::fs::read(dir.join(font::BODY)).expect("Fntl2_14.pl8");
    let f = font::Font::new(bytes, 16).expect("the font loads");
    assert_eq!(f.width("@1000"), f.width("1000"), "FUN_004014F0 charges '@' nothing");
    assert_eq!(f.width(" 1000"), f.width("1000") + 4, "and a space four");

    let flat = font::Style { colour: font::TEXT, shadow: None, caps: None };
    let drawn = |s: &str| f.draw(&mut Canvas::new(120, 40), 0, 2, s, &flat);
    assert_eq!(drawn("@1000"), drawn(" 1000"), "Ui_DrawText advances both four");
}

/// Every lowercase letter in a font must land on one baseline once drawn.
///
/// This is the test that would have caught the bug a player found by opening
/// the original next to our demo: `a c e m n o s u x z` sat three pixels below
/// `b d f h i k l t`, and the split was exactly frame record byte `0x0D`.
/// `Font::draw` added that byte to `y` while the decoder had already reserved
/// the same rows at the top of the canvas, so the offset was applied twice —
/// but only for the frames whose rows were *stored*, which is why the two
/// halves of one alphabet disagreed.
///
/// It draws through `Font::draw` rather than reading frame records, because the
/// records were never wrong. The whole bug lived between the decoder and the
/// blitter, and only an end-to-end render can see that seam.
///
/// The tolerance is one pixel and it is earned, not slack: `r v w` in
/// `Fntl2_14.pl8` and `s` in `Fntl2_22.pl8` end in a single-pixel terminal one
/// row below the stroke. A misapplied `0x0D` is three pixels, or four in the
/// heading font — far outside it.
///
/// **Four of the five faces.** `Font_10.pl8` has no lowercase to put on a
/// baseline; its digits are checked in
/// [`font_10_is_a_numeral_face_read_through_the_shared_table`].
#[test]
fn every_font_puts_its_lowercase_on_one_baseline() {
    let Some(dir) = install() else {
        eprintln!("skipping: no game install");
        return;
    };

    for (name, line) in [(font::EIGHT, 12), (font::SMALL, 12), (font::BODY, 16), (font::HEADING, 24)] {
        let bytes = std::fs::read(dir.join(name)).unwrap_or_else(|_| panic!("{name}"));
        let records: Vec<u8> = l2_formats::Pl8::parse(&bytes)
            .expect("the font parses")
            .frames
            .iter()
            .map(|f| f.overhang_rows)
            .collect();
        let f = font::Font::new(bytes, line).expect("the font loads");
        // Flat, so a shadow pass cannot extend a glyph a row past its own ink.
        let flat = font::Style { colour: font::TEXT, shadow: None, caps: None };

        let mut bottoms: Vec<(char, u8, usize)> = Vec::new();
        for c in ON_THE_BASELINE.chars() {
            let mut canvas = Canvas::new(48, 48);
            f.draw(&mut canvas, 2, 2, &c.to_string(), &flat);
            let bottom = (0..canvas.height)
                .rfind(|&y| (0..canvas.width).any(|x| canvas.at(x, y) != 0))
                .unwrap_or_else(|| panic!("{name}: '{c}' drew nothing"));
            let over = records[font::GLYPH_MAP[c as usize - 0x20] as usize - 1];
            bottoms.push((c, over, bottom));
        }

        let base = bottoms.iter().map(|&(_, _, b)| b).min().expect("letters");
        for &(c, over, b) in &bottoms {
            assert!(
                b == base || b == base + 1,
                "{name}: '{c}' (0x0D = {over}) bottoms at row {b}, not {base} or {}",
                base + 1,
            );
        }

        // The sharp half: the letters that declare overhang rows and the ones
        // that do not must reach the *same* first row. Under the old
        // double-application these two groups differed by exactly the declared
        // count, which is what the player saw.
        let mut groups: std::collections::BTreeMap<u8, usize> = Default::default();
        for &(_, over, b) in &bottoms {
            let e = groups.entry(over).or_insert(b);
            *e = (*e).min(b);
        }
        println!("{name}: baseline {base}, by 0x0D {groups:?}");
        for (over, b) in &groups {
            assert_eq!(
                *b, base,
                "{name}: the letters declaring {over} overhang rows sit {} pixels off \
                 the letters that do not — 0x0D is being applied twice",
                *b as i32 - base as i32,
            );
        }
        // Not vacuous in the fonts that have both kinds. `Fnt_8.pl8` has
        // `0x0D == 0` on all 150 frames (`docs/formats/pl8-failures.md` §4), so
        // for it only the one-baseline half above is a claim — and it holds.
        if name != font::HEADING && name != font::EIGHT {
            assert!(groups.len() >= 2, "{name}: expected both 0x0D = 0 and 0x0D > 0 letters");
        }
    }
}

/// **`Font_10.pl8` is a numeral face in a full font's layout, read through the
/// same table as the other four.** **[V]**
///
/// `Glyph_Draw` (`0x00402A14`) has one character map, `g_glyphWidths`, and no
/// per-face anything: `frame = g_glyphWidths[c - 0x20] - 1`, the record at
/// `font + frame * 0x10 + 8`, no check against the file's frame count. So the
/// only ways a face could be "partial" are a table that reaches past its end or
/// frames that are not glyphs, and this asserts which it is:
///
/// 1. **108 frames, and nothing in the table reaches past them** — not a
///    different index base, not a short file;
/// 2. **every letter is a 2 × 2 stub** that advances three, and a lowercase
///    word drawn in it has next to no ink — *"Seasons"* is one `'e'`, four
///    pixels, where `Fntl2_9.pl8` draws it in dozens. That is the "renders
///    nothing" a canvas diff passes over;
/// 3. **everything the nine call sites build is a glyph or a blank** — `'+'`,
///    `'-'` and the ten digits have ink, `' '` and `'@'` are table zeros — and
///    the digits sit on one baseline.
///
/// Ablated: pointing the test at `font::SMALL` turns claim 2 red on `'a'`
/// (advance 8, not 3). Pointing it at `font::EIGHT` turns claim 1 red (150
/// frames).
#[test]
fn font_10_is_a_numeral_face_read_through_the_shared_table() {
    let Some(dir) = install() else {
        eprintln!("skipping: no game install");
        return;
    };
    let name = font::TEN;
    let bytes = std::fs::read(dir.join(name)).expect("Font_10.pl8");
    let pl8 = l2_formats::Pl8::parse(&bytes).expect("Font_10.pl8 parses");
    // 1
    assert_eq!(pl8.frames.len(), 108, "{name}: the full 108-frame layout");
    let furthest = font::GLYPH_MAP.iter().copied().max().expect("a table") as usize;
    assert!(
        furthest <= pl8.frames.len(),
        "{name}: g_glyphWidths reaches frame {furthest} and the file has {}",
        pl8.frames.len()
    );

    let f = font::Font::new(bytes.clone(), 12).expect("the font loads");
    let flat = font::Style { colour: font::TEXT, shadow: None, caps: None };
    let ink = |s: &str| -> (usize, Canvas) {
        let mut canvas = Canvas::new(160, 32);
        f.draw(&mut canvas, 2, 2, s, &flat);
        let n = canvas.pixels.iter().filter(|&&p| p != 0).count();
        (n, canvas)
    };

    // 2
    for c in ('a'..='z').chain('A'..='Z') {
        let frame = &pl8.frames[font::GLYPH_MAP[c as usize - 0x20] as usize - 1];
        assert_eq!(frame.height, 2, "{name}: '{c}' should be a 2x2 stub");
        let advance = f.draw(&mut Canvas::new(16, 16), 0, 0, &c.to_string(), &flat);
        assert_eq!(advance, 3, "{name}: '{c}' advances its stub's width plus one");
    }
    let (word, _) = ink("Seasons");
    assert!(word <= 4, "{name}: \"Seasons\" should draw at most its 'e' - it drew {word} pixels");

    // 3
    for c in "0123456789+-".chars() {
        let (n, _) = ink(&c.to_string());
        assert!(n >= 8, "{name}: '{c}' is a real glyph and drew only {n} pixels");
    }
    for c in [' ', '@'] {
        assert_eq!(font::GLYPH_MAP[c as usize - 0x20], 0, "'{c}' is a blank, not a glyph");
    }
    let bottom = |c: char| -> usize {
        let (_, canvas) = ink(&c.to_string());
        (0..canvas.height)
            .rfind(|&y| (0..canvas.width).any(|x| canvas.at(x, y) != 0))
            .unwrap_or_else(|| panic!("{name}: '{c}' drew nothing"))
    };
    let base = bottom('0');
    for c in "123456789".chars() {
        assert_eq!(bottom(c), base, "{name}: '{c}' is off the digits' baseline");
    }
}

// ------------------------------------------------------------------ helpers

/// The n'th menu item of setup pages 1 and 2, in canvas coordinates.
fn setup_item(i: usize) -> (i32, i32) {
    (0xE0 + 8, 0x5B + i as i32 * 0x24 + 8)
}

/// The n'th bottom button of the custom-game page.
fn custom_button(i: usize) -> (i32, i32) {
    ([0xA5, 0xF3, 0x141, 399][i] + 8, 0xC6 + 4)
}

fn click((x, y): (i32, i32)) -> Event {
    Event::Click { x, y }
}

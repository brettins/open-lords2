//! **Can a player type their name?**
//!
//! ```text
//! cargo test -p l2-game --test text
//! LORDS2_DIR="F:\games\Lords of the Realm II" cargo test -p l2-game --test text
//! ```
//!
//! The report was one line — *"I can't type my name in the start menu?"* — and
//! the answer was that this workspace had no keyboard text entry at all. So the
//! first test here is the report, driven the way a person drives it: through
//! [`Machine::handle`] with [`Event`] values, no window, no focus, no cursor
//! (`docs/agents.md`).
//!
//! # What the rest of the file is guarding against
//!
//! `docs/agents.md` names the failure this feature is shaped exactly like:
//! **a field is only tested if something a test reads was written by something
//! the game runs.** Six instances so far, every one behind a green suite. A
//! typed name has to survive four separate hand-offs and a test that skips any
//! of them is checking its own fixture:
//!
//! 1. the keystroke into the field ([`typing_a_name_reaches_the_field`]);
//! 2. the field into `g_playerNames` ([`start_puts_the_typed_name_into_the_realm`]);
//! 3. `g_playerNames` into the file and back
//!    ([`a_typed_name_survives_the_save_and_the_reload`]);
//! 4. the field onto the screen ([`the_name_and_its_caret_are_painted`]).
//!
//! Only (2) needs the install, because only *Start* needs a map to build.

use l2_game::game::{Assets, Game};
use l2_game::input::{Event, Key};
use l2_game::screen::{Ctx, Screen, Transition};
use l2_game::screens::setup::{SetupPage, SetupScreen, NAME_PLATE_X, NAME_PLATE_Y, NAME_X};
use l2_game::text::{FontMetrics, Kind, PlayerName, TextField, NAME_MAX_TYPED, PLAYER_NAME_LEN};
use l2_kingdom::realm::MAX_REALMS;
use l2_kingdom::tables::Tables;
use l2_view::Canvas;

/// Everything the front end needs and nothing else. `Assets::placeholder` is
/// used only where the assertion is about a *value*; the drawing tests below
/// refuse it by name, because `docs/agents.md` records that every campaign-map
/// test ran on the placeholder, the one configuration where a broken hit test
/// and the picture agree.
fn bare() -> (Game, Assets) {
    (Game::new(1), Assets::placeholder())
}

/// Type a string the way `main.rs` delivers it: `WM_KEYDOWN` **and** `WM_CHAR`,
/// in that order, for every printable key.
///
/// **Both, deliberately.** Sending only `Event::Text` would let a screen that
/// wrongly acts on the `KeyDown` half pass — which is the bug this pair was
/// introduced to make impossible, and it was live for one compile: `Space`
/// typed a space *and* pressed the highlighted button.
fn type_into(m: &mut SetupScreen, game: &mut Game, assets: &Assets, s: &str) -> Vec<Transition> {
    let mut out = Vec::new();
    for c in s.chars() {
        let key = if c == ' ' { Key::Space } else { Key::letter(c) };
        for e in [Event::KeyDown(key), Event::Text(c)] {
            let mut ctx = Ctx { game, assets };
            out.push(m.handle(e, &mut ctx));
        }
    }
    out
}

fn press(m: &mut SetupScreen, game: &mut Game, assets: &Assets, key: Key) -> Transition {
    let mut ctx = Ctx { game, assets };
    m.handle(Event::KeyDown(key), &mut ctx)
}

/// The setup screen on page 4, **reached the way a person reaches it** — the
/// title menu's *Multiple players*, whose arm is one of the three that runs
/// `Edit_Begin`. Building `SetupScreen::new(SetupPage::Shield)` would skip the
/// seeding and test a field nothing had opened.
fn name_page(game: &mut Game, assets: &Assets) -> SetupScreen {
    let mut m = SetupScreen::new(SetupPage::Title);
    let mut ctx = Ctx { game, assets };
    m.handle(Event::KeyDown(Key::Down), &mut ctx);
    m.handle(Event::KeyDown(Key::Enter), &mut ctx);
    assert_eq!(m.page(), SetupPage::Shield, "the title menu's second item opens page 4");
    m
}

fn field_of(m: &SetupScreen) -> String {
    m.name()
}

fn pushed(ts: &[Transition]) -> bool {
    ts.iter().any(|t| matches!(t, Transition::Push(_)))
}

// ---------------------------------------------------------------------------
// 1. The report
// ---------------------------------------------------------------------------

/// **The bug, as reported.** A person on the start menu types their name and it
/// appears.
#[test]
fn typing_a_name_reaches_the_field() {
    let (mut game, assets) = bare();
    let mut m = name_page(&mut game, &assets);
    assert_eq!(field_of(&m), "Player1", "the field opens seeded — Edit_Begin(&g_options, …)");

    // `Edit_Begin` puts the caret at 0 and the default is OVERWRITE, so the
    // first thing a person sees is their typing replacing the default name.
    type_into(&mut m, &mut game, &assets, "Richard");
    assert_eq!(field_of(&m), "Richard", "seven characters over seven");
}

/// **Overwrite is the default, and Delete is how a person gets out of it.**
///
/// Reproduced rather than corrected: `g_editInsert` starts at zero and zero is
/// the overwrite branch. `docs/bugs.md` B79.
#[test]
fn a_short_name_leaves_the_tail_of_the_old_one_until_delete_or_insert() {
    let (mut game, assets) = bare();

    let mut m = name_page(&mut game, &assets);
    type_into(&mut m, &mut game, &assets, "Ed");
    assert_eq!(field_of(&m), "Edayer1", "the original's behaviour, not a defect of ours");
    for _ in 0..5 {
        press(&mut m, &mut game, &assets, Key::Delete);
    }
    assert_eq!(field_of(&m), "Ed", "VK_DELETE clears the tail");

    // The other way out is the Insert key, which is why it is wired at all.
    let mut m = name_page(&mut game, &assets);
    press(&mut m, &mut game, &assets, Key::Insert);
    type_into(&mut m, &mut game, &assets, "Ed");
    assert_eq!(field_of(&m), "EdPlayer1");
}

/// **A space is a character and `I` is a letter**, on the one page that has a
/// field — and both were live bugs the moment the field arrived, because
/// `main.rs` sends the `WM_KEYDOWN` half too and the screen was acting on it.
#[test]
fn the_field_takes_the_keys_the_menu_would_otherwise_spend() {
    let (mut game, assets) = bare();
    let mut m = name_page(&mut game, &assets);

    let ts = type_into(&mut m, &mut game, &assets, "Ivo Rex");
    assert_eq!(field_of(&m), "Ivo Rex", "the I typed and the space typed");
    assert!(!pushed(&ts), "and the I did not also open the screen index");
    assert_eq!(m.page(), SetupPage::Shield, "and the space did not also press a button");

    // Off the name page they mean what they meant before: `I` is the index.
    let (mut game, assets) = bare();
    let mut title = SetupScreen::new(SetupPage::Title);
    let mut ctx = Ctx { game: &mut game, assets: &assets };
    let t = title.handle(Event::KeyDown(Key::Char('I')), &mut ctx);
    assert!(matches!(t, Transition::Push(_)), "the title page still has its keyboard");
}

// ---------------------------------------------------------------------------
// 2. The field into the game
// ---------------------------------------------------------------------------

/// **The hand-off the six previous instances of this bug all failed at.**
///
/// Typing into a field that nothing reads is the same defect as
/// `castle_degraded`, which was written by nothing. *Start* runs
/// `Player_SetHuman` (`0x0049BAE9`), and this asserts on `Game::player_names`
/// after driving the whole page — the name is never assigned by the test.
///
/// Install-gated because *Start* builds a world out of `L2_maps.dat` and
/// refuses rather than half-starting one when it cannot.
///
/// **Page 4 is reached through the campaign chooser, and that is not
/// incidental.** `FUN_00433155`'s *Continue* arm only starts a game when
/// `DAT_0057D320` says a campaign was chosen; every other route out of page 4
/// walks on to the page that picks a game. This test used to arrive from the
/// title menu's *Multiple players*, which is one of the arms that **clears**
/// that flag, so after that arm was reproduced it was pressing a button that
/// correctly does not start anything. The name still has to survive the trip:
/// `Setup_ChooseCampaign` re-seeds the field on arrival, so *"Aethelred"* is
/// typed after page 4 is open, exactly as a person types it.
#[test]
fn start_puts_the_typed_name_into_the_realm() {
    let Some(dir) = l2_testkit::install_dir() else {
        l2_testkit::skip!("no game install, so Start has no map to build");
    };
    let platform = l2_mods::Platform::builder().base(&dir).build().expect("the install mounts");
    let assets = Assets::load(&platform.vfs).expect("assets load");
    let mut game = Game::new(1);

    let mut screen = SetupScreen::new(SetupPage::Title);
    {
        let mut ctx = Ctx { game: &mut game, assets: &assets };
        // "Single player", "Play Now!", then the left-hand campaign — the
        // three items that are already highlighted, so Enter three times.
        screen.handle(Event::KeyDown(Key::Enter), &mut ctx);
        assert_eq!(screen.page(), SetupPage::Options);
        screen.handle(Event::KeyDown(Key::Enter), &mut ctx);
        assert_eq!(screen.page(), SetupPage::Campaign);
        screen.handle(Event::KeyDown(Key::Enter), &mut ctx);
        screen.update(&mut ctx);
    }
    assert_eq!(screen.page(), SetupPage::Shield);

    for c in "Aethelred".chars() {
        let mut ctx = Ctx { game: &mut game, assets: &assets };
        screen.handle(Event::Text(c), &mut ctx);
    }
    assert_eq!(screen.name(), "Aethelred");

    // *Continue* — the second of page 4's two captions, at (0x150, 0xD7).
    {
        let mut ctx = Ctx { game: &mut game, assets: &assets };
        screen.handle(Event::Click { x: 0x150 + 40, y: 0xD7 + 8 }, &mut ctx);
    }

    let player = game.player as usize;
    assert_eq!(
        game.player_names[player].as_str(),
        "Aethelred",
        "g_playerNames[g_localPlayer] is what Player_SetHuman writes, and nothing \
         in this test assigned it"
    );

    // **And the other five slots are the AI lords', from `L2.eng` group 7.**
    // The index is the LORD, not the realm — `docs/diplomacy.md` §0.1 — so this
    // also checks the array is not simply indexed by slot.
    let others: Vec<String> = (0..MAX_REALMS)
        .filter(|r| *r != player)
        .map(|r| game.player_names[r].as_str())
        .collect();
    assert!(
        others.iter().all(|n| !n.is_empty()),
        "every realm is named, not only the human's: {others:?}"
    );
    for (r, n) in (0..MAX_REALMS).filter(|r| *r != player).zip(&others) {
        let lord = game.kingdom.realms[r].lord as usize;
        let title = assets.shell.text(7, lord.min(4));
        assert_eq!(n, &title.chars().take(0x10).collect::<String>(), "realm {r} is lord {lord}");
    }
}

// ---------------------------------------------------------------------------
// 3. The file
// ---------------------------------------------------------------------------

/// **A name that does not survive a save is a name a player loses.**
///
/// `docs/agents.md`: six fields have reached `main` written by nothing or
/// dropped by the codec, every one behind a green suite. This drives the real
/// encoder and the real decoder — `l2_game::save::encode`/`decode`, the same
/// pair a player's *Save* button reaches — and it is the reason
/// `l2_game::save::VERSION` moved to 3.
///
/// The **ablation** for this one is mechanical and was run: deleting the
/// `for name in &game.player_names` loop from `encode_prefix` turns
/// `crates/l2-testkit/tests/encoding.rs` red with
/// `Game.player_names — not named in encode`. It did **not**, on the first
/// attempt, because that check matched the comment above the loop; it reads
/// code with the comments stripped now, which is a defect fixed in the shared
/// check rather than worked around here.
#[test]
fn a_typed_name_survives_the_save_and_the_reload() {
    let mut game = Game::new(7);
    // Six distinct names, so a codec that wrote one slot six times, or walked
    // the array in the wrong order, cannot pass.
    let names = ["Aethelred", "The Knight", "The Baron", "The Countess", "The Bishop", "No player"];
    for (slot, n) in game.player_names.iter_mut().zip(names) {
        *slot = PlayerName::new(n);
    }

    let bytes = l2_game::save::encode(&game);
    let back = l2_game::save::decode(&bytes, Tables::DEFAULT).expect("the save reads back");

    for (r, want) in names.iter().enumerate() {
        assert_eq!(back.player_names[r].as_str(), *want, "realm {r}");
    }
}

/// The record is 31 bytes with no length prefix, which is `g_playerNames`'
/// own shape and what keeps the encoding fixed-width.
#[test]
fn a_name_is_thirty_one_bytes_and_truncates_rather_than_growing() {
    let long = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghij";
    assert!(long.len() > PLAYER_NAME_LEN);
    let n = PlayerName::new(long);
    assert_eq!(n.bytes().len(), PLAYER_NAME_LEN);
    assert_eq!(n.as_str().len(), PLAYER_NAME_LEN);
    assert_eq!(n.as_str(), &long[..PLAYER_NAME_LEN]);
    assert!(PlayerName::EMPTY.is_empty());
    // Round trip through the raw bytes, which is what the codec does.
    assert_eq!(PlayerName::from_bytes(*n.bytes()), n);
}

/// **The two limits are the original's, and sixteen is the one a person meets.**
///
/// `Edit_Begin(&g_options, 0x10, 0xC0, 0)` and `Edit_Commit(&g_options, 0x1F)`
/// are different numbers on purpose: sixteen is what may be typed, thirty-one
/// is how wide the destination is.
#[test]
fn the_name_field_stops_at_sixteen_characters() {
    // **Overwrite**, the default: sixteen characters go in and the rest are
    // dropped in silence.
    let (mut game, assets) = bare();
    let mut m = name_page(&mut game, &assets);
    type_into(&mut m, &mut game, &assets, "ABCDEFGHIJKLMNOPQRSTUVWXYZ");
    assert_eq!(field_of(&m), "ABCDEFGHIJKLMNOP", "sixteen, and no message about the rest");

    // **Insert**, and the limit is a *different* expression of the same
    // sixteen: `Edit_Insert`'s insert branch tests the LENGTH against the
    // limit, not the caret, so a field still holding its seven-character seed
    // accepts only nine more. That asymmetry is the original's and this is
    // where it shows.
    let (mut game, assets) = bare();
    let mut m = name_page(&mut game, &assets);
    press(&mut m, &mut game, &assets, Key::Insert);
    press(&mut m, &mut game, &assets, Key::Home);
    type_into(&mut m, &mut game, &assets, "ABCDEFGHIJKLMNOPQRSTUVWXYZ");
    assert_eq!(
        field_of(&m),
        "ABCDEFGHIPlayer1",
        "nine inserted in front of the seven-character seed, and the tenth refused",
    );
    assert_eq!(field_of(&m).len(), NAME_MAX_TYPED);
}

// ---------------------------------------------------------------------------
// 4. The picture
// ---------------------------------------------------------------------------

/// **The name and its caret are actually painted**, and this is asserted the
/// way `docs/agents.md` says to assert a draw: by drawing the page twice and
/// requiring the second draw to change nothing.
///
/// Text is an opaque blit, so re-drawing it over itself is a no-op — *but only
/// if it was there the first time*. Counting non-background pixels in a band
/// would measure `gateway.pl8`, which is exactly how the build-stamp test came
/// to pass with the stamp deleted.
///
/// The **caret** is checked separately and by difference, because it is the
/// half that blinks: two draws one tick apart, and the pixels under the caret
/// must differ.
#[test]
fn the_name_and_its_caret_are_painted() {
    let Some(dir) = l2_testkit::install_dir() else {
        l2_testkit::skip!("no game install, so no Panels2.pl8 and no Fntl2_14.pl8");
    };
    let platform = l2_mods::Platform::builder().base(&dir).build().expect("the install mounts");
    let assets = Assets::load(&platform.vfs).expect("assets load");
    let mut game = Game::new(1);

    let mut screen = SetupScreen::new(SetupPage::Title);
    {
        let mut ctx = Ctx { game: &mut game, assets: &assets };
        screen.handle(Event::KeyDown(Key::Down), &mut ctx);
        screen.handle(Event::KeyDown(Key::Enter), &mut ctx);
        for c in "Aethelred".chars() {
            screen.handle(Event::Text(c), &mut ctx);
        }
    }

    let draw = |screen: &mut SetupScreen, game: &mut Game| {
        let mut c = Canvas::new(640, 480);
        let ctx = Ctx { game, assets: &assets };
        screen.draw(&ctx, &mut c);
        c
    };

    // **The name is on the plate.** Blank the field, draw, restore, draw: the
    // two pictures must differ inside the plate and nowhere else on the page.
    let with = draw(&mut screen, &mut game);
    let mut blank = SetupScreen::new(SetupPage::Shield);
    {
        // A field opened on an empty seed — the same page with no name in it.
        let mut ctx = Ctx { game: &mut game, assets: &assets };
        for _ in 0..NAME_MAX_TYPED {
            blank.handle(Event::KeyDown(Key::Delete), &mut ctx);
        }
    }
    let without = draw(&mut blank, &mut game);
    let differing: Vec<(i32, i32)> = (0..640)
        .flat_map(|x| (0..480).map(move |y| (x, y)))
        .filter(|&(x, y)| {
            with.pixels[y as usize * 640 + x as usize] != without.pixels[y as usize * 640 + x as usize]
        })
        .collect();
    assert!(!differing.is_empty(), "the name is drawn at all");
    let plate = (NAME_PLATE_X, NAME_PLATE_Y, 0xE0, 0x20);
    for &(x, y) in &differing {
        assert!(
            x >= plate.0 && x < plate.0 + plate.2 && y >= plate.1 && y < plate.1 + plate.3,
            "a pixel at ({x}, {y}) changed with the name and is outside the name plate \
             ({}, {})..({}, {}) — the field is drawing out of its recess",
            plate.0,
            plate.1,
            plate.0 + plate.2,
            plate.1 + plate.3,
        );
    }
    // And the text starts where `Ui_DrawText(&g_options, 0xD6, 0x50, …)` puts
    // it, which is six pixels into the plate and eight down.
    let left = differing.iter().map(|p| p.0).min().expect("some pixel");
    assert!(
        (NAME_X..NAME_X + 8).contains(&left),
        "the name starts at x = {left}, and the painter's origin is {NAME_X}"
    );

    // **The caret blinks**, which is the whole of the affordance. Step the
    // field a whole period and require both a lit and an unlit picture.
    let mut lit = false;
    let mut dark = false;
    for _ in 0..20 {
        {
            let mut ctx = Ctx { game: &mut game, assets: &assets };
            screen.update(&mut ctx);
        }
        let now = draw(&mut screen, &mut game);
        let same = (0..640).all(|x| {
            (0..480).all(|y| now.pixels[y as usize * 640 + x as usize] == with.pixels[y as usize * 640 + x as usize])
        });
        if same {
            dark = true;
        } else {
            lit = true;
        }
    }
    assert!(lit && dark, "the caret is drawn on some ticks and not on others: lit {lit}, dark {dark}");
}

// ---------------------------------------------------------------------------
// The engine, against the metrics rather than a font
// ---------------------------------------------------------------------------

/// **The pixel limit is a real limit and is measured with the font.**
///
/// It is what stops a sixteen-character name from running off a 224-pixel
/// plate, and it bites before the character limit for any name of ordinary
/// letters. Gated because it needs the real glyph widths — with no font every
/// character is four pixels wide and the limit means something else.
#[test]
fn the_pixel_limit_bites_before_the_character_limit() {
    let Some(dir) = l2_testkit::install_dir() else {
        l2_testkit::skip!("no game install, so no Fntl2_14.pl8 to measure with");
    };
    let platform = l2_mods::Platform::builder().base(&dir).build().expect("the install mounts");
    let assets = Assets::load(&platform.vfs).expect("assets load");
    let m = FontMetrics::of(&assets.shell);

    let mut f = TextField::begin("", NAME_MAX_TYPED, l2_game::text::NAME_MAX_PIXELS, Kind::Text);
    f.toggle_insert();
    for _ in 0..NAME_MAX_TYPED {
        f.type_char('W', &m);
    }
    let wide = f.text();
    assert!(
        wide.len() < NAME_MAX_TYPED,
        "sixteen W's are wider than 192 pixels, so the field stopped early: got {} of {}",
        wide.len(),
        NAME_MAX_TYPED,
    );
    assert!(!wide.is_empty(), "and it did not refuse the first one");
}

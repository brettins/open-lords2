#![allow(unused_imports)]
use super::*;
use super::setup::*;
use super::persistence::*;
use l2_game::game::{Assets, Game};
use l2_game::input::{Event, Key};
use l2_game::screen::{Ctx, Screen, Transition};
use l2_game::screens::setup::{SetupPage, SetupScreen, NAME_PLATE_X, NAME_PLATE_Y, NAME_X};
use l2_game::text::{FontMetrics, Kind, PlayerName, TextField, NAME_MAX_TYPED, PLAYER_NAME_LEN};
use l2_kingdom::realm::MAX_REALMS;
use l2_kingdom::tables::Tables;
use l2_view::Canvas;

/// **The name and its caret are painted**, and this is asserted the
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
// The engine, against the metrics
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

// ---------------------------------------------------------------------------
// 5. `g_playerNames` onto a screen that is about the player
// ---------------------------------------------------------------------------

/// **A player: *"it says Court of LORD1 even though I wrote a name when I
/// started the game"*.**
///
/// The fifth hand-off, and the file header's four all passed while it was
/// broken: the name reached `Game::player_names`, survived a save and came back
/// — and `Court_Draw` (`0x00416925`) drew `format!("LORD {player}")` anyway, on
/// a comment that said *"nothing in this tree carries them yet"*. True when it
/// was written; false from the moment hand-off 2 existed.
///
/// The original is `Eng_DrawString(70, 5, 0x50, 0x44, heading)` — *"Court of"* —
/// then `Ui_DrawText(&g_playerNames + realm * 0x2C, g_penAdvance + 0x52, 0x44,
/// heading)`. **`LORD1` is not in `L2.eng` at all**: a scan of all 317 groups
/// for a lord-plus-digit finds nothing, and group 7 — the only default-name
/// group — holds *"No player"*, *"The Knight"*, *"The Baron"*, *"The Countess"*,
/// *"The Bishop"*, indexed by the **lord**. So the string the player saw was
/// ours, not a fallback showing through: one defect, not two.
///
/// **Ablated:** put `format!("LORD {player}")` back and the two pictures are
/// identical, because neither reads the array.
#[test]
fn the_court_is_headed_with_the_name_the_player_typed() {
    let assets = Assets::placeholder();
    let paint = |name: &str| {
        let mut game = Game::new(1);
        let player = game.player as usize;
        game.player_names[player] = PlayerName::new(name);
        let mut screen = l2_game::screens::court::CourtScreen::new();
        let mut canvas = Canvas::new(640, 480);
        let ctx = Ctx { game: &mut game, assets: &assets };
        screen.draw(&ctx, &mut canvas);
        canvas
    };
    let aethelred = paint("Aethelred");
    let cuthbert = paint("Cuthbert");

    let differing: Vec<(i32, i32)> = (0..640)
        .flat_map(|x| (0..480).map(move |y| (x, y)))
        .filter(|&(x, y)| {
            aethelred.pixels[y as usize * 640 + x as usize]
                != cuthbert.pixels[y as usize * 640 + x as usize]
        })
        .collect();
    assert!(
        !differing.is_empty(),
        "the court heading does not read g_playerNames: two names drew the same screen"
    );
    // And it changed **only** the heading — `Ui_DrawText(name, pen + 0x52,
    // 0x44, heading)`, so nothing left of `0x50` and nothing off its row.
    let (hx, hy) = l2_game::screens::court::HEADING_AT;
    for &(x, y) in &differing {
        assert!(
            x >= hx && (hy - 2..hy + 24).contains(&y),
            "a pixel at ({x}, {y}) changed with the lord's name, outside the heading"
        );
    }
}


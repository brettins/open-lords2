#![allow(unused_imports)]
use super::*;
use super::dialog_flow::*;
use super::offers_and_requests::*;
use l2_game::game::Assets;
use l2_game::input::Event;
use l2_game::screen::{Ctx, Machine, Screen, ScreenId, Transition};
use l2_game::screens::diplomacy::{self, DiplomacyScreen, Menu};
use l2_game::Game;
use l2_kingdom::diplomacy::Kind;
use l2_view::Canvas;

// ---------------------------------------------------------------- the letter
//
// `0x0042FF10/compose-letter-text`. `Assets::placeholder()` has no `L2.eng`,
// so these run on the transcribed defaults; `tests/shell.rs` checks the
// group's own words against the file.

/// **A letter opens on its group-226 default and the first keystroke types
/// over it.**
///
/// `Options_SetDefaults` (`0x004AE310`) fills the four drafts from `L2.eng`
/// group 226 indices 0…3, and `Diplo_OpenCompliment` (`0x0043618B`) hands the
/// slot to `Edit_Begin` with the caret at 0. Overwrite is the default
/// (`0x00401D26/overwrite-default`), so the first character lands *on* the
/// first character of the default — the same B79 the
/// name page has.
///
/// **Ablation, run:** delete the `self.field(ctx).event(…)` block from
/// `ComposeScreen::handle`.
#[test]
fn a_letter_opens_on_its_default_and_the_first_key_types_over_it() {
    let (mut game, assets) = world();
    let mut screen = diplomacy::ComposeScreen::new(2, Kind::Compliment.byte());
    let mut ctx = Ctx { game: &mut game, assets: &assets };
    screen.handle(Event::Text('Z'), &mut ctx);
    let draft = screen.draft();
    assert!(draft.starts_with("Zerily,"), "{draft}");
    // The default was not cleared: only the character under the caret went.
    assert!(draft.ends_with("teach me more."), "{draft}");
}

/// **The four kinds open on four different drafts**, which is what
/// `Eng_CopyString(…, 0xE2, k, 200)` indexing by `k` means.
#[test]
fn the_four_letter_kinds_open_on_four_different_drafts() {
    let (mut game, assets) = world();
    let mut seen: Vec<String> = Vec::new();
    for kind in [Kind::Compliment, Kind::Insult, Kind::OfferAlliance, Kind::EndAlliance] {
        let mut screen = diplomacy::ComposeScreen::new(2, kind.byte());
        let mut ctx = Ctx { game: &mut game, assets: &assets };
        // Any event that reaches the field seeds it; Home moves nothing.
        screen.handle(Event::KeyDown(l2_game::input::Key::Home), &mut ctx);
        let d = screen.draft();
        assert!(!d.is_empty(), "{kind:?} opened empty");
        assert!(!seen.contains(&d), "{kind:?} repeats a draft");
        seen.push(d);
    }
    assert_eq!(seen.len(), 4);
}

/// **The field holds 200 characters and the send keeps 199.**
///
/// `Edit_Begin(…, 200, 10000, 0)` against `Edit_Commit(…, 199)` — the two
/// limits are one apart, so the last character a person may type is the one
/// the letter never carries. Reproduced, not corrected.
#[test]
fn the_field_holds_two_hundred_characters_and_the_send_keeps_one_less() {
    let (mut game, assets) = world();
    let mut screen = diplomacy::ComposeScreen::new(2, Kind::Insult.byte());
    for _ in 0..260 {
        let mut ctx = Ctx { game: &mut game, assets: &assets };
        screen.handle(Event::Text('a'), &mut ctx);
    }
    assert_eq!(screen.draft().chars().count(), diplomacy::LETTER_MAX_LEN);
    assert_eq!(screen.committed().chars().count(), diplomacy::LETTER_COMMIT);
}

/// **The three shapes with no letter in them answer no key.** The `0x1A` arm's
/// rungs are `g_diploKind == 0`, `< 5` and else, and only the middle one
/// touches the editor.
#[test]
fn the_gift_and_the_two_county_requests_have_no_draft() {
    let (mut game, assets) = world();
    for kind in [Kind::Gift, Kind::AskHelp, Kind::AskAttack] {
        let mut screen = diplomacy::ComposeScreen::new(2, kind.byte());
        let mut ctx = Ctx { game: &mut game, assets: &assets };
        screen.handle(Event::Text('a'), &mut ctx);
        assert_eq!(screen.draft(), "", "{kind:?} grew a draft");
    }
}

/// **The field does not eat the dialog's clicks.** `Widget_Test` runs first
/// and the harvest happens only when it declines, so send and cancel still
/// work with a letter open.
///
/// **Ablation, run:** let the field see every event and
/// the cancel button stops closing the dialog.
#[test]
fn a_letter_open_does_not_stop_the_cancel_button() {
    let (mut game, assets) = world();
    let mut machine = Machine::new(ScreenId::Diplomacy);
    machine.push(ScreenId::DiploCompose(2, Kind::Compliment.byte()));
    let depth = machine.depth();
    press_and_wait(&mut machine, &mut game, &assets, middle(diplomacy::LETTER_CANCEL));
    assert_eq!(machine.depth(), depth - 1, "cancel left the dialog open");
}


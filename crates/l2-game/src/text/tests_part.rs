#![allow(unused_imports)]
use super::*;
use super::player_name::*;
use l2_view::Canvas;
use crate::input::{Event, Key};

#[cfg(test)]
mod tests {
    use super::*;

    /// Every character four pixels wide, so the pixel limit is a character
    /// count and a test can say what it means.
    struct Four;
    impl Metrics for Four {
        fn advance(&self, _c: char) -> i32 {
            4
        }
    }

    fn text(seed: &str, max: usize) -> TextField {
        TextField::begin(seed, max, 10_000, Kind::Text)
    }

    fn typed(f: &mut TextField, s: &str) {
        for c in s.chars() {
            f.type_char(c, &Four);
        }
    }

    /// **The default is overwrite, and this is the reported bug's real shape.**
    #[test]
    fn a_seeded_field_overwrites_because_that_is_where_the_original_starts() {
        let mut f = text("Player1", 16);
        assert!(!f.inserting(), "g_editInsert is zero in a fresh game");
        typed(&mut f, "Ed");
        assert_eq!(f.text(), "Edayer1", "overwrite, not insert");
        // And the Insert key is the way out of it.
        let mut f = text("Player1", 16);
        f.toggle_insert();
        typed(&mut f, "Ed");
        assert_eq!(f.text(), "EdPlayer1");
    }

    /// Typing past the end of a seeded field extends it, because the original
    /// is writing into cleared buffer
    #[test]
    fn overwriting_past_the_end_extends_the_text() {
        let mut f = text("ab", 16);
        f.end();
        typed(&mut f, "cd");
        assert_eq!(f.text(), "abcd");
    }

    /// **The character filter, both kinds.** Every row is a byte range out of
    /// `Edit_TypeChar`.
    #[test]
    fn the_filter_is_the_originals_seven_ranges() {
        let mut f = text("", 64);
        typed(&mut f, "Ab9 -,.?!");
        assert_eq!(f.text(), "Ab9 -,.?!", "text kind keeps its four punctuation marks");

        let mut f = text("", 64);
        typed(&mut f, "O'Neill (the 3rd) & co_");
        assert_eq!(
            f.text(),
            "ONeill the 3rd  co",
            "apostrophe, brackets, ampersand and underscore are dropped in silence"
        );

        let mut f = TextField::begin("", 64, 10_000, Kind::Filename);
        typed(&mut f, "MySave.sav");
        assert_eq!(f.text(), "mysavesav", "a filename lower-cases and refuses the dot");

        // The three high runs, which are the accented letters and are accepted
        // by both kinds.
        let mut f = TextField::begin("", 64, 10_000, Kind::Filename);
        typed(&mut f, "\u{80}\u{9a}\u{a0}\u{a7}\u{e1}\u{9b}\u{a8}\u{e0}");
        assert_eq!(
            f.text(),
            "\u{80}\u{9a}\u{a0}\u{a7}\u{e1}",
            "0x9B, 0xA8 and 0xE0 are between the runs and are refused"
        );
    }

    /// The two limits
    #[test]
    fn typing_stops_at_the_character_limit_and_at_the_pixel_limit() {
        let mut f = text("", 4);
        typed(&mut f, "abcdefg");
        assert_eq!(f.text(), "abcd", "max_len 4");
        assert_eq!(f.state(), State::Full);
        f.backspace(&Four);
        assert_eq!(f.text(), "abc");
        f.type_char('z', &Four);
        assert_eq!(f.text(), "abcz", "a delete reopens the field");

        // Sixteen characters allowed, but only twenty pixels of them: at four
        // pixels each the fifth is refused, because the test is `>=`.
        let mut f = TextField::begin("", 16, 20, Kind::Text);
        typed(&mut f, "abcdefgh");
        assert_eq!(f.text(), "abcde", "width 20 >= max 20 stops the sixth");
        assert_eq!(f.state(), State::Full);
    }

    /// Backspace deletes to the left, Delete to the right, and neither moves
    /// the other's character.
    #[test]
    fn the_caret_keys_are_the_window_procedures_six() {
        let mut f = text("abcd", 16);
        f.end();
        assert_eq!(f.caret(), 4);
        f.right();
        assert_eq!(f.caret(), 4, "Edit_Right stops at the length");
        f.left();
        f.left();
        assert_eq!(f.caret(), 2);
        f.backspace(&Four);
        assert_eq!((f.text().as_str(), f.caret()), ("acd", 1));
        f.delete(&Four);
        assert_eq!((f.text().as_str(), f.caret()), ("ad", 1));
        f.home();
        assert_eq!(f.caret(), 0);
        f.left();
        assert_eq!(f.caret(), 0, "and Edit_Left stops at zero");
    }

    /// The caret x is the width of the text before it, which is what
    /// `Ui_DrawText` captures while it draws.
    #[test]
    fn the_caret_sits_after_the_characters_before_it() {
        let mut f = text("abcd", 16);
        assert_eq!(f.caret_x(&Four), 0);
        f.right();
        f.right();
        assert_eq!(f.caret_x(&Four), 8);
        f.end();
        assert_eq!(f.caret_x(&Four), 16);
    }

    /// Eight ticks lit in seventeen.
    #[test]
    fn the_caret_blinks_on_the_originals_period() {
        let mut f = text("", 16);
        let lit = (0..BLINK_PERIOD)
            .filter(|_| {
                f.tick();
                f.caret_lit()
            })
            .count();
        assert_eq!(lit, 8, "counter 9..=16 of 0..=16");
    }

    /// `Edit_Commit`'s truncation is the destination's size, and it is larger
    /// than the edit limit everywhere the game uses it.
    #[test]
    fn the_commit_limit_is_not_the_edit_limit() {
        let mut f = text("", 16);
        typed(&mut f, "Aethelred The Un");
        assert_eq!(f.text().len(), 16);
        assert_eq!(f.commit(31), f.text(), "31 is g_options' name field");
        assert_eq!(f.commit(4), "Aeth");
    }
}


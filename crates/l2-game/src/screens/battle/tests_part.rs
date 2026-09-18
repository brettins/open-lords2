#![allow(unused_imports)]
use super::*;
use super::render::*;
use super::prompt::*;
use super::result::*;
use l2_kingdom::battle::Outcome;
use l2_kingdom::unit::ALL_TROOP_TYPES;
use l2_view::chrome::system;
use l2_view::{text, Canvas};
use crate::engagement::{Answer, Roster};
use crate::input::{Event, Rect};
use crate::press::{Press, Widget};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::shell::{self, font, Pen};
use crate::turn::{self, Question, TurnStep};

#[cfg(test)]
mod tests {
    use super::*;

    /// Every one of `L2.eng` group 82's seven pairs is reachable, and each maps
    /// to the index the original reads.
    #[test]
    fn all_seven_outcome_banners_are_reachable_and_distinct() {
        use l2_kingdom::battle::{outcome, Verdict};
        let a_won = Verdict::a_won(1, 2);
        let b_won = Verdict::b_won(1, 2);
        let cases = [
            (false, 1u8, 1u8, 2u8, a_won, Outcome::Won),
            (false, 2, 1, 2, a_won, Outcome::Lost),
            (true, 1, 1, 2, a_won, Outcome::SiegeWon),
            (true, 1, 1, 2, b_won, Outcome::SiegeLifted),
            (true, 2, 1, 2, a_won, Outcome::CastleLost),
            (true, 2, 1, 2, b_won, Outcome::SiegeLost),
            (false, 9, 1, 2, a_won, Outcome::Bystander),
        ];
        let mut pairs = std::collections::BTreeSet::new();
        for (siege, local, winner, loser, verdict, want) in cases {
            let got = outcome(verdict, siege, local, winner, loser);
            assert_eq!(got, want, "siege={siege} local={local}");
            assert!(pairs.insert(got.pair()), "{want:?} shares a pair with another");
            assert!(got.pair() * 2 + 1 < 14);
        }
        assert_eq!(pairs.len(), 7, "all seven, and no two the same");
        assert!(ours_banner(Outcome::Bystander).contains("CONFLICT"));
    }

    #[test]
    fn both_thumbs_and_the_corner_are_inside_the_window() {
        let w = window();
        for r in [widget_rect(TAKE_THE_FIELD), widget_rect(DECLINE), ok_rect()] {
            assert!(r.x >= w.x && r.x + r.w <= w.x + w.w, "{r:?} escapes in x");
            assert!(r.y >= w.y && r.y + r.h <= w.y + w.h, "{r:?} escapes in y");
        }
        let (a, b) = (widget_rect(TAKE_THE_FIELD), widget_rect(DECLINE));
        assert!(a.x + a.w <= b.x, "the thumbs overlap: {a:?} {b:?}");
        assert_eq!((a.x, a.y), (332, 116));
        assert_eq!((b.x, b.y), (372, 116));
    }

    #[test]
    fn the_roster_fits_between_the_names_and_the_corner() {
        let w = window();
        let last = ROSTER_Y + 6 * ROW_PITCH + 4;
        assert!(last < TOTAL_Y, "the rows run into the totals");
        assert!(TOTAL_Y < w.y + w.h, "the totals fall out of the window");
        assert!(ROSTER_Y > LORD_A.1, "the roster starts above the names");
        for x in [COL_A_BEFORE, COL_A_AFTER, COL_NOUN, COL_B_AFTER, COL_B_BEFORE] {
            assert!(x >= w.x && x < w.x + w.w, "column {x} is outside the window");
        }
    }
}


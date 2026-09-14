#![allow(unused_imports)]
use super::*;
use super::layout::*;
use super::queue::*;
use l2_kingdom::diplomacy::Letter;
use l2_kingdom::victory::{self, Ending, Outcome, OutcomeStep};
use crate::game::Game;

#[cfg(test)]
mod tests {
    use super::*;

    fn notice(to: u8, group: u16) -> Record {
        Record { to, group, ..Record::default() }
    }

    /// **The filter that makes this display state and not simulation state.**
    #[test]
    fn a_message_addressed_to_another_realm_never_enters_this_peers_ring() {
        let mut q = MessageQueue::new();
        assert!(q.enqueue(notice(0, 0x92), 1), "realm 0 is everybody");
        assert!(q.enqueue(notice(1, 0x92), 1), "and me");
        assert!(!q.enqueue(notice(3, 0x92), 1), "and not realm 3");
        assert_eq!(q.queued(), 2);
    }

    #[test]
    fn a_pulled_message_opens_a_window_and_empties_its_slot() {
        let mut q = MessageQueue::new();
        q.enqueue(notice(0, 0x92), 1);
        assert!(!q.is_open());
        assert!(q.pull());
        assert_eq!(q.open().map(|r| r.group), Some(0x92));
        assert_eq!(q.timer(), TIMER_START);
        assert!(q.just_opened());
        assert_eq!(q.queued(), 0, "the slot was cleared");
    }

    /// `Msg_Pump` only pulls when the timer has run out, and in single player it
    /// never does. So a second message waits behind the first.
    #[test]
    fn a_second_message_waits_behind_the_first() {
        let mut q = MessageQueue::new();
        q.enqueue(notice(0, 0x92), 1);
        q.enqueue(notice(0, 0x93), 1);
        assert!(q.pull());
        assert_eq!(q.open().map(|r| r.group), Some(0x92));
        assert!(!q.pull(), "the timer is at 2000");
        q.close();
        assert!(q.pull());
        assert_eq!(q.open().map(|r| r.group), Some(0x93));
    }

    /// **Single player never times out.** Two thousand ticks and the message is
    /// still there, clamped at 1.
    #[test]
    fn in_single_player_a_message_waits_for_ever() {
        let mut q = MessageQueue::new();
        q.enqueue(notice(0, 0x92), 1);
        q.pull();
        for _ in 0..(TIMER_START + 500) {
            assert_eq!(q.advance(false), Tick::Open);
        }
        assert!(q.is_open());
        assert_eq!(q.timer(), 1, "clamped, not expired");
    }

    /// **In a network game it goes after 399 ticks** — `2000 - 0x641`.
    #[test]
    fn in_a_network_game_a_message_expires_after_three_hundred_and_ninety_nine_ticks() {
        let mut q = MessageQueue::new();
        q.enqueue(notice(0, 0x92), 1);
        q.pull();
        let mut ticks = 0;
        while q.is_open() {
            assert_eq!(ticks < 1000, true, "it must expire");
            if q.advance(true) == Tick::TimedOut {
                break;
            }
            ticks += 1;
        }
        assert_eq!(ticks + 1, TIMER_START - MULTIPLAYER_TIMEOUT + 1);
        assert!(!q.is_open());
    }

    /// …but an **ending** does not, on either kind of game. It is the one
    /// category the multiplayer timeout exempts.
    #[test]
    fn an_ending_never_times_out_even_in_a_network_game() {
        let mut q = MessageQueue::new();
        q.enqueue(
            Record { group: 225, category: category::ENDING, ..Record::default() },
            1,
        );
        q.pull();
        for _ in 0..TIMER_START * 2 {
            assert_eq!(q.advance(true), Tick::Open);
        }
        assert!(q.is_open());
    }

    /// The tip is the other exemption, in the other direction: it goes even in
    /// single player, because the draw shortens its timer to 100.
    #[test]
    fn a_tip_closes_itself_and_has_no_button_to_close_it_with() {
        let mut q = MessageQueue::new();
        q.enqueue(
            Record { group: 0xE6, category: category::TIP, ..Record::default() },
            1,
        );
        q.pull();
        assert!(!Shape::of(category::TIP).has_ok_button());
        q.clamp_tip_timer();
        assert_eq!(q.timer(), TIP_TIMER);
        // The tip's own branch is only reached in a network game; in single
        // player the first branch clamps it to 1 and it stays up. That is the
        // original, and it is why `Msg_Pump`'s ladder tests multiplayer FIRST.
        for _ in 0..TIP_TIMER {
            q.advance(false);
        }
        assert!(q.is_open(), "single player: clamped to 1, like everything else");
    }

    /// **A gap in the ring costs one call, not a search.** `Msg_Pump` advances
    /// the tail by one and returns when it lands on an empty slot.
    #[test]
    fn an_empty_slot_costs_one_pull_each() {
        let mut q = MessageQueue::new();
        q.enqueue(notice(0, 0x92), 1);
        q.enqueue(notice(0, 0x93), 1);
        q.pull();
        q.close();
        q.pull();
        q.close();
        // Both gone; the tail is at 2 and the head at 2, so the next pull sees
        // an empty slot with tail == head and stops.
        assert!(!q.pull());
        assert!(!q.is_open());
    }

    /// The four question categories and the four widget tests are the same four.
    #[test]
    fn the_categories_that_survive_a_map_click_are_the_ones_with_an_answer() {
        for c in 0u8..=0x14 {
            let r = Record { group: 1, category: c, ..Record::default() };
            let questioned = r.is_question();
            let has_widget = matches!(
                c,
                category::GARRISON_PROMPT
                    | category::PAY_PROMPT
                    | category::ALLIANCE_PROMPT
                    | category::DIPLOMACY
            );
            assert_eq!(questioned, has_widget, "category {c:#x}");
        }
    }

    #[test]
    fn a_map_click_closes_a_notice_and_leaves_a_question_standing() {
        let mut q = MessageQueue::new();
        q.enqueue(notice(0, 0x92), 1);
        q.pull();
        assert!(q.dismiss_unless_question());
        assert!(!q.is_open());

        q.enqueue(
            Record { group: 180, category: category::ALLIANCE_PROMPT, ..Record::default() },
            1,
        );
        q.pull();
        assert!(!q.dismiss_unless_question());
        assert!(q.is_open(), "an alliance offer is not closed by a stray click");
    }

    /// The OK hit box is **twice** the picture, and off-centre by nothing:
    /// twelve pixels of slop on each side.
    #[test]
    fn the_ok_hitbox_is_forty_eight_square_round_a_twenty_four_square_button() {
        let f = frame_of(&Record { group: 1, category: category::NOTICE, ..Record::default() })
            .expect("category 0 has a constant frame");
        let (bx, by) = f.ok_button();
        assert_eq!((bx, by), (0x20 + 0x1A0 - 0x30, 0xA0 + 0xE0 - 0x30));
        let hit = f.ok_hitbox();
        assert_eq!((hit.w, hit.h), (0x30, 0x30));
        assert!(hit.contains(bx, by));
        assert!(hit.contains(bx - 12, by - 12), "twelve pixels above and left");
        assert!(hit.contains(bx + 35, by + 35), "and beyond the picture's corner");
        assert!(!hit.contains(bx - 13, by));
    }

    /// The five prompts are five distinct tables and three distinct positions,
    /// and every one is a 36-pixel step with a 4-pixel drop.
    #[test]
    fn every_prompt_is_a_thumb_up_and_a_thumb_down_thirty_six_pixels_apart() {
        for p in [
            Prompt::Garrison,
            Prompt::PayForHelp,
            Prompt::AcceptAlliance,
            Prompt::AnswerHelpRequest,
            Prompt::AnswerAttackRequest,
        ] {
            let [yes, no] = p.widgets();
            assert_eq!(no.0 - yes.0, 36, "{p:?}");
            assert_eq!(no.1 - yes.1, 4, "{p:?}");
            assert_eq!(p.hit(yes.0 + 1, yes.1 + 1), Some(true), "{p:?}");
            assert_eq!(p.hit(no.0 + 1, no.1 + 1), Some(false), "{p:?}");
            assert_eq!(p.hit(0, 0), None, "{p:?}");
        }
    }

    /// The paragraph categories are a *range* and the count is in the byte.
    #[test]
    fn the_paragraph_categories_carry_their_own_count() {
        assert_eq!(Shape::of(5), Shape::Paragraphs(1));
        assert_eq!(Shape::of(9), Shape::Paragraphs(5));
        assert_eq!(Shape::of(4), Shape::Tip);
        assert_eq!(Shape::of(10), Shape::Prompt);
    }

    /// A category nobody enqueues draws nothing **and cannot be left with the
    /// left button**, because every `Ui_OkButton` call is inside an arm.
    #[test]
    /// **The event window's height is a rule, and the corner button rides on
    /// it.** `Msg_DrawWindow`'s category-`0x0F` arm opens
    /// `DAT_00552ff8 = (short)eventId < 0x12E ? 0xC0: 0xE0;` and the group *is*
    /// the event id. The constant was documented here and not applied, so the
    /// sixteen ids from `0x12E` up drew in a box `0x20` short with their
    /// `Ui_OkButton`, and its 48 × 48 hit box, `0x20` too high.
    ///
    /// Ablation, run: `event_height(record.group)` back to a literal `0xC0` →
    /// this alone goes red.
    #[test]
    fn a_high_numbered_event_gets_the_taller_window_and_its_button_moves_with_it() {
        let at = |group: u16| {
            frame_of(&Record { group, category: category::EVENT, ..Record::default() })
                .expect("the event category has a frame")
        };
        assert_eq!(at(0x87).h, 0xC0, "Rats: the eight with a number line");
        assert_eq!(at(0x8E).h, 0xC0, "Wedding fever, the last of them");
        assert_eq!(at(0x12E).h, 0xE0, "Healthy eating: the first without one");
        assert_eq!(at(0x13D).h, 0xE0, "No songs, the last id in the deck");
        assert_eq!(at(0).h, 0xC0, "a county that never drew: the test is signed");
        assert_eq!(at(0x12E).ok_button().1 - at(0x87).ok_button().1, 0x20);
    }

    #[test]
    fn an_unhandled_category_has_no_button() {
        assert_eq!(Shape::of(0x15), Shape::Unhandled);
        assert!(!Shape::of(0x15).has_ok_button());
        assert!(frame_of(&Record { group: 1, category: 0x15, ..Record::default() }).is_none());
    }
}


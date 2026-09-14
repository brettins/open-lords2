//! **The letters an army's arrival posts, onto this peer's ring** —
//! `Unit_EnterCounty` (`0x004ABB36`), `County_GreetArmy` (`0x004ABF77`) and
//! `County_ChangeOwner` (`0x004A72FE`).
//!
//! Two reports asked for this, from two directions. A player, on build
//! `73DF34969`: *"county did not give me a message when I moved an army into
//! it."* And the films work: `County_ChangeOwner` posts its capture letters in
//! category `0x0D`, and nothing in the engine posted one, so the capture films
//! and the voices after them could never play.
//!
//! # Where the rules are, and what is left here
//!
//! The rules are `l2_kingdom`'s and the sweep reports them as it runs them —
//! [`l2_kingdom::units_tick::Posted`] from `Units_Tick`,
//! [`l2_kingdom::battle::Aftermath::captures`] from `Battle_ReturnToCampaign`.
//! What is left for this module is the one decision that belongs to a peer:
//!
//! * a **greeting** or an **invasion letter** is addressed by the world, so it
//!   goes straight through `Msg_Enqueue`'s filter, [`crate::message::MessageQueue::post`];
//! * a **capture** is not. `County_ChangeOwner` compares the taker and the loser
//!   against `g_localPlayer` to choose among thirteen letters, so
//!   [`capture_record`] makes that choice for this peer's player.
//!
//! Every letter here is posted from the event that posts it in the original:
//! the crossing, the town, and the end of the battle. None of them changes the
//! world from this side of the seam — the one posting that does, the invasion
//! letter's voice rotation, happens inside the simulation, in
//! [`l2_kingdom::arrival::enter_county`].
//!
//! # What the window, the films and the voice do with them
//!
//! Nothing new. The categories are the original's, so the message scroll lays
//! each out by its category and [`crate::audio`] voices each by its category
//! and group — `ff_capt.wav` for the three notices `0x72`…`0x74`, the trumpet
//! and a ninety-tick voice for a capture —
//! categories that had senders.
//!
//! # The words
//!
//! `CLAUDE.md` rule 6. Every group posted here is drawn from the player's own
//! `L2.eng`; [`TEXT`] is our transcription, used only where the file gave
//! nothing, and `tests/arrival.rs` holds it against the file string for string.

use l2_kingdom::conquest::Capture;
use l2_kingdom::units_tick::Posted;

use crate::game::Game;
use crate::message::{category, Record};

/// The capture letters' `L2.eng` groups — message id and group are one number.
pub mod group {
    /// 114 — *"This enemy shire has a new ruler…"*: one lord took another's.
    pub const NEW_RULER: u16 = 0x72;
    /// 115 — *"Our county is lost!"*
    pub const LOST: u16 = 0x73;
    /// 116 — *"This poor, defenseless county has been ruthlessly occupied…"*
    pub const OCCUPIED: u16 = 0x74;
    /// 117 — *"Bravo!!"*, the first county past a peak of one.
    pub const BRAVO: u16 = 0x75;
    /// 118 — *"…a solid base."*
    pub const SOLID_BASE: u16 = 0x76;
    /// 119, 120, 121, 122 — the share-of-map rungs below 26, 41, 61 and 81 %.
    pub const SHARE_26: u16 = 0x77;
    pub const SHARE_41: u16 = 0x78;
    pub const SHARE_61: u16 = 0x79;
    pub const SHARE_81: u16 = 0x7A;
    /// 123 — *"One more county and the crown is yours!"*
    pub const ONE_MORE: u16 = 0x7B;
    /// 125 — *"Another county falls before you…"*, 81 % and above.
    pub const SHARE_TOP: u16 = 0x7D;
    /// 126 — *"The county is yours. May you rule it wisely."* — no new peak.
    pub const RULE_WISELY: u16 = 0x7E;
    /// 129 — *"This county is too far from the heart of your lands…"*. The
    /// `else` branch, to the taker alone: see [`super::capture_record`].
    pub const TOO_FAR: u16 = 0x81;
}

/// **`County_ChangeOwner`'s letter for one peer's player**, or `None`.
///
/// `[V]`, `0x004A72FE`, with `countyCount` the taker's recount plus one and
/// `share` `Realm_UpdateTotals`' `PctOf(recount, g_countyCount)` — the share
/// **before** this county, because the recount runs before the increment:
///
/// | who the player is | letter | category | from | spare |
/// |---|---|---|---|---|
/// | the taker, `countyCount == g_countyCount - 1` | 123 | `0x0D` | 0 | loser |
/// | the taker, past his peak, peak < 2 | 117 | `0x0D` | 0 | loser |
/// | … peak < 3 | 118 | | | |
/// | … share < 26 / 41 / 61 / 81 | 119 / 120 / 121 / 122 | | | |
/// | … share ≥ 81 | 125 | | | |
/// | the taker, not past his peak | 126 | | | |
/// | the loser | 115 | 0 | taker | 0 |
/// | anyone else, the county was neutral | 116 | 0 | taker | 0 |
/// | anyone else | 114 | 0 | taker | loser |
///
/// **Group 124 is never posted** — no call site passes it — so the *"You are the
/// Lord of the Realm"* capture letter is dead text, and the ladder's rung for
/// the last county is 125 or 126 like any other.
///
/// **An ungovernable county is 129 to its taker and nothing to anybody else.**
/// `County_ChangeOwner`'s `else` branch has one `Msg_Enqueue` and it is inside
/// `if (newOwner == g_localPlayer)`, so the loser is never told his county went
/// its own way — he finds out from the map. The county itself is made
/// independent by [`l2_kingdom::conquest::change_owner`].
pub fn capture_record(capture: &Capture, player: u8, county_count: usize) -> Option<Record> {
    if !capture.governable {
        // ```c
        // if (newOwner == g_localPlayer) Msg_Enqueue(0, g_localPlayer, 0x81, 0, '\0', county, '\0', 0);
        // County_MakeIndependent(county);
        // ```
        // **Only the taker is told.** The loser is not: the county he lost is
        // nobody's now and no letter in the ladder says so, which is the
        // original's silence and not ours.
        if capture.new_owner != player {
            return None;
        }
        return Some(Record {
            to: player,
            from: 0,
            group: group::TOO_FAR,
            variant: 0,
            category: category::NOTICE,
            county: capture.county,
            spare: 0,
            payload: 0,
        });
    }
    let from_the_taker = |group: u16, spare: u8| Record {
        to: player,
        from: capture.new_owner,
        group,
        variant: 0,
        category: category::NOTICE,
        county: capture.county,
        spare,
        payload: 0,
    };
    if capture.new_owner == player {
        let held = capture.held_after() as u32;
        let peak = capture.peak_before;
        let group = if held == (county_count as u32).wrapping_sub(1) {
            group::ONE_MORE
        } else if (peak as u32) < held {
            let share = if capture.held_before == 0 {
                0
            } else {
                l2_kingdom::industry::pct_of(capture.held_before as i32, county_count as i32)
            };
            if peak < 2 {
                group::BRAVO
            } else if peak < 3 {
                group::SOLID_BASE
            } else if share < 26 {
                group::SHARE_26
            } else if share < 41 {
                group::SHARE_41
            } else if share < 61 {
                group::SHARE_61
            } else if share < 81 {
                group::SHARE_81
            } else {
                group::SHARE_TOP
            }
        } else {
            group::RULE_WISELY
        };
        // `Msg_Enqueue(0, g_localPlayer, group, 0, '\r', county, g_counties[county].owner, 0)`.
        return Some(Record {
            to: player,
            from: 0,
            group,
            variant: 0,
            category: category::CAPTURE,
            county: capture.county,
            spare: capture.old_owner,
            payload: 0,
        });
    }
    Some(if capture.old_owner == player {
        // `Msg_Enqueue(newOwner, g_counties[county].owner, 0x73, 0, 0, county, 0, 0)`.
        from_the_taker(group::LOST, 0)
    } else if capture.old_owner == 0 {
        from_the_taker(group::OCCUPIED, 0)
    } else {
        from_the_taker(group::NEW_RULER, capture.old_owner)
    })
}

/// **One unit sweep's letters**, in the order the sweep made them.
pub fn post(game: &mut Game, posted: &[Posted]) {
    let player = game.player;
    for p in posted {
        match p {
            // `Msg_Enqueue` keeps it only when it is to this peer's player.
            Posted::Letter(letter) => {
                game.messages.post(*letter, player);
            }
            Posted::Capture(capture) => post_capture(game, capture),
        }
    }
}

/// **A settled battle's captures** — `Battle_ReturnToCampaign`'s one or two
/// `County_ChangeOwner` calls, in call order.
pub fn post_captures(game: &mut Game, captures: &[Option<Capture>]) {
    for capture in captures.iter().flatten() {
        post_capture(game, capture);
    }
}

fn post_capture(game: &mut Game, capture: &Capture) {
    let player = game.player;
    if let Some(record) = capture_record(capture, player, game.kingdom.county_count) {
        game.messages.enqueue(record, player);
    }
}

// ------------------------------------------------------------------ the words

/// **Our transcription of every group this module posts**, for an install whose
/// `L2.eng` cannot be read. Index 0 is the group's own label — `FREE` for the
/// letters whose heading is the county's name — and the rest are the bodies.
/// `tests/arrival.rs` holds it against the player's file, string for string.
pub const TEXT: &[(u16, &[&str])] = &[
    (
        114,
        &[
            "FREE",
            "This enemy shire has a new ruler, as the pretenders to the crown squabble over land.",
        ],
    ),
    (115, &["FREE", "Our county is lost! The town was overrun by enemy troops."]),
    (
        116,
        &[
            "FREE",
            "This poor, defenseless county has been ruthlessly occupied by one of your enemies.",
        ],
    ),
    (
        117,
        &[
            "FREE",
            "Bravo!! The county has fallen to our troops.   You have made an excellent start in your bid to become the King.",
        ],
    ),
    (
        118,
        &[
            "FREE",
            "The addition of this new shire gives you a solid base. You should now consolidate your gains, my Lord.",
        ],
    ),
    (
        119,
        &[
            "FREE",
            "All must fall before you, it seems. Your growing tally of lands is being whispered of in other halls.",
        ],
    ),
    (
        120,
        &[
            "FREE",
            "Another conquest, my Lord - but will you hold on to it? For now, congratulations are the order of the day.",
        ],
    ),
    (
        121,
        &[
            "FREE",
            "Your domain increases. With skill, vigilance and a little luck, you will soon be poised to strike for the crown.",
        ],
    ),
    (
        122,
        &[
            "FREE",
            "The addition of this shire to your lands brings the whole country within your grasp.",
        ],
    ),
    (
        123,
        &[
            "FREE",
            "This county all but seals your conquest. One more county and the crown is yours!",
        ],
    ),
    (125, &["FREE", "Another county falls before you as your influence grows across the realm."]),
    (126, &["FREE", "The county is yours. May you rule it wisely."]),
    (
        129,
        &[
            "FREE",
            "This county is too far from the heart of your lands, my Lord. You cannot govern it.",
        ],
    ),
    (
        130,
        &[
            "FREE",
            "The people are wretched, my liege. We beg that you journey to our town and help us out of our misfortune.",
        ],
    ),
    (
        131,
        &[
            "FREE",
            "We welcome you to our humble county, noble lord. We hope your journey through it will be a pleasant one.",
        ],
    ),
    (
        132,
        &[
            "FREE",
            "We have had no notification of your army's passage into our county. Please ensure that your men pass swiftly through, as their presence here disturbs the people.",
        ],
    ),
    (
        133,
        &[
            "FREE",
            "The presence of your men at arms in our county is unacceptable. Remove your troops now, or face the consequences!",
        ],
    ),
    (
        134,
        &["FREE", "Your violation of our borders is an outrage. You will pay for these bully-boy tactics..."],
    ),
    (
        170,
        &[
            "Invasion of",
            "\"I intend to help myself to some of your land, fool, and you are powerless to stop me!\"",
            "\"Your people are unfortunate to be ruled by you. When I take your lands they will finally know greatness!\"",
            "\"You are a coward and a weakling. Invading you will be a pleasure.\"",
            "\"I am a powerful warrior and you are not. Surrender to me now and maybe I will let you live!\"",
            "\"This county is vital to my plans. Please relinquish control of it to my lads or face the consequences.\"",
            "\"Sir, this county is part of my ancestral lands and I hereby claim it by right of birth.  Yield or face the might of my army.\"",
            "\"Begone!  You have occupied my lands for far too long. \"",
            "\"All must fall before the might and right of my cause.  You shall be the first.\"",
            "\"I am intent on restoring my royal banner to this county, that has for so long been without its rightful sovereign.\"",
            "\"The presence of your troops in my lands is most unwelcome.  It is time I removed them!\"",
            "\"I grow angrier with every second of your insolence.  Shortly this will be fixed when I reclaim my lands!\"",
            "\"You have governed my lands long enough.  I will take back what is rightfully mine!\"",
            "\"My troops have entered your county on a purely fact-finding mission, my friend. Please do not be alarmed.\"",
            "\"Out of necessity I have been forced to seize a portion of your realm, Lord. I apologize for the inconvenience.\"",
            "\"By the Church's authority, I hereby seize a portion of your land as my own. Kindly keep your distance.\"",
            "\"The presence of my troops in your county has been deemed necessary by the Church Council. Do not protest--- this action is in everyone's best interests.\"",
        ],
    ),
];

/// Our transcription of `(group, index)`, or `""`.
pub fn transcribed(group: u16, index: usize) -> &'static str {
    TEXT.iter()
        .find(|(g, _)| *g == group)
        .and_then(|(_, s)| s.get(index))
        .copied()
        .unwrap_or("")
}

/// **`Eng_DrawString(group, index)`**: the player's own `L2.eng`, and our
/// transcription only where the file gave nothing.
pub fn words(shell: &crate::shell::ShellAssets, group: u16, index: usize) -> String {
    let s = shell.text(group as usize, index);
    if s.is_empty() {
        transcribed(group, index).to_string()
    } else {
        s.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn taken(new_owner: u8, old_owner: u8, held_before: u8, peak_before: u8) -> Capture {
        Capture { new_owner, old_owner, county: 5, held_before, peak_before, governable: true, penalty: 10, garrison: None }
    }

    /// **Every rung of the taker's ladder, and both edges of each share band.**
    /// A map of a hundred counties so the percentages are the counts; the peak
    /// is set equal to the holding so each capture is a new high.
    ///
    /// The 25 and 26 rows are the ablation for *"the share before the county"*:
    /// computed after the increment, 25 held becomes 26 % and the first row
    /// reads 120. The thresholds are typed here, not read from the function.
    #[test]
    fn the_taker_is_sent_the_rung_his_holding_and_his_peak_choose() {
        let cases: [(u8, u8, u16, &str); 14] = [
            (0, 0, 117, "a realm with nothing: past a peak of nothing"),
            (1, 1, 117, "Bravo!!"),
            (2, 2, 118, "a solid base"),
            (3, 3, 119, "3 %"),
            (25, 25, 119, "25 %, measured before this county"),
            (26, 26, 120, "26 %"),
            (40, 40, 120, "40 %"),
            (41, 41, 121, "41 %"),
            (60, 60, 121, "60 %"),
            (61, 61, 122, "61 %"),
            (80, 80, 122, "80 %"),
            (81, 81, 125, "81 % and up is 125, not 124"),
            (98, 98, 123, "the ninety-ninth of a hundred: one more and the crown"),
            (5, 9, 126, "back up to a holding already reached once"),
        ];
        for (held, peak, want, why) in cases {
            let r = capture_record(&taken(1, 3, held, peak), 1, 100).expect("the taker is told");
            assert_eq!(r.group, want, "{why}");
            assert_eq!(
                (r.to, r.from, r.category, r.county, r.spare, r.variant),
                (1, 0, category::CAPTURE, 5, 3, 0),
                "{why}"
            );
        }
        // `countyCount == g_countyCount - 1` is tested before the peak, so it
        // wins even for a holding that is not a new high.
        assert_eq!(capture_record(&taken(1, 0, 98, 120), 1, 100).map(|r| r.group), Some(123));
    }

    /// **The loser and everybody else are told in a plain notice, from the
    /// taker** — and only the new-ruler notice names the loser.
    #[test]
    fn the_loser_and_the_onlookers_are_told_in_a_notice_from_the_taker() {
        for (old, want, spare) in [(1u8, 115u16, 0u8), (0, 116, 0), (3, 114, 3)] {
            let r = capture_record(&taken(2, old, 4, 4), 1, 14).expect("everyone is told");
            assert_eq!(
                (r.to, r.from, r.group, r.category, r.county, r.spare),
                (1, 2, want, category::NOTICE, 5, spare),
                "old owner {old}"
            );
        }
    }

    /// **129 to the taker, silence to everybody else** — the whole of the
    /// `else` branch's one `Msg_Enqueue`, which sits inside
    /// `if (newOwner == g_localPlayer)`. `from` is a literal 0 here and not the
    /// taker, unlike the three notices above.
    ///
    /// Ablation: return `None` for `!governable`, as this did before the branch
    /// was built, and the first assertion goes red.
    #[test]
    fn an_ungovernable_county_tells_its_taker_and_nobody_else() {
        let far = Capture { governable: false, ..taken(1, 4, 3, 3) };
        let r = capture_record(&far, 1, 14).expect("the taker is told he cannot govern it");
        assert_eq!(
            (r.to, r.from, r.group, r.category, r.county, r.spare, r.variant),
            (1, 0, group::TOO_FAR, category::NOTICE, 5, 0, 0),
        );
        assert_eq!(capture_record(&far, 4, 14), None, "the loser is not told");
        assert_eq!(capture_record(&far, 2, 14), None, "nor is anybody else");
    }

    #[test]
    fn a_word_the_file_gives_is_the_files_and_a_silent_file_falls_back_to_ours() {
        let shell = crate::game::Assets::placeholder().shell;
        assert_eq!(words(&shell, 115, 1), "Our county is lost! The town was overrun by enemy troops.");
        assert_eq!(words(&shell, 170, 0), "Invasion of");
        assert_eq!(words(&shell, 124, 1), "", "124 is never posted, so it is not transcribed");
    }
}

//! **A campaign, and how one ends.** The half of the ending chain that is not a
//! rule.
//!
//! `l2_kingdom::victory` has the rules: who is eliminated, who wins, and what
//! `DAT_0053F0C4` becomes. This module has the session state those rules are read
//! against — the campaign counter, the ending messages waiting to be shown, and
//! the step into screen `0x1C`.
//!
//! # A campaign is eight maps, and *nothing* carries between them
//!
//! That question was worth asking and the answer is smaller than it looks. When
//! the person presses OK on the conquest screen, `FUN_004329EC` calls
//! **`Game_NewGame`** — the same function the front end calls. No gold, no
//! armies, no counties, no diplomacy and no realm records survive a map boundary.
//! Three globals do, and they are the whole of a campaign's memory:
//!
//! | global | here | what it is |
//! |---|---|---|
//! | `DAT_0053F258` | [`Campaign::map`] | how many maps have been won; also the index into the table below |
//! | `DAT_0053F640` | [`Campaign::track`] | which of the two campaigns |
//! | `DAT_0053F0C4` | [`Campaign::outcome`] | how the map just played ended |
//!
//! So "finish a campaign" is "win eight scenarios in a row", and a scenario
//! ending is a whole ending. Nothing in this workspace has to model continuity
//! across a boundary because the original models none.
//!
//! # The campaign table, read out of the binary
//!
//! **[V]** — the bytes at `0x004D8E18` and `0x004D8F58` in a GOG `Lords2.exe`,
//! eight 32-bit words per entry, read by `FUN_00499E5D`:
//!
//! ```text
//! +0x00 -> g_scenarioIndex   +0x10 -> DAT_0053F27C
//! +0x04 -> g_optDifficulty   +0x14 -> DAT_0053F270
//! +0x08 -> DAT_0053F278      +0x18 -> DAT_0053F274
//! +0x0C -> DAT_0053F280      +0x1C -> DAT_0053F268
//! ```
//!
//! The first three are named because their values name them: column 0 indexes
//! `L2.eng` group 101 and produces *Quaintville, Rose, Ireland, Italy, England,
//! France, Crusades, Germany* — a campaign ladder — column 1 is 0, 0, 1, 1, 2, 2,
//! 2, 2, and column 2 is 5000 / 2500 / 1000, which is exactly the starting-gold
//! dropdown's own table at `0x004DBC18`. Column 3 runs 1, 1, 2, 3, 4, 4, 5, 5 and
//! is **[D]** the opponent count — five is the most realms there are — but it is
//! not verified and is carried unnamed. Columns 4 to 7 are carried and not
//! guessed at.
//!
//! **The second track starts at index 2**, which is not a typo:
//! `FUN_00433461` sets `DAT_0053F258 = 2` when the campaign hotspot is 1 and 0
//! otherwise, and track B's rows 0 and 1 are the same bytes as track A's zero
//! padding. So track B is *six* maps and still ends on the same `< 8` test.
//!
//! **The zero padding is load-bearing.** On the eighth win `FUN_00497879`
//! increments the counter to 8 and `FUN_00499E5D` then reads entry **8** — one
//! past the eight real rows. Entries 8 and 9 of both tracks are all zeros, so
//! that read yields scenario 0 and a harmless `Map_LoadPlanes(0)` behind the
//! end-of-campaign screen rather than garbage. Verified by reading the bytes, not
//! assumed.

use l2_kingdom::victory::{self, Ending, Outcome, OutcomeStep, Ranking};

/// How many maps a campaign is. `FUN_0041E1DD` and the screen `0x1C` click
/// handler both test `DAT_0053F258 < 8`.
pub const CAMPAIGN_LENGTH: usize = 8;

/// One row of the campaign table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CampaignMap {
    /// `g_scenarioIndex` — the map slot, and the `L2.eng` group 101 index.
    pub scenario: usize,
    /// `g_optDifficulty`, 0..=2.
    pub difficulty: u8,
    /// `DAT_0053F278` — the starting treasury, 5000 / 2500 / 1000.
    pub gold: i32,
    /// `DAT_0053F280`, `DAT_0053F27C`, `DAT_0053F270`, `DAT_0053F274`,
    /// `DAT_0053F268`, in the order `FUN_00499E5D` assigns them. Carried
    /// verbatim and deliberately unnamed — see the module docs.
    pub options: [i32; 5],
}

/// Which of the two campaigns. `DAT_0053F640`, set from the hotspot the person
/// clicked on setup page 4.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Track {
    /// `DAT_004D8E18`, entered at index 0. Eight maps.
    #[default]
    First,
    /// `DAT_004D8F58`, entered at index **2**. Six maps.
    Second,
}

const fn map(scenario: usize, difficulty: u8, gold: i32, options: [i32; 5]) -> CampaignMap {
    CampaignMap { scenario, difficulty, gold, options }
}

/// `DAT_004D8E18` rows 0..=7 — Quaintville, Rose, Ireland, Italy, England,
/// France, Crusades, Germany.
pub const TRACK_FIRST: [CampaignMap; CAMPAIGN_LENGTH] = [
    map(17, 0, 5000, [1, 1, 1, 0, 1]),
    map(12, 0, 2500, [1, 1, 1, 0, 2]),
    map(2, 1, 5000, [2, 1, 1, 0, 3]),
    map(5, 1, 2500, [3, 0, 1, 0, 4]),
    map(0, 2, 5000, [4, 2, 1, 0, 4]),
    map(3, 2, 2500, [4, 1, 1, 0, 4]),
    map(7, 2, 1000, [5, 0, 1, 0, 4]),
    map(4, 2, 1000, [5, 0, 1, 0, 4]),
];

/// `DAT_004D8F58` rows **2..=7** — Australia, Central Am., S. America, U.S.A.,
/// Imperium, The World. Rows 0 and 1 are the zero padding this track is never
/// entered at.
pub const TRACK_SECOND: [CampaignMap; CAMPAIGN_LENGTH] = [
    map(0, 0, 0, [0; 5]),
    map(0, 0, 0, [0; 5]),
    map(52, 2, 5000, [2, 1, 1, 0, 1]),
    map(53, 2, 2500, [3, 0, 1, 0, 1]),
    map(45, 2, 2500, [4, 2, 1, 0, 2]),
    map(44, 2, 2500, [4, 1, 1, 0, 3]),
    map(41, 2, 1000, [5, 0, 1, 0, 4]),
    map(42, 2, 1000, [5, 0, 1, 0, 4]),
];

impl Track {
    /// The index `FUN_00433461` starts this track's counter at.
    pub fn first_map(self) -> usize {
        match self {
            Track::First => 0,
            Track::Second => 2,
        }
    }

    pub fn table(self) -> &'static [CampaignMap; CAMPAIGN_LENGTH] {
        match self {
            Track::First => &TRACK_FIRST,
            Track::Second => &TRACK_SECOND,
        }
    }

    /// How many maps this track actually asks for.
    pub fn length(self) -> usize {
        CAMPAIGN_LENGTH - self.first_map()
    }
}

/// A campaign in progress, and the ending messages the current map has raised.
///
/// Everything here is plain data with no handles in it, for the reason
/// [`crate::game::Game`] gives about itself.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Campaign {
    /// `DAT_0053F640`.
    pub track: Track,
    /// `DAT_0053F258` — how many maps of this track have been won, and the index
    /// of the one being played. Runs from [`Track::first_map`] to
    /// [`CAMPAIGN_LENGTH`].
    pub map: usize,
    /// `DAT_0053F0C4`.
    pub outcome: Outcome,
    /// The ending messages raised and not yet shown, oldest first.
    ///
    /// The original has one message ring for everything; this is only the
    /// category-`0x0E`-and-friends subset the ending chain raises, because
    /// nothing else in this workspace has a queue yet. When one lands, this
    /// becomes a filter over it.
    pub pending: Vec<Ending>,
    /// What the last `Score_RankRealms` left behind. [`l2_kingdom::victory::outcome_of`]
    /// reads `opponents_remaining` out of it.
    pub ranking: Ranking,
}

impl Campaign {
    pub fn new(track: Track) -> Campaign {
        Campaign { track, map: track.first_map(), ..Campaign::default() }
    }

    /// The map now being played, or `None` once the campaign is over.
    pub fn current(&self) -> Option<CampaignMap> {
        if self.map >= CAMPAIGN_LENGTH {
            return None;
        }
        Some(self.track.table()[self.map])
    }

    /// Whether the eighth map has been won. `DAT_0053F258 < 8`, negated.
    pub fn is_complete(&self) -> bool {
        self.map >= CAMPAIGN_LENGTH
    }

    /// Raise one ending message. `Msg_Enqueue`.
    pub fn raise(&mut self, msg: Ending) {
        self.pending.push(msg);
    }

    /// Show the queued messages, in order, until one ends the game.
    ///
    /// This is `Msg_DrawWindow`'s category-`0x0E` arm run over the queue.
    /// **The game stops at the first message that sets a non-zero outcome**,
    /// because in the original dismissing that message is what enters screen
    /// `0x1C` — nothing behind it in the ring is ever shown.
    ///
    /// [`OutcomeStep::EnqueueVictory`] appends group 225 to the queue and carries
    /// on, exactly as the original does, so the victory arrives one message later
    /// rather than immediately.
    ///
    /// Returns the outcome, which is also left in [`Campaign::outcome`].
    pub fn settle(&mut self, local_player: u8) -> Outcome {
        let mut i = 0;
        while i < self.pending.len() {
            let msg = self.pending[i];
            i += 1;
            if !msg.sets_outcome() {
                continue;
            }
            match victory::outcome_of(msg, local_player, self.ranking) {
                OutcomeStep::Set(o) => {
                    self.outcome = o;
                    if o.is_over() {
                        self.pending.drain(..i);
                        return o;
                    }
                }
                OutcomeStep::EnqueueVictory => {
                    self.outcome = Outcome::InPlay;
                    self.pending.push(victory::victory_message(local_player));
                }
            }
        }
        self.pending.clear();
        self.outcome
    }

    /// `FUN_00497879` — the step from the last message into screen `0x1C`.
    ///
    /// **A win advances the counter and a loss does not**, which is the whole
    /// rule for replaying a map: on a loss `FUN_00499E5D` reloads the same table
    /// entry and the person fights the same country again.
    ///
    /// Returns which of the conquest screen's three branches to draw.
    pub fn enter_conquest_screen(&mut self) -> ConquestBranch {
        if self.outcome == Outcome::Won {
            self.map += 1;
        }
        self.branch()
    }

    /// Which of `L2.eng` group 36's three sentences applies now.
    pub fn branch(&self) -> ConquestBranch {
        if self.is_complete() {
            ConquestBranch::Finished
        } else if self.outcome == Outcome::Won {
            ConquestBranch::Won
        } else {
            ConquestBranch::Lost
        }
    }
}

/// The three things screen `0x1C` can say. Mirrors
/// [`crate::screens::conquest::Outcome`], which is the painter's own copy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConquestBranch {
    Won,
    Lost,
    Finished,
}

#[cfg(test)]
mod tests {
    use super::*;
    use l2_kingdom::victory::{MSG_AI_ELIMINATED, MSG_DEFEAT, MSG_VICTORY};

    fn ending(group: u16, from: u8) -> Ending {
        Ending { group, from, to: 0, category: victory::CATEGORY_ENDING }
    }

    #[test]
    fn the_first_track_is_eight_named_maps_and_the_second_is_six() {
        assert_eq!(Track::First.first_map(), 0);
        assert_eq!(Track::First.length(), 8);
        assert_eq!(Track::Second.first_map(), 2);
        assert_eq!(Track::Second.length(), 6);
        // The scenario column, as read out of `Lords2.exe`.
        let first: Vec<usize> = TRACK_FIRST.iter().map(|m| m.scenario).collect();
        assert_eq!(first, vec![17, 12, 2, 5, 0, 3, 7, 4]);
        let second: Vec<usize> = TRACK_SECOND[2..].iter().map(|m| m.scenario).collect();
        assert_eq!(second, vec![52, 53, 45, 44, 41, 42]);
        // Every scenario is a real slot of the sixty `L2.eng` group 101 names.
        for m in TRACK_FIRST.iter().chain(TRACK_SECOND[2..].iter()) {
            assert!(m.scenario < 60, "scenario {} is not a map slot", m.scenario);
            assert!(m.difficulty <= 2, "difficulty {} is out of range", m.difficulty);
            assert!(matches!(m.gold, 1000 | 2500 | 5000), "gold {} is not a dropdown value", m.gold);
        }
    }

    /// **The purse is not monotonic, and finding that out is why this test is
    /// written the long way.** It reads 5000, 2500, 5000, 2500, 5000, 2500, 1000,
    /// 1000: the difficulty steps 0, 0, 1, 1, 2, 2, 2, 2 and the purse **resets
    /// at the top of each difficulty tier** and falls inside it. An earlier
    /// version of this test asserted the purse never rises and went red on the
    /// third row, which is the table telling the truth about its own shape.
    #[test]
    fn the_campaign_gets_harder_tier_by_tier_and_poorer_inside_each_tier() {
        let mut difficulty = 0;
        let mut gold = i32::MAX;
        let mut opponents = 0;
        for m in TRACK_FIRST {
            assert!(m.difficulty >= difficulty, "difficulty went backwards");
            if m.difficulty > difficulty {
                gold = i32::MAX; // a new tier; the purse starts again
            }
            assert!(m.gold <= gold, "the purse went up inside one difficulty tier");
            // Column 3, the one carried unnamed: whatever it is, it never falls
            // and never exceeds the number of realms there are.
            assert!(m.options[0] >= opponents && m.options[0] <= 5);
            difficulty = m.difficulty;
            gold = m.gold;
            opponents = m.options[0];
        }
        assert_eq!(difficulty, 2, "the last map is on the hardest setting");
        assert_eq!(gold, 1000, "with the smallest purse");
    }

    #[test]
    fn a_win_advances_the_campaign_and_a_loss_replays_the_map() {
        let mut c = Campaign::new(Track::First);
        assert_eq!(c.current().map(|m| m.scenario), Some(17));

        c.outcome = Outcome::Lost;
        assert_eq!(c.enter_conquest_screen(), ConquestBranch::Lost);
        assert_eq!(c.map, 0, "a loss does not advance");
        assert_eq!(c.current().map(|m| m.scenario), Some(17), "and the same map comes back");

        c.outcome = Outcome::Won;
        assert_eq!(c.enter_conquest_screen(), ConquestBranch::Won);
        assert_eq!(c.map, 1);
        assert_eq!(c.current().map(|m| m.scenario), Some(12));
    }

    #[test]
    fn the_eighth_win_finishes_the_campaign() {
        let mut c = Campaign::new(Track::First);
        c.outcome = Outcome::Won;
        for _ in 0..7 {
            assert_eq!(c.enter_conquest_screen(), ConquestBranch::Won);
        }
        assert_eq!(c.map, 7, "seven wins, one map to go");
        assert!(!c.is_complete());
        assert_eq!(c.enter_conquest_screen(), ConquestBranch::Finished);
        assert_eq!(c.map, 8);
        assert!(c.is_complete());
        assert_eq!(c.current(), None, "and there is no ninth map");
    }

    /// Track two ends after six because it starts at two.
    #[test]
    fn the_second_track_finishes_after_six_wins() {
        let mut c = Campaign::new(Track::Second);
        c.outcome = Outcome::Won;
        assert_eq!(c.current().map(|m| m.scenario), Some(52), "Australia");
        for _ in 0..5 {
            assert_eq!(c.enter_conquest_screen(), ConquestBranch::Won);
        }
        assert_eq!(c.enter_conquest_screen(), ConquestBranch::Finished);
        assert_eq!(c.map, 8);
    }

    // --- the queue ---------------------------------------------------------

    #[test]
    fn an_ai_dying_while_others_live_ends_nothing() {
        let mut c = Campaign::new(Track::First);
        c.ranking = Ranking { opponents_remaining: 2, ..Ranking::default() };
        c.raise(ending(MSG_AI_ELIMINATED, 3));
        assert_eq!(c.settle(1), Outcome::InPlay);
        assert!(c.pending.is_empty(), "a message that ends nothing is still shown and discarded");
    }

    #[test]
    fn my_own_defeat_notice_loses_the_game() {
        let mut c = Campaign::new(Track::First);
        c.ranking = Ranking { opponents_remaining: 2, ..Ranking::default() };
        c.raise(ending(MSG_DEFEAT, 1));
        assert_eq!(c.settle(1), Outcome::Lost);
        assert_eq!(c.outcome, Outcome::Lost);
        assert_eq!(c.outcome.value(), 11);
    }

    /// The mainline win: the last AI's obituary, with nobody left, becomes a
    /// victory one message later.
    #[test]
    fn the_last_ai_dying_wins_the_game_through_an_enqueued_victory() {
        let mut c = Campaign::new(Track::First);
        c.ranking = Ranking { opponents_remaining: 0, ..Ranking::default() };
        c.raise(ending(MSG_AI_ELIMINATED, 3));
        assert_eq!(c.settle(1), Outcome::Won);
        assert_eq!(c.outcome.value(), 10);
    }

    #[test]
    fn a_victory_message_stops_the_queue_dead() {
        let mut c = Campaign::new(Track::First);
        c.ranking = Ranking { opponents_remaining: 1, ..Ranking::default() };
        c.raise(ending(MSG_VICTORY, 0));
        c.raise(ending(MSG_DEFEAT, 1));
        assert_eq!(c.settle(1), Outcome::Won);
        assert_eq!(c.pending.len(), 1, "the message behind it is never shown");
        assert_eq!(c.pending[0].group, MSG_DEFEAT);
    }

    /// Group 195 travels as category 1 and so cannot touch the outcome.
    #[test]
    fn the_ai_coronation_taunt_does_not_end_a_game() {
        let mut c = Campaign::new(Track::First);
        c.ranking = Ranking { opponents_remaining: 1, ..Ranking::default() };
        c.raise(Ending {
            group: victory::MSG_AI_CROWNED,
            from: 3,
            to: 0,
            category: 1,
        });
        assert_eq!(c.settle(1), Outcome::InPlay);
    }
}

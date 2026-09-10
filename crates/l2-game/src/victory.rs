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
//! +0x00 -> g_scenarioIndex        +0x10 -> g_startCountyStatus
//! +0x04 -> g_optDifficulty        +0x14 -> g_startWeapons
//! +0x08 -> g_startingGoldChosen   +0x18 -> g_startArmySize
//! +0x0C -> g_startCastle          +0x1C -> g_aiLordCount
//! ```
//!
//! **All eight are named now, and one of the names is a correction.** The
//! campaign table writes exactly the globals `Setup_CommitOptions`
//! (`0x00499DC3`) writes — the campaign is the custom game with its options
//! chosen for you — so naming that function's destinations named this table's
//! columns for free. `crate::setup` is where each one goes and
//! `docs/decisions.md` C44 is the whole reading.
//!
//! The first three were already legible from their values: column 0 indexes
//! `L2.eng` group 101 and produces *Quaintville, Rose, Ireland, Italy, England,
//! France, Crusades, Germany* — a campaign ladder — column 1 is 0, 0, 1, 1, 2, 2,
//! 2, 2, and column 2 is 5000 / 2500 / 1000, which is exactly the *Crowns*
//! drop-down's own table at `0x004DBC18`.
//!
//! **Column 3 is not the opponent count.** It runs 1, 1, 2, 3, 4, 4, 5, 5, which
//! is why it was carried as **[D]** *"the opponent count — five is the most
//! realms there are"*. It is `g_startCastle`: the *Starting Castle* option, a
//! wooden keep on Quaintville climbing to a royal one on Germany. **The opponent
//! count is column 7**, which runs 1, 2, 3, 4, 4, 4, 4, 4 — one lord on the first
//! map, four from the fourth on. The two columns both climb and both stop at 5
//! and 4, which is how a plausible reading survived being written down.
//!
//! Columns 4, 5 and 6 are the county status, the weapons and the army size, and
//! they say something about the campaign: **`g_startArmySize` is 0 on every row
//! of both tracks**, so no campaign map ever starts you with an army, and
//! `g_startWeapons` is 1 — *few* — on every row but Italy's and the Crusades',
//! where it is 0.
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

use l2_kingdom::victory::{Outcome, Ranking};

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
    /// `g_startingGoldChosen` — the starting treasury, 5000 / 2500 / 1000.
    pub gold: i32,
    /// `g_startCastle`, `g_startCountyStatus`, `g_startWeapons`,
    /// `g_startArmySize`, `g_aiLordCount`, in the order `Campaign_LoadEntry`
    /// assigns them — which is the order of the four accessors below.
    ///
    /// Kept as one array rather than five fields because they are still read
    /// verbatim out of the table and written verbatim back; the accessors say
    /// what each slot is.
    pub options: [i32; 5],
}

impl CampaignMap {
    /// The castle this map starts you with, 0..=5. Column `+0x0C`, and
    /// `docs/decisions.md` C44 is why it is not the opponent count.
    pub fn castle(&self) -> u8 {
        self.options[0].clamp(0, 5) as u8
    }

    /// The county-status row, 0..=2 — weak, medium, strong.
    pub fn county_status(&self) -> usize {
        self.options[1].clamp(0, 2) as usize
    }

    /// The armoury row, 0..=3 — none, few, some, many.
    pub fn weapons(&self) -> usize {
        self.options[2].clamp(0, 3) as usize
    }

    /// The garrison row, 0..=3 — no army, small, medium, large. **0 on every
    /// row of both tracks.**
    pub fn army_size(&self) -> usize {
        self.options[3].clamp(0, 3) as usize
    }

    /// How many AI lords this map is played against. Column `+0x1C`, and *this*
    /// is the opponent count.
    pub fn ai_lords(&self) -> i32 {
        self.options[4]
    }
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
    /// What the last `Score_RankRealms` left behind. [`l2_kingdom::victory::outcome_of`]
    /// reads `opponents_remaining` out of it.
    pub ranking: Ranking,
}

// **`Campaign::pending` is gone, and that is the point of this branch.**
//
// It used to be a `Vec<Ending>` with the note *"the original has one message
// ring for everything … when one lands, this becomes a filter over it."* One has
// landed: [`crate::message::MessageQueue`], on [`crate::Game`]. The endings now
// go into the ring every other message goes into, are pulled by `Msg_Pump`,
// **displayed**, and act on being **dismissed** — which is what `docs/plan.md`
// says the win is and what nothing here could do while there was a second queue
// nothing drew.

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
    use l2_kingdom::victory::{
        self as victory, Ending, MSG_AI_ELIMINATED, MSG_DEFEAT, MSG_VICTORY,
    };

    fn ending(group: u16, from: u8) -> Ending {
        Ending { group, from, to: 0, category: victory::CATEGORY_ENDING, variant: 0 }
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
        let mut castle = 0;
        let mut opponents = 0;
        for m in TRACK_FIRST {
            assert!(m.difficulty >= difficulty, "difficulty went backwards");
            if m.difficulty > difficulty {
                gold = i32::MAX; // a new tier; the purse starts again
            }
            assert!(m.gold <= gold, "the purse went up inside one difficulty tier");
            // **Both of these climb, which is why they were confusable.**
            // Column 3 is the castle you start with and column 7 is how many
            // lords you start against; `docs/decisions.md` C44.
            assert!(m.castle() >= castle && m.castle() <= 5);
            assert!(m.ai_lords() >= opponents && m.ai_lords() <= 4);
            difficulty = m.difficulty;
            gold = m.gold;
            castle = m.castle();
            opponents = m.ai_lords();
        }
        assert_eq!(difficulty, 2, "the last map is on the hardest setting");
        assert_eq!(gold, 1000, "with the smallest purse");
        assert_eq!(castle, 5, "and a royal castle to hold");
        assert_eq!(opponents, 4, "against four lords");
    }

    /// **No campaign map starts you with an army, on either track.** Column 6
    /// is `g_startArmySize` and it is 0 on all fourteen real rows, which is what
    /// makes it distinguishable from the three columns beside it that do move.
    #[test]
    fn the_campaign_never_starts_you_with_a_garrison() {
        for m in TRACK_FIRST.iter().chain(TRACK_SECOND[2..].iter()) {
            assert_eq!(m.army_size(), 0, "map {} raises an army", m.scenario);
            assert!(m.county_status() <= 2, "map {}", m.scenario);
            assert!(m.weapons() <= 3, "map {}", m.scenario);
        }
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
    //
    // These used to drive `Campaign::settle` over a private `Vec<Ending>`. They
    // now drive the **real ring** on a real [`crate::Game`], through the same
    // [`crate::message::show`] and [`crate::message::dismiss`] the message
    // screen calls — because a queue only one code path can reach is the thing
    // this branch existed to remove.

    use crate::message::{self, Record};
    use crate::Game;

    fn game_with(opponents: u8) -> Game {
        let mut g = Game::new(0x51EED);
        g.player = 1;
        g.campaign.ranking = Ranking { opponents_remaining: opponents, ..Ranking::default() };
        g
    }

    fn post(g: &mut Game, msg: Ending) {
        let player = g.player;
        g.messages.enqueue(Record::from(msg), player);
    }

    #[test]
    fn an_ai_dying_while_others_live_ends_nothing() {
        let mut g = game_with(2);
        post(&mut g, ending(MSG_AI_ELIMINATED, 3));
        assert_eq!(message::drain(&mut g), Outcome::InPlay);
        assert!(g.messages.is_empty(), "a message that ends nothing is still shown and discarded");
    }

    #[test]
    fn my_own_defeat_notice_loses_the_game() {
        let mut g = game_with(2);
        post(&mut g, ending(MSG_DEFEAT, 1));
        assert_eq!(message::drain(&mut g), Outcome::Lost);
        assert_eq!(g.campaign.outcome, Outcome::Lost);
        assert_eq!(g.campaign.outcome.value(), 11);
    }

    /// The mainline win: the last AI's obituary, with nobody left, becomes a
    /// victory one message later — and the victory arrives **through the ring**,
    /// enqueued by the arm that displayed the obituary.
    #[test]
    fn the_last_ai_dying_wins_the_game_through_an_enqueued_victory() {
        let mut g = game_with(0);
        post(&mut g, ending(MSG_AI_ELIMINATED, 3));
        assert_eq!(message::drain(&mut g), Outcome::Won);
        assert_eq!(g.campaign.outcome.value(), 10);
    }

    #[test]
    fn a_victory_message_stops_the_queue_dead() {
        let mut g = game_with(1);
        post(&mut g, ending(MSG_VICTORY, 0));
        post(&mut g, ending(MSG_DEFEAT, 1));
        assert_eq!(message::drain(&mut g), Outcome::Won);
        assert_eq!(g.messages.queued(), 1, "the message behind it is never shown");
        assert!(!g.messages.is_open());
    }

    /// Group 195 travels as category 1 and so cannot touch the outcome.
    #[test]
    fn the_ai_coronation_taunt_does_not_end_a_game() {
        let mut g = game_with(1);
        post(
            &mut g,
            Ending { group: victory::MSG_AI_CROWNED, from: 3, to: 0, category: 1, variant: 0 },
        );
        assert_eq!(message::drain(&mut g), Outcome::InPlay);
    }
}

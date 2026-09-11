//! **The tip screens** — `Tip_Update` (`0x00476AA7`), `Tip_Show`
//! (`0x00476DA9`), the restore `FUN_00476E21` and the reset `FUN_00476A5D`.
//!
//! Twenty `L2.eng` groups, 200…219, of which fourteen are posted: the first-time
//! advice the original gives a new player — *"Game Objectives:"* on the
//! campaign map, *"The Town Center:"* in the village, *"Army Movement:"* the
//! first time an army is picked up. None of it existed here until this module,
//! and with it went forty narration files: the first line of every tip and the
//! twenty-seven chained takes after it. `docs/audio-triggers.md`.
//!
//! # Four functions and five globals
//!
//! ```c
//! void Tip_Update(void) {                          /* once a frame, Battle_Frame */
//!   if (g_optTipScreens && g_appPhase == 3) {
//!     if (DAT_004F0358 == 0) { …the ladder, below… }
//!     else DAT_004F0358--;
//!   }
//! }
//! void Tip_Show(int group) {
//!   if (g_screenId != 0x27) {
//!     DAT_004F0350 = g_screenId; g_screenId = 0x27;
//!     g_tipShown[group] = 1; DAT_0052F004 = 0;
//!     Msg_Enqueue(0, g_localPlayer, group, 0, g_tipCategory[group], 0, 0, 0);
//!   }
//! }
//! void FUN_00476E21(void) {                        /* from every Msg_Dismiss */
//!   if (g_screenId == 0x27) {
//!     g_screenId = DAT_004F0350; if (DAT_004F0350 == 0x27) g_screenId = 0;
//!     DAT_004F0358 = 0x14;
//!   }
//! }
//! void FUN_00476A5D(void) {                        /* App_WinMain, Opt_ToggleTipScreens */
//!   for (i = 0; i < 0x14; i++) g_tipShown[200 + i] = 0;
//!   DAT_004F0350 = 0; DAT_004F0358 = 0x14;
//! }
//! ```
//!
//! `[V]`, all four, read out of the decompilation.
//!
//! # Two corrections to what was on file
//!
//! **The twenty frames are not "after a screen is first opened".**
//! `docs/symbols.md` said so. `DAT_004F0358` is written in exactly two places —
//! the reset and the restore — so the delay is a re-arm after start-up, after
//! the toggle and **after every tip is dismissed**, and nothing else. A screen
//! that opens with the counter already at zero gets its tip on the same frame.
//!
//! **"Once per game" is once per run.** `FUN_00476A5D` has two callers,
//! `App_WinMain` and `Opt_ToggleTipScreens`. `Game_NewGame` does not clear
//! `g_tipShown`, and neither does a load, so a player who starts a second game
//! without quitting sees none of the tips again. That is why [`Tips`] is carried
//! across the two places this engine replaces the whole [`crate::Game`] —
//! `screens::setup`'s start and `screens::saveload`'s load.
//!
//! # Screen `0x27` is a screen, and it is [`crate::screen::ScreenId::Tip`]
//!
//! `Tip_Show` does not open a window; it **changes `g_screenId`** and posts a
//! message. The window follows because `Msg_Pump` runs on `0x27`, and the
//! screen the player was on stops answering because `Screen_FrameInput`
//! dispatches on the byte. A scan of the decompilation finds no comparison of
//! `g_screenId` with `0x27` anywhere but `Msg_Pump`, `Tip_Show` and
//! `FUN_00476E21` — the one `case 0x27` in the image is `App_WndProc`'s
//! VK_RIGHT — so `0x27` has **no input arm and no painter of its own** `[I]`: a
//! jump-table dispatch would not show up as a comparison, and none was looked
//! for. [`crate::screens::tip`] is that screen: an overlay that draws nothing
//! and consumes every event.
//!
//! [`Tips::hosting`] is `g_screenId == 0x27`, and
//! [`crate::screen::Machine`] keeps a [`crate::screen::ScreenId::Tip`] on the
//! stack exactly while it is true.
//!
//! # What this module does not know
//!
//! The ladder tests the original's screen byte, and our stack is not a byte.
//! [`View`] is the projection, and [`View::of`] is where every mapping is
//! written down — two of them are not one-to-one and are explained there.
//!
//! **The battle arms cannot fire, and that is recorded rather than decided.**
//! Tips 212, 214 and 215 are guarded by `g_screenId == 0 && g_battlePhase == 2`.
//! Every write of `g_battlePhase = 2` found — `Battle_Start`, `FUN_00477C89`,
//! the skirmish set-up — writes `g_screenId = 0x29` beside it, and both battle
//! ends write `0x13` or `0x2E` before `g_battlePhase = 0`. And `Msg_Pump` opens
//! with *"if `g_battlePhase == 2` and a message is up, dismiss it"*, so a tip
//! posted during a battle would be closed the frame after it opened. No path
//! was found on which the guard holds across a frame. `[I]` — the search was
//! the phase writers and the two outcome functions, not every one of the
//! hundred `g_screenId = 0` writes. The arms are built as the original writes
//! them, and in this engine they are equally unreachable.
//!
//! not-encoded: see [`Tips`].

use crate::message::{self, Record};
use crate::screen::ScreenId;
use crate::Game;

/// The first tip group. `FUN_00476A5D` clears `g_tipShown + 200` for `0x14`
/// bytes, which is what fixes the range.
pub const FIRST: u16 = 200;
/// How many groups the range holds.
pub const COUNT: usize = 20;

/// `DAT_004F0358`'s re-arm: `0x14` frames, written by `FUN_00476E21` and
/// `FUN_00476A5D` and by nothing else.
pub const DELAY: u8 = 0x14;

/// **`g_tipCategory` (`0x004D6ED8`)**, groups 200…219 — the message category
/// `Tip_Show` posts each group with.
///
/// `[V]` read out of the executable at `0x004D6ED8 + 200`. For every group with
/// words the category is the paragraph count plus four — `Msg_DrawWindow`'s
/// `0x05`…`0x09` arm draws `category − 4` paragraphs — and the unit test below
/// holds that against [`TEXT`]. `docs/formats/eng.md` §5.3 says it of all
/// twenty; the six label-only groups (203…205, 213, 215, 216) were not checked
/// here.
pub const CATEGORY: [u8; COUNT] = [7, 8, 9, 5, 5, 5, 5, 8, 5, 8, 7, 5, 6, 5, 7, 5, 5, 9, 7, 5];

/// The group numbers the ladder names, as the original writes them.
pub mod group {
    pub const OBJECTIVES: u16 = 200;
    pub const GETTING_STARTED: u16 = 201;
    pub const FOOD_AND_HAPPINESS: u16 = 202;
    pub const KINGDOM_OVERVIEW: u16 = 206;
    pub const TOWN_CENTRE: u16 = 207;
    pub const BLACKSMITH: u16 = 208;
    pub const ARMOURY: u16 = 209;
    pub const ARMY_MOVEMENT: u16 = 210;
    pub const INVASIONS: u16 = 211;
    pub const BATTLES: u16 = 212;
    pub const SIEGES: u16 = 214;
    pub const SIEGES_2: u16 = 215;
    pub const CASTLE_BUILDING: u16 = 217;
    pub const ADVANCED_OPTIONS: u16 = 218;
}

/// **The tip screens' state** — `g_tipShown[200..220]`, `DAT_004F0358`,
/// `g_screenId == 0x27`, `DAT_00553210` and `DAT_0052F004`'s reset.
///
/// not-encoded: per-peer display state, and per *run* rather than per game —
/// see the module header. Nothing in the world reads any of it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tips {
    /// `g_tipShown` (`0x004F0298`), from group 200.
    shown: [bool; COUNT],
    /// `DAT_004F0358` — frames until the ladder runs again.
    delay: u8,
    /// `g_screenId == 0x27`: a tip has been posted and nothing has dismissed a
    /// message since. `DAT_004F0350`, the screen to go back to, is the stack
    /// underneath [`crate::screen::ScreenId::Tip`] and needs no field.
    hosting: bool,
    /// **`DAT_00553210` — the invasion flag.** Set by `Unit_EnterCounty`
    /// (`0x004ABB36`) when one of the local player's units crosses into a county
    /// whose owner is not that unit's owner, and cleared by nothing but the
    /// ladder's last arm. See [`Tips::note_incursions`].
    invaded: bool,
    /// How many times `Tip_Show` has posted. `Tip_Show` zeroes `DAT_0052F004`,
    /// the chained takes' cursor, and that cursor is audio state
    /// [`crate::audio::Director`] keeps — so the reset crosses the seam as a
    /// count the director diffs, the same shape as
    /// [`crate::screen::Machine::clicks`], and nothing flows back.
    shows: u32,
}

impl Default for Tips {
    fn default() -> Tips {
        Tips::new()
    }
}

impl Tips {
    /// `FUN_00476A5D` as `App_WinMain` calls it: nothing shown, and twenty
    /// frames before the first tip.
    pub fn new() -> Tips {
        Tips { shown: [false; COUNT], delay: DELAY, hosting: false, invaded: false, shows: 0 }
    }

    /// **`FUN_00476A5D` (`0x00476A5D`)** — every tip unshown and the delay
    /// re-armed. `Opt_ToggleTipScreens` (`0x00434787`) calls it on *every*
    /// flip, off as well as on.
    pub fn reset(&mut self) {
        self.shown = [false; COUNT];
        self.delay = DELAY;
    }

    /// **`FUN_00476E21` (`0x00476E21`)** — the restore `Msg_Dismiss` runs on
    /// every dismissal. It acts only while `g_screenId` is `0x27`, so a tip
    /// window pulled on the campaign map after its host was already restored
    /// re-arms nothing when it closes.
    pub fn restore(&mut self) {
        if self.hosting {
            self.hosting = false;
            self.delay = DELAY;
        }
    }

    /// Whether `g_tipShown[group]` is set.
    pub fn shown(&self, group: u16) -> bool {
        index(group).is_some_and(|i| self.shown[i])
    }

    /// `DAT_004F0358`.
    pub fn delay(&self) -> u8 {
        self.delay
    }

    /// `g_screenId == 0x27`.
    pub fn hosting(&self) -> bool {
        self.hosting
    }

    /// `DAT_00553210`.
    pub fn invaded(&self) -> bool {
        self.invaded
    }

    /// How many tips `Tip_Show` has posted. See the field.
    pub fn shows(&self) -> u32 {
        self.shows
    }

    /// **`Unit_EnterCounty`'s one line for the tips**, fed from the simulation's
    /// report:
    ///
    /// ```c
    /// if (g_counties[county].owner != unit.owner) {
    ///     if (unit.owner == g_localPlayer) DAT_00553210 = 1;
    /// ```
    ///
    /// The comparison is made in `l2-kingdom` at the crossing, where the owner
    /// is the owner at that instant, and reported as
    /// [`l2_kingdom::units_tick::Incursion`]; the *local player* test is made
    /// here, because `g_localPlayer` is a peer's and not the world's.
    pub fn note_incursions(&mut self, incursions: &[l2_kingdom::units_tick::Incursion], player: u8) {
        if incursions.iter().any(|i| i.owner == player) {
            self.invaded = true;
        }
    }
}

fn index(group: u16) -> Option<usize> {
    let i = group.checked_sub(FIRST)? as usize;
    (i < COUNT).then_some(i)
}

/// **What `Tip_Update` reads**, projected out of our stack and world.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct View {
    /// `g_optTipScreens`.
    pub enabled: bool,
    /// `g_appPhase == 3`.
    pub in_play: bool,
    /// `g_screenId`, or `None` for a screen the ladder never names and whose
    /// byte is therefore not written down here.
    pub screen: Option<u8>,
    /// `g_jobPanelJob`, 1…9.
    pub job: usize,
    /// `g_mapZoom == 2`.
    pub zoom_far: bool,
    /// `g_battlePhase == 2`.
    pub battle: bool,
    /// `g_battleIsSiege`.
    pub siege: bool,
    /// `DAT_0057A0F0` — the Battle Master skirmish, which the tips skip.
    pub skirmish: bool,
}

impl View {
    /// **The projection, one question at a time.**
    ///
    /// * **`g_screenId`** is the top screen that is not the message scroll — the
    ///   scroll is not a screen id in the original (`crate::screens::message`) —
    ///   answered by [`crate::screen::Screen::mode_screen_id`] where the
    ///   original's byte is not a function of our [`ScreenId`], and by
    ///   [`screen_byte`] otherwise. The one screen that needs the first is the
    ///   campaign map: `Map_BeginMoveSelection` writes `g_screenId = 0x10`, the
    ///   only writer of that value in the image (`docs/screens.md` §9.5), and
    ///   ours keeps the same thing as `MapScreen::move_order`.
    /// * **`g_appPhase == 3`** is *the campaign is on the stack*. `Setup_StartGame`
    ///   and its two network twins write 3 as they enter the game, and every
    ///   writer of 2 leaves it for the front end, which here pops the campaign.
    /// * **`g_battlePhase == 2`** is *the battlefield is on the stack* — the same
    ///   reading `crate::audio::scene` makes, because the phase outlives the
    ///   field's three screen ids.
    /// * **`DAT_0057A0F0`** is set by the Battle Master's two set-up functions
    ///   (`0x0042B919` and its sibling) and cleared by `FUN_00497A34` on the
    ///   campaign route. There is no skirmish mode here, so it is `false`.
    pub fn of(machine: &crate::screen::Machine, game: &Game) -> View {
        let ids = machine.ids();
        let job = match machine.top_screen_id() {
            Some(ScreenId::Job(_, j)) => j + 1,
            _ => 0,
        };
        View {
            enabled: game.prefs.tip_screens,
            in_play: ids.contains(&ScreenId::Campaign),
            screen: machine.top_screen_byte(game),
            job,
            zoom_far: game.map_zoom_far,
            battle: ids.contains(&ScreenId::Battlefield),
            siege: game.battle.as_ref().is_some_and(|b| b.is_siege()),
            skirmish: false,
        }
    }
}

/// **The original's `g_screenId` for a [`ScreenId`]**, for the screens the tip
/// ladder asks about, and `None` for the rest.
///
/// Exhaustive, with no wildcard: adding a screen is a compile error here until
/// somebody has said whether the tips care about it. `None` is not *"has no
/// byte"* — every screen of the original's has one — it is *"not one the
/// ladder names, so which byte it is was not needed and is not asserted"*.
pub fn screen_byte(id: ScreenId, game: &Game) -> Option<u8> {
    use ScreenId as S;
    match id {
        S::Campaign => Some(0x00),
        S::Village(_) => Some(0x02),
        S::Job(..) => Some(0x0F),
        S::RaiseArmy(_) => Some(0x17),
        S::Castle(_) => Some(0x1B),
        S::Tip => Some(0x27),
        S::Options(page) => page.screen_id(),
        S::SaveLoad(mode) => Some(mode.screen_id()),
        S::Battlefield => Some(game.battle.as_ref().map_or(0x29, |b| b.screen_id())),
        S::Menu
        | S::Index
        | S::Message
        | S::County(..)
        | S::Setup(_)
        | S::Conquest
        | S::Diplomacy
        | S::DiploCompose(..)
        | S::Siege(_)
        | S::Armoury(_)
        | S::Rack(..)
        | S::Divide(_)
        | S::Merchant(_)
        | S::Trade(..)
        | S::BattlePrompt
        | S::BattleResult
        | S::MenuBar(_)
        | S::About
        | S::Court
        | S::Supplies(_)
        | S::Ratings
        | S::Info(_) => None,
    }
}

/// **`Tip_Update` (`0x00476AA7`)**, the whole ladder. Returns the group it
/// hands to `Tip_Show`, which may still refuse — see [`show`].
///
/// The order is the original's and two things about it are load-bearing:
///
/// * the campaign map's four tips are **one per re-arm**, in the order 206
///   (only when zoomed out), 200, 201, 202 — so a player meets three windows
///   in a row with twenty frames between each;
/// * the invasion arm is the **last `else if` and has no screen test**, and it
///   clears `DAT_00553210` *before* asking whether 211 was shown. So the flag is
///   consumed by any frame that reaches that arm — including one on screen
///   `0x27`, where `Tip_Show` then refuses and the tip is lost until the next
///   crossing. Reproduced; `docs/bugs.md` BNEW-invasion-tip-swallowed.
pub fn update(tips: &mut Tips, view: &View) -> Option<u16> {
    use group as g;
    if !(view.enabled && view.in_play) {
        return None;
    }
    if tips.delay != 0 {
        tips.delay -= 1;
        return None;
    }
    let shown = tips.shown;
    let unshown = move |group: u16| index(group).is_some_and(|i| !shown[i]);
    let screen = view.screen;

    if screen == Some(0x00) && !view.battle {
        if view.zoom_far && unshown(g::KINGDOM_OVERVIEW) {
            return Some(g::KINGDOM_OVERVIEW);
        }
        for group in [g::OBJECTIVES, g::GETTING_STARTED, g::FOOD_AND_HAPPINESS] {
            if unshown(group) {
                return Some(group);
            }
        }
    }
    if screen == Some(0x00) && view.battle {
        if view.skirmish {
            return None;
        }
        if !view.siege {
            if unshown(g::BATTLES) {
                return Some(g::BATTLES);
            }
        } else {
            if unshown(g::SIEGES) {
                return Some(g::SIEGES);
            }
            if unshown(g::SIEGES_2) {
                return Some(g::SIEGES_2);
            }
        }
    }
    if screen == Some(0x17) && unshown(g::ARMOURY) {
        Some(g::ARMOURY)
    } else if screen == Some(0x0F) && view.job == 8 && unshown(g::BLACKSMITH) {
        Some(g::BLACKSMITH)
    } else if screen == Some(0x02) && unshown(g::TOWN_CENTRE) {
        Some(g::TOWN_CENTRE)
    } else if screen == Some(0x39) && unshown(g::ADVANCED_OPTIONS) {
        Some(g::ADVANCED_OPTIONS)
    } else if screen == Some(0x1B) && unshown(g::CASTLE_BUILDING) {
        Some(g::CASTLE_BUILDING)
    } else if screen == Some(0x10) && unshown(g::ARMY_MOVEMENT) {
        Some(g::ARMY_MOVEMENT)
    } else if tips.invaded {
        tips.invaded = false;
        unshown(g::INVASIONS).then_some(g::INVASIONS)
    } else {
        None
    }
}

/// **`Tip_Show` (`0x00476DA9`).** Returns whether it posted.
///
/// The record is `Msg_Enqueue(0, g_localPlayer, group, 0, g_tipCategory[group],
/// 0, 0, 0)` — **from** realm 0 **to** the local player, which the peer filter
/// always keeps.
pub fn show(game: &mut Game, group: u16) -> bool {
    let Some(i) = index(group) else { return false };
    if game.tips.hosting {
        return false;
    }
    game.tips.hosting = true;
    game.tips.shown[i] = true;
    game.tips.shows = game.tips.shows.wrapping_add(1);
    let player = game.player;
    let record = Record {
        to: player,
        from: 0,
        group,
        variant: 0,
        category: CATEGORY[i],
        county: 0,
        spare: 0,
        payload: 0,
    };
    game.messages.enqueue(record, player);
    true
}

/// `Tip_Update` then `Tip_Show`, as `Battle_Frame` runs them. Returns the group
/// posted, if one was.
pub fn tick(game: &mut Game, view: &View) -> Option<u16> {
    let group = update(&mut game.tips, view)?;
    show(game, group).then_some(group)
}

// ------------------------------------------------------------------ the words

/// **Our transcription of groups 200…218**, for an install whose `L2.eng`
/// cannot be read. `CLAUDE.md` rule 6: the words are the feature, so the
/// player's own file is what is drawn and this is only the fallback.
/// `tests/tips.rs` holds it against the file, string for string.
///
/// Index 0 of each is the heading; the rest are the paragraphs, in order.
pub const TEXT: &[(u16, &[&str])] = &[
    (
        200,
        &[
            "Game Objectives:",
            "Feed your peasants to keep them happy and make them multiply.",
            "Make weapons and create an army.",
            "Conquer thy neighbors!",
        ],
    ),
    (
        201,
        &[
            "Getting started:",
            "Right click on items for information.  Left click to perform actions.",
            "Use the slider bar to divide peasants between farming and industry (forestry, iron mining, stone quarrying and weapon making).",
            "Clicking on industries on the map switches them on and off.",
            "Changes you make in a county do not take place until the following season.",
        ],
    ),
    (
        202,
        &[
            "Food and Happiness:",
            "Cattle provide dairy foods each season, or they can be eaten.",
            "Wheat can be bought from a merchant and planted in winter. Click on a fallow field to change its usage.",
            "Plan ahead several seasons. Don't get caught without food for your peasants.",
            "High taxes and army recruiting lower happiness.",
            "Very low happiness could result in riots.",
        ],
    ),
    (206, &["Kingdom overview:", "Click on a county to go to that county"]),
    (
        207,
        &[
            "The Town Center:",
            "Click on any area for details about it.",
            "Click and drag a box around any peasants you wish to move, then click on the task area you wish to allocate them to.  Double-click town center to gather all idle workers.",
            "Blackened figures indicate there are not enough workers to meet optimal production in that task area.",
            "Idle peasants appear in an area if you allocate more than enough labor for the task.",
        ],
    ),
    (
        208,
        &[
            "The Blacksmith:",
            "To build a weapon, you must produce or buy the necessary amounts of iron and/or wood.",
        ],
    ),
    (
        209,
        &[
            "The Armoury:",
            "Use the slider bar to draft peasants from the population into the army.",
            "Don't draft too much of your population or happiness will drop too low.",
            "Choose units by clicking on the appropriate weapon on the wall. You can only raise the types of units you have weapons for.",
            "When your army is ready, click the Create button. Your army appears on the main map when you return to it.",
        ],
    ),
    (
        210,
        &[
            "Army Movement:",
            "After raising an army, move it toward a neighboring county.",
            "To move an army, click once on it, then click on a destination. The army marches toward the destination until it runs out of moves for the turn.",
            "Right clicking on an army calls up its vital statistics.",
        ],
    ),
    (
        211,
        &[
            "Invasions:",
            "To try to conquer a county, attack it by moving your army on top of the town center.",
        ],
    ),
    (
        212,
        &[
            "Battles:",
            "To move your units, click and drag a box around any number of them, then click on a destination.",
            "To attack, select units and then move the cursor onto an enemy unit. When the cursor turns red, click on the unit and your soldiers will attack it.",
        ],
    ),
    (
        214,
        &[
            "Sieges:",
            "Get your soldiers inside the castle to fight the defenders. To do this, break through the castle wall or gate with a catapult or battering ram, or go over the castle wall using a siege tower.",
            "Battering rams and siege towers must be moved right up to the castle wall to work. A catapult can attack from a distance.",
            "If the castle has a moat, select some of your soldiers (preferably peasants) and position the cursor over the water.  Click on the water and the soldiers will begin filling in the moat.",
        ],
    ),
    (215, &["Sieges2"]),
    (
        217,
        &[
            "Castle Building:",
            "Building a castle makes it much harder for enemy troops to conquer your county.",
            "Start with a simple castle design, then upgrade as your materials and builders increase.",
            "You may produce castle-building materials (stone and wood) in your county or buy them from a merchant.",
            "When you give orders to build a castle, some of your laborers in industry will move to castle building until the castle is finished.",
            "If your castle suffers damage from a siege, you may repair it as long as you have the materials and the builders.",
        ],
    ),
    (
        218,
        &[
            "Advanced Game Options:",
            "When Advanced Farming is on, the number of peasants required for grain farming varies from season to season, and weather and soil fertility affect wheat output.",
            "When Army Foraging is turned on, armies eat food in the county they're traveling in.",
            "When Exploration is turned on, the world outside your county is blacked out. It is gradually revealed as your armies move through and conquer new counties.",
        ],
    ),
];

/// Our transcription of one string, or `""`.
pub fn transcribed(group: u16, index: usize) -> &'static str {
    TEXT.iter()
        .find(|(g, _)| *g == group)
        .and_then(|(_, s)| s.get(index))
        .copied()
        .unwrap_or("")
}

/// **`Eng_DrawString(group, index)` for a tip**: the player's own `L2.eng`,
/// and our transcription only where the file gave nothing.
pub fn words(shell: &crate::shell::ShellAssets, group: u16, index: usize) -> String {
    let s = shell.text(group as usize, index);
    if s.is_empty() {
        transcribed(group, index).to_string()
    } else {
        s.to_string()
    }
}

/// Whether a record is a tip window — categories `0x05`…`0x09`, which only
/// `Tip_Show` posts.
pub fn is_tip_window(record: &Record) -> bool {
    matches!(record.shape(), message::Shape::Paragraphs(_))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **Group 215 is posted and has no words.** `Tip_Update`'s siege arm shows
    /// it right after 214, `g_tipCategory[215]` is 5 — one paragraph — and the
    /// group holds only its label, `"Sieges2"`. So the window would draw that
    /// label as a heading over an empty paragraph. It cannot be reached (see the
    /// module header), which is presumably why nobody noticed.
    #[test]
    fn the_second_siege_tip_is_a_label_with_nothing_under_it() {
        assert_eq!(CATEGORY[(group::SIEGES_2 - FIRST) as usize], 5);
        assert_eq!(transcribed(group::SIEGES_2, 0), "Sieges2");
        assert_eq!(transcribed(group::SIEGES_2, 1), "");
    }

    /// The table's claim, checked against our own transcription: the category
    /// is the paragraph count plus four for every group with words.
    #[test]
    fn every_category_is_its_groups_paragraph_count_plus_four() {
        for (g, s) in TEXT {
            if s.len() < 2 {
                continue;
            }
            let c = CATEGORY[(*g - FIRST) as usize] as usize;
            assert_eq!(c, s.len() - 1 + 4, "group {g}");
        }
    }
}

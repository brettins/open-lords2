
mod assets;
pub use assets::*;

mod unit_frames;
pub use unit_frames::*;
mod levy;
pub use levy::*;
mod game_methods;
pub use game_methods::*;

use l2_formats::maps::{MapSet, MapSlot};
use l2_formats::Palette;
use l2_kingdom::county::{LABOUR_CEILING_IGNORED, MAX_COUNTIES};
use l2_kingdom::realm::MAX_REALMS;
use l2_kingdom::tables::JOB_IDLE_TOWNSFOLK;
use l2_kingdom::{Kingdom, SeasonReport};
use l2_mods::vfs::Vfs;
use l2_view::campaign::{self, MapAssets};
use l2_view::chrome::{Chrome, Minimap};
use l2_view::village::VillageArt;
use l2_view::Ink;

use crate::shell::ShellAssets;

pub use l2_kingdom::tables::MAX_TAX_RATE;

/// `Ration_SliderClick` (`0x0043A379`) clamps `mouseX - 224` to `0 … 100` and
/// the track is
/// geometry are the same number.
pub use l2_kingdom::county::MAX_RATION_SPLIT;

/// `Options_Save` (`0x004AE15F`) writes the whole `g_options` block —
/// **0x468 bytes, and the shipped `lords2.inf` is
/// single call site on the shutdown path, so the original loses every setting
/// changed that session if it crashes. `Options_Load` (`0x004AE1B8`) reads it
/// back and `Options_Validate` (`0x004AE2BD`) checks `g_optionsMagic`
/// (`0x0053F204`) against **0x7EC** *after* the read, defaulting the whole block
/// when it does not match. `[V]` on all of it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Prefs {
    /// `g_optMusic` (`0x0053F218`). Default 1 — `FUN_004AF35E` sets all three
    /// sound flags at start-up.
    pub music: bool,
    /// `g_optSoundEffects` (`0x0053F214`).
    pub effects: bool,
    /// `g_optSpeech` (`0x0053F20C`). **Defaulted twice** by
    /// `Options_SetDefaults` — `0x004AE369` and `0x004AE389` both store 1 into
    /// `0x0053F20C` — which is harmless as shipped and means one of that
    /// function's five sound lines lost its target. `docs/bugs.md` B66.
    pub speech: bool,
    /// `g_optAnimations` (`0x0053F248`) — **five reads in four functions, and
    /// every one of them is answered here**. This list used to name
    /// `Map_ClampScroll` (`0x00429B1D`), which does not read the flag.
    ///
    /// | reader | what it chooses | ours |
    /// |---|---|---|
    /// | `Screen_BattleOutcome` `0x00423241` | the tall box with the film recess, not the short one | [`crate::screens::battlefield`] |
    /// | `CastleBuild_Confirm` `0x00436B59` | `Castle1..5.smk` over the chooser | [`crate::screens::castle`] |
    /// | `Msg_DrawWindow` `0x0047309E` ×2 | the capture and ending films, each dismissing its own letter | [`crate::message::animate`] |
    /// | `Battle_CheckOutcome` `0x00477DFC` | `bat_win1.smk` … by outcome and take | [`crate::screens::battlefield`] |
    pub animations: bool,
    /// `g_optTipScreens` (`0x0053F24C`), read by `Tip_Update` (`0x00476AA7`).
    pub tip_screens: bool,
    /// `g_optToolTips` (`0x0053F250`).
    pub tool_tips: bool,
    /// `g_optScrollSpeed` (`0x0053F234`) — **0, 10, 20 … 100, eleven settings**,
    /// because `Ui_OpenSlider` is opened with step 10, minimum 0 and maximum
    /// 100 and the two arrows are the only writers. Shown as 0…10,
    /// spinner's format 1 divides by ten.
    ///
    /// `Map_ScrollThrottle` (`0x004BBBE3`) turns it into a delay:
    ///
    /// `((100 - speed) / 10) * 12 + 2` milliseconds, integer division
    /// throughout, plus 24 ms on the army-movement screen — and `q >= 10`
    /// returns early, so **only speed 0 disables scrolling**. Default 60, which
    /// is 50 ms. `[V]`
    pub scroll_speed: i32,
    /// `g_optGameSpeed` (`0x0053F230`) — the same eleven settings and the same
    /// spinner, default **90**. Its consumer is `0x004BBAC3`, whose arithmetic
    /// is the parallel of the scroll throttle's; that it is the main loop's tick
    /// budget is **`[I]` and untraced**, so nothing here acts on it.
    pub game_speed: i32,
    pub debug_overlay: bool,
}

impl Default for Prefs {
    /// `Options_SetDefaults` (`0x004AE310`), for the fields we carry.
    fn default() -> Prefs {
        Prefs {
            music: true,
            effects: true,
            speech: true,
            animations: true,
            tip_screens: true,
            tool_tips: true,
            scroll_speed: 60,
            game_speed: 90,
            debug_overlay: false,
        }
    }
}

impl Prefs {
    pub const SPEED_STEP: i32 = 10;
    pub const SPEED_MIN: i32 = 0;
    pub const SPEED_MAX: i32 = 100;

    pub fn scroll_delay_ms(&self) -> Option<i32> {
        let q = (Prefs::SPEED_MAX - self.scroll_speed) / Prefs::SPEED_STEP;
        if q >= 10 {
            return None;
        }
        Some(q * 12 + 2)
    }

    pub fn step_speed(value: i32, up: bool) -> i32 {
        if up {
            if value < Prefs::SPEED_MAX {
                value + Prefs::SPEED_STEP
            } else {
                value
            }
        } else if Prefs::SPEED_MIN < value {
            value - Prefs::SPEED_STEP
        } else {
            value
        }
    }
}

/// A behavioural quirk still belongs on [`l2_kingdom::kingdom::Options`] and
/// still costs the bump. `docs/bugs.md` §6.3a has the one-line test that tells
/// them apart — *if flipping it can change a number in a saved game it is
/// behavioural; if it can only change which pixels are painted from the same
/// numbers it is presentation* — and `docs/decisions.md` C62 the argument for
/// the group switch that spans both.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Quirks {
    pub grey_county_name: bool,
}

pub const PRESENTATION: &[(&str, &str)] = &[("grey_county_name", "B64")];

impl Quirks {
    pub fn is_fixed(&self, field: &str) -> bool {
        match field {
            "grey_county_name" => self.grey_county_name,
            other => {
                debug_assert!(
                    !PRESENTATION.iter().any(|(f, _)| *f == other),
                    "{other} is in PRESENTATION and has no arm in Quirks::is_fixed"
                );
                false
            }
        }
    }

    pub fn set_fixed(&mut self, field: &str, fixed: bool) {
        match field {
            "grey_county_name" => self.grey_county_name = fixed,
            other => debug_assert!(
                !PRESENTATION.iter().any(|(f, _)| *f == other),
                "{other} is in PRESENTATION and has no arm in Quirks::set_fixed"
            ),
        }
    }

    pub fn tally(&self) -> (usize, usize) {
        let fixed = PRESENTATION.iter().filter(|(f, _)| self.is_fixed(f)).count();
        (PRESENTATION.len() - fixed, PRESENTATION.len())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Game {
    pub kingdom: Kingdom,
    pub player: u8,
    /// The map slot the scenario runs on. **`g_scenarioIndex` *is* the slot**,
    /// 0..=59, used unshifted: `Map_LoadLattice` seeks `slot * 0x80C1`, the
    /// slot stride, and `Eng_DrawString(101, g_scenarioIndex, …)` indexes the
    /// 60 slot names in `L2.eng` group 101. An earlier revision shifted it
    /// right by two on the belief that the low bits selected a season; they do
    /// not — the season is its own global, and the low two bits pick which of
    /// four map slots inside a `MAPnn.PL8` the minimap comes from.
    pub map_slot: usize,
    /// Realm `+0x0A`, **raw**: which colour a realm flies. It picks the banner
    /// in the menu bar and the ramp the minimap tints a county with.
    ///
    /// Stored as the save holds it and clamped only at the point of use, by
    /// [`l2_view::chrome::realm_colour`] — the same 1..=5 clamp `FUN_004171EE`
    /// applies before using it as a frame index. Clamping on load would turn a
    /// misread offset into a plausible colour 1 for every realm, which is
    /// exactly the failure a test cannot see.
    pub realm_colour: [u8; MAX_REALMS],
    /// **`g_playerNames` (`0x00553D54`)** — what each realm's lord is called.
    ///
    /// `Player_SetHuman` (`0x0049BAE9`) writes `g_realms[p].isHuman` and
    /// `g_playerNames[p]` from the same argument. Two sources fill it and they
    /// are both new-game work:
    ///
    /// * **the human's** is the string a person typed on setup page 4, copied
    ///   out of `g_options` — see [`crate::text`] and
    ///   [`crate::screens::setup::SetupScreen`];
    /// * **an AI lord's** is `Eng_Seek(7, realm.lord)` and sixteen bytes
    ///   copied. `L2.eng` group 7 is *The Knight, The Baron, The Countess, The
    ///   Bishop* — indexed by the **lord**, which is not the realm and not the
    ///   colour (`docs/diplomacy.md` §0.1).
    pub player_names: [crate::text::PlayerName; MAX_REALMS],
    pub selected: u8,
    /// `Event_Post` (`0x00448D7E`), called by `Battle_Frame` at `0x004BA187`
    /// for `g_selectedCounty`, does `eventFired = 0` on the county it posts.
    ///
    /// The original can write that into `g_counties` because it is **one
    /// machine**: there is one selection, so the write is the same write
    /// everywhere. Here [`Game::selected`] is a per-peer cursor, and
    /// `County::event_fired` is `County+0x000` inside `Encode for County`,
    /// which `l2_kingdom::save::checksum` — `Canonical::hash_of(kingdom)` — is
    /// the per-tick lockstep digest of. Two peers looking at different counties
    /// would clear different bytes and hash differently on the next tick, for a
    /// difference the simulation never reads. `docs/netcode.md` §6.
    ///
    /// **So the clearing moves here and the latch stays as the simulation wrote
    /// it.** `event_fired` is now written only by `Event_RollAll`
    /// (`0x00448819`) and the 24 handlers — the same value on every peer — and
    /// this mirror carries the posting, which is this peer's alone. It is the
    /// same split as [`Game::player_names`] and for the same stated reason: the
    /// digest is over the kingdom, so nothing a single player's interface does
    /// may live in the kingdom.
    pub event_posted: [bool; MAX_COUNTIES],
    /// County `+0x6C`, `+0x6D` — each county's anchor tile, which is where its
/// marker is drawn. Two arrays: index order
    /// is the only order anything here is ever walked in.
    pub anchor_x: [u8; MAX_COUNTIES],
    pub anchor_y: [u8; MAX_COUNTIES],
    pub gold_last: [i32; MAX_REALMS],
    pub last_report: Option<SeasonReport>,
    pub turns_played: u32,
    pub ai_granted: bool,
    /// The three globals a campaign is made of — `DAT_0053F258`, `DAT_0053F640`
    /// and `DAT_0053F0C4` — plus the ending messages the current map has raised.
    pub campaign: crate::victory::Campaign,
    pub field_policy: crate::engagement::Answer,
    pub(crate) turn: Option<crate::turn::TurnProgress>,
    pub prefs: Prefs,
    /// So the setting lives here, where the page can write it, and `main.rs`
    /// pushes it into [`Assets`] before each frame. **One authority, one
    /// projection** — and `crates/l2-game/tests/options/main.rs` asserts the
    /// projection happens, so this cannot become a field the drawing code never
    /// sees, which is the failure `docs/decisions.md` C30 records five of.
    pub presentation_quirks: Quirks,
    pub levy: LevyOrder,
    pub battle: Option<Box<crate::battlefield::LiveBattle>>,
    /// `Panel_MoveButton` (`0x004371CE`) is two statements — `g_screenId = 0`
    /// and `Map_BeginMoveSelection()` — because in the original the selection
    /// is a global and the screen is a byte. Ours has neither: move-order mode
    /// is [`crate::screens::map::MapScreen`]'s own state,
    /// panel that holds the button is a *different screen* with no handle on
    /// it. So the panel writes the request here and pops, and the map picks it
    /// up on its next tick — which is the same two steps in the same order.
    pub begin_move_order: Option<usize>,
    /// `Map_ConfirmMoveOrder` (`0x004A9252`) asks *"Combine armies?"* —
    /// `L2.eng` group 10 index 5, `g_hoverMergeUnit` (`0x00553F44`) — and
    /// `MoveOrder_ConfirmCombine` (`0x004A975D`) passes the standing unit as
    /// `Unit_OrderMove`'s fifth argument, so the merge happens on arrival
    /// through `Army_Combine` (`0x004AA181`). **We ask later than that**: the
    /// original's tile carries a stack and ours does not,
    /// crosses your own army halts on `Entry::Occupied` where the original
    /// walks over it. Same question, same words, same merge; a different
    /// moment, which is `[I]`. Written by [`crate::turn::tick_units_only`],
    /// consumed by the map screen's yes/no box.
    pub combine_ask: Option<(usize, usize)>,
    /// **`g_mapZoom` (`0x0057CB18`), projected out of the campaign map.**
    ///
    /// The zoom is a global in the original and three arms that are *not* on
    /// the campaign map read it: `Map_EdgeScroll` returns 0 at the far zoom, so
    /// `Screen_FrameInput`'s `0x04` arm cannot close the information panel by
    /// edge-scrolling there, and `FUN_00438ACC` and `FUN_0043893C` both open
    /// `if (g_mapZoom != 2)`. An overlay in our stack cannot see
    /// [`crate::screens::map::MapScreen`] at all, so without this those guards
    /// could not be reproduced and would have been silently dropped.
    pub map_zoom_far: bool,
    pub messages: crate::message::MessageQueue,
    pub tips: crate::tip::Tips,
    /// **`g_multiplayer`** (`0x00553D18`). False in every game this workspace
    /// can start; it is here because two rules branch on it and neither is
    /// reachable without it — `Msg_Pump`'s 399-tick message timeout, and the
    /// battle prompt's answer timeout. A flag nothing can set is a rule nothing
    /// can test, which is `docs/decisions.md`'s C27 in miniature.
    pub multiplayer: bool,
    /// **The turn timer** — `DAT_005440C8` and the three globals beside it. See
    /// [`crate::turn_clock`].
    pub turn_clock: crate::turn_clock::TurnClock,
    /// **`DAT_00553ED4` and `DAT_0053F084`** — which capture film and which of a
    /// battle row's four films come next — and the latch `Smk_OnFinished`'s
    /// return to the battle banner reads. See [`crate::movie::Reel`].
    pub films: crate::movie::Reel,

    /// **The frame each unit's tick handler wrote before it stepped** —
    /// `+0x07`, which `Map_DrawArmies` draws. See [`UnitFrames`].
    pub unit_frames: UnitFrames,

    /// **`DAT_0055CE7C`** — which of the standings page's seven categories is
    /// being looked at, 0…6. See [`crate::screens::nobles`].
    ///
    /// `Game_NewGame` (`0x00497CED`) zeroes it, `FUN_0043524E` writes it from
    /// the tab that was clicked, and `FUN_004351C4` — the court's button —
    /// *reads* it on the way in, to speak the category's name before the page
    /// is drawn. A screen that kept it to itself could not be read by that
    /// button and could not be read by [`crate::audio::Director`], which is
    /// exactly why `docs/audio.json` files the castle chooser's five spoken
    /// names as `blocked`.
    pub nobles_category: u8,
/// Ours, and it is a mechanism: `FUN_004B3994(category)`
    /// has exactly two callers — `FUN_004351C4`, the court's button, and
    /// `FUN_0043524E`, a tab — and *both* speak unconditionally, including
    /// when the tab pressed is the one already showing. A diff on
    /// [`Game::nobles_category`] would miss that press and would miss an open
    /// that did not change the category, so the edge the director watches is
    /// this counter and not the value. Same shape as
    /// [`crate::screen::Machine::clicks`], for the same reason.
    pub nobles_spoken: u32,

    /// Ours, and the same mechanism as [`Game::nobles_spoken`] for the same
    /// reason: a screen may not reach [`crate::audio::Audio`]
    /// (`docs/netcode.md` D-3), and six of the original's `Sound_PlayFile`
    /// sites sit *inside* a screen handler with their condition on state the
    /// screen keeps to itself. `SaveLoad_Tick` (`0x004AD9F0`) has two, on the
    /// frame the thumb up's latch is taken up; the merchant's four quantity
    /// handlers — `FUN_00435339`, `FUN_0043543D`, `FUN_00435541`,
    /// `FUN_004355DB` — have one each, on the step that crosses from selling
    /// into buying. Neither condition survives to the next tick,
    /// cannot see it and the screen has to say so.
    pub spoken: (u32, &'static str),
}

#[cfg(test)]
mod tests {
    use super::*;
    use l2_kingdom::tables::RATION_LEVEL_COUNT;

    fn two_counties() -> Game {
        let mut g = Game::new(7);
        g.kingdom.set_county_count(2);
        g.kingdom.counties[1].owner = 1;
        g.kingdom.counties[2].owner = 2;
        g
    }

    #[test]
    fn only_the_players_own_counties_take_orders() {
        let mut g = two_counties();
        assert!(g.set_tax_rate(1, 9));
        assert_eq!(g.kingdom.counties[1].tax_rate, 9);

        assert!(!g.set_tax_rate(2, 9), "county 2 belongs to another realm");
        assert_eq!(g.kingdom.counties[2].tax_rate, 0, "and it is unchanged");
        assert!(!g.set_ration(2, 5));
        assert!(!g.set_tax_rate(9, 1), "and 9 is not a county at all");
    }

    #[test]
    fn orders_are_clamped_to_the_ranges_the_rules_have() {
        let mut g = two_counties();
        g.set_tax_rate(1, -40);
        assert_eq!(g.kingdom.counties[1].tax_rate, 0);
        g.set_tax_rate(1, 10_000);
        assert_eq!(g.kingdom.counties[1].tax_rate, MAX_TAX_RATE);

        g.set_ration(1, 99);
        assert_eq!(g.kingdom.counties[1].ration_wanted, RATION_LEVEL_COUNT as i32 - 1);
        g.set_ration(1, -3);
        assert_eq!(g.kingdom.counties[1].ration_wanted, 0);

        // **What the player asks for (`+0x15E`) is not what the county managed
        // to feed (`+0x15D`)**, and the way to show that is a county that
        // cannot afford what is asked — *not* by requiring the control to leave
        // `ration_achieved` alone.
        //
        // This assertion used to be `ration_achieved == achieved`, and it was
        // asserting the defect: `Ration_IncreaseCounty` (`0x0043A23F`) calls
        // `Ration_Apply` before it repaints, so in the original the achieved
        // level moves the instant you press the arrow. Our setter wrote the
        // field and returned, the test agreed with it, and both were wrong
        // together. `docs/decisions.md` C125.
        g.kingdom.counties[1].population = 400;
        g.kingdom.counties[1].herd = 0;
        g.kingdom.counties[1].grain = 0;
        g.set_ration(1, RATION_LEVEL_COUNT as i32 - 1);
        assert_eq!(g.kingdom.counties[1].ration_wanted, RATION_LEVEL_COUNT as i32 - 1);
        assert_eq!(
            g.kingdom.counties[1].ration_achieved, 0,
            "an empty larder feeds nobody, whatever the player asked for",
        );
    }

    #[test]
    fn selection_refuses_ids_that_are_not_counties_on_this_map() {
        let mut g = two_counties();
        assert!(g.select(2));
        assert_eq!(g.selected, 2);
        assert!(!g.select(3), "county 3 is past g_countyCount");
        assert_eq!(g.selected, 2, "and the refusal leaves the old selection alone");
        assert!(g.select(0));
        assert_eq!(g.selected, 0);
    }

    #[test]
    fn counting_owners_walks_the_map_rather_than_trusting_the_realm_record() {
        let mut g = two_counties();
        g.kingdom.realms[1].county_count = 99; // a stale record
        assert_eq!(g.owned_by(1), 1);
        assert_eq!(g.owned_by(0), 0);
    }
}



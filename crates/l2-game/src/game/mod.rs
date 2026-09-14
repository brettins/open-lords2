//! The world, and the assets drawn from it.
//!
//! # One `Game`, borrowed by the screens
//!
//! `docs/plan.md`: *"A `Game` holds the kingdom, the active battle if any, and
//! the screen stack. Screens borrow it; they do not each keep a copy of the
//! world."* [`Game`] is that state, and it is a **plain struct of plain
//! fields** — fixed arrays, small integers, and types that are themselves plain
//! (`l2_kingdom::Kingdom` is arrays of counties and realms). No handle, no
//! index into a texture table, no `Rc`, nothing that only means something while
//! this process is running. That is what lets somebody serialise it later
//! without rewriting it first.
//!
//! [`Assets`] is deliberately *not* part of it. Decoded sprite sheets and a
//! palette are what the machine happens to have loaded, not what the world is,
//! and putting them in the same struct is how a save file ends up with a
//! tile-set in it.

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

/// The highest tax rate the interface will set.
///
/// **It lives in `l2-kingdom` now, and this is a re-export.** It was defined
/// here, in the application crate, which is the wrong side of the seam: 50 is
/// not a widget's range, it is the length of `g_taxHappinessOther` minus one
/// (`l2_kingdom::tables::TAX_HAPPINESS_OTHER`), and a ruleset that replaces
/// that table is entitled to move it. The screens keep importing it under this
/// name; only its home changed.
pub use l2_kingdom::tables::MAX_TAX_RATE;

/// The grain-to-livestock split runs the full width of its slider track.
///
/// `Ration_SliderClick` (`0x0043A379`) clamps `mouseX - 224` to `0 … 100` and
/// the track is
/// geometry are the same number.
///
/// **Re-exported**: the rule moved to `l2-kingdom` with
/// [`l2_kingdom::Kingdom::set_ration_split`], and a bound the rule clamps to a
/// hundred times per drag belongs beside the rule
/// widget.
pub use l2_kingdom::county::MAX_RATION_SPLIT;

/// **The machine's preferences** — the original's `g_options` block, minus the
/// parts that are the world's.
///
/// # The third category, and it is a category
///
/// There are three kinds of switch in this engine and conflating any two of
/// them is a real fault:
///
/// | | where | reaches the simulation? | in the save? |
/// |---|---|---|---|
/// | a **rule** the game was started with | [`l2_kingdom::kingdom::Options`] | yes, and the lockstep digest | the world's save |
/// | a **quirk** — one of the original's defects, switched | `Options::quirks` if behavioural, [`Quirks`] if presentation | behavioural: yes. presentation: never | behavioural only |
/// | a **preference** — sound, animation, scroll speed | here | **never** | not the world's save |
///
/// A preference is what *this machine* is like, not what *this game* is. Two
/// players in a lockstep session may disagree about every field here and compute
/// identical turns, which is exactly why none of it may reach `l2-kingdom` or
/// `l2-sim` (`docs/netcode.md` D-12) and why none of it is in
/// `l2_kingdom::save`. **A debug overlay toggle belongs here too**, not on
/// `Options::quirks` and not on `Tables`: it is not a rule variation at all.
///
/// # The original keeps these in a file, and we do not yet
///
/// `Options_Save` (`0x004AE15F`) writes the whole `g_options` block —
/// **0x468 bytes, and the shipped `lords2.inf` is
/// single call site on the shutdown path, so the original loses every setting
/// changed that session if it crashes. `Options_Load` (`0x004AE1B8`) reads it
/// back and `Options_Validate` (`0x004AE2BD`) checks `g_optionsMagic`
/// (`0x0053F204`) against **0x7EC** *after* the read, defaulting the whole block
/// when it does not match. `[V]` on all of it.
///
/// **We do not write a preferences file yet, and that is a gap
/// decision.** Saying so here is the point: a reader who finds no persistence
/// should meet the fact. What a file would
/// need is settled — `docs/environment.md` already says where our own files go —
/// and it is not this branch's job.
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
    /// **Ours: the debug overlay** — every marker, outline and line of our own
    /// 5 × 7 text that the original does not draw, on every screen. **Off by
    /// default**, and nothing
    /// else. Ctrl+D flips it, from anywhere, in [`crate::screen::Machine::handle`].
    ///
    /// A player: *"debug text everywhere, i'd like a toggle or hotkey."* The
    /// squares on the town square and the fields, the outline and caption over
    /// the sidebar icons, the status lines and the *NOT SIMULATED* stubs were all
    /// drawn unconditionally, so the only way to see the original's picture was
    /// to read the source.
    ///
    /// **What it does not gate**, deliberately: a fallback that runs only when a
    /// file of the install is missing (a normal install never shows one), the
    /// title page's build stamp, which exists so a report names its build,
    /// the two screens that are wholly ours (`screens::index`, `screens::menu`).
    ///
    /// A preference, so never in the save, never in the digest and never below
    /// this crate — the table above.
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
            // Ours, and off: see the field.
            debug_overlay: false,
        }
    }
}

impl Prefs {
    /// The eleven settings both speed spinners step through.
    pub const SPEED_STEP: i32 = 10;
    pub const SPEED_MIN: i32 = 0;
    pub const SPEED_MAX: i32 = 100;

    /// `Map_ScrollThrottle`'s delay in milliseconds, or `None` when scrolling is
    /// off altogether.
    ///
    /// **The early return is the whole reason this returns an `Option`.** The
    /// original tests `q >= 10` and returns 0 — *"do not scroll this frame"* —
/// so speed 0 is not "very slow",
    /// it is "never". A reimplementation that only computed the delay would
    /// creep instead of stopping.
    pub fn scroll_delay_ms(&self) -> Option<i32> {
        let q = (Prefs::SPEED_MAX - self.scroll_speed) / Prefs::SPEED_STEP;
        if q >= 10 {
            return None;
        }
        Some(q * 12 + 2)
    }

    /// Step a speed the way `Ui_SliderUp` / `Ui_SliderDown` do.
    ///
    /// **Not a clamp, a gate**: the original's arrows are
    /// `if (*v < max) *v += step` and `if (min < *v) *v -= step`,
/// already at the end does not move. That is the same shape as
    /// `l2_kingdom::mercenary::bands_in_play`'s "clamped" that turned out not to
    /// be a clamp, and it is written the original's way for the same reason.
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

/// **Switches that turn one of the original's *presentation* defects off.**
///
/// Every field defaults to `false`, which is the original's behaviour, and
/// `docs/bugs.md` names the switch in the entry for the defect it undoes.
///
/// # Why this is not `Options`
///
/// `docs/bugs.md` §6.3 recommends `Options` as the home for a quirk set, and it
/// is right — for the quirks it is arguing about. Every one of them changes a
/// *rule*, and its three constraints follow from that: the value must reach the
/// simulation, must be agreed in the lobby handshake, and must be stamped into
/// replays, at the cost of a `save::VERSION` bump.
///
/// **None of that applies to the colour of a text shadow.** A quirk here cannot
/// reach `l2-kingdom` or `l2-sim`, is not in the save body, is not in the
/// lockstep state, and two players running with different values here compute
/// identical turns — `docs/netcode.md` D-12 says display state must *not* reach
/// the simulation, so putting a shadow colour in the hashed options would be
/// the wrong answer. It lives on [`Assets`],
/// whose whole definition is *"everything the screens draw with, not part of
/// the world."*
///
/// A behavioural quirk still belongs on [`l2_kingdom::kingdom::Options`] and
/// still costs the bump. `docs/bugs.md` §6.3a has the one-line test that tells
/// them apart — *if flipping it can change a number in a saved game it is
/// behavioural; if it can only change which pixels are painted from the same
/// numbers it is presentation* — and `docs/decisions.md` C62 the argument for
/// the group switch that spans both.
///
/// # The table is how the group switch reaches this half
///
/// [`PRESENTATION`] names every field and the `docs/bugs.md` entry it undoes,
/// and [`Quirks::get`] / [`Quirks::set`] go through it. Rust cannot enumerate a
/// struct's fields, and the quirks page must be able to walk *both* sets to show
/// one tri-state parent — a parent that spoke for only half of them would be a
/// parent a player could not trust. `crates/l2-testkit/tests/quirks_catalogue/main.rs`
/// asserts the table names every field and no others.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Quirks {
    /// Draw a county's **name** on the cloudy plate with the grey emboss the
    /// *Sovereign land* lines beneath it use, instead of the parchment emboss
    /// the original uses everywhere.
    ///
    /// Off by default, and that is the whole design: **a set bit means *fixed*,
    /// so faithful is zero.** `docs/bugs.md` B64 has the defect and
    /// [`crate::shell::font::SHADOW_GREY`] the two palette indices.
    ///
    /// This field and the table around it were built on two branches that
/// met, so the comment standing in this space said *"no
    /// presentation quirk has landed yet"* and named this one as the first.
    pub grey_county_name: bool,
}

/// `(field, docs/bugs.md entry)` for every field of [`Quirks`], in declaration
/// order.
///
/// **The join between this half of the switch list and the catalogue.** It is
/// prose that a machine reads, which is the only kind of prose that cannot go
/// stale: `quirks_catalogue.rs` fails if a field is missing from it, if a row
/// names a field that does not exist, or if a row cites an entry `docs/bugs.md`
/// does not have.
pub const PRESENTATION: &[(&str, &str)] = &[("grey_county_name", "B64")];

impl Quirks {
    /// Whether this defect is **fixed** — the field's own sense, and the
    /// opposite of `l2_net::Quirks::reproduces`, which is the sense the
    /// behavioural half is stored in.
    ///
    /// Named by string because the group switch walks [`PRESENTATION`]; every
    /// drawing site reads its own field directly and never comes through here.
    pub fn is_fixed(&self, field: &str) -> bool {
        // One arm per field, and then the fall-through. A field that reached
        // [`PRESENTATION`] without an arm here would read as *"the original's
        // behaviour"* for ever, silently, which is the one failure this method
// can have — so it is asserted.
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

/// Set one field. Unknown names are ignored: the
    /// caller is a click on a checkbox, and a page that could crash the game by
    /// naming a field that has been renamed is worse than one that does
    /// nothing.
    pub fn set_fixed(&mut self, field: &str, fixed: bool) {
        match field {
            "grey_county_name" => self.grey_county_name = fixed,
            other => debug_assert!(
                !PRESENTATION.iter().any(|(f, _)| *f == other),
                "{other} is in PRESENTATION and has no arm in Quirks::set_fixed"
            ),
        }
    }

    /// How many presentation defects are reproduced, of how many there are.
    pub fn tally(&self) -> (usize, usize) {
        let fixed = PRESENTATION.iter().filter(|(f, _)| self.is_fixed(f)).count();
        (PRESENTATION.len() - fixed, PRESENTATION.len())
    }
}

/// The world.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Game {
    pub kingdom: Kingdom,
    /// `g_localPlayer` — the realm this machine drives.
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
    /// Six records, indexed by realm, and the index really is the realm:
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
    ///
/// **Here, and it changes what it is covered by.**
    /// The original keeps names outside `g_realms` too — they are a save block
    /// of their own —
    /// `Canonical::hash_of(kingdom)` and a name cannot change a number. Putting
    /// it in the kingdom would make a cosmetic string a desync source. It is in
    /// the save, in the prefix beside [`Game::realm_colour`], which is the
    /// other per-realm thing the interface draws and the rules never read.
    pub player_names: [crate::text::PlayerName; MAX_REALMS],
    /// The county under the cursor's last click, or 0 for none. County ids are
    /// 1-based in the original, so 0 is a usable "nothing".
    pub selected: u8,
    /// **Which counties' waiting letters this peer has already taken** — the
    /// half of the original's latch that is not the simulation's.
    ///
    /// `Event_Post` (`0x00448D7E`), called by `Battle_Frame` at `0x004BA187`
    /// for `g_selectedCounty`, does `eventFired = 0` on the county it posts.
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
    ///
    /// Raised by [`crate::message::post_event`], lowered again by
    /// [`crate::turn`] for every county `Event_RollAll` gave a new event to, so
    /// a second letter on a county whose first was read still arrives. In
    /// single player the two together are byte-for-byte the original's latch;
    /// multiplayer is the one place the original is not the authority, and it
    /// has no answer here to copy.
    pub event_posted: [bool; MAX_COUNTIES],
    /// County `+0x6C`, `+0x6D` — each county's anchor tile, which is where its
/// marker is drawn. Two arrays: index order
    /// is the only order anything here is ever walked in.
    pub anchor_x: [u8; MAX_COUNTIES],
    pub anchor_y: [u8; MAX_COUNTIES],
    /// Each realm's treasury as it stood before the last end-of-turn, so the
    /// interface can show which way the money went.
    pub gold_last: [i32; MAX_REALMS],
    /// What the last season did. Plain data: passes, messages and revolts.
    pub last_report: Option<SeasonReport>,
    /// How many turns this session has ended. `Kingdom::turn_count` is the
    /// game's own counter and starts at 1 in the England turn-one fixture; this one counts
    /// what the player did.
    pub turns_played: u32,
    /// **The AI's one grant of the turn has been made**, for the phase-4 arm
    /// the interactive frames run. [`crate::turn::open_players_turn`] clears it;
    /// `TurnProgress::granted` is the same latch for a wind-on.
    pub ai_granted: bool,
    /// **Whether this game is over, and where it sits in its campaign.**
    ///
    /// The three globals a campaign is made of — `DAT_0053F258`, `DAT_0053F640`
    /// and `DAT_0053F0C4` — plus the ending messages the current map has raised.
/// It is here because the original keeps it here
    /// too: `Game_NewGame` *clears* the outcome and *does not touch* the campaign
    /// counter, which is exactly the line between "the world" and "the session
    /// playing through it". See [`crate::victory`].
    pub campaign: crate::victory::Campaign,
    /// **What to answer *"Will you take the field?"* when nobody is asked.**
    ///
    /// [`crate::turn::end_turn`] is the headless door and cannot raise a screen,
    /// so every prompt it meets is answered with this. Declining is the
    /// original's own autocalc branch — it is a way out of *watching* a battle,
    /// not out of fighting one — so it is the default and nothing about a
    /// headless turn changed when the prompt was built.
    ///
    /// The interactive door ([`crate::turn::begin_turn`]) ignores it and asks.
    ///
    /// not-encoded: session state. A save is written between turns, so no
    /// prompt is outstanding when one is taken.
    pub field_policy: crate::engagement::Answer,
    /// **A turn that stopped to ask.** `None` between turns, which is almost
    /// always.
    ///
/// It is here because a half-run turn is
    /// not something a caller may drop: the kingdom is in a state no rule
    /// describes — two armies on one tile with the battle unresolved — and the
    /// only safe thing to do with it is finish it. See [`crate::turn`].
    ///
    /// not-encoded: session state,
    /// [`Game::field_policy`] — a half-run turn cannot be in a file because the
    /// only door to the save screen is between turns.
    pub(crate) turn: Option<crate::turn::TurnProgress>,
    /// **What this machine is like**, as against what this game is. Never in
    /// the save, never in the digest, never below this crate. See [`Prefs`].
    ///
    /// not-encoded: one person's preferences, not the world's state. A save
    /// that carried them would push them onto whoever loads it.
    pub prefs: Prefs,
    /// **The presentation quirks, as the person set them.**
    ///
    /// [`Assets::quirks`] is where the *drawing* code reads them, and it is a
    /// per-frame copy of this. The split is forced, and it is worth naming: a
    /// screen is handed `Ctx { game: &mut Game, assets: &Assets }`, so it can
    /// write the world and only read the assets — which is the property that
    /// makes `draw` unable to change anything (see [`crate::screen`]). A quirks
    /// page that wrote `Assets` directly would need `Ctx` to carry
    /// `&mut Assets`, and then `draw` could mutate too.
    ///
    /// So the setting lives here, where the page can write it, and `main.rs`
    /// pushes it into [`Assets`] before each frame. **One authority, one
    /// projection** — and `crates/l2-game/tests/options/main.rs` asserts the
    /// projection happens, so this cannot become a field the drawing code never
    /// sees, which is the failure `docs/decisions.md` C30 records five of.
    ///
    /// Never in the save and never in the digest, exactly like [`Prefs`].
    ///
    /// not-encoded: presentation. The *behavioural* quirks are the world's and
    /// are in `l2_kingdom::save`; these can only change which pixels are
    /// painted from the same numbers.
    pub presentation_quirks: Quirks,
    /// **The levy in progress** — the original's five globals, which three
    /// screens share and none of them owns.
    ///
    /// Session state, exactly like [`Game::field_policy`] and [`Game::turn`]
    /// above, and not in the save for the same reason: the original saves from
    /// the campaign map and nowhere else,
    /// file is written. See [`LevyOrder`].
    ///
    /// not-encoded: session state. The durable half — the realm's weapon stocks
    /// the basket was seeded from — is in the kingdom already.
    pub levy: LevyOrder,
    /// **The battle the player is watching**, or `None`, which is almost
    /// always.
    ///
    /// `docs/plan.md`: *"A `Game` holds the kingdom, the active battle if any,
/// and the screen stack."* This is that. It is here
    /// [`crate::screens::battlefield::BattlefieldScreen`] for the same reason
/// [`Game::turn`] is here: a screen is
    /// built from a bare [`crate::screen::ScreenId`] with no access to the
    /// world, and a half-fought battle is not something anyone may drop — the
    /// campaign is in a state no rule describes until it is settled.
    ///
    /// **It is not saved.** `l2_game::save` writes the campaign, and the
    /// original cannot save inside a battle either: `Menu_SaveGame` is on the
    /// File menu, whose three titles the battle screen does draw, and
    /// `Screen_HandleInput` has no arm for `0x29`. Whether the original refuses
    /// or misbehaves there was not established.
    ///
    /// not-encoded: session state,
    /// argument — the original cannot save inside a battle either.
    pub battle: Option<Box<crate::battlefield::LiveBattle>>,
    /// **`Map_BeginMoveSelection`, asked for by something that is not the map.**
    ///
    /// `Panel_MoveButton` (`0x004371CE`) is two statements — `g_screenId = 0`
    /// and `Map_BeginMoveSelection()` — because in the original the selection
    /// is a global and the screen is a byte. Ours has neither: move-order mode
    /// is [`crate::screens::map::MapScreen`]'s own state,
    /// panel that holds the button is a *different screen* with no handle on
    /// it. So the panel writes the request here and pops, and the map picks it
    /// up on its next tick — which is the same two steps in the same order.
    ///
    /// `None` almost always: it is consumed by the frame after it is written.
    ///
    /// not-encoded: session state. It cannot outlive the frame that set it, and
    /// the original's `g_selectedUnit` is not saved either.
    pub begin_move_order: Option<usize>,
    /// **A halted march wants to know whether to combine** — `(mover,
    /// occupant)`, both the local player's armies.
    ///
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
    ///
    /// **Not for multiplayer as it stands**: the original's combine is network
    /// command `0x2C`, and this answer is local.
    ///
    /// not-encoded: session state, like `begin_move_order` above.
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
    ///
    /// **One authority, one projection**, the same shape as
    /// [`Game::presentation_quirks`] → [`Assets::quirks`]: `MapScreen::zoom` is
    /// the authority and every write mirrors into this, so nothing reads a
    /// second copy that drifted. `crates/l2-game/tests/right_column/main.rs` asserts
    /// the projection holds, which is what stops it becoming a field the map
    /// forgets to write.
    ///
    /// not-encoded: presentation. Which zoom a person is looking at cannot
    /// change a number in the world.
    pub map_zoom_far: bool,
    /// **The message ring and the window over it.** `g_messageQueue`,
    /// `g_messageGroup`, `g_messageTimer` and their cursors.
    ///
/// Here and **out of the digest on purpose**:
    /// `Msg_Enqueue` keeps a record only when `to == 0 || to == g_localPlayer`,
    /// so two peers of one game hold different rings by construction. See
    /// [`crate::message`], which has the whole argument.
    ///
    /// not-encoded: per-peer display state. The original saves from the campaign
    /// map with `Msg_Pump` running,
    /// never during one.
    pub messages: crate::message::MessageQueue,
    /// **The tip screens** — `g_tipShown`, the twenty-frame re-arm, whether
    /// `g_screenId` is the tip's `0x27`, and the invasion flag. See
    /// [`crate::tip`].
    ///
/// not-encoded: per-peer display state, and per *run*.
    /// The original clears it at start-up and on the toggle and never on a new
    /// game or a load, so `screens::setup` and `screens::saveload` carry it
    /// across the two places a whole `Game` is replaced.
    pub tips: crate::tip::Tips,
    /// **`g_multiplayer`** (`0x00553D18`). False in every game this workspace
    /// can start; it is here because two rules branch on it and neither is
    /// reachable without it — `Msg_Pump`'s 399-tick message timeout, and the
    /// battle prompt's answer timeout. A flag nothing can set is a rule nothing
    /// can test, which is `docs/decisions.md`'s C27 in miniature.
    ///
    /// not-encoded: a property of the session, not of the world.
    pub multiplayer: bool,
    /// **The turn timer** — `DAT_005440C8` and the three globals beside it. See
    /// [`crate::turn_clock`].
    ///
/// Here because the original counts it in
    /// `Turn_Tick`, which runs whatever screen is up, and because a screen
    /// cannot outlive being replaced.
    ///
    /// not-encoded: session state. `Setup_StartGame` restarts the count on a
    /// start and on a load,
    /// the original throws away; and it is not the world's — the only thing it
    /// can do to the world is press End Turn.
    pub turn_clock: crate::turn_clock::TurnClock,
    /// **`DAT_00553ED4` and `DAT_0053F084`** — which capture film and which of a
    /// battle row's four films come next — and the latch `Smk_OnFinished`'s
    /// return to the battle banner reads. See [`crate::movie::Reel`].
    ///
    /// not-encoded: presentation. Neither counter is in any of the original's
    /// save blocks and neither changes anything but which picture is shown.
    pub films: crate::movie::Reel,

    /// **The frame each unit's tick handler wrote before it stepped** —
    /// `+0x07`, which `Map_DrawArmies` draws. See [`UnitFrames`].
    ///
    /// not-encoded: presentation, and one tick of it. Nothing in the kingdom
    /// reads it, and a loaded game's first sweep writes it afresh.
    pub unit_frames: UnitFrames,

    /// **`DAT_0055CE7C`** — which of the standings page's seven categories is
    /// being looked at, 0…6. See [`crate::screens::nobles`].
    ///
/// Here because **the original's is a global**:
    /// `Game_NewGame` (`0x00497CED`) zeroes it, `FUN_0043524E` writes it from
    /// the tab that was clicked, and `FUN_004351C4` — the court's button —
    /// *reads* it on the way in, to speak the category's name before the page
    /// is drawn. A screen that kept it to itself could not be read by that
    /// button and could not be read by [`crate::audio::Director`], which is
    /// exactly why `docs/audio.json` files the castle chooser's five spoken
    /// names as `blocked`.
    ///
    /// not-encoded: presentation. Which page of a scoreboard somebody has open
    /// cannot change a number in the world.
    pub nobles_category: u8,
    /// **How many times the standings page has been asked to say its category
    /// out loud**, monotone.
    ///
/// Ours, and it is a mechanism: `FUN_004B3994(category)`
    /// has exactly two callers — `FUN_004351C4`, the court's button, and
    /// `FUN_0043524E`, a tab — and *both* speak unconditionally, including
    /// when the tab pressed is the one already showing. A diff on
    /// [`Game::nobles_category`] would miss that press and would miss an open
    /// that did not change the category, so the edge the director watches is
    /// this counter and not the value. Same shape as
    /// [`crate::screen::Machine::clicks`], for the same reason.
    ///
    /// not-encoded: presentation, and one tick of it.
    pub nobles_spoken: u32,

    /// **A narrator line a screen has already decided on** — a monotone count
    /// and the file, the newest of which [`crate::audio::Director`] plays.
    ///
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
    ///
    /// The count and not the name is the edge, because two presses of the same
    /// button are two lines: [`Game::nobles_spoken`]'s note is the argument.
    ///
    /// not-encoded: presentation, and one tick of it. What a screen asked the
    /// narrator for cannot change a number in the world.
    pub spoken: (u32, &'static str),
}

#[cfg(test)]
mod tests {
    use super::*;
    // The clamp lives in `Kingdom::set_ration_wanted` now, so the lib no longer
    // names this constant — only the test that pins the cap does.
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
        // People, and nothing to feed them with. A county with **no** people is
        // fed at triple rations trivially — the requirement is zero — which is
        // what the first draft of this assertion tripped over.
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



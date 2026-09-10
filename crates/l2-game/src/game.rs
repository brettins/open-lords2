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

use l2_formats::maps::{MapSet, MapSlot};
use l2_formats::Palette;
use l2_kingdom::county::{LABOUR_CEILING_IGNORED, MAX_COUNTIES};
use l2_kingdom::realm::MAX_REALMS;
use l2_kingdom::tables::{JOB_IDLE_TOWNSFOLK, RATION_LEVEL_COUNT};
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
/// the track is exactly 100 pixels wide, so the field's range and the widget's
/// geometry are the same number.
///
/// **Re-exported**: the rule moved to `l2-kingdom` with
/// [`l2_kingdom::Kingdom::set_ration_split`], and a bound the rule clamps to a
/// hundred times per drag belongs beside the rule rather than beside the
/// widget.
pub use l2_kingdom::county::MAX_RATION_SPLIT;

/// **The machine's preferences** — the original's `g_options` block, minus the
/// parts that are the world's.
///
/// # The third category, and it is a category
///
/// There are three kinds of switch in this engine and conflating any two of
/// them is a real fault rather than an untidiness:
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
/// **0x468 bytes, and the shipped `lords2.inf` is exactly that long** — from a
/// single call site on the shutdown path, so the original loses every setting
/// changed that session if it crashes. `Options_Load` (`0x004AE1B8`) reads it
/// back and `Options_Validate` (`0x004AE2BD`) checks `g_optionsMagic`
/// (`0x0053F204`) against **0x7EC** *after* the read, defaulting the whole block
/// when it does not match. `[V]` on all of it.
///
/// **We do not write a preferences file yet, and that is a gap rather than a
/// decision.** Saying so here is the point: a reader who finds no persistence
/// should meet the fact rather than assume it was considered. What a file would
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
    /// `g_optAnimations` (`0x0053F248`). Read by `Screen_BattleOutcome`
    /// (`0x00423241`), `Msg_DrawWindow` (`0x0047309E`) and `Map_ClampScroll`
    /// (`0x00429B1D`).
    pub animations: bool,
    /// `g_optTipScreens` (`0x0053F24C`), read by `Tip_Update` (`0x00476AA7`).
    pub tip_screens: bool,
    /// `g_optToolTips` (`0x0053F250`).
    pub tool_tips: bool,
    /// `g_optScrollSpeed` (`0x0053F234`) — **0, 10, 20 … 100, eleven settings**,
    /// because `Ui_OpenSlider` is opened with step 10, minimum 0 and maximum
    /// 100 and the two arrows are the only writers. Shown as 0…10, because the
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
    /// rather than computing a very long delay, so speed 0 is not "very slow",
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
    /// `if (*v < max) *v += step` and `if (min < *v) *v -= step`, so a value
    /// already at the end simply does not move. That is the same shape as
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
/// the wrong answer rather than the expensive one. It lives on [`Assets`],
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
/// parent a player could not trust. `crates/l2-testkit/tests/quirks_catalogue.rs`
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
    /// This field and the table around it were built on two branches that never
    /// met, which is why the comment standing in this space said *"no
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
        // can have — so it is asserted rather than left to be noticed.
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

    /// Set one field. Unknown names are ignored rather than panicking: the
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

/// Everything the screens draw with. Not part of the world.
pub struct Assets {
    pub palette: Palette,
    pub ink: Ink,
    pub map: MapAssets,
    /// Presentation switches. See [`Quirks`].
    pub quirks: Quirks,
    /// The original's interface artwork — `Panels.pl8` and `Misc_cty.pl8`.
    ///
    /// `None` when the install does not supply them, which is the placeholder
    /// case: every screen then falls back to its own flat panels, and looks it.
    /// That is deliberate — a stub that is visibly ours beats one that looks
    /// finished.
    pub chrome: Option<Chrome>,
    /// `vill.pl8`, `villtops.pl8` and `vill_gd8.pl8` — the village screen's own
    /// files, which no other screen loads.
    ///
    /// `None` on an install without them, and the village then draws its own
    /// ground and refuses to move anybody, because the grid that decides where
    /// a drop lands *is* one of those files.
    pub village: Option<VillageArt>,
    /// `T32_bat1.pl8`, its palette and six colours of seven troop sheets — the
    /// battlefield's own artwork, loaded by `Battle_LoadAssets` (`0x004987B7`)
    /// and by nothing else.
    ///
    /// `None` on a partial install, and the battlefield then draws its own flat
    /// ground and a block for each man. That keeps every input arm testable
    /// without the install, and it is safe here for a reason the campaign map's
    /// hit test was not (`docs/decisions.md` C61): **every hotspot on this
    /// screen is a constant out of the binary**, not a consequence of the
    /// artwork, so the placeholder and the real install hit-test identically.
    pub battle: Option<l2_view::scene::BattleAssets>,
    /// What the shell screens draw with: `L2.eng`, the two panel fonts, and
    /// the per-screen artwork the front end and the management screens load.
    /// See [`crate::shell`].
    pub shell: ShellAssets,
    /// `L2_maps.dat` whole. A `MapSlot` borrows its file, so the bytes are kept
    /// and the slot is re-parsed on demand — which is bounds arithmetic, not
    /// decoding, and costs nothing.
    maps: Vec<u8>,
    /// The `MAPnn.PL8` files, by file number 1..=15, unparsed. Four map slots
    /// live in each and only one is ever wanted at a time, so they are decoded
    /// on demand by [`Assets::minimap`] and cached by the screen.
    minimap_files: Vec<Option<Vec<u8>>>,
}

impl Assets {
    /// Load through the mod overlay, so a mod that supplies its own `Base2a.pl8`
    /// or its own palette is picked up with no change to any drawing path.
    pub fn load(vfs: &Vfs) -> Result<Assets, String> {
        let maps = vfs.read("L2_maps.dat").map_err(|e| format!("L2_maps.dat: {e}"))?;
        MapSet::parse(&maps).map_err(|e| format!("L2_maps.dat: {e}"))?;
        let palette = vfs
            .palette(campaign::PALETTE)
            .map_err(|e| format!("{}: {e}", campaign::PALETTE))?;
        let map = MapAssets::load(|name| vfs.read(name).map_err(|e| format!("{name}: {e}")))?;
        // The chrome is optional: a partial install still starts, with our own
        // panels instead of the original's.
        let chrome =
            Chrome::load(|name| vfs.read(name).map_err(|e| format!("{name}: {e}"))).ok();
        let village =
            VillageArt::load(|name| vfs.read(name).map_err(|e| format!("{name}: {e}"))).ok();
        let battle = l2_view::scene::BattleAssets::load(
            |name| vfs.read(name).map_err(|e| format!("{name}: {e}")),
            l2_view::figures::Colour::Red,
            l2_view::figures::Colour::Blue,
        )
        .ok();
        // The game ships 11 of the 15 `MAPnn.PL8` names; the four it does not
        // are exactly the empty map slots 24..39 (`docs/screens.md` §3.1).
        let minimap_files = (0..16)
            .map(|n| vfs.read(&Minimap::file_for_slot(n * 4)).ok())
            .collect();
        Ok(Assets {
            ink: Ink::for_palette(&palette),
            quirks: Quirks::default(),
            palette,
            map,
            chrome,
            village,
            battle,
            shell: ShellAssets::load(vfs),
            maps,
            minimap_files,
        })
    }

    pub fn slot(&self, index: usize) -> Option<MapSlot<'_>> {
        MapSet::parse(&self.maps).ok()?.slot(index).ok()
    }

    /// The two 128 x 128 minimap rasters for a map slot, or `None` when the
    /// install has no `MAPnn.PL8` for it.
    pub fn minimap(&self, slot: usize) -> Option<Minimap> {
        let bytes = self.minimap_files.get(slot >> 2)?.as_ref()?;
        Minimap::load(bytes, slot).ok()
    }

    /// Assets with nothing in them: a grey ramp for a palette, one blank map
    /// slot, and five tile banks holding a single 2 x 2 frame.
    ///
    /// This is what lets the interface be tested on a machine with no copy of
    /// the game — every screen still lays out, every button is still where it
    /// is, and every assertion about *structure* still holds. Assertions about
    /// the shipped artwork need the install and live in the tests that skip
    /// without it.
    pub fn placeholder() -> Assets {
        // 256 greys, in the 6-bit range a `.256` file holds.
        let mut palette_bytes = vec![0u8; Palette::FILE_LEN];
        for i in 0..256usize {
            let v = (i / 4) as u8;
            palette_bytes[i * 3] = v;
            palette_bytes[i * 3 + 1] = v;
            palette_bytes[i * 3 + 2] = v;
        }
        let palette = Palette::from_bytes(&palette_bytes).expect("768 bytes");

        // The smallest legal PL8: one raw 2 x 2 frame.
        let mut pl8 = vec![0u8; 8 + 16];
        pl8[2] = 1; // one frame
        pl8[8] = 2; // width
        pl8[10] = 2; // height
        pl8[12..16].copy_from_slice(&24u32.to_le_bytes());
        pl8.extend_from_slice(&[1, 2, 3, 4]);
        let map = MapAssets::load(|_| Ok(pl8.clone())).expect("a synthetic sheet parses");

        Assets {
            ink: Ink::for_palette(&palette),
            quirks: Quirks::default(),
            palette,
            map,
            chrome: None,
            village: None,
            battle: None,
            shell: ShellAssets::empty(),
            maps: vec![0u8; l2_formats::maps::SLOT_LEN],
            minimap_files: vec![None; 16],
        }
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
    /// **Here rather than on `Realm`, and it changes what it is covered by.**
    /// The original keeps names outside `g_realms` too — they are a save block
    /// of their own — and the reason holds for us: the lockstep digest is
    /// `Canonical::hash_of(kingdom)` and a name cannot change a number. Putting
    /// it in the kingdom would make a cosmetic string a desync source. It is in
    /// the save, in the prefix beside [`Game::realm_colour`], which is the
    /// other per-realm thing the interface draws and the rules never read.
    pub player_names: [crate::text::PlayerName; MAX_REALMS],
    /// The county under the cursor's last click, or 0 for none. County ids are
    /// 1-based in the original, so 0 is a usable "nothing".
    pub selected: u8,
    /// County `+0x6C`, `+0x6D` — each county's anchor tile, which is where its
    /// marker is drawn. Two arrays rather than an array of pairs: index order
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
    /// **Whether this game is over, and where it sits in its campaign.**
    ///
    /// The three globals a campaign is made of — `DAT_0053F258`, `DAT_0053F640`
    /// and `DAT_0053F0C4` — plus the ending messages the current map has raised.
    /// It is here rather than in [`Kingdom`] because the original keeps it here
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
    /// It is here rather than in the caller's hands because a half-run turn is
    /// not something a caller may drop: the kingdom is in a state no rule
    /// describes — two armies on one tile with the battle unresolved — and the
    /// only safe thing to do with it is finish it. See [`crate::turn`].
    ///
    /// not-encoded: session state, and the same argument as
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
    /// projection** — and `crates/l2-game/tests/options.rs` asserts the
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
    /// the campaign map and nowhere else, so a levy is never half-made when a
    /// file is written. See [`LevyOrder`].
    ///
    /// not-encoded: session state. The durable half — the realm's weapon stocks
    /// the basket was seeded from — is in the kingdom already.
    pub levy: LevyOrder,
    /// **The battle the player is watching**, or `None`, which is almost
    /// always.
    ///
    /// `docs/plan.md`: *"A `Game` holds the kingdom, the active battle if any,
    /// and the screen stack."* This is that. It is here rather than inside
    /// [`crate::screens::battlefield::BattlefieldScreen`] for the same reason
    /// [`Game::turn`] is here rather than in the caller's hands: a screen is
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
    /// not-encoded: session state, and the paragraph above is the whole
    /// argument — the original cannot save inside a battle either.
    pub battle: Option<Box<crate::battlefield::LiveBattle>>,
    /// **`Map_BeginMoveSelection`, asked for by something that is not the map.**
    ///
    /// `Panel_MoveButton` (`0x004371CE`) is two statements — `g_screenId = 0`
    /// and `Map_BeginMoveSelection()` — because in the original the selection
    /// is a global and the screen is a byte. Ours has neither: move-order mode
    /// is [`crate::screens::map::MapScreen`]'s own state, and the information
    /// panel that holds the button is a *different screen* with no handle on
    /// it. So the panel writes the request here and pops, and the map picks it
    /// up on its next tick — which is the same two steps in the same order.
    ///
    /// `None` almost always: it is consumed by the frame after it is written.
    ///
    /// not-encoded: session state. It cannot outlive the frame that set it, and
    /// the original's `g_selectedUnit` is not saved either.
    pub begin_move_order: Option<usize>,
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
    /// second copy that drifted. `crates/l2-game/tests/right_column.rs` asserts
    /// the projection holds, which is what stops it becoming a field the map
    /// forgets to write.
    ///
    /// not-encoded: presentation. Which zoom a person is looking at cannot
    /// change a number in the world.
    pub map_zoom_far: bool,
}

/// `g_levyPercent`, `g_levyMen`, `g_levyHappinessCost`, `g_levyBasket` and
/// `DAT_0055446C` — **one order, three screens.**
///
/// The original has no stack. `g_screenId` is a byte, and the raise-army screen
/// (`0x17`), the armoury (`0x0A`) and one weapon's rack (`0x0D`) are three
/// values of it that all read and write the same globals: the slider on `0x17`
/// writes `men` and `happiness_cost`, the `+`/`−` on `0x0D` move men between
/// `basket` slots, and `Army_RaiseConfirm` — which is a button on the
/// **armoury**, not on the levy screen — spends the lot.
///
/// So it cannot live in a screen. Our machine destroys a screen the moment it
/// is replaced, and `0x17 → 0x0A → 0x17` is two replacements; anything the
/// player chose in between would go with them. It lives here because it lives
/// in the original's data segment, which is the same reason.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LevyOrder {
    /// The county being levied. 0 when there is no order.
    pub county: u8,
    /// `g_levyPercent` (`0x0056D65C`) — **where the player put the slider**,
    /// which is not necessarily what the county gave up.
    ///
    /// `Sidebar_Button` does not reset it: it calls `Levy_SetPercent(county,
    /// g_levyPercent)` with whatever the last levy left there, so opening the
    /// screen for a second county starts at the first county's percentage.
    /// Reproduced — [`Game::open_levy`] takes no percentage.
    pub percent: i32,
    /// `g_levyMen` (`0x00543FD8`) and `g_levyHappinessCost` (`0x00565400`),
    /// both written by `Levy_SetPercent` and by nothing else.
    pub men: i32,
    pub happiness_cost: i32,
    /// `g_levyBasket[g_localPlayer]` (`0x0053F6A0`) — who is carrying what.
    pub basket: l2_kingdom::LevyBasket,
    /// `DAT_0055446C` — the mercenary hire flag, cleared by `Sidebar_Button`
    /// every time the screen opens and toggled by the tick and cross on it.
    pub hire: bool,
    /// `DAT_00553F20` — the rack the player last opened, 1…6, or 0 for none.
    /// `FUN_004AA90A` clears it whenever the basket is re-seeded.
    pub rack: u8,
}

impl Game {
    /// An empty world. The scenario loader fills it; nothing else should
    /// construct a half-populated one.
    pub fn new(seed: u64) -> Game {
        Game {
            kingdom: Kingdom::new(seed),
            player: 1,
            map_slot: 0,
            realm_colour: [0; MAX_REALMS],
            // Empty, not "Player1": `Options_SetDefaults` seeds the persisted
            // *settings* block with a default name and new-game setup is what
            // copies a name into `g_playerNames`. A world nobody has set up has
            // no lords in it to be called anything.
            player_names: [crate::text::PlayerName::EMPTY; MAX_REALMS],
            selected: 0,
            anchor_x: [0; MAX_COUNTIES],
            anchor_y: [0; MAX_COUNTIES],
            gold_last: [0; MAX_REALMS],
            last_report: None,
            turns_played: 0,
            campaign: crate::victory::Campaign::new(crate::victory::Track::First),
            field_policy: crate::engagement::Answer::Decline,
            turn: None,
            prefs: Prefs::default(),
            presentation_quirks: Quirks::default(),
            levy: LevyOrder::default(),
            battle: None,
            begin_move_order: None,
            map_zoom_far: false,
        }
    }

    // ------------------------------------------------------------ the levy

    /// `Sidebar_Button`'s hotspot 1 (`0x0043AE30`) — **open the levy.**
    ///
    /// ```c
    /// if (county.owner != g_localPlayer) { Msg_Enqueue(0x70); return; }
    /// Levy_SetPercent(county, g_levyPercent);     /* the slider is NOT reset */
    /// FUN_004AA90A(county, g_levyMen);            /* seed the basket        */
    /// g_screenId = 0x17;  DAT_005679D0 = 0;  DAT_0055446C = 0;
    /// ```
    ///
    /// Returns false for a county that is not the player's, which is the
    /// message-`0x70` arm.
    pub fn open_levy(&mut self, county: u8) -> bool {
        if !self.is_players(county) {
            return false;
        }
        self.levy.county = county;
        self.set_levy_percent(self.levy.percent);
        self.seed_levy_basket();
        self.levy.hire = false;
        true
    }

    /// `Levy_SetPercent(g_selectedCounty, g_levyPercent)` — the whole of what
    /// the slider does. **It does not touch the basket**; `Levy_SliderClick`'s
    /// tail is this call and a redraw request, and nothing else.
    pub fn set_levy_percent(&mut self, percent: i32) {
        self.levy.percent = percent.clamp(0, 100);
        let Some(county) = self.kingdom.counties.get(self.levy.county as usize) else { return };
        let levy = l2_kingdom::levy::set_percent(&self.kingdom.tables, county, self.levy.percent);
        self.levy.men = levy.men;
        self.levy.happiness_cost = levy.happiness_cost;
    }

    /// `FUN_004AA90A(county, g_levyMen)` — re-seed the basket from the realm's
    /// weapon stocks and the levy's headcount, and forget the selected rack.
    ///
    /// **Every door into the armoury calls it.** `Sidebar_Button` on the way in
    /// to `0x17`, `FUN_00435CBF` on the *Continue* button, and the right-release
    /// arm of `0x17`. So walking back to the levy screen and forward again
    /// throws away everything the player equipped — the original's behaviour,
    /// and the reason a slider move appears to strip the army even though the
    /// slider itself never touches the basket.
    pub fn seed_levy_basket(&mut self) {
        let realm = self.player as usize;
        let Some(realm) = self.kingdom.realms.get(realm) else { return };
        self.levy.basket = l2_kingdom::LevyBasket::seed(realm, self.levy.men);
        self.levy.rack = 0;
    }

    /// Whether this game has ended, and how. `DAT_0053F0C4`.
    pub fn outcome(&self) -> l2_kingdom::victory::Outcome {
        self.campaign.outcome
    }

    /// `FUN_0049B42B` for one realm, then `Score_RankRealms` — the ending chain's
    /// two halves in the order the original runs them, with the messages landing
    /// in [`Game::campaign`].
    ///
    /// **Called for every realm, the human included.** `AI_RunTurnStep`'s
    /// `isHuman` test guards the fourteen handlers, not the step-0
    /// initialisation above them, so the human's strength is recounted and the
    /// human's defeat detected on the human's own turn. Skipping humans here is
    /// the one way to build a game that cannot be lost.
    pub fn recount_realm(&mut self, realm: u8) {
        let msg = l2_kingdom::victory::recount_strength(
            &mut self.kingdom.realms,
            &self.kingdom.counties,
            self.kingdom.county_count,
            &self.kingdom.campaign.units,
            realm,
            self.player,
        );
        if let Some(msg) = msg {
            self.campaign.raise(msg);
        }
        self.rank_realms();
    }

    /// `Score_RankRealms`, with its three globals kept.
    pub fn rank_realms(&mut self) {
        let mut out = Vec::new();
        self.campaign.ranking = l2_kingdom::victory::rank_and_crown(
            &self.kingdom.tables,
            &mut self.kingdom.realms,
            self.player,
            self.kingdom.options.quirks,
            &mut out,
        );
        for msg in out {
            self.campaign.raise(msg);
        }
    }

    /// The player's treasury.
    pub fn gold(&self) -> i32 {
        self.kingdom.realms.get(self.player as usize).map_or(0, |r| r.gold)
    }

    /// What the treasury did over the last end-of-turn.
    pub fn gold_change(&self) -> i32 {
        self.gold() - self.gold_last.get(self.player as usize).copied().unwrap_or(0)
    }

    /// Counties held by a realm, counted in index order.
    pub fn owned_by(&self, realm: u8) -> usize {
        self.kingdom
            .county_ids()
            .filter(|&id| self.kingdom.counties[id].owner == realm)
            .count()
    }

    pub fn is_county(&self, id: u8) -> bool {
        id >= 1 && (id as usize) <= self.kingdom.county_count
    }

    /// Whether the player may give this county orders. Setting another realm's
    /// taxes is not a thing the interface refuses for tidiness; it is not the
    /// player's county.
    pub fn is_players(&self, id: u8) -> bool {
        self.is_county(id) && self.kingdom.counties[id as usize].owner == self.player
    }

    /// Whether the player may give this **unit** orders.
    ///
    /// Ownership, not type: a player's merchant does not exist (merchants are
    /// realm 6's), but a peasant mob or a transport of the player's realm is
    /// theirs to move, and the original's map click does not check the type
    /// either.
    pub fn is_players_unit(&self, unit: usize) -> bool {
        self.kingdom.campaign.units.get(unit).is_some_and(|u| u.owner == self.player)
    }

    /// `Unit_OrderMove` (`0x004A7EEC`) — **the player's move order**, and the
    /// way anything on the campaign map is set walking from outside the turn
    /// machine.
    ///
    /// Three things it is, each of which is a rule rather than a convenience:
    ///
    /// * **[`Routing::Direct`]**, because a human order uses the cost map as it
    ///   stands. Road-hugging is what the game does for its own units — the AI's
    ///   armies, the merchants, the transports — and
    ///   [`l2_kingdom::movement::Routing::PreferRoads`] records that asymmetry.
    /// * **it starts the unit immediately**, `moving = 2` in the original, so
    ///   the army walks on the next tick whatever phase is current. There is no
    ///   phase for player movement, which is the finding
    ///   [`crate::turn`] is built on.
    /// * **the cost map is rebuilt for the order**, inside
    ///   [`l2_kingdom::movement::order_move`], so a tile trampled two steps ago
    ///   is already impassable to this one.
    ///
    /// Returns the number of steps ordered, or `None` if the unit is not the
    /// player's or no path reaches the tile. A refusal changes nothing.
    pub fn order_unit_move(&mut self, unit: usize, dest: (u8, u8)) -> Option<usize> {
        if !self.is_players_unit(unit) {
            return None;
        }
        l2_kingdom::movement::order_move(
            &self.kingdom.campaign.map,
            &mut self.kingdom.campaign.units,
            unit,
            dest,
            l2_kingdom::movement::Routing::Direct,
        )
    }

    /// The player's units, in ascending slot order — what a map screen would
    /// draw and cycle through.
    pub fn player_units(&self) -> Vec<usize> {
        self.kingdom
            .campaign
            .units
            .iter()
            .filter(|(_, u)| u.owner == self.player)
            .map(|(id, _)| id)
            .collect()
    }

    /// The unit standing on a tile, if any. `Map_ResolvePick`'s
    /// `g_pickedTileUnit`, which is what every branch of [`Map_Click`] tests
    /// first.
    ///
    /// [`Map_Click`]: crate::screens::map
    pub fn unit_at(&self, x: u8, y: u8) -> Option<usize> {
        self.kingdom.campaign.units.at(x, y)
    }

    /// **Raise an army** — `FUN_00435B4D`, the raise-army screen's yes-button,
    /// end to end.
    ///
    /// ```c
    /// if (levyTotal == 0   && !hireMercs) message 0xA8;   /* group 168 */
    /// else if (levyTotal < 0x32 && !hireMercs) message 0x94;   /* group 148 */
    /// else if (Army_Create(localPlayer, county, hireMercs, g_levyHappinessCost) == 0)
    ///     message 0xDD;                                   /* group 221 */
    /// ```
    ///
    /// The two size guards are `&&`-ed with `hireMercs`, so **hiring a band
    /// bypasses both**: the band supplies the men and a levy of nothing is a
    /// legal army. [`l2_kingdom::levy::refuse_levy`] is that pair of guards and
    /// this is its only caller.
    ///
    /// `hire` is the county's standing offer, `county +0x1AD`, or `None` for a
    /// pure levy. The price is **not** checked inside
    /// [`l2_kingdom::MercenaryBands::hire`] — the screen refuses first, with
    /// `L2.eng` 69/3 — so this checks it here rather than taking the treasury
    /// negative.
    ///
    /// Returns the new army's slot.
    pub fn raise_army(
        &mut self,
        county: u8,
        basket: &l2_kingdom::LevyBasket,
        happiness_cost: i32,
        hire: Option<u8>,
    ) -> Result<usize, l2_kingdom::LevyRefusal> {
        if !self.is_players(county) {
            return Err(l2_kingdom::LevyRefusal::NowhereToStand);
        }
        if let Some(no) = l2_kingdom::levy::refuse_levy(basket.total(), hire.is_some()) {
            return Err(no);
        }
        let k = &mut self.kingdom;
        let muster = l2_kingdom::levy::Muster {
            realm: self.player,
            county,
            happiness_cost,
            year: k.year,
        };
        let id = l2_kingdom::levy::create_army(
            &k.tables,
            &k.campaign.map,
            &mut k.counties,
            &mut k.realms,
            &mut k.campaign.units,
            &mut k.campaign.names,
            basket,
            muster,
        )?;
        // `Mercenary_Hire` runs from **inside** `Army_Create`, after the men
        // and the troop counts are written and before the wage recount. Ours
        // runs immediately after, which lands the same numbers because nothing
        // between the two reads `men`.
        if let Some(band) = hire {
            let k = &mut self.kingdom;
            let l2_kingdom::Kingdom { counties, realms, campaign, .. } = k;
            campaign.mercenaries.hire(&mut campaign.units, counties, realms, id, band);
            l2_kingdom::unit::refresh_wages(
                &k.tables,
                &mut k.campaign.units,
                &mut k.realms,
                self.player,
                0,
            );
        }
        Ok(id)
    }

    /// **Split an army** — `FUN_00437AFB` then `Army_Split` (`0x00437FD7`).
    /// See [`l2_kingdom::divide`].
    pub fn split_army(
        &mut self,
        army: usize,
        basket: &l2_kingdom::SplitBasket,
        into: l2_kingdom::SplitInto,
    ) -> Result<usize, l2_kingdom::SplitRefusal> {
        if !self.is_players_unit(army) {
            return Err(l2_kingdom::SplitRefusal::NotAnArmy);
        }
        let k = &mut self.kingdom;
        let l2_kingdom::Kingdom { tables, counties, realms, campaign, year, .. } = k;
        let l2_kingdom::kingdom::Campaign { units, map, mercenaries, names, .. } = campaign;
        l2_kingdom::divide::split(
            tables, map, counties, realms, units, names, mercenaries, army, basket, into, *year,
        )
    }

    /// **Disband an army** — `Panel_DisbandButton` (`0x0043733A`) then
    /// `Army_Disband` (`0x00438681`). Returns the county the men joined and how
    /// many joined it.
    pub fn disband_army(&mut self, army: usize) -> Result<(u8, i32), l2_kingdom::DisbandRefusal> {
        if !self.is_players_unit(army) {
            return Err(l2_kingdom::DisbandRefusal::NotAnArmy);
        }
        let k = &mut self.kingdom;
        let difficulty = k.options.difficulty;
        let l2_kingdom::Kingdom { tables, counties, realms, campaign, .. } = k;
        let l2_kingdom::kingdom::Campaign { units, mercenaries, names, .. } = campaign;
        l2_kingdom::divide::disband(
            tables, counties, realms, units, names, mercenaries, army, difficulty,
        )
    }

    /// Select a county, or clear the selection with 0. An id that is not a
    /// county on this map is refused rather than stored.
    pub fn select(&mut self, id: u8) -> bool {
        if id == 0 {
            self.selected = 0;
            return true;
        }
        if !self.is_county(id) {
            return false;
        }
        self.selected = id;
        true
    }

    /// Set a county's tax rate, clamped. Returns false, and changes nothing,
    /// for a county the player does not hold.
    pub fn set_tax_rate(&mut self, id: u8, rate: i32) -> bool {
        if !self.is_players(id) {
            return false;
        }
        self.kingdom.counties[id as usize].tax_rate = rate.clamp(0, MAX_TAX_RATE);
        true
    }

    /// Set a county's wanted ration level, clamped to the six the table holds.
    ///
    /// It writes `rationWanted` (`+0x15E`), never `rationAchieved` (`+0x15D`):
    /// what the player asks for and what the county's stores could actually
    /// feed are different fields, and only the season pipeline decides the
    /// second one.
    pub fn set_ration(&mut self, id: u8, level: i32) -> bool {
        if !self.is_players(id) {
            return false;
        }
        self.kingdom.counties[id as usize].ration_wanted =
            level.clamp(0, RATION_LEVEL_COUNT as i32 - 1);
        true
    }

    /// Set a county's grain-to-livestock split (`+0x15F`), clamped 0 … 100.
    ///
    /// The third order the original's ration panel gives, and the only one of
    /// the three that is a slider rather than a pair of arrows.
    ///
    /// **`sweep` is 1 for a jump on the track and 0 for an arrow**, and it is
    /// `Ration_SliderClick`'s `g_uiHotspotArg`. The two gestures end
    /// differently and a player can see the difference, so it is a parameter
    /// and not a detail — see [`l2_kingdom::Kingdom::set_ration_split`], which
    /// is the whole of the rule.
    ///
    /// This used to write the field and stop, and its own doc comment said so:
    /// *"we write the field and stop, because our food pass only runs at end of
    /// turn."* A player reported the result as **"rations slider moves but is
    /// inoperable"**, which it was — the thumb travelled and every number on
    /// the panel stayed where it was. The original re-runs the food pass on the
    /// spot, searches for a split that actually changes something, reallocates
    /// the county twice and repaints the panel.
    ///
    /// Returns whether the split ended anywhere other than where it started.
    pub fn set_ration_split(&mut self, id: u8, split: i32, sweep: bool) -> bool {
        if !self.is_players(id) {
            return false;
        }
        self.kingdom.set_ration_split(id as usize, split, sweep)
    }

    /// Move peasants from one job to another — the village screen's only order.
    ///
    /// `Labour_Move` (`0x00439B52`), and its caller `FUN_004399B0` which is
    /// where the arithmetic actually is:
    ///
    /// ```c
    /// workers = selectedIcons * county[+0xB8];
    /// if (labour[from] < workers) workers = labour[from];
    /// labour[to] += workers; labour[from] -= workers;
    /// ```
    ///
    /// So **an icon is `popBand` people**, and dragging every icon out of a job
    /// takes every worker out of it even when `icons * popBand` overshoots.
    /// Returns how many people actually moved.
    ///
    /// **What this does not do**, and the original does: `Labour_Move` re-runs
    /// the county's food pass, its industry estimates and its labour-share
    /// recompute *twice* before returning, so the whole panel is live the
    /// instant you let go. Ours runs those at end of turn, so the numbers a
    /// drag changes are the worker counts and nothing else. That is the same
    /// choice [`Game::set_ration_split`] already documents.
    pub fn move_labour(&mut self, id: u8, from: usize, to: usize, icons: i32) -> i32 {
        let band = self.kingdom.counties.get(id as usize).map_or(0, |c| c.pop_band);
        self.move_workers(id, from, to, icons.saturating_mul(band))
    }

    /// The same move counted in **people** rather than icons.
    ///
    /// `Labour_Move` itself takes a worker count; it is `Village_Drop` that
    /// multiplies by `popBand` and clamps. The double click
    /// (`Village_BalanceJob`, `0x00439F6A`) does not go through icons at all —
    /// it moves exactly the shortfall or exactly the surplus — so the two
    /// callers need the two shapes, and [`Game::move_labour`] is now this
    /// function with the icon arithmetic in front of it.
    pub fn move_workers(&mut self, id: u8, from: usize, to: usize, workers: i32) -> i32 {
        if !self.is_players(id) || from == to || workers <= 0 {
            return 0;
        }
        let c = &mut self.kingdom.counties[id as usize];
        let (Some(&held), true) = (c.labour.get(from), to < c.labour.len()) else {
            return 0;
        };
        let workers = workers.min(held).max(0);
        c.labour[to] += workers;
        c.labour[from] -= workers;
        workers
    }

    /// **The double click on the village: balance one job against the idle
    /// pool.** `Village_BalanceJob` (`0x00439F6A`), given a *cluster*.
    ///
    /// One gesture, two directions, and which one it is depends on the job:
    ///
    /// * a job **below its wanted floor** takes people *from* the idle
    ///   townsfolk — as many as it is short, or as many as are idle, whichever
    ///   is fewer;
    /// * a job **above its useful ceiling** puts the surplus *back* into the
    ///   idle townsfolk. That is the one the player asked for: *"I can't double
    ///   click idle peasants in a task to remove them from the task."*
    ///
    /// `fill` is the original's third argument. With it clear the shortfall
    /// branch is skipped entirely, so the job can only *shed* — which is how
    /// [`Game::balance_all_labour`] empties every job before refilling any.
    ///
    /// The floor is ignored when it is not positive and the ceiling when it is
    /// [`l2_kingdom::county::LABOUR_CEILING_IGNORED`] or above, exactly as the
    /// original's two guards do. Returns how many people moved.
    pub fn balance_labour(&mut self, id: u8, cluster: usize, fill: bool) -> i32 {
        let Some(c) = self.kingdom.counties.get(id as usize) else { return 0 };
        let slot = l2_view::village::slot_for_cluster(
            cluster,
            c.industry[3].has_resource,
            c.industry[1].has_resource,
        );
        let wanted = c.labour_wanted[slot];
        let useful = c.labour_useful[slot];
        let workers = c.labour[slot];
        let short = if wanted < 1 { 0 } else { wanted - workers };
        let surplus = if useful < LABOUR_CEILING_IGNORED { workers - useful } else { 0 };
        let idle = c.labour[JOB_IDLE_TOWNSFOLK];

        if short < 1 || !fill {
            if surplus < 1 {
                return 0;
            }
            self.move_workers(id, slot, JOB_IDLE_TOWNSFOLK, surplus)
        } else {
            if idle == 0 {
                return 0;
            }
            self.move_workers(id, JOB_IDLE_TOWNSFOLK, slot, idle.min(short))
        }
    }

    /// **A double click on the idle townsfolk themselves: put everybody to
    /// work.** `Village_BalanceAll` (`0x00439EDB`)'s cluster-6 branch.
    ///
    /// Two passes, and the order is the whole point: every job **sheds** its
    /// surplus into the pool first, and only then does every job draw from the
    /// pool to fill its shortfall. One pass would let whichever job came first
    /// take people the later ones needed.
    ///
    /// Ten clusters, not eight — see
    /// [`l2_view::village::CLUSTER_TO_SLOT_BALANCE`].
    pub fn balance_all_labour(&mut self, id: u8) -> i32 {
        let clusters = l2_view::village::CLUSTER_TO_SLOT_BALANCE.len();
        let mut moved = 0;
        for fill in [false, true] {
            for cluster in 0..clusters {
                moved += self.balance_labour(id, cluster, fill);
            }
        }
        moved
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

        let achieved = g.kingdom.counties[1].ration_achieved;
        g.set_ration(1, 99);
        assert_eq!(g.kingdom.counties[1].ration_wanted, RATION_LEVEL_COUNT as i32 - 1);
        g.set_ration(1, -3);
        assert_eq!(g.kingdom.counties[1].ration_wanted, 0);
        assert_eq!(
            g.kingdom.counties[1].ration_achieved, achieved,
            "what the player asks for (+0x15E) is not what the county managed to feed (+0x15D)"
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

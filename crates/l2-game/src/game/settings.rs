#![allow(unused_imports)]
use super::*;
use super::quirk_set::*;
use super::assets::*;
use super::unit_frames::*;
use super::levy::*;
use l2_kingdom::county::MAX_COUNTIES;
use l2_kingdom::realm::MAX_REALMS;
use l2_kingdom::{Kingdom, SeasonReport};
use l2_kingdom::tables::MAX_TAX_RATE;
use l2_kingdom::county::MAX_RATION_SPLIT;

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
/// # The original keeps these in a file, and so do we
///
/// `Options_Save` (`0x004AE15F`) writes the whole `g_options` block —
/// **0x468 bytes, and the shipped `lords2.inf` is
/// single call site on the shutdown path, so the original loses every setting
/// changed that session if it crashes. `Options_Load` (`0x004AE1B8`) reads it
/// back and `Options_Validate` (`0x004AE2BD`) checks `g_optionsMagic`
/// (`0x0053F204`) against **0x7EC** *after* the read, defaulting the whole block
/// when it does not match. `[V]` on all of it.
///
/// [`crate::prefs`] is ours: the same two moments, a text file beside our
/// saves, one key per field named after the global. **`L2.INI` is not that
/// file** — `Ini_ReadConfig` (`0x0040ECC4`) reads `WindowsDir=` out of it and
/// nothing else, and nothing in the image writes it.
///
/// A new game and a load carry this block forward rather than defaulting it,
/// because `g_optTipScreens` has two writers in the image and neither is
/// `Game_NewGame` or the loader.
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
    /// `g_optFullScreen` (`0x0053F228`). `Display_Init` (`0x0042E5C0`) forces 1
    /// on any desktop that is not 8bpp. Our windowed mode is [D], C238.
    pub full_screen: bool,
    /// The rest of `g_options` (`0x0053F1E0`, 0x468 bytes) we carry.
    pub kept: crate::prefs::Kept,
    /// [D] ours, C257: this install's id, generated once; a resumed network game seats by it.
    pub install: crate::net::InstallId,
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
            full_screen: true,
            kept: crate::prefs::Kept::default(),
            install: crate::net::InstallId::NIL,
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


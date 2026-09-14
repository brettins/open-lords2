#![allow(unused_imports)]
use super::*;

use super::*;
use super::words::*;
use super::screen::*;
use l2_view::Canvas;
use crate::game::PRESENTATION;
use crate::input::{Event, Key, Rect};
use crate::press::{Kind, Press, Widget};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::shell::{self, font, Pen};

impl Page {
    /// `g_screenId`, or `None` for the page that is ours.
    pub fn screen_id(self) -> Option<u8> {
        match self {
            Page::Advanced => Some(0x39),
            Page::Sound => Some(0x42),
            Page::Display => Some(0x43),
            Page::Help => Some(0x31),
            Page::Quirks => None,
        }
    }

    /// The painter, for anyone going back to the binary.
    pub fn painter(self) -> Option<u32> {
        match self {
            Page::Advanced => Some(0x0041_4F68),
            Page::Sound => Some(0x0041_515C),
            Page::Display => Some(0x0041_52EA),
            Page::Help => Some(0x0041_54EA),
            Page::Quirks => None,
        }
    }

    /// The `L2.eng` group whose index 0 is the heading.
    pub fn group(self) -> Option<usize> {
        match self {
            Page::Advanced => Some(GROUP_ADVANCED),
            Page::Sound => Some(GROUP_SOUND),
            Page::Display => Some(GROUP_DISPLAY),
            Page::Help => Some(GROUP_HELP),
            Page::Quirks => None,
        }
    }

    /// `Ui_DrawBox(x, y, cols, rows)` in cells of sixteen pixels, and the border
    /// set — all four panels use `FUN_004093E0`, which is set 1.
    pub fn window(self) -> (i32, i32, i32, i32, usize) {
        match self {
            Page::Advanced => (0x30, 0x60, 0x18, 0x0D, 1),
            Page::Sound => (0x30, 0x60, 0x18, 0x0C, 1),
            Page::Display => (0x30, 0x90, 0x18, 0x0A, 1),
            Page::Help => (0x60, 0x80, 0x16, 0x0B, 1),
            // Ours, and the widest of the five because it lists a lot of rows.
            Page::Quirks => (0x10, 0x20, 0x26, 0x1A, 1),
        }
    }

    /// The heading's baseline.
    pub fn heading_at(self) -> (i32, i32) {
        match self {
            Page::Advanced => (0x40, 0x74),
            Page::Sound => (0x40, 0x74),
            Page::Display => (0x40, 0xA4),
            Page::Help => (0x80, 0x94),
            Page::Quirks => (0x20, 0x44),
        }
    }

    /// `Ui_OkButton(x, y, mode)` — the corner picture that closes the panel.
    pub fn close_at(self) -> (i32, i32) {
        match self {
            Page::Advanced => (0x188, 0x100),
            Page::Sound => (0x188, 0xF0),
            Page::Display => (0x188, 0x100),
            Page::Help => (0x194, 0x106),
            Page::Quirks => (0x228, 0x1B0),
        }
    }

    /// The close button's hit box.
    pub fn close_hit(self) -> Rect {
        let (x, y) = self.close_at();
        Rect::new(x, y, WIDGET, WIDGET)
    }

    /// The rows the painter draws, in the painter's order. Empty for the quirks
    /// page, which is built from the catalogue instead.
    pub fn rows(self) -> &'static [Row] {
        match self {
            Page::Advanced => ADVANCED,
            Page::Sound => SOUND,
            Page::Display => DISPLAY,
            Page::Help => HELP,
            Page::Quirks => &[],
        }
    }

    /// The page's widget table — `g_advancedOptWidgets` and its three siblings —
    /// in record order, which is the order [`Page::rows`] is in.
    pub fn widgets(self) -> Vec<Widget> {
        self.rows().iter().map(Row::widget).collect()
    }

    /// Whether this page is the original's or ours.
    pub fn is_ours(self) -> bool {
        self == Page::Quirks
    }

    /// The five, in the order the demo index offers them.
    pub const ALL: [Page; 5] =
        [Page::Advanced, Page::Sound, Page::Display, Page::Help, Page::Quirks];
}

// ---------------------------------------------------------------------------
// Reading and writing a setting
// ---------------------------------------------------------------------------

/// What a row's state word says right now.
///
/// The four *rule* settings come off [`l2_kingdom::kingdom::Options`], which is
/// the world's; the sound and interface settings are the machine's and come off
/// [`Prefs`]. That split is not cosmetic — the first four are in the save body
/// and in the lockstep digest and the rest are not, which is the same line
/// `docs/bugs.md` §6.3a draws between the two quirk sets.
pub fn value(setting: Setting, ctx: &Ctx) -> bool {
    let o = &ctx.game.kingdom.options;
    match setting {
        Setting::AdvancedFarming => o.advanced_farming,
        Setting::ArmyForaging => o.armies_eat,
        Setting::Exploration => o.exploration,
        // Inverted in the original: the byte is 0 when the option displays
        // *Yes*. `l2_kingdom::battle::settlement` tests the byte against 0
        //
        // is turned round for a reader.
        Setting::FightHumansOnly => o.fight_humans_only_byte == 0,
        Setting::Music => ctx.game.prefs.music,
        Setting::SoundEffects => ctx.game.prefs.effects,
        Setting::Speech => ctx.game.prefs.speech,
        Setting::Animations => ctx.game.prefs.animations,
        Setting::TipScreens => ctx.game.prefs.tip_screens,
        Setting::ToolTips => ctx.game.prefs.tool_tips,
        // Neither is honoured; both report the original's default so the panel
        // reads as the original's does.
        Setting::FullScreen => true,
        Setting::StartGameHelp => true,
    }
}

/// Flip a row, the way its `Opt_Toggle*` does: `x = (x != 1)`.
///
/// **What the flip reaches, row by row**, because a switch that is drawn and
/// flips a field nothing reads is a switch that does nothing, and the panel
/// cannot show the difference:
///
/// | row | read by, in this engine |
/// |---|---|
/// | Advanced farming | `l2_kingdom` — fertility, weather, the harvest, the AI's planting |
/// | Army foraging | `l2_kingdom::ration`, `unit::starve`, the county panel's OK corner |
/// | Exploration | **nothing**. Carried and saved; the fog is not built (`docs/mechanics.md`) |
/// | Fight humans only? | `l2_kingdom::battle::settlement` |
/// | Music, Sound effects, Speech | `audio::Director::listen`, every tick |
/// | Animations | **nothing**. Five readers in the original, none built |
/// | Tip screens | `tip::Tips` |
/// | Tool tips | **nothing**. The tooltip layer, `FUN_00476E95`, is not built |
///
/// The multiplayer branch of the first four — `Net_SendCommand(0x32, 0)` in
/// place of the flip, then `g_screenId = g_menuPrevScreen` — is not here, for
/// `docs/netcode.md`'s reason: the original's sync is not the authority.
pub fn toggle(setting: Setting, ctx: &mut Ctx) {
    let on = {
        let read = Ctx { game: ctx.game, assets: ctx.assets };
        value(setting, &read)
    };
    let o = &mut ctx.game.kingdom.options;
    // **Each row's arm is declared on its row** — the `arm!` in [`ADVANCED`],
    // [`SOUND`], [`DISPLAY`] and [`HELP`], beside the kind it is answered with
    // — and that is where the handler addresses are.
    match setting {
        Setting::AdvancedFarming => o.advanced_farming = !on,
        // **Not a flip alone**: the ration pass and the forecasts are re-run
        // over every county. See `Kingdom::toggle_army_foraging`.
        Setting::ArmyForaging => ctx.game.kingdom.toggle_army_foraging(),
        Setting::Exploration => o.exploration = !on,
        Setting::FightHumansOnly => o.fight_humans_only_byte = u8::from(on),
        // `Music_Stop` or the phase's bed follows from the flag on the next
        // tick: `audio::Director::listen` pushes it and `Audio::follow`
        // re-derives the bed, which is `Opt_ToggleMusic`'s two sound sites.
        Setting::Music => ctx.game.prefs.music = !on,
        Setting::SoundEffects => ctx.game.prefs.effects = !on,
        Setting::Speech => ctx.game.prefs.speech = !on,
        Setting::Animations => ctx.game.prefs.animations = !on,
        // `Opt_ToggleTipScreens` (`0x00434787`) is two statements, and the
        // second is `FUN_00476A5D()`: every tip unshown and twenty frames of
        // quiet, on the flip OFF as well as on.
        Setting::TipScreens => {
            ctx.game.prefs.tip_screens = !on;
            ctx.game.tips.reset();
        }
        // Its second statement, `_DAT_004EA830 = 0`, belongs to the tooltip
        // layer this engine does not have.
        Setting::ToolTips => ctx.game.prefs.tool_tips = !on,
        Setting::FullScreen | Setting::StartGameHelp => {}
    }
}



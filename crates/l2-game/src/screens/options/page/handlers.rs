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
    pub fn screen_id(self) -> Option<u8> {
        match self {
            Page::Advanced => Some(0x39),
            Page::Sound => Some(0x42),
            Page::Display => Some(0x43),
            Page::Help => Some(0x31),
            Page::Quirks => None,
        }
    }

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
            Page::Quirks => (0x10, 0x20, 0x26, 0x1A, 1),
        }
    }

    pub fn heading_at(self) -> (i32, i32) {
        match self {
            Page::Advanced => (0x40, 0x74),
            Page::Sound => (0x40, 0x74),
            Page::Display => (0x40, 0xA4),
            Page::Help => (0x80, 0x94),
            Page::Quirks => (0x20, 0x44),
        }
    }

    pub fn close_at(self) -> (i32, i32) {
        match self {
            Page::Advanced => (0x188, 0x100),
            Page::Sound => (0x188, 0xF0),
            Page::Display => (0x188, 0x100),
            Page::Help => (0x194, 0x106),
            Page::Quirks => (0x228, 0x1B0),
        }
    }

    pub fn close_hit(self) -> Rect {
        let (x, y) = self.close_at();
        Rect::new(x, y, WIDGET, WIDGET)
    }

    pub fn rows(self) -> &'static [Row] {
        match self {
            Page::Advanced => ADVANCED,
            Page::Sound => SOUND,
            Page::Display => DISPLAY,
            Page::Help => HELP,
            Page::Quirks => &[],
        }
    }

    pub fn widgets(self) -> Vec<Widget> {
        self.rows().iter().map(Row::widget).collect()
    }

    pub fn is_ours(self) -> bool {
        self == Page::Quirks
    }

    pub const ALL: [Page; 5] =
        [Page::Advanced, Page::Sound, Page::Display, Page::Help, Page::Quirks];
}


pub fn value(setting: Setting, ctx: &Ctx) -> bool {
    let o = &ctx.game.kingdom.options;
    match setting {
        Setting::AdvancedFarming => o.advanced_farming,
        Setting::ArmyForaging => o.armies_eat,
        Setting::Exploration => o.exploration,
        Setting::FightHumansOnly => o.fight_humans_only_byte == 0,
        Setting::Music => ctx.game.prefs.music,
        Setting::SoundEffects => ctx.game.prefs.effects,
        Setting::Speech => ctx.game.prefs.speech,
        Setting::Animations => ctx.game.prefs.animations,
        Setting::TipScreens => ctx.game.prefs.tip_screens,
        Setting::ToolTips => ctx.game.prefs.tool_tips,
        Setting::FullScreen => true,
        Setting::StartGameHelp => true,
    }
}

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
pub fn toggle(setting: Setting, ctx: &mut Ctx) {
    let on = {
        let read = Ctx { game: ctx.game, assets: ctx.assets };
        value(setting, &read)
    };
    let o = &mut ctx.game.kingdom.options;
    match setting {
        Setting::AdvancedFarming => o.advanced_farming = !on,
        Setting::ArmyForaging => ctx.game.kingdom.toggle_army_foraging(),
        Setting::Exploration => o.exploration = !on,
        Setting::FightHumansOnly => o.fight_humans_only_byte = u8::from(on),
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
        Setting::ToolTips => ctx.game.prefs.tool_tips = !on,
        Setting::FullScreen | Setting::StartGameHelp => {}
    }
}



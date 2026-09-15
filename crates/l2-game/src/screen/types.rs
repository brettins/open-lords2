#![allow(unused_imports)]
use super::*;
use super::screen::*;
use super::machine_struct::*;
use machine::*;
use l2_view::Canvas;
use crate::game::{Assets, Game};
use crate::input::Event;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScreenId {
    Menu,
    Campaign,
    County(u8, crate::screens::county::Panel),
    Village(u8),
    Job(u8, usize),
    Setup(crate::screens::setup::SetupPage),
    /// `g_screenId` `0x1E` — **the yes/no box** `Ui_OpenConfirm`
    /// (`0x0040E6F2`) opens. The question is part of the identity because in
    /// the original it is `g_confirmCallback`, and two questions are two
    /// functions. See [`crate::screens::confirm`].
    Confirm(crate::screens::confirm::Ask),
    Conquest,
    Diplomacy,
    DiploCompose(u8, u8),
    SaveLoad(crate::screens::saveload::Mode),
    Castle(u8),
    Siege(usize),
    RaiseArmy(u8),
    Armoury(u8),
    /// `g_screenId` `0x0D` — one weapon's rack, opened by clicking that weapon
    /// on the armoury's wall. The county and the troop type are both part of
    /// the identity because the original's `DAT_00553F20` is what picks the
    /// sprite sheet, the noun and the basket slot.
    Rack(u8, u8),
    Divide(usize),
    /// `DAT_00553C64` is written on the map click and read only by this
    /// screen's plaque and by the panel's price arithmetic. See
    /// [`crate::screens::merchant`].
    Merchant(usize),
    /// `g_screenId` `0x0C` — the trade panel, for one merchant and one
    /// `L2.eng` group 6 good id.
    Trade(usize, u8),
    BattlePrompt,
    BattleResult,
    Options(crate::screens::options::Page),
    Battlefield,
    /// **`g_screenId` `0x32` — a menu-bar drop-down is open**, carrying the
    /// 1-based title index the original keeps in `DAT_00522CB4`.
    ///
    /// It is a screen id in the original too, and a strange one: its painter
    /// (`Menu_RestoreBackdrop`, `0x0040C928`) only puts the 400 × 180 band at
    /// (0, 24) back. See [`crate::screens::menubar`].
    MenuBar(usize),
    About,
    Court,
    /// The category being looked at is **not** part of the identity, for
    /// `ScreenId::Diplomacy`'s reason: `DAT_0055CE7C` is a global the original
    /// keeps outside the screen, it survives the page being closed, and there
/// is one of these open. It is [`Game::nobles_category`]. See
    /// [`crate::screens::nobles`].
    Nobles,
    Supplies(u8),
    Ratings,
    /// The target is part of the identity because the original keeps it in
    /// `g_pickedTileUnit` and `DAT_0056795C` and picks the painter from them;
    /// a value that had to guess would be a value that guessed. See
    /// [`crate::screens::info`].
    Info(crate::screens::info::Target),
    /// **The message scroll.** Not a `g_screenId` at all: the original paints it
    /// over whatever is up and leaves the screen id alone, and its input arm
    /// (`Msg_HandleInput`, `0x0047685D`) runs *before* every per-screen arm.
    Message,
    /// `Tip_Show` (`0x00476DA9`) does not open a window: it writes
    /// `g_screenId = 0x27` and posts a message, and the window follows because
    /// `Msg_Pump` runs on `0x27`. So the screen the player was on stops answering
    /// input while the tip is up, and comes back when `FUN_00476E21` restores the
    /// byte on the dismissal. [`Machine`] keeps this on the stack exactly while
    /// [`crate::tip::Tips::hosting`] is true — see [`Machine::update`] — and
    /// nothing else pushes it. See [`crate::tip`] and [`crate::screens::tip`].
    Tip,
    /// **`g_screenId` `0x22` — a film is playing.** `Smk_Play` (`0x0042D91B`)
    /// parks the screen id here and `Smk_OnFinished` puts back the one it was
    /// told to return to. The film is the identity because each of `Smk_Play`'s
    /// seven callers decides what the end of it does. See
    /// [`crate::screens::movie`] and [`crate::movie`].
    Movie(crate::movie::Film),
    Index,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Transition {
    Stay,
    Push(ScreenId),
    Pop,
    Replace(ScreenId),
    Quit,
    Pass,
    /// ```text
    /// if ((leftPressed || rightPressed) && g_screenId != 0x12 && FUN_004323FE()) {
    ///     if (g_screenId == 0x0F) { Sound_StopOneShot(); FUN_0041438C(); }
    ///     if (g_battlePhase == 0) g_screenId = 0;
    /// }
    /// ```
    ///
    /// `FUN_004323FE` (`0x004323FE`) is `Minimap_Click` (`0x0043253A`) outside a
    /// battle.
    Reveal,
    /// `Smk_Play` (`0x0042D91B`) stores its fifth argument and
    /// `Smk_OnFinished` (`0x0042E060`) performs it as one statement,
    /// `g_screenId = g_smkReturnScreen;`. That is a *destination*, and it is
    /// neither of the two things our stack could already say: not [`Pop`]
    /// only knows what it is leaving, and not [`Replace`], which leaves
    /// everything underneath standing.
    ///
    /// **Six of the seven `Smk_Play` call sites pass `g_screenId` itself or the
    /// front end's `0x1F`, which in a stack is "come back where you were" —
    /// [`Pop`]. One passes a literal: `CastleBuild_Confirm` (`0x00436B59`)
    /// passes `0`, the campaign map.** `[V]`, read at each call site. So the end
    /// of a castle film is the map, and the chooser that raised it is gone with
    /// it, because the original has no stack to leave it on: `g_screenId` is one
    /// byte.
    Goto(ScreenId),
}

pub struct Ctx<'a> {
    pub game: &'a mut Game,
    pub assets: &'a Assets,
}

impl ScreenId {
    pub fn build(self) -> Box<dyn Screen> {
        match self {
            ScreenId::Menu => Box::new(crate::screens::menu::MenuScreen::new()),
            ScreenId::Campaign => Box::new(crate::screens::map::MapScreen::new()),
            ScreenId::County(id, panel) => {
                Box::new(crate::screens::county::CountyScreen::new(id, panel))
            }
            ScreenId::Village(id) => Box::new(crate::screens::village::VillageScreen::new(id)),
            ScreenId::Job(id, job) => Box::new(crate::screens::job::JobScreen::new(id, job)),
            ScreenId::Setup(page) => Box::new(crate::screens::setup::SetupScreen::new(page)),
            ScreenId::Confirm(ask) => Box::new(crate::screens::confirm::ConfirmScreen::new(ask)),
            ScreenId::Conquest => Box::new(crate::screens::conquest::ConquestScreen::new()),
            ScreenId::Diplomacy => {
                Box::new(crate::screens::diplomacy::DiplomacyScreen::new())
            }
            ScreenId::DiploCompose(target, kind) => {
                Box::new(crate::screens::diplomacy::ComposeScreen::new(target, kind))
            }
            ScreenId::SaveLoad(mode) => {
                Box::new(crate::screens::saveload::SaveLoadScreen::new(mode))
            }
            ScreenId::Castle(county) => {
                Box::new(crate::screens::castle::CastleScreen::new(county))
            }
            ScreenId::Siege(unit) => Box::new(crate::screens::siege::SiegeScreen::new(unit)),
            ScreenId::RaiseArmy(county) => {
                Box::new(crate::screens::army::RaiseArmyScreen::new(county))
            }
            ScreenId::Armoury(county) => {
                Box::new(crate::screens::armoury::ArmouryScreen::new(county))
            }
            ScreenId::Rack(county, troop) => {
                Box::new(crate::screens::armoury::RackScreen::new(county, troop))
            }
            ScreenId::Divide(unit) => Box::new(crate::screens::divide::DivideScreen::new(unit)),
            ScreenId::Merchant(unit) => {
                Box::new(crate::screens::merchant::MerchantScreen::new(unit))
            }
            ScreenId::Trade(unit, good) => {
                Box::new(crate::screens::merchant::TradeScreen::new(unit, good))
            }
            ScreenId::BattlePrompt => {
                Box::new(crate::screens::battle::BattlePromptScreen::new())
            }
            ScreenId::BattleResult => {
                Box::new(crate::screens::battle::BattleResultScreen::new())
            }
            ScreenId::Battlefield => {
                Box::new(crate::screens::battlefield::BattlefieldScreen::new())
            }
            ScreenId::Options(page) => {
                Box::new(crate::screens::options::OptionsScreen::new(page))
            }
            ScreenId::MenuBar(title) => {
                Box::new(crate::screens::menubar::DropdownScreen::new(title))
            }
            ScreenId::About => Box::new(crate::screens::about::AboutScreen::new()),
            ScreenId::Court => Box::new(crate::screens::court::CourtScreen::new()),
            ScreenId::Nobles => Box::new(crate::screens::nobles::NoblesScreen::new()),
            ScreenId::Supplies(to) => {
                Box::new(crate::screens::supplies::SuppliesScreen::new(to))
            }
            ScreenId::Ratings => Box::new(crate::screens::ratings::RatingsScreen::new()),
            ScreenId::Info(target) => Box::new(crate::screens::info::InfoScreen::new(target)),
            ScreenId::Message => Box::new(crate::screens::message::MessageScreen::new()),
            ScreenId::Tip => Box::new(crate::screens::tip::TipScreen::new()),
            ScreenId::Movie(film) => Box::new(crate::screens::movie::MovieScreen::new(film)),
            ScreenId::Index => Box::new(crate::screens::index::IndexScreen::new()),
        }
    }
}


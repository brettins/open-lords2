//! The screen trait, and the state machine that owns the stack.
//!
//! # Transitions are values
//!
//! `docs/plan.md`: *"A screen is a trait, and screens do not know about each
//! other. Each screen handles input, updates, and draws into the canvas;
//! transitions are returned as values to the state machine
//! performed by the screen. A screen that can push another screen is a screen
//! that will eventually own the whole game."*
//!
//! So a screen returns a [`Transition`], and it names its destination with a
//! [`ScreenId`] — a plain value. It cannot construct another screen, cannot
//! hold one, and cannot reach the stack: the only type that can is [`Machine`],
//! so [`ScreenId::build`] is the single place any screen is made.
//!
//! # Draw cannot mutate
//!
//! [`Ctx`] carries `&mut Game`, and `draw` is handed `&Ctx`
//! `&mut Ctx`. The compiler enforces it.
//! that drawing a frame cannot change the world. A renderer that can nudge the
//! simulation is a renderer that makes the simulation depend on how often it
//! drew, and `docs/netcode.md` does not allow that.
//!
//! A screen may still mutate **itself** while drawing (`&mut self`), which is
//! how the campaign map caches its painted tiles and its pick plane.

use l2_view::Canvas;

use crate::game::{Assets, Game};
use crate::input::Event;

/// Which screen, as a value a screen may name without being able to build one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScreenId {
    Menu,
    Campaign,
    /// A county panel — the original's `0x14`, `0x15`, `0x16` and `0x19` — for
    /// one county id.
    ///
    /// **The panel is part of the identity**, because in the original the four
    /// are four different screen ids and the *only* route into any of them is
    /// the quadrant of the county strip drawn above it
    /// (`docs/screens-county.md` §2.3). A `County(id)` with no panel had to
    /// guess one, and guessed tax; the player then had no way to reach the
    /// other three, because the map screen never offered him the strip at all.
    County(u8, crate::screens::county::Panel),
    /// The village, for one county id — the original's screen `0x02`.
    Village(u8),
    /// The job popup, for one county and one of its nine labour slots — the
    /// original's screen `0x0F`. It floats over whatever opened it, which is
    /// either the village or the campaign sidebar.
    Job(u8, usize),
    /// `g_screenId` `0x1F` — the front end and game setup, by sub-page.
    Setup(crate::screens::setup::SetupPage),
    /// `g_screenId` `0x1C` — the campaign interstitial.
    Conquest,
    /// `g_screenId` `0x0B` — **the other lords**: one card per rival and the
    /// action menu. See [`crate::screens::diplomacy`].
    ///
    /// The rival being looked at is **not** part of the identity, unlike the
    /// county on a county panel: `g_diploTarget` is a global the screen owns
/// and changes under itself when a card is clicked,
    /// one of these open.
    Diplomacy,
    /// `g_screenId` `0x1A` — one of the seven compose dialogs, for one rival
    /// and one message kind.
    ///
    /// Both are part of the identity because both are what the painter
    /// dispatches on: `Screen_DiploDialog` switches on `g_diploKind` and every
    /// line of every shape names `g_diploTarget`.
    DiploCompose(u8, u8),
    /// `g_screenId` `0x35` and `0x36` — loading and saving a conquest. One
    /// painter with a mode flag, so one screen with a mode.
    SaveLoad(crate::screens::saveload::Mode),
    /// `g_screenId` `0x1B` — **the castle chooser**, for one county. Five
    /// picture buttons and an OK; see [`crate::screens::castle`].
    Castle(u8),
    /// `g_screenId` `0x1D` — the siege-preparation screen, for one besieging
    /// army. See [`crate::screens::siege`].
    Siege(usize),
    /// `g_screenId` `0x17` — **the raise-army screen**, for one county. The
/// shell table called it *"Hire mercenaries"*
    /// screen, and the offer is a block on this one. See
    /// [`crate::screens::army`].
    RaiseArmy(u8),
    /// `g_screenId` `0x0A` — **the armoury**, for the county whose levy is
    /// being equipped.
    ///
    /// It is not reached *from* the raise-army screen so much as it is the
    /// other half of it: `Screen_Draw` paints both with `Screen_Armoury`, the
/// two share the one `g_levyBasket`, and the button that raises
    /// the army is on this one. See [`crate::screens::armoury`].
    Armoury(u8),
    /// `g_screenId` `0x0D` — one weapon's rack, opened by clicking that weapon
    /// on the armoury's wall. The county and the troop type are both part of
    /// the identity because the original's `DAT_00553F20` is what picks the
    /// sprite sheet, the noun and the basket slot.
    Rack(u8, u8),
    /// `g_screenId` `0x11` — the army-division screen, for one army. See
    /// [`crate::screens::divide`].
    Divide(usize),
    /// `g_screenId` `0x08` — **the merchant's stall**, for the merchant unit
    /// being traded with.
    ///
    /// The unit is part of the identity because the *price* depends on it:
    /// `DAT_00553C64` is written on the map click and read only by this
    /// screen's plaque and by the panel's price arithmetic. See
    /// [`crate::screens::merchant`].
    Merchant(usize),
    /// `g_screenId` `0x0C` — the trade panel, for one merchant and one
    /// `L2.eng` group 6 good id.
    Trade(usize, u8),
    /// `g_screenId` `0x12` — ***"A Battle is to be fought. Will you take the
    /// field?"*** See [`crate::screens::battle`].
    ///
    /// **The battle it is about is not part of the identity.** It is on the
    /// suspended turn, where the answer has to go back to, and there can only
    /// ever be one because there is only one campaign.
    BattlePrompt,
    /// `g_screenId` `0x13` — *"The Battle is decided."*
    BattleResult,
    /// The options panels — `g_screenId` `0x39` (Advanced), `0x42` (Sounds),
    /// `0x43` (Display) and `0x31` (Help) — **and the quirks page, which is
    /// ours**.
    ///
    /// The page is part of the identity for the same reason
    /// [`ScreenId::County`]'s panel is: in the original these are four separate
    /// screen ids reached from four separate menu items, and a value that had to
    /// guess which one it meant would be a value that guessed. See
    /// [`crate::screens::options`].
    Options(crate::screens::options::Page),
    /// **`g_screenId` `0x29`, `0x2A` and `0x2B`** — the battlefield, the
    /// selection drag over it, and the outcome banner.
    ///
    /// One id for three, because the battle they are about is on the [`Game`]
    /// and all three draw the same field: which of the three is up is
    /// [`crate::battlefield::Mode`], and
    /// [`crate::battlefield::LiveBattle::screen_id`] answers it in the
    /// original's own numbers. `0x28` is the fourth of that block and is
    /// unreachable in the shipped binary — see [`crate::battlefield`].
    Battlefield,
    /// **`g_screenId` `0x32` — a menu-bar drop-down is open**, carrying the
    /// 1-based title index the original keeps in `DAT_00522CB4`.
    ///
    /// It is a screen id in the original too, and a strange one: its painter
    /// (`Menu_RestoreBackdrop`, `0x0040C928`) only puts the 400 × 180 band at
    /// (0, 24) back. See [`crate::screens::menubar`].
    MenuBar(usize),
    /// `g_screenId` `0x25` — **About**, the Help menu's box. See
    /// [`crate::screens::about`].
    About,
    /// `g_screenId` `0x09` — **the court**, the realm's balance sheet. Not a
    /// diplomacy screen; see [`crate::screens::court`].
    Court,
    /// `g_screenId` `0x20` — **the standings**, the court's one button.
    ///
    /// The category being looked at is **not** part of the identity, for
    /// `ScreenId::Diplomacy`'s reason: `DAT_0055CE7C` is a global the original
    /// keeps outside the screen, it survives the page being closed, and there
/// is one of these open. It is [`Game::nobles_category`]. See
    /// [`crate::screens::nobles`].
    Nobles,
    /// `g_screenId` `0x18` — **send supplies**, from one county to another.
    /// The destination is part of the identity because the screen opens with it
    /// equal to the source and the player moves it with the minimap. See
    /// [`crate::screens::supplies`].
    Supplies(u8),
    /// `g_screenId` `0x2E` — **the Battle Master ratings**, the skirmish
    /// scoreboard. See [`crate::screens::ratings`].
    Ratings,
    /// `g_screenId` `0x04` — **the map information panel**, for whatever the
    /// right click resolved to.
    ///
    /// The target is part of the identity because the original keeps it in
    /// `g_pickedTileUnit` and `DAT_0056795C` and picks the painter from them;
    /// a value that had to guess would be a value that guessed. See
    /// [`crate::screens::info`].
    Info(crate::screens::info::Target),
    /// **The message scroll.** Not a `g_screenId` at all: the original paints it
    /// over whatever is up and leaves the screen id alone, and its input arm
    /// (`Msg_HandleInput`, `0x0047685D`) runs *before* every per-screen arm.
    ///
    /// It is a screen here because our machine has exactly the two properties
    /// that arrangement needs — an overlay draws over what is beneath it, and
    /// the top screen gets first refusal — and because
    /// [`Transition::Pass`] can say the thing that matters, which is that a
    /// click the window does not want **falls through**. See
    /// [`crate::screens::message`].
    ///
    /// The record it is about is on [`Game`], in `messages`, because in the
    /// original it is in the data segment: the window is opened by the frame
    /// driver and not by anything the player did.
    Message,
    /// **`g_screenId` `0x27` — the screen a tip is shown on.**
    ///
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
    /// **Ours.** The demo's index of every screen; see [`crate::screens::index`].
    Index,
}

/// What a screen asks the machine to do next.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Transition {
    /// Nothing. The overwhelmingly common answer, and the default.
    Stay,
    /// Put a screen on top of this one; this one is still underneath.
    Push(ScreenId),
    /// Leave, revealing whatever was underneath. Popping the last screen quits.
    Pop,
    /// Leave and arrive in one step, without growing the stack.
    Replace(ScreenId),
    /// Leave the game.
    Quit,
    /// **Not mine — offer it to the screen underneath.**
    ///
    /// `Screen_FrameInput`'s per-screen arms are *ladders of guards*, and a
    /// guard that returns zero has not consumed the click: the arm falls
    /// through to the next one. The village's arm (`g_screenId == 0x02`) opens
    /// with six of them before any village verb is tried — see
    /// [`crate::screens::village`] — and every one of the six belongs to the
    /// campaign map's sidebar, not to the village. So the sidebar stays live
    /// with the village open, which is a thing our stack could not say until
    /// this variant existed: [`Machine::handle`] offered the event to the top
    /// screen and stopped.
    ///
/// The lower screen acts for real, and what it
    /// asks for lands *at its own depth* — see [`Machine::handle`], where a
    /// `Push` from underneath truncates everything above it first. That is the
    /// original's single-byte `g_screenId` reproduced, not a convenience.
    Pass,
    /// **I acted, and everything above me closes.**
    ///
    /// The other half of [`Transition::Pass`], and it exists for one arm:
    /// `Screen_FrameInput`'s **epilogue**, which runs on every screen id but
    /// `0x12` and is the whole of the campaign minimap's reach —
    ///
    /// ```text
    /// if ((leftPressed || rightPressed) && g_screenId != 0x12 && FUN_004323FE()) {
    ///     if (g_screenId == 0x0F) { Sound_StopOneShot(); FUN_0041438C(); }
    ///     if (g_battlePhase == 0) g_screenId = 0;
    /// }
    /// ```
    ///
    /// `FUN_004323FE` (`0x004323FE`) is `Minimap_Click` (`0x0043253A`) outside a
    /// battle.
    /// selects that county, centres the map on it **and drops the whole
    /// management surface**. Our stack says that as: the overlay passes the
    /// event down, the campaign map acts, and the campaign map asks for
    /// everything above it to be thrown away —
    /// stack underneath.
    ///
    /// It is deliberately not `Replace(self.id())`: that rebuilds the screen,
    /// and the campaign map's viewport is re-centre is *about*.
    Reveal,
    /// **Go to screen X, unwinding the stack** — `g_smkReturnScreen`.
    ///
    /// `Smk_Play` (`0x0042D91B`) stores its fifth argument and
    /// `Smk_OnFinished` (`0x0042E060`) performs it as one statement,
    /// `g_screenId = g_smkReturnScreen;`. That is a *destination*, and it is
    /// neither of the two things our stack could already say: not [`Pop`], which
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
    ///
    /// Applied as: truncate to the screen already on the stack, or — if it is
    /// not there — clear and build it, which is the byte's behaviour when the
/// destination was not open.
    ///
    /// [`Pop`]: Transition::Pop
    /// [`Replace`]: Transition::Replace
    Goto(ScreenId),
}

/// What a screen is given. `game` is mutable through `handle` and `update`, and
/// read-only through `draw`, because `draw` gets `&Ctx`.
pub struct Ctx<'a> {
    pub game: &'a mut Game,
    pub assets: &'a Assets,
}

pub trait Screen {
    fn id(&self) -> ScreenId;

    /// What the window is called while this screen is on top.
    fn title(&self, ctx: &Ctx) -> String;

    /// One input event. The default ignores everything,
/// down what it responds to.
    fn handle(&mut self, _event: Event, _ctx: &mut Ctx) -> Transition {
        Transition::Stay
    }

    /// One fixed simulation tick.
    ///
    /// **Not one frame.** Nothing here is told how much time passed, because
    /// nothing below this crate may learn anything from a clock. The renderer
    /// draws when it can; this steps at a fixed rate and never asks what the
    /// rate was.
    fn update(&mut self, _ctx: &mut Ctx) -> Transition {
        Transition::Stay
    }

/// **`Turn_Tick(); Units_Tick();` — the half of a frame
    /// screen's.**
    ///
    /// `Battle_Frame` (`0x004B99C0`) ends its inner loop with
    ///
    /// ```c
    /// if ((g_battlePhase == 0) && (ticksDue != 0)) { FUN_0040490d(); Turn_Tick(); Units_Tick(); }
    /// ```
    ///
    /// and — `[V]`, read whole. Only
    /// `Screen_Draw` and `Screen_FrameInput`, in the tail below it, dispatch on
    /// the screen. So the campaign winds on under a county panel, a village, an
    /// open menu and the message scroll alike, and the one thing that stops it
    /// is a battle.
    ///
    /// [`Screen::update`] is `Screen_FrameInput`'s half and this is the loop's,
/// so the campaign map has both: [`Machine::wind_turn`] calls this
    /// one every frame whatever is on top.
    fn wind_turn(&mut self, _ctx: &mut Ctx) -> Transition {
        Transition::Stay
    }

    /// Whether the last [`Screen::update`] changed what is on screen.
    ///
    /// Input already forces a repaint — [`Machine::handle`] marks the machine
    /// dirty for every event — so this exists for the one thing that changes
    /// without an event arriving: **edge scrolling**, where the pointer is held
    /// still against the edge of the window and the map moves under it. Taking
/// the flag keeps a still screen costing nothing,
    /// which is the property [`Machine::update`] was written to preserve.
    fn take_redraw(&mut self) -> bool {
        false
    }

    /// **How many widget clicks this screen owes the audio layer**, taken and
    /// forgotten — `Widget_Test`'s (`0x0040DA1E`) `Sound_RestartSlot(1)`.
    ///
    /// The original plays `click3.wav` *inside* the hit test, so the sound is
    /// not a decision any caller makes: pressing a kind-4 or kind-5 widget
    /// sounds, and nothing else in the interface does — not a hotspot, not the
    /// OK button, not the auto-repeat's later pulses. Ours hit-test through
    /// [`crate::press::Press`], which is the same function in the same place,
    /// and this is the one wire out of it.
    ///
    /// **It goes up, never down.** `docs/netcode.md` D-3 keeps
    /// [`crate::audio::Audio`] out of [`Ctx`] so that a screen cannot branch on
    /// a sound; a screen that can only *report* a press it has already acted on
    /// keeps that property exactly. Nothing here is on [`crate::Game`], so it
    /// is not in the save and not in the lockstep digest.
    ///
    /// A screen with no [`crate::press::Press`] answers zero, which is not an
    /// approximation: the original's other tester plays no sound.
    fn take_clicks(&mut self) -> u8 {
        0
    }

    /// **Whether this screen has just reached a `Save_RotateAndWrite`
    /// (`0x0049A453`)**, taken and forgotten.
    ///
    /// The original calls it from exactly two places, `[V]` — both of them
    /// screens here:
    ///
    /// * `FUN_0049A3E6`, which fires on `g_screenId == 0x24` and is the bottom
    ///   of the end-of-turn fade: reload the seasonal art, fade back up,
    ///   autosave. [`crate::screens::map::MapScreen`] raises it there.
    /// * `Game_NewGame` (`0x00497CED`), after `Season_Advance` and
    ///   `Move_BuildCostMap`. [`crate::screens::setup::SetupScreen`] raises it
/// there,
    ///
/// **It goes up**, as [`Screen::take_clicks`] does and
    /// for the same reason: a screen may report that a turn came round, and may
    /// not learn whether a file was written or where it went. Nothing here is on
    /// [`Game`], so it is not in the save and not in the lockstep digest.
    /// [`crate::saves::run_pending`] is the one place it becomes a file.
    fn take_autosave(&mut self) -> bool {
        false
    }

    /// **The original's `g_screenId`
    /// [`Screen::id`].**
    ///
    /// Almost every screen's byte follows from its [`ScreenId`], and
    /// [`crate::tip::screen_byte`] writes those down. One does not: the
    /// campaign map is `0x00`, and *in move-order mode* it is `0x10` —
    /// `Map_BeginMoveSelection` writes it and is the only writer of that value —
    /// while ours keeps the mode inside `MapScreen`. `Tip_Update`'s *"Army
    /// Movement:"* arm tests exactly that byte, so the mode has to be askable.
    fn mode_screen_id(&self) -> Option<u8> {
        None
    }

    /// **`g_minimapMode` (`0x0057A0C4`)**, from the one screen that keeps it.
    ///
    /// A global in the original and a field of the campaign map here, and the
    /// tool-tip ladder (`FUN_00477320`) reads it on every screen that sits on
    /// the sidebar — so the machine asks the stack. See [`crate::tooltip`].
    fn minimap_mode(&self) -> Option<u8> {
        None
    }

    /// The `.256` this screen runs under, if it is not the campaign palette.
    ///
    /// A [`Canvas`] is a plane of palette *indices* and means nothing without
    /// one. Most screens use the campaign palette and answer `None`; the front
    /// end, the merchant, the armoury, castle building and the ratings each
    /// read a palette of their own (`File_ReadChunk("gateway.256", …)` then
/// `Palette_Set`), and the presenter asks the top screen
    /// assuming there is only one.
    fn palette(&self) -> Option<&'static str> {
        None
    }

/// **A palette** — a film's, which changes as it plays.
    ///
    /// `Smk_PlayLoop` copies the film's 768 bytes into `g_paletteRgb` and
    /// uploads them whenever a frame carries a palette, so while a film is up
    /// *the whole screen* runs under it, the window it was raised over
    /// included. When this answers `Some`, the presenter uses it in place of
    /// [`Screen::palette`].
    fn live_palette(&self) -> Option<l2_formats::Palette> {
        None
    }

    /// How far into the end-of-turn screen fade this screen is, or `None` for
    /// the ordinary full-brightness palette.
    ///
    /// **The canvas is not involved.** `FUN_004B0CB4` is entirely a palette
    /// effect — no dither table, no half-brightness blit —
    /// fading draws exactly what it always draws and answers this instead. The
    /// presenter turns the number into colour, which is the same division
    /// [`Screen::palette`] already makes and for the same reason: a [`Canvas`]
    /// is a plane of indices and only one place in the application knows what
    /// they mean.
    ///
    /// The value is a phase in `0 ..= l2_view::fade::PHASES`.
    fn fade(&self) -> Option<u8> {
        None
    }

/// Whether this screen is an **inset over what was underneath**
    /// a page of its own.
    ///
    /// `docs/screens-county.md` §1: *"The game's management surface is a
    /// campaign map plus insets, not a set of full-screen pages."* An overlay
    /// does not clear the canvas, and [`Machine::draw`] paints the screens
    /// beneath it first, back to the last one that is not an overlay.
    ///
    /// It changes nothing about input: only the top screen is ever offered an
    /// event, which is what makes a popup modal.
    ///
    /// **There are two kinds of inset and both answer true**, which matters
    /// because only one of them looks like a window:
    ///
    /// * a framed `Ui_DrawBox` window — the four county panels, the job popup;
    /// * a raw blit with no frame and no clear — **the village**, which is
    ///   `vill.pl8` frame 0, 363 × 320, dropped at (64, `g_villageTopY`) over a
    ///   campaign map it repaints itself (`FUN_004050C0` → `FUN_004CFB08` →
    ///   `Map_DrawFrame`).
    ///
    /// The village was modelled as a page until a player opened one and said it
    /// was a dialogue with the map still showing round it. He was right;
    /// `docs/decisions.md` C22 records why the decompiled reasoning was not.
    fn is_overlay(&self) -> bool {
        false
    }

    fn draw(&mut self, ctx: &Ctx, canvas: &mut Canvas);
}

impl ScreenId {
    /// The one place a screen is constructed.
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
            // **`ScreenId::Diplomacy` was matched twice**, here and further up,
            // both arms constructing the same screen. It is the pilot's second
            // finding recurring — *a screen was in the index twice*
            // (`docs/draws.md` §2) — and the only thing that noticed was
            // rustc's `unreachable_patterns` warning, which had been printing
            // on every build. The earlier arm is the one that runs; this one is
            // removed. `screens/index.rs` listed the same screen twice as well,
            // once as *"THE OTHER LORDS"* and once as *"DIPLOMACY"*.
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

/// The screen stack.
pub struct Machine {
    stack: Vec<Box<dyn Screen>>,
    quit: bool,
    /// Set whenever anything happened that could change what is on screen.
    /// The event loop consults it to decide whether to repaint, which is the
    /// difference between a still menu costing nothing and costing a GPU
    /// submission sixty times a second.
    dirty: bool,
    /// **Every widget click the stack has made since the process started.**
    ///
    /// Monotone on purpose. [`crate::audio::Director`] keeps the previous
    /// tick's value and plays `click3.wav` when this has moved, which is the
    /// same diffing it already does for the screen stack and for where every
    /// unit stood — and it is the only shape that survives the thing that makes
    /// a per-screen counter useless: **the click that opens a screen is
    /// counted by the screen that is then popped.** A sum over the live stack
    /// would lose exactly the presses a player notices most.
    ///
/// Drained out of the screens by [`Machine::handle`]
    /// them,
    /// with it. See [`Screen::take_clicks`].
    clicks: u32,
    /// `(g_mouseX, g_mouseY)` — the last canvas pixel an event put the pointer
    /// on. The original reads `GetCursorPos` every frame; ours hears about it.
    pointer: (i32, i32),
    /// **`g_mouseInputChanged`** — the pointer moved or a button changed since
    /// the last tick. `FUN_004B191E` recomputes it once a frame, so it is taken
    /// once a tick, by [`Machine::run_tooltips`].
    pointer_changed: bool,
    /// **The tool tips** — `FUN_00476E95`'s state. Here and not on
    /// [`Game`], because it counts frames and `Game` is compared whole by the
    /// save round trips. See [`crate::tooltip`].
    tooltips: crate::tooltip::Tooltips,
    /// The screens the last tick painted, to see a repaint — `Screen_Draw`
    /// opens with `FUN_0047703A`.
    tooltip_screens: Vec<ScreenId>,
    /// `g_optToolTips` as the last tick saw it, to see `Opt_ToggleToolTips`.
    tool_tips_seen: Option<bool>,
    /// **A `Save_RotateAndWrite` (`0x0049A453`) is owed**, drained out of the
/// screens. See [`Screen::take_autosave`] and
    /// [`crate::saves::run_pending`].
    ///
    /// A latch and not a counter, because two turns cannot come round between
    /// two pumps: the request is raised in a tick and taken in the same tick's
    /// tail by the application.
    autosave: bool,
}

impl Machine {
    pub fn new(root: ScreenId) -> Machine {
        Machine {
            stack: vec![root.build()],
            quit: false,
            dirty: true,
            clicks: 0,
            pointer: (0, 0),
            pointer_changed: false,
            tooltips: crate::tooltip::Tooltips::new(),
            tooltip_screens: Vec::new(),
            tool_tips_seen: None,
            autosave: false,
        }
    }

    /// **The standing autosave request**, taken and cleared. The application
    /// pumps it through [`crate::saves::run_pending`], which is its only caller
    /// outside a test.
    pub fn take_autosave(&mut self) -> bool {
        core::mem::take(&mut self.autosave)
    }

    /// The tool-tip layer's state: which tip is up and where. See
    /// [`crate::tooltip`].
    pub fn tooltips(&self) -> &crate::tooltip::Tooltips {
        &self.tooltips
    }

    /// Every widget click the stack has made, ever. See the field.
    pub fn clicks(&self) -> u32 {
        self.clicks
    }

    pub fn depth(&self) -> usize {
        self.stack.len()
    }

    pub fn should_quit(&self) -> bool {
        self.quit
    }

    /// The screen ids on the stack, bottom first. For tests and for the window
    /// title; nothing in the game branches on it.
    pub fn ids(&self) -> Vec<ScreenId> {
        self.stack.iter().map(|s| s.id()).collect()
    }

    pub fn top_id(&self) -> Option<ScreenId> {
        self.stack.last().map(|s| s.id())
    }

    /// **The screen `g_screenId` names** — the top of the stack, looking
    /// through the message scroll, which in the original is painted over a
    /// screen and never changes the byte.
    pub fn top_screen_id(&self) -> Option<ScreenId> {
        self.stack.iter().rev().map(|s| s.id()).find(|id| *id != ScreenId::Message)
    }

    /// **`g_screenId` itself**, for the screens [`crate::tip`] asks about. See
    /// [`Screen::mode_screen_id`] and [`crate::tip::screen_byte`].
    pub fn top_screen_byte(&self, game: &Game) -> Option<u8> {
        let s = self.stack.iter().rev().find(|s| s.id() != ScreenId::Message)?;
        s.mode_screen_id().or_else(|| crate::tip::screen_byte(s.id(), game))
    }

    /// **The pointer this frame** — `Battle_Frame`'s two-way choice
    /// (`0x004B99C0`): the battlefield ids run the hover ladder, every other
    /// screen is one lookup in `g_cursorByScreen` (`0x004E3098`). See
    /// [`crate::cursor`].
    ///
    /// A screen whose byte [`crate::tip::screen_byte`] does not assert takes
    /// the arrow, which is what the table gives every screen but five.
    pub fn pointer(&self, game: &Game) -> crate::cursor::Pointer {
        let byte = self.top_screen_byte(game).unwrap_or(0);
        if (0x28..0x2B).contains(&byte) {
            return match game.battle.as_ref() {
                Some(b) => b.cursor().into(),
                None => crate::cursor::Pointer::Arrow,
            };
        }
        crate::cursor::by_screen(byte)
    }

    pub fn take_dirty(&mut self) -> bool {
        core::mem::replace(&mut self.dirty, false)
    }

    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    /// Put a screen on the stack from outside.
    ///
    /// This does not weaken the invariant at the top of this file. A *screen*
    /// still cannot reach the stack — it has no `&mut Machine` and never will.
    /// The application owns the machine, and so does a test that wants to open
    /// a screen the interface can only reach through three clicks.
    pub fn push(&mut self, id: ScreenId) {
        self.stack.push(id.build());
        self.dirty = true;
    }

    /// Deliver one event, top screen first, down through anything that passes.
    ///
    /// **The top screen still gets first refusal, and almost always keeps it.**
    /// A screen that does not return [`Transition::Pass`] ends the walk,
    /// popup is modal by default and two screens never act on one click.
    ///
    /// The exception is written down where it is used: `Screen_FrameInput`'s
    /// arm for a screen that is an *inset* can begin with guards belonging to
    /// the surface underneath, and the village's arm begins with six of the
    /// campaign map's. See [`Transition::Pass`].
    ///
    /// # A pass lands at the depth it came from
    ///
    /// The original has no stack: `g_screenId` is one byte, and 57 of the 100
    /// writes to it in `Screen_FrameInput` are the literal `0`.
    /// opened from the sidebar *while the village was up* still exits to the
    /// campaign map, because its arm's exit is a constant and not a memory of
    /// where it was opened from — the village goes with it. A player who tried
    /// it put it exactly: *"when you close that dialogue it will close town
    /// square and that dialogue"*.
    ///
    /// [`Machine::apply_at`] reproduces that by truncating the stack to the
    /// depth that acted before applying the transition. For the top screen —
    /// every other caller — truncating to the top is a no-op, so this is the
    /// same machine it has always been for everything that does not pass.
    /// `docs/bugs.md` B63 catalogues the collapse and what a switch would cost.
    pub fn handle(&mut self, event: Event, ctx: &mut Ctx) {
        // **Ours: Ctrl+D flips the debug overlay, on every screen, and no screen
        // sees the key.** See [`crate::game::Prefs::debug_overlay`].
        //
        // Chosen because the original answers it with nothing at all: the
        // window procedure (`0x004B29BE`) has `WM_KEYDOWN` arms for Backspace,
        // Enter, Control, Escape, End, Home, the arrows, Insert, Delete, the
        // digits, two keypad keys and F1 … F9, F11 and F12 — no letter — and the
        // `WM_CHAR` it sends for Ctrl+D is `0x04`, which `Edit_TypeChar`
        // (`0x00401A20`) rejects. F5 is `main.rs`'s window snap and is not
        // delivered here. Taken before the stack so that a text field cannot
        // swallow it and a screen that reads [`Key::CtrlChar`] — only the
        // battlefield does, and only for the nine digits — never meets it.
        // arm: ours/debug-overlay-toggle key
        if event == Event::KeyDown(crate::input::Key::CtrlChar('D')) {
            ctx.game.prefs.debug_overlay = !ctx.game.prefs.debug_overlay;
            self.dirty = true;
            return;
        }
        // `FUN_004B191E`'s `g_mouseInputChanged`: the position moved, or either
        // button went down or up. A key does not set it.
        match event {
            Event::Pointer { x, y } => {
                if (x, y) != self.pointer {
                    self.pointer = (x, y);
                    self.pointer_changed = true;
                }
            }
            Event::Click { x, y }
            | Event::Release { x, y }
            | Event::DoubleClick { x, y }
            | Event::RightClick { x, y } => {
                self.pointer = (x, y);
                self.pointer_changed = true;
            }
            Event::KeyDown(_) | Event::Text(_) | Event::PointerLeft => {}
        }
        for depth in (0..self.stack.len()).rev() {
            let t = self.stack[depth].handle(event, ctx);
            // **Before the transition, because the transition may drop the
            // screen that clicked.** `Widget_Test` plays the sound inside the
            // hit test and before it calls the handler; taking the count here
            // is that ordering, and it is why a press that opens a screen is
            // still heard. See [`Screen::take_clicks`].
            let clicked = self.stack[depth].take_clicks();
            self.clicks = self.clicks.wrapping_add(clicked as u32);
            // Before the transition for the same reason the clicks are: a new
            // game is started by a click, and the transition that starts it
            // drops the screen that asked. See [`Screen::take_autosave`].
            self.autosave |= self.stack[depth].take_autosave();
            if t == Transition::Pass {
                continue;
            }
            self.apply_at(depth, t);
            break;
        }
        self.dirty = true;
    }

    /// One fixed tick of the top screen — **and, before it, `Msg_Pump`.**
    ///
    /// `Msg_Pump` (`0x00472E46`) is not called by any screen: `Battle_Frame`
    /// (`0x004B99C0`) calls it once a frame, which makes this — our frame
    /// driver's per-tick step — the place it belongs. It is also why the screen
/// test inside it is a test of `g_screenId`
    /// message knows: see [`Machine::pump_messages`].
    pub fn update(&mut self, ctx: &mut Ctx) {
        self.run_tips(ctx);
        self.pump_messages(ctx);
        // `Battle_Frame`'s `FUN_00448d7e(g_selectedCounty)` at `0x004BA187`, in
        // its place: `Tip_Update` (106), `Msg_Pump` (107), … this (190), …
        // `Turn_Tick` (262). **After the pump on purpose** — a letter posted now
// is pulled off the ring on the next frame, as in the original,
        // which is what gives the player one frame of the county they just
        // clicked before the scroll covers it. See
        // [`crate::message::post_event`]. No screen test: the original has none.
        crate::message::post_event(ctx.game);
        self.run_turn_clock(ctx);
        if self.wind_turn(ctx) {
            // The stack moved underneath us, so the screen that was on top no
            // longer is. [`Machine::handle`] breaks for the same reason.
            self.run_tooltips(ctx);
            return;
        }
        if let Some(top) = self.stack.last_mut() {
            let t = top.update(ctx);
            if top.take_redraw() {
                self.dirty = true;
            }
            // **Nothing on this path clicks today**, and it is drained anyway.
            // `Widget_Test`'s auto-repeat and its delayed fire are both silent, so
            // `Screen::update` never counts one — but if a screen ever did, the
            // count would sit in its `Press` until the *next event* drained it in
            // [`Machine::handle`], and a click from a tick would be heard on the
            // release. The ablation that added a click to `Press::tick` stayed
            // green until this line existed.
            self.clicks = self.clicks.wrapping_add(top.take_clicks() as u32);
            // `FUN_0049A3E6`'s `Save_RotateAndWrite()`, which is the bottom of
            // the end-of-turn fade and reaches here through the map screen's
            // `tick_fade`. Before the transition, because a turn that ended the
            // game leaves for screen `0x1C` on the same frame.
            self.autosave |= top.take_autosave();
            if t != Transition::Stay {
                self.apply(t);
                self.dirty = true;
            }
        }
        self.run_tooltips(ctx);
    }

    /// **`Battle_Frame`'s `Turn_Tick(); Units_Tick();`** — the campaign winds
    /// on under whatever is on top of it.
    ///
    /// ```c
    /// if ((g_battlePhase == 0) && (ticksDue != 0)) { FUN_0040490d(); Turn_Tick(); Units_Tick(); }
    /// else if ((g_battlePhase == 2) && (ticksDue != 0)) { …the battle's passes… }
    /// ```
    ///
    /// `[V]`, `0x004B99C0`, and **the absence is the finding**:
    /// `g_screenId` test on either arm. Everything in the loop's tail that does
    /// test the screen — `Screen_Draw`, `Screen_FrameInput`, the cursor ladder —
    /// is *drawing and input*. So the only thing on the stack that suspends a
    /// turn is a battle, which is `g_battlePhase != 0` and here is
    /// [`crate::game::Game::battle`] plus the battlefield screen.
    ///
    /// **The defect this replaces**: [`Screen::update`] is run for the top
    /// screen only, and the campaign map is what wound the turn,
    ///
    /// [`crate::screens::message`] — stopped the turn while it was open. So did
    /// walking into a county panel. `docs/decisions.md` C197.
    ///
/// **And the `else if` is the battle's**, so this is one function.
    /// The second arm has no `g_screenId` test either,
/// whatever is on top of *it* as a turn runs under whatever is on
    /// top of the map. That mattered the moment the battlefield got a menu bar:
    /// the drop-down is screen `0x32`, a push here, and
    /// [`Screen::update`] is the top screen's alone — so opening *File* over a
    /// battle froze the battle until the menu closed. See
    /// [`crate::screens::battlefield`].
    ///
    /// Returns whether the stack moved.
    // arm: 0x004B99C0/frame-winds-the-turn frame
    fn wind_turn(&mut self, ctx: &mut Ctx) -> bool {
        // `g_battlePhase != 0`. The battlefield is on the stack for the whole
        // of a fought battle and `Game::battle` for the whole of a suspended
        // one, and neither implies the other.
        if ctx.game.battle.is_some() || self.stack.iter().any(|s| s.id() == ScreenId::Battlefield) {
            // **`else if ((g_battlePhase == 2) && ticksDue) { …the battle's
            // passes… }`** — the loop's other arm, and it is the battlefield's
            // own tick run at whatever depth it sits. Only when something is
            // over it: on top it is [`Machine::update`]'s job and running both
            // would step the simulation twice a frame.
            // arm: 0x004B99C0/frame-winds-the-battle frame
            if let Some(depth) = self.stack.iter().position(|s| s.id() == ScreenId::Battlefield) {
                if depth + 1 < self.stack.len() {
                    self.wind_battle(ctx, depth);
                }
            }
            return false;
        }
        let Some(depth) = self.stack.iter().position(|s| s.id() == ScreenId::Campaign) else {
            return false;
        };
        let t = self.stack[depth].wind_turn(ctx);
        if self.stack[depth].take_redraw() {
            self.dirty = true;
        }
        self.autosave |= self.stack[depth].take_autosave();
        // **A screen the turn is already waiting on is not put up twice.**
        // `resume_turn` asks for `0x12`/`0x13` on every frame the question
        // stands, which before this ran only on the frame the map was on top.
        if let Transition::Push(id) = t {
            if self.stack.iter().any(|s| s.id() == id) {
                return false;
            }
        }
        if t == Transition::Stay {
            return false;
        }
        self.apply_at(depth, t);
        self.dirty = true;
        true
    }

    /// **`Battle_Frame`'s `g_battlePhase == 2` arm** — the battle's passes, run
    /// at `depth` while something else is on top of the battlefield.
    ///
    /// The original's inner loop is one `if / else if` on `g_battlePhase` with
    /// **no `g_screenId` test on either side**,
/// drop-down as a campaign turn steps under an open letter. Ours
    /// stepped only from [`Screen::update`], which the machine gives to the top
    /// screen alone, so the menu bar this screen has just been given would have
    /// frozen the fight every time a player opened it. Same absence, same fix,
    /// one screen along: `docs/decisions.md` C197 is the turn's half of it.
    ///
    /// A transition from down here is applied at `depth`, which is what
    /// [`Machine::apply_at`] is for: a battle that *ends* while a menu is open
/// settles underneath the menu.
    fn wind_battle(&mut self, ctx: &mut Ctx, depth: usize) {
        let t = self.stack[depth].update(ctx);
        if self.stack[depth].take_redraw() {
            self.dirty = true;
        }
        self.clicks = self.clicks.wrapping_add(self.stack[depth].take_clicks() as u32);
        self.autosave |= self.stack[depth].take_autosave();
        // **A screen it is already waiting on is not put up twice** — the same
        // guard `wind_turn` needs, for the same reason: the outcome film is
        // asked for on every frame the banner stands.
        if let Transition::Push(id) = t {
            if self.stack.iter().any(|s| s.id() == id) {
                return;
            }
        }
        if t != Transition::Stay {
            self.apply_at(depth, t);
            self.dirty = true;
        }
    }

    /// **`FUN_00476E95` (`0x00476E95`)** — the tool tips, near the end of
    /// `Battle_Frame` and after everything above. [`crate::tooltip`] has the
    /// decompilation; this is the three things only the machine can see.
    ///
    /// * **A repaint.** `Screen_Draw` opens with `FUN_0047703A`, which drops the
    /// tip and keeps its stamp. A painter runs when the screen changes,
    ///   change in the screens on the stack — looking through the message
    /// scroll — is that call. `[I]`, and the module
    ///   header says what it does not cover.
    /// * **`Opt_ToggleToolTips` (`0x004347C7`)** is three statements, and the
    ///   second is `_DAT_004EA830 = 0`. `g_optToolTips` has no other writer in
    /// play,
    /// * **`Map_InitMode` (`0x00498270`)** writes the same zero, from
    ///   `FUN_00497A34`, the campaign's bring-up: the campaign map arriving on
    ///   the stack.
    ///
    /// And the lookup: `DAT_004D6FB8[g_screenId]` for the top screen `g_screenId`
    /// names, resolved through the campaign map's minimap mode and the selected
    /// county's produce rows.
    fn run_tooltips(&mut self, ctx: &mut Ctx) {
        let changed = core::mem::take(&mut self.pointer_changed);
        let screens: Vec<ScreenId> =
            self.stack.iter().map(|s| s.id()).filter(|id| *id != ScreenId::Message).collect();
        if screens != self.tooltip_screens {
            let arrived = screens.contains(&ScreenId::Campaign)
                && !self.tooltip_screens.contains(&ScreenId::Campaign);
            if arrived {
                self.tooltips.rearm();
            }
            if self.tooltips.drop_tip() {
                self.dirty = true;
            }
            self.tooltip_screens = screens;
        }
        let enabled = ctx.game.prefs.tool_tips;
        if self.tool_tips_seen.is_some_and(|was| was != enabled) {
            self.tooltips.rearm();
        }
        self.tool_tips_seen = Some(enabled);

        let top = self.stack.iter().rev().find(|s| s.id() != ScreenId::Message);
        let byte = top.and_then(|s| {
            crate::tooltip::screen_byte(s.id(), ctx.game, s.mode_screen_id())
        });
        let minimap = self.stack.iter().find_map(|s| s.minimap_mode()).unwrap_or(0);
        let game: &Game = ctx.game;
        let resolve = |x: i32, y: i32| match crate::tooltip::ladder_of(byte) {
            crate::tooltip::ladder::CAMPAIGN => {
                crate::tooltip::campaign_tip(&crate::tooltip::Sidebar::of(game, minimap), x, y)
            }
            crate::tooltip::ladder::BATTLE => crate::tooltip::battle_tip(x, y),
            _ => 0,
        };
        if self.tooltips.frame(enabled, changed, self.pointer, resolve) {
            self.dirty = true;
        }
    }

    /// **`Msg_Pump`'s screen ladder** — which screens the message scroll runs
    /// on, and what happens on the rest.
    ///
    /// ```c
    /// if (g_screenId == 0x00 || g_screenId == 0x27 ||
    ///     (g_screenId == 0x0F && g_jobPanelJob == 8) || g_screenId == 0x29) { … pump … }
    /// else if (g_messageGroup != 0) Msg_Dismiss();
    /// ```
    ///
    /// Two arms, and the second is the one nobody had written down: **opening
    /// any other screen while a message is up closes it.** Walk into the village
    /// with a letter on screen and the letter is gone.
    ///
    /// `0x27` has no screen here and `g_jobPanelJob` is the job slot **plus
    /// one**, which is what `CountyStrip_JobClick` writes — so the panel that
    /// pumps is slot 7.
    ///
    /// # `Msg_Pump` is one function and its two halves are exclusive
    ///
    /// ```c
    /// if (g_messageTimer < 1) { …pull one record, timer = 2000… }
    /// else                    { …count down, and maybe dismiss… }
    /// if (g_messageGroup != 0) Msg_DrawWindow();
    /// ```
    ///
    /// So the frame that opens a window **does draw it** — that trailing call is
    /// not in either arm — and does **not** count its timer down. Both halves
    /// are here, and the draw's own side effects are the message screen's
    /// `update`, which the caller runs immediately after this. Splitting the
    /// countdown out into the screen instead cost the message one tick of life:
    /// invisible in single player, where the timer is clamped and never expires,
    /// and a measurable 399 against 400 in a network game.
    // arm: 0x00472E46/pump-screen-ladder frame
    fn pump_messages(&mut self, ctx: &mut Ctx) {
        if self.top_id() == Some(ScreenId::Message) {
            // The countdown half. When it expires the window closes and the
            // screen's own `update` pops itself on its first line.
            if ctx.game.messages.advance(ctx.game.multiplayer) == crate::message::Tick::TimedOut {
                // `Msg_Pump`'s two timeouts call `Msg_Dismiss`, and so reach
                // `FUN_00476E21`: a network game's tip that expires restores
// its screen as a clicked one does.
                ctx.game.tips.restore();
                self.dirty = true;
            }
            return;
        }
        let pumps = match self.top_id() {
            Some(ScreenId::Campaign) => true,
            Some(ScreenId::Battlefield) => true,
            // `g_screenId == 0x27`, the tip's own screen — the reason a tip
            // shown in the village gets a window at all.
            Some(ScreenId::Tip) => true,
            Some(ScreenId::Job(_, job)) => job + 1 == crate::message::PUMP_JOB,
            _ => false,
        };
        if pumps {
            // The pull half.
            if ctx.game.messages.pull() {
                self.push(ScreenId::Message);
            }
        } else if ctx.game.messages.is_open() {
            crate::message::dismiss(ctx.game);
            self.dirty = true;
        }
    }

    /// **`Turn_Tick`'s turn timer, and the frames after it runs out.**
    ///
    /// `Turn_Tick` (`0x0049A010`) is called from `Battle_Frame`'s loop whenever
    /// `g_battlePhase == 0`, not from any screen, so the count goes on whether
    /// the person is looking at the map, a county panel or the village. That
    /// makes the frame driver its place, for the same reason it is `Msg_Pump`'s.
    /// The clock itself is [`crate::turn_clock`].
    ///
    /// When it runs out it calls `Turn_End` (`0x0043AC23`), and three things
    /// follow:
    ///
    /// 1. `Turn_End`'s first statement, `if (g_messageGroup != 0) Msg_Dismiss();`;
    /// 2. from the next frame, `Screen_FrameInput` closes every screen whose arm
    ///    carries the turn-ended guard — [`crate::turn_clock::closed_by_turn_end`],
    ///    popped here top first for as long as the request stands;
    /// 3. the turn begins. **Only the map can start one here**, so the request
    ///    waits on [`crate::game::Game::turn_clock`] until the map is on top, and
    ///    `MapScreen::update` carries it out through the End Turn button's own
    ///    door.
    ///
    /// **One difference left, and the other one is gone.** The original runs
    /// the turn behind a screen the guard does not close — the job popup, an
    /// open menu, the About box — and so do we now: that used to wait for the
    /// person to close it, because the map wound the turn and only the top
    /// screen was ticked. [`Machine::wind_turn`] is where that stopped being
    /// true. What remains is `0x13`, the battle report, which is closed by the
    /// guard there and not here: popping it would leave the report unseen on
    /// the suspended turn and the map would put it straight back.
    // arm: 0x0049A010/turn-time-limit timer
    fn run_turn_clock(&mut self, ctx: &mut Ctx) {
        // `2 < g_appPhase` — a game is up, which here is a campaign map on the
        // stack. The front end and the demo index have no turn to time.
        if !self.stack.iter().any(|s| s.id() == ScreenId::Campaign) {
            return;
        }
        let before = crate::turn_clock::shown(ctx.game);
        let frame = crate::turn_clock::Frame::of(ctx.game);
        if ctx.game.turn_clock.tick(frame) == crate::turn_clock::Tick::Expired
            && ctx.game.messages.is_open()
        {
            // The message screen pops itself on its own update once its record
            // is gone, so it is not popped here.
            crate::message::dismiss(ctx.game);
            self.dirty = true;
        }
        // **`Screen_FrameInput`'s force-close guard**, and it is the standing
// latch:
        //
        //   if (DAT_00553FC8 != 0 || (DAT_0055403C != 0 && DAT_00553018 == 0))
        //
        // `Turn_End` (`0x0043AC23`) writes `DAT_0055403C = 2` — whichever door
        // the turn was ended through, the clock's or the person's own End Turn
        // — and `Turn_Tick`'s restart clears it on the first frame of his next
        // live turn. So the guard stands for the whole turn in between, and
        // twenty-six of the twenty-seven sites close their screen. This tested
        // `end_turn_pending` instead, which is the clock's request and is taken
        // by the map the moment the map is on top: a panel opened *during* the
        // turn that followed stayed open, and ending the turn by hand with one
        // up closed nothing at all.
        // arm: 0x0042FF10/force-close-on-turn-end timer
        if ctx.game.turn_clock.force_close() {
            let in_battle = ctx.game.battle.is_some();
            while let Some(top) = self.stack.last() {
                let id = top.id();
                if id == ScreenId::BattleResult
                    || !crate::turn_clock::closed_by_turn_end(id, in_battle)
                {
                    break;
                }
                self.stack.pop();
                self.dirty = true;
            }
        }
        // The number is whole seconds, so this is a repaint a second and not a
        // repaint a tick.
        if crate::turn_clock::shown(ctx.game) != before {
            self.dirty = true;
        }
    }

    /// **`Tip_Update` (`0x00476AA7`), and `g_screenId = 0x27` made a stack.**
    ///
    /// `Battle_Frame` calls `Tip_Update` immediately before `Msg_Pump`, which is
    /// the order here. The ladder is [`crate::tip::update`]; what this adds is
    /// the one thing the ladder cannot do, which is put screen `0x27` on the
    /// stack and take it off again:
    ///
    /// * **on** when `Tip_Show` posts — *under* the message scroll if one is up,
    ///   because a message open on the campaign map is painted over `g_screenId
    ///   0` and stays painted over `0x27` when the byte changes beneath it;
    /// * **off** when `FUN_00476E21` has restored the byte, which any
    ///   `Msg_Dismiss` does — including the dismissal of a message that was
    ///   already queued ahead of the tip, so the tip's own record can outlive
    ///   its screen and open later on the campaign map. That is the original.
    ///
    /// Seating is done at the *start* of the tick as well as after the ladder,
    /// because a dismissal happens in [`Machine::handle`] between two ticks.
    // arm: 0x00476AA7/tip-screen-ladder frame
    fn run_tips(&mut self, ctx: &mut Ctx) {
        self.seat_tip_host(ctx.game);
        let view = crate::tip::View::of(self, ctx.game);
        if crate::tip::tick(ctx.game, &view).is_some() {
            self.seat_tip_host(ctx.game);
        }
    }

    /// Keep [`ScreenId::Tip`] on the stack exactly while
    /// [`crate::tip::Tips::hosting`] says `g_screenId` is `0x27`.
    // arm: 0x00476E21/tip-restores-its-screen frame
    fn seat_tip_host(&mut self, game: &Game) {
        let seated = self.stack.iter().any(|s| s.id() == ScreenId::Tip);
        if game.tips.hosting() && !seated {
            let at = match self.top_id() {
                Some(ScreenId::Message) => self.stack.len() - 1,
                _ => self.stack.len(),
            };
            self.stack.insert(at, ScreenId::Tip.build());
            self.dirty = true;
        } else if !game.tips.hosting() && seated {
            self.stack.retain(|s| s.id() != ScreenId::Tip);
            self.dirty = true;
        }
    }

    /// **`FUN_0041A639` (`0x0041A639`) — the turn timer**, drawn over whatever is
    /// up, after it.
    ///
    /// Not a painter's draw: `Battle_Frame` calls it once a frame near the end of
/// its tail, after the widgets and the message window,
    /// drawn here after the stack and not by the campaign map. Whether it is
    /// drawn at all is `DAT_004D2E80[g_screenId]`, the table in
    /// [`crate::turn_clock::SCREENS`].
    fn draw_turn_timer(&self, ctx: &Ctx, canvas: &mut Canvas) {
        if !self.stack.iter().any(|s| s.id() == ScreenId::Campaign) {
            return;
        }
        let Some(id) = crate::turn_clock::timer_screen(self.stack.iter().map(|s| s.id())) else {
            return;
        };
        if crate::turn_clock::drawn_over(id) {
            crate::turn_clock::draw(ctx, canvas);
        }
    }

    /// Paint the stack from the last screen that is not an overlay upwards.
    ///
    /// An overlay is drawn over what was underneath, which is what the
/// original's management surface is; a page clears and replaces.
    /// The common case — a stack whose top is a page — draws exactly one
    /// screen, as it always did.
    ///
/// **The original has no screen clear anywhere**:
    /// `Screen_Draw` picks a painter and the painter fills a rectangle,
    /// whatever is outside it is still there from the last frame.
    /// `Village_Draw` repaints the campaign map itself and blits its picture on
    /// top of it; `Panel_JobDetail` draws a window over the village.
    pub fn draw(&mut self, ctx: &Ctx, canvas: &mut Canvas) {
        let from = self.base();
        for screen in &mut self.stack[from..] {
            screen.draw(ctx, canvas);
        }
        self.draw_turn_timer(ctx, canvas);
        // `FUN_00476E95` comes after `FUN_0041A639` in `Battle_Frame`.
        crate::tooltip::draw(ctx, &self.tooltips, canvas);
    }

    /// The lowest screen that has to be painted for the top one to make sense.
    fn base(&self) -> usize {
        for i in (0..self.stack.len()).rev() {
            if !self.stack[i].is_overlay() {
                return i;
            }
        }
        0
    }

    /// The `.256` the stack runs under, or `None` for the campaign palette. The
    /// presenter is the only caller: it is the one place that turns indices
    /// into colour.
    ///
    /// **The nearest screen that names one, looking down through overlays.**
    /// The original has one display palette and only a *painter* writes it —
    /// `Screen_Armoury` ends with `Palette_Set(armoury.256)`,
    /// `Screen_DrawBattlefield` (`0x004233F7`) sets `T32_bat1.256` — and nothing
    /// that draws over a page touches it.
    /// `Tip_Show` (`0x00476DA9`) saves `g_screenId`, writes `0x27` and posts a
    /// message; `FUN_00476E21` puts the byte back; `Msg_DrawWindow`
    /// (`0x0047309E`) has no `Palette_Set` anywhere in its 10,915 bytes.
    /// window over the armoury is in the armoury's colours. `[V]`
    ///
    /// This used to ask the top screen alone, and an overlay that names no
    /// palette — the tip host, the message scroll, the menu bar, the options
    /// pages — handed the page beneath it the campaign palette. A player saw the
    /// raise-army and castle screens *"color reversed"* behind their first tip
    /// until he dismissed it. `docs/decisions.md` C178.
    ///
    /// A page that names none *is* the campaign palette and ends the search,
    /// which is the same boundary [`Machine::draw`] stops at for pixels.
    pub fn palette_name(&self) -> Option<&'static str> {
        for screen in self.stack.iter().rev() {
            if let Some(name) = screen.palette() {
                return Some(name);
            }
            if !screen.is_overlay() {
                return None;
            }
        }
        None
    }

    /// **The frame as colour** — `canvas` through a playing film's palette or
    /// else [`Machine::palette_name`]'s, and through the end-of-turn fade when
    /// the top screen is fading.
    ///
    /// `main.rs`'s presenter is this and a window. It lives here so that the
    /// colours a player is shown can be asserted without one: a canvas is a
    /// plane of indices, and every defect of the *"right picture, wrong
    /// colours"* kind is invisible to a test that stops at the canvas.
    pub fn present(&self, assets: &crate::game::Assets, canvas: &Canvas, rgba: &mut [u8]) {
// A film's palette changes as it plays; while one is
        // up it is the whole screen's (`Smk_ApplyPalette`), so it outranks
        // every `.256` on the stack. See [`Machine::live_palette`].
        let live = self.live_palette();
        let palette = live.as_ref().unwrap_or_else(|| {
            self.palette_name()
                .and_then(|n| assets.shell.palette(n))
                .unwrap_or(&assets.palette)
        });
        // **The end-of-turn fade, and it is the whole of the effect.**
        // `FUN_004B0CB4` never touches the framebuffer — it rewrites the display
        // palette and lets the unchanged plane of indices resolve darker. See
        // `l2_view::fade` and [`Screen::fade`].
        let faded = self.fade().map(|phase| l2_view::fade::at(palette, phase));
        canvas.to_rgba(faded.as_ref().unwrap_or(palette), rgba);
    }

    /// The top screen's [`Screen::live_palette`], which outranks
    /// [`Machine::palette_name`] when it answers.
    pub fn live_palette(&self) -> Option<l2_formats::Palette> {
        self.stack.last().and_then(|s| s.live_palette())
    }

    /// The end-of-turn fade phase of the top screen, or `None`. The presenter
    /// is the only caller; see [`Screen::fade`].
    pub fn fade(&self) -> Option<u8> {
        self.stack.last().and_then(|s| s.fade())
    }

    pub fn title(&self, ctx: &Ctx) -> String {
        self.stack.last().map(|s| s.title(ctx)).unwrap_or_default()
    }

    fn apply(&mut self, t: Transition) {
        let depth = self.stack.len().saturating_sub(1);
        self.apply_at(depth, t);
    }

    /// Apply a transition **asked for by the screen at `depth`**.
    ///
    /// Everything above `depth` is discarded first. For the top screen that is
/// nothing, so every existing caller is unaffected; for a screen
    /// that was reached by a [`Transition::Pass`] it is the whole point, and it
/// is the original's behaviour — see
    /// [`Machine::handle`].
    fn apply_at(&mut self, depth: usize, t: Transition) {
        match t {
            Transition::Stay | Transition::Pass => {}
            // `g_screenId = 0` from an arm that was reached by falling through:
            // this screen stays and everything opened over it goes.
            Transition::Reveal => self.stack.truncate(depth + 1),
            Transition::Push(id) => {
                self.stack.truncate(depth + 1);
                self.stack.push(id.build());
            }
            Transition::Pop => {
                self.stack.truncate(depth);
                if self.stack.is_empty() {
                    self.quit = true;
                }
            }
            Transition::Replace(id) => {
                self.stack.truncate(depth);
                self.stack.push(id.build());
            }
            Transition::Quit => {
                self.stack.clear();
                self.quit = true;
            }
            // `g_screenId = g_smkReturnScreen`. `depth` is deliberately not
            // consulted: the original writes the byte whatever was up, and the
            // destination may be *below* the screen that asked — which is the
            // whole case this exists for, a castle film ending on the map with
            // the chooser that raised it in between.
            Transition::Goto(id) => match self.stack.iter().position(|s| s.id() == id) {
                Some(at) => self.stack.truncate(at + 1),
                None => {
                    self.stack.clear();
                    self.stack.push(id.build());
                }
            },
        }
    }
}

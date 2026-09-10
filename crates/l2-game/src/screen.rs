//! The screen trait, and the state machine that owns the stack.
//!
//! # Transitions are values
//!
//! `docs/plan.md`: *"A screen is a trait, and screens do not know about each
//! other. Each screen handles input, updates, and draws into the canvas;
//! transitions are returned as values to the state machine rather than
//! performed by the screen. A screen that can push another screen is a screen
//! that will eventually own the whole game."*
//!
//! So a screen returns a [`Transition`], and it names its destination with a
//! [`ScreenId`] — a plain value. It cannot construct another screen, cannot
//! hold one, and cannot reach the stack: the only type that can is [`Machine`],
//! which is why [`ScreenId::build`] is the single place any screen is made.
//!
//! # Draw cannot mutate
//!
//! [`Ctx`] carries `&mut Game`, and `draw` is handed `&Ctx` rather than
//! `&mut Ctx`. That is not a stylistic preference: it is the compiler enforcing
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
    /// and changes under itself when a card is clicked, and there is only ever
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
    /// shell table called it *"Hire mercenaries"*; there is no mercenaries
    /// screen, and the offer is a block on this one. See
    /// [`crate::screens::army`].
    RaiseArmy(u8),
    /// `g_screenId` `0x0A` — **the armoury**, for the county whose levy is
    /// being equipped.
    ///
    /// It is not reached *from* the raise-army screen so much as it is the
    /// other half of it: `Screen_Draw` paints both with `Screen_Armoury`, the
    /// two share the one `g_levyBasket`, and the button that actually raises
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
    /// **A pass is not a peek.** The lower screen acts for real, and what it
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
    /// battle. So a press on the minimap raster from *any* management screen
    /// selects that county, centres the map on it **and drops the whole
    /// management surface**. Our stack says that as: the overlay passes the
    /// event down, the campaign map acts, and the campaign map asks for
    /// everything above it to be thrown away — which is `g_screenId = 0` with a
    /// stack underneath.
    ///
    /// It is deliberately not `Replace(self.id())`: that rebuilds the screen,
    /// and the campaign map's viewport is exactly what a re-centre is *about*.
    Reveal,
}

/// What a screen is given. `game` is mutable through `handle` and `update`, and
/// read-only through `draw`, because `draw` only ever gets `&Ctx`.
pub struct Ctx<'a> {
    pub game: &'a mut Game,
    pub assets: &'a Assets,
}

pub trait Screen {
    fn id(&self) -> ScreenId;

    /// What the window is called while this screen is on top.
    fn title(&self, ctx: &Ctx) -> String;

    /// One input event. The default ignores everything, so a screen only writes
    /// down what it actually responds to.
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

    /// Whether the last [`Screen::update`] changed what is on screen.
    ///
    /// Input already forces a repaint — [`Machine::handle`] marks the machine
    /// dirty for every event — so this exists for the one thing that changes
    /// without an event arriving: **edge scrolling**, where the pointer is held
    /// still against the edge of the window and the map moves under it. Taking
    /// the flag rather than reading it keeps a still screen costing nothing,
    /// which is the property [`Machine::update`] was written to preserve.
    fn take_redraw(&mut self) -> bool {
        false
    }

    /// The `.256` this screen runs under, if it is not the campaign palette.
    ///
    /// A [`Canvas`] is a plane of palette *indices* and means nothing without
    /// one. Most screens use the campaign palette and answer `None`; the front
    /// end, the merchant, the armoury, castle building and the ratings each
    /// read a palette of their own (`File_ReadChunk("gateway.256", …)` then
    /// `Palette_Set`), and the presenter asks the top screen rather than
    /// assuming there is only one.
    fn palette(&self) -> Option<&'static str> {
        None
    }

    /// How far into the end-of-turn screen fade this screen is, or `None` for
    /// the ordinary full-brightness palette.
    ///
    /// **The canvas is not involved.** `FUN_004B0CB4` is entirely a palette
    /// effect — no dither table, no half-brightness blit — so a screen that is
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

    /// Whether this screen is an **inset over what was underneath** rather than
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
}

impl Machine {
    pub fn new(root: ScreenId) -> Machine {
        Machine { stack: vec![root.build()], quit: false, dirty: true }
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
    /// A screen that does not return [`Transition::Pass`] ends the walk, so a
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
    /// writes to it in `Screen_FrameInput` are the literal `0`. So a screen
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
        for depth in (0..self.stack.len()).rev() {
            let t = self.stack[depth].handle(event, ctx);
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
    /// test inside it is a test of `g_screenId` rather than of anything the
    /// message knows: see [`Machine::pump_messages`].
    pub fn update(&mut self, ctx: &mut Ctx) {
        self.pump_messages(ctx);
        let Some(top) = self.stack.last_mut() else { return };
        let t = top.update(ctx);
        if top.take_redraw() {
            self.dirty = true;
        }
        if t != Transition::Stay {
            self.apply(t);
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
    // arm: 0x00472E46/pump-screen-ladder
    fn pump_messages(&mut self, ctx: &mut Ctx) {
        if self.top_id() == Some(ScreenId::Message) {
            // The countdown half. When it expires the window closes and the
            // screen's own `update` pops itself on its first line.
            if ctx.game.messages.advance(ctx.game.multiplayer) == crate::message::Tick::TimedOut {
                self.dirty = true;
            }
            return;
        }
        let pumps = match self.top_id() {
            Some(ScreenId::Campaign) => true,
            Some(ScreenId::Battlefield) => true,
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

    /// Paint the stack from the last screen that is not an overlay upwards.
    ///
    /// An overlay is drawn over what was underneath, which is what the
    /// original's management surface actually is; a page clears and replaces.
    /// The common case — a stack whose top is a page — draws exactly one
    /// screen, as it always did.
    ///
    /// This is not a convenience. **The original has no screen clear anywhere**:
    /// `Screen_Draw` picks a painter and the painter fills a rectangle, so
    /// whatever is outside it is still there from the last frame.
    /// `Village_Draw` repaints the campaign map itself and blits its picture on
    /// top of it; `Panel_JobDetail` draws a window over the village.
    pub fn draw(&mut self, ctx: &Ctx, canvas: &mut Canvas) {
        let from = self.base();
        for screen in &mut self.stack[from..] {
            screen.draw(ctx, canvas);
        }
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

    /// The `.256` the top screen runs under, or `None` for the campaign
    /// palette. The presenter is the only caller: it is the one place that
    /// turns indices into colour.
    pub fn palette_name(&self) -> Option<&'static str> {
        self.stack.last().and_then(|s| s.palette())
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
    /// nothing, which is why every existing caller is unaffected; for a screen
    /// that was reached by a [`Transition::Pass`] it is the whole point, and it
    /// is the original's behaviour rather than a simplification of it — see
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
        }
    }
}

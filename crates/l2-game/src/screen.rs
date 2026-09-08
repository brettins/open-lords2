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
    /// The county panel, for one county id.
    County(u8),
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
    /// A screen that is drawn and not yet driven, named by its `g_screenId`.
    Shell(u8),
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
            ScreenId::County(id) => Box::new(crate::screens::county::CountyScreen::new(id)),
            ScreenId::Village(id) => Box::new(crate::screens::village::VillageScreen::new(id)),
            ScreenId::Job(id, job) => Box::new(crate::screens::job::JobScreen::new(id, job)),
            ScreenId::Setup(page) => Box::new(crate::screens::setup::SetupScreen::new(page)),
            ScreenId::Conquest => Box::new(crate::screens::conquest::ConquestScreen::new()),
            ScreenId::Shell(id) => Box::new(crate::screens::shells::ShellScreen::new(id)),
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

    /// Deliver one event to the top screen only.
    ///
    /// Only the top screen is offered input. A stack where every layer gets a
    /// look is a stack where two screens act on the same click.
    pub fn handle(&mut self, event: Event, ctx: &mut Ctx) {
        let Some(top) = self.stack.last_mut() else { return };
        let t = top.handle(event, ctx);
        self.apply(t);
        self.dirty = true;
    }

    /// One fixed tick of the top screen.
    pub fn update(&mut self, ctx: &mut Ctx) {
        let Some(top) = self.stack.last_mut() else { return };
        let t = top.update(ctx);
        if t != Transition::Stay {
            self.apply(t);
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

    pub fn title(&self, ctx: &Ctx) -> String {
        self.stack.last().map(|s| s.title(ctx)).unwrap_or_default()
    }

    fn apply(&mut self, t: Transition) {
        match t {
            Transition::Stay => {}
            Transition::Push(id) => self.stack.push(id.build()),
            Transition::Pop => {
                self.stack.pop();
                if self.stack.is_empty() {
                    self.quit = true;
                }
            }
            Transition::Replace(id) => {
                self.stack.pop();
                self.stack.push(id.build());
            }
            Transition::Quit => {
                self.stack.clear();
                self.quit = true;
            }
        }
    }
}

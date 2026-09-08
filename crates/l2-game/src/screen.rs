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

    fn draw(&mut self, ctx: &Ctx, canvas: &mut Canvas);
}

impl ScreenId {
    /// The one place a screen is constructed.
    pub fn build(self) -> Box<dyn Screen> {
        match self {
            ScreenId::Menu => Box::new(crate::screens::menu::MenuScreen::new()),
            ScreenId::Campaign => Box::new(crate::screens::map::MapScreen::new()),
            ScreenId::County(id) => Box::new(crate::screens::county::CountyScreen::new(id)),
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

    pub fn draw(&mut self, ctx: &Ctx, canvas: &mut Canvas) {
        if let Some(top) = self.stack.last_mut() {
            top.draw(ctx, canvas);
        }
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

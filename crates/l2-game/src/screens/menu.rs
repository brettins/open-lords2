//! The main menu: start, and quit.
//!
//! Two items, because the slice has two. Everything else the original's menu
//! offers — load, multiplayer, the map editor, the options — is refused
//! (`docs/plan.md`: *"Scope is what kills a UI layer"*).

use l2_view::{text, Canvas};

use crate::input::{Event, Key, Rect};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::widget;

const ITEM_W: i32 = 220;
const ITEM_H: i32 = 20;
const ITEM_X: i32 = (l2_view::canvas::WIDTH as i32 - ITEM_W) / 2;
const ITEM_Y: i32 = 250;
const ITEM_GAP: i32 = 30;

const ITEMS: [&str; 2] = ["START CAMPAIGN", "QUIT"];
const START: usize = 0;
const QUIT: usize = 1;

pub struct MenuScreen {
    /// Which item the keyboard is on. The pointer moves it too, so hover and
    /// keyboard selection are the same state and cannot disagree.
    selected: usize,
}

impl MenuScreen {
    pub fn new() -> MenuScreen {
        MenuScreen { selected: START }
    }

    pub fn item_rect(index: usize) -> Rect {
        Rect::new(ITEM_X, ITEM_Y + index as i32 * ITEM_GAP, ITEM_W, ITEM_H)
    }

    fn at(x: i32, y: i32) -> Option<usize> {
        (0..ITEMS.len()).find(|&i| MenuScreen::item_rect(i).contains(x, y))
    }

    fn activate(&self) -> Transition {
        match self.selected {
            START => Transition::Push(ScreenId::Campaign),
            QUIT => Transition::Quit,
            _ => Transition::Stay,
        }
    }
}

impl Default for MenuScreen {
    fn default() -> Self {
        MenuScreen::new()
    }
}

impl Screen for MenuScreen {
    fn id(&self) -> ScreenId {
        ScreenId::Menu
    }

    fn title(&self, _ctx: &Ctx) -> String {
        "Lords of the Realm II".into()
    }

    fn handle(&mut self, event: Event, _ctx: &mut Ctx) -> Transition {
        match event {
            Event::KeyDown(Key::Up) => {
                self.selected = (self.selected + ITEMS.len() - 1) % ITEMS.len();
            }
            Event::KeyDown(Key::Down) => {
                self.selected = (self.selected + 1) % ITEMS.len();
            }
            Event::KeyDown(Key::Enter) | Event::KeyDown(Key::Space) => return self.activate(),
            Event::KeyDown(Key::Escape) => return Transition::Quit,
            Event::Pointer { x, y } => {
                if let Some(i) = MenuScreen::at(x, y) {
                    self.selected = i;
                }
            }
            Event::Click { x, y } => {
                if let Some(i) = MenuScreen::at(x, y) {
                    self.selected = i;
                    return self.activate();
                }
            }
            _ => {}
        }
        Transition::Stay
    }

    fn draw(&mut self, ctx: &Ctx, canvas: &mut Canvas) {
        let ink = &ctx.assets.ink;
        canvas.clear(ink.background);
        let mid = canvas.width as i32 / 2;

        text::draw_centred(canvas, mid, 150, "LORDS OF THE REALM II", ink.highlight);
        text::draw_centred(canvas, mid, 172, "AN OPEN REIMPLEMENTATION", ink.dim);

        for (i, label) in ITEMS.iter().enumerate() {
            widget::button(canvas, ink, MenuScreen::item_rect(i), label, i == self.selected);
        }

        text::draw_centred(canvas, mid, 420, "ARROWS OR MOUSE TO CHOOSE, ENTER TO CONFIRM", ink.dim);
        let counties = ctx.game.kingdom.county_count;
        if counties > 0 {
            let line = format!(
                "SCENARIO LOADED: {counties} COUNTIES, YEAR {}",
                ctx.game.kingdom.year
            );
            text::draw_centred(canvas, mid, 440, &line, ink.dim);
        }
    }
}

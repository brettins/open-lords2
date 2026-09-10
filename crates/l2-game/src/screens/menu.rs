//! **Ours, entirely — and the shipped binary cannot reach it.**
//!
//! # It is not screen `0x32`, and it is not the front end either
//!
//! This file was a bootstrap: a two-item menu written before there was a front
//! end to reproduce. There is one now. **The original's front end is
//! `g_screenId` `0x1F` page 1** (`FUN_0041E7E1`), and
//! [`crate::screens::setup`] draws it out of `L2.eng` group 11 and
//! `Gateway.pl8`. `crates/l2-game/src/main.rs` boots straight to
//! `ScreenId::Setup(SetupPage::Title)`; `ScreenId::Menu` is constructed in
//! exactly one place, `screen.rs`'s factory, and **nothing outside
//! `crates/l2-game/tests/` ever pushes it.** Verified by grep, both ways.
//!
//! So every draw call in this file is an invention on an unreachable screen,
//! which is the pair `docs/agents.md` records for screen `0x28`: *counting arms
//! cannot tell you whether a screen is reachable*, and an audit that counts an
//! unreachable screen has put rows into a denominator that should not have
//! them.
//!
//! **The recommendation is that it goes**, and [`crate::screens::setup`]
//! replaces it outright — `screen.rs`, `screens/mod.rs` and
//! `crates/l2-game/tests/machine.rs` all name it, and none of those is this
//! agent's file. Until somebody does that, it is kept the way
//! [`crate::screens::index`] is kept: **ours on purpose, and saying so on
//! itself.**
//!
//! # The title was a misspelling of the game's own
//!
//! It drew `"LORDS OF THE REALM II"`. `L2.eng` group 11 index 0 — which
//! `crates/l2-game/tests/shell.rs` has asserted for weeks — is
//! **`"Lords of the Realm 2"`**, with a digit. [`TITLE_GROUP`] draws the
//! game's own string where the install has one, so the one line on this screen
//! that has a right answer now has it.

use l2_view::{text, Canvas};

use crate::input::{Event, Key, Rect};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::shell::{font, Pen};
use crate::widget;

const ITEM_W: i32 = 220;
const ITEM_H: i32 = 20;
const ITEM_X: i32 = (l2_view::canvas::WIDTH as i32 - ITEM_W) / 2;
const ITEM_Y: i32 = 250;
const ITEM_GAP: i32 = 30;

/// `L2.eng` group 11 — the front end's own words. Index 0 is the game's title,
/// *"Lords of the Realm 2"*, and it is the one string on this screen that the
/// original has an opinion about. Verified against the words;
/// `crates/l2-game/tests/shell.rs` asserts it.
pub const TITLE_GROUP: usize = 11;
pub const TITLE_INDEX: usize = 0;
/// What to draw with no install. **Ours**, and deliberately not a guess at the
/// game's spelling.
const TITLE_FALLBACK: &str = "OURS: NO L2.ENG. SEE SCREENS/SETUP.RS FOR THE REAL FRONT END";

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

        // **The game's own title, in the game's own font**, where the install
        // has one. This drew `"LORDS OF THE REALM II"` in our 5 x 7 font, which
        // is a misspelling of `L2.eng` 11/0, *"Lords of the Realm 2"*.
        let pen = Pen {
            assets: &ctx.assets.shell,
            ink,
            chrome: ctx.assets.chrome.as_ref(),
            shadow: Some(font::SHADOW),
            caps: None,
        };
        let title = ctx.assets.shell.text(TITLE_GROUP, TITLE_INDEX).to_string();
        if title.is_empty() {
            text::draw_centred(canvas, mid, 150, TITLE_FALLBACK, ink.dim);
        } else {
            pen.heading_centred(canvas, 0, 140, canvas.width as i32, &title, font::TEXT);
        }

        // Everything below this line is **ours**, in our own 5 x 7 font, and
        // says so: this whole screen is an invention the shipped binary cannot
        // reach. See the module header.
        text::draw_centred(canvas, mid, 172, "AN OPEN REIMPLEMENTATION", ink.dim);

        for (i, label) in ITEMS.iter().enumerate() {
            widget::button(canvas, ink, MenuScreen::item_rect(i), label, i == self.selected);
        }

        text::draw_centred(canvas, mid, 400, "OURS: UNREACHABLE. THE REAL FRONT END IS 0X1F PAGE 1", ink.dim);
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

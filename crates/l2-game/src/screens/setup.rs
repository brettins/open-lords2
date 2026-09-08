//! Screen `0x1F` — game setup, and the thirteen sub-pages `g_setupPage`
//! selects.
//!
//! # This is the front end
//!
//! `docs/screens-county.md` §1 called `0x1C` *"the front end"*. It is not, and
//! [`super::conquest`] says what it actually is. The screen a player of the
//! original meets first is **this one, page 1**: `Ui_DrawCentred(11, 0, …)` is
//! *"Lords of the Realm 2"* over `gateway.pl8`, with *"The siege is on"* under
//! it and four items in `panels2.pl8` recesses.
//!
//! # The thirteen pages
//!
//! **[D]** `FUN_0041E7E1` (`0x0041E7E1`) is one `if`/`else if` chain on
//! `g_setupPage` (`0x005530F0`) with thirteen arms, and `FUN_0041E61D`
//! (`0x0041E61D`) is the background loader in front of it. Together they are
//! the whole page table:
//!
//! | page | painter | background | what it is |
//! |---:|---|---|---|
//! | 1 | `0x0041EA14` | `gateway` + `panels2` | the title menu — group 11.0/11.1, items 11.2, 11.3, 11.47, 11.4 |
//! | 2 | `0x0041EC8A` | `gateway` + `panels2` | *"Your options"* — items 11.6, 11.7, 11.19, 11.8, 11.9 |
//! | 3 | `0x0041EF42` → `FUN_004148E4(5)` | `gateway` + `panels2` | load a game — group 40.5 |
//! | 4 | `0x0041EF57` | `gateway` + `panels2` | *"Choose your title and your shield."* — 11.10 |
//! | 5 | `0x0041F592` | `gateway` + `panels2` | original campaign or the new one — group 39.0, 39.4, 39.5 |
//! | 6 | `0x0041F3E9` | `gateway` + `panels2` | full game or skirmish — group 39.0, 39.1, 39.2 |
//! | 7 | `0x0041F6C7` | `custom` | the custom game, single player |
//! | 8 | `0x0041F77A` | `custom` | the custom game, multiplayer |
//! | 9 | `0x0041FDD6` | *the page underneath* | one open drop-down over page 7, 8, 11 or 12 |
//! | 10 | `0x00420428` | `gateway` + `panels2` | *"No Lords of the Realm CD"* — 11.16, 11.17, 11.18, 11.48, 11.49 |
//! | 11 | `0x00420630` | `skirmish` / `skircust` | skirmish setup, multiplayer |
//! | 12 | `0x0042051C` | `skirmish` / `skircust` | skirmish setup, single player |
//! | 13 | `0x0042150B` | `skirmish` / `skircust` | the skirmish file box, over page 12 |
//!
//! **[V]** for every row that names an `L2.eng` group; the string reads as what
//! the page is.
//!
//! # The custom game's twelve options close exactly
//!
//! **[V]**, and this is the arithmetic. Three tables in `.data` describe the
//! twelve drop-downs: `0x004D3098` gives each one `(x, boxY, labelY)`,
//! `0x004D3128` gives its base index into `L2.eng` group 103, and `0x004D3158`
//! gives `(x, y, rows)` for the open list, where `rows` is the item count plus
//! two — a border cell at each end.
//!
//! Take the counts out of that third table and lay them against the bases:
//! every option's run ends exactly where the next one's begins, and the twelve
//! runs cover group 103's 46 strings with **one** left over — index 4, *"one"*,
//! which nothing reaches because *Nobles* starts at *"two"*. Twelve labels in
//! group 102, twelve bases, twelve counts, 45 of 46 strings used, and no
//! remainder. None of that was chosen by us.
//!
//! # What this page does not do
//!
//! Nothing here starts a game with the settings it shows. The options carry
//! their own selection so the drop-downs behave, and the transitions are the
//! page graph read out of `FUN_00432B05` and `FUN_00432CC8`; everything past
//! *Start* is [`Transition::Push`] into the campaign as the demo already had
//! it. That is the honest boundary and it is marked at the call site.

use l2_view::Canvas;

use crate::input::{Event, Key, Rect};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::shell::{self, font, Pen};

/// `g_setupPage` (`0x005530F0`). The thirteen values `FUN_0041E7E1` switches on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SetupPage {
    /// 1 — the title menu. The first screen of the original.
    Title,
    /// 2 — *"Your options"*.
    Options,
    /// 3 — load a game.
    Load,
    /// 4 — *"Choose your title and your shield."*
    Shield,
    /// 5 — the original campaign, or the one the expansion adds.
    Campaign,
    /// 6 — full game, or skirmish.
    GameType,
    /// 7 — the custom game, single player.
    Custom,
    /// 8 — the custom game, multiplayer.
    CustomMulti,
    /// 9 — one option's drop-down, open over the page underneath.
    Dropdown,
    /// 10 — *"No Lords of the Realm CD"*.
    NoCd,
    /// 11 — skirmish setup, multiplayer.
    SkirmishMulti,
    /// 12 — skirmish setup, single player.
    Skirmish,
    /// 13 — the skirmish file box, over page 12.
    SkirmishFile,
}

impl SetupPage {
    /// The value `g_setupPage` holds for this page.
    pub fn number(self) -> u8 {
        match self {
            SetupPage::Title => 1,
            SetupPage::Options => 2,
            SetupPage::Load => 3,
            SetupPage::Shield => 4,
            SetupPage::Campaign => 5,
            SetupPage::GameType => 6,
            SetupPage::Custom => 7,
            SetupPage::CustomMulti => 8,
            SetupPage::Dropdown => 9,
            SetupPage::NoCd => 10,
            SetupPage::SkirmishMulti => 11,
            SetupPage::Skirmish => 12,
            SetupPage::SkirmishFile => 13,
        }
    }

    /// All thirteen, in `g_setupPage` order.
    pub const ALL: [SetupPage; 13] = [
        SetupPage::Title,
        SetupPage::Options,
        SetupPage::Load,
        SetupPage::Shield,
        SetupPage::Campaign,
        SetupPage::GameType,
        SetupPage::Custom,
        SetupPage::CustomMulti,
        SetupPage::Dropdown,
        SetupPage::NoCd,
        SetupPage::SkirmishMulti,
        SetupPage::Skirmish,
        SetupPage::SkirmishFile,
    ];

    /// Which full-screen background `FUN_0041E61D` reads for this page.
    ///
    /// **[D]** The chain is: page 10 and pages below 7 take `gateway.256` and
    /// `gateway.pl8` with `panels2.pl8` on top; pages 7, 8 and 9 take
    /// `custom.256` and `custom.pl8`; pages 11 and up take `skirmish.256` and
    /// either `skirmish.pl8` or `skircust.pl8`. Page 9 draws the page beneath
    /// it first and so inherits whichever that was.
    pub fn background(self) -> &'static str {
        match self {
            SetupPage::Custom | SetupPage::CustomMulti | SetupPage::Dropdown => "Custom.pl8",
            SetupPage::SkirmishMulti | SetupPage::Skirmish | SetupPage::SkirmishFile => {
                "Skirmish.pl8"
            }
            _ => "Gateway.pl8",
        }
    }

    /// The `.256` that background comes with.
    pub fn palette(self) -> &'static str {
        match self {
            SetupPage::Custom | SetupPage::CustomMulti | SetupPage::Dropdown => "Custom.256",
            SetupPage::SkirmishMulti | SetupPage::Skirmish | SetupPage::SkirmishFile => {
                "Skirmish.256"
            }
            _ => "Gateway.256",
        }
    }
}

/// `L2.eng` group 11 — the front end's own strings, from index 0
/// *"Lords of the Realm 2"* to index 49.
pub const GROUP: usize = 11;
/// Group 39, the expansion pack's two choices.
pub const GROUP_EXPANSION: usize = 39;
/// Group 40, the load/save captions.
pub const GROUP_FILE: usize = 40;
/// Group 101, the sixty map names `g_scenarioIndex` indexes.
pub const GROUP_MAPS: usize = 101;
/// Group 102, the twelve custom-game option labels.
pub const GROUP_OPTIONS: usize = 102;
/// Group 103, their 46 values.
pub const GROUP_VALUES: usize = 103;

/// `panels2.pl8` — the box kit these pages draw their windows from. It has the
/// same frame layout as `Panels.pl8` (`docs/screens-county.md` §4.1).
const BOX_SHEET: &str = "Panels2.pl8";
/// `misc_sel.pl8` — `g_miscCtySheet` while the setup screen is up.
const ICON_SHEET: &str = "Misc_sel.pl8";

// ------------------------------------------------------------ menu geometry

/// A menu item on pages 1, 2 and 4: `FUN_00403EE4(x, y, 192, 24)` with the
/// label centred in the same 192 pixels, four below the top.
const ITEM_W: i32 = 0xC0;
const ITEM_H: i32 = 0x18;
/// Pages 1 and 2 put every item at the same x.
const ITEM_X: i32 = 0xE0;
/// And step them 36 apart from y = 91: `0x5B, 0x7F, 0xA3, 0xC7, 0xEB`.
const ITEM_Y: i32 = 0x5B;
const ITEM_STEP: i32 = 0x24;
/// The label sits five pixels into the recess.
const ITEM_TEXT: i32 = 5;

fn item_rect(index: usize) -> Rect {
    Rect::new(ITEM_X, ITEM_Y + index as i32 * ITEM_STEP, ITEM_W, ITEM_H)
}

/// Page 1's four items, as `L2.eng` group 11 indices — *"Single player"*,
/// *"Multiple players"*, *"Lords of Magic?"*, *"Exit game"*.
pub const TITLE_ITEMS: [usize; 4] = [2, 3, 47, 4];
/// Page 2's five — *"Play Now!"*, *"Load a game"*, *"Skirmish!"*,
/// *"Custom game"*, *"Back"*.
pub const OPTION_ITEMS: [usize; 5] = [6, 7, 19, 8, 9];

/// Page 4's two buttons, from `FUN_0041F01F`: *"Back"* at `(0x70, 0xD7)` and
/// *"Continue"* at `(0x150, 0xD7)`, both 192 × 24.
const SHIELD_BUTTONS: [(i32, i32, usize); 2] = [(0x70, 0xD7, 9), (0x150, 0xD7, 11)];

/// The five shields: `x = 0x70 + 0x58 i`, `y = 0x8C`, from `FUN_0041F1DD`.
const SHIELD_X: i32 = 0x70;
const SHIELD_STEP: i32 = 0x58;
const SHIELD_Y: i32 = 0x8C;

/// Pages 5 and 6 share a geometry: `FUN_00403EE4(0x6E, 0x74, 0xA4, 0x18)` and
/// the same 164 × 24 again at `x = 0x16E`, with the label centred in 160
/// pixels from two inside each recess.
const PAIR_X: [i32; 2] = [0x6E, 0x16E];
const PAIR_TEXT_X: [i32; 2] = [0x70, 0x170];
const PAIR_Y: i32 = 0x74;
const PAIR_W: i32 = 0xA4;
const PAIR_TEXT_W: i32 = 0xA0;

// ------------------------------------------------- the custom game's tables

/// `0x004D3098` — twelve records of `(x, boxY, labelY)`, four columns of three.
///
/// The value box is `FUN_004093E0(x, boxY, 6, 3)`: a `Panels.pl8` box in border
/// set 1, 96 × 48 pixels, with the value centred in 96 from `x + 1` at
/// `boxY + 16`. The label is the wrapped group 102 string at `(x, labelY)`.
pub const OPTION_CELLS: [(i32, i32, i32); 12] = [
    (170, 280, 246),
    (170, 353, 337),
    (170, 426, 410),
    (290, 280, 246),
    (290, 353, 337),
    (290, 426, 410),
    (410, 280, 246),
    (410, 353, 337),
    (410, 426, 410),
    (530, 280, 246),
    (530, 353, 337),
    (530, 426, 410),
];

/// `0x004D3128` — each option's base index into `L2.eng` group 103.
pub const OPTION_BASE: [usize; 12] = [0, 2, 5, 9, 11, 15, 19, 25, 29, 34, 37, 44];

/// `0x004D3158` — the open drop-down: `(x, y, rows)`, `rows` being the item
/// count plus two. [`OPTION_COUNT`] is that minus two.
pub const OPTION_LIST: [(i32, i32, i32); 12] = [
    (170, 280, 4),
    (170, 353, 4),
    (170, 426, 6),
    (290, 280, 4),
    (290, 337, 6),
    (290, 378, 6),
    (410, 280, 8),
    (410, 337, 6),
    (410, 362, 7),
    (530, 280, 5),
    (530, 305, 9),
    (530, 426, 4),
];

/// How many values each option has: `OPTION_LIST[i].2 - 2`.
pub const OPTION_COUNT: [usize; 12] = [2, 2, 4, 2, 4, 4, 6, 4, 5, 3, 7, 2];

/// The value box is six cells wide and three tall.
const OPTION_BOX_W: i32 = 6 * 16;
const OPTION_BOX_H: i32 = 3 * 16;
/// And the label above it wraps at a hundred pixels.
const OPTION_LABEL_W: i32 = 100;

/// Page 7's three buttons — *"Cancel"*, *"Start"*, *"Defaults"* — centred in
/// 76 pixels at `y = 0xC6`. Page 8 adds *"Load"* at `x = 399`.
const CUSTOM_BUTTONS: [(i32, usize); 4] = [(0xA5, 12), (0xF3, 13), (0x141, 14), (399, 15)];
const CUSTOM_BUTTON_Y: i32 = 0xC6;
const CUSTOM_BUTTON_W: i32 = 0x4C;

/// The map list on the custom-game pages: `misc_sel.pl8` frame 0x10 at
/// `(0x1F0, 9)`, five rows of 16 pixels of group 101 from `(0x1F2, 0x8E)`, the
/// selected row filled 105 × 16 from `(0x1F0, 0x8D)`.
const MAP_LIST_X: i32 = 0x1F0;
const MAP_LIST_TEXT_X: i32 = 0x1F2;
const MAP_LIST_Y: i32 = 0x8D;
const MAP_LIST_ROW: i32 = 0x10;
const MAP_LIST_ROWS: usize = 5;
const MAP_LIST_W: i32 = 0x69;
/// Group 101 has sixty entries, one per map slot.
pub const MAP_COUNT: usize = 60;

// --------------------------------------------------------------- the screen

pub struct SetupScreen {
    page: SetupPage,
    /// Which item the pointer or the keyboard is on — `DAT_00553114`, the
    /// global the painters compare against to pick `0xF9` over `0x3F`.
    selected: usize,
    /// Which page the drop-down is open over: `DAT_00553E5C`.
    under: SetupPage,
    /// Which of the twelve options is open: `DAT_0055306C`.
    open: usize,
    /// The twelve settings. Ours — the original keeps them in `g_opt*` globals
    /// that nothing in this workspace reads yet — so that the drop-downs
    /// actually change something and the interface can be walked.
    options: [usize; 12],
    /// Which of the five shields page 4 has picked. Realm `+0x0A` in the
    /// original, one-based there and zero-based here because this is an index
    /// into the five frame pairs and nothing else yet.
    shield: usize,
    /// The top row of the map list, and the selected map. `g_scenarioIndex`
    /// *is* the map slot (`Game::map_slot`), but the setup screen has not
    /// chosen one yet, so this is local until *Start* is wired.
    map_top: usize,
    map: usize,
}

impl SetupScreen {
    pub fn new(page: SetupPage) -> SetupScreen {
        SetupScreen {
            page,
            selected: 0,
            under: SetupPage::Custom,
            open: 0,
            options: [0; 12],
            shield: 0,
            map_top: 0,
            map: 0,
        }
    }

    pub fn page(&self) -> SetupPage {
        self.page
    }

    /// The value of option `i`, as an index into `L2.eng` group 103.
    pub fn option_value(&self, i: usize) -> usize {
        OPTION_BASE[i] + self.options[i].min(OPTION_COUNT[i] - 1)
    }

    /// The clickable rectangles of the page, in the order the painter draws
    /// them. Hit-testing and highlighting read the same list, so they cannot
    /// disagree — the failure `menu.rs` avoids by the same means.
    fn hotspots(&self) -> Vec<(Rect, Action)> {
        let mut v = Vec::new();
        match self.page {
            SetupPage::Title => {
                for (i, _) in TITLE_ITEMS.iter().enumerate() {
                    v.push((item_rect(i), Action::Item(i)));
                }
            }
            SetupPage::Options => {
                for (i, _) in OPTION_ITEMS.iter().enumerate() {
                    v.push((item_rect(i), Action::Item(i)));
                }
            }
            SetupPage::Shield => {
                for i in 0..5 {
                    v.push((Rect::new(SHIELD_X + i * SHIELD_STEP, SHIELD_Y, 60, 65), {
                        Action::Item(i as usize)
                    }));
                }
                for (i, (x, y, _)) in SHIELD_BUTTONS.iter().enumerate() {
                    v.push((Rect::new(*x, *y, ITEM_W, ITEM_H), Action::Item(5 + i)));
                }
            }
            SetupPage::Campaign | SetupPage::GameType => {
                for (i, x) in PAIR_X.iter().enumerate() {
                    v.push((Rect::new(*x, PAIR_Y, PAIR_W, ITEM_H), Action::Item(i)));
                }
            }
            SetupPage::Custom | SetupPage::CustomMulti => {
                for (i, &(x, y, _)) in OPTION_CELLS.iter().enumerate() {
                    v.push((Rect::new(x, y, OPTION_BOX_W, OPTION_BOX_H), Action::Open(i)));
                }
                for row in 0..MAP_LIST_ROWS {
                    let y = MAP_LIST_Y + row as i32 * MAP_LIST_ROW;
                    v.push((
                        Rect::new(MAP_LIST_X, y, MAP_LIST_W, MAP_LIST_ROW),
                        Action::Map(row),
                    ));
                }
                let n = if self.page == SetupPage::Custom { 3 } else { 4 };
                for (i, (x, _)) in CUSTOM_BUTTONS.iter().take(n).enumerate() {
                    // [I] The painter centres the caption in 76 pixels at
                    // y = 0xC6 and registers no rectangle of its own; the
                    // hit box is that caption's box, four pixels above it.
                    v.push((
                        Rect::new(*x, CUSTOM_BUTTON_Y - 4, CUSTOM_BUTTON_W, ITEM_H),
                        Action::Item(i),
                    ));
                }
            }
            SetupPage::Dropdown => {
                let (x, y, rows) = OPTION_LIST[self.open];
                let y = self.dropdown_y(y, rows);
                for i in 0..OPTION_COUNT[self.open] {
                    v.push((
                        Rect::new(x, y + 16 + i as i32 * 16, OPTION_BOX_W, 16),
                        Action::Choose(i),
                    ));
                }
            }
            SetupPage::NoCd | SetupPage::Load | SetupPage::SkirmishFile => {
                // One way out, and the whole page is it.
                v.push((Rect::new(0, 0, 640, 480), Action::Item(0)));
            }
            SetupPage::Skirmish | SetupPage::SkirmishMulti => {
                for (i, x) in [0x1CD, 0x207, 0x241].iter().enumerate() {
                    v.push((Rect::new(*x, 0x1B8 - 4, 0x38, ITEM_H), Action::Item(i)));
                }
            }
        }
        v
    }

    /// `FUN_0041FDD6` shifts the *Nobles* drop-down up by one row per item so
    /// that a list opened from the bottom row of the grid still fits on the
    /// screen. It is the only option that gets the treatment.
    fn dropdown_y(&self, y: i32, rows: i32) -> i32 {
        if self.open == 2 {
            y - (rows - 2 - 1) * 16
        } else {
            y
        }
    }

    fn at(&self, x: i32, y: i32) -> Option<(usize, Action)> {
        self.hotspots()
            .into_iter()
            .enumerate()
            .find(|(_, (r, _))| r.contains(x, y))
            .map(|(i, (_, a))| (i, a))
    }

    fn count(&self) -> usize {
        self.hotspots().len()
    }

    fn activate(&mut self) -> Transition {
        let Some(&(_, action)) = self.hotspots().get(self.selected) else {
            return Transition::Stay;
        };
        self.act(action)
    }

    /// The page graph, read out of `FUN_00432B05` (page 1) and `FUN_00432CC8`
    /// (page 2).
    ///
    /// **[D], and one thing in it is unresolved.** Those handlers branch on
    /// `g_uiHotspotId`, and the ids do not run in the order the painter draws
    /// the items: on page 1 id 3 sets the quit flag and id 4 plays `lom.smk`,
    /// while the painter draws *"Lords of Magic?"* third and *"Exit game"*
    /// fourth. Either the widget table is not in drawing order or one of the
    /// two readings is wrong, and nothing in the decompilation settles it. The
    /// destinations below are keyed to the **captions**, which are [V], not to
    /// the ids.
    fn act(&mut self, action: Action) -> Transition {
        match action {
            Action::Item(i) => self.item(i),
            Action::Open(i) => {
                self.under = self.page;
                self.open = i;
                self.page = SetupPage::Dropdown;
                self.selected = self.options[i];
                Transition::Stay
            }
            Action::Choose(v) => {
                self.options[self.open] = v;
                self.page = self.under;
                self.selected = 0;
                Transition::Stay
            }
            Action::Map(row) => {
                self.map = (self.map_top + row).min(MAP_COUNT - 1);
                Transition::Stay
            }
        }
    }

    fn item(&mut self, i: usize) -> Transition {
        match (self.page, i) {
            // Page 1. "Single player" opens page 2; "Multiple players" opens
            // page 4 (or page 10 with no disc); "Lords of Magic?" plays an
            // advertisement we have no player for; "Exit game" quits.
            (SetupPage::Title, 0) => self.go(SetupPage::Options),
            (SetupPage::Title, 1) => self.go(SetupPage::Shield),
            (SetupPage::Title, 2) => self.go(SetupPage::NoCd),
            (SetupPage::Title, 3) => Transition::Quit,
            // Page 2.
            (SetupPage::Options, 0) => self.go(SetupPage::Campaign),
            (SetupPage::Options, 1) => self.go(SetupPage::Load),
            (SetupPage::Options, 2) => self.go(SetupPage::Skirmish),
            (SetupPage::Options, 3) => self.go(SetupPage::Custom),
            (SetupPage::Options, 4) => self.go(SetupPage::Title),
            // Page 4: five shields, then "Back" and "Continue".
            (SetupPage::Shield, 0..=4) => {
                self.shield = i;
                Transition::Stay
            }
            (SetupPage::Shield, 5) => self.go(SetupPage::Title),
            (SetupPage::Shield, 6) => self.start(),
            // Page 5: either campaign. Page 6: full game or skirmish.
            (SetupPage::Campaign, _) => self.go(SetupPage::Shield),
            (SetupPage::GameType, 0) => self.go(SetupPage::Shield),
            (SetupPage::GameType, 1) => self.go(SetupPage::Skirmish),
            // Pages 7 and 8: "Cancel", "Start", "Defaults", "Load".
            (SetupPage::Custom | SetupPage::CustomMulti, 0) => self.go(SetupPage::Options),
            (SetupPage::Custom | SetupPage::CustomMulti, 1) => self.start(),
            (SetupPage::Custom | SetupPage::CustomMulti, 2) => {
                self.options = [0; 12];
                Transition::Stay
            }
            (SetupPage::Custom | SetupPage::CustomMulti, _) => self.go(SetupPage::Load),
            // Pages 11 and 12: "Back", "Cust."/"Norm.", "Go".
            (SetupPage::Skirmish | SetupPage::SkirmishMulti, 0) => self.go(SetupPage::Options),
            (SetupPage::Skirmish | SetupPage::SkirmishMulti, 2) => self.go(SetupPage::Skirmish),
            (SetupPage::Skirmish | SetupPage::SkirmishMulti, _) => Transition::Stay,
            // The three pages with one way out.
            (SetupPage::NoCd, _) => self.go(SetupPage::Title),
            (SetupPage::Load, _) => self.go(SetupPage::Options),
            (SetupPage::SkirmishFile, _) => self.go(SetupPage::Skirmish),
            _ => Transition::Stay,
        }
    }

    fn go(&mut self, page: SetupPage) -> Transition {
        self.page = page;
        self.selected = 0;
        Transition::Stay
    }

    /// The boundary. The setup screen chooses a scenario; nothing in this
    /// workspace can yet build a world from that choice, so *Start* enters the
    /// campaign the scenario loader already put in `Game`.
    fn start(&mut self) -> Transition {
        Transition::Push(ScreenId::Campaign)
    }
}

/// What a rectangle on one of these pages does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Action {
    /// The n'th menu item or button of this page.
    Item(usize),
    /// Open custom-game option n's drop-down.
    Open(usize),
    /// Choose value n from the open drop-down.
    Choose(usize),
    /// The n'th visible row of the map list.
    Map(usize),
}

impl Screen for SetupScreen {
    fn id(&self) -> ScreenId {
        ScreenId::Setup(self.page)
    }

    fn title(&self, ctx: &Ctx) -> String {
        let name = ctx.assets.shell.text(GROUP, 0);
        if name.is_empty() {
            format!("Lords of the Realm II — setup page {}", self.page.number())
        } else {
            format!("{name} — setup page {}", self.page.number())
        }
    }

    fn palette(&self) -> Option<&'static str> {
        Some(self.page.palette())
    }

    fn handle(&mut self, event: Event, _ctx: &mut Ctx) -> Transition {
        let n = self.count().max(1);
        match event {
            Event::KeyDown(Key::Up) => self.selected = (self.selected + n - 1) % n,
            Event::KeyDown(Key::Down) => self.selected = (self.selected + 1) % n,
            Event::KeyDown(Key::Enter) | Event::KeyDown(Key::Space) => return self.activate(),
            // Ours: the demo's index of every screen. `screens::index` says
            // why it exists and marks itself as not the game's.
            Event::KeyDown(Key::Char('I')) => return Transition::Push(ScreenId::Index),
            Event::KeyDown(Key::Escape) => {
                // Whatever the page is, Escape is its own way back — the
                // original's Back button where there is one, and out of the
                // front end where there is not.
                return match self.page {
                    // Pop, not Quit. Popping the last screen quits anyway —
                    // `Machine::apply` — so this is the right answer both when
                    // the front end is the root and when it was opened from
                    // somewhere else, without the screen having to know which.
                    SetupPage::Title => Transition::Pop,
                    SetupPage::Dropdown => {
                        self.page = self.under;
                        Transition::Stay
                    }
                    _ => self.go(SetupPage::Title),
                };
            }
            Event::Pointer { x, y } => {
                if let Some((i, _)) = self.at(x, y) {
                    self.selected = i;
                }
            }
            Event::Click { x, y } => {
                if let Some((i, action)) = self.at(x, y) {
                    self.selected = i;
                    return self.act(action);
                }
            }
            _ => {}
        }
        Transition::Stay
    }

    fn draw(&mut self, ctx: &Ctx, canvas: &mut Canvas) {
        let a = &ctx.assets.shell;
        // **[D]** The front end's own text flags. `DAT_005AEA40` is set around
        // every menu item, button caption and body line and cleared for the
        // heading, so on these pages *only the heading is embossed*; and
        // `DAT_0058FE2C` is set around the heading alone, which draws its
        // capitals in colour 1. So there are two pens here, not one: `head`
        // for the heading and `pen` — flat, no drop capitals — for everything
        // else. A reimplementation that embossed the lot would be wrong on
        // every page of the front end at once.
        let head = Pen {
            assets: a,
            ink: &ctx.assets.ink,
            chrome: ctx.assets.chrome.as_ref(),
            shadow: Some(font::SHADOW_GATEWAY),
            caps: Some(1),
        };
        let pen = head.flat();
        // Page 9 is drawn over whatever was underneath it, so the background
        // and the page beneath are painted first and only then the open list.
        let base = if self.page == SetupPage::Dropdown { self.under } else { self.page };
        if !shell::background(canvas, a, base.background()) {
            // No install, or a partial one. Say so in our own font — never in
            // the original's — so that an empty page can never be mistaken for
            // a page the game drew empty.
            canvas.clear(ctx.assets.ink.background);
            let line = format!(
                "SETUP PAGE {} - NO {}",
                base.number(),
                base.background().to_uppercase()
            );
            l2_view::text::draw(canvas, 4, 4, &line, ctx.assets.ink.dim);
        }
        self.paint(canvas, &pen, &head, base);
        if self.page == SetupPage::Dropdown {
            self.paint_dropdown(canvas, &pen, ctx);
        }
    }
}

impl SetupScreen {
    fn colour(&self, index: usize) -> u8 {
        if index == self.selected {
            font::HIGHLIGHT
        } else {
            font::TEXT
        }
    }

    /// A menu item: the recess, then the caption centred in it.
    fn draw_item(
        &self,
        canvas: &mut Canvas,
        pen: &Pen,
        index: usize,
        rect: Rect,
        group: usize,
        s: usize,
    ) {
        shell::button_recess(canvas, rect.x, rect.y, rect.w, rect.h);
        pen.eng_centred(
            canvas,
            group,
            s,
            rect.x,
            rect.y + ITEM_TEXT,
            rect.w,
            self.colour(index),
        );
    }

    fn paint(&self, canvas: &mut Canvas, pen: &Pen, head: &Pen, page: SetupPage) {
        match page {
            SetupPage::Title => {
                pen.window_from(canvas, BOX_SHEET, 0xA0, 10, 0x14, 0xF);
                head.eng_heading_centred(canvas, GROUP, 0, 0x80, 0x20, 0x180, font::TEXT);
                pen.eng_centred(canvas, GROUP, 1, 0x80, 0x3A, 0x180, font::TEXT);
                for (i, s) in TITLE_ITEMS.iter().enumerate() {
                    self.draw_item(canvas, pen, i, item_rect(i), GROUP, *s);
                }
            }
            SetupPage::Options => {
                pen.window_from(canvas, BOX_SHEET, 0xB0, 10, 0x12, 0x12);
                head.eng_heading_centred(canvas, GROUP, 5, 0xB0, 0x2D, 0x120, font::TEXT);
                for (i, s) in OPTION_ITEMS.iter().enumerate() {
                    self.draw_item(canvas, pen, i, item_rect(i), GROUP, *s);
                }
            }
            SetupPage::Load => {
                // `FUN_004148E4(5)`: the box, the caption, one big recess and
                // three inset rectangles — the name field, the file list and
                // the description. The list itself is `FUN_004149EC`, which
                // walks the save directory; a shell has no directory to walk,
                // so the three rectangles are drawn empty and say so.
                pen.window_from(canvas, BOX_SHEET, 0x60, 10, 0x1C, 0x15);
                head.eng_heading_centred(canvas, GROUP_FILE, 5, 0x60, 0x22, 0x1C0, font::TEXT);
                shell::button_recess(canvas, 0x70, 0x42, 400, 0x100);
                for (x, y, w, h) in [(0x78, 0x4A, 0xC0, 0x20), (0x78, 0x72, 0x160, 0xA4), (0x78, 0x11E, 0x180, 0x1C)] {
                    shell::button_recess(canvas, x, y, w, h);
                }
                pen.eng_centred(canvas, GROUP_FILE, 8, 0x78, 0x124, 0x180, font::TEXT);
            }
            SetupPage::Shield => {
                pen.window_from(canvas, BOX_SHEET, 0x50, 10, 0x1E, 0x10);
                head.eng_heading_centred(canvas, GROUP, 10, 0x50, 0x23, 0x1E0, font::TEXT);
                self.paint_shields(canvas, pen);
                for (i, (x, y, s)) in SHIELD_BUTTONS.iter().enumerate() {
                    self.draw_item(
                        canvas,
                        pen,
                        5 + i,
                        Rect::new(*x, *y, ITEM_W, ITEM_H),
                        GROUP,
                        *s,
                    );
                }
            }
            SetupPage::Campaign | SetupPage::GameType => {
                pen.window_from(canvas, BOX_SHEET, 0x40, 0x32, 0x20, 8);
                head.eng_heading_centred(canvas, GROUP_EXPANSION, 0, 0x40, 0x50, 0x200, font::TEXT);
                let items: [usize; 2] =
                    if page == SetupPage::Campaign { [4, 5] } else { [1, 2] };
                for i in 0..2 {
                    shell::button_recess(canvas, PAIR_X[i], PAIR_Y, PAIR_W, ITEM_H);
                    pen.eng_centred(
                        canvas,
                        GROUP_EXPANSION,
                        items[i],
                        PAIR_TEXT_X[i],
                        PAIR_Y + 6,
                        PAIR_TEXT_W,
                        self.colour(i),
                    );
                }
            }
            SetupPage::NoCd => {
                pen.window_from(canvas, BOX_SHEET, 0x50, 10, 0x1E, 0x13);
                head.eng_heading_centred(canvas, GROUP, 0x10, 0x50, 0x24, 0x1E0, font::TEXT);
                // Three wrapped paragraphs at width 0x180 and one plain line.
                // The plain one is drawn in colour 1, not 0x3F — the only
                // string on any of these pages that is.
                let a = pen.assets;
                for (i, y) in [(0x11usize, 0x48), (0x12, 0x78)] {
                    let t = a.text(GROUP, i).to_string();
                    pen.body_wrapped(canvas, 0x80, y, 0x180, &t, font::TEXT);
                }
                pen.eng(canvas, GROUP, 0x30, 0x80, 0xDC, 1);
                let t = a.text(GROUP, 0x31).to_string();
                pen.body_wrapped(canvas, 0x80, 0xF0, 0x180, &t, font::TEXT);
            }
            SetupPage::Custom | SetupPage::CustomMulti | SetupPage::Dropdown => {
                self.paint_custom(canvas, pen, page)
            }
            SetupPage::Skirmish | SetupPage::SkirmishMulti | SetupPage::SkirmishFile => {
                self.paint_skirmish(canvas, pen, head, page)
            }
        }
    }

    /// `FUN_0041F1DD` and `FUN_0041F321`: the name field and the five shields.
    ///
    /// **[V]** and worth writing down, because the obvious reading is wrong.
    /// The painter blits from `DAT_004EABEC` — the general scratch buffer,
    /// which on this page holds **`panels2.pl8`** — and *not* from
    /// `g_miscCtySheet`. `Misc_sel.pl8` has seventeen frames; the indices here
    /// run to 215, and `Panels2.pl8` has 216. The file settles it: frames
    /// 205 … 214 are five pairs of roughly 60 × 65 shields, one pair per realm
    /// colour, 204 is a 224 × 32 plate the size of a name field, and 215 is a
    /// 54 × 27 plaque. Nothing else in either file is that shape.
    ///
    /// Frame `2i + 0xCB` is the shield when the colour is free and `2i + 0xCC`
    /// when it is taken, at `x = 0x70 + 88(i - 1)`, `y = 0x8C`; the 54 × 27
    /// plaque marks the chosen one at `(x + 4, 0x70)`.
    fn paint_shields(&self, canvas: &mut Canvas, pen: &Pen) {
        let Some(sheet) = pen.assets.sheet(BOX_SHEET) else { return };
        // The name field, and the player's name in it at (0xD6, 0x50).
        if let Some(f) = sheet.frame(0xCC) {
            canvas.blit(&f, 0xD0, 0x48);
        }
        for i in 1..6usize {
            let x = SHIELD_X + (i as i32 - 1) * SHIELD_STEP;
            if i - 1 == self.shield {
                if let Some(f) = sheet.frame(0xD7) {
                    canvas.blit(&f, x + 4, 0x70);
                }
            }
            // Free, not taken: this shell has no lobby, so every colour is
            // offered and the taken variant (`2i + 0xCC`) is never drawn.
            if let Some(f) = sheet.frame(i * 2 + 0xCB) {
                canvas.blit(&f, x, 0x8C);
            }
        }
    }

    /// Pages 7 and 8: the twelve options, the map list, the buttons.
    fn paint_custom(&self, canvas: &mut Canvas, pen: &Pen, page: SetupPage) {
        let a = pen.assets;
        if page == SetupPage::Custom {
            if let Some(s) = a.sheet(ICON_SHEET) {
                if let Some(f) = s.frame(0x0F) {
                    canvas.blit(&f, 0xA0, 0);
                }
            }
        }
        // The map list plate, its five rows, and the row the pointer is on.
        if let Some(s) = a.sheet(ICON_SHEET) {
            if let Some(f) = s.frame(0x10) {
                canvas.blit(&f, MAP_LIST_X, 9);
            }
        }
        for row in 0..MAP_LIST_ROWS {
            let slot = self.map_top + row;
            if slot >= MAP_COUNT {
                break;
            }
            let y = MAP_LIST_Y + row as i32 * MAP_LIST_ROW;
            let chosen = slot == self.map;
            canvas.fill_rect(
                MAP_LIST_X,
                y,
                MAP_LIST_W,
                MAP_LIST_ROW,
                if chosen { font::TEXT } else { font::DISABLED },
            );
            let name = a.text(GROUP_MAPS, slot).to_string();
            pen.body(
                canvas,
                MAP_LIST_TEXT_X,
                y + 1,
                &name,
                if chosen { font::DISABLED } else { font::TEXT },
            );
        }
        // The twelve options.
        let chrome = pen.chrome;
        for (i, &(x, boxy, labely)) in OPTION_CELLS.iter().enumerate() {
            // `FUN_0040328E(102, i, x, labelY, 100, …)`: wrapped at 100 pixels,
            // which is what makes "Advanced Farming" two lines that end where
            // the box begins instead of one that runs into the next column.
            let label = a.text(GROUP_OPTIONS, i).to_string();
            pen.body_wrapped(canvas, x, labely, OPTION_LABEL_W, &label, font::TEXT);
            match chrome {
                // `FUN_004093E0` is `Ui_DrawBox` with border set 1.
                Some(c) => c.draw_box(canvas, x, boxy, 6, 3, 1),
                None => shell::button_recess(canvas, x, boxy, OPTION_BOX_W, OPTION_BOX_H),
            }
            let value = a.text(GROUP_VALUES, self.option_value(i)).to_string();
            pen.body_centred(canvas, x + 1, boxy + 16, 0x60, &value, font::HIGHLIGHT);
        }
        let n = if page == SetupPage::Custom { 3 } else { 4 };
        for (i, (x, s)) in CUSTOM_BUTTONS.iter().take(n).enumerate() {
            pen.eng_centred(
                canvas,
                GROUP,
                *s,
                *x,
                CUSTOM_BUTTON_Y,
                CUSTOM_BUTTON_W,
                self.colour(12 + MAP_LIST_ROWS + i),
            );
        }
        if page == SetupPage::CustomMulti {
            // [I] Page 8 also draws the five player cards down the left edge
            // (`FUN_0041FBCB`, `misc_sel` frames `2 * shield - 2` at x = 10,
            // 94 apart) and the eight-line chat log at (0xA8, 8). Neither has
            // any state in this workspace, so neither is drawn; the page is
            // otherwise page 7 with a fourth button.
        }
    }

    /// Page 9: the box the option opens, over the page underneath.
    fn paint_dropdown(&self, canvas: &mut Canvas, pen: &Pen, _ctx: &Ctx) {
        let (x, y, rows) = OPTION_LIST[self.open];
        let y = self.dropdown_y(y, rows);
        pen.window(canvas, x, y, 6, rows, 1);
        for i in 0..OPTION_COUNT[self.open] {
            let s = pen.assets.text(GROUP_VALUES, OPTION_BASE[self.open] + i).to_string();
            let colour = if i == self.selected { font::HIGHLIGHT } else { font::TEXT };
            pen.body_centred(canvas, x + 1, y + 16 + i as i32 * 16, 0x60, &s, colour);
        }
    }

    /// Pages 11, 12 and 13. The battlefield itself is `l2-sim`'s, and the
    /// skirmish editor's palette of tools is `L2.eng` group 41; neither is
    /// drawn here. What is drawn is the page's own furniture: the background,
    /// and the three captions at `y = 0x1B8` that `FUN_0042051C` and
    /// `FUN_00420630` put there.
    fn paint_skirmish(&self, canvas: &mut Canvas, pen: &Pen, head: &Pen, page: SetupPage) {
        let items: [usize; 3] = match page {
            // 12 and 11: "Back", "Cust.", "Go".
            SetupPage::SkirmishFile => [0x24, 0x25, 0x26],
            _ => [0x25, 0x26, 0x24],
        };
        for (i, x) in [0x1CD, 0x207, 0x241].iter().enumerate() {
            pen.eng_centred(canvas, GROUP, items[i], *x, 0x1B8, 0x38, self.colour(i));
        }
        if page == SetupPage::SkirmishFile {
            // `FUN_0042150B` opens `Ui_DrawBox(0x60, 100, 0x1C, 0x12)` over the
            // skirmish page and draws group 40's file captions into it.
            pen.window(canvas, 0x60, 100, 0x1C, 0x12, 0);
            head.eng_heading_centred(canvas, GROUP_FILE, 6, 0x60, 0x84, 0x1C0, font::TEXT);
            pen.eng_centred(canvas, GROUP_FILE, 8, 0x60, 0x164, 0x1C0, font::TEXT);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn there_are_thirteen_pages_and_each_knows_its_number() {
        assert_eq!(SetupPage::ALL.len(), 13);
        for (i, p) in SetupPage::ALL.iter().enumerate() {
            assert_eq!(p.number() as usize, i + 1);
        }
    }

    #[test]
    fn the_twelve_option_runs_cover_group_103_with_one_string_left_over() {
        // The counts come out of the drop-down table as rows minus two.
        for i in 0..12 {
            assert_eq!(OPTION_COUNT[i], OPTION_LIST[i].2 as usize - 2, "option {i}");
        }
        // Every run ends where the next begins, except the gap at index 4.
        for i in 0..11 {
            let end = OPTION_BASE[i] + OPTION_COUNT[i];
            let next = OPTION_BASE[i + 1];
            assert!(end <= next, "option {i} runs past option {}", i + 1);
            assert!(next - end <= 1, "option {i} leaves {} strings unused", next - end);
        }
        let used: usize = OPTION_COUNT.iter().sum();
        assert_eq!(used, 45);
        assert_eq!(OPTION_BASE[11] + OPTION_COUNT[11], 46, "the last run ends at the group's end");
        // The one gap is "one", which Nobles skips: it starts at "two".
        let gaps: Vec<usize> =
            (0..11).filter(|&i| OPTION_BASE[i + 1] != OPTION_BASE[i] + OPTION_COUNT[i]).collect();
        assert_eq!(gaps, vec![1], "exactly one gap, after Exploration");
    }

    #[test]
    fn the_option_grid_is_four_columns_of_three() {
        let xs: Vec<i32> = OPTION_CELLS.iter().map(|c| c.0).collect();
        assert_eq!(xs, vec![170, 170, 170, 290, 290, 290, 410, 410, 410, 530, 530, 530]);
        // Every drop-down opens at its own box's x.
        for i in 0..12 {
            assert_eq!(OPTION_LIST[i].0, OPTION_CELLS[i].0, "option {i}");
        }
    }

    #[test]
    fn the_menu_items_step_thirty_six_apart_from_ninety_one() {
        assert_eq!(item_rect(0), Rect::new(0xE0, 0x5B, 0xC0, 0x18));
        assert_eq!(item_rect(1).y, 0x7F);
        assert_eq!(item_rect(2).y, 0xA3);
        assert_eq!(item_rect(3).y, 0xC7);
        assert_eq!(item_rect(4).y, 0xEB);
    }

    #[test]
    fn every_page_names_a_background_and_a_palette_that_go_together() {
        for p in SetupPage::ALL {
            let (b, pal) = (p.background(), p.palette());
            assert_eq!(
                b.split('.').next(),
                pal.split('.').next(),
                "page {} loads {b} but sets {pal}",
                p.number()
            );
        }
    }
}

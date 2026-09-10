//! **Ours, not the game's.** The demo's index of every screen there is.
//!
//! Nothing like this exists in `Lords2.exe`. It is here because most of the
//! twenty-nine screens `docs/screens-county.md` §1 identifies cannot yet be
//! *reached*: the court opens off a sidebar button that needs a selected
//! county, the siege screen needs a siege, the ratings screen needs a battle to
//! have happened. A click-through demo whose screens cannot be clicked to is
//! not a demo, so this is the door to all of them.
//!
//! It is drawn in `l2_view::text` — **our** 5 × 7 font, never `Fntl2_14.pl8` —
//! and titled as ours, so that a screenshot of it can never be mistaken for
//! something the original drew. Every other screen in this crate is trying to
//! look like the game; this one is deliberately not.
//!
//! Press `I` on the campaign map, or `I` on the front end's title page.

use l2_view::{text, Canvas};

use crate::input::{Event, Key, Rect};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::screens::options::Page as OptionsPage;
use crate::screens::saveload::Mode as SaveLoadMode;
use crate::screens::setup::SetupPage;
use crate::widget;

/// One row: what it is called, where it goes, and whether it goes anywhere.
struct Row {
    label: String,
    to: Option<ScreenId>,
}

const COL_W: i32 = 300;
const ROW_H: i32 = 13;
const TOP: i32 = 44;
const LEFT: i32 = 12;
/// Two columns of this many rows each.
const PER_COL: usize = 32;

pub struct IndexScreen {
    rows: Vec<Row>,
    selected: usize,
}

impl IndexScreen {
    pub fn new() -> IndexScreen {
        let mut rows = Vec::new();
        let mut push = |label: String, to: Option<ScreenId>| rows.push(Row { label, to });

        push("-- IMPLEMENTED --".into(), None);
        push("0x00 CAMPAIGN MAP".into(), Some(ScreenId::Campaign));
        push("0x14/15/16/19 COUNTY PANELS".into(), Some(ScreenId::County(1, crate::screens::county::Panel::Tax)));
        // `0x02` is being written by another agent. This is the hook and
        // nothing else: no `ScreenId` for it is invented here, because the
        // agent that owns the screen owns its id.
        push("0x02 THE VILLAGE (ANOTHER AGENT)".into(), None);
        // Unit 1 is a stand-in: the screen takes the besieging army's slot, and
        // from the index there is no siege in progress to take it from. It
        // draws an empty order and a zero countdown, which is what the original
        // shows the instant a siege is laid.
        push("0x1D SIEGE PREPARATIONS".into(), Some(ScreenId::Siege(1)));
        // The same stand-in argument as the siege screen's, for the same
        // reason: from the index there is no selected county and no selected
        // army to take one from. `0x17` on county 1 draws the no-offer layout
        // with a zero levy; `0x11` on a slot with no army in it says so.
        push("0x17 RAISE AN ARMY".into(), Some(ScreenId::RaiseArmy(1)));
        // The other half of it, and the half that raises the army. From the
        // index the levy is whatever the last one left, so an armoury opened
        // cold shows a levy of nobody with every rack full — which is exactly
        // what the original shows before a slider has been touched.
        push("0x0A THE ARMOURY".into(), Some(ScreenId::Armoury(1)));
        push("0x0D THE SWORD RACK".into(), Some(ScreenId::Rack(1, 3)));
        push("0x11 ARMY DIVISION".into(), Some(ScreenId::Divide(1)));
        // The same stand-in again, and here it shows something the screen
        // itself is about: unit 1 is not a merchant from the index, so its
        // morale reads 0 and every buy price falls to the markup's floor of one
        // crown. That is the formula working, not a placeholder — reached from
        // the map the unit is a merchant at morale 100 and the prices double.
        push("0x08 THE MERCHANT".into(), Some(ScreenId::Merchant(1)));
        push("0x0C TRADE GOODS (GRAIN)".into(), Some(ScreenId::Trade(1, 1)));
        push("0x0B THE OTHER LORDS".into(), Some(ScreenId::Diplomacy));
        // The compose dialog has three shapes and the index reaches all three,
        // because they are three painters and three widget tables rather than
        // three states of one. Rival 2 is a stand-in the same way unit 1 is
        // above: from the index there is no `g_diploTarget` to take.
        push("0x1A   DISPATCH A GIFT".into(), Some(ScreenId::DiploCompose(2, 0)));
        push("0x1A   A LETTER".into(), Some(ScreenId::DiploCompose(2, 1)));
        push("0x1A   ASK AN ALLY FOR HELP".into(), Some(ScreenId::DiploCompose(2, 5)));
        push("0x35 LOAD A CONQUEST".into(), Some(ScreenId::SaveLoad(SaveLoadMode::Load)));
        push("0x36 SAVE A CONQUEST".into(), Some(ScreenId::SaveLoad(SaveLoadMode::Save)));
        // The last seven shells. Four take no argument; the other three take a
        // county or a right-click target and get the same stand-in the siege
        // and merchant rows above take, for the same reason.
        push("0x04 MAP INFO: A UNIT".into(), Some(ScreenId::Info(crate::screens::info::Target::Unit(1))));
        push("0x04 MAP INFO: A TILE".into(), Some(ScreenId::Info(crate::screens::info::Target::Tile(0))));
        push("0x09 THE COURT".into(), Some(ScreenId::Court));
        push("0x0B DIPLOMACY".into(), Some(ScreenId::Diplomacy));
        push("0x18 SEND SUPPLIES".into(), Some(ScreenId::Supplies(1)));
        push("0x25 ABOUT".into(), Some(ScreenId::About));
        push("0x2E BATTLE MASTER RATINGS".into(), Some(ScreenId::Ratings));

        push(String::new(), None);
        push("-- 0x1F GAME SETUP, 13 PAGES --".into(), None);
        for p in SetupPage::ALL {
            push(format!("  PAGE {:>2}  {}", p.number(), setup_name(p)), Some(ScreenId::Setup(p)));
        }

        push(String::new(), None);
        push("-- OPTIONS: 4 PANELS, 1 MENU --".into(), None);
        for p in OptionsPage::ALL {
            let label = match p.screen_id() {
                Some(id) => format!("  0x{id:02X} POPUP {}", options_name(p)),
                None => format!("  OURS  POPUP {}", options_name(p)),
            };
            push(label, Some(ScreenId::Options(p)));
        }

        push(String::new(), None);
        push("-- 0x1C CONQUEST --".into(), None);
        push("  WON / LOST / CAMPAIGN OVER".into(), Some(ScreenId::Conquest));


        IndexScreen { rows, selected: 1 }
    }

    fn rect(&self, i: usize) -> Rect {
        let (col, row) = (i / PER_COL, i % PER_COL);
        Rect::new(LEFT + col as i32 * COL_W, TOP + row as i32 * ROW_H, COL_W - 8, ROW_H)
    }

    fn at(&self, x: i32, y: i32) -> Option<usize> {
        (0..self.rows.len())
            .find(|&i| self.rows[i].to.is_some() && self.rect(i).contains(x, y))
    }

    /// Step to the next row that goes somewhere, so the headings cannot be
    /// landed on.
    fn step(&mut self, by: i32) {
        let n = self.rows.len();
        for _ in 0..n {
            self.selected = ((self.selected as i32 + by).rem_euclid(n as i32)) as usize;
            if self.rows[self.selected].to.is_some() {
                return;
            }
        }
    }

    fn activate(&self) -> Transition {
        match self.rows.get(self.selected).and_then(|r| r.to) {
            Some(id) => Transition::Push(id),
            None => Transition::Stay,
        }
    }
}

impl Default for IndexScreen {
    fn default() -> Self {
        IndexScreen::new()
    }
}

/// What each setup page is, in this index's words.
fn setup_name(p: SetupPage) -> &'static str {
    match p {
        SetupPage::Title => "TITLE MENU",
        SetupPage::Options => "YOUR OPTIONS",
        SetupPage::Load => "LOAD A GAME",
        SetupPage::Shield => "TITLE AND SHIELD",
        SetupPage::Campaign => "WHICH CAMPAIGN",
        SetupPage::GameType => "FULL GAME OR SKIRMISH",
        SetupPage::Custom => "CUSTOM GAME",
        SetupPage::CustomMulti => "CUSTOM GAME, MULTIPLAYER",
        SetupPage::Dropdown => "AN OPTION DROP-DOWN",
        SetupPage::NoCd => "NO LORDS OF THE REALM CD",
        SetupPage::SkirmishMulti => "SKIRMISH, MULTIPLAYER",
        SetupPage::Skirmish => "SKIRMISH",
        SetupPage::SkirmishFile => "SKIRMISH FILE BOX",
    }
}

impl Screen for IndexScreen {
    fn id(&self) -> ScreenId {
        ScreenId::Index
    }

    fn title(&self, _ctx: &Ctx) -> String {
        "open-lords2 — screen index (ours)".into()
    }

    fn handle(&mut self, event: Event, _ctx: &mut Ctx) -> Transition {
        match event {
            Event::KeyDown(Key::Escape) => Transition::Pop,
            Event::KeyDown(Key::Up) => {
                self.step(-1);
                Transition::Stay
            }
            Event::KeyDown(Key::Down) => {
                self.step(1);
                Transition::Stay
            }
            Event::KeyDown(Key::Left) => {
                self.step(-(PER_COL as i32));
                Transition::Stay
            }
            Event::KeyDown(Key::Right) => {
                self.step(PER_COL as i32);
                Transition::Stay
            }
            Event::KeyDown(Key::Enter) | Event::KeyDown(Key::Space) => self.activate(),
            Event::Pointer { x, y } => {
                if let Some(i) = self.at(x, y) {
                    self.selected = i;
                }
                Transition::Stay
            }
            Event::Click { x, y } => match self.at(x, y) {
                Some(i) => {
                    self.selected = i;
                    self.activate()
                }
                None => Transition::Stay,
            },
            _ => Transition::Stay,
        }
    }

    fn draw(&mut self, ctx: &Ctx, canvas: &mut Canvas) {
        let ink = &ctx.assets.ink;
        canvas.clear(ink.background);
        text::draw(canvas, LEFT, 10, "OPEN-LORDS2 SCREEN INDEX", ink.highlight);
        text::draw(
            canvas,
            LEFT,
            24,
            "THIS SCREEN IS OURS. THE GAME HAS NOTHING LIKE IT.",
            ink.dim,
        );

        for (i, row) in self.rows.iter().enumerate() {
            if row.label.is_empty() {
                continue;
            }
            let r = self.rect(i);
            if r.y + ROW_H > 460 {
                continue;
            }
            match row.to {
                None => {
                    text::draw(canvas, r.x, r.y, &row.label, ink.dim);
                }
                Some(_) if i == self.selected => {
                    widget::frame(canvas, r, ink.highlight);
                    text::draw(canvas, r.x + 3, r.y + 3, &row.label, ink.highlight);
                }
                Some(_) => {
                    text::draw(canvas, r.x + 3, r.y + 3, &row.label, ink.text);
                }
            }
        }

        let missing = if ctx.assets.shell.has_artwork() {
            "GAME ARTWORK AND L2.ENG LOADED"
        } else {
            "NO GAME ARTWORK: EVERY SHELL IS DRAWN IN OUR OWN PLACEHOLDER"
        };
        text::draw(canvas, LEFT, 464, missing, ink.dim);
    }
}

/// What the index calls each options page. The original supplies a name for
/// four of the five (`L2.eng` group 2 and group 45's own heading); the fifth is
/// ours and says so.
fn options_name(p: OptionsPage) -> &'static str {
    match p {
        OptionsPage::Advanced => "ADVANCED OPTIONS",
        OptionsPage::Sound => "SOUNDS",
        OptionsPage::Display => "DISPLAY OPTIONS",
        OptionsPage::Help => "HELP OPTIONS",
        OptionsPage::Quirks => "THE ORIGINAL'S BUGS (OURS)",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_screen_this_workspace_can_draw_is_reachable_from_here() {
        let s = IndexScreen::new();
        let dests: Vec<ScreenId> = s.rows.iter().filter_map(|r| r.to).collect();
        assert!(dests.contains(&ScreenId::Campaign));
        assert!(dests.contains(&ScreenId::County(1, crate::screens::county::Panel::Tax)));
        assert!(dests.contains(&ScreenId::Conquest));
        assert!(dests.contains(&ScreenId::SaveLoad(SaveLoadMode::Load)));
        assert!(dests.contains(&ScreenId::SaveLoad(SaveLoadMode::Save)));
        for p in SetupPage::ALL {
            assert!(dests.contains(&ScreenId::Setup(p)), "setup page {}", p.number());
        }
        // **The shell section is gone**, because the table is empty. What
        // replaces it is the graduated list: every id that ever had a row must
        // be reachable from here under its own name, or the demo has lost a
        // door that used to exist.
        for &(id, module) in crate::screens::shells::GRADUATED {
            if let Some(to) = crate::screens::shells::screen_for(id) {
                assert!(
                    dests.contains(&to),
                    "0x{id:02X} ({module}) graduated and is not on the index",
                );
            }
        }
        for p in OptionsPage::ALL {
            assert!(
                dests.contains(&ScreenId::Options(p)),
                "options page {p:?} is not reachable from the index"
            );
        }
    }

    #[test]
    fn the_headings_cannot_be_selected() {
        let mut s = IndexScreen::new();
        for _ in 0..s.rows.len() * 2 {
            s.step(1);
            assert!(s.rows[s.selected].to.is_some(), "landed on a heading");
        }
    }

    #[test]
    fn the_two_columns_do_not_overlap_and_stay_on_screen() {
        let s = IndexScreen::new();
        for i in 0..s.rows.len() {
            let r = s.rect(i);
            assert!(r.x >= 0 && r.x + r.w <= 640, "row {i} at x {}", r.x);
        }
        assert!(s.rows.len() <= PER_COL * 2, "the index needs a third column");
    }
}

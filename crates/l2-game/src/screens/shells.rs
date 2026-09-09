//! The screens that are drawn but not yet wired: one table, one painter.
//!
//! # What a shell is
//!
//! Each entry below names the screen's `g_screenId`, its painter's address,
//! the `.pl8` that painter loads, the window it opens, and the `L2.eng` group
//! it draws — every one of them read out of the painter, none of them invented.
//! [`ShellScreen`] draws exactly that and nothing else, so what appears on
//! screen is the original's artwork with the original's words on it, sitting in
//! the original's rectangles, with no logic behind it.
//!
//! This is deliberately not a "generic panel". A generic panel would be the
//! invented interface layer `screens/county.rs` warns about. What makes these
//! honest is that every number in the table has an address next to it, and that
//! [`Shell::unfinished`] says in the screen's own words what the painter draws
//! that the shell does not.
//!
//! # Popups are popups
//!
//! `docs/screens-county.md` §1: *"Our five-screen model is not the game's. The
//! management surface is a campaign map plus insets."* The screens marked
//! [`Shell::overlay`] are drawn over whatever was underneath rather than
//! clearing to a page of their own, which is what
//! [`crate::screen::Machine::draw`] walking back to the last non-overlay screen
//! is for.
//!
//! **The word in that quotation moved from "popups" to "insets" for a reason.**
//! An inset need not look like a window: the village is a raw 363 × 320 blit at
//! (64, 64) with no frame and no clear at all, and was modelled as a page until
//! a player said he could see the map around it (`docs/decisions.md` C22). A
//! shell that loads a `.pl8` is not thereby a page either — read its rectangle.
//!
//! # The three that were whole pictures
//!
//! The merchant (`0x08`), the armoury (`0x0A`) and castle building (`0x1B`)
//! each load a **640 × 480 `.pl8` and a `.256` of their own** and draw their
//! widgets on top. For those three a shell was very nearly the real screen: the
//! artwork is the artwork, and what was missing is the grid of prices, the
//! weapon stocks and the castle plan drawn over it. Two of the three have left.
//!
//! **The armoury is the row that shows what "very nearly" was worth.** Its
//! artwork was right and everything else about the row was wrong: the `L2.eng`
//! group, the description of `arm_grid.pl8`, and — worst — the implication that
//! a screen drawing the artwork was most of the screen. It is where an army is
//! *created*; the levy screen next door cannot do it. See `screens/armoury.rs`.

use l2_view::Canvas;

use crate::input::{Event, Key};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::shell::{self, font, Pen};

/// One line of `L2.eng` a painter draws at a fixed place: `(index, x, y)`.
pub type Line = (usize, i32, i32);

/// A screen we can draw and cannot yet drive.
pub struct Shell {
    /// `g_screenId`, the byte `Screen_Draw` switches on.
    pub id: u8,
    /// Its painter, for anyone going back to the binary.
    pub painter: u32,
    /// What this document calls it.
    pub name: &'static str,
    /// The full-screen background, if the painter loads one.
    pub background: Option<&'static str>,
    /// The `.256` it sets with it.
    pub palette: Option<&'static str>,
    /// `Ui_DrawBox`/`FUN_004093E0`: `(x, y, cols, rows, borderSet)` in cells of
    /// 16 pixels. `FUN_004093E0` is border set 1, `Ui_DrawBox` is set 0.
    pub window: Option<(i32, i32, i32, i32, usize)>,
    /// The `L2.eng` group this painter draws.
    pub group: usize,
    /// The heading, in the 22-pixel font.
    pub heading: Option<Line>,
    /// The body lines, in the 14-pixel font.
    pub lines: &'static [Line],
    /// `Ui_OkButton(x, y, mode)` — the corner picture that closes the panel: a
    /// mouse pointer going into a black hole, not a tick. Mode 0 is
    /// button-sheet frame `0x33`, mode 1 is frame `0x10`.
    pub ok: Option<(i32, i32, usize)>,
    /// Whether this draws over what was underneath rather than replacing it.
    pub overlay: bool,
    /// What the painter does that this shell does not. Shown on the screen, so
    /// that nobody walking the interface mistakes a shell for a finished one.
    pub unfinished: &'static str,
}

/// Every shell, in `g_screenId` order.
///
/// `0x00` the campaign map, `0x02` the village and `0x14`/`0x15`/`0x16`/`0x19`
/// the four county panels are **not** here: those are implemented screens.
pub const SHELLS: &[Shell] = &[
    Shell {
        id: 0x04,
        painter: 0x0041_B032,
        name: "The map information panel",
        background: None,
        palette: None,
        // `FUN_0041B032` draws no `Ui_DrawBox`: it paints over the campaign map
        // and its two halves place their own lines. Ours has no window either,
        // which is why what it says about itself is the whole of it.
        window: None,
        // `UnitPanel_Draw` (`0x0041B19D`) draws group 31 index 9, "An army
        // from", and then the county name out of group 100 at
        // `homeCounty + scenarioIndex * 20`. The y is a runtime row
        // (`DAT_00553D2C * 16 + 74`) and we will not invent one, so the shell
        // places no lines rather than placing them somewhere plausible.
        group: 31,
        heading: None,
        lines: &[],
        // `Ui_OkButton(0x1AC, 0x1B6, 0)`, in both halves of the painter.
        ok: Some((0x1AC, 0x1B6, 0)),
        overlay: true,
        unfinished: "the unit's own lines, its county of origin, and the tile panel for a \
                     right-click that hits no unit",
    },
    // `0x08` **graduated**, and so did `0x0C` below it. This row said *"the
    // price grid (mercgrid.pl8) and the eight commodities"*, and both halves
    // were wrong in a way worth keeping: `mercgrid.pl8` is **not drawn** — it is
    // the hit test, an 80 x 60 byte map of good ids over the whole screen — and
    // there are twelve wares on the stall rather than eight. See
    // `screens/merchant.rs`.
    Shell {
        id: 0x09,
        painter: 0x0041_6925,
        name: "The court",
        background: None,
        palette: None,
        window: Some((0x40, 0x30, 0x18, 0x16, 1)),
        group: 70,
        heading: Some((5, 0x50, 0x44)),
        lines: &[(0, 0x60, 0x72), (2, 0x60, 0x90), (3, 0x60, 0xAE), (4, 0x60, 0xCC), (6, 0x88, 0x15C)],
        ok: Some((0x198, 0x158, 0)),
        overlay: true,
        unfinished: "the realm's stock numbers, the six weapon rows and the wage lines",
    },
    // `0x0A` **graduated, and it took `0x0D` with it and a mistake out of this
    // row.** The group was **69**, not 16 — 16 is the twelve mercenary
    // nationalities and the armoury's painter never touches it, while 69/6,
    // 69/7 and 69/8 are the three words on its right-hand edge: *"Create"*,
    // *"Change"*, *"Cancel"*. Filing the screen under the wrong group is what
    // made it look like a mercenary panel with nothing behind it rather than
    // the screen the whole levy is confirmed on. And `arm_grid.pl8` is not a
    // "buy grid": nothing here is bought. See `screens/armoury.rs`.
    Shell {
        id: 0x0B,
        painter: 0x0041_6CF3,
        name: "The other lords",
        background: None,
        palette: None,
        window: Some((0x10, 0x20, 0x1C, 0x1B, 1)),
        group: 72,
        heading: None,
        // `g_diploMenuState == 0`, the no-alliance layout: four actions at
        // x = 0xE0, 50 apart from y = 0x70.
        lines: &[(2, 0xE0, 0x70), (3, 0xE0, 0xA2), (4, 0xE0, 0xD4), (5, 0xE0, 0x106)],
        ok: Some((0x1A8, 0x1A6, 0)),
        overlay: true,
        unfinished: "the lord cards from faces.pl8 and the other three menu layouts",
    },
    Shell {
        id: 0x0F,
        painter: 0x0041_2B33,
        name: "The job popup",
        background: None,
        palette: None,
        // [I] `Panel_JobDetail`'s own window was not read; the box below is the
        // county panels' shape and is marked as ours rather than the game's.
        window: Some((0x40, 0x40, 0x18, 0x12, 1)),
        group: 74,
        heading: Some((1, 0x50, 0x54)),
        lines: &[],
        ok: None,
        overlay: true,
        unfinished: "everything: the worker count, the output and the arrows. The window is ours",
    },
    // `0x11` **graduated**. It was the row that said *"the eight troop rows and
    // the two Total men lines"*; both columns of both are drawn now, out of a
    // real [`l2_kingdom::SplitBasket`], and the buttons split and disband. See
    // `screens/divide.rs`.
    // `0x17` **graduated, and its name was the finding.** This table called it
    // *"Hire mercenaries"* while `docs/symbols.json` called its painter
    // `Screen_RaiseArmy` — and there is no mercenaries screen in the game at
    // all: the offer is a block on the raise-army screen, which is the only
    // door to `Army_Create` a player has. A name is a claim, and this one set
    // the priority of the most gameplay-critical shell in the table for weeks.
    // `docs/decisions.md` C45. See `screens/army.rs`.
    Shell {
        id: 0x18,
        painter: 0x0041_AD5D,
        name: "Send supplies",
        background: None,
        palette: None,
        window: Some((0x40, 0x30, 0x16, 0x16, 1)),
        group: 33,
        heading: Some((0, 0x50, 0x44)),
        lines: &[(7, 0xF0, 0x70), (1, 0xF0, 0x9E), (2, 0xF0, 0xC2), (6, 0x60, 0x160)],
        ok: None,
        overlay: true,
        unfinished: "the county picture, the two county names and the grain/sheep/cattle rows",
    },
    Shell {
        id: 0x1B,
        painter: 0x0041_9789,
        name: "Castle building",
        background: Some("Cas_back.pl8"),
        palette: Some("Cas_back.256"),
        window: None,
        group: 30,
        heading: None,
        lines: &[],
        ok: Some((640 - 0x1C, 480 - 0x1C, 1)),
        overlay: false,
        unfinished: "the castle plan from caspics.pl8 and the piece palette from cas_bits.pl8",
    },
    Shell {
        id: 0x25,
        painter: 0x0041_543F,
        name: "About",
        background: None,
        palette: None,
        window: Some((0x60, 0xE0, 0x16, 0x09, 1)),
        group: 59,
        heading: Some((0, 0x80, 0xF4)),
        lines: &[(2, 0x80, 0x110), (1, 0x80, 0x148)],
        ok: Some((0x194, 0x146, 0)),
        overlay: true,
        unfinished: "nothing — this is the whole screen",
    },
    Shell {
        id: 0x2E,
        painter: 0x0042_1707,
        name: "Battle master ratings",
        background: Some("Score1.pl8"),
        palette: Some("Score1.256"),
        window: None,
        group: 37,
        // `Ui_DrawCentred(37, 0, 0x60, 0x44, 0x1BE, heading, 0x3F)`; the shell
        // draws it left-aligned at the same x, which is close but not it.
        heading: Some((0, 0x60, 0x44)),
        lines: &[(2, 0x68, 0xC8), (3, 0x68, 0xDC)],
        ok: Some((0x204, 0x186, 0)),
        overlay: false,
        unfinished: "the seven rating rows per player and the shield sprites",
    },
    Shell {
        id: 0x31,
        painter: 0x0041_54EA,
        name: "Help options",
        background: None,
        palette: None,
        window: Some((0x60, 0x80, 0x16, 0x0B, 1)),
        group: 45,
        heading: Some((0, 0x80, 0x94)),
        lines: &[(1, 0x80, 0xC0), (2, 0x80, 0xE0), (3, 0x80, 0x100)],
        ok: Some((0x194, 0x106, 0)),
        overlay: true,
        unfinished: "the On/Off values from group 18, which need the settings",
    },
    // `0x35` and `0x36` **graduated**. They were the two rows here that said
    // *"the file list, which walks the save directory"*; the list walks it now
    // and the buttons work, so they are `screens/saveload.rs` and no longer a
    // shell. See the note on `find` below.
    Shell {
        id: 0x39,
        painter: 0x0041_4F68,
        name: "Advanced options",
        background: None,
        palette: None,
        window: Some((0x30, 0x60, 0x18, 0x0D, 1)),
        group: 50,
        heading: Some((0, 0x40, 0x74)),
        lines: &[(1, 0x60, 0xA0), (2, 0x60, 0xC0), (3, 0x60, 0xE0), (4, 0x60, 0x100)],
        ok: None,
        overlay: true,
        unfinished: "the four Yes/No values from group 18 at x = 0x140",
    },
    Shell {
        id: 0x42,
        painter: 0x0041_515C,
        name: "Sound options",
        background: None,
        palette: None,
        window: Some((0x30, 0x60, 0x18, 0x0C, 1)),
        group: 51,
        heading: Some((0, 0x40, 0x74)),
        lines: &[(1, 0x60, 0xA0), (2, 0x60, 0xC0), (3, 0x60, 0xE0)],
        ok: Some((0x188, 0xF0, 0)),
        overlay: true,
        unfinished: "the three On/Off values from group 19 at x = 0x140",
    },
    Shell {
        id: 0x43,
        painter: 0x0041_52EA,
        name: "Display options",
        background: None,
        palette: None,
        window: Some((0x30, 0x90, 0x18, 0x0A, 1)),
        group: 52,
        heading: Some((0, 0x40, 0xA4)),
        lines: &[(1, 0x60, 0xD0), (2, 0x60, 0xF0), (3, 0x48, 0x108)],
        ok: Some((0x188, 0x100, 0)),
        overlay: true,
        unfinished: "the two values, and the F5 note that only shows in windowed mode",
    },
];

/// The shell for a screen id, if there is one.
///
/// **A screen leaving this table is the measure of progress.** `0x35` and
/// `0x36` — load and save — were here until there was a save format behind
/// them; `find` answering `None` for a screen id is now the check that the
/// implemented screen and the shell for it cannot both exist.
pub fn find(id: u8) -> Option<&'static Shell> {
    SHELLS.iter().find(|s| s.id == id)
}

/// `System.pl8` frames for `Ui_OkButton`'s two modes.
const OK_FRAME: [usize; 2] = [l2_view::chrome::system::OK, l2_view::chrome::system::OK_ALT];

pub struct ShellScreen {
    spec: &'static Shell,
}

impl ShellScreen {
    /// `id` is the `g_screenId` byte. An id with no shell falls back to the
    /// first one rather than panicking, because `ScreenId` is a plain value and
    /// nothing stops a caller naming a screen that does not exist.
    pub fn new(id: u8) -> ShellScreen {
        ShellScreen { spec: find(id).unwrap_or(&SHELLS[0]) }
    }

    pub fn spec(&self) -> &'static Shell {
        self.spec
    }
}

impl Screen for ShellScreen {
    fn id(&self) -> ScreenId {
        ScreenId::Shell(self.spec.id)
    }

    fn title(&self, _ctx: &Ctx) -> String {
        format!("{} — screen 0x{:02X} (shell)", self.spec.name, self.spec.id)
    }

    fn palette(&self) -> Option<&'static str> {
        self.spec.palette
    }

    fn is_overlay(&self) -> bool {
        self.spec.overlay
    }

    fn handle(&mut self, event: Event, _ctx: &mut Ctx) -> Transition {
        match event {
            // Every one of these closes and nothing else. That is the whole
            // truth about a shell, and pretending otherwise would be the
            // invented interface again.
            //
            // The right one is the original's: `Screen_FrameInput` has a
            // right-release arm for almost every screen id in this table, and
            // `L2.eng` group 12 index 0 — *"Click Right to Exit"*, printed on
            // the value spinner — is the game saying so in English.
            Event::KeyDown(Key::Escape)
            | Event::KeyDown(Key::Enter)
            | Event::KeyDown(Key::Space)
            | Event::RightClick { .. }
            | Event::Click { .. } => Transition::Pop,
            _ => Transition::Stay,
        }
    }

    fn draw(&mut self, ctx: &Ctx, canvas: &mut Canvas) {
        let a = &ctx.assets.shell;
        let pen = Pen {
            assets: a,
            ink: &ctx.assets.ink,
            chrome: ctx.assets.chrome.as_ref(),
            // The management painters set neither text flag, so this is the
            // ordinary emboss with no drop capitals.
            shadow: Some(font::SHADOW),
            caps: None,
        };
        let s = self.spec;

        if let Some(bg) = s.background {
            if !shell::background(canvas, a, bg) {
                canvas.clear(ctx.assets.ink.background);
            }
        } else if !s.overlay {
            canvas.clear(ctx.assets.ink.background);
        }

        if let Some((x, y, cols, rows, set)) = s.window {
            pen.window(canvas, x, y, cols, rows, set);
        }
        if let Some((i, x, y)) = s.heading {
            let t = a.text(s.group, i).to_string();
            pen.heading(canvas, x, y, &t, font::TEXT);
        }
        for &(i, x, y) in s.lines {
            let t = a.text(s.group, i).to_string();
            pen.body(canvas, x, y, &t, font::TEXT);
        }
        if let Some((x, y, mode)) = s.ok {
            let drawn = ctx
                .assets
                .chrome
                .as_ref()
                .is_some_and(|c| c.draw_system(canvas, OK_FRAME[mode], x, y));
            if !drawn {
                shell::button_recess(canvas, x, y, 24, 24);
            }
        }

        // The mark. A shell says so, in our own font, in the dim colour, at the
        // bottom of the screen — never in the original's font, so that nobody
        // can mistake it for something the game said.
        let note = format!("SHELL 0x{:02X} - NOT WIRED: {}", s.id, s.unfinished.to_uppercase());
        l2_view::text::draw(canvas, 4, 470, &note, ctx.assets.ink.dim);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_shell_has_a_distinct_screen_id_and_a_painter() {
        let mut ids: Vec<u8> = SHELLS.iter().map(|s| s.id).collect();
        let before = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), before, "two shells claim the same g_screenId");
        for s in SHELLS {
            assert!(s.painter >= 0x0040_0000, "{} has no painter address", s.name);
            assert!(!s.unfinished.is_empty(), "{} does not say what it is missing", s.name);
        }
    }

    #[test]
    fn a_background_and_its_palette_come_as_a_pair() {
        for s in SHELLS {
            assert_eq!(
                s.background.is_some(),
                s.palette.is_some(),
                "{}: a full-screen background is always read with its own .256",
                s.name
            );
            if let (Some(b), Some(p)) = (s.background, s.palette) {
                assert_eq!(b.split('.').next(), p.split('.').next(), "{}", s.name);
            }
        }
    }

    #[test]
    fn a_screen_with_its_own_background_is_a_page_and_not_a_popup() {
        for s in SHELLS {
            if s.background.is_some() {
                assert!(!s.overlay, "{} replaces the screen, so it cannot be an overlay", s.name);
            }
        }
    }

    #[test]
    fn every_window_and_button_fits_on_a_640_by_480_screen() {
        for s in SHELLS {
            if let Some((x, y, cols, rows, _)) = s.window {
                assert!(x >= 0 && y >= 0, "{}", s.name);
                assert!(x + cols * 16 <= 640, "{} is {} wide", s.name, x + cols * 16);
                assert!(y + rows * 16 <= 480, "{} is {} tall", s.name, y + rows * 16);
            }
            if let Some((x, y, mode)) = s.ok {
                assert!(mode < 2, "{}", s.name);
                assert!(x + 24 <= 640 && y + 24 <= 480, "{}'s corner picture is off screen", s.name);
            }
        }
    }

    #[test]
    fn find_answers_for_the_ids_in_the_table_and_nothing_else() {
        assert_eq!(find(0x09).unwrap().name, "The court");
        assert_eq!(find(0x2E).unwrap().group, 37);
        assert!(find(0x00).is_none(), "the campaign map is implemented, not shelled");
        assert!(find(0x02).is_none(), "the village is somebody else's");
        assert!(find(0x14).is_none(), "the four county panels are implemented");
        assert!(find(0x35).is_none(), "loading a game is implemented");
        assert!(find(0x36).is_none(), "saving a game is implemented");
        assert!(find(0x11).is_none(), "army division is implemented");
        assert!(
            find(0x17).is_none(),
            "raising an army is implemented - and it is not a mercenaries screen"
        );
        assert!(find(0x1D).is_none(), "siege preparation is implemented");
        assert!(find(0x08).is_none(), "the merchant trades");
        assert!(find(0x0C).is_none(), "the trade panel is the merchant's other half");
        assert!(find(0x0A).is_none(), "the armoury equips a levy - and raises the army");
        assert!(find(0x0D).is_none(), "a weapon's rack was never in this table at all");
    }
}

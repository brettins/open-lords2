//! The county panel: what a county has, and the two orders the slice lets the
//! player give it — the tax rate and the ration level.
//!
//! Everything shown is read straight off `l2_kingdom::County`, which is the
//! same record the season pipeline reads, so nothing here can drift from what
//! the rules will do next turn. Where a value has a "last season" twin the
//! panel shows the movement, because a turn that changes nothing visible is a
//! turn the player has no reason to believe happened.
//!
//! Two orders, and no more. Labour allocation, field assignment, buying and
//! selling, raising men and building a castle are all things this county
//! record can express and this screen refuses, for the reason `docs/plan.md`
//! gives: the slice is a hard boundary.

use l2_kingdom::tables::{HEALTH_BAND_NAMES, RATION_LEVEL_COUNT, RATION_NAMES};
use l2_view::{text, Canvas};

use crate::game::MAX_TAX_RATE;
use crate::input::{Event, Key, Rect};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::widget;

/// The two things that can be changed from here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Row {
    Tax,
    Ration,
}

const ROWS: [Row; 2] = [Row::Tax, Row::Ration];

const PANEL: Rect = Rect::new(16, 16, 608, 448);
const CONTROL_Y: i32 = 330;
const CONTROL_GAP: i32 = 34;

pub struct CountyScreen {
    county: u8,
    row: usize,
}

impl CountyScreen {
    pub fn new(county: u8) -> CountyScreen {
        CountyScreen { county, row: 0 }
    }

    pub fn county(&self) -> u8 {
        self.county
    }

    pub fn focused_row(&self) -> Row {
        ROWS[self.row]
    }

    fn row_y(index: usize) -> i32 {
        CONTROL_Y + index as i32 * CONTROL_GAP
    }

    pub fn less_button(index: usize) -> Rect {
        Rect::new(220, CountyScreen::row_y(index), 24, 18)
    }

    pub fn more_button(index: usize) -> Rect {
        Rect::new(360, CountyScreen::row_y(index), 24, 18)
    }

    pub fn back_button() -> Rect {
        Rect::new(478, 424, 130, 22)
    }

    /// Change the focused row's value by `step`. Refused, and silently, for a
    /// county the player does not hold — the panel says so on its face.
    fn adjust(&self, ctx: &mut Ctx, row: Row, step: i32) {
        let id = self.county;
        let Some(c) = ctx.game.kingdom.counties.get(id as usize) else { return };
        match row {
            Row::Tax => {
                let next = (c.tax_rate + step).clamp(0, MAX_TAX_RATE);
                ctx.game.set_tax_rate(id, next);
            }
            Row::Ration => {
                let next = (c.ration_wanted + step).clamp(0, RATION_LEVEL_COUNT as i32 - 1);
                ctx.game.set_ration(id, next);
            }
        }
    }
}

impl Screen for CountyScreen {
    fn id(&self) -> ScreenId {
        ScreenId::County(self.county)
    }

    fn title(&self, _ctx: &Ctx) -> String {
        format!("County {}", self.county)
    }

    fn handle(&mut self, event: Event, ctx: &mut Ctx) -> Transition {
        match event {
            Event::KeyDown(Key::Escape) | Event::KeyDown(Key::Enter) => return Transition::Pop,
            Event::KeyDown(Key::Up) => self.row = (self.row + ROWS.len() - 1) % ROWS.len(),
            Event::KeyDown(Key::Down) => self.row = (self.row + 1) % ROWS.len(),
            Event::KeyDown(Key::Left) => self.adjust(ctx, self.focused_row(), -1),
            Event::KeyDown(Key::Right) => self.adjust(ctx, self.focused_row(), 1),
            Event::Click { x, y } => {
                if CountyScreen::back_button().contains(x, y) {
                    return Transition::Pop;
                }
                for (i, row) in ROWS.iter().enumerate() {
                    if CountyScreen::less_button(i).contains(x, y) {
                        self.row = i;
                        self.adjust(ctx, *row, -1);
                    } else if CountyScreen::more_button(i).contains(x, y) {
                        self.row = i;
                        self.adjust(ctx, *row, 1);
                    }
                }
            }
            _ => {}
        }
        Transition::Stay
    }

    fn draw(&mut self, ctx: &Ctx, canvas: &mut Canvas) {
        let ink = &ctx.assets.ink;
        canvas.clear(ink.background);
        widget::panel(canvas, ink, PANEL);

        let Some(c) = ctx.game.kingdom.counties.get(self.county as usize) else { return };
        let mine = ctx.game.is_players(self.county);

        let owner = if c.owner == 0 {
            "UNCLAIMED".to_string()
        } else if mine {
            "YOUR COUNTY".to_string()
        } else {
            format!("REALM {}", c.owner)
        };
        text::draw(canvas, 32, 32, &format!("COUNTY {}", self.county), ink.highlight);
        text::draw(canvas, 160, 32, &owner, ink.text);
        text::draw_right(
            canvas,
            608,
            32,
            &format!(
                "{} {}",
                super::map::season_name(ctx.game.kingdom.season),
                ctx.game.kingdom.year
            ),
            ink.dim,
        );
        canvas.fill_rect(32, 46, 576, 1, ink.border);

        // Left column: the people.
        let (lx, lr) = (32, 300);
        let mut y = 62;
        let step = 18;
        widget::stat_delta(
            canvas,
            ink,
            lx,
            y,
            lr,
            "POPULATION",
            &c.population.to_string(),
            c.population - c.pop_last,
        );
        y += step;
        widget::stat(canvas, ink, lx, y, lr, "  LAST SEASON", &c.pop_last.to_string());
        y += step;
        widget::stat(canvas, ink, lx, y, lr, "  BIRTHS", &c.births.to_string());
        y += step;
        widget::stat(canvas, ink, lx, y, lr, "  DEATHS", &c.deaths.to_string());
        y += step;
        widget::stat_delta(
            canvas,
            ink,
            lx,
            y,
            lr,
            "HAPPINESS",
            &c.happiness.to_string(),
            c.happiness - c.happiness_last,
        );
        y += step;
        widget::stat(canvas, ink, lx, y, lr, "  FROM TAX", &widget::signed(c.shown_tax));
        y += step;
        widget::stat(canvas, ink, lx, y, lr, "  FROM RATIONS", &widget::signed(c.shown_ration));
        y += step;
        widget::stat(canvas, ink, lx, y, lr, "  FROM HEALTH", &widget::signed(c.shown_health));
        y += step;
        widget::stat(canvas, ink, lx, y, lr, "  FROM EVENTS", &widget::signed(c.shown_events));
        y += step;
        let band = HEALTH_BAND_NAMES
            .get(c.health_band as usize)
            .copied()
            .unwrap_or("?");
        widget::stat(canvas, ink, lx, y, lr, "HEALTH", &format!("{} ({band})", c.health_meter));
        y += step;
        widget::stat(canvas, ink, lx, y, lr, "UNREST", &c.unrest.to_string());

        // Right column: the land.
        let (rx, rr) = (330, 592);
        let mut y = 62;
        widget::stat(canvas, ink, rx, y, rr, "GRAIN IN STORE", &c.grain.to_string());
        y += step;
        widget::stat(canvas, ink, rx, y, rr, "  EATEN LAST SEASON", &c.grain_eaten.to_string());
        y += step;
        widget::stat(canvas, ink, rx, y, rr, "HERD", &c.herd.to_string());
        y += step;
        widget::stat(canvas, ink, rx, y, rr, "  SLAUGHTERED", &c.herd_eaten.to_string());
        y += step;
        widget::stat(canvas, ink, rx, y, rr, "FIELDS - GRAIN", &c.fields_grain.to_string());
        y += step;
        widget::stat(canvas, ink, rx, y, rr, "FIELDS - CATTLE", &c.fields_cattle.to_string());
        y += step;
        widget::stat(canvas, ink, rx, y, rr, "FIELDS - FALLOW", &c.fields_fallow.to_string());
        y += step;
        widget::stat(canvas, ink, rx, y, rr, "FERTILITY", &c.fertility.to_string());
        y += step;
        widget::stat(canvas, ink, rx, y, rr, "WEATHER", c.weather.name());
        y += step;
        widget::stat(canvas, ink, rx, y, rr, "CASTLE TYPE", &c.castle_type.to_string());
        y += step;
        widget::stat(canvas, ink, rx, y, rr, "TAX COLLECTED", &c.tax_collected.to_string());

        // The two orders.
        canvas.fill_rect(32, CONTROL_Y - 18, 576, 1, ink.border);
        for (i, row) in ROWS.iter().enumerate() {
            let y = CountyScreen::row_y(i);
            let focused = i == self.row;
            let (label, value) = match row {
                Row::Tax => ("TAX RATE", format!("{}%", c.tax_rate)),
                Row::Ration => (
                    "RATIONS",
                    RATION_NAMES
                        .get(c.ration_wanted.clamp(0, RATION_LEVEL_COUNT as i32 - 1) as usize)
                        .copied()
                        .unwrap_or("?")
                        .to_string(),
                ),
            };
            let colour = if focused { ink.highlight } else { ink.text };
            text::draw(canvas, 32, y + 6, label, colour);
            widget::button(canvas, ink, CountyScreen::less_button(i), "-", focused && mine);
            text::draw_centred(canvas, 302, y + 6, &value, colour);
            widget::button(canvas, ink, CountyScreen::more_button(i), "+", focused && mine);
        }
        if let Row::Ration = self.focused_row() {
            text::draw(
                canvas,
                32,
                CountyScreen::row_y(1) + 22,
                &format!("ACHIEVED LAST SEASON: {}", c.ration_achieved),
                ink.dim,
            );
        }

        let hint = if mine {
            "UP DOWN CHOOSE A ROW, LEFT RIGHT CHANGE IT"
        } else {
            "THIS COUNTY IS NOT YOURS - NOTHING HERE CAN BE SET"
        };
        text::draw(canvas, 32, 430, hint, if mine { ink.dim } else { ink.bad });
        widget::button(canvas, ink, CountyScreen::back_button(), "BACK TO MAP", false);
    }
}

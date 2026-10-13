//! The join page (C255): six symbols of `l2_relay::ALPHABET` (C254) or a
//! `host:port` (C253), and who you are, set before the join because
//! `l2_net::Lobby` takes a name and shield only in `join`/`with_shield` (C255).

use l2_view::Canvas;

use super::draw::{self, Heading, Look, Pens, BUTTON_H, BUTTON_W, LEFT, SHIELD_H, SHIELD_W, WIDTH};
use super::Item;
use crate::input::{Event, Rect};
use crate::net::{Identity, Link, Phase, Request, Setup};
use crate::game::Prefs;
use crate::screen::Ctx;
use crate::shell::font;
use crate::text::{self, FontMetrics, Kind, Metrics, TextField};

const HEADING: &str = "Join a game";
const CAPTION_Y: i32 = 0x70;
const ADDRESS_CAPTION: &str = "Type the host's address as 192.168.1.20:4712.";
const CELLS: i32 = l2_relay::CODE_LEN as i32;
const CELL_W: i32 = 44;
const CELL_H: i32 = 40;
const CELL_Y: i32 = 0x84;
const ADDRESS_W: i32 = 0x100;
const ADDRESS_Y: i32 = 0x8C;
const LINK_Y: i32 = 0xB4;
const LINK_W: i32 = 0xC8;
const STATUS_Y: i32 = 0xCC;
const WHO_Y: i32 = 0xF4;
const NAME_X: i32 = LEFT + 0x50;
const SHIELD_X: i32 = 0x1C8;
const SHIELD_Y: i32 = 0xE8;
const BUTTON_Y: i32 = 0x138;
const CONNECTING: &str = "Connecting...";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Focus {
    Target,
    Name,
}

pub(super) struct JoinPage {
    code: TextField,
    address: TextField,
    by_address: bool,
    name: TextField,
    shield: u8,
    focus: Focus,
}

impl JoinPage {
    pub(super) fn new(name: &str) -> JoinPage {
        JoinPage {
            code: TextField::begin("", l2_relay::CODE_LEN, CELLS * CELL_W, Kind::Address),
            address: TextField::begin(&Link::default_address(), 40, ADDRESS_W - 12, Kind::Address),
            by_address: false,
            // `Edit_Begin` (`0x00402009`) as `FUN_00432B05` opens page 4's name field: 0x10 chars, 0xC0 px.
            name: TextField::begin(name, text::NAME_MAX_TYPED, text::NAME_MAX_PIXELS, Kind::Text),
            shield: 1,
            focus: Focus::Target,
        }
    }

    /// C253's `--join`, already on the link: its code in the box, or its address in the field.
    pub(super) fn seed(&mut self, target: &str) {
        match l2_relay::as_code(target) {
            Some(code) => self.code = TextField::begin(&code, l2_relay::CODE_LEN, CELLS * CELL_W, Kind::Address),
            None => {
                self.by_address = true;
                self.address = TextField::begin(target, 40, ADDRESS_W - 12, Kind::Address);
            }
        }
    }

    /// C253's `--name`/`--shield` (`launch::open_lobby`).
    pub(super) fn set_identity(&mut self, name: &str, shield: u8) {
        self.name = TextField::begin(name, text::NAME_MAX_TYPED, text::NAME_MAX_PIXELS, Kind::Text);
        self.shield = shield.clamp(1, l2_net::lobby::SHIELDS);
    }

    pub(super) fn code(&self) -> String {
        self.code.text()
    }

    pub(super) fn by_address(&self) -> bool {
        self.by_address
    }

    pub(super) fn shield(&self) -> u8 {
        self.shield
    }

    pub(super) fn name(&self) -> String {
        self.name.commit(text::PLAYER_NAME_LEN)
    }

    pub(super) fn toggle(&mut self) {
        self.by_address = !self.by_address;
        self.focus = Focus::Target;
    }

    pub(super) fn tick(&mut self) {
        self.code.tick();
        self.address.tick();
        self.name.tick();
    }

    pub(super) fn caret_lit(&self) -> bool {
        match self.focus {
            Focus::Name => self.name.caret_lit(),
            Focus::Target if self.by_address => self.address.caret_lit(),
            Focus::Target => self.code.caret_lit(),
        }
    }

    /// The code box takes C254's alphabet only, upper-cased as `l2_relay::as_code` would.
    pub(super) fn type_event(&mut self, event: Event, m: &dyn Metrics) -> bool {
        match self.focus {
            Focus::Name => self.name.event(event, m),
            Focus::Target if self.by_address => self.address.event(event, m),
            Focus::Target => match event {
                Event::Text(c) => {
                    let u = c.to_ascii_uppercase();
                    l2_relay::ALPHABET.contains(&(u as u8)) && self.code.type_char(u, m)
                }
                e => self.code.event(e, m),
            },
        }
    }

    pub(super) fn click(&mut self, x: i32, y: i32) -> bool {
        if self.target_rect().contains(x, y) {
            self.focus = Focus::Target;
        } else if name_rect().contains(x, y) {
            self.focus = Focus::Name;
        } else if shield_rect().contains(x, y) {
            self.shield = self.shield % l2_net::lobby::SHIELDS + 1;
        } else {
            return false;
        }
        true
    }

    pub(super) fn request(&self, prefs: &mut Prefs) -> Option<Request> {
        let target = if self.by_address { self.address.text().trim().to_string() } else { self.code() };
        if target.is_empty() {
            return None;
        }
        let mut name = self.name();
        if name.trim().is_empty() {
            name = text::DEFAULT_PLAYER_NAME.to_string();
        }
        prefs.kept.player_name = text::PlayerName::new(&name);
        Some(Request::Join { address: target })
    }

    pub(super) fn identity(&self, prefs: &Prefs) -> Identity {
        let mut name = self.name();
        if name.trim().is_empty() {
            name = text::DEFAULT_PLAYER_NAME.to_string();
        }
        Identity { name, shield: self.shield, setup: Setup { map: 0, options: prefs.kept.setup } }
    }

    pub(super) fn items(&self, link: &Link) -> Vec<Item> {
        if matches!(link.phase, Phase::Connecting(_)) {
            return vec![Item::Back];
        }
        let swap = if self.by_address { Item::UseCode } else { Item::UseAddress };
        vec![Item::Connect, swap, Item::Back]
    }

    pub(super) fn rects(&self, link: &Link) -> Vec<Rect> {
        let back = Rect::new(LEFT + WIDTH - BUTTON_W, BUTTON_Y, BUTTON_W, BUTTON_H);
        if matches!(link.phase, Phase::Connecting(_)) {
            return vec![back];
        }
        let join = Rect::new(LEFT, BUTTON_Y, BUTTON_W, BUTTON_H);
        let link_rect = Rect::new(LEFT + (WIDTH - LINK_W) / 2, LINK_Y - 2, LINK_W, 16);
        vec![join, link_rect, back]
    }

    fn target_rect(&self) -> Rect {
        if self.by_address {
            Rect::new(LEFT + (WIDTH - ADDRESS_W) / 2, ADDRESS_Y, ADDRESS_W, BUTTON_H)
        } else {
            Rect::new(LEFT + (WIDTH - CELLS * CELL_W) / 2, CELL_Y, CELLS * CELL_W, CELL_H)
        }
    }

    pub(super) fn draw(&self, ctx: &Ctx, canvas: &mut Canvas, pens: &Pens, selected: usize) {
        let link = &ctx.game.net;
        draw::backdrop(ctx, canvas, pens, Heading::Text(HEADING));
        if self.by_address {
            pens.body.body_centred(canvas, LEFT, CAPTION_Y, WIDTH, ADDRESS_CAPTION, font::TEXT);
        }
        let focused = self.focus == Focus::Target;
        if self.by_address {
            draw::plate(canvas, &pens.body, self.target_rect(), &self.address, focused);
        } else {
            self.draw_cells(canvas, pens, focused);
        }
        let status = match &link.phase {
            Phase::Connecting(_) => Some(CONNECTING.to_string()),
            Phase::Failed(why) => Some(why.clone()),
            _ => None,
        };
        if let Some(s) = status {
            pens.body.body_wrapped(canvas, LEFT, STATUS_Y, WIDTH, &s, font::TEXT);
        }
        pens.body.body(canvas, LEFT, WHO_Y + draw::ITEM_TEXT, "Playing as", font::TEXT);
        draw::plate(canvas, &pens.body, name_rect(), &self.name, self.focus == Focus::Name);
        if !draw::shield(canvas, &pens.body, self.shield, SHIELD_X, SHIELD_Y) {
            draw::plaque(canvas, &pens.body, self.shield, SHIELD_X, WHO_Y + 4);
        }
        for (i, (item, r)) in self.items(link).iter().zip(self.rects(link)).enumerate() {
            let look = if i == selected { Look::Selected } else { Look::Plain };
            match item {
                Item::UseAddress | Item::UseCode => self.draw_link(canvas, pens, r, item.label(), look),
                _ => draw::button(canvas, &pens.body, r, item.label(), look),
            }
        }
    }

    fn draw_cells(&self, canvas: &mut Canvas, pens: &Pens, focused: bool) {
        let r = self.target_rect();
        let typed: Vec<char> = self.code.text().chars().collect();
        for i in 0..CELLS {
            let x = r.x + i * CELL_W;
            crate::shell::button_recess(canvas, x, r.y, CELL_W, CELL_H);
            if let Some(c) = typed.get(i as usize) {
                draw::symbol(canvas, pens, Rect::new(x, r.y, CELL_W, CELL_H), *c, font::TEXT);
            }
            if focused && self.code.caret_lit() && self.code.caret() as i32 == i {
                canvas.fill_rect(x + 8, r.y + CELL_H - 8, CELL_W - 16, 2, font::TEXT);
            }
        }
    }

    fn draw_link(&self, canvas: &mut Canvas, pens: &Pens, r: Rect, label: &str, look: Look) {
        let w = FontMetrics::of(pens.body.assets).width(&label.chars().collect::<Vec<_>>());
        let x = r.x + (r.w - w) / 2;
        pens.body.body(canvas, x, r.y + 2, label, look.ink());
        canvas.fill_rect(x, r.y + 14, w, 1, look.ink());
    }
}

fn name_rect() -> Rect {
    Rect::new(NAME_X, WHO_Y, BUTTON_W, BUTTON_H)
}

fn shield_rect() -> Rect {
    Rect::new(SHIELD_X, SHIELD_Y, SHIELD_W, SHIELD_H)
}

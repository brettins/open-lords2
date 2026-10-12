//! The network lobby, three pages: entry, join, room. `[D]` ours: the original
//! handed this to DirectPlay's dialogs (`docs/netcode.md` Appendix A); page 7's
//! backdrop and pens are reused. No `L2.eng` group covers it (rule 6, C255).

mod draw;
mod entry;
mod item;
mod join;
mod room;

use l2_view::Canvas;

use crate::game::Game;
use crate::input::{Event, Key, Rect};
use crate::net::{Link, Phase, Request};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::screens::setup::SetupPage;
use crate::text::FontMetrics;

pub use item::{Item, Page};
pub use room::{blocker, message, next_holder, reach_text, realm_at, waiting_for, CODE_CAPTION};

pub struct LobbyScreen {
    page: Page,
    selected: usize,
    settled: bool,
    host: bool,
    join: join::JoinPage,
    lit: bool,
    redraw: bool,
    /// C263: *Continue a saved game*'s file box, open over the entry page.
    pick: Option<crate::screens::saveload::SaveLoadScreen>,
    /// Where the room's roster rows began when last drawn.
    roster_top: i32,
}

impl LobbyScreen {
    pub fn new() -> LobbyScreen {
        LobbyScreen {
            page: Page::Entry,
            selected: 0,
            settled: false,
            host: false,
            join: join::JoinPage::new(crate::text::DEFAULT_PLAYER_NAME),
            lit: false,
            redraw: false,
            pick: None,
            roster_top: 0,
        }
    }

    pub fn page(&self) -> Page {
        self.page
    }

    pub fn message(link: &Link) -> Vec<String> {
        room::message(link)
    }

    pub fn code(&self) -> String {
        self.join.code()
    }

    pub fn by_address(&self) -> bool {
        self.join.by_address()
    }

    pub fn shield(&self) -> u8 {
        self.join.shield()
    }

    pub fn name(&self) -> String {
        self.join.name()
    }

    /// C255: drawing and Up/Down order; the last is Back or Leave, which Escape presses.
    pub fn items(&self, link: &Link) -> Vec<Item> {
        match self.page {
            Page::Entry => entry::ITEMS.to_vec(),
            Page::Join => self.join.items(link),
            Page::Room => room::items(link, self.host),
        }
    }

    pub fn enabled(&self, item: Item, link: &Link) -> bool {
        item != Item::Start || blocker(link).is_none()
    }

    pub fn picking(&self) -> bool {
        self.pick.is_some()
    }

    /// The room's roster rows begin here.
    pub fn roster_top(&self) -> i32 {
        self.roster_top
    }

    /// C263: the picker closed; with a save chosen, host it.
    fn picked(&mut self, t: Transition, ctx: &mut Ctx) -> Transition {
        if t == Transition::Stay {
            return Transition::Stay;
        }
        self.pick = None;
        self.redraw = true;
        let Some(resume) = ctx.game.net.resume.as_deref() else { return Transition::Stay };
        let prefs = &ctx.game.prefs;
        let shield = resume.mine(prefs.install).map_or(1, |r| r.shield);
        let name = prefs.kept.player_name.as_str().to_string();
        let name = if name.trim().is_empty() { crate::text::DEFAULT_PLAYER_NAME.to_string() } else { name };
        let setup = crate::net::Setup { map: 0, options: prefs.kept.setup };
        ctx.game.net.identity = Some(crate::net::Identity { name, shield, setup });
        let addr = Request::every_interface(crate::net::DEFAULT_PORT);
        ctx.game.net.requests.push(Request::Host { addr });
        self.host = true;
        self.turn(Page::Room)
    }

    pub fn rects(&self, link: &Link) -> Vec<Rect> {
        match self.page {
            Page::Entry => entry::rects(),
            Page::Join => self.join.rects(link),
            Page::Room => room::rects(link, self.host),
        }
    }

    pub fn hit(&self, link: &Link, x: i32, y: i32) -> Option<usize> {
        self.rects(link).iter().position(|r| r.contains(x, y))
    }

    /// C253's shell flags arrive as requests: a `Request::Host` or a hosting lobby
    /// is the room, a `Request::Join` or a connect in flight is the join page (C255).
    fn settle(&mut self, game: &Game) {
        if core::mem::replace(&mut self.settled, true) {
            return;
        }
        let link = &game.net;
        self.join = join::JoinPage::new(&game.prefs.kept.player_name.as_str());
        let pending_host = link.requests.iter().any(|r| matches!(r, Request::Host { .. }));
        let pending_join = link.requests.iter().find_map(|r| match r {
            Request::Join { address } => Some(address.clone()),
            _ => None,
        });
        if pending_host || (link.phase == Phase::Lobby && link.hosting) {
            self.host = true;
            self.page = Page::Room;
        } else if let Some(address) = pending_join {
            self.page = Page::Join;
            self.join.seed(&address);
            if let Some(id) = &link.identity {
                self.join.set_identity(&id.name, id.shield);
            }
        } else if link.phase != Phase::Idle {
            self.page = Page::Join;
        }
    }

    pub fn press(&mut self, item: Item, ctx: &mut Ctx) -> Transition {
        let link = &mut ctx.game.net;
        match item {
            Item::Host => Transition::Replace(ScreenId::Setup(SetupPage::Shield)),
            Item::Join => self.turn(Page::Join),
            Item::Resume => {
                ctx.game.net.resume = None;
                self.pick = Some(crate::screens::saveload::SaveLoadScreen::for_resume());
                Transition::Stay
            }
            Item::Connect => {
                if let Some(request) = self.join.request(&mut ctx.game.prefs) {
                    ctx.game.net.identity = Some(self.join.identity(&ctx.game.prefs));
                    ctx.game.net.requests.push(request);
                }
                Transition::Stay
            }
            Item::UseAddress | Item::UseCode => {
                self.join.toggle();
                self.selected = 0;
                Transition::Stay
            }
            Item::Back if self.page == Page::Entry => Transition::Pop,
            Item::Back => {
                if link.phase != Phase::Idle {
                    link.requests.push(Request::Leave);
                }
                self.turn(Page::Entry)
            }
            Item::Start => {
                if blocker(link).is_none() {
                    link.requests.push(Request::Start);
                }
                Transition::Stay
            }
            Item::Ready => {
                link.requests.push(Request::Ready(true));
                Transition::Stay
            }
            Item::NotReady => {
                link.requests.push(Request::Ready(false));
                Transition::Stay
            }
            Item::Leave => {
                link.requests.push(Request::Leave);
                Transition::Pop
            }
        }
    }

    fn turn(&mut self, page: Page) -> Transition {
        self.page = page;
        self.selected = 0;
        Transition::Stay
    }
}

impl Default for LobbyScreen {
    fn default() -> LobbyScreen {
        LobbyScreen::new()
    }
}

impl Screen for LobbyScreen {
    fn id(&self) -> ScreenId {
        ScreenId::Lobby
    }

    fn title(&self, _ctx: &Ctx) -> String {
        match self.page {
            Page::Entry => "Multiple players",
            Page::Join => "Join a game",
            Page::Room => "Lobby",
        }
        .into()
    }

    fn palette(&self) -> Option<&'static str> {
        Some(SetupPage::Custom.palette())
    }

    fn take_redraw(&mut self) -> bool {
        core::mem::take(&mut self.redraw)
    }

    fn update(&mut self, ctx: &mut Ctx) -> Transition {
        self.settle(ctx.game);
        if let Some(pick) = self.pick.as_mut() {
            let t = pick.update(ctx);
            self.redraw |= pick.take_redraw();
            return self.picked(t, ctx);
        }
        self.join.tick();
        let link = &ctx.game.net;
        match &link.phase {
            Phase::Started => return Transition::Replace(ScreenId::Campaign),
            Phase::Lobby => {
                self.host = link.hosting;
                if self.page != Page::Room {
                    self.turn(Page::Room);
                }
            }
            Phase::Failed(_) if self.page == Page::Room && !self.host => {
                self.turn(Page::Join);
            }
            _ => {}
        }
        if self.page == Page::Join {
            let lit = self.join.caret_lit();
            self.redraw |= lit != self.lit;
            self.lit = lit;
        }
        Transition::Stay
    }

    fn handle(&mut self, event: Event, ctx: &mut Ctx) -> Transition {
        self.settle(ctx.game);
        if let Some(pick) = self.pick.as_mut() {
            self.redraw = true;
            let t = match event {
                Event::KeyDown(Key::Escape) | Event::RightClick { .. } => Transition::Pop,
                e => pick.handle(e, ctx),
            };
            return self.picked(t, ctx);
        }
        let items = self.items(&ctx.game.net);
        let n = items.len().max(1);
        match event {
            Event::KeyDown(Key::Escape) | Event::RightClick { .. } => {
                items.last().map_or(Transition::Stay, |&last| self.press(last, ctx))
            }
            Event::KeyDown(Key::Up) => {
                self.selected = (self.selected + n - 1) % n;
                Transition::Stay
            }
            Event::KeyDown(Key::Down) => {
                self.selected = (self.selected + 1) % n;
                Transition::Stay
            }
            Event::KeyDown(Key::Enter) => match items.get(self.selected) {
                Some(&item) => self.press(item, ctx),
                None => Transition::Stay,
            },
            Event::Click { x, y } => match self.hit(&ctx.game.net, x, y) {
                Some(i) => {
                    self.selected = i;
                    self.press(items[i], ctx)
                }
                None => {
                    if self.page == Page::Join {
                        self.join.click(x, y);
                    }
                    let link = &ctx.game.net;
                    let realm = realm_at(link, self.roster_top, x, y).filter(|_| self.page == Page::Room && self.host);
                    if let Some(request) = realm.and_then(|r| next_holder(link, r)) {
                        ctx.game.net.requests.push(request);
                    }
                    Transition::Stay
                }
            },
            e if self.page == Page::Join => {
                self.join.type_event(e, &FontMetrics::of(&ctx.assets.shell));
                Transition::Stay
            }
            _ => Transition::Stay,
        }
    }

    fn draw(&mut self, ctx: &Ctx, canvas: &mut Canvas) {
        self.settle(ctx.game);
        let pens = draw::pens(ctx);
        match self.page {
            Page::Entry => entry::draw(ctx, canvas, &pens, self.selected),
            Page::Join => self.join.draw(ctx, canvas, &pens, self.selected),
            Page::Room => self.roster_top = room::draw(ctx, canvas, &pens, self.host, self.selected),
        }
        if let Some(pick) = self.pick.as_mut() {
            pick.draw(ctx, canvas);
        }
    }
}

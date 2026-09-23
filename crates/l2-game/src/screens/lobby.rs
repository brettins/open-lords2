//! The network lobby: host or join, the roster, ready, Start. `[D]` ours: the
//! original handed this to DirectPlay's own dialogs (`docs/netcode.md`
//! Appendix A); only page 7's backdrop and pens are reused. L2.eng has no
//! words for it (`docs/formats/eng.md` section 5), so these strings are ours.

use l2_view::Canvas;

use crate::input::{Event, Key, Rect};
use crate::net::{Link, Phase, Request, DEFAULT_PORT};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::screens::setup::SetupPage;
use crate::shell::{self, font, Pen};
use crate::text::{FontMetrics, Kind, TextField};

/// `L2.eng` group 11 index 3, *"Multiple players"*, page 1's own item.
const HEADING: (usize, usize) = (11, 3);
const BOX: (i32, i32, i32, i32) = (0x60, 0x40, 0x1C, 0x14);
const LEFT: i32 = 0x80;
const ROW: i32 = 18;
const ITEM_W: i32 = 0x100;
const ADDRESS_Y: i32 = 0x70;
const ITEMS_Y: i32 = 0x90;
const ROSTER_Y: i32 = 0x70;
const MESSAGE_Y: i32 = 0x130;
const MESSAGE_COLS: usize = 44;
const LINE: i32 = 14;
const ADDRESS_LEN: usize = 40;
const ADDRESS_PX: i32 = 0x100;
/// DAT_00553114's pick, as the setup pages paint it: 0xF9 over 0x3F.
const SELECTED: u8 = 0xF9;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Item {
    Host,
    Join,
    Ready,
    Start,
    Leave,
}

pub struct LobbyScreen {
    address: TextField,
    selected: usize,
}

impl LobbyScreen {
    pub fn new() -> LobbyScreen {
        let address = TextField::begin(&Link::default_address(), ADDRESS_LEN, ADDRESS_PX, Kind::Text);
        LobbyScreen { address, selected: 0 }
    }

    /// What this seat can press, in drawing order.
    pub fn items(link: &Link) -> Vec<Item> {
        match link.phase {
            Phase::Idle | Phase::Failed(_) => vec![Item::Host, Item::Join, Item::Leave],
            Phase::Lobby if link.hosting => vec![Item::Start, Item::Leave],
            Phase::Lobby => vec![Item::Ready, Item::Leave],
            Phase::Connecting(_) => vec![Item::Leave],
            Phase::Started => Vec::new(),
        }
    }

    fn item_y(link: &Link, i: usize) -> i32 {
        let top = match link.phase {
            Phase::Lobby => ROSTER_Y + ROW * (link.roster.len() as i32 + 1),
            _ => ITEMS_Y,
        };
        top + ROW * i as i32
    }

    fn at(link: &Link, x: i32, y: i32) -> Option<usize> {
        (0..Self::items(link).len())
            .find(|&i| Rect::new(LEFT, Self::item_y(link, i), ITEM_W, ROW).contains(x, y))
    }

    /// The request an item sends to the app loop's driver.
    pub fn press(&mut self, item: Item, ctx: &mut Ctx) -> Transition {
        let link = &mut ctx.game.net;
        let request = match item {
            Item::Host => Request::Host { addr: Request::every_interface(port_of(&self.address.text())) },
            Item::Join => Request::Join { address: self.address.text() },
            Item::Ready => {
                let me = link.me.and_then(|s| link.roster.get(s));
                Request::Ready(!me.is_some_and(|p| p.ready))
            }
            Item::Start => Request::Start,
            Item::Leave => {
                link.requests.push(Request::Leave);
                return Transition::Pop;
            }
        };
        link.requests.push(request);
        Transition::Stay
    }
}

impl Default for LobbyScreen {
    fn default() -> LobbyScreen {
        LobbyScreen::new()
    }
}

/// `host:port` or a bare port; anything else takes ours.
fn port_of(address: &str) -> u16 {
    let tail = address.rsplit(':').next().unwrap_or("");
    tail.trim().parse().unwrap_or(DEFAULT_PORT)
}

impl Screen for LobbyScreen {
    fn id(&self) -> ScreenId {
        ScreenId::Lobby
    }

    fn title(&self, _ctx: &Ctx) -> String {
        "Multiplayer lobby".into()
    }

    fn palette(&self) -> Option<&'static str> {
        Some(SetupPage::Custom.palette())
    }

    fn update(&mut self, ctx: &mut Ctx) -> Transition {
        self.address.tick();
        if ctx.game.net.phase == Phase::Started {
            return Transition::Replace(ScreenId::Campaign);
        }
        Transition::Stay
    }

    fn handle(&mut self, event: Event, ctx: &mut Ctx) -> Transition {
        let items = Self::items(&ctx.game.net);
        let typing = matches!(ctx.game.net.phase, Phase::Idle | Phase::Failed(_));
        match event {
            Event::KeyDown(Key::Escape) | Event::RightClick { .. } => self.press(Item::Leave, ctx),
            Event::KeyDown(Key::Up) if !items.is_empty() => {
                self.selected = (self.selected + items.len() - 1) % items.len();
                Transition::Stay
            }
            Event::KeyDown(Key::Down) if !items.is_empty() => {
                self.selected = (self.selected + 1) % items.len();
                Transition::Stay
            }
            Event::KeyDown(Key::Enter) => match items.get(self.selected) {
                Some(&item) => self.press(item, ctx),
                None => Transition::Stay,
            },
            Event::Click { x, y } => match Self::at(&ctx.game.net, x, y) {
                Some(i) => {
                    self.selected = i;
                    self.press(items[i], ctx)
                }
                None => Transition::Stay,
            },
            e if typing => {
                self.address.event(e, &FontMetrics::of(&ctx.assets.shell));
                Transition::Stay
            }
            _ => Transition::Stay,
        }
    }

    fn draw(&mut self, ctx: &Ctx, canvas: &mut Canvas) {
        let a = &ctx.assets.shell;
        let head = Pen {
            assets: a,
            ink: &ctx.assets.ink,
            chrome: ctx.assets.chrome.as_ref(),
            shadow: Some(font::SHADOW_GATEWAY),
            caps: Some(1),
        };
        let pen = head.flat();
        if !shell::background(canvas, a, SetupPage::Custom.background()) {
            canvas.clear(ctx.assets.ink.background);
        }
        let (x, y, cols, rows) = BOX;
        pen.window(canvas, x, y, cols, rows, 1);
        head.eng_heading_centred(canvas, HEADING.0, HEADING.1, x, y + 0x10, cols * 16, font::TEXT);
        let link = &ctx.game.net;
        match &link.phase {
            Phase::Lobby | Phase::Started => {
                for (i, p) in link.roster.players.iter().enumerate() {
                    let mark = if Some(p.slot) == link.me { ">" } else { " " };
                    let state = if p.is_host { "host" } else if p.ready { "ready" } else { "waiting" };
                    let line = format!("{mark} {}  shield {}  {state}", p.name, p.shield);
                    pen.body(canvas, LEFT, ROSTER_Y + ROW * i as i32, &line, font::TEXT);
                }
            }
            _ => {
                let text = format!("Address: {}", self.address.text());
                pen.body(canvas, LEFT, ADDRESS_Y, &text, font::TEXT);
            }
        }
        for (i, item) in Self::items(link).iter().enumerate() {
            let colour = if i == self.selected { SELECTED } else { font::TEXT };
            let words = match item {
                Item::Host => "Host a game",
                Item::Join => "Join a game",
                Item::Ready => "Ready",
                Item::Start => "Start",
                Item::Leave => "Leave",
            };
            pen.body(canvas, LEFT, Self::item_y(link, i), words, colour);
        }
        for (i, line) in Self::message(link).iter().enumerate() {
            pen.body(canvas, LEFT, MESSAGE_Y + LINE * i as i32, line, font::TEXT);
        }
    }
}

impl LobbyScreen {
    /// The lines under the items: why a join failed, where the host is reached.
    pub fn message(link: &Link) -> Vec<String> {
        let text = match &link.phase {
            Phase::Failed(why) => why.clone(),
            Phase::Connecting(to) => format!("Connecting to {to}..."),
            Phase::Lobby if link.hosting => {
                let at: Vec<String> = link.reach.iter().map(|a| a.to_string()).collect();
                format!(
                    "Others join at {}. If Windows asks, allow the game through its firewall.",
                    at.join(" or ")
                )
            }
            _ => return Vec::new(),
        };
        crate::net::wrap(&text, MESSAGE_COLS)
    }
}

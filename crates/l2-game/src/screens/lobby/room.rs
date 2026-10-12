//! The room (`Phase::Lobby`): the code, where the host is reached, the roster,
//! Start game or I'm ready, Leave (C255). The C252/C254 sentences are kept.

use l2_view::Canvas;

use super::draw::{self, Heading, Look, Pens, BUTTON_H, BUTTON_W, LEFT, WIDTH};
use super::Item;
use crate::input::Rect;
use crate::net::{Link, Phase, PortMap, DEFAULT_PORT};
use crate::screen::Ctx;
use crate::shell::font;

pub const CODE_CAPTION: &str = "Tell your friends this code";
const MESSAGE_COLS: usize = 44;
const CODE_Y: i32 = 0x66;
const CODE_CELL: i32 = 32;
const CODE_CELL_H: i32 = 24;
const REACH_Y: i32 = 0x9C;
const CODE_HINT: &str = "They choose Join a game and type it.";
const ROSTER_Y: i32 = 0xC4;
const ROW: i32 = 22;
const STATE_X: i32 = LEFT + 0x110;
const BUTTON_Y: i32 = 0x13C;
const CAPTION_Y: i32 = BUTTON_Y + BUTTON_H + 6;

/// The host's whereabouts, one sentence: the relay's refusal (C254) and the
/// port's state (C252). `None` for a joiner, or before the lobby.
pub fn reach_text(link: &Link) -> Option<String> {
    if link.phase != Phase::Lobby || !link.hosting {
        return None;
    }
    let at: Vec<String> = link.reach.iter().map(|a| a.to_string()).collect();
    let at = at.join(" or ");
    let port = link.reach.first().map_or(DEFAULT_PORT, |a| a.port());
    const FIREWALL: &str = "If Windows asks, allow the game through its firewall.";
    let text = match &link.port {
        PortMap::Off => format!("Others join at {at}. {FIREWALL}"),
        PortMap::Asking => format!(
            "On your network, others join at {at}. Asking your router to open port {port} for the internet..."
        ),
        PortMap::Open { public, via, lifetime } => {
            let span = if *lifetime == 0 { "until you leave".to_string() } else { format!("for {} hours", lifetime / 3600) };
            format!(
                "On your network, others join at {at}. Over the internet, at {public} ({via} opened the port {span}). {FIREWALL}"
            )
        }
        PortMap::Shut(why) => format!(
            "On your network, others join at {at}. Over the internet: {why}. Forward TCP port {port} on your router to this machine by hand, then give your public address. {FIREWALL}"
        ),
    };
    match (&link.code, &link.relay) {
        (None, Some(why)) => Some(format!("{why} {text}")),
        _ => Some(text),
    }
}

/// The lines a test reads: the code first (C254), then [`reach_text`] wrapped;
/// a joiner's failure or connect as it stands.
pub fn message(link: &Link) -> Vec<String> {
    let mut lines = Vec::new();
    match &link.phase {
        Phase::Failed(why) => return crate::net::wrap(why, MESSAGE_COLS),
        Phase::Connecting(to) => return crate::net::wrap(&format!("Connecting to {to}..."), MESSAGE_COLS),
        _ => {}
    }
    if let (Some(code), true, Phase::Lobby) = (&link.code, link.hosting, &link.phase) {
        lines.push(format!("Join code: {code}"));
    }
    if let Some(text) = reach_text(link) {
        lines.extend(crate::net::wrap(&text, MESSAGE_COLS));
    }
    lines
}

pub fn waiting_for(link: &Link) -> Option<String> {
    link.roster.players.iter().find(|p| !p.is_host && !p.ready).map(|p| p.name.clone())
}

fn host_name(link: &Link) -> Option<String> {
    link.roster.players.iter().find(|p| p.is_host).map(|p| p.name.clone())
}

pub(super) fn items(link: &Link, host: bool) -> Vec<Item> {
    match &link.phase {
        Phase::Failed(_) => vec![Item::Leave],
        _ if host => vec![Item::Start, Item::Leave],
        _ => {
            let me = link.me.and_then(|s| link.roster.get(s));
            let ready = me.is_some_and(|p| p.ready);
            vec![if ready { Item::NotReady } else { Item::Ready }, Item::Leave]
        }
    }
}

pub(super) fn rects(link: &Link, host: bool) -> Vec<Rect> {
    let leave = Rect::new(LEFT + WIDTH - BUTTON_W, BUTTON_Y, BUTTON_W, BUTTON_H);
    match items(link, host).len() {
        1 => vec![leave],
        _ => vec![Rect::new(LEFT, BUTTON_Y, BUTTON_W, BUTTON_H), leave],
    }
}

pub(super) fn draw(ctx: &Ctx, canvas: &mut Canvas, pens: &Pens, host: bool, selected: usize) {
    let link = &ctx.game.net;
    let heading = match host_name(link) {
        _ if host => "Your game".to_string(),
        Some(name) => format!("{name}'s game"),
        None => "Joining".to_string(),
    };
    draw::backdrop(ctx, canvas, pens, Heading::Text(&heading));
    let mut roster_y = ROSTER_Y;
    if let Phase::Failed(why) = &link.phase {
        pens.body.body_wrapped(canvas, LEFT, ROSTER_Y, WIDTH, why, font::TEXT);
    } else {
        match (&link.code, host) {
            (Some(code), true) => {
                pens.body.body_centred(canvas, LEFT, CODE_Y, WIDTH, CODE_CAPTION, font::TEXT);
                let x0 = LEFT + (WIDTH - CODE_CELL * code.len() as i32) / 2;
                for (i, c) in code.chars().enumerate() {
                    let cell = Rect::new(x0 + i as i32 * CODE_CELL, CODE_Y + 0x10, CODE_CELL - 4, CODE_CELL_H);
                    crate::shell::button_recess(canvas, cell.x, cell.y, cell.w, cell.h);
                    draw::symbol(canvas, pens, cell, c, font::TEXT);
                }
                pens.body.body_centred(canvas, LEFT, REACH_Y, WIDTH, CODE_HINT, font::TEXT);
            }
            _ => {
                if let Some(text) = reach_text(link) {
                    let height = pens.body.body_wrapped(canvas, LEFT, CODE_Y, WIDTH, &text, font::TEXT);
                    roster_y = roster_y.max(CODE_Y + height + 8);
                }
            }
        }
        draw_roster(canvas, pens, link, roster_y);
    }
    for (i, (item, r)) in items(link, host).iter().zip(rects(link, host)).enumerate() {
        let enabled = *item != Item::Start || waiting_for(link).is_none();
        let look = match (enabled, i == selected) {
            (false, _) => Look::Disabled,
            (true, true) => Look::Selected,
            (true, false) => Look::Plain,
        };
        draw::button(canvas, &pens.body, r, item.label(), look);
        if *item == Item::Start {
            if let Some(name) = waiting_for(link) {
                let caption = format!("Waiting for {name}");
                pens.body.body_centred(canvas, r.x, CAPTION_Y, r.w, &caption, font::TEXT);
            }
        }
    }
}

fn draw_roster(canvas: &mut Canvas, pens: &Pens, link: &Link, top: i32) {
    for (i, p) in link.roster.players.iter().enumerate() {
        let y = top + ROW * i as i32;
        let mine = Some(p.slot) == link.me;
        let ink = if mine { font::HIGHLIGHT } else { font::TEXT };
        draw::plaque(canvas, &pens.body, p.shield, LEFT, y + 1);
        let name = if mine { format!("{} (you)", p.name) } else { p.name.clone() };
        pens.body.body(canvas, LEFT + 22, y + 2, &name, ink);
        if p.is_host {
            pens.body.body(canvas, STATE_X, y + 2, "host", ink);
        } else if p.ready {
            draw::tick(canvas, STATE_X, y + 1);
            pens.body.body(canvas, STATE_X + 20, y + 2, "ready", ink);
        } else {
            pens.body.body(canvas, STATE_X, y + 2, "not ready yet", ink);
        }
    }
}

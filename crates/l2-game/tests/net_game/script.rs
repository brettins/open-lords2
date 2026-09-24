//! A scripted player: orders from the state at the tick they execute on.

use l2_game::command::{self as cmd, Answer, GameCommand};
use l2_game::{turn, Game};
use l2_kingdom::unit::{Unit, UnitKind};
use l2_net::{Canonical, Command, PlayerSlot};

pub struct Script {
    pub mine: usize,
    pub theirs: usize,
    pub target: (u8, u8),
    pub county: u8,
    pub asked: u32,
    pub reports: u32,
    pub ended: u32,
    /// `theirs`' men when the first report came up: the AI may recruit into it later.
    pub theirs_at_report: Option<i32>,
    idle: u32,
}

fn army(game: &mut Game, owner: u8, x: u8, y: u8, men: i32) -> usize {
    let mut u = Unit::new(UnitKind::Army, owner, x, y);
    u.men = men;
    u.troops[0] = men;
    u.county = game.kingdom.campaign.map.county_at(x, y);
    u.home_county = u.county;
    u.owner_is_human = game.kingdom.realms[owner as usize].is_human;
    game.kingdom.campaign.units.spawn(u).expect("a free slot")
}

/// World-building, done alike on every copy: a player army at his county
/// town and an AI army one tile off, which the script then marches into.
pub fn stage(game: &mut Game) -> Script {
    let me = game.player;
    let county = game.kingdom.county_ids().find(|&c| game.kingdom.counties[c].owner == me);
    let county = county.expect("the player holds a county") as u8;
    let (ax, ay) = (game.anchor_x[county as usize], game.anchor_y[county as usize]);
    let foe = (1..l2_kingdom::MAX_REALMS as u8)
        .find(|&r| r != me && game.kingdom.realms[r as usize].in_play)
        .expect("an AI realm");
    let map = &game.kingdom.campaign.map;
    let target = [(2i16, 0i16), (0, 2), (-2, 0), (0, -2)]
        .iter()
        .map(|&(dx, dy)| ((ax as i16 + dx) as u8, (ay as i16 + dy) as u8))
        .find(|&(x, y)| map.county_at(x, y) != 0)
        .expect("land beside the town");
    let mine = army(game, me, ax, ay, 400);
    let theirs = army(game, foe, target.0, target.1, 60);
    Script { mine, theirs, target, county, asked: 0, reports: 0, ended: 0, theirs_at_report: None, idle: 0 }
}

impl Script {
    pub fn orders(&mut self, game: &Game, tick: u32) -> Vec<GameCommand> {
        let mut out: Vec<GameCommand> = Vec::new();
        match tick {
            2 => out.push(cmd::SetTaxRate { county: self.county, rate: 3 }.into()),
            3 => out.push(cmd::MoveUnit { unit: self.mine, dest: self.target }.into()),
            _ => {}
        }
        if turn::pending_question(game).is_some() {
            self.asked += 1;
            out.push(cmd::AnswerBattle { answer: Answer::Decline }.into());
        } else if turn::pending_report(game).is_some() {
            self.reports += 1;
            let men = game.kingdom.campaign.units.get(self.theirs).map_or(0, |u| u.men);
            self.theirs_at_report.get_or_insert(men);
            out.push(cmd::DismissReport {}.into());
        } else if !turn::turn_in_flight(game) {
            self.idle += 1;
            if self.idle >= 40 {
                self.idle = 0;
                self.ended += 1;
                out.push(cmd::EndTurn {}.into());
            }
        }
        out
    }
}

pub fn wire(orders: &[GameCommand]) -> Vec<Command> {
    orders
        .iter()
        .enumerate()
        .map(|(i, o)| Command::new(PlayerSlot::new(0), i as u32, Canonical::bytes_of(o)))
        .collect()
}

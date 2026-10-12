//! C256: the host's world as bytes, and a joiner taking it in. `Sync_ResendState`
//! (`0x0043F939`) is the original's: 80,000 raw bytes of snapshot from the master
//! machine. Ours is `save::encode` plus what a save leaves out because a load starts
//! the turn afresh; [`l2_net::Session::resynced`] checks the digest it reproduces.

use crate::battlefield::{LiveBattle, Mode};
use crate::game::Game;
use crate::{save, turn};
use l2_net::{Canonical, PlayerSlot, Reader};

pub fn encode(game: &Game, malformed: u32) -> Vec<u8> {
    let mut out = Canonical::recording();
    out.bytes(&save::encode(game));
    out.bool(game.ai_granted);
    out.seq(&game.seats, |c, &(slot, realm)| {
        c.u8(slot.index());
        c.u8(realm);
    });
    out.option(game.turn.as_ref(), turn::sync::encode);
    game.turn_clock.encode_sync(&mut out);
    let c = &game.kingdom.campaign;
    out.u8(c.battle_county);
    out.seq(c.move_cost.as_slice(), |o, v| o.i16(*v));
    out.u32(malformed);
    out.finish().bytes.expect("a recording encoder keeps its bytes")
}

/// The digest's fields come from the host (`net::sim::encode_state`); C210's per-seat
/// ones stay this seat's.
pub fn adopt(game: &mut Game, malformed: &mut u32, bytes: &[u8]) -> Result<(), String> {
    let mut r = Reader::new(bytes);
    let host = save::decode(r.bytes().map_err(|e| e.to_string())?, game.kingdom.tables)
        .map_err(|e| e.to_string())?;
    let mut rest = || -> Result<_, l2_net::CodecError> {
        let ai_granted = r.bool()?;
        let seats = r.seq(|r| {
            let at = r.position();
            let slot = PlayerSlot::from_wire(r.u8()?)
                .ok_or(l2_net::CodecError::BadTag { tag: 0, expected: "player slot", at })?;
            Ok((slot, r.u8()?))
        })?;
        let progress = r.option(turn::sync::decode)?;
        let mut clock = game.turn_clock;
        clock.adopt_sync(&mut r)?;
        let county = r.u8()?;
        let cost = r.seq(|r| r.i16())?;
        let m = r.u32()?;
        Ok((ai_granted, seats, progress, clock, (county, cost), m))
    };
    let (ai_granted, seats, progress, clock, (county, cost), m) = rest().map_err(|e| e.to_string())?;
    r.finish().map_err(|e| e.to_string())?;
    game.kingdom = host.kingdom;
    // [D] in neither a save nor the digest (B108). Without `move_cost` the seats
    // diverge again (C256: tick 3,155, measured).
    game.kingdom.campaign.battle_county = county;
    game.kingdom.campaign.move_cost = l2_kingdom::map::CostMap::from_slice(&cost);
    game.turns_played = host.turns_played;
    game.gold_last = host.gold_last;
    game.ai_granted = ai_granted;
    game.seats = seats;
    game.turn = progress;
    game.turn_clock = clock;
    game.battle = match (game.battle.take(), host.battle) {
        (Some(mut mine), Some(mut theirs)) => {
            keep_view(&mut mine, &mut theirs);
            Some(theirs)
        }
        (_, theirs) => theirs,
    };
    *malformed = m;
    Ok(())
}

/// C210: `encode_battle`'s "out, per seat" list stays this seat's; `mode` only short of
/// the outcome.
fn keep_view(mine: &mut LiveBattle, theirs: &mut LiveBattle) {
    use std::mem::swap;
    if theirs.mode != Mode::Outcome && mine.mode != Mode::Outcome {
        theirs.mode = mine.mode;
    }
    theirs.owner = mine.owner;
    theirs.cam = mine.cam;
    swap(&mut theirs.drag, &mut mine.drag);
    swap(&mut theirs.hover, &mut mine.hover);
    theirs.pointer = mine.pointer;
    theirs.pointer_in = mine.pointer_in;
    swap(&mut theirs.groups, &mut mine.groups);
    theirs.current_unit = mine.current_unit;
    theirs.order_mark = mine.order_mark;
    theirs.order_mark_ticks = mine.order_mark_ticks;
    theirs.scroll_wait = mine.scroll_wait;
    theirs.redraw = mine.redraw;
    swap(&mut theirs.cries, &mut mine.cries);
    theirs.deferred = mine.deferred;
    swap(&mut theirs.outbox, &mut mine.outbox);
    swap(&mut theirs.awaiting, &mut mine.awaiting);
}

/// C260: a dropped seat's world from the host's `Rejoin`: the save inside C256's
/// payload as a load decodes it, then the rest of the payload over it.
pub fn rebuild(r: &l2_net::Rejoin, tables: l2_kingdom::tables::Tables) -> Result<Game, String> {
    let bytes = Reader::new(&r.state).bytes().map_err(|e| e.to_string())?;
    let mut game = save::decode(bytes, tables).map_err(|e| e.to_string())?;
    let mut malformed = 0;
    adopt(&mut game, &mut malformed, &r.state)?;
    game.net.me = Some(r.slot);
    game.player = game.realm_of(r.slot).ok_or("the host gave this seat no realm")?;
    let (k, mine) = (&game.kingdom, game.player);
    game.selected = k.county_ids().find(|&c| k.counties[c].owner == mine).map_or(0, |c| c as u8);
    game.multiplayer = true;
    Ok(game)
}

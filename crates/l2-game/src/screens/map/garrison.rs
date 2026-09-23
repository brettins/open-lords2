//! The map's two move-order questions on screen 0x1E: Map_ConfirmMoveOrder 0x004A9252.

use l2_kingdom::industry::garrison_cap;
use l2_kingdom::map::{self, flags, terrain};
use l2_kingdom::unit::CombineRefusal;

use crate::screen::{Ctx, Transition};
use crate::screens::confirm::Ask;

/// Map_HoverUnitTarget: `flags & 0x80 && content > 0x14 && county.owner == unit.owner`
/// (`004a0000.c:3759`); all prompts cleared when the moves left fall short (`:3778`).
pub(crate) fn garrison_ask(ctx: &Ctx, unit: usize, dest: (u8, u8), dist: usize) -> Option<Ask> {
    let u = ctx.game.kingdom.campaign.units.get(unit)?;
    let m = &ctx.game.kingdom.campaign.map;
    let tile = map::index(dest.0, dest.1);
    let castle = m.flags[tile] & flags::SETTLEMENT != 0 && m.terrain[tile] > terrain::CASTLE_PLOT;
    let county = m.county[tile];
    let owner = ctx.game.kingdom.counties.get(county as usize)?.owner;
    let reach = dist <= u.moves_left().max(0) as usize;
    (castle && owner == u.owner && reach).then_some(Ask::Garrison { unit, county, dest })
}

/// Map_HoverUnitTarget's `local_8` (`004a0000.c:3729`): g_moveDistLocal at the target, and past
/// 99 FUN_00470678 0x00470678's least non-zero of it and its eight neighbours, plus one.
pub(crate) fn hover_distance(field: &l2_kingdom::movement::DistanceField, dest: (u8, u8)) -> usize {
    let at = field.raw(dest.0, dest.1);
    if at <= 99 {
        return at.max(0) as usize;
    }
    let mut least = at;
    for (dx, dy) in [(0, -1), (1, -1), (1, 0), (1, 1), (0, 1), (-1, 1), (-1, 0), (-1, -1)] {
        let (x, y) = (dest.0 as i32 + dx, dest.1 as i32 + dy);
        if !(0..map::MAP_DIM as i32).contains(&x) || !(0..map::MAP_DIM as i32).contains(&y) {
            continue;
        }
        let d = field.raw(x as u8, y as u8);
        if d != 0 && d < least {
            least = d;
        }
    }
    least as usize + 1
}

/// MoveOrder_ConfirmCombine 0x004A975D: over the cap, `Msg_Enqueue(0, g_localPlayer, 0x112, …)`
/// (`004a0000.c:4009`).
pub(crate) fn answer_combine(ctx: &mut Ctx, yes: bool) -> Transition {
    let Some((mover, occupant)) = ctx.game.combine_ask.take() else { return Transition::Pop };
    if !yes {
        return Transition::Pop;
    }
    let player = ctx.game.player;
    match ctx.issue(crate::command::CombineArmies { occupant, mover }) {
        Err(CombineRefusal::TooMany) => notice(ctx, player, 0x112),
        // [I] MoveOrder_ConfirmGarrison's merc refusal 0xA7 (`004a0000.c:3925`); the combine has none.
        Err(CombineRefusal::TwoMercenaryBands) => notice(ctx, player, 0xA7),
        _ => {}
    }
    Transition::Pop
}

/// Why MoveOrder_ConfirmGarrison 0x004A9372 refuses, as its `Msg_Enqueue` group; `None` orders.
pub(crate) fn garrison_refusal(ctx: &Ctx, unit: usize, county: u8) -> Option<u16> {
    let units = &ctx.game.kingdom.campaign.units;
    let u = units.get(unit)?;
    let c = ctx.game.kingdom.counties.get(county as usize)?;
    // `DAT_004d8a0c[castleType]` is g_castleGarrisonCap for types 1..5 (job/castle.rs castle_word).
    let mut room = garrison_cap(&ctx.game.kingdom.tables, c.castle_type);
    let garrison = (c.garrison_unit != 0).then(|| units.get(c.garrison_unit)).flatten();
    if let Some(g) = garrison {
        room -= g.men;
        if g.besieged_by != 0 {
            return Some(0x121); // `:3933`
        }
    }
    if c.castle_degraded == 1 && c.castle_building == 0 {
        Some(0xA4) // `:3906`
    } else if c.castle_ruined {
        Some(0xA5) // `:3929`, county +0x1C2
    } else if room < 1 {
        Some(0x11B) // `:3910`
    } else if room < u.men {
        Some(0xA6) // `:3913`
    } else if u.mercenaries.is_some() && garrison.is_some_and(|g| g.mercenaries.is_some()) {
        Some(0xA7) // `:3925`
    } else {
        None
    }
}

/// MoveOrder_ConfirmGarrison 0x004A9372: `to` is the mover's owner; 0xA6 carries
/// `g_selectedUnit, 0x11, g_hoverGarrisonCounty, 0, DAT_004ee898` (`004a0000.c:3913`).
pub(crate) fn answer_garrison(
    ctx: &mut Ctx,
    yes: bool,
    unit: usize,
    county: u8,
    dest: (u8, u8),
) -> Transition {
    if !yes {
        return Transition::Pop;
    }
    let Some(to) = ctx.game.kingdom.campaign.units.get(unit).map(|u| u.owner) else {
        return Transition::Pop;
    };
    match garrison_refusal(ctx, unit, county) {
        // `DAT_004ee898 = g_hoverTileOffset` (`:3844`), a byte offset `(y*64+x)*8`.
        Some(0xA6) => {
            let record = crate::message::Record {
                to,
                group: 0xA6,
                variant: unit as u8,
                category: crate::message::category::GARRISON_PROMPT,
                county,
                payload: (map::index(dest.0, dest.1) * 8) as i32,
                ..Default::default()
            };
            let player = ctx.game.player;
            ctx.game.messages.enqueue(record, player);
        }
        Some(group) => notice(ctx, to, group),
        // `Unit_OrderMove(g_localPlayer, g_selectedUnit, g_moveTargetX, g_moveTargetY, 0)`.
        None => {
            ctx.issue(crate::command::MoveUnit { unit, dest });
        }
    }
    Transition::Pop
}

fn notice(ctx: &mut Ctx, to: u8, group: u16) {
    let record = crate::message::Record { to, group, ..Default::default() };
    let player = ctx.game.player;
    ctx.game.messages.enqueue(record, player);
}

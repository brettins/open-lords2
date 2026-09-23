//! The map's move-order questions on screen 0x1E: Map_ConfirmMoveOrder 0x004A9252.

use l2_kingdom::industry::garrison_cap;
use l2_kingdom::map::{self, flags, terrain};
use l2_kingdom::unit::{CombineRefusal, UnitKind};

use crate::screen::{Ctx, Transition};
use crate::screens::confirm::Ask;

/// Map_HoverUnitTarget 0x004A8E0B's six targets for `dest`; 0 is none, as in the original.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Hover {
    pub village: u8,
    pub field: u8,
    pub town: u8,
    pub merge: usize,
    pub garrison: u8,
    pub siege: u8,
}

/// Map_HoverUnitTarget 0x004A8E0B `004a0000.c:3744-3786`: each test reads the target tile only.
pub(crate) fn hover(ctx: &Ctx, unit: usize, dest: (u8, u8), dist: usize) -> Hover {
    let mut h = Hover::default();
    let units = &ctx.game.kingdom.campaign.units;
    let Some(u) = units.get(unit) else { return h };
    let m = &ctx.game.kingdom.campaign.map;
    let tile = map::index(dest.0, dest.1);
    let (f, content, county) = (m.flags[tile], m.terrain[tile], m.county[tile]);
    let owner = ctx.game.kingdom.counties.get(county as usize).map(|c| c.owner);
    let foreign = owner != Some(u.owner);
    if f & flags::CASTLE != 0 && foreign {
        h.town = county; // `:3745` g_hoverCountyTownCounty
    }
    if f & flags::PLOT != 0 && foreign {
        h.village = county; // `:3749`
    }
    // `:3753` County_TileIsField 0x00469E0E: `1 < content < 0x17`.
    if f & flags::FARMLAND != 0 && (2..0x17).contains(&content) && foreign {
        h.field = county;
    }
    // `:3759`: `uVar1 = uVar3` makes a foreign castle the siege county.
    if f & flags::SETTLEMENT != 0 && content > terrain::CASTLE_PLOT {
        if foreign {
            h.siege = county;
        } else {
            h.garrison = county;
        }
    }
    // `:3767`: the tile's unit, g_localPlayer's, not the mover, kind != 4.
    if let Some(o) = units.at(dest.0, dest.1) {
        let mine = units
            .get(o)
            .is_some_and(|w| w.owner == ctx.game.player && w.kind != UnitKind::Transport);
        if o != unit && mine {
            h.merge = o;
        }
    }
    // `:3778`: all six cleared when the moves left fall short.
    if (u.moves_left().max(0) as usize) < dist {
        return Hover::default();
    }
    h
}

/// Map_ConfirmMoveOrder 0x004A9252 `004a0000.c:3845-3869`: the first rung set picks the question.
pub(crate) fn pick(h: Hover, unit: usize, dest: (u8, u8)) -> Option<Ask> {
    let town = h.town;
    if h.village != 0 {
        Some(Ask::Slaughter { unit, dest, town }) // `:3869` Ui_OpenConfirm(4, …, MoveOrder_Confirm)
    } else if h.field != 0 {
        Some(Ask::DestroyField { unit, dest, town }) // `:3865` Ui_OpenConfirm(10, …)
    } else if h.merge != 0 {
        None // [D] C226: 10/5 is asked on contact (turn/engine/tick.rs ask_combine).
    } else if h.garrison != 0 {
        Some(Ask::Garrison { unit, county: h.garrison, dest })
    } else if h.siege != 0 {
        Some(Ask::Siege { unit, county: h.siege, dest })
    } else {
        None
    }
}

/// MoveOrder_Confirm 0x004A983B `004a0000.c:4031`: a county town whose castle holds a garrison
/// refuses with 0x11E, ahead of the answer; `true` when it refused.
pub(crate) fn town_refusal(ctx: &mut Ctx, town: u8) -> bool {
    let held = town != 0
        && ctx.game.kingdom.counties.get(town as usize).is_some_and(|c| c.garrison_unit != 0);
    if held {
        let player = ctx.game.player;
        notice(ctx, player, 0x11E);
    }
    held
}

/// MoveOrder_Confirm 0x004A983B, the yes of 10/4 and 10/10: `Unit_OrderMove(…, 0)` (`:4034`).
pub(crate) fn answer_move(
    ctx: &mut Ctx,
    yes: bool,
    unit: usize,
    dest: (u8, u8),
    town: u8,
) -> Transition {
    if !town_refusal(ctx, town) && yes {
        ctx.issue(crate::command::MoveUnit { unit, dest });
    }
    Transition::Pop
}

/// MoveOrder_ConfirmSiege 0x004A960F `004a0000.c:3956-3976`.
pub(crate) fn answer_siege(
    ctx: &mut Ctx,
    yes: bool,
    unit: usize,
    county: u8,
    dest: (u8, u8),
) -> Transition {
    if !yes {
        return Transition::Pop;
    }
    let Some(c) = ctx.game.kingdom.counties.get(county as usize) else { return Transition::Pop };
    let (keep, degraded) = (c.garrison_unit, c.castle_degraded);
    let besieged = keep != 0
        && ctx.game.kingdom.campaign.units.get(keep).is_some_and(|g| g.besieged_by != 0);
    let player = ctx.game.player;
    if besieged {
        notice(ctx, player, 0x113); // `:3975`
    } else if keep == 0 {
        notice(ctx, player, if degraded == 0 { 0x11D } else { 0x11C }); // `:3961`/`:3964`
    } else {
        ctx.issue(crate::command::MoveUnit { unit, dest }); // `:3968`
    }
    Transition::Pop
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
                variant: unit as i32,
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

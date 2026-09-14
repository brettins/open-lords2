#![allow(unused_imports)]
use super::*;
use super::types::*;
use world::*;
use handlers::*;
use tests::*;
use crate::figure::{Figure, State};
use crate::troop::Troop;
use crate::unit::{chebyshev, pct_of, Units, MAX_UNITS, REFORM_INTERVAL};
use l2_net::Pcg32;

type Handler = fn(&mut World, usize);

/// One dispatch slot.
///
/// The name and address are carried so that a slot can be
/// checked against `docs/battle-ai.md` §1.3 — and so a test can say "these
/// seven slots are the empty handler" without comparing function pointers,
/// which `docs/netcode.md` forbids anywhere a decision is made and which is a
/// bad habit to start in a test.
#[derive(Clone, Copy)]
pub struct Slot {
    pub name: &'static str,
    /// Address in the GOG Windows build, `ImageBase 0x400000`, no ASLR.
    pub addr: u32,
    run: Handler,
}

impl Slot {
    const fn new(name: &'static str, addr: u32, run: Handler) -> Slot {
        Slot { name, addr, run }
    }
}

impl core::fmt::Debug for Slot {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{} (0x{:08X})", self.name, self.addr)
    }
}

/// `g_unitOrderTableField` (`0x004D91B8`), bound `category < 5`.
///
/// **In a field battle, categories 5 to 8 have no handler at all** — the table
/// has five entries and the code tests `< 5`, so an AI catapult in an open
/// field is never given an order. Its figures still shoot; the unit never
/// repositions. Not observed in a running game.
pub const TABLE_FIELD: [Slot; 5] = [
    Slot::new("UnitOrder_None", 0x0048_A9C7, order_none),
    Slot::new("UnitOrder_FieldMissile", 0x0048_A9D2, field_missile),
    Slot::new("UnitOrder_FieldFoot", 0x0048_ACD2, field_foot),
    Slot::new("UnitOrder_FieldMelee", 0x0048_B02B, field_melee),
    Slot::new("UnitOrder_FieldMelee", 0x0048_B02B, field_melee),
];

/// `g_unitOrderTableSiegeAtt` (`0x004D91D0`), bound `category < 9`.
pub const TABLE_SIEGE_ATT: [Slot; 9] = [
    Slot::new("UnitOrder_None", 0x0048_A9C7, order_none),
    Slot::new("UnitOrder_SiegeAttMissile", 0x0048_D16E, siege_att_missile),
    Slot::new("UnitOrder_SiegeAttFoot", 0x0048_D412, siege_att_foot),
    Slot::new("UnitOrder_SiegeAttMelee", 0x0048_D6FC, siege_att_melee),
    Slot::new("UnitOrder_SiegeAttKnight", 0x0048_D9CE, siege_att_knight),
    Slot::new("UnitOrder_SiegeAttCatapult", 0x0048_DB84, siege_att_catapult),
    Slot::new("UnitOrder_SiegeAttTower", 0x0048_DDC7, siege_att_tower),
    Slot::new("UnitOrder_SiegeAttRam", 0x0048_DFBB, siege_att_ram),
    // Oil is a defender's weapon, so the attacker's slot for it is empty.
    Slot::new("UnitOrder_None", 0x0048_A9C7, order_none),
];

/// `g_unitOrderTableSiegeDef` (`0x004D91F8`), bound `category < 11`.
///
/// The three stubs at 5, 6 and 7 line up exactly with `g_raiseOrderSiege`: a
/// castle garrison holds no catapult, siege tower or ram. Two unrelated
/// structures in the binary agreeing about which troops a garrison never has.
/// **[V]**
pub const TABLE_SIEGE_DEF: [Slot; 11] = [
    Slot::new("UnitOrder_None", 0x0048_A9C7, order_none),
    Slot::new("UnitOrder_SiegeDefMissile", 0x0048_E097, siege_def_missile),
    Slot::new("UnitOrder_SiegeDefFoot", 0x0048_E39C, siege_def_foot),
    Slot::new("UnitOrder_SiegeDefMelee", 0x0048_E4CC, siege_def_melee),
    Slot::new("UnitOrder_SiegeDefKnight", 0x0048_E774, siege_def_knight),
    Slot::new("UnitOrder_None", 0x0048_A9C7, order_none),
    Slot::new("UnitOrder_None", 0x0048_A9C7, order_none),
    Slot::new("UnitOrder_None", 0x0048_A9C7, order_none),
    Slot::new("UnitOrder_SiegeDefOil", 0x0048_E8B8, siege_def_oil),
    Slot::new("UnitOrder_SiegeDefWallMissileA", 0x0048_E234, siege_def_wall_missile_a),
    Slot::new("UnitOrder_SiegeDefWallMissileB", 0x0048_E2C8, siege_def_wall_missile_b),
];

/// **Which of the eighteen handlers a unit dispatches to**, or `None` when its
/// category is past its table's bound and it is given no order at all.
///
/// One function so that `update_all_units` and anything asking *"is this
/// handler reachable?"* cannot disagree — which matters, because fourteen of
/// the seventeen were unreachable for as long as nothing could produce a siege,
/// and the only honest way to say they are reachable now is to name the
/// position that reaches each one.
pub fn handler_for(is_siege: bool, side: crate::Side, category: u8) -> Option<Slot> {
    let category = category as usize;
    if !is_siege {
        TABLE_FIELD.get(category).copied()
    } else if side == crate::SIDE_A {
        TABLE_SIEGE_DEF.get(category).copied()
    } else {
        TABLE_SIEGE_ATT.get(category).copied()
    }
}

/// `Battle_UpdateAllUnits` (`0x00489401`), once per frame.
///
/// The order of operations is the original's and it matters: the strength
/// advantage is recomputed *before* any unit thinks, each unit is recentred on
/// its figures *before* its handler runs, and the reform countdown runs
/// **outside** the human-control guard — so a player's units reform
/// too.
///
/// Returns the units whose reform countdown reached zero and which
/// `BattleUnit_NeedsReform` accepts. Reforming assigns figures to formation
/// slots on a battlefield, so it belongs to whoever owns positions; this crate
/// says *which* units want it and stops there.
pub fn update_all_units(
    units: &mut Units,
    figures: &mut [Figure],
    positions: &[(u8, u8)],
    field: &AiField,
    ai: &mut Ai,
) -> Vec<usize> {
    ai.advantage_timer += 1;
    if ai.advantage_timer > ADVANTAGE_INTERVAL - 1 {
        ai.update_strength_advantage(figures);
        ai.advantage_timer = 0;
    }

    let mut reform = Vec::new();
    for cur in 1..=MAX_UNITS {
        if !units.get(cur).is_live() {
            continue;
        }
        units.recentre(cur, figures, positions);
        if !units.get(cur).human {
            let slot = handler_for(ai.is_siege, units.get(cur).side, units.get(cur).category);
            if let Some(slot) = slot {
                let mut world = World { units, figures, positions, field, ai };
                (slot.run)(&mut world, cur);
            } else {
                // Out of the table's bound: no order at all. In a field battle
                // that is every category from 5 to 8.
                ai.record(cur, Action::NoThink);
            }
        } else {
            ai.record(cur, Action::NoThink);
        }

        let u = units.get_mut(cur);
        u.reform -= 1;
        if u.reform < 1 {
            u.reform = REFORM_INTERVAL;
            if needs_reform(units.get(cur)) {
                reform.push(cur);
            }
        }
    }
    reform
}

/// `BattleUnit_NeedsReform` (`0x00489654`): false for an empty unit, false for
/// one that has been ordered to charge, and false for a **human-controlled**
/// unit of fewer than four figures.
pub fn needs_reform(u: &crate::unit::BattleUnit) -> bool {
    if u.figures == 0 || u.halted {
        return false;
    }
    !(u.figures < 4 && !u.reform_gate && u.human)
}



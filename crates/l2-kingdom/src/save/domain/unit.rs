#![allow(unused_imports)]
use super::*;
use super::county::*;
use super::industry::*;
use super::realm::*;
use super::history::*;
use super::tables::*;
use super::*;
use super::codec::*;
use crate::county::{ChangeReason, County, Industry, MAX_COUNTIES, MAX_INFLOW_SOURCES, MAX_NEIGHBOURS};
use crate::kingdom::{History, HistoryEntry, Kingdom, Options};
use crate::phase::{Phase, TurnMachine};
use crate::realm::{Realm, MAX_REALMS};
use crate::tables::{
    Tables, Weather, HISTORY_COUNTIES, HISTORY_SEASONS, JOB_COUNT, WEAPON_TYPE_COUNT,
};
use l2_net::canonical::{Canonical, CodecError, Decode, Encode, Reader};

impl Encode for crate::unit::Unit {
    fn encode(&self, out: &mut Canonical) {
        out.u8(self.owner);
        out.bool(self.owner_is_human);
        out.u8(self.shield);
        out.bool(self.player_driven);
        out.u8(self.kind.byte());
        out.u8(self.facing);
        out.u8(self.x);
        out.u8(self.y);
        out.u8(self.county);
        out.u8(self.home_county);
        match self.dest {
            None => out.bool(false),
            Some((x, y)) => {
                out.bool(true);
                out.u8(x);
                out.u8(y);
            }
        }
        out.u32(self.path.len() as u32);
        for (x, y) in &self.path {
            out.u8(*x);
            out.u8(*y);
        }
        out.bool(self.moving);
        out.bool(self.on_road);
        out.u8(self.sub_tile);
        out.u8(self.sub_frame);
        out.bool(self.at_tile_edge);
        out.u8(self.name_index);
        out.bool(self.needs_destination);
        out.u8(self.dest_county);
        out.i32(self.moves_used);
        out.i32(self.move_allowance);
        out.i32(self.starvation);
        out.i32(self.wages);
        out.i32(self.year_formed);
        out.i32(self.morale);
        out.i32(self.men);
        for count in &self.troops {
            out.i32(*count);
        }
        match self.mercenaries {
            None => out.bool(false),
            Some(m) => {
                out.bool(true);
                out.u8(m.band);
                out.u8(m.troop as u8);
                out.u8(m.men);
            }
        }
        out.u8(self.garrison_county);
        out.u8(self.besieging_county);
        out.u8(self.besieged_by);
        out.u8(self.cargo_county);
        // The mission byte and its county (`VERSION` 14). `+0x1A` is what AI
        // step 11 dispatches on,
        // army as an attacker � including the garrisons, which would then
        // walk out of their castles.
        out.u8(self.mission);
        out.u8(self.mission_county);
        // The siege state. `defence_mark` joins it here for the reason C30
        // gives: it was in no save and in no checksum, and it is the byte that
        // decides whether winning a battle also wins the county. It is
        // short-lived — written when a defence is found, read when the battle
        // returns — but a save taken between those two points loses the county.
        out.u8(self.defence_mark);
        for record in &self.engines {
            out.i16(record.ordered);
            out.i16(record.percent);
            out.i16(record.work_done);
        }
        out.u8(self.siege_seasons_left);
    }
}

impl Decode for crate::unit::Unit {
    fn decode(input: &mut Reader<'_>) -> Result<crate::unit::Unit, CodecError> {
        let owner = input.u8()?;
        let owner_is_human = input.bool()?;
        let shield = input.u8()?;
        let player_driven = input.bool()?;
        let tag = input.u8()?;
        let kind = crate::unit::UnitKind::from_byte(tag).ok_or(CodecError::BadTag {
            tag,
            expected: "unit type 1..=4",
            at: input.position() - 1,
        })?;
        let mut u = crate::unit::Unit::new(kind, owner, 0, 0);
        u.owner_is_human = owner_is_human;
        u.shield = shield;
        u.player_driven = player_driven;
        u.facing = input.u8()?;
        u.x = input.u8()?;
        u.y = input.u8()?;
        u.county = input.u8()?;
        u.home_county = input.u8()?;
        u.dest = if input.bool()? { Some((input.u8()?, input.u8()?)) } else { None };
        let steps = input.u32()? as usize;
        if steps > crate::unit::MAX_PATH {
            return Err(CodecError::BadTag {
                tag: steps.min(255) as u8,
                expected: "a path of at most 150 steps",
                at: input.position(),
            });
        }
        u.path = Vec::with_capacity(steps);
        for _ in 0..steps {
            u.path.push((input.u8()?, input.u8()?));
        }
        u.moving = input.bool()?;
        u.on_road = input.bool()?;
        u.sub_tile = input.u8()?;
        u.sub_frame = input.u8()?;
        u.at_tile_edge = input.bool()?;
        u.name_index = input.u8()?;
        u.needs_destination = input.bool()?;
        u.dest_county = input.u8()?;
        u.moves_used = input.i32()?;
        u.move_allowance = input.i32()?;
        u.starvation = input.i32()?;
        u.wages = input.i32()?;
        u.year_formed = input.i32()?;
        u.morale = input.i32()?;
        u.men = input.i32()?;
        for slot in 0..crate::unit::TROOP_TYPES {
            u.troops[slot] = input.i32()?;
        }
        u.mercenaries = if input.bool()? {
            let band = input.u8()?;
            let tag = input.u8()?;
            let troop = crate::unit::TroopType::from_index(tag as usize).ok_or(CodecError::BadTag {
                tag,
                expected: "troop type 0..=6",
                at: input.position() - 1,
            })?;
            Some(crate::unit::Mercenaries { band, troop, men: input.u8()? })
        } else {
            None
        };
        u.garrison_county = input.u8()?;
        u.besieging_county = input.u8()?;
        u.besieged_by = input.u8()?;
        u.cargo_county = input.u8()?;
        u.mission = input.u8()?;
        u.mission_county = input.u8()?;
        u.defence_mark = input.u8()?;
        for record in u.engines.iter_mut() {
            record.ordered = input.i16()?;
            record.percent = input.i16()?;
            record.work_done = input.i16()?;
        }
        u.siege_seasons_left = input.u8()?;
        Ok(u)
    }
}


#![allow(unused_imports)]
use super::*;
use super::unit::*;
use super::county::*;
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

impl Encode for Industry {
    fn encode(&self, out: &mut Canonical) {
        out.i32(self.output);
        out.i32(self.efficiency);
        out.i32(self.last_efficiency);
        out.i32(self.capacity);
        out.bool(self.has_resource);
        out.bool(self.enabled);
        out.i32(self.disabled_seasons);
        out.i32(self.total);
        out.i32(self.next_season);
    }
}

impl Decode for Industry {
    fn decode(input: &mut Reader<'_>) -> Result<Industry, CodecError> {
        Ok(Industry {
            output: input.i32()?,
            efficiency: input.i32()?,
            last_efficiency: input.i32()?,
            capacity: input.i32()?,
            has_resource: input.bool()?,
            enabled: input.bool()?,
            disabled_seasons: input.i32()?,
            total: input.i32()?,
            next_season: input.i32()?,
        })
    }
}


#![allow(unused_imports)]
use super::*;
use super::unit::*;
use super::county::*;
use super::industry::*;
use super::realm::*;
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

impl Encode for History {
    fn encode(&self, out: &mut Canonical) {
        out.u32(self.head as u32);
        out.u32(self.tail as u32);
        out.u32(self.len as u32);
        out.u32(HISTORY_SEASONS as u32);
        out.u32(HISTORY_COUNTIES as u32);
        for season in &self.entries {
            for entry in season {
                out.i32(entry.population);
                out.i8(entry.happiness);
            }
        }
    }
}

pub(crate) fn decode_history(input: &mut Reader<'_>) -> Result<History, LoadError> {
    let head = input.u32()? as usize;
    let tail = input.u32()? as usize;
    let len = input.u32()? as usize;
    let seasons = input.u32()? as usize;
    let counties = input.u32()? as usize;
    if seasons != HISTORY_SEASONS || counties != HISTORY_COUNTIES {
        return Err(LoadError::CorruptHistory { head, tail, len });
    }
    if head >= HISTORY_SEASONS || tail >= HISTORY_SEASONS || len > HISTORY_SEASONS {
        return Err(LoadError::CorruptHistory { head, tail, len });
    }
    let mut history = History::new();
    history.head = head;
    history.tail = tail;
    history.len = len;
    for season in 0..HISTORY_SEASONS {
        for county in 0..HISTORY_COUNTIES {
            history.entries[season][county] =
                HistoryEntry { population: input.i32()?, happiness: input.i8()? };
        }
    }
    Ok(history)
}



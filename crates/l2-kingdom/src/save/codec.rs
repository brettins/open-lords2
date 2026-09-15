#![allow(unused_imports)]
use super::*;
use super::domain::*;
use crate::county::{ChangeReason, County, Industry, MAX_COUNTIES, MAX_INFLOW_SOURCES, MAX_NEIGHBOURS};
use crate::kingdom::{History, HistoryEntry, Kingdom, Options};
use crate::phase::{Phase, TurnMachine};
use crate::realm::{Realm, MAX_REALMS};
use crate::tables::{
    Tables, Weather, HISTORY_COUNTIES, HISTORY_SEASONS, JOB_COUNT, WEAPON_TYPE_COUNT,
};
use l2_net::canonical::{Canonical, CodecError, Decode, Encode, Reader};

pub fn ruleset_fingerprint(tables: &Tables) -> u64 {
    Canonical::hash_of(tables)
}

pub fn encode(kingdom: &Kingdom) -> Vec<u8> {
    let mut body = Canonical::recording();
    kingdom.encode(&mut body);
    let digest = body.finish();
    let bytes = digest.bytes.expect("a recording encoder keeps its bytes");

    let mut out = Vec::with_capacity(HEADER_LEN + bytes.len());
    out.extend_from_slice(&MAGIC);
    out.extend_from_slice(&VERSION.to_le_bytes());
    out.extend_from_slice(&ruleset_fingerprint(&kingdom.tables).to_le_bytes());
    out.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
    out.extend_from_slice(&bytes);
    out.extend_from_slice(&digest.hash.to_le_bytes());
    out
}

pub fn checksum(kingdom: &Kingdom) -> u64 {
    Canonical::hash_of(kingdom)
}

pub fn decode(bytes: &[u8], tables: Tables) -> Result<Kingdom, LoadError> {
    if bytes.len() < HEADER_LEN + 8 || bytes[..8] != MAGIC {
        return Err(LoadError::NotASave);
    }
    let version = u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]);
    if version != VERSION {
        return Err(LoadError::UnsupportedVersion { found: version, supported: VERSION });
    }
    let mut fingerprint = [0u8; 8];
    fingerprint.copy_from_slice(&bytes[12..20]);
    let fingerprint = u64::from_le_bytes(fingerprint);
    let supplied = ruleset_fingerprint(&tables);
    if fingerprint != supplied {
        return Err(LoadError::RulesetMismatch { save: fingerprint, supplied });
    }
    let declared =
        u32::from_le_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]) as usize;
    let actual = bytes.len() - HEADER_LEN - 8;
    if declared != actual {
        return Err(LoadError::TruncatedBody { declared, actual });
    }
    let body = &bytes[HEADER_LEN..HEADER_LEN + declared];
    let mut trailer = [0u8; 8];
    trailer.copy_from_slice(&bytes[HEADER_LEN + declared..]);
    let expected = u64::from_le_bytes(trailer);

    let mut c = Canonical::hashing();
    c.raw(body);
    let actual_hash = c.finish().hash;
    if actual_hash != expected {
        return Err(LoadError::Corrupt { expected, actual: actual_hash });
    }

    let mut reader = Reader::new(body);
    let mut kingdom = decode_kingdom(&mut reader, tables)?;
    reader.finish()?;
    kingdom.tables = tables;
    Ok(kingdom)
}


impl Encode for Kingdom {
    fn encode(&self, out: &mut Canonical) {
        out.section("clock");
        out.u32(self.county_count as u32);
        out.u8(self.season);
        out.u8(self.season_next);
        out.u8(self.season_prev);
        out.i32(self.year);
        out.i32(self.year_next);
        out.u32(self.turn_count);
        out.u8(self.turn.phase.index());
        out.u32(self.turn.step);
        out.u32(self.weather_county as u32);

        out.section("options");
        out.u8(self.options.difficulty);
        out.bool(self.options.advanced_farming);
        out.bool(self.options.armies_eat);
        out.u8(self.options.fight_humans_only_byte);
        out.bool(self.options.exploration);
        out.i32(self.options.time_limit);
        out.encode(&self.options.quirks);

        out.section("rng");
        out.encode(&self.rng);

        out.section("counties");
        out.u32(MAX_COUNTIES as u32);
        for county in &self.counties {
            county.encode(out);
        }

        out.section("realms");
        out.u32(MAX_REALMS as u32);
        for realm in &self.realms {
            realm.encode(out);
        }

        out.section("history");
        self.history.encode(out);

        out.section("campaign");
        encode_campaign(&self.campaign, out);

        out.section("diplomacy");
        encode_diplomacy(&self.diplomacy, out);
    }
}

fn encode_diplomacy(d: &crate::diplomacy::Diplomacy, out: &mut Canonical) {
    out.u32(MAX_REALMS as u32);
    out.u32(crate::diplomacy::INBOX_SLOTS as u32);
    for realm in &d.inbox {
        for slot in realm {
            out.u8(slot.from);
            out.u8(slot.kind);
            out.u8(slot.county);
            out.i32(slot.gold);
        }
    }
    out.i32(d.help_price);
    out.u8(d.help_county);
    let (state, increment) = d.dice.parts();
    out.u64(state);
    out.u64(increment);
}

fn decode_diplomacy(input: &mut Reader<'_>) -> Result<crate::diplomacy::Diplomacy, LoadError> {
    let realms = input.u32()? as usize;
    let slots = input.u32()? as usize;
    if realms != MAX_REALMS || slots != crate::diplomacy::INBOX_SLOTS {
        return Err(LoadError::Malformed(CodecError::BadTag {
            tag: realms.min(255) as u8,
            expected: "diplomacy inbox shape",
            at: input.position(),
        }));
    }
    let mut d = crate::diplomacy::Diplomacy::new(0);
    for realm in 0..MAX_REALMS {
        for slot in 0..crate::diplomacy::INBOX_SLOTS {
            d.inbox[realm][slot] = crate::diplomacy::InboxSlot {
                from: input.u8()?,
                kind: input.u8()?,
                county: input.u8()?,
                gold: input.i32()?,
            };
        }
    }
    d.help_price = input.i32()?;
    d.help_county = input.u8()?;
    let state = input.u64()?;
    let increment = input.u64()?;
    d.dice = crate::diplomacy::Dice::from_parts(state, increment);
    Ok(d)
}

fn encode_campaign(campaign: &crate::kingdom::Campaign, out: &mut Canonical) {
    out.section("units");
    out.u32(crate::unit::MAX_UNITS as u32);
    for slot in 0..crate::unit::MAX_UNITS {
        match campaign.units.get(slot) {
            None => out.bool(false),
            Some(u) => {
                out.bool(true);
                u.encode(out);
            }
        }
    }

    out.section("map");
    out.u32(crate::map::MAP_TILES as u32);
    out.raw(&campaign.map.terrain);
    out.raw(&campaign.map.flags);
    out.raw(&campaign.map.bank);
    out.raw(&campaign.map.county);

    out.section("mercenaries");
    out.u32(campaign.mercenaries.in_play() as u32);
    for band in 1..crate::mercenary::BAND_SLOTS {
        let b = campaign.mercenaries.band_raw(band);
        out.u16(b.hired_by);
        out.u8(b.offered_in);
        out.u8(b.next_county);
        out.i8(b.countdown);
        out.i8(b.reload);
    }

    out.section("army_names");
    for realm in 0..MAX_REALMS {
        out.raw(campaign.names.counters(realm as u8));
    }

    out.section("routes");
    for route in 0..crate::merchant::ROUTES {
        out.raw(campaign.routes.row(route));
    }
    out.u32(campaign.mob_cursor as u32);

    // `Net_WriteField(&DAT_005653F8, 4); Net_WriteField(&DAT_0057CAE0, 0x30);`
    // — `FUN_00444A2F` sends both halves at every battle start, so both are
    // state a peer that loads must agree on. Version 31.
    out.section("field_playlist");
    out.raw(campaign.field_playlist.frames());
    out.u32(campaign.field_playlist.cursor() as u32);

    out.section("explored");
    campaign.explored.encode(out);
}

fn decode_campaign(input: &mut Reader<'_>) -> Result<crate::kingdom::Campaign, LoadError> {
    let mut campaign = crate::kingdom::Campaign::new();

    let slots = input.u32()? as usize;
    if slots != crate::unit::MAX_UNITS {
        return Err(LoadError::UnitCount(slots as u32));
    }
    for slot in 0..crate::unit::MAX_UNITS {
        if input.bool()? {
            campaign.units.put(slot, crate::unit::Unit::decode(input)?);
        }
    }

    let tiles = input.u32()? as usize;
    if tiles != crate::map::MAP_TILES {
        return Err(LoadError::MapSize(tiles as u32));
    }
    for plane in [0usize, 1, 2, 3] {
        let bytes = input.raw(crate::map::MAP_TILES)?;
        let target = match plane {
            0 => &mut campaign.map.terrain,
            1 => &mut campaign.map.flags,
            2 => &mut campaign.map.bank,
            _ => &mut campaign.map.county,
        };
        target.copy_from_slice(bytes);
    }

    let in_play = input.u32()? as usize;
    if in_play > crate::mercenary::MERCENARY_BANDS {
        return Err(LoadError::BandCount(in_play as u32));
    }
    campaign.mercenaries.set_in_play(in_play);
    for band in 1..crate::mercenary::BAND_SLOTS {
        let b = crate::mercenary::Band {
            hired_by: input.u16()?,
            offered_in: input.u8()?,
            next_county: input.u8()?,
            countdown: input.i8()?,
            reload: input.i8()?,
        };
        campaign.mercenaries.set_band_raw(band, b);
    }

    for realm in 0..MAX_REALMS {
        let bytes = input.raw(crate::unit::ARMY_NAME_SLOTS)?;
        let mut row = [0u8; crate::unit::ARMY_NAME_SLOTS];
        row.copy_from_slice(bytes);
        campaign.names.set_counters(realm as u8, row);
    }

    for route in 0..crate::merchant::ROUTES {
        let bytes = input.raw(crate::merchant::ROUTE_SLOTS)?;
        let mut row = [0u8; crate::merchant::ROUTE_SLOTS];
        row.copy_from_slice(bytes);
        campaign.routes.set_row(route, row);
    }
    campaign.mob_cursor = input.u32()? as usize;

    let frames = input.raw(crate::field_playlist::PLAYLIST_LEN)?;
    let mut order = [0u8; crate::field_playlist::PLAYLIST_LEN];
    order.copy_from_slice(frames);
    let cursor = input.u32()? as usize;
    campaign.field_playlist = crate::field_playlist::FieldPlaylist::restore(order, cursor);

    let seen = input.raw(crate::map::MAP_TILES)?;
    campaign.explored.copy_from_bytes(seen);
    Ok(campaign)
}

fn decode_kingdom(input: &mut Reader<'_>, tables: Tables) -> Result<Kingdom, LoadError> {
    let county_count = input.u32()?;
    if county_count as usize >= MAX_COUNTIES {
        return Err(LoadError::CountyCount(county_count));
    }
    let mut k = Kingdom::with_tables(0, tables);
    k.county_count = county_count as usize;
    k.season = input.u8()?;
    k.season_next = input.u8()?;
    k.season_prev = input.u8()?;
    k.year = input.i32()?;
    k.year_next = input.i32()?;
    k.turn_count = input.u32()?;
    let phase = input.u8()?;
    k.turn = TurnMachine {
        phase: Phase::from_index(phase).ok_or(CodecError::BadTag {
            tag: phase,
            expected: "turn phase",
            at: input.position() - 1,
        })?,
        step: input.u32()?,
        players_turn_open: false,
    };
    k.weather_county = input.u32()? as usize;

    k.options = Options {
        difficulty: input.u8()?,
        advanced_farming: input.bool()?,
        armies_eat: input.bool()?,
        fight_humans_only_byte: input.u8()?,
        exploration: input.bool()?,
        time_limit: input.i32()?,
        quirks: input.decode()?,
    };
    k.rng = input.decode()?;

    let counties = input.u32()? as usize;
    if counties != MAX_COUNTIES {
        return Err(LoadError::CountyCount(counties as u32));
    }
    for slot in 0..MAX_COUNTIES {
        k.counties[slot] = County::decode(input)?;
    }

    let realms = input.u32()? as usize;
    if realms != MAX_REALMS {
        return Err(LoadError::Malformed(CodecError::BadTag {
            tag: realms.min(255) as u8,
            expected: "realm count",
            at: input.position(),
        }));
    }
    for slot in 0..MAX_REALMS {
        k.realms[slot] = Realm::decode(input)?;
    }

    k.history = decode_history(input)?;
    k.campaign = decode_campaign(input)?;
    k.diplomacy = decode_diplomacy(input)?;
    Ok(k)
}


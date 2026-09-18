#![allow(unused_imports)]
use super::*;
use super::unit::*;
use super::county::*;
use super::industry::*;
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

impl Encode for Realm {
    fn encode(&self, out: &mut Canonical) {
        out.i32(self.ai_step);
        out.bool(self.in_play);
        out.u8(self.strength);
        out.bool(self.is_human);
        out.u8(self.lord);
        // Realm `+0x0A`, the banner. It was missing from this codec until the
        // campaign layer arrived and `tests/campaign.rs` caught it: no rule in
        // the economy reads it, so nothing noticed, and `County_ChangeOwner`
        // reads it now — a captured county draws its new owner's shield.
        out.u8(self.shield_index);
        out.i8(self.tax_hap_empire);
        out.u8(self.county_count);
        out.u8(self.rank);
        out.i32(self.score);
        out.i32(self.wages);
        out.i32(self.gold);
        out.i32(self.iron);
        out.i32(self.stone);
        out.i32(self.wood);
        for weapon in &self.weapons {
            out.i32(*weapon);
        }
        out.u8(self.bankrupt_stage);
        out.i32(self.trade_spent_a);
        out.i32(self.trade_spent_b);
        out.i32(self.trade_received_a);
        out.i32(self.trade_received_b);
        for v in &self.tax_ledger {
            out.i32(*v);
        }
        out.i32(self.weapon_rota);
        out.i32(self.population_total);
        out.i32(self.population_last);
        out.i32(self.population_mean);
        out.i32(self.mean_happiness);
        out.i32(self.mean_health);
        out.i32(self.share_of_map_pct);
        out.u8(self.army_count);
        out.i32(self.total_men);
        for input in &self.score_inputs {
            out.i32(*input);
        }

        out.bool(self.offer_pending);
        out.u8(self.ally_candidate);
        out.u8(self.ally);
        for pair in &self.pairs {
            pair.encode(out);
        }
        out.u8(self.target_county);
        out.u8(self.taunt_timer);
        out.u8(self.taunt_stage);
        out.u8(self.war_target);
        out.i8(self.offer_timer);
        out.bool(self.crowned_once);
        out.u8(self.voice_rotation);

        out.u8(self.muster_county);
        out.u8(self.raid_county);
        out.u8(self.muster_timer);
        out.u8(self.threat_realm);
        out.u8(self.attack_county);
        out.u8(self.raid_timer);
        for want in &self.want {
            out.i32(*want);
        }

        // Realm `+0x2A` (`VERSION` 24) — the most counties ever held, which
        // `County_ChangeOwner` reads to choose the capture letter.
        out.u8(self.peak_counties);
    }
}

impl Encode for crate::realm::Pair {
    fn encode(&self, out: &mut Canonical) {
        out.i8(self.standing);
        out.bool(self.allied);
        out.u8(self.grudge);
        out.u8(self.warnings_sent);
        out.bool(self.at_war);
        out.u8(self.compliments_from);
        out.i32(self.best_gift);
        out.bool(self.has_mail);
        out.u8(self.help_price_multiple);
    }
}

impl Decode for crate::realm::Pair {
    fn decode(input: &mut Reader<'_>) -> Result<crate::realm::Pair, CodecError> {
        Ok(crate::realm::Pair {
            standing: input.i8()?,
            allied: input.bool()?,
            grudge: input.u8()?,
            warnings_sent: input.u8()?,
            at_war: input.bool()?,
            compliments_from: input.u8()?,
            best_gift: input.i32()?,
            has_mail: input.bool()?,
            help_price_multiple: input.u8()?,
        })
    }
}

impl Decode for Realm {
    fn decode(input: &mut Reader<'_>) -> Result<Realm, CodecError> {
        let mut r = Realm::new();
        r.ai_step = input.i32()?;
        r.in_play = input.bool()?;
        r.strength = input.u8()?;
        r.is_human = input.bool()?;
        r.lord = input.u8()?;
        r.shield_index = input.u8()?;
        r.tax_hap_empire = input.i8()?;
        r.county_count = input.u8()?;
        r.rank = input.u8()?;
        r.score = input.i32()?;
        r.wages = input.i32()?;
        r.gold = input.i32()?;
        r.iron = input.i32()?;
        r.stone = input.i32()?;
        r.wood = input.i32()?;
        for slot in 0..WEAPON_TYPE_COUNT {
            r.weapons[slot] = input.i32()?;
        }
        r.bankrupt_stage = input.u8()?;
        r.trade_spent_a = input.i32()?;
        r.trade_spent_b = input.i32()?;
        r.trade_received_a = input.i32()?;
        r.trade_received_b = input.i32()?;
        for slot in 0..r.tax_ledger.len() {
            r.tax_ledger[slot] = input.i32()?;
        }
        r.weapon_rota = input.i32()?;
        r.population_total = input.i32()?;
        r.population_last = input.i32()?;
        r.population_mean = input.i32()?;
        r.mean_happiness = input.i32()?;
        r.mean_health = input.i32()?;
        r.share_of_map_pct = input.i32()?;
        r.army_count = input.u8()?;
        r.total_men = input.i32()?;
        for slot in 0..r.score_inputs.len() {
            r.score_inputs[slot] = input.i32()?;
        }
        r.offer_pending = input.bool()?;
        r.ally_candidate = input.u8()?;
        r.ally = input.u8()?;
        for slot in 0..MAX_REALMS {
            r.pairs[slot] = crate::realm::Pair::decode(input)?;
        }
        r.target_county = input.u8()?;
        r.taunt_timer = input.u8()?;
        r.taunt_stage = input.u8()?;
        r.war_target = input.u8()?;
        r.offer_timer = input.i8()?;
        r.crowned_once = input.bool()?;
        r.voice_rotation = input.u8()?;
        r.muster_county = input.u8()?;
        r.raid_county = input.u8()?;
        r.muster_timer = input.u8()?;
        r.threat_realm = input.u8()?;
        r.attack_county = input.u8()?;
        r.raid_timer = input.u8()?;
        for slot in 0..r.want.len() {
            r.want[slot] = input.i32()?;
        }
        r.peak_counties = input.u8()?;
        Ok(r)
    }
}


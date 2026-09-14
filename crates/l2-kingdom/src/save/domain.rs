#![allow(unused_imports)]
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

impl Encode for County {
    fn encode(&self, out: &mut Canonical) {
        out.bool(self.event_fired);
        out.u16(self.event_id);
        out.u8(self.owner);
        out.u8(self.health_band);
        out.i32(self.health_meter);
        out.i32(self.happiness);
        out.i32(self.happiness_last);
        out.i32(self.d_hap_tax);
        out.i32(self.d_hap_tax_local);
        out.i32(self.d_hap_health);
        out.i32(self.d_hap_ration);
        out.i32(self.shown_tax);
        out.i32(self.shown_ration);
        out.i32(self.shown_health);
        out.i32(self.shown_army);
        out.i32(self.tax_hap_other);
        out.i32(self.shown_events);
        out.i32(self.happiness_avg);
        out.i32(self.happiness_sum);
        out.i32(self.shown_ale);
        out.i32(self.ale_happiness_given);
        out.u8(self.unrest);
        out.bool(self.unrest_warned);

        out.i32(self.population);
        out.i32(self.pop_last);
        out.i32(self.pop_change_pct);
        out.i32(self.births);
        out.i32(self.deaths);
        out.i32(self.army);
        out.i32(self.emigrants);
        out.i32(self.immigrants);
        out.i32(self.largest_inflow);
        out.u8(self.emigrant_destination);
        out.u8(self.largest_inflow_source);
        out.raw(&self.inflow_sources);
        out.u8(self.neighbour_count);
        out.raw(&self.neighbours);
        out.u8(self.change_reason as u8);
        out.i32(self.pop_band);
        out.u8(self.anchor_x);
        out.u8(self.anchor_y);

        out.i32(self.tax_rate);
        out.i32(self.tax_collected);
        out.i32(self.tax_shown);
        out.i32(self.purse);
        out.i32(self.merchant_count);
        out.u8(self.merchant_unit);
        out.i32(self.merchant_visits);
        for job in &self.labour {
            out.i32(*job);
        }
        // **The other three labour arrays and the industry split.** They were
        // missing until a game save round-tripped the England position and came
        // back with `County::new`'s defaults in all four; see [`VERSION`] 5.
        // `labour_useful` and `labour_share` are what `FUN_0044F6E7` allocates
        // *from*, so they are simulation state and not a display hint, and
        // leaving them out of the encoding left them out of the lockstep
        // checksum too.
        for job in &self.labour_wanted {
            out.i32(*job);
        }
        for job in &self.labour_useful {
            out.i32(*job);
        }
        for job in &self.labour_share {
            out.i32(*job);
        }
        out.i32(self.industry_share);
        for field in &self.field_progress {
            out.u16(*field);
        }
        out.i32(self.ration_achieved);
        out.i32(self.ration_wanted);
        out.i32(self.ration_split);
        out.i32(self.grain_eaten);
        out.i32(self.herd_eaten);
        out.i32(self.grain_available);
        out.i32(self.herd_available);
        out.i32(self.friendly_troops);
        out.i32(self.enemy_troops);
        out.u8(self.mercenary_offer);
        out.u32(self.garrison_unit as u32);
        out.i32(self.levy_surcharge);
        out.u8(self.castle_type);
        out.u8(self.castle_building);
        out.u8(self.castle_degraded);
        out.bool(self.castle_ruined);
        out.u8(self.castle_level_left);
        // The scars, `VERSION` 15.
        out.u16(self.siege_scars.moat_filled);
        out.u16(self.siege_scars.wall_damage);
        out.i32(self.siege_scars.breach_score);
        out.i32(self.siege_scars.approach_score);
        out.u8(self.siege_scars.ramparts_breached);
        out.bool(self.siege_scars.gate_open);
        out.bool(self.castle_switch);
        out.u8(self.castle_percent);
        out.i32(self.castle_work_left);
        out.i32(self.castle_work_total);
        out.i32(self.castle_stone_owed);
        out.i32(self.castle_stone_total);
        out.i32(self.castle_wood_owed);
        out.i32(self.castle_wood_total);
        out.i32(self.event_population_pct);
        // The letter's figure, `VERSION` 20.
        out.i32(self.event_population_swing);
        out.i32(self.event_grain_pct);
        out.i32(self.event_herd_pct);
        for tile in &self.field_tiles {
            out.u16(*tile);
        }
        // The two round-robin field cursors, `VERSION` 27.
        out.u8(self.pasture_cursor);
        out.u8(self.blight_cursor);
        out.i32(self.fields_fallow);
        out.i32(self.fields_cattle);
        out.i32(self.fields_grain);
        out.i32(self.fields_waste);
        out.i32(self.fields_reclaiming);
        out.i32(self.fertility);
        out.u8(self.weather.index());
        out.i32(self.dryness);
        out.i32(self.grain);
        for stage in &self.crop {
            out.i32(*stage);
        }
        out.i32(self.fields_grain_sown);
        out.i32(self.fields_grain_standing);
        out.bool(self.sow_shortfall);
        out.i32(self.herd);
        out.i32(self.herd_crowding);
        out.i32(self.herd_births_expected);
        out.i32(self.herd_deaths_expected);
        out.i32(self.herd_change_expected);
        out.i32(self.grain_weather_change);
        out.i32(self.grain_event_change);
        out.i32(self.herd_weather_change);
        out.i32(self.herd_event_change);
        out.i32(self.grain_sown_expected);
        out.i32(self.grain_grown_expected);
        out.i32(self.grain_change_expected);
        out.i32(self.reclaim_fields_finishing);
        out.i32(self.reclaim_seasons_to_next);
        for industry in &self.industry {
            industry.encode(out);
        }
        out.u32(self.weapon_type as u32);
        out.u8(self.farm_style);
        out.bool(self.tax_suppressed);
    }
}

impl Decode for County {
    fn decode(input: &mut Reader<'_>) -> Result<County, CodecError> {
        let mut c = County::new();
        c.event_fired = input.bool()?;
        c.event_id = input.u16()?;
        c.owner = input.u8()?;
        c.health_band = input.u8()?;
        c.health_meter = input.i32()?;
        c.happiness = input.i32()?;
        c.happiness_last = input.i32()?;
        c.d_hap_tax = input.i32()?;
        c.d_hap_tax_local = input.i32()?;
        c.d_hap_health = input.i32()?;
        c.d_hap_ration = input.i32()?;
        c.shown_tax = input.i32()?;
        c.shown_ration = input.i32()?;
        c.shown_health = input.i32()?;
        c.shown_army = input.i32()?;
        c.tax_hap_other = input.i32()?;
        c.shown_events = input.i32()?;
        c.happiness_avg = input.i32()?;
        c.happiness_sum = input.i32()?;
        c.shown_ale = input.i32()?;
        c.ale_happiness_given = input.i32()?;
        c.unrest = input.u8()?;
        c.unrest_warned = input.bool()?;

        c.population = input.i32()?;
        c.pop_last = input.i32()?;
        c.pop_change_pct = input.i32()?;
        c.births = input.i32()?;
        c.deaths = input.i32()?;
        c.army = input.i32()?;
        c.emigrants = input.i32()?;
        c.immigrants = input.i32()?;
        c.largest_inflow = input.i32()?;
        c.emigrant_destination = input.u8()?;
        c.largest_inflow_source = input.u8()?;
        c.inflow_sources.copy_from_slice(input.raw(MAX_INFLOW_SOURCES)?);
        c.neighbour_count = input.u8()?;
        c.neighbours.copy_from_slice(input.raw(MAX_NEIGHBOURS)?);
        let at = input.position();
        c.change_reason = match input.u8()? {
            0 => ChangeReason::None,
            1 => ChangeReason::Births,
            2 => ChangeReason::Deaths,
            3 => ChangeReason::Emigration,
            4 => ChangeReason::Immigration,
            tag => return Err(CodecError::BadTag { tag, expected: "change reason", at }),
        };
        c.pop_band = input.i32()?;
        c.anchor_x = input.u8()?;
        c.anchor_y = input.u8()?;

        c.tax_rate = input.i32()?;
        c.tax_collected = input.i32()?;
        c.tax_shown = input.i32()?;
        c.purse = input.i32()?;
        c.merchant_count = input.i32()?;
        c.merchant_unit = input.u8()?;
        c.merchant_visits = input.i32()?;
        for job in 0..JOB_COUNT {
            c.labour[job] = input.i32()?;
        }
        for job in 0..JOB_COUNT {
            c.labour_wanted[job] = input.i32()?;
        }
        for job in 0..JOB_COUNT {
            c.labour_useful[job] = input.i32()?;
        }
        for job in 0..JOB_COUNT - 1 {
            c.labour_share[job] = input.i32()?;
        }
        c.industry_share = input.i32()?;
        for field in 0..c.field_progress.len() {
            c.field_progress[field] = input.u16()?;
        }
        c.ration_achieved = input.i32()?;
        c.ration_wanted = input.i32()?;
        c.ration_split = input.i32()?;
        c.grain_eaten = input.i32()?;
        c.herd_eaten = input.i32()?;
        c.grain_available = input.i32()?;
        c.herd_available = input.i32()?;
        c.friendly_troops = input.i32()?;
        c.enemy_troops = input.i32()?;
        c.mercenary_offer = input.u8()?;
        c.garrison_unit = input.u32()? as usize;
        c.levy_surcharge = input.i32()?;
        c.castle_type = input.u8()?;
        c.castle_building = input.u8()?;
        c.castle_degraded = input.u8()?;
        c.castle_ruined = input.bool()?;
        c.castle_level_left = input.u8()?;
        // The scars, `VERSION` 15.
        c.siege_scars.moat_filled = input.u16()?;
        c.siege_scars.wall_damage = input.u16()?;
        c.siege_scars.breach_score = input.i32()?;
        c.siege_scars.approach_score = input.i32()?;
        c.siege_scars.ramparts_breached = input.u8()?;
        c.siege_scars.gate_open = input.bool()?;
        c.castle_switch = input.bool()?;
        c.castle_percent = input.u8()?;
        c.castle_work_left = input.i32()?;
        c.castle_work_total = input.i32()?;
        c.castle_stone_owed = input.i32()?;
        c.castle_stone_total = input.i32()?;
        c.castle_wood_owed = input.i32()?;
        c.castle_wood_total = input.i32()?;
        c.event_population_pct = input.i32()?;
        c.event_population_swing = input.i32()?;
        c.event_grain_pct = input.i32()?;
        c.event_herd_pct = input.i32()?;
        for slot in 0..c.field_tiles.len() {
            c.field_tiles[slot] = input.u16()?;
        }
        c.pasture_cursor = input.u8()?;
        c.blight_cursor = input.u8()?;
        c.fields_fallow = input.i32()?;
        c.fields_cattle = input.i32()?;
        c.fields_grain = input.i32()?;
        c.fields_waste = input.i32()?;
        c.fields_reclaiming = input.i32()?;
        c.fertility = input.i32()?;
        let at = input.position();
        let byte = input.u8()?;
        c.weather = Weather::from_index(byte)
            .ok_or(CodecError::BadTag { tag: byte, expected: "weather", at })?;
        c.dryness = input.i32()?;
        c.grain = input.i32()?;
        for stage in 0..c.crop.len() {
            c.crop[stage] = input.i32()?;
        }
        c.fields_grain_sown = input.i32()?;
        c.fields_grain_standing = input.i32()?;
        c.sow_shortfall = input.bool()?;
        c.herd = input.i32()?;
        c.herd_crowding = input.i32()?;
        c.herd_births_expected = input.i32()?;
        c.herd_deaths_expected = input.i32()?;
        c.herd_change_expected = input.i32()?;
        c.grain_weather_change = input.i32()?;
        c.grain_event_change = input.i32()?;
        c.herd_weather_change = input.i32()?;
        c.herd_event_change = input.i32()?;
        c.grain_sown_expected = input.i32()?;
        c.grain_grown_expected = input.i32()?;
        c.grain_change_expected = input.i32()?;
        c.reclaim_fields_finishing = input.i32()?;
        c.reclaim_seasons_to_next = input.i32()?;
        for slot in 0..c.industry.len() {
            c.industry[slot] = Industry::decode(input)?;
        }
        c.weapon_type = input.u32()? as usize;
        c.farm_style = input.u8()?;
        c.tax_suppressed = input.bool()?;
        Ok(c)
    }
}

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

        // The diplomacy record (`docs/diplomacy.md` §1). It landed on `main`
        // outside this file and was absent from the encoding — and therefore
        // from the lockstep digest — until the census below found it. See
        // [`VERSION`] 8.
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

        // The war plan (`VERSION` 14). Seven fields the AI writes in steps 7,
        // 9 and 10 and reads again next turn, plus the four resource wants
        // step 4 fills. A realm that reloaded without them would forget which
        // county it musters from, restart every lord's muster and raid
        // counters at zero, and lose the county its main army is marching on.
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

impl Encode for History {
    /// All four hundred slots, not only the live ones.
    ///
    /// The ring is 400 × 16 × `{i32, i8}` and writing it whole costs 32,000
    /// bytes. Writing only the live window would be smaller and would need the
    /// reader to reconstruct which slots the writer considered empty — an
    /// invariant restated in a second place, which is how the two drift apart.
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

pub(super) fn decode_history(input: &mut Reader<'_>) -> Result<History, LoadError> {
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

// ---------------------------------------------------------------------------
// The ruleset fingerprint
// ---------------------------------------------------------------------------

/// The whole of [`Tables`], written out so it can be hashed.
///
/// Encode only: the save does not carry a ruleset, it carries this hash. Every
/// field is here, and `tests/save.rs` mutates each sub-table in turn to check
/// that the hash notices — which is the guard against a constant being added to
/// `Tables` and quietly left out of the fingerprint.
impl Encode for Tables {
    fn encode(&self, out: &mut Canonical) {
        out.section("food");
        out.i32(self.food.dairy_per_head);
        out.i32(self.food.food_per_head);
        out.i32(self.food.food_per_sack);

        out.section("grain");
        out.i32(self.grain.yield_per_sack);
        out.i32(self.grain.max_sacks_per_field);
        out.i32(self.grain.labour_divisor_advanced);
        out.i32(self.grain.labour_divisor_basic);

        out.section("field");
        out.i32(self.field.progress_max);
        out.i32(self.field.reclaim_per_season);

        out.section("event");
        out.i32(self.event.population_cap_pct);
        out.i32(self.event.first_year);

        out.section("season");
        for row in &self.season {
            out.i32(row.death_rate);
            out.i32(row.dryness);
        }

        out.section("ration");
        for row in &self.ration {
            out.i32(row.divisor);
            out.i32(row.multiplier);
            for delta in &row.health_delta {
                out.i32(*delta);
            }
        }
        out.i32(self.ration_happiness_slope);
        out.i32(self.ration_happiness_offset);

        out.section("health");
        for row in &self.health {
            out.i32(row.happiness);
            out.i32(row.death_rate);
        }
        for (threshold, band) in &self.health_band_ladder {
            out.i32(*threshold);
            out.i32(*band);
        }

        out.section("population");
        for (up_to, percent) in &self.population.birth_rate_ladder {
            out.i32(*up_to);
            out.i32(*percent);
        }
        for (below, percent) in &self.population.happiness_factor_ladder {
            out.i32(*below);
            out.i32(*percent);
        }

        out.section("tax");
        for v in &self.tax_happiness_other {
            out.i32(*v);
        }

        out.section("weather");
        for row in &self.weather {
            out.i32(row.herd_pct);
        }

        out.section("herd");
        out.i32(self.herd.labour_per_head);
        out.i32(self.herd.staffing_max);
        out.i32(self.herd.understaffing_divisor);
        for row in &self.herd.crowding {
            out.i32(row.density_max);
            out.i32(row.level);
            out.i32(row.death_rate);
            out.i32(row.birth_rate);
        }
        for (below, bonus) in &self.herd.small_bonus {
            out.i32(*below);
            out.i32(*bonus);
        }
        out.i32(self.herd.no_pasture_density);
        out.i32(self.herd.no_pasture_kill_all_below);
        out.i32(self.herd.no_pasture_divisor);
        out.u8(self.herd.calving_season);
        out.u8(self.herd.culling_season);
        out.i32(self.herd.season_bonus.0);
        out.i32(self.herd.season_bonus.1);

        out.section("castle");
        out.u8(self.castle.starting_type);
        for v in &self.castle.tax_base {
            out.i32(*v);
        }
        for v in &self.castle.tax_bonus_pct {
            out.i32(*v);
        }
        for (wood, stone) in &self.castle.cost {
            out.i32(*wood);
            out.i32(*stone);
        }
        for (a, b) in &self.castle.workforce {
            out.i32(*a);
            out.i32(*b);
        }
        for v in &self.castle.garrison_cap {
            out.i32(*v);
        }
        for v in &self.castle.free_archers {
            out.i32(*v);
        }

        out.section("commodity");
        for row in &self.commodity {
            out.u32(row.job as u32);
            out.i32(row.divisor);
            out.i32(row.base_efficiency);
        }

        out.section("job");
        out.u32(self.job.count as u32);
        out.u32(self.job.iron_mining as u32);
        out.u32(self.job.stone_quarrying as u32);
        out.u32(self.job.wood_cutting as u32);
        out.u32(self.job.blacksmith as u32);
        out.u32(self.job.grain_farming as u32);
        out.u32(self.job.cattle_farming as u32);
        out.u32(self.job.castle_building as u32);

        out.section("weapon");
        for row in &self.weapon {
            out.i32(row.wood);
            out.i32(row.iron);
        }

        out.section("good");
        for row in &self.good {
            out.i32(row.sell_price);
        }

        out.section("wages");
        out.i32(self.wages.divisor_human);
        for v in &self.wages.divisor_ai {
            out.i32(*v);
        }
        out.u8(self.wages.bankrupt_stage_max);

        out.section("efficiency");
        out.i32(self.efficiency.max);
        out.i32(self.efficiency.without_advanced_farming);

        out.section("ale");
        out.i32(self.ale.step_pct);
        out.i32(self.ale.max);

        out.section("army_happiness");
        for v in &self.army_happiness_cost {
            out.i32(*v);
        }

        out.section("ai");
        for row in &self.ai.gold_grant {
            for v in row {
                out.i32(*v);
            }
        }
        out.i32(self.ai.grant_population_per_difficulty);
        out.i32(self.ai.grant_herd_per_difficulty);
        out.i32(self.ai.grant_grain_per_difficulty);
        out.i32(self.ai.grant_min_population);
        out.i32(self.ai.grant_min_herd);
        out.i32(self.ai.grant_min_grain);
        for (threshold, rate) in &self.ai.tax_ladder_neutral {
            out.i32(*threshold);
            out.i32(*rate);
        }
        for ladder in &self.ai.tax_ladders {
            for (threshold, rate) in ladder {
                out.i32(*threshold);
                out.i32(*rate);
            }
        }
        for row in &self.ai.personality {
            out.u8(row.farm_style);
            out.u32(row.tax_ladder as u32);
        }

        out.section("score");
        for (at_least, points) in &self.score.gold_brackets {
            out.i32(*at_least);
            out.i32(*points);
        }
        for (numerator, denominator) in &self.score.weights {
            out.i32(*numerator);
            out.i32(*denominator);
        }
        for offset in &self.score.input_offsets {
            out.u16(*offset);
        }
    }
}


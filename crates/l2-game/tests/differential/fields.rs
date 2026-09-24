//! The field tables: what is compared, and why a field is left out.

use l2_formats::save::{County as SavedCounty, DiploPair, Globals, Realm as SavedRealm};
use l2_game::game::Game;
use l2_kingdom::county::County;
use l2_kingdom::realm::{Pair as OurPair, Realm};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Kind {
    Simulated,
    PlayerInput,
    Unsimulated,
    Excluded(&'static str),
}

impl Kind {
    pub fn tag(self) -> &'static str {
        match self {
            Kind::Simulated => "simulated",
            Kind::PlayerInput => "player-input",
            Kind::Unsimulated => "unsimulated",
            Kind::Excluded(_) => "excluded",
        }
    }

    pub fn compared(self) -> bool {
        !matches!(self, Kind::Excluded(_))
    }
}

pub struct CountyField {
    pub path: &'static str,
    pub kind: Kind,
    pub ours: fn(&County) -> i64,
    pub theirs: fn(&SavedCounty) -> i64,
}

pub struct RealmField {
    pub path: &'static str,
    pub kind: Kind,
    pub ours: fn(&Realm) -> i64,
    pub theirs: fn(&SavedRealm) -> i64,
}

pub struct PairField {
    pub path: &'static str,
    pub kind: Kind,
    pub ours: fn(&OurPair) -> i64,
    pub theirs: fn(&DiploPair) -> i64,
}

pub struct GlobalField {
    pub path: &'static str,
    pub kind: Kind,
    pub ours: fn(&Game) -> i64,
    pub theirs: fn(&Globals) -> i64,
}

#[rustfmt::skip]
pub const COUNTY_FIELDS: &[CountyField] = &[
    CountyField { path: "county.owner", kind: Kind::Simulated, ours: |c| c.owner as i64, theirs: |c| c.owner as i64 },
    CountyField { path: "county.happiness", kind: Kind::Simulated, ours: |c| c.happiness as i64, theirs: |c| c.happiness as i64 },
    CountyField { path: "county.happiness_last", kind: Kind::Simulated, ours: |c| c.happiness_last as i64, theirs: |c| c.happiness_last as i64 },
    CountyField { path: "county.d_hap_tax", kind: Kind::Simulated, ours: |c| c.d_hap_tax as i64, theirs: |c| c.d_hap_tax as i64 },
    CountyField { path: "county.d_hap_health", kind: Kind::Simulated, ours: |c| c.d_hap_health as i64, theirs: |c| c.d_hap_health as i64 },
    CountyField { path: "county.d_hap_ration", kind: Kind::Simulated, ours: |c| c.d_hap_ration as i64, theirs: |c| c.d_hap_ration as i64 },
    CountyField { path: "county.shown_tax", kind: Kind::Simulated, ours: |c| c.shown_tax as i64, theirs: |c| c.shown_tax as i64 },
    CountyField { path: "county.shown_ration", kind: Kind::Simulated, ours: |c| c.shown_ration as i64, theirs: |c| c.shown_ration as i64 },
    CountyField { path: "county.shown_health", kind: Kind::Simulated, ours: |c| c.shown_health as i64, theirs: |c| c.shown_health as i64 },
    CountyField { path: "county.shown_army", kind: Kind::Simulated, ours: |c| c.shown_army as i64, theirs: |c| c.shown_army as i64 },
    CountyField { path: "county.shown_events", kind: Kind::Simulated, ours: |c| c.shown_events as i64, theirs: |c| c.shown_events as i64 },
    // `+0x18` and `+0x1C` are a running mean and its accumulator over the
    // **whole game**, and `l2-scenario` seeds both from the current happiness
    // because the save stores no sample count to resume from. A pair twelve
    // turns in therefore cannot agree, and the divergence would say nothing
    // about the rule.
    CountyField { path: "county.happiness_avg", kind: Kind::Excluded("a running mean over the whole game; the save stores no sample count, so the import restarts it"), ours: |c| c.happiness_avg as i64, theirs: |c| c.happiness_avg as i64 },
    CountyField { path: "county.happiness_sum", kind: Kind::Excluded("the accumulator behind happiness_avg, restarted by the import for the same reason"), ours: |c| c.happiness_sum as i64, theirs: |c| c.happiness_sum as i64 },
    CountyField { path: "county.health_meter", kind: Kind::Simulated, ours: |c| c.health_meter as i64, theirs: |c| c.health_meter as i64 },
    CountyField { path: "county.health_band", kind: Kind::Simulated, ours: |c| c.health_band as i64, theirs: |c| c.health_band as i64 },
    CountyField { path: "county.unrest", kind: Kind::Simulated, ours: |c| c.unrest as i64, theirs: |c| c.unrest as i64 },
    CountyField { path: "county.population", kind: Kind::Simulated, ours: |c| c.population as i64, theirs: |c| c.population as i64 },
    CountyField { path: "county.pop_last", kind: Kind::Simulated, ours: |c| c.pop_last as i64, theirs: |c| c.pop_last as i64 },
    CountyField { path: "county.births", kind: Kind::Simulated, ours: |c| c.births as i64, theirs: |c| c.births as i64 },
    CountyField { path: "county.deaths", kind: Kind::Simulated, ours: |c| c.deaths as i64, theirs: |c| c.deaths as i64 },
    CountyField { path: "county.emigrants", kind: Kind::Simulated, ours: |c| c.emigrants as i64, theirs: |c| c.emigrants as i64 },
    CountyField { path: "county.immigrants", kind: Kind::Simulated, ours: |c| c.immigrants as i64, theirs: |c| c.immigrants as i64 },
    CountyField { path: "county.pop_band", kind: Kind::Simulated, ours: |c| c.pop_band as i64, theirs: |c| c.pop_band as i64 },
    CountyField { path: "county.neighbour_count", kind: Kind::Simulated, ours: |c| c.neighbour_count as i64, theirs: |c| c.neighbour_count as i64 },
    CountyField { path: "county.anchor_x", kind: Kind::Simulated, ours: |c| c.anchor_x as i64, theirs: |c| c.anchor_x as i64 },
    CountyField { path: "county.anchor_y", kind: Kind::Simulated, ours: |c| c.anchor_y as i64, theirs: |c| c.anchor_y as i64 },
    CountyField { path: "county.tax_rate", kind: Kind::PlayerInput, ours: |c| c.tax_rate as i64, theirs: |c| c.tax_rate as i64 },
    CountyField { path: "county.tax_collected", kind: Kind::Simulated, ours: |c| c.tax_collected as i64, theirs: |c| c.tax_collected as i64 },
    CountyField { path: "county.ration_wanted", kind: Kind::PlayerInput, ours: |c| c.ration_wanted as i64, theirs: |c| c.ration_wanted as i64 },
    CountyField { path: "county.ration_split", kind: Kind::PlayerInput, ours: |c| c.ration_split as i64, theirs: |c| c.ration_split as i64 },
    CountyField { path: "county.ration_achieved", kind: Kind::Simulated, ours: |c| c.ration_achieved as i64, theirs: |c| c.ration_achieved as i64 },
    CountyField { path: "county.grain_eaten", kind: Kind::Simulated, ours: |c| c.grain_eaten as i64, theirs: |c| c.grain_eaten as i64 },
    CountyField { path: "county.herd_eaten", kind: Kind::Simulated, ours: |c| c.herd_eaten as i64, theirs: |c| c.herd_eaten as i64 },
    CountyField { path: "county.grain_available", kind: Kind::Simulated, ours: |c| c.grain_available as i64, theirs: |c| c.grain_available as i64 },
    CountyField { path: "county.herd_available", kind: Kind::Simulated, ours: |c| c.herd_available as i64, theirs: |c| c.herd_available as i64 },
    CountyField { path: "county.crop.1", kind: Kind::Simulated, ours: |c| c.crop[1] as i64, theirs: |c| c.crop[1] as i64 },
    CountyField { path: "county.grain", kind: Kind::Simulated, ours: |c| c.grain as i64, theirs: |c| c.grain as i64 },
    CountyField { path: "county.herd", kind: Kind::Simulated, ours: |c| c.herd as i64, theirs: |c| c.herd as i64 },
    CountyField { path: "county.fields_fallow", kind: Kind::Simulated, ours: |c| c.fields_fallow as i64, theirs: |c| c.fields_fallow as i64 },
    CountyField { path: "county.fields_cattle", kind: Kind::Simulated, ours: |c| c.fields_cattle as i64, theirs: |c| c.fields_cattle as i64 },
    CountyField { path: "county.fields_grain", kind: Kind::Simulated, ours: |c| c.fields_grain as i64, theirs: |c| c.fields_grain as i64 },
    CountyField { path: "county.fertility", kind: Kind::Simulated, ours: |c| c.fertility as i64, theirs: |c| c.fertility as i64 },
    CountyField { path: "county.castle_type", kind: Kind::Simulated, ours: |c| c.castle_type as i64, theirs: |c| c.castle_type as i64 },
    CountyField { path: "county.castle_building", kind: Kind::PlayerInput, ours: |c| c.castle_building as i64, theirs: |c| c.castle_building as i64 },
    CountyField { path: "county.weather", kind: Kind::Excluded("Weather_UpdateAll's band is drawn from the original's two 31-bit LFSRs, whose state Save_Write does not store"), ours: |c| c.weather.index() as i64, theirs: |c| c.weather as i64 },
    CountyField { path: "county.dryness", kind: Kind::Excluded("the dryness accumulator the band comes from, moved by the same unreproducible jitter"), ours: |c| c.dryness as i64, theirs: |c| c.dryness as i64 },
];

#[rustfmt::skip]
pub const REALM_FIELDS: &[RealmField] = &[
    RealmField { path: "realm.strength", kind: Kind::Simulated, ours: |r| r.strength as i64, theirs: |r| r.strength as i64 },
    RealmField { path: "realm.is_human", kind: Kind::Simulated, ours: |r| r.is_human as i64, theirs: |r| r.is_human as i64 },
    RealmField { path: "realm.lord", kind: Kind::Simulated, ours: |r| r.lord as i64, theirs: |r| r.lord as i64 },
    RealmField { path: "realm.shield_index", kind: Kind::Simulated, ours: |r| r.shield_index as i64, theirs: |r| r.shield_index as i64 },
    RealmField { path: "realm.tax_hap_empire", kind: Kind::Simulated, ours: |r| r.tax_hap_empire as i64, theirs: |r| r.tax_hap_empire as i64 },
    RealmField { path: "realm.county_count", kind: Kind::Simulated, ours: |r| r.county_count as i64, theirs: |r| r.county_count as i64 },
    RealmField { path: "realm.rank", kind: Kind::Simulated, ours: |r| r.rank as i64, theirs: |r| r.rank as i64 },
    RealmField { path: "realm.score", kind: Kind::Simulated, ours: |r| r.score as i64, theirs: |r| r.score as i64 },
    RealmField { path: "realm.wages", kind: Kind::Simulated, ours: |r| r.wages as i64, theirs: |r| r.wages as i64 },
    RealmField { path: "realm.gold", kind: Kind::Simulated, ours: |r| r.gold as i64, theirs: |r| r.gold as i64 },
    RealmField { path: "realm.iron", kind: Kind::Simulated, ours: |r| r.iron as i64, theirs: |r| r.iron as i64 },
    RealmField { path: "realm.stone", kind: Kind::Simulated, ours: |r| r.stone as i64, theirs: |r| r.stone as i64 },
    RealmField { path: "realm.wood", kind: Kind::Simulated, ours: |r| r.wood as i64, theirs: |r| r.wood as i64 },
    RealmField { path: "realm.weapons.0", kind: Kind::PlayerInput, ours: |r| r.weapons[0] as i64, theirs: |r| r.weapons[0] as i64 },
    RealmField { path: "realm.weapons.1", kind: Kind::PlayerInput, ours: |r| r.weapons[1] as i64, theirs: |r| r.weapons[1] as i64 },
    RealmField { path: "realm.weapons.2", kind: Kind::PlayerInput, ours: |r| r.weapons[2] as i64, theirs: |r| r.weapons[2] as i64 },
    RealmField { path: "realm.weapons.3", kind: Kind::PlayerInput, ours: |r| r.weapons[3] as i64, theirs: |r| r.weapons[3] as i64 },
    RealmField { path: "realm.weapons.4", kind: Kind::PlayerInput, ours: |r| r.weapons[4] as i64, theirs: |r| r.weapons[4] as i64 },
    RealmField { path: "realm.weapons.5", kind: Kind::PlayerInput, ours: |r| r.weapons[5] as i64, theirs: |r| r.weapons[5] as i64 },
    // `+0x00` is the AI turn cursor and the save catches it wherever the AI
    // happened to be when the rotation ran. Our machine is between turns when
    // `end_turn` returns, so the two are not sampled at the same instant and
    // the comparison would measure the sampling point.
    RealmField { path: "realm.ai_step", kind: Kind::Simulated, ours: |r| r.ai_step as i64, theirs: |r| r.ai_step as i64 },
];

/// Realm `+0x84 + other * 0x10` — one realm's view of another. Nine fields,
/// and they move every turn: the standing heals by one a season, the warning
/// ladder climbs, the grudge accumulates while allied.
#[rustfmt::skip]
pub const PAIR_FIELDS: &[PairField] = &[
    PairField { path: "pair.standing", kind: Kind::Simulated, ours: |p| p.standing as i64, theirs: |p| p.standing as i64 },
    PairField { path: "pair.allied", kind: Kind::Simulated, ours: |p| p.allied as i64, theirs: |p| p.allied as i64 },
    PairField { path: "pair.grudge", kind: Kind::Simulated, ours: |p| p.grudge as i64, theirs: |p| p.grudge as i64 },
    PairField { path: "pair.warnings_sent", kind: Kind::Simulated, ours: |p| p.warnings_sent as i64, theirs: |p| p.warnings_sent as i64 },
    PairField { path: "pair.at_war", kind: Kind::Simulated, ours: |p| p.at_war as i64, theirs: |p| p.at_war as i64 },
    PairField { path: "pair.compliments_from", kind: Kind::Simulated, ours: |p| p.compliments_from as i64, theirs: |p| p.compliments_from as i64 },
    PairField { path: "pair.best_gift", kind: Kind::Simulated, ours: |p| p.best_gift as i64, theirs: |p| p.best_gift as i64 },
    PairField { path: "pair.has_mail", kind: Kind::Simulated, ours: |p| p.has_mail as i64, theirs: |p| p.has_mail as i64 },
    PairField { path: "pair.help_price_multiple", kind: Kind::Simulated, ours: |p| p.help_price_multiple as i64, theirs: |p| p.help_price_multiple as i64 },
];

#[rustfmt::skip]
pub const GLOBAL_FIELDS: &[GlobalField] = &[
    GlobalField { path: "global.county_count", kind: Kind::Simulated, ours: |g| g.kingdom.county_count as i64, theirs: |g| g.county_count as i64 },
    GlobalField { path: "global.local_player", kind: Kind::Simulated, ours: |g| g.player as i64, theirs: |g| g.local_player as i64 },
    GlobalField { path: "global.season", kind: Kind::Simulated, ours: |g| g.kingdom.season as i64, theirs: |g| g.season as i64 },
    GlobalField { path: "global.season_next", kind: Kind::Simulated, ours: |g| g.kingdom.season_next as i64, theirs: |g| g.season_next as i64 },
    GlobalField { path: "global.year", kind: Kind::Simulated, ours: |g| g.kingdom.year as i64, theirs: |g| g.year as i64 },
    GlobalField { path: "global.turn_count", kind: Kind::Simulated, ours: |g| g.kingdom.turn_count as i64, theirs: |g| g.turn_count as i64 },
    GlobalField { path: "global.merchant_count", kind: Kind::Simulated, ours: merchant_count, theirs: |g| g.merchant_count as i64 },
    GlobalField { path: "global.opt_difficulty", kind: Kind::Simulated, ours: |g| g.kingdom.options.difficulty as i64, theirs: |g| g.opt_difficulty as i64 },
    GlobalField { path: "global.opt_advanced_farming", kind: Kind::Simulated, ours: |g| g.kingdom.options.advanced_farming as i64, theirs: |g| (g.opt_advanced_farming != 0) as i64 },
    GlobalField { path: "global.opt_armies_eat", kind: Kind::Simulated, ours: |g| g.kingdom.options.armies_eat as i64, theirs: |g| (g.opt_armies_eat != 0) as i64 },
    GlobalField { path: "global.opt_exploration", kind: Kind::Simulated, ours: |g| g.kingdom.options.exploration as i64, theirs: |g| (g.opt_exploration != 0) as i64 },
    GlobalField { path: "global.opt_time_limit", kind: Kind::Simulated, ours: |g| g.kingdom.options.time_limit as i64, theirs: |g| g.opt_time_limit as i64 },
    // `g_optAiLords` (`0x0053F268`) is stored by `Save_Write` and read by
    // `l2_formats::save::Globals`; `l2_scenario::Scenario::from_save` does not
    // carry it into `Options` and nothing after a load reads it. `grep -rn
    // ai_lords crates/` finds it in the save reader, in `l2_game::setup` (which
    // *starts* a game) and nowhere on the load path.
    GlobalField { path: "global.ai_lords", kind: Kind::Unsimulated, ours: |_| 0, theirs: |g| g.ai_lords as i64 },
    GlobalField { path: "global.turn_phase", kind: Kind::Simulated, ours: |g| g.kingdom.turn.phase.index() as i64, theirs: |g| g.turn_phase as i64 },
    GlobalField { path: "global.turn_phase_step", kind: Kind::Excluded("the step counter inside that phase, for the same reason"), ours: |g| g.kingdom.turn.step as i64, theirs: |g| g.turn_phase_step as i64 },
    GlobalField { path: "global.weather_county", kind: Kind::Excluded("the county Weather_UpdateAll drew for the local swing, from the LFSR stream we do not reproduce"), ours: |g| g.kingdom.weather_county as i64, theirs: |g| g.weather_county as i64 },
];

pub fn merchant_count(g: &Game) -> i64 {
    g.kingdom
        .campaign
        .units
        .iter()
        .filter(|(_, u)| u.kind == l2_kingdom::unit::UnitKind::Merchant)
        .count() as i64
}

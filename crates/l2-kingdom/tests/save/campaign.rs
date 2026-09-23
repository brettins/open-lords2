#![allow(unused_imports)]
use super::*;
use super::furnished_state::*;
use super::round_trips::*;
use super::coverage::*;
use super::header::*;
use super::census_part::*;
use l2_kingdom::county::{ChangeReason, MAX_COUNTIES};
use l2_kingdom::realm::{Pair, MAX_REALMS};
use l2_kingdom::save::{
    checksum, decode, encode, ruleset_fingerprint, LoadError, HEADER_LEN, MAGIC, VERSION,
};
use l2_kingdom::tables::{Tables, Weather, JOB_COUNT};
use l2_kingdom::{Kingdom, Options};

pub(super) fn furnish_campaign(k: &mut Kingdom) {
    use l2_kingdom::unit::{Mercenaries, TroopType, Unit, UnitKind};

    for i in 0..l2_kingdom::MAP_TILES {
        k.campaign.map.terrain[i] = (i % 24) as u8 + 1;
        k.campaign.map.flags[i] = (i % 7) as u8 + 1;
        k.campaign.map.bank[i] = (i % 31) as u8 + 1;
        k.campaign.map.county[i] = (i % 14) as u8 + 1;
    }

    let kinds = [UnitKind::Army, UnitKind::PeasantMob, UnitKind::Merchant, UnitKind::Transport];
    for (n, kind) in kinds.into_iter().enumerate() {
        let mut u = Unit::new(kind, 0, 0, 0);
        u.owner = (n % 5) as u8 + 1;
        u.x = 10 + n as u8;
        u.y = 20 + n as u8;
        u.owner_is_human = n % 2 == 0;
        u.shield = n as u8 + 1;
        u.player_driven = n % 2 == 1;
        u.facing = (n as u8 * 2) % 8 + 1;
        u.county = n as u8 + 1;
        u.home_county = n as u8 + 2;
        u.dest = (n % 2 == 0).then_some((30 + n as u8, 40));
        u.path = (0..n * 3 + 1).map(|s| (s as u8, (s * 2) as u8)).collect();
        u.moving = n % 2 == 0;
        u.on_road = n % 2 == 1;
        // The sub-tile counter — `Unit_StepOnce`'s `+0x149`, `+0x14A` and
        // `+0x14B` bit 0. `at_tile_edge` is furnished **false** on two of the
        // four because `Unit::new` leaves it *set* (`Unit_Spawn`)
        // here would be indistinguishable from the default and the round trip
        // could drop the byte unnoticed.
        u.sub_tile = (n as u8 + 1) * 2;
        u.sub_frame = n as u8 % 4 + 1;
        u.at_tile_edge = n % 2 == 0;
        u.name_index = n as u8 + 3;
        u.needs_destination = n % 2 == 1;
        u.dest_county = n as u8 + 4;
        u.moves_used = n as i32 + 1;
        u.move_allowance = 15;
        u.starvation = (n as i32) % 5 + 1;
        u.wages = 40 + n as i32;
        u.year_formed = 1268 + n as i32;
        u.morale = 50 + n as i32;
        u.troops = core::array::from_fn(|t| (t as i32 + 1) * (n as i32 + 1));
        u.men = u.troops.iter().sum();
        u.mercenaries = (n != 3).then(|| Mercenaries {
            band: n as u8 + 1,
            troop: TroopType::from_index(n % 7).unwrap(),
            men: 150 + n as u8,
        });
        u.men += u.mercenaries.map_or(0, |m| m.men as i32);
        u.garrison_county = n as u8 + 3;
        u.besieging_county = n as u8 + 5;
        u.besieged_by = n as u8 + 2;
        u.cargo_county = n as u8 + 6;
        // `+0x167`, the county-defence mark — `docs/armies.md` §8.1
        // second half of what `VERSION` 10 restored.
        u.defence_mark = (n % 2 + 1) as u8;
        // `+0x1A` and `+0x19B`.
        u.mission = (n % 6 + 2) as u8;
        u.mission_county = n as u8 + 4;
        for (e, record) in u.engines.iter_mut().enumerate() {
            record.ordered = (n + e) as i16 % 5;
            record.percent = ((n + e) as i16 * 7) % 101;
            record.work_done = ((n + e) as i16 * 43) % 201;
        }
        u.siege_seasons_left = (n % 4) as u8;
        k.campaign.units.put(n * 2 + 1, u);
    }

    k.campaign.mercenaries = l2_kingdom::MercenaryBands::init(14);
    let mut counties = k.counties.clone();
    for _ in 0..3 {
        k.campaign.mercenaries.advance(&mut counties, 14, k.options.quirks);
    }
    k.counties = counties;
    for band in 1..l2_kingdom::mercenary::BAND_SLOTS {
        let mut b = k.campaign.mercenaries.band_raw(band);
        b.hired_by = band as u16 + 1;
        b.offered_in = (band % 14 + 1) as u8;
        b.next_county = (band % 13 + 1) as u8;
        b.countdown = band as i8 + 1;
        b.reload = band as i8 + 2;
        k.campaign.mercenaries.set_band_raw(band, b);
    }
    k.campaign.mercenaries.set_in_play(4);

    for realm in 1..=5u8 {
        for _ in 0..realm {
            k.campaign.names.pick(realm);
        }
    }

    for route in 0..l2_kingdom::merchant::ROUTES {
        let mut row = [0u8; l2_kingdom::merchant::ROUTE_SLOTS];
        for (slot, county) in row.iter_mut().enumerate() {
            *county = ((route + slot) % 14 + 1) as u8;
        }
        k.campaign.routes.set_row(route, row);
    }
    k.campaign.mob_cursor = 7;
    // `DAT_0057CAE0` dealt, `DAT_005653F8` walked off zero so the round trip
    // has a cursor to lose.
    k.campaign.field_playlist = l2_kingdom::field_playlist::FieldPlaylist::deal(0x5EED);
    for _ in 0..5 {
        k.campaign.field_playlist.take_next();
    }
    k.campaign.explored.reveal_square(1, 12, 40, 6);
    k.campaign.explored.reveal_square(4, 16, 44, 3);
    // `g_battleApproachLane` (`0x0056D6B0`) and `g_battleRallyGroup`, off zero.
    k.campaign.battle_selectors = (3, 1);
    k.campaign.battle_markers = ([[(9, 70); 4], [(40, 5); 4]], [[[(12, 34); 3]; 2], [[(56, 78); 3]; 2]]);

    k.diplomacy.help_price = 3400;
    k.diplomacy.help_county = 9;
    for (n, kind) in l2_kingdom::DiploKind::ALL.iter().enumerate() {
        let to = (n % 5 + 1) as u8;
        k.diplomacy.inbox[to as usize][n % l2_kingdom::INBOX_SLOTS] = l2_kingdom::InboxSlot {
            from: (n % 4 + 1) as u8,
            kind: kind.byte(),
            county: (n + 2) as u8,
            gold: 100 * n as i32 + 7,
        };
    }
    for _ in 0..5 {
        k.diplomacy.dice.rand7b();
    }
}
// --- the fixture: end -----------------------------------------------------

pub(crate) fn played(seasons: usize) -> Kingdom {
    let mut k = furnished(0xC0FFEE);
    k.start_new_game();
    for _ in 1..seasons {
        k.advance_season();
    }
    k
}





mod allocators;
mod stalls;
pub(crate) use stalls::*;

mod market;

use super::*;
use crate::field::terrain;
use crate::map::CampaignMap;

const T: &Tables = &Tables::DEFAULT;

fn env() -> FarmEnv {
    FarmEnv::new(Season::Winter, Season::Spring)
}

fn realms() -> Vec<Realm> {
    let mut realms = vec![Realm::new(); crate::realm::MAX_REALMS];
    for (i, r) in realms.iter_mut().enumerate().skip(1) {
        r.in_play = true;
        r.lord = ((i - 1) % crate::tables::AI_PERSONALITY_COUNT + 1) as u8;
    }
    realms
}

fn lay(style: FarmStyle, c: &mut County, map: &mut CampaignMap, e: &FarmEnv) {
    let mut counties = vec![County::new(), c.clone()];
    lay_out(T, style, &mut counties, 1, 1, map, &realms(), &mut NoMarket, e);
    *c = counties.remove(1);
}

fn county_with(n: usize) -> (County, CampaignMap) {
    let mut c = County::new();
    let mut map = CampaignMap::empty();
    for i in 0..n {
        let tile = crate::map::index(i as u8, 8);
        c.set_field_tile(i, Some(tile));
        map.terrain[tile] = terrain::FALLOW;
    }
    c.population = 500;
    crate::field::recount(&mut c, &map);
    (c, map)
}

/// **An AI realm that never plants is the original's rule, not a gap in
/// ours** — `Ai_FarmStyleGrazing` (`0x004A42E3`), whose first act is to
/// clear every grain field, in every season.
///
/// `Ai_ManageCountyFarms` (`0x0049DD01`) copies the lord's style byte into
/// county `+0x1FE` and dispatches on it,
/// `[1, 1, 0, 9]` — **two of the four graze**, so an AI realm behind either
/// sows nothing all game.
///
/// This pins **which lords it happens to**, which is the half that makes an
/// AI realm look broken: ablation, carried inside — lord 3's style is 0 and
/// the same county sows.
#[test]
fn two_of_the_four_lords_graze_so_their_realms_never_sow() {
    assert_eq!(crate::tables::AI_PERSONALITY_FARM_STYLE, [1, 1, 0, 9]);
    for lord in [1u8, 2] {
        assert_eq!(T.ai_farm_style(lord), Some(1), "lord {lord}");
        assert_eq!(FarmStyle::for_realm(1), Some(FarmStyle::RealmGrazing));
    }
    assert_eq!(T.ai_farm_style(3), Some(0));
    assert_eq!(T.ai_farm_style(4), Some(9));

    let sown = |style| {
        let (mut c, mut map) = county_with(8);
        let e = FarmEnv::new(Season::Winter, Season::Spring);
        lay(style, &mut c, &mut map, &e);
        counts(&c, &map).0
    };
    assert_eq!(sown(FarmStyle::RealmGrazing), 0, "lords 1 and 2");
    assert!(sown(FarmStyle::RealmArable) > 0, "lord 3 sows the same county");
}

fn counts(county: &County, map: &CampaignMap) -> (i32, i32, i32) {
    let mut probe = county.clone();
    crate::field::recount(&mut probe, map);
    (probe.fields_grain, probe.fields_cattle, probe.fields_fallow)
}

struct Ledger {
    asked: Vec<(i32, Good)>,
    grant: bool,
}

impl Market for Ledger {
    fn buy(
        &mut self,
        _id: usize,
        county: &mut County,
        _map: &mut CampaignMap,
        qty: i32,
        good: Good,
    ) -> bool {
        self.asked.push((qty, good));
        if !self.grant {
            return false;
        }
        match good {
            Good::Grain => county.grain += qty,
            Good::Cattle => county.herd += qty,
        }
        true
    }
}

fn ledger(grant: bool) -> Ledger {
    Ledger { asked: Vec::new(), grant }
}


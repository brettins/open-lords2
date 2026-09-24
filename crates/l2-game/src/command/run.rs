//! Each order's effect, moved verbatim from the screen that used to do it.

use super::*;
use l2_kingdom::industry::{self, CastleRefusal};
use l2_kingdom::{siege, Kingdom};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Castled {
    Refused(CastleRefusal),
    Ownerless,
    Ordered,
}

impl Order for OrderCastle {
    type Reply = Castled;
    fn run(self, game: &mut Game) -> Castled {
        let t = game.kingdom.tables;
        let county = &game.kingdom.counties[self.county as usize];
        if let Some(why) = industry::castle_refusal(&t, county, self.castle) {
            return Castled::Refused(why);
        }
        let owner = county.owner as usize;
        let Kingdom { counties, realms, campaign, .. } = &mut game.kingdom;
        let Some(realm) = realms.get_mut(owner) else { return Castled::Ownerless };
        industry::order_castle(&t, &mut counties[self.county as usize], realm, self.castle);
        l2_kingdom::map::stamp_castle_terrain(&mut campaign.map, self.county, self.castle);
        l2_kingdom::movement::evict_castle_plot(&campaign.move_cost, &campaign.map, &mut campaign.units, self.county);
        Castled::Ordered
    }
}

impl Order for CombineArmies {
    type Reply = Result<i32, l2_kingdom::unit::CombineRefusal>;
    fn run(self, game: &mut Game) -> Self::Reply {
        l2_kingdom::unit::combine(&mut game.kingdom.campaign.units, self.occupant, self.mover)
    }
}

impl Order for ValidateSiegeLink {
    type Reply = ();
    fn run(self, game: &mut Game) {
        let Kingdom { counties, campaign, .. } = &mut game.kingdom;
        if campaign.units.get(self.unit).is_some_and(|u| u.besieging_county != 0) {
            siege::validate_link(counties, &mut campaign.units, self.unit);
        }
    }
}

impl Order for Trade {
    type Reply = Result<l2_kingdom::trade::Receipt, l2_kingdom::trade::Refusal>;
    fn run(self, game: &mut Game) -> Self::Reply {
        let order = l2_kingdom::trade::Order {
            qty: self.qty,
            good: self.good,
            buy_price: self.buy_price,
            sell_price: self.sell_price,
            realm: game.player as usize,
            county: self.county,
        };
        l2_kingdom::trade::trade(&mut game.kingdom, order)
    }
}

/// `Diplo_PayHelpClicked` (`0x004367FF`).
impl Order for PayForHelp {
    type Reply = ();
    fn run(self, game: &mut Game) {
        let me = game.player;
        let ally = game.kingdom.realms[me as usize].ally;
        let county = game.kingdom.diplomacy.help_county;
        let price = game.kingdom.diplomacy.help_price;
        l2_kingdom::diplomacy::pay_for_help(&mut game.kingdom.realms, ally, me, county, price);
    }
}

/// `FUN_00436872` (`0x00436872`).
impl Order for FormAlliance {
    type Reply = ();
    fn run(self, game: &mut Game) {
        l2_kingdom::diplomacy::form_alliance(&mut game.kingdom.realms, game.player, self.offerer);
    }
}

/// `0x0043B681` / `0x0043B741`, both ending in `0x0043B7C4`.
impl Order for OrderEngine {
    type Reply = bool;
    fn run(self, game: &mut Game) -> bool {
        siege::order_engine(&mut game.kingdom.campaign.units, self.unit, self.engine, self.delta)
    }
}

impl Order for LiftSiege {
    type Reply = ();
    fn run(self, game: &mut Game) {
        let Kingdom { counties, campaign, .. } = &mut game.kingdom;
        siege::break_siege(counties, &mut campaign.units, self.unit);
    }
}

/// `FUN_0043B145` — `Transport_Spawn`, then the source county's passes.
impl Order for SendSupplies {
    type Reply = l2_kingdom::supply::Sent;
    fn run(self, game: &mut Game) -> Self::Reply {
        let k = &mut game.kingdom;
        let owner = k.counties[self.from as usize].owner;
        let sent = l2_kingdom::supply::spawn(
            &k.campaign.map,
            &mut k.counties,
            &mut k.campaign.units,
            owner,
            self.from,
            self.to,
            self.grain,
            self.cattle,
        );
        if let l2_kingdom::supply::Sent::Unit(_) = sent {
            let tables = k.tables;
            let Kingdom { counties, campaign, .. } = k;
            l2_kingdom::field::herd_update_crowding(
                &tables,
                &mut counties[self.from as usize],
                &mut campaign.map,
            );
        }
        sent
    }
}

impl Order for ToggleOption {
    type Reply = ();
    fn run(self, game: &mut Game) {
        let o = &mut game.kingdom.options;
        match self.option {
            GameOption::AdvancedFarming => o.advanced_farming = !o.advanced_farming,
            GameOption::ArmyForaging => game.kingdom.toggle_army_foraging(),
            GameOption::Exploration => o.exploration = !o.exploration,
            GameOption::FightHumansOnly => {
                o.fight_humans_only_byte = u8::from(o.fight_humans_only_byte == 0)
            }
        }
    }
}

impl Order for SetQuirk {
    type Reply = ();
    fn run(self, game: &mut Game) {
        game.kingdom.options.quirks.set_reproduced(self.quirk, self.reproduced);
    }
}

/// `NetAct_RankRealms` (`0x00448422`), the deferred action of
/// `Net_SendCommand(0x3B, 0)` that `Court_OpenGreatestNoble` sends.
impl Order for RankRealms {
    type Reply = ();
    fn run(self, game: &mut Game) {
        let t = game.kingdom.tables;
        l2_kingdom::ai::rank_realms(&t, &mut game.kingdom.realms);
        for realm in 1..l2_kingdom::MAX_REALMS as u8 {
            let (armies, total_men) = game.kingdom.campaign.units.realm_totals(realm);
            let Kingdom { realms, counties, county_count, .. } = &mut game.kingdom;
            l2_kingdom::ai::update_realm_totals(
                &mut realms[realm as usize],
                counties,
                *county_count,
                realm,
                armies,
                total_men,
            );
        }
    }
}

macro_rules! via_game {
    ($($name:ident -> $reply:ty = |$g:ident, $o:ident| $body:expr;)*) => {$(
        impl Order for $name {
            type Reply = $reply;
            fn run(self, $g: &mut Game) -> $reply {
                let $o = self;
                $body
            }
        }
    )*};
}

via_game! {
    SetTaxRate -> bool = |g, o| g.set_tax_rate(o.county, o.rate);
    SetRation -> bool = |g, o| g.set_ration(o.county, o.level);
    SetRationSplit -> bool = |g, o| g.set_ration_split(o.county, o.split, o.sweep);
    RaiseArmy -> Result<usize, l2_kingdom::LevyRefusal> =
        |g, o| g.raise_army(o.county, &o.basket, o.happiness_cost, o.hire);
    SplitArmy -> Result<usize, l2_kingdom::SplitRefusal> =
        |g, o| g.split_army(o.army, &o.basket, o.into);
    LeaveCastle -> l2_kingdom::conquest::LeftCastle = |g, o| g.leave_castle(o.unit);
    DisbandArmy -> Result<(u8, i32), l2_kingdom::DisbandRefusal> = |g, o| g.disband_army(o.army);
    MoveUnit -> Option<usize> = |g, o| g.order_unit_move(o.unit, o.dest);
    MoveLabour -> i32 = |g, o| g.move_labour(o.county, o.from, o.to, o.icons);
    BalanceLabour -> i32 = |g, o| g.balance_labour(o.county, o.cluster, o.fill);
    BalanceAllLabour -> i32 = |g, o| g.balance_all_labour(o.county);
    PaintField -> Result<(), l2_kingdom::field::BrushRefusal> =
        |g, o| g.kingdom.paint_field(o.county, o.tile, o.kind);
    ToggleIndustry -> bool = |g, o| g.kingdom.toggle_industry(o.county, o.what);
}

/// `NetCmd_ReadDiploLetter` (`0x00445960`): every peer copies the text into
/// `g_diploLetter + from * 0xCA`, and `Msg_Enqueue` (`0x00472BC5`) rings only the
/// addressee (`g_localPlayer == to`). Group `0xF5 + kind`, category `0x0C`;
/// kind 0 carries the gold, kinds 5..=10 the county. [V] The text travels in the
/// command (C244).
impl Order for SendLetter {
    type Reply = ();
    fn run(self, game: &mut Game) {
        let from = game.player;
        let letters = &mut game.messages.letters;
        if letters.len() <= usize::from(from) {
            letters.resize(usize::from(from) + 1, String::new());
        }
        // `Net_ReadField(&g_diploLetter + from * 0xCA, 0xCA)`; `FUN_0040210C(draft, 199)`.
        letters[usize::from(from)] = self.text.chars().take(199).collect();
        if self.kind > 0x0C {
            return;
        }
        let record = crate::message::Record {
            to: self.to,
            from,
            group: 0xF5 + u16::from(self.kind),
            variant: if self.kind == 0 { self.gold } else { 0 },
            category: crate::message::category::DIPLOMACY,
            county: if (5..=10).contains(&self.kind) { self.county } else { 0 },
            ..Default::default()
        };
        let local = game.local_realm();
        game.messages.enqueue(record, local);
        // `if (g_localPlayer == local_8)`: kind 0 `Net_SendCommand(0x4A)`, kind 4 `(0x4B)`. [V]
        if !game.net.lockstep || local != self.to {
            return;
        }
        match self.kind {
            0 => game.net.outbox.push(GiftReply { from, gold: self.gold }.into()),
            4 => game.net.outbox.push(EndAllianceReply { with: from }.into()),
            _ => {}
        }
    }
}

/// `Net_DiploPost` (`0x00448308`), action 0x2A of opcode 0x48: `Diplo_SendClicked`
/// (`0x00436408`) posts to an AI lord through the network, never directly. [V]
impl Order for PostLetter {
    type Reply = ();
    fn run(self, game: &mut Game) {
        let Some(kind) = l2_kingdom::diplomacy::Kind::from_byte(self.kind) else { return };
        let from = game.player;
        game.kingdom.post_letter(from, self.to, kind, self.gold, self.county);
    }
}

/// `NetAct_DiploReply` (`0x00448339`), opcode 0x4A: issued by the gift's addressee.
impl Order for GiftReply {
    type Reply = bool;
    fn run(self, game: &mut Game) -> bool {
        let me = game.player;
        l2_kingdom::diplomacy::gift_reply(&mut game.kingdom.realms, me, self.from, self.gold)
    }
}

/// `NetAct_DiploBreakAlliance` (`0x004483D1`), opcode 0x4B: `Diplo_BreakAlliance(to, from)`.
impl Order for EndAllianceReply {
    type Reply = ();
    fn run(self, game: &mut Game) {
        let me = game.player;
        l2_kingdom::diplomacy::break_alliance(&mut game.kingdom.realms, me, self.with);
    }
}

via_game! {
    // `Blacksmith_WeaponClicked` (`0x0043A950`): opcode 0x21, `Blacksmith_SetWeaponType` (`0x0043A997`).
    SetWeaponType -> bool = |g, o| g.kingdom.set_weapon_type(o.county, o.weapon);
    // `NetAct_SetIndustryShare` (`0x00447FA1`): `Labour_SetIndustryShare(county, share)`.
    SetIndustryShare -> bool = |g, o| g.kingdom.set_industry_share(o.county, o.share);
}

//! The payloads. Every field is what the screen chose; who chose it is the slot.

use super::codec::{wire_struct, Wire};
use l2_kingdom::divide::{SplitBasket, SplitInto};
use l2_kingdom::field::FieldType;
use l2_kingdom::industry::MapToggle;
use l2_kingdom::levy::LevyBasket;
use l2_kingdom::siege::Engine;
use l2_kingdom::trade::Good;
use l2_net::{Canonical, CodecError, Quirk, Reader};

/// The four options that change the rules; the rest are prefs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameOption {
    AdvancedFarming,
    ArmyForaging,
    Exploration,
    FightHumansOnly,
}

pub const GAME_OPTIONS: [GameOption; 4] = [
    GameOption::AdvancedFarming,
    GameOption::ArmyForaging,
    GameOption::Exploration,
    GameOption::FightHumansOnly,
];

macro_rules! orders {
    ($($name:ident { $($f:ident: $t:ty),* })*) => {
        $(
            #[derive(Debug, Clone, PartialEq, Eq)]
            pub struct $name { $(pub $f: $t),* }
        )*
        wire_struct! { $($name { $($f),* })* }
    };
}

orders! {
    OrderCastle { county: u8, castle: u8 }
    CombineArmies { occupant: usize, mover: usize }
    ValidateSiegeLink { unit: usize }
    Trade { qty: i32, good: Good, buy_price: i32, sell_price: i32, county: usize }
    PayForHelp {}
    FormAlliance { offerer: u8 }
    OrderEngine { unit: usize, engine: Engine, delta: i16 }
    LiftSiege { unit: usize }
    SendSupplies { from: u8, to: u8, grain: i32, cattle: i32 }
    ToggleOption { option: GameOption }
    SetQuirk { quirk: Quirk, reproduced: bool }
    RankRealms {}
    SetTaxRate { county: u8, rate: i32 }
    SetRation { county: u8, level: i32 }
    SetRationSplit { county: u8, split: i32, sweep: bool }
    RaiseArmy { county: u8, basket: LevyBasket, happiness_cost: i32, hire: Option<u8> }
    SplitArmy { army: usize, basket: SplitBasket, into: SplitInto }
    LeaveCastle { unit: usize }
    DisbandArmy { army: usize }
    MoveUnit { unit: usize, dest: (u8, u8) }
    MoveLabour { county: u8, from: usize, to: usize, icons: i32 }
    BalanceLabour { county: u8, cluster: usize, fill: bool }
    BalanceAllLabour { county: u8 }
    PaintField { county: usize, tile: usize, kind: FieldType }
    ToggleIndustry { county: usize, what: MapToggle }
}

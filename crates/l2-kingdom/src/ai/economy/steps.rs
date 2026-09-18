#![allow(unused_imports)]
use super::*;
use super::taxes::*;
use super::grants::*;
use super::construction::*;
use super::industry::*;
use super::diplomacy::*;
use super::*;
use crate::county::County;
use crate::realm::{Realm, AI_STEP_DONE};
use crate::tables::{
    ai_grant_tier, tax_rate_for, Tables, AI_CASTLE_LADDER_LEN, AI_GOLD_GRANT_SMALL_COUNTIES,
    AI_WEAPON_ROTA_ORDER,
};

impl AiStep {
    pub const ALL: [AiStep; 14] = [
        AiStep::Diplomacy,
        AiStep::ConsiderWar,
        AiStep::SetTaxRates,
        AiStep::ResourceWants,
        AiStep::ManageFields,
        AiStep::BuildCastles,
        AiStep::ManageArmies,
        AiStep::Nothing,
        AiStep::RaiseArmy,
        AiStep::SendUnit,
        AiStep::MoveArmies,
        AiStep::ChooseIndustry,
        AiStep::Taunt,
        AiStep::UpdateTotals,
    ];

    pub fn from_counter(step: i32) -> Option<AiStep> {
        AiStep::ALL.get(usize::try_from(step.checked_sub(1)?).ok()?).copied()
    }

    pub fn counter(self) -> i32 {
        self as i32
    }

    pub fn address(self) -> u32 {
        match self {
            AiStep::Diplomacy => 0x004A277D,
            AiStep::ConsiderWar => 0x004A0C1D,
            AiStep::SetTaxRates => 0x0049D638,
            AiStep::ResourceWants => 0x0049E1BF,
            AiStep::ManageFields => 0x0049DD01,
            AiStep::BuildCastles => 0x0049EDC7,
            AiStep::ManageArmies => 0x0049F93D,
            AiStep::Nothing => 0x0049F96C,
            AiStep::RaiseArmy => 0x0049F977,
            AiStep::SendUnit => 0x004A0015,
            AiStep::MoveArmies => 0x004A5667,
            AiStep::ChooseIndustry => 0x0049E77D,
            AiStep::Taunt => 0x004A13A6,
            AiStep::UpdateTotals => 0x0049D1E0,
        }
    }

    pub fn is_empty(self) -> bool {
        self == AiStep::Nothing
    }

    pub fn is_implemented(self) -> bool {
        true
    }
}


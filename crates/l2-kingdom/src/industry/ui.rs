#![allow(unused_imports)]
use super::*;
use super::production::*;
use super::wages::*;
use super::castles::*;
use crate::county::County;
use crate::math::pct;
use crate::realm::Realm;
use crate::report::Message;
use crate::tables::{Commodity, Tables, RESOURCE_LIMIT_UNLIMITED, WEAPON_TYPE_COUNT};
use l2_net::{Quirk, Quirks};


pub fn map_toggle_for_graphic(graphic: u8) -> Option<MapToggle> {
    match graphic {
        0..=3 => Some(MapToggle::Industry(Commodity::Iron)),
        4..=6 => Some(MapToggle::Industry(Commodity::Stone)),
        7..=9 => Some(MapToggle::Industry(Commodity::Weapons)),
        10..=12 => Some(MapToggle::Industry(Commodity::Wood)),
        13..=20 => None,
        _ => Some(MapToggle::Castle),
    }
}

/// `Industry_ToggleFromMap` (`0x0043D309`) — switch one industry, or castle
/// building, on or off.
///
/// ```c
/// enabled ^= 1;                                   /* +0x297 + c*0x18, or +0x1B0 */
/// County_RefreshEstimates(county, seasonNext);
/// Labour_ToggleIndustryShare(county, jobFor(industry), enabled);
/// County_RefreshEstimates(county, seasonNext);
/// Labour_Allocate(county); Ration_Apply(county, season); Labour_Allocate(county);
/// County_RefreshEstimates(county, seasonNext);
/// ```
///
/// **`[D]`.** The enable byte is what [`crate::labour::ceilings`] already gates
/// each mining job on, so switching one off empties that job on the next
/// allocation — which is the point of the button. The castle arm reads its
/// switch *before* flipping it, so the share is toggled to the state the switch
/// was **leaving**, not the one it lands in; that is the original's order and
/// it is kept.
pub fn toggle_from_map(county: &mut County, what: MapToggle, quirks: Quirks) -> bool {
    match what {
        MapToggle::Industry(c) => {
            let slot = c.index();
            county.industry[slot].enabled = !county.industry[slot].enabled;
            crate::labour::toggle_industry_share(
                county,
                c.job(),
                county.industry[slot].enabled,
            );
            county.industry[slot].enabled
        }
        MapToggle::Castle => {
            let was = county.castle_switch;
            let told = if quirks.reproduces(Quirk::CastleSwitchMovesShareBackwards) {
                was
            } else {
                !was
            };
            crate::labour::toggle_industry_share(county, crate::tables::JOB_CASTLE_BUILDING, told);
            county.castle_switch = !was;
            county.castle_switch
        }
    }
}

/// ```c
/// if (g_counties[county].owner == g_localPlayer) {
///     DAT_0053F0A0 = g_mouseX; DAT_0053F09C = g_mouseY;
///     Msg_Enqueue(0, g_localPlayer, local_10 + 0xE6, 0, '\x04', '\0', '\0', 0);
/// }
/// ```
///
/// **`L2.eng` groups 228 … 237 are ten consecutive one-string groups** and they
/// land in exactly that order, which is an independent confirmation of the
/// commodity numbering — the *strings* say wood is 0 and stone is 3, with no
/// reference to the labour ladder:
///
/// Read out of the player's own `L2.eng`, not from a table here. All ten
/// `S2xx_01.wav` narrations ship with the game,
/// hangs on: `Msg_DrawWindow`'s category-4 arm speaks at `g_messageTimer ==
/// 0x5A`.
pub fn toggle_message_group(what: MapToggle, on: bool) -> u16 {
    match what {
        MapToggle::Industry(c) => 0xE6 + (c.index() as u16) * 2 + u16::from(on),
        MapToggle::Castle => 0xE6 - 2 + u16::from(on),
    }
}



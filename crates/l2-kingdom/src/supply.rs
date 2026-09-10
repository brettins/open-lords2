//! **Supply transports** — one county sending grain and cattle to another.
//!
//! `Transport_Spawn` (`0x004292AF`) puts a `kind == 4` unit on the map with the
//! cargo in its troop counters; phase 3 walks it (`crate::units_tick`); and
//! `Transport_Deliver` (`0x004296B5`) unloads it into the destination county
//! and destroys it.
//!
//! # The cargo is in the troop counters
//!
//! A transport is an army record with different fields meaning different
//! things, which is `docs/armies.md`'s whole point about `g_units`:
//!
//! | field | on an army | on a transport |
//! |---|---|---|
//! | `troops[0]` | peasants | **sacks of grain** |
//! | `troops[1]` | crossbowmen | *sheep* — written by nothing, added back by nothing |
//! | `troops[2]` | macemen | **head of cattle** |
//! | `morale` | morale | **the county it left** |
//! | `cargo_county` (`+0x167`) | the unit's role | **the county it is going to** |
//!
//! The unit panel is what settles the first and third of those: it draws
//! `Ui_DrawCount(troops[0], 0x44)` beside `Misc_cty` frame `0x21`, the grain
//! sack, and `Ui_DrawCount(troops[2], 0x46)` beside frame `0x26`, the cattle —
//! and group 8's noun `0x44` is *"Grain"* and `0x46` is *"Cow."*/*"Cows."*
//! **[V]**
//!
//! # Two behaviours worth reproducing on purpose
//!
//! **A shipment with nowhere to stand is lost in silence.** `Transport_Spawn`
//! looks for a free tile with `County_FindFreeRoadTile` and then
//! `County_FindFreeOpenTile`; if neither answers it spawns nothing **and
//! deducts nothing**, and the screen that sent it has already closed. There is
//! no message. [`Sent::Nowhere`] is that outcome, and it is a returned value
//! rather than a warning because a warning is not what the original gives.
//!
//! **`troops[1]` is loaded and never unloaded.** `Transport_Spawn` copies the
//! record's dead sheep word into it and `Transport_Deliver` adds `troops[0]`
//! and `troops[2]` back and not `troops[1]`. Harmless — the word is always zero
//! — and reproduced, because it is the cut subsystem's last live wire and
//! deleting it would delete the evidence. `docs/bugs.md` D20.

use crate::county::County;
use crate::map::CampaignMap;
use crate::unit::{Unit, UnitKind, Units};
use crate::MAX_COUNTIES;

/// What one dispatch did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sent {
    /// The unit's slot. The cargo has been deducted from the source county.
    Unit(usize),
    /// **No free tile within three of the county's anchor**, or no free unit
    /// slot. Nothing spawned, nothing deducted, nothing said.
    Nowhere,
}

/// `Transport_Spawn` (`0x004292AF`).
///
/// The tile search is `crate::levy::muster_tile` — the same
/// `County_FindFreeRoadTile` then `County_FindFreeOpenTile` box walk around the
/// county's **anchor**, radius 1 then 2 then 3, that an army is raised on
/// (`docs/decisions.md` C47).
///
/// The cargo is clamped to what the county actually holds, which the original
/// does not need to do because the screen's own arithmetic conserves the sum —
/// see `screens/supplies.rs`. It is done here so that a caller that is not that
/// screen (the AI's abandon-a-county path, `FUN_0049F431`, which ships
/// *everything*) cannot drive a county negative.
pub fn spawn(
    map: &CampaignMap,
    counties: &mut [County; MAX_COUNTIES],
    units: &mut Units,
    owner: u8,
    from: u8,
    to: u8,
    grain: i32,
    cattle: i32,
) -> Sent {
    let anchor = counties.get(from as usize).map_or((0, 0), |c| (c.anchor_x, c.anchor_y));
    let Some((x, y)) = crate::levy::muster_tile(map, units, anchor) else {
        return Sent::Nowhere;
    };
    let Some(c) = counties.get_mut(from as usize) else { return Sent::Nowhere };
    let grain = grain.clamp(0, c.grain);
    let cattle = cattle.clamp(0, c.herd);

    let mut unit = Unit::new(UnitKind::Transport, owner, x, y);
    unit.needs_destination = false;
    unit.county = from;
    // **`u.morale = county`** — the original reuses the morale byte as the
    // source id on a transport, and `docs/armies.md` §2 says so.
    unit.morale = from as i32;
    unit.cargo_county = to;
    unit.troops[0] = grain;
    // `troops[1]` takes the record's dead sheep word, which is always zero.
    unit.troops[1] = 0;
    unit.troops[2] = cattle;
    unit.men = grain + cattle;

    let Some(slot) = units.spawn(unit) else { return Sent::Nowhere };
    c.grain -= grain;
    c.herd -= cattle;
    Sent::Unit(slot)
}

/// `Transport_Deliver` (`0x004296B5`) — unload into the destination county.
///
/// Returns what was added, `(grain, cattle)`. The caller destroys the unit;
/// this does not, because the unit array is walked by its own phase and a
/// function that removed an entry mid-walk would be the wrong shape.
///
/// **`troops[1]` is not added back.** See the module docs.
pub fn deliver(counties: &mut [County; MAX_COUNTIES], unit: &Unit) -> (i32, i32) {
    let (grain, cattle) = (unit.troops[0], unit.troops[2]);
    if let Some(c) = counties.get_mut(unit.cargo_county as usize) {
        c.grain += grain;
        c.herd += cattle;
    }
    (grain, cattle)
}

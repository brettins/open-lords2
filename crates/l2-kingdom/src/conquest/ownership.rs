#![allow(unused_imports)]
use super::*;
use super::attack::*;
use super::castle::*;
use crate::county::{County, MAX_COUNTIES};
use crate::levy::{self, Defence};
use crate::map::CampaignMap;
use crate::realm::{Realm, MAX_REALMS};
use crate::tables::{Season, Tables};
use crate::unit::{ArmyNames, UnitKind, Units};

/// `County_MakeIndependent` (`0x004AC3C6`) — **the common ending of every way a
/// county stops being owned**: secession, revolt, the elimination of a realm,
/// and a conquest the taker cannot reach.
///
/// ```c
/// owner = 0; shieldIndex = 0;
/// for (i = 0; i < 4; i++) industry[i].enabled = 0;
/// castleSwitch = 0;
/// Labour_Allocate(county); Ration_Apply(county, g_season);
/// County_RefreshEstimates(county, g_seasonNext); Tax_RecomputePreview(county);
/// if (garrison) FUN_00437535(garrison, county);
/// ```
///
/// **Switching all four industries off is the mechanism, not a flourish.** It
/// is what turns the county's four industry ceilings to zero,
/// re-allocation on the next line is what moves those people into *Idle
/// townsfolk*.
///
/// County `+0x07`, the owner's shield byte, is presentation and [`County`] has
/// no such field; the garrison hand-off is `FUN_00437535`, which lives in the
/// unit layer and is left to the caller.
pub fn make_independent(
    t: &Tables,
    counties: &mut [County; MAX_COUNTIES],
    realms: &[Realm; MAX_REALMS],
    county: u8,
    map: &CampaignMap,
    r: Restore,
) {
    let id = county as usize;
    if id == 0 || id >= counties.len() {
        return;
    }
    {
        let c = &mut counties[id];
        c.owner = 0;
        for industry in c.industry.iter_mut() {
            industry.enabled = false;
        }
        c.castle_switch = false;
    }
    crate::labour::allocate(&mut counties[id]);
    crate::ration::apply(t, &mut counties[id], r.armies_eat);
    // `County_RefreshEstimates(county, g_seasonNext)` with the owner it now
    // has, which is nobody: realm 0 owns no blacksmith, so the share is what a
    // neutral county's share is.
    let share = crate::industry::weapon_shares(t, counties, r.county_count, 0);
    let neutral = Realm::new();
    let realm = realms.first().unwrap_or(&neutral);
    crate::field::refresh_estimates(
        &mut counties[id],
        map,
        r.season_next,
        t,
        r.advanced_farming,
        realm,
        share,
    );
    crate::tax::recompute_preview(t, &mut counties[id]);
}

/// `FUN_00437535` (`0x00437535`) for a capture — [`leave_castle`], plus the
/// county link when the pair was half-written.
///
/// `FUN_00437535` tests neither half: `Army_GarrisonApply` writes county
/// `+0x1BC` and unit `+0x198` together, so the original can never hold a county
/// pointing at an army that does not point back. [`leave_castle`] does test,
/// and a half-link would otherwise survive the capture.
fn hand_off(
    map: &CampaignMap,
    counties: &mut [County; MAX_COUNTIES],
    realms: &[Realm; MAX_REALMS],
    units: &mut Units,
    garrison: usize,
    county: u8,
) -> LeftCastle {
    let out = leave_castle(map, counties, realms, units, garrison, county);
    if out == LeftCastle::NotAGarrison {
        if let Some(c) = counties.get_mut(county as usize) {
            c.garrison_unit = 0;
        }
    }
    out
}

/// `County_ChangeOwner` (`FUN_004A72FE`, `0x004A72FE`) — the county changes
/// hands.
///
/// ```c
/// realm[new].countyCount++;
/// ...messages, one of nine, by how many counties the taker now holds...
/// county.owner  = new;
/// penalty = realm[new].isHuman ? difficulty * 20 + 10 : 30;
/// if (county.happiness < penalty) { shownEvents -= happiness; happiness = 0; }
/// else                            { happiness -= penalty; shownEvents -= penalty; }
/// county.shield = realm[new].shield;
/// realm[new].peakCounties = max(peakCounties, countyCount);
/// ```
///
/// Two things worth naming. **The conquest penalty is drawn on the *"From
/// events"* line** (`+0x17`, `L2.eng` group 85 index 9), not on the army line —
/// so a county the player has just taken shows its resentment where a plague or
/// a fire would show. And the same clamp shape as the levy: the panel is
/// debited what was taken.
///
/// # The letter, and why it is reported
///
/// Between the recount and the owner write the original posts one of thirteen
/// letters, and **which one depends on `g_localPlayer`** — the taker is told
/// *"Bravo!!"*, the loser *"Our county is lost!"*, everybody else *"This enemy
/// shire has a new ruler"*. `g_localPlayer` is a peer's and not the world's, so
/// this returns a [`Capture`] — the same on every peer — and `l2_game::arrival`
/// picks the letter for its own player. The whole ladder, `[V]`:
///
/// ```c
/// FUN_0049d1e0(newOwner);                        /* Realm_UpdateTotals: recount, share */
/// if (realm[new].countyCount == 0 || County_BordersRealm(new, county)) {
///     realm[new].countyCount++;
///     if (new == g_localPlayer) {                /* category 0x0D, from 0, spare = old owner */
///         if (countyCount == g_countyCount - 1)  Msg(0x7B);   /* 123 "one more county" */
///         else if (peak < countyCount) {
///             if (peak < 2)       Msg(0x75);                  /* 117 "Bravo!!" */
///             else if (peak < 3)  Msg(0x76);                  /* 118 "a solid base" */
///             else if (share < 26) Msg(0x77); else if (share < 41) Msg(0x78);
///             else if (share < 61) Msg(0x79); else if (share < 81) Msg(0x7A);
///             else                 Msg(0x7D);                 /* 125 */
///         } else                  Msg(0x7E);                  /* 126 "may you rule it wisely" */
///     }
///     else if (county.owner == g_localPlayer) Msg(new, local, 0x73, category 0);  /* 115 */
///     else if (county.owner == 0)             Msg(new, local, 0x74, category 0);  /* 116 */
///     else                                    Msg(new, local, 0x72, category 0, spare = old); /* 114 */
///     …the write, the penalty, the shield, peak = max(peak, countyCount)…
/// } else {
///     if (new == g_localPlayer) Msg(0, local, 0x81, category 0);  /* 129 "too far … cannot govern it" */
///     County_MakeIndependent(county);
/// }
/// ```
///
/// # The `else` branch — a county too far to govern
///
/// **A county that borders none of the taker's lands, taken by a realm that
/// already holds one, is not given to the taker at all**: the taker is posted
/// 129 and the county is made independent. `[V]`,
/// — it does **not** increment `countyCount`, does not call
/// `Realm_RecountStrength` on the loser, does not take the happiness penalty,
/// does not write the shield and does not raise the peak. The one thing it does
/// is [`make_independent`], so this function needs a [`Restore`].
///
/// [`Capture::governable`] carries the test out to the letter layer, which
/// picks 129 for the taker and nothing for anyone else.
/// `docs/decisions.md` C197.
///
/// Returns what the letter is chosen from, the penalty included.
///
/// **Its first statement is the fog of war's**:
/// `if (newOwner == g_localPlayer) FUN_0046DFD5(county);` — the county taken is
/// seen, with a one-tile border, by the realm that took it. Before any of the
/// bookkeeping and whatever the option says; `crate::explore` has the table of
/// writers and why the realm stands in for the local player.
#[allow(clippy::too_many_arguments)]
pub fn change_owner(
    t: &Tables,
    counties: &mut [County; MAX_COUNTIES],
    realms: &mut [Realm; MAX_REALMS],
    units: &mut Units,
    new_owner: u8,
    county: u8,
    difficulty: u8,
    map: &CampaignMap,
    explored: &mut crate::explore::Explored,
    restore: Restore,
) -> Capture {
    explored.reveal_county(new_owner, map, county);
    let is_human = realms.get(new_owner as usize).is_some_and(|r| r.is_human);
    // `Realm_UpdateTotals(newOwner)`, the function's second statement: the
    // taker's holding as the map stands **before** this county is written.
    // Records above `g_countyCount` are zero, so owner 0, so never counted.
    let held_before = counties
        .iter()
        .skip(1)
        .filter(|c| c.owner == new_owner)
        .count()
        .min(u8::MAX as usize) as u8;
    let governable = held_before == 0 || crate::ai_army::county_borders_realm(counties, county, new_owner);
    let peak_before = realms.get(new_owner as usize).map_or(0, |r| r.peak_counties);
    let mut capture = Capture {
        new_owner,
        old_owner: 0,
        county,
        held_before,
        peak_before,
        governable,
        penalty: 0,
        garrison: None,
    };
    let Some(c) = counties.get_mut(county as usize) else { return capture };
    capture.old_owner = c.owner;

    // `else { if (newOwner == g_localPlayer) Msg(0x81); County_MakeIndependent(county); }`
    // — the whole of the branch. Nothing below this line runs for it: no
    // count, no penalty, no shield, no peak. The letter is the caller's, from
    // [`Capture::governable`].
    if !governable {
        let garrison = c.garrison_unit;
        make_independent(t, counties, realms, county, map, restore);
        // `County_MakeIndependent`'s last statement, `FUN_00437535(garrison,
        // county)` (`0x004AC3C6` tail, `0x00437535`) — **the hand-off**, `[V]`
        // from both bodies. The castle is nobody's, so its garrison marches
        // out: the *old* owner keeps the men as a field army on the nearest
        // free tile, the new owner gets no garrison at all, and nowhere to
        // stand is `Army_Destroy` — the men are lost. Only `FUN_00437535`
        // clears county `+0x1BC`; `0x004AC3C6` does not touch it itself.
        if garrison != 0 {
            capture.garrison = Some(hand_off(map, counties, realms, units, garrison, county));
        }
        recount_realm_counties(counties, realms);
        return capture;
    }

    c.owner = new_owner;
    let penalty = capture_happiness_penalty(is_human, difficulty);
    if c.happiness < penalty {
        c.shown_events -= c.happiness;
        c.happiness = 0;
    } else {
        c.happiness -= penalty;
        c.shown_events -= penalty;
    }

    // The original also writes county `+0x07` from the taker's shield byte, so
    // the county draws the new banner. [`County`] has no such field — it is
    // presentation,
    // would read it from.

    // The castle, if any, is no longer garrisoned by the loser. **`[V]` that
    // `County_ChangeOwner` (`0x004A72FE`) has no garrison statement of its own
    // in this branch** — read end to end, it writes owner, happiness, shield
    // and peak and never touches county `+0x1BC` or calls `FUN_00437535`; only
    // the `else` branch hands the garrison off, through
    // `County_MakeIndependent`. `[I]` that the eviction belongs here too: the
    // original reaches this branch with a foreign garrison only from
    // `Battle_ReturnToCampaign`, which has already cleared `+0x1BC` and
    // destroyed the loser, so its own state is never the dangling one. A
    // walk-in capture can reach it here, and `FUN_00437535` (`0x00437535`) is
    // the original's answer everywhere else it is asked — the men march out
    // under their old flag
    let garrison = c.garrison_unit;
    if garrison != 0 && units.get(garrison).map(|u| u.owner) != Some(new_owner) {
        capture.garrison = Some(hand_off(map, counties, realms, units, garrison, county));
    }

    // `if (peak < countyCount) peak = countyCount` — the function's last
    // statement, inside the governable branch, with `countyCount` the recount
    // plus one. **Plus one, not the true count**: a second call on a county the
    // taker already holds counts it twice, and `Battle_ReturnToCampaign` makes
    // that second call when a beaten garrison also carried a defence mark.
    if governable {
        if let Some(r) = realms.get_mut(new_owner as usize) {
            let after = capture.held_after();
            if r.peak_counties < after {
                r.peak_counties = after;
            }
        }
    }

    recount_realm_counties(counties, realms);
    capture.penalty = penalty;
    capture
}

/// Realm `+0x29` — how many counties each realm holds, and `+0x2A`, the most it
/// has ever held. Rebuilt, because a capture moves a
/// county between two realms and incrementing one without decrementing the
/// other is how a count drifts.
pub fn recount_realm_counties(counties: &[County; MAX_COUNTIES], realms: &mut [Realm; MAX_REALMS]) {
    for r in realms.iter_mut() {
        r.county_count = 0;
    }
    for c in counties.iter().skip(1) {
        if let Some(r) = realms.get_mut(c.owner as usize) {
            if c.owner != 0 {
                r.county_count = r.county_count.saturating_add(1);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Reaching the castle building
// ---------------------------------------------------------------------------


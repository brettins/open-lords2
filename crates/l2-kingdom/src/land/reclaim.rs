use super::*;

/// County `+0x210` — **which field the reclamation gang is working on.**
///
/// `FUN_0044C53B` and `FUN_0044C5FC` are the same three lines twice: the slot,
/// among the county's fields whose *terrain* says reclamation, with the
/// **highest progress**, ties going to the lowest slot. So the gang finishes
/// the nearly-done field before it starts the next one, and a county with two
/// hundred workers completes one field a season
/// together.
pub fn reclaim_leader(county: &County, map: &crate::map::CampaignMap) -> Option<usize> {
    let mut best: Option<(usize, i32)> = None;
    for slot in 0..MAX_FIELDS {
        let Some(tile) = county.field_tile(slot) else { continue };
        if crate::field::classify(map.terrain[tile]) != crate::field::FieldType::Reclaiming {
            continue;
        }
        let progress = county.field_progress[slot] as i32;
        if best.is_none_or(|(_, p)| p < progress) {
            best = Some((slot, progress));
        }
    }
    best.map(|(slot, _)| slot)
}

pub fn reclaim_terrain(t: &Tables, progress: i32) -> u8 {
    let max = t.field.progress_max;
    for quarter in 1..=4 {
        if progress < max * quarter / 4 {
            return crate::field::terrain::RECLAIM_FIRST + (quarter as u8 - 1);
        }
    }
    crate::field::terrain::FALLOW
}

/// `Field_ReclaimTick` (`0x0044C093`) — **spend the county's reclamation
/// labour.**
pub fn reclaim_fields(t: &Tables, county: &mut County, map: &mut crate::map::CampaignMap) {
    let mut budget = county.labour[crate::tables::JOB_FIELD_RECLAMATION];
    let start = reclaim_leader(county, map).unwrap_or(0);
    for step in 0..MAX_FIELDS {
        let slot = (start + step) % MAX_FIELDS;
        let Some(tile) = county.field_tile(slot) else { continue };
        if crate::field::classify(map.terrain[tile]) != crate::field::FieldType::Reclaiming {
            continue;
        }
        let mut progress = county.field_progress[slot] as i32;
        if budget <= t.field.reclaim_per_season {
            progress += budget;
            budget = 0;
        } else {
            progress += t.field.reclaim_per_season;
            budget -= t.field.reclaim_per_season;
        }
        if progress > t.field.progress_max {
            budget += progress - t.field.progress_max;
        }
        crate::field::paint_tile(map, tile, reclaim_terrain(t, progress));
        county.field_progress[slot] = progress.clamp(0, u16::MAX as i32) as u16;
        if budget < 1 {
            return;
        }
    }
}


/// **`Field_ReclaimEstimate`'s tail (`0x0044C278`) — the reclamation row's two
/// figures**, and the third unwritten tail of the evening.
///
/// ```c
/// county.field_0x20C = 0;  county.field_0x214 = 0;
/// if (county.reclaimLeadOr99 < 99) {
///     Field_ReclaimLeadSlot(county);                 /* +0x210, the nearest-done field */
///     left = county.labour[2].workers;
///     slot = county.field_0x210;
///     for (n = 0; n < 20; n++) {                     /* wrapping at 20 */
///         if (this slot is a field under reclamation) {
///             p = progress[slot];
///             if (left < 201) { p += left; left = 0; } else { p += 200; left -= 200; }
///             if (p > 799) { left += p - 800; county.field_0x20C += 1; }
///             if (left < 1) break;
///         }
///         slot = (slot + 1) % 20;
///     }
///     left = county.labour[2].workers;               /* re-read, not the remainder */
///     need = 800 - progress[county.field_0x210];
///     if (left > 0) {
///         if (left > 200) left = 200;
///         county.field_0x214 = need / left + (need % left != 0);   /* round up */
///     }
/// }
/// ```
///
/// **So `+0x20C` is a count of *fields finished next season*, not of work
/// done** — the player's "+1" is one field completed — and `+0x214` is *"seasons
/// to the next completed field"*, the number the same row draws beside it.
///
/// * **The gang works the nearest-to-finished field first** and wraps around the
///   twenty slots from there, so the labour is spent finishing
///   spread. `Field_ReclaimLeadSlot` (`0x0044C53B`) picks the highest progress,
///   ties to the lowest slot.
///
/// `[D]`, read out of `0x0044C278`. **C129.**
pub fn reclaim_preview(t: &Tables, county: &mut County, map: &crate::map::CampaignMap) {
    county.reclaim_fields_finishing = 0;
    county.reclaim_seasons_to_next = 0;
    let Some(lead) = reclaim_lead_slot(county, map) else { return };

    let mut is_reclaiming = [false; MAX_FIELDS];
    for (slot, flag) in is_reclaiming.iter_mut().enumerate() {
        *flag = county
            .field_tile(slot)
            .is_some_and(|tile| crate::field::classify(map.terrain[tile]) == FieldType::Reclaiming);
    }

    let mut left = county.labour[crate::tables::JOB_FIELD_RECLAMATION];
    let mut slot = lead;
    for _ in 0..MAX_FIELDS {
        if is_reclaiming[slot] {
            let mut p = county.field_progress[slot] as i32;
            if left <= t.field.reclaim_per_season {
                p += left;
                left = 0;
            } else {
                p += t.field.reclaim_per_season;
                left -= t.field.reclaim_per_season;
            }
            if p >= t.field.progress_max {
                left += p - t.field.progress_max;
                county.reclaim_fields_finishing += 1;
            }
            if left < 1 {
                break;
            }
        }
        slot = (slot + 1) % MAX_FIELDS;
    }

    let mut hands = county.labour[crate::tables::JOB_FIELD_RECLAMATION];
    let need = t.field.progress_max - county.field_progress[lead] as i32;
    if hands > 0 {
        hands = hands.min(t.field.reclaim_per_season);
        county.reclaim_seasons_to_next = need / hands + i32::from(need % hands != 0);
    }
}

/// `Field_ReclaimLeadSlot` (`0x0044C53B`) — county `+0x210`, the field slot with
/// the **highest** progress among those under reclamation, ties to the lowest
/// slot. `None` when nothing is being reclaimed, which is the original's `99`
/// sentinel from `FUN_0044C5FC`.
fn reclaim_lead_slot(county: &County, map: &crate::map::CampaignMap) -> Option<usize> {
    let mut best: Option<(usize, i32)> = None;
    for slot in 0..MAX_FIELDS {
        let Some(tile) = county.field_tile(slot) else { continue };
        if crate::field::classify(map.terrain[tile]) != FieldType::Reclaiming {
            continue;
        }
        let p = county.field_progress[slot] as i32;
        if best.is_none_or(|(_, b)| p > b) {
            best = Some((slot, p));
        }
    }
    best.map(|(slot, _)| slot)
}

/// `Field_ReclaimEstimate` (`0x0044C278`) — the **reclamation** ceiling: the
/// work outstanding in every field under reclamation, at most a season's worth
/// each.
///
/// **`[D]`.** The `> 0x18` test is the same one that puts a field in
/// [`County::fields_reclaiming`], so the two agree by construction.
pub fn reclaim_labour_estimate(
    t: &Tables,
    county: &County,
    map: &crate::map::CampaignMap,
) -> i32 {
    let mut total = 0;
    for slot in 0..MAX_FIELDS {
        let Some(tile) = county.field_tile(slot) else { continue };
        if crate::field::classify(map.terrain[tile]) != crate::field::FieldType::Reclaiming {
            continue;
        }
        let left = t.field.progress_max - county.field_progress[slot] as i32;
        total += left.clamp(0, t.field.reclaim_per_season);
    }
    total
}


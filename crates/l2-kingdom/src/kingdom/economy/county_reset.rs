//! `County_Reset` (`0x00451150`)'s **tail**, per county.

use super::*;

impl Kingdom {
    /// **`County_Reset` (`0x00451150`)'s closing six calls for one county**, run
    /// on *every* county — owned or not — before the seating.
    ///
    /// ```c
    /// County_RecountFields(county);
    /// Herd_UpdateCrowding(county);
    /// Herd_LabourEstimate(county, g_seasonNext);
    /// Field_ReclaimEstimate(county);
    /// Labour_Allocate(county);
    /// Ration_Apply(county, g_season);
    /// ```
    ///
    /// [V] — read straight off `00450000.c`. The opening numbers the loop above
    /// it writes are already ours ([`l2_scenario`]'s `county_reset`); this is
    /// the half that was not, and `l2-scenario`'s own new-game comparison said
    /// so in as many words: *"Labour_Allocate has not run on the map path —
    /// County_Reset's own call is not reproduced, so a new game's counties
    /// arrive unallocated."*
    ///
    /// **It is what staffs a county nobody owns.** `Settings::apply_to`'s
    /// [`Kingdom::settle_start_county`] rounds are `FUN_0049BD99`'s and reach
    /// the *start* counties only; the unowned ones are not allocated again until
    /// turn phase 1's `AI_ManageFields(0)` (`0x0049DFC6`) — which is *after* the
    /// opening season. So without this the whole neutral half of the map went
    /// into `Herd_SeasonTick` with nobody minding the cattle. Measured on a new
    /// England against `england-turn1.sav`: 0 cattle labour over the unowned
    /// counties against the original's 323, and a herd of 44 against 67.
    ///
    /// **Three estimates, not the eight of `County_RefreshEstimates`.** The
    /// original names the herd's and the reclamation's here and no others, so
    /// grain, the four industries and the castle keep the ceilings the record
    /// was zeroed to and the allocator deals against those. Reproduced as
    /// written; not tidied.
    ///
    /// Call order matters twice over: the crowding feeds the herd estimate, and
    /// both run at `County_Reset`'s population of 150 — the county-status row
    /// overwrites the population *afterwards*, without reallocating.
    pub fn reset_county_for_new_game(&mut self, county: usize) {
        if county == 0 || county > self.county_count || county >= self.counties.len() {
            return;
        }
        let season_next = self.season_next;
        let armies_eat = self.options.armies_eat;

        crate::field::recount(&mut self.counties[county], &self.campaign.map);
        crate::field::herd_update_crowding(
            &self.tables,
            &mut self.counties[county],
            &mut self.campaign.map,
        );

        // `Herd_LabourEstimate` — the search loop only, guarded by `popBand` as
        // the original guards it. A county with no people leaves the ceiling on
        // `LABOUR_UNSET`, which `Labour_Allocate` reads back as 0.
        if self.counties[county].pop_band != 0 {
            let herd =
                crate::land::herd_labour_estimate(&self.tables, &self.counties[county], season_next);
            let c = &mut self.counties[county];
            c.labour_wanted[crate::tables::JOB_CATTLE_FARMING] = herd.wanted;
            c.labour_useful[crate::tables::JOB_CATTLE_FARMING] = herd.useful;
        }
        crate::land::herd_preview(&self.tables, &mut self.counties[county], season_next);

        // `Field_ReclaimEstimate`, loop and tail — as
        // [`crate::field::refresh_estimates`] runs it.
        self.counties[county].labour_wanted[crate::tables::JOB_FIELD_RECLAMATION] =
            crate::county::LABOUR_NO_FLOOR;
        self.counties[county].labour_useful[crate::tables::JOB_FIELD_RECLAMATION] =
            crate::land::reclaim_labour_estimate(
                &self.tables,
                &self.counties[county],
                &self.campaign.map,
            );
        crate::land::reclaim_preview(&self.tables, &mut self.counties[county], &self.campaign.map);

        crate::labour::allocate(&mut self.counties[county]);
        // `Ration_Apply` is [`crate::ration::preview`]: it records and does not
        // spend, for the reason [`Kingdom::set_ration_wanted`] gives.
        crate::ration::preview(&self.tables, &mut self.counties[county], armies_eat, crate::ration::Sowing::from_index(self.season, self.options.advanced_farming));
    }
}

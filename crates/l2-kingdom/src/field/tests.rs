#![allow(unused_imports)]
use super::*;
use super::operations::*;
use super::ui::*;
use crate::county::{County, MAX_FIELDS};
use crate::map::CampaignMap;

#[cfg(test)]
mod tests {
    use super::*;

    const NO_REALMS: [crate::realm::Realm; 0] = [];
    use crate::tables::{Season, Tables, JOB_FIELD_RECLAMATION};

    fn county_with(n: usize) -> (County, CampaignMap) {
        let mut c = County::new();
        let mut map = CampaignMap::empty();
        for i in 0..n {
            let tile = crate::map::index(i as u8, 8);
            c.set_field_tile(i, Some(tile));
            map.terrain[tile] = terrain::FALLOW;
        }
        (c, map)
    }

    #[test]
    fn the_pasture_sweep_starts_after_the_cursor_and_leaves_it_on_what_it_took() {
        let (mut c, mut map) = county_with(6);
        c.pasture_cursor = 2;
        recount(&mut c, &map);
        ensure_pasture(&mut c, &mut map);
        assert_eq!(map.terrain[c.field_tile(3).unwrap()], terrain::PASTURE);
        assert_eq!(c.pasture_cursor, 3);
        assert_eq!((c.fields_cattle, c.fields_fallow), (1, 5), "the counts are recounted");
        ensure_pasture(&mut c, &mut map);
        assert_eq!(c.pasture_cursor, 3, "the cursor does not move when there is pasture");
    }

    /// No fallow left: `FUN_0046965A` takes a grain field instead, docks one
    /// field's share of the standing crop and steps `+0x206` down.
    #[test]
    fn with_no_fallow_the_sweep_eats_a_grain_field_and_docks_the_crop() {
        let (mut c, mut map) = county_with(4);
        for slot in 0..4 {
            map.terrain[c.field_tile(slot).unwrap()] = terrain::GRAIN;
        }
        c.crop[1] = 400;
        c.fields_grain_standing = 4;
        ensure_pasture(&mut c, &mut map);
        assert_eq!(map.terrain[c.field_tile(1).unwrap()], terrain::PASTURE);
        assert_eq!(c.pasture_cursor, 1);
        assert_eq!(c.crop[1], 300);
        assert_eq!(c.fields_grain_standing, 3, "`fieldsGrain <= +0x206`, so it steps down");
        assert_eq!((c.fields_grain, c.fields_cattle), (3, 1));
    }

    #[test]
    fn the_blight_sweep_walks_its_own_cursor_over_every_kind_of_field() {
        let (mut c, mut map) = county_with(3);
        map.terrain[c.field_tile(1).unwrap()] = terrain::PASTURE;
        blight_one_field(&mut c, &mut map, terrain::PARCHED);
        assert_eq!(map.terrain[c.field_tile(1).unwrap()], terrain::PARCHED);
        assert_eq!((c.blight_cursor, c.pasture_cursor), (1, 0), "`+0x15B`, not `+0x15A`");
        blight_one_field(&mut c, &mut map, terrain::FLOODED);
        assert_eq!(map.terrain[c.field_tile(2).unwrap()], terrain::FLOODED);
        // `FUN_0046942C`: both go back to waste next pass.
        clear_blight(&c, &mut map);
        recount(&mut c, &map);
        assert_eq!(c.fields_waste, 2);
        assert_eq!(map.terrain[c.field_tile(1).unwrap()], terrain::WASTE);
    }

    #[test]
    fn the_ladder_classifies_every_terrain_the_original_branches_on() {
        use FieldType::*;
        let want = [
            (0x00, Waste),
            (0x01, Fallow),
            (0x02, Grain),
            (0x0E, Grain),
            (0x0F, Pasture),
            (0x13, Pasture),
            (0x16, Pasture),
            (0x17, Waste),
            (0x18, Waste),
            (0x19, Reclaiming),
            (0x1C, Reclaiming),
            (0xFF, Reclaiming),
        ];
        for (terrain, kind) in want {
            assert_eq!(classify(terrain), kind, "terrain {terrain:#04x}");
        }
    }

    #[test]
    fn the_five_counts_always_sum_to_the_number_of_fields() {
        let (mut c, mut map) = county_with(20);
        for t in 0..=255u8 {
            for slot in 0..20 {
                map.terrain[c.field_tile(slot).unwrap()] = t;
            }
            recount(&mut c, &map);
            let total = c.fields_fallow
                + c.fields_cattle
                + c.fields_grain
                + c.fields_waste
                + c.fields_reclaiming;
            assert_eq!(total, 20, "terrain {t:#04x} counted {total} of 20 fields");
        }
    }

    #[test]
    fn painting_a_fallow_field_to_grain_gives_the_county_a_grain_field() {
        let mut counties = vec![County::new(); 3];
        let (c, mut map) = county_with(6);
        counties[1] = c;
        recount(&mut counties[1], &map);
        assert_eq!(counties[1].fields_grain, 0, "nothing is sown to begin with");
        assert_eq!(counties[1].fields_fallow, 6);

        let tile = counties[1].field_tile(0).unwrap();
        set_type(&mut counties, 2, &mut map, 1, tile, FieldType::Grain, Season::Spring, &Tables::DEFAULT, false, &NO_REALMS).unwrap();
        assert_eq!(counties[1].fields_grain, 1);
        assert_eq!(counties[1].fields_fallow, 5);
        assert_eq!(map.terrain[tile], terrain::GRAIN);
    }

    #[test]
    fn only_the_countys_own_field_tiles_can_be_painted() {
        let mut counties = vec![County::new(); 3];
        let (c, mut map) = county_with(4);
        counties[1] = c;
        let stranger = crate::map::index(40, 40);
        assert_eq!(
            set_type(&mut counties, 2, &mut map, 1, stranger, FieldType::Grain, Season::Spring, &Tables::DEFAULT, false, &NO_REALMS),
            Err(BrushRefusal::NotAField)
        );
    }

    #[test]
    fn a_field_cannot_be_reclaimed_and_waste_cannot_be_sown() {
        let mut counties = vec![County::new(); 3];
        let (c, mut map) = county_with(4);
        counties[1] = c;
        let tile = counties[1].field_tile(0).unwrap();

        assert_eq!(
            set_type(&mut counties, 2, &mut map, 1, tile, FieldType::Reclaiming, Season::Spring, &Tables::DEFAULT, false, &NO_REALMS),
            Err(BrushRefusal::WrongMenu),
            "a standing field has no reclaim button"
        );

        map.terrain[tile] = terrain::WASTE;
        assert_eq!(
            set_type(&mut counties, 2, &mut map, 1, tile, FieldType::Grain, Season::Spring, &Tables::DEFAULT, false, &NO_REALMS),
            Err(BrushRefusal::WrongMenu),
            "waste has to be reclaimed before it can be sown"
        );
        set_type(&mut counties, 2, &mut map, 1, tile, FieldType::Reclaiming, Season::Spring, &Tables::DEFAULT, false, &NO_REALMS).unwrap();
        assert_eq!(map.terrain[tile], terrain::RECLAIM_FIRST);

        map.terrain[tile] = terrain::FLOODED;
        assert_eq!(
            set_type(&mut counties, 2, &mut map, 1, tile, FieldType::Fallow, Season::Spring, &Tables::DEFAULT, false, &NO_REALMS),
            Err(BrushRefusal::Blighted),
            "a ruined field opens no menu at all"
        );
    }

    #[test]
    fn starting_a_reclamation_gives_the_job_a_share_of_the_farm() {
        let mut counties = vec![County::new(); 3];
        let (c, mut map) = county_with(4);
        counties[1] = c;
        counties[1].labour_share = [50, 50, 0, 0, 0, 0, 100, 0];
        let tile = counties[1].field_tile(0).unwrap();
        map.terrain[tile] = terrain::WASTE;

        set_type(&mut counties, 2, &mut map, 1, tile, FieldType::Reclaiming, Season::Spring, &Tables::DEFAULT, false, &NO_REALMS).unwrap();
        assert!(
            counties[1].labour_share[JOB_FIELD_RECLAMATION] > 0,
            "somebody has to do the reclaiming: {:?}",
            counties[1].labour_share
        );
        let farm: i32 = counties[1].labour_share[..3].iter().sum();
        assert_eq!(farm, 100, "and the farm group still closes");

        set_type(&mut counties, 2, &mut map, 1, tile, FieldType::Waste, Season::Spring, &Tables::DEFAULT, false, &NO_REALMS).unwrap();
        assert_eq!(counties[1].labour_share[JOB_FIELD_RECLAMATION], 0);
        assert_eq!(counties[1].labour_share[..3].iter().sum::<i32>(), 100);
    }

    #[test]
    fn setting_a_count_converts_up_and_down_and_never_creates_waste() {
        let (c, mut map) = county_with(12);
        assert_eq!(set_count(&c, &mut map, FieldType::Grain, 6), 0);
        let mut n = c.clone();
        recount(&mut n, &map);
        assert_eq!((n.fields_grain, n.fields_fallow, n.fields_waste), (6, 6, 0));

        set_count(&c, &mut map, FieldType::Grain, 6);
        recount(&mut n, &map);
        assert_eq!(n.fields_grain, 6, "asking for the same number changes nothing");

        set_count(&c, &mut map, FieldType::Grain, 2);
        recount(&mut n, &map);
        assert_eq!((n.fields_grain, n.fields_fallow, n.fields_waste), (2, 10, 0));

        assert_eq!(set_count(&c, &mut map, FieldType::Grain, 30), 18);
        recount(&mut n, &map);
        assert_eq!(n.fields_grain, 12);
    }

    #[test]
    fn clearing_a_type_matches_every_stage_of_it() {
        let (c, mut map) = county_with(3);
        for (slot, t) in [terrain::GRAIN, terrain::GRAIN_LAST, terrain::PASTURE_LAST]
            .into_iter()
            .enumerate()
        {
            map.terrain[c.field_tile(slot).unwrap()] = t;
        }
        clear_type(&c, &mut map, FieldType::Grain);
        let mut n = c.clone();
        recount(&mut n, &map);
        assert_eq!((n.fields_grain, n.fields_cattle, n.fields_fallow), (0, 1, 2));
    }
}


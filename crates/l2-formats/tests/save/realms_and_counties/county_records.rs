#![allow(unused_imports)]
use super::*;
use super::realms_and_map::*;
use super::*;
use super::structure::*;
use super::units_and_merchants::*;
use l2_formats::save::{Save, SaveError, COUNTY_RECORDS, NEIGHBOUR_SLOTS, REALM_RECORDS};
use l2_testkit::{executable, saves, skip, SaveFile};

#[test]
fn the_county_count_is_the_number_of_county_records() {
    let saves = saves!();
    for s in &saves {
        let g = s.save.globals().expect("globals");
        let counties = s.save.counties().expect("counties");
        let real = counties.iter().filter(|c| c.is_county()).count();
        assert_eq!(real as i32, g.county_count, "{}: g_counties", s.label());
        assert!(g.county_count >= 1, "{}: a map with no counties", s.label());
        assert!(g.county_count as usize <= COUNTY_RECORDS - 1, "{}", s.label());

        for c in counties.iter() {
            let expected = c.index >= 1 && c.index as i32 <= g.county_count;
            assert_eq!(c.is_county(), expected, "{}: record {}", s.label(), c.index);
        }
        eprintln!("{}: {} counties", s.label(), g.county_count);
    }
}

#[test]
fn the_addressed_read_and_the_record_read_agree() {
    let saves = saves!();
    for s in &saves {
        for c in s.save.counties().expect("counties").iter() {
            let base = l2_formats::save::COUNTY_BASE
                + (c.index * l2_formats::save::COUNTY_STRIDE) as u32;
            assert_eq!(s.save.u8_at(base + 0x05).unwrap(), c.owner, "{}", s.label());
            assert_eq!(s.save.i8_at(base + 0x0C).unwrap(), c.happiness, "{}", s.label());
            assert_eq!(s.save.i32_at(base + 0x24).unwrap(), c.population, "{}", s.label());
            assert_eq!(s.save.i32_at(base + 0x28).unwrap(), c.pop_last, "{}", s.label());
        }
    }
}

#[test]
fn opening_the_same_bytes_twice_reads_the_same_save() {
    let Some(exe) = executable() else {
        skip!("no Lords2.exe to read the block table from");
    };
    let saves = saves!();
    for s in &saves {
        let bytes = std::fs::read(&s.path).expect("re-read");
        let again = Save::open(&exe, &bytes).expect("opens");
        assert_eq!(again.counties().unwrap(), s.save.counties().unwrap(), "{}", s.label());
        assert_eq!(again.globals().unwrap(), s.save.globals().unwrap(), "{}", s.label());
    }
}


//! `Battle_Start` (`0x004778A0`) reaches `Battlefield_BuildRandom`
//! (`0x0047AAA3`) whenever; this is the seam between
//! [`l2_game::batfield`], the process global the original reads
//! `batfield.pl8` out of, and the muster the runner deploys onto it.

use l2_sim::runner::{blank_field, BattleRunner, Muster};
use l2_sim::terrain::{id, FieldSheets, CELLS, DIM, RASTER_BYTES};
use l2_sim::{Battlefield, Troop};

/// One synthetic `batfield.pl8`: a directory record at `+0x0C` and one 80 x 80
/// plane with water, woodland, hills and a marker pair per side.
fn file() -> Vec<u8> {
    let mut bytes = vec![0u8; 1000 + RASTER_BYTES];
    let off = 1000usize;
    bytes[0x0C] = (off & 0xFF) as u8;
    bytes[0x0D] = ((off >> 8) & 0xFF) as u8;
    let plane = &mut bytes[off..];
    for y in 30..40 {
        for x in 10..20 {
            plane[y * DIM + x] = 0x09; // water
            plane[y * DIM + x + 20] = 0x0A; // woodland
            plane[y * DIM + x + 40] = 0x02; // hills
        }
    }
    for (b, y) in [(0x04u8, 20usize), (0x0F, 60)] {
        plane[y * DIM + 40] = b;
        plane[(y + 1) * DIM + 40] = b;
        plane[y * DIM + 42] = 0x40;
    }
    bytes
}

fn kinds(f: &Battlefield) -> usize {
    let mut seen = [false; 256];
    for c in &f.cells {
        seen[c.terrain as usize] = true;
    }
    seen.iter().filter(|s| **s).count()
}

#[test]
fn a_field_battle_deploys_onto_terrain_and_a_blank_field_has_none() {
    // **The ablation first**, because the publish below is irreversible: this
    // is what every field battle used to get, and what a checkout with no
    // install still gets.
    let blank = blank_field();
    assert_eq!(kinds(&blank), 1, "the blank template is open ground and nothing else");
    assert!(blank.cells.iter().all(|c| c.terrain == id::OPEN));

    let sheets = FieldSheets::parse(&file()).expect("one whole plane");
    assert_eq!(sheets.len(), 1);
    assert!(l2_game::batfield::publish(sheets));
    assert!(l2_game::batfield::loaded());

    let field = l2_game::batfield::field(0x5EED);
    assert!(kinds(&field) > 1, "a built field carries more than open ground");
    let count = |t: u8| field.cells.iter().filter(|c| c.terrain == t).count();
    assert_eq!(count(id::WATER), 100);
    assert_eq!(count(id::WOODLAND), 100);
    assert_eq!(count(id::OBSTACLE), 100);
    assert_eq!(count(id::OPEN), CELLS - 300);
    assert_eq!(field.deploy_side0[0], (42, 20));
    assert_eq!(field.deploy_side4[0], (42, 60));

    let a: &[(Troop, u32)] = &[(Troop::Swordsmen, 4)];
    let d: &[(Troop, u32)] = &[(Troop::Archers, 4)];
    let runner = BattleRunner::deploy_muster(
        l2_game::batfield::field(0x5EED),
        0x5EED,
        Muster { troops: a, owner: 1, human: true },
        Muster { troops: d, owner: 2, human: false },
    );
    assert!(runner.field.cells.iter().any(|c| c.impassable()));
    assert!(!blank.cells.iter().any(|c| c.impassable()));
}

/// The shipped file, when there is one: 48 fields of exactly 6,400 bytes, the
/// directory record 16 bytes wide with its 24-bit offset at `+0x0C`. **[V]**
#[test]
fn the_shipped_batfield_holds_forty_eight_fields() {
    let Some(dir) = l2_testkit::install_dir() else {
        println!("SKIP the_shipped_batfield_holds_forty_eight_fields: no LORDS2_DIR");
        return;
    };
    let path = dir.join(FieldSheets::FILE);
    let Ok(bytes) = std::fs::read(&path) else {
        println!("SKIP the_shipped_batfield_holds_forty_eight_fields: no {}", path.display());
        return;
    };
    let sheets = FieldSheets::parse(&bytes).expect("batfield.pl8 parses");
    assert_eq!(sheets.len(), FieldSheets::PLAYLIST, "the playlist at 0x0057CAE0 scatters 48");
    let mut featured = 0;
    let mut across = [false; 256];
    for map in 0..sheets.len() {
        let f = l2_sim::terrain::build_field(sheets.get(map).unwrap(), 1);
        assert!(kinds(&f) >= 2, "field {map} is not even open ground");
        featured += usize::from(kinds(&f) >= 3);
        assert_ne!(f.home_side0, (0, 0), "field {map} has no side-0 marker");
        assert_ne!(f.home_side4, (0, 0), "field {map} has no side-4 marker");
        assert!(f.cells.iter().all(|c| c.elevation == 0), "field {map} is not flat");
        for c in &f.cells {
            across[c.terrain as usize] = true;
        }
    }
    assert!(featured >= 40, "only {featured} of 48 fields carry terrain");
    for t in [id::WATER, id::OBSTACLE, id::ROCKS] {
        assert!(across[t as usize], "no field anywhere carries terrain {t:#04x}");
    }
    assert!(!across[id::WOODLAND as usize]);
}

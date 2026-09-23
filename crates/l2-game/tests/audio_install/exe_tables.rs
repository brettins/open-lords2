use super::*;
use l2_game::audio::{names, LORD_STING};

/// `char[n][16]` at `va`, each a NUL-terminated file name.
fn table(exe: &[u8], va: u32, n: usize) -> Vec<String> {
    let base = l2_testkit::pe::va_to_offset(exe, va).expect("the table is in the image");
    (0..n)
        .map(|i| {
            let row = &exe[base + i * 16..base + i * 16 + 16];
            let end = row.iter().position(|&b| b == 0).expect("NUL within 16 bytes");
            String::from_utf8_lossy(&row[..end]).into_owned()
        })
        .collect()
}

/// The spoken-line tables are transcribed from `.data`, not generated; each is
/// read back out of `Lords2.exe` here. Ablation: `S020_04` → `S020_05` at
/// `POPULATION_HEALTH[4]` fails (the 0x004E2058 duplicate).
#[test]
fn the_voice_tables_are_the_executables_bytes() {
    let dir = l2_testkit::install!();
    let exe = std::fs::read(dir.join("Lords2.exe")).expect("the executable reads");
    let ours: [(u32, &[&str]); 7] = [
        (0x004D_F7B8, &names::speech::CONFIRM_PROMPT),
        (0x004D_F8B8, &names::speech::MERCENARY_OFFER),
        (0x004E_2058, &names::speech::POPULATION_HEALTH),
        (0x004E_20D8, &names::speech::PICKED_UNIT),
        (0x004E_2118, &names::speech::PICKED_CASTLE),
        (0x004E_2168, &names::speech::STANDINGS_CATEGORY),
        (0x004E_2470, &LORD_STING),
    ];
    for (va, names) in ours {
        let exe_names = table(&exe, va, names.len());
        for (i, (want, have)) in exe_names.iter().zip(names).enumerate() {
            assert!(want.eq_ignore_ascii_case(have), "{va:#010X}[{i}] is {want}, ours {have}");
        }
    }
}

/// `Panel_OpenPopulation` (`0x0043A8F2`) indexes 0x004E2058 by `healthBand`,
/// which `health_band` keeps in 0..=4: `S020_05.wav` ships, is named at [5], and
/// is never spoken.
#[test]
fn s020_05_ships_and_no_health_band_reaches_it() {
    let dir = l2_testkit::install!();
    let top = (-200..=200).map(l2_kingdom::tables::health_band).max().unwrap();
    assert_eq!(top, 4);
    assert_eq!(names::speech::POPULATION_HEALTH[5], "S020_05.wav");
    assert!(find(&dir, "S020_05.wav").is_some());
    assert!(!names::speech::POPULATION_HEALTH[..=top as usize].contains(&"S020_05.wav"));
}

//! Where the player's copy of the original is. [D] ours: `Lords2.exe` runs from
//! its own directory and never looks for one (`Ini_ReadConfig` `0x0040ECC4`
//! reads sierra.ini for the CD drive and hardware keys, 00400000.c:8352-8410, never an install path).

use std::path::{Path, PathBuf};

pub const DIR_VAR: &str = "LORDS2_DIR";

/// `Assets::load` fails first without it.
pub const REQUIRED: &str = "L2_maps.dat";
pub const ANY_OF: [&str; 2] = ["Lords2.exe", "L2.eng"];

/// [V] `goggame-1207663263.info` in the GOG install; the key holds `path`.
pub const GOG_KEY: &str = r"SOFTWARE\WOW6432Node\GOG.com\Games\1207663263";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    Argument,
    Environment,
    Remembered,
    BesideExe,
    Store,
    Common,
    Picked,
}

/// Case-insensitive: the GOG install has `L2_maps.dat`, other copies upper case.
pub fn is_install(dir: &Path) -> bool {
    let Ok(entries) = std::fs::read_dir(dir) else { return false };
    let names: Vec<String> =
        entries.flatten().map(|e| e.file_name().to_string_lossy().to_ascii_lowercase()).collect();
    let has = |f: &str| names.iter().any(|n| *n == f.to_ascii_lowercase());
    has(REQUIRED) && ANY_OF.iter().any(|f| has(f))
}

/// [I] defaults, none read from an installer: GOG's two, Steam's, and Sierra's
/// `C:\SIERRA\<game>` convention.
pub fn common(program_files_x86: Option<&str>, drive: &str) -> Vec<PathBuf> {
    let root = PathBuf::from(format!("{drive}\\"));
    let pf = program_files_x86.map(PathBuf::from).unwrap_or_else(|| root.join("Program Files (x86)"));
    vec![
        root.join("GOG Games").join("Lords of the Realm II"),
        pf.join("GOG Galaxy").join("Games").join("Lords of the Realm II"),
        pf.join("Steam").join("steamapps").join("common").join("Lords of the Realm II"),
        root.join("SIERRA").join("LORDS2"),
        root.join("Sierra").join("Lords of the Realm II"),
    ]
}

/// The first candidate that [`is_install`]; order is the caller's.
pub fn first(candidates: impl IntoIterator<Item = (Source, PathBuf)>) -> Option<(Source, PathBuf)> {
    candidates.into_iter().find(|(_, d)| is_install(d))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fake(name: &str, files: &[&str]) -> PathBuf {
        let d = std::env::temp_dir().join(format!("l2-install-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        for f in files {
            std::fs::write(d.join(f), b"").unwrap();
        }
        d
    }

    #[test]
    fn an_install_needs_the_maps_and_the_exe_or_the_strings() {
        let good = fake("good", &["L2_MAPS.DAT", "L2.eng"]);
        let no_maps = fake("nomaps", &["Lords2.exe", "L2.eng"]);
        let maps_only = fake("mapsonly", &["L2_maps.dat"]);
        assert!(is_install(&good), "names are matched case-insensitively");
        assert!(!is_install(&no_maps), "L2_maps.dat is required");
        assert!(!is_install(&maps_only), "one of Lords2.exe, L2.eng is required");
        assert!(!is_install(&good.join("absent")));
        for d in [good, no_maps, maps_only] {
            let _ = std::fs::remove_dir_all(d);
        }
    }

    #[test]
    fn the_first_valid_candidate_wins_in_the_callers_order() {
        let a = fake("a", &["L2_maps.dat", "Lords2.exe"]);
        let b = fake("b", &["L2_maps.dat", "Lords2.exe"]);
        let bad = fake("bad", &[]);
        let got = first([
            (Source::Argument, bad.clone()),
            (Source::Environment, b.clone()),
            (Source::Remembered, a.clone()),
        ]);
        assert_eq!(got, Some((Source::Environment, b.clone())));
        assert_eq!(first([(Source::Argument, bad.clone())]), None);
        for d in [a, b, bad] {
            let _ = std::fs::remove_dir_all(d);
        }
    }

    #[test]
    fn the_common_list_names_gog_steam_and_sierra() {
        let c = common(Some(r"D:\PF86"), "C:");
        assert!(c.contains(&PathBuf::from(r"C:\").join("GOG Games").join("Lords of the Realm II")));
        assert!(c.contains(&PathBuf::from(r"D:\PF86\Steam\steamapps\common\Lords of the Realm II")));
        assert!(c.contains(&PathBuf::from(r"C:\SIERRA\LORDS2")));
    }
}

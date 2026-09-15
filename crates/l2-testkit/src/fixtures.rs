#![allow(unused_imports)]
use super::*;

use std::path::{Path, PathBuf};
use l2_formats::save::{Save, SaveError};

pub fn find(dir: &Path, name: &str) -> Option<PathBuf> {
    std::fs::read_dir(dir).ok()?.filter_map(|e| e.ok()).map(|e| e.path()).find(|p| {
        p.file_name().and_then(|f| f.to_str()).is_some_and(|f| f.eq_ignore_ascii_case(name))
    })
}

pub fn read_install(name: &str) -> Option<Vec<u8>> {
    std::fs::read(find(&install_dir()?, name)?).ok()
}

pub fn executable() -> Option<Vec<u8>> {
    if let Some(dir) = fixtures_dir() {
        if let Some(p) = find(&dir, "Lords2.exe") {
            return std::fs::read(p).ok();
        }
    }
    read_install("Lords2.exe")
}


/// This is what an *invariant* runs over. One save agreeing proves nothing
/// about a format (`docs/decisions.md` C1); the point of a property like
/// neighbour-list symmetry is that it holds over all of them, including the
/// ones nobody has looked at.
pub fn every_available_save() -> Vec<SaveFile> {
    let Some(exe) = executable() else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for (dir, origin) in
        [(fixtures_dir(), Origin::Fixture), (install_dir(), Origin::Install)]
    {
        let Some(dir) = dir else { continue };
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        let mut paths: Vec<PathBuf> = entries
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| {
                p.extension().and_then(|e| e.to_str()).is_some_and(|e| e.eq_ignore_ascii_case("sav"))
            })
            .collect();
        paths.sort();
        for path in paths {
            let name = path
                .file_name()
                .and_then(|f| f.to_str())
                .unwrap_or_default()
                .to_ascii_lowercase();
            let Ok(bytes) = std::fs::read(&path) else { continue };
            match Save::open(&exe, &bytes) {
                Ok(save) => out.push(SaveFile { name, path, origin, save }),
                Err(e) => eprintln!("  {} does not open as a save: {e}", path.display()),
            }
        }
    }
    out
}


pub const ENGLAND_TURN1_FILE: &str = "england-turn1.sav";

pub const ENGLAND_TURN1_COUNTIES: [usize; 5] = [1, 4, 8, 11, 13];

pub const ENGLAND_COUNTIES: i32 = 14;

pub fn england_turn1() -> FixtureState {
    let Some(dir) = fixtures_dir() else {
        return FixtureState::Absent(format!(
            "{FIXTURES_VAR} is unset and {DEFAULT_FIXTURES} does not exist, so \
             {ENGLAND_TURN1_FILE} cannot be found. See l2_testkit::england_turn1."
        ));
    };
    let Some(path) = find(&dir, ENGLAND_TURN1_FILE) else {
        return FixtureState::Absent(format!(
            "{} holds no {ENGLAND_TURN1_FILE}. See l2_testkit::england_turn1 for what that \
             file must be and how to regenerate it.",
            dir.display()
        ));
    };
    let Some(exe) = executable() else {
        return FixtureState::Absent(format!(
            "{} is present, but no Lords2.exe is reachable and a save cannot be read without \
             the block table in the executable",
            path.display()
        ));
    };
    let wrong = |why: String| {
        FixtureState::WrongGame(format!(
            "{} is named as the England turn-one fixture but {why}.\n\
             The contract is documented at l2_testkit::england_turn1.",
            path.display()
        ))
    };
    let bytes = match std::fs::read(&path) {
        Ok(b) => b,
        Err(e) => return wrong(format!("cannot be read: {e}")),
    };
    let save = match Save::open(&exe, &bytes) {
        Ok(s) => s,
        Err(e) => return wrong(format!("does not open as a save: {e}")),
    };
    match england_turn1_fingerprint(&save) {
        Ok(()) => FixtureState::Ready(Box::new(save)),
        Err(why) => wrong(why),
    }
}

pub fn england_turn1_fingerprint(save: &Save) -> Result<(), String> {
    let g = save.globals().map_err(|e| format!("globals unreadable: {e}"))?;
    let want = [
        ("g_scenarioIndex", g.scenario_index, 0),
        ("g_counties", g.county_count, ENGLAND_COUNTIES),
        ("g_turnCount", g.turn_count, 1),
        ("g_season", g.season, 4),
        ("g_year", g.year, 1268),
        ("g_localPlayer", g.local_player, 1),
    ];
    for (name, got, expect) in want {
        if got != expect {
            return Err(format!("{name} is {got}, expected {expect}"));
        }
    }
    let counties = save.counties().map_err(|e| format!("counties unreadable: {e}"))?;
    let owned: Vec<usize> =
        counties.iter().filter(|c| c.is_owned()).map(|c| c.index).collect();
    if owned != ENGLAND_TURN1_COUNTIES {
        return Err(format!(
            "the owned counties are {owned:?}, expected {ENGLAND_TURN1_COUNTIES:?}"
        ));
    }
    let mut realms: Vec<u8> = counties.iter().filter(|c| c.is_owned()).map(|c| c.owner).collect();
    realms.sort_unstable();
    if realms != [1, 2, 3, 4, 5] {
        return Err(format!("the owning realms are {realms:?}, expected one county each for 1..=5"));
    }
    Ok(())
}

pub fn england_county_of_realm(save: &Save, realm: u8) -> usize {
    save.counties()
        .expect("counties")
        .iter()
        .find(|c| c.owner == realm)
        .unwrap_or_else(|| panic!("no county is owned by realm {realm}"))
        .index
}


pub fn fixture_save(name: &str) -> Result<Save, String> {
    let dir = fixtures_dir().ok_or_else(|| {
        format!("{FIXTURES_VAR} is not set and {DEFAULT_FIXTURES} does not exist")
    })?;
    let path = find(&dir, name).ok_or_else(|| format!("{} holds no {name}", dir.display()))?;
    let exe = executable().ok_or_else(|| "no Lords2.exe to read the block table from".to_string())?;
    let bytes = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    Save::open(&exe, &bytes).map_err(|e| format!("{} does not open as a save: {e}", path.display()))
}


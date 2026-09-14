#![allow(unused_imports)]
use super::*;

use std::path::{Path, PathBuf};
use l2_formats::save::{Save, SaveError};

/// The install is inconsistent about casing, so every lookup is
/// case-insensitive — the same rule the mod overlay applies.
pub fn find(dir: &Path, name: &str) -> Option<PathBuf> {
    std::fs::read_dir(dir).ok()?.filter_map(|e| e.ok()).map(|e| e.path()).find(|p| {
        p.file_name().and_then(|f| f.to_str()).is_some_and(|f| f.eq_ignore_ascii_case(name))
    })
}

/// Read a named file out of the install.
pub fn read_install(name: &str) -> Option<Vec<u8>> {
    std::fs::read(find(&install_dir()?, name)?).ok()
}

/// `Lords2.exe`, which every save read needs: the block table that says what a
/// save file *is* lives in the executable, not in the save.
///
/// Looked for in the fixture directory first, so that a machine with preserved
/// saves and a copy of the executable beside them can run the save suite with
/// no install at all.
pub fn executable() -> Option<Vec<u8>> {
    if let Some(dir) = fixtures_dir() {
        if let Some(p) = find(&dir, "Lords2.exe") {
            return std::fs::read(p).ok();
        }
    }
    read_install("Lords2.exe")
}

// --- saves -----------------------------------------------------------------

/// Every save this machine can offer, fixtures first, each already opened
/// against the executable's block table.
///
/// This is what an *invariant* runs over. One save agreeing proves nothing
/// about a format (`docs/decisions.md` C1); the point of a property like
/// neighbour-list symmetry is that it holds over all of them, including the
/// ones nobody has looked at.
///
/// Files that do not open are reported and skipped
/// dropped — a `.sav` that is not a save is worth a line of output.
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
        // Sorted, because a test that iterates a directory is a test whose
        // failure message depends on the filesystem's mood.
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

// --- the England turn-one fixture ------------------------------------------

/// File name the [`england_turn1`] fixture is looked for under, inside
/// `LORDS2_FIXTURES`.
pub const ENGLAND_TURN1_FILE: &str = "england-turn1.sav";

/// The five counties somebody starts on, on the England map. **The set is
/// fixed; the assignment is not.**
///
/// This distinction cost the suite nine red tests, and it was only settled by
/// having two England turn-one saves to compare. Both give land at exactly
/// these five indices, and in both, realms 1 to 5 each take exactly one — but
/// *which* realm takes which county is rolled per game:
///
/// | county | first save | second save |
/// |---:|---:|---:|
/// | 1 | realm 5 | realm 4 |
/// | 4 | realm 4 | realm 2 |
/// | 8 | realm 1 | realm 5 |
/// | 11 | realm 3 | realm 3 |
/// | 13 | realm 2 | realm 1 |
///
/// County 11 agreeing twice is a coincidence, and is a fair warning about how
/// easily one playthrough reads as a rule: the old test asserted the whole
/// left-hand column as *the* scenario.
pub const ENGLAND_TURN1_COUNTIES: [usize; 5] = [1, 4, 8, 11, 13];

/// Counties on the England map, in seventeen records. Records 0, 15 and 16 are
/// array slots
pub const ENGLAND_COUNTIES: i32 = 14;

/// The England map at the start of turn one, Winter 1268: fourteen counties,
/// five of them owned, one realm each.
///
/// # This is not a shipped file
///
/// A clean GOG install ships **no saves at all** — verified by diffing a
/// pristine copy of the install against a played one: exactly six files differ,
/// five of them saves. The game writes `lastturn.sav` when turn one begins, so
/// the save this suite asserted against for months was produced by somebody in
/// an earlier session of this project starting a campaign and looking around.
/// Calling it "the shipped save" was wrong, and that wrong word is what made a
/// file the game rewrites every turn look permanent.
///
/// # Where it is looked for
///
/// `%LORDS2_FIXTURES%\england-turn1.sav`, and **nowhere else**. There is
/// deliberately no fall back to the install's `lastturn.sav`: that fall back is
/// the entire defect. Three directories on this machine hold a file of that
/// name and they are three different games, so a path is not an identity — the
/// fixture is a name plus [`england_turn1_fingerprint`], and a file that fails
/// the fingerprint is [`FixtureState::WrongGame`]
/// diff sixteen tests deep.
///
/// # Regenerating it
///
/// Start a new England campaign in the original game, let turn one begin, quit,
/// and copy that install's `lastturn.sav` to
/// `%LORDS2_FIXTURES%\england-turn1.sav`. It must then satisfy:
///
/// * `g_scenarioIndex` 0 — the England map;
/// * fourteen counties in seventeen records, 0, 15 and 16 being array slots;
/// * land at exactly [`ENGLAND_TURN1_COUNTIES`], with realms 1 to 5 taking one
///   each — **in any assignment**;
/// * turn 1, season 4 (Winter), year 1268, `g_localPlayer` 1.
///
/// Everything else the scenario tests assert — happiness 72 owned / 77 unowned,
/// `happinessLast` 65 everywhere, `healthMeter` 67, population 417 → 435 or
/// 456, and realm 5 starting short of food — follows from the rules given that
/// position, and is the point of the exercise.
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
    // No Lords2.exe is a missing *tool*, not a wrong fixture.
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

/// The identity check behind [`england_turn1`]. Separate so that a test can run
/// it over an arbitrary save and report *why* that save is not the fixture.
///
/// **What is in here and what is deliberately not.** Every clause holds across
/// two independently created England turn-one saves. The realm→county
/// assignment, the weather county and which county the human ends up on all
/// differ between those two, so none of them is here; asserting them is what
/// turned a regenerable fixture into an irreproducible one.
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

/// The county realm `r` starts on, in an England turn-one save.
///
/// The assignment is rolled per game, so a test that wants "realm 5's county"
/// has to ask the file
/// content of the correction that produced this function.
pub fn england_county_of_realm(save: &Save, realm: u8) -> usize {
    save.counties()
        .expect("counties")
        .iter()
        .find(|c| c.owner == realm)
        .unwrap_or_else(|| panic!("no county is owned by realm {realm}"))
        .index
}

// --- gates -----------------------------------------------------------------

/// Open a fixture save by file name.
pub fn fixture_save(name: &str) -> Result<Save, String> {
    let dir = fixtures_dir().ok_or_else(|| {
        format!("{FIXTURES_VAR} is not set and {DEFAULT_FIXTURES} does not exist")
    })?;
    let path = find(&dir, name).ok_or_else(|| format!("{} holds no {name}", dir.display()))?;
    let exe = executable().ok_or_else(|| "no Lords2.exe to read the block table from".to_string())?;
    let bytes = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    Save::open(&exe, &bytes).map_err(|e| format!("{} does not open as a save: {e}", path.display()))
}


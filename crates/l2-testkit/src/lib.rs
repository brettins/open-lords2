//! Where the test suite's inputs live, and what it does when they are missing.
//!
//! # The problem this crate exists to fix
//!
//! Ninety-one test functions in this workspace need a copy of the game. Each of
//! them found it for itself, with a copy of
//!
//! ```text
//! std::env::var("LORDS2_DIR").ok()...or_else(|| r"F:\games\Lords of the Realm II")
//! ```
//!
//! and each of them, when the copy was missing, printed a line to stderr and
//! returned green. Two consequences followed, and both had already bitten:
//!
//! * **CI runs no install, so those tests do not exist there.** They pass
//!   locally, they pass on CI, and the passing means two different things. The
//!   project's strongest evidence — the reproduction against a real save, the
//!   renderer against real sprites — was invisible in the place that gates
//!   merges.
//! * **`lastturn.sav` is not a fixture.** It is the game's rolling autosave,
//!   rewritten every turn a human plays. Nine tests asserted one particular
//!   game's numbers against it and stayed green for months because nobody had
//!   played; the first ten minutes of play turned them all red at once. A
//!   *path* was being treated as an *identity*.
//!
//! So this crate does three things: it puts the search for an install in one
//! place, it makes a fixture something you name and verify rather than a file
//! you happen to open, and it gives every gate a shape that
//! [`tests/census.rs`](../../tests/census.rs) can count.
//!
//! # Two directories, two variables
//!
//! | variable | default | holds |
//! |---|---|---|
//! | `LORDS2_DIR` | `F:\games\Lords of the Realm II` | the read-only game install: `Lords2.exe`, the `.pl8` art, `USER.SKR` |
//! | `LORDS2_FIXTURES` | `E:\dev\lords2-fixtures` | preserved saves, **outside** the install, which nothing rewrites |
//!
//! The install is read-only (`CLAUDE.md` rule 2) and its saves are volatile.
//! Fixtures live outside it precisely so that playing the game cannot destroy
//! them. Neither directory is ever committed: `.gitignore` refuses `*.sav`, and
//! `tests/census.rs` asserts that no `.sav` has crept into the tree.
//!
//! # Named fixtures
//!
//! A fixture is a *name*, a *file*, and a *fingerprint*. [`england_turn1`]
//! returns [`FixtureState::WrongGame`] rather than a `Save` when the file at the
//! expected path is some other game, so the three states stay distinct:
//!
//! * **ready** — the file is there and it is the right game;
//! * **absent** — nothing is configured, and the test skips, visibly;
//! * **wrong game** — something is there and it is not what the test describes,
//!   and the test *fails*, with a message saying so.
//!
//! Conflating the last two is how this went unnoticed: nine tests failing with
//! bare assertion diffs read as "our reader broke", when the truth was "this is
//! a different saved game".

use std::path::{Path, PathBuf};

pub use l2_formats::save::{Save, SaveError};

pub mod pe;

/// Environment variable naming the game install.
pub const INSTALL_VAR: &str = "LORDS2_DIR";

/// Environment variable naming the fixture directory.
pub const FIXTURES_VAR: &str = "LORDS2_FIXTURES";

/// Where the install is on the machine this project was developed on. A
/// default, not a requirement — `LORDS2_DIR` overrides it and CI sets neither.
///
/// It is spelled out in exactly one place so that the census can assert it is
/// spelled out in exactly one place.
pub const DEFAULT_INSTALL: &str = r"F:\games\Lords of the Realm II";

/// Likewise for the preserved saves. Deliberately **not** inside the install:
/// the install is read-only and its own saves are overwritten by play.
pub const DEFAULT_FIXTURES: &str = r"E:\dev\lords2-fixtures";

/// Environment variable naming the older DOS install, which a couple of tests
/// diff the Windows files against.
pub const DOS_INSTALL_VAR: &str = "LORDS2_DOS_DIR";

/// Where the DOS install is on this machine. Also read-only.
pub const DEFAULT_DOS_INSTALL: &str = r"F:\games\LORDS2";

/// The game install, or `None`.
pub fn install_dir() -> Option<PathBuf> {
    dir(INSTALL_VAR, DEFAULT_INSTALL)
}

/// The fixture directory, or `None`.
pub fn fixtures_dir() -> Option<PathBuf> {
    dir(FIXTURES_VAR, DEFAULT_FIXTURES)
}

/// The older DOS install, or `None`.
pub fn dos_install_dir() -> Option<PathBuf> {
    dir(DOS_INSTALL_VAR, DEFAULT_DOS_INSTALL)
}

fn dir(var: &str, default: &str) -> Option<PathBuf> {
    match std::env::var(var) {
        Ok(v) if Path::new(&v).is_dir() => Some(PathBuf::from(v)),
        // An explicitly set variable pointing nowhere is a mistake worth
        // hearing about, rather than a silent fall back to somebody else's
        // machine.
        Ok(v) if !v.is_empty() => {
            eprintln!("{var} is set to {v:?}, which is not a directory - ignoring it");
            None
        }
        _ => {
            let p = PathBuf::from(default);
            p.is_dir().then_some(p)
        }
    }
}

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

/// Where a save was found. Only [`Origin::Fixture`] is stable across a session
/// of play.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    /// `LORDS2_FIXTURES` — preserved, outside the install, nothing rewrites it.
    Fixture,
    /// Inside the game install. **Volatile.** `lastturn.sav`, `safeturn.sav`
    /// and `old_turn.sav` are rewritten every turn a human plays, and
    /// `incombat.sav` and `endedcom.sav` every battle. Fine to assert
    /// *invariants* over; never a fixture.
    Install,
}

/// One save file that is actually present, already opened.
pub struct SaveFile {
    /// The file name, lowercased, e.g. `battle-before.sav`.
    pub name: String,
    pub path: PathBuf,
    pub origin: Origin,
    pub save: Save,
}

impl SaveFile {
    /// How a failure message should name it.
    pub fn label(&self) -> String {
        match self.origin {
            Origin::Fixture => format!("fixture {}", self.name),
            Origin::Install => format!("install {} (volatile)", self.name),
        }
    }
}

/// Every save this machine can offer, fixtures first, each already opened
/// against the executable's block table.
///
/// This is what an *invariant* runs over. One save agreeing proves nothing
/// about a format (`docs/decisions.md` C1); the point of a property like
/// neighbour-list symmetry is that it holds over all of them, including the
/// ones nobody has looked at.
///
/// Files that do not open are reported and skipped rather than silently
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
/// array slots rather than places.
pub const ENGLAND_COUNTIES: i32 = 14;

/// What [`england_turn1`] found.
pub enum FixtureState {
    /// The file is there and it is the game the tests describe.
    Ready(Box<Save>),
    /// Nothing is configured. The test should skip, visibly.
    Absent(String),
    /// Something is configured and it is a different saved game. The test
    /// should **fail**, saying so.
    WrongGame(String),
}

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
/// the fingerprint is [`FixtureState::WrongGame`] rather than a bare assertion
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
/// has to ask the file rather than write down an index — which is the whole
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

/// The name cargo gave the running test, for a skip line that says which test
/// went quiet.
pub fn current_test() -> String {
    std::thread::current().name().unwrap_or("<test>").to_string()
}

/// Skip the current test, loudly enough to be found in a log.
///
/// Every gate in the workspace goes through one of the macros below, and
/// `l2-testkit/tests/census.rs` counts them. That is the whole mechanism: a
/// gate that is not counted is a gate that does not compile past the census.
#[macro_export]
macro_rules! skip {
    ($($arg:tt)*) => {{
        eprintln!("SKIP {}: {}", $crate::current_test(), format!($($arg)*));
        return;
    }};
}

/// The game install directory, or skip.
#[macro_export]
macro_rules! install {
    () => {
        match $crate::install_dir() {
            Some(d) => d,
            None => $crate::skip!(
                "no game install ({} unset and the default is absent)",
                $crate::INSTALL_VAR
            ),
        }
    };
}

/// `Lords2.exe`'s bytes, or skip. The oracle tests' gate.
#[macro_export]
macro_rules! executable {
    () => {
        match $crate::executable() {
            Some(e) => e,
            None => $crate::skip!("no Lords2.exe to read ({} unset)", $crate::INSTALL_VAR),
        }
    };
}

/// Every save this machine can offer, or skip when there are none. The gate an
/// **invariant** sits behind: it does not care which game it is handed.
#[macro_export]
macro_rules! saves {
    () => {{
        let found = $crate::every_available_save();
        if found.is_empty() {
            $crate::skip!(
                "no save files reachable (neither {} nor {} yields one)",
                $crate::INSTALL_VAR,
                $crate::FIXTURES_VAR
            );
        }
        found
    }};
}

/// The England turn-one fixture, or skip when it is absent — and **panic** when
/// something is there that is not it.
#[macro_export]
macro_rules! england {
    () => {
        match $crate::england_turn1() {
            $crate::FixtureState::Ready(s) => *s,
            $crate::FixtureState::Absent(why) => $crate::skip!("{why}"),
            $crate::FixtureState::WrongGame(why) => panic!("{why}"),
        }
    };
}

/// A named fixture from `LORDS2_FIXTURES`, or skip. Used by the army oracle,
/// whose three saves are one battle caught at three moments.
#[macro_export]
macro_rules! fixture {
    ($name:expr) => {
        match $crate::fixture_save($name) {
            Ok(s) => s,
            Err(why) => $crate::skip!("{why}"),
        }
    };
}

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


use std::path::{Path, PathBuf};

use l2_kingdom::tables::Tables;

use crate::game::Game;
use crate::save::{self, LoadError, EXTENSION};

pub const DIR_VAR: &str = "LORDS2_SAVES";

pub const APP_DIR: &str = "open-lords2";

/// How long a save name may be. The original's own file list holds 65-byte
/// records (`SaveLoad_DrawStatus` indexes `DAT_004e8790 + i * 0x41`), so 64
/// characters is the number it can display; anything longer would be a name the
/// list could not show.
pub const MAX_NAME: usize = 64;

#[derive(Debug)]
pub enum Error {
    NoDirectory,
    BadName(String),
    Io { path: PathBuf, detail: String },
    Load { name: String, detail: LoadError },
}

impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Error::NoDirectory => write!(
                f,
                "no save directory: neither {DIR_VAR} nor a profile directory is set"
            ),
            Error::BadName(n) => write!(f, "{n:?} is not a save name"),
            Error::Io { path, detail } => write!(f, "{}: {detail}", path.display()),
            Error::Load { name, detail } => write!(f, "{name}: {detail}"),
        }
    }
}

impl std::error::Error for Error {}

impl Error {
    /// `(found, expected)` when either layer refused the file on its format version. C239
    pub fn version_mismatch(&self) -> Option<(u32, u32)> {
        let Error::Load { detail, .. } = self else { return None };
        match detail {
            LoadError::UnsupportedVersion { found, supported } => Some((*found, *supported)),
            LoadError::Kingdom(l2_kingdom::save::LoadError::UnsupportedVersion { found, supported }) => {
                Some((*found, *supported))
            }
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub name: String,
    pub path: PathBuf,
    pub bytes: u64,
}

pub fn dir() -> Option<PathBuf> {
    if let Some(d) = SCOPED.with(|s| s.borrow().clone()) {
        return Some(d);
    }
    if let Ok(v) = std::env::var(DIR_VAR) {
        if !v.is_empty() {
            return Some(PathBuf::from(v));
        }
    }
    Some(data_home()?.join(APP_DIR).join("saves"))
}

thread_local! {
    static SCOPED: std::cell::RefCell<Option<PathBuf>> = const { std::cell::RefCell::new(None) };
}

pub fn scoped_dir(path: impl Into<PathBuf>) -> ScopedDir {
    let previous = SCOPED.with(|s| s.borrow_mut().replace(path.into()));
    ScopedDir { previous, _not_send: core::marker::PhantomData }
}

#[must_use = "the directory is scoped only while the guard is alive"]
pub struct ScopedDir {
    previous: Option<PathBuf>,
    _not_send: core::marker::PhantomData<*const ()>,
}

impl Drop for ScopedDir {
    fn drop(&mut self) {
        let previous = self.previous.take();
        SCOPED.with(|s| *s.borrow_mut() = previous);
    }
}

fn data_home() -> Option<PathBuf> {
    if cfg!(windows) {
        if let Ok(v) = std::env::var("APPDATA") {
            if !v.is_empty() {
                return Some(PathBuf::from(v));
            }
        }
    }
    if let Ok(v) = std::env::var("XDG_DATA_HOME") {
        if !v.is_empty() {
            return Some(PathBuf::from(v));
        }
    }
    let home = std::env::var("HOME").ok().filter(|h| !h.is_empty())?;
    Some(PathBuf::from(home).join(".local").join("share"))
}

pub fn is_valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= MAX_NAME
        && name != "."
        && name != ".."
        && !name.starts_with('.')
        && !name.starts_with(' ')
        && !name.ends_with(' ')
        && !name.ends_with('.')
        && name.chars().all(|c| {
            !c.is_control() && !matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|')
        })
}

pub fn path_for(name: &str) -> Result<PathBuf, Error> {
    if !is_valid_name(name) {
        return Err(Error::BadName(name.to_string()));
    }
    let dir = dir().ok_or(Error::NoDirectory)?;
    Ok(dir.join(format!("{name}.{EXTENSION}")))
}

pub fn list() -> Vec<Entry> {
    let Some(dir) = dir() else { return Vec::new() };
    let Ok(entries) = std::fs::read_dir(&dir) else { return Vec::new() };
    let mut out: Vec<Entry> = entries
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let path = e.path();
            if !path.extension().is_some_and(|x| x.eq_ignore_ascii_case(EXTENSION)) {
                return None;
            }
            let name = path.file_stem()?.to_str()?.to_string();
            let bytes = e.metadata().ok().map_or(0, |m| m.len());
            Some(Entry { name, path, bytes })
        })
        .collect();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

pub fn write(name: &str, game: &Game) -> Result<PathBuf, Error> {
    write_bytes(name, &save::encode(game))
}

/// [`write`] of a world already encoded, as the host's season save is (C257).
pub fn write_bytes(name: &str, bytes: &[u8]) -> Result<PathBuf, Error> {
    let path = path_for(name)?;
    let dir = path.parent().expect("path_for joins a directory").to_path_buf();
    std::fs::create_dir_all(&dir).map_err(|e| io(&dir, e))?;

    let temp = path.with_extension(format!("{EXTENSION}.part"));
    std::fs::write(&temp, bytes).map_err(|e| io(&temp, e))?;
    std::fs::rename(&temp, &path).map_err(|e| {
        let _ = std::fs::remove_file(&temp);
        io(&path, e)
    })?;
    Ok(path)
}

pub fn read(name: &str, tables: Tables) -> Result<Game, Error> {
    let path = path_for(name)?;
    read_path(&path, tables)
}

pub fn read_path(path: &Path, tables: Tables) -> Result<Game, Error> {
    let bytes = std::fs::read(path).map_err(|e| io(path, e))?;
    let name = path.file_stem().and_then(|s| s.to_str()).unwrap_or("").to_string();
    let mut game = save::decode(&bytes, tables).map_err(|detail| Error::Load { name, detail })?;
    // `Save_Read` (`0x004ADFE6`) closes the file and calls `FUN_004976A1` —
    // `004a0000.c:5952-5956`. The blocks are the codec; this is the loader.
    save::resume(&mut game);
    alone(&mut game);
    Ok(game)
}

/// C257: a network game's save (it names an owner) loaded by one player: every
/// other human realm is the computer's, as C240's survivor's were.
fn alone(game: &mut Game) {
    if game.owners.is_empty() {
        return;
    }
    let realms = game.kingdom.realms.iter().enumerate();
    let others: Vec<u8> = realms.filter(|(r, x)| x.is_human && *r != game.player as usize).map(|(r, _)| r as u8).collect();
    for r in others {
        if let Some(realm) = game.kingdom.realms.get_mut(r as usize) {
            realm.is_human = false;
        }
        for (_, u) in game.kingdom.campaign.units.iter_mut().filter(|(_, u)| u.owner == r) {
            u.owner_is_human = false;
        }
    }
}

/// **`Save_RotateAndWrite`'s three names** (`0x0049A453`), newest first.
///
/// The original's are `lastturn.sav`, `old_turn.sav` and `safeturn.sav` — three
/// 13-byte literals at `0x004DC2F0`, `0x004DC300` and `0x004DC310`, `[V]` read
/// out of `.rdata`. The stems are kept and the extension is ours.
pub const AUTOSAVES: [&str; 3] = ["lastturn", "old_turn", "safeturn"];

/// **`Save_RotateAndWrite` (`0x0049A453`)** — shift the window down one and
/// write the newest.
///
/// ```c
/// if (DAT_00553260 < 1) {
///     remove(safeturn);  rename(old_turn, safeturn);  rename(lastturn, old_turn);
/// }
/// Save_Write(lastturn);
/// ```
///
/// `[V]`, decompiled. So it keeps **three** turns, not a numbered series, and
/// the newest name is rewritten every time: `lastturn` is the turn that just
/// began, `old_turn` the one before it, `safeturn` the one before that. That is
/// the shape the player asked for — *"there doesn't seem to be a last-turn
/// autosave either so I can't easily repro that for you"* — and three deep is
/// what makes the turn *before* the one that went wrong reachable too.
///
/// `DAT_00553260` is not built. It is set to 2 by `FUN_0049B973`, the
/// multiplayer com-link-error path that reloads `lastturn.sva`/`.svb`, and
/// decremented per turn — *don't shift the snapshot we just resynced from out
/// of the window*. `FUN_0043F01D`, the network bring-up, zeroes it. In a single
/// player game it is zero and the rotation always runs. `[V]` on both writers.
pub fn rotate_and_write(game: &Game) -> Result<PathBuf, Error> {
    rotate_and_write_as(AUTOSAVES, &save::encode(game))
}

/// C257: a network host's three, apart from single player's as the original's `.sva`
/// triple is (`Save_RotateAndWrite` `0x0049A453`, `g_netIsMaster`).
pub const NET_AUTOSAVES: [&str; 3] = ["net-lastturn", "net-old_turn", "net-safeturn"];

pub fn rotate_and_write_as(names: [&str; 3], bytes: &[u8]) -> Result<PathBuf, Error> {
    let [newest, middle, oldest] = names;
    let (newest, middle, oldest) = (path_for(newest)?, path_for(middle)?, path_for(oldest)?);
    let _ = std::fs::remove_file(&oldest);
    let _ = std::fs::rename(&middle, &oldest);
    let _ = std::fs::rename(&newest, &middle);
    write_bytes(names[0], bytes)
}

/// C257: the host's season save, encoded by `net::frame` at the tick the season
/// ended, written between ticks (D-5). A joiner never holds one.
pub fn write_season(game: &mut Game) -> Option<Result<PathBuf, Error>> {
    let bytes = game.season_save.take()?;
    Some(rotate_and_write_as(NET_AUTOSAVES, &bytes))
}

/// **`FUN_0049A3E6`'s last statement, pumped** — the machine's standing autosave
/// request, carried out.
pub fn run_pending(
    machine: &mut crate::screen::Machine,
    game: &Game,
) -> Option<Result<PathBuf, Error>> {
    // C257: a network game's is the host's, written by `net::Live` at the season's tick.
    (machine.take_autosave() && !game.net.lockstep).then(|| rotate_and_write(game))
}

pub fn remove(name: &str) -> Result<(), Error> {
    let path = path_for(name)?;
    match std::fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(io(&path, e)),
    }
}

fn io(path: &Path, e: std::io::Error) -> Error {
    Error::Io { path: path.to_path_buf(), detail: e.to_string() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_name_that_could_name_a_path_is_refused_rather_than_scrubbed() {
        assert!(is_valid_name("Winter 1268"));
        assert!(is_valid_name("autosave"));
        assert!(is_valid_name("a"));

        for bad in [
            "",
            ".",
            "..",
            ".hidden",
            "../../etc/passwd",
            "sub/dir",
            r"sub\dir",
            "C:evil",
            "trailing ",
            " leading",
            "trailing.",
            "with\nnewline",
            "wild*card",
        ] {
            assert!(!is_valid_name(bad), "{bad:?} was accepted as a save name");
        }
        assert!(!is_valid_name(&"x".repeat(MAX_NAME + 1)), "longer than the list can show");
        assert!(is_valid_name(&"x".repeat(MAX_NAME)));
    }

    /// C257: the fade's autosave is single player's; a network game's is the host's.
    /// Ablation: `!game.net.lockstep` in `run_pending`.
    #[test]
    fn the_screens_autosave_request_writes_nothing_in_a_network_game() {
        let dir = std::env::temp_dir().join(format!("l2-c257-fade-{}", std::process::id()));
        let _scope = scoped_dir(&dir);
        let mut machine = crate::screen::Machine::new(crate::screen::ScreenId::Campaign);
        machine.autosave = true;
        let mut game = Game::new(1);
        game.net.lockstep = true;
        assert!(run_pending(&mut machine, &game).is_none(), "a seat wrote the fade's autosave");
        assert!(!machine.autosave, "the request is spent all the same");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_refused_name_never_reaches_the_file_system() {
        assert!(matches!(path_for("../escape"), Err(Error::BadName(_))));
        assert!(matches!(path_for(""), Err(Error::BadName(_))));
    }

    #[test]
    fn the_directory_is_under_the_users_profile_and_never_inside_an_install() {
        let Some(d) = dir() else { return };
        let shown = d.display().to_string().to_ascii_lowercase();
        assert!(
            !shown.contains("lords of the realm"),
            "a save must never land inside the game install: {shown}"
        );
        assert!(
            shown.contains(APP_DIR) || std::env::var(DIR_VAR).is_ok(),
            "the default directory is named after this project: {shown}"
        );
    }
}

//! Finding the install and failing where a player can see it. [D] ours, C???:
//! a release build has no console (`windows_subsystem`), so stderr alone is lost.

use std::path::PathBuf;

use l2_game::install::{self, Source};

use crate::os;

const TITLE: &str = "open-lords2";

pub(crate) struct Args {
    pub dir: Option<PathBuf>,
    pub mods: Option<PathBuf>,
    pub sound: bool,
}

pub(crate) fn args() -> Args {
    let mut a = Args { dir: None, mods: None, sound: true };
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let mut it = argv.iter();
    while let Some(s) = it.next() {
        match s.as_str() {
            "--mods" => a.mods = Some(PathBuf::from(it.next().unwrap_or_else(|| crate::usage()))),
            "--no-sound" => a.sound = false,
            "-h" | "--help" => crate::usage(),
            other => a.dir = Some(PathBuf::from(other)),
        }
    }
    a
}

pub(crate) fn fatal(text: &str) -> ! {
    eprintln!("{text}");
    os::error(TITLE, text);
    std::process::exit(1)
}

/// Argument, `LORDS2_DIR`, the remembered path, beside the exe, GOG's registry
/// key, the common folders, then the picker. An argument that is not an
/// install is an error, not a fall-through: the player named it.
pub(crate) fn install_dir(arg: Option<PathBuf>, prefs: &l2_game::game::Prefs) -> PathBuf {
    if let Some(d) = arg {
        if !install::is_install(&d) {
            fatal(&not_an_install(&d));
        }
        return remembered(d, prefs);
    }
    let env = |k: &str| std::env::var(k).ok().filter(|v| !v.is_empty());
    let exe = std::env::current_exe().ok().and_then(|e| e.parent().map(PathBuf::from));
    let drive = env("SystemDrive").unwrap_or_else(|| "C:".into());
    let mut c: Vec<(Source, PathBuf)> = Vec::new();
    c.extend(env(install::DIR_VAR).map(|d| (Source::Environment, d.into())));
    c.extend(l2_game::prefs::remembered_install().map(|d| (Source::Remembered, d)));
    if let Some(e) = exe {
        c.push((Source::BesideExe, e.clone()));
        c.extend(e.parent().map(|p| (Source::BesideExe, p.to_path_buf())));
    }
    c.extend(os::registry_path(install::GOG_KEY, "path").map(|d| (Source::Store, d)));
    let pf86 = env("ProgramFiles(x86)");
    c.extend(install::common(pf86.as_deref(), &drive).into_iter().map(|d| (Source::Common, d)));
    if let Some((_, d)) = install::first(c) {
        return remembered(d, prefs);
    }
    remembered(pick(), prefs)
}

fn pick() -> PathBuf {
    let intro = "open-lords2 needs your own copy of Lords of the Realm II.\n\n\
        Choose the folder that holds Lords2.exe and L2_maps.dat.";
    if !os::ask(TITLE, intro) {
        std::process::exit(2);
    }
    loop {
        match os::pick_folder("The Lords of the Realm II folder (Lords2.exe, L2_maps.dat)") {
            Some(d) if install::is_install(&d) => return d,
            Some(d) => os::error(TITLE, &not_an_install(&d)),
            None => std::process::exit(2),
        }
    }
}

fn not_an_install(d: &std::path::Path) -> String {
    format!(
        "{} is not a Lords of the Realm II folder: it needs {} and one of {}.",
        d.display(),
        install::REQUIRED,
        install::ANY_OF.join(", ")
    )
}

fn remembered(d: PathBuf, prefs: &l2_game::game::Prefs) -> PathBuf {
    if l2_game::prefs::remembered_install().as_deref() != Some(d.as_path()) {
        if let Err(e) = l2_game::prefs::remember_install(prefs, &d) {
            eprintln!("preferences: {e}");
        }
    }
    d
}

/// Crash log beside the saves, then a box. `crate::crash` formats it.
pub(crate) fn panic_hook() {
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        default(info);
        let bt = std::backtrace::Backtrace::force_capture().to_string();
        let screen = l2_game::crash::screen();
        let text = l2_game::crash::report(&info.to_string(), screen.as_deref(), &bt);
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        let where_ = match l2_game::saves::dir().map(|d| l2_game::crash::write(&d, secs, &text)) {
            Some(Ok(p)) => format!("A report was written to\n{}", p.display()),
            _ => "No report could be written.".to_string(),
        };
        os::error(TITLE, &format!("open-lords2 stopped: {info}\n\n{where_}"));
    }));
}

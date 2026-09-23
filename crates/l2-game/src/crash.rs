//! The crash log. [D] ours: the original has no panic path to mirror; its
//! `Log_Write` lines are start-up diagnostics (`Display_Init` `0x0042E5C0`).
//! The clock is the caller's: `docs/netcode.md` D-5 keeps `SystemTime` out of
//! this library.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

static SCREEN: Mutex<String> = Mutex::new(String::new());

/// Called on every redraw; read only by the panic hook.
pub fn note_screen(s: &str) {
    if let Ok(mut g) = SCREEN.lock() {
        if *g != s {
            g.clear();
            g.push_str(s);
        }
    }
}

pub fn screen() -> Option<String> {
    SCREEN.try_lock().ok().map(|g| g.clone()).filter(|s| !s.is_empty())
}

/// `crash-YYYYMMDD-HHMMSS.log`, UTC.
pub fn file_name(unix_secs: i64) -> String {
    let (y, m, d) = civil(unix_secs.div_euclid(86_400));
    let t = unix_secs.rem_euclid(86_400);
    format!("crash-{y:04}{m:02}{d:02}-{:02}{:02}{:02}.log", t / 3600, t / 60 % 60, t % 60)
}

/// Days since 1970-01-01 to a proleptic Gregorian date (Hinnant's algorithm).
fn civil(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

pub fn report(message: &str, screen: Option<&str>, backtrace: &str) -> String {
    format!(
        "open-lords2 crash\nbuild: {}\nscreen: {}\n\n{message}\n\nbacktrace:\n{backtrace}\n",
        crate::build_id::ID,
        screen.unwrap_or("unknown"),
    )
}

pub fn write(dir: &Path, unix_secs: i64, text: &str) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let path = dir.join(file_name(unix_secs));
    std::fs::write(&path, text)?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_file_name_is_the_utc_date_and_time() {
        assert_eq!(file_name(0), "crash-19700101-000000.log");
        assert_eq!(file_name(1_790_000_000), "crash-20260921-141320.log");
        assert_eq!(file_name(951_782_400), "crash-20000229-000000.log", "a leap day");
    }

    #[test]
    fn the_report_names_the_build_the_screen_and_the_backtrace() {
        let r = report("panicked at x.rs:1", Some("Map"), "0: main");
        assert!(r.contains(&format!("build: {}", crate::build_id::ID)));
        assert!(r.contains("screen: Map"));
        assert!(r.contains("panicked at x.rs:1") && r.contains("0: main"));
        assert!(report("m", None, "").contains("screen: unknown"));
    }

    #[test]
    fn the_last_noted_screen_is_what_the_hook_reads() {
        note_screen("Setup(Title)");
        note_screen("Map");
        assert_eq!(screen().as_deref(), Some("Map"));
    }

    #[test]
    fn the_log_is_written_where_it_is_told() {
        let dir = std::env::temp_dir().join(format!("l2-crash-{}", std::process::id()));
        let p = write(&dir, 0, "x").unwrap();
        assert_eq!(p, dir.join("crash-19700101-000000.log"));
        assert_eq!(std::fs::read_to_string(&p).unwrap(), "x");
        let _ = std::fs::remove_dir_all(dir);
    }
}

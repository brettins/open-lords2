//! CLAUDE.md rule 1 for the player zip: `tools/run/package.ps1`'s file list
//! names no game asset, and its one executable is ours.

use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().and_then(Path::parent).unwrap().to_path_buf()
}

const ASSETS: &[&str] = &["pl8", "256", "smk", "wav", "dat", "eng", "sav", "saf", "cur", "bmp", "skr", "inf", "dll", "hlp"];

/// Each `'src' = 'name'` / `"src" = 'name'` line between the markers.
fn entries(script: &str) -> Vec<(String, String)> {
    let a = script.find("# files:begin").expect("files:begin marker");
    let b = script.find("# files:end").expect("files:end marker");
    script[a..b]
        .lines()
        .skip(1)
        .filter_map(|l| l.split_once('='))
        .map(|(s, n)| {
            let q = |x: &str| x.trim().trim_matches(['\'', '"']).to_string();
            (q(s), q(n))
        })
        .collect()
}

fn ext(p: &str) -> String {
    Path::new(p).extension().map(|e| e.to_string_lossy().to_ascii_lowercase()).unwrap_or_default()
}

fn violations(list: &[(String, String)]) -> Vec<String> {
    let mut bad = Vec::new();
    for (src, name) in list {
        for p in [src, name] {
            let e = ext(p);
            if ASSETS.contains(&e.as_str()) || (e == "exe" && !p.ends_with("l2-game.exe")) {
                bad.push(p.clone());
            }
        }
    }
    bad
}

#[test]
fn the_package_list_holds_no_game_asset() {
    let script = std::fs::read_to_string(root().join("tools/run/package.ps1")).unwrap();
    let list = entries(&script);
    assert_eq!(list.len(), 4, "exe, LICENSE, NOTICE, README: {list:?}");
    assert!(list.iter().any(|(_, n)| n == "l2-game.exe"));
    assert_eq!(violations(&list), Vec::<String>::new());
}

/// The check itself: an asset, or a foreign exe, in the list is caught.
#[test]
fn an_asset_in_the_list_is_caught() {
    let script = "# files:begin\n  'x/L2.eng' = 'L2.eng'\n  'Lords2.exe' = 'Lords2.exe'\n  \
                  'a.PL8' = 'a.pl8'\n  \"$exe\" = 'l2-game.exe'\n  # files:end\n";
    let bad = violations(&entries(script));
    assert!(bad.contains(&"L2.eng".to_string()) && bad.contains(&"Lords2.exe".to_string()));
    assert!(bad.contains(&"a.PL8".to_string()));
    assert!(!bad.iter().any(|b| b.contains("l2-game")));
}

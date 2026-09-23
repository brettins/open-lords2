#![allow(unused_imports)]

mod dispositions;
pub use dispositions::*;
mod switches;
pub use switches::*;

use super::*;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use Disposition::{Retracted, Switchable, Unswitchable, Unwired};
use Home::{Behavioural, Presentation};

fn presentation_rows(root: &Path) -> usize {
    presentation(root).0.len()
}

pub(crate) fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().parent().unwrap().to_path_buf()
}

fn read(root: &Path, rel: &str) -> String {
    let p = root.join(rel);
    if p.file_name().is_some_and(|f| f == "mod.rs") {
        let dir = p.parent().unwrap();
        let mut names: Vec<_> = std::fs::read_dir(dir)
            .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|q| q.extension().is_some_and(|x| x == "rs"))
            .collect();
        names.sort();
        return names.iter().map(|q| std::fs::read_to_string(q).unwrap().replace("\r\n", "\n")).collect::<Vec<_>>().join("\n");
    }
    std::fs::read_to_string(&p)
        .unwrap_or_else(|e| panic!("{} is part of this check and could not be read: {e}", p.display()))
        .replace("\r\n", "\n")
}

fn catalogue(root: &Path) -> Vec<String> {
    catalogue_of(&read(root, "docs/bugs.md"))
}

fn catalogue_of(doc: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut inside = false;
    for line in doc.lines() {
        if line.starts_with("# 2. The catalogue") {
            inside = true;
            continue;
        }
        if inside && line.starts_with("# 3.") {
            break;
        }
        if !inside {
            continue;
        }
        let id = if let Some(rest) = line.strip_prefix("### ") {
            rest.split_whitespace().next().unwrap_or("")
        } else if let Some(rest) = line.strip_prefix("| **") {
            rest.split("**").next().unwrap_or("")
        } else if let Some(rest) = line.strip_prefix("| ~~**") {
            rest.split("**").next().unwrap_or("")
        } else {
            continue;
        };
        if is_entry_id(id) {
            out.push(id.to_string());
        }
    }
    out
}

fn is_entry_id(s: &str) -> bool {
    let Some(rest) = s.strip_prefix('B') else { return false };
    if let Some(slug) = rest.strip_prefix("NEW-") {
        return slug
            .split('-')
            .all(|run| !run.is_empty() && run.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'));
    }
    let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        return false;
    }
    let tail = &rest[digits.len()..];
    tail.is_empty() || (tail.len() == 1 && tail.chars().all(|c| c.is_ascii_lowercase()))
}

fn quirk_variants(root: &Path) -> Vec<(String, String)> {
    let src = read(root, "crates/l2-net/src/quirks/mod.rs");
    let body = src
        .split("pub const fn entry(self) -> &'static str {")
        .nth(1)
        .expect("l2_net::Quirk::entry is what this check joins on; it has moved or been renamed");
    let body = body.split("\n    }").next().unwrap_or(body);
    let mut out = Vec::new();
    for arm in body.split("Quirk::").skip(1) {
        let variant: String =
            arm.chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
        let Some(rest) = arm.split_once("=> ") else { continue };
        let entry: String = rest.1.trim_start().trim_start_matches('"').chars().take_while(|c| *c != '"').collect();
        if !variant.is_empty() && is_entry_id(&entry) {
            out.push((variant, entry));
        }
    }
    out
}

fn sources(root: &Path) -> Vec<(String, String)> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else { return };
        let mut paths: Vec<PathBuf> = entries.filter_map(|e| e.ok()).map(|e| e.path()).collect();
        paths.sort();
        for p in paths {
            if p.is_dir() {
                if p.file_name().is_some_and(|n| n == "target") {
                    continue;
                }
                walk(&p, out);
            } else if p.extension().is_some_and(|e| e == "rs") {
                out.push(p);
            }
        }
    }
    let mut paths = Vec::new();
    walk(&root.join("crates"), &mut paths);
    paths
        .into_iter()
        .map(|p| {
            let rel = p.strip_prefix(root).unwrap_or(&p).to_string_lossy().replace('\\', "/");
            let text = std::fs::read_to_string(&p).unwrap_or_default().replace("\r\n", "\n");
            (rel, text)
        })
        .collect()
}

fn presentation(root: &Path) -> (Vec<(String, String)>, Vec<String>) {
    let src = read(root, "crates/l2-game/src/game/mod.rs");

    let table = src
        .split("pub const PRESENTATION: &[(&str, &str)] = &[")
        .nth(1)
        .expect("l2_game::game::PRESENTATION is what this check joins on; it has moved");
    let table = table.split("];").next().unwrap_or("");
    let mut rows = Vec::new();
    for row in table.split('(').skip(1) {
        let quoted: Vec<&str> = row.split('"').skip(1).step_by(2).collect();
        if quoted.len() >= 2 {
            rows.push((quoted[0].to_string(), quoted[1].to_string()));
        }
    }

    let body = src
        .split("pub struct Quirks {")
        .nth(1)
        .expect("l2_game::game::Quirks is what this check joins on; it has moved");
    let body = body.split("\n}").next().unwrap_or("");
    let fields: Vec<String> = body
        .lines()
        .filter_map(|l| l.trim().strip_prefix("pub "))
        .filter_map(|l| l.split(':').next())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    (rows, fields)
}


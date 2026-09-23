#![allow(unused_imports)]
use super::*;
use super::furnished_state::*;
use super::campaign::*;
use super::round_trips::*;
use super::coverage::*;
use super::header::*;
use l2_kingdom::county::{ChangeReason, MAX_COUNTIES};
use l2_kingdom::realm::{Pair, MAX_REALMS};
use l2_kingdom::save::{
    checksum, decode, encode, ruleset_fingerprint, LoadError, HEADER_LEN, MAGIC, VERSION,
};
use l2_kingdom::tables::{Tables, Weather, JOB_COUNT};
use l2_kingdom::{Kingdom, Options};

pub(crate) mod census {
    use std::collections::{BTreeMap, BTreeSet};
    use std::path::{Path, PathBuf};

    pub type Structs = BTreeMap<String, Vec<(String, String)>>;

    pub fn repo_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().parent().unwrap().to_path_buf()
    }

    pub fn structs_of(dir: &Path) -> Structs {
        let mut out = Structs::new();
        fn walk(dir: &Path, files: &mut Vec<PathBuf>) {
            for e in std::fs::read_dir(dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display())).filter_map(|e| e.ok()) {
                let p = e.path();
                if p.is_dir() { walk(&p, files); } else if p.extension().is_some_and(|x| x == "rs") { files.push(p); }
            }
        }
        let mut files: Vec<PathBuf> = Vec::new();
        walk(dir, &mut files);
        files.sort();
        for file in files {
            let src = std::fs::read_to_string(&file).expect("a source file");
            parse_into(&src, &mut out);
        }
        out
    }

    fn parse_into(src: &str, out: &mut Structs) {
        let mut lines = src.lines().peekable();
        while let Some(line) = lines.next() {
            let Some(name) = struct_header(line) else { continue };
            let mut fields = Vec::new();
            for body in lines.by_ref() {
                if body == "}" {
                    break;
                }
                if let Some(field) = field_line(body) {
                    fields.push(field);
                }
            }
            out.entry(name).or_insert(fields);
        }
    }

    fn struct_header(line: &str) -> Option<String> {
        let rest = line
            .strip_prefix("pub(crate) struct ")
            .or_else(|| line.strip_prefix("pub struct "))
            .or_else(|| line.strip_prefix("struct "))?;
        let (name, tail) = rest.split_once(' ')?;
        if tail.trim() != "{" || !name.chars().all(|c| c.is_alphanumeric() || c == '_') {
            return None;
        }
        Some(name.to_string())
    }

    fn field_line(line: &str) -> Option<(String, String)> {
        let body = line.strip_prefix("    ")?;
        if body.starts_with(' ') || body.starts_with("//") || body.starts_with('#') {
            return None;
        }
        let body = body
            .strip_prefix("pub(crate) ")
            .or_else(|| body.strip_prefix("pub "))
            .unwrap_or(body);
        let (name, ty) = body.split_once(": ")?;
        if name.is_empty() || !name.chars().all(|c| c.is_lowercase() || c.is_numeric() || c == '_') {
            return None;
        }
        Some((name.to_string(), ty.trim_end_matches(',').to_string()))
    }

    pub fn reachable(structs: &Structs, root: &str, stop: &[(&str, &str, &str)]) -> BTreeSet<String> {
        let mut seen = BTreeSet::new();
        let mut queue = vec![root.to_string()];
        while let Some(name) = queue.pop() {
            let Some(fields) = structs.get(&name) else { continue };
            if !seen.insert(name.clone()) {
                continue;
            }
            for (field, ty) in fields {
                if listed(stop, &name, field).is_some() {
                    continue;
                }
                for token in identifiers(ty) {
                    if structs.contains_key(&token) {
                        queue.push(token);
                    }
                }
            }
        }
        seen
    }

    fn identifiers(ty: &str) -> Vec<String> {
        let mut out = Vec::new();
        let mut word = String::new();
        for c in ty.chars() {
            if c.is_alphanumeric() || c == '_' {
                word.push(c);
            } else if !word.is_empty() {
                out.push(std::mem::take(&mut word));
            }
        }
        if !word.is_empty() {
            out.push(word);
        }
        out
    }

    pub fn listed<'a>(
        table: &'a [(&'a str, &'a str, &'a str)],
        name: &str,
        field: &str,
    ) -> Option<&'a str> {
        table.iter().find(|(s, f, _)| *s == name && *f == field).map(|(_, _, note)| *note)
    }

    pub fn check_tables_are_live(structs: &Structs, table: &[(&str, &str, &str)], which: &str) {
        for (name, field, _) in table {
            let fields = structs
                .get(*name)
                .unwrap_or_else(|| panic!("{which} names struct `{name}`, which does not exist"));
            assert!(
                fields.iter().any(|(f, _)| f == field),
                "{which} names `{name}::{field}`, which is not a field of `{name}` - \
                 a stale exemption covers nothing. Delete the line."
            );
        }
    }

    pub fn fixture_region() -> String {
        let src = std::fs::read_to_string(
            repo_root().join("crates/l2-kingdom/tests/save/main.rs"),
        )
        .expect("this file");
        let begin = "// --- the fixture: begin ";
        let end = "// --- the fixture: end ";
        let from = src.find(begin).expect("the opening fixture marker");
        let to = src.find(end).expect("the closing fixture marker");
        assert!(to > from, "the fixture markers are the wrong way round");
        src[from..to]
            .lines()
            .map(|l| match l.find("//") {
                Some(at) => &l[..at],
                None => l,
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    pub fn mentions(region: &str, field: &str) -> bool {
        let dotted = format!(".{field}");
        let mut from = 0;
        while let Some(rel) = region[from..].find(&dotted) {
            let at = from + rel;
            let after = region[at + dotted.len()..].chars().next();
            match after {
                None => return true,
                Some(c) if !c.is_alphanumeric() && c != '_' && c != '(' => return true,
                _ => {}
            }
            from = at + 1;
        }
        let literal = format!("{field}:");
        let mut from = 0;
        while let Some(rel) = region[from..].find(&literal) {
            let at = from + rel;
            let before = region[..at].chars().next_back();
            match before {
                None => return true,
                Some(c) if !c.is_alphanumeric() && c != '_' && c != '.' && c != ':' => return true,
                _ => {}
            }
            from = at + 1;
        }
        false
    }
}


//! Every (address, name) an emit site cites is a row of `docs/symbols.md`. CNEW-trace-sink.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use l2_net::trace::Record;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// `| \`0x0040A21A\` | \`Pl8_DrawFrame(buf, frame, x, y)\` |` -> 0x0040A21A: Pl8_DrawFrame.
pub fn symbols() -> BTreeMap<u32, Vec<String>> {
    let text = std::fs::read_to_string(root().join("docs/symbols.md")).expect("symbols.md");
    let mut map: BTreeMap<u32, Vec<String>> = BTreeMap::new();
    for line in text.lines() {
        let cells: Vec<&str> = line.split('|').map(str::trim).collect();
        if cells.len() < 3 {
            continue;
        }
        let Some(hex) = cells[1].trim_matches('`').strip_prefix("0x") else { continue };
        let Ok(addr) = u32::from_str_radix(hex, 16) else { continue };
        let name = cells[2].trim_matches('`');
        let name = name.split(['(', ' ']).next().unwrap_or("");
        map.entry(addr).or_default().push(name.to_string());
    }
    map
}

fn sources(dir: &Path, out: &mut Vec<PathBuf>) {
    for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let p = e.path();
        if p.is_dir() {
            sources(&p, out);
        } else if p.extension().is_some_and(|x| x == "rs") {
            out.push(p);
        }
    }
}

/// `trace!("kind", 0x........, "Name"` and `trace_changed!(v; "kind", 0x........, "Name"` in every crate.
pub fn sites() -> Vec<(String, u32, String, PathBuf)> {
    let mut files = Vec::new();
    sources(&root().join("crates"), &mut files);
    let mut found = Vec::new();
    for f in files {
        let text = std::fs::read_to_string(&f).unwrap_or_default();
        for (at, _) in text.match_indices("trace!(").chain(text.match_indices("trace_changed!(")) {
            let call = &text[at..text.len().min(at + 400)];
            let head = call.split_once('(').map_or("", |(_, r)| r);
            let head = if call.starts_with("trace_changed") {
                head.split_once(';').map_or("", |(_, r)| r)
            } else {
                head
            };
            let parts: Vec<&str> = head.splitn(4, ',').map(str::trim).collect();
            if parts.len() < 3 || !parts[0].starts_with('"') {
                continue;
            }
            let addr = match parts[1].strip_prefix("0x") {
                Some(h) => u32::from_str_radix(h, 16),
                None => parts[1].parse(),
            };
            let Ok(addr) = addr else { continue };
            let name = parts[2].trim_matches(|c| c == '"' || c == ')').to_string();
            found.push((parts[0].trim_matches('"').to_string(), addr, name, f.clone()));
        }
    }
    found
}

pub fn every_record_is_named(records: &[Record]) {
    let map = symbols();
    for r in records.iter().filter(|r| r.addr != 0) {
        let names = map.get(&r.addr).unwrap_or_else(|| panic!("{:#010X} not in symbols.md", r.addr));
        assert!(names.iter().any(|n| n == r.name), "{:#010X} is {names:?}, not {}", r.addr, r.name);
    }
}

#[test]
fn every_emit_site_cites_a_symbol_by_its_name() {
    let map = symbols();
    let sites = sites();
    assert!(sites.len() >= 15, "the emit sites are found: {}", sites.len());
    for (kind, addr, name, file) in &sites {
        if *addr == 0 {
            continue;
        }
        let names = map.get(addr);
        assert!(
            names.is_some_and(|n| n.iter().any(|n| n == name)),
            "{kind} at {}: {addr:#010X} {name} is not symbols.md's {names:?}",
            file.display()
        );
    }
}

#[test]
fn the_symbol_reader_reads_a_known_row() {
    assert_eq!(symbols().get(&0x0040A21A).map(|v| v[0].as_str()), Some("Pl8_DrawFrame"));
}

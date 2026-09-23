//! Chain mode: the one-End-Turn differential run over every consecutive pair of
//! a recorded chain (`tools/oracle/chain.ps1`), and over each `chain-NNN.pre.sav`
//! (the players' turn, `g_turnPhase` 4, with the orders in it) into `chain-NNN.sav`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use l2_formats::save::Save;

use crate::compare::{clock, run_saves, PairReport};

pub struct Link {
    pub label: String,
    pub before_name: String,
    pub before: Save,
    pub after: Save,
}

pub fn chain_dir() -> Option<PathBuf> {
    if let Ok(v) = std::env::var("LORDS2_CHAIN") {
        return Path::new(&v).is_dir().then(|| PathBuf::from(v));
    }
    let d = l2_testkit::fixtures_dir()?.join("chain");
    d.is_dir().then_some(d)
}

fn open(path: &Path) -> Result<Save, String> {
    let exe = l2_testkit::executable().ok_or("no Lords2.exe to read the block table from")?;
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    Save::open(&exe, &bytes).map_err(|e| format!("{} does not open: {e}", path.display()))
}

/// `chain-NNN.sav` in order, each followed by the link from its `.pre.sav` when present.
pub fn links_in(dir: &Path) -> Result<Vec<Link>, String> {
    let mut saves: Vec<(u32, PathBuf)> = std::fs::read_dir(dir)
        .map_err(|e| e.to_string())?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter_map(|p| {
            let n = p
                .file_name()?
                .to_str()?
                .strip_prefix("chain-")?
                .strip_suffix(".sav")?;
            Some((n.parse().ok()?, p))
        })
        .collect();
    saves.sort();
    let mut links = Vec::new();
    for w in saves.windows(2) {
        let ((a, pa), (b, pb)) = (&w[0], &w[1]);
        let after = open(pb)?;
        let pre = dir.join(format!("chain-{b:03}.pre.sav"));
        if pre.is_file() {
            let name = format!("chain-{b:03}.pre.sav");
            links.push(Link {
                label: format!("pre {b:03}"),
                before_name: name,
                before: open(&pre)?,
                after: open(pb)?,
            });
        }
        let name = format!("chain-{a:03}.sav");
        links.push(Link {
            label: format!("{a:03}->{b:03}"),
            before_name: name,
            before: open(pa)?,
            after,
        });
    }
    Ok(links)
}

pub fn fixture_links(names: &[&str]) -> Result<Vec<Link>, String> {
    let mut links = Vec::new();
    for w in names.windows(2) {
        links.push(Link {
            label: format!("{}->{}", w[0], w[1]),
            before_name: w[0].to_string(),
            before: l2_testkit::fixture_save(w[0])?,
            after: l2_testkit::fixture_save(w[1])?,
        });
    }
    Ok(links)
}

pub struct Walk {
    pub reports: Vec<(String, PairReport)>,
    /// Field path -> the label of the first link on which any record disagrees.
    pub first_disagree: BTreeMap<&'static str, String>,
}

/// Runs every link one turn apart; a link that is not is printed and skipped.
pub fn walk(links: &[Link]) -> Result<Walk, String> {
    let mut w = Walk {
        reports: Vec::new(),
        first_disagree: BTreeMap::new(),
    };
    eprintln!(
        "\n{:<32} {:>4} {:>6} {:>8} {:>6} {:>6} {:>7}",
        "link", "turn", "season", "compared", "agree", "moved", "m-agree"
    );
    for l in links {
        let ((bt, bs, _), (at, as_, _)) = (clock(&l.before), clock(&l.after));
        if at - bt != 1 {
            eprintln!(
                "{:<32} skipped: turn {bt} -> {at} is not one End Turn",
                l.label
            );
            continue;
        }
        let r = run_saves(&l.label, &l.before_name, &l.before, &l.after)?;
        eprintln!(
            "{:<32} {bt:>4} {bs:>3}->{as_} {:>8} {:>6} {:>6} {:>7}",
            l.label, r.compared, r.agree, r.moved, r.moved_agree
        );
        for (path, (_, agree)) in &r.per_field {
            let n = r.per_field[path].0;
            if *agree < n {
                w.first_disagree
                    .entry(*path)
                    .or_insert_with(|| l.label.clone());
            }
        }
        w.reports.push((l.label.clone(), r));
    }
    print_fields(&w);
    Ok(w)
}

fn print_fields(w: &Walk) {
    let mut paths: Vec<&'static str> = w
        .reports
        .iter()
        .flat_map(|(_, r)| r.per_field.keys().copied())
        .collect();
    paths.sort();
    paths.dedup();
    eprintln!("\nper field, agree/compared on each link; first link that disagrees");
    let mut clean = 0;
    for p in paths {
        let Some(first) = w.first_disagree.get(p) else {
            clean += 1;
            continue;
        };
        let cells: Vec<String> = w
            .reports
            .iter()
            .map(|(_, r)| {
                r.per_field
                    .get(p)
                    .map_or("-".into(), |(n, a)| format!("{a}/{n}"))
            })
            .collect();
        eprintln!("  {p:<28} {}   first: {first}", cells.join(" "));
    }
    eprintln!("  ({clean} more fields agree on every link)\n");
}

/// `battle 3->4` and `battle 4->5` of `PAIRS`, walked as one chain.
const FIXTURE_CHAIN: &[&str] = &["safeturn.sav", "old_turn.sav", "battle-before.sav"];

#[test]
fn the_fixture_pairs_walk_as_a_chain() {
    if l2_testkit::fixtures_dir().is_none() {
        l2_testkit::skip!("no fixture directory");
    }
    let links = match fixture_links(FIXTURE_CHAIN) {
        Ok(l) => l,
        Err(why) => l2_testkit::skip!("{why}"),
    };
    let w = walk(&links).expect("the fixture chain walks");
    assert_eq!(w.reports.len(), 2, "both links are one End Turn apart");
    for (label, r) in &w.reports {
        let pair = crate::PAIRS
            .iter()
            .find(|p| label == &format!("{}->{}", p.before, p.after));
        let single = crate::compare::run(pair.expect("each link is one of PAIRS")).unwrap();
        assert_eq!(
            (r.compared, r.agree),
            (single.compared, single.agree),
            "{label}"
        );
    }
    assert_eq!(
        w.first_disagree.get("global.ai_lords").map(String::as_str),
        Some("safeturn.sav->old_turn.sav"),
        "the unsimulated field disagrees from the first link"
    );
}

#[test]
fn a_recorded_chain_walks() {
    let Some(dir) = chain_dir() else {
        l2_testkit::skip!("no chain directory (LORDS2_CHAIN, or <fixtures>/chain)");
    };
    let links = match links_in(&dir) {
        Ok(l) => l,
        Err(why) => l2_testkit::skip!("{why}"),
    };
    if links.is_empty() {
        l2_testkit::skip!("{} holds fewer than two chain saves", dir.display());
    }
    eprintln!("chain {}: {} links", dir.display(), links.len());
    walk(&links).expect("the chain walks");
}

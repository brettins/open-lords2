
use crate::value::{join_path, Origin, Spanned, Table, Value, DELETE_KEY};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Override {
    pub path: String,
    pub previous: Origin,
    pub current: Origin,
    pub type_changed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Deletion {
    pub path: String,
    pub removed: Origin,
    pub by: Origin,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DanglingDelete {
    pub path: String,
    pub by: Origin,
}

#[derive(Debug, Default, Clone)]
pub struct MergeLog {
    pub overrides: Vec<Override>,
    pub deletions: Vec<Deletion>,
    pub dangling_deletes: Vec<DanglingDelete>,
}

impl MergeLog {
    pub fn overrides_under<'a>(&'a self, prefix: &'a str) -> impl Iterator<Item = &'a Override> {
        self.overrides
            .iter()
            .filter(move |o| o.path == prefix || o.path.starts_with(&format!("{prefix}.")))
    }

    pub fn contested_paths(&self) -> Vec<(&str, usize)> {
        let mut counts: std::collections::BTreeMap<&str, usize> = Default::default();
        for o in &self.overrides {
            *counts.entry(o.path.as_str()).or_insert(1) += 1;
        }
        let mut out: Vec<_> = counts.into_iter().filter(|(_, n)| *n > 2).collect();
        out.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
        out
    }
}

pub fn merge(dst: &mut Table, src: Table, log: &mut MergeLog) {
    let mut path = Vec::new();
    merge_tables(dst, src, &mut path, log);
}

fn merge_tables(dst: &mut Table, mut src: Table, path: &mut Vec<String>, log: &mut MergeLog) {
    if let Some(directive) = src.remove(DELETE_KEY) {
        apply_deletes(dst, &directive, path, log);
    }

    for (key, incoming) in src {
        path.push(key.clone());
        match dst.get_mut(&key) {
            None => {
                dst.insert(key, incoming);
            }
            Some(existing) => {
                match (&mut existing.value, incoming.value) {
                    (Value::Table(d), Value::Table(s)) => {
                        merge_tables(d, s, path, log);
                    }
                    (existing_value, incoming_value) => {
                        log.overrides.push(Override {
                            path: join_path(path),
                            previous: existing.origin.clone(),
                            current: incoming.origin.clone(),
                            type_changed: existing_value.type_name()
                                != incoming_value.type_name(),
                        });
                        *existing_value = incoming_value;
                        existing.origin = incoming.origin;
                    }
                }
            }
        }
        path.pop();
    }
}

fn apply_deletes(
    dst: &mut Table,
    directive: &Spanned<Value>,
    path: &mut Vec<String>,
    log: &mut MergeLog,
) {
    let names: Vec<&str> = match &directive.value {
        Value::Array(items) => items.iter().filter_map(|i| i.value.as_str()).collect(),
        Value::String(s) => vec![s.as_str()],
        _ => Vec::new(),
    };
    for name in names {
        path.push(name.to_string());
        let full = join_path(path);
        path.pop();
        match dst.remove(name) {
            Some(gone) => log.deletions.push(Deletion {
                path: full,
                removed: gone.origin,
                by: directive.origin.clone(),
            }),
            None => log
                .dangling_deletes
                .push(DanglingDelete { path: full, by: directive.origin.clone() }),
        }
    }
}

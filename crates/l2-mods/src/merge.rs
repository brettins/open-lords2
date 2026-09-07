//! Merging rule documents.
//!
//! This is the mechanism that makes a mod composable: a mod restates only the
//! keys it changes, and the rest of the table survives. It is deliberately a
//! small set of rules, because every extra rule is one more thing a mod author
//! has to hold in their head to predict what two mods will do together.
//!
//! 1. **Table into table: recurse.** Keys only the newer document has are
//!    added. Keys both have are resolved one level deeper.
//! 2. **Anything else: replace.** Scalars replace scalars. Arrays replace
//!    arrays *whole* — there is no element-wise array merge, because with no
//!    identity on an element there is no principled way to say which element
//!    an override refers to. A table keyed by name is the right shape for
//!    anything that wants partial override, and the seeded rulesets use it.
//! 3. **`"$delete" = ["a", "b"]`** removes those keys from the table it
//!    appears in, before the rest of that table is merged.
//!
//! Every replacement of an existing value is logged with both origins. That
//! log is the answer to "two mods touched the same thing".

use crate::value::{join_path, Origin, Spanned, Table, Value, DELETE_KEY};

/// One value replaced by a later layer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Override {
    /// Dotted path of the value that was replaced.
    pub path: String,
    /// Where the value being replaced came from.
    pub previous: Origin,
    /// Where the replacement came from.
    pub current: Origin,
    /// True when the two values are structurally different types — almost
    /// always a mistake rather than an intended rebalance.
    pub type_changed: bool,
}

/// One key removed by a `"$delete"` directive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Deletion {
    pub path: String,
    /// Where the deleted value had been defined.
    pub removed: Origin,
    /// Where the `"$delete"` was written.
    pub by: Origin,
}

/// A `"$delete"` naming a key that is not there. Usually a typo, or a mod
/// written against a version of another mod that has since renamed something.
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
    /// Overrides whose path starts with `prefix`, for narrower reporting.
    pub fn overrides_under<'a>(&'a self, prefix: &'a str) -> impl Iterator<Item = &'a Override> {
        self.overrides
            .iter()
            .filter(move |o| o.path == prefix || o.path.starts_with(&format!("{prefix}.")))
    }

    /// Paths that three or more documents have fought over — a stronger smell
    /// than a single override, and worth surfacing by default.
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

/// Merge `src` into `dst`, recording what happened.
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

//! The merged ruleset, and the typed accessors the engine reads it through.
//!
//! A ruleset is built by applying documents in load order. Assets shadow —
//! only the top layer's `Base1a.pl8` is used — but **rules accumulate**: every
//! layer's rule documents are read and merged. That asymmetry is the point. A
//! sprite has no partial form, so overriding it means replacing it; a rule
//! table does, so overriding it means changing one number.

use crate::merge::{merge, MergeLog};
use crate::reader::{self, ParseError};
use crate::value::{Origin, Spanned, Table, Value};
use crate::vfs::Vfs;
use std::fmt;
use std::path::Path;

/// Where rule documents live inside a layer.
pub const RULES_DIR: &str = "rules";

#[derive(Debug)]
pub enum RuleError {
    Syntax(ParseError),
    Io { source_name: String, source: std::io::Error },
    /// A path the engine needs is absent.
    Missing { path: String },
    /// A path exists but holds the wrong kind of value.
    Type { path: String, expected: &'static str, found: &'static str, origin: Origin },
    /// A value is the right type but outside the range the engine can use.
    Range { path: String, message: String, origin: Origin },
}

impl fmt::Display for RuleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RuleError::Syntax(e) => write!(f, "{e}"),
            RuleError::Io { source_name, source } => write!(f, "{source_name}: {source}"),
            RuleError::Missing { path } => write!(f, "rule '{path}' is not defined"),
            RuleError::Type { path, expected, found, origin } => {
                write!(f, "{origin}: rule '{path}' should be a {expected}, found a {found}")
            }
            RuleError::Range { path, message, origin } => {
                write!(f, "{origin}: rule '{path}': {message}")
            }
        }
    }
}

impl std::error::Error for RuleError {}

impl From<ParseError> for RuleError {
    fn from(e: ParseError) -> Self {
        RuleError::Syntax(e)
    }
}

/// The result of merging every enabled layer's rule documents.
#[derive(Debug, Default)]
pub struct Ruleset {
    root: Table,
    pub log: MergeLog,
    /// Document names in the order they were applied.
    pub sources: Vec<String>,
    /// What each applied document actually said: its source name and every
    /// leaf path it set, in sorted order.
    ///
    /// The merge log records *contests*; this records *claims*. Both are
    /// needed to answer "why did my mod not take effect", because the two
    /// answers are different: a rule can lose to a later mod (a contest) or it
    /// can have been a typo that nothing below ever defined (a claim that
    /// overrode nothing).
    pub documents: Vec<Document>,
}

/// One rule document, and the leaf paths it claimed.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Document {
    /// `"<layer id>:<relative path>"`.
    pub source: String,
    /// Every leaf this document set, sorted. `"$delete"` directives are not
    /// leaves and are not listed here.
    pub leaves: Vec<String>,
}

impl Ruleset {
    pub fn new() -> Self {
        Ruleset::default()
    }

    /// Apply one document's text. `source` names it in diagnostics.
    pub fn apply_str(&mut self, text: &str, source: &str) -> Result<(), RuleError> {
        let doc = reader::parse(text, source)?;
        let Value::Table(table) = doc.value else { unreachable!("parse returns a table") };
        let mut leaves = Vec::new();
        collect_leaves(&table, &mut Vec::new(), &mut leaves);
        leaves.sort();
        self.documents.push(Document { source: source.to_string(), leaves });
        merge(&mut self.root, table, &mut self.log);
        self.sources.push(source.to_string());
        Ok(())
    }

    pub fn apply_file(&mut self, path: &Path, source: &str) -> Result<(), RuleError> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| RuleError::Io { source_name: source.to_string(), source: e })?;
        self.apply_str(&text, source)
    }

    /// Load every layer's rule documents, in layer order.
    ///
    /// Within a layer, documents apply in sorted name order, which is why a
    /// mod that wants to be sure it lands last inside its own layer names its
    /// file `zz-final.toml` and not something alphabetically unlucky. Across
    /// layers the load order decides, and that is the one that matters.
    pub fn load(vfs: &Vfs) -> Result<Ruleset, RuleError> {
        let mut rs = Ruleset::new();
        for layer in 0..vfs.layers().len() {
            let id = vfs.layer_id(layer).to_string();
            for (name, path) in vfs.layer_entries_under(layer, RULES_DIR) {
                if !name.ends_with(".toml") {
                    continue;
                }
                let source = format!("{id}:{name}");
                rs.apply_file(path, &source)?;
            }
        }
        Ok(rs)
    }

    pub fn root(&self) -> &Table {
        &self.root
    }

    /// Look up a dotted path. Numeric segments index arrays, so
    /// `"battle.three_bridges.attacker.crossbows"` and `"campaign.0.name"`
    /// both work.
    pub fn get(&self, path: &str) -> Option<&Spanned<Value>> {
        let mut segs = path.split('.').filter(|s| !s.is_empty());
        let mut cur = self.root.get(segs.next()?)?;
        for seg in segs {
            cur = match &cur.value {
                Value::Table(t) => t.get(seg)?,
                Value::Array(a) => a.get(seg.parse::<usize>().ok()?)?,
                _ => return None,
            };
        }
        Some(cur)
    }

    pub fn table(&self, path: &str) -> Result<&Table, RuleError> {
        let v = self.require(path)?;
        v.value.as_table().ok_or_else(|| self.type_err(path, "table", v))
    }

    /// Keys of a table, or an empty list when the table is absent. Used when
    /// "no mod defined any troops" is a legitimate state rather than an error.
    pub fn keys(&self, path: &str) -> Vec<&str> {
        match self.get(path).and_then(|v| v.value.as_table()) {
            Some(t) => t.keys().map(|k| k.as_str()).collect(),
            None => Vec::new(),
        }
    }

    pub fn integer(&self, path: &str) -> Result<i64, RuleError> {
        let v = self.require(path)?;
        v.value.as_integer().ok_or_else(|| self.type_err(path, "integer", v))
    }

    pub fn integer_or(&self, path: &str, default: i64) -> Result<i64, RuleError> {
        match self.get(path) {
            None => Ok(default),
            Some(v) => v.value.as_integer().ok_or_else(|| self.type_err(path, "integer", v)),
        }
    }

    /// An integer that must fit a range, reported with the line that broke it.
    pub fn integer_in(&self, path: &str, lo: i64, hi: i64) -> Result<i64, RuleError> {
        let v = self.require(path)?;
        let n = v.value.as_integer().ok_or_else(|| self.type_err(path, "integer", v))?;
        if n < lo || n > hi {
            return Err(RuleError::Range {
                path: path.to_string(),
                message: format!("{n} is outside {lo}..={hi}"),
                origin: v.origin.clone(),
            });
        }
        Ok(n)
    }

    pub fn string(&self, path: &str) -> Result<&str, RuleError> {
        let v = self.require(path)?;
        v.value.as_str().ok_or_else(|| self.type_err(path, "string", v))
    }

    pub fn string_or<'a>(&'a self, path: &str, default: &'a str) -> &'a str {
        self.get(path).and_then(|v| v.value.as_str()).unwrap_or(default)
    }

    pub fn boolean_or(&self, path: &str, default: bool) -> Result<bool, RuleError> {
        match self.get(path) {
            None => Ok(default),
            Some(v) => v.value.as_bool().ok_or_else(|| self.type_err(path, "boolean", v)),
        }
    }

    /// Which document last set this value. The answer a mod author wants when
    /// the number in the game is not the number they wrote.
    pub fn origin(&self, path: &str) -> Option<&Origin> {
        self.get(path).map(|v| &v.origin)
    }

    fn require(&self, path: &str) -> Result<&Spanned<Value>, RuleError> {
        self.get(path).ok_or_else(|| RuleError::Missing { path: path.to_string() })
    }

    fn type_err(&self, path: &str, expected: &'static str, v: &Spanned<Value>) -> RuleError {
        RuleError::Type {
            path: path.to_string(),
            expected,
            found: v.value.type_name(),
            origin: v.origin.clone(),
        }
    }
}

impl Ruleset {
    /// The engine's own rules, before any layer is consulted.
    ///
    /// OpenXcom's lesson, and `docs/modding.md` §1: **the base game is itself
    /// the first mod.** These documents are compiled into the binary rather
    /// than read from disk, for two reasons. They are our own numbers, not the
    /// player's game files, so `CLAUDE.md` rule 1 does not stop us shipping
    /// them. And a rules layer that can go missing is a rules layer that can
    /// go missing *on one peer only*.
    ///
    /// They are still plain, readable `.toml` in the source tree, and
    /// [`crate::core::write_to`] will drop a copy next to a player's mods so
    /// the first thing a would-be author can do is read the base rules.
    pub fn core() -> Ruleset {
        let mut rs = Ruleset::new();
        for (name, text) in crate::core::DOCUMENTS {
            rs.apply_str(text, name).expect("the shipped core ruleset parses");
        }
        rs
    }

    /// Load every layer's rule documents, in layer order, on top of the core
    /// ruleset.
    ///
    /// This is what [`crate::Platform`] uses. [`Ruleset::load`] is the same
    /// thing without the core layer, kept for callers that want to look at
    /// exactly one install's documents and nothing else.
    pub fn load_over_core(vfs: &Vfs) -> Result<Ruleset, RuleError> {
        let mut rs = Ruleset::core();
        rs.apply_layers(vfs)?;
        Ok(rs)
    }

    /// Apply every layer's rule documents to an existing ruleset.
    pub fn apply_layers(&mut self, vfs: &Vfs) -> Result<(), RuleError> {
        for layer in 0..vfs.layers().len() {
            let id = vfs.layer_id(layer).to_string();
            for (name, path) in vfs.layer_entries_under(layer, RULES_DIR) {
                if !name.ends_with(".toml") {
                    continue;
                }
                let source = format!("{id}:{name}");
                self.apply_file(path, &source)?;
            }
        }
        Ok(())
    }

    /// An array of integers of a known length.
    ///
    /// Arrays replace whole on merge (merge rule 2), so a mod that wants to
    /// change one element restates all of them. The length check is what turns
    /// "restated nine of ten" into an error at load rather than a zero in the
    /// tenth slot at turn forty.
    pub fn integer_array(&self, path: &str, len: usize) -> Result<Vec<i64>, RuleError> {
        let v = self.require(path)?;
        let items = v.value.as_array().ok_or_else(|| self.type_err(path, "array", v))?;
        if items.len() != len {
            return Err(RuleError::Range {
                path: path.to_string(),
                message: format!("expected {len} numbers, found {}", items.len()),
                origin: v.origin.clone(),
            });
        }
        let mut out = Vec::with_capacity(len);
        for (i, item) in items.iter().enumerate() {
            match item.value.as_integer() {
                Some(n) => out.push(n),
                None => {
                    return Err(RuleError::Type {
                        path: format!("{path}.{i}"),
                        expected: "integer",
                        found: item.value.type_name(),
                        origin: item.origin.clone(),
                    })
                }
            }
        }
        Ok(out)
    }

    /// How many entries an array of tables has, or an error if it is not one.
    pub fn array_len(&self, path: &str) -> Result<usize, RuleError> {
        let v = self.require(path)?;
        Ok(v.value.as_array().ok_or_else(|| self.type_err(path, "array", v))?.len())
    }

    /// Every leaf path in the merged tree, sorted.
    pub fn leaves(&self) -> Vec<String> {
        let mut out = Vec::new();
        collect_leaves(&self.root, &mut Vec::new(), &mut out);
        out.sort();
        out
    }

    /// Leaf paths whose value is a float.
    ///
    /// Not an error — a mod may legitimately carry a float for something the
    /// simulation never touches — but every one of them is a place a rule
    /// could reach the lockstep simulation as a float, which `docs/netcode.md`
    /// forbids. [`crate::Report`] prints them so the question is asked at load
    /// rather than at the first desync.
    pub fn float_rules(&self) -> Vec<(String, Origin)> {
        let mut out = Vec::new();
        collect_floats(&self.root, &mut Vec::new(), &mut out);
        out.sort_by(|a, b| a.0.cmp(&b.0));
        out
    }
}

/// Walk a table, recording the dotted path of every leaf.
///
/// A leaf is any non-table value, arrays included: an array replaces whole, so
/// it is one claim rather than N. `"$delete"` is a directive, not a claim, and
/// is skipped.
fn collect_leaves(table: &Table, stack: &mut Vec<String>, out: &mut Vec<String>) {
    for (key, spanned) in table {
        if key == crate::value::DELETE_KEY {
            continue;
        }
        stack.push(key.clone());
        match &spanned.value {
            Value::Table(t) => collect_leaves(t, stack, out),
            _ => out.push(crate::value::join_path(stack)),
        }
        stack.pop();
    }
}

fn collect_floats(table: &Table, stack: &mut Vec<String>, out: &mut Vec<(String, Origin)>) {
    for (key, spanned) in table {
        stack.push(key.clone());
        match &spanned.value {
            Value::Table(t) => collect_floats(t, stack, out),
            Value::Float(_) => out.push((crate::value::join_path(stack), spanned.origin.clone())),
            Value::Array(items) => {
                if items.iter().any(|i| matches!(i.value, Value::Float(_))) {
                    out.push((crate::value::join_path(stack), spanned.origin.clone()));
                }
            }
            _ => {}
        }
        stack.pop();
    }
}

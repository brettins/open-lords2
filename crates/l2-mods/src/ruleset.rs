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
}

impl Ruleset {
    pub fn new() -> Self {
        Ruleset::default()
    }

    /// Apply one document's text. `source` names it in diagnostics.
    pub fn apply_str(&mut self, text: &str, source: &str) -> Result<(), RuleError> {
        let doc = reader::parse(text, source)?;
        let Value::Table(table) = doc.value else { unreachable!("parse returns a table") };
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

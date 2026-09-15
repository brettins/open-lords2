
use crate::merge::{merge, MergeLog};
use crate::reader::{self, ParseError};
use crate::value::{Origin, Spanned, Table, Value};
use crate::vfs::Vfs;
use std::fmt;
use std::path::Path;

pub const RULES_DIR: &str = "rules";

#[derive(Debug)]
pub enum RuleError {
    Syntax(ParseError),
    Io { source_name: String, source: std::io::Error },
    Missing { path: String },
    Type { path: String, expected: &'static str, found: &'static str, origin: Origin },
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

#[derive(Debug, Default)]
pub struct Ruleset {
    root: Table,
    pub log: MergeLog,
    pub sources: Vec<String>,
    pub documents: Vec<Document>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Document {
    pub source: String,
    pub leaves: Vec<String>,
}

impl Ruleset {
    pub fn new() -> Self {
        Ruleset::default()
    }

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
    pub fn core() -> Ruleset {
        let mut rs = Ruleset::new();
        for (name, text) in crate::core::DOCUMENTS {
            rs.apply_str(text, name).expect("the shipped core ruleset parses");
        }
        rs
    }

    pub fn load_over_core(vfs: &Vfs) -> Result<Ruleset, RuleError> {
        let mut rs = Ruleset::core();
        rs.apply_layers(vfs)?;
        Ok(rs)
    }

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

    pub fn array_len(&self, path: &str) -> Result<usize, RuleError> {
        let v = self.require(path)?;
        Ok(v.value.as_array().ok_or_else(|| self.type_err(path, "array", v))?.len())
    }

    pub fn leaves(&self) -> Vec<String> {
        let mut out = Vec::new();
        collect_leaves(&self.root, &mut Vec::new(), &mut out);
        out.sort();
        out
    }

    pub fn float_rules(&self) -> Vec<(String, Origin)> {
        let mut out = Vec::new();
        collect_floats(&self.root, &mut Vec::new(), &mut out);
        out.sort_by(|a, b| a.0.cmp(&b.0));
        out
    }
}

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

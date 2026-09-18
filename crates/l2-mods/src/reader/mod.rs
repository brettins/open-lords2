
mod parser;
pub use parser::*;

use crate::value::{Origin, Spanned, Table, Value};
use std::collections::BTreeSet;
use std::fmt;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    pub message: String,
    pub origin: Origin,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.origin, self.message)
    }
}

impl std::error::Error for ParseError {}

type PResult<T> = Result<T, ParseError>;

struct Parser<'a> {
    s: &'a str,
    b: &'a [u8],
    i: usize,
    line: u32,
    line_start: usize,
    source: Arc<str>,
    defined: BTreeSet<Vec<String>>,
    arrays: BTreeSet<Vec<String>>,
}

fn is_bare_key_byte(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_' || c == b'-'
}

fn descend<'t>(root: &'t mut Table, path: &[String], origin: &Origin) -> PResult<&'t mut Table> {
    let mut here = root;
    for (n, seg) in path.iter().enumerate() {
        let entry = here
            .entry(seg.clone())
            .or_insert_with(|| Spanned::new(Value::Table(Table::new()), origin.clone()));
        here = match &mut entry.value {
            Value::Table(t) => t,
            Value::Array(items) => match items.last_mut().map(|s| &mut s.value) {
                Some(Value::Table(t)) => t,
                _ => {
                    return Err(ParseError {
                        message: format!("'{}' is not a table", path[..=n].join(".")),
                        origin: origin.clone(),
                    })
                }
            },
            other => {
                return Err(ParseError {
                    message: format!("'{}' is a {}, not a table", path[..=n].join("."), other.type_name()),
                    origin: origin.clone(),
                })
            }
        };
    }
    Ok(here)
}

fn insert_dotted(
    table: &mut Table,
    key: &[String],
    value: Spanned<Value>,
    origin: &Origin,
) -> PResult<()> {
    let (parent, last) = key.split_at(key.len() - 1);
    let target = descend(table, parent, origin)?;
    if let Some(existing) = target.get(&last[0]) {
        return Err(ParseError {
            message: format!(
                "key '{}' is set twice in this document (first at {})",
                key.join("."),
                existing.origin
            ),
            origin: origin.clone(),
        });
    }
    target.insert(last[0].clone(), value);
    Ok(())
}


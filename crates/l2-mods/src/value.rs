
use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Origin {
    pub source: Arc<str>,
    pub line: u32,
    pub col: u32,
}

impl Origin {
    pub fn new(source: Arc<str>, line: u32, col: u32) -> Self {
        Origin { source, line, col }
    }

    pub fn synthetic(what: &str) -> Self {
        Origin { source: Arc::from(what), line: 0, col: 0 }
    }
}

impl fmt::Display for Origin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.line == 0 {
            write!(f, "{}", self.source)
        } else {
            write!(f, "{}:{}:{}", self.source, self.line, self.col)
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Spanned<T> {
    pub value: T,
    pub origin: Origin,
}

impl<T> Spanned<T> {
    pub fn new(value: T, origin: Origin) -> Self {
        Spanned { value, origin }
    }
}

pub type Table = BTreeMap<String, Spanned<Value>>;

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    String(String),
    Integer(i64),
    Float(f64),
    Boolean(bool),
    Array(Vec<Spanned<Value>>),
    Table(Table),
}

impl Value {
    pub fn type_name(&self) -> &'static str {
        match self {
            Value::String(_) => "string",
            Value::Integer(_) => "integer",
            Value::Float(_) => "float",
            Value::Boolean(_) => "boolean",
            Value::Array(_) => "array",
            Value::Table(_) => "table",
        }
    }

    pub fn as_table(&self) -> Option<&Table> {
        match self {
            Value::Table(t) => Some(t),
            _ => None,
        }
    }

    pub fn as_table_mut(&mut self) -> Option<&mut Table> {
        match self {
            Value::Table(t) => Some(t),
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<&[Spanned<Value>]> {
        match self {
            Value::Array(a) => Some(a),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::String(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_integer(&self) -> Option<i64> {
        match self {
            Value::Integer(i) => Some(*i),
            _ => None,
        }
    }

    pub fn as_float(&self) -> Option<f64> {
        match self {
            Value::Float(f) => Some(*f),
            Value::Integer(i) => Some(*i as f64),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Boolean(b) => Some(*b),
            _ => None,
        }
    }

    pub fn get(&self, path: &str) -> Option<&Spanned<Value>> {
        let mut cur: Option<&Spanned<Value>> = None;
        let mut here = self;
        for seg in path.split('.').filter(|s| !s.is_empty()) {
            let next = match here {
                Value::Table(t) => t.get(seg)?,
                Value::Array(a) => a.get(seg.parse::<usize>().ok()?)?,
                _ => return None,
            };
            cur = Some(next);
            here = &next.value;
        }
        cur
    }
}

pub const DELETE_KEY: &str = "$delete";

pub fn join_path(parts: &[String]) -> String {
    parts.join(".")
}

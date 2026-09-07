//! A reader for the rule-document syntax: a deliberate subset of TOML.
//!
//! ## Why a subset, and why hand-written
//!
//! See `docs/modding.md` for the full argument. In brief: rule documents have
//! to *merge*, merging needs a generic value tree, and once you have a generic
//! value tree the only thing a serialisation crate still contributes is
//! `#[derive]` — which we cannot use on a merged tree anyway. Against that,
//! writing the reader ourselves buys exact line/column provenance for every
//! scalar and keeps the dependency count at zero, matching `l2-formats`.
//!
//! ## Supported
//!
//! Comments, bare and quoted keys, dotted keys, `[table]`, `[[array of
//! tables]]`, basic and literal strings (single- and multi-line), integers
//! (decimal with `_` separators, `0x`/`0o`/`0b`), floats, `true`/`false`,
//! arrays and inline tables.
//!
//! ## Not supported, on purpose
//!
//! Dates and times — the game has no use for them, and accepting them would
//! mean carrying a date type through the merge and the accessors for nothing.
//! A document containing one gets a clear error rather than a wrong parse.

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

/// Parse a rule document. `source` is the name used in diagnostics.
pub fn parse(text: &str, source: &str) -> PResult<Spanned<Value>> {
    let src: Arc<str> = Arc::from(source);
    let mut p = Parser {
        s: text,
        b: text.as_bytes(),
        i: 0,
        line: 1,
        line_start: 0,
        source: src.clone(),
        defined: BTreeSet::new(),
        arrays: BTreeSet::new(),
    };
    let root = p.run()?;
    Ok(Spanned::new(Value::Table(root), Origin::new(src, 1, 1)))
}

struct Parser<'a> {
    s: &'a str,
    b: &'a [u8],
    i: usize,
    line: u32,
    line_start: usize,
    source: Arc<str>,
    /// Explicitly written `[table]` headers, so a repeat is an error.
    defined: BTreeSet<Vec<String>>,
    /// Paths introduced by `[[array of tables]]`.
    arrays: BTreeSet<Vec<String>>,
}

impl<'a> Parser<'a> {
    // ---- position helpers -------------------------------------------------

    fn origin(&self) -> Origin {
        Origin::new(self.source.clone(), self.line, (self.i - self.line_start + 1) as u32)
    }

    fn err<T>(&self, msg: impl Into<String>) -> PResult<T> {
        Err(ParseError { message: msg.into(), origin: self.origin() })
    }

    fn peek(&self) -> Option<u8> {
        self.b.get(self.i).copied()
    }

    fn peek_at(&self, n: usize) -> Option<u8> {
        self.b.get(self.i + n).copied()
    }

    fn starts_with(&self, pat: &str) -> bool {
        self.s[self.i..].starts_with(pat)
    }

    /// Advance one byte, tracking line breaks. Callers only ever step over
    /// ASCII structure or whole UTF-8 sequences copied wholesale, so this
    /// never lands inside a character for the purposes of slicing.
    fn bump(&mut self) {
        if self.b[self.i] == b'\n' {
            self.line += 1;
            self.line_start = self.i + 1;
        }
        self.i += 1;
    }

    fn bump_n(&mut self, n: usize) {
        for _ in 0..n {
            if self.i < self.b.len() {
                self.bump();
            }
        }
    }

    /// Spaces and tabs only.
    fn skip_inline_ws(&mut self) {
        while matches!(self.peek(), Some(b' ') | Some(b'\t')) {
            self.bump();
        }
    }

    fn skip_comment(&mut self) {
        if self.peek() == Some(b'#') {
            while let Some(c) = self.peek() {
                if c == b'\n' {
                    break;
                }
                self.bump();
            }
        }
    }

    /// Whitespace, newlines and comments — used between statements.
    fn skip_trivia(&mut self) {
        loop {
            match self.peek() {
                Some(b' ') | Some(b'\t') | Some(b'\r') | Some(b'\n') => self.bump(),
                Some(b'#') => self.skip_comment(),
                _ => return,
            }
        }
    }

    /// After a statement: allow trailing spaces and a comment, then demand a
    /// newline or end of input. This is what catches `a = 1 b = 2`.
    fn expect_line_end(&mut self) -> PResult<()> {
        self.skip_inline_ws();
        self.skip_comment();
        match self.peek() {
            None => Ok(()),
            Some(b'\n') => {
                self.bump();
                Ok(())
            }
            Some(b'\r') if self.peek_at(1) == Some(b'\n') => {
                self.bump_n(2);
                Ok(())
            }
            Some(c) => self.err(format!("expected end of line, found {:?}", c as char)),
        }
    }

    // ---- top level --------------------------------------------------------

    fn run(&mut self) -> PResult<Table> {
        let mut root = Table::new();
        let mut path: Vec<String> = Vec::new();

        loop {
            self.skip_trivia();
            if self.peek().is_none() {
                return Ok(root);
            }
            if self.peek() == Some(b'[') {
                path = self.parse_header(&mut root)?;
            } else {
                let origin = self.origin();
                let key = self.parse_key()?;
                self.skip_inline_ws();
                if self.peek() != Some(b'=') {
                    return self.err("expected '=' after key");
                }
                self.bump();
                self.skip_inline_ws();
                let value = self.parse_value()?;
                let table = descend(&mut root, &path, &origin)?;
                insert_dotted(table, &key, value, &origin)?;
                self.expect_line_end()?;
            }
        }
    }

    /// `[a.b]` or `[[a.b]]`. Returns the new current path.
    fn parse_header(&mut self, root: &mut Table) -> PResult<Vec<String>> {
        let origin = self.origin();
        let array = self.peek_at(1) == Some(b'[');
        self.bump_n(if array { 2 } else { 1 });
        self.skip_inline_ws();
        let path = self.parse_key()?;
        self.skip_inline_ws();
        let close = if array { "]]" } else { "]" };
        if !self.starts_with(close) {
            return self.err(format!("expected '{close}' to close the table header"));
        }
        self.bump_n(close.len());
        self.expect_line_end()?;

        if array {
            if self.defined.contains(&path) {
                return Err(ParseError {
                    message: format!(
                        "'{}' was already defined as a plain table",
                        path.join(".")
                    ),
                    origin,
                });
            }
            let (parent, last) = path.split_at(path.len() - 1);
            let parent_tbl = descend(root, parent, &origin)?;
            let entry = parent_tbl
                .entry(last[0].clone())
                .or_insert_with(|| Spanned::new(Value::Array(Vec::new()), origin.clone()));
            match &mut entry.value {
                Value::Array(items) => {
                    items.push(Spanned::new(Value::Table(Table::new()), origin.clone()))
                }
                other => {
                    return Err(ParseError {
                        message: format!(
                            "'{}' is a {} and cannot take [[...]] entries",
                            path.join("."),
                            other.type_name()
                        ),
                        origin,
                    })
                }
            }
            self.arrays.insert(path.clone());
        } else {
            if !self.defined.insert(path.clone()) {
                return Err(ParseError {
                    message: format!("table '{}' is defined twice", path.join(".")),
                    origin,
                });
            }
            if self.arrays.contains(&path) {
                return Err(ParseError {
                    message: format!("'{}' was already defined as [[...]]", path.join(".")),
                    origin,
                });
            }
            // Force the table into existence even if it stays empty.
            descend(root, &path, &origin)?;
        }
        Ok(path)
    }

    // ---- keys -------------------------------------------------------------

    /// A dotted key: `a`, `"a b"`, `a.b.c`.
    fn parse_key(&mut self) -> PResult<Vec<String>> {
        let mut parts = Vec::new();
        loop {
            self.skip_inline_ws();
            parts.push(self.parse_key_part()?);
            self.skip_inline_ws();
            if self.peek() == Some(b'.') {
                self.bump();
            } else {
                return Ok(parts);
            }
        }
    }

    fn parse_key_part(&mut self) -> PResult<String> {
        match self.peek() {
            Some(b'"') => self.parse_basic_string(),
            Some(b'\'') => self.parse_literal_string(),
            Some(c) if is_bare_key_byte(c) => {
                let start = self.i;
                while matches!(self.peek(), Some(c) if is_bare_key_byte(c)) {
                    self.bump();
                }
                Ok(self.s[start..self.i].to_string())
            }
            Some(c) => self.err(format!("expected a key, found {:?}", c as char)),
            None => self.err("expected a key, found end of input"),
        }
    }

    // ---- values -----------------------------------------------------------

    fn parse_value(&mut self) -> PResult<Spanned<Value>> {
        let origin = self.origin();
        let v = match self.peek() {
            None => return self.err("expected a value, found end of input"),
            Some(b'"') => {
                if self.starts_with("\"\"\"") {
                    Value::String(self.parse_multiline_basic()?)
                } else {
                    Value::String(self.parse_basic_string()?)
                }
            }
            Some(b'\'') => {
                if self.starts_with("'''") {
                    Value::String(self.parse_multiline_literal()?)
                } else {
                    Value::String(self.parse_literal_string()?)
                }
            }
            Some(b'[') => self.parse_array()?,
            Some(b'{') => self.parse_inline_table()?,
            Some(b't') if self.starts_with("true") => {
                self.bump_n(4);
                Value::Boolean(true)
            }
            Some(b'f') if self.starts_with("false") => {
                self.bump_n(5);
                Value::Boolean(false)
            }
            Some(_) => self.parse_number()?,
        };
        Ok(Spanned::new(v, origin))
    }

    fn parse_array(&mut self) -> PResult<Value> {
        self.bump(); // '['
        let mut items = Vec::new();
        loop {
            self.skip_trivia();
            match self.peek() {
                None => return self.err("unterminated array"),
                Some(b']') => {
                    self.bump();
                    return Ok(Value::Array(items));
                }
                _ => {}
            }
            items.push(self.parse_value()?);
            self.skip_trivia();
            match self.peek() {
                Some(b',') => self.bump(),
                Some(b']') => {}
                None => return self.err("unterminated array"),
                Some(c) => return self.err(format!("expected ',' or ']', found {:?}", c as char)),
            }
        }
    }

    fn parse_inline_table(&mut self) -> PResult<Value> {
        self.bump(); // '{'
        let mut table = Table::new();
        self.skip_inline_ws();
        if self.peek() == Some(b'}') {
            self.bump();
            return Ok(Value::Table(table));
        }
        loop {
            self.skip_inline_ws();
            let origin = self.origin();
            let key = self.parse_key()?;
            self.skip_inline_ws();
            if self.peek() != Some(b'=') {
                return self.err("expected '=' in inline table");
            }
            self.bump();
            self.skip_inline_ws();
            let value = self.parse_value()?;
            insert_dotted(&mut table, &key, value, &origin)?;
            self.skip_inline_ws();
            match self.peek() {
                Some(b',') => {
                    self.bump();
                }
                Some(b'}') => {
                    self.bump();
                    return Ok(Value::Table(table));
                }
                None => return self.err("unterminated inline table"),
                Some(c) => return self.err(format!("expected ',' or '}}', found {:?}", c as char)),
            }
        }
    }

    // ---- strings ----------------------------------------------------------

    fn parse_basic_string(&mut self) -> PResult<String> {
        self.bump(); // opening quote
        let mut out = String::new();
        loop {
            match self.peek() {
                None | Some(b'\n') => return self.err("unterminated string"),
                Some(b'"') => {
                    self.bump();
                    return Ok(out);
                }
                Some(b'\\') => {
                    self.bump();
                    let c = self.parse_escape()?;
                    out.push(c);
                }
                Some(_) => {
                    let start = self.i;
                    self.bump();
                    // Copy any continuation bytes of a multi-byte character.
                    while matches!(self.peek(), Some(c) if c & 0xC0 == 0x80) {
                        self.bump();
                    }
                    out.push_str(&self.s[start..self.i]);
                }
            }
        }
    }

    fn parse_escape(&mut self) -> PResult<char> {
        let c = match self.peek() {
            None => return self.err("unterminated escape"),
            Some(c) => c,
        };
        self.bump();
        Ok(match c {
            b'b' => '\u{8}',
            b't' => '\t',
            b'n' => '\n',
            b'f' => '\u{c}',
            b'r' => '\r',
            b'"' => '"',
            b'\\' => '\\',
            b'u' => self.parse_hex_char(4)?,
            b'U' => self.parse_hex_char(8)?,
            other => return self.err(format!("unknown escape '\\{}'", other as char)),
        })
    }

    fn parse_hex_char(&mut self, n: usize) -> PResult<char> {
        let start = self.i;
        for _ in 0..n {
            match self.peek() {
                Some(c) if c.is_ascii_hexdigit() => self.bump(),
                _ => return self.err(format!("expected {n} hex digits in escape")),
            }
        }
        let code = u32::from_str_radix(&self.s[start..self.i], 16).unwrap();
        match char::from_u32(code) {
            Some(c) => Ok(c),
            None => self.err(format!("\\u{code:X} is not a character")),
        }
    }

    fn parse_literal_string(&mut self) -> PResult<String> {
        self.bump(); // opening quote
        let start = self.i;
        loop {
            match self.peek() {
                None | Some(b'\n') => return self.err("unterminated literal string"),
                Some(b'\'') => {
                    let s = self.s[start..self.i].to_string();
                    self.bump();
                    return Ok(s);
                }
                _ => self.bump(),
            }
        }
    }

    fn parse_multiline_basic(&mut self) -> PResult<String> {
        self.bump_n(3);
        self.trim_leading_newline();
        let mut out = String::new();
        loop {
            if self.starts_with("\"\"\"") {
                self.bump_n(3);
                return Ok(out);
            }
            match self.peek() {
                None => return self.err("unterminated multi-line string"),
                Some(b'\\') if self.escapes_line_end() => {
                    // A backslash at end of line swallows the newline and the
                    // whitespace that follows it.
                    self.bump();
                    while matches!(self.peek(), Some(b' ') | Some(b'\t') | Some(b'\r') | Some(b'\n'))
                    {
                        self.bump();
                    }
                }
                Some(b'\\') => {
                    self.bump();
                    let c = self.parse_escape()?;
                    out.push(c);
                }
                Some(_) => {
                    let start = self.i;
                    self.bump();
                    while matches!(self.peek(), Some(c) if c & 0xC0 == 0x80) {
                        self.bump();
                    }
                    out.push_str(&self.s[start..self.i]);
                }
            }
        }
    }

    fn escapes_line_end(&self) -> bool {
        let mut j = self.i + 1;
        while matches!(self.b.get(j), Some(b' ') | Some(b'\t') | Some(b'\r')) {
            j += 1;
        }
        self.b.get(j) == Some(&b'\n')
    }

    fn parse_multiline_literal(&mut self) -> PResult<String> {
        self.bump_n(3);
        self.trim_leading_newline();
        let start = self.i;
        loop {
            if self.starts_with("'''") {
                let s = self.s[start..self.i].to_string();
                self.bump_n(3);
                return Ok(s);
            }
            if self.peek().is_none() {
                return self.err("unterminated multi-line literal string");
            }
            self.bump();
        }
    }

    /// A newline immediately after the opening delimiter is not content.
    fn trim_leading_newline(&mut self) {
        if self.peek() == Some(b'\r') && self.peek_at(1) == Some(b'\n') {
            self.bump_n(2);
        } else if self.peek() == Some(b'\n') {
            self.bump();
        }
    }

    // ---- numbers ----------------------------------------------------------

    fn parse_number(&mut self) -> PResult<Value> {
        let start = self.i;
        if matches!(self.peek(), Some(b'+') | Some(b'-')) {
            self.bump();
        }

        // Radix prefixes, which have no sign, no fraction and no exponent.
        if self.peek() == Some(b'0') {
            if let Some(radix) = match self.peek_at(1) {
                Some(b'x') | Some(b'X') => Some(16),
                Some(b'o') | Some(b'O') => Some(8),
                Some(b'b') | Some(b'B') => Some(2),
                _ => None,
            } {
                self.bump_n(2);
                let digits_start = self.i;
                while matches!(self.peek(), Some(c) if c.is_ascii_alphanumeric() || c == b'_') {
                    self.bump();
                }
                let digits: String =
                    self.s[digits_start..self.i].chars().filter(|c| *c != '_').collect();
                if digits.is_empty() {
                    return self.err("radix prefix with no digits");
                }
                return match i64::from_str_radix(&digits, radix) {
                    Ok(v) => Ok(Value::Integer(if self.b[start] == b'-' { -v } else { v })),
                    Err(_) => self.err(format!("'{digits}' is not a base-{radix} integer")),
                };
            }
        }

        let mut is_float = false;
        loop {
            match self.peek() {
                Some(c) if c.is_ascii_digit() || c == b'_' => self.bump(),
                Some(b'.') => {
                    is_float = true;
                    self.bump();
                }
                Some(b'e') | Some(b'E') => {
                    is_float = true;
                    self.bump();
                    if matches!(self.peek(), Some(b'+') | Some(b'-')) {
                        self.bump();
                    }
                }
                _ => break,
            }
        }

        // `1979-05-27` and `07:32:00` are valid TOML and are not accepted
        // here. Saying so beats "expected end of line", which is what the
        // caller would otherwise report two characters later.
        if matches!(self.peek(), Some(b'-') | Some(b':')) {
            return self.err("dates and times are not part of the rule syntax");
        }

        let raw = &self.s[start..self.i];
        if raw.is_empty() || raw == "+" || raw == "-" {
            return self.err(format!(
                "expected a value, found {:?}",
                self.peek().map(|c| c as char).unwrap_or(' ')
            ));
        }
        let cleaned: String = raw.chars().filter(|c| *c != '_').collect();
        if is_float {
            match cleaned.parse::<f64>() {
                Ok(v) => Ok(Value::Float(v)),
                Err(_) => self.err(format!("'{raw}' is not a number")),
            }
        } else {
            match cleaned.parse::<i64>() {
                Ok(v) => Ok(Value::Integer(v)),
                Err(_) => self.err(format!("'{raw}' is not an integer")),
            }
        }
    }
}

fn is_bare_key_byte(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_' || c == b'-'
}

/// Walk `path`, creating tables as needed. A path segment landing on an array
/// descends into that array's last element, which is how `[[a]]` followed by
/// `[a.b]` finds the right parent.
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

/// Insert `value` at a dotted key relative to `table`, refusing to overwrite.
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

#![allow(unused_imports)]
use super::*;
use super::cursor::*;
use super::strings::*;
use super::super::*;
use super::super::*;
use crate::value::{Origin, Spanned, Table, Value};
use std::collections::BTreeSet;
use std::fmt;
use std::sync::Arc;

impl<'a> Parser<'a> {
    pub(super) fn parse_header(&mut self, root: &mut Table) -> PResult<Vec<String>> {
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
            descend(root, &path, &origin)?;
        }
        Ok(path)
    }


    pub(super) fn parse_key(&mut self) -> PResult<Vec<String>> {
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


    pub(super) fn parse_value(&mut self) -> PResult<Spanned<Value>> {
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


}

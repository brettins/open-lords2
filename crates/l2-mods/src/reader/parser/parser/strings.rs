#![allow(unused_imports)]
use super::*;
use super::cursor::*;
use super::structure::*;
use super::super::*;
use super::super::*;
use crate::value::{Origin, Spanned, Table, Value};
use std::collections::BTreeSet;
use std::fmt;
use std::sync::Arc;

impl<'a> Parser<'a> {
    pub(crate) fn parse_basic_string(&mut self) -> PResult<String> {
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

    pub(crate) fn parse_literal_string(&mut self) -> PResult<String> {
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

    pub(super) fn parse_multiline_basic(&mut self) -> PResult<String> {
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

    pub(super) fn parse_multiline_literal(&mut self) -> PResult<String> {
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

    fn trim_leading_newline(&mut self) {
        if self.peek() == Some(b'\r') && self.peek_at(1) == Some(b'\n') {
            self.bump_n(2);
        } else if self.peek() == Some(b'\n') {
            self.bump();
        }
    }


    pub(super) fn parse_number(&mut self) -> PResult<Value> {
        let start = self.i;
        if matches!(self.peek(), Some(b'+') | Some(b'-')) {
            self.bump();
        }

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




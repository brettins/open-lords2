#![allow(unused_imports)]
use super::*;
use super::structure::*;
use super::strings::*;
use super::super::*;
use super::super::*;
use crate::value::{Origin, Spanned, Table, Value};
use std::collections::BTreeSet;
use std::fmt;
use std::sync::Arc;

impl<'a> Parser<'a> {

    pub(crate) fn origin(&self) -> Origin {
        Origin::new(self.source.clone(), self.line, (self.i - self.line_start + 1) as u32)
    }

    pub(crate) fn err<T>(&self, msg: impl Into<String>) -> PResult<T> {
        Err(ParseError { message: msg.into(), origin: self.origin() })
    }

    pub(crate) fn peek(&self) -> Option<u8> {
        self.b.get(self.i).copied()
    }

    pub(crate) fn peek_at(&self, n: usize) -> Option<u8> {
        self.b.get(self.i + n).copied()
    }

    pub(crate) fn starts_with(&self, pat: &str) -> bool {
        self.s[self.i..].starts_with(pat)
    }

    pub(crate) fn bump(&mut self) {
        if self.b[self.i] == b'\n' {
            self.line += 1;
            self.line_start = self.i + 1;
        }
        self.i += 1;
    }

    pub(crate) fn bump_n(&mut self, n: usize) {
        for _ in 0..n {
            if self.i < self.b.len() {
                self.bump();
            }
        }
    }

    pub(crate) fn skip_inline_ws(&mut self) {
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

    pub(crate) fn skip_trivia(&mut self) {
        loop {
            match self.peek() {
                Some(b' ') | Some(b'\t') | Some(b'\r') | Some(b'\n') => self.bump(),
                Some(b'#') => self.skip_comment(),
                _ => return,
            }
        }
    }

    pub(super) fn expect_line_end(&mut self) -> PResult<()> {
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


    pub(super) fn run(&mut self) -> PResult<Table> {
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

}

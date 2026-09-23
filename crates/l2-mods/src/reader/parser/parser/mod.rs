#![allow(unused_imports)]

mod cursor;
pub use cursor::*;
mod structure;
pub use structure::*;
mod strings;
pub use strings::*;

use super::*;

use super::*;
use crate::value::{Origin, Spanned, Table, Value};
use std::collections::BTreeSet;
use std::fmt;
use std::sync::Arc;

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


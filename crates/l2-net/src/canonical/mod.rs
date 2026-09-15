
mod canonical;
pub use canonical::*;
mod reader;
pub use reader::*;
mod impls;
pub use impls::*;

use crate::fixed::Fixed;
use crate::hash::XxHash64;

pub const CHECKSUM_SEED: u64 = 0;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SectionDigest {
    pub name: &'static str,
    pub hash: u64,
    pub len: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Digest {
    pub hash: u64,
    pub len: u64,
    pub sections: Vec<SectionDigest>,
    pub bytes: Option<Vec<u8>>,
}

#[derive(Debug)]
pub struct Canonical {
    root: XxHash64,
    open: Option<(&'static str, XxHash64)>,
    sections: Vec<SectionDigest>,
    bytes: Option<Vec<u8>>,
}

pub trait Encode {
    fn encode(&self, out: &mut Canonical);
}

pub trait Decode: Sized {
    fn decode(input: &mut Reader<'_>) -> Result<Self, CodecError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodecError {
    UnexpectedEnd { wanted: usize, remaining: usize, at: usize },
    TrailingBytes { unread: usize },
    LengthOverrun { declared: usize, remaining: usize, at: usize },
    NotUtf8 { at: usize },
    BadTag { tag: u8, expected: &'static str, at: usize },
}

impl core::fmt::Display for CodecError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            CodecError::UnexpectedEnd { wanted, remaining, at } => {
                write!(f, "wanted {wanted} bytes at offset {at}, {remaining} remain")
            }
            CodecError::TrailingBytes { unread } => {
                write!(f, "{unread} unread bytes after decoding")
            }
            CodecError::LengthOverrun { declared, remaining, at } => {
                write!(f, "length {declared} at offset {at} exceeds the {remaining} bytes left")
            }
            CodecError::NotUtf8 { at } => write!(f, "not valid UTF-8, at offset {at}"),
            CodecError::BadTag { tag, expected, at } => {
                write!(f, "tag {tag} at offset {at} is not a {expected}")
            }
        }
    }
}

impl std::error::Error for CodecError {}

#[derive(Debug, Clone)]
pub struct Reader<'a> {
    input: &'a [u8],
    pos: usize,
}

pub fn decode_all<T: Decode>(input: &[u8]) -> Result<T, CodecError> {
    let mut reader = Reader::new(input);
    let value = T::decode(&mut reader)?;
    reader.finish()?;
    Ok(value)
}


//! The host's map slot and twelve options, carried in `Start::setup`.
//! Ours, not the original's: `docs/netcode.md` D-12.

use crate::setup::{SetupOptions, Settings, OPTION_COUNT};
use l2_kingdom::Quirks;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Setup {
    pub map: u8,
    pub options: SetupOptions,
}

impl Setup {
    pub fn encode(&self) -> Vec<u8> {
        let mut out = vec![self.map];
        out.extend((0..OPTION_COUNT).map(|i| self.options.get(i) as u8));
        out
    }

    pub fn decode(bytes: &[u8]) -> Option<Setup> {
        let (&map, rest) = bytes.split_first()?;
        if rest.len() != OPTION_COUNT {
            return None;
        }
        let mut options = SetupOptions::new();
        for (i, &v) in rest.iter().enumerate() {
            options.set(i, v as usize);
        }
        Some(Setup { map, options })
    }

    /// `Setup_CommitOptions` (`0x00499DC3`) with the roster's head count.
    pub fn settings(&self, humans: usize, quirks: Quirks) -> Settings {
        self.options.commit(humans, quirks)
    }
}

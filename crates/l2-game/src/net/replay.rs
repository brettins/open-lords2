//! Every network game on disk, replayed bit-exact in a fresh process:
//! `docs/netcode.md` §6 *The replay harness*. Ours. The world is rebuilt from
//! `Start`, as each peer built it (D-3b), then fed the recorded commands.

use super::{new_game, Stepper};
use crate::game::{Assets, Game};
use l2_kingdom::tables::Tables;
use l2_kingdom::Quirks;
use l2_net::{decode_all, Canonical, CodecError, Decode, Encode, PlayerSlot, Reader, Replay, Start};
use std::path::{Path, PathBuf};

pub const MAGIC: [u8; 4] = *b"L2NR";
pub const VERSION: u16 = 1;

/// What `net::new_game` was given: the rest of a peer's world is `Start`'s.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Origin {
    pub start: Start,
    pub quirks: Quirks,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Record {
    pub origin: Origin,
    pub me: PlayerSlot,
    /// `Canonical::hash_of(&Tables)`: a replay under another mod set is refused.
    pub tables: u64,
    pub replay: Replay,
}

impl Encode for Record {
    fn encode(&self, out: &mut Canonical) {
        out.raw(&MAGIC);
        out.u16(VERSION);
        self.origin.start.encode(out);
        out.u64(self.origin.quirks.bits());
        out.u8(self.me.index());
        out.u64(self.tables);
        self.replay.encode(out);
    }
}

impl Decode for Record {
    fn decode(input: &mut Reader<'_>) -> Result<Self, CodecError> {
        let at = input.position();
        if input.raw(4)? != MAGIC || input.u16()? != VERSION {
            return Err(CodecError::BadTag { tag: 0, expected: "network replay, version 1", at });
        }
        let start = Start::decode(input)?;
        let quirks = Quirks::from_bits(input.u64()?);
        let at = input.position();
        let me = PlayerSlot::from_wire(input.u8()?)
            .ok_or(CodecError::BadTag { tag: 0, expected: "player slot", at })?;
        let tables = input.u64()?;
        let replay = Replay::decode(input)?;
        Ok(Record { origin: Origin { start, quirks }, me, tables, replay })
    }
}

/// Beside the saves, never the install.
pub fn home() -> Option<PathBuf> {
    Some(crate::saves::dir()?.parent()?.join("replays"))
}

pub fn file_name(record: &Record) -> String {
    format!("net-{:016x}-slot{}.l2nr", record.origin.start.seed, record.me.index())
}

pub fn write(dir: &Path, record: &Record) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let path = dir.join(file_name(record));
    std::fs::write(&path, Canonical::bytes_of(record))?;
    Ok(path)
}

/// The world a peer played, as `net::service` then `Live::new` left it.
pub fn rebuild(assets: &Assets, record: &Record, tables: Tables) -> Result<Game, String> {
    let Origin { start, quirks } = &record.origin;
    let mut game = new_game(assets, start, record.me, *quirks, tables).map_err(|e| e.to_string())?;
    game.net.me = Some(record.me);
    game.net.lockstep = true;
    Ok(game)
}

/// `--verify-replay`: `Ok` names the last tick that matched.
pub fn verify_file(assets: &Assets, path: &Path, tables: Tables) -> Result<String, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let record = decode_all::<Record>(&bytes).map_err(|e| format!("{}: {e}", path.display()))?;
    if record.tables != Canonical::hash_of(&tables) {
        return Err("the replay was recorded under another mod set".into());
    }
    let mut game = rebuild(assets, &record, tables)?;
    let mut malformed = 0;
    let mut sim = Stepper { game: &mut game, malformed: &mut malformed };
    let last = record.replay.verify(&mut sim).map_err(|e| e.to_string())?;
    let n = record.replay.hashes.len();
    Ok(format!("replay verified: {n} ticks bit-exact through {last}"))
}

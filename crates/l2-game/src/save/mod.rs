
mod codec;
pub use codec::*;

use l2_kingdom::county::MAX_COUNTIES;
use l2_kingdom::industry::BankruptcyAction;
use l2_kingdom::realm::MAX_REALMS;
use l2_kingdom::report::{Message, SeasonReport};
use l2_kingdom::tables::Tables;
use l2_kingdom::{EventKind, Kingdom, Pass, SEASON_PIPELINE};
use l2_net::canonical::{Canonical, CodecError, Reader};

use crate::game::Game;
use crate::screens::setup::MAP_COUNT;

pub const MAGIC: [u8; 8] = *b"L2GSAVE\x01";

pub const VERSION: u32 = 6;

pub const HEADER_LEN: usize = 8 + 4 + 4 + 4;

pub const EXTENSION: &str = "l2sav";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoadError {
    NotASave,
    UnsupportedVersion { found: u32, supported: u32 },
    TruncatedBody { declared: usize, actual: usize },
    Corrupt { expected: u64, actual: u64 },
    Malformed(CodecError),
    Kingdom(l2_kingdom::save::LoadError),
    Player(u8),
    /// A map slot outside the sixty `L2.eng` group 101 names.
    MapSlot(u32),
    Selected(u8),
    BadPass(u32),
    BadMessage(u8),
    BadEvent(u16),
    BadBankruptcy(u8),
}

impl core::fmt::Display for LoadError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            LoadError::NotASave => write!(f, "not an open-lords2 saved game"),
            LoadError::UnsupportedVersion { found, supported } => write!(
                f,
                "saved game format version {found}; this build reads {supported}. \
                 Refusing rather than guessing at the layout"
            ),
            LoadError::TruncatedBody { declared, actual } => {
                write!(f, "the body declares {declared} bytes and {actual} follow")
            }
            LoadError::Corrupt { expected, actual } => {
                write!(f, "checksum {actual:#018x}, expected {expected:#018x}")
            }
            LoadError::Malformed(e) => write!(f, "{e}"),
            LoadError::Kingdom(e) => write!(f, "the world in this save: {e}"),
            LoadError::Player(p) => {
                write!(f, "realm {p} drives this game, and there are {MAX_REALMS} realm slots")
            }
            LoadError::MapSlot(s) => write!(f, "map slot {s}, and there are {MAP_COUNT}"),
            LoadError::Selected(c) => {
                write!(f, "county {c} is selected and it is not a county on this map")
            }
            LoadError::BadPass(p) => {
                write!(f, "season pass {p}, and the pipeline has {}", SEASON_PIPELINE.len())
            }
            LoadError::BadMessage(t) => write!(f, "season message tag {t} names nothing"),
            LoadError::BadEvent(id) => write!(f, "event {id:#x} is not in the deck"),
            LoadError::BadBankruptcy(t) => write!(f, "bankruptcy action tag {t} names nothing"),
        }
    }
}

impl std::error::Error for LoadError {}

impl From<CodecError> for LoadError {
    fn from(e: CodecError) -> LoadError {
        LoadError::Malformed(e)
    }
}

impl From<l2_kingdom::save::LoadError> for LoadError {
    fn from(e: l2_kingdom::save::LoadError) -> LoadError {
        LoadError::Kingdom(e)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Header {
    pub version: u32,
    pub prefix_len: usize,
    pub kingdom_len: usize,
}

pub fn peek(bytes: &[u8]) -> Result<Header, LoadError> {
    if bytes.len() < HEADER_LEN + 8 || bytes[..8] != MAGIC {
        return Err(LoadError::NotASave);
    }
    let word = |at: usize| {
        u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]]) as usize
    };
    let version = word(8) as u32;
    if version != VERSION {
        return Err(LoadError::UnsupportedVersion { found: version, supported: VERSION });
    }
    let prefix_len = word(12);
    let kingdom_len = word(16);
    let declared = prefix_len.saturating_add(kingdom_len);
    let actual = bytes.len() - HEADER_LEN - 8;
    if declared != actual {
        return Err(LoadError::TruncatedBody { declared, actual });
    }
    Ok(Header { version, prefix_len, kingdom_len })
}

pub fn encode(game: &Game) -> Vec<u8> {
    let mut prefix = Canonical::recording();
    encode_prefix(game, &mut prefix);
    let prefix = prefix.finish().bytes.expect("a recording encoder keeps its bytes");
    let kingdom = l2_kingdom::save::encode(&game.kingdom);

    let mut out = Vec::with_capacity(HEADER_LEN + prefix.len() + kingdom.len() + 8);
    out.extend_from_slice(&MAGIC);
    out.extend_from_slice(&VERSION.to_le_bytes());
    out.extend_from_slice(&(prefix.len() as u32).to_le_bytes());
    out.extend_from_slice(&(kingdom.len() as u32).to_le_bytes());
    out.extend_from_slice(&prefix);
    out.extend_from_slice(&kingdom);

    let mut c = Canonical::hashing();
    c.raw(&out[HEADER_LEN..]);
    out.extend_from_slice(&c.finish().hash.to_le_bytes());
    out
}

pub fn decode(bytes: &[u8], tables: Tables) -> Result<Game, LoadError> {
    let header = peek(bytes)?;

    let body = &bytes[HEADER_LEN..bytes.len() - 8];
    let mut trailer = [0u8; 8];
    trailer.copy_from_slice(&bytes[bytes.len() - 8..]);
    let expected = u64::from_le_bytes(trailer);
    let mut c = Canonical::hashing();
    c.raw(body);
    let actual = c.finish().hash;
    if actual != expected {
        return Err(LoadError::Corrupt { expected, actual });
    }

    let kingdom = l2_kingdom::save::decode(&body[header.prefix_len..], tables)?;

    let mut reader = Reader::new(&body[..header.prefix_len]);
    let game = decode_prefix(&mut reader, kingdom)?;
    reader.finish()?;
    Ok(game)
}


#[cfg(test)]
mod tests {
    use super::*;

    fn a_game() -> Game {
        let mut g = Game::new(0x0F1E_2D3C);
        g.kingdom.set_county_count(4);
        for id in 1..=4u8 {
            g.kingdom.counties[id as usize].owner = if id % 2 == 0 { 2 } else { 1 };
            g.anchor_x[id as usize] = id * 3;
            g.anchor_y[id as usize] = id * 5;
        }
        g.player = 1;
        g.map_slot = 7;
        g.selected = 3;
        g.turns_played = 11;
        g.realm_colour = [0, 1, 2, 3, 4, 5];
        g.gold_last = [0, 1500, -20, 3, 4, 5];
        g
    }

    #[test]
    fn a_game_round_trips_field_for_field() {
        let game = a_game();
        let bytes = encode(&game);
        let back = decode(&bytes, Tables::DEFAULT).expect("our own bytes");
        assert_eq!(back, game);
    }

    #[test]
    fn the_header_reads_without_decoding_the_body() {
        let bytes = encode(&a_game());
        let h = peek(&bytes).expect("a header");
        assert_eq!(h.version, VERSION);
        assert_eq!(HEADER_LEN + h.prefix_len + h.kingdom_len + 8, bytes.len());
    }

    #[test]
    fn a_season_report_round_trips_including_its_tagged_union() {
        let mut game = a_game();
        let mut report = SeasonReport::new();
        report.passes = SEASON_PIPELINE.to_vec();
        report.message(Message::UnrestWarning { county: 1 });
        report.message(Message::UnrestRising { county: 2, level: 3 });
        report.message(Message::Revolt { county: 2 });
        report.message(Message::Bankrupt {
            realm: 1,
            stage: 4,
            action: BankruptcyAction::LastWarning,
        });
        report.message(Message::Event { county: 3, kind: EventKind::Plague });
        report.message(Message::CastleBuilt { county: 4, castle_type: 2 });
        report.message(Message::ArmyStarving { realm: 2, unit: 40, county: 1, stage: 5 });
        game.last_report = Some(report);

        let back = decode(&encode(&game), Tables::DEFAULT).expect("our own bytes");
        assert_eq!(back, game);
        assert_eq!(back.last_report.as_ref().unwrap().revolts, vec![2]);
    }

    #[test]
    fn every_pass_in_the_pipeline_survives_the_round_trip() {
        for (i, pass) in SEASON_PIPELINE.iter().enumerate() {
            assert_eq!(pass.order(), i, "{pass:?}");
        }
    }

    #[test]
    fn a_file_that_is_not_ours_is_refused_on_the_magic() {
        assert_eq!(decode(b"not a save at all, really", Tables::DEFAULT), Err(LoadError::NotASave));
        assert_eq!(decode(&[], Tables::DEFAULT), Err(LoadError::NotASave));
        let mut theirs = vec![0u8; 256];
        theirs[..8].copy_from_slice(b"L2KSAVE\x01");
        assert_eq!(decode(&theirs, Tables::DEFAULT), Err(LoadError::NotASave));
    }

    #[test]
    fn a_version_this_build_does_not_know_is_named_rather_than_read() {
        let mut bytes = encode(&a_game());
        bytes[8..12].copy_from_slice(&(VERSION + 1).to_le_bytes());
        assert_eq!(
            decode(&bytes, Tables::DEFAULT),
            Err(LoadError::UnsupportedVersion { found: VERSION + 1, supported: VERSION })
        );
        let text = LoadError::UnsupportedVersion { found: 99, supported: VERSION }.to_string();
        assert!(text.contains("99"), "{text}");
        assert!(text.contains("Refusing rather than guessing"), "{text}");
    }

    #[test]
    fn a_worlds_version_this_build_does_not_know_comes_back_named_too() {
        let bytes = encode(&a_game());
        let h = peek(&bytes).unwrap();
        let at = HEADER_LEN + h.prefix_len + 8;
        let mut bad = bytes.clone();
        let bumped = l2_kingdom::save::VERSION + 1;
        bad[at..at + 4].copy_from_slice(&bumped.to_le_bytes());
        rehash(&mut bad);
        assert_eq!(
            decode(&bad, Tables::DEFAULT),
            Err(LoadError::Kingdom(l2_kingdom::save::LoadError::UnsupportedVersion {
                found: bumped,
                supported: l2_kingdom::save::VERSION,
            })),
            "the world's version is the world's to refuse, and it says so in its own words"
        );
    }

    #[test]
    fn a_truncated_file_is_refused_rather_than_read_short() {
        let bytes = encode(&a_game());
        let short = &bytes[..bytes.len() - 40];
        assert!(matches!(
            decode(short, Tables::DEFAULT),
            Err(LoadError::TruncatedBody { .. }) | Err(LoadError::NotASave)
        ));
    }

    #[test]
    fn one_flipped_byte_anywhere_in_the_body_is_caught_by_the_checksum() {
        let good = encode(&a_game());
        for at in [HEADER_LEN, HEADER_LEN + 3, good.len() - 20] {
            let mut bad = good.clone();
            bad[at] ^= 0xFF;
            assert!(
                matches!(decode(&bad, Tables::DEFAULT), Err(LoadError::Corrupt { .. })),
                "byte {at} flipped and the file still loaded"
            );
        }
    }

    #[test]
    fn a_player_that_is_not_a_realm_is_refused() {
        let mut game = a_game();
        game.player = 9;
        assert_eq!(decode(&encode(&game), Tables::DEFAULT), Err(LoadError::Player(9)));
        let mut bytes = encode(&a_game());
        bytes[HEADER_LEN] = MAX_REALMS as u8;
        rehash(&mut bytes);
        assert_eq!(
            decode(&bytes, Tables::DEFAULT),
            Err(LoadError::Player(MAX_REALMS as u8)),
            "the player byte is the first byte of the prefix"
        );
    }

    #[test]
    fn a_selection_that_is_not_a_county_on_this_map_is_refused() {
        let mut game = a_game();
        game.selected = 12; // four counties on this map
        let bytes = encode(&game);
        assert_eq!(decode(&bytes, Tables::DEFAULT), Err(LoadError::Selected(12)));
    }

    #[test]
    fn a_map_slot_past_the_sixty_is_refused() {
        let mut game = a_game();
        game.map_slot = MAP_COUNT;
        let bytes = encode(&game);
        assert_eq!(decode(&bytes, Tables::DEFAULT), Err(LoadError::MapSlot(MAP_COUNT as u32)));
    }

    #[test]
    fn a_ruleset_the_save_was_not_written_under_is_refused_by_the_kingdom() {
        let bytes = encode(&a_game());
        let mut other = Tables::DEFAULT;
        other.ration_happiness_slope += 1;
        assert!(
            matches!(
                decode(&bytes, other),
                Err(LoadError::Kingdom(l2_kingdom::save::LoadError::RulesetMismatch { .. }))
            ),
            "a mod set that is not the one the save was made with must be named, not applied"
        );
    }

    fn rehash(bytes: &mut [u8]) {
        let n = bytes.len();
        let mut c = Canonical::hashing();
        c.raw(&bytes[HEADER_LEN..n - 8]);
        let hash = c.finish().hash;
        bytes[n - 8..].copy_from_slice(&hash.to_le_bytes());
    }
}


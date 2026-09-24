#![allow(unused_imports)]
use super::*;
use super::furnished_state::*;
use super::campaign::*;
use super::round_trips::*;
use super::header::*;
use super::census_part::*;
use l2_kingdom::county::{ChangeReason, MAX_COUNTIES};
use l2_kingdom::realm::{Pair, MAX_REALMS};
use l2_kingdom::save::{
    checksum, decode, encode, ruleset_fingerprint, LoadError, HEADER_LEN, MAGIC, VERSION,
};
use l2_kingdom::tables::{Tables, Weather, JOB_COUNT};
use l2_kingdom::{Kingdom, Options};

const NOT_IN_THE_BODY: &[(&str, &str, &str)] = &[
    (
        "Campaign",
        "battle_county",
        "g_battleCounty, a process global every campaign battle rewrites before it reads \
         it; only a skirmish reads it stale (Battlefield_BuildCastle 0x0047C4BA:5344)",
    ),
    (
        "Campaign",
        "move_cost",
        "g_moveCost 0x004F4080, .bss and in no Save_Write block; only Move_BuildCostMap \
         0x0046FF43 writes it, and a load keeps the process value (B108)",
    ),
    (
        "Kingdom",
        "tables",
        "the ruleset is fingerprinted into the header rather than written into the \
         body (docs/modding.md); every_sub_table_reaches_the_fingerprint covers it",
    ),
    (
        "TurnMachine",
        "players_turn_open",
        "the latch that stops `Turn_BeginPlayersTurn` running twice over one phase 4, \
         and not a fact about the kingdom: a loaded game is parked at phase 1 and its \
         first map frames open phase 4 again (docs/netcode.md)",
    ),
];

const FURNISHED_BY_CALL: &[(&str, &str, &str)] = &[
    ("Units", "slots", "units.put("),
    ("ArmyNames", "counters", "names.pick("),
    ("MerchantRoutes", "rows", "routes.set_row("),
    ("MercenaryBands", "bands", "set_band_raw("),
    ("MercenaryBands", "in_play", "set_in_play("),
    ("History", "entries", "history.record("),
    ("History", "head", "history.record("),
    ("History", "tail", "history.record("),
    ("History", "len", "history.record("),
    ("HistoryEntry", "population", "history.record("),
    ("HistoryEntry", "happiness", "history.record("),
    ("County", "neighbour_count", "add_neighbour("),
    ("County", "neighbours", "add_neighbour("),
    ("Unit", "kind", "Unit::new(kind,"),
    ("Explored", "seen", "explored.reveal_square("),
    // The deal is `PlayerStart_Shuffle` (`0x00497E65`) and the cursor is
    // `Battlefield_BuildRandom` (`0x0047AAA3`); neither is set field by field.
    ("FieldPlaylist", "frames", "FieldPlaylist::deal("),
    ("FieldPlaylist", "budgets", "FieldPlaylist::deal("),
    ("FieldPlaylist", "cursor", "field_playlist.take_next()"),
];

/// This is the guard `docs/decisions.md` C30 said was missing and C39 wrote.
///
/// The old one was an enumeration of mutations: it checked the fields somebody
/// had listed, so the four fields C30 is about were absent from the encoding
/// *and* from the list.
#[test]
fn every_field_of_the_state_is_furnished() {
    let structs = census::structs_of(&census::repo_root().join("crates/l2-kingdom/src"));

    assert!(structs.len() >= 40, "only {} structs parsed - the parser broke", structs.len());
    assert!(
        structs["County"].len() >= 85,
        "County parsed as {} fields - the parser broke",
        structs["County"].len()
    );

    census::check_tables_are_live(&structs, NOT_IN_THE_BODY, "NOT_IN_THE_BODY");
    census::check_tables_are_live(&structs, FURNISHED_BY_CALL, "FURNISHED_BY_CALL");

    let reachable = census::reachable(&structs, "Kingdom", NOT_IN_THE_BODY);
    let fixture = census::fixture_region();

    let mut fields = 0usize;
    let mut missing: Vec<String> = Vec::new();
    for name in &reachable {
        for (field, _) in &structs[name] {
            if census::listed(NOT_IN_THE_BODY, name, field).is_some() {
                continue;
            }
            fields += 1;
            let found = match census::listed(FURNISHED_BY_CALL, name, field) {
                Some(call) => fixture.contains(call),
                None => census::mentions(&fixture, field),
            };
            if !found {
                missing.push(format!("  {name}::{field}"));
            }
        }
    }

    assert!(
        missing.is_empty(),
        "{} field(s) of the state are not furnished, so the round trip cannot see them:\n{}\n\n\
         Give each a value `{}::new` would not, between the fixture markers in this file. \
         A field that is genuinely not simulation state goes in NOT_IN_THE_BODY with its \
         reason - and then out of `l2_kingdom::save`'s Encode impl too.",
        missing.len(),
        missing.join("\n"),
        "County"
    );

    assert!(reachable.len() >= 15, "only {} structs reached from Kingdom", reachable.len());
    assert!(fields >= 200, "only {fields} fields reached from Kingdom");

    println!(
        "{} structs reachable from Kingdom, {fields} fields, all furnished",
        reachable.len()
    );
}

/// * the entries are `1 … n` with no gap and no duplicate — a duplicate is
///   exactly what a two-branch collision leaves behind;
/// * `VERSION` equals the highest of them — so a merge that keeps one branch's
///   constant and both branches' entries goes red;
/// * every entry says something, so the changelog
/// exists: an older save's *absence* of a field is a question.
#[test]
fn the_version_is_ahead_of_its_own_changelog() {
    let source = include_str!("../../src/save/codec_part.rs");
    let head = source.split("pub const VERSION").next().expect("VERSION is declared");
    let mut entries: Vec<u32> = Vec::new();
    for line in head.lines() {
        let line = line.trim();
        let Some(rest) = line.strip_prefix("/// * ") else { continue };
        let Some((number, tail)) = rest.split_once(' ') else { continue };
        let Ok(n) = number.parse::<u32>() else { continue };
        assert!(
            tail.starts_with('\u{2014}') || tail.starts_with('-'),
            "changelog entry {n} has no description: {line}"
        );
        entries.push(n);
    }

    assert!(entries.len() > 1, "the changelog was not found; did the comment's shape change?");

    let expected: Vec<u32> = (1..=entries.len() as u32).collect();
    assert_eq!(
        entries, expected,
        "the save changelog is {entries:?}, which is not 1..={}. A repeated number is what \
         two branches bumping VERSION in parallel leaves behind: give the later layout the \
         next free number and say so in its entry.",
        entries.len()
    );

    assert_eq!(
        VERSION,
        *entries.last().unwrap(),
        "VERSION is {VERSION} and the changelog's last entry is {}. Every layout change \
         needs both, and a merge that takes one branch's constant with both branches' \
         entries is exactly what this catches.",
        entries.last().unwrap()
    );
}



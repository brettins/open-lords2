//! **Eleven `Realm` fields are in no save and in no lockstep checksum.**
//!
//! ```text
//! cargo test -p l2-kingdom --test save_gap
//! ```
//!
//! This file pins a defect so that fixing it is what turns it red. It is the
//! same shape as `labour_gap.rs` was, and the same shape as correction **C30** —
//! which is the point: C30 recorded four `County` fields missing from
//! `Encode`, restored them, and said in as many words that **the mechanism was
//! not fixed**, because `every_part_of_the_state_reaches_the_bytes` is a
//! hand-written list and a hand-written list cannot fail for a field nobody
//! remembered.
//!
//! Checking the whole struct rather than the newly-added fields — matching the
//! source's `pub` fields against the ones that list touches — found the rest of
//! it. `County` was six short, and all six turned out to be encoded correctly,
//! so nothing was broken and the list was simply covering less than it looked.
//! `Realm` is worse:
//!
//! | field | in the save? |
//! |---|---|
//! | `pairs` — **the whole diplomatic matrix**: standing, alliance, grudge, at-war, gifts | **no** |
//! | `ally`, `ally_candidate`, `offer_pending`, `offer_timer` | **no** |
//! | `war_target`, `target_county` | **no** |
//! | `taunt_timer`, `taunt_stage` | **no** |
//! | `crowned_once` — the one-shot *"Just call me king."* guard | **no** |
//! | `voice_rotation` | **no** |
//!
//! None of that is cosmetic. `pairs` is what `diplomacy::ai_diplomacy` reads to
//! decide who to court and who to attack; `crowned_once` is what stops a lone
//! AI ending the game twice (C32); `voice_rotation` is advanced after every
//! message a realm sends, so it is deterministic state even though what it
//! selects is a sound.
//!
//! **The lockstep consequence is C30's, exactly.** `docs/netcode.md` §6's
//! per-tick checksum runs through the same `Encode` impl as the save, so two
//! peers could diverge on the entire diplomatic state of the game and every
//! checksum they exchanged would report agreement.
//!
//! # Why this is pinned rather than fixed here
//!
//! Encoding eleven fields is a save-format change with a judgement call attached
//! to each one — what an older save's *absence* of that field should mean, which
//! is the question `save::VERSION`'s changelog exists to answer, and the answer
//! for `pairs` is not obviously "default". It belongs with the work that makes
//! the enumeration derived rather than hand-written, where the same pass can
//! close the hole and remove the possibility of a new one.
//!
//! Delete this file when it goes red.

use l2_kingdom::save;
use l2_kingdom::Kingdom;

/// A realm with every unsaved field set to something a default cannot be
/// confused with, round-tripped. **It comes back unequal**, and that is the
/// defect.
#[test]
fn eleven_realm_fields_do_not_survive_a_round_trip() {
    let mut k = Kingdom::new(0xD1F0);
    k.set_county_count(4);

    let r = &mut k.realms[2];
    r.in_play = true;
    r.ally = 3;
    r.ally_candidate = 4;
    r.offer_pending = true;
    r.offer_timer = 21;
    r.war_target = 5;
    r.target_county = 3;
    r.taunt_timer = 17;
    r.taunt_stage = 2;
    r.crowned_once = true;
    r.voice_rotation = 3;
    r.pairs[3].standing = -40;
    r.pairs[3].allied = true;
    r.pairs[3].at_war = true;

    let bytes = save::encode(&k);
    let back = save::decode(&bytes, k.tables.clone()).expect("a kingdom this crate wrote must decode");

    assert_ne!(
        k, back,
        "eleven Realm fields now survive the save. That is the fix this file was \
         waiting for: add them to `every_part_of_the_state_reaches_the_bytes` in \
         tests/save.rs, note the version bump in `save::VERSION`'s changelog, and \
         delete this file."
    );

    // And name them, so a *partial* fix says which half landed rather than
    // leaving the assertion above to fail for an unknown reason.
    let got = &back.realms[2];
    let missing: Vec<&str> = [
        ("ally", got.ally == 3),
        ("ally_candidate", got.ally_candidate == 4),
        ("offer_pending", got.offer_pending),
        ("offer_timer", got.offer_timer == 21),
        ("war_target", got.war_target == 5),
        ("target_county", got.target_county == 3),
        ("taunt_timer", got.taunt_timer == 17),
        ("taunt_stage", got.taunt_stage == 2),
        ("crowned_once", got.crowned_once),
        ("voice_rotation", got.voice_rotation == 3),
        ("pairs", got.pairs[3].standing == -40),
    ]
    .iter()
    .filter(|(_, survived)| !survived)
    .map(|(name, _)| *name)
    .collect();

    assert_eq!(
        missing.len(),
        11,
        "the set of Realm fields outside the save has changed - it is now {missing:?}. \
         If that is because some were fixed, finish the job and delete this file; if it \
         is because a new one appeared, the hole is growing."
    );
}

#![allow(unused_imports)]
use super::*;
use super::basic::*;
use super::ai_battle::*;
use super::runner_battle::*;
use super::runner_tests::*;
use l2_net::{
    decode_all, frame, Advance, Canonical, Command, Config, FrameReader, Message, PeerId,
    PlayerSlot, Session, Simulation, TcpTransport, Tick, Transport,
};
use l2_sim::{Battle, Troop, SIDE_A, SIDE_B};
use l2_sim::ai::{self, Ai, AiField};
use l2_sim::unit::{Units, CATEGORY_OF_TROOP};
use l2_sim::{Figure, State};
use l2_sim::runner::{Army, BattleRunner};

/// A peer that had one arrow where the other had none, and agreed on every
/// checksum, would kill a different man three ticks later and diverge
/// invisibly. The original agrees that this is state: its own sync digest
/// copies all hundred records, `0x1DB0` bytes, one `Sync_RecordDigest` a slot.
#[test]
fn an_arrow_in_flight_changes_the_state_hash() {
    let mut sim = RunnerNetBattle {
        runner: BattleRunner::deploy_armies(
            close_field(),
            RUNNER_SEED,
            l2_sim::runner::Army { troops: &[(Troop::Peasants, 12)], owner: 2, human: false },
            l2_sim::runner::Army { troops: &[(Troop::Archers, 4)], owner: 1, human: true },
        ),
        nudge_at: None,
    };
    let mut loosed = None;
    for t in 0..4_000u32 {
        sim.step(Tick(t), &[]);
        if sim.runner.missiles.live() > 0 {
            loosed = Some(Canonical::hash_of(&Hashable(&sim)));
            break;
        }
    }
    let hash = loosed.expect("no missile was ever loosed");

    let mut emptied = sim.runner.clone();
    for slot in 1..=l2_sim::missile::MAX_MISSILES {
        emptied.missiles.free(slot);
    }
    let stripped = RunnerNetBattle { runner: emptied, nudge_at: None };
    assert_ne!(
        hash,
        Canonical::hash_of(&Hashable(&stripped)),
        "the missile array is outside the checksum: two peers could disagree about \
         every arrow in the air and every checksum would report agreement"
    );
}

struct Hashable<'a>(&'a RunnerNetBattle);

impl l2_net::canonical::Encode for Hashable<'_> {
    fn encode(&self, out: &mut Canonical) {
        self.0.encode_state(out);
    }
}

/// `docs/decisions.md` C39: *completeness must be derived, not remembered.* The
/// encoder above is a hand-written list, and a hand-written list cannot fail for
/// a field nobody put in it — which is how eleven `Realm` fields, the whole
/// diplomatic matrix among them, once sat outside the lockstep checksum with
/// every test green.
///
/// The exemption table has the polarity C39 insists on: inclusion is the
/// default, a line is a claim with a reason attached, and a line naming a field
/// that no longer exists fails too — because a stale exemption looks like a
/// decision and covers nothing.
#[test]
fn every_field_of_a_missile_and_a_fighter_reaches_the_bytes() {
    const NOT_HASHED: &[(&str, &str, &str)] = &[
        ("Fighter", "path", "recomputed from hashed state; its length is hashed"),
        ("Fighter", "sim", "the index of the figure this Fighter is, and both walks are by index"),
        ("Fighter", "troop", "fixed at deployment and never written again"),
        ("Fighter", "side", "fixed at deployment and never written again"),
        // Every reader draws: `l2-view/src/figures.rs:129,235,257,282`,
        // `scene/render.rs:147,162,221`. The blow is timed by
        // `melee.rs:44` `recovery_counter`, as in `Melee_Tick` (`0x00494908`,
        // `docs/battle.md:570`) — no simulation read of the counter.
        ("Fighter", "phase", "animation counter; read only by the renderer"),
        ("Fighter", "facing_drawn", "drawn facing (+0x0D); presentation, read only by the renderer"),
        ("Fighter", "fidget", "Anim_StandA2's shuffle counter (+0x0B); presentation"),
        ("Fighter", "fidget_period", "seeded from index and x (+0x0C); presentation"),
        ("Fighter", "horse_frame", "knight's horse frame (+0x11); presentation, read only by the renderer"),
    ];

    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let src = std::fs::read_to_string(root.join("tests/lockstep/runner_battle.rs")).expect("this file");
    // **`encode_state`'s body, not the whole file.** Scanning the file let any
    // `.name` anywhere in it stand for a hashed field: dropping
    // `Progress::free` from the encoder stayed green because
    // `emptied.missiles.free(slot)`, in an unrelated test below, matches
    // `.free`. A check a deliberate ablation cannot turn red is not a check.
    let head = "fn encode_state(&self, out: &mut Canonical) {\n        out.section(\"runner\");";
    let at = src.find(head).expect("the encoder");
    let rest = &src[at..];
    let end = rest.find("\n    }\n").expect("the encoder's closing brace");
    let encoder = rest[..end].to_string();
    let mut all: Vec<(String, String)> = Vec::new();
    for (file, want, least) in [
        ("src/missile/types.rs", "Missile", 5),
        ("src/runner/mod.rs", "Fighter", 5),
        ("src/siege/mod.rs", "SiegeState", 5),
        // **Nested structs were outside the walk.** `Fighter::progress` is one
        // name in the census and three fields in the digest, so `free` could
        // have been added and hashed by nobody with this test green — which is
        // exactly C39's failure, one level down.
        ("src/movement.rs", "Progress", 3),
    ] {
        let src = std::fs::read_to_string(root.join(file)).expect(file);
        let fields = fields_of(&src, want);
        assert!(fields.len() >= least, "{want} parsed as {} fields - the parser broke", fields.len());
        all.extend(fields.into_iter().map(|f| (want.to_string(), f)));
    }

    for (s, f, _) in NOT_HASHED {
        assert!(
            all.iter().any(|(a, b)| a == s && b == f),
            "NOT_HASHED names {s}::{f}, which no longer exists"
        );
    }

    let missing: Vec<String> = all
        .iter()
        .filter(|(s, f)| !NOT_HASHED.iter().any(|(a, b, _)| a == s && b == f))
        .filter(|(_, f)| !mentions(&encoder, f))
        .map(|(s, f)| format!("  {s}::{f}"))
        .collect();
    assert!(
        missing.is_empty(),
        "{} field(s) of the battle state are outside the lockstep checksum:\n{}\n\n\
         Add each to `RunnerNetBattle::encode_state`. A field that genuinely is not \
         simulation state goes in NOT_HASHED with the reason it is not.",
        missing.len(),
        missing.join("\n")
    );
    println!("{} fields of Missile and Fighter, all hashed", all.len());
}

fn fields_of(src: &str, want: &str) -> Vec<String> {
    let header = format!("struct {want} {{");
    let mut lines = src.lines();
    while let Some(line) = lines.next() {
        if !line.ends_with(&header) {
            continue;
        }
        let mut out = Vec::new();
        for body in lines.by_ref() {
            if body == "}" {
                return out;
            }
            let Some(rest) = body.strip_prefix("    ") else { continue };
            if rest.starts_with(' ') || rest.starts_with("//") || rest.starts_with('#') {
                continue;
            }
            let rest = rest.strip_prefix("pub(crate) ").or_else(|| rest.strip_prefix("pub ")).unwrap_or(rest);
            let Some((name, _)) = rest.split_once(": ") else { continue };
            if !name.is_empty()
                && name.chars().all(|c| c.is_lowercase() || c.is_numeric() || c == '_')
            {
                out.push(name.to_string());
            }
        }
        return out;
    }
    Vec::new()
}

fn mentions(src: &str, field: &str) -> bool {
    let needle = format!(".{field}");
    let mut from = 0;
    while let Some(at) = src[from..].find(&needle) {
        let end = from + at + needle.len();
        if !src[end..].chars().next().is_some_and(|c| c.is_alphanumeric() || c == '_') {
            return true;
        }
        from = end;
    }
    false
}


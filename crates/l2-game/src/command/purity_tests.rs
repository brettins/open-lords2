//! Presentation code changes the hashed world (`net::sim::encode_state`,
//! `turn::engine::wind::encode_progress`) only through `crate::command`.
//! `docs/netcode.md` §5; C245.

use std::path::Path;

/// Presentation code: screens, the frame driver, the letter ring, sound, app shell.
/// `setup/` is the lobby, before the world is shared.
const SCANNED: &[&str] = &[
    "src/screens",
    "src/screen",
    "src/message",
    "src/audio",
    "src/debug_keys.rs",
    "src/app.rs",
    "src/main.rs",
];

/// Sim-state mutation a screen must not do itself.
const FORBIDDEN: &[&str] = &[
    "&mut ctx.game.kingdom",
    "&mut game.kingdom",
    "*ctx.game = ",
    "battle.take(",
    "ctx.game.battle = ",
    "battle.as_mut()",
    "runner.step(",
    "live.tick(",
    "turn::wind(",
    "turn_clock.tick(",
    "kingdom.counties.get_mut(",
    "kingdom.realms.get_mut(",
    "campaign.units.get_mut(",
    "diplomacy::",
    "siege::",
    "levy::raise",
    "ctx.game.set_tax_rate(",
    "ctx.game.set_ration(",
    "ctx.game.set_ration_split(",
    "ctx.game.raise_army(",
    "ctx.game.split_army(",
    "ctx.game.leave_castle(",
    "ctx.game.disband_army(",
    "ctx.game.order_unit_move(",
    "ctx.game.move_labour(",
    "ctx.game.balance_labour(",
    "ctx.game.balance_all_labour(",
    "ctx.game.rank_realms(",
    "kingdom.paint_field(",
    "kingdom.toggle_industry(",
    "kingdom.toggle_army_foraging(",
    "kingdom.post_letter(",
    "kingdom.set_weapon_type(",
    "kingdom.set_industry_share(",
    "kingdom.offend(",
    "break_alliance(",
    "gift_reply(",
    "trade::trade(",
    "form_alliance(",
    "pay_for_help(",
    "order_engine(",
    "break_siege(",
    "validate_link(",
    "supply::spawn(",
    "unit::combine(",
    "ai::rank_realms(",
    "order_castle(",
    "stamp_castle_terrain(",
    "quirks.set_reproduced(",
    "turn::end_players_turn(",
    "turn::begin_turn(",
    "turn::end_turn(",
    "turn::answer_battle(",
    "turn::take_the_field(",
    "turn::finish_battle(",
    "turn::dismiss_report(",
];

/// A hit that stays: file, pattern, the gate that must stand within `back` lines
/// above (or two below), and why.
struct Allowed {
    file: &'static str,
    pattern: &'static str,
    gate: &'static str,
    back: usize,
    why: &'static str,
}

const ALLOWED: &[Allowed] = &[
    Allowed {
        file: "screens/map/camera.rs",
        pattern: "turn::wind(",
        gate: "if ctx.game.net.lockstep { turn::observe",
        back: 0,
        why: "single player winds here; net::frame winds a network game",
    },
    Allowed {
        file: "screens/battlefield/view/screen/screen.rs",
        pattern: "live.tick(",
        gate: "if !live.deferred {",
        back: 1,
        why: "net::frame steps a deferred battle",
    },
    Allowed {
        file: "screens/saveload/screen/methods.rs",
        pattern: "*ctx.game = ",
        gate: "if ctx.game.net.lockstep {",
        back: 40,
        why: "C245: Load is refused in a network game",
    },
    Allowed {
        file: "screens/saveload/screen/methods.rs",
        pattern: "game.kingdom.campaign.battle_county = ",
        gate: "if ctx.game.net.lockstep {",
        back: 40,
        why: "the loaded world, before the swap above",
    },
    Allowed {
        file: "screens/saveload/screen/methods.rs",
        pattern: "game.kingdom.campaign.move_cost = ",
        gate: "if ctx.game.net.lockstep {",
        back: 40,
        why: "g_moveCost is .bss; the loaded world, before the swap",
    },
    Allowed {
        file: "screens/battlefield/mod.rs",
        pattern: "battle.take(",
        gate: "debug_assert!(!live.deferred && !ctx.game.net.lockstep",
        back: 0,
        why: "a skirmish is local, started from setup/",
    },
    Allowed {
        file: "screen/machine/frame.rs",
        pattern: "turn_clock.tick(",
        gate: "",
        back: 0,
        why: "run_turn_clock's one caller is behind !net.lockstep; net_game holds it",
    },
    Allowed {
        file: "debug_keys.rs",
        pattern: "battle.as_mut()",
        gate: "",
        back: 0,
        why: "add_hits: F4 is behind !net.lockstep, f4_changes_nothing_in_a_network_game",
    },
    Allowed {
        file: "debug_keys.rs",
        pattern: "kingdom.counties.get_mut(",
        gate: "",
        back: 0,
        why: "add_hits, as above",
    },
    Allowed {
        file: "screens/diplomacy/screen.rs",
        pattern: "diplomacy::default_target(",
        gate: "",
        back: 0,
        why: "reads the realms only",
    },
    Allowed {
        file: "screens/siege/mod.rs",
        pattern: "siege::ENGINE_WORK[",
        gate: "",
        back: 0,
        why: "a constant",
    },
];

/// A path through our own screens (`screens::siege`, `super::siege`) is not the sim's.
fn forbidden(code: &str) -> Option<&'static str> {
    FORBIDDEN.iter().copied().find(|p| {
        code.match_indices(p).any(|(at, _)| {
            let before = &code[..at];
            !before.ends_with("screens::") && !before.ends_with("super::")
        })
    })
}

fn assigns_kingdom(line: &str) -> bool {
    let at = line.find("game.kingdom.").or_else(|| {
        line.match_indices("g.kingdom.")
            .find(|(i, _)| *i == 0 || !line.as_bytes()[i - 1].is_ascii_alphanumeric())
            .map(|(i, _)| i)
    });
    let Some(at) = at else { return false };
    let rest = &line[at..];
    let stmt = &rest[..rest.find(';').unwrap_or(rest.len())];
    [" = ", " += ", " -= ", " |= ", " &= "].iter().any(|op| stmt.contains(op))
        && !line.trim_start().starts_with("let ")
}

/// Lines outside `#[cfg(test)]` items, comments and `use` lines cut.
fn live_lines(text: &str) -> Vec<(usize, String)> {
    let (mut out, mut skip, mut depth, mut opened) = (Vec::new(), false, 0i32, false);
    for (n, line) in text.lines().enumerate() {
        let code = line.split("//").next().unwrap_or("");
        if code.trim_start().starts_with("#[cfg(test)]") {
            (skip, depth, opened) = (true, 0, false);
            continue;
        }
        if skip {
            depth += code.matches('{').count() as i32 - code.matches('}').count() as i32;
            opened |= code.contains('{');
            if (opened && depth <= 0) || (!opened && code.trim_end().ends_with(';')) {
                skip = false;
            }
            continue;
        }
        if !code.trim_start().starts_with("use ") {
            out.push((n, code.to_string()));
        }
    }
    out
}

fn scan(root: &Path, path: &Path, out: &mut Vec<String>) {
    if path.is_dir() {
        for entry in std::fs::read_dir(path).expect("dir") {
            let p = entry.expect("entry").path();
            if p.file_name().is_some_and(|n| n != "setup") {
                scan(root, &p, out);
            }
        }
        return;
    }
    let rel = path.strip_prefix(root).expect("under src").to_string_lossy().replace('\\', "/");
    if rel.contains("test") {
        return;
    }
    let text = std::fs::read_to_string(path).expect("source");
    let all: Vec<&str> = text.lines().collect();
    for (n, code) in live_lines(&text) {
        let Some(p) = forbidden(&code).or(assigns_kingdom(&code).then_some("kingdom assign")) else {
            continue;
        };
        let allowed = ALLOWED.iter().any(|a| {
            let window = all[n.saturating_sub(a.back)..(n + 3).min(all.len())].join("\n");
            rel == a.file && code.contains(a.pattern) && window.contains(a.gate) && !a.why.is_empty()
        });
        if !allowed {
            out.push(format!("{rel}:{}: [{p}] {}", n + 1, all[n].trim()));
        }
    }
}

#[test]
fn screens_change_the_world_only_through_commands() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut hits = Vec::new();
    for s in SCANNED {
        scan(&src, &Path::new(env!("CARGO_MANIFEST_DIR")).join(s), &mut hits);
    }
    assert!(hits.is_empty(), "issue a crate::command order instead:\n{}", hits.join("\n"));
}

/// Ablation: drop a gate from `ALLOWED` and its hit returns.
#[test]
fn an_allowed_hit_needs_its_gate() {
    let text = "fn f() {\n    let x = turn::wind(ctx.game);\n}\n";
    let lines = live_lines(text);
    assert!(forbidden(&lines[1].1).is_some());
    assert!(!lines[1].1.contains(ALLOWED[0].gate), "a bare wind is not the gated one");
    let tested = "#[cfg(test)]\nmod t {\n    fn g() { turn::wind(g); }\n}\nfn h() {}\n";
    assert_eq!(live_lines(tested).len(), 1, "a cfg(test) item is skipped whole");
}

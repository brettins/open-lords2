//! `tools/oracle/scenarios/*.jsonl`, one step a line; `FORMAT.md` there is the contract.

#[derive(Debug, Clone, PartialEq)]
pub enum Step {
    Load(String),
    /// `true`: the new campaign, page 5's right box (trace-run.ps1 `$NewGameSteps`).
    NewGame(bool),
    /// x, y, right button; harness-only when its comment ends "(this harness)", which ours skips.
    Click(i32, i32, bool, bool),
    /// `false`: every film takes `Smk_Play` 0x0042D91B's failed-open branch, as `H_SmkPlay` does.
    Films(bool),
    WaitPage(i64, u64),
    WaitScreenNot(i64, u64),
    Dismiss,
    /// trace-run.ps1 `ok`: the OK of the box drawn next frame, whatever the screen.
    Ok,
    Peek,
    Move(i32, i32),
    Drag(i32, i32, i32, i32),
    Record(u64),
    /// A raw event key of the original (`call:army`); ours waits on the record kind it maps to.
    WaitEvent(String, u64),
    Key(String),
    WaitScreen(i64, u64),
    WaitPhase(i64, u64),
    WaitIdle(u64, u64),
    Advance(u64),
    Snap,
    /// The framebuffer as it stands, no frame run (trace-run.ps1 `shot`).
    Shot,
    Anchor(String),
}

pub const MAX_FRAMES: u64 = 20_000;

fn field<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    let quoted = format!("\"{key}\"");
    let rest = line.match_indices(&quoted).find_map(|(at, _)| {
        line[at + quoted.len()..].trim_start().strip_prefix(':')
    })?;
    let rest = rest.trim_start();
    if let Some(s) = rest.strip_prefix('"') {
        return Some(&s[..s.find('"')?]);
    }
    let end = rest.find([',', '}']).unwrap_or(rest.len());
    Some(rest[..end].trim())
}

fn num(line: &str, key: &str) -> Option<i64> {
    let v = field(line, key)?;
    match v.strip_prefix("0x") {
        Some(h) => i64::from_str_radix(h, 16).ok(),
        None => v.parse().ok(),
    }
}

pub fn parse(text: &str) -> Result<Vec<Step>, String> {
    let mut steps = Vec::new();
    let mut harness = false;
    for (n, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with("//") {
            harness = line.ends_with("(this harness)");
            continue;
        }
        let bad = || format!("line {}: {line}", n + 1);
        let max = num(line, "max").map_or(MAX_FRAMES, |m| m as u64);
        let step = match field(line, "op").ok_or_else(bad)? {
            "load" => Step::Load(field(line, "save").ok_or_else(bad)?.to_string()),
            "new-game" => Step::NewGame(field(line, "campaign") == Some("new")),
            "click" => Step::Click(
                num(line, "x").ok_or_else(bad)? as i32,
                num(line, "y").ok_or_else(bad)? as i32,
                field(line, "button") == Some("right"),
                harness,
            ),
            "films" => Step::Films(field(line, "mode") != Some("skip")),
            "wait-page" => Step::WaitPage(num(line, "page").ok_or_else(bad)?, max),
            "dismiss" => Step::Dismiss,
            "peek" => Step::Peek,
            "ok" => Step::Ok,
            "move" => Step::Move(num(line, "x").ok_or_else(bad)? as i32, num(line, "y").ok_or_else(bad)? as i32),
            "drag" => Step::Drag(
                num(line, "x").ok_or_else(bad)? as i32,
                num(line, "y").ok_or_else(bad)? as i32,
                num(line, "x2").ok_or_else(bad)? as i32,
                num(line, "y2").ok_or_else(bad)? as i32,
            ),
            "record" => Step::Record(num(line, "frames").ok_or_else(bad)? as u64),
            "wait-event" => Step::WaitEvent(field(line, "event").ok_or_else(bad)?.to_string(), max),
            "wait-screen-not" => Step::WaitScreenNot(num(line, "id").ok_or_else(bad)?, max),
            "key" => Step::Key(field(line, "key").ok_or_else(bad)?.to_string()),
            "wait-screen" => Step::WaitScreen(num(line, "id").ok_or_else(bad)?, max),
            "wait-phase" => Step::WaitPhase(num(line, "phase").ok_or_else(bad)?, max),
            "wait-idle" => Step::WaitIdle(num(line, "frames").ok_or_else(bad)? as u64, max),
            "advance" => Step::Advance(num(line, "frames").ok_or_else(bad)? as u64),
            "snap" => Step::Snap,
            "shot" => Step::Shot,
            "anchor" => Step::Anchor(field(line, "label").ok_or_else(bad)?.to_string()),
            _ => return Err(bad()),
        };
        harness = false;
        steps.push(step);
    }
    Ok(steps)
}

#[test]
fn every_op_parses() {
    let text = r#"{"op":"load","save":"england.sav"}
{"op":"new-game","campaign":"new"}
{"op":"click","x":320,"y":240}
{"op":"key","key":"Escape"}
{"op":"wait-screen","id":"0x1F","max":900}
{"op":"wait-phase","phase":4}
{"op":"wait-idle","frames":30}
{"op":"advance","frames":5}
{"op":"snap"}
{"op":"anchor","label":"title"}
{"op":"drag","x":1,"y":2,"x2":3,"y2":4}
{"op":"wait-event","event":"call:army"}"#;
    let steps = parse(text).unwrap();
    assert_eq!(steps[2], Step::Click(320, 240, false, false));
    assert_eq!(steps[1], Step::NewGame(true));
    let right = parse(r#"{"op":"click","x":1,"y":2,"button":"right"}"#).unwrap();
    assert_eq!(right, [Step::Click(1, 2, true, false)]);
    let harness = parse("// OK of message 298 (this harness)\n{\"op\":\"click\",\"x\":1,\"y\":2}");
    assert_eq!(harness.unwrap(), [Step::Click(1, 2, false, true)]);
    assert_eq!(steps[4], Step::WaitScreen(0x1F, 900));
    assert_eq!(steps[5], Step::WaitPhase(4, MAX_FRAMES));
    assert_eq!(steps.len(), 12);
    assert_eq!(steps[10], Step::Drag(1, 2, 3, 4));
    assert_eq!(steps[11], Step::WaitEvent("call:army".into(), MAX_FRAMES));
    assert!(parse(r#"{"op":"dance"}"#).is_err());
}

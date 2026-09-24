//! `tools/oracle/scenarios/*.jsonl`, one step a line; `FORMAT.md` there is the contract.

#[derive(Debug, Clone, PartialEq)]
pub enum Step {
    Load(String),
    NewGame,
    /// x, y, right button.
    Click(i32, i32, bool),
    Key(String),
    WaitScreen(i64, u64),
    WaitPhase(i64, u64),
    WaitIdle(u64, u64),
    Advance(u64),
    Snap,
    Anchor(String),
}

const MAX_FRAMES: u64 = 20_000;

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
    for (n, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with("//") {
            continue;
        }
        let bad = || format!("line {}: {line}", n + 1);
        let max = num(line, "max").map_or(MAX_FRAMES, |m| m as u64);
        let step = match field(line, "op").ok_or_else(bad)? {
            "load" => Step::Load(field(line, "save").ok_or_else(bad)?.to_string()),
            "new-game" => Step::NewGame,
            "click" => Step::Click(
                num(line, "x").ok_or_else(bad)? as i32,
                num(line, "y").ok_or_else(bad)? as i32,
                field(line, "button") == Some("right"),
            ),
            "key" => Step::Key(field(line, "key").ok_or_else(bad)?.to_string()),
            "wait-screen" => Step::WaitScreen(num(line, "id").ok_or_else(bad)?, max),
            "wait-phase" => Step::WaitPhase(num(line, "phase").ok_or_else(bad)?, max),
            "wait-idle" => Step::WaitIdle(num(line, "frames").ok_or_else(bad)? as u64, max),
            "advance" => Step::Advance(num(line, "frames").ok_or_else(bad)? as u64),
            "snap" => Step::Snap,
            "anchor" => Step::Anchor(field(line, "label").ok_or_else(bad)?.to_string()),
            _ => return Err(bad()),
        };
        steps.push(step);
    }
    Ok(steps)
}

#[test]
fn every_op_parses() {
    let text = r#"{"op":"load","save":"england.sav"}
{"op":"new-game"}
{"op":"click","x":320,"y":240}
{"op":"key","key":"Escape"}
{"op":"wait-screen","id":"0x1F","max":900}
{"op":"wait-phase","phase":4}
{"op":"wait-idle","frames":30}
{"op":"advance","frames":5}
{"op":"snap"}
{"op":"anchor","label":"title"}"#;
    let steps = parse(text).unwrap();
    assert_eq!(steps[2], Step::Click(320, 240, false));
    let right = parse(r#"{"op":"click","x":1,"y":2,"button":"right"}"#).unwrap();
    assert_eq!(right, [Step::Click(1, 2, true)]);
    assert_eq!(steps[4], Step::WaitScreen(0x1F, 900));
    assert_eq!(steps[5], Step::WaitPhase(4, MAX_FRAMES));
    assert_eq!(steps.len(), 10);
    assert!(parse(r#"{"op":"dance"}"#).is_err());
}

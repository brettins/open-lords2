//! The event sink the differ reads (C246). Only with `--features trace`.
//! `tools/oracle/scenarios/FORMAT.md` is the record's shape, shared with `native/l2-trace`.

use std::cell::RefCell;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Arg {
    I(i64),
    S(String),
}

pub trait IntoArg {
    fn into_arg(self) -> Arg;
}

macro_rules! int_arg {
    ($($t:ty),*) => {$(impl IntoArg for $t { fn into_arg(self) -> Arg { Arg::I(self as i64) } })*};
}
int_arg!(u8, u16, u32, u64, usize, i8, i16, i32, i64, isize, bool);

impl IntoArg for &str {
    fn into_arg(self) -> Arg {
        Arg::S(self.to_string())
    }
}
impl IntoArg for String {
    fn into_arg(self) -> Arg {
        Arg::S(self)
    }
}
impl IntoArg for &String {
    fn into_arg(self) -> Arg {
        Arg::S(self.clone())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Record {
    pub frame: u64,
    pub kind: &'static str,
    /// 0: ours alone, no original function.
    pub addr: u32,
    pub name: &'static str,
    pub args: Vec<(&'static str, Arg)>,
}

impl Record {
    pub fn to_json(&self) -> String {
        let mut s = format!(
            "{{\"frame\":{},\"kind\":\"{}\",\"addr\":{},\"name\":\"{}\",\"caller\":null,\"args\":{{",
            self.frame,
            self.kind,
            if self.addr == 0 { "null".to_string() } else { format!("\"0x{:08X}\"", self.addr) },
            self.name
        );
        for (i, (k, v)) in self.args.iter().enumerate() {
            if i > 0 {
                s.push(',');
            }
            match v {
                Arg::I(n) => s.push_str(&format!("\"{k}\":{n}")),
                Arg::S(t) => s.push_str(&format!("\"{k}\":{}", quote(t))),
            }
        }
        s.push_str("}}");
        s
    }
}

fn quote(t: &str) -> String {
    let mut q = String::from("\"");
    for c in t.chars() {
        match c {
            '"' => q.push_str("\\\""),
            '\\' => q.push_str("\\\\"),
            c if (c as u32) < 0x20 => q.push_str(&format!("\\u{:04x}", c as u32)),
            c => q.push(c),
        }
    }
    q.push('"');
    q
}

#[derive(Default)]
struct Sink {
    on: bool,
    frame: u64,
    records: Vec<Record>,
    rand: u64,
    watched: Vec<(&'static str, i64)>,
    sheet: Option<String>,
    sprite: Option<(String, i64)>,
}

thread_local! {
    static SINK: RefCell<Sink> = RefCell::new(Sink::default());
}

pub fn start() {
    crate::trace_draw::new_run();
    SINK.with(|s| *s.borrow_mut() = Sink { on: true, ..Sink::default() });
}

pub fn stop() -> Vec<Record> {
    set_frame(u64::MAX);
    SINK.with(|s| std::mem::take(&mut *s.borrow_mut()).records)
}

pub fn on() -> bool {
    SINK.with(|s| s.borrow().on)
}

/// Closes the frame: the draws of `Pcg32` since the last call become one `rand` record.
pub fn set_frame(frame: u64) {
    SINK.with(|s| {
        let mut s = s.borrow_mut();
        if s.rand > 0 {
            let r = Record {
                frame: s.frame,
                kind: "rand",
                addr: 0,
                name: "Pcg32",
                args: vec![("count", Arg::I(s.rand as i64))],
            };
            s.records.push(r);
            s.rand = 0;
        }
        if frame != u64::MAX {
            s.frame = frame;
        }
    });
}

pub fn emit(kind: &'static str, addr: u32, name: &'static str, args: Vec<(&'static str, Arg)>) {
    SINK.with(|s| {
        let mut s = s.borrow_mut();
        if s.on {
            let frame = s.frame;
            s.records.push(Record { frame, kind, addr, name, args });
        }
    });
}

pub fn count() -> usize {
    SINK.with(|s| s.borrow().records.len())
}

/// The records from t on, and the frame they are in.
pub fn records_from(at: usize) -> (u64, Vec<Record>) {
    SINK.with(|s| {
        let s = s.borrow();
        (s.frame, s.records.iter().skip(at).cloned().collect())
    })
}

pub fn kinds_from(at: usize) -> Vec<&'static str> {
    SINK.with(|s| s.borrow().records.iter().skip(at).map(|r| r.kind).collect())
}

/// True when `slot` holds a value other than the last one seen (a global's write, as a detour on it sees it).
pub fn changed(slot: &'static str, value: i64) -> bool {
    SINK.with(|s| {
        let mut s = s.borrow_mut();
        if !s.on {
            return false;
        }
        match s.watched.iter_mut().find(|(k, _)| *k == slot) {
            Some((_, v)) if *v == value => false,
            Some((_, v)) => {
                *v = value;
                true
            }
            None => {
                s.watched.push((slot, value));
                true
            }
        }
    })
}

pub fn rand_draw() {
    SINK.with(|s| {
        let mut s = s.borrow_mut();
        if s.on {
            s.rand += 1;
        }
    });
}

/// `Assets::sheet(name)`: the sheet a following `Sheet::frame` belongs to.
pub fn note_sheet(name: &str) {
    SINK.with(|s| {
        let mut s = s.borrow_mut();
        if s.on {
            s.sheet = Some(name.to_ascii_lowercase());
        }
    });
}

/// `Sheet::frame(index)`: pairs the index with the noted sheet for the next blit.
pub fn note_frame(index: usize) {
    SINK.with(|s| {
        let mut s = s.borrow_mut();
        if s.on {
            s.sprite = s.sheet.take().map(|n| (n, index as i64));
        }
    });
}

/// `Sheet::frame` of a sheet whose file is known (`trace_draw::frame_of`).
pub fn set_sprite(sheet: String, index: i64) {
    SINK.with(|s| {
        let mut s = s.borrow_mut();
        if s.on {
            s.sheet = None;
            s.sprite = Some((sheet, index));
        }
    });
}

/// The blit's (sheet, frame), once; `None` when the frame came from a cache.
pub fn take_sprite() -> Option<(String, i64)> {
    SINK.with(|s| s.borrow_mut().sprite.take())
}

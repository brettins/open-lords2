//! Emits a deterministic digest of every PL8 frame in a directory, one line per
//! frame, for differential testing against `tools/pl8digest.js`.
//!
//! ```text
//! cargo run -p l2-formats --release --example pl8digest -- "F:\games\Lords of the Realm II"
//! ```
//!
//! The two implementations were written from `docs/formats/pl8.md` independently;
//! `tools/pl8diff.ps1` runs both and compares the streams line for line. An
//! example rather than a test so the output is clean enough to diff, and so the
//! crate stays dependency-free.

use l2_formats::{Error, Pl8, Storage};
use std::{env, fs, path::PathBuf, process};

/// FNV-1a 64. Hand-rolled in both implementations rather than pulling in a hash
/// crate, so `l2-formats` keeps zero dependencies and the Node side needs no
/// package either. Any stable hash would do; this one is trivially portable.
const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

struct Fnv(u64);

impl Fnv {
    fn new() -> Self {
        Fnv(FNV_OFFSET)
    }

    #[inline]
    fn push(&mut self, b: u8) {
        self.0 = (self.0 ^ b as u64).wrapping_mul(FNV_PRIME);
    }
}

/// Error tokens are normalised to a fixed vocabulary shared with the Node side.
/// Comparing verdicts as well as pixels means the two decoders must also agree
/// on *which* files fail and exactly how, not just on the happy path.
fn token(e: &Error) -> String {
    match e {
        Error::Truncated { .. } => "err:truncated".into(),
        Error::UnsupportedStorage(m) => format!("err:unsupported:{m}"),
        Error::RowOverrun { frame, row, got, want } => {
            format!("err:rowoverrun:{frame}:{row}:{got}:{want}")
        }
        Error::FrameSizeMismatch { frame, .. } => format!("err:framesize:{frame}"),
        Error::FrameOutOfRange { index, .. } => format!("err:framerange:{index}"),
        Error::ZeroLengthRun { frame, row } => format!("err:zerorun:{frame}:{row}"),
        Error::BadPaletteLength(n) => format!("err:palettelen:{n}"),
    }
}

fn main() {
    let Some(dir) = env::args().nth(1) else {
        eprintln!("usage: pl8digest <dir>");
        process::exit(2);
    };

    let mut paths: Vec<PathBuf> = match fs::read_dir(&dir) {
        Ok(rd) => rd
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| {
                p.extension()
                    .and_then(|e| e.to_str())
                    .is_some_and(|e| e.eq_ignore_ascii_case("pl8"))
            })
            .collect(),
        Err(e) => {
            eprintln!("cannot read {dir}: {e}");
            process::exit(2);
        }
    };
    paths.sort_by(|a, b| a.file_name().cmp(&b.file_name()));

    let mut out = String::new();
    for path in &paths {
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        let bytes = match fs::read(path) {
            Ok(b) => b,
            Err(e) => {
                out.push_str(&format!("{name} hdr err:read:{}\n", e.kind() as i32));
                continue;
            }
        };

        let pl8 = match Pl8::parse(&bytes) {
            Ok(p) => p,
            Err(e) => {
                out.push_str(&format!("{name} hdr {}\n", token(&e)));
                continue;
            }
        };

        let mode = match pl8.storage {
            Storage::Raw => 0,
            Storage::Rle => 1,
            Storage::Unknown(m) => m,
        };
        let verdict = match pl8.validate() {
            Ok(()) => "ok".to_string(),
            Err(e) => token(&e),
        };
        out.push_str(&format!(
            "{name} hdr mode={mode} sub={} frames={} verdict={verdict}\n",
            pl8.sub_mode,
            pl8.frames.len()
        ));

        // Frames are digested even when the file fails its end-offset invariant:
        // most of the KNOWN_FAILING files decode every frame correctly and only
        // trip on trailing bytes, so their pixels are still worth comparing.
        for i in 0..pl8.frames.len() {
            let info = &pl8.frames[i];
            match pl8.decode(i) {
                Ok(f) => {
                    let mut h = Fnv::new();
                    for (px, &op) in f.indices.iter().zip(f.opaque.iter()) {
                        h.push(*px);
                        h.push(op as u8);
                    }
                    out.push_str(&format!(
                        "{name} {i} {}x{} {:016x}\n",
                        f.width, f.height, h.0
                    ));
                }
                Err(e) => out.push_str(&format!(
                    "{name} {i} {}x{} {}\n",
                    info.width,
                    info.height,
                    token(&e)
                )),
            }
        }
    }
    print!("{out}");
}

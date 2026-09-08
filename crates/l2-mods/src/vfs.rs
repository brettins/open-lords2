//! The overlay virtual filesystem.
//!
//! Assets resolve through a stack of layers: the base game install at the
//! bottom, then each enabled mod, last one winning. Nothing above the VFS ever
//! sees a real path, which is what lets a mod replace `Base1a.pl8` without any
//! code knowing that `Base1a.pl8` can be replaced.
//!
//! ## Case
//!
//! The shipped install is not consistent about case. In one directory it has
//! `Axemen.smk` and `AXMEN.SMK`, `Bat_los4.smk` and `BAT_LOS5.SMK`, and the
//! executable asks for names in a third casing again. Lookup is therefore
//! case-insensitive (ASCII only — the names are all ASCII) regardless of what
//! the host filesystem does. On Windows that is redundant; on Linux, where the
//! same install lives on a case-sensitive filesystem, it is the difference
//! between a working game and 45 missing videos.
//!
//! Case-folding can collide where the filesystem allowed two files that differ
//! only in case. That cannot happen on NTFS but can on ext4, so the index
//! resolves it deterministically — lowest raw name wins — and records a
//! [`CaseCollision`] rather than picking whichever `read_dir` happened to
//! return first.
//!
//! ## Read-only by construction
//!
//! There is no write API. `CLAUDE.md` rule 2 says the game installs are
//! read-only, and the cheapest way to keep a rule is to make breaking it
//! impossible: a mod's output goes somewhere else entirely, through code that
//! never had a handle on a layer root.

use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// One layer of the stack.
#[derive(Debug, Clone)]
pub struct Layer {
    /// Stable identifier used in diagnostics: `"base"`, or a mod's id.
    pub id: String,
    pub root: PathBuf,
}

/// A file offered by one layer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Provider {
    /// Index into [`Vfs::layers`].
    pub layer: usize,
    /// The real path on disk, in whatever case the filesystem holds.
    pub path: PathBuf,
}

/// Two files in a single layer whose names differ only in case.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaseCollision {
    pub layer: String,
    /// The normalised key both files claim.
    pub key: String,
    /// Every real name, sorted. The first is the one that wins.
    pub names: Vec<String>,
}

#[derive(Debug)]
pub enum VfsError {
    /// The layer root does not exist or is not a directory.
    BadRoot { id: String, root: PathBuf },
    /// A layer id was added twice.
    DuplicateLayer(String),
    NotFound(String),
    Io { name: String, source: io::Error },
}

impl fmt::Display for VfsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VfsError::BadRoot { id, root } => {
                write!(f, "layer '{id}': {} is not a directory", root.display())
            }
            VfsError::DuplicateLayer(id) => write!(f, "layer '{id}' is already mounted"),
            VfsError::NotFound(n) => write!(f, "no layer provides '{n}'"),
            VfsError::Io { name, source } => write!(f, "reading '{name}': {source}"),
        }
    }
}

impl std::error::Error for VfsError {}

/// The resolved overlay.
#[derive(Debug, Default)]
pub struct Vfs {
    layers: Vec<Layer>,
    /// Normalised name -> providers in layer order. The last is the winner.
    index: BTreeMap<String, Vec<Provider>>,
    collisions: Vec<CaseCollision>,
}

impl Vfs {
    pub fn new() -> Self {
        Vfs::default()
    }

    /// Mount a directory on top of the stack. Later layers win.
    ///
    /// The whole tree is indexed eagerly. For the shipped install that is one
    /// flat directory of ~1,200 entries, so the cost is a single `read_dir`
    /// and the payoff is that every later lookup is a map hit with no
    /// filesystem round trip — which matters when the alternative is probing
    /// N layers for every one of 291 sprite files.
    pub fn push_layer(&mut self, id: impl Into<String>, root: impl AsRef<Path>) -> Result<(), VfsError> {
        let id = id.into();
        let root = root.as_ref().to_path_buf();
        if self.layers.iter().any(|l| l.id == id) {
            return Err(VfsError::DuplicateLayer(id));
        }
        if !root.is_dir() {
            return Err(VfsError::BadRoot { id, root });
        }

        let layer_index = self.layers.len();
        // key -> (raw relative name, real path), collecting every candidate so
        // a case collision is reported instead of silently resolved.
        let mut found: BTreeMap<String, Vec<(String, PathBuf)>> = BTreeMap::new();
        walk(&root, &root, &mut found);

        for (key, mut candidates) in found {
            candidates.sort();
            if candidates.len() > 1 {
                self.collisions.push(CaseCollision {
                    layer: id.clone(),
                    key: key.clone(),
                    names: candidates.iter().map(|(n, _)| n.clone()).collect(),
                });
            }
            let (_, path) = candidates.into_iter().next().expect("non-empty");
            self.index
                .entry(key)
                .or_default()
                .push(Provider { layer: layer_index, path });
        }

        self.layers.push(Layer { id, root });
        Ok(())
    }

    pub fn layers(&self) -> &[Layer] {
        &self.layers
    }

    pub fn layer_id(&self, index: usize) -> &str {
        self.layers.get(index).map(|l| l.id.as_str()).unwrap_or("<unknown>")
    }

    pub fn case_collisions(&self) -> &[CaseCollision] {
        &self.collisions
    }

    /// Every layer that offers `name`, bottom first.
    pub fn providers(&self, name: &str) -> &[Provider] {
        self.index.get(&normalise(name)).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// The real path the overlay resolves `name` to, or `None`.
    pub fn resolve(&self, name: &str) -> Option<&Path> {
        self.providers(name).last().map(|p| p.path.as_path())
    }

    pub fn exists(&self, name: &str) -> bool {
        self.index.contains_key(&normalise(name))
    }

    /// The bytes the overlay resolves `name` to.
    ///
    /// Sprite decoding stays a two-step: `l2_formats::Pl8` borrows its buffer,
    /// so the caller has to own the bytes.
    ///
    /// ```no_run
    /// # use l2_mods::Vfs;
    /// # let vfs = Vfs::new();
    /// let bytes = vfs.read("Base1a.pl8").unwrap();
    /// let pl8 = l2_formats::Pl8::parse(&bytes).unwrap();
    /// # let _ = pl8;
    /// ```
    pub fn read(&self, name: &str) -> Result<Vec<u8>, VfsError> {
        let path = self.resolve(name).ok_or_else(|| VfsError::NotFound(name.to_string()))?;
        fs::read(path).map_err(|e| VfsError::Io { name: name.to_string(), source: e })
    }

    pub fn read_to_string(&self, name: &str) -> Result<String, VfsError> {
        let path = self.resolve(name).ok_or_else(|| VfsError::NotFound(name.to_string()))?;
        fs::read_to_string(path).map_err(|e| VfsError::Io { name: name.to_string(), source: e })
    }

    /// Normalised names of everything the overlay can see, sorted.
    pub fn entries(&self) -> impl Iterator<Item = &str> {
        self.index.keys().map(|k| k.as_str())
    }

    /// Normalised names with the given extension, sorted.
    pub fn entries_with_extension(&self, ext: &str) -> Vec<&str> {
        let suffix = format!(".{}", ext.to_ascii_lowercase());
        self.index.keys().filter(|k| k.ends_with(&suffix)).map(|k| k.as_str()).collect()
    }

    /// Normalised names under a directory prefix, sorted. `"rules"` matches
    /// `rules/troops.toml` but not `rulesets/x`.
    pub fn entries_under(&self, prefix: &str) -> Vec<&str> {
        let mut p = normalise(prefix);
        if !p.ends_with('/') {
            p.push('/');
        }
        self.index.keys().filter(|k| k.starts_with(&p)).map(|k| k.as_str()).collect()
    }

    /// Every name a single layer contributes, sorted.
    ///
    /// What that layer *offers*, not what it wins — a name here may well
    /// resolve to a higher layer. Answering "did my mod's file take effect"
    /// needs the offer and the winner separately.
    pub fn layer_entries(&self, layer: usize) -> Vec<&str> {
        self.index
            .iter()
            .filter(|(_, providers)| providers.iter().any(|p| p.layer == layer))
            .map(|(key, _)| key.as_str())
            .collect()
    }

    /// Files a single layer contributes under a prefix, sorted.
    ///
    /// Rule documents need this rather than [`Self::resolve`]: rules from
    /// every layer are *merged*, so every layer's copy must be read, not just
    /// the winning one. Assets shadow; rules accumulate. That asymmetry is
    /// deliberate and is the reason both accessors exist.
    pub fn layer_entries_under(&self, layer: usize, prefix: &str) -> Vec<(&str, &Path)> {
        let mut p = normalise(prefix);
        if !p.ends_with('/') {
            p.push('/');
        }
        let mut out = Vec::new();
        for (key, providers) in &self.index {
            if !key.starts_with(&p) {
                continue;
            }
            if let Some(prov) = providers.iter().find(|prov| prov.layer == layer) {
                out.push((key.as_str(), prov.path.as_path()));
            }
        }
        out
    }

    /// Every name more than one layer provides, with the layers involved.
    ///
    /// This is the asset half of the "two mods touched the same thing"
    /// diagnostic; [`crate::MergeLog`] is the rules half.
    pub fn shadowed(&self) -> Vec<(&str, Vec<&str>)> {
        self.index
            .iter()
            .filter(|(_, p)| p.len() > 1)
            .map(|(k, p)| {
                (k.as_str(), p.iter().map(|prov| self.layer_id(prov.layer)).collect::<Vec<_>>())
            })
            .collect()
    }

    // ---- typed loads, via l2-formats -------------------------------------
    //
    // Decoding lives in l2-formats and stays there. These are only the wiring
    // that gets bytes to it through the overlay.

    /// Read and parse a 768-byte palette.
    pub fn palette(&self, name: &str) -> Result<l2_formats::Palette, AssetError> {
        let bytes = self.read(name)?;
        l2_formats::Palette::from_bytes(&bytes)
            .map_err(|e| AssetError::Decode { name: name.to_string(), source: e })
    }
}

/// A read that got as far as decoding.
#[derive(Debug)]
pub enum AssetError {
    Vfs(VfsError),
    Decode { name: String, source: l2_formats::Error },
}

impl From<VfsError> for AssetError {
    fn from(e: VfsError) -> Self {
        AssetError::Vfs(e)
    }
}

impl fmt::Display for AssetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AssetError::Vfs(e) => write!(f, "{e}"),
            AssetError::Decode { name, source } => write!(f, "decoding '{name}': {source}"),
        }
    }
}

impl std::error::Error for AssetError {}

/// Fold a name into the form the index is keyed on: forward slashes, ASCII
/// lowercase, no leading `./` or `/`.
///
/// ASCII-only folding is a decision, not an oversight. Every name in the
/// shipped game is ASCII, and Unicode case folding is locale-dependent in ways
/// that would make the same mod resolve differently in Turkey than in England.
pub fn normalise(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for ch in name.chars() {
        match ch {
            '\\' => out.push('/'),
            c => out.push(c.to_ascii_lowercase()),
        }
    }
    let trimmed = out.trim_start_matches("./").trim_start_matches('/');
    trimmed.to_string()
}

fn walk(root: &Path, dir: &Path, out: &mut BTreeMap<String, Vec<(String, PathBuf)>>) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk(root, &path, out);
        } else if let Ok(rel) = path.strip_prefix(root) {
            let raw = rel.to_string_lossy().replace('\\', "/");
            out.entry(normalise(&raw)).or_default().push((raw, path));
        }
    }
}

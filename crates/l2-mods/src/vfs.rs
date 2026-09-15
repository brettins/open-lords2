
use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct Layer {
    pub id: String,
    pub root: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Provider {
    pub layer: usize,
    pub path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaseCollision {
    pub layer: String,
    pub key: String,
    pub names: Vec<String>,
}

#[derive(Debug)]
pub enum VfsError {
    BadRoot { id: String, root: PathBuf },
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

#[derive(Debug, Default)]
pub struct Vfs {
    layers: Vec<Layer>,
    index: BTreeMap<String, Vec<Provider>>,
    collisions: Vec<CaseCollision>,
}

impl Vfs {
    pub fn new() -> Self {
        Vfs::default()
    }

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

    pub fn providers(&self, name: &str) -> &[Provider] {
        self.index.get(&normalise(name)).map(|v| v.as_slice()).unwrap_or(&[])
    }

    pub fn resolve(&self, name: &str) -> Option<&Path> {
        self.providers(name).last().map(|p| p.path.as_path())
    }

    pub fn exists(&self, name: &str) -> bool {
        self.index.contains_key(&normalise(name))
    }

    pub fn read(&self, name: &str) -> Result<Vec<u8>, VfsError> {
        let path = self.resolve(name).ok_or_else(|| VfsError::NotFound(name.to_string()))?;
        fs::read(path).map_err(|e| VfsError::Io { name: name.to_string(), source: e })
    }

    pub fn read_to_string(&self, name: &str) -> Result<String, VfsError> {
        let path = self.resolve(name).ok_or_else(|| VfsError::NotFound(name.to_string()))?;
        fs::read_to_string(path).map_err(|e| VfsError::Io { name: name.to_string(), source: e })
    }

    pub fn entries(&self) -> impl Iterator<Item = &str> {
        self.index.keys().map(|k| k.as_str())
    }

    pub fn entries_with_extension(&self, ext: &str) -> Vec<&str> {
        let suffix = format!(".{}", ext.to_ascii_lowercase());
        self.index.keys().filter(|k| k.ends_with(&suffix)).map(|k| k.as_str()).collect()
    }

    pub fn entries_under(&self, prefix: &str) -> Vec<&str> {
        let mut p = normalise(prefix);
        if !p.ends_with('/') {
            p.push('/');
        }
        self.index.keys().filter(|k| k.starts_with(&p)).map(|k| k.as_str()).collect()
    }

    pub fn layer_entries(&self, layer: usize) -> Vec<&str> {
        self.index
            .iter()
            .filter(|(_, providers)| providers.iter().any(|p| p.layer == layer))
            .map(|(key, _)| key.as_str())
            .collect()
    }

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

    pub fn shadowed(&self) -> Vec<(&str, Vec<&str>)> {
        self.index
            .iter()
            .filter(|(_, p)| p.len() > 1)
            .map(|(k, p)| {
                (k.as_str(), p.iter().map(|prov| self.layer_id(prov.layer)).collect::<Vec<_>>())
            })
            .collect()
    }


    pub fn palette(&self, name: &str) -> Result<l2_formats::Palette, AssetError> {
        let bytes = self.read(name)?;
        l2_formats::Palette::from_bytes(&bytes)
            .map_err(|e| AssetError::Decode { name: name.to_string(), source: e })
    }
}

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

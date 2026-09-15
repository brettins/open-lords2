
use crate::digest;
use crate::modmeta::{MetaError, ModMeta, MANIFEST};
use crate::ruleset::{RuleError, Ruleset, RULES_DIR};
use std::fmt;
use std::path::{Path, PathBuf};

pub fn is_platform_metadata(name: &str) -> bool {
    name == MANIFEST
        || (name.len() > RULES_DIR.len()
            && name.starts_with(RULES_DIR)
            && name.as_bytes()[RULES_DIR.len()] == b'/'
            && name.ends_with(".toml"))
}

#[derive(Debug)]
pub struct ModPackage {
    pub meta: ModMeta,
    pub rule_documents: Vec<String>,
    pub assets: Vec<String>,
    pub rule_paths: Vec<String>,
    pub warnings: Vec<Warning>,
    pub rules_digest: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Warning {
    TomlOutsideRules(String),
    NonTomlInRules(String),
    Empty,
    FloatRule { path: String, at: String },
}

impl fmt::Display for Warning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Warning::TomlOutsideRules(p) => write!(
                f,
                "'{p}' is a .toml outside rules/, so it is treated as a file to shadow rather \
                 than as rules"
            ),
            Warning::NonTomlInRules(p) => {
                write!(f, "'{p}' is in rules/ but is not a .toml, so it will be ignored")
            }
            Warning::Empty => write!(f, "this mod sets no rules and provides no files"),
            Warning::FloatRule { path, at } => write!(
                f,
                "{at}: rule '{path}' is a decimal. The simulation is integer-only \
                 (docs/netcode.md), so a decimal here cannot reach it"
            ),
        }
    }
}

#[derive(Debug)]
pub enum PackageError {
    NotAMod(PathBuf),
    Meta(MetaError),
    Rule(RuleError),
    Io { path: PathBuf, source: std::io::Error },
}

impl fmt::Display for PackageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PackageError::NotAMod(p) => {
                write!(f, "{}: no {MANIFEST}, so this is not a mod", p.display())
            }
            PackageError::Meta(e) => write!(f, "{e}"),
            PackageError::Rule(e) => write!(f, "{e}"),
            PackageError::Io { path, source } => write!(f, "{}: {source}", path.display()),
        }
    }
}

impl std::error::Error for PackageError {}

impl From<MetaError> for PackageError {
    fn from(e: MetaError) -> Self {
        PackageError::Meta(e)
    }
}
impl From<RuleError> for PackageError {
    fn from(e: RuleError) -> Self {
        PackageError::Rule(e)
    }
}

pub fn inspect(dir: &Path) -> Result<ModPackage, PackageError> {
    if !dir.join(MANIFEST).is_file() {
        return Err(PackageError::NotAMod(dir.to_path_buf()));
    }
    let meta = ModMeta::load(dir)?;

    let mut files = Vec::new();
    walk(dir, dir, &mut files).map_err(|source| PackageError::Io {
        path: dir.to_path_buf(),
        source,
    })?;
    files.sort();

    let rules_prefix = format!("{RULES_DIR}/");
    let mut rule_documents = Vec::new();
    let mut assets = Vec::new();
    let mut warnings = Vec::new();

    for rel in files {
        if rel == MANIFEST {
            continue;
        }
        let lower = rel.to_ascii_lowercase();
        if lower.starts_with(&rules_prefix) {
            if lower.ends_with(".toml") {
                rule_documents.push(rel);
            } else {
                warnings.push(Warning::NonTomlInRules(rel.clone()));
                assets.push(rel);
            }
        } else {
            if lower.ends_with(".toml") {
                warnings.push(Warning::TomlOutsideRules(rel.clone()));
            }
            assets.push(rel);
        }
    }

    let mut rs = Ruleset::new();
    for rel in &rule_documents {
        let source = format!("{}:{rel}", meta.id);
        rs.apply_file(&dir.join(rel.replace('/', std::path::MAIN_SEPARATOR_STR)), &source)?;
    }
    let rule_paths = rs.leaves();
    for (path, origin) in rs.float_rules() {
        warnings.push(Warning::FloatRule { path, at: origin.to_string() });
    }
    if rule_paths.is_empty() && assets.is_empty() {
        warnings.push(Warning::Empty);
    }

    Ok(ModPackage {
        rules_digest: digest::digest(&rs),
        meta,
        rule_documents,
        assets,
        rule_paths,
        warnings,
    })
}

pub fn inspect_all(dir: &Path) -> Result<Vec<ModPackage>, PackageError> {
    let mut dirs: Vec<PathBuf> = match std::fs::read_dir(dir) {
        Ok(entries) => entries.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect(),
        Err(source) => return Err(PackageError::Io { path: dir.to_path_buf(), source }),
    };
    dirs.sort();
    let mut out = Vec::new();
    for d in dirs {
        if d.join(MANIFEST).is_file() {
            out.push(inspect(&d)?);
        }
    }
    out.sort_by(|a, b| a.meta.id.cmp(&b.meta.id));
    Ok(out)
}

impl fmt::Display for ModPackage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "{} {} ({})", self.meta.id, self.meta.version, self.meta.name)?;
        if let Some(a) = &self.meta.author {
            writeln!(f, "  by {a}")?;
        }
        writeln!(f, "  rules digest {:016x}", self.rules_digest)?;
        if !self.meta.requires.is_empty() {
            let list: Vec<String> =
                self.meta.requires.iter().map(|d| format!("{} {}", d.id, d.req)).collect();
            writeln!(f, "  requires  {}", list.join(", "))?;
        }
        if !self.meta.after.is_empty() {
            writeln!(f, "  after     {}", self.meta.after.join(", "))?;
        }
        if !self.meta.conflicts.is_empty() {
            writeln!(f, "  conflicts {}", self.meta.conflicts.join(", "))?;
        }
        writeln!(
            f,
            "  {} rule document(s) setting {} rule(s); {} file(s)",
            self.rule_documents.len(),
            self.rule_paths.len(),
            self.assets.len()
        )?;
        for w in &self.warnings {
            writeln!(f, "  warning: {w}")?;
        }
        Ok(())
    }
}

fn walk(root: &Path, dir: &Path, out: &mut Vec<String>) -> std::io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            walk(root, &path, out)?;
        } else if let Ok(rel) = path.strip_prefix(root) {
            out.push(rel.to_string_lossy().replace('\\', "/"));
        }
    }
    Ok(())
}

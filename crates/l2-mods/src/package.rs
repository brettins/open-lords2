//! Mod packages: what a mod is on disk, and checking one before it is loaded.
//!
//! # What a package is
//!
//! A directory with a `mod.toml` at its root. Everything else — which assets
//! it replaces, which rules it changes — is discovered from its contents
//! rather than declared, because a declaration is a second source of truth
//! that goes stale the first time someone adds a file and forgets.
//!
//! ```text
//! longbows/
//!   mod.toml            manifest: id, name, version, ordering constraints
//!   rules/*.toml        rule documents; every layer's are read and merged
//!   <anything else>     assets, shadowing the same name in a lower layer
//! ```
//!
//! # Why there is no archive format
//!
//! The obvious next step is a `.l2mod` file, and it was left out on purpose.
//! An archive buys one thing — a single file to send someone — and every
//! operating system already ships a zip tool that does exactly that to a
//! directory. Against that: the platform has to read directories anyway,
//! because that is what a mod under development is, so an archive format would
//! be a *second* loading path to keep working and to keep in step. `docs/
//! decisions.md` D5a is a standing reminder of what a compression dependency
//! costs on this project's licence position, and a stored-only zip writer is
//! code we would own forever to save the user one right-click.
//!
//! Revisit when mods are distributed rather than hand-copied, which is the
//! same trigger as signing and checksums (`docs/modding.md` §13).
//!
//! # What inspection is for
//!
//! [`inspect`] answers the questions a mod author asks *before* trying to load
//! the thing, and answers them one mod at a time: does the manifest parse, do
//! all the rule documents parse, what does this mod actually claim, and is
//! this the same package I shipped? A load failure names one error and stops;
//! an inspection reports everything at once, which is the right shape for a
//! tool a person runs on their own work.

use crate::digest;
use crate::modmeta::{MetaError, ModMeta, MANIFEST};
use crate::ruleset::{RuleError, Ruleset, RULES_DIR};
use std::fmt;
use std::path::{Path, PathBuf};

/// True for files that belong to the platform rather than to the game.
///
/// `name` must already be normalised the way [`crate::Vfs`] keys its index:
/// lowercase, forward slashes.
///
/// Two kinds of file live in a mod directory and are not assets:
///
/// * **`mod.toml`.** Every mod has one, so without this every *pair* of
///   enabled mods reports a conflict over their manifests — noise that would
///   bury the one real conflict underneath it.
/// * **`rules/*.toml`.** Rules accumulate rather than shadow. Two mods each
///   carrying a `rules/rules.toml` are both read and both merged, so calling
///   the later one the winner would be exactly backwards.
///
/// The overlay index still holds both, because it is a faithful index of
/// what is on disk and it is not its business to decide what a file means.
/// The decision is made here, once, so the report and the per-mod effect
/// analysis cannot disagree about it.
pub fn is_platform_metadata(name: &str) -> bool {
    name == MANIFEST
        || (name.len() > RULES_DIR.len()
            && name.starts_with(RULES_DIR)
            && name.as_bytes()[RULES_DIR.len()] == b'/'
            && name.ends_with(".toml"))
}

/// A mod directory, read and checked.
#[derive(Debug)]
pub struct ModPackage {
    pub meta: ModMeta,
    /// Rule documents, as paths relative to the mod root, sorted.
    pub rule_documents: Vec<String>,
    /// Every other file, relative to the mod root, sorted. These are the names
    /// that will shadow a lower layer's file of the same name.
    pub assets: Vec<String>,
    /// Every rule leaf this mod sets, sorted. What it *claims*; whether a
    /// claim survives depends on load order and is [`crate::effect`]'s job.
    pub rule_paths: Vec<String>,
    /// Problems that do not stop the package being read.
    pub warnings: Vec<Warning>,
    /// A checksum over the rules this mod sets, independent of where the mod
    /// is installed. Two copies of the same mod at different paths give the
    /// same digest; a mod whose rules were edited gives a different one.
    ///
    /// Deliberately *not* a checksum of the files: an asset the author
    /// re-exported with a different timestamp is the same mod, and a digest
    /// that said otherwise would be noise. Asset identity, if it is ever
    /// needed, is a separate question from rule identity.
    pub rules_digest: u64,
}

/// Something worth telling the author, which is not a reason to refuse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Warning {
    /// A `.toml` outside `rules/`. Legal — it will be treated as an asset —
    /// but it is nearly always a rule file in the wrong directory, which is a
    /// mod that loads cleanly and does nothing.
    TomlOutsideRules(String),
    /// A `rules/` file that is not `.toml`, so the loader will skip it.
    NonTomlInRules(String),
    /// The mod sets no rules and provides no assets.
    Empty,
    /// A rule value that is a float. `docs/netcode.md` forbids floats in
    /// anything the simulation evaluates.
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

/// Read and check one mod directory.
///
/// Every rule document is parsed, so a syntax error is found here rather than
/// at the load that a player triggers.
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

    // Parse the documents on their own, in the same order the loader would.
    // On their own is the point: this is what the mod says, not what it says
    // once everything else has had its turn.
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

/// Every mod directory under `dir`, inspected. Sorted by id.
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

//! The modding platform for the lords2 engine.
//!
//! Three pieces, and the seam between them is the whole design:
//!
//! * [`Vfs`] — an overlay filesystem. Assets resolve through a stack of
//!   layers, base install at the bottom, mods above, last one winning. No code
//!   above this layer ever holds a real path, so any file is replaceable
//!   without anything knowing it can be.
//! * [`Ruleset`] — game rules as merged data documents. A mod restates only
//!   the keys it changes. Every value remembers which document and which line
//!   set it, so "these two mods disagree" is a diagnostic rather than a
//!   mystery.
//! * [`ModMeta`] and [`resolve_load_order`] — discovery, dependencies,
//!   conflicts and a deterministic order.
//!
//! Assets shadow; rules accumulate. A sprite has no partial form, so
//! overriding one means replacing it. A rule table does, so overriding one
//! means changing a number.
//!
//! # Putting it together
//!
//! ```no_run
//! use l2_mods::{Platform, Vfs};
//!
//! let platform = Platform::builder()
//!     .base(r"F:\games\Lords of the Realm II")
//!     .mods_dir("mods")
//!     .enable(["longbows", "harder-sieges"])
//!     .build()?;
//!
//! // An asset, resolved through the overlay.
//! let bytes = platform.vfs.read("Base1a.pl8")?;
//!
//! // A rule, with the mod that set it.
//! let n = platform.rules.integer("battle.three_bridges.attacker.crossbows")?;
//! println!("{n} from {}", platform.rules.origin("battle.three_bridges.attacker.crossbows").unwrap());
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! # What is deliberately absent
//!
//! There is no scripting, no event hooks and no way for a mod to run code.
//! Every one of those is a decision that should be made against a real
//! simulation, and making it now would be guessing. The data-driven half is
//! the half that is expensive to retrofit; the scripting half is not.

pub mod merge;
pub mod modmeta;
pub mod reader;
pub mod ruleset;
pub mod seed;
pub mod troops;
pub mod value;
pub mod vfs;

pub use merge::{Deletion, MergeLog, Override};
pub use modmeta::{
    discover, resolve_load_order, Dependency, LoadOrderError, MetaError, ModMeta, Version,
    VersionReq,
};
pub use reader::ParseError;
pub use ruleset::{RuleError, Ruleset, RULES_DIR};
pub use troops::{Side, TroopRules};
pub use value::{Origin, Spanned, Table, Value};
pub use vfs::{AssetError, CaseCollision, Layer, Vfs, VfsError};

use std::fmt;
use std::path::{Path, PathBuf};

/// The id given to the base game layer.
pub const BASE_LAYER: &str = "base";

/// A loaded game: the overlay, the merged rules, and the order that produced
/// them.
#[derive(Debug)]
pub struct Platform {
    pub vfs: Vfs,
    pub rules: Ruleset,
    /// Mods in load order. The base install is not in this list.
    pub load_order: Vec<ModMeta>,
}

impl Platform {
    pub fn builder() -> PlatformBuilder {
        PlatformBuilder::default()
    }

    /// Everything worth telling the player after a load: files one mod took
    /// from another, rules one mod took from another, and anything that looks
    /// like a mistake.
    pub fn report(&self) -> Report {
        Report {
            shadowed_assets: self
                .vfs
                .shadowed()
                .into_iter()
                .map(|(n, layers)| (n.to_string(), layers.into_iter().map(str::to_string).collect()))
                .collect(),
            case_collisions: self.vfs.case_collisions().to_vec(),
            overrides: self.rules.log.overrides.clone(),
            deletions: self.rules.log.deletions.clone(),
            dangling_deletes: self
                .rules
                .log
                .dangling_deletes
                .iter()
                .map(|d| (d.path.clone(), d.by.to_string()))
                .collect(),
            type_changes: self
                .rules
                .log
                .overrides
                .iter()
                .filter(|o| o.type_changed)
                .cloned()
                .collect(),
        }
    }
}

/// What happened during a load, in a form suitable for printing.
#[derive(Debug, Default, Clone)]
pub struct Report {
    /// `(file, layers that provide it, in order)`.
    pub shadowed_assets: Vec<(String, Vec<String>)>,
    pub case_collisions: Vec<CaseCollision>,
    pub overrides: Vec<Override>,
    pub deletions: Vec<Deletion>,
    /// `("$delete" target, where it was written)` for targets that were absent.
    pub dangling_deletes: Vec<(String, String)>,
    /// Overrides that changed a value's type. Nearly always a bug.
    pub type_changes: Vec<Override>,
}

impl Report {
    /// True when nothing at all was contested.
    pub fn is_quiet(&self) -> bool {
        self.shadowed_assets.is_empty()
            && self.case_collisions.is_empty()
            && self.overrides.is_empty()
            && self.deletions.is_empty()
            && self.dangling_deletes.is_empty()
    }
}

impl fmt::Display for Report {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_quiet() {
            return writeln!(f, "no conflicts");
        }
        if !self.shadowed_assets.is_empty() {
            writeln!(f, "assets provided by more than one layer:")?;
            for (name, layers) in &self.shadowed_assets {
                writeln!(f, "  {name}: {} (last wins)", layers.join(" -> "))?;
            }
        }
        if !self.case_collisions.is_empty() {
            writeln!(f, "filenames differing only in case:")?;
            for c in &self.case_collisions {
                writeln!(f, "  {}: {} ({} used)", c.layer, c.names.join(", "), c.names[0])?;
            }
        }
        if !self.overrides.is_empty() {
            writeln!(f, "rules overridden:")?;
            for o in &self.overrides {
                writeln!(f, "  {} : {} -> {}", o.path, o.previous, o.current)?;
            }
        }
        if !self.type_changes.is_empty() {
            writeln!(f, "rules whose type changed (probably a mistake):")?;
            for o in &self.type_changes {
                writeln!(f, "  {} : {} -> {}", o.path, o.previous, o.current)?;
            }
        }
        for d in &self.deletions {
            writeln!(f, "rule deleted: {} (was {}, by {})", d.path, d.removed, d.by)?;
        }
        for (path, by) in &self.dangling_deletes {
            writeln!(f, "\"$delete\" named '{path}', which was not defined (at {by})")?;
        }
        Ok(())
    }
}

/// Assembles a [`Platform`].
#[derive(Debug, Default)]
pub struct PlatformBuilder {
    base: Option<PathBuf>,
    mods_dir: Option<PathBuf>,
    extra: Vec<ModMeta>,
    enabled: Vec<String>,
}

impl PlatformBuilder {
    /// The game install. Read-only; nothing here ever writes to it.
    pub fn base(mut self, dir: impl AsRef<Path>) -> Self {
        self.base = Some(dir.as_ref().to_path_buf());
        self
    }

    /// A directory whose immediate subdirectories are candidate mods.
    pub fn mods_dir(mut self, dir: impl AsRef<Path>) -> Self {
        self.mods_dir = Some(dir.as_ref().to_path_buf());
        self
    }

    /// A mod from somewhere other than the mods directory — a development
    /// checkout, say.
    pub fn add_mod(mut self, meta: ModMeta) -> Self {
        self.extra.push(meta);
        self
    }

    /// The ids to enable, in the player's preferred order. Order is honoured
    /// wherever dependencies allow.
    pub fn enable<I, S>(mut self, ids: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.enabled.extend(ids.into_iter().map(Into::into));
        self
    }

    pub fn build(self) -> Result<Platform, Error> {
        let mut available = self.extra;
        if let Some(dir) = &self.mods_dir {
            if dir.is_dir() {
                available.extend(discover(dir)?);
            }
        }
        available.sort_by(|a, b| a.id.cmp(&b.id));
        for pair in available.windows(2) {
            if pair[0].id == pair[1].id {
                return Err(Error::LoadOrder(LoadOrderError::Duplicate(pair[0].id.clone())));
            }
        }

        let load_order = resolve_load_order(&available, &self.enabled)?;

        let mut vfs = Vfs::new();
        if let Some(base) = &self.base {
            vfs.push_layer(BASE_LAYER, base)?;
        }
        for m in &load_order {
            vfs.push_layer(m.id.clone(), &m.root)?;
        }

        let rules = Ruleset::load(&vfs)?;
        Ok(Platform { vfs, rules, load_order })
    }
}

#[derive(Debug)]
pub enum Error {
    Meta(MetaError),
    LoadOrder(LoadOrderError),
    Vfs(VfsError),
    Rule(RuleError),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Meta(e) => write!(f, "{e}"),
            Error::LoadOrder(e) => write!(f, "{e}"),
            Error::Vfs(e) => write!(f, "{e}"),
            Error::Rule(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<MetaError> for Error {
    fn from(e: MetaError) -> Self {
        Error::Meta(e)
    }
}
impl From<LoadOrderError> for Error {
    fn from(e: LoadOrderError) -> Self {
        Error::LoadOrder(e)
    }
}
impl From<VfsError> for Error {
    fn from(e: VfsError) -> Self {
        Error::Vfs(e)
    }
}
impl From<RuleError> for Error {
    fn from(e: RuleError) -> Self {
        Error::Rule(e)
    }
}

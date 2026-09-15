
pub mod core;
pub mod digest;
pub mod effect;
pub mod kingdom;
pub mod merge;
pub mod modmeta;
pub mod package;
pub mod reader;
pub mod ruleset;
pub mod seed;
pub mod troops;
pub mod units;
pub mod value;
pub mod vfs;

pub use core::CORE_LAYER;
pub use digest::{digest, digest_hex, session_digest};
pub use effect::{AssetClaim, EffectReport, Fate, LayerEffect, LayerKind, RuleClaim};
pub use merge::{Deletion, MergeLog, Override};
pub use modmeta::{
    discover, resolve_load_order, Dependency, LoadOrderError, MetaError, ModMeta, Version,
    VersionReq,
};
pub use package::{inspect, inspect_all, ModPackage, PackageError, Warning};
pub use reader::ParseError;
pub use ruleset::{Document, RuleError, Ruleset, RULES_DIR};
pub use troops::{Side, TroopRules};
pub use value::{Origin, Spanned, Table, Value};
pub use vfs::{AssetError, CaseCollision, Layer, Vfs, VfsError};

use std::fmt;
use std::path::{Path, PathBuf};

pub const BASE_LAYER: &str = "base";

#[derive(Debug)]
pub struct Platform {
    pub vfs: Vfs,
    pub rules: Ruleset,
    pub load_order: Vec<ModMeta>,
    pub packages: Vec<ModPackage>,
}

impl Platform {
    pub fn builder() -> PlatformBuilder {
        PlatformBuilder::default()
    }

    pub fn package(&self, id: &str) -> Option<&ModPackage> {
        self.packages.iter().find(|p| p.meta.id == id)
    }

    pub fn troop_table(&self) -> Result<l2_sim::TroopTable, RuleError> {
        units::troop_table(&self.rules)
    }

    pub fn kingdom_tables(&self) -> Result<l2_kingdom::tables::Tables, RuleError> {
        kingdom::tables(&self.rules)
    }

    pub fn digest(&self) -> u64 {
        digest::digest(&self.rules)
    }

    pub fn session_digest(&self) -> u64 {
        digest::session_digest(&self.rules, &self.load_order)
    }

    pub fn effects(&self) -> Vec<LayerEffect> {
        effect::analyse(&self.vfs, &self.rules, &self.load_order)
    }

    pub fn report(&self) -> Report {
        let effects = self.effects();
        let mut added_rules: Vec<(String, String)> = Vec::new();
        let mut inert_layers: Vec<String> = Vec::new();
        for e in &effects {
            if e.kind != effect::LayerKind::Mod {
                continue;
            }
            if e.is_inert() && !e.is_empty() {
                inert_layers.push(e.id.clone());
            }
            for claim in e.rules_added() {
                added_rules.push((claim.path.clone(), e.id.clone()));
            }
        }

        Report {
            shadowed_assets: self
                .vfs
                .shadowed()
                .into_iter()
                .filter(|(n, _)| !package::is_platform_metadata(n))
                .map(|(n, layers)| (n.to_string(), layers.into_iter().map(str::to_string).collect()))
                .collect(),
            added_rules,
            inert_layers,
            float_rules: self
                .rules
                .float_rules()
                .into_iter()
                .map(|(p, o)| (p, o.to_string()))
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
            package_warnings: self
                .packages
                .iter()
                .flat_map(|p| {
                    p.warnings.iter().map(|w| (p.meta.id.clone(), w.to_string()))
                })
                .collect(),
        }
    }

    pub fn effect_report(&self) -> String {
        EffectReport(&self.effects()).to_string()
    }
}

#[derive(Debug, Default, Clone)]
pub struct Report {
    pub shadowed_assets: Vec<(String, Vec<String>)>,
    pub case_collisions: Vec<CaseCollision>,
    pub overrides: Vec<Override>,
    pub deletions: Vec<Deletion>,
    pub dangling_deletes: Vec<(String, String)>,
    pub type_changes: Vec<Override>,
    pub added_rules: Vec<(String, String)>,
    pub inert_layers: Vec<String>,
    pub float_rules: Vec<(String, String)>,
    pub package_warnings: Vec<(String, String)>,
}

impl Report {
    pub fn is_quiet(&self) -> bool {
        self.shadowed_assets.is_empty()
            && self.case_collisions.is_empty()
            && self.overrides.is_empty()
            && self.deletions.is_empty()
            && self.dangling_deletes.is_empty()
            && self.added_rules.is_empty()
            && self.inert_layers.is_empty()
            && self.float_rules.is_empty()
            && self.package_warnings.is_empty()
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
        if !self.added_rules.is_empty() {
            writeln!(f, "rules a mod added rather than overrode (check the spelling):")?;
            for (path, by) in &self.added_rules {
                writeln!(f, "  {path} (set by {by}, defined nowhere below)")?;
            }
        }
        if !self.float_rules.is_empty() {
            writeln!(f, "rules holding a decimal, which the simulation cannot use:")?;
            for (path, at) in &self.float_rules {
                writeln!(f, "  {path} at {at}")?;
            }
        }
        if !self.package_warnings.is_empty() {
            writeln!(f, "mods worth a second look:")?;
            for (id, warning) in &self.package_warnings {
                writeln!(f, "  {id}: {warning}")?;
            }
        }
        for id in &self.inert_layers {
            writeln!(f, "mod '{id}' loaded but changes nothing: everything it sets is overridden")?;
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct PlatformBuilder {
    base: Option<PathBuf>,
    mods_dir: Option<PathBuf>,
    extra: Vec<ModMeta>,
    enabled: Vec<String>,
    core_rules: bool,
}

impl Default for PlatformBuilder {
    fn default() -> Self {
        PlatformBuilder {
            base: None,
            mods_dir: None,
            extra: Vec::new(),
            enabled: Vec::new(),
            core_rules: true,
        }
    }
}

impl PlatformBuilder {
    pub fn base(mut self, dir: impl AsRef<Path>) -> Self {
        self.base = Some(dir.as_ref().to_path_buf());
        self
    }

    pub fn mods_dir(mut self, dir: impl AsRef<Path>) -> Self {
        self.mods_dir = Some(dir.as_ref().to_path_buf());
        self
    }

    pub fn add_mod(mut self, meta: ModMeta) -> Self {
        self.extra.push(meta);
        self
    }

    pub fn enable<I, S>(mut self, ids: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.enabled.extend(ids.into_iter().map(Into::into));
        self
    }

    pub fn core_rules(mut self, on: bool) -> Self {
        self.core_rules = on;
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

        let rules = if self.core_rules {
            Ruleset::load_over_core(&vfs)?
        } else {
            Ruleset::load(&vfs)?
        };

        let packages = load_order
            .iter()
            .filter_map(|m| package::inspect(&m.root).ok())
            .collect();

        Ok(Platform { vfs, rules, load_order, packages })
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

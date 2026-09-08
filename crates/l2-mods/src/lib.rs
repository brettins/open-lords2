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
    /// Each enabled mod inspected on its own, in load order.
    ///
    /// Inspection is deliberately separate from loading, because it answers a
    /// different question. Loading asks what the game will run on once every
    /// layer has had its turn; inspection asks what *this one mod* says, with
    /// nothing else in the picture — which is what produces a per-mod rules
    /// digest and what catches a rule file in the wrong directory.
    ///
    /// Best effort: a mod supplied programmatically through
    /// [`PlatformBuilder::add_mod`] may have no `mod.toml` on disk to inspect,
    /// and that is not a reason to refuse the load. Anything genuinely wrong
    /// with its documents still fails in the merge.
    pub packages: Vec<ModPackage>,
}

impl Platform {
    pub fn builder() -> PlatformBuilder {
        PlatformBuilder::default()
    }

    /// The inspected package for one enabled mod.
    pub fn package(&self, id: &str) -> Option<&ModPackage> {
        self.packages.iter().find(|p| p.meta.id == id)
    }

    /// The combat constants this load produced, ready to hand to
    /// `l2_sim::Battle::with_troops`.
    pub fn troop_table(&self) -> Result<l2_sim::TroopTable, RuleError> {
        units::troop_table(&self.rules)
    }

    /// The kingdom economy this load produced.
    ///
    /// Loaded and validated; see [`kingdom`] for what is not yet wired to it.
    pub fn kingdom_tables(&self) -> Result<l2_kingdom::tables::Tables, RuleError> {
        kingdom::tables(&self.rules)
    }

    /// A checksum over the merged rules, independent of where anything is
    /// installed and of how the load order arrived at them.
    ///
    /// The right answer to "do our rules agree". For the handshake, which asks
    /// a stricter question, use [`Platform::session_digest`].
    pub fn digest(&self) -> u64 {
        digest::digest(&self.rules)
    }

    /// The value for `l2_net::Hello::ruleset_hash`: the merged rules plus the
    /// mod ids in load order.
    ///
    /// Stricter than [`Platform::digest`] on purpose — see
    /// [`digest::session_digest`] for what it catches that the rules alone do
    /// not, and for the one thing neither of them catches yet.
    pub fn session_digest(&self) -> u64 {
        digest::session_digest(&self.rules, &self.load_order)
    }

    /// What each layer contributed and how much of it survived — the "why is
    /// my mod not working" view. See [`effect`].
    pub fn effects(&self) -> Vec<LayerEffect> {
        effect::analyse(&self.vfs, &self.rules, &self.load_order)
    }

    /// Everything worth telling the player after a load: files one mod took
    /// from another, rules one mod took from another, and anything that looks
    /// like a mistake.
    pub fn report(&self) -> Report {
        let effects = self.effects();
        // Rules that overrode nothing, from mod layers only. Every rule the
        // core and base layers set is new by definition, so flagging theirs
        // would bury the one case that matters under thousands that do not.
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
                // A manifest and a rule document are not assets: every mod has
                // a `mod.toml`, and rule documents are merged rather than
                // shadowed. See `package::is_platform_metadata`.
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

    /// The per-mod view: what each layer supplied and how much of it the
    /// engine will actually read.
    ///
    /// This is the one a mod author wants. [`Platform::report`] is organised
    /// by conflict — good for "my two mods are fighting" — and this is
    /// organised by mod, which is the shape of "my mod is not working".
    ///
    /// ```text
    /// 1. core: 401 rule(s) and 0 file(s) in force
    /// 2. base: 1155 rule(s) and 1196 file(s) in force
    /// 3. longbows: 5 rule(s) and 1 file(s) in force
    ///      rule battle.three_bridges.attacker.archers lost to sharpshooters:...
    /// 4. sharpshooters: HAS NO EFFECT - everything it supplies is overridden below
    /// ```
    pub fn effect_report(&self) -> String {
        EffectReport(&self.effects()).to_string()
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
    /// `(rule path, mod id)` for rules a mod set that overrode nothing.
    ///
    /// Not an error: a mod may legitimately introduce a rule. But a misspelt
    /// battle id looks exactly like this and nothing else catches it, so it is
    /// worth a line. `docs/modding.md` §7.3 explains it; the example-mod
    /// corpus test makes the same check by hand.
    pub added_rules: Vec<(String, String)>,
    /// Mods that loaded without error and changed nothing the engine reads.
    pub inert_layers: Vec<String>,
    /// `(rule path, origin)` for rules whose value is a decimal.
    ///
    /// `docs/netcode.md` is categorical that the simulation is integer-only,
    /// so a decimal here cannot reach it and is either dead or a mistake.
    pub float_rules: Vec<(String, String)>,
    /// `(mod id, warning)` from inspecting each enabled mod on its own.
    ///
    /// These are things a merge cannot see, because they are about the shape
    /// of the mod rather than about what it collided with — a rule file
    /// outside `rules/` being the common one, which loads with no error and
    /// does nothing.
    pub package_warnings: Vec<(String, String)>,
}

impl Report {
    /// True when nothing at all was contested.
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

/// Assembles a [`Platform`].
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
            // The engine's own rules are the bottom layer of every real load.
            core_rules: true,
        }
    }
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

    /// Whether to apply the engine's own ruleset underneath everything else.
    ///
    /// On by default, and the only reason to turn it off is to inspect exactly
    /// one install's documents in isolation — a diagnostic, not a game. A
    /// running game without the core rules has no combat constants and no
    /// economy.
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

        // Inspect each mod on its own, in load order. Best effort: a mod
        // handed in through `add_mod` need not have a manifest on disk, and a
        // document that is genuinely broken has already failed the merge
        // above, so nothing is lost by skipping a mod that cannot be read
        // here.
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

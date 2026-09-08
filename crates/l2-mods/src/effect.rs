//! What each mod actually did — the answer to "why is my mod not working?"
//!
//! [`crate::Report`] answers it from the *conflict's* side: this path was
//! overridden, this file was shadowed. That is the right shape for "the two
//! mods I installed are fighting", and the wrong shape for the question mod
//! authors actually ask, which is about one mod and starts from the
//! assumption that it should have worked.
//!
//! A rule a mod wrote can fail to reach the game in three quite different
//! ways, and telling them apart is the whole value here:
//!
//! 1. **It lost.** A later mod in the load order set the same path. The fix is
//!    load order, and the report names the mod that won.
//! 2. **It landed on nothing.** The path existed nowhere below, so the merge
//!    added it and the engine reads it from a name nothing looks up. Almost
//!    always a typo — `battle.three_brdiges` — or a mod written against a
//!    version of another mod that has since renamed something. This one is
//!    invisible without provenance, which is why the value tree carries it.
//! 3. **It was deleted.** A later `"$delete"` removed the table it was in.
//!
//! Case 2 is the one worth the machinery. `docs/modding.md` §7 already uses it
//! as a corpus check — the example mod must *override* every leaf it sets,
//! never add one — and this generalises that check to every mod at runtime.
//!
//! Assets are simpler, because assets shadow: a mod's file either wins or is
//! covered by a later layer's file of the same name.

use crate::modmeta::ModMeta;
use crate::ruleset::{Ruleset, RULES_DIR};
use crate::vfs::Vfs;
use std::fmt;

/// A path the layer set, and what happened to it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleClaim {
    pub path: String,
    pub fate: Fate,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Fate {
    /// This layer's value is the one the engine reads, and it replaced a value
    /// from the named source.
    Overrode(String),
    /// This layer's value is the one the engine reads, and nothing below
    /// defined the path. A new rule if that was intended, a typo if it was
    /// not — the platform cannot tell, and says so rather than guessing.
    Added,
    /// A later source set the same path and won.
    LostTo(String),
    /// A later `"$delete"` removed it.
    Deleted(String),
}

impl fmt::Display for Fate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Fate::Overrode(prev) => write!(f, "overrode {prev}"),
            Fate::Added => write!(f, "added; nothing below defined it"),
            Fate::LostTo(who) => write!(f, "lost to {who}"),
            Fate::Deleted(by) => write!(f, "deleted by {by}"),
        }
    }
}

/// A file the layer provided, and what happened to it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetClaim {
    /// Normalised name, as the overlay keys it.
    pub name: String,
    /// `None` when this layer's copy is the one that resolves; otherwise the
    /// later layer whose copy does.
    pub shadowed_by: Option<String>,
}

/// Everything one layer contributed, and how much of it survived.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayerEffect {
    pub id: String,
    /// Position in the resolved load order. The base install and the core
    /// ruleset are layers too and come first.
    pub position: usize,
    /// Rule documents this layer supplied, in the order they applied.
    pub documents: Vec<String>,
    /// Every rule leaf this layer set, sorted by path.
    pub rules: Vec<RuleClaim>,
    /// Every file this layer provided, sorted, excluding rule documents.
    pub assets: Vec<AssetClaim>,
}

impl LayerEffect {
    pub fn rules_winning(&self) -> usize {
        self.rules
            .iter()
            .filter(|r| matches!(r.fate, Fate::Overrode(_) | Fate::Added))
            .count()
    }

    pub fn rules_lost(&self) -> Vec<&RuleClaim> {
        self.rules
            .iter()
            .filter(|r| matches!(r.fate, Fate::LostTo(_) | Fate::Deleted(_)))
            .collect()
    }

    /// Rules that overrode nothing. Not an error, and not necessarily wrong —
    /// but every typo in a mod file looks exactly like this.
    pub fn rules_added(&self) -> Vec<&RuleClaim> {
        self.rules.iter().filter(|r| r.fate == Fate::Added).collect()
    }

    pub fn assets_winning(&self) -> usize {
        self.assets.iter().filter(|a| a.shadowed_by.is_none()).count()
    }

    /// True when this layer changed nothing the engine will read.
    ///
    /// A mod that provides only assets already shadowed and only rules already
    /// overridden is enabled, loaded, reported without error, and completely
    /// inert. That state is silent in every mod manager the author has used,
    /// and it is the single most common "why is my mod not working".
    pub fn is_inert(&self) -> bool {
        self.rules_winning() == 0 && self.assets_winning() == 0
    }

    /// True when this layer supplied nothing at all — an empty directory with
    /// a `mod.toml`, or a mod whose files are all in the wrong place.
    pub fn is_empty(&self) -> bool {
        self.rules.is_empty() && self.assets.is_empty()
    }
}

/// Work out what every layer contributed.
///
/// `mods` is the resolved load order, which the VFS layers agree with by
/// construction: [`crate::PlatformBuilder`] mounts the base install and then
/// each mod in that order.
pub fn analyse(vfs: &Vfs, rules: &Ruleset, mods: &[ModMeta]) -> Vec<LayerEffect> {
    let mut out = Vec::new();

    // The core ruleset is not a VFS layer — it is compiled in — so it is
    // accounted for first and separately, and only for rules.
    let core = crate::core::CORE_LAYER;
    if rules.documents.iter().any(|d| layer_of(&d.source) == core) {
        out.push(layer_effect(core, 0, None, rules));
    }

    for (i, layer) in vfs.layers().iter().enumerate() {
        let position = out.len();
        out.push(layer_effect(&layer.id, position, Some((vfs, i)), rules));
    }

    // A mod that resolved into the load order but mounted no layer cannot
    // happen today; if it ever does, saying so beats leaving it out.
    for m in mods {
        if !out.iter().any(|e| e.id == m.id) {
            let position = out.len();
            out.push(LayerEffect {
                id: m.id.clone(),
                position,
                documents: Vec::new(),
                rules: Vec::new(),
                assets: Vec::new(),
            });
        }
    }
    out
}

fn layer_of(source: &str) -> &str {
    source.split_once(':').map(|(l, _)| l).unwrap_or(source)
}

fn layer_effect(
    id: &str,
    position: usize,
    vfs_layer: Option<(&Vfs, usize)>,
    rules: &Ruleset,
) -> LayerEffect {
    let mut documents = Vec::new();
    let mut claims: Vec<RuleClaim> = Vec::new();

    for doc in &rules.documents {
        if layer_of(&doc.source) != id {
            continue;
        }
        documents.push(doc.source.clone());
        for path in &doc.leaves {
            claims.push(RuleClaim { path: path.clone(), fate: fate_of(rules, &doc.source, path) });
        }
    }
    // Two documents in one layer can both claim a path; the later one is the
    // layer's answer. Sorting by path then keeping the last claim per path
    // gives that, deterministically.
    claims.sort_by(|a, b| a.path.cmp(&b.path));
    claims.dedup_by(|later, earlier| {
        if later.path == earlier.path {
            *earlier = later.clone();
            true
        } else {
            false
        }
    });

    let mut assets = Vec::new();
    if let Some((vfs, index)) = vfs_layer {
        let rules_prefix = format!("{RULES_DIR}/");
        for name in vfs.layer_entries(index) {
            // Rule documents are not assets. They do not shadow — every
            // layer's copy is read and merged — so listing them as shadowed
            // would be exactly backwards.
            if name.starts_with(&rules_prefix) && name.ends_with(".toml") {
                continue;
            }
            let winner = vfs.providers(name).last().map(|p| p.layer);
            assets.push(AssetClaim {
                name: name.to_string(),
                shadowed_by: match winner {
                    Some(w) if w != index => Some(vfs.layer_id(w).to_string()),
                    _ => None,
                },
            });
        }
    }

    LayerEffect { id: id.to_string(), position, documents, rules: claims, assets }
}

fn fate_of(rules: &Ruleset, source: &str, path: &str) -> Fate {
    // Did something later delete it outright?
    if let Some(d) = rules.log.deletions.iter().find(|d| d.path == path) {
        if d.removed.source.as_ref() == source {
            return Fate::Deleted(d.by.to_string());
        }
    }
    match rules.origin(path) {
        // Gone from the tree entirely: a `"$delete"` took the table it was in.
        None => Fate::Deleted(String::from("a later \"$delete\"")),
        Some(origin) if origin.source.as_ref() != source => Fate::LostTo(origin.to_string()),
        Some(_) => {
            // The winner is this document. Did it replace anything?
            match rules
                .log
                .overrides
                .iter()
                .rev()
                .find(|o| o.path == path && o.current.source.as_ref() == source)
            {
                Some(o) => Fate::Overrode(o.previous.to_string()),
                None => Fate::Added,
            }
        }
    }
}

/// The human-readable form: one paragraph per layer, worst news first.
pub struct EffectReport<'a>(pub &'a [LayerEffect]);

impl fmt::Display for EffectReport<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for e in self.0 {
            write!(f, "{}. {}", e.position + 1, e.id)?;
            if e.is_empty() {
                writeln!(f, ": supplied nothing (no rule documents, no files)")?;
                continue;
            }
            if e.is_inert() {
                writeln!(f, ": HAS NO EFFECT - everything it supplies is overridden below")?;
            } else {
                writeln!(
                    f,
                    ": {} rule(s) and {} file(s) in force",
                    e.rules_winning(),
                    e.assets_winning()
                )?;
            }
            for claim in e.rules_lost() {
                writeln!(f, "     rule {} {}", claim.path, claim.fate)?;
            }
            for claim in e.rules_added() {
                writeln!(
                    f,
                    "     rule {} was added, not overridden - check the spelling",
                    claim.path
                )?;
            }
            for asset in &e.assets {
                if let Some(who) = &asset.shadowed_by {
                    writeln!(f, "     file {} is shadowed by {who}", asset.name)?;
                }
            }
        }
        Ok(())
    }
}

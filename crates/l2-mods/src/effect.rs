
use crate::modmeta::ModMeta;
use crate::ruleset::Ruleset;
use crate::vfs::Vfs;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleClaim {
    pub path: String,
    pub fate: Fate,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Fate {
    Overrode(String),
    Added,
    LostTo(String),
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetClaim {
    pub name: String,
    pub shadowed_by: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayerKind {
    Core,
    Base,
    Mod,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayerEffect {
    pub id: String,
    pub kind: LayerKind,
    pub position: usize,
    pub documents: Vec<String>,
    pub rules: Vec<RuleClaim>,
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

    pub fn rules_added(&self) -> Vec<&RuleClaim> {
        self.rules.iter().filter(|r| r.fate == Fate::Added).collect()
    }

    pub fn assets_winning(&self) -> usize {
        self.assets.iter().filter(|a| a.shadowed_by.is_none()).count()
    }

    pub fn is_inert(&self) -> bool {
        self.rules_winning() == 0 && self.assets_winning() == 0
    }

    pub fn is_empty(&self) -> bool {
        self.rules.is_empty() && self.assets.is_empty()
    }
}

pub fn analyse(vfs: &Vfs, rules: &Ruleset, mods: &[ModMeta]) -> Vec<LayerEffect> {
    let mut out = Vec::new();

    let core = crate::core::CORE_LAYER;
    if rules.documents.iter().any(|d| layer_of(&d.source) == core) {
        out.push(layer_effect(core, LayerKind::Core, 0, None, rules));
    }

    for (i, layer) in vfs.layers().iter().enumerate() {
        let position = out.len();
        let kind = if layer.id == crate::BASE_LAYER { LayerKind::Base } else { LayerKind::Mod };
        out.push(layer_effect(&layer.id, kind, position, Some((vfs, i)), rules));
    }

    for m in mods {
        if !out.iter().any(|e| e.id == m.id) {
            let position = out.len();
            out.push(LayerEffect {
                id: m.id.clone(),
                kind: LayerKind::Mod,
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
    kind: LayerKind,
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
        for name in vfs.layer_entries(index) {
            if crate::package::is_platform_metadata(name) {
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

    LayerEffect { id: id.to_string(), kind, position, documents, rules: claims, assets }
}

fn fate_of(rules: &Ruleset, source: &str, path: &str) -> Fate {
    if let Some(d) = rules.log.deletions.iter().find(|d| d.path == path) {
        if d.removed.source.as_ref() == source {
            return Fate::Deleted(d.by.to_string());
        }
    }
    match rules.origin(path) {
        None => Fate::Deleted(String::from("a later \"$delete\"")),
        Some(origin) if origin.source.as_ref() != source => Fate::LostTo(origin.to_string()),
        Some(_) => {
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
            if e.kind == LayerKind::Mod {
                for claim in e.rules_added() {
                    writeln!(
                        f,
                        "     rule {} was added, not overridden - check the spelling",
                        claim.path
                    )?;
                }
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


use std::io;
use std::path::Path;

pub const UNITS_TOML: &str = include_str!("../rulesets/core/rules/units.toml");

pub const KINGDOM_TOML: &str = include_str!("../rulesets/core/rules/kingdom.toml");

pub const CORE_LAYER: &str = "core";

pub const DOCUMENTS: [(&str, &str); 2] =
    [("core:rules/kingdom.toml", KINGDOM_TOML), ("core:rules/units.toml", UNITS_TOML)];

pub fn write_to(dir: &Path) -> io::Result<Vec<std::path::PathBuf>> {
    let rules = dir.join("rules");
    std::fs::create_dir_all(&rules)?;
    let mut written = Vec::new();
    for (source, text) in DOCUMENTS {
        let name = source.rsplit('/').next().unwrap_or(source);
        let path = rules.join(name);
        std::fs::write(&path, text)?;
        written.push(path);
    }
    Ok(written)
}

pub fn render() -> [(&'static str, String); 2] {
    [
        ("kingdom.toml", crate::kingdom::render_toml(&l2_kingdom::tables::Tables::DEFAULT)),
        ("units.toml", crate::units::render_toml(&l2_sim::TroopTable::DEFAULT)),
    ]
}

pub fn source_dir() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("rulesets").join("core").join("rules")
}

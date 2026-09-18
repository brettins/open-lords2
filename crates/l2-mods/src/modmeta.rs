
use crate::reader::{self, ParseError};
use crate::value::{Table, Value};
use std::fmt;
use std::path::{Path, PathBuf};

pub const MANIFEST: &str = "mod.toml";


#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct Version {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl Version {
    pub fn new(major: u32, minor: u32, patch: u32) -> Self {
        Version { major, minor, patch }
    }

    pub fn parse(s: &str) -> Result<Version, String> {
        let mut parts = s.trim().split('.');
        let mut next = |what: &str| -> Result<u32, String> {
            match parts.next() {
                None => Ok(0),
                Some(p) => p
                    .trim()
                    .parse::<u32>()
                    .map_err(|_| format!("'{s}' is not a version: {what} component '{p}'")),
            }
        };
        let major = next("major")?;
        let minor = next("minor")?;
        let patch = next("patch")?;
        if parts.next().is_some() {
            return Err(format!("'{s}' is not a version: too many components"));
        }
        Ok(Version { major, minor, patch })
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VersionReq {
    Any,
    AtLeast(Version),
    Exactly(Version),
    Compatible(Version),
}

impl VersionReq {
    pub fn matches(self, v: Version) -> bool {
        match self {
            VersionReq::Any => true,
            VersionReq::AtLeast(min) => v >= min,
            VersionReq::Exactly(want) => v == want,
            VersionReq::Compatible(min) => {
                if v < min {
                    return false;
                }
                if min.major > 0 {
                    v.major == min.major
                } else {
                    v.major == 0 && v.minor == min.minor
                }
            }
        }
    }
}

impl fmt::Display for VersionReq {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VersionReq::Any => write!(f, "any version"),
            VersionReq::AtLeast(v) => write!(f, ">= {v}"),
            VersionReq::Exactly(v) => write!(f, "= {v}"),
            VersionReq::Compatible(v) => write!(f, "^{v}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dependency {
    pub id: String,
    pub req: VersionReq,
}

impl Dependency {
    pub fn parse(spec: &str) -> Result<Dependency, String> {
        let spec = spec.trim();
        let split = spec
            .find(|c: char| c.is_whitespace() || c == '>' || c == '=' || c == '^')
            .unwrap_or(spec.len());
        let (id, rest) = spec.split_at(split);
        let id = id.trim();
        if id.is_empty() {
            return Err(format!("'{spec}' names no mod"));
        }
        let rest = rest.trim();
        let req = if rest.is_empty() {
            VersionReq::Any
        } else if let Some(v) = rest.strip_prefix(">=") {
            VersionReq::AtLeast(Version::parse(v)?)
        } else if let Some(v) = rest.strip_prefix('^') {
            VersionReq::Compatible(Version::parse(v)?)
        } else if let Some(v) = rest.strip_prefix('=') {
            VersionReq::Exactly(Version::parse(v)?)
        } else {
            return Err(format!(
                "'{spec}': expected a version requirement like '>= 1.2', '^1.2' or '= 1.2'"
            ));
        };
        Ok(Dependency { id: id.to_string(), req })
    }
}


#[derive(Debug, Clone)]
pub struct ModMeta {
    pub id: String,
    pub name: String,
    pub version: Version,
    pub author: Option<String>,
    pub description: Option<String>,
    pub requires: Vec<Dependency>,
    pub after: Vec<String>,
    pub conflicts: Vec<String>,
    pub root: PathBuf,
}

#[derive(Debug)]
pub enum MetaError {
    Io { path: PathBuf, source: std::io::Error },
    Syntax(ParseError),
    Field { path: PathBuf, message: String },
}

impl fmt::Display for MetaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MetaError::Io { path, source } => write!(f, "{}: {source}", path.display()),
            MetaError::Syntax(e) => write!(f, "{e}"),
            MetaError::Field { path, message } => write!(f, "{}: {message}", path.display()),
        }
    }
}

impl std::error::Error for MetaError {}

impl ModMeta {
    pub fn load(dir: &Path) -> Result<ModMeta, MetaError> {
        let path = dir.join(MANIFEST);
        let text = std::fs::read_to_string(&path)
            .map_err(|e| MetaError::Io { path: path.clone(), source: e })?;
        ModMeta::from_str(&text, dir)
    }

    pub fn from_str(text: &str, dir: &Path) -> Result<ModMeta, MetaError> {
        let source = dir.join(MANIFEST);
        let doc = reader::parse(text, &source.to_string_lossy()).map_err(MetaError::Syntax)?;
        let bad = |m: String| MetaError::Field { path: source.clone(), message: m };

        let table = doc
            .value
            .get("mod")
            .and_then(|v| v.value.as_table())
            .ok_or_else(|| bad("no [mod] table".into()))?;

        let id = string_field(table, "id")
            .ok_or_else(|| bad("[mod] has no 'id'".into()))?
            .to_string();
        if !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') || id.is_empty() {
            return Err(bad(format!(
                "mod id '{id}' must be non-empty and only letters, digits, '-' and '_'"
            )));
        }

        let version = match string_field(table, "version") {
            None => Version::default(),
            Some(v) => Version::parse(v).map_err(bad)?,
        };

        let mut requires = Vec::new();
        for spec in string_list(table, "requires") {
            requires.push(Dependency::parse(&spec).map_err(bad)?);
        }

        Ok(ModMeta {
            name: string_field(table, "name").unwrap_or(&id).to_string(),
            id,
            version,
            author: string_field(table, "author").map(str::to_string),
            description: string_field(table, "description").map(str::to_string),
            requires,
            after: string_list(table, "after"),
            conflicts: string_list(table, "conflicts"),
            root: dir.to_path_buf(),
        })
    }
}

fn string_field<'t>(t: &'t Table, key: &str) -> Option<&'t str> {
    t.get(key).and_then(|v| v.value.as_str())
}

fn string_list(t: &Table, key: &str) -> Vec<String> {
    match t.get(key).map(|v| &v.value) {
        Some(Value::Array(items)) => {
            items.iter().filter_map(|i| i.value.as_str()).map(str::to_string).collect()
        }
        Some(Value::String(s)) => vec![s.clone()],
        _ => Vec::new(),
    }
}


pub fn discover(dir: &Path) -> Result<Vec<ModMeta>, MetaError> {
    let mut found = Vec::new();
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) => return Err(MetaError::Io { path: dir.to_path_buf(), source: e }),
    };
    let mut dirs: Vec<PathBuf> =
        entries.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect();
    dirs.sort();
    for d in dirs {
        if d.join(MANIFEST).is_file() {
            found.push(ModMeta::load(&d)?);
        }
    }
    found.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(found)
}


#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoadOrderError {
    Unknown(String),
    Duplicate(String),
    MissingDependency { dependent: String, needs: String },
    VersionMismatch { dependent: String, needs: String, want: VersionReq, found: Version },
    Conflict { a: String, b: String },
    Cycle(Vec<String>),
}

impl fmt::Display for LoadOrderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LoadOrderError::Unknown(id) => write!(f, "no mod with id '{id}' was found"),
            LoadOrderError::Duplicate(id) => write!(f, "mod id '{id}' appears more than once"),
            LoadOrderError::MissingDependency { dependent, needs } => {
                write!(f, "'{dependent}' requires '{needs}', which is not enabled")
            }
            LoadOrderError::VersionMismatch { dependent, needs, want, found } => write!(
                f,
                "'{dependent}' requires '{needs}' {want}, but {needs} {found} is enabled"
            ),
            LoadOrderError::Conflict { a, b } => {
                write!(f, "'{a}' declares a conflict with '{b}'; enable only one")
            }
            LoadOrderError::Cycle(ids) => {
                write!(f, "dependency cycle: {}", ids.join(" -> "))
            }
        }
    }
}

impl std::error::Error for LoadOrderError {}

pub fn resolve_load_order(
    available: &[ModMeta],
    enabled: &[String],
) -> Result<Vec<ModMeta>, LoadOrderError> {
    let mut chosen: Vec<&ModMeta> = Vec::with_capacity(enabled.len());
    for id in enabled {
        if chosen.iter().any(|m| &m.id == id) {
            return Err(LoadOrderError::Duplicate(id.clone()));
        }
        match available.iter().find(|m| &m.id == id) {
            Some(m) => chosen.push(m),
            None => return Err(LoadOrderError::Unknown(id.clone())),
        }
    }

    let pos = |id: &str| chosen.iter().position(|m| m.id == id);

    for m in &chosen {
        for c in &m.conflicts {
            if pos(c).is_some() {
                return Err(LoadOrderError::Conflict { a: m.id.clone(), b: c.clone() });
            }
        }
        for dep in &m.requires {
            match pos(&dep.id) {
                None => {
                    return Err(LoadOrderError::MissingDependency {
                        dependent: m.id.clone(),
                        needs: dep.id.clone(),
                    })
                }
                Some(i) => {
                    let found = chosen[i].version;
                    if !dep.req.matches(found) {
                        return Err(LoadOrderError::VersionMismatch {
                            dependent: m.id.clone(),
                            needs: dep.id.clone(),
                            want: dep.req,
                            found,
                        });
                    }
                }
            }
        }
    }

    let n = chosen.len();
    let mut edges: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut indegree = vec![0usize; n];
    let add = |from: usize, to: usize, edges: &mut Vec<Vec<usize>>, indeg: &mut Vec<usize>| {
        if from != to && !edges[from].contains(&to) {
            edges[from].push(to);
            indeg[to] += 1;
        }
    };
    for (i, m) in chosen.iter().enumerate() {
        for dep in &m.requires {
            if let Some(j) = pos(&dep.id) {
                add(j, i, &mut edges, &mut indegree);
            }
        }
        for a in &m.after {
            if let Some(j) = pos(a) {
                add(j, i, &mut edges, &mut indegree);
            }
        }
    }

    let mut out: Vec<usize> = Vec::with_capacity(n);
    let mut done = vec![false; n];
    for _ in 0..n {
        let Some(next) = (0..n).find(|&i| !done[i] && indegree[i] == 0) else {
            return Err(LoadOrderError::Cycle(find_cycle(&chosen, &edges, &done)));
        };
        done[next] = true;
        out.push(next);
        for &t in &edges[next] {
            indegree[t] -= 1;
        }
    }

    Ok(out.into_iter().map(|i| chosen[i].clone()).collect())
}

fn find_cycle(chosen: &[&ModMeta], edges: &[Vec<usize>], done: &[bool]) -> Vec<String> {
    let start = match (0..chosen.len()).find(|&i| !done[i]) {
        Some(i) => i,
        None => return Vec::new(),
    };
    let mut path = vec![start];
    let mut seen = vec![false; chosen.len()];
    seen[start] = true;
    let mut here = start;
    while let Some(&next) = edges[here].iter().find(|&&t| !done[t]) {
        if seen[next] {
            let at = path.iter().position(|&p| p == next).unwrap_or(0);
            let mut ids: Vec<String> = path[at..].iter().map(|&i| chosen[i].id.clone()).collect();
            ids.push(chosen[next].id.clone());
            return ids;
        }
        seen[next] = true;
        path.push(next);
        here = next;
    }
    path.iter().map(|&i| chosen[i].id.clone()).collect()
}

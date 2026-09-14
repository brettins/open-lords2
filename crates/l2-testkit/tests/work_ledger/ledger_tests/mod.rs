#![allow(unused_imports)]

mod schema;
pub use schema::*;
mod view;
pub use view::*;

use super::*;

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// Git in a scratch repository, isolated from this machine's configuration so
/// a developer's global hooks, signing or aliases cannot change what the
/// fixture is.
fn scratch_git(dir: &Path, empty_config: &Path, args: &[&str]) -> Option<Output> {
    Command::new("git")
        .args(["-c", "user.name=l2 fixture", "-c", "user.email=fixture@invalid", "-c", "core.autocrlf=false"])
        .args(args)
        .current_dir(dir)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", empty_config)
        .output()
        .ok()
        .filter(|o| o.status.success())
}


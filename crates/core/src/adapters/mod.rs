//! Concrete adapters. Each one encodes where an app family installs, how its copies are
//! isolated, and how its main process is recognised.

pub mod chromium;
pub mod claude;
pub mod custom;
pub mod electron;
pub mod webapp;

use std::path::{Path, PathBuf};

use crate::env::{Env, Os};
use crate::model::{InstallSource, Installation};

/// Builds a detected installation with the common fields filled in.
pub(crate) fn detected(id: &str, adapter: &str, name: &str, exe: PathBuf) -> Installation {
    Installation {
        id: id.to_string(),
        adapter: adapter.to_string(),
        name: name.to_string(),
        exe,
        version: None,
        source: InstallSource::Detected,
        options: Default::default(),
        present: true,
        hidden: false,
    }
}

/// A Windows app-execution alias (`%LOCALAPPDATA%\Microsoft\WindowsApps\x.exe`) is a reparse
/// point that `is_file()` cannot follow; existence of the link itself is what counts.
pub(crate) fn alias_exists(p: &Path) -> bool {
    std::fs::symlink_metadata(p).is_ok()
}

pub(crate) fn file_name_lower(p: &Path) -> String {
    p.file_name().map(|s| s.to_string_lossy().to_lowercase()).unwrap_or_default()
}

pub(crate) fn opt_str<'a>(install: &'a Installation, key: &str) -> Option<&'a str> {
    install.options.get(key).and_then(|v| v.as_str())
}

pub(crate) fn exe_names(env: &Env, names: &[&str]) -> Vec<String> {
    names
        .iter()
        .map(|n| if env.os == Os::Windows { format!("{n}.exe") } else { n.to_string() })
        .collect()
}

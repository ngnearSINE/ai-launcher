//! Shared pieces for usage read through a command-line sign-in on this computer (Codex, Grok
//! CLI, Antigravity). Unlike the app readers these are not tied to a profile folder: the user
//! links one CLI sign-in to an instance by hand, and the launcher only reads it.
//!
//! - Nothing here writes to a CLI's files or refreshes its tokens.
//! - Programs are started without a console window, with no input, and are killed when the
//!   read is done or times out.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use serde::Serialize;

/// The sign-in a CLI holds, shown when the user picks it as an instance's usage source.
#[derive(Debug, Clone, Default, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CliAccount {
    /// Account email when the CLI keeps one locally (empty otherwise).
    pub email: String,
    /// Plan label (`Pro`, `Plus`…) when known locally.
    pub plan: String,
}

/// The user's home folder.
pub(crate) fn home() -> Option<PathBuf> {
    let var = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
    std::env::var_os(var).filter(|v| !v.is_empty()).map(PathBuf::from)
}

/// `%LOCALAPPDATA%` on Windows.
pub(crate) fn local_app_data() -> Option<PathBuf> {
    std::env::var_os("LOCALAPPDATA").filter(|v| !v.is_empty()).map(PathBuf::from)
}

/// A folder from an environment variable, else `default` under the home folder.
pub(crate) fn dir_from_env(var: &str, default: &str) -> Option<PathBuf> {
    std::env::var_os(var).filter(|v| !v.is_empty()).map(PathBuf::from).or_else(|| home().map(|h| h.join(default)))
}

/// File name of a program on this OS (`codex.exe` on Windows). Only real executables count:
/// npm's `.cmd` shims would need a console to run.
pub(crate) fn exe_name(name: &str) -> String {
    if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_string()
    }
}

/// Every `name` on `PATH`.
pub(crate) fn on_path(name: &str) -> Vec<PathBuf> {
    let file = exe_name(name);
    std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).map(|d| d.join(&file)).filter(|p| p.is_file()).collect())
        .unwrap_or_default()
}

fn modified(p: &Path) -> SystemTime {
    std::fs::metadata(p).and_then(|m| m.modified()).unwrap_or(SystemTime::UNIX_EPOCH)
}

/// The most recently updated of `candidates` that exists.
pub(crate) fn newest(candidates: impl IntoIterator<Item = PathBuf>) -> Option<PathBuf> {
    candidates.into_iter().filter(|p| p.is_file()).max_by_key(|p| modified(p))
}

/// Children of `dir` (empty when it cannot be read).
pub(crate) fn children(dir: &Path) -> Vec<PathBuf> {
    std::fs::read_dir(dir).map(|r| r.flatten().map(|e| e.path()).collect()).unwrap_or_default()
}

/// A program started quietly: no console window, no input, killed if the read is dropped.
pub(crate) fn command(exe: &Path) -> tokio::process::Command {
    let mut c = tokio::process::Command::new(exe);
    c.stdin(std::process::Stdio::null()).stderr(std::process::Stdio::null()).kill_on_drop(true);
    #[cfg(windows)]
    {
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        c.creation_flags(CREATE_NO_WINDOW);
    }
    c
}

/// The JSON object in a program's output, skipping any notice printed around it.
pub(crate) fn json_in(out: &[u8]) -> Option<serde_json::Value> {
    let s = String::from_utf8_lossy(out);
    let (a, b) = (s.find('{')?, s.rfind('}')?);
    serde_json::from_str(s.get(a..=b)?).ok()
}

/// `pro`, `plus`, `free_trial` → `Pro`, `Plus`, `Free trial`.
pub(crate) fn plan_label(raw: &str) -> String {
    crate::cursor::plan_label(raw)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_json_between_notices() {
        let v = json_in(b"update available\n{\"a\":1}\n").unwrap();
        assert_eq!(v["a"], 1);
        assert!(json_in(b"no json here").is_none());
    }
}

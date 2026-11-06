use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::settings::Tristate;

/// One installed copy of an app the launcher can start (a Claude Desktop install,
/// a Chrome install, a web app served through a browser, or a program the user pointed at).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Installation {
    /// Stable id: `<adapter>:<discriminator>` for detected installs, a uuid for manual ones.
    pub id: String,
    /// Adapter kind that knows how to run it (`claude`, `chromium`, `electron`, `webapp`, `custom`).
    pub adapter: String,
    pub name: String,
    /// Executable (or app-execution alias) that is started.
    pub exe: PathBuf,
    #[serde(default)]
    pub version: Option<String>,
    pub source: InstallSource,
    /// Adapter-specific settings, e.g. `{ "browser": "<installation id>", "url": "…" }` for web apps,
    /// `{ "dataDirName": "Orca" }` for Electron apps.
    #[serde(default)]
    pub options: serde_json::Map<String, serde_json::Value>,
    /// False once a detected install disappears; kept so instances are not orphaned silently.
    #[serde(default = "yes")]
    pub present: bool,
    #[serde(default)]
    pub hidden: bool,
}

fn yes() -> bool {
    true
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum InstallSource {
    Detected,
    Manual,
}

/// One account copy of an installation: its own profile folder, name and look.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Instance {
    pub id: String,
    pub installation_id: String,
    pub alias: String,
    /// Hex colour used for the card, the floating launcher and the taskbar badge.
    pub color: String,
    /// Short label (1–2 characters or an emoji) drawn on the avatar.
    #[serde(default)]
    pub glyph: Option<String>,
    pub profile: Profile,
    #[serde(default)]
    pub group_id: Option<String>,
    #[serde(default)]
    pub note: String,
    #[serde(default)]
    pub launch: LaunchOptions,
    #[serde(default)]
    pub advanced: InstanceAdvanced,
    #[serde(default)]
    pub order: i64,
    #[serde(default)]
    pub favorite: bool,
    pub created_at: DateTime<Utc>,
    #[serde(default)]
    pub last_launched_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub launch_count: u64,
}

/// Where an instance keeps its data.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Profile {
    /// The app's own default data folder — the copy the app runs when started normally.
    Default,
    /// A separate data folder the launcher passes to the app.
    Dir {
        path: PathBuf,
        /// True when the launcher created the folder (and may offer to remove it).
        #[serde(default)]
        managed: bool,
    },
}

impl Profile {
    pub fn dir(&self) -> Option<&PathBuf> {
        match self {
            Profile::Default => None,
            Profile::Dir { path, .. } => Some(path),
        }
    }
    pub fn is_default(&self) -> bool {
        matches!(self, Profile::Default)
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LaunchOptions {
    /// Extra command-line arguments appended after the adapter's own.
    #[serde(default)]
    pub args: Vec<String>,
    /// Extra environment variables (`KEY`, `VALUE`).
    #[serde(default)]
    pub env: Vec<(String, String)>,
    /// URL for web-app instances (overrides the installation default).
    #[serde(default)]
    pub url: Option<String>,
    /// Start this instance when the launcher starts with the OS.
    #[serde(default)]
    pub autostart: bool,
    /// Ask the app to start minimised where the app supports it.
    #[serde(default)]
    pub start_minimized: bool,
}

/// Per-instance overrides of the global advanced settings. `Inherit` means "use the global value".
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct InstanceAdvanced {
    #[serde(default)]
    pub taskbar_separate: Tristate,
    #[serde(default)]
    pub taskbar_badge: Tristate,
    #[serde(default)]
    pub relaunch_through_launcher: Tristate,
    #[serde(default)]
    pub usage_enabled: Tristate,
    /// Which organisations of the signed-in account to show (empty = all).
    #[serde(default)]
    pub usage_orgs: Vec<String>,
    /// Hide from the floating launcher / tray list without deleting.
    #[serde(default)]
    pub hidden_in_quick: bool,
}

impl Default for InstanceAdvanced {
    fn default() -> Self {
        Self {
            taskbar_separate: Tristate::Inherit,
            taskbar_badge: Tristate::Inherit,
            relaunch_through_launcher: Tristate::Inherit,
            usage_enabled: Tristate::Inherit,
            usage_orgs: Vec::new(),
            hidden_in_quick: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Group {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub order: i64,
}

/// Palette offered when creating an instance; picked round-robin so new cards differ.
pub const PALETTE: &[&str] = &[
    "#0A84FF", "#FF9F0A", "#30D158", "#BF5AF2", "#FF375F", "#64D2FF", "#FFD60A", "#AC8E68", "#5E5CE6",
    "#FF6B3D",
];

pub fn new_id() -> String {
    uuid::Uuid::new_v4().simple().to_string()[..12].to_string()
}

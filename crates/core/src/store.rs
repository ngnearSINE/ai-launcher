use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::model::{Group, Installation, Instance};
use crate::settings::Settings;

pub const SCHEMA_VERSION: u32 = 1;

/// The single JSON document that holds the user's catalog.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct StoreFile {
    pub version: u32,
    pub installations: Vec<Installation>,
    pub instances: Vec<Instance>,
    pub groups: Vec<Group>,
    pub settings: Settings,
    /// Profile folders already offered as instances; removing such an instance does not
    /// bring it back on the next scan.
    pub imported_profiles: Vec<String>,
    /// Detected installations (and web-app presets) the user removed from the launcher.
    pub dismissed: Vec<String>,
}

impl Default for StoreFile {
    fn default() -> Self {
        Self {
            version: SCHEMA_VERSION,
            installations: Vec::new(),
            instances: Vec::new(),
            groups: Vec::new(),
            settings: Settings::default(),
            imported_profiles: Vec::new(),
            dismissed: Vec::new(),
        }
    }
}

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
}

pub struct Store {
    pub path: PathBuf,
    pub data: StoreFile,
}

impl Store {
    /// Loads the catalog. A file that cannot be parsed is kept aside as `*.broken-<time>.json`
    /// and an empty catalog is used, so a bad edit never blocks the launcher from starting.
    pub fn load(path: &Path) -> Result<Self, StoreError> {
        let data = match std::fs::read(path) {
            Ok(bytes) => match serde_json::from_slice::<StoreFile>(&bytes) {
                Ok(mut d) => {
                    d.settings.sanitize();
                    d.version = SCHEMA_VERSION;
                    d
                }
                Err(e) => {
                    let stamp = chrono::Utc::now().format("%Y%m%d%H%M%S");
                    let aside = path.with_extension(format!("broken-{stamp}.json"));
                    log::warn!("catalog unreadable ({e}); kept as {}", aside.display());
                    let _ = std::fs::rename(path, &aside);
                    StoreFile::default()
                }
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => StoreFile::default(),
            Err(e) => return Err(e.into()),
        };
        Ok(Self { path: path.to_path_buf(), data })
    }

    /// Writes through a temporary file and renames it over the old one.
    pub fn save(&self) -> Result<(), StoreError> {
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let tmp = self.path.with_extension("json.tmp");
        {
            let mut f = std::fs::File::create(&tmp)?;
            f.write_all(&serde_json::to_vec_pretty(&self.data)?)?;
            f.sync_all()?;
        }
        std::fs::rename(&tmp, &self.path)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_and_recover() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("store.json");
        let mut s = Store::load(&p).unwrap();
        s.data.settings.glass_strength = 200;
        s.save().unwrap();
        let s2 = Store::load(&p).unwrap();
        assert_eq!(s2.data.settings.glass_strength, 100);
        std::fs::write(&p, b"{not json").unwrap();
        let s3 = Store::load(&p).unwrap();
        assert!(s3.data.instances.is_empty());
        assert!(std::fs::read_dir(dir.path()).unwrap().any(|e| e.unwrap().file_name().to_string_lossy().contains("broken")));
    }
}

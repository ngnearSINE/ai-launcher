//! App icons for the UI, read from the installed executables and cached as PNG files.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::core::Core;

fn png_data_url(path: &Path) -> Option<String> {
    use base64::Engine;
    let bytes = std::fs::read(path).ok()?;
    Some(format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes)))
}

#[cfg(windows)]
fn extract(exe: &Path, out: &Path) -> bool {
    let Some(img) = launcher_platform::win::icon::extract_rgba(exe, 0, 128) else { return false };
    if let Some(dir) = out.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    img.save(out).is_ok()
}

#[cfg(not(windows))]
fn extract(_exe: &Path, _out: &Path) -> bool {
    false
}

/// `installation id → data URL`. Web apps have no executable icon and are drawn by the UI.
pub fn app_icons(core: &Core) -> HashMap<String, String> {
    let installs: Vec<(String, String, PathBuf)> = {
        let store = core.store.lock();
        store.data.installations.iter().map(|i| (i.id.clone(), i.adapter.clone(), i.exe.clone())).collect()
    };
    // A running process gives the real executable behind a Store alias.
    let running_exe: HashMap<String, PathBuf> = {
        let store = core.store.lock();
        let rt = core.runtime.read();
        store
            .data
            .instances
            .iter()
            .filter_map(|i| Some((i.installation_id.clone(), rt.by_instance.get(&i.id)?.exe.clone()?)))
            .collect()
    };
    let mut out = HashMap::new();
    for (id, adapter, exe) in installs {
        if adapter == "webapp" {
            continue;
        }
        let file = core.icons_dir().join(format!("app-{}.png", id.replace([':', '/', '\\'], "_")));
        if !file.is_file() {
            let ok = extract(&exe, &file) || running_exe.get(&id).map(|e| extract(e, &file)).unwrap_or(false);
            if !ok {
                continue;
            }
        }
        if let Some(url) = png_data_url(&file) {
            out.insert(id, url);
        }
    }
    out
}

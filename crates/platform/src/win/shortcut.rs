//! `.lnk` shortcuts that carry an AppUserModelID, so a shortcut pinned from the Start menu
//! and the running instance window share one taskbar button.

use std::path::Path;

use ::windows::core::{Interface, HSTRING};
use ::windows::Win32::Storage::EnhancedStorage::PKEY_AppUserModel_ID;
use ::windows::Win32::System::Com::StructuredStorage::PropVariantClear;
use ::windows::Win32::System::Com::{CoCreateInstance, IPersistFile, CLSCTX_INPROC_SERVER};
use ::windows::Win32::UI::Shell::PropertiesSystem::IPropertyStore;
use ::windows::Win32::UI::Shell::{IShellLinkW, ShellLink};

use super::{com_init, pv_string};

pub struct Shortcut<'a> {
    pub target: &'a Path,
    pub args: &'a str,
    pub description: &'a str,
    /// `(file, index)`.
    pub icon: Option<(&'a Path, i32)>,
    pub app_id: Option<&'a str>,
}

pub fn create(link_path: &Path, s: &Shortcut) -> Result<(), String> {
    com_init();
    if let Some(dir) = link_path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    unsafe {
        let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER).map_err(|e| e.to_string())?;
        link.SetPath(&HSTRING::from(s.target.as_os_str())).map_err(|e| e.to_string())?;
        link.SetArguments(&HSTRING::from(s.args)).map_err(|e| e.to_string())?;
        link.SetDescription(&HSTRING::from(s.description)).map_err(|e| e.to_string())?;
        if let Some(dir) = s.target.parent() {
            let _ = link.SetWorkingDirectory(&HSTRING::from(dir.as_os_str()));
        }
        if let Some((file, idx)) = s.icon {
            let _ = link.SetIconLocation(&HSTRING::from(file.as_os_str()), idx);
        }
        if let Some(id) = s.app_id {
            let store: IPropertyStore = link.cast().map_err(|e| e.to_string())?;
            let mut pv = pv_string(id).map_err(|e| e.to_string())?;
            let r = store.SetValue(&PKEY_AppUserModel_ID, &pv);
            let _ = PropVariantClear(&mut pv);
            r.map_err(|e| e.to_string())?;
            store.Commit().map_err(|e| e.to_string())?;
        }
        let file: IPersistFile = link.cast().map_err(|e| e.to_string())?;
        file.Save(&HSTRING::from(link_path.as_os_str()), true).map_err(|e| e.to_string())
    }
}

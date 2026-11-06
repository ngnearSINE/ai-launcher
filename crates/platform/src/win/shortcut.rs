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

/// Target, arguments and icon of an existing `.lnk`.
#[derive(Debug, Clone)]
pub struct LinkInfo {
    pub target: std::path::PathBuf,
    pub args: String,
    pub icon: Option<(std::path::PathBuf, i32)>,
}

pub fn read(link_path: &Path) -> Option<LinkInfo> {
    use ::windows::Win32::System::Com::STGM_READ;
    com_init();
    unsafe {
        let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER).ok()?;
        let file: IPersistFile = link.cast().ok()?;
        file.Load(&HSTRING::from(link_path.as_os_str()), STGM_READ).ok()?;
        let mut buf = [0u16; 1024];
        link.GetPath(&mut buf, std::ptr::null_mut(), 0).ok()?;
        let target = String::from_utf16_lossy(&buf[..buf.iter().position(|c| *c == 0).unwrap_or(0)]);
        let mut abuf = [0u16; 2048];
        link.GetArguments(&mut abuf).ok()?;
        let args = String::from_utf16_lossy(&abuf[..abuf.iter().position(|c| *c == 0).unwrap_or(0)]);
        let mut ibuf = [0u16; 1024];
        let mut idx = 0i32;
        let icon = link.GetIconLocation(&mut ibuf, &mut idx).ok().and_then(|_| {
            let s = String::from_utf16_lossy(&ibuf[..ibuf.iter().position(|c| *c == 0).unwrap_or(0)]);
            (!s.is_empty()).then(|| (std::path::PathBuf::from(s), idx))
        });
        Some(LinkInfo { target: target.into(), args, icon })
    }
}

/// `.lnk` files under the user's and the machine's Start menu.
pub fn start_menu_links() -> Vec<std::path::PathBuf> {
    let mut roots = Vec::new();
    if let Some(a) = std::env::var_os("APPDATA") {
        roots.push(std::path::PathBuf::from(a).join(r"Microsoft\Windows\Start Menu\Programs"));
    }
    if let Some(p) = std::env::var_os("ProgramData") {
        roots.push(std::path::PathBuf::from(p).join(r"Microsoft\Windows\Start Menu\Programs"));
    }
    let mut out = Vec::new();
    let mut stack: Vec<(std::path::PathBuf, u8)> = roots.into_iter().map(|r| (r, 0)).collect();
    while let Some((dir, depth)) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() && depth < 3 {
                stack.push((p, depth + 1));
            } else if p.extension().map(|x| x.eq_ignore_ascii_case("lnk")).unwrap_or(false) {
                out.push(p);
            }
        }
    }
    out
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

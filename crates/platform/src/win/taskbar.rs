//! Per-window taskbar identity.
//!
//! The taskbar groups windows by AppUserModelID. Giving another process's window its own
//! explicit ID splits it into its own taskbar button; the relaunch properties make a pinned
//! button start the launcher command instead of the app. Microsoft documents that
//! RelaunchCommand and RelaunchDisplayNameResource only work together and only on windows with
//! an explicit ID, and that they should be in place before the user pins.

use ::windows::Win32::Foundation::{HWND, PROPERTYKEY};
use ::windows::Win32::Storage::EnhancedStorage::{
    PKEY_AppUserModel_ID, PKEY_AppUserModel_RelaunchCommand, PKEY_AppUserModel_RelaunchDisplayNameResource,
    PKEY_AppUserModel_RelaunchIconResource,
};
use ::windows::Win32::System::Com::StructuredStorage::{PropVariantClear, PROPVARIANT};
use ::windows::Win32::UI::Shell::PropertiesSystem::{IPropertyStore, SHGetPropertyStoreForWindow};

use super::pv_string;

#[derive(Debug, Clone, PartialEq)]
pub struct WindowIdentity {
    pub app_id: String,
    /// Full command line run when the pinned button is clicked; `None` keeps the app's own.
    pub relaunch_command: Option<String>,
    pub display_name: String,
    /// `path,index` or a path to an `.ico`.
    pub icon: Option<String>,
}

unsafe fn set(store: &IPropertyStore, key: &PROPERTYKEY, value: Option<&str>) -> ::windows::core::Result<()> {
    let mut pv = match value {
        Some(v) => pv_string(v)?,
        None => PROPVARIANT::default(),
    };
    let r = store.SetValue(key, &pv);
    let _ = PropVariantClear(&mut pv);
    r
}

/// Gives the window its own taskbar group (and pin behaviour).
pub fn apply(hwnd: isize, id: &WindowIdentity) -> Result<(), String> {
    unsafe {
        let store: IPropertyStore = SHGetPropertyStoreForWindow(HWND(hwnd as _)).map_err(|e| e.to_string())?;
        if let Some(cmd) = &id.relaunch_command {
            set(&store, &PKEY_AppUserModel_RelaunchCommand, Some(cmd)).map_err(|e| e.to_string())?;
            set(&store, &PKEY_AppUserModel_RelaunchDisplayNameResource, Some(&id.display_name)).map_err(|e| e.to_string())?;
            if let Some(icon) = &id.icon {
                set(&store, &PKEY_AppUserModel_RelaunchIconResource, Some(icon)).map_err(|e| e.to_string())?;
            }
        }
        set(&store, &PKEY_AppUserModel_ID, Some(&id.app_id)).map_err(|e| e.to_string())?;
        store.Commit().map_err(|e| e.to_string())
    }
}

/// Returns the window to its process's normal group.
pub fn clear(hwnd: isize) -> Result<(), String> {
    unsafe {
        let store: IPropertyStore = SHGetPropertyStoreForWindow(HWND(hwnd as _)).map_err(|e| e.to_string())?;
        for key in [
            &PKEY_AppUserModel_ID,
            &PKEY_AppUserModel_RelaunchCommand,
            &PKEY_AppUserModel_RelaunchDisplayNameResource,
            &PKEY_AppUserModel_RelaunchIconResource,
        ] {
            set(&store, key, None).map_err(|e| e.to_string())?;
        }
        store.Commit().map_err(|e| e.to_string())
    }
}

/// The explicit ID currently on the window, if any.
pub fn current_app_id(hwnd: isize) -> Option<String> {
    unsafe {
        let store: IPropertyStore = SHGetPropertyStoreForWindow(HWND(hwnd as _)).ok()?;
        let mut pv = store.GetValue(&PKEY_AppUserModel_ID).ok()?;
        let inner = &pv.Anonymous.Anonymous;
        let out = if inner.vt == ::windows::Win32::System::Variant::VT_LPWSTR {
            inner.Anonymous.pwszVal.to_string().ok()
        } else {
            None
        };
        let _ = PropVariantClear(&mut pv);
        out
    }
}

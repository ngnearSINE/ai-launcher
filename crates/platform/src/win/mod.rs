//! Windows integration.

pub mod dock;
pub mod dock_input;
pub mod ghost;
pub mod icon;
pub mod magnet;
pub mod shortcut;
pub mod taskbar;
pub mod watcher;
pub mod windows;

use ::windows::core::{HSTRING, PWSTR};
use ::windows::Win32::System::Com::StructuredStorage::PROPVARIANT;
use ::windows::Win32::System::Variant::VT_LPWSTR;
use ::windows::Win32::UI::Shell::SHStrDupW;

/// 8.3 form of an existing path. The shell parses `path,index` icon resources up to the first
/// space, so icon paths handed to the taskbar must not contain spaces. Falls back to the input
/// when short names are disabled on the volume.
pub fn short_path(p: &std::path::Path) -> std::path::PathBuf {
    use ::windows::Win32::Storage::FileSystem::GetShortPathNameW;
    let wide: Vec<u16> = p.as_os_str().encode_wide_nul();
    let mut buf = vec![0u16; 1024];
    let n = unsafe { GetShortPathNameW(::windows::core::PCWSTR(wide.as_ptr()), Some(&mut buf)) } as usize;
    if n == 0 || n >= buf.len() {
        return p.to_path_buf();
    }
    let s = String::from_utf16_lossy(&buf[..n]);
    if s.contains(' ') {
        p.to_path_buf()
    } else {
        std::path::PathBuf::from(s)
    }
}

trait EncodeWideNul {
    fn encode_wide_nul(&self) -> Vec<u16>;
}
impl EncodeWideNul for std::ffi::OsStr {
    fn encode_wide_nul(&self) -> Vec<u16> {
        use std::os::windows::ffi::OsStrExt;
        self.encode_wide().chain(std::iter::once(0)).collect()
    }
}

/// AppUserModelID of a packaged (MSIX/Store) process, e.g. `Claude_pzs8sxrjxfjjc!Claude`.
/// `None` for ordinary desktop processes, whose ID cannot be read from outside.
pub fn process_app_id(pid: u32) -> Option<String> {
    use ::windows::Win32::Foundation::CloseHandle;
    use ::windows::Win32::Storage::Packaging::Appx::GetApplicationUserModelId;
    use ::windows::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};
    unsafe {
        let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut len: u32 = 256;
        let mut buf = vec![0u16; len as usize];
        let r = GetApplicationUserModelId(h, &mut len, Some(PWSTR(buf.as_mut_ptr())));
        let _ = CloseHandle(h);
        if r.is_err() || len == 0 {
            return None;
        }
        Some(String::from_utf16_lossy(&buf[..(len as usize).saturating_sub(1)]))
    }
}

/// A `VT_LPWSTR` PROPVARIANT whose buffer the caller frees with `PropVariantClear`.
pub(crate) unsafe fn pv_string(s: &str) -> ::windows::core::Result<PROPVARIANT> {
    let p: PWSTR = SHStrDupW(&HSTRING::from(s))?;
    let mut pv = PROPVARIANT::default();
    (*pv.Anonymous.Anonymous).vt = VT_LPWSTR;
    (*pv.Anonymous.Anonymous).Anonymous.pwszVal = p;
    Ok(pv)
}

/// Initialises COM (apartment) for the calling thread; harmless if already initialised.
/// Threads that touch window property stores or shortcuts call this once.
pub fn com_init() {
    use ::windows::Win32::System::Com::{CoInitializeEx, COINIT_APARTMENTTHREADED};
    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
    }
}

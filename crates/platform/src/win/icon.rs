//! Reads an executable's icon as RGBA pixels (for badges and the UI).

use std::path::Path;

use ::windows::core::HSTRING;
use ::windows::Win32::Graphics::Gdi::{
    CreateCompatibleDC, DeleteDC, DeleteObject, GetDIBits, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS,
    HGDIOBJ,
};
use ::windows::Win32::UI::Shell::SHDefExtractIconW;
use ::windows::Win32::UI::WindowsAndMessaging::{DestroyIcon, GetIconInfo, HICON, ICONINFO};

/// Icon `index` of `file` at `size`×`size`, or `None` if it cannot be read.
pub fn extract_rgba(file: &Path, index: i32, size: u32) -> Option<image::RgbaImage> {
    unsafe {
        let mut hicon = HICON::default();
        SHDefExtractIconW(&HSTRING::from(file.as_os_str()), index, 0, Some(&mut hicon), None, size).ok().ok()?;
        if hicon.is_invalid() {
            return None;
        }
        let out = icon_pixels(hicon, size);
        let _ = DestroyIcon(hicon);
        out
    }
}

unsafe fn icon_pixels(hicon: HICON, size: u32) -> Option<image::RgbaImage> {
    let mut info = ICONINFO::default();
    GetIconInfo(hicon, &mut info).ok()?;
    let dc = CreateCompatibleDC(None);
    let mut bmi = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: size as i32,
            biHeight: -(size as i32),
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut buf = vec![0u8; (size * size * 4) as usize];
    let lines = GetDIBits(dc, info.hbmColor, 0, size, Some(buf.as_mut_ptr() as _), &mut bmi, DIB_RGB_COLORS);
    let _ = DeleteDC(dc);
    let _ = DeleteObject(HGDIOBJ(info.hbmColor.0));
    let _ = DeleteObject(HGDIOBJ(info.hbmMask.0));
    if lines == 0 {
        return None;
    }
    let has_alpha = buf.chunks_exact(4).any(|p| p[3] != 0);
    for p in buf.chunks_exact_mut(4) {
        p.swap(0, 2);
        if !has_alpha {
            p[3] = 255;
        }
    }
    image::RgbaImage::from_raw(size, size, buf)
}

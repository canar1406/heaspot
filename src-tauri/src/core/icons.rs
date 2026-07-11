//! Trích xuất icon thật của app (.lnk/.exe) qua Windows Shell,
//! encode PNG rồi trả về dạng data URL cho frontend hiển thị.

use base64::Engine;
use std::ffi::c_void;

pub fn extract_icon_data_url(path: &str) -> Option<String> {
    ensure_com();
    let png = unsafe { extract_png(path) }?;
    Some(format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(png)
    ))
}

thread_local! {
    static COM_INIT: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// SHGetFileInfoW cần COM (nhất là để resolve .lnk) — khởi tạo 1 lần mỗi thread.
fn ensure_com() {
    COM_INIT.with(|c| {
        if !c.get() {
            unsafe {
                use windows_sys::Win32::System::Com::{CoInitializeEx, COINIT_APARTMENTTHREADED};
                CoInitializeEx(std::ptr::null(), COINIT_APARTMENTTHREADED as _);
            }
            c.set(true);
        }
    });
}

unsafe fn extract_png(path: &str) -> Option<Vec<u8>> {
    use windows_sys::Win32::UI::Shell::{
        SHGetFileInfoW, SHFILEINFOW, SHGFI_ICON, SHGFI_LARGEICON,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::DestroyIcon;

    let wide: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
    let mut shfi: SHFILEINFOW = std::mem::zeroed();
    let ok = SHGetFileInfoW(
        wide.as_ptr(),
        0,
        &mut shfi,
        std::mem::size_of::<SHFILEINFOW>() as u32,
        SHGFI_ICON | SHGFI_LARGEICON,
    );
    if ok == 0 || shfi.hIcon.is_null() {
        return None;
    }
    let png = hicon_to_png(shfi.hIcon);
    DestroyIcon(shfi.hIcon);
    png
}

unsafe fn hicon_to_png(
    hicon: windows_sys::Win32::UI::WindowsAndMessaging::HICON,
) -> Option<Vec<u8>> {
    use windows_sys::Win32::Graphics::Gdi::{
        DeleteObject, GetDC, GetDIBits, GetObjectW, ReleaseDC, BITMAP, BITMAPINFO,
        BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{GetIconInfo, ICONINFO};

    let mut info: ICONINFO = std::mem::zeroed();
    if GetIconInfo(hicon, &mut info) == 0 {
        return None;
    }
    let hbm_color = info.hbmColor;
    let hbm_mask = info.hbmMask;

    let result = (|| {
        let mut bm: BITMAP = std::mem::zeroed();
        if GetObjectW(
            hbm_color,
            std::mem::size_of::<BITMAP>() as i32,
            &mut bm as *mut _ as *mut c_void,
        ) == 0
        {
            return None;
        }
        let (w, h) = (bm.bmWidth, bm.bmHeight);
        if w <= 0 || h <= 0 || w > 512 || h > 512 {
            return None;
        }

        let hdc = GetDC(std::ptr::null_mut());
        let mut bi: BITMAPINFO = std::mem::zeroed();
        bi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
        bi.bmiHeader.biWidth = w;
        bi.bmiHeader.biHeight = -h; // top-down
        bi.bmiHeader.biPlanes = 1;
        bi.bmiHeader.biBitCount = 32;
        bi.bmiHeader.biCompression = BI_RGB as u32;

        let mut pixels = vec![0u8; (w as usize) * (h as usize) * 4];
        let got = GetDIBits(
            hdc,
            hbm_color,
            0,
            h as u32,
            pixels.as_mut_ptr() as *mut c_void,
            &mut bi,
            DIB_RGB_COLORS,
        );
        ReleaseDC(std::ptr::null_mut(), hdc);
        if got == 0 {
            return None;
        }

        // BGRA -> RGBA
        for px in pixels.chunks_exact_mut(4) {
            px.swap(0, 2);
        }
        // Icon cũ không có alpha channel -> coi như đặc hoàn toàn
        if pixels.chunks_exact(4).all(|p| p[3] == 0) {
            for p in pixels.chunks_exact_mut(4) {
                p[3] = 255;
            }
        }

        let img = image::RgbaImage::from_raw(w as u32, h as u32, pixels)?;
        let mut buf = Vec::new();
        img.write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png)
            .ok()?;
        Some(buf)
    })();

    DeleteObject(hbm_color);
    DeleteObject(hbm_mask);
    result
}

//! Windows icon resolver via the Shell namespace.
//!
//! Uses `IShellItemImageFactory::GetImage` which understands:
//! - Win32 paths (`.lnk`, `.exe`, files, folders)
//! - UWP / MSIX entries (`shell:AppsFolder\{AUMID}`)
//! - Any other parsable shell namespace path
//!
//! Returns a `data:image/png;base64,…` URL the frontend can drop into an
//! `<img>` tag directly. Caching is handled one layer up in
//! `platform::get_icon` so this function may run cold per (kind, path) pair.

use base64::Engine;
use windows::Win32::Foundation::SIZE;
use windows::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, DIB_RGB_COLORS, DeleteObject, GetDC, GetDIBits, HBITMAP,
    HGDIOBJ, ReleaseDC,
};
use windows::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx};
use windows::Win32::UI::Shell::{
    IShellItemImageFactory, SHCreateItemFromParsingName, SIIGBF_RESIZETOFIT,
};
use windows::core::HSTRING;

const ICON_PX: i32 = 48;

pub(crate) fn resolve(_kind: &str, path: &str) -> Option<String> {
    if path.is_empty() {
        return None;
    }
    // SHCreateItemFromParsingName speaks native paths. Indexer rows store
    // file paths with forward slashes; convert before handing to the shell.
    // shell: URIs keep their backslashes (we constructed them that way).
    let parsing = if path.starts_with("shell:") {
        path.to_string()
    } else {
        path.replace('/', "\\")
    };
    unsafe {
        // Idempotent; RPC_E_CHANGED_MODE is harmless if some other code already
        // CoInit'd this thread with a different apartment model.
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);

        let factory: IShellItemImageFactory =
            match SHCreateItemFromParsingName(&HSTRING::from(parsing.as_str()), None) {
                Ok(f) => f,
                Err(e) => {
                    eprintln!("[icons] parse {parsing:?} failed: {e}");
                    return None;
                }
            };

        let size = SIZE {
            cx: ICON_PX,
            cy: ICON_PX,
        };
        let hbitmap = match factory.GetImage(size, SIIGBF_RESIZETOFIT) {
            Ok(h) => h,
            Err(e) => {
                eprintln!("[icons] GetImage {parsing:?} failed: {e}");
                return None;
            }
        };

        let png_bytes = hbitmap_to_png(hbitmap);
        let _ = DeleteObject(HGDIOBJ(hbitmap.0));

        let bytes = png_bytes?;
        let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
        Some(format!("data:image/png;base64,{b64}"))
    }
}

unsafe fn hbitmap_to_png(hbitmap: HBITMAP) -> Option<Vec<u8>> {
    let w = ICON_PX;
    let h = ICON_PX;

    let mut bmi = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: w,
            biHeight: -h, // negative = top-down rows
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            biSizeImage: 0,
            biXPelsPerMeter: 0,
            biYPelsPerMeter: 0,
            biClrUsed: 0,
            biClrImportant: 0,
        },
        bmiColors: [Default::default(); 1],
    };

    let mut pixels = vec![0u8; (w * h * 4) as usize];

    let scanlines = unsafe {
        let hdc = GetDC(None);
        let n = GetDIBits(
            hdc,
            hbitmap,
            0,
            h as u32,
            Some(pixels.as_mut_ptr() as *mut _),
            &mut bmi,
            DIB_RGB_COLORS,
        );
        ReleaseDC(None, hdc);
        n
    };
    if scanlines == 0 {
        return None;
    }

    // GetDIBits returns BGRA; PNG wants RGBA.
    for chunk in pixels.chunks_exact_mut(4) {
        chunk.swap(0, 2);
    }

    let mut buf = Vec::with_capacity(2048);
    {
        let mut encoder = png::Encoder::new(&mut buf, w as u32, h as u32);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().ok()?;
        writer.write_image_data(&pixels).ok()?;
    }
    Some(buf)
}

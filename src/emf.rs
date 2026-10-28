//! A diagram image in a higher resolution (LC-305): EA saves bitmaps of a diagram at 100% only, so an image enlarged in
//! Jira or Confluence is blurred. EA also saves the diagram as an enhanced metafile (vector), and Windows draws a
//! metafile at any size: lines and text stay sharp.

use anyhow::{bail, Context, Result};
use std::path::Path;

/// The longest side of a rendered image; larger diagrams are drawn smaller than asked.
const MAX_SIDE: f32 = 8000.0;

/// Draws the metafile `emf` at `width` x `height` pixels on white and saves it as PNG.
#[cfg(windows)]
pub fn emf_to_png(emf: &Path, png: &Path, width: u32, height: u32) -> Result<(u32, u32)> {
    use std::os::windows::ffi::OsStrExt;
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::RECT;
    use windows::Win32::Graphics::Gdi::*;

    let wide: Vec<u16> = emf.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
    unsafe {
        let meta = GetEnhMetaFileW(PCWSTR(wide.as_ptr()));
        if meta.is_invalid() {
            bail!("{}", crate::i18n::tr("EA did not save the diagram as a metafile"));
        }
        let mut header = ENHMETAHEADER::default();
        if GetEnhMetaFileHeader(meta, std::mem::size_of::<ENHMETAHEADER>() as u32, Some(&mut header)) == 0 {
            let _ = DeleteEnhMetaFile(meta);
            bail!("{}", crate::i18n::tr("EA did not save the diagram as a metafile"));
        }
        let fit = (MAX_SIDE / (width.max(height).max(1) as f32)).min(1.0);
        let (w, h) = (((width as f32) * fit).round().max(1.0) as i32, ((height as f32) * fit).round().max(1.0) as i32);

        let dc = CreateCompatibleDC(None);
        let mut info = BITMAPINFO::default();
        info.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
        info.bmiHeader.biWidth = w;
        info.bmiHeader.biHeight = -h; // top-down rows
        info.bmiHeader.biPlanes = 1;
        info.bmiHeader.biBitCount = 32;
        info.bmiHeader.biCompression = BI_RGB.0;
        let mut bits: *mut std::ffi::c_void = std::ptr::null_mut();
        let bitmap = CreateDIBSection(dc, &info, DIB_RGB_COLORS, &mut bits, None, 0).context("CreateDIBSection")?;
        let old = SelectObject(dc, bitmap);
        let _ = PatBlt(dc, 0, 0, w, h, WHITENESS);
        SetStretchBltMode(dc, HALFTONE);
        let played = PlayEnhMetaFile(dc, meta, &RECT { left: 0, top: 0, right: w, bottom: h });
        let _ = GdiFlush();
        let pixels = std::slice::from_raw_parts(bits as *const u8, (w * h * 4) as usize).to_vec();
        SelectObject(dc, old);
        let _ = DeleteObject(bitmap);
        let _ = DeleteDC(dc);
        let _ = DeleteEnhMetaFile(meta);
        if !played.as_bool() {
            bail!("{}", crate::i18n::tr("Windows could not draw the diagram's metafile"));
        }
        // BGRA from GDI to RGB for the PNG.
        let mut rgb = Vec::with_capacity((w * h * 3) as usize);
        for px in pixels.chunks_exact(4) {
            rgb.extend_from_slice(&[px[2], px[1], px[0]]);
        }
        let img = image::RgbImage::from_raw(w as u32, h as u32, rgb).context("image")?;
        img.save(png).with_context(|| png.display().to_string())?;
        Ok((w as u32, h as u32))
    }
}

#[cfg(not(windows))]
pub fn emf_to_png(_emf: &Path, _png: &Path, _width: u32, _height: u32) -> Result<(u32, u32)> {
    bail!("metafiles are drawn only on Windows")
}

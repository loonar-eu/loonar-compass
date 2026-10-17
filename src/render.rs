//! Images drawn by the assistant: SVG (e.g. matrix, table, chart, diagram) converted to PNG.

use anyhow::{Context, Result};
use std::sync::{Arc, OnceLock};

/// System fonts (loaded once): text in the SVG is drawn with Segoe UI, including Polish characters.
fn fonts() -> Arc<resvg::usvg::fontdb::Database> {
    static FONTS: OnceLock<Arc<resvg::usvg::fontdb::Database>> = OnceLock::new();
    FONTS
        .get_or_init(|| {
            let mut db = resvg::usvg::fontdb::Database::new();
            db.load_system_fonts();
            db.set_sans_serif_family("Segoe UI");
            Arc::new(db)
        })
        .clone()
}

/// Largest image side in pixels: a bigger SVG would need gigabytes of memory.
const MAX_SIDE: u32 = 8000;

/// Converts SVG to PNG on a white background, at scale `scale` (1.0 = the size from the SVG).
pub fn svg_to_png(svg: &str, scale: f32) -> Result<Vec<u8>> {
    let mut options = resvg::usvg::Options { font_family: "Segoe UI".into(), ..Default::default() };
    options.fontdb = fonts();
    // Only images embedded in the SVG (data: URLs). An href to a file or a network path is ignored, so an SVG written
    // by the model can neither read the user's files nor make the computer connect to another server.
    options.image_href_resolver = resvg::usvg::ImageHrefResolver {
        resolve_data: resvg::usvg::ImageHrefResolver::default_data_resolver(),
        resolve_string: Box::new(|_, _| None),
    };
    let tree = resvg::usvg::Tree::from_str(svg, &options).context(crate::i18n::tr("invalid SVG"))?;
    let size = tree.size().to_int_size().scale_by(scale).context(crate::i18n::tr("the image has no size"))?;
    if size.width() > MAX_SIDE || size.height() > MAX_SIDE {
        anyhow::bail!("{}", crate::i18n::fmt("the image is too large ({0} × {1} px; at most {2} px per side)", &[size.width().to_string(), size.height().to_string(), MAX_SIDE.to_string()]));
    }
    let mut pixmap = resvg::tiny_skia::Pixmap::new(size.width(), size.height()).context(crate::i18n::tr("image too large"))?;
    pixmap.fill(resvg::tiny_skia::Color::WHITE);
    resvg::render(&tree, resvg::tiny_skia::Transform::from_scale(scale, scale), &mut pixmap.as_mut());
    pixmap.encode_png().context(crate::i18n::tr("saving PNG"))
}

#[cfg(test)]
mod tests {
    #[test]
    fn svg_with_polish_text_renders() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="200" height="60"><rect x="1" y="1" width="198" height="58" fill="#E6F1FB" stroke="#0D355C"/><text x="10" y="35" font-size="16">Zażółć gęślą jaźń</text></svg>"##;
        let png = super::svg_to_png(svg, 1.5).unwrap();
        let img = image::load_from_memory(&png).unwrap();
        assert_eq!((img.width(), img.height()), (300, 90));
    }

    #[test]
    fn svg_cannot_read_files_or_allocate_huge_images() {
        // A local file referenced by the SVG is not drawn (the image stays white, rendering does not fail).
        let file = std::env::temp_dir().join("loonar-render-test.png");
        image::RgbaImage::from_pixel(4, 4, image::Rgba([255, 0, 0, 255])).save(&file).unwrap();
        let svg = format!(r#"<svg xmlns="http://www.w3.org/2000/svg" width="4" height="4"><image href="{}" width="4" height="4"/></svg>"#, file.display());
        let png = super::svg_to_png(&svg, 1.0).unwrap();
        let img = image::load_from_memory(&png).unwrap().to_rgba8();
        assert_eq!(img.get_pixel(1, 1).0, [255, 255, 255, 255], "the file must not be embedded");
        let huge = r#"<svg xmlns="http://www.w3.org/2000/svg" width="100000" height="100000"/>"#;
        assert!(super::svg_to_png(huge, 1.0).is_err());
    }
}

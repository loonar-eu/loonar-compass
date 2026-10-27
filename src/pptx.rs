//! PowerPoint presentations (PPTX) built by the application: a title slide and content slides with a title, bullet
//! points (two levels) and an optional picture (a diagram from EA or an image drawn by the assistant), 16:9.
//! The Office Open XML parts are written directly (no Office needed); PDF goes through Word (office.rs).

use anyhow::Result;
use std::io::Write;

/// One slide: title, bullets (text, level 0 or 1) and an optional PNG picture.
pub struct Slide {
    pub title: String,
    pub bullets: Vec<(String, u8)>,
    pub picture: Option<Vec<u8>>,
}

const W: i64 = 12_192_000;
const H: i64 = 6_858_000;
const NS: &str = r#"xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main""#;

fn esc(text: &str) -> String {
    text.chars()
        .filter(|c| !c.is_control() || *c == '\t')
        .collect::<String>()
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn text_box(id: u32, name: &str, x: i64, y: i64, w: i64, h: i64, paragraphs: &str) -> String {
    format!(
        r#"<p:sp><p:nvSpPr><p:cNvPr id="{id}" name="{name}"/><p:cNvSpPr txBox="1"/><p:nvPr/></p:nvSpPr><p:spPr><a:xfrm><a:off x="{x}" y="{y}"/><a:ext cx="{w}" cy="{h}"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom><a:noFill/></p:spPr><p:txBody><a:bodyPr wrap="square" lIns="91440" tIns="45720" rIns="91440" bIns="45720"><a:normAutofit/></a:bodyPr><a:lstStyle/>{paragraphs}</p:txBody></p:sp>"#
    )
}

fn title_paragraph(text: &str, size: u32, bold: bool) -> String {
    format!(r#"<a:p><a:r><a:rPr lang="en-US" sz="{size}" b="{}" dirty="0"><a:solidFill><a:srgbClr val="1B2A3A"/></a:solidFill><a:latin typeface="Segoe UI"/></a:rPr><a:t>{}</a:t></a:r></a:p>"#, if bold { 1 } else { 0 }, esc(text))
}

fn bullet_paragraph(text: &str, level: u8) -> String {
    let (indent, size, ch) = if level == 0 { (342_900, 2000, "•") } else { (685_800, 1700, "–") };
    format!(
        r#"<a:p><a:pPr marL="{indent}" indent="-285750"><a:spcBef><a:spcPts val="600"/></a:spcBef><a:buFont typeface="Arial"/><a:buChar char="{ch}"/></a:pPr><a:r><a:rPr lang="en-US" sz="{size}" dirty="0"><a:solidFill><a:srgbClr val="2B3A4A"/></a:solidFill><a:latin typeface="Segoe UI"/></a:rPr><a:t>{}</a:t></a:r></a:p>"#,
        esc(text)
    )
}

/// A picture placed in the given box, keeping its proportions.
fn picture(id: u32, png: &[u8], x: i64, y: i64, w: i64, h: i64) -> String {
    let (pw, ph) = image::load_from_memory(png).map(|i| (i.width() as i64, i.height() as i64)).unwrap_or((4, 3));
    let scale = (w as f64 / pw as f64).min(h as f64 / ph as f64);
    let (cw, ch) = ((pw as f64 * scale) as i64, (ph as f64 * scale) as i64);
    let (cx, cy) = (x + (w - cw) / 2, y + (h - ch) / 2);
    format!(
        r#"<p:pic><p:nvPicPr><p:cNvPr id="{id}" name="Picture {id}"/><p:cNvPicPr><a:picLocks noChangeAspect="1"/></p:cNvPicPr><p:nvPr/></p:nvPicPr><p:blipFill><a:blip r:embed="rIdImg"/><a:stretch><a:fillRect/></a:stretch></p:blipFill><p:spPr><a:xfrm><a:off x="{cx}" y="{cy}"/><a:ext cx="{cw}" cy="{ch}"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></p:spPr></p:pic>"#
    )
}

fn slide_xml(shapes: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><p:sld {NS}><p:cSld><p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="0" cy="0"/><a:chOff x="0" y="0"/><a:chExt cx="0" cy="0"/></a:xfrm></p:grpSpPr>{shapes}</p:spTree></p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:sld>"#
    )
}

const THEME: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><a:theme xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" name="Compass"><a:themeElements><a:clrScheme name="Compass"><a:dk1><a:srgbClr val="1B2A3A"/></a:dk1><a:lt1><a:srgbClr val="FFFFFF"/></a:lt1><a:dk2><a:srgbClr val="1F3A5F"/></a:dk2><a:lt2><a:srgbClr val="F4F1EA"/></a:lt2><a:accent1><a:srgbClr val="2F5FA8"/></a:accent1><a:accent2><a:srgbClr val="D9822B"/></a:accent2><a:accent3><a:srgbClr val="4C9A6A"/></a:accent3><a:accent4><a:srgbClr val="8A5BA8"/></a:accent4><a:accent5><a:srgbClr val="C0504D"/></a:accent5><a:accent6><a:srgbClr val="4BACC6"/></a:accent6><a:hlink><a:srgbClr val="2F5FA8"/></a:hlink><a:folHlink><a:srgbClr val="8A5BA8"/></a:folHlink></a:clrScheme><a:fontScheme name="Compass"><a:majorFont><a:latin typeface="Segoe UI"/><a:ea typeface=""/><a:cs typeface=""/></a:majorFont><a:minorFont><a:latin typeface="Segoe UI"/><a:ea typeface=""/><a:cs typeface=""/></a:minorFont></a:fontScheme><a:fmtScheme name="Compass"><a:fillStyleLst><a:solidFill><a:schemeClr val="phClr"/></a:solidFill><a:solidFill><a:schemeClr val="phClr"/></a:solidFill><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:fillStyleLst><a:lnStyleLst><a:ln w="6350"><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:ln><a:ln w="12700"><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:ln><a:ln w="19050"><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:ln></a:lnStyleLst><a:effectStyleLst><a:effectStyle><a:effectLst/></a:effectStyle><a:effectStyle><a:effectLst/></a:effectStyle><a:effectStyle><a:effectLst/></a:effectStyle></a:effectStyleLst><a:bgFillStyleLst><a:solidFill><a:schemeClr val="phClr"/></a:solidFill><a:solidFill><a:schemeClr val="phClr"/></a:solidFill><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:bgFillStyleLst></a:fmtScheme></a:themeElements></a:theme>"#;

const EMPTY_TREE: &str = r#"<p:cSld><p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr/></p:spTree></p:cSld>"#;

/// Builds the PPTX: a title slide (title and subtitle) and the content slides.
pub fn build(title: &str, subtitle: &str, slides: &[Slide]) -> Result<Vec<u8>> {
    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    let mut put = |name: &str, body: &[u8]| -> Result<()> {
        zip.start_file(name, options)?;
        zip.write_all(body)?;
        Ok(())
    };
    let count = slides.len() + 1;
    let mut types = String::from(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Default Extension="png" ContentType="image/png"/><Override PartName="/ppt/presentation.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml"/><Override PartName="/ppt/slideMasters/slideMaster1.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slideMaster+xml"/><Override PartName="/ppt/slideLayouts/slideLayout1.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slideLayout+xml"/><Override PartName="/ppt/theme/theme1.xml" ContentType="application/vnd.openxmlformats-officedocument.theme+xml"/>"#);
    for n in 1..=count {
        types.push_str(&format!(r#"<Override PartName="/ppt/slides/slide{n}.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slide+xml"/>"#));
    }
    types.push_str("</Types>");
    put("[Content_Types].xml", types.as_bytes())?;
    put("_rels/.rels", br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="ppt/presentation.xml"/></Relationships>"#)?;

    let mut slide_ids = String::new();
    let mut pres_rels = String::from(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdM" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideMaster" Target="slideMasters/slideMaster1.xml"/><Relationship Id="rIdT" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/theme" Target="theme/theme1.xml"/>"#);
    for n in 1..=count {
        slide_ids.push_str(&format!(r#"<p:sldId id="{}" r:id="rIdS{n}"/>"#, 255 + n));
        pres_rels.push_str(&format!(r#"<Relationship Id="rIdS{n}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide" Target="slides/slide{n}.xml"/>"#));
    }
    pres_rels.push_str("</Relationships>");
    put(
        "ppt/presentation.xml",
        format!(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><p:presentation {NS} saveSubsetFonts="1"><p:sldMasterIdLst><p:sldMasterId id="2147483648" r:id="rIdM"/></p:sldMasterIdLst><p:sldIdLst>{slide_ids}</p:sldIdLst><p:sldSz cx="{W}" cy="{H}"/><p:notesSz cx="6858000" cy="9144000"/></p:presentation>"#).as_bytes(),
    )?;
    put("ppt/_rels/presentation.xml.rels", pres_rels.as_bytes())?;
    put("ppt/theme/theme1.xml", THEME.as_bytes())?;
    put(
        "ppt/slideMasters/slideMaster1.xml",
        format!(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><p:sldMaster {NS}><p:cSld><p:bg><p:bgPr><a:solidFill><a:srgbClr val="FFFFFF"/></a:solidFill><a:effectLst/></p:bgPr></p:bg><p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr/></p:spTree></p:cSld><p:clrMap bg1="lt1" tx1="dk1" bg2="lt2" tx2="dk2" accent1="accent1" accent2="accent2" accent3="accent3" accent4="accent4" accent5="accent5" accent6="accent6" hlink="hlink" folHlink="folHlink"/><p:sldLayoutIdLst><p:sldLayoutId id="2147483649" r:id="rIdL"/></p:sldLayoutIdLst></p:sldMaster>"#).as_bytes(),
    )?;
    put("ppt/slideMasters/_rels/slideMaster1.xml.rels", br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdL" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout" Target="../slideLayouts/slideLayout1.xml"/><Relationship Id="rIdT" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/theme" Target="../theme/theme1.xml"/></Relationships>"#)?;
    put("ppt/slideLayouts/slideLayout1.xml", format!(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><p:sldLayout {NS} type="blank" preserve="1">{EMPTY_TREE}<p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:sldLayout>"#).as_bytes())?;
    put("ppt/slideLayouts/_rels/slideLayout1.xml.rels", br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdM" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideMaster" Target="../slideMasters/slideMaster1.xml"/></Relationships>"#)?;

    let layout_rel = r#"<Relationship Id="rIdL" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout" Target="../slideLayouts/slideLayout1.xml"/>"#;
    let rels = |image: Option<usize>| {
        let img = image.map(|n| format!(r#"<Relationship Id="rIdImg" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="../media/image{n}.png"/>"#)).unwrap_or_default();
        format!(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">{layout_rel}{img}</Relationships>"#)
    };
    // Title slide: a band of colour, the title and the subtitle.
    let band = r#"<p:sp><p:nvSpPr><p:cNvPr id="2" name="Band"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr><p:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="12192000" cy="228600"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom><a:solidFill><a:srgbClr val="2F5FA8"/></a:solidFill><a:ln><a:noFill/></a:ln></p:spPr></p:sp>"#;
    let first = format!("{band}{}{}", text_box(3, "Title", 838_200, 2_286_000, W - 1_676_400, 1_371_600, &title_paragraph(title, 4000, true)), text_box(4, "Subtitle", 838_200, 3_771_900, W - 1_676_400, 914_400, &title_paragraph(subtitle, 2000, false)));
    put("ppt/slides/slide1.xml", slide_xml(&first).as_bytes())?;
    put("ppt/slides/_rels/slide1.xml.rels", rels(None).as_bytes())?;
    let mut images = 0;
    for (i, slide) in slides.iter().enumerate() {
        let n = i + 2;
        let mut shapes = format!("{band}{}", text_box(3, "Title", 609_600, 381_000, W - 1_219_200, 914_400, &title_paragraph(&slide.title, 3000, true)));
        let text_width = if slide.picture.is_some() && !slide.bullets.is_empty() { (W - 1_219_200) / 2 - 152_400 } else { W - 1_219_200 };
        if !slide.bullets.is_empty() {
            let paragraphs: String = slide.bullets.iter().map(|(t, l)| bullet_paragraph(t, *l)).collect();
            shapes.push_str(&text_box(4, "Content", 609_600, 1_447_800, text_width, H - 1_905_000, &paragraphs));
        }
        let image = match &slide.picture {
            Some(png) => {
                images += 1;
                put(&format!("ppt/media/image{images}.png"), png)?;
                let x = if slide.bullets.is_empty() { 609_600 } else { 609_600 + text_width + 304_800 };
                let w = if slide.bullets.is_empty() { W - 1_219_200 } else { W - x - 609_600 };
                shapes.push_str(&picture(5, png, x, 1_447_800, w, H - 1_905_000));
                Some(images)
            }
            None => None,
        };
        put(&format!("ppt/slides/slide{n}.xml"), slide_xml(&shapes).as_bytes())?;
        put(&format!("ppt/slides/_rels/slide{n}.xml.rels"), rels(image).as_bytes())?;
    }
    Ok(zip.finish()?.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png() -> Vec<u8> {
        let img = image::RgbaImage::from_pixel(40, 20, image::Rgba([47, 95, 168, 255]));
        let mut out = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(img).write_to(&mut out, image::ImageFormat::Png).unwrap();
        out.into_inner()
    }

    fn sample() -> Vec<u8> {
        build(
            "Shop model review",
            "Loonar Compass · 2026",
            &[
                Slide { title: "Findings".into(), bullets: vec![("Two classes without a description".into(), 0), ("Order & Invoice <linked>".into(), 1)], picture: None },
                Slide { title: "Domain diagram".into(), bullets: vec![], picture: Some(png()) },
                Slide { title: "Both".into(), bullets: vec![("Text left".into(), 0)], picture: Some(png()) },
            ],
        )
        .unwrap()
    }

    #[test]
    fn builds_a_presentation() {
        let bytes = sample();
        let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
        assert!(zip.by_name("ppt/slides/slide4.xml").is_ok());
        assert!(zip.by_name("ppt/media/image2.png").is_ok());
        let mut xml = String::new();
        std::io::Read::read_to_string(&mut zip.by_name("ppt/slides/slide2.xml").unwrap(), &mut xml).unwrap();
        assert!(xml.contains("Order &amp; Invoice &lt;linked&gt;"));
    }

    /// Opens the presentation in the installed PowerPoint and exports it to PDF (proof the file is valid):
    /// `cargo test pptx_opens_in_powerpoint -- --ignored --nocapture`
    #[test]
    #[ignore = "requires Microsoft PowerPoint"]
    fn pptx_opens_in_powerpoint() {
        let dir = std::env::temp_dir().join(format!("loonar-pptx-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("review.pptx");
        std::fs::write(&file, sample()).unwrap();
        let pdf = dir.join("review.pdf");
        crate::office::pptx_to_pdf(&file, &pdf).unwrap();
        let size = std::fs::metadata(&pdf).unwrap().len();
        println!("{} -> {} bytes", pdf.display(), size);
        assert!(size > 1000);
    }
}

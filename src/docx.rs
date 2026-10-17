//! Word documents (DOCX) assembled by the assistant from Markdown: headings, paragraphs with bold text,
//! lists, tables and images (diagrams from EA or images drawn earlier).

use anyhow::{Context, Result};
use docx_rs::{AlignmentType, Docx, Paragraph, Pic, Run, RunFonts, Style, StyleType, Table, TableCell, TableRow};

/// Image inserted into the document in place of the `![caption](source)` marker.
pub struct Picture {
    pub png: Vec<u8>,
    pub caption: String,
}

/// Page width for images: 16 cm (A4 with 2.5 cm margins), in EMU.
const MAX_WIDTH_EMU: u32 = 5_760_000;
const EMU_PER_PX: u32 = 9_525;

fn runs(text: &str, bold_all: bool) -> Vec<Run> {
    // **bold**; `code` markers are removed (only the text remains).
    let text = text.replace('`', "");
    let mut out = Vec::new();
    for (i, part) in text.split("**").enumerate() {
        if part.is_empty() {
            continue;
        }
        let run = Run::new().add_text(part);
        out.push(if bold_all || i % 2 == 1 { run.bold() } else { run });
    }
    out
}

fn paragraph(text: &str, style: Option<&str>, bold: bool) -> Paragraph {
    let mut p = Paragraph::new();
    for r in runs(text, bold) {
        p = p.add_run(r);
    }
    match style {
        Some(s) => p.style(s),
        None => p,
    }
}

fn table(rows: &[Vec<String>]) -> Table {
    let rows = rows
        .iter()
        .enumerate()
        .map(|(i, cells)| {
            TableRow::new(cells.iter().map(|c| TableCell::new().add_paragraph(paragraph(c, None, i == 0))).collect())
        })
        .collect();
    Table::new(rows)
}

fn cells(line: &str) -> Vec<String> {
    line.trim().trim_matches('|').split('|').map(|c| c.trim().to_string()).collect()
}

/// Builds the document. `resolve` turns an image source from the Markdown (e.g. `diagram:12`, `image:file.png`) into an image.
pub fn build(title: &str, markdown: &str, mut resolve: impl FnMut(&str, &str) -> Result<Picture>) -> Result<Vec<u8>> {
    let mut doc = Docx::new()
        .default_fonts(RunFonts::new().ascii("Calibri").hi_ansi("Calibri").cs("Calibri").east_asia("Calibri"))
        .default_size(22)
        .add_style(Style::new("Title", StyleType::Paragraph).name("Title").size(48).bold())
        .add_style(Style::new("Heading1", StyleType::Paragraph).name("Heading 1").size(32).bold())
        .add_style(Style::new("Heading2", StyleType::Paragraph).name("Heading 2").size(28).bold())
        .add_style(Style::new("Heading3", StyleType::Paragraph).name("Heading 3").size(24).bold())
        .add_style(Style::new("Caption", StyleType::Paragraph).name("Caption").size(18).italic());
    if !title.trim().is_empty() {
        doc = doc.add_paragraph(paragraph(title.trim(), Some("Title"), false));
    }
    let lines: Vec<&str> = markdown.lines().collect();
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i].trim_end();
        let trimmed = line.trim();
        if trimmed.starts_with('|') {
            // Table: consecutive lines with |, without the separator line (|---|---|).
            let mut rows = Vec::new();
            while i < lines.len() && lines[i].trim().starts_with('|') {
                let row = lines[i].trim();
                if !row.chars().all(|c| matches!(c, '|' | '-' | ':' | ' ')) {
                    rows.push(cells(row));
                }
                i += 1;
            }
            doc = doc.add_table(table(&rows)).add_paragraph(Paragraph::new());
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("![") {
            if let Some((caption, source)) = rest.split_once("](") {
                let source = source.trim_end_matches(')');
                let picture = resolve(source, caption).with_context(|| crate::i18n::fmt("image \"{0}\"", &[source.to_string()]))?;
                let img = image::load_from_memory(&picture.png).context(crate::i18n::tr("PNG image@@error-context"))?;
                let (mut w, mut h) = (img.width() * EMU_PER_PX, img.height() * EMU_PER_PX);
                if w > MAX_WIDTH_EMU {
                    h = (h as u64 * MAX_WIDTH_EMU as u64 / w as u64) as u32;
                    w = MAX_WIDTH_EMU;
                }
                let pic = Pic::new(&picture.png).size(w, h);
                doc = doc.add_paragraph(Paragraph::new().add_run(Run::new().add_image(pic)).align(AlignmentType::Center));
                if !picture.caption.trim().is_empty() {
                    doc = doc.add_paragraph(paragraph(picture.caption.trim(), Some("Caption"), false).align(AlignmentType::Center));
                }
                i += 1;
                continue;
            }
        }
        let p = if let Some(t) = trimmed.strip_prefix("### ") {
            paragraph(t, Some("Heading3"), false)
        } else if let Some(t) = trimmed.strip_prefix("## ") {
            paragraph(t, Some("Heading2"), false)
        } else if let Some(t) = trimmed.strip_prefix("# ") {
            paragraph(t, Some("Heading1"), false)
        } else if let Some(t) = trimmed.strip_prefix("- ").or_else(|| trimmed.strip_prefix("* ")) {
            let indent = (line.len() - line.trim_start().len()) as i32;
            paragraph(&format!("•  {t}"), None, false).indent(Some(360 + indent * 120), None, None, None)
        } else if trimmed.split_once(". ").is_some_and(|(n, _)| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit())) {
            paragraph(trimmed, None, false).indent(Some(360), None, None, None)
        } else if trimmed.is_empty() {
            i += 1;
            continue;
        } else {
            paragraph(trimmed, None, false)
        };
        doc = doc.add_paragraph(p);
        i += 1;
    }
    let mut out = std::io::Cursor::new(Vec::new());
    doc.build().pack(&mut out).context(crate::i18n::tr("saving DOCX"))?;
    Ok(out.into_inner())
}

#[cfg(test)]
mod tests {
    #[test]
    fn builds_document_with_table_list_and_picture() {
        let md = "# Description\n\nText with **bold words**.\n\n- item\n- second\n\n| UC | FR |\n|---|---|\n| UC-04 | FR-05 |\n\n![Matrix](image:test)\n";
        let png = crate::render::svg_to_png(r#"<svg xmlns="http://www.w3.org/2000/svg" width="40" height="20"><rect width="40" height="20" fill="red"/></svg>"#, 1.0).unwrap();
        let bytes = super::build("Document", md, |_, caption| Ok(super::Picture { png: png.clone(), caption: caption.into() })).unwrap();
        assert!(bytes.starts_with(b"PK"), "DOCX is a ZIP archive");
        let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
        let mut xml = String::new();
        std::io::Read::read_to_string(&mut zip.by_name("word/document.xml").unwrap(), &mut xml).unwrap();
        assert!(xml.contains("UC-04") && xml.contains("bold words") && xml.contains("Matrix"));
    }
}

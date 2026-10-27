//! Text of documents the assistant reads: PDF (every page), PowerPoint (slides with their notes), Visio (pages, shapes
//! and the connections between them), Excel (sheets as rows), Word, HTML and plain text.
//! Office files are ZIP archives of XML; every part is read with a size limit (a ZIP bomb must not exhaust the memory).

use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::io::Read;

/// Largest part of an Office file that is read.
const PART_LIMIT: u64 = 20 * 1024 * 1024;
/// Most characters of text returned to the model.
pub const TEXT_LIMIT: usize = 200_000;

fn part(archive: &mut zip::ZipArchive<std::io::Cursor<&[u8]>>, name: &str) -> Option<String> {
    let entry = archive.by_name(name).ok()?;
    let mut text = String::new();
    entry.take(PART_LIMIT).read_to_string(&mut text).ok()?;
    Some(text)
}

fn names(archive: &zip::ZipArchive<std::io::Cursor<&[u8]>>, prefix: &str) -> Vec<String> {
    let mut list: Vec<String> = archive.file_names().filter(|n| n.starts_with(prefix) && n.ends_with(".xml")).map(String::from).collect();
    // slide2.xml before slide10.xml.
    list.sort_by_key(|n| (n.trim_end_matches(".xml").chars().rev().take_while(|c| c.is_ascii_digit()).collect::<String>().chars().rev().collect::<String>().parse::<u32>().unwrap_or(0), n.clone()));
    list
}

fn unescape(text: &str) -> String {
    text.replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&apos;", "'").replace("&#10;", "\n").replace("&amp;", "&")
}

/// The text inside every `<tag …>…</tag>` of the XML (e.g. a:t), in order.
fn texts(xml: &str, tag: &str) -> Vec<String> {
    let open = format!("<{tag}");
    let close = format!("</{tag}>");
    let mut out = Vec::new();
    let mut rest = xml;
    while let Some(start) = rest.find(&open) {
        let after = &rest[start + open.len()..];
        // `<a:t>` or `<a:t xml:space="preserve">`, not `<a:tab>`.
        if !after.starts_with('>') && !after.starts_with(' ') {
            rest = after;
            continue;
        }
        let Some(gt) = after.find('>') else { break };
        if after[..gt].ends_with('/') {
            rest = &after[gt + 1..];
            continue;
        }
        let body = &after[gt + 1..];
        let Some(end) = body.find(&close) else { break };
        out.push(unescape(&body[..end]));
        rest = &body[end + close.len()..];
    }
    out
}

/// An attribute of the first tag in a piece of XML.
fn attr(tag: &str, name: &str) -> Option<String> {
    let key = format!(" {name}=\"");
    let start = tag.find(&key)? + key.len();
    let end = tag[start..].find('"')?;
    Some(unescape(&tag[start..start + end]))
}

/// Paragraph texts of DrawingML (PowerPoint): the runs of each <a:p> joined.
fn paragraphs(xml: &str) -> Vec<String> {
    xml.split("</a:p>").map(|p| texts(p, "a:t").join("")).map(|t| t.trim().to_string()).filter(|t| !t.is_empty()).collect()
}

fn pptx(bytes: &[u8]) -> Result<String> {
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).context("the PPTX is not a valid ZIP")?;
    let slides = names(&archive, "ppt/slides/slide");
    if slides.is_empty() {
        bail!("no slides in the PPTX");
    }
    let mut out = Vec::new();
    for (n, slide) in slides.iter().enumerate() {
        let xml = part(&mut archive, slide).unwrap_or_default();
        out.push(format!("--- Slide {} ---", n + 1));
        out.extend(paragraphs(&xml));
        let number = slide.trim_start_matches("ppt/slides/slide").trim_end_matches(".xml");
        if let Some(notes) = part(&mut archive, &format!("ppt/notesSlides/notesSlide{number}.xml")) {
            let lines: Vec<String> = paragraphs(&notes).into_iter().filter(|l| l.trim() != number).collect();
            if !lines.is_empty() {
                out.push(format!("Notes: {}", lines.join(" ")));
            }
        }
    }
    Ok(out.join("\n"))
}

fn vsdx(bytes: &[u8]) -> Result<String> {
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).context("the VSDX is not a valid ZIP")?;
    let index = part(&mut archive, "visio/pages/pages.xml").unwrap_or_default();
    let page_names: Vec<String> = index.split("<Page ").skip(1).map(|p| attr(&format!(" {p}"), "NameU").or_else(|| attr(&format!(" {p}"), "Name")).unwrap_or_default()).collect();
    let pages = names(&archive, "visio/pages/page");
    let pages: Vec<String> = pages.into_iter().filter(|p| p != "visio/pages/pages.xml").collect();
    if pages.is_empty() {
        bail!("no pages in the VSDX");
    }
    let mut out = Vec::new();
    for (n, page) in pages.iter().enumerate() {
        let xml = part(&mut archive, page).unwrap_or_default();
        out.push(format!("--- Page {}{} ---", n + 1, page_names.get(n).filter(|s| !s.is_empty()).map(|s| format!(": {s}")).unwrap_or_default()));
        // Shapes: id → (master name, text).
        let mut shapes: BTreeMap<String, (String, String)> = BTreeMap::new();
        for chunk in xml.split("<Shape ").skip(1) {
            let head = format!(" {}", &chunk[..chunk.find('>').unwrap_or(0)]);
            let Some(id) = attr(&head, "ID") else { continue };
            let name = attr(&head, "NameU").or_else(|| attr(&head, "Name")).unwrap_or_default();
            // The shape's own text (up to its first nested shape).
            let own = chunk.split("<Shape ").next().unwrap_or("");
            let text: String = texts(own, "Text").iter().map(|t| strip_tags(t)).collect::<Vec<_>>().join(" ").split_whitespace().collect::<Vec<_>>().join(" ");
            shapes.insert(id, (name, text));
        }
        // Connections: a connector shape glued at its begin and end.
        let mut ends: BTreeMap<String, (Option<String>, Option<String>)> = BTreeMap::new();
        for c in xml.split("<Connect ").skip(1) {
            let head = format!(" {}", &c[..c.find('>').unwrap_or(0)]);
            let (Some(from), Some(to), cell) = (attr(&head, "FromSheet"), attr(&head, "ToSheet"), attr(&head, "FromCell").unwrap_or_default()) else { continue };
            let entry = ends.entry(from).or_default();
            if cell.starts_with("Begin") {
                entry.0 = Some(to);
            } else {
                entry.1 = Some(to);
            }
        }
        let label = |id: &str| shapes.get(id).map(|(n, t)| if t.is_empty() { n.clone() } else { t.clone() }).unwrap_or_else(|| format!("#{id}"));
        for (id, (name, text)) in &shapes {
            if ends.contains_key(id) || text.is_empty() {
                continue;
            }
            out.push(format!("[{}] {text}", if name.is_empty() { "shape" } else { name.split('.').next().unwrap_or(name) }));
        }
        for (connector, (begin, end)) in &ends {
            if let (Some(b), Some(e)) = (begin, end) {
                let text = shapes.get(connector).map(|s| s.1.clone()).filter(|t| !t.is_empty()).map(|t| format!(" ({t})")).unwrap_or_default();
                out.push(format!("{} → {}{text}", label(b), label(e)));
            }
        }
    }
    Ok(out.join("\n"))
}

fn strip_tags(text: &str) -> String {
    let mut out = String::new();
    let mut inside = false;
    for c in text.chars() {
        match c {
            '<' => inside = true,
            '>' => inside = false,
            c if !inside => out.push(c),
            _ => {}
        }
    }
    out
}

fn xlsx(bytes: &[u8]) -> Result<String> {
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).context("the XLSX is not a valid ZIP")?;
    let shared: Vec<String> = part(&mut archive, "xl/sharedStrings.xml")
        .map(|xml| xml.split("</si>").map(|si| texts(si, "t").join("")).collect())
        .unwrap_or_default();
    let workbook = part(&mut archive, "xl/workbook.xml").unwrap_or_default();
    let sheet_names: Vec<String> = workbook.split("<sheet ").skip(1).map(|s| attr(&format!(" {s}"), "name").unwrap_or_default()).collect();
    let sheets = names(&archive, "xl/worksheets/sheet");
    let mut out = Vec::new();
    for (n, sheet) in sheets.iter().enumerate() {
        let xml = part(&mut archive, sheet).unwrap_or_default();
        out.push(format!("--- Sheet {} ---", sheet_names.get(n).cloned().unwrap_or_else(|| (n + 1).to_string())));
        for row in xml.split("</row>") {
            let mut cells = Vec::new();
            for c in row.split("<c ").skip(1) {
                let head = format!(" {}", &c[..c.find('>').unwrap_or(0)]);
                let reference = attr(&head, "r").unwrap_or_default();
                let kind = attr(&head, "t").unwrap_or_default();
                let value = match kind.as_str() {
                    "s" => texts(c, "v").first().and_then(|i| i.parse::<usize>().ok()).and_then(|i| shared.get(i).cloned()).unwrap_or_default(),
                    "inlineStr" => texts(c, "t").join(""),
                    _ => texts(c, "v").first().cloned().unwrap_or_default(),
                };
                if !value.trim().is_empty() {
                    cells.push(format!("{reference}: {}", value.trim()));
                }
            }
            if !cells.is_empty() {
                out.push(cells.join(" | "));
            }
        }
    }
    Ok(out.join("\n"))
}

fn pdf(bytes: &[u8]) -> Result<String> {
    // The PDF library can panic on damaged files: the reading is isolated.
    let owned = bytes.to_vec();
    let result = std::panic::catch_unwind(move || pdf_extract::extract_text_from_mem(&owned));
    match result {
        Ok(Ok(text)) if !text.trim().is_empty() => Ok(text),
        Ok(Ok(_)) => bail!("{}", crate::i18n::tr("the PDF has no text layer (a scan): attach it to the conversation instead, the model reads the pages as images")),
        Ok(Err(e)) => bail!("{}", crate::i18n::fmt("the PDF cannot be read: {0}", &[e.to_string()])),
        Err(_) => bail!("{}", crate::i18n::tr("the PDF cannot be read (damaged or unsupported)")),
    }
}

/// The text of a document by its extension.
pub fn text_of(name: &str, bytes: &[u8]) -> Result<String> {
    let ext = std::path::Path::new(name).extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    let text = match ext.as_str() {
        "pdf" => pdf(bytes)?,
        "pptx" | "pptm" => pptx(bytes)?,
        "vsdx" | "vsdm" => vsdx(bytes)?,
        "xlsx" | "xlsm" => xlsx(bytes)?,
        "docx" | "docm" => {
            let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).context("the DOCX is not a valid ZIP")?;
            let xml = part(&mut archive, "word/document.xml").context("word/document.xml is missing from the DOCX")?;
            xml.split("</w:p>").map(|p| texts(p, "w:t").join("")).collect::<Vec<_>>().join("\n")
        }
        "html" | "htm" => strip_tags(&String::from_utf8_lossy(bytes)),
        "ppt" | "vsd" | "xls" | "doc" => bail!("{}", crate::i18n::fmt("the old binary format .{0} cannot be read: save it as .{0}x", &[ext.clone()])),
        _ => match std::str::from_utf8(bytes) {
            Ok(text) => text.to_string(),
            Err(_) => bail!("{}", crate::i18n::fmt("binary file of type .{0}: its text cannot be read", &[ext.clone()])),
        },
    };
    Ok(text)
}

/// Cut to the limit with a note.
pub fn limited(text: String) -> String {
    if text.chars().count() <= TEXT_LIMIT {
        return text;
    }
    let cut: String = text.chars().take(TEXT_LIMIT).collect();
    format!("{cut}\n\n[… the document is longer: only the first {TEXT_LIMIT} characters]")
}

pub fn definition() -> Value {
    json!({
        "name": "read_document",
        "description": "Reads a document on this computer and returns its text: PDF (every page; a scan without a text layer cannot be read this way), PowerPoint .pptx (each slide with its notes), \
Visio .vsdx (each page: shapes with their text and the connections between shapes, A → B), Excel .xlsx (each sheet, cells with their values), Word .docx, HTML and text files. \
Use it when the user names a file to read (a specification, a presentation, a process drawn in Visio to model in EA). Files outside the attachments folder are asked for first.",
        "input_schema": {
            "type": "object",
            "properties": { "file": { "type": "string", "description": "Full path of the file." } },
            "required": ["file"]
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn office(files: &[(&str, &str)]) -> Vec<u8> {
        let mut buffer = std::io::Cursor::new(Vec::new());
        {
            let mut zip = zip::ZipWriter::new(&mut buffer);
            for (name, body) in files {
                zip.start_file(*name, zip::write::SimpleFileOptions::default()).unwrap();
                zip.write_all(body.as_bytes()).unwrap();
            }
            zip.finish().unwrap();
        }
        buffer.into_inner()
    }

    #[test]
    fn office_documents() {
        let p = office(&[
            ("ppt/slides/slide1.xml", r#"<p:sld><a:p><a:r><a:t>Order process</a:t></a:r></a:p><a:p><a:r><a:t>Customer </a:t></a:r><a:r><a:t xml:space="preserve">pays &amp; waits</a:t></a:r></a:p></p:sld>"#),
            ("ppt/slides/slide10.xml", "<p:sld><a:p><a:r><a:t>Last</a:t></a:r></a:p></p:sld>"),
            ("ppt/slides/slide2.xml", "<p:sld><a:p><a:r><a:t>Second</a:t></a:r></a:p></p:sld>"),
            ("ppt/notesSlides/notesSlide1.xml", "<p:notes><a:p><a:r><a:t>Say it slowly</a:t></a:r></a:p><a:p><a:r><a:t>1</a:t></a:r></a:p></p:notes>"),
        ]);
        let text = text_of("deck.pptx", &p).unwrap();
        assert!(text.contains("Customer pays & waits") && text.contains("Notes: Say it slowly"), "{text}");
        assert!(text.find("Second").unwrap() < text.find("Last").unwrap(), "slides in number order");

        let v = office(&[
            ("visio/pages/pages.xml", r#"<Pages><Page ID="0" NameU="Process" Name="Proces"/></Pages>"#),
            ("visio/pages/page1.xml", r#"<PageContents><Shapes><Shape ID="1" NameU="Process.1" Type="Shape"><Text>Take order</Text></Shape><Shape ID="2" NameU="Process.2"><Text>Ship</Text></Shape><Shape ID="3" NameU="Dynamic connector"><Text>then</Text></Shape></Shapes><Connects><Connect FromSheet="3" FromCell="BeginX" ToSheet="1"/><Connect FromSheet="3" FromCell="EndX" ToSheet="2"/></Connects></PageContents>"#),
        ]);
        let text = text_of("flow.vsdx", &v).unwrap();
        assert!(text.contains("Page 1: Process") && text.contains("[Process] Take order") && text.contains("Take order → Ship (then)"), "{text}");

        let x = office(&[
            ("xl/workbook.xml", r#"<workbook><sheets><sheet name="Requirements" sheetId="1"/></sheets></workbook>"#),
            ("xl/sharedStrings.xml", "<sst><si><t>W1</t></si><si><r><t>Customer </t></r><r><t>orders</t></r></si></sst>"),
            ("xl/worksheets/sheet1.xml", r#"<worksheet><sheetData><row r="1"><c r="A1" t="s"><v>0</v></c><c r="B1" t="s"><v>1</v></c><c r="C1"><v>42</v></c></row></sheetData></worksheet>"#),
        ]);
        let text = text_of("reqs.xlsx", &x).unwrap();
        assert!(text.contains("Sheet Requirements") && text.contains("A1: W1 | B1: Customer orders | C1: 42"), "{text}");
        assert!(text_of("old.doc", b"\xd0\xcf").is_err());
    }

    /// A real PDF: `LOONAR_TEST_PDF=… cargo test real_pdf -- --ignored --nocapture`
    #[test]
    #[ignore = "requires a PDF file"]
    fn real_pdf() {
        let path = std::env::var("LOONAR_TEST_PDF").unwrap();
        let text = text_of(&path, &std::fs::read(&path).unwrap()).unwrap();
        println!("{} characters
{}", text.chars().count(), text.chars().take(400).collect::<String>());
        assert!(text.chars().count() > 500);
    }
}

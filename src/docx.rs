//! Word documents (DOCX) assembled by the assistant from Markdown: headings, paragraphs with inline formatting
//! (bold, italic, code, strike-through, colours, highlight, links, changes), lists, tables, code blocks with their
//! indentation, quotes and images (diagrams from EA or images drawn earlier).
//!
//! Inline formatting: **bold**, *italic*, `code`, ~~strike~~, ==highlight==, [text](https://…) and a few HTML tags:
//! <span style="color:red; background:yellow">…</span>, <font color="…">, <b>, <i>, <u>, <mark>, <del> (red,
//! struck through) and <ins> (green, underlined) for marking changes.

use anyhow::{Context, Result};
use docx_rs::{
    AlignmentType, BorderType, Docx, Hyperlink, HyperlinkType, LineSpacing, Paragraph, Pic, Run, RunFonts, Shading, Style, StyleType,
    Table, TableBorders, TableCell, TableCellBorder, TableCellBorderPosition, TableCellMargins, TableRow, WidthType,
};

/// Image inserted into the document in place of the `![caption](source)` marker.
pub struct Picture {
    pub png: Vec<u8>,
    pub caption: String,
}

/// Page width for images: 16 cm (A4 with 2.5 cm margins), in EMU.
const MAX_WIDTH_EMU: u32 = 5_760_000;
const EMU_PER_PX: u32 = 9_525;
/// Colours of the document: headings and table headers (navy), code blocks (light grey), quotes (light blue).
const NAVY: &str = "1F3864";
const ACCENT: &str = "2E74B5";
const CODE_FILL: &str = "F3F4F6";
const QUOTE_FILL: &str = "EAF2FA";
const ROW_FILL: &str = "F7F9FC";
const LINK: &str = "0563C1";
const CODE_FONT: &str = "Consolas";

/// Formatting of a piece of text while the inline markup is read.
#[derive(Clone, Default)]
struct Format {
    bold: bool,
    italic: bool,
    underline: bool,
    strike: bool,
    code: bool,
    color: Option<String>,
    fill: Option<String>,
}

/// A piece of a paragraph: text with its formatting, or a link.
enum Piece {
    Text(String, Format),
    Link(String, String, Format),
}

/// A colour from markup: a name (red, green…) or a hex value (#C00000, C00000, #c00).
fn colour(value: &str) -> Option<String> {
    let v = value.trim().trim_matches(|c| c == '"' || c == '\'').to_lowercase();
    let named = match v.as_str() {
        "red" | "czerwony" => "C00000",
        "darkred" => "8B0000",
        "green" | "zielony" => "00843D",
        "blue" | "niebieski" => "0070C0",
        "navy" => NAVY,
        "orange" | "pomarańczowy" => "E36C09",
        "purple" | "fioletowy" => "7030A0",
        "gray" | "grey" | "szary" => "7F7F7F",
        "black" => "000000",
        "white" => "FFFFFF",
        "yellow" | "żółty" => "FFF2A8",
        "lightgreen" => "E2F0D9",
        "lightblue" => "DEEBF7",
        "lightred" | "pink" => "FBE4E4",
        _ => "",
    };
    if !named.is_empty() {
        return Some(named.to_string());
    }
    let hex = v.trim_start_matches('#');
    match hex.len() {
        6 if hex.chars().all(|c| c.is_ascii_hexdigit()) => Some(hex.to_uppercase()),
        3 if hex.chars().all(|c| c.is_ascii_hexdigit()) => Some(hex.chars().flat_map(|c| [c, c]).collect::<String>().to_uppercase()),
        _ => None,
    }
}

/// The formatting an opening HTML tag adds (None: not a tag this module knows).
fn tag_format(tag: &str, mut f: Format) -> Option<Format> {
    let lower = tag.to_lowercase();
    let name = lower.split(|c: char| c.is_whitespace() || c == '>').next().unwrap_or("");
    match name {
        "b" | "strong" => f.bold = true,
        "i" | "em" => f.italic = true,
        "u" => f.underline = true,
        "s" | "strike" => f.strike = true,
        "code" => f.code = true,
        "mark" => f.fill = Some("FFF2A8".into()),
        "del" => {
            f.strike = true;
            f.color = Some("C00000".into());
        }
        "ins" => {
            f.underline = true;
            f.color = Some("00843D".into());
        }
        "span" | "font" => {
            // style="color: red; background: yellow" or color="red".
            let attrs = &tag[name.len()..];
            for (key, value) in attrs.split(';').flat_map(|s| s.split(['"', '\''])).filter_map(|s| s.split_once(':').or_else(|| s.split_once('='))) {
                let key = key.trim().trim_start_matches("style").trim_start_matches('=').trim().to_lowercase();
                let key = key.rsplit(' ').next().unwrap_or("").to_string();
                match key.as_str() {
                    "color" => f.color = colour(value).or(f.color),
                    "background" | "background-color" => f.fill = colour(value).or(f.fill),
                    "font-weight" if value.contains("bold") => f.bold = true,
                    "font-style" if value.contains("italic") => f.italic = true,
                    _ => {}
                }
            }
            if name == "font" {
                if let Some(c) = attrs.split("color=").nth(1).and_then(|v| v.split(['"', '\'']).find(|s| !s.trim().is_empty())) {
                    f.color = colour(c).or(f.color);
                }
            }
        }
        _ => return None,
    }
    Some(f)
}

/// Reads the inline markup of one line into pieces.
fn pieces(text: &str, base: Format) -> Vec<Piece> {
    let mut out = Vec::new();
    let mut stack: Vec<(String, Format)> = Vec::new();
    let mut f = base;
    let mut buf = String::new();
    let flush = |buf: &mut String, f: &Format, out: &mut Vec<Piece>| {
        if !buf.is_empty() {
            out.push(Piece::Text(std::mem::take(buf), f.clone()));
        }
    };
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    let starts = |i: usize, s: &str| chars[i..].iter().take(s.chars().count()).copied().eq(s.chars());
    while i < chars.len() {
        let c = chars[i];
        // `code`: everything up to the closing backtick as it is.
        if c == '`' && !f.code {
            if let Some(end) = chars[i + 1..].iter().position(|&x| x == '`') {
                flush(&mut buf, &f, &mut out);
                let code: String = chars[i + 1..i + 1 + end].iter().collect();
                out.push(Piece::Text(code, Format { code: true, ..f.clone() }));
                i += end + 2;
                continue;
            }
        }
        // [text](https://…)
        if c == '[' {
            let rest: String = chars[i..].iter().collect();
            if let Some((label, after)) = rest[1..].split_once("](") {
                if let Some((url, _)) = after.split_once(')') {
                    if !label.contains(']') && (url.starts_with("http://") || url.starts_with("https://") || url.starts_with("mailto:")) {
                        flush(&mut buf, &f, &mut out);
                        out.push(Piece::Link(label.to_string(), url.to_string(), f.clone()));
                        i += 1 + label.chars().count() + 2 + url.chars().count() + 1;
                        continue;
                    }
                }
            }
        }
        // Paired markers.
        let mut toggled = false;
        for (marker, kind) in [("**", 0), ("~~", 1), ("==", 2), ("__", 0)] {
            if starts(i, marker) {
                flush(&mut buf, &f, &mut out);
                match kind {
                    0 => f.bold = !f.bold,
                    1 => f.strike = !f.strike,
                    _ => f.fill = if f.fill.is_some() { None } else { Some("FFF2A8".into()) },
                }
                i += marker.len();
                toggled = true;
                break;
            }
        }
        if toggled {
            continue;
        }
        // *italic* (a single star next to text, not a list marker or a multiplication with spaces).
        let word_edge = |j: usize| j >= chars.len() || !chars[j].is_alphanumeric();
        let opening = !f.italic && chars.get(i + 1).is_some_and(|n| !n.is_whitespace()) && (c == '*' || (i == 0 || word_edge(i - 1)));
        let closing = f.italic && i > 0 && !chars[i - 1].is_whitespace() && (c == '*' || word_edge(i + 1));
        if (c == '*' || c == '_') && (opening || closing) {
            flush(&mut buf, &f, &mut out);
            f.italic = !f.italic;
            i += 1;
            continue;
        }
        // HTML-like tags.
        if c == '<' {
            if let Some(end) = chars[i..].iter().position(|&x| x == '>') {
                let tag: String = chars[i + 1..i + end].iter().collect();
                if let Some(name) = tag.strip_prefix('/') {
                    let name = name.trim().to_lowercase();
                    if let Some(pos) = stack.iter().rposition(|(n, _)| *n == name) {
                        flush(&mut buf, &f, &mut out);
                        f = stack[pos].1.clone();
                        stack.truncate(pos);
                        i += end + 1;
                        continue;
                    }
                } else if tag.trim().eq_ignore_ascii_case("br") || tag.trim().eq_ignore_ascii_case("br/") {
                    buf.push('\n');
                    i += end + 1;
                    continue;
                } else if let Some(next) = tag_format(&tag, f.clone()) {
                    flush(&mut buf, &f, &mut out);
                    let name = tag.split(|c: char| c.is_whitespace()).next().unwrap_or("").to_lowercase();
                    stack.push((name, f.clone()));
                    f = next;
                    i += end + 1;
                    continue;
                }
            }
        }
        buf.push(c);
        i += 1;
    }
    flush(&mut buf, &f, &mut out);
    out
}

fn run(text: &str, f: &Format) -> Run {
    let mut r = Run::new();
    for (n, line) in text.split('\n').enumerate() {
        if n > 0 {
            r = r.add_break(docx_rs::BreakType::TextWrapping);
        }
        r = r.add_text(line);
    }
    if f.bold {
        r = r.bold();
    }
    if f.italic {
        r = r.italic();
    }
    if f.underline {
        r = r.underline("single");
    }
    if f.strike {
        r = r.strike();
    }
    if f.code {
        r = r.fonts(RunFonts::new().ascii(CODE_FONT).hi_ansi(CODE_FONT).cs(CODE_FONT)).size(19).color(f.color.clone().unwrap_or_else(|| "B4235A".into()));
        if f.fill.is_none() {
            r = r.shading(Shading::new().fill(CODE_FILL));
        }
    }
    if let Some(c) = &f.color {
        r = r.color(c.clone());
    }
    if let Some(fill) = &f.fill {
        r = r.shading(Shading::new().fill(fill.clone()));
    }
    r
}

fn add_pieces(mut p: Paragraph, text: &str, base: Format) -> Paragraph {
    for piece in pieces(text, base) {
        p = match piece {
            Piece::Text(t, f) => p.add_run(run(&t, &f)),
            Piece::Link(label, url, f) => {
                let f = Format { underline: true, color: Some(f.color.clone().unwrap_or_else(|| LINK.into())), ..f };
                p.add_hyperlink(Hyperlink::new(url, HyperlinkType::External).add_run(run(&label, &f)))
            }
        };
    }
    p
}

fn paragraph(text: &str, style: Option<&str>, bold: bool) -> Paragraph {
    let p = add_pieces(Paragraph::new(), text, Format { bold, ..Format::default() });
    match style {
        Some(s) => p.style(s),
        None => p,
    }
}

/// A one-cell table used as a box (code blocks, quotes): background colour and a coloured left edge.
fn boxed(paragraphs: Vec<Paragraph>, fill: &str, edge: &str) -> Table {
    let mut cell = TableCell::new().shading(Shading::new().fill(fill)).set_borders(docx_rs::TableCellBorders::with_empty());
    cell = cell.set_border(TableCellBorder::new(TableCellBorderPosition::Left).border_type(BorderType::Single).size(18).color(edge));
    for p in paragraphs {
        cell = cell.add_paragraph(p);
    }
    Table::new(vec![TableRow::new(vec![cell])]).set_borders(TableBorders::new().clear_all()).width(5000, WidthType::Pct).margins(padding(100))
}

/// Colours of a JSON line (keys, strings, numbers, true/false/null), so that a request or response reads like in Postman.
fn json_runs(line: &str) -> Vec<Run> {
    let font = || RunFonts::new().ascii(CODE_FONT).hi_ansi(CODE_FONT).cs(CODE_FONT);
    let mut out = Vec::new();
    let chars: Vec<char> = line.chars().collect();
    let mut i = 0;
    let mut plain = String::new();
    let push = |text: String, color: &str, out: &mut Vec<Run>| {
        if !text.is_empty() {
            out.push(Run::new().add_text(text).fonts(font()).size(19).color(color));
        }
    };
    while i < chars.len() {
        let c = chars[i];
        if c == '"' {
            push(std::mem::take(&mut plain), "1F2328", &mut out);
            let mut j = i + 1;
            while j < chars.len() && chars[j] != '"' {
                if chars[j] == '\\' {
                    j += 1;
                }
                j += 1;
            }
            let s: String = chars[i..(j + 1).min(chars.len())].iter().collect();
            let is_key = chars[(j + 1).min(chars.len())..].iter().find(|c| !c.is_whitespace()) == Some(&':');
            push(s, if is_key { "A31515" } else { "0451A5" }, &mut out);
            i = j + 1;
            continue;
        }
        if c.is_ascii_digit() || (c == '-' && chars.get(i + 1).is_some_and(|n| n.is_ascii_digit())) {
            push(std::mem::take(&mut plain), "1F2328", &mut out);
            let mut j = i + 1;
            while j < chars.len() && (chars[j].is_ascii_digit() || matches!(chars[j], '.' | 'e' | 'E' | '+' | '-')) {
                j += 1;
            }
            push(chars[i..j].iter().collect(), "098658", &mut out);
            i = j;
            continue;
        }
        let rest: String = chars[i..].iter().take(5).collect();
        if let Some(word) = ["true", "false", "null"].into_iter().find(|w| rest.starts_with(w)) {
            push(std::mem::take(&mut plain), "1F2328", &mut out);
            push(word.into(), "0000FF", &mut out);
            i += word.len();
            continue;
        }
        plain.push(c);
        i += 1;
    }
    push(plain, "1F2328", &mut out);
    out
}

/// A code block: every line as it is (indentation kept), monospaced, in a grey box; JSON in colour. An HTTP request
/// (GET /path, headers) keeps its first line bold, as in Postman.
fn code_block(lines: &[&str], lang: &str) -> Table {
    let json = matches!(lang, "json" | "jsonc") || (lang.is_empty() && lines.iter().find(|l| !l.trim().is_empty()).is_some_and(|l| l.trim_start().starts_with(['{', '['])));
    let http_verb = |l: &str| ["GET ", "POST ", "PUT ", "PATCH ", "DELETE ", "HEAD ", "OPTIONS "].iter().any(|v| l.trim_start().starts_with(v));
    let spacing = LineSpacing::new().before(0).after(0).line(240);
    let font = RunFonts::new().ascii(CODE_FONT).hi_ansi(CODE_FONT).cs(CODE_FONT);
    let mut paragraphs: Vec<Paragraph> = lines
        .iter()
        .map(|line| {
            let line = line.replace('\t', "    ");
            let mut p = Paragraph::new().line_spacing(spacing.clone());
            if line.is_empty() {
                return p.add_run(Run::new().add_text(" ").fonts(font.clone()).size(19));
            }
            if json || (line.trim_start().starts_with(['{', '[', '"', '}', ']'])) {
                for r in json_runs(&line) {
                    p = p.add_run(r);
                }
            } else {
                let mut r = Run::new().add_text(line.clone()).fonts(font.clone()).size(19).color("1F2328");
                if http_verb(&line) {
                    r = r.bold().color(ACCENT);
                }
                p = p.add_run(r);
            }
            p
        })
        .collect();
    if !lang.is_empty() {
        paragraphs.insert(0, Paragraph::new().line_spacing(LineSpacing::new().before(0).after(60)).add_run(Run::new().add_text(lang.to_uppercase()).size(16).bold().color("7F7F7F")));
    }
    boxed(paragraphs, CODE_FILL, "A6A6A6")
}

fn table(rows: &[Vec<String>]) -> Table {
    let rows = rows
        .iter()
        .enumerate()
        .map(|(i, cells)| {
            TableRow::new(
                cells
                    .iter()
                    .map(|c| {
                        let cell = if i == 0 {
                            TableCell::new()
                                .shading(Shading::new().fill(NAVY))
                                .add_paragraph(add_pieces(Paragraph::new(), c, Format { bold: true, color: Some("FFFFFF".into()), ..Format::default() }))
                        } else {
                            let cell = TableCell::new().add_paragraph(paragraph(c, None, false));
                            if i % 2 == 0 {
                                cell.shading(Shading::new().fill(ROW_FILL))
                            } else {
                                cell
                            }
                        };
                        cell
                    })
                    .collect(),
            )
        })
        .collect();
    Table::new(rows)
        .set_borders(TableBorders::new().set(docx_rs::TableBorder::new(docx_rs::TableBorderPosition::InsideH).color("D9D9D9")))
        .width(5000, WidthType::Pct)
        .margins(padding(50))
}

/// Space inside table cells: `v` above and below, more on the sides (twentieths of a point).
fn padding(v: usize) -> TableCellMargins {
    TableCellMargins::new().margin_top(v, WidthType::Dxa).margin_bottom(v, WidthType::Dxa).margin_left(140, WidthType::Dxa).margin_right(140, WidthType::Dxa)
}

fn cells(line: &str) -> Vec<String> {
    line.trim().trim_matches('|').split('|').map(|c| c.trim().to_string()).collect()
}

/// Builds the document. `resolve` turns an image source from the Markdown (e.g. `diagram:12`, `image:file.png`) into an image.
pub fn build(title: &str, markdown: &str, mut resolve: impl FnMut(&str, &str) -> Result<Picture>) -> Result<Vec<u8>> {
    // Control characters are not allowed in XML 1.0: one in the text made the file unreadable for Word (LC-360).
    let clean = |text: &str| text.chars().filter(|c| !c.is_control() || matches!(c, '\n' | '\t' | '\r')).collect::<String>();
    let (title, markdown) = (clean(title), clean(markdown));
    let (title, markdown) = (title.as_str(), markdown.as_str());
    let heading = |id: &str, name: &str, size: usize, color: &str, before: u32| {
        Style::new(id, StyleType::Paragraph).name(name).size(size).bold().color(color).line_spacing(LineSpacing::new().before(before).after(80))
    };
    // The real time of writing (LC-465: the file said 1970-01-01, so a document could not be told older than the model).
    let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
    let mut doc = Docx::new()
        .created_at(&now)
        .updated_at(&now)
        .default_fonts(RunFonts::new().ascii("Calibri").hi_ansi("Calibri").cs("Calibri").east_asia("Calibri"))
        .default_size(22)
        .add_style(Style::new("Title", StyleType::Paragraph).name("Title").size(52).bold().color(NAVY).line_spacing(LineSpacing::new().after(240)))
        .add_style(heading("Heading1", "Heading 1", 32, NAVY, 360))
        .add_style(heading("Heading2", "Heading 2", 28, ACCENT, 280))
        .add_style(heading("Heading3", "Heading 3", 24, ACCENT, 220))
        .add_style(heading("Heading4", "Heading 4", 22, "404040", 180))
        .add_style(Style::new("Caption", StyleType::Paragraph).name("Caption").size(18).italic().color("595959"));
    if !title.trim().is_empty() {
        doc = doc.add_paragraph(paragraph(title.trim(), Some("Title"), false));
    }
    let lines: Vec<&str> = markdown.lines().collect();
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i].trim_end();
        let trimmed = line.trim();
        // ```lang … ``` code block: lines as they are.
        if let Some(lang) = trimmed.strip_prefix("```").or_else(|| trimmed.strip_prefix("~~~")) {
            let fence = &trimmed[..3];
            let start = i + 1;
            let mut end = start;
            while end < lines.len() && !lines[end].trim().starts_with(fence) {
                end += 1;
            }
            // The block's own indentation (e.g. inside a list) is removed, the code's indentation stays.
            let block: Vec<&str> = lines[start..end].iter().map(|l| l.trim_end()).collect();
            let common = block.iter().filter(|l| !l.trim().is_empty()).map(|l| l.len() - l.trim_start().len()).min().unwrap_or(0);
            let block: Vec<&str> = block.iter().map(|l| if l.len() >= common { &l[common..] } else { l.trim_start() }).collect();
            doc = doc.add_table(code_block(&block, lang.trim().to_lowercase().as_str())).add_paragraph(Paragraph::new());
            i = end + 1;
            continue;
        }
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
        if trimmed.starts_with('>') {
            // Quote or note: consecutive > lines in a light blue box.
            let mut quote = Vec::new();
            while i < lines.len() && lines[i].trim().starts_with('>') {
                let text = lines[i].trim().trim_start_matches('>').trim();
                quote.push(add_pieces(Paragraph::new().line_spacing(LineSpacing::new().before(40).after(40)), text, Format::default()));
                i += 1;
            }
            doc = doc.add_table(boxed(quote, QUOTE_FILL, ACCENT)).add_paragraph(Paragraph::new());
            continue;
        }
        if matches!(trimmed, "---" | "***" | "___") {
            doc = doc.add_paragraph(Paragraph::new().add_run(Run::new().add_text("─".repeat(60)).color("BFBFBF")).align(AlignmentType::Center));
            i += 1;
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
        let indent = (line.len() - line.trim_start().len()) as i32;
        let p = if let Some(t) = trimmed.strip_prefix("#### ") {
            paragraph(t, Some("Heading4"), false)
        } else if let Some(t) = trimmed.strip_prefix("### ") {
            paragraph(t, Some("Heading3"), false)
        } else if let Some(t) = trimmed.strip_prefix("## ") {
            paragraph(t, Some("Heading2"), false)
        } else if let Some(t) = trimmed.strip_prefix("# ") {
            paragraph(t, Some("Heading1"), false)
        } else if let Some(t) = trimmed.strip_prefix("- [ ] ").or_else(|| trimmed.strip_prefix("* [ ] ")) {
            paragraph(&format!("☐  {t}"), None, false).indent(Some(360 + indent * 120), None, None, None)
        } else if let Some(t) = trimmed.strip_prefix("- [x] ").or_else(|| trimmed.strip_prefix("- [X] ")) {
            paragraph(&format!("☑  {t}"), None, false).indent(Some(360 + indent * 120), None, None, None)
        } else if let Some(t) = trimmed.strip_prefix("- ").or_else(|| trimmed.strip_prefix("* ")).or_else(|| trimmed.strip_prefix("+ ")) {
            paragraph(&format!("•  {t}"), None, false).indent(Some(360 + indent * 120), None, None, None)
        } else if trimmed.split_once(". ").is_some_and(|(n, _)| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit())) {
            paragraph(trimmed, None, false).indent(Some(360 + indent * 120), None, None, None)
        } else if trimmed.is_empty() {
            i += 1;
            continue;
        } else {
            paragraph(trimmed, None, false).line_spacing(LineSpacing::new().after(120))
        };
        doc = doc.add_paragraph(p);
        i += 1;
    }
    let mut out = std::io::Cursor::new(Vec::new());
    doc.build().pack(&mut out).context(crate::i18n::tr("saving DOCX"))?;
    with_author(out.into_inner())
}

/// The document's author: the Windows user and Loonar Compass instead of "unknown" (LC-465), which the DOCX library
/// writes and cannot be told otherwise.
fn with_author(bytes: Vec<u8>) -> Result<Vec<u8>> {
    use std::io::{Read, Write};
    let user = std::env::var("USERNAME").unwrap_or_default();
    let escape = |t: &str| t.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;");
    let creator = if user.trim().is_empty() { "Loonar Compass".to_string() } else { escape(user.trim()) };
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes))?;
    let mut out = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i)?;
        let name = entry.name().to_string();
        let mut data = Vec::new();
        entry.read_to_end(&mut data)?;
        if name == "docProps/core.xml" {
            let text = String::from_utf8_lossy(&data)
                .replace("<dc:creator>unknown</dc:creator>", &format!("<dc:creator>{creator}</dc:creator>"))
                .replace("<cp:lastModifiedBy>unknown</cp:lastModifiedBy>", "<cp:lastModifiedBy>Loonar Compass</cp:lastModifiedBy>");
            data = text.into_bytes();
        }
        out.start_file(name, zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated))?;
        out.write_all(&data)?;
    }
    Ok(out.finish()?.into_inner())
}

#[cfg(test)]
mod tests {
    fn xml_of(md: &str) -> String {
        let png = crate::render::svg_to_png(r#"<svg xmlns="http://www.w3.org/2000/svg" width="40" height="20"><rect width="40" height="20" fill="red"/></svg>"#, 1.0).unwrap();
        let bytes = super::build("Document", md, |_, caption| Ok(super::Picture { png: png.clone(), caption: caption.into() })).unwrap();
        assert!(bytes.starts_with(b"PK"), "DOCX is a ZIP archive");
        let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
        let mut xml = String::new();
        std::io::Read::read_to_string(&mut zip.by_name("word/document.xml").unwrap(), &mut xml).unwrap();
        xml
    }

    #[test]
    fn builds_document_with_table_list_and_picture() {
        let xml = xml_of("# Description\n\nText with **bold words**.\n\n- item\n- second\n\n| UC | FR |\n|---|---|\n| UC-04 | FR-05 |\n\n![Matrix](image:test)\n");
        assert!(xml.contains("UC-04") && xml.contains("bold words") && xml.contains("Matrix"));
    }

    /// LC-465: the document has the time it was written, not 1970.
    #[test]
    fn document_has_its_date() {
        let bytes = super::build("T", "x", |_, _| anyhow::bail!("no pictures")).unwrap();
        let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
        let mut core = String::new();
        std::io::Read::read_to_string(&mut zip.by_name("docProps/core.xml").unwrap(), &mut core).unwrap();
        assert!(core.contains(&chrono::Utc::now().format("%Y-%m-%d").to_string()) && !core.contains("1970"), "{core}");
        assert!(!core.contains(">unknown<"), "{core}");
        let bytes = super::build("T", "x", |_, _| anyhow::bail!("no pictures")).unwrap();
        assert!(crate::template_doctor::saved_at(&bytes).is_some(), "LC-383: the date is read back");
        assert!(crate::documents::text_of("t.docx", &bytes).unwrap().contains('x'), "still a readable document");
    }

    /// LC-360: control characters do not reach document.xml.
    #[test]
    fn control_characters_left_out() {
        let xml = xml_of("Tekst\u{0}z\u{8}znakami\u{1b}sterującymi");
        assert!(xml.contains("Tekstzznakamisterującymi"), "{xml}");
        assert!(!xml.chars().any(|c| c.is_control() && !matches!(c, '\n' | '\t' | '\r')));
    }

    #[test]
    fn code_keeps_indentation_and_colours() {
        let md = "## Request\n\n```http\nPOST /api/orders HTTP/1.1\nContent-Type: application/json\n```\n\n```json\n{\n    \"customer\": \"Anna\",\n    \"items\": [\n        { \"id\": 7, \"paid\": true }\n    ]\n}\n```\n";
        let xml = xml_of(md);
        // The indentation of the JSON is kept (spaces preserved), the font is monospaced and keys have their colour.
        assert!(xml.contains(r#"xml:space="preserve">    </w:t>"#) || xml.contains(r#"xml:space="preserve">        "#), "{xml}");
        assert!(xml.contains("Consolas") && xml.contains("A31515") && xml.contains("POST /api/orders"));
        assert!(!xml.contains("```"));
    }

    #[test]
    fn colours_changes_and_links() {
        let xml = xml_of("Changed: <del>old value</del> <ins>new value</ins>, <span style=\"color:red\">in red</span>, ==marked==, *italic*, `code` and [Loonar](https://loonar.eu).\n\n> Note: keep it.\n");
        assert!(xml.contains("C00000") && xml.contains("00843D") && xml.contains("<w:strike"), "{xml}");
        assert!(xml.contains("FFF2A8") && xml.contains("<w:i ") && xml.contains("hyperlink"));
        assert!(xml.contains("in red") && !xml.contains("&lt;span") && !xml.contains("==marked"));
    }

    /// Writes a sample with every kind of formatting to LOONAR_DOCX_SAMPLE (a .docx path) for a look in Word.
    #[test]
    #[ignore = "writes a file for a manual look"]
    fn sample_document() {
        let md = "# REST API – zmiany w zamówieniach\n\nWersja **2.1**, zmiany oznaczone kolorem: <del>stare pole `amount`</del> <ins>nowe pole `total`</ins>.\n\n> Uwaga: pola oznaczone <span style=\"color:red\">na czerwono</span> są nowe w tej wersji.\n\n## Żądanie\n\n```http\nPOST https://api.example.com/v2/orders\nContent-Type: application/json\nAuthorization: Bearer {{token}}\n```\n\n```json\n{\n    \"customer\": { \"id\": 42, \"name\": \"Anna Nowak\" },\n    \"items\": [\n        { \"sku\": \"A-100\", \"qty\": 2, \"price\": 19.99 },\n        { \"sku\": \"B-200\", \"qty\": 1, \"gift\": true }\n    ],\n    \"coupon\": null\n}\n```\n\n## Pola\n\n| Pole | Typ | Opis |\n|---|---|---|\n| customer.id | integer | Identyfikator klienta |\n| <span style=\"color:red\">total</span> | number | ==Nowe:== suma zamówienia |\n| items | array | Pozycje |\n\n### Do zrobienia\n\n- [x] *Opis* pól\n- [ ] Przykłady błędów\n  - kod 400\n  - kod 409\n\nWięcej: [dokumentacja](https://loonar.eu/compass/).\n";
        let bytes = super::build("Specyfikacja API", md, |_, _| anyhow::bail!("no pictures")).unwrap();
        std::fs::write(std::env::var("LOONAR_DOCX_SAMPLE").unwrap(), bytes).unwrap();
    }
}

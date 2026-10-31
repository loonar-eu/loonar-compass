//! Template doctor (LC-383, /template-doctor): why an element is missing from a document EA generated – the package
//! the document was made from, the filters and sections of the EA template (t_document, SSDOCSTYLE), the element's
//! status, phase, notes and dates – and which elements of a package a DOCX does not mention. Also lists the model's
//! templates and explains one in plain words (sections, fields, filters). Reads only. LC-423: an EA template turned into a
//! Compass Markdown template, the heading styles of a DOCX checked, and the reviewers' Word comments matched to elements
//! (as Note elements linked to them, saved through ea_write).

use crate::config::{AppConfig, ModelSource};
use crate::tools::query_rows;
use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use std::collections::{BTreeSet, HashMap};

pub fn definition() -> Value {
    json!({
        "name": "template_doctor",
        "description": "EA document templates and generated documents. action \"list\": the templates kept in the model (t_document). \
\"explain\": a template in plain words – its sections (package, element, diagram, child elements…), the fields it prints and its \
filters (excluded element types, status, phase, elements without notes, dates). \"why_missing\": why an element is not in a document \
generated from a package – checks the package, the template's filters and sections and the element itself, and with `document` (a DOCX \
attached to the conversation) whether its name is there and whether the element is newer than the document. \"compare\": the elements of \
a package (with child packages) that a DOCX does not mention, each with the likely reason. \"to_markdown\": an EA template as a Compass \
template (Markdown: headings for its sections, the model fields in braces, its filters as hints) to save with save_template. \
\"check_headings\": whether a DOCX uses the Heading 1…6 styles (Compass and EA find the chapters only by them): paragraphs that look like \
headings but are not styled, skipped levels. \"comments\": the reviewers' comments of a DOCX matched to the elements they are about (the \
commented text, else the heading above it) with the ea_write ops that add each as a Note linked to its element; show the list and save only \
after the user agrees. Reads only. Use it for /template-doctor.",
        "input_schema": { "type": "object", "properties": {
            "action": { "type": "string", "enum": ["list", "explain", "why_missing", "compare", "to_markdown", "check_headings", "comments"] },
            "template": { "type": "string", "description": "Name of the EA template (optional for why_missing and compare)" },
            "package": { "description": "package_id or exact name the document was generated from" },
            "element": { "description": "object_id or exact name of the missing element (why_missing)" },
            "document": { "type": "string", "description": "File name of a DOCX attached to the conversation (compare, check_headings, comments; optional for why_missing)" }
        }, "required": ["action"] }
    })
}

pub fn run(input: &Value, cfg: &AppConfig) -> Result<String> {
    match input["action"].as_str().unwrap_or("list") {
        "list" => list(cfg),
        "explain" => {
            let name = input["template"].as_str().map(str::trim).filter(|t| !t.is_empty()).context("template: the name of the template")?;
            Ok(Template::load(cfg, name)?.explain())
        }
        "why_missing" => why_missing(input, cfg),
        "compare" => compare(input, cfg),
        "to_markdown" => {
            let name = input["template"].as_str().map(str::trim).filter(|t| !t.is_empty()).context("template: the name of the template")?;
            Ok(Template::load(cfg, name)?.markdown())
        }
        "check_headings" => Ok(check_headings(&docx_xml(cfg, input)?.0)),
        "comments" => comments(input, cfg),
        other => bail!("unknown action {other}: list, explain, why_missing, compare, to_markdown, check_headings or comments"),
    }
}

fn list(cfg: &AppConfig) -> Result<String> {
    let rows = query_rows(cfg, "select docname as c0, coalesce(author,'') as c1, coalesce(notes,'') as c2 from t_document where doctype = 'SSDOCSTYLE' order by docname")?;
    if rows.is_empty() {
        return Ok("The model keeps no templates of its own (EA's built-in templates are not stored in the model).".into());
    }
    let mut out = vec![format!("Templates in the model ({}); EA's built-in templates are not stored in the model:", rows.len())];
    out.extend(rows.iter().map(|r| format!("- {}{}", r[0], if r[1].is_empty() { String::new() } else { format!(" (author {})", r[1]) })));
    Ok(out.join("\n"))
}

/// A template read from the model: its options, the sections its body uses and the fields it prints.
struct Template {
    name: String,
    options: HashMap<String, String>,
    sections: Vec<String>,
    fields: BTreeSet<String>,
    /// The body as plain text (section markers, fields and the text around them).
    text: String,
}

impl Template {
    fn load(cfg: &AppConfig, name: &str) -> Result<Template> {
        let bytes = template_bytes(cfg, name)?;
        Self::parse(name, &bytes)
    }

    /// The stored template: a zip with str.dat (UTF-16 SSDocument XML whose body is base64).
    fn parse(name: &str, bytes: &[u8]) -> Result<Template> {
        use base64::Engine;
        use std::io::Read;
        let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).context("the template is not a zip as EA keeps it")?;
        let mut raw = Vec::new();
        zip.by_name("str.dat").context("str.dat is missing in the template")?.read_to_end(&mut raw)?;
        let xml = if raw.get(1) != Some(&0) { String::from_utf8_lossy(&raw).to_string() } else { String::from_utf16_lossy(&raw.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect::<Vec<_>>()) };
        let options_tag = between(&xml, "<SSDocument.Options", ">").unwrap_or_default();
        let options = attributes(&options_tag);
        let body64: String = between(&xml, "<SSDocument.Document", "</SSDocument.Document>")
            .and_then(|b| b.split_once('>').map(|(_, rest)| rest.to_string()))
            .unwrap_or_default()
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect();
        let body = base64::engine::general_purpose::STANDARD.decode(body64.as_bytes()).map(|b| b.iter().map(|&c| c as char).collect::<String>()).unwrap_or_default();
        let text = if body.contains("TeCfMt") { tx_text(&body) } else { plain(&body) };
        // The sections the body uses: their start markers ("element >") are in the text.
        let mut sections = Vec::new();
        for tag in xml.split("<SSDocument.Section ").skip(1) {
            let a = attributes(tag.split('>').next().unwrap_or(""));
            let marker = a.get("markerstart").map(|m| m.replace("&gt;", ">").replace("&lt;", "<")).unwrap_or_default();
            let word = marker.trim_end_matches(" >").to_string();
            if !marker.is_empty() && text.contains(&marker) && !sections.contains(&word) {
                sections.push(word);
            }
        }
        // Fields: EA writes them as {Element.Name} in the body, Compass's own templates as \{Element.Name\}.
        let fields = fields(&body.replace("\\{", "{").replace("\\}", "}"));
        Ok(Template { name: name.to_string(), options, sections, fields, text })
    }

    /// The template as a Compass template (LC-423): a heading for each section that prints a name, the other lines of the
    /// body as they are, fields kept in braces, and its sections and filters as hints for whoever fills it in.
    fn markdown(&self) -> String {
        let mut out = vec![
            format!("> Template: converted from the EA template \"{}\" (sections: {}).", self.name, if self.sections.is_empty() { "none".to_string() } else { self.sections.join(", ") }),
            "> Fields in braces ({Element.Name}…) come from the model: fill them in from the elements of the package the document is about; repeat a section for each of its elements.".to_string(),
        ];
        let filters = self.filters();
        if !filters.first().is_some_and(|f| f.starts_with("none")) {
            out.push(format!("> The original leaves out: {}.", filters.join("; ")));
        }
        out.push(String::new());
        let mut depth = 0usize;
        for line in self.text.lines().map(str::trim).filter(|l| !l.is_empty()) {
            let marker = |l: &str| l.chars().all(|c| c.is_alphabetic() || c == ' ');
            if let Some(word) = line.strip_suffix(" >").filter(|w| marker(w)) {
                depth += 1;
                if !self.has_section(word) {
                    out.push(format!("> Section: {word}"));
                }
                continue;
            }
            if line.strip_prefix("< ").is_some_and(marker) {
                depth = depth.saturating_sub(1);
                continue;
            }
            // A short line that prints a name is the section's heading; one that is only a field becomes a hint.
            let fields = fields(line);
            let bare: String = line.chars().filter(|c| !matches!(c, '*' | '#' | '_')).collect();
            let bare = bare.trim();
            if bare.starts_with('|') {
                // A table: its first row is the header.
                let first = !out.last().is_some_and(|l| l.starts_with('|'));
                out.push(bare.to_string());
                if first {
                    out.push(format!("|{}", "---|".repeat(bare.matches('|').count().saturating_sub(1).max(1))));
                }
            } else if fields.iter().any(|f| f.ends_with(".Name") || f.ends_with(".FullName")) && bare.chars().count() <= 60 {
                out.push(format!("{} {bare}", "#".repeat(depth.clamp(1, 6))));
            } else if !fields.is_empty() && fields.iter().map(|f| f.len() + 2).sum::<usize>() >= bare.replace(' ', "").len() {
                out.push(format!("> Fill in: {}", fields.iter().cloned().collect::<Vec<_>>().join(", ")));
            } else {
                out.push(bare.to_string());
            }
        }
        out.join("\n")
    }

    fn option(&self, key: &str) -> &str {
        self.options.get(key).map(String::as_str).unwrap_or("")
    }

    /// Element types the template leaves out ("exdetails": ";Type;;Type;").
    fn excluded_types(&self) -> Vec<String> {
        let mut types: Vec<String> = self.option("exdetails").split(';').map(str::trim).filter(|t| !t.is_empty()).map(String::from).collect();
        types.dedup();
        types
    }

    fn has_section(&self, word: &str) -> bool {
        self.sections.iter().any(|s| s == word)
    }

    fn explain(&self) -> String {
        let mut out = vec![format!("Template \"{}\"", self.name)];
        out.push(format!("Sections it uses: {}", if self.sections.is_empty() { "none found (an empty template, or one EA stores differently)".to_string() } else { self.sections.join(", ") }));
        if !self.has_section("element") {
            out.push("- It has no \"element\" section: elements of a package are not printed, only what the other sections show.".into());
        }
        if self.has_section("element") && !self.has_section("child elements") {
            out.push("- It has no \"child elements\" section: elements nested in other elements are not printed.".into());
        }
        let fields: Vec<&String> = self.fields.iter().take(60).collect();
        out.push(format!("Fields it prints ({}): {}", self.fields.len(), fields.iter().map(|f| f.as_str()).collect::<Vec<_>>().join(", ")));
        out.push("Filters:".into());
        out.extend(self.filters().into_iter().map(|f| format!("- {f}")));
        out.join("\n")
    }

    /// The template's filters in plain words.
    fn filters(&self) -> Vec<String> {
        let mut out = Vec::new();
        let excluded = self.excluded_types();
        if !excluded.is_empty() {
            out.push(format!("element types left out ({}): {}", excluded.len(), excluded.join(", ")));
        }
        if !self.option("statuslist").trim().is_empty() {
            out.push(format!("status filter: {} (statuslike={})", self.option("statuslist"), self.option("statuslike")));
        }
        let phase = self.option("phase");
        if !phase.is_empty() && !phase.eq_ignore_ascii_case("all") {
            out.push(format!("phase filter: {phase} (phaselike={})", self.option("phaselike")));
        }
        if self.option("noteless") == "1" {
            out.push("elements without notes are skipped (noteless=1)".into());
        }
        if self.option("hideanonymous") == "1" {
            out.push("elements without a name are hidden (hideanonymous=1)".into());
        }
        if self.option("skiproot") == "1" {
            out.push("the package the document starts from is skipped (skiproot=1)".into());
        }
        if self.option("modcreate") != "-1" && !self.option("modcreate").is_empty() {
            out.push(format!("a date filter on {} (modcreate={}, beforeafter={})", if self.option("modcreate") == "0" { "the created date" } else { "the modified date" }, self.option("modcreate"), self.option("beforeafter")));
        }
        if out.is_empty() {
            out.push("none (every element type, status and phase)".into());
        }
        out
    }
}

fn between(text: &str, start: &str, end: &str) -> Option<String> {
    let from = text.find(start)? + start.len();
    let len = text[from..].find(end)?;
    Some(text[from..from + len].to_string())
}

/// name="value" pairs of an XML tag.
fn attributes(tag: &str) -> HashMap<String, String> {
    let mut out = HashMap::new();
    let mut rest = tag;
    while let Some(eq) = rest.find("=\"") {
        let key = rest[..eq].rsplit(|c: char| c.is_whitespace()).next().unwrap_or("").to_string();
        let after = &rest[eq + 2..];
        let Some(end) = after.find('"') else { break };
        out.insert(key, after[..end].replace("&quot;", "\"").replace("&amp;", "&"));
        rest = &after[end + 1..];
    }
    out
}

/// The text of an RTF body without control words and groups' braces (EA's markers and fields stay readable).
fn plain(rtf: &str) -> String {
    let mut out = String::new();
    let mut chars = rtf.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            // A destination group ({\*\bkmkstart …}): left out whole.
            '{' if rtf_starts(&chars, "\\*") => {
                let mut depth = 1;
                while depth > 0 {
                    match chars.next() {
                        Some('\\') => {
                            chars.next();
                        }
                        Some('{') => depth += 1,
                        Some('}') => depth -= 1,
                        Some(_) => {}
                        None => break,
                    }
                }
            }
            '\\' => match chars.peek() {
                Some('{') | Some('}') | Some('\\') => out.push(chars.next().unwrap_or(' ')),
                Some('\'') => {
                    chars.next();
                    chars.next();
                    chars.next();
                }
                _ => {
                    let mut word = String::new();
                    while let Some(c) = chars.peek().copied().filter(|c| c.is_ascii_alphabetic() || c.is_ascii_digit() || *c == '-') {
                        word.push(c);
                        chars.next();
                    }
                    // The end of a paragraph or line: a line of the text (to_markdown reads them).
                    if word == "par" || word == "line" {
                        out.push('\n');
                    }
                    if chars.peek() == Some(&' ') {
                        chars.next();
                    }
                }
            },
            '{' | '}' | '\r' => {}
            c => out.push(c),
        }
    }
    out
}

/// The text of a template in EA's own format (TX Text Control, "TeCfMt" after the text; LC-423): paragraphs end with 0x15,
/// table cells with 0x13 and rows with 0x12, a bare line break is only a wrap; a field is kept twice ({Element.Name} shown,
/// then its code, sometimes with its first bytes garbled), so the second copy is left out. Table rows become "| a | b |".
fn tx_text(body: &str) -> String {
    let bytes: Vec<u8> = body.chars().map(|c| c as u32 as u8).collect();
    let decoded = String::from_utf8_lossy(&bytes).to_string();
    let text = decoded.split("~`!@#$%^&*()-+|=-TeCfMt").next().unwrap_or("");
    let text = text.replace("\u{15}\r\n", "\n").replace("\u{13}\r\n", "\u{13}").replace("\u{12}\r\n", "\u{12}").replace("\r\n", "");
    // A field's second copy: up to the next "}" (or break), not much longer than the field.
    let mut out = String::new();
    let mut rest = text.as_str();
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let Some(close) = rest[open..].find('}') else { break };
        let field = &rest[open..open + close + 1];
        out.push_str(field);
        rest = &rest[open + close + 1..];
        let end = rest.find(|c: char| c == '}' || c.is_whitespace() || ('\u{0}'..='\u{1f}').contains(&c)).unwrap_or(rest.len());
        if end > 0 && end <= field.len() * 2 + 4 && !rest[..end].contains('{') || rest.starts_with(field) {
            rest = &rest[(end + usize::from(rest[end..].starts_with('}'))).min(rest.len())..];
        }
    }
    out.push_str(rest);
    // Table rows: the cells of a row on one line.
    let mut lines = Vec::new();
    for row in out.split('\n') {
        if row.contains('\u{13}') {
            for r in row.split('\u{12}').map(str::trim).filter(|r| !r.is_empty()) {
                let cells: Vec<&str> = r.trim_end_matches('\u{13}').split('\u{13}').map(str::trim).collect();
                // A one-cell row (a heading or a section marker in a frame) is a line of its own.
                lines.push(if cells.len() == 1 { cells[0].to_string() } else { format!("| {} |", cells.join(" | ")) });
            }
        } else {
            lines.push(row.replace('\u{12}', "").trim_end().to_string());
        }
    }
    lines.join("\n")
}

fn rtf_starts(chars: &std::iter::Peekable<std::str::Chars>, text: &str) -> bool {
    chars.clone().take(text.len()).eq(text.chars())
}

/// EA fields the template prints ({Element.Name}, {Pkg.Notes}, {Diagram.DiagramImg}…).
fn fields(text: &str) -> BTreeSet<String> {
    let flat: String = text.chars().filter(|c| *c != '\n').collect();
    let mut out = BTreeSet::new();
    let mut rest = flat.as_str();
    while let Some(start) = rest.find('{') {
        rest = &rest[start + 1..];
        let Some(end) = rest.find('}') else { break };
        let field = &rest[..end];
        if let Some((head, tail)) = field.split_once('.') {
            if !head.is_empty() && head.chars().all(|c| c.is_ascii_alphabetic()) && !tail.is_empty() && tail.chars().all(|c| c.is_ascii_alphanumeric() || "._()".contains(c)) {
                out.insert(field.to_string());
            }
        }
    }
    out
}

/// The stored template's bytes, read as hexadecimal text in pieces (ODBC drivers cut long values).
fn template_bytes(cfg: &AppConfig, name: &str) -> Result<Vec<u8>> {
    let quoted = crate::model_db::sql_text(cfg.model_source, name);
    let length = match cfg.model_source {
        ModelSource::SqlServer => "datalength(bincontent)",
        ModelSource::Oracle => bail!("On Oracle the template cannot be read here; open it in EA (Publish → Templates) or export it."),
        _ => "length(bincontent)",
    };
    let found = query_rows(cfg, &format!("select docid as c0, {length} as c1 from t_document where doctype = 'SSDOCSTYLE' and docname = '{quoted}'"))?;
    let Some(row) = found.first() else {
        let known = query_rows(cfg, "select docname as c0 from t_document where doctype = 'SSDOCSTYLE' order by docname")?;
        bail!("there is no template \"{name}\" in the model; templates: {}", known.iter().map(|r| r[0].as_str()).collect::<Vec<_>>().join(", "));
    };
    if row[1].trim().parse::<usize>().unwrap_or(0) == 0 {
        bail!("the template \"{name}\" is empty");
    }
    document_blob(cfg, &row[0])
}

/// The bincontent of a row of t_document (a template, a baseline), read as hexadecimal text in pieces (ODBC drivers
/// cut long values).
pub(crate) fn document_blob(cfg: &AppConfig, docid: &str) -> Result<Vec<u8>> {
    let (length, hex, part) = match cfg.model_source {
        ModelSource::File => ("length(bincontent)", "hex(bincontent)", "substr"),
        ModelSource::MySql => ("length(bincontent)", "hex(bincontent)", "substring"),
        ModelSource::Postgres => ("length(bincontent)", "encode(bincontent, 'hex')", "substr"),
        ModelSource::SqlServer => ("datalength(bincontent)", "convert(varchar(max), bincontent, 2)", "substring"),
        ModelSource::Oracle => bail!("On Oracle the stored content cannot be read here; open it in EA."),
    };
    let docid = crate::model_db::sql_text(cfg.model_source, docid);
    let size: usize = query_rows(cfg, &format!("select {length} as c0 from t_document where docid = '{docid}'"))?.first().and_then(|r| r[0].trim().parse().ok()).unwrap_or(0);
    const PIECE: usize = 60_000;
    let mut text = String::new();
    let mut start = 1;
    while start <= size * 2 {
        let rows = query_rows(cfg, &format!("select {part}({hex}, {start}, {PIECE}) as c0 from t_document where docid = '{docid}'"))?;
        text.push_str(rows.first().map(|r| r[0].trim()).unwrap_or(""));
        start += PIECE;
    }
    let text = text.trim_start_matches("0x").trim_start_matches("\\x");
    (0..text.len() / 2).map(|i| u8::from_str_radix(&text[i * 2..i * 2 + 2], 16).context("the template is not hexadecimal")).collect()
}

/// A DOCX attached to the conversation: its text in lower case and its time.
fn document(cfg: &AppConfig, input: &Value) -> Result<Option<(String, String, Option<chrono::NaiveDateTime>)>> {
    let Some(name) = input["document"].as_str().map(str::trim).filter(|d| !d.is_empty()) else { return Ok(None) };
    let file = std::path::Path::new(name).file_name().context("document")?.to_string_lossy().to_string();
    let path = crate::history::attachment_path(&cfg.attachments_dir, &file);
    let bytes = std::fs::read(&path).with_context(|| format!("there is no attachment {file}"))?;
    let text = crate::documents::text_of(&file, &bytes)?;
    let time = std::fs::metadata(&path).and_then(|m| m.modified()).ok().map(|t| chrono::DateTime::<chrono::Local>::from(t).naive_local());
    Ok(Some((file, text.to_lowercase(), time)))
}

/// One element as the checks need it.
struct Element {
    id: String,
    name: String,
    object_type: String,
    status: String,
    phase: String,
    note: String,
    package_id: String,
    parent_id: String,
    created: String,
    modified: String,
}

const ELEMENT_COLUMNS: &str = "o.object_id as c0, o.name as c1, o.object_type as c2, coalesce(o.status,'') as c3, coalesce(o.phase,'') as c4, \
coalesce(o.note,'') as c5, o.package_id as c6, coalesce(o.parentid,0) as c7, o.createddate as c8, o.modifieddate as c9";

fn element_of(r: &[String]) -> Element {
    Element {
        id: r[0].clone(),
        name: r[1].clone(),
        object_type: r[2].clone(),
        status: r[3].clone(),
        phase: r[4].clone(),
        note: r[5].clone(),
        package_id: r[6].clone(),
        parent_id: r[7].clone(),
        created: r[8].clone(),
        modified: r[9].clone(),
    }
}

/// The reasons the template's filters and sections give for leaving the element out.
fn template_reasons(t: &Template, e: &Element, root_package: &str) -> Vec<String> {
    let mut out = Vec::new();
    if !t.has_section("element") {
        out.push(format!("the template \"{}\" has no element section, so it prints no elements", t.name));
    }
    if t.excluded_types().iter().any(|x| x.eq_ignore_ascii_case(&e.object_type)) {
        out.push(format!("the template leaves out elements of type {} (Document Options → Exclude Details)", e.object_type));
    }
    let statuses: Vec<String> = t.option("statuslist").split([';', ',']).map(|s| s.trim().to_lowercase()).filter(|s| !s.is_empty()).collect();
    if !statuses.is_empty() {
        let listed = statuses.contains(&e.status.to_lowercase());
        if listed == (t.option("statuslike") == "0") {
            out.push(format!("the template filters by status ({}) and the element's status is \"{}\"", t.option("statuslist"), e.status));
        }
    }
    let phase = t.option("phase");
    if !phase.is_empty() && !phase.eq_ignore_ascii_case("all") && !phase.eq_ignore_ascii_case(&e.phase) {
        out.push(format!("the template filters by phase ({phase}) and the element's phase is \"{}\"", e.phase));
    }
    if t.option("noteless") == "1" && e.note.trim().is_empty() {
        out.push("the template skips elements without notes and this element has none".into());
    }
    if t.option("hideanonymous") == "1" && e.name.trim().is_empty() {
        out.push("the template hides elements without a name".into());
    }
    if t.option("skiproot") == "1" && e.package_id == root_package {
        out.push("the template skips the package the document starts from, and the element is directly in it".into());
    }
    if e.parent_id != "0" && !e.parent_id.is_empty() && t.has_section("element") && !t.has_section("child elements") {
        out.push("the element is nested in another element and the template has no child elements section".into());
    }
    out
}

/// A date of the model (EA writes "2026-10-30 12:34:56", with or without fractions, or an ISO time).
fn parse_time(text: &str) -> Option<chrono::NaiveDateTime> {
    let text = text.trim();
    ["%Y-%m-%d %H:%M:%S%.f", "%Y-%m-%dT%H:%M:%S%.f", "%Y-%m-%d %H:%M:%S", "%Y-%m-%d %H:%M"]
        .iter()
        .find_map(|f| chrono::NaiveDateTime::parse_from_str(text, f).ok())
        .or_else(|| chrono::NaiveDate::parse_from_str(text.get(..10)?, "%Y-%m-%d").ok()?.and_hms_opt(0, 0, 0))
}

/// Reasons from the element and the document alone.
fn element_reasons(e: &Element, doc_time: Option<chrono::NaiveDateTime>) -> Vec<String> {
    let mut out = Vec::new();
    if let (Some(doc), Some(created)) = (doc_time, parse_time(&e.created)) {
        if created > doc {
            out.push(format!("the element was created ({}) after the document was saved ({}) – generate the document again", e.created, doc.format("%Y-%m-%d %H:%M")));
        }
    }
    if e.name.trim().is_empty() {
        out.push("the element has no name".into());
    }
    out
}

fn why_missing(input: &Value, cfg: &AppConfig) -> Result<String> {
    let element = &input["element"];
    let rows = if let Some(id) = element.as_i64().or_else(|| element.as_str().and_then(|s| s.trim().parse().ok())) {
        query_rows(cfg, &format!("select {ELEMENT_COLUMNS} from t_object o where o.object_id = {id}"))?
    } else {
        let name = crate::model_db::sql_text(cfg.model_source, element.as_str().unwrap_or("").trim());
        query_rows(cfg, &format!("select {ELEMENT_COLUMNS} from t_object o where o.name = '{name}' order by o.object_id"))?
    };
    let e = match rows.as_slice() {
        [] => bail!("{}", crate::i18n::fmt("there is no element with id {0}", &[element.to_string()])),
        [one] => element_of(one),
        many => bail!("several elements are named {element}: {} - pass the object_id", many.iter().map(|r| format!("{} ({}, id {})", r[1], r[2], r[0])).collect::<Vec<_>>().join("; ")),
    };
    let mut out = vec![format!("{} ({}, id {}), status \"{}\", phase \"{}\", created {}, modified {}", e.name, e.object_type, e.id, e.status, e.phase, e.created, e.modified)];
    out.push(format!("It is in: {}", package_path(cfg, &e.package_id)?));
    let mut reasons = Vec::new();
    let mut root = String::new();
    if !input["package"].is_null() && input["package"].as_str() != Some("") {
        let (scope, title) = crate::model_usage::package_scope(cfg, &input["package"])?;
        root = scope.split('(').nth(1).and_then(|s| s.split([',', ')']).next()).unwrap_or("").to_string();
        let inside = scope.split('(').nth(1).unwrap_or("").trim_end_matches(')').split(',').any(|p| p == e.package_id);
        if !inside {
            reasons.push(format!("the element is not in the {title}: the document generated from it cannot show it"));
        }
    }
    if let Some(t) = input["template"].as_str().map(str::trim).filter(|t| !t.is_empty()) {
        reasons.extend(template_reasons(&Template::load(cfg, t)?, &e, &root));
    }
    let doc = document(cfg, input)?;
    reasons.extend(element_reasons(&e, doc.as_ref().and_then(|d| d.2)));
    if let Some((file, text, _)) = &doc {
        if !e.name.trim().is_empty() && text.contains(&e.name.to_lowercase()) {
            out.push(format!("Its name IS in {file} – it may be under another heading or in a table."));
        } else {
            out.push(format!("Its name is not in {file}."));
        }
    }
    if reasons.is_empty() {
        out.push("Nothing found explains it. Check in EA: the generator's Document Options (Element Filters, Exclude Details), whether the package is excluded from documentation (package properties), and whether the document was generated from this package and this model.".into());
        if input["template"].is_null() {
            out.push("Give the template's name too, and its filters are checked.".into());
        }
    } else {
        out.push("Likely reasons:".into());
        out.extend(reasons.into_iter().map(|r| format!("- {r}")));
    }
    Ok(out.join("\n"))
}

fn compare(input: &Value, cfg: &AppConfig) -> Result<String> {
    let Some((file, text, doc_time)) = document(cfg, input)? else { bail!("document: the DOCX attached to the conversation") };
    let (scope, title) = crate::model_usage::package_scope(cfg, &input["package"])?;
    let root = scope.split('(').nth(1).and_then(|s| s.split([',', ')']).next()).unwrap_or("").to_string();
    let template = match input["template"].as_str().map(str::trim).filter(|t| !t.is_empty()) {
        Some(t) => Some(Template::load(cfg, t)?),
        None => None,
    };
    let rows = query_rows(cfg, &format!("select {ELEMENT_COLUMNS} from t_object o where o.object_type not in ('Package', 'Note', 'Text', 'Boundary', 'ProxyConnector') {scope} order by o.name"))?;
    let total = rows.len();
    let missing: Vec<Element> = rows.iter().map(|r| element_of(r)).filter(|e| !e.name.trim().is_empty() && !text.contains(&e.name.to_lowercase())).collect();
    let mut out = vec![format!("{file} against the {title}: {} of {total} elements are not mentioned.", missing.len())];
    for e in missing.iter().take(200) {
        let mut reasons = template.as_ref().map(|t| template_reasons(t, e, &root)).unwrap_or_default();
        reasons.extend(element_reasons(e, doc_time));
        out.push(format!("- {} ({}, id {}){}", e.name, e.object_type, e.id, if reasons.is_empty() { String::new() } else { format!(": {}", reasons.join("; ")) }));
    }
    if missing.len() > 200 {
        out.push(format!("… and {} more", missing.len() - 200));
    }
    if template.is_none() && !missing.is_empty() {
        out.push("Give the template's name too, and its filters are checked for each element.".into());
    }
    Ok(out.join("\n"))
}

/// The word/document.xml (and word/comments.xml) of the DOCX attached to the conversation.
fn docx_xml(cfg: &AppConfig, input: &Value) -> Result<(String, String)> {
    let name = input["document"].as_str().map(str::trim).filter(|d| !d.is_empty()).context("document: the DOCX attached to the conversation")?;
    let file = std::path::Path::new(name).file_name().context("document")?.to_string_lossy().to_string();
    let bytes = std::fs::read(crate::history::attachment_path(&cfg.attachments_dir, &file)).with_context(|| format!("there is no attachment {file}"))?;
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes.as_slice())).context("the document is not a DOCX")?;
    let document = crate::documents::part(&mut archive, "word/document.xml").context("word/document.xml is missing: not a Word document")?;
    Ok((document, crate::documents::part(&mut archive, "word/comments.xml").unwrap_or_default()))
}

/// The paragraphs of a Word document: (heading level by style, text, bold throughout, comment ids anchored in it).
fn docx_paragraphs(xml: &str) -> Vec<(Option<usize>, String, bool, Vec<String>)> {
    xml.split("</w:p>")
        .map(|p| {
            let start = p.rfind("<w:p>").or_else(|| p.rfind("<w:p ")).unwrap_or(0);
            let p = &p[start..];
            let style = p.find("<w:pStyle").and_then(|i| crate::documents::attr(&p[i..], "w:val")).unwrap_or_default();
            let text = crate::documents::texts(p, "w:t").join("").trim().to_string();
            let runs: Vec<&str> = p.split("<w:r>").skip(1).chain(p.split("<w:r ").skip(1)).filter(|r| r.contains("<w:t")).collect();
            let bold = !runs.is_empty() && runs.iter().all(|r| r.contains("<w:b/>") || r.contains("<w:b ") && !r.contains("<w:b w:val=\"0\""));
            let ids = p.split("<w:commentRangeStart ").skip(1).filter_map(|t| crate::documents::attr(&format!(" {t}"), "w:id")).collect();
            (crate::templates::heading_level(&style), text, bold, ids)
        })
        .collect()
}

/// Headings of a DOCX (LC-423): how many of each level, skipped levels, and paragraphs that look like headings (bold or
/// numbered, short, no full stop) without a heading style.
fn check_headings(xml: &str) -> String {
    let paragraphs = docx_paragraphs(xml);
    let mut counts = [0usize; 7];
    let (mut previous, mut skips, mut unstyled) = (0usize, Vec::new(), Vec::new());
    for (level, text, bold, _) in paragraphs.iter().filter(|p| !p.1.is_empty()) {
        match level {
            Some(l) => {
                counts[*l] += 1;
                if *l > previous + 1 {
                    skips.push(format!("\"{text}\" is Heading {l} right after Heading {previous}"));
                }
                previous = *l;
            }
            None => {
                let numbered = text.split_whitespace().next().is_some_and(|w| w.trim_end_matches('.').split('.').all(|n| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit())));
                let short = text.chars().count() <= 80 && !text.ends_with(['.', ':', ';', ',']);
                if short && (*bold || numbered && text.split_whitespace().count() > 1) {
                    unstyled.push(format!("\"{text}\"{}", if *bold { " (bold)" } else { " (numbered)" }));
                }
            }
        }
    }
    let total: usize = counts.iter().sum();
    let mut out = vec![format!(
        "Headings with a Heading style: {total}{}",
        if total == 0 { String::new() } else { format!(" ({})", (1..=6).filter(|l| counts[*l] > 0).map(|l| format!("Heading {l}: {}", counts[l])).collect::<Vec<_>>().join(", ")) }
    )];
    if !unstyled.is_empty() {
        out.push(format!("{} paragraph(s) look like headings but have no heading style (Compass, EA and the table of contents do not see them as chapters; give them Heading 1…6):", unstyled.len()));
        out.extend(unstyled.iter().take(60).map(|u| format!("- {u}")));
    }
    if !skips.is_empty() {
        out.push("Skipped levels:".into());
        out.extend(skips.iter().take(30).map(|s| format!("- {s}")));
    }
    if unstyled.is_empty() && skips.is_empty() {
        out.push("Nothing to fix: the chapters use the heading styles in order.".into());
    }
    out.join("\n")
}

/// The reviewers' comments of a DOCX matched to elements (LC-423): the commented text names the element, else the heading
/// above it does. Each match gets a Note with the comment linked to the element (ops for ea_write, saved after approval).
fn comments(input: &Value, cfg: &AppConfig) -> Result<String> {
    let (document, comments_xml) = docx_xml(cfg, input)?;
    let mut said: Vec<(String, String, String)> = Vec::new();
    for c in comments_xml.split("<w:comment ").skip(1) {
        let tag = format!(" {}", c.split('>').next().unwrap_or(""));
        let id = crate::documents::attr(&tag, "w:id").unwrap_or_default();
        let author = crate::documents::attr(&tag, "w:author").unwrap_or_default();
        let text = c.split("</w:p>").map(|p| crate::documents::texts(p, "w:t").join("")).filter(|t| !t.trim().is_empty()).collect::<Vec<_>>().join("\n");
        said.push((id, author, text));
    }
    if said.is_empty() {
        return Ok("The document has no comments.".into());
    }
    // Where each comment is: the paragraph its range starts in and the heading above it.
    let mut place: HashMap<String, (String, String)> = HashMap::new();
    let mut heading = String::new();
    for (level, text, _, ids) in docx_paragraphs(&document) {
        if level.is_some() {
            heading = text.clone();
        }
        for id in ids {
            place.entry(id).or_insert((text.clone(), heading.clone()));
        }
    }
    let scope = if input["package"].is_null() { String::new() } else { crate::model_usage::package_scope(cfg, &input["package"])?.0 };
    let mut elements: Vec<(String, String, String)> = query_rows(cfg, &format!(
        "select o.object_id as c0, o.name as c1, o.package_id as c2 from t_object o where o.object_type not in ('Package', 'Note', 'Text', 'Boundary') and o.name is not null {scope}"
    ))?
    .into_iter()
    .filter(|r| r[1].trim().chars().count() >= 3)
    .map(|r| (r[0].clone(), r[1].trim().to_string(), r[2].clone()))
    .collect();
    // The longest name first: "Order line" before "Order".
    elements.sort_by_key(|e| std::cmp::Reverse(e.1.chars().count()));
    let find = |text: &str| {
        let text = text.to_lowercase();
        elements.iter().find(|e| text.contains(&e.1.to_lowercase())).cloned()
    };
    let (mut lines, mut ops, mut unmatched) = (Vec::new(), Vec::new(), Vec::new());
    for (n, (id, author, text)) in said.iter().enumerate() {
        let (anchored, under) = place.get(id).cloned().unwrap_or_default();
        let found = find(&anchored).map(|e| (e, "the commented text")).or_else(|| find(&under).map(|e| (e, "the heading above")));
        let Some(((object, name, package), how)) = found else {
            unmatched.push(format!("- {author}: \"{text}\" (on \"{}\")", if anchored.is_empty() { &under } else { &anchored }));
            continue;
        };
        lines.push(format!("- {name} (id {object}, by {how}) ← {author}: \"{text}\""));
        let key = format!("comment{}", n + 1);
        ops.push(json!({ "op": "create_element", "key": key, "package": package.parse::<i64>().unwrap_or(0), "type": "Note", "name": "",
            "notes": format!("{author}: {text}") }));
        ops.push(json!({ "op": "create_connector", "source": format!("#{key}"), "target": object.parse::<i64>().unwrap_or(0), "type": "NoteLink" }));
    }
    let mut out = vec![format!("{} comment(s), {} matched to elements:", said.len(), lines.len())];
    out.extend(lines);
    if !unmatched.is_empty() {
        out.push(format!("Not matched ({}; name the element and they can be added the same way):", unmatched.len()));
        out.extend(unmatched);
    }
    if !ops.is_empty() {
        out.push(format!("To add them as Notes linked to the elements, after the user agrees: ea_write with ops {}", Value::Array(ops)));
    }
    Ok(out.join("\n"))
}

/// Model › Package › … of a package.
fn package_path(cfg: &AppConfig, package: &str) -> Result<String> {
    let mut names = Vec::new();
    let mut id = package.to_string();
    for _ in 0..30 {
        let Some(row) = query_rows(cfg, &format!("select name as c0, parent_id as c1 from t_package where package_id = {}", id.parse::<i64>().unwrap_or(0)))?.into_iter().next() else { break };
        names.push(row[0].clone());
        if row[1] == "0" || row[1].is_empty() {
            break;
        }
        id = row[1].clone();
    }
    names.reverse();
    Ok(format!("{} (package_id {package})", names.join(" › ")))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// LC-383: a template made the way EA keeps it: options, sections and fields read back.
    #[test]
    fn template_parsed() {
        let rtf = crate::ea_template::rtf("## {Pkg.Name}", "**{Element.Name}** {Element.Notes}", "{Diagram.Name}");
        let bytes = crate::ea_template::document("Mine", &rtf).unwrap();
        let t = Template::parse("Mine", &bytes).unwrap();
        assert!(t.has_section("package") && t.has_section("element") && t.has_section("diagram"), "{:?}", t.sections);
        assert!(t.fields.contains("Element.Name") && t.fields.contains("Pkg.Name"), "{:?}", t.fields);
        assert!(t.explain().contains("child elements"));
        let e = Element {
            id: "1".into(),
            name: "Order".into(),
            object_type: "Class".into(),
            status: "Proposed".into(),
            phase: "1.0".into(),
            note: String::new(),
            package_id: "5".into(),
            parent_id: "7".into(),
            created: String::new(),
            modified: String::new(),
        };
        let reasons = template_reasons(&t, &e, "5");
        assert!(reasons.iter().any(|r| r.contains("nested")), "{reasons:?}");
        // LC-423: the same template as a Compass template.
        let md = t.markdown();
        assert!(md.starts_with("> Template: converted from the EA template \"Mine\"") && md.contains("# {Pkg.Name}") && md.contains("## {Element.Name}"), "{md}");
        let mut strict = Template { name: "x".into(), options: HashMap::new(), sections: vec!["element".into()], fields: BTreeSet::new(), text: String::new() };
        strict.options.insert("exdetails".into(), ";Class;;Note;".into());
        strict.options.insert("noteless".into(), "1".into());
        let reasons = template_reasons(&strict, &e, "9");
        assert!(reasons.iter().any(|r| r.contains("type Class")) && reasons.iter().any(|r| r.contains("without notes")), "{reasons:?}");
    }

    /// LC-423: EA's own template format (as read from the model: one char per byte), fields kept twice, a table.
    #[test]
    fn tx_template_text() {
        let raw: &[u8] = b"package >\x15\r\n{Diagram.Name}{Diagram.Name}-Score\x15\r\nelement >\x15\r\n{Element.FullName}\xc3\x93\xc2\xa4lement.FullName}\x13\r\n\x12\r\nID\x13\r\nEstimated \r\nCost\x13\r\n\x12\r\n{Elem\r\nent.Alias}{Element.Alias}\x13\r\n{Element.Notes}\xc3\xb6\xc2\xb1\x13\r\n\x12\r\n< element\x15\r\n< package\x15\r\n~`!@#$%^&*()-+|=-TeCfMt\r\n\x00\x02";
        let body: String = raw.iter().map(|&b| b as char).collect();
        let text = tx_text(&body);
        assert_eq!(text, "package >\n{Diagram.Name}-Score\nelement >\n{Element.FullName}\n| ID | Estimated Cost |\n| {Element.Alias} | {Element.Notes} |\n< element\n< package\n", "{text:?}");
    }

    /// LC-423: headings without a style, a skipped level, and where comments are.
    #[test]
    fn docx_headings_and_comments() {
        let p = |style: &str, bold: bool, text: &str, comment: &str| {
            let style = if style.is_empty() { String::new() } else { format!("<w:pPr><w:pStyle w:val=\"{style}\"/></w:pPr>") };
            let start = if comment.is_empty() { String::new() } else { format!("<w:commentRangeStart w:id=\"{comment}\"/>") };
            format!("<w:p>{style}{start}<w:r>{}<w:t>{text}</w:t></w:r></w:p>", if bold { "<w:rPr><w:b/></w:rPr>" } else { "" })
        };
        let xml = [p("Heading1", false, "Scope", ""), p("Heading3", false, "Details", ""), p("", true, "Order handling", ""), p("", false, "2.1 Payments", ""),
            p("", false, "The Invoice is sent by mail.", "7"), p("", false, "Plain text, nothing else.", "")].concat();
        let report = check_headings(&xml);
        assert!(report.contains("Heading 1: 1, Heading 3: 1") && report.contains("\"Details\" is Heading 3 right after Heading 1"), "{report}");
        assert!(report.contains("\"Order handling\" (bold)") && report.contains("\"2.1 Payments\" (numbered)") && !report.contains("Plain text"), "{report}");
        let paragraphs = docx_paragraphs(&xml);
        let commented = paragraphs.iter().find(|p| p.3 == vec!["7".to_string()]).unwrap();
        assert_eq!(commented.1, "The Invoice is sent by mail.");
    }

    /// LC-423 on a model copy: a reviewer's comment in a DOCX becomes a Note linked to the element it is about.
    /// `LOONAR_TEST_PROPS=… cargo test word_comments_live -- --ignored --nocapture`
    #[test]
    #[ignore = "requires a test model"]
    fn word_comments_live() {
        use std::io::Write;
        let cfg = AppConfig::load(std::path::Path::new(&std::env::var("LOONAR_TEST_PROPS").unwrap())).unwrap();
        let q = |sql: &str| query_rows(&cfg, sql).unwrap();
        let root: i64 = q("select package_id from t_package where parent_id = 0 order by package_id")[0][0].parse().unwrap();
        let stamp = chrono::Local::now().format("%H%M%S").to_string();
        let created = crate::ea::run_batch(&cfg, &[
            json!({"op":"create_package","key":"pkg","parent":root,"name":format!("ZZ_comments_{stamp}")}),
            json!({"op":"create_element","key":"inv","package":"#pkg","type":"Class","name":format!("Invoice{stamp}")}),
        ], &crate::ea::BatchOptions::default()).unwrap();
        let id_of = |k: &str| created["created"].as_array().unwrap().iter().find(|c| c["key"] == k).unwrap()["id"].as_i64().unwrap();
        let (pkg, invoice) = (id_of("pkg"), id_of("inv"));
        let document = format!(r#"<w:document xmlns:w="w"><w:body><w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>Invoice{stamp}</w:t></w:r></w:p><w:p><w:commentRangeStart w:id="0"/><w:r><w:t>Sent by mail.</w:t></w:r><w:commentRangeEnd w:id="0"/></w:p><w:p><w:commentRangeStart w:id="1"/><w:r><w:t>Unrelated.</w:t></w:r></w:p></w:body></w:document>"#);
        let comments = r#"<w:comments xmlns:w="w"><w:comment w:id="0" w:author="Reviewer"><w:p><w:r><w:t>Also by e-mail?</w:t></w:r></w:p></w:comment></w:comments>"#;
        let file = format!("review{stamp}.docx");
        std::fs::create_dir_all(&cfg.attachments_dir).unwrap();
        let mut zip = zip::ZipWriter::new(std::fs::File::create(cfg.attachments_dir.join(&file)).unwrap());
        for (name, body) in [("word/document.xml", document.as_str()), ("word/comments.xml", comments)] {
            zip.start_file(name, zip::write::SimpleFileOptions::default()).unwrap();
            zip.write_all(body.as_bytes()).unwrap();
        }
        zip.finish().unwrap();
        let report = run(&json!({ "action": "comments", "document": file, "package": pkg }), &cfg).unwrap();
        println!("{report}");
        assert!(report.contains(&format!("Invoice{stamp} (id {invoice}, by the heading above) ← Reviewer: \"Also by e-mail?\"")), "{report}");
        let ops: Value = serde_json::from_str(report.split("ea_write with ops ").nth(1).unwrap()).unwrap();
        crate::ea::run_batch(&cfg, ops.as_array().unwrap(), &crate::ea::BatchOptions::default()).unwrap();
        let notes = q(&format!("select n.note from t_connector c join t_object n on n.object_id = c.start_object_id where c.connector_type = 'NoteLink' and c.end_object_id = {invoice}"));
        crate::ea::run_batch(&cfg, &[json!({"op":"delete_package","package":pkg})], &crate::ea::BatchOptions::default()).expect("cleanup");
        let _ = std::fs::remove_file(cfg.attachments_dir.join(&file));
        assert!(notes.first().is_some_and(|n| n[0].contains("Reviewer: Also by e-mail?")), "{notes:?}");
    }

    /// The template doctor on a model copy: `LOONAR_TEST_PROPS=… cargo test template_doctor_live -- --ignored --nocapture`
    #[test]
    #[ignore = "requires a test model"]
    fn template_doctor_live() {
        let cfg = AppConfig::load(std::path::Path::new(&std::env::var("LOONAR_TEST_PROPS").unwrap())).unwrap();
        let listed = run(&json!({ "action": "list" }), &cfg).unwrap();
        println!("{listed}");
        if let Some(name) = listed.lines().nth(1).map(|l| l.trim_start_matches("- ").split(" (author").next().unwrap().to_string()) {
            let explained = run(&json!({ "action": "explain", "template": name }), &cfg).unwrap();
            println!("{explained}");
            assert!(explained.contains("Sections it uses"));
            let md = run(&json!({ "action": "to_markdown", "template": name }), &cfg).unwrap();
            println!("{md}");
            assert!(md.starts_with("> Template: converted"));
        }
        let element = query_rows(&cfg, "select object_id from t_object where object_type = 'Class'").unwrap()[0][0].clone();
        let root = query_rows(&cfg, "select package_id from t_package where parent_id = 0").unwrap()[0][0].clone();
        let why = run(&json!({ "action": "why_missing", "element": element.parse::<i64>().unwrap(), "package": root, "template": "Balanced Score Card" }), &cfg).unwrap();
        println!("{why}");
        assert!(why.contains("It is in:"));
    }
}

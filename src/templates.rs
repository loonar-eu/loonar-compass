//! Document templates: Markdown files in the templates folder next to the conversation history. The assistant fills them
//! with content from the model and the conversation (Word document or Confluence page). The first line `> Szablon: description`
//! (in other languages `> Template:`, `> Vorlage:`, `> Modèle:`) is the description for the list; `> …` lines in the body are hints for the assistant.
//! The default templates exist in four languages; the user can change them and add their own.
//!
//! LC-311: any number of templates, each with what it is for in the lines under the description: the tool
//! (`> Cel: Confluence` / Jira / Word, none = any), a group of the user's own (`> Grupa: Technical level`) and the model
//! for writing it (`> Model: stronger`, or a model name). Templates are added from Markdown, Word, HTML, PDF and other
//! files or from a Confluence page; headings and tables are kept.

use crate::i18n::Lang;
use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};

/// The templates shipped up to version 0.9.1 (file name without .md, content). Since 0.9.2 (LC-311) Compass ships no
/// templates: the user adds their own; untouched copies of these are moved to the bin folder of the templates.
fn legacy_defaults(lang: Lang) -> [(&'static str, &'static str); 4] {
    match lang {
        Lang::Pl => [
            ("Wstępna analiza wykonalności", include_str!("../templates/pl/Feasibility_analysis.md")),
            ("BRD - Wymagania biznesowe", include_str!("../templates/pl/BRD.md")),
            ("Opis zmian w systemie", include_str!("../templates/pl/System_change.md")),
            ("Opis procesu", include_str!("../templates/pl/Process_description.md")),
        ],
        Lang::En => [
            ("Preliminary feasibility analysis", include_str!("../templates/en/Feasibility_analysis.md")),
            ("BRD - Business requirements", include_str!("../templates/en/BRD.md")),
            ("System change description", include_str!("../templates/en/System_change.md")),
            ("Process description", include_str!("../templates/en/Process_description.md")),
        ],
        Lang::De => [
            ("Vorläufige Machbarkeitsanalyse", include_str!("../templates/de/Feasibility_analysis.md")),
            ("BRD - Geschäftsanforderungen", include_str!("../templates/de/BRD.md")),
            ("Beschreibung der Systemänderungen", include_str!("../templates/de/System_change.md")),
            ("Prozessbeschreibung", include_str!("../templates/de/Process_description.md")),
        ],
        Lang::Fr => [
            ("Analyse de faisabilité préliminaire", include_str!("../templates/fr/Feasibility_analysis.md")),
            ("BRD - Exigences métier", include_str!("../templates/fr/BRD.md")),
            ("Description des modifications du système", include_str!("../templates/fr/System_change.md")),
            ("Description du processus", include_str!("../templates/fr/Process_description.md")),
        ],
    }
}

/// Creates the templates folder; untouched templates of earlier versions (`legacy_defaults`) go to its bin folder.
pub fn prepare(dir: &Path) {
    if std::fs::create_dir_all(dir).is_err() {
        return;
    }
    let normalize = |t: &str| t.trim_start_matches('\u{feff}').replace("\r\n", "\n").trim().to_string();
    let legacy: Vec<(&str, String)> = Lang::ALL.into_iter().flat_map(legacy_defaults).map(|(name, text)| (name, normalize(text))).collect();
    for path in files(dir) {
        let stem = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        let text = normalize(&std::fs::read_to_string(&path).unwrap_or_default());
        if legacy.iter().any(|(name, body)| *name == stem && *body == text) {
            let _ = delete(dir, &stem);
        }
    }
}

/// Description from the first line `> Word: description`.
fn description(text: &str) -> String {
    text.trim_start_matches('\u{feff}')
        .lines()
        .next()
        .and_then(|l| l.trim().strip_prefix('>'))
        .and_then(|l| l.split_once(':'))
        .map(|(_, d)| d.trim().to_string())
        .unwrap_or_default()
}

fn files(dir: &Path) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "md"))
        .collect();
    out.sort();
    out
}

/// Templates: (name, description), alphabetically.
pub fn list(dir: &Path) -> Vec<(String, String)> {
    prepare(dir);
    files(dir)
        .into_iter()
        .filter_map(|p| {
            let name = p.file_stem()?.to_string_lossy().to_string();
            let text = std::fs::read_to_string(&p).ok()?;
            Some((name, description(&text)))
        })
        .collect()
}

fn file_of(dir: &Path, name: &str) -> Result<PathBuf> {
    let clean = name.trim().trim_end_matches(".md").replace(['\\', '/', ':', '*', '?', '"', '<', '>', '|'], "");
    if clean.is_empty() {
        bail!("{}", crate::i18n::tr("empty template name"));
    }
    Ok(dir.join(format!("{clean}.md")))
}

/// Whether a template of this name (as it would be saved) exists.
pub fn exists(dir: &Path, name: &str) -> bool {
    file_of(dir, name).map(|p| p.is_file()).unwrap_or(false)
}

pub fn read(dir: &Path, name: &str) -> Result<String> {
    prepare(dir);
    let path = file_of(dir, name)?;
    std::fs::read_to_string(&path).with_context(|| crate::i18n::fmt("there is no template \"{0}\" (available: {1})", &[name.to_string(), list(dir).into_iter().map(|t| t.0).collect::<Vec<_>>().join(", ")]))
}

pub fn save(dir: &Path, name: &str, content: &str) -> Result<String> {
    std::fs::create_dir_all(dir).context(crate::i18n::tr("templates folder"))?;
    let path = file_of(dir, name)?;
    std::fs::write(&path, content).with_context(|| crate::i18n::fmt("saving {0}", &[path.display().to_string()]))?;
    Ok(path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default())
}

/// Language change: the templates for the AI to translate (all of them: every template is the user's own).
pub fn to_translate(dir: &Path) -> Vec<PathBuf> {
    prepare(dir);
    files(dir)
}

/// List of templates for the model instructions: name, tool, group, description.
pub fn prompt_section(dir: &Path) -> String {
    let items = list_meta(dir);
    if items.is_empty() {
        return "<templates>\nNo templates.\n</templates>".into();
    }
    let lines: Vec<String> = items
        .iter()
        .map(|(n, m)| {
            let tool = if m.target.is_empty() { "any tool" } else { m.target.as_str() };
            let group = if m.group.is_empty() { String::new() } else { format!(", group \"{}\"", m.group) };
            format!("- {n} [{tool}{group}]: {}", m.description)
        })
        .collect();
    format!("<templates>\n{}\n</templates>", lines.join("\n"))
}

/// What a template is for (LC-311).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Meta {
    pub description: String,
    /// "confluence", "jira", "word" or "" (any tool).
    pub target: String,
    /// The user's own group, e.g. a level of documentation or a project ("" = none).
    pub group: String,
    /// "" = the model of the settings, "strong" = a stronger model of the same provider, otherwise a model name.
    pub model: String,
}

/// The tools a template can be for ("" = any).
pub const TARGETS: [&str; 4] = ["confluence", "jira", "word", ""];

const DESCRIPTION_KEYS: [&str; 4] = ["szablon", "template", "vorlage", "modèle"];
const TARGET_KEYS: [&str; 4] = ["cel", "target", "ziel", "cible"];
const GROUP_KEYS: [&str; 4] = ["grupa", "group", "gruppe", "groupe"];
const MODEL_KEYS: [&str; 3] = ["model", "modell", "modèle ia"];
const STRONG: [&str; 10] = ["strong", "stronger", "mocny", "mocniejszy", "stark", "stärker", "fort", "plus fort", "silny", "silniejszy"];
const DEFAULT_MODEL: [&str; 5] = ["default", "domyślny", "standard", "par défaut", "-"];

/// The names of the header lines in a language: description, tool, group, model; and the word for a stronger model.
fn keys(lang: Lang) -> [&'static str; 5] {
    match lang {
        Lang::Pl => ["Szablon", "Cel", "Grupa", "Model", "mocniejszy"],
        Lang::En => ["Template", "Target", "Group", "Model", "stronger"],
        Lang::De => ["Vorlage", "Ziel", "Gruppe", "Modell", "stärker"],
        Lang::Fr => ["Modèle", "Cible", "Groupe", "Modèle IA", "plus fort"],
    }
}

/// A header line `> Key: value` with a known key: (which: 0 description, 1 tool, 2 group, 3 model; value).
fn header_line(line: &str, first: bool) -> Option<(usize, String)> {
    let (key, value) = line.trim().strip_prefix('>')?.split_once(':')?;
    let key = key.trim().to_lowercase();
    let value = value.trim().to_string();
    let which = if MODEL_KEYS.contains(&key.as_str()) {
        3
    } else if DESCRIPTION_KEYS.contains(&key.as_str()) {
        0
    } else if TARGET_KEYS.contains(&key.as_str()) {
        1
    } else if GROUP_KEYS.contains(&key.as_str()) {
        2
    } else if first && !key.contains(' ') && key.chars().count() <= 20 {
        // Templates of earlier versions: the first line `> Word: description` in any language.
        0
    } else {
        return None;
    };
    Some((which, value))
}

/// The tool from a header value ("Confluence", "Word (DOCX)", "dowolne"…).
fn target_of(value: &str) -> String {
    let v = value.to_lowercase();
    if v.contains("confluence") {
        "confluence".into()
    } else if v.contains("jira") {
        "jira".into()
    } else if v.contains("word") || v.contains("docx") {
        "word".into()
    } else {
        String::new()
    }
}

/// The header of a template and the rest of its text.
pub fn split_header(text: &str) -> (Meta, String) {
    let text = text.trim_start_matches('\u{feff}').replace("\r\n", "\n");
    let mut meta = Meta::default();
    let mut body_start = 0;
    for (i, line) in text.split('\n').enumerate() {
        match header_line(line, i == 0) {
            Some((which, value)) => {
                match which {
                    0 => meta.description = value,
                    1 => meta.target = target_of(&value),
                    2 => meta.group = value,
                    _ => {
                        let v = value.to_lowercase();
                        meta.model = if STRONG.contains(&v.as_str()) { "strong".into() } else if v.is_empty() || DEFAULT_MODEL.contains(&v.as_str()) { String::new() } else { value };
                    }
                }
                body_start += line.len() + 1;
            }
            None => break,
        }
    }
    let body = text.get(body_start.min(text.len())..).unwrap_or("").trim_start_matches('\n').to_string();
    (meta, body)
}

/// The header of a template in the language of the application followed by its text.
pub fn compose(meta: &Meta, body: &str, lang: Lang) -> String {
    let [description, target, group, model, stronger] = keys(lang);
    let mut out = format!("> {description}: {}\n", meta.description.trim());
    let tool = match meta.target.as_str() {
        "confluence" => "Confluence",
        "jira" => "Jira",
        "word" => "Word",
        _ => "",
    };
    if !tool.is_empty() {
        out.push_str(&format!("> {target}: {tool}\n"));
    }
    if !meta.group.trim().is_empty() {
        out.push_str(&format!("> {group}: {}\n", meta.group.trim()));
    }
    match meta.model.trim() {
        "" => {}
        "strong" => out.push_str(&format!("> {model}: {stronger}\n")),
        name => out.push_str(&format!("> {model}: {name}\n")),
    }
    out.push('\n');
    out.push_str(body.trim_start_matches('\n'));
    out
}

/// Templates with what they are for, alphabetically.
pub fn list_meta(dir: &Path) -> Vec<(String, Meta)> {
    prepare(dir);
    files(dir)
        .into_iter()
        .filter_map(|p| {
            let name = p.file_stem()?.to_string_lossy().to_string();
            Some((name, split_header(&std::fs::read_to_string(&p).ok()?).0))
        })
        .collect()
}

/// Saves a template with its header; with `old_name` (another name) the template is renamed.
pub fn save_with_meta(dir: &Path, old_name: Option<&str>, name: &str, meta: &Meta, body: &str) -> Result<String> {
    if let Some(old) = old_name.filter(|o| o.trim() != name.trim()) {
        if exists(dir, name) {
            bail!("{}", crate::i18n::fmt("a template named \"{0}\" already exists", &[name.to_string()]));
        }
        let saved = save(dir, name, &compose(meta, body, crate::i18n::current()))?;
        let _ = std::fs::remove_file(file_of(dir, old)?);
        return Ok(saved);
    }
    save(dir, name, &compose(meta, body, crate::i18n::current()))
}

/// Moves a template to the folder `bin` of the templates (it can be put back from there).
pub fn delete(dir: &Path, name: &str) -> Result<()> {
    let file = file_of(dir, name)?;
    let bin = dir.join("bin");
    std::fs::create_dir_all(&bin).context(crate::i18n::tr("templates folder"))?;
    let target = bin.join(file.file_name().context("name")?);
    let _ = std::fs::remove_file(&target);
    std::fs::rename(&file, &target).with_context(|| crate::i18n::fmt("cannot remove {0}", &[file.display().to_string()]))
}

/// The text of a template body read from the template file (without the header).
pub fn body(dir: &Path, name: &str) -> Result<String> {
    Ok(split_header(&read(dir, name)?).1)
}

/// A file turned into a template text (Markdown): Markdown and text as they are; Word and HTML with headings, lists and
/// tables; other documents (PDF, PowerPoint, Excel…) as their text.
pub fn to_markdown(file_name: &str, bytes: &[u8]) -> Result<String> {
    let ext = Path::new(file_name).extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    let text = match ext.as_str() {
        "md" | "markdown" | "txt" => String::from_utf8_lossy(bytes).to_string(),
        "docx" | "docm" | "dotx" => docx_markdown(bytes)?,
        "html" | "htm" | "xhtml" | "xml" => html_markdown(&String::from_utf8_lossy(bytes)),
        _ => crate::documents::text_of(file_name, bytes)?,
    };
    if text.trim().is_empty() {
        bail!("{}", crate::i18n::fmt("{0} has no text to make a template from", &[file_name.to_string()]));
    }
    Ok(text.replace("\r\n", "\n"))
}

/// A heading level from a Word paragraph style (Heading1, Nagwek1, berschrift1, Titre1, Title…).
fn heading_level(style: &str) -> Option<usize> {
    let s = style.to_lowercase();
    if s == "title" || s.starts_with("tytu") || s == "titel" {
        return Some(1);
    }
    let digit = s.chars().last()?.to_digit(10)? as usize;
    let word = s.trim_end_matches(|c: char| c.is_ascii_digit());
    let heading = ["heading", "nagwek", "nagłówek", "berschrift", "überschrift", "titre", "kop"].iter().any(|w| word.ends_with(w) || word.contains(w));
    (heading && (1..=6).contains(&digit)).then_some(digit)
}

/// The start of the next paragraph `<w:p>` / `<w:p …>` (not `<w:pPr>`, `<w:proofErr>`…).
fn next_paragraph(xml: &str) -> Option<usize> {
    let mut from = 0;
    while let Some(p) = xml[from..].find("<w:p") {
        let at = from + p;
        if matches!(xml.as_bytes().get(at + 4), Some(b'>') | Some(b' ')) {
            return Some(at);
        }
        from = at + 4;
    }
    None
}

/// A Word paragraph as a Markdown line.
fn paragraph_line(p: &str) -> String {
    let text = crate::documents::texts(p, "w:t").join("");
    if text.trim().is_empty() {
        return String::new();
    }
    let style = p.find("<w:pStyle").and_then(|i| crate::documents::attr(&p[i..], "w:val")).unwrap_or_default();
    if let Some(level) = heading_level(&style) {
        return format!("{} {}", "#".repeat(level), text.trim());
    }
    if p.contains("<w:numPr>") || style.to_lowercase().contains("list") || style.to_lowercase().contains("lista") {
        return format!("- {}", text.trim());
    }
    text
}

/// A Word document as Markdown: headings from the paragraph styles, lists, tables.
fn docx_markdown(bytes: &[u8]) -> Result<String> {
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).context("the DOCX is not a valid ZIP")?;
    let xml = crate::documents::part(&mut archive, "word/document.xml").context("word/document.xml is missing from the DOCX")?;
    let mut out: Vec<String> = Vec::new();
    let mut rest = xml.as_str();
    loop {
        let table = rest.find("<w:tbl>");
        let para = next_paragraph(rest);
        match (table, para) {
            (Some(t), p) if p.is_none_or(|p| t < p) => {
                let end = rest[t..].find("</w:tbl>").map_or(rest.len(), |e| t + e + "</w:tbl>".len());
                let rows: Vec<Vec<String>> = rest[t..end]
                    .split("</w:tr>")
                    .filter(|r| r.contains("<w:tc"))
                    .map(|r| {
                        r.split("</w:tc>")
                            .filter(|c| c.contains("<w:t"))
                            .map(|c| crate::documents::texts(c, "w:t").join("").replace('|', "/").trim().to_string())
                            .collect()
                    })
                    .filter(|r: &Vec<String>| !r.is_empty())
                    .collect();
                if !rows.is_empty() {
                    out.push(String::new());
                    for (i, row) in rows.iter().enumerate() {
                        out.push(format!("| {} |", row.join(" | ")));
                        if i == 0 {
                            out.push(format!("|{}", "---|".repeat(row.len())));
                        }
                    }
                    out.push(String::new());
                }
                rest = &rest[end..];
            }
            (_, Some(p)) => {
                let end = rest[p..].find("</w:p>").map_or(rest.len(), |e| p + e + "</w:p>".len());
                let line = paragraph_line(&rest[p..end]);
                if line.starts_with('#') {
                    out.push(String::new());
                }
                out.push(line);
                rest = &rest[end..];
            }
            _ => break,
        }
    }
    let mut text = out.join("\n");
    while text.contains("\n\n\n") {
        text = text.replace("\n\n\n", "\n\n");
    }
    Ok(text.trim().to_string())
}

/// HTML (also a Confluence page in its storage format) as Markdown: headings, paragraphs, lists, tables.
pub fn html_markdown(html: &str) -> String {
    let mut out = String::new();
    let mut rest = html;
    let mut skip = 0usize; // inside script, style or a macro parameter
    let mut row_cells = 0usize;
    let mut first_row = false;
    while !rest.is_empty() {
        let Some(lt) = rest.find('<') else {
            if skip == 0 {
                out.push_str(&entities(rest));
            }
            break;
        };
        if skip == 0 {
            out.push_str(&entities(&rest[..lt]).replace('\n', " "));
        }
        let Some(gt) = rest[lt..].find('>') else { break };
        let tag = &rest[lt + 1..lt + gt];
        rest = &rest[lt + gt + 1..];
        let closing = tag.starts_with('/');
        let name = tag.trim_start_matches('/').split(|c: char| c.is_whitespace() || c == '/').next().unwrap_or("").to_lowercase();
        if matches!(name.as_str(), "script" | "style" | "ac:parameter") {
            if closing { skip = skip.saturating_sub(1) } else if !tag.ends_with('/') { skip += 1 }
            continue;
        }
        if skip > 0 {
            continue;
        }
        match (name.as_str(), closing) {
            (h, false) if h.len() == 2 && h.starts_with('h') && h[1..].parse::<usize>().is_ok_and(|n| (1..=6).contains(&n)) => {
                out.push_str(&format!("\n\n{} ", "#".repeat(h[1..].parse().unwrap_or(1))));
            }
            (h, true) if h.len() == 2 && h.starts_with('h') && h[1..].parse::<usize>().is_ok() => out.push('\n'),
            ("p" | "div" | "br" | "ul" | "ol", _) => out.push('\n'),
            ("li", false) => out.push_str("\n- "),
            ("table", false) => {
                first_row = true;
                out.push('\n');
            }
            ("table", true) => out.push('\n'),
            ("tr", false) => {
                row_cells = 0;
                out.push_str("\n|");
            }
            ("tr", true) => {
                if first_row && row_cells > 0 {
                    out.push_str(&format!("\n|{}", "---|".repeat(row_cells)));
                }
                first_row = false;
            }
            ("td" | "th", false) => {
                row_cells += 1;
                out.push(' ');
            }
            ("td" | "th", true) => out.push_str(" |"),
            _ => {}
        }
    }
    // Lines trimmed; at most one empty line in a row; no empty list items.
    let mut lines: Vec<String> = Vec::new();
    for line in out.lines().map(|l| l.split_whitespace().collect::<Vec<_>>().join(" ")) {
        if line == "-" || (line.is_empty() && lines.last().is_some_and(|l: &String| l.is_empty())) {
            continue;
        }
        lines.push(line);
    }
    lines.join("\n").trim().to_string()
}

fn entities(text: &str) -> String {
    text.replace("&nbsp;", " ").replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&#39;", "'").replace("&apos;", "'").replace("&amp;", "&")
}

/// A Confluence page as a template text: by its address (…/pages/123…, ?pageId=123) or id.
pub fn from_confluence(cfg: &crate::config::AppConfig, page: &str) -> Result<(String, String)> {
    let page = page.trim();
    let id = if page.chars().all(|c| c.is_ascii_digit()) {
        page.to_string()
    } else if let Some(i) = page.find("pageId=") {
        page[i + 7..].chars().take_while(char::is_ascii_digit).collect()
    } else if let Some(i) = page.find("/pages/") {
        page[i + 7..].chars().take_while(char::is_ascii_digit).collect()
    } else {
        String::new()
    };
    if id.is_empty() {
        bail!("{}", crate::i18n::tr("give the address of the Confluence page (with /pages/ID or pageId=ID) or its id"));
    }
    let (title, storage) = crate::tools::confluence_connection(cfg)?.page_body(&id)?;
    Ok((title, html_markdown(&storage)))
}

#[cfg(test)]
mod tests {
    use crate::i18n::Lang;

    #[test]
    fn header_with_tool_group_and_model() {
        let text = "> Szablon: Opis techniczny\n> Cel: Confluence\n> Grupa: Poziom techniczny\n> Model: mocniejszy\n\n# 1. Architektura\n> wskazówka\n";
        let (meta, body) = super::split_header(text);
        assert_eq!(meta, super::Meta { description: "Opis techniczny".into(), target: "confluence".into(), group: "Poziom techniczny".into(), model: "strong".into() });
        assert!(body.starts_with("# 1. Architektura"), "{body}");
        let again = super::compose(&meta, &body, Lang::En);
        assert!(again.starts_with("> Template: Opis techniczny\n> Target: Confluence\n> Group: Poziom techniczny\n> Model: stronger\n"), "{again}");
        assert_eq!(super::split_header(&again).0, meta);
        // Older templates: only the description; a hint below it stays in the body.
        let (old, rest) = super::split_header("> Template: BRD\n> Requirements come from the conversation.\n# 1. Goal");
        assert_eq!((old.description.as_str(), old.target.as_str()), ("BRD", ""));
        assert!(rest.starts_with("> Requirements"), "{rest}");
        assert_eq!(super::split_header("> Modèle: X\n> Modèle IA: claude-opus-5-5\n").0.model, "claude-opus-5-5");
    }

    #[test]
    fn html_and_word_keep_headings_and_tables() {
        let md = super::html_markdown("<h1>Cel</h1><p>Opis <b>A</b> &amp; B</p><ac:structured-macro><ac:parameter ac:name=\"x\">toc</ac:parameter></ac:structured-macro><h2>Wymagania</h2><table><tr><th>Id</th><th>Opis</th></tr><tr><td>W1</td><td>Login</td></tr></table><ul><li>jeden</li></ul>");
        assert_eq!(md, "# Cel\n\nOpis A & B\n\n## Wymagania\n\n| Id | Opis |\n|---|---|\n| W1 | Login |\n\n- jeden");
        assert_eq!(super::heading_level("Nagwek2"), Some(2));
        assert_eq!(super::heading_level("Heading1"), Some(1));
        assert_eq!(super::heading_level("Normal"), None);
        let p = r#"<w:p><w:pPr><w:pStyle w:val="Heading2"/></w:pPr><w:r><w:t>Zakres</w:t></w:r></w:p>"#;
        assert_eq!(super::paragraph_line(p), "## Zakres");
    }

    #[test]
    fn untouched_old_templates_go_to_the_bin() {
        let dir = std::env::temp_dir().join(format!("loonar-templates-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        for (name, text) in super::legacy_defaults(Lang::Pl) {
            std::fs::write(dir.join(format!("{name}.md")), text.replace('\n', "\r\n")).unwrap();
        }
        super::save(&dir, "My template", "> Szablon: test\n# A").unwrap();
        super::save(&dir, "Opis procesu", "> Szablon: modified\n# B").unwrap();
        let names: Vec<String> = super::list_meta(&dir).into_iter().map(|(n, _)| n).collect();
        assert_eq!(names, vec!["My template".to_string(), "Opis procesu".to_string()], "only own and modified templates stay");
        assert!(dir.join("bin").join("BRD - Wymagania biznesowe.md").is_file());
        assert_eq!(super::to_translate(&dir).len(), 2);
        let _ = std::fs::remove_dir_all(&dir);
    }
}

//! Requirements document ↔ model: requirements read from a document (Word, text/Markdown, a Confluence page or text
//! given in the conversation) are matched with the model deterministically: requirement elements and requirement ids
//! in names, aliases, notes and tagged values, existing trace / realisation relationships, and word similarity.
//! The result is a traceability matrix: covered, likely covered (candidates), not covered, and model elements that
//! no requirement mentions. The assistant judges the candidates and, on request, saves trace relationships.
//! With a source folder, every requirement is followed further: model element → code → test → issue key, and the
//! report says where the chain breaks.

use crate::config::AppConfig;
use crate::tools::query_rows;
use anyhow::{bail, Context, Result};
use std::collections::{HashMap, HashSet};
use std::io::Read;
use std::path::Path;

#[derive(Clone, Debug)]
pub struct Requirement {
    pub id: String,
    pub text: String,
}

struct ModelElement {
    id: String,
    name: String,
    kind: String,
    package: String,
    text: String,
    tokens: HashSet<String>,
}

/// Text of a Word document: paragraphs, and table rows as "cell | cell".
pub fn docx_text(path: &std::path::Path) -> Result<String> {
    let file = std::fs::File::open(path).with_context(|| crate::i18n::fmt("cannot read {0}", &[path.display().to_string()]))?;
    let mut zip = zip::ZipArchive::new(file).context("DOCX")?;
    let mut xml = String::new();
    zip.by_name("word/document.xml").context("word/document.xml")?.read_to_string(&mut xml)?;
    Ok(word_xml_text(&xml))
}

fn word_xml_text(xml: &str) -> String {
    let mut out = String::new();
    let mut cell = String::new();
    let mut cells: Vec<String> = Vec::new();
    let mut in_table = 0;
    let mut i = 0;
    let bytes = xml.as_bytes();
    while i < bytes.len() {
        if bytes[i] == b'<' {
            let end = xml[i..].find('>').map(|e| i + e).unwrap_or(bytes.len() - 1);
            let tag = &xml[i + 1..end];
            let name = tag.split_whitespace().next().unwrap_or("");
            match name {
                "w:tbl" => in_table += 1,
                "/w:tbl" => in_table -= 1,
                "/w:tc" => cells.push(std::mem::take(&mut cell).trim().to_string()),
                "/w:tr" => {
                    out.push_str(&cells.join(" | "));
                    out.push('\n');
                    cells.clear();
                }
                "/w:p" => {
                    if in_table > 0 {
                        cell.push(' ');
                    } else {
                        out.push('\n');
                    }
                }
                "w:tab/" | "w:br/" => {
                    if in_table > 0 { cell.push(' ') } else { out.push(' ') }
                }
                _ => {}
            }
            i = end + 1;
            continue;
        }
        let next = xml[i..].find('<').map(|e| i + e).unwrap_or(bytes.len());
        let text = xml[i..next].replace("&lt;", "<").replace("&gt;", ">").replace("&amp;", "&").replace("&quot;", "\"").replace("&apos;", "'");
        if in_table > 0 { cell.push_str(&text) } else { out.push_str(&text) }
        i = next;
    }
    out
}

/// Text of HTML / Confluence storage format, block elements as lines, table rows as "cell | cell".
pub fn html_text(html: &str) -> String {
    let mut s = html.to_string();
    for (a, b) in [("</p>", "\n"), ("<br/>", "\n"), ("<br />", "\n"), ("</li>", "\n"), ("</h1>", "\n"), ("</h2>", "\n"), ("</h3>", "\n"), ("</h4>", "\n"), ("</tr>", "\n"), ("</td>", " | "), ("</th>", " | ")] {
        s = s.replace(a, b);
    }
    let mut out = String::new();
    let mut tag = false;
    for c in s.chars() {
        match c {
            '<' => tag = true,
            '>' if tag => tag = false,
            _ if !tag => out.push(c),
            _ => {}
        }
    }
    out.replace("&nbsp;", " ").replace("&lt;", "<").replace("&gt;", ">").replace("&amp;", "&").replace("&quot;", "\"")
}

/// A requirement id at the start of a line or cell: W1, W1.2, R.1.3, REQ-12, FR_01, NFR-REC-01, US-7, 4.2.1.
fn leading_id(text: &str) -> Option<(String, String)> {
    let t = text.trim_start_matches(|c: char| c == '|' || c == '-' || c == '*' || c == '•' || c.is_whitespace());
    let mut end = 0;
    let mut has_digit = false;
    let mut has_alpha = false;
    for (i, c) in t.char_indices() {
        if c.is_ascii_uppercase() || c.is_ascii_digit() || c == '.' || c == '-' || c == '_' {
            has_digit |= c.is_ascii_digit();
            has_alpha |= c.is_ascii_uppercase();
            end = i + c.len_utf8();
        } else {
            break;
        }
    }
    let id = t[..end].trim_end_matches(['.', '-', '_']);
    if !has_digit || id.len() < 2 || id.len() > 20 || (!has_alpha && id.matches('.').count() == 0) {
        return None;
    }
    let rest = t[end..].trim_start_matches([' ', ':', ')', '|', '\t']).trim();
    if rest.chars().filter(|c| c.is_alphabetic()).count() < 8 {
        return None;
    }
    Some((id.to_string(), rest.to_string()))
}

/// The text of a requirements document: Word (.docx), HTML or plain text / Markdown.
pub fn read_document(path: &Path) -> Result<String> {
    let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    match ext.as_str() {
        "docx" => docx_text(path),
        "html" | "htm" => Ok(html_text(&std::fs::read_to_string(path).with_context(|| crate::i18n::fmt("cannot read {0}", &[path.display().to_string()]))?)),
        "pdf" | "pptx" | "xlsx" | "vsdx" => crate::documents::text_of(&path.display().to_string(), &std::fs::read(path).with_context(|| crate::i18n::fmt("cannot read {0}", &[path.display().to_string()]))?),
        _ => std::fs::read_to_string(path).with_context(|| crate::i18n::fmt("cannot read {0}", &[path.display().to_string()])),
    }
}

/// Requirements of a document: lines (or table rows) starting with a requirement id. Without any ids, every
/// sentence-like line becomes a requirement numbered D1, D2…
pub fn extract(text: &str) -> Vec<Requirement> {
    let mut found: Vec<Requirement> = Vec::new();
    let mut seen = HashSet::new();
    for line in text.lines() {
        if let Some((id, rest)) = leading_id(line) {
            if seen.insert(id.clone()) {
                found.push(Requirement { id, text: rest.replace(" | ", " · ").trim_end_matches(" ·").to_string() });
            }
        }
    }
    if found.is_empty() {
        for line in text.lines().map(str::trim).filter(|l| l.split_whitespace().count() >= 5) {
            found.push(Requirement { id: format!("D{}", found.len() + 1), text: line.to_string() });
        }
    }
    found
}

const STOP: &[&str] = &[
    "the", "and", "for", "with", "that", "this", "from", "shall", "must", "should", "will", "can", "are", "has", "have", "into", "when", "user", "system",
    "oraz", "jest", "musi", "może", "przez", "który", "która", "które", "jako", "dla", "się", "nie", "lub", "albo", "jeśli", "gdy", "aby", "żeby",
    "system", "systemu", "użytkownik", "użytkownika", "powinien", "powinna", "zostać", "będzie", "każdy", "wszystkie",
];

/// Words of a text for similarity: lowercase, without short and common words, cut to the first 6 letters
/// (a simple stem that joins Polish and English inflected forms).
fn tokens(text: &str) -> HashSet<String> {
    let mut out = HashSet::new();
    // CamelCase names become separate words (OrderService → order service).
    let mut spaced = String::new();
    let mut prev_lower = false;
    for c in text.chars() {
        if c.is_uppercase() && prev_lower {
            spaced.push(' ');
        }
        prev_lower = c.is_lowercase();
        spaced.push(c);
    }
    for w in spaced.to_lowercase().split(|c: char| !c.is_alphanumeric()) {
        if w.chars().count() < 4 || w.chars().all(|c| c.is_ascii_digit()) || STOP.contains(&w) {
            continue;
        }
        out.insert(w.chars().take(6).collect());
    }
    out
}

fn elements(cfg: &AppConfig, package: Option<i64>) -> Result<Vec<ModelElement>> {
    let filter = match package {
        Some(p) => {
            let ids: Vec<String> = query_rows(cfg, &format!(
                "with recursive tree(package_id) as (select package_id from t_package where package_id = {p} \
                 union all select x.package_id from t_package x join tree t on x.parent_id = t.package_id) select package_id from tree"
            ))?
            .into_iter()
            .map(|r| r[0].clone())
            .collect();
            if ids.is_empty() {
                bail!("{}", crate::i18n::fmt("there is no package with id {0}", &[p.to_string()]));
            }
            format!(" and o.package_id in ({})", ids.join(","))
        }
        None => String::new(),
    };
    let rows = query_rows(cfg, &format!(
        "select o.object_id as c0, coalesce(o.name,'') as c1, o.object_type as c2, coalesce(o.stereotype,'') as c3, coalesce(o.alias,'') as c4, \
         coalesce(o.note,'') as c5, coalesce(p.name,'') as c6 from t_object o left join t_package p on p.package_id = o.package_id \
         where o.object_type not in ('Package','Note','Text','Boundary','Port','ActionPin','Decision','MergeNode','Synchronization','InitialNode','ActivityFinal','FlowFinal','StateNode'){filter}"
    ))?;
    let mut tags: HashMap<String, String> = HashMap::new();
    for r in query_rows(cfg, "select object_id as c0, coalesce(property,'') as c1, coalesce(value,'') as c2 from t_objectproperties").unwrap_or_default() {
        let e = tags.entry(r[0].clone()).or_default();
        e.push(' ');
        e.push_str(&r[1]);
        e.push('=');
        e.push_str(&r[2]);
    }
    Ok(rows
        .into_iter()
        .map(|r| {
            let text = format!("{} {} {} {}", r[1], r[4], r[5], tags.get(&r[0]).cloned().unwrap_or_default());
            ModelElement {
                id: r[0].clone(),
                name: r[1].clone(),
                kind: if r[3].is_empty() { r[2].clone() } else { format!("{} «{}»", r[2], r[3]) },
                package: r[6].clone(),
                tokens: tokens(&format!("{} {}", r[1], r[5])),
                text,
            }
        })
        .collect())
}

/// Whether the id appears in the text as a whole token (W1 does not match W12).
fn mentions(text: &str, id: &str) -> bool {
    let upper = text.to_uppercase();
    let id = id.to_uppercase();
    let mut start = 0;
    while let Some(i) = upper[start..].find(&id) {
        let at = start + i;
        let before = upper[..at].chars().last();
        let after = upper[at + id.len()..].chars().next();
        let boundary = |c: Option<char>| c.is_none_or(|c| !(c.is_alphanumeric() || c == '.' || c == '_' || c == '-'));
        if boundary(before) && (boundary(after) || after == Some('.') && !upper[at + id.len() + 1..].starts_with(|c: char| c.is_ascii_digit())) {
            return true;
        }
        start = at + id.len();
    }
    false
}

/// Coverage of one requirement in the model.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Coverage {
    Covered,
    Candidates,
    Missing,
    ContentDiffers,
}

/// One requirement with the elements that carry it and the candidates by word similarity.
pub struct TraceRow {
    pub id: String,
    pub text: String,
    pub coverage: Coverage,
    /// Elements carrying the id or linked to them by a trace relationship: (id, name, kind, text for searching).
    pub traced: Vec<(String, String, String, String)>,
    /// Candidates as "name [kind] #id (score)".
    pub candidates: Vec<String>,
}

/// The traceability matrix as data.
pub struct Trace {
    pub rows: Vec<TraceRow>,
    pub elements_checked: usize,
    pub unreferenced: Vec<String>,
}

impl Trace {
    pub fn count(&self, coverage: Coverage) -> usize {
        self.rows.iter().filter(|r| r.coverage == coverage).count()
    }
}

fn coverage_label(c: Coverage) -> &'static str {
    match c {
        Coverage::Covered => crate::i18n::tr("covered"),
        Coverage::Candidates => crate::i18n::tr("candidates"),
        Coverage::Missing => crate::i18n::tr("NOT COVERED"),
        Coverage::ContentDiffers => crate::i18n::tr("ID MATCHES, CONTENT DIFFERS"),
    }
}

/// Requirements matched with the model (elements of one package with subpackages, or the whole model).
pub fn trace_data(cfg: &AppConfig, requirements: &[Requirement], package: Option<i64>) -> Result<Trace> {
    if requirements.is_empty() {
        bail!("{}", crate::i18n::tr("no requirements found in the document (lines starting with an id such as W1, R.1.2, REQ-12)"));
    }
    let elements = elements(cfg, package)?;
    // Trace / realisation relationships of requirement elements.
    let mut traced: HashMap<String, Vec<String>> = HashMap::new();
    for r in query_rows(cfg, "select s.object_id as c0, t.object_id as c1 from t_connector c join t_object s on s.object_id = c.start_object_id join t_object t on t.object_id = c.end_object_id \
                             where c.connector_type in ('Realisation','Abstraction','Dependency','Trace') or coalesce(c.stereotype,'') in ('trace','satisfy','refine','realize')").unwrap_or_default() {
        traced.entry(r[0].clone()).or_default().push(r[1].clone());
        traced.entry(r[1].clone()).or_default().push(r[0].clone());
    }
    let by_id: HashMap<&str, &ModelElement> = elements.iter().map(|e| (e.id.as_str(), e)).collect();
    let mut referenced: HashSet<String> = HashSet::new();
    let mut rows = Vec::new();
    for req in requirements {
        // 1. Elements that carry the requirement id (a Requirement element named "W1 …", an alias, a note, a tag).
        let direct: Vec<&ModelElement> = elements.iter().filter(|e| mentions(&e.text, &req.id)).collect();
        // 2. Elements linked to those by a trace / realisation relationship.
        let mut linked: Vec<&ModelElement> = Vec::new();
        for d in &direct {
            for other in traced.get(&d.id).into_iter().flatten() {
                if let Some(e) = by_id.get(other.as_str()) {
                    if !direct.iter().any(|x| x.id == e.id) && !linked.iter().any(|x| x.id == e.id) {
                        linked.push(e);
                    }
                }
            }
        }
        // 3. Word similarity: candidates sharing the most meaningful words with the requirement.
        let req_tokens = tokens(&req.text);
        let mut similar: Vec<(f64, &ModelElement)> = elements
            .iter()
            .filter(|e| !direct.iter().any(|x| x.id == e.id) && !linked.iter().any(|x| x.id == e.id))
            .filter_map(|e| {
                let common = req_tokens.intersection(&e.tokens).count();
                let name_hits = tokens(&e.name).intersection(&req_tokens).count();
                let score = common as f64 / (req_tokens.len().max(1) as f64).sqrt() + name_hits as f64;
                (common >= 1 && score >= 1.0).then_some((score, e))
            })
            .collect();
        similar.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        similar.truncate(3);
        // The id matches but the content does not: no meaningful word of the requirement in the elements carrying the id.
        let id_token = tokens(&req.id);
        let content_differs = !direct.is_empty()
            && req_tokens.len() >= 3
            && direct.iter().all(|e| req_tokens.difference(&id_token).filter(|w| e.tokens.contains(*w)).count() == 0);
        let coverage = if content_differs {
            Coverage::ContentDiffers
        } else if !direct.is_empty() || !linked.is_empty() {
            Coverage::Covered
        } else if !similar.is_empty() {
            Coverage::Candidates
        } else {
            Coverage::Missing
        };
        for e in direct.iter().chain(linked.iter()) {
            referenced.insert(e.id.clone());
        }
        for (_, e) in &similar {
            referenced.insert(e.id.clone());
        }
        rows.push(TraceRow {
            id: req.id.clone(),
            text: req.text.clone(),
            coverage,
            traced: direct.iter().chain(linked.iter()).map(|e| (e.id.clone(), e.name.clone(), e.kind.clone(), e.text.clone())).collect(),
            candidates: similar.iter().map(|(s, e)| format!("{} [{}] #{} ({:.1})", e.name, e.kind, e.id, s)).collect(),
        });
    }
    // Elements of the main kinds that no requirement refers to.
    let main = ["UseCase", "Requirement", "Activity", "Component", "Class", "Interface", "Feature"];
    let unreferenced: Vec<String> = elements
        .iter()
        .filter(|e| !referenced.contains(&e.id) && main.iter().any(|k| e.kind.starts_with(k)))
        .take(150)
        .map(|e| format!("{} [{}] #{} ({})", e.name, e.kind, e.id, e.package))
        .collect();
    Ok(Trace { rows, elements_checked: elements.len(), unreferenced })
}

/// The traceability matrix as text for the assistant (and a table for Excel); with a source folder also the chain
/// requirement → model → code → test → issue.
pub fn trace(cfg: &AppConfig, requirements: &[Requirement], package: Option<i64>) -> Result<String> {
    trace_with_code(cfg, requirements, package, None)
}

pub fn trace_with_code(cfg: &AppConfig, requirements: &[Requirement], package: Option<i64>, code: Option<&Path>) -> Result<String> {
    let t = trace_data(cfg, requirements, package)?;
    let mut out = vec![crate::i18n::fmt(
        "Traceability: {0} requirements – covered {1}, only candidates {2}, NOT covered {3}, id matches but content differs {6}; model elements checked: {4}; elements no requirement refers to: {5}.",
        &[t.rows.len().to_string(), t.count(Coverage::Covered).to_string(), t.count(Coverage::Candidates).to_string(), t.count(Coverage::Missing).to_string(), t.elements_checked.to_string(), t.unreferenced.len().to_string(), t.count(Coverage::ContentDiffers).to_string()],
    )];
    out.push("requirement | text | status | traced elements (id in the model or a trace relationship) | candidates by word similarity (score)".into());
    for r in &t.rows {
        out.push(format!(
            "{} | {} | {} | {} | {}",
            r.id,
            r.text.chars().take(160).collect::<String>(),
            coverage_label(r.coverage),
            r.traced.iter().map(|(id, name, kind, _)| format!("{name} [{kind}] #{id}")).collect::<Vec<_>>().join("; "),
            r.candidates.join("; ")
        ));
    }
    if !t.unreferenced.is_empty() {
        out.push(format!("\n{}:", crate::i18n::tr("Model elements no requirement refers to")));
        out.extend(t.unreferenced.iter().cloned());
    }
    if let Some(root) = code {
        let rows = chain(&t, requirements, root)?;
        out.push(String::new());
        out.push(chain_report(&rows));
    }
    out.push(crate::i18n::tr("Judge the candidates yourself (read the elements if needed): a candidate is only a hint. Point out requirements that contradict the model.").to_string());
    Ok(out.join("\n"))
}

/// Where the chain of a requirement ends.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum ChainStatus {
    Complete,
    NotInModel,
    NoCode,
    NoTests,
}

pub fn chain_label(s: ChainStatus) -> &'static str {
    match s {
        ChainStatus::Complete => crate::i18n::tr("complete"),
        ChainStatus::NotInModel => crate::i18n::tr("BROKEN: not in the model"),
        ChainStatus::NoCode => crate::i18n::tr("BROKEN: no code"),
        ChainStatus::NoTests => crate::i18n::tr("BROKEN: no tests"),
    }
}

/// One requirement along the chain requirement → model → code → test → issue.
pub struct ChainRow {
    pub id: String,
    pub model: Vec<String>,
    pub code: Vec<String>,
    pub tests: Vec<String>,
    pub issues: Vec<String>,
    pub status: ChainStatus,
}

/// Whether a path is a test: a test folder or a test file name (OrderTest.java, OrderIT.java, order.spec.ts, test_order.py…).
pub fn is_test_path(rel: &str) -> bool {
    let p = rel.replace('\\', "/");
    let file = p.rsplit('/').next().unwrap_or(&p);
    let stem = file.split('.').next().unwrap_or(file);
    let (lp, lfile, lstem) = (p.to_lowercase(), file.to_lowercase(), stem.to_lowercase());
    let folder = lp.starts_with("test/") || lp.starts_with("tests/") || lp.contains("/test/") || lp.contains("/tests/") || lp.contains("__tests__/");
    folder || stem.ends_with("Test") || stem.ends_with("Tests") || stem.ends_with("IT") || lstem.ends_with("_test") || lstem.starts_with("test_")
        || lfile.contains(".spec.") || lfile.contains(".test.") || lfile.ends_with(".feature") || lfile.ends_with(".robot")
}

/// The name a test file is about: OrderTest.java → order, order.spec.ts → order, test_order.py → order.
fn test_subject(rel: &str) -> String {
    let file = rel.rsplit(['/', '\\']).next().unwrap_or(rel);
    let stem = file.split('.').next().unwrap_or(file);
    let stem = stem.strip_prefix("test_").unwrap_or(stem);
    let stem = stem.trim_end_matches("Tests").trim_end_matches("Test").trim_end_matches("IT").trim_end_matches("_test");
    crate::code_drift::norm(stem)
}

/// Whether an identifier appears in the text as a whole word.
fn has_word(text: &str, word: &str) -> bool {
    if word.is_empty() {
        return false;
    }
    let ident = |c: char| c.is_alphanumeric() || c == '_';
    let mut start = 0;
    while let Some(i) = text[start..].find(word) {
        let at = start + i;
        let before = text[..at].chars().last();
        let after = text[at + word.len()..].chars().next();
        if !before.is_some_and(ident) && !after.is_some_and(ident) {
            return true;
        }
        start = at + word.len();
    }
    false
}

/// Issue keys (ABC-123) in a text, except the given requirement ids.
pub fn issue_keys(text: &str, except: &HashSet<String>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for w in text.split(|c: char| !(c.is_ascii_alphanumeric() || c == '-')) {
        let Some((project, number)) = w.split_once('-') else { continue };
        let ok = project.len() >= 2
            && project.len() <= 10
            && project.starts_with(|c: char| c.is_ascii_uppercase())
            && project.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
            && !number.is_empty()
            && number.len() <= 7
            && number.chars().all(|c| c.is_ascii_digit());
        // Standards and encodings look like keys (UTF-8, ISO-9001, SHA-256): not issues.
        const NOT_PROJECTS: &[&str] = &["UTF", "ISO", "SHA", "RFC", "TLS", "SSL", "IEC", "EN", "DIN", "MD", "AES", "RSA", "WCAG", "PCI", "GDPR"];
        if ok && !NOT_PROJECTS.contains(&project) && !except.contains(&w.to_uppercase()) && !out.iter().any(|k| k == w) {
            out.push(w.to_string());
        }
    }
    out
}

/// Text files of a source folder (code, tests, API definitions): (path relative to the folder, content).
fn source_files(root: &Path) -> Result<Vec<(String, String)>> {
    if !root.is_dir() {
        bail!("{}", crate::i18n::fmt("folder {0} does not exist", &[root.display().to_string()]));
    }
    const EXT: &[&str] = &["java", "cs", "kt", "ts", "tsx", "js", "jsx", "rs", "py", "go", "sql", "yaml", "yml", "json", "feature", "robot", "groovy", "scala", "swift", "php", "rb"];
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            if path.is_dir() {
                if !crate::code_drift::SKIP_DIRS.contains(&name.as_str()) && !name.starts_with('.') {
                    stack.push(path);
                }
                continue;
            }
            let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
            if !EXT.contains(&ext.as_str()) || out.len() >= crate::code_drift::MAX_FILES || entry.metadata().map(|m| m.len() > crate::code_drift::MAX_FILE_BYTES).unwrap_or(true) {
                continue;
            }
            if let Ok(text) = std::fs::read_to_string(&path) {
                out.push((path.strip_prefix(root).unwrap_or(&path).display().to_string().replace('\\', "/"), text));
            }
        }
    }
    out.sort();
    Ok(out)
}

/// The chain requirement → model element → code → test → issue for every requirement of the matrix.
pub fn chain(t: &Trace, requirements: &[Requirement], root: &Path) -> Result<Vec<ChainRow>> {
    let files = source_files(root)?;
    let code_model = crate::code_drift::scan(root)?;
    // Code types by normalized name → files declaring them.
    let mut declared: HashMap<String, Vec<String>> = HashMap::new();
    for (name, (_, _, file)) in &code_model.types {
        declared.entry(crate::code_drift::norm(name)).or_default().push(file.replace('\\', "/"));
    }
    let tests: Vec<&(String, String)> = files.iter().filter(|(p, _)| is_test_path(p)).collect();
    let ids: HashSet<String> = requirements.iter().map(|r| r.id.to_uppercase()).collect();
    let mut rows = Vec::new();
    for r in &t.rows {
        let model: Vec<String> = r.traced.iter().map(|(id, name, kind, _)| format!("{name} [{kind}] #{id}")).collect();
        // Code: files declaring a type named like a traced element, and source files naming the requirement id.
        let mut code: Vec<String> = Vec::new();
        let mut subjects: Vec<(String, String)> = Vec::new();
        for (_, name, _, _) in &r.traced {
            if let Some(found) = declared.get(&crate::code_drift::norm(name)) {
                for f in found {
                    if !is_test_path(f) && !code.contains(f) {
                        code.push(f.clone());
                    }
                }
                if !subjects.iter().any(|(n, _)| n == name) {
                    subjects.push((name.clone(), crate::code_drift::norm(name)));
                }
            }
        }
        for (path, text) in &files {
            if !is_test_path(path) && mentions(text, &r.id) && !code.contains(path) {
                code.push(path.clone());
            }
        }
        // Tests: test files naming the requirement id, or about a type of the code above (file name or a whole word in the text).
        let mut tested: Vec<String> = Vec::new();
        for (path, text) in &tests {
            let about = subjects.iter().any(|(name, n)| test_subject(path) == *n || has_word(text, name.replace(' ', "").as_str()));
            if (mentions(text, &r.id) || about) && !tested.contains(path) {
                tested.push(path.clone());
            }
        }
        // Issues: keys in the requirement and in the traced elements (name, alias, notes, tagged values).
        let mut issues = issue_keys(&r.text, &ids);
        for (_, _, _, text) in &r.traced {
            for k in issue_keys(text, &ids) {
                if !issues.contains(&k) {
                    issues.push(k);
                }
            }
        }
        let status = if !matches!(r.coverage, Coverage::Covered | Coverage::ContentDiffers) {
            ChainStatus::NotInModel
        } else if code.is_empty() {
            ChainStatus::NoCode
        } else if tested.is_empty() {
            ChainStatus::NoTests
        } else {
            ChainStatus::Complete
        };
        rows.push(ChainRow { id: r.id.clone(), model, code, tests: tested, issues, status });
    }
    Ok(rows)
}

/// The chain as a text table with a summary line.
pub fn chain_report(rows: &[ChainRow]) -> String {
    let count = |s: ChainStatus| rows.iter().filter(|r| r.status == s).count().to_string();
    let mut out = vec![crate::i18n::fmt(
        "Chain requirement → model → code → test → issue: complete {0}; broken: not in the model {1}, no code {2}, no tests {3}.",
        &[count(ChainStatus::Complete), count(ChainStatus::NotInModel), count(ChainStatus::NoCode), count(ChainStatus::NoTests)],
    )];
    out.push("requirement | chain | model | code | tests | issues".into());
    let list = |v: &Vec<String>| if v.is_empty() { "–".to_string() } else { v.iter().take(6).cloned().collect::<Vec<_>>().join("; ") + if v.len() > 6 { " …" } else { "" } };
    for r in rows {
        out.push(format!("{} | {} | {} | {} | {} | {}", r.id, chain_label(r.status), list(&r.model), list(&r.code), list(&r.tests), list(&r.issues)));
    }
    out.join("\n")
}

/// Tool definition for the model.
pub fn definition() -> serde_json::Value {
    serde_json::json!({
        "name": "trace_requirements",
        "description": "Requirements document ↔ model consistency (traceability matrix). Reads the requirements from a document: file (a full path to .docx, .txt or .md; \
an attachment of the conversation has its saved path), confluence_page (page id, Confluence from the settings) or text (requirements pasted in the conversation). \
Requirements are lines or table rows starting with an id (W1, R.1.2, REQ-12, FR-01…). Matches them with the model: elements carrying the id (name, alias, notes, \
tagged values), trace / realisation relationships, and similar words (candidates with a score). Returns covered / candidates / NOT COVERED per requirement \
and model elements no requirement refers to. Judge the candidates, point out contradictions; on request save the matrix (create_spreadsheet) or create trace \
relationships in the model (ea_write create_connector, type Realisation or Dependency «trace», after the user agrees). With 'code' (a source folder, \
read only) every requirement is also followed to the code (types named like the traced elements, files naming the requirement id), the tests (test files \
naming the id or the type) and issue keys (ABC-123 in the requirement and the elements); the chain table says where it breaks: not in the model, no code, no tests.",
        "input_schema": {
            "type": "object",
            "properties": {
                "file": { "type": "string" },
                "confluence_page": { "type": "string" },
                "text": { "type": "string" },
                "package": { "type": "integer", "description": "Optionally only this package with subpackages." },
                "code": { "type": "string", "description": "Optionally a source folder (full path) for the chain requirement → model → code → test → issue." }
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requirement_ids_and_text() {
        let doc = "Wymagania biznesowe\nW1 Klient składa zamówienie w sklepie internetowym.\nW2: System wystawia fakturę po opłaceniu zamówienia.\n\
R.1.2 | Płatność kartą przez bramkę płatności | TAK\nREQ-12 The shop sends an e-mail notification to the customer.\nW12 Krótko\nzwykła linia bez identyfikatora\n";
        let reqs = extract(doc);
        let ids: Vec<&str> = reqs.iter().map(|r| r.id.as_str()).collect();
        assert_eq!(ids, ["W1", "W2", "R.1.2", "REQ-12"]);
        assert!(reqs[2].text.starts_with("Płatność kartą"));
        assert!(mentions("Spełnia W1 oraz W2.", "W1") && !mentions("W12 coś", "W1") && mentions("(R.1.2)", "R.1.2") && !mentions("R.1.23", "R.1.2"));
        assert!(tokens("OrderService obsługuje zamówienia").contains("servic") && tokens("zamówienie").contains("zamówi"));
        let html = "<table><tr><td>W5</td><td>Raport sprzedaży miesięcznej dla kierownika</td></tr></table><p>W6 Eksport danych do pliku Excel.</p>";
        assert_eq!(extract(&html_text(html)).len(), 2);
        let word = "<w:body><w:p><w:r><w:t>W7 Zwrot towaru w ciągu 14 dni.</w:t></w:r></w:p><w:tbl><w:tr><w:tc><w:p><w:r><w:t>W8</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>Program lojalnościowy dla klientów VIP</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:body>";
        let reqs = extract(&word_xml_text(word));
        assert_eq!(reqs.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(), ["W7", "W8"]);
    }

    #[test]
    fn chain_helpers() {
        assert!(is_test_path("src/test/java/shop/OrderTest.java") && is_test_path("OrderIT.java") && is_test_path("web/order.spec.ts"));
        assert!(is_test_path("tests/test_order.py") && is_test_path("features/checkout.feature") && is_test_path("pkg/order_test.go"));
        assert!(!is_test_path("src/main/java/shop/Order.java") && !is_test_path("src/Latest.java") && !is_test_path("src/Contest.ts"));
        assert_eq!(test_subject("src/test/java/OrderTest.java"), "order");
        assert_eq!(test_subject("test_order_item.py"), "orderitem");
        assert!(has_word("new Order(1)", "Order") && !has_word("new OrderItem()", "Order"));
        let except: HashSet<String> = ["REQ-12".to_string()].into_iter().collect();
        assert_eq!(issue_keys("See SHOP-123 and REQ-12, also UTF-8 and ab-1, X-1, SHOP-123.", &except), ["SHOP-123"]);
    }

    #[test]
    fn chain_of_requirements() {
        let dir = std::env::temp_dir().join(format!("loonar-chain-{}", std::process::id()));
        let _ = std::fs::create_dir_all(dir.join("src/main/java"));
        let _ = std::fs::create_dir_all(dir.join("src/test/java"));
        std::fs::write(dir.join("src/main/java/Order.java"), "public class Order {\n  private long id;\n}\n").unwrap();
        std::fs::write(dir.join("src/main/java/Invoice.java"), "// W2 invoice after payment\npublic class Invoice {\n  private long id;\n}\n").unwrap();
        std::fs::write(dir.join("src/test/java/OrderTest.java"), "class OrderTest { void places() { new Order(); } }\n").unwrap();
        let row = |id: &str, name: &str| TraceRow {
            id: id.into(), text: "text of the requirement SHOP-7".into(), coverage: if name.is_empty() { Coverage::Missing } else { Coverage::Covered },
            traced: if name.is_empty() { vec![] } else { vec![("1".into(), name.into(), "Class".into(), format!("{name} notes ARCH-9"))] }, candidates: vec![],
        };
        let t = Trace { rows: vec![row("W1", "Order"), row("W2", "Payment"), row("W3", "Shipment"), row("W4", "")], elements_checked: 3, unreferenced: vec![] };
        let reqs: Vec<Requirement> = ["W1", "W2", "W3", "W4"].iter().map(|id| Requirement { id: id.to_string(), text: String::new() }).collect();
        let rows = chain(&t, &reqs, &dir).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(rows.iter().map(|r| r.status).collect::<Vec<_>>(), [ChainStatus::Complete, ChainStatus::NoTests, ChainStatus::NoCode, ChainStatus::NotInModel]);
        assert_eq!(rows[0].code, ["src/main/java/Order.java"]);
        assert_eq!(rows[0].tests, ["src/test/java/OrderTest.java"]);
        assert_eq!(rows[1].code, ["src/main/java/Invoice.java"], "the file names the requirement id");
        assert_eq!(rows[0].issues, ["SHOP-7", "ARCH-9"]);
    }
}

#[cfg(test)]
mod model_tests {
    /// A Word document against a model with requirement elements and use cases (LOONAR_TEST_PROPS, LOONAR_TEST_DOCX).
    /// Run with: `cargo test trace_document_against_model -- --ignored --nocapture`.
    #[test]
    #[ignore = "requires a test model with requirements and a test document"]
    fn trace_document_against_model() {
        let cfg = crate::config::AppConfig::load(std::path::Path::new(&std::env::var("LOONAR_TEST_PROPS").unwrap())).unwrap();
        crate::i18n::set(crate::i18n::Lang::En);
        let text = super::docx_text(std::path::Path::new(&std::env::var("LOONAR_TEST_DOCX").unwrap())).unwrap();
        let reqs = super::extract(&text);
        let out = super::trace(&cfg, &reqs, None).unwrap();
        println!("{}", out.lines().take(14).collect::<Vec<_>>().join("\n"));
        let line = |id: &str| out.lines().find(|l| l.starts_with(&format!("{id} |"))).unwrap_or("").to_string();
        assert_eq!(reqs.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(), ["FR01", "FR09", "FR30"]);
        assert!(line("FR01").contains("| covered |"), "{}", line("FR01"));
        assert!(line("FR09").contains("| covered |"), "{}", line("FR09"));
        assert!(line("FR30").contains("ID MATCHES, CONTENT DIFFERS"), "FR30 in the model is about something else: {}", line("FR30"));
    }
}

//! Import from a sheet (LC-384, /import-sheet): rows of an Excel or CSV file become elements of a package – matched to
//! the existing ones (by GUID, alias or name and type) so a second import updates instead of duplicating, with a
//! preview create / update / no change before anything is saved, tagged values and relationships from columns, and long
//! texts in the notes. EA's CSV import breaks on long cells, knows no relationships and makes duplicates.
//! LC-424: names that are only nearly the same are shown (and used with similar "use"), stereotypes of an MDG technology
//! bring their missing tagged values and the relationship types the technology defines, documents (a file column or a
//! whole folder) become elements with the file linked, and "fidelity" checks what EA's own ArchiMate (MEF), BPMN or XMI
//! import left out.

use crate::config::AppConfig;
use crate::tools::query_rows;
use anyhow::{bail, Context, Result};
use serde_json::{json, Map, Value};
use std::collections::HashMap;

/// At most this many rows in one import.
const MAX: usize = 1000;

pub fn definition() -> Value {
    json!({
        "name": "import_sheet",
        "description": "Import rows of an attached Excel (.xlsx, first sheet) or CSV file as elements of a package, matched to existing elements so \
a repeated import updates instead of duplicating. Columns are named by their header: map name (required), type (a column or a fixed type such as \
Requirement), stereotype, notes, alias, status, tags {tag: column}, and relationships [{column, type, to_type?}] whose cells name other elements \
(comma-separated). Matching: guid, alias, or name and type (default) in the package and its child packages; a name that is only nearly the same \
(case, accents, spaces, a typo) is shown as similar, and similar \"use\" updates that element instead of creating a new one. A stereotype \
\"Profile::Name\" (or profile in the mapping) of an MDG technology adds the tagged values it defines that are missing; a relationship with a \
stereotype gets the type the technology defines for it. file (a column with paths or web addresses) links the document to the element; \
folder (instead of file) imports every document of a folder as an Artifact with the file linked. action columns: the headers and three sample \
rows (to propose a mapping); action preview: the numbered list create / update / no change (nothing saved); action apply: one save the user \
approves (undoable). action fidelity: an ArchiMate exchange file (MEF), BPMN or XMI the user imported with EA, against the package it went \
into: what is missing, what changed type, how many relationships arrived. At most 1000 rows. Use it for /import-sheet.",
        "input_schema": { "type": "object", "properties": {
            "action": { "type": "string", "enum": ["columns", "preview", "apply", "fidelity"] },
            "file": { "type": "string", "description": "The attachment's file name" },
            "folder": { "type": "string", "description": "A folder of documents to import as Artifacts (instead of file)" },
            "similar": { "type": "string", "enum": ["show", "use"], "description": "Nearly the same names: only shown (default) or used as matches" },
            "package": { "description": "Target package: package_id or exact name" },
            "mapping": { "type": "object" },
            "match_by": { "type": "string", "enum": ["name", "alias", "guid"] }
        }, "required": ["action"] }
    })
}

/// Rows of the first sheet of an .xlsx or of a CSV (comma or semicolon), the first row being the header.
pub(crate) fn read_rows(name: &str, bytes: &[u8]) -> Result<Vec<Vec<String>>> {
    if name.to_lowercase().ends_with(".csv") {
        let text = String::from_utf8_lossy(bytes).trim_start_matches('\u{feff}').to_string();
        let first = text.lines().next().unwrap_or("");
        let separator = if first.matches(';').count() > first.matches(',').count() { ';' } else { ',' };
        return Ok(csv_rows(&text, separator));
    }
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).context("the file is not an .xlsx (nor a .csv)")?;
    let shared: Vec<String> = crate::documents::part(&mut archive, "xl/sharedStrings.xml")
        .map(|xml| xml.split("</si>").map(|si| crate::documents::texts(si, "t").join("")).collect())
        .unwrap_or_default();
    let xml = crate::documents::part(&mut archive, "xl/worksheets/sheet1.xml").context("the workbook has no first sheet")?;
    let mut rows = Vec::new();
    for row in xml.split("</row>") {
        let mut cells: Vec<String> = Vec::new();
        for c in row.split("<c ").skip(1) {
            let head = format!(" {}", &c[..c.find('>').unwrap_or(0)]);
            let reference = crate::documents::attr(&head, "r").unwrap_or_default();
            let column = reference.chars().take_while(|c| c.is_ascii_alphabetic()).fold(0usize, |n, c| n * 26 + (c.to_ascii_uppercase() as usize - 'A' as usize + 1));
            let value = match crate::documents::attr(&head, "t").unwrap_or_default().as_str() {
                "s" => crate::documents::texts(c, "v").first().and_then(|i| i.parse::<usize>().ok()).and_then(|i| shared.get(i).cloned()).unwrap_or_default(),
                "inlineStr" => crate::documents::texts(c, "t").join(""),
                _ => crate::documents::texts(c, "v").first().cloned().unwrap_or_default(),
            };
            if column > 0 {
                if cells.len() < column {
                    cells.resize(column, String::new());
                }
                cells[column - 1] = value.replace("&amp;", "&").replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&apos;", "'");
            }
        }
        if cells.iter().any(|c| !c.trim().is_empty()) {
            rows.push(cells);
        }
    }
    Ok(rows)
}

/// CSV with quotes ("" inside a quoted cell is a quote; a quoted cell may span lines).
fn csv_rows(text: &str, separator: char) -> Vec<Vec<String>> {
    let (mut rows, mut row, mut cell, mut quoted) = (Vec::new(), Vec::new(), String::new(), false);
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' if quoted && chars.peek() == Some(&'"') => {
                cell.push('"');
                chars.next();
            }
            '"' => quoted = !quoted,
            c if c == separator && !quoted => row.push(std::mem::take(&mut cell)),
            '\r' if !quoted => {}
            '\n' if !quoted => {
                row.push(std::mem::take(&mut cell));
                if row.iter().any(|c: &String| !c.trim().is_empty()) {
                    rows.push(std::mem::take(&mut row));
                }
                row.clear();
            }
            c => cell.push(c),
        }
    }
    row.push(cell);
    if row.iter().any(|c| !c.trim().is_empty()) {
        rows.push(row);
    }
    rows
}

/// One planned row: (number, what, label, operation).
type Planned = (usize, &'static str, String, Option<Value>);

/// A name folded for comparing: lower case, without accents, spaces or punctuation.
fn fold(name: &str) -> String {
    name.to_lowercase()
        .chars()
        .map(|c| match c {
            'ą' => 'a', 'ć' => 'c', 'ę' => 'e', 'ł' => 'l', 'ń' => 'n', 'ó' => 'o', 'ś' => 's', 'ź' | 'ż' => 'z',
            'ä' | 'à' | 'á' | 'â' => 'a', 'ö' | 'ô' | 'ò' => 'o', 'ü' | 'ù' | 'û' | 'ú' => 'u', 'é' | 'è' | 'ê' | 'ë' => 'e', 'ß' => 's', 'ç' => 'c',
            c => c,
        })
        .filter(|c| c.is_alphanumeric())
        .collect()
}

/// Edits between two short texts (Levenshtein).
fn distance(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut previous = row[0];
        row[0] = i + 1;
        for j in 0..b.len() {
            let here = row[j + 1];
            row[j + 1] = (previous + usize::from(ca != b[j])).min(row[j] + 1).min(row[j + 1] + 1);
            previous = here;
        }
    }
    row[b.len()]
}

/// Whether two names are nearly the same: equal when folded, or a typo or two apart in a longer name.
fn similar(a: &str, b: &str) -> bool {
    let (a, b) = (fold(a), fold(b));
    if a.is_empty() || b.is_empty() {
        return false;
    }
    a == b || a.chars().count() >= 6 && distance(&a, &b) <= (a.chars().count() / 8).max(1)
}

/// The rows of a folder of documents: a header and one row (name, path) per file (LC-424).
fn folder_rows(folder: &str) -> Result<Vec<Vec<String>>> {
    let mut rows = vec![vec!["Name".to_string(), "File".to_string()]];
    let mut files: Vec<std::path::PathBuf> = std::fs::read_dir(folder).with_context(|| format!("cannot read the folder {folder}"))?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_file())
        .collect();
    files.sort();
    for p in files {
        let name = p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        rows.push(vec![name, p.display().to_string()]);
    }
    Ok(rows)
}

/// The plan of an import: the rows with their action and operation, and the relationship operations.
fn plan(input: &Value, cfg: &AppConfig) -> Result<(Vec<Planned>, Vec<Value>, String, Vec<String>)> {
    let folder = input["folder"].as_str().map(str::trim).filter(|f| !f.is_empty());
    let rows = match folder {
        Some(f) => folder_rows(f)?,
        None => {
            let file = std::path::Path::new(input["file"].as_str().unwrap_or("")).file_name().context("file (or folder)")?.to_string_lossy().to_string();
            let bytes = std::fs::read(cfg.attachments_dir.join(&file)).with_context(|| format!("there is no attachment {file}"))?;
            read_rows(&file, &bytes)?
        }
    };
    // A folder of documents: each an Artifact with its file linked, unless the mapping says otherwise.
    let mut input = input.clone();
    if folder.is_some() {
        let m = &mut input["mapping"];
        if !m.is_object() {
            *m = json!({});
        }
        for (k, v) in [("name", "Name"), ("file", "File"), ("type", "Artifact")] {
            if m[k].is_null() {
                m[k] = json!(v);
            }
        }
    }
    let input = &input;
    let Some((header, data)) = rows.split_first() else { bail!("the file is empty") };
    if data.len() > MAX {
        bail!("{} rows; at most {MAX} at once", data.len());
    }
    let col = |name: &str| header.iter().position(|h| h.trim().eq_ignore_ascii_case(name.trim()));
    let mapping = &input["mapping"];
    let mapped = |key: &str| mapping[key].as_str().and_then(col);
    let name_col = mapped("name").context("mapping.name: the column with the names")?;
    let (scope, title) = crate::model_usage::package_scope(cfg, &input["package"])?;
    let package: i64 = scope.split(['(', ',']).nth(1).and_then(|p| p.trim_matches([' ', ')']).parse().ok()).context("give the target package")?;
    let existing = query_rows(cfg, &format!(
        "select o.object_id, o.name, o.object_type, coalesce(o.alias,''), o.ea_guid, coalesce(o.stereotype,''), coalesce(o.note,''), coalesce(o.status,'') from t_object o where 1 = 1 {scope}"
    ))?;
    // Tagged values and relationships already there: a repeated import changes nothing and adds no duplicate.
    let mut have_tags: HashMap<(String, String), String> = HashMap::new();
    let mut have_links: std::collections::HashSet<(String, String, String)> = std::collections::HashSet::new();
    if !existing.is_empty() {
        let ids = existing.iter().map(|e| e[0].clone()).collect::<Vec<_>>().join(",");
        for t in query_rows(cfg, &format!("select object_id, lower(property), coalesce(value,'') from t_objectproperties where object_id in ({ids})"))? {
            have_tags.insert((t[0].clone(), t[1].clone()), t[2].clone());
        }
        for c in query_rows(cfg, &format!("select start_object_id, end_object_id, lower(connector_type) from t_connector where start_object_id in ({ids})"))? {
            have_links.insert((c[0].clone(), c[1].clone(), c[2].clone()));
        }
    }
    let match_by = input["match_by"].as_str().unwrap_or("name");
    let use_similar = input["similar"].as_str() == Some("use");
    let mut notes: Vec<String> = Vec::new();
    // Stereotypes of MDG technologies, looked up once each.
    let mut stereotypes: HashMap<String, Option<crate::mdg::Stereotype>> = HashMap::new();
    let mut mdg = |full: &str| stereotypes.entry(full.to_lowercase()).or_insert_with(|| crate::mdg::stereotype(cfg, full)).clone();
    let profile = mapping["profile"].as_str().map(str::trim).filter(|p| !p.is_empty()).map(str::to_string);
    let full_name = |s: &str| if s.contains("::") || s.is_empty() { s.to_string() } else { profile.as_ref().map(|p| format!("{p}::{s}")).unwrap_or_else(|| s.to_string()) };
    let mut planned: Vec<Planned> = Vec::new();
    let mut keys: HashMap<String, Value> = HashMap::new();
    for (i, row) in data.iter().enumerate() {
        let cell = |c: Option<usize>| c.and_then(|c| row.get(c)).map(|v| v.trim().to_string()).unwrap_or_default();
        let name = cell(Some(name_col));
        if name.is_empty() {
            planned.push((i + 1, "skip", "(no name)".into(), None));
            continue;
        }
        let kind = mapped("type").map(|c| cell(Some(c))).filter(|t| !t.is_empty()).or_else(|| mapping["type"].as_str().filter(|t| col(t).is_none()).map(str::to_string)).unwrap_or_else(|| "Class".into());
        let mut found = existing.iter().find(|e| match match_by {
            "guid" => mapped("guid").is_some_and(|c| cell(Some(c)) == e[4]),
            "alias" => mapped("alias").is_some_and(|c| !cell(Some(c)).is_empty() && cell(Some(c)) == e[3]),
            _ => e[1].eq_ignore_ascii_case(&name) && e[2].eq_ignore_ascii_case(&kind),
        });
        // A nearly identical name (LC-424): shown, or used as the match when the user said so.
        let mut similar_note = String::new();
        if found.is_none() && match_by == "name" {
            if let Some(e) = existing.iter().find(|e| e[2].eq_ignore_ascii_case(&kind) && similar(&e[1], &name)) {
                if use_similar {
                    similar_note = format!(" = \"{}\" (similar name)", e[1]);
                    found = Some(e);
                } else {
                    similar_note = format!(" – similar to \"{}\" (id {}); similar \"use\" updates it instead", e[1], e[0]);
                }
            }
        }
        let mut fields = Map::new();
        for (key, field) in [("notes", "notes"), ("alias", "alias"), ("status", "status"), ("stereotype", "stereotype")] {
            if let Some(c) = mapped(key) {
                fields.insert(field.into(), json!(if key == "stereotype" { full_name(&cell(Some(c))) } else { cell(Some(c)) }));
            }
        }
        let files: Vec<String> = mapped("file").map(|c| cell(Some(c)).split(['\n', ';']).map(|f| f.trim().to_string()).filter(|f| !f.is_empty()).collect()).unwrap_or_default();
        let mut tags = Map::new();
        for (tag, column) in mapping["tags"].as_object().into_iter().flatten() {
            if let Some(c) = column.as_str().and_then(col) {
                tags.insert(tag.clone(), json!(cell(Some(c))));
            }
        }
        // The tagged values an MDG stereotype defines that neither the row nor the element has (LC-424), with their defaults.
        let stereotype = fields.get("stereotype").and_then(Value::as_str).map(str::to_string).unwrap_or_default();
        let mut mdg_tags = 0;
        if stereotype.contains("::") {
            match mdg(&stereotype) {
                // A new element gets them from EA with the stereotype; an existing one only the ones it lacks.
                Some(def) if found.is_none() => mdg_tags = def.tags.len(),
                Some(def) => {
                    let element = found.map(|e| e[0].clone()).unwrap_or_default();
                    for (tag, _, _, default, _) in &def.tags {
                        if !tags.keys().any(|k| k.eq_ignore_ascii_case(tag)) && !have_tags.contains_key(&(element.clone(), tag.to_lowercase())) {
                            tags.insert(tag.clone(), json!(default));
                            mdg_tags += 1;
                        }
                    }
                }
                None => {
                    if !notes.iter().any(|n| n.contains(&stereotype)) {
                        notes.push(format!("No MDG technology here defines {stereotype}: its tagged values cannot be checked."));
                    }
                }
            }
        }
        let extra = format!("{}{similar_note}", if mdg_tags > 0 { format!(" +{mdg_tags} MDG tag(s)") } else { String::new() });
        let label = format!("\"{name}\" ({kind}){extra}");
        match found {
            Some(e) => {
                let id: i64 = e[0].parse().unwrap_or(0);
                keys.insert(name.to_lowercase(), json!(id));
                let short = |v: &str| v.rsplit("::").next().unwrap_or("").to_string();
                let same = |field: &str, idx: usize| fields.get(field).is_none_or(|v| if field == "stereotype" { short(v.as_str().unwrap_or("")) == e[idx] } else { v.as_str() == Some(e[idx].as_str()) });
                let tags_same = tags.iter().all(|(k, v)| have_tags.get(&(e[0].clone(), k.to_lowercase())).map(String::as_str).unwrap_or("") == v.as_str().unwrap_or(""));
                if same("notes", 6) && same("alias", 3) && same("status", 7) && same("stereotype", 5) && tags_same && files.is_empty() {
                    planned.push((i + 1, "no change", label, None));
                } else {
                    let mut op = json!({ "op": "update_element", "element": id });
                    for (k, v) in fields {
                        op[k] = v;
                    }
                    if !tags.is_empty() {
                        op["tags"] = Value::Object(tags);
                    }
                    if !files.is_empty() {
                        op["files"] = json!(files);
                    }
                    planned.push((i + 1, "update", label, Some(op)));
                }
            }
            None => {
                let key = format!("r{}", i + 1);
                keys.insert(name.to_lowercase(), json!(format!("#{key}")));
                let mut op = json!({ "op": "create_element", "key": key, "package": package, "name": name, "type": kind });
                for (k, v) in fields {
                    op[k] = v;
                }
                if !tags.is_empty() {
                    op["tags"] = Value::Object(tags);
                }
                if !files.is_empty() {
                    op["files"] = json!(files);
                }
                planned.push((i + 1, "create", label, Some(op)));
            }
        }
    }
    // Relationships from columns naming other elements (of the sheet or already in the package).
    let mut relations = Vec::new();
    for rel in mapping["relationships"].as_array().into_iter().flatten() {
        let (Some(c), Some(given)) = (rel["column"].as_str().and_then(col), rel["type"].as_str()) else { continue };
        // A stereotyped relationship gets the type its MDG technology defines (LC-424): ArchiMate Serving is an Association.
        let stereotype = rel["stereotype"].as_str().map(&full_name).unwrap_or_default();
        let mut kind = given.to_string();
        if stereotype.contains("::") {
            if let Some(base) = mdg(&stereotype).and_then(|d| d.applies_to.first().cloned()).filter(|b| !b.eq_ignore_ascii_case(given)) {
                notes.push(format!("{stereotype}: {given} → {base} (the type the MDG technology defines for it)"));
                kind = base;
            }
        }
        let kind = kind.as_str();
        for row in data {
            let source = row.get(name_col).map(|n| n.trim().to_lowercase()).unwrap_or_default();
            for target in row.get(c).map(|v| v.split([',', ';']).map(|t| t.trim().to_lowercase()).filter(|t| !t.is_empty()).collect::<Vec<_>>()).unwrap_or_default() {
                if let (Some(s), Some(t)) = (keys.get(&source), keys.get(&target)) {
                    let known = matches!((s, t), (Value::Number(a), Value::Number(b)) if have_links.contains(&(a.to_string(), b.to_string(), kind.to_lowercase())));
                    if !known {
                        let mut op = json!({ "op": "create_connector", "source": s, "target": t, "type": kind });
                        if !stereotype.is_empty() {
                            op["stereotype"] = json!(stereotype);
                        }
                        relations.push(op);
                    }
                }
            }
        }
    }
    Ok((planned, relations, title, notes))
}

/// Element kinds of BPMN that are shapes (not flows).
const BPMN_NODES: [&str; 22] = ["task", "userTask", "serviceTask", "manualTask", "scriptTask", "sendTask", "receiveTask", "businessRuleTask",
    "callActivity", "subProcess", "startEvent", "endEvent", "intermediateCatchEvent", "intermediateThrowEvent", "boundaryEvent",
    "exclusiveGateway", "parallelGateway", "inclusiveGateway", "eventBasedGateway", "dataObject", "dataStore", "participant"];

/// What an exchange file holds: (format, elements (kind, name), relationships by kind). ArchiMate MEF, BPMN 2 or XMI.
fn exchange_items(xml: &str) -> (&'static str, Vec<(String, String)>, std::collections::BTreeMap<String, usize>) {
    let mut elements = Vec::new();
    let mut relations: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
    let format = if xml.contains("opengroup.org/xsd/archimate") { "ArchiMate (MEF)" } else if xml.contains("omg.org/spec/BPMN") { "BPMN" } else { "XMI" };
    let attr = |tag: &str, name: &str| crate::documents::attr(tag, name);
    let mut rest = xml;
    while let Some(at) = rest.find('<') {
        rest = &rest[at + 1..];
        if rest.starts_with(['/', '?', '!']) {
            continue;
        }
        let end = rest.find('>').unwrap_or(rest.len());
        let tag = format!(" {}", &rest[..end]);
        let local = rest[..end].split([' ', '/', '\n', '\r', '\t']).next().unwrap_or("").rsplit(':').next().unwrap_or("").to_string();
        match format {
            "ArchiMate (MEF)" => {
                let kind = attr(&tag, "xsi:type").unwrap_or_default();
                if local == "element" && !kind.is_empty() {
                    // The name is the first <name> inside the element.
                    let body = &rest[end..];
                    let stop = body.find("</element>").unwrap_or(body.len());
                    let name = body[..stop].find("<name").and_then(|n| body[n..stop].find('>').map(|g| n + g + 1)).map(|from| body[from..stop].split('<').next().unwrap_or("").trim().to_string()).unwrap_or_default();
                    elements.push((kind, crate::documents::texts(&format!("<n>{name}</n>"), "n").concat()));
                } else if local == "relationship" && !kind.is_empty() {
                    *relations.entry(kind).or_insert(0) += 1;
                }
            }
            "BPMN" => {
                if BPMN_NODES.contains(&local.as_str()) {
                    elements.push((local.clone(), attr(&tag, "name").unwrap_or_default()));
                } else if local.ends_with("Flow") && local != "dataFlow" || local == "association" {
                    *relations.entry(local.clone()).or_insert(0) += 1;
                }
            }
            _ => {
                if matches!(local.as_str(), "packagedElement" | "ownedMember" | "ownedAttribute") || local == "generalization" {
                    let kind = attr(&tag, "xmi:type").unwrap_or_default().trim_start_matches("uml:").to_string();
                    if local == "generalization" || matches!(kind.as_str(), "Association" | "Dependency" | "Realization" | "Usage" | "Abstraction" | "InformationFlow" | "InterfaceRealization") {
                        *relations.entry(if local == "generalization" { "Generalization".into() } else { kind }).or_insert(0) += 1;
                    } else if local != "ownedAttribute" && !kind.is_empty() && kind != "Package" && kind != "Model" {
                        elements.push((kind, attr(&tag, "name").unwrap_or_default()));
                    }
                }
            }
        }
        rest = &rest[end.min(rest.len())..];
    }
    (format, elements, relations)
}

/// LC-424: what EA's import of an exchange file left out, against the package it went into.
fn fidelity(input: &Value, cfg: &AppConfig) -> Result<String> {
    let file = std::path::Path::new(input["file"].as_str().unwrap_or("")).file_name().context("file: the exchange file (XML) attached")?.to_string_lossy().to_string();
    let bytes = std::fs::read(cfg.attachments_dir.join(&file)).with_context(|| format!("there is no attachment {file}"))?;
    let xml = String::from_utf8_lossy(&bytes).to_string();
    let (format, elements, relations) = exchange_items(&xml);
    if elements.is_empty() {
        bail!("{file}: no elements found (an ArchiMate exchange file, BPMN 2 or XMI is expected)");
    }
    let (scope, title) = crate::model_usage::package_scope(cfg, &input["package"])?;
    let model = query_rows(cfg, &format!("select lower(o.name), o.object_type, coalesce(o.stereotype,'') from t_object o where o.object_type <> 'Package' {scope}"))?;
    let connectors = query_rows(cfg, &format!(
        "select count(*) from t_connector c join t_object o on o.object_id = c.start_object_id where c.connector_type not in ('NoteLink') {scope}"
    ))?.first().map(|r| r[0].parse::<usize>().unwrap_or(0)).unwrap_or(0);
    let mut by_kind: std::collections::BTreeMap<&str, (usize, usize)> = std::collections::BTreeMap::new();
    let (mut missing, mut changed, mut unnamed) = (Vec::new(), Vec::new(), 0);
    for (kind, name) in &elements {
        let entry = by_kind.entry(kind.as_str()).or_insert((0, 0));
        entry.0 += 1;
        if name.trim().is_empty() {
            unnamed += 1;
            continue;
        }
        let hits: Vec<&Vec<String>> = model.iter().filter(|m| m[0] == name.trim().to_lowercase()).collect();
        if hits.is_empty() {
            missing.push(format!("- {name} ({kind})"));
            continue;
        }
        entry.1 += 1;
        // The kind it should have in EA: ArchiMate as a stereotype ArchiMate_<kind>, BPMN as a stereotype, XMI as the type.
        let k = kind.to_lowercase();
        let kept = hits.iter().any(|m| {
            let (t, s) = (m[1].to_lowercase(), m[2].to_lowercase());
            match format {
                "ArchiMate (MEF)" => s.trim_start_matches("archimate_") == k || s == k,
                "BPMN" => s.contains(k.trim_end_matches("task")) || t.contains(&k) || k.contains("task") && (s.contains("activity") || t == "activity") || k.contains("gateway") && s.contains("gateway") || k.contains("event") && s.contains("event"),
                _ => t == k || s == k,
            }
        });
        if !kept {
            changed.push(format!("- {name}: {kind} in the file, {} in the model", hits.iter().map(|m| if m[2].is_empty() { m[1].clone() } else { format!("{} «{}»", m[1], m[2]) }).collect::<Vec<_>>().join(" / ")));
        }
    }
    let file_relations: usize = relations.values().sum();
    let arrived = elements.len() - unnamed - missing.len();
    let mut out = vec![format!(
        "{file} ({format}) against the {title}: {} element(s) in the file, {arrived} found in the package, {} missing, {} with another kind{}; relationships: {file_relations} in the file, {connectors} in the package.",
        elements.len(), missing.len(), changed.len(), if unnamed > 0 { format!(", {unnamed} without a name (not checked)") } else { String::new() }
    )];
    out.push(format!("By kind (in the file / found): {}", by_kind.iter().map(|(k, (a, b))| format!("{k} {a}/{b}")).collect::<Vec<_>>().join(", ")));
    if !relations.is_empty() {
        out.push(format!("Relationships in the file: {}", relations.iter().map(|(k, n)| format!("{k} {n}")).collect::<Vec<_>>().join(", ")));
    }
    if connectors < file_relations {
        out.push(format!("{} relationship(s) fewer in the package than in the file: EA may have skipped the ones whose ends are missing or of a kind it does not know.", file_relations - connectors));
    }
    if !missing.is_empty() {
        out.push(format!("Missing ({}):", missing.len()));
        out.extend(missing.into_iter().take(100));
    }
    if !changed.is_empty() {
        out.push(format!("Another kind in the model ({}):", changed.len()));
        out.extend(changed.into_iter().take(60));
    }
    Ok(out.join("\n"))
}

/// columns: headers and samples; preview: the plan as text; apply: the operations.
pub fn run(input: &Value, cfg: &AppConfig) -> Result<(Vec<Value>, String)> {
    if input["action"].as_str() == Some("fidelity") {
        return Ok((Vec::new(), fidelity(input, cfg)?));
    }
    if input["action"].as_str() == Some("columns") {
        let rows = match input["folder"].as_str().map(str::trim).filter(|f| !f.is_empty()) {
            Some(folder) => folder_rows(folder)?,
            None => {
                let file = std::path::Path::new(input["file"].as_str().unwrap_or("")).file_name().context("file")?.to_string_lossy().to_string();
                read_rows(&file, &std::fs::read(cfg.attachments_dir.join(&file)).with_context(|| format!("there is no attachment {file}"))?)?
            }
        };
        let mut text = format!("{} rows. Columns: {}\nSamples:\n", rows.len().saturating_sub(1), rows.first().map(|h| h.join(" | ")).unwrap_or_default());
        for r in rows.iter().skip(1).take(3) {
            text.push_str(&format!("  {}\n", r.join(" | ")));
        }
        return Ok((Vec::new(), text));
    }
    let (planned, relations, title, notes) = plan(input, cfg)?;
    if input["action"].as_str() == Some("apply") {
        let mut ops: Vec<Value> = planned.into_iter().filter_map(|p| p.3).collect();
        ops.extend(relations);
        if ops.is_empty() {
            bail!("nothing to import: every row is unchanged or skipped");
        }
        return Ok((ops, String::new()));
    }
    let count = |what: &str| planned.iter().filter(|p| p.1 == what).count();
    let mut text = format!(
        "Import into {title}: {} to create, {} to update, {} without change, {} skipped; {} relationships. Nothing was saved yet.\n",
        count("create"), count("update"), count("no change"), count("skip"), relations.len()
    );
    for (n, what, label, _) in planned.iter().take(150) {
        text.push_str(&format!("{n} | {what} | {label}\n"));
    }
    for note in notes {
        text.push_str(&format!("Note: {note}\n"));
    }
    Ok((Vec::new(), text))
}

#[cfg(test)]
mod tests {
    #[test]
    fn csv_with_quotes_and_long_cells() {
        let long = "x".repeat(400);
        let text = format!("Name;Type;Notes\r\nOrder;Class;\"Line one\nline \"\"two\"\"\"\r\nInvoice;Class;{long}\r\n");
        let rows = super::read_rows("a.csv", text.as_bytes()).unwrap();
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[1], vec!["Order", "Class", "Line one\nline \"two\""]);
        assert_eq!(rows[2][2].len(), 400, "a long cell stays whole");
    }

    /// LC-424: nearly the same names, and what exchange files hold.
    #[test]
    fn similar_names_and_exchange_files() {
        assert!(super::similar("Zamówienie klienta", "zamowienie  Klienta"));
        assert!(super::similar("Customer order", "Custmer order"));
        assert!(!super::similar("Order", "Border") && !super::similar("Invoice", "Payment"));
        let mef = r#"<model xmlns="http://www.opengroup.org/xsd/archimate/3.0/" xmlns:xsi="x"><elements><element identifier="a" xsi:type="BusinessActor"><name xml:lang="en">Customer</name></element><element identifier="b" xsi:type="ApplicationService"><name>Billing &amp; pay</name></element></elements><relationships><relationship identifier="r" xsi:type="Serving" source="b" target="a"/></relationships></model>"#;
        let (format, elements, relations) = super::exchange_items(mef);
        assert_eq!(format, "ArchiMate (MEF)");
        assert_eq!(elements, vec![("BusinessActor".to_string(), "Customer".to_string()), ("ApplicationService".to_string(), "Billing & pay".to_string())]);
        assert_eq!(relations.get("Serving"), Some(&1));
        let bpmn = r#"<bpmn:definitions xmlns:bpmn="http://www.omg.org/spec/BPMN/20100524/MODEL"><bpmn:process><bpmn:startEvent id="s" name="Order in"/><bpmn:userTask id="t" name="Check"/><bpmn:sequenceFlow id="f" sourceRef="s" targetRef="t"/></bpmn:process></bpmn:definitions>"#;
        let (format, elements, relations) = super::exchange_items(bpmn);
        assert_eq!((format, elements.len(), relations.get("sequenceFlow")), ("BPMN", 2, Some(&1)));
        let xmi = r#"<xmi:XMI xmlns:uml="u" xmlns:xmi="x"><uml:Model><packagedElement xmi:type="uml:Class" name="Order"><generalization general="b"/></packagedElement><packagedElement xmi:type="uml:Association" name=""/></uml:Model></xmi:XMI>"#;
        let (format, elements, relations) = super::exchange_items(xmi);
        assert_eq!((format, elements, relations.values().sum::<usize>()), ("XMI", vec![("Class".to_string(), "Order".to_string())], 2));
    }

    /// LC-384: a CSV imported through EA on a model COPY, then imported again (no duplicates):
    /// `LOONAR_TEST_PROPS=E:/livetest/undo/undo.properties cargo test import_sheet_live -- --ignored --nocapture`
    #[test]
    #[ignore = "requires EA and a copy of a test model"]
    fn import_sheet_live() {
        let mut cfg = crate::config::AppConfig::load(std::path::Path::new(&std::env::var("LOONAR_TEST_PROPS").expect("LOONAR_TEST_PROPS"))).expect("app.properties");
        let dir = std::env::temp_dir().join(format!("loonar-import-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        cfg.attachments_dir = dir.clone();
        std::fs::write(dir.join("reqs.csv"), "Name,Type,Owner,Depends on,Notes\nLog in,Requirement,Sales,,Users sign in.\nPay,Requirement,Finance,Log in,\"Pay by card, or by transfer\"\n").unwrap();
        let q = |sql: &str| crate::tools::query_rows(&cfg, sql).unwrap();
        let root: i64 = q("select package_id from t_package where parent_id = 0 order by package_id")[0][0].parse().unwrap();
        let made = crate::ea::run_batch(&cfg, &[serde_json::json!({"op":"create_package","key":"pkg","parent":root,"name":format!("ZZ_import_{}", chrono::Local::now().format("%H%M%S"))})], &crate::ea::BatchOptions::default()).unwrap();
        let pkg = made["created"][0]["id"].as_i64().unwrap();
        let input = |action: &str| serde_json::json!({ "action": action, "file": "reqs.csv", "package": pkg,
            "mapping": { "name": "Name", "type": "Type", "notes": "Notes", "tags": { "Owner": "Owner" }, "relationships": [{ "column": "Depends on", "type": "Dependency" }] } });
        println!("{}", super::run(&input("preview"), &cfg).unwrap().1);
        let (ops, _) = super::run(&input("apply"), &cfg).unwrap();
        crate::ea::run_batch(&cfg, &ops, &crate::ea::BatchOptions::default()).expect("import");
        let again = super::run(&input("preview"), &cfg).unwrap().1;
        println!("{again}");
        let elements = q(&format!("select count(*) from t_object where package_id = {pkg}"))[0][0].clone();
        let links = q(&format!("select count(*) from t_connector c join t_object o on o.object_id = c.start_object_id where o.package_id = {pkg}"))[0][0].clone();
        crate::ea::run_batch(&cfg, &[serde_json::json!({"op":"delete","package":pkg})], &crate::ea::BatchOptions::default()).expect("cleanup");
        crate::ea::shutdown();
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!((elements.as_str(), links.as_str()), ("2", "1"));
        assert!(again.contains("0 to create, 0 to update, 2 without change") && again.contains("; 0 relationships"), "the second import changes nothing: {again}");
    }

    /// LC-424 on a model COPY: a similar name shown then used, MDG tags and relationship type, documents of a folder,
    /// and the fidelity of an ArchiMate exchange file.
    /// `LOONAR_TEST_PROPS=E:/livetest/undo/undo.properties cargo test import_fidelity_live -- --ignored --nocapture`
    #[test]
    #[ignore = "requires EA and a copy of a test model"]
    fn import_fidelity_live() {
        use serde_json::json;
        let mut cfg = crate::config::AppConfig::load(std::path::Path::new(&std::env::var("LOONAR_TEST_PROPS").expect("LOONAR_TEST_PROPS"))).expect("app.properties");
        let dir = std::env::temp_dir().join(format!("loonar-import424-{}", std::process::id()));
        let docs = dir.join("docs");
        std::fs::create_dir_all(&docs).unwrap();
        cfg.attachments_dir = dir.clone();
        std::fs::write(docs.join("Contract.pdf"), b"%PDF-1.4").unwrap();
        std::fs::write(docs.join("Price list.xlsx"), b"x").unwrap();
        let q = |sql: &str| crate::tools::query_rows(&cfg, sql).unwrap();
        let batch = |ops: &[serde_json::Value]| crate::ea::run_batch(&cfg, ops, &crate::ea::BatchOptions::default()).expect("batch");
        let root: i64 = q("select package_id from t_package where parent_id = 0 order by package_id")[0][0].parse().unwrap();
        let made = batch(&[json!({"op":"create_package","key":"pkg","parent":root,"name":format!("ZZ_import424_{}", chrono::Local::now().format("%H%M%S"))})]);
        let pkg = made["created"][0]["id"].as_i64().unwrap();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            // 1. ArchiMate actors with the MDG stereotype, and a Serving relationship given as a Dependency.
            std::fs::write(dir.join("actors.csv"), "Name,Serves\nCustomer order,\nBilling,Customer order\n").unwrap();
            let actors = json!({ "action": "preview", "file": "actors.csv", "package": pkg, "mapping": { "name": "Name", "type": "Class",
                "stereotype": "Stereo", "profile": "ArchiMate3", "relationships": [{ "column": "Serves", "type": "Dependency", "stereotype": "ArchiMate_Serving" }] } });
            let mut with_stereo = actors.clone();
            with_stereo["mapping"]["stereotype"] = serde_json::Value::Null;
            println!("{:?}", crate::mdg::stereotype(&cfg, "ArchiMate3::ArchiMate_BusinessActor").map(|s| (s.applies_to, s.tags.len())));
            std::fs::write(dir.join("actors.csv"), "Name,Stereo,Serves\nCustomer order,ArchiMate_BusinessActor,\nBilling,ArchiMate_ApplicationService,Customer order\n").unwrap();
            let preview = super::run(&actors, &cfg).unwrap().1;
            println!("{preview}");
            let mut apply = actors.clone();
            apply["action"] = json!("apply");
            batch(&super::run(&apply, &cfg).unwrap().0);
            let link = q(&format!("select c.connector_type, coalesce(c.stereotype,'') from t_connector c join t_object o on o.object_id = c.start_object_id where o.package_id = {pkg}"));
            println!("{link:?}");
            // 2. A similar name: shown, then used.
            std::fs::write(dir.join("again.csv"), "Name,Notes\nCustomer  Ordr,From the sheet\n").unwrap();
            let similar = json!({ "action": "preview", "file": "again.csv", "package": pkg, "mapping": { "name": "Name", "type": "Class", "notes": "Notes" } });
            let shown = super::run(&similar, &cfg).unwrap().1;
            println!("{shown}");
            let mut used = similar.clone();
            used["similar"] = json!("use");
            let used_text = super::run(&used, &cfg).unwrap().1;
            println!("{used_text}");
            // 3. The documents of a folder.
            let folder = json!({ "action": "apply", "folder": docs.display().to_string(), "package": pkg });
            println!("{}", super::run(&json!({ "action": "preview", "folder": docs.display().to_string(), "package": pkg }), &cfg).unwrap().1);
            batch(&super::run(&folder, &cfg).unwrap().0);
            let files = q(&format!("select o.name, o.object_type, f.filename from t_objectfiles f join t_object o on o.object_id = f.object_id where o.package_id = {pkg} order by o.name"));
            println!("{files:?}");
            // 4. Fidelity of an exchange file against the package.
            std::fs::write(dir.join("model.xml"), r#"<model xmlns="http://www.opengroup.org/xsd/archimate/3.0/" xmlns:xsi="x"><elements><element identifier="a" xsi:type="BusinessActor"><name>Customer order</name></element><element identifier="b" xsi:type="BusinessRole"><name>Billing</name></element><element identifier="c" xsi:type="Node"><name>Server</name></element></elements><relationships><relationship identifier="r" xsi:type="Serving" source="b" target="a"/><relationship identifier="s" xsi:type="Flow" source="b" target="c"/></relationships></model>"#).unwrap();
            let report = super::run(&json!({ "action": "fidelity", "file": "model.xml", "package": pkg }), &cfg).unwrap().1;
            println!("{report}");
            // 5. A BPMN activity: the tagged values BPMN defines come with it.
            std::fs::write(dir.join("bpmn.csv"), "Name,Stereo
Approve,BPMN2.0::Activity
").unwrap();
            let bpmn = super::run(&json!({ "action": "preview", "file": "bpmn.csv", "package": pkg, "mapping": { "name": "Name", "type": "Activity", "stereotype": "Stereo" } }), &cfg).unwrap().1;
            println!("{bpmn}");
            assert!(bpmn.contains("MDG tag(s)"), "{bpmn}");
            let bpmn_input = json!({ "action": "apply", "file": "bpmn.csv", "package": pkg, "mapping": { "name": "Name", "type": "Activity", "stereotype": "Stereo" } });
            batch(&super::run(&bpmn_input, &cfg).unwrap().0);
            let tags = q(&format!("select count(*) from t_objectproperties p join t_object o on o.object_id = p.object_id where o.package_id = {pkg} and o.name = 'Approve'"));
            let mut again = bpmn_input.clone();
            again["action"] = json!("preview");
            let again = super::run(&again, &cfg).unwrap().1;
            println!("tags {tags:?}
{again}");
            assert!(tags[0][0] != "0" && again.contains("1 without change"), "EA adds the tags with the stereotype: {tags:?} {again}");
            (preview, link, shown, used_text, files, report)
        }));
        batch(&[json!({"op":"delete","package":pkg})]);
        crate::ea::shutdown();
        let _ = std::fs::remove_dir_all(&dir);
        let (preview, link, shown, used_text, files, report) = result.unwrap();
        assert!(preview.contains("2 to create") && preview.contains("1 relationships"), "{preview}");
        assert_eq!(link.len(), 1, "{link:?}");
        assert!(shown.contains("similar to \"Customer order\"") && shown.contains("1 to create"), "{shown}");
        assert!(used_text.contains("0 to create, 1 to update") && used_text.contains("(similar name)"), "{used_text}");
        assert_eq!(files.iter().map(|f| (f[0].as_str(), f[1].as_str())).collect::<Vec<_>>(), vec![("Contract", "Artifact"), ("Price list", "Artifact")], "{files:?}");
        assert!(report.contains("3 element(s) in the file, 2 found in the package, 1 missing") && report.contains("- Server (Node)") && report.contains("Billing: BusinessRole in the file"), "{report}");
    }
}

//! Import from a sheet (LC-384, /import-sheet): rows of an Excel or CSV file become elements of a package – matched to
//! the existing ones (by GUID, alias or name and type) so a second import updates instead of duplicating, with a
//! preview create / update / no change before anything is saved, tagged values and relationships from columns, and long
//! texts in the notes. EA's CSV import breaks on long cells, knows no relationships and makes duplicates.

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
(comma-separated). Matching: guid, alias, or name and type (default) in the package and its child packages. action columns: the headers and \
three sample rows (to propose a mapping); action preview: the numbered list create / update / no change (nothing saved); action apply: one save \
the user approves (undoable). At most 1000 rows. Use it for /import-sheet.",
        "input_schema": { "type": "object", "properties": {
            "action": { "type": "string", "enum": ["columns", "preview", "apply"] },
            "file": { "type": "string", "description": "The attachment's file name" },
            "package": { "description": "Target package: package_id or exact name" },
            "mapping": { "type": "object" },
            "match_by": { "type": "string", "enum": ["name", "alias", "guid"] }
        }, "required": ["action", "file"] }
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

/// The plan of an import: the rows with their action and operation, and the relationship operations.
fn plan(input: &Value, cfg: &AppConfig) -> Result<(Vec<Planned>, Vec<Value>, String)> {
    let file = std::path::Path::new(input["file"].as_str().unwrap_or("")).file_name().context("file")?.to_string_lossy().to_string();
    let bytes = std::fs::read(cfg.attachments_dir.join(&file)).with_context(|| format!("there is no attachment {file}"))?;
    let rows = read_rows(&file, &bytes)?;
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
        let found = existing.iter().find(|e| match match_by {
            "guid" => mapped("guid").is_some_and(|c| cell(Some(c)) == e[4]),
            "alias" => mapped("alias").is_some_and(|c| !cell(Some(c)).is_empty() && cell(Some(c)) == e[3]),
            _ => e[1].eq_ignore_ascii_case(&name) && e[2].eq_ignore_ascii_case(&kind),
        });
        let mut fields = Map::new();
        for (key, field) in [("notes", "notes"), ("alias", "alias"), ("status", "status"), ("stereotype", "stereotype")] {
            if let Some(c) = mapped(key) {
                fields.insert(field.into(), json!(cell(Some(c))));
            }
        }
        let mut tags = Map::new();
        for (tag, column) in mapping["tags"].as_object().into_iter().flatten() {
            if let Some(c) = column.as_str().and_then(col) {
                tags.insert(tag.clone(), json!(cell(Some(c))));
            }
        }
        let label = format!("\"{name}\" ({kind})");
        match found {
            Some(e) => {
                let id: i64 = e[0].parse().unwrap_or(0);
                keys.insert(name.to_lowercase(), json!(id));
                let same = |field: &str, idx: usize| fields.get(field).is_none_or(|v| v.as_str() == Some(e[idx].as_str()));
                let tags_same = tags.iter().all(|(k, v)| have_tags.get(&(e[0].clone(), k.to_lowercase())).map(String::as_str).unwrap_or("") == v.as_str().unwrap_or(""));
                if same("notes", 6) && same("alias", 3) && same("status", 7) && same("stereotype", 5) && tags_same {
                    planned.push((i + 1, "no change", label, None));
                } else {
                    let mut op = json!({ "op": "update_element", "element": id });
                    for (k, v) in fields {
                        op[k] = v;
                    }
                    if !tags.is_empty() {
                        op["tags"] = Value::Object(tags);
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
                planned.push((i + 1, "create", label, Some(op)));
            }
        }
    }
    // Relationships from columns naming other elements (of the sheet or already in the package).
    let mut relations = Vec::new();
    for rel in mapping["relationships"].as_array().into_iter().flatten() {
        let (Some(c), Some(kind)) = (rel["column"].as_str().and_then(col), rel["type"].as_str()) else { continue };
        for row in data {
            let source = row.get(name_col).map(|n| n.trim().to_lowercase()).unwrap_or_default();
            for target in row.get(c).map(|v| v.split([',', ';']).map(|t| t.trim().to_lowercase()).filter(|t| !t.is_empty()).collect::<Vec<_>>()).unwrap_or_default() {
                if let (Some(s), Some(t)) = (keys.get(&source), keys.get(&target)) {
                    let known = matches!((s, t), (Value::Number(a), Value::Number(b)) if have_links.contains(&(a.to_string(), b.to_string(), kind.to_lowercase())));
                    if !known {
                        relations.push(json!({ "op": "create_connector", "source": s, "target": t, "type": kind }));
                    }
                }
            }
        }
    }
    Ok((planned, relations, title))
}

/// columns: headers and samples; preview: the plan as text; apply: the operations.
pub fn run(input: &Value, cfg: &AppConfig) -> Result<(Vec<Value>, String)> {
    if input["action"].as_str() == Some("columns") {
        let file = std::path::Path::new(input["file"].as_str().unwrap_or("")).file_name().context("file")?.to_string_lossy().to_string();
        let rows = read_rows(&file, &std::fs::read(cfg.attachments_dir.join(&file)).with_context(|| format!("there is no attachment {file}"))?)?;
        let mut text = format!("{} rows. Columns: {}\nSamples:\n", rows.len().saturating_sub(1), rows.first().map(|h| h.join(" | ")).unwrap_or_default());
        for r in rows.iter().skip(1).take(3) {
            text.push_str(&format!("  {}\n", r.join(" | ")));
        }
        return Ok((Vec::new(), text));
    }
    let (planned, relations, title) = plan(input, cfg)?;
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
}

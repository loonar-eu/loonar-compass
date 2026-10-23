//! Model ↔ code drift: a source folder (read only) is compared with the model. From the code: classes, interfaces,
//! enums and records with their fields (Java, Kotlin, C#, TypeScript, Rust, Python), database tables with columns
//! (SQL CREATE TABLE) and API operations (OpenAPI operationId). The report says what is in the code and not in the
//! model, what is in the model and no longer in the code, and how the fields / columns / operations differ.

use crate::config::AppConfig;
use crate::tools::query_rows;
use anyhow::{bail, Result};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::Path;

const SKIP_DIRS: &[&str] = &["node_modules", "target", "build", "dist", ".git", ".idea", "bin", "obj", "out", "vendor", "__pycache__", ".gradle", "coverage"];
const MAX_FILES: usize = 20_000;
const MAX_FILE_BYTES: u64 = 1_000_000;

#[derive(Default, Debug)]
pub struct CodeModel {
    /// Type name → (kind, fields, file).
    pub types: BTreeMap<String, (String, BTreeSet<String>, String)>,
    /// Table name → columns.
    pub tables: BTreeMap<String, BTreeSet<String>>,
    /// API operation ids.
    pub operations: BTreeSet<String>,
    pub files: usize,
}

fn word_after<'a>(line: &'a str, keyword: &str) -> Option<&'a str> {
    let mut rest = line;
    while let Some(i) = rest.find(keyword) {
        let before_ok = i == 0 || !rest.as_bytes()[i - 1].is_ascii_alphanumeric();
        let after = &rest[i + keyword.len()..];
        if before_ok && after.starts_with(' ') {
            let name: &str = after.trim_start().split(|c: char| !(c.is_alphanumeric() || c == '_')).next().unwrap_or("");
            if !name.is_empty() && name.chars().next().is_some_and(|c| c.is_alphabetic()) {
                return Some(name);
            }
        }
        rest = after;
    }
    None
}

/// A field of a class body line: "private String name;", "public int Id { get; set; }", "name: string;", "pub name: String,", "self.name =".
fn field_of(line: &str, lang: &str) -> Option<String> {
    let l = line.trim();
    if l.is_empty() || l.starts_with("//") || l.starts_with('*') || l.starts_with('@') || l.starts_with('#') || l.contains('(') && !l.contains("=>") && lang != "py" {
        return None;
    }
    match lang {
        "java" | "cs" | "kt" => {
            let l = l.trim_end_matches(';').split('=').next()?.trim();
            let l = l.split('{').next()?.trim();
            let words: Vec<&str> = l.split_whitespace().filter(|w| !["private", "public", "protected", "internal", "static", "final", "readonly", "val", "var", "lateinit", "override", "const", "transient", "volatile"].contains(w)).collect();
            if lang == "kt" {
                return l.split(':').next().map(|s| s.split_whitespace().last().unwrap_or("").to_string()).filter(|s| !s.is_empty() && (line.contains("val ") || line.contains("var ")));
            }
            (words.len() == 2 && (line.trim_end().ends_with(';') || line.contains("get;"))).then(|| words[1].to_string())
        }
        "ts" => {
            let name = l.split(':').next()?.trim().trim_start_matches("readonly ").trim_start_matches("private ").trim_start_matches("public ").trim_end_matches('?');
            (l.contains(':') && name.chars().all(|c| c.is_alphanumeric() || c == '_') && !name.is_empty()).then(|| name.to_string())
        }
        "rs" => {
            let l = l.trim_start_matches("pub(crate) ").trim_start_matches("pub ");
            let name = l.split(':').next()?.trim();
            (l.contains(':') && l.ends_with(',') && name.chars().all(|c| c.is_alphanumeric() || c == '_')).then(|| name.to_string())
        }
        "py" => l.strip_prefix("self.").and_then(|r| r.split(['=', ':', ' ']).next()).filter(|n| !n.is_empty()).map(String::from),
        _ => None,
    }
}

pub fn scan(root: &Path) -> Result<CodeModel> {
    if !root.is_dir() {
        bail!("{}", crate::i18n::fmt("folder {0} does not exist", &[root.display().to_string()]));
    }
    let mut model = CodeModel::default();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            if path.is_dir() {
                if !SKIP_DIRS.contains(&name.as_str()) && !name.starts_with('.') {
                    stack.push(path);
                }
                continue;
            }
            if model.files >= MAX_FILES || entry.metadata().map(|m| m.len() > MAX_FILE_BYTES).unwrap_or(true) {
                continue;
            }
            let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
            let lang = match ext.as_str() {
                "java" => "java",
                "cs" => "cs",
                "kt" => "kt",
                "ts" | "tsx" => "ts",
                "rs" => "rs",
                "py" => "py",
                "sql" => "sql",
                "yaml" | "yml" | "json" => "api",
                _ => continue,
            };
            let Ok(text) = std::fs::read_to_string(&path) else { continue };
            model.files += 1;
            let rel = path.strip_prefix(root).unwrap_or(&path).display().to_string();
            match lang {
                "sql" => scan_sql(&text, &mut model),
                "api" => {
                    if text.contains("openapi") || text.contains("swagger") {
                        for line in text.lines() {
                            let l = line.trim().trim_start_matches('"');
                            if let Some(rest) = l.strip_prefix("operationId") {
                                let id = rest.trim_start_matches(['"', ':', ' ']).trim_end_matches([',', '"', ' ']).trim_matches('"');
                                if !id.is_empty() {
                                    model.operations.insert(id.to_string());
                                }
                            }
                        }
                    }
                }
                _ => scan_types(&text, lang, &rel, &mut model),
            }
        }
    }
    Ok(model)
}

fn scan_types(text: &str, lang: &str, file: &str, model: &mut CodeModel) {
    let keywords: &[(&str, &str)] = &[("class", "Class"), ("interface", "Interface"), ("enum", "Enumeration"), ("record", "Class"), ("struct", "Class"), ("trait", "Interface")];
    let mut current: Option<String> = None;
    let mut depth = 0i32;
    let mut class_depth = 0i32;
    for line in text.lines() {
        let trimmed = line.trim();
        if !(trimmed.starts_with("//") || trimmed.starts_with('*') || trimmed.starts_with('#') && lang != "py") {
            if let Some((name, kind)) = keywords.iter().find_map(|(k, kind)| word_after(trimmed, k).map(|n| (n, *kind))) {
                let declaration = trimmed.contains('{') || lang == "py" && trimmed.ends_with(':') || lang == "rs" || lang == "kt" || trimmed.ends_with(name);
                if declaration && !trimmed.starts_with("import") && !trimmed.contains(" new ") {
                    model.types.entry(name.to_string()).or_insert_with(|| (kind.to_string(), BTreeSet::new(), file.to_string()));
                    current = Some(name.to_string());
                    class_depth = depth + 1;
                    // Kotlin / Java record parameters: class Name(val a: T, val b: T)
                    if let Some(params) = trimmed.split_once('(').map(|p| p.1) {
                        if lang == "kt" || trimmed.contains("record ") {
                            for p in params.split(',') {
                                let p = p.trim().trim_start_matches("val ").trim_start_matches("var ").trim_start_matches("private ");
                                let n = if lang == "kt" { p.split(':').next() } else { p.split_whitespace().nth(1) };
                                if let Some(n) = n.map(|n| n.trim().trim_end_matches(')').trim()).filter(|n| !n.is_empty() && n.chars().all(|c| c.is_alphanumeric() || c == '_')) {
                                    model.types.get_mut(name).map(|t| t.1.insert(n.to_string()));
                                }
                            }
                        }
                    }
                }
            } else if let Some(c) = &current {
                if depth == class_depth || lang == "py" {
                    if let Some(f) = field_of(line, lang) {
                        if f.chars().next().is_some_and(|c| c.is_alphabetic() || c == '_') {
                            model.types.get_mut(c).map(|t| t.1.insert(f));
                        }
                    }
                }
            }
        }
        depth += line.matches('{').count() as i32 - line.matches('}').count() as i32;
        if lang != "py" && current.is_some() && depth < class_depth {
            current = None;
        }
    }
}

fn scan_sql(text: &str, model: &mut CodeModel) {
    let lower = text.to_lowercase();
    let mut rest = lower.as_str();
    let mut offset = 0;
    while let Some(i) = rest.find("create table") {
        let start = offset + i + "create table".len();
        let tail = &lower[start..];
        let tail = tail.trim_start().trim_start_matches("if not exists").trim_start();
        let name: String = tail.split(|c: char| c.is_whitespace() || c == '(').next().unwrap_or("").trim_matches(['"', '`', '[', ']']).rsplit('.').next().unwrap_or("").to_string();
        let mut columns = BTreeSet::new();
        if let (Some(open), true) = (tail.find('('), !name.is_empty()) {
            let mut depth = 0;
            let mut body = String::new();
            for c in tail[open..].chars() {
                match c {
                    '(' => depth += 1,
                    ')' => {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                    }
                    _ => {}
                }
                if depth >= 1 {
                    body.push(c);
                }
            }
            let mut d = 0;
            let mut item = String::new();
            for c in body.trim_start_matches('(').chars().chain(std::iter::once(',')) {
                match c {
                    '(' => d += 1,
                    ')' => d -= 1,
                    ',' if d == 0 => {
                        let col = item.split_whitespace().next().unwrap_or("").trim_matches(['"', '`', '[', ']']).to_string();
                        if !col.is_empty() && !["primary", "constraint", "foreign", "unique", "key", "index", "check"].contains(&col.as_str()) {
                            columns.insert(col);
                        }
                        item.clear();
                        continue;
                    }
                    _ => {}
                }
                item.push(c);
            }
            model.tables.insert(name, columns);
        }
        offset = start;
        rest = &lower[offset..];
    }
}

fn norm(name: &str) -> String {
    name.to_lowercase().replace(['_', '-', ' '], "")
}

/// The drift report.
pub fn compare(cfg: &AppConfig, root: &Path, package: Option<i64>) -> Result<String> {
    let code = scan(root)?;
    let filter = match package {
        Some(p) => {
            let ids: Vec<String> = query_rows(cfg, &format!(
                "with recursive tree(package_id) as (select package_id from t_package where package_id = {p} \
                 union all select x.package_id from t_package x join tree t on x.parent_id = t.package_id) select package_id from tree"
            ))?
            .into_iter()
            .map(|r| r[0].clone())
            .collect();
            format!(" and o.package_id in ({})", ids.join(","))
        }
        None => String::new(),
    };
    let rows = query_rows(cfg, &format!(
        "select o.object_id as c0, coalesce(o.name,'') as c1, o.object_type as c2, coalesce(o.stereotype,'') as c3 from t_object o \
         where o.object_type in ('Class','Interface','Enumeration','DataType','Table'){filter}"
    ))?;
    let mut attributes: HashMap<String, BTreeSet<String>> = HashMap::new();
    for r in query_rows(cfg, "select object_id as c0, coalesce(name,'') as c1 from t_attribute").unwrap_or_default() {
        attributes.entry(r[0].clone()).or_default().insert(norm(&r[1]));
    }
    let model_operations: BTreeSet<String> = query_rows(cfg, "select distinct coalesce(name,'') as c0 from t_operation").unwrap_or_default().into_iter().map(|r| norm(&r[0])).collect();
    let is_table = |r: &Vec<String>| r[2] == "Table" || r[3].eq_ignore_ascii_case("table");
    let model_types: HashMap<String, &Vec<String>> = rows.iter().filter(|r| !is_table(r)).map(|r| (norm(&r[1]), r)).collect();
    let model_tables: HashMap<String, &Vec<String>> = rows.iter().filter(|r| is_table(r)).map(|r| (norm(&r[1]), r)).collect();

    let mut only_code = Vec::new();
    let mut differences = Vec::new();
    for (name, (kind, fields, file)) in &code.types {
        match model_types.get(&norm(name)) {
            None => only_code.push(format!("{name} ({kind}) – {file}")),
            Some(r) => {
                let model_fields = attributes.get(&r[0]).cloned().unwrap_or_default();
                if fields.is_empty() || model_fields.is_empty() {
                    continue;
                }
                let code_fields: BTreeSet<String> = fields.iter().map(|f| norm(f)).collect();
                let missing: Vec<&String> = fields.iter().filter(|f| !model_fields.contains(&norm(f))).collect();
                let gone: Vec<&String> = model_fields.iter().filter(|f| !code_fields.contains(*f)).collect();
                if !missing.is_empty() || !gone.is_empty() {
                    differences.push(crate::i18n::fmt("{0} (#{1}): in the code only: {2}; in the model only: {3}", &[
                        name.clone(), r[0].clone(),
                        missing.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", "),
                        gone.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", "),
                    ]));
                }
            }
        }
    }
    let code_names: BTreeSet<String> = code.types.keys().map(|n| norm(n)).collect();
    let only_model: Vec<String> = if code.types.is_empty() {
        Vec::new()
    } else {
        rows.iter().filter(|r| !is_table(r) && !code_names.contains(&norm(&r[1]))).map(|r| format!("{} ({}) #{}", r[1], r[2], r[0])).collect()
    };
    let mut tables = Vec::new();
    for (name, columns) in &code.tables {
        match model_tables.get(&norm(name)) {
            None => tables.push(crate::i18n::fmt("table {0}: in the code only ({1} columns)", &[name.clone(), columns.len().to_string()])),
            Some(r) => {
                let model_cols = attributes.get(&r[0]).cloned().unwrap_or_default();
                let code_cols: BTreeSet<String> = columns.iter().map(|c| norm(c)).collect();
                let missing: Vec<&String> = columns.iter().filter(|c| !model_cols.contains(&norm(c))).collect();
                let gone: Vec<&String> = model_cols.iter().filter(|c| !code_cols.contains(*c)).collect();
                if !missing.is_empty() || !gone.is_empty() {
                    tables.push(crate::i18n::fmt("table {0}: columns in the code only: {1}; in the model only: {2}", &[name.clone(), missing.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", "), gone.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ")]));
                }
            }
        }
    }
    let api_missing: Vec<&String> = code.operations.iter().filter(|o| !model_operations.contains(&norm(o))).collect();
    let mut out = vec![crate::i18n::fmt(
        "Model ↔ code: {0} source files; in the code {1} types, {2} tables, {3} API operations. Types only in the code: {4}; only in the model: {5}; field differences: {6}; table differences: {7}; API operations missing in the model: {8}.",
        &[code.files.to_string(), code.types.len().to_string(), code.tables.len().to_string(), code.operations.len().to_string(), only_code.len().to_string(), only_model.len().to_string(), differences.len().to_string(), tables.len().to_string(), api_missing.len().to_string()],
    )];
    let mut section = |title: &str, items: Vec<String>| {
        if !items.is_empty() {
            out.push(format!("\n## {title}"));
            out.extend(items.into_iter().take(200));
        }
    };
    section(crate::i18n::tr("In the code, not in the model"), only_code);
    section(crate::i18n::tr("In the model, not in the code"), only_model);
    section(crate::i18n::tr("Different fields (code vs model attributes)"), differences);
    section(crate::i18n::tr("Database tables"), tables);
    section(crate::i18n::tr("API operations (OpenAPI operationId) not in the model"), api_missing.into_iter().cloned().collect());
    Ok(out.join("\n"))
}

pub fn definition() -> serde_json::Value {
    serde_json::json!({
        "name": "code_drift",
        "description": "Model ↔ code drift: reads a source folder (read only, a local clone of a repository) and compares it with the model: classes, interfaces, \
enums and records with their fields (Java, Kotlin, C#, TypeScript, Rust, Python), database tables and columns (SQL CREATE TABLE) and API operations (OpenAPI \
operationId) against classes, attributes, tables and operations in the model. Reports what is only in the code, what is only in the model and the field / column \
differences. Present the most important drift and, on request, update the model (ea_write) or save the report to Excel.",
        "input_schema": {
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Full path to the source folder (e.g. a local clone of a repository)." },
                "package": { "type": "integer", "description": "Optionally only this package of the model with subpackages." }
            },
            "required": ["path"]
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scans_languages() {
        let dir = std::env::temp_dir().join(format!("loonar-drift-{}", std::process::id()));
        let _ = std::fs::create_dir_all(dir.join("src/main/java"));
        let _ = std::fs::create_dir_all(dir.join("node_modules/x"));
        std::fs::write(dir.join("src/main/java/Customer.java"), "package shop;\n// class Fake {\npublic class Customer {\n    private String name;\n    private String email;\n    public String getName() { return name; }\n}\n").unwrap();
        std::fs::write(dir.join("src/Order.ts"), "export interface Order {\n  id: string;\n  total?: number;\n}\nexport enum Status { New, Paid }\n").unwrap();
        std::fs::write(dir.join("model.rs"), "pub struct Invoice {\n    pub number: String,\n    amount: u64,\n}\n").unwrap();
        std::fs::write(dir.join("Payment.kt"), "data class Payment(val id: String, val amount: Long)\n").unwrap();
        std::fs::write(dir.join("schema.sql"), "CREATE TABLE IF NOT EXISTS public.orders (\n  id bigint primary key,\n  customer_id bigint,\n  total numeric(10,2),\n  CONSTRAINT fk FOREIGN KEY (customer_id) REFERENCES customers(id)\n);\n").unwrap();
        std::fs::write(dir.join("api.yaml"), "openapi: 3.0.0\npaths:\n  /orders:\n    get:\n      operationId: listOrders\n    post:\n      operationId: \"createOrder\"\n").unwrap();
        std::fs::write(dir.join("node_modules/x/Skip.java"), "public class Skip {}\n").unwrap();
        let m = scan(&dir).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        assert!(m.types.contains_key("Customer") && !m.types.contains_key("Fake") && !m.types.contains_key("Skip"), "{:?}", m.types.keys());
        assert_eq!(m.types["Customer"].1, ["email", "name"].iter().map(|s| s.to_string()).collect());
        assert_eq!(m.types["Order"].1, ["id", "total"].iter().map(|s| s.to_string()).collect());
        assert_eq!(m.types["Status"].0, "Enumeration");
        assert_eq!(m.types["Invoice"].1, ["amount", "number"].iter().map(|s| s.to_string()).collect());
        assert_eq!(m.types["Payment"].1, ["amount", "id"].iter().map(|s| s.to_string()).collect());
        assert_eq!(m.tables["orders"], ["customer_id", "id", "total"].iter().map(|s| s.to_string()).collect());
        assert_eq!(m.operations, ["createOrder", "listOrders"].iter().map(|s| s.to_string()).collect());
    }
}

#[cfg(test)]
mod model_tests {
    /// A small repository against a model (LOONAR_TEST_PROPS). `cargo test drift_against_model -- --ignored --nocapture`
    #[test]
    #[ignore = "requires a test model"]
    fn drift_against_model() {
        let cfg = crate::config::AppConfig::load(std::path::Path::new(&std::env::var("LOONAR_TEST_PROPS").unwrap())).unwrap();
        crate::i18n::set(crate::i18n::Lang::En);
        let dir = std::env::temp_dir().join(format!("loonar-drift-model-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("Customer.java"), "public class Customer {\n  private String name;\n  private String email;\n}\n").unwrap();
        std::fs::write(dir.join("Order.java"), "public class Order {\n  private long id;\n}\n").unwrap();
        std::fs::write(dir.join("Shipment.java"), "public class Shipment {\n  private String carrier;\n}\n").unwrap();
        std::fs::write(dir.join("schema.sql"), "create table orders (id bigint, total numeric);\n").unwrap();
        let out = super::compare(&cfg, &dir, None).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        println!("{out}");
        let section = |title: &str| out.split("\n## ").find(|s| s.starts_with(title)).unwrap_or("").to_string();
        assert!(section("In the code, not in the model").contains("Shipment"));
        assert!(!section("In the code, not in the model").contains("Customer"), "Customer is in the model");
        assert!(section("In the model, not in the code").contains("Invoice"), "Invoice is only in the model");
        assert!(section("Database tables").contains("orders"));
    }
}

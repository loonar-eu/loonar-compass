//! Two notation bridges (LC-408):
//! - import_mermaid: a Mermaid classDiagram as classes (attributes, operations) and relationships (inheritance,
//!   realisation, composition, aggregation, association, dependency) in a new package with a diagram – one save;
//! - table_ddl: CREATE TABLE statements of the Table elements of a package for PostgreSQL, SQL Server, MySQL, Oracle or
//!   SQLite – columns with types mapped to the dialect, NOT NULL, primary keys (EA's PK operations). Reads only.

use crate::config::AppConfig;
use crate::tools::query_rows;
use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use std::collections::BTreeMap;

pub fn definitions() -> Vec<Value> {
    vec![
        json!({
            "name": "import_mermaid",
            "description": "A Mermaid classDiagram (text, or an attached .mmd/.md) as a new package with classes (attributes with types and \
visibility, operations), interfaces and enumerations (<<interface>>, <<enumeration>>), relationships (<|-- inheritance, ..|> realisation, \
*-- composition, o-- aggregation, --> association, ..> dependency, with labels) and a class diagram laid out by EA. action preview shows what \
would be created; apply saves it (one save the user approves).",
            "input_schema": { "type": "object", "properties": {
                "text": { "type": "string" }, "file": { "type": "string" },
                "package": { "description": "package_id or exact name of the parent" }, "name": { "type": "string" },
                "action": { "type": "string", "enum": ["preview", "apply"] }
            }, "required": ["package"] }
        }),
        json!({
            "name": "table_ddl",
            "description": "CREATE TABLE statements of the Table elements of a package (with child packages) for postgres, sqlserver, mysql, \
oracle or sqlite: columns with their types mapped to the dialect, lengths, NOT NULL, primary keys (EA's PK operations). For databases EA \
does not generate, or to compare. Reads only; give the SQL to the user in a code block.",
            "input_schema": { "type": "object", "properties": {
                "package": { "description": "package_id or exact name" },
                "dialect": { "type": "string", "enum": ["postgres", "sqlserver", "mysql", "oracle", "sqlite"] }
            }, "required": ["package", "dialect"] }
        }),
    ]
}

// ---------- Mermaid ----------

#[derive(Debug, Default, Clone, PartialEq)]
pub(crate) struct MClass {
    pub name: String,
    pub kind: String,
    pub attributes: Vec<(String, String, String)>, // (visibility, name, type)
    pub operations: Vec<(String, String, String)>, // (visibility, name(params), return)
}

/// (from, to, type, label); for a composition or aggregation from = the part, to = the whole (as EA draws it).
pub(crate) type MLink = (String, String, String, String);

fn visibility(c: char) -> &'static str {
    match c {
        '-' => "Private",
        '#' => "Protected",
        '~' => "Package",
        _ => "Public",
    }
}

/// A member line: "+name : Type", "+Type name", "+method(args) Return".
fn member(line: &str, class: &mut MClass) {
    let line = line.trim().trim_end_matches(['$', '*']);
    if line.is_empty() || line.starts_with("%%") {
        return;
    }
    if let Some(stereo) = line.strip_prefix("<<").and_then(|s| s.strip_suffix(">>")) {
        class.kind = match stereo.trim().to_lowercase().as_str() { "interface" => "Interface", "enumeration" | "enum" => "Enumeration", _ => "Class" }.to_string();
        return;
    }
    let (vis, rest) = match line.chars().next() {
        Some(c @ ('+' | '-' | '#' | '~')) => (visibility(c), line[1..].trim()),
        _ => ("Public", line),
    };
    if let Some(open) = rest.find('(') {
        let close = rest.rfind(')').unwrap_or(rest.len());
        let name = rest[..open].trim().to_string();
        let ret = rest[close.min(rest.len())..].trim_start_matches(')').trim().trim_start_matches(':').trim().to_string();
        class.operations.push((vis.into(), format!("{name}({})", &rest[open + 1..close.max(open + 1)]), ret));
    } else if let Some((name, ty)) = rest.split_once(':') {
        class.attributes.push((vis.into(), name.trim().to_string(), ty.trim().to_string()));
    } else if let Some((ty, name)) = rest.rsplit_once(' ') {
        class.attributes.push((vis.into(), name.trim().to_string(), ty.trim().to_string()));
    } else {
        class.attributes.push((vis.into(), rest.to_string(), String::new()));
    }
}

/// The relationship arrows of Mermaid, longest first: (arrow, type, reversed = the arrow points left).
const ARROWS: &[(&str, &str, bool)] = &[
    ("<|--", "Generalization", true), ("--|>", "Generalization", false), ("<|..", "Realisation", true), ("..|>", "Realisation", false),
    ("*--", "Composition", true), ("--*", "Composition", false), ("o--", "Aggregation", true), ("--o", "Aggregation", false),
    ("<--", "Association", true), ("-->", "Association", false), ("<..", "Dependency", true), ("..>", "Dependency", false), ("--", "Association", false), ("..", "Dependency", false),
];

pub(crate) fn parse_mermaid(text: &str) -> Result<(BTreeMap<String, MClass>, Vec<MLink>)> {
    let mut classes: BTreeMap<String, MClass> = BTreeMap::new();
    let mut links = Vec::new();
    let mut open: Option<String> = None;
    let mut seen_header = false;
    let ensure = |classes: &mut BTreeMap<String, MClass>, name: &str| {
        classes.entry(name.to_string()).or_insert_with(|| MClass { name: name.to_string(), kind: "Class".into(), ..Default::default() });
    };
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with("%%") || line.starts_with("```") {
            continue;
        }
        if line.starts_with("classDiagram") {
            seen_header = true;
            continue;
        }
        if let Some(name) = &open {
            if line == "}" {
                open = None;
            } else {
                let mut c = classes.remove(name).unwrap_or_default();
                member(line, &mut c);
                classes.insert(name.clone(), c);
            }
            continue;
        }
        if let Some(rest) = line.strip_prefix("class ") {
            let rest = rest.trim();
            let (name, body) = match rest.split_once('{') { Some((n, b)) => (n.trim(), Some(b.trim())), None => (rest.split_whitespace().next().unwrap_or(""), None) };
            let name = name.split('~').next().unwrap_or(name).trim().to_string();
            ensure(&mut classes, &name);
            if let Some(stereo) = rest.split_once("<<").and_then(|(_, s)| s.split_once(">>")).map(|(s, _)| s) {
                let mut c = classes.remove(&name).unwrap_or_default();
                member(&format!("<<{stereo}>>"), &mut c);
                classes.insert(name.clone(), c);
            }
            if body.is_some_and(|b| !b.ends_with('}')) {
                open = Some(name);
            }
            continue;
        }
        if let Some(stereo) = line.strip_prefix("<<").and_then(|s| s.split_once(">>")) {
            let name = stereo.1.trim().to_string();
            ensure(&mut classes, &name);
            let mut c = classes.remove(&name).unwrap_or_default();
            member(&format!("<<{}>>", stereo.0), &mut c);
            classes.insert(name, c);
            continue;
        }
        // "A <|-- B : label" (cardinalities in quotes are skipped).
        let (head, label) = line.split_once(" : ").map(|(h, l)| (h, l.trim())).unwrap_or((line, ""));
        let cleaned: String = head.split('"').enumerate().filter(|(i, _)| i % 2 == 0).map(|(_, p)| p).collect::<Vec<_>>().join(" ");
        if let Some((arrow, kind, reversed)) = ARROWS.iter().find(|(a, _, _)| cleaned.contains(a)) {
            let (a, b) = cleaned.split_once(arrow).unwrap_or(("", ""));
            let (a, b) = (a.trim().to_string(), b.trim().to_string());
            if a.is_empty() || b.is_empty() {
                continue;
            }
            ensure(&mut classes, &a);
            ensure(&mut classes, &b);
            let (from, to) = if *reversed { (b, a) } else { (a, b) };
            links.push((from, to, kind.to_string(), label.to_string()));
            continue;
        }
        // "Class : +member"
        if let Some((name, m)) = line.split_once(" : ").filter(|(n, _)| !n.contains(' ')) {
            ensure(&mut classes, name.trim());
            let mut c = classes.remove(name.trim()).unwrap_or_default();
            member(m, &mut c);
            classes.insert(name.trim().to_string(), c);
        }
    }
    if !seen_header && classes.is_empty() {
        bail!("not a Mermaid classDiagram (it starts with \"classDiagram\")");
    }
    Ok((classes, links))
}

pub fn import_mermaid(input: &Value, cfg: &AppConfig) -> Result<(Vec<Value>, String)> {
    let text = match input["file"].as_str().filter(|f| !f.trim().is_empty()) {
        Some(f) => {
            let name = std::path::Path::new(f).file_name().context("file")?.to_string_lossy().to_string();
            std::fs::read_to_string(crate::history::attachment_path(&cfg.attachments_dir, &name)).with_context(|| format!("there is no attachment {name}"))?
        }
        None => input["text"].as_str().unwrap_or("").to_string(),
    };
    let (classes, links) = parse_mermaid(&text)?;
    if classes.is_empty() {
        bail!("no classes in the diagram");
    }
    let (scope, _) = crate::model_usage::package_scope(cfg, &input["package"])?;
    let parent: i64 = scope.split('(').nth(1).and_then(|s| s.split([',', ')']).next()).and_then(|p| p.parse().ok()).context("package")?;
    let name = input["name"].as_str().map(str::trim).filter(|n| !n.is_empty()).unwrap_or("Mermaid import");
    let key = |n: &str| format!("m_{}", n.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect::<String>());
    let mut ops = vec![
        json!({ "op": "create_package", "key": "pkg", "parent": parent, "name": name }),
        json!({ "op": "create_diagram", "key": "dia", "package": "#pkg", "name": name, "type": "Logical" }),
    ];
    for (i, c) in classes.values().enumerate() {
        let k = key(&c.name);
        ops.push(json!({ "op": "create_element", "key": k, "package": "#pkg", "name": c.name, "type": c.kind }));
        for (vis, n, ty) in &c.attributes {
            ops.push(json!({ "op": "create_attribute", "element": format!("#{k}"), "name": n, "type": ty, "visibility": vis }));
        }
        for (vis, sig, ret) in &c.operations {
            let (n, params) = sig.split_once('(').unwrap_or((sig, ""));
            let params: Vec<Value> = params.trim_end_matches(')').split(',').map(str::trim).filter(|p| !p.is_empty()).map(|p| {
                let (pn, pt) = p.split_once(':').map(|(a, b)| (a.trim(), b.trim())).or_else(|| p.rsplit_once(' ').map(|(t, n)| (n.trim(), t.trim()))).unwrap_or((p, ""));
                json!({ "name": pn, "type": pt })
            }).collect();
            ops.push(json!({ "op": "create_operation", "element": format!("#{k}"), "name": n, "return_type": ret, "visibility": vis, "parameters": params }));
        }
        ops.push(json!({ "op": "place", "diagram": "#dia", "element": format!("#{k}"), "left": 40 + (i as i64 % 4) * 230, "top": 40 + (i as i64 / 4) * 200, "width": 180, "height": 120 }));
    }
    for (from, to, kind, label) in &links {
        let (kind, subtype) = match kind.as_str() { "Composition" => ("Aggregation", Some("Strong")), "Aggregation" => ("Aggregation", Some("Weak")), k => (k, None) };
        // In EA an aggregation goes from the part to the whole; parse_mermaid already gives (part, whole).
        let (s, t) = (from, to);
        let mut op = json!({ "op": "create_connector", "source": format!("#{}", key(s)), "target": format!("#{}", key(t)), "type": kind, "name": label });
        if let Some(st) = subtype {
            op["subtype"] = json!(st);
        }
        ops.push(op);
    }
    ops.push(json!({ "op": "layout_diagram", "diagram": "#dia", "direction": "down", "spacing": 40 }));
    let text = format!("Mermaid classDiagram → package \"{name}\": {} classes, {} relationships, a diagram.", classes.len(), links.len());
    Ok(if input["action"].as_str() == Some("apply") { (ops, text) } else { (Vec::new(), format!("{text} Nothing saved yet: action apply saves it.")) })
}

// ---------- DDL ----------

/// A column type in a dialect (generic names mapped; others kept as they are).
pub(crate) fn column_type(dialect: &str, ty: &str, length: &str) -> String {
    let t = ty.trim().to_lowercase();
    let len = length.trim().parse::<u32>().ok().filter(|l| *l > 0);
    let with_len = |base: &str, default: u32| format!("{base}({})", len.unwrap_or(default));
    match (dialect, t.as_str()) {
        (_, "varchar" | "nvarchar" | "string" | "char varying") => match dialect { "oracle" => with_len("VARCHAR2", 255), "sqlserver" => with_len("NVARCHAR", 255), "sqlite" => "TEXT".into(), _ => with_len("VARCHAR", 255) },
        (_, "char" | "nchar") => with_len("CHAR", 1),
        (_, "text" | "clob" | "longtext" | "memo") => match dialect { "oracle" => "CLOB".into(), "sqlserver" => "NVARCHAR(MAX)".into(), "mysql" => "LONGTEXT".into(), _ => "TEXT".into() },
        (_, "int" | "integer" | "int4") => match dialect { "oracle" => "NUMBER(10)".into(), _ => "INTEGER".into() },
        (_, "bigint" | "long" | "int8") => match dialect { "oracle" => "NUMBER(19)".into(), _ => "BIGINT".into() },
        (_, "smallint" | "short") => match dialect { "oracle" => "NUMBER(5)".into(), _ => "SMALLINT".into() },
        (_, "boolean" | "bool" | "bit") => match dialect { "oracle" => "NUMBER(1)".into(), "sqlserver" => "BIT".into(), "sqlite" => "INTEGER".into(), _ => "BOOLEAN".into() },
        (_, "datetime" | "timestamp") => match dialect { "sqlserver" => "DATETIME2".into(), "mysql" => "DATETIME".into(), "sqlite" => "TEXT".into(), _ => "TIMESTAMP".into() },
        (_, "date") => match dialect { "sqlite" => "TEXT".into(), _ => "DATE".into() },
        (_, "decimal" | "numeric" | "money") => match dialect { "oracle" => "NUMBER(18,2)".into(), "sqlite" => "REAL".into(), _ => "DECIMAL(18,2)".into() },
        (_, "float" | "double" | "real") => match dialect { "oracle" => "BINARY_DOUBLE".into(), "sqlserver" => "FLOAT".into(), "postgres" => "DOUBLE PRECISION".into(), _ => "REAL".into() },
        (_, "uuid" | "guid" | "uniqueidentifier") => match dialect { "sqlserver" => "UNIQUEIDENTIFIER".into(), "postgres" => "UUID".into(), "oracle" => "RAW(16)".into(), _ => "CHAR(36)".into() },
        (_, "blob" | "binary" | "varbinary" | "bytea") => match dialect { "postgres" => "BYTEA".into(), "sqlserver" => "VARBINARY(MAX)".into(), _ => "BLOB".into() },
        _ if ty.trim().is_empty() => "VARCHAR(255)".into(),
        _ => ty.trim().to_uppercase(),
    }
}

fn quote_name(dialect: &str, name: &str) -> String {
    let plain = name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') && !name.chars().next().is_some_and(|c| c.is_ascii_digit());
    if plain {
        return name.to_string();
    }
    match dialect { "mysql" => format!("`{}`", name.replace('`', "``")), "sqlserver" => format!("[{}]", name.replace(']', "]]")), _ => format!("\"{}\"", name.replace('"', "\"\"")) }
}

pub fn table_ddl(input: &Value, cfg: &AppConfig) -> Result<String> {
    let dialect = input["dialect"].as_str().unwrap_or("postgres");
    if !matches!(dialect, "postgres" | "sqlserver" | "mysql" | "oracle" | "sqlite") {
        bail!("dialect: postgres, sqlserver, mysql, oracle or sqlite");
    }
    let (scope, title) = crate::model_usage::package_scope(cfg, &input["package"])?;
    let tables = query_rows(cfg, &format!("select o.object_id, o.name from t_object o where (o.object_type = 'Table' or (o.object_type = 'Class' and lower(coalesce(o.stereotype,'')) = 'table')) {scope} order by o.name"))?;
    if tables.is_empty() {
        bail!("no Table elements in the {title}");
    }
    let mut out = vec![format!("-- {} table(s) of the {title}, {dialect}", tables.len())];
    for t in &tables {
        let id: i64 = t[0].parse().unwrap_or(0);
        let columns = query_rows(cfg, &format!("select name, coalesce(type,''), coalesce(length,''), coalesce(allowduplicates,0) from t_attribute where object_id = {id} order by pos"))?;
        let pk: Vec<String> = query_rows(cfg, &format!(
            "select p.name from t_operation o join t_operationparams p on p.operationid = o.operationid where o.object_id = {id} and upper(coalesce(o.stereotype,'')) = 'PK' order by p.pos"
        ))?.into_iter().map(|r| quote_name(dialect, &r[0])).collect();
        let mut lines: Vec<String> = columns.iter().map(|c| {
            let not_null = c[3] == "1" || c[3].eq_ignore_ascii_case("true") || pk.contains(&quote_name(dialect, &c[0]));
            format!("  {} {}{}", quote_name(dialect, &c[0]), column_type(dialect, &c[1], &c[2]), if not_null { " NOT NULL" } else { "" })
        }).collect();
        if !pk.is_empty() {
            lines.push(format!("  PRIMARY KEY ({})", pk.join(", ")));
        }
        out.push(format!("CREATE TABLE {} (\n{}\n);", quote_name(dialect, &t[1]), lines.join(",\n")));
    }
    Ok(out.join("\n\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mermaid_classes_and_links() {
        let (c, l) = parse_mermaid("classDiagram\n  class Order {\n    +String id\n    -date : Date\n    +total() Money\n  }\n  class Shape {\n    <<interface>>\n  }\n  Order \"1\" *-- \"many\" Line : contains\n  Animal <|-- Duck\n  Order ..> Shape\n  Duck : +swim()\n").unwrap();
        assert_eq!(c["Order"].attributes, vec![("Public".to_string(), "id".to_string(), "String".to_string()), ("Private".to_string(), "date".to_string(), "Date".to_string())]);
        assert_eq!(c["Order"].operations[0], ("Public".to_string(), "total()".to_string(), "Money".to_string()));
        assert_eq!(c["Shape"].kind, "Interface");
        assert!(l.contains(&("Line".to_string(), "Order".to_string(), "Composition".to_string(), "contains".to_string())), "{l:?}");
        assert!(l.contains(&("Duck".to_string(), "Animal".to_string(), "Generalization".to_string(), String::new())));
        assert_eq!(c["Duck"].operations[0].1, "swim()");
    }

    #[test]
    fn ddl_types() {
        assert_eq!(column_type("oracle", "varchar", "40"), "VARCHAR2(40)");
        assert_eq!(column_type("sqlserver", "text", ""), "NVARCHAR(MAX)");
        assert_eq!(column_type("postgres", "datetime", ""), "TIMESTAMP");
        assert_eq!(quote_name("sqlserver", "Order Line"), "[Order Line]");
    }

    /// `LOONAR_TEST_PROPS=… cargo test mermaid_ddl_live -- --ignored --nocapture`: a Mermaid diagram imported into EA; tables made
    /// with columns and a primary key, their DDL in two dialects.
    #[test]
    #[ignore = "requires EA and a copy of a test model"]
    fn mermaid_ddl_live() {
        let cfg = AppConfig::load(std::path::Path::new(&std::env::var("LOONAR_TEST_PROPS").unwrap())).unwrap();
        let root: i64 = query_rows(&cfg, "select package_id from t_package where parent_id = 0 order by package_id").unwrap()[0][0].parse().unwrap();
        let name = format!("ZZ_mmd_{}", chrono::Local::now().format("%H%M%S"));
        let (ops, text) = import_mermaid(&json!({ "text": "classDiagram\nclass Order {\n +String id\n +total() Money\n}\nOrder *-- Line : contains\nAnimal <|-- Duck", "package": root, "name": name, "action": "apply" }), &cfg).unwrap();
        println!("{text}");
        let done = crate::ea::run_batch(&cfg, &ops, &crate::ea::BatchOptions { rollback_key: Some("pkg".into()), keep_locks: false }).expect("import");
        let pkg = done["created"].as_array().unwrap().iter().find(|c| c["key"] == "pkg").unwrap()["id"].as_i64().unwrap();
        let classes = query_rows(&cfg, &format!("select name from t_object where package_id = {pkg} order by name")).unwrap();
        let links = query_rows(&cfg, &format!("select c.connector_type, coalesce(c.subtype,'') from t_connector c join t_object o on o.object_id = c.start_object_id where o.package_id = {pkg} order by 1")).unwrap();
        // Tables: two columns, the first in a primary key (EA's PK operation).
        crate::ea::run_batch(&cfg, &[
            json!({"op":"create_element","key":"t","package":pkg,"name":"customer","type":"Table"}),
            json!({"op":"create_attribute","element":"#t","name":"id","type":"integer"}),
            json!({"op":"create_attribute","element":"#t","name":"full name","type":"varchar"}),
            json!({"op":"create_operation","element":"#t","name":"PK_customer","stereotype":"PK","parameters":[{"name":"id","type":"integer"}]}),
        ], &crate::ea::BatchOptions::default()).unwrap();
        let pg = table_ddl(&json!({ "package": pkg, "dialect": "postgres" }), &cfg).unwrap();
        let ms = table_ddl(&json!({ "package": pkg, "dialect": "sqlserver" }), &cfg).unwrap();
        println!("{pg}\n{ms}");
        crate::ea::run_batch(&cfg, &[json!({"op":"delete","package":pkg})], &crate::ea::BatchOptions::default()).unwrap();
        crate::ea::shutdown();
        assert_eq!(classes.iter().map(|r| r[0].as_str()).collect::<Vec<_>>(), ["Animal", "Duck", "Line", "Order"]);
        assert!(links.iter().any(|l| l[0] == "Aggregation" && l[1] == "Strong") && links.iter().any(|l| l[0] == "Generalization"), "{links:?}");
        assert!(pg.contains("id INTEGER NOT NULL") && pg.contains("PRIMARY KEY (id)") && ms.contains("[full name] NVARCHAR(255)"), "{pg}\n{ms}");
    }
}

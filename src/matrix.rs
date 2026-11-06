//! Package matrix (LC-334): which elements of one package are related to which elements of another, like EA's
//! Relationship Matrix, built from the model database. The assistant gets a summary; the user gets the matrix as an
//! image in the chat and as an Excel workbook (the matrix, the list of relationships, elements without any).

use crate::config::AppConfig;
use crate::tools::query_rows;
use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

/// The largest matrix drawn as an image (rows × columns); a bigger one is only in the workbook.
const MAX_DRAWN: usize = 60;

pub fn definition() -> Value {
    json!({
        "name": "package_matrix",
        "description": "Builds a relationship matrix between two packages (like EA's Relationship Matrix): the elements of the source package \
in rows, of the target package in columns, a mark where they are related. Use it when the user asks for a matrix, a comparison or coverage of \
two packages (e.g. requirements × use cases, components × interfaces, applications × capabilities). The user sees the matrix as an image in the \
chat and gets an Excel workbook (matrix, list of relationships, elements without any); you get a summary. If the user did not say which \
relationship types, use all; ask only when the packages are unclear. Ids from t_package.package_id (or exact package names).",
        "input_schema": {
            "type": "object",
            "properties": {
                "source": { "description": "Source package: package_id or exact name." },
                "target": { "description": "Target package: package_id or exact name." },
                "types": { "type": "array", "items": { "type": "string" }, "description": "Relationship types or stereotypes, e.g. Realisation, Dependency, Association, Trace (= Abstraction «trace»), Usage. Empty: all." },
                "element_types": { "type": "array", "items": { "type": "string" }, "description": "Only elements of these types (e.g. Requirement, UseCase, Component). Empty: all except notes, texts and boundaries." },
                "subpackages": { "type": "boolean", "description": "Include child packages (default true)." },
                "direction": { "type": "string", "enum": ["both", "source_to_target", "target_to_source"], "description": "Which ends count (default both)." },
                "file_name": { "type": "string", "description": "Name of the workbook without extension (optional)." }
            },
            "required": ["source", "target"]
        }
    })
}

fn package_id(cfg: &AppConfig, value: &Value) -> Result<(i64, String)> {
    let rows = if let Some(id) = value.as_i64().or_else(|| value.as_str().and_then(|s| s.trim().parse().ok())) {
        query_rows(cfg, &format!("select package_id, name from t_package where package_id = {id}"))?
    } else {
        let name = crate::model_db::sql_text(cfg.model_source, value.as_str().unwrap_or("").trim());
        query_rows(cfg, &format!("select package_id, name from t_package where name = '{name}' order by package_id"))?
    };
    match rows.as_slice() {
        [] => bail!("{}", crate::i18n::fmt("there is no package {0}", &[value.to_string()])),
        [one] => Ok((one[0].parse()?, one[1].clone())),
        many => bail!("several packages are named {}: ids {} - pass the package_id", value, many.iter().map(|r| r[0].as_str()).collect::<Vec<_>>().join(", ")),
    }
}

/// The package and, when asked, all packages below it.
fn packages(cfg: &AppConfig, root: i64, deep: bool) -> Result<Vec<i64>> {
    let mut all = vec![root];
    let mut level = vec![root];
    while deep && !level.is_empty() {
        let list = level.iter().map(i64::to_string).collect::<Vec<_>>().join(",");
        level = query_rows(cfg, &format!("select package_id from t_package where parent_id in ({list})"))?.iter().filter_map(|r| r[0].parse().ok()).collect();
        level.retain(|p| !all.contains(p));
        all.extend(&level);
    }
    Ok(all)
}

struct Element {
    id: i64,
    name: String,
    kind: String,
}

fn elements(cfg: &AppConfig, packages: &[i64], kinds: &[String]) -> Result<Vec<Element>> {
    let list = packages.iter().map(i64::to_string).collect::<Vec<_>>().join(",");
    let filter = if kinds.is_empty() {
        "and o.object_type not in ('Note', 'Text', 'Boundary', 'Package', 'ProvidedInterface', 'RequiredInterface', 'Port')".to_string()
    } else {
        format!("and o.object_type in ({})", kinds.iter().map(|k| format!("'{}'", crate::model_db::sql_text(cfg.model_source, k))).collect::<Vec<_>>().join(","))
    };
    Ok(query_rows(cfg, &format!("select o.object_id, o.name, o.object_type from t_object o where o.package_id in ({list}) {filter} order by o.name"))?
        .into_iter()
        .filter_map(|r| Some(Element { id: r[0].parse().ok()?, name: r[1].clone(), kind: r[2].clone() }))
        .collect())
}

/// The relationship type shown in a cell: the stereotype when it names the relationship (trace, refine…), else the type.
fn label(kind: &str, stereotype: &str) -> String {
    if stereotype.is_empty() { kind.to_string() } else { stereotype.to_string() }
}

fn wanted(types: &[String], kind: &str, stereotype: &str) -> bool {
    types.is_empty()
        || types.iter().any(|t| {
            let t = t.trim().to_lowercase();
            t == kind.to_lowercase() || t == stereotype.to_lowercase() || (t == "realization" && kind == "Realisation") || (t == "trace" && stereotype.eq_ignore_ascii_case("trace"))
        })
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn short(s: &str, n: usize) -> String {
    if s.chars().count() <= n { s.to_string() } else { format!("{}…", s.chars().take(n - 1).collect::<String>()) }
}

/// The matrix as an SVG: row and column names, a filled cell with a mark where elements are related.
fn svg(title: &str, rows: &[&Element], cols: &[&Element], cells: &BTreeMap<(i64, i64), BTreeSet<String>>) -> String {
    let (cell, left, top) = (22.0, 240.0, 170.0);
    // Room on the right for the slanted names of the last columns.
    let (w, h) = (left + cols.len() as f64 * cell + 110.0, top + rows.len() as f64 * cell + 40.0);
    let mut out = format!(r##"<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" font-family="Segoe UI" font-size="12"><rect width="{w}" height="{h}" fill="#ffffff"/><text x="10" y="22" font-size="15" font-weight="bold" fill="#1F3864">{}</text>"##, esc(title));
    for (j, c) in cols.iter().enumerate() {
        let x = left + j as f64 * cell + cell / 2.0 + 4.0;
        out.push_str(&format!(r##"<text transform="translate({x},{}) rotate(-60)" fill="#333">{}</text>"##, top - 6.0, esc(&short(&c.name, 26))));
    }
    for (i, r) in rows.iter().enumerate() {
        let y = top + i as f64 * cell;
        if i % 2 == 1 {
            out.push_str(&format!(r##"<rect x="0" y="{y}" width="{w}" height="{cell}" fill="#F7F9FC"/>"##));
        }
        out.push_str(&format!(r##"<text x="{}" y="{}" text-anchor="end" fill="#333">{}</text>"##, left - 8.0, y + 15.0, esc(&short(&r.name, 34))));
        for (j, c) in cols.iter().enumerate() {
            let x = left + j as f64 * cell;
            out.push_str(&format!(r##"<rect x="{x}" y="{y}" width="{cell}" height="{cell}" fill="none" stroke="#D9D9D9"/>"##));
            if cells.contains_key(&(r.id, c.id)) {
                out.push_str(&format!(r##"<rect x="{}" y="{}" width="{}" height="{}" rx="3" fill="#2E74B5"/>"##, x + 3.0, y + 3.0, cell - 6.0, cell - 6.0));
            }
        }
    }
    out.push_str("</svg>");
    out
}

pub fn tool(input: &Value, cfg: &AppConfig) -> Result<crate::tools::ToolOutput> {
    let (source, source_name) = package_id(cfg, &input["source"])?;
    let (target, target_name) = package_id(cfg, &input["target"])?;
    let deep = input["subpackages"].as_bool().unwrap_or(true);
    let strings = |v: &Value| v.as_array().into_iter().flatten().filter_map(|t| t.as_str()).map(str::to_string).filter(|t| !t.trim().is_empty()).collect::<Vec<_>>();
    let (types, kinds) = (strings(&input["types"]), strings(&input["element_types"]));
    let direction = input["direction"].as_str().unwrap_or("both");
    // When one package lies inside the other, the inner one belongs only to its own side of the matrix. The same package
    // on both sides is a matrix of its elements with each other (LC-444: it was emptied on one side).
    let mut source_packages = packages(cfg, source, deep)?;
    let mut target_packages = packages(cfg, target, deep)?;
    if source != target && source_packages.contains(&target) {
        source_packages.retain(|p| !target_packages.contains(p));
    } else if source != target && target_packages.contains(&source) {
        target_packages.retain(|p| !source_packages.contains(p));
    }
    let rows = elements(cfg, &source_packages, &kinds)?;
    let cols = elements(cfg, &target_packages, &kinds)?;
    if rows.is_empty() || cols.is_empty() {
        bail!("no elements: source package \"{source_name}\" has {}, target package \"{target_name}\" has {} (element types: {})", rows.len(), cols.len(), if kinds.is_empty() { "all".into() } else { kinds.join(", ") });
    }
    let row_ids: BTreeSet<i64> = rows.iter().map(|e| e.id).collect();
    let col_ids: BTreeSet<i64> = cols.iter().map(|e| e.id).collect();
    let list = |ids: &BTreeSet<i64>| ids.iter().map(i64::to_string).collect::<Vec<_>>().join(",");
    let links = query_rows(cfg, &format!(
        "select c.start_object_id, c.end_object_id, c.connector_type, coalesce(c.stereotype,''), coalesce(c.name,'') from t_connector c \
         where (c.start_object_id in ({rs}) and c.end_object_id in ({cs})) or (c.start_object_id in ({cs}) and c.end_object_id in ({rs}))",
        rs = list(&row_ids), cs = list(&col_ids)
    ))?;
    // (row, column) → relationship labels, with an arrow showing which way the relationship goes.
    let mut cells: BTreeMap<(i64, i64), BTreeSet<String>> = BTreeMap::new();
    let mut listed = Vec::new();
    for l in &links {
        let (start, end): (i64, i64) = (l[0].parse()?, l[1].parse()?);
        if !wanted(&types, &l[2], &l[3]) {
            continue;
        }
        let forward = row_ids.contains(&start) && col_ids.contains(&end);
        let backward = col_ids.contains(&start) && row_ids.contains(&end);
        let (row, col, arrow) = match (forward, backward, direction) {
            (true, _, "both" | "source_to_target") => (start, end, "→"),
            (_, true, "both" | "target_to_source") => (end, start, "←"),
            _ => continue,
        };
        let text = label(&l[2], &l[3]);
        cells.entry((row, col)).or_default().insert(format!("{arrow} {text}"));
        listed.push((row, text, col, arrow, l[4].clone()));
    }
    let name_of = |id: i64| rows.iter().chain(cols.iter()).find(|e| e.id == id).map(|e| e.name.clone()).unwrap_or_default();
    let lonely_rows: Vec<&Element> = rows.iter().filter(|r| !cells.keys().any(|(a, _)| *a == r.id)).collect();
    let lonely_cols: Vec<&Element> = cols.iter().filter(|c| !cells.keys().any(|(_, b)| *b == c.id)).collect();

    // The workbook: the matrix, the relationships, the elements without any.
    let mut header = vec![format!("{source_name} \\ {target_name}")];
    header.extend(cols.iter().map(|c| c.name.clone()));
    let matrix_rows: Vec<Vec<Value>> = rows
        .iter()
        .map(|r| {
            let mut row = vec![json!(format!("{} ({})", r.name, r.kind))];
            row.extend(cols.iter().map(|c| json!(cells.get(&(r.id, c.id)).map(|s| s.iter().cloned().collect::<Vec<_>>().join(", ")).unwrap_or_default())));
            row
        })
        .collect();
    let relation_rows: Vec<Vec<Value>> = listed.iter().map(|(r, t, c, a, n)| vec![json!(name_of(*r)), json!(if *a == "→" { "→" } else { "←" }), json!(t), json!(name_of(*c)), json!(n)]).collect();
    let mut gap_rows: Vec<Vec<Value>> = lonely_rows.iter().map(|e| vec![json!(source_name), json!(e.name), json!(e.kind)]).collect();
    gap_rows.extend(lonely_cols.iter().map(|e| vec![json!(target_name), json!(e.name), json!(e.kind)]));
    let sheets = vec![
        crate::xlsx::Sheet { name: crate::i18n::tr("Matrix").into(), columns: header, rows: matrix_rows },
        crate::xlsx::Sheet {
            name: crate::i18n::tr("Relationships").into(),
            columns: vec![source_name.clone(), crate::i18n::tr("Direction").into(), crate::i18n::tr("Type").into(), target_name.clone(), crate::i18n::tr("Name").into()],
            rows: relation_rows,
        },
        crate::xlsx::Sheet { name: crate::i18n::tr("Without relationships").into(), columns: vec![crate::i18n::tr("Package").into(), crate::i18n::tr("Element").into(), crate::i18n::tr("Type").into()], rows: gap_rows },
    ];
    let bytes = crate::xlsx::build(&sheets)?;
    std::fs::create_dir_all(&cfg.attachments_dir).context(crate::i18n::tr("attachments folder"))?;
    let base = input["file_name"].as_str().map(str::to_string).unwrap_or_else(|| format!("{source_name} x {target_name}"));
    let stored = format!("{}_{}.xlsx", uuid::Uuid::new_v4().simple(), crate::history::sanitize_name(&base));
    std::fs::write(cfg.attachments_dir.join(&stored), &bytes).context(crate::i18n::tr("saving the workbook"))?;
    let mut images = Vec::new();
    if rows.len() <= MAX_DRAWN && cols.len() <= MAX_DRAWN {
        let drawn_rows: Vec<&Element> = rows.iter().collect();
        let drawn_cols: Vec<&Element> = cols.iter().collect();
        let png = crate::render::svg_to_png(&svg(&format!("{source_name} × {target_name}"), &drawn_rows, &drawn_cols, &cells), 1.5)?;
        let image = format!("{}_{}.png", uuid::Uuid::new_v4().simple(), crate::history::sanitize_name(&base));
        std::fs::write(cfg.attachments_dir.join(&image), &png).context(crate::i18n::tr("saving the image"))?;
        images.push(image);
    }
    let names = |list: &[&Element]| list.iter().take(25).map(|e| e.name.as_str()).collect::<Vec<_>>().join(", ");
    // The pairs themselves as text (round 13: from the count and the picture alone the answer read the columns one off).
    let pairs = listed.iter().take(60).map(|(r, t, c, a, _)| format!("{} {a} {} ({t})", name_of(*r), name_of(*c))).collect::<Vec<_>>().join("; ");
    let text = format!(
        "Matrix \"{source_name}\" ({} elements) × \"{target_name}\" ({} elements), relationship types: {}, direction: {direction}. \
Related pairs: {} – {pairs}{}. Source elements without a relationship ({}): {}. Target elements without a relationship ({}): {}. \
The user sees {} and the workbook \"{base}.xlsx\" (sheets: matrix, relationships, without relationships) in the chat. \
If the user wants this matrix in EA itself, save it with ea_write op create_matrix_specification (source_package {source}, target_package {target}, \
the element types of the rows and columns, the link type): EA then shows it live when the user double-clicks the artifact.",
        rows.len(),
        cols.len(),
        if types.is_empty() { "all".to_string() } else { types.join(", ") },
        cells.len(),
        if listed.len() > 60 { format!(" … and {} more (in the workbook)", listed.len() - 60) } else { String::new() },
        lonely_rows.len(),
        names(&lonely_rows),
        lonely_cols.len(),
        names(&lonely_cols),
        if images.is_empty() { "no image (the matrix is too big to draw, it is in the workbook)" } else { "the matrix as an image" },
    );
    Ok(crate::tools::ToolOutput { text, images, files: vec![stored] })
}

#[cfg(test)]
mod tests {
    #[test]
    fn types_match_by_kind_or_stereotype() {
        let t = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert!(super::wanted(&[], "Association", ""));
        assert!(super::wanted(&t(&["Trace"]), "Abstraction", "trace"));
        assert!(super::wanted(&t(&["realization"]), "Realisation", ""));
        assert!(!super::wanted(&t(&["Dependency"]), "Association", ""));
    }

    /// The matrix of two packages of the test model and EA's own Matrix Specification of the same packages.
    /// Run with: `cargo test matrix_live -- --ignored --nocapture` (LOONAR_TEST_PROPS = a copy of the model).
    #[test]
    #[ignore = "requires EA, a license and the model from LOONAR_TEST_PROPS"]
    fn matrix_live() {
        let cfg = crate::config::AppConfig::load(std::path::Path::new(&std::env::var("LOONAR_TEST_PROPS").expect("LOONAR_TEST_PROPS"))).expect("app.properties");
        let out = super::tool(&serde_json::json!({ "source": 385, "target": 387, "types": ["Realisation"] }), &cfg).expect("matrix");
        println!("{}\nfiles: {:?} images: {:?}", out.text, out.files, out.images);
        assert_eq!(out.files.len(), 1);
        assert_eq!(out.images.len(), 1);
        let ops = [serde_json::json!({"op":"create_matrix_specification","key":"m","package":385,"name":"Stakeholders × Attributes","source_package":385,"target_package":387,"source_type":"Stakeholder","target_type":"Class","link_type":"Realisation"})];
        let created = crate::ea::run_batch(&cfg, &ops, &crate::ea::BatchOptions::default()).expect("matrix specification");
        let id = created["created"][0]["id"].as_i64().unwrap();
        let rows = crate::tools::query_rows(&cfg, &format!("select o.object_type, o.stereotype, p.property, p.notes from t_object o join t_objectproperties p on p.object_id = o.object_id where o.object_id = {id}")).unwrap();
        println!("{rows:?}");
        crate::ea::shutdown();
        assert_eq!(rows.len(), 1, "one MatrixOptions tag");
        assert_eq!(rows[0][0], "Artifact");
        assert!(rows[0][1].contains("EAMatrixSpecification") && rows[0][2] == "MatrixOptions" && rows[0][3].contains("linktype=\"Realisation\""));
    }
}

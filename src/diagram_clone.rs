//! Cloning a diagram the way the user means it (LC-388): EA's shallow and deep copy are easy to confuse. view = a new
//! diagram of the same elements (nothing copied); variant = copies of all its elements and of the relationships between
//! them in a new package, on a new diagram in the same places (AS-IS → TO-BE); selection = copies of the chosen
//! elements only, the rest of the diagram shared. The plan says how many objects are new and how many shared.

use crate::config::AppConfig;
use crate::tools::query_rows;
use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use std::collections::HashSet;

pub fn definition() -> Value {
    json!({
        "name": "clone_diagram",
        "description": "Clone a diagram in one of three clear ways (EA's shallow/deep copy is easy to confuse): mode view – a new diagram showing the \
same elements (nothing copied); mode variant – a new package with copies of all its elements and the relationships between them, on a new diagram in \
the same places (AS-IS → TO-BE); mode selection – copies of the chosen elements (and the relationships between them), the other elements of the \
diagram shared. action preview: how many objects are new and how many shared (reads only); action apply: one save the user approves (undoable).",
        "input_schema": { "type": "object", "properties": {
            "action": { "type": "string", "enum": ["preview", "apply"] },
            "diagram": { "type": "integer" },
            "mode": { "type": "string", "enum": ["view", "variant", "selection"] },
            "elements": { "type": "array", "items": { "type": "integer" }, "description": "For selection: the object_ids to copy" },
            "name": { "type": "string", "description": "Name of the new diagram (and package for a variant)" }
        }, "required": ["action", "diagram", "mode"] }
    })
}

/// (operations, description).
pub fn plan(input: &Value, cfg: &AppConfig) -> Result<(Vec<Value>, String)> {
    let diagram = input["diagram"].as_i64().context("diagram")?;
    let mode = input["mode"].as_str().unwrap_or("view");
    let d = query_rows(cfg, &format!("select name, package_id, diagram_type from t_diagram where diagram_id = {diagram}"))?;
    let d = d.first().with_context(|| format!("there is no diagram with id {diagram}"))?;
    let name = input["name"].as_str().map(str::to_string).unwrap_or_else(|| format!("{} – {}", d[0], if mode == "view" { "view" } else { "variant" }));
    let objects = query_rows(cfg, &format!(
        "select x.object_id, x.rectleft, x.recttop, x.rectright, x.rectbottom, o.object_type, o.name, coalesce(o.stereotype,''), coalesce(o.note,'') \
         from t_diagramobjects x join t_object o on o.object_id = x.object_id where x.diagram_id = {diagram} order by x.sequence desc"
    ))?;
    let copy: HashSet<i64> = match mode {
        "view" => HashSet::new(),
        "variant" => objects.iter().filter(|r| !matches!(r[5].as_str(), "Note" | "Text" | "Boundary")).filter_map(|r| r[0].parse().ok()).collect(),
        "selection" => {
            let chosen: HashSet<i64> = input["elements"].as_array().into_iter().flatten().filter_map(Value::as_i64).collect();
            if chosen.is_empty() {
                bail!("give the elements to copy");
            }
            chosen
        }
        other => bail!("unknown mode {other}"),
    };
    let mut ops = Vec::new();
    let package = if mode == "view" {
        json!(d[1].parse::<i64>().unwrap_or(0))
    } else {
        ops.push(json!({ "op": "create_package", "key": "pkg", "parent": d[1].parse::<i64>().unwrap_or(0), "name": name }));
        json!("#pkg")
    };
    ops.push(json!({ "op": "create_diagram", "key": "dia", "package": package, "name": name, "type": d[2] }));
    for r in &objects {
        let id: i64 = r[0].parse().unwrap_or(0);
        if copy.contains(&id) {
            let mut e = json!({ "op": "create_element", "key": format!("e{id}"), "package": package, "name": r[6], "type": r[5], "notes": r[8] });
            if !r[7].is_empty() {
                e["stereotype"] = json!(r[7]);
            }
            ops.push(e);
        }
    }
    for r in &objects {
        let id: i64 = r[0].parse().unwrap_or(0);
        let n = |i: usize| r[i].parse::<i64>().unwrap_or(0);
        let element = if copy.contains(&id) { json!(format!("#e{id}")) } else { json!(id) };
        ops.push(json!({ "op": "place", "diagram": "#dia", "element": element, "left": n(1), "top": -n(2), "width": n(3) - n(1), "height": n(2) - n(4) }));
    }
    // Relationships of the copies: between two copies, or between a copy and a shared element of the diagram.
    let on: Vec<String> = objects.iter().map(|r| r[0].clone()).collect();
    let mut relationships = 0;
    if !copy.is_empty() && !on.is_empty() {
        for c in query_rows(cfg, &format!(
            "select start_object_id, end_object_id, connector_type, coalesce(stereotype,''), coalesce(name,'') from t_connector \
             where start_object_id in ({0}) and end_object_id in ({0}) and connector_type not in ('Sequence', 'Collaboration', 'NoteLink')",
            on.join(",")
        ))? {
            let (a, b): (i64, i64) = (c[0].parse().unwrap_or(0), c[1].parse().unwrap_or(0));
            if !copy.contains(&a) && !copy.contains(&b) {
                continue;
            }
            let end = |x: i64| if copy.contains(&x) { json!(format!("#e{x}")) } else { json!(x) };
            let mut op = json!({ "op": "create_connector", "source": end(a), "target": end(b), "type": c[2], "name": c[4] });
            if !c[3].is_empty() {
                op["stereotype"] = json!(c[3]);
            }
            ops.push(op);
            relationships += 1;
        }
    }
    let text = format!(
        "{}: a new diagram \"{name}\"{} with {} elements – {} new copies, {} shared with \"{}\"; {} relationships copied.",
        match mode { "view" => "View", "variant" => "Variant", _ => "Selection" },
        if mode == "view" { String::new() } else { format!(" in a new package \"{name}\"") },
        objects.len(),
        copy.len(),
        objects.len() - copy.len().min(objects.len()),
        d[0],
        relationships
    );
    Ok((ops, text))
}

#[cfg(test)]
mod tests {
    /// LC-388: a variant through EA on a model COPY (removed again):
    /// `LOONAR_TEST_PROPS=E:/livetest/undo/undo.properties cargo test clone_diagram_live -- --ignored --nocapture`
    #[test]
    #[ignore = "requires EA and a copy of a test model"]
    fn clone_diagram_live() {
        let cfg = crate::config::AppConfig::load(std::path::Path::new(&std::env::var("LOONAR_TEST_PROPS").expect("LOONAR_TEST_PROPS"))).expect("app.properties");
        let q = |sql: &str| crate::tools::query_rows(&cfg, sql).unwrap();
        // A small logical diagram with relationships between its elements.
        let diagram: i64 = q("select d.diagram_id from t_diagram d join t_diagramobjects x on x.diagram_id = d.diagram_id where d.diagram_type = 'Logical' \
                              group by d.diagram_id having count(*) between 3 and 8 order by d.diagram_id")[0][0].parse().unwrap();
        let (ops, text) = super::plan(&serde_json::json!({ "diagram": diagram, "mode": "variant" }), &cfg).unwrap();
        println!("{text}");
        let done = crate::ea::run_batch(&cfg, &ops, &crate::ea::BatchOptions { rollback_key: Some("pkg".into()), keep_locks: false }).expect("variant");
        let created = done["created"].as_array().unwrap();
        let pkg = created.iter().find(|c| c["key"] == "pkg").unwrap()["id"].as_i64().unwrap();
        let dia = created.iter().find(|c| c["key"] == "dia").unwrap()["id"].as_i64().unwrap();
        let (original, copied) = (q(&format!("select count(*) from t_diagramobjects where diagram_id = {diagram}"))[0][0].clone(), q(&format!("select count(*) from t_diagramobjects where diagram_id = {dia}"))[0][0].clone());
        let in_pkg = q(&format!("select count(*) from t_object where package_id = {pkg}"))[0][0].clone();
        crate::ea::run_batch(&cfg, &[serde_json::json!({"op":"delete","package":pkg})], &crate::ea::BatchOptions::default()).expect("cleanup");
        crate::ea::shutdown();
        println!("{original} on the original, {copied} on the variant, {in_pkg} elements in the new package");
        assert_eq!(original, copied);
        assert!(in_pkg.parse::<i64>().unwrap() > 0);
    }
}

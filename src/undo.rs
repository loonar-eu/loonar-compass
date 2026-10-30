//! Time machine: every save of the assistant gets a journal entry with the operations that reverse it (created objects
//! are deleted, changed fields, tagged values, positions and moves get their previous values). Before a save that
//! deletes, a baseline of the affected package is made in EA. `undo_change` lists the journal and undoes the latest saves.

use crate::config::AppConfig;
use crate::tools::query_rows;
use anyhow::Result;
use serde_json::{json, Map, Value};
use std::collections::HashMap;
use std::io::Write;
use std::path::PathBuf;

pub fn journal_file(cfg: &AppConfig) -> PathBuf {
    cfg.base_dir.join("undo_journal.jsonl")
}

/// Which model a journal entry belongs to.
pub fn model_key(cfg: &AppConfig) -> String {
    if cfg.uses_file() { cfg.model_file.display().to_string().to_lowercase() } else { format!("{}@{}:{}", cfg.db_name, cfg.db_host, cfg.db_port) }
}

fn one(cfg: &AppConfig, sql: String) -> Option<Vec<String>> {
    query_rows(cfg, &sql).ok().and_then(|r| r.into_iter().next())
}

/// Reverse operations for the changes of existing objects, read BEFORE the save. Ops creating objects get their
/// reverse after the save (from the created ids).
/// Every reverse operation carries "_at", the index of the operation it reverses (a failed save journals only the
/// operations before the failed one, LC-348); `record` takes the field off.
pub fn before(cfg: &AppConfig, ops: &[Value]) -> (Vec<Value>, Vec<String>) {
    let mut reverse = Vec::new();
    let mut not_undoable = Vec::new();
    for (at, op) in ops.iter().enumerate() {
        let start = reverse.len();
        before_one(cfg, op, &mut reverse, &mut not_undoable);
        for r in &mut reverse[start..] {
            r["_at"] = json!(at);
        }
    }
    (reverse, not_undoable)
}

/// Operations whose objects are journalled from the save's result (deleted on undo) or that do not change the model.
const UNDONE_FROM_RESULT: &[&str] = &[
    "create_package", "create_diagram", "create_element", "create_connector", "create_attribute", "create_operation", "clone_package",
    "clone_element_version", "create_matrix_specification", "export_diagram", "export_xmi", "generate_report", "create_baseline", "ping",
];

fn before_one(cfg: &AppConfig, op: &Value, reverse: &mut Vec<Value>, not_undoable: &mut Vec<String>) {
    {
        let kind = op["op"].as_str().unwrap_or("");
        let id = |field: &str| op[field].as_i64();
        match kind {
            "update_element" => {
                let Some(e) = id("element") else { return };
                let Some(r) = one(cfg, format!(
                    "select coalesce(name,'') as c0, coalesce(note,'') as c1, coalesce(alias,'') as c2, coalesce(stereotype,'') as c3, coalesce(status,'') as c4, \
                     coalesce(version,'') as c5, coalesce(author,'') as c6, coalesce(phase,'') as c7 from t_object where object_id = {e}"
                )) else { return };
                let mut back = Map::new();
                back.insert("op".into(), json!("update_element"));
                back.insert("element".into(), json!(e));
                for (field, index) in [("name", 0), ("notes", 1), ("alias", 2), ("status", 4), ("version", 5), ("author", 6), ("phase", 7)] {
                    if !op[field].is_null() {
                        back.insert(field.into(), json!(r[index]));
                    }
                }
                if !op["stereotype"].is_null() || op["clear_stereotype"] == true {
                    if r[3].is_empty() {
                        back.insert("clear_stereotype".into(), json!(true));
                    } else {
                        back.insert("stereotype".into(), json!(r[3]));
                    }
                }
                if let Some(tags) = op["tags"].as_object() {
                    let now: HashMap<String, String> = query_rows(cfg, &format!("select property as c0, coalesce(value,'') as c1 from t_objectproperties where object_id = {e}"))
                        .unwrap_or_default()
                        .into_iter()
                        .map(|r| (r[0].clone(), r[1].clone()))
                        .collect();
                    let previous: Map<String, Value> = tags.keys().map(|k| (k.clone(), now.get(k).map(|v| json!(v)).unwrap_or(Value::Null))).collect();
                    back.insert("tags".into(), Value::Object(previous));
                }
                if op.as_object().is_some_and(|o| o.keys().any(|k| ["color", "keywords", "complexity", "abstract"].contains(&k.as_str()))) {
                    not_undoable.push(format!("update_element {e}: color / keywords / complexity / abstract"));
                }
                reverse.push(Value::Object(back));
            }
            "update_connector" => {
                let Some(c) = id("connector") else { return };
                let Some(r) = one(cfg, format!("select coalesce(name,'') as c0, coalesce(stereotype,'') as c1, coalesce(notes,'') as c2 from t_connector where connector_id = {c}")) else { return };
                let mut back = Map::new();
                back.insert("op".into(), json!("update_connector"));
                back.insert("connector".into(), json!(c));
                if !op["name"].is_null() {
                    back.insert("name".into(), json!(r[0]));
                }
                if !op["notes"].is_null() {
                    back.insert("notes".into(), json!(r[2]));
                }
                if !op["stereotype"].is_null() || op["clear_stereotype"] == true {
                    if r[1].is_empty() { back.insert("clear_stereotype".into(), json!(true)); } else { back.insert("stereotype".into(), json!(r[1])); }
                }
                reverse.push(Value::Object(back));
            }
            "update_diagram" => {
                let Some(d) = id("diagram") else { return };
                let Some(r) = one(cfg, format!("select coalesce(name,'') as c0, coalesce(notes,'') as c1 from t_diagram where diagram_id = {d}")) else { return };
                let mut back = json!({ "op": "update_diagram", "diagram": d });
                if !op["name"].is_null() {
                    back["name"] = json!(r[0]);
                }
                if !op["notes"].is_null() {
                    back["notes"] = json!(r[1]);
                }
                reverse.push(back);
            }
            "set_bounds" => {
                let (Some(d), Some(e)) = (id("diagram"), id("element")) else { return };
                let Some(r) = one(cfg, format!("select rectleft as c0, recttop as c1, rectright as c2, rectbottom as c3 from t_diagramobjects where diagram_id = {d} and object_id = {e}")) else { return };
                let n = |i: usize| r[i].parse::<i64>().unwrap_or(0);
                reverse.push(json!({ "op": "set_bounds", "diagram": d, "element": e, "left": n(0), "top": -n(1), "width": n(2) - n(0), "height": n(1) - n(3) }));
            }
            "move" => {
                if let Some(e) = id("element") {
                    if let Some(r) = one(cfg, format!("select package_id as c0 from t_object where object_id = {e}")) {
                        reverse.push(json!({ "op": "move", "package": r[0].parse::<i64>().unwrap_or(0), "element": e }));
                    }
                } else if let Some(d) = id("diagram") {
                    if let Some(r) = one(cfg, format!("select package_id as c0 from t_diagram where diagram_id = {d}")) {
                        reverse.push(json!({ "op": "move", "package": r[0].parse::<i64>().unwrap_or(0), "diagram": d }));
                    }
                } else if let Some(p) = id("child_package") {
                    if let Some(r) = one(cfg, format!("select parent_id as c0 from t_package where package_id = {p}")) {
                        reverse.push(json!({ "op": "move", "package": r[0].parse::<i64>().unwrap_or(0), "child_package": p }));
                    }
                }
            }
            "place" => {
                // Placing an existing element: undone by removing it from the diagram (new elements are deleted anyway).
                if let (Some(d), Some(e)) = (id("diagram"), id("element")) {
                    reverse.push(json!({ "op": "remove_from_diagram", "diagram": d, "element": e }));
                }
            }
            "update_attribute" | "update_operation" => {
                // The previous values of the fields this save sets (LC-348: they were not reversed at all).
                let attribute = kind == "update_attribute";
                let Some(m) = id(if attribute { "attribute" } else { "operation" }) else { return };
                let sql = if attribute {
                    format!("select coalesce(name,'') as c0, coalesce(type,'') as c1, coalesce(scope,'') as c2, coalesce(notes,'') as c3, coalesce({},'') as c4, \
                             coalesce(stereotype,'') as c5, coalesce(style,'') as c6, coalesce(isstatic,0) as c7 from t_attribute where id = {m}", "\"default\"")
                } else {
                    format!("select coalesce(name,'') as c0, coalesce(type,'') as c1, coalesce(scope,'') as c2, coalesce(notes,'') as c3, '' as c4, \
                             coalesce(stereotype,'') as c5, coalesce(style,'') as c6, coalesce(isstatic,0) as c7 from t_operation where operationid = {m}")
                };
                let Some(r) = one(cfg, sql) else {
                    not_undoable.push(format!("{kind} {m}"));
                    return;
                };
                let mut back = Map::new();
                back.insert("op".into(), json!(kind));
                back.insert((if attribute { "attribute" } else { "operation" }).into(), json!(m));
                let type_field = if attribute { "type" } else { "return_type" };
                for (field, index) in [("name", 0), (type_field, 1), ("visibility", 2), ("notes", 3), ("default", 4), ("stereotype", 5), ("alias", 6)] {
                    if !op[field].is_null() && (attribute || field != "default") {
                        back.insert(field.into(), json!(r[index]));
                    }
                }
                if !op["static"].is_null() {
                    back.insert("static".into(), json!(r[7] == "1" || r[7].eq_ignore_ascii_case("true")));
                }
                reverse.push(Value::Object(back));
            }
            "update_package" => {
                let Some(p) = op["package"].as_i64() else {
                    not_undoable.push(kind.to_string());
                    return;
                };
                let Some(r) = one(cfg, format!(
                    "select coalesce(p.name,'') as c0, coalesce(p.notes,'') as c1, coalesce(p.version,'') as c2, coalesce(o.status,'') as c3, coalesce(o.stereotype,'') as c4 \
                     from t_package p left join t_object o on o.ea_guid = p.ea_guid and o.object_type = 'Package' where p.package_id = {p}"
                )) else {
                    not_undoable.push(format!("{kind} {p}"));
                    return;
                };
                let mut back = json!({ "op": "update_package", "package": p });
                for (field, index) in [("name", 0), ("notes", 1), ("version", 2), ("status", 3), ("stereotype", 4)] {
                    if !op[field].is_null() {
                        back[field] = json!(r[index]);
                    }
                }
                reverse.push(back);
            }
            // Reversing a connector again restores it.
            "reverse_connector" => {
                if let Some(c) = id("connector") {
                    reverse.push(json!({ "op": "reverse_connector", "connector": c }));
                }
            }
            k if UNDONE_FROM_RESULT.contains(&k) => {}
            // Everything else cannot be reversed by operations, and says so (LC-348: set_composite, set_linked_document,
            // scenarios, glossary terms, legends, navigation cells, templates, version control… were silent).
            _ => not_undoable.push(kind.to_string()),
        }
    }
}

/// Packages to baseline before a save that deletes: the packages of the deleted objects (or the deleted package's parent).
pub fn packages_to_baseline(cfg: &AppConfig, ops: &[Value]) -> Vec<i64> {
    let mut out = Vec::new();
    for op in ops.iter().filter(|o| matches!(o["op"].as_str(), Some("delete" | "delete_attribute" | "delete_operation" | "delete_scenario" | "import_xmi" | "layout_diagram"))) {
        let package = if let Some(e) = op["element"].as_i64() {
            one(cfg, format!("select package_id as c0 from t_object where object_id = {e}"))
        } else if let Some(d) = op["diagram"].as_i64() {
            one(cfg, format!("select package_id as c0 from t_diagram where diagram_id = {d}"))
        } else if let Some(p) = op["package"].as_i64() {
            if op["op"] == "import_xmi" { Some(vec![p.to_string()]) } else { one(cfg, format!("select parent_id as c0 from t_package where package_id = {p}")) }
        } else if let Some(c) = op["connector"].as_i64() {
            one(cfg, format!("select o.package_id as c0 from t_connector c join t_object o on o.object_id = c.start_object_id where c.connector_id = {c}"))
        } else {
            None
        };
        if let Some(p) = package.and_then(|r| r[0].parse::<i64>().ok()).filter(|p| *p > 0) {
            if !out.contains(&p) {
                out.push(p);
            }
        }
    }
    out
}

/// Records a successful save: the reverse of the created objects (newest first) plus the reverse read before.
pub fn record(cfg: &AppConfig, summary: &str, result: &Value, reverse_before: Vec<Value>, not_undoable: Vec<String>, baselines: Vec<String>) {
    let mut reverse: Vec<Value> = Vec::new();
    let created: Vec<&Value> = result["created"].as_array().into_iter().flatten().collect();
    // Attributes and operations first (LC-348: they were left behind), then connectors, elements, diagrams, packages
    // (an element takes its connectors with it).
    for kind in ["attribute", "operation", "connector", "element", "diagram", "package"] {
        for c in created.iter().rev().filter(|c| c["kind"] == kind) {
            if let Some(id) = c["id"].as_i64() {
                reverse.push(match kind {
                    "attribute" => json!({ "op": "delete_attribute", "attribute": id }),
                    "operation" => json!({ "op": "delete_operation", "operation": id }),
                    _ => json!({ "op": "delete", kind: id }),
                });
            }
        }
    }
    // Changes of existing objects are reversed in the opposite order of the save.
    reverse.extend(reverse_before.into_iter().rev().map(|mut r| {
        if let Some(o) = r.as_object_mut() {
            o.remove("_at");
        }
        r
    }));
    if reverse.is_empty() && not_undoable.is_empty() {
        return;
    }
    let entry = json!({
        "id": uuid::Uuid::new_v4().to_string(), "time": chrono::Local::now().to_rfc3339(), "model": model_key(cfg),
        "summary": summary, "reverse": reverse, "not_undoable": not_undoable, "baselines": baselines, "undone": false
    });
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(journal_file(cfg)) {
        let _ = writeln!(f, "{entry}");
    }
}

/// A save that failed at operation `failed_at`: the changes of existing objects made by the operations before it stay
/// in the model, so their reverse goes to the journal (the created objects were rolled back) (LC-348).
pub fn record_partial(cfg: &AppConfig, summary: &str, failed_at: usize, reverse_before: Vec<Value>, baselines: Vec<String>) {
    let kept: Vec<Value> = reverse_before.into_iter().filter(|r| r["_at"].as_u64().is_some_and(|at| (at as usize) < failed_at)).collect();
    if kept.is_empty() {
        return;
    }
    record(cfg, &format!("{summary} [{}]", crate::i18n::tr("failed part-way; the changes made before the error")), &json!({ "created": [] }), kept, Vec::new(), baselines);
}

pub fn entries(cfg: &AppConfig) -> Vec<Value> {
    let key = model_key(cfg);
    std::fs::read_to_string(journal_file(cfg))
        .unwrap_or_default()
        .lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .filter(|e| e["model"] == key.as_str())
        .collect()
}

/// Marks entries as undone (the journal file is rewritten).
pub fn mark_undone(cfg: &AppConfig, ids: &[String]) {
    let text = std::fs::read_to_string(journal_file(cfg)).unwrap_or_default();
    let lines: Vec<String> = text
        .lines()
        .map(|l| match serde_json::from_str::<Value>(l) {
            Ok(mut e) if e["id"].as_str().is_some_and(|i| ids.iter().any(|x| x == i)) => {
                e["undone"] = json!(true);
                e.to_string()
            }
            _ => l.to_string(),
        })
        .collect();
    // Written to a copy and renamed (LC-348): a reader never sees a half-written journal.
    let tmp = journal_file(cfg).with_extension("jsonl.tmp");
    if std::fs::write(&tmp, lines.join("\n") + "\n").is_ok() {
        let _ = std::fs::rename(&tmp, journal_file(cfg));
    }
}

/// The latest `count` saves not undone yet (newest first).
pub fn latest(cfg: &AppConfig, count: usize) -> Vec<Value> {
    let mut list: Vec<Value> = entries(cfg).into_iter().filter(|e| e["undone"] != true).collect();
    list.reverse();
    list.truncate(count);
    list
}

pub fn list_text(cfg: &AppConfig) -> Result<String> {
    let mut all = entries(cfg);
    all.reverse();
    let mut out = vec![format!("Undo journal of this model: {} saves.", all.len()), "# | time | undone | summary | reversible ops | not undoable | baselines".into()];
    for (i, e) in all.iter().take(30).enumerate() {
        out.push(format!(
            "{} | {} | {} | {} | {} | {} | {}",
            i + 1,
            e["time"].as_str().unwrap_or("").get(..19).unwrap_or("").replace('T', " "),
            if e["undone"] == true { "yes" } else { "no" },
            e["summary"].as_str().unwrap_or("").replace('\n', "; "),
            e["reverse"].as_array().map_or(0, Vec::len),
            e["not_undoable"].as_array().map(|a| a.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>().join(", ")).unwrap_or_default(),
            e["baselines"].as_array().map(|a| a.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>().join(", ")).unwrap_or_default(),
        ));
    }
    Ok(out.join("\n"))
}

pub fn definition() -> Value {
    json!({
        "name": "undo_change",
        "description": "Time machine of the assistant's saves to this model. action list: the journal (newest first) with what can be undone; undo: undoes the latest \
`count` saves not undone yet (default 1), newest first: created objects are deleted, changed fields, tagged values, positions and moves get their previous values. \
Attributes, operations and packages changed or created are reversed too. Deletions, imports, diagram layout and the operations listed as not undoable (e.g. scenarios, glossary terms, linked documents, legends) cannot be reversed by operations: before such a save Compass made an EA baseline of the package - tell the user to restore it in EA \
(package in the browser → Manage Baselines → Restore). The user approves the undo.",
        "input_schema": { "type": "object", "properties": {
            "action": { "type": "string", "enum": ["list", "undo"] },
            "count": { "type": "integer" }
        }, "required": ["action"] }
    })
}

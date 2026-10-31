//! Bulk changes of elements (LC-366, /bulk-set): the set from a filter (package, types, stereotype, name, status or a
//! list of ids), the change (status, phase, version, author, stereotype, tagged values, a text replaced in names),
//! first a numbered table "before → after", then the save of the rows the user keeps – one ea_write, so it is
//! approved once, journalled for undo, audited and verified like any other save. EA itself changes only status, phase
//! and version of many elements at once; everything else needed a script.

use crate::config::AppConfig;
use crate::tools::query_rows;
use anyhow::{bail, Result};
use serde_json::{json, Map, Value};
use std::collections::HashMap;

/// At most this many elements in one bulk change.
const MAX: usize = 500;

pub fn definition() -> Value {
    json!({
        "name": "bulk_set",
        "description": "Change many elements at once (EA changes only status, phase and version in bulk). action preview: the elements of the \
filter with a numbered table of each change before → after (nothing is saved; show the table to the user). action apply: save it, leaving \
out the rows the user does not want (skip: their numbers from the preview) – one save, approved once, undoable. Filter: package (with child \
packages), types, stereotype, name_contains, status, ids. Set: status, phase, version, author, stereotype (or clear_stereotype), tags \
{name: value, null removes}, rename {find, replace} (plain text in names). At most 500 elements. Use it for /bulk-set.",
        "input_schema": { "type": "object", "properties": {
            "action": { "type": "string", "enum": ["preview", "apply"] },
            "filter": { "type": "object", "properties": {
                "package": { "description": "package_id or exact name" },
                "types": { "type": "array", "items": { "type": "string" } },
                "stereotype": { "type": "string" },
                "name_contains": { "type": "string" },
                "status": { "type": "string" },
                "ids": { "type": "array", "items": { "type": "integer" } }
            } },
            "set": { "type": "object", "properties": {
                "status": { "type": "string" }, "phase": { "type": "string" }, "version": { "type": "string" }, "author": { "type": "string" },
                "stereotype": { "type": "string" }, "clear_stereotype": { "type": "boolean" },
                "tags": { "type": "object" },
                "rename": { "type": "object", "properties": { "find": { "type": "string" }, "replace": { "type": "string" } } }
            } },
            "skip": { "type": "array", "items": { "type": "integer" }, "description": "For apply: row numbers of the preview to leave out" }
        }, "required": ["action", "filter", "set"] }
    })
}

/// One element of the set with its current values.
struct Row {
    id: i64,
    name: String,
    kind: String,
    fields: HashMap<&'static str, String>,
    tags: HashMap<String, String>,
}

/// The elements of the filter, in object_id order (the same order for preview and apply).
fn select(cfg: &AppConfig, filter: &Value, tag_names: &[String]) -> Result<Vec<Row>> {
    let text = |v: &str| crate::model_db::sql_text(cfg.model_source, v);
    let mut conditions = vec!["o.object_type not in ('Package', 'Note', 'Text', 'Boundary')".to_string()];
    let (scope, _) = crate::model_usage::package_scope(cfg, &filter["package"])?;
    if !scope.is_empty() {
        conditions.push(scope.trim_start_matches("and ").to_string());
    }
    let types: Vec<String> = filter["types"].as_array().into_iter().flatten().filter_map(Value::as_str).map(|t| format!("'{}'", text(t))).collect();
    if !types.is_empty() {
        conditions.push(format!("o.object_type in ({})", types.join(",")));
    }
    if let Some(s) = filter["stereotype"].as_str().filter(|s| !s.trim().is_empty()) {
        conditions.push(format!("lower(coalesce(o.stereotype,'')) = lower('{}')", text(s.trim().trim_matches(['«', '»']))));
    }
    if let Some(n) = filter["name_contains"].as_str().filter(|s| !s.is_empty()) {
        conditions.push(format!("lower(o.name) like '%{}%'", text(&n.to_lowercase().replace(['%', '_'], ""))));
    }
    if let Some(s) = filter["status"].as_str().filter(|s| !s.is_empty()) {
        conditions.push(format!("coalesce(o.status,'') = '{}'", text(s)));
    }
    let ids: Vec<String> = filter["ids"].as_array().into_iter().flatten().filter_map(Value::as_i64).map(|i| i.to_string()).collect();
    if !ids.is_empty() {
        conditions.push(format!("o.object_id in ({})", ids.join(",")));
    }
    if conditions.len() == 1 {
        bail!("give a filter (package, types, stereotype, name_contains, status or ids): a bulk change of the whole model is not done");
    }
    let rows = query_rows(cfg, &format!(
        "select o.object_id, coalesce(o.name,''), o.object_type, coalesce(o.status,''), coalesce(o.phase,''), coalesce(o.version,''), coalesce(o.author,''), coalesce(o.stereotype,'') \
         from t_object o where {} order by o.object_id",
        conditions.join(" and ")
    ))?;
    if rows.len() > MAX {
        bail!("the filter gives {} elements; at most {MAX} at once – narrow it", rows.len());
    }
    let mut out: Vec<Row> = rows
        .iter()
        .map(|r| Row {
            id: r[0].parse().unwrap_or(0),
            name: r[1].clone(),
            kind: r[2].clone(),
            fields: HashMap::from([("status", r[3].clone()), ("phase", r[4].clone()), ("version", r[5].clone()), ("author", r[6].clone()), ("stereotype", r[7].clone())]),
            tags: HashMap::new(),
        })
        .collect();
    if !tag_names.is_empty() && !out.is_empty() {
        let list = out.iter().map(|r| r.id.to_string()).collect::<Vec<_>>().join(",");
        for t in query_rows(cfg, &format!("select object_id, property, coalesce(value,'') from t_objectproperties where object_id in ({list})"))? {
            if tag_names.iter().any(|n| n.eq_ignore_ascii_case(&t[1])) {
                if let Some(row) = out.iter_mut().find(|r| r.id.to_string() == t[0]) {
                    row.tags.insert(t[1].to_lowercase(), t[2].clone());
                }
            }
        }
    }
    Ok(out)
}

/// The changes of one element: (field shown, before, after) and the update_element operation, or None when nothing changes.
fn change(row: &Row, set: &Value) -> Option<(Vec<(String, String, String)>, Value)> {
    let mut shown = Vec::new();
    let mut op = Map::new();
    for field in ["status", "phase", "version", "author"] {
        if let Some(new) = set[field].as_str() {
            let old = &row.fields[field];
            if old != new {
                shown.push((field.to_string(), old.clone(), new.to_string()));
                op.insert(field.into(), json!(new));
            }
        }
    }
    let old_stereo = &row.fields["stereotype"];
    if set["clear_stereotype"] == true && !old_stereo.is_empty() {
        shown.push(("stereotype".into(), old_stereo.clone(), String::new()));
        op.insert("clear_stereotype".into(), json!(true));
    } else if let Some(new) = set["stereotype"].as_str().map(|s| s.trim().trim_matches(['«', '»'])).filter(|s| !s.is_empty()) {
        if !old_stereo.eq_ignore_ascii_case(new) {
            shown.push(("stereotype".into(), old_stereo.clone(), new.to_string()));
            op.insert("stereotype".into(), json!(new));
        }
    }
    if let (Some(find), Some(replace)) = (set["rename"]["find"].as_str().filter(|f| !f.is_empty()), set["rename"]["replace"].as_str()) {
        let new = row.name.replace(find, replace);
        if new != row.name {
            shown.push(("name".into(), row.name.clone(), new.clone()));
            op.insert("name".into(), json!(new));
        }
    }
    let mut tags = Map::new();
    for (name, value) in set["tags"].as_object().into_iter().flatten() {
        let old = row.tags.get(&name.to_lowercase()).cloned();
        let new = value.as_str().map(str::to_string).or_else(|| (!value.is_null()).then(|| value.to_string()));
        if old != new {
            shown.push((format!("tag {name}"), old.unwrap_or_else(|| "(none)".into()), new.clone().unwrap_or_else(|| "(removed)".into())));
            tags.insert(name.clone(), new.map(Value::String).unwrap_or(Value::Null));
        }
    }
    if !tags.is_empty() {
        op.insert("tags".into(), Value::Object(tags));
    }
    if shown.is_empty() {
        return None;
    }
    op.insert("op".into(), json!("update_element"));
    op.insert("element".into(), json!(row.id));
    Some((shown, Value::Object(op)))
}

/// The numbered rows of the change: (number, element, its changes, the operation).
fn plan(cfg: &AppConfig, input: &Value) -> Result<Vec<(usize, String, Vec<(String, String, String)>, Value)>> {
    let tag_names: Vec<String> = input["set"]["tags"].as_object().map(|t| t.keys().cloned().collect()).unwrap_or_default();
    let rows = select(cfg, &input["filter"], &tag_names)?;
    Ok(rows
        .iter()
        .filter_map(|row| change(row, &input["set"]).map(|(shown, op)| (format!("\"{}\" ({}, id {})", row.name, row.kind, row.id), shown, op)))
        .enumerate()
        .map(|(i, (label, shown, op))| (i + 1, label, shown, op))
        .collect())
}

/// Preview: the table; apply: the operations to save (rows in `skip` left out).
pub fn preview(input: &Value, cfg: &AppConfig) -> Result<String> {
    let rows = plan(cfg, input)?;
    if rows.is_empty() {
        return Ok("Nothing to change: the filter gives no element whose values differ from the new ones.".into());
    }
    let mut out = format!("Bulk change – {} elements would change (nothing saved yet):\n# | element | before → after\n", rows.len());
    for (n, label, shown, _) in &rows {
        let changes: Vec<String> = shown.iter().map(|(f, a, b)| format!("{f}: {} → {}", if a.is_empty() { "(empty)" } else { a }, if b.is_empty() { "(empty)" } else { b })).collect();
        out.push_str(&format!("{n} | {label} | {}\n", changes.join("; ")));
    }
    out.push_str("\nShow this table to the user. To save, call bulk_set with action apply and the same filter and set; rows the user does not want go in skip.\n");
    Ok(out)
}

pub fn ops_to_apply(input: &Value, cfg: &AppConfig) -> Result<Vec<Value>> {
    let skip: Vec<u64> = input["skip"].as_array().into_iter().flatten().filter_map(Value::as_u64).collect();
    let ops: Vec<Value> = plan(cfg, input)?.into_iter().filter(|(n, ..)| !skip.contains(&(*n as u64))).map(|(.., op)| op).collect();
    if ops.is_empty() {
        bail!("nothing to save: every row was left out or nothing differs");
    }
    Ok(ops)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row() -> Row {
        Row {
            id: 7,
            name: "Customer Service".into(),
            kind: "Class".into(),
            fields: HashMap::from([("status", "Proposed".into()), ("phase", "1.0".into()), ("version", "1.0".into()), ("author", "anna".into()), ("stereotype", "".into())]),
            tags: HashMap::from([("owner".to_string(), "Sales".to_string())]),
        }
    }

    #[test]
    fn changes_before_and_after() {
        let set = json!({ "status": "Approved", "phase": "1.0", "stereotype": "«service»", "tags": { "Owner": "Finance", "Gone": null }, "rename": { "find": "Service", "replace": "API" } });
        let (shown, op) = change(&row(), &set).unwrap();
        let fields: Vec<&str> = shown.iter().map(|s| s.0.as_str()).collect();
        assert_eq!(fields, ["status", "stereotype", "name", "tag Owner"], "the same phase is no change; a tag that is not there and set to null is no change");
        assert_eq!(op["status"], "Approved");
        assert_eq!(op["stereotype"], "service");
        assert_eq!(op["name"], "Customer API");
        assert_eq!(op["tags"], json!({ "Owner": "Finance" }));
        assert!(change(&row(), &json!({ "status": "Proposed" })).is_none());
    }

    /// LC-366: a bulk change saved through EA and undone from the journal, on a model COPY:
    /// `LOONAR_TEST_PROPS=E:/livetest/undo/undo.properties cargo test bulk_apply_and_undo_live -- --ignored --nocapture`
    #[test]
    #[ignore = "requires EA and a copy of a test model"]
    fn bulk_apply_and_undo_live() {
        let cfg = crate::config::AppConfig::load(std::path::Path::new(&std::env::var("LOONAR_TEST_PROPS").expect("LOONAR_TEST_PROPS"))).expect("app.properties");
        let q = |sql: &str| crate::tools::query_rows(&cfg, sql).unwrap();
        let ids: Vec<i64> = q("select object_id from t_object where object_type = 'Requirement' order by object_id").iter().take(2).map(|r| r[0].parse().unwrap()).collect();
        let before = q(&format!("select object_id, coalesce(status,''), coalesce(author,'') from t_object where object_id in ({}, {}) order by object_id", ids[0], ids[1]));
        let input = json!({ "filter": { "ids": ids }, "set": { "status": "ZZ-bulk", "author": "bulk-test", "tags": { "ZZBulk": "yes" } } });
        let ops = ops_to_apply(&input, &cfg).unwrap();
        let (reverse, not_undoable) = crate::undo::before(&cfg, &ops);
        let done = crate::ea::run_batch(&cfg, &ops, &crate::ea::BatchOptions::default()).expect("bulk save");
        let after = q(&format!("select coalesce(status,''), coalesce(author,'') from t_object where object_id = {}", ids[0]));
        assert_eq!(after[0], vec!["ZZ-bulk".to_string(), "bulk-test".to_string()]);
        let journal = std::env::temp_dir().join(format!("loonar-bulk-{}", std::process::id()));
        std::fs::create_dir_all(&journal).unwrap();
        let mut jcfg = cfg.clone();
        jcfg.base_dir = journal.clone();
        crate::undo::record(&jcfg, "bulk", &done, reverse, not_undoable, Vec::new());
        let back: Vec<Value> = crate::undo::latest(&jcfg, 1)[0]["reverse"].as_array().unwrap().clone();
        crate::ea::run_batch(&cfg, &back, &crate::ea::BatchOptions::default()).expect("undo");
        crate::ea::shutdown();
        let restored = q(&format!("select object_id, coalesce(status,''), coalesce(author,'') from t_object where object_id in ({}, {}) order by object_id", ids[0], ids[1]));
        assert_eq!(restored, before, "status and author back");
        assert!(q(&format!("select value from t_objectproperties where object_id = {} and property = 'ZZBulk' and coalesce(value,'') <> ''", ids[0])).is_empty(), "the new tag is gone or empty");
        let _ = std::fs::remove_dir_all(&journal);
    }

    /// LC-366: preview and the operations on a model copy (nothing is saved here):
    /// `LOONAR_TEST_PROPS=E:/livetest/undo/undo.properties cargo test bulk_preview_live -- --ignored --nocapture`
    #[test]
    #[ignore = "requires the model from LOONAR_TEST_PROPS"]
    fn bulk_preview_live() {
        let cfg = crate::config::AppConfig::load(std::path::Path::new(&std::env::var("LOONAR_TEST_PROPS").expect("LOONAR_TEST_PROPS"))).expect("app.properties");
        let input = json!({ "action": "preview", "filter": { "types": ["Requirement"], "name_contains": "customer" }, "set": { "status": "Approved", "tags": { "Owner": "Sales" } } });
        let table = preview(&input, &cfg).unwrap();
        println!("{}", table.lines().take(6).collect::<Vec<_>>().join("\n"));
        let all = ops_to_apply(&input, &cfg).unwrap();
        let fewer = ops_to_apply(&json!({ "filter": input["filter"], "set": input["set"], "skip": [1] }), &cfg).unwrap();
        assert_eq!(fewer.len() + 1, all.len());
        assert!(select(&cfg, &json!({}), &[]).is_err(), "no filter, no bulk change");
    }
}

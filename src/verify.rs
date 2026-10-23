//! Verification after a write: the model is read again and compared with what the operations asked for, so the
//! assistant reports what is really in the model (a write that "succeeded" but left nothing is caught here).

use crate::config::AppConfig;
use crate::tools::query_rows;
use serde_json::Value;
use std::collections::HashMap;

/// Result of the verification: how many checks passed and the differences found.
pub struct Verification {
    pub passed: usize,
    pub problems: Vec<String>,
}

fn esc(text: &str) -> String {
    text.replace('\'', "''")
}

/// Text compared the way EA stores it: line endings and outer spaces do not count.
fn norm(text: &str) -> String {
    text.replace("\r\n", "\n").trim().to_string()
}

pub fn verify(cfg: &AppConfig, ops: &[Value], result: &Value) -> Verification {
    let keys: HashMap<String, i64> = result["created"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|c| Some((c["key"].as_str()?.to_string(), c["id"].as_i64()?)))
        .collect();
    // An id given directly or a "#key" of an object created in the same job.
    let id_of = |v: &Value| -> Option<i64> {
        match v {
            Value::Number(n) => n.as_i64(),
            Value::String(s) => s.strip_prefix('#').and_then(|k| keys.get(k).copied()).or_else(|| s.parse().ok()),
            _ => None,
        }
    };
    let one = |sql: String| -> Option<Vec<String>> { query_rows(cfg, &sql).ok().and_then(|r| r.into_iter().next()) };
    let count = |sql: String| -> i64 { one(sql).and_then(|r| r.first().and_then(|v| v.parse().ok())).unwrap_or(-1) };
    let mut v = Verification { passed: 0, problems: Vec::new() };
    let mut check = |ok: bool, problem: String, v: &mut Verification| if ok { v.passed += 1 } else { v.problems.push(problem) };

    // Tagged values of an element after the write: name -> value.
    let tags_of = |id: i64| -> HashMap<String, String> {
        query_rows(cfg, &format!("select property as c0, coalesce(value,'') as c1 from t_objectproperties where object_id = {id}"))
            .unwrap_or_default()
            .into_iter()
            .map(|r| (r[0].clone(), r[1].clone()))
            .collect()
    };
    let check_tags = |id: i64, label: &str, tags: &Value, v: &mut Verification| {
        let Some(tags) = tags.as_object() else { return };
        let now = tags_of(id);
        for (name, want) in tags {
            match want {
                Value::Null => {
                    if now.contains_key(name) {
                        v.problems.push(crate::i18n::fmt("{0}: tagged value {1} is still there", &[label.to_string(), name.clone()]));
                    } else {
                        v.passed += 1;
                    }
                }
                other => {
                    let want = match other { Value::String(s) => s.clone(), x => x.to_string() };
                    match now.get(name) {
                        Some(got) if norm(got) == norm(&want) => v.passed += 1,
                        Some(got) => v.problems.push(crate::i18n::fmt("{0}: tagged value {1} = \"{2}\" instead of \"{3}\"", &[label.to_string(), name.clone(), got.clone(), want])),
                        None => v.problems.push(crate::i18n::fmt("{0}: tagged value {1} is missing", &[label.to_string(), name.clone()])),
                    }
                }
            }
        }
    };

    for op in ops {
        let kind = op["op"].as_str().unwrap_or("");
        let key = op["key"].as_str().unwrap_or("");
        let created = keys.get(key).copied();
        match kind {
            "create_package" => {
                let ok = created.is_some_and(|id| count(format!("select count(*) as c0 from t_package where package_id = {id}")) == 1);
                check(ok, crate::i18n::fmt("package \"{0}\" is not in the model", &[op["name"].as_str().unwrap_or("").to_string()]), &mut v);
            }
            "create_diagram" => {
                let ok = created.is_some_and(|id| count(format!("select count(*) as c0 from t_diagram where diagram_id = {id}")) == 1);
                check(ok, crate::i18n::fmt("diagram \"{0}\" is not in the model", &[op["name"].as_str().unwrap_or("").to_string()]), &mut v);
            }
            "create_element" => {
                let name = op["name"].as_str().unwrap_or("").to_string();
                match created.and_then(|id| one(format!("select object_type as c0, coalesce(stereotype,'') as c1, package_id as c2, parentid as c3 from t_object where object_id = {id}")).map(|r| (id, r))) {
                    None => check(false, crate::i18n::fmt("element \"{0}\" is not in the model", &[name.clone()]), &mut v),
                    Some((id, row)) => {
                        v.passed += 1;
                        // In the package (or inside the element) that was asked for.
                        if let Some(package) = id_of(&op["package"]) {
                            check(row[2] == package.to_string(), crate::i18n::fmt("element \"{0}\" is in package {1} instead of {2}", &[name.clone(), row[2].clone(), package.to_string()]), &mut v);
                        } else if let Some(parent) = id_of(&op["parent_element"]) {
                            check(row[3] == parent.to_string(), crate::i18n::fmt("element \"{0}\" is not inside element {1}", &[name.clone(), parent.to_string()]), &mut v);
                        }
                        if let Some(stereo) = op["stereotype"].as_str().filter(|s| !s.is_empty()) {
                            let want = stereo.rsplit("::").next().unwrap_or(stereo).to_lowercase();
                            check(row[1].to_lowercase().contains(&want), crate::i18n::fmt("element \"{0}\": stereotype \"{1}\" instead of \"{2}\"", &[name.clone(), row[1].clone(), stereo.to_string()]), &mut v);
                        }
                        check_tags(id, &name, &op["tags"], &mut v);
                    }
                }
            }
            "create_connector" => {
                let row = created.and_then(|id| one(format!(
                    "select c.connector_type as c0, c.start_object_id as c1, c.end_object_id as c2, coalesce(s.name,'') as c3, coalesce(t.name,'') as c4 \
                     from t_connector c left join t_object s on s.object_id = c.start_object_id left join t_object t on t.object_id = c.end_object_id where c.connector_id = {id}"
                )));
                let want = op["type"].as_str().unwrap_or("");
                match row {
                    None => check(false, crate::i18n::fmt("connector {0} is not in the model", &[want.to_string()]), &mut v),
                    Some(r) => {
                        check(want.is_empty() || r[0].eq_ignore_ascii_case(want), crate::i18n::fmt("connector of type {0} instead of {1}", &[r[0].clone(), want.to_string()]), &mut v);
                        // The ends as asked: the source and the target element.
                        if let (Some(source), Some(target)) = (id_of(&op["source"]), id_of(&op["target"])) {
                            let name = |id: i64| one(format!("select coalesce(name,'') as c0 from t_object where object_id = {id}")).map(|r| r[0].clone()).unwrap_or_else(|| id.to_string());
                            check(
                                r[1] == source.to_string() && r[2] == target.to_string(),
                                crate::i18n::fmt("connector {0} goes {1} → {2} instead of {3} → {4}", &[want.to_string(), r[3].clone(), r[4].clone(), name(source), name(target)]),
                                &mut v,
                            );
                        }
                    }
                }
            }
            "place" => {
                if let (Some(d), Some(e)) = (id_of(&op["diagram"]), id_of(&op["element"])) {
                    let ok = count(format!("select count(*) as c0 from t_diagramobjects where diagram_id = {d} and object_id = {e}")) >= 1;
                    check(ok, crate::i18n::fmt("element {0} is not on diagram {1}", &[e.to_string(), d.to_string()]), &mut v);
                }
            }
            "update_element" => {
                let Some(id) = id_of(&op["element"]) else { continue };
                let Some(row) = one(format!(
                    "select coalesce(name,'') as c0, coalesce(alias,'') as c1, coalesce(note,'') as c2, coalesce(stereotype,'') as c3 from t_object where object_id = {id}"
                )) else {
                    check(false, crate::i18n::fmt("element {0} is not in the model", &[id.to_string()]), &mut v);
                    continue;
                };
                let label = row[0].clone();
                for (field, index) in [("name", 0), ("alias", 1), ("notes", 2)] {
                    if let Some(want) = op[field].as_str() {
                        check(norm(&row[index]) == norm(want), crate::i18n::fmt("{0}: {1} is \"{2}\" instead of \"{3}\"", &[label.clone(), field.to_string(), norm(&row[index]).chars().take(80).collect(), want.chars().take(80).collect()]), &mut v);
                    }
                }
                if op["clear_stereotype"] == true || op["stereotype"].as_str() == Some("") {
                    check(row[3].is_empty(), crate::i18n::fmt("{0}: the stereotype \"{1}\" is still there", &[label.clone(), row[3].clone()]), &mut v);
                } else if let Some(stereo) = op["stereotype"].as_str() {
                    let want = stereo.rsplit("::").next().unwrap_or(stereo).to_lowercase();
                    check(row[3].to_lowercase().contains(&want), crate::i18n::fmt("{0}: stereotype \"{1}\" instead of \"{2}\"", &[label.clone(), row[3].clone(), stereo.to_string()]), &mut v);
                }
                check_tags(id, &label, &op["tags"], &mut v);
            }
            "update_connector" => {
                let Some(id) = id_of(&op["connector"]) else { continue };
                let Some(row) = one(format!("select coalesce(name,'') as c0, coalesce(stereotype,'') as c1 from t_connector where connector_id = {id}")) else {
                    check(false, crate::i18n::fmt("connector {0} is not in the model", &[id.to_string()]), &mut v);
                    continue;
                };
                if let Some(want) = op["name"].as_str() {
                    check(norm(&row[0]) == norm(want), crate::i18n::fmt("connector {0}: name \"{1}\" instead of \"{2}\"", &[id.to_string(), row[0].clone(), want.to_string()]), &mut v);
                }
                if op["clear_stereotype"] == true {
                    check(row[1].is_empty(), crate::i18n::fmt("connector {0}: the stereotype \"{1}\" is still there", &[id.to_string(), row[1].clone()]), &mut v);
                }
            }
            "delete" => {
                for (field, table, column) in [("element", "t_object", "object_id"), ("diagram", "t_diagram", "diagram_id"), ("connector", "t_connector", "connector_id"), ("package", "t_package", "package_id")] {
                    if let Some(id) = id_of(&op[field]) {
                        let left = count(format!("select count(*) as c0 from {table} where {column} = {id}"));
                        check(left == 0, crate::i18n::fmt("{0} {1} is still in the model", &[field.to_string(), id.to_string()]), &mut v);
                    }
                }
            }
            "export_xmi" | "generate_report" => {
                // The file the operation wrote.
                let file = op["file"].as_str().unwrap_or("").replace('/', "\\");
                let size = std::fs::metadata(&file).map(|m| m.len()).unwrap_or(0);
                check(size > 0, crate::i18n::fmt("file {0} was not written", &[file.clone()]), &mut v);
            }
            "move" => {
                if let (Some(e), Some(p)) = (id_of(&op["element"]), id_of(&op["package"])) {
                    let now = one(format!("select package_id as c0 from t_object where object_id = {e}")).map(|r| r[0].clone()).unwrap_or_default();
                    check(now == p.to_string(), crate::i18n::fmt("element {0} is in package {1} instead of {2}", &[e.to_string(), now, p.to_string()]), &mut v);
                }
            }
            _ => {}
        }
    }
    let _ = esc;
    v
}

/// Relationships of the elements a write would delete (for the confirmation): one line per element that has any.
pub fn relations_of_deleted(cfg: &AppConfig, ops: &[Value]) -> Vec<String> {
    let mut out = Vec::new();
    for op in ops.iter().filter(|o| o["op"] == "delete") {
        let Some(id) = op["element"].as_i64() else { continue };
        let rows = query_rows(cfg, &format!(
            "select c.connector_type as c0, s.name as c1, t.name as c2 from t_connector c join t_object s on s.object_id = c.start_object_id \
             join t_object t on t.object_id = c.end_object_id where c.start_object_id = {id} or c.end_object_id = {id}"
        ))
        .unwrap_or_default();
        if rows.is_empty() {
            continue;
        }
        let name = query_rows(cfg, &format!("select name as c0 from t_object where object_id = {id}")).ok().and_then(|r| r.into_iter().next()).map(|r| r[0].clone()).unwrap_or_default();
        let list: Vec<String> = rows.iter().take(12).map(|r| format!("{} {} → {}", r[0], r[1], r[2])).collect();
        out.push(crate::i18n::fmt("\"{0}\" (id {1}) has {2} relationships that will be deleted with it: {3}", &[name, id.to_string(), rows.len().to_string(), list.join("; ")]));
    }
    out
}

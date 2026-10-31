//! A recycle bin for the model (LC-367): elements are moved to the package "Recycle bin (Compass)" instead of being
//! deleted, with the package they came from in the tagged value CompassBinFrom, and moved back on restore. Both are
//! ordinary saves (approval, undo journal, audit). Deleting for good stays EA's (or Compass's delete) after a look at
//! what the element is used by (where_used); the deletion card names its relationships and diagrams.

use crate::config::AppConfig;
use crate::tools::query_rows;
use anyhow::{bail, Result};
use serde_json::{json, Value};

pub const BIN: &str = "Recycle bin (Compass)";
pub const FROM_TAG: &str = "CompassBinFrom";

pub fn definition() -> Value {
    json!({
        "name": "recycle_bin",
        "description": "A recycle bin in the model instead of deleting: action move puts elements into the package \"Recycle bin (Compass)\" \
(made under the first top package when missing) and remembers where each came from; action restore moves elements of the bin back to their \
packages; action list shows the bin. move and restore are saves the user approves (and can undo). Prefer it to deleting when the user is not \
sure an element is unused; check where_used first.",
        "input_schema": { "type": "object", "properties": {
            "action": { "type": "string", "enum": ["move", "restore", "list"] },
            "elements": { "type": "array", "items": { "type": "integer" }, "description": "object_ids (move; for restore optional: empty = all of the bin)" }
        }, "required": ["action"] }
    })
}

/// The bin package's id, if there is one.
fn bin(cfg: &AppConfig) -> Result<Option<i64>> {
    Ok(query_rows(cfg, &format!("select package_id from t_package where name = '{BIN}' order by package_id"))?.first().and_then(|r| r[0].parse().ok()))
}

/// The elements in the bin: (object_id, name, the package they came from (id), its name).
fn contents(cfg: &AppConfig, bin: i64) -> Result<Vec<(i64, String, Option<i64>, String)>> {
    Ok(query_rows(cfg, &format!(
        "select o.object_id, o.name, coalesce((select max(t.value) from t_objectproperties t where t.object_id = o.object_id and t.property = '{FROM_TAG}'), '') \
         from t_object o where o.package_id = {bin} order by o.name"
    ))?
    .into_iter()
    .map(|r| {
        let from: Option<i64> = r[2].parse().ok();
        let name = from.and_then(|p| query_rows(cfg, &format!("select name from t_package where package_id = {p}")).ok()?.first().map(|x| x[0].clone())).unwrap_or_default();
        (r[0].parse().unwrap_or(0), r[1].clone(), from, name)
    })
    .collect())
}

/// The operations of an action (empty for list); the text for list.
pub fn plan(input: &Value, cfg: &AppConfig) -> Result<(Vec<Value>, String)> {
    let ids: Vec<i64> = input["elements"].as_array().into_iter().flatten().filter_map(Value::as_i64).collect();
    let bin_id = bin(cfg)?;
    match input["action"].as_str().unwrap_or("list") {
        "move" => {
            if ids.is_empty() {
                bail!("give the object_ids of the elements to put into the recycle bin");
            }
            let mut ops = Vec::new();
            let target = match bin_id {
                Some(id) => json!(id),
                None => {
                    let root: i64 = query_rows(cfg, "select package_id from t_package where parent_id = 0 order by package_id")?
                        .iter()
                        .filter_map(|r| r[0].parse().ok())
                        .find_map(|root: i64| query_rows(cfg, &format!("select package_id from t_package where parent_id = {root} order by tpos, package_id")).ok()?.first()?.first()?.parse().ok())
                        .ok_or_else(|| anyhow::anyhow!("the model has no package to put the recycle bin in"))?;
                    ops.push(json!({ "op": "create_package", "key": "bin", "parent": root, "name": BIN }));
                    json!("#bin")
                }
            };
            for id in ids {
                let Some(row) = query_rows(cfg, &format!("select package_id from t_object where object_id = {id}"))?.into_iter().next() else {
                    bail!("there is no element with id {id}");
                };
                if Some(row[0].parse::<i64>().unwrap_or(0)) == bin_id {
                    continue;
                }
                ops.push(json!({ "op": "update_element", "element": id, "tags": { FROM_TAG: row[0] } }));
                ops.push(json!({ "op": "move", "element": id, "package": target }));
            }
            Ok((ops, String::new()))
        }
        "restore" => {
            let Some(bin_id) = bin_id else { bail!("the model has no recycle bin") };
            let mut ops = Vec::new();
            for (id, name, from, _) in contents(cfg, bin_id)? {
                if !ids.is_empty() && !ids.contains(&id) {
                    continue;
                }
                let Some(from) = from else { bail!("\"{name}\" (id {id}) does not say where it came from: move it by hand") };
                ops.push(json!({ "op": "move", "element": id, "package": from }));
                ops.push(json!({ "op": "update_element", "element": id, "tags": { FROM_TAG: null } }));
            }
            if ops.is_empty() {
                bail!("nothing to restore");
            }
            Ok((ops, String::new()))
        }
        _ => {
            let Some(bin_id) = bin_id else { return Ok((Vec::new(), "The model has no recycle bin yet.".into())) };
            let list = contents(cfg, bin_id)?;
            let mut text = format!("Recycle bin (package_id {bin_id}): {} elements.\n", list.len());
            for (id, name, _, from) in list {
                text.push_str(&format!("  - \"{name}\" (id {id}) from package \"{}\"\n", if from.is_empty() { "?" } else { from.as_str() }));
            }
            Ok((Vec::new(), text))
        }
    }
}

/// What deleting an element takes with it, for the deletion card: its relationships and the diagrams it is on.
pub fn element_reach(cfg: &AppConfig, id: i64) -> Option<String> {
    let links = query_rows(cfg, &format!("select count(*) from t_connector where start_object_id = {id} or end_object_id = {id}")).ok()?.first()?.first()?.clone();
    let diagrams = query_rows(cfg, &format!("select d.name from t_diagramobjects x join t_diagram d on d.diagram_id = x.diagram_id where x.object_id = {id} order by d.name")).ok()?;
    let names: Vec<String> = diagrams.iter().take(5).map(|r| format!("\"{}\"", r[0])).collect();
    Some(crate::i18n::fmt("relationships: {0}, diagrams: {1}{2}", &[
        links,
        diagrams.len().to_string(),
        if names.is_empty() { String::new() } else { format!(": {}{}", names.join(", "), if diagrams.len() > 5 { ", …" } else { "" }) },
    ]))
}

#[cfg(test)]
mod tests {
    /// LC-367: into the bin and back through EA, on a model COPY:
    /// `LOONAR_TEST_PROPS=E:/livetest/undo/undo.properties cargo test recycle_bin_live -- --ignored --nocapture`
    #[test]
    #[ignore = "requires EA and a copy of a test model"]
    fn recycle_bin_live() {
        let cfg = crate::config::AppConfig::load(std::path::Path::new(&std::env::var("LOONAR_TEST_PROPS").expect("LOONAR_TEST_PROPS"))).expect("app.properties");
        let q = |sql: &str| crate::tools::query_rows(&cfg, sql).unwrap();
        let row = q("select object_id, package_id from t_object where object_type = 'Requirement' order by object_id")[0].clone();
        let id: i64 = row[0].parse().unwrap();
        println!("reach: {:?}", super::element_reach(&cfg, id));
        let (ops, _) = super::plan(&serde_json::json!({ "action": "move", "elements": [id] }), &cfg).unwrap();
        crate::ea::run_batch(&cfg, &ops, &crate::ea::BatchOptions::default()).expect("into the bin");
        let bin = super::bin(&cfg).unwrap().expect("the bin exists");
        assert_eq!(q(&format!("select package_id from t_object where object_id = {id}"))[0][0], bin.to_string());
        println!("{}", super::plan(&serde_json::json!({ "action": "list" }), &cfg).unwrap().1);
        let (back, _) = super::plan(&serde_json::json!({ "action": "restore", "elements": [id] }), &cfg).unwrap();
        crate::ea::run_batch(&cfg, &back, &crate::ea::BatchOptions::default()).expect("restore");
        crate::ea::shutdown();
        assert_eq!(q(&format!("select package_id from t_object where object_id = {id}"))[0][0], row[1], "back in its package");
    }
}

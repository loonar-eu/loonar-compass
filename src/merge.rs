//! Merge and replace (LC-370, /merge-elements): one element takes the place of its duplicate everywhere – its
//! relationships, its places on diagrams (positions kept), instances and lifelines classified by it, attributes,
//! parameters and operations typed by it, tagged values pointing at it – its tagged values and notes are added to the
//! kept one, and the duplicate is deleted. What has to change is read here with SQL; EA does it in one operation
//! (merge_elements) through its API. A baseline of the package is made before (a merge cannot be undone by operations).

use crate::config::AppConfig;
use crate::tools::query_rows;
use anyhow::{bail, Result};
use serde_json::{json, Value};

pub fn definition() -> Value {
    json!({
        "name": "merge_elements",
        "description": "Merge a duplicate element into the one to keep: its relationships move to the kept element, it is replaced on every \
diagram (same position; where both are on a diagram, the duplicate's place goes), instances, lifelines and actions classified by it, attributes, \
parameters and operations typed by it and tagged values pointing at it are re-pointed, its tagged values the kept one lacks and (notes: \"append\") its \
notes are added, then it is deleted. action preview: what would change (reads only; show it). action apply: do it – the user approves it always, a \
baseline of the package is made first. Both must be of the same type unless the user says otherwise. Use find_duplicates to find candidates and \
/merge-elements.",
        "input_schema": { "type": "object", "properties": {
            "action": { "type": "string", "enum": ["preview", "apply"] },
            "keep": { "type": "integer", "description": "object_id of the element that stays" },
            "remove": { "type": "integer", "description": "object_id of the duplicate" },
            "notes": { "type": "string", "enum": ["append", "keep"], "description": "append (default): the duplicate's notes are added to the kept one's" },
            "allow_other_type": { "type": "boolean" }
        }, "required": ["action", "keep", "remove"] }
    })
}

fn ids(cfg: &AppConfig, sql: String) -> Result<Vec<i64>> {
    Ok(query_rows(cfg, &sql)?.iter().filter_map(|r| r[0].parse().ok()).collect())
}

/// The merge_elements operation with everything EA has to re-point, and its description.
pub fn plan(input: &Value, cfg: &AppConfig) -> Result<(Value, String)> {
    let (Some(keep), Some(remove)) = (input["keep"].as_i64(), input["remove"].as_i64()) else { bail!("give keep and remove (object_ids)") };
    if keep == remove {
        bail!("keep and remove are the same element");
    }
    let info = |id: i64| -> Result<Vec<String>> {
        query_rows(cfg, &format!("select o.name, o.object_type, o.ea_guid, coalesce(o.note,''), p.name from t_object o join t_package p on p.package_id = o.package_id where o.object_id = {id}"))?
            .into_iter()
            .next()
            .ok_or_else(|| anyhow::anyhow!("there is no element with id {id}"))
    };
    let (k, r) = (info(keep)?, info(remove)?);
    if k[1] != r[1] && input["allow_other_type"] != true {
        bail!("\"{}\" is a {} and \"{}\" a {}: merge elements of one type (or set allow_other_type when the user wants it)", k[0], k[1], r[0], r[1]);
    }
    let guid = crate::model_db::sql_text(cfg.model_source, &r[2]);
    let connectors = ids(cfg, format!("select connector_id from t_connector where start_object_id = {remove} or end_object_id = {remove}"))?;
    let both = ids(cfg, format!("select a.diagram_id from t_diagramobjects a where a.object_id = {remove} and exists (select 1 from t_diagramobjects b where b.diagram_id = a.diagram_id and b.object_id = {keep})"))?;
    let diagrams = ids(cfg, format!("select distinct diagram_id from t_diagramobjects where object_id = {remove}"))?;
    let instances = ids(cfg, format!("select object_id from t_object where classifier = {remove} or classifier_guid = '{guid}'"))?;
    let attributes = ids(cfg, format!("select id from t_attribute where classifier = '{remove}'"))?;
    let operations = ids(cfg, format!("select operationid from t_operation where classifier = '{remove}'"))?;
    let parameters = ids(cfg, format!("select distinct operationid from t_operationparams where classifier = '{remove}'"))?;
    let tag_owners = ids(cfg, format!("select distinct object_id from t_objectproperties where value = '{guid}'"))?;
    let op = json!({
        "op": "merge_elements", "keep": keep, "remove": remove, "notes": input["notes"].as_str().unwrap_or("append"),
        "connectors": connectors, "diagrams": diagrams, "diagrams_with_both": both, "instances": instances, "attributes": attributes,
        "operations": operations, "parameter_operations": parameters, "tag_owners": tag_owners,
    });
    let text = format!(
        "Merge \"{}\" ({}, id {remove}, package \"{}\") into \"{}\" ({}, id {keep}, package \"{}\"):\n\
         - {} relationships move to the kept element\n- replaced on {} diagrams (on {} of them both were: the duplicate's place goes)\n\
         - {} instances/lifelines/actions, {} attributes, {} operation return types, parameters of {} operations re-pointed\n\
         - tagged values pointing at it on {} elements re-pointed; its own tagged values the kept one lacks are added\n- notes: {}\n\
         - then the duplicate is deleted (a baseline of its package is made first)",
        r[0], r[1], r[4], k[0], k[1], k[4], connectors.len(), diagrams.len(), both.len(), instances.len(), attributes.len(), operations.len(), parameters.len(), tag_owners.len(),
        if input["notes"].as_str() == Some("keep") || r[3].trim().is_empty() || r[3] == k[3] { "the kept element's stay as they are" } else { "the duplicate's are added below the kept element's" }
    );
    Ok((op, text))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    /// LC-370: a duplicate merged through EA on a model COPY – its relationship, its place on a diagram, the diagram
    /// with both, its tag and notes go to the kept element, and it is deleted:
    /// `LOONAR_TEST_PROPS=E:/livetest/undo/undo.properties cargo test merge_live -- --ignored --nocapture`
    #[test]
    #[ignore = "requires EA and a copy of a test model"]
    fn merge_live() {
        let cfg = crate::config::AppConfig::load(std::path::Path::new(&std::env::var("LOONAR_TEST_PROPS").expect("LOONAR_TEST_PROPS"))).expect("app.properties");
        let q = |sql: &str| crate::tools::query_rows(&cfg, sql).unwrap();
        let root: i64 = q("select package_id from t_package where parent_id = 0 order by package_id")[0][0].parse().unwrap();
        let ops = vec![
            json!({"op":"create_package","key":"pkg","parent":root,"name":format!("ZZ_merge_{}", chrono::Local::now().format("%H%M%S"))}),
            json!({"op":"create_element","key":"keep","package":"#pkg","name":"Customer","type":"Class","notes":"Kept notes."}),
            json!({"op":"create_element","key":"dup","package":"#pkg","name":"Customers","type":"Class","notes":"Duplicate notes.","tags":{"Owner":"Sales"}}),
            json!({"op":"create_element","key":"order","package":"#pkg","name":"Order","type":"Class"}),
            json!({"op":"create_connector","key":"c","source":"#order","target":"#dup","type":"Association"}),
            json!({"op":"create_diagram","key":"d1","package":"#pkg","name":"Only the duplicate","type":"Logical"}),
            json!({"op":"create_diagram","key":"d2","package":"#pkg","name":"Both","type":"Logical"}),
            json!({"op":"place","diagram":"#d1","element":"#dup","left":300,"top":120,"width":120,"height":70}),
            json!({"op":"place","diagram":"#d1","element":"#order","left":40,"top":120,"width":120,"height":70}),
            json!({"op":"place","diagram":"#d2","element":"#dup","left":40,"top":40,"width":120,"height":70}),
            json!({"op":"place","diagram":"#d2","element":"#keep","left":300,"top":40,"width":120,"height":70}),
        ];
        let created = crate::ea::run_batch(&cfg, &ops, &crate::ea::BatchOptions { rollback_key: Some("pkg".into()), keep_locks: false }).expect("creation");
        let id_of = |key: &str| created["created"].as_array().unwrap().iter().find(|c| c["key"] == key).expect(key)["id"].as_i64().unwrap();
        let (pkg, keep, dup, d1, d2) = (id_of("pkg"), id_of("keep"), id_of("dup"), id_of("d1"), id_of("d2"));
        let (op, text) = super::plan(&json!({ "keep": keep, "remove": dup }), &cfg).unwrap();
        println!("{text}");
        let place_before = q(&format!("select rectleft, recttop from t_diagramobjects where diagram_id = {d1} and object_id = {dup}"));
        crate::ea::run_batch(&cfg, &[op], &crate::ea::BatchOptions::default()).expect("merge");
        let check = || -> Vec<String> {
            let mut problems = Vec::new();
            if !q(&format!("select object_id from t_object where object_id = {dup}")).is_empty() { problems.push("the duplicate is still there".to_string()); }
            if q(&format!("select connector_id from t_connector where end_object_id = {keep} or start_object_id = {keep}")).len() != 1 { problems.push("the relationship did not move".into()); }
            if q(&format!("select rectleft, recttop from t_diagramobjects where diagram_id = {d1} and object_id = {keep}")) != place_before { problems.push("not in the duplicate's place".into()); }
            if q(&format!("select object_id from t_diagramobjects where diagram_id = {d2}")).len() != 1 { problems.push("the diagram with both should have only the kept one".into()); }
            if q(&format!("select value from t_objectproperties where object_id = {keep} and property = 'Owner'")).first().map(|r| r[0].as_str()) != Some("Sales") { problems.push("the tag did not come over".into()); }
            let notes = q(&format!("select note from t_object where object_id = {keep}"))[0][0].clone();
            if !(notes.contains("Kept notes.") && notes.contains("Duplicate notes.")) { problems.push(format!("notes: {notes}")); }
            problems
        };
        let problems = check();
        crate::ea::run_batch(&cfg, &[json!({"op":"delete","package":pkg})], &crate::ea::BatchOptions::default()).expect("cleanup");
        crate::ea::shutdown();
        assert!(problems.is_empty(), "{problems:?}");
    }
}

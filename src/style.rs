//! One look for many diagrams (LC-379, /apply-style): elements coloured by their stereotype, type or status on every
//! diagram of a package (or the diagrams given), from the colours the user gives or a readable palette. One save the
//! user approves; colours are cosmetic and not undone by operations (the card says so).

use crate::config::AppConfig;
use crate::tools::query_rows;
use anyhow::{bail, Result};
use serde_json::{json, Value};
use std::collections::BTreeMap;

/// Light fills that keep black text readable, in this order for the values found.
const PALETTE: &[&str] = &["#CFE2F3", "#D9EAD3", "#FFF2CC", "#F4CCCC", "#D9D2E9", "#FCE5CD", "#D0E0E3", "#EAD1DC", "#E6E6E6", "#C9DAF8"];

pub fn definition() -> Value {
    json!({
        "name": "apply_style",
        "description": "Colour the elements on many diagrams by their stereotype, type or status (one look for a whole package): colors \
{value: \"#RRGGBB\"} or, without them, a readable palette for the values found. Scope: diagrams (ids) or package (all its diagrams, child \
packages included). action preview: which value gets which colour and how many elements change (reads only); action apply: one save the user \
approves. Use it for /apply-style; offer a legend (create_legend) on the main diagram afterwards. profile (LC-420) picks a ready look and adds a \
legend on each diagram: management (by status – done green, in progress yellow, proposed blue, rejected or retired red), technical (by type, \
the palette), impact (by status as a heat scale for a change: approved/implemented calm, proposed and in progress warm, deprecated hot).",
        "input_schema": { "type": "object", "properties": {
            "action": { "type": "string", "enum": ["preview", "apply"] },
            "profile": { "type": "string", "enum": ["management", "technical", "impact"] },
            "by": { "type": "string", "enum": ["stereotype", "type", "status"] },
            "colors": { "type": "object" },
            "diagrams": { "type": "array", "items": { "type": "integer" } },
            "package": { "description": "package_id or exact name" }
        }, "required": ["action"] }
    })
}

/// The colours: given ones (case-insensitive keys) and the palette for the rest, in the order of the values.
pub(crate) fn colours(values: &[String], given: &Value) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    let mut next = 0;
    for v in values {
        let chosen = given.as_object().and_then(|g| g.iter().find(|(k, _)| k.eq_ignore_ascii_case(v)).and_then(|(_, c)| c.as_str().map(str::to_string)));
        let colour = chosen.unwrap_or_else(|| {
            let c = PALETTE[next % PALETTE.len()].to_string();
            next += 1;
            c
        });
        out.insert(v.clone(), colour);
    }
    out
}

/// The ready looks (LC-420): what to colour by and the colours of known values (the palette for the rest).
pub(crate) fn profile(name: &str) -> Option<(&'static str, Value)> {
    match name {
        "management" => Some(("status", json!({ "Approved": "#B6D7A8", "Implemented": "#B6D7A8", "Done": "#B6D7A8", "Validated": "#B6D7A8", "Mandatory": "#B6D7A8",
            "In Progress": "#FFE599", "Under Construction": "#FFE599", "Active": "#FFE599", "Proposed": "#CFE2F3", "Rejected": "#EA9999", "Retired": "#EA9999", "Deprecated": "#EA9999", "Invalidated": "#EA9999" }))),
        "technical" => Some(("type", json!({}))),
        "impact" => Some(("status", json!({ "Approved": "#D9EAD3", "Implemented": "#D9EAD3", "Validated": "#D9EAD3", "Proposed": "#FCE5CD", "In Progress": "#F9CB9C",
            "Under Construction": "#F9CB9C", "Deprecated": "#E06666", "Retired": "#E06666", "Rejected": "#E06666" }))),
        _ => None,
    }
}

/// (operations, description).
pub fn plan(input: &Value, cfg: &AppConfig) -> Result<(Vec<Value>, String)> {
    // A ready look: its property and colours (the given colours still win), and a legend on each diagram.
    let chosen = input["profile"].as_str().and_then(profile);
    let mut input = input.clone();
    if let Some((by, colors)) = &chosen {
        input["by"] = json!(by);
        let mut merged = colors.clone();
        for (k, v) in input["colors"].as_object().cloned().unwrap_or_default() {
            merged[k] = v;
        }
        input["colors"] = merged;
    }
    let input = &input;
    let column = match input["by"].as_str().unwrap_or("stereotype") {
        "type" => "o.object_type",
        "status" => "coalesce(o.status,'')",
        _ => "coalesce(o.stereotype,'')",
    };
    let mut diagrams: Vec<String> = input["diagrams"].as_array().into_iter().flatten().filter_map(Value::as_i64).map(|d| d.to_string()).collect();
    if diagrams.is_empty() {
        let (scope, _) = crate::model_usage::package_scope(cfg, &input["package"])?;
        if scope.is_empty() {
            bail!("give diagrams or a package");
        }
        diagrams = query_rows(cfg, &format!("select d.diagram_id from t_diagram d where 1 = 1 {}", scope.replace("o.package_id", "d.package_id")))?.into_iter().map(|r| r[0].clone()).collect();
    }
    if diagrams.is_empty() {
        bail!("there are no diagrams there");
    }
    let rows = query_rows(cfg, &format!(
        "select x.diagram_id, x.object_id, {column} from t_diagramobjects x join t_object o on o.object_id = x.object_id \
         where x.diagram_id in ({}) and o.object_type not in ('Note', 'Text', 'Boundary', 'Package', 'ActionPin', 'ActivityParameter', 'ActivityPartition', 'Partition', 'Swimlane', 'Port', 'ProvidedInterface', 'RequiredInterface', 'InitialNode', 'ActivityFinal', 'FlowFinal', 'Decision', 'MergeNode', 'Synchronization', 'StateNode')",
        diagrams.join(",")
    ))?;
    let mut values: Vec<String> = rows.iter().map(|r| r[2].clone()).filter(|v| !v.is_empty()).collect();
    values.sort();
    values.dedup();
    let colours = colours(&values, &input["colors"]);
    let mut ops: Vec<Value> = rows
        .iter()
        .filter_map(|r| colours.get(&r[2]).map(|c| json!({ "op": "set_color", "diagram": r[0].parse::<i64>().unwrap_or(0), "element": r[1].parse::<i64>().unwrap_or(0), "bgcolor": c })))
        .collect();
    if chosen.is_some() {
        // The legend of each diagram lists the values found on it.
        let filter = match input["by"].as_str().unwrap_or("") { "status" => "Element.Status", "type" => "Element.Type", _ => "Element.Stereotype" };
        let title = format!("{} view", input["profile"].as_str().unwrap_or("").to_uppercase());
        for d in &diagrams {
            let mut here: Vec<&String> = rows.iter().filter(|r| &r[0] == d).map(|r| &r[2]).filter(|v| colours.contains_key(*v)).collect();
            here.sort();
            here.dedup();
            if !here.is_empty() {
                ops.push(json!({ "op": "create_legend", "diagram": d.parse::<i64>().unwrap_or(0), "title": title, "filter": filter,
                    "entries": here.iter().map(|v| json!({ "value": v, "color": colours[*v], "label": v })).collect::<Vec<_>>() }));
            }
        }
    }
    let text = format!(
        "{} elements on {} diagrams, by {}:\n{}\nColours are not undone by operations.",
        ops.len(),
        diagrams.len(),
        input["by"].as_str().unwrap_or("stereotype"),
        colours.iter().map(|(v, c)| format!("  - {v}: {c}")).collect::<Vec<_>>().join("\n")
    );
    Ok((ops, text))
}

#[cfg(test)]
mod tests {
    #[test]
    fn given_colours_win_and_the_palette_fills_in() {
        let values = vec!["Approved".to_string(), "Proposed".to_string(), "Rejected".to_string()];
        let c = super::colours(&values, &serde_json::json!({ "approved": "#00FF00" }));
        assert_eq!(c["Approved"], "#00FF00");
        assert_eq!(c["Proposed"], super::PALETTE[0]);
        assert_eq!(c["Rejected"], super::PALETTE[1]);
    }

    /// LC-379: the plan on a model copy (reads only):
    /// `LOONAR_TEST_PROPS=E:/livetest/undo/undo.properties cargo test apply_style_live -- --ignored --nocapture`
    #[test]
    #[ignore = "requires the model from LOONAR_TEST_PROPS"]
    fn apply_style_live() {
        let cfg = crate::config::AppConfig::load(std::path::Path::new(&std::env::var("LOONAR_TEST_PROPS").expect("LOONAR_TEST_PROPS"))).expect("app.properties");
        let package: i64 = crate::tools::query_rows(&cfg, "select package_id from t_diagram group by package_id order by count(*) desc").unwrap()[0][0].parse().unwrap();
        let (ops, text) = super::plan(&serde_json::json!({ "by": "type", "package": package }), &cfg).unwrap();
        println!("{text}");
        assert!(!ops.is_empty() && ops.iter().all(|o| o["op"] == "set_color"));
    }

    /// LC-420: the management look in EA – colours by status and a legend on the diagram.
    /// `LOONAR_TEST_PROPS=… cargo test style_profile_live -- --ignored --nocapture`
    #[test]
    #[ignore = "requires EA and a copy of a test model"]
    fn style_profile_live() {
        use serde_json::json;
        let cfg = crate::config::AppConfig::load(std::path::Path::new(&std::env::var("LOONAR_TEST_PROPS").unwrap())).unwrap();
        let q = |sql: &str| crate::tools::query_rows(&cfg, sql).unwrap();
        let root: i64 = q("select package_id from t_package where parent_id = 0 order by package_id")[0][0].parse().unwrap();
        let created = crate::ea::run_batch(&cfg, &[
            json!({"op":"create_package","key":"pkg","parent":root,"name":format!("ZZ_look_{}", chrono::Local::now().format("%H%M%S"))}),
            json!({"op":"create_element","key":"a","package":"#pkg","name":"CRM","type":"Component"}),
            json!({"op":"update_element","element":"#a","status":"Approved"}),
            json!({"op":"create_element","key":"b","package":"#pkg","name":"Billing","type":"Component"}),
            json!({"op":"update_element","element":"#b","status":"Deprecated"}),
            json!({"op":"create_diagram","key":"d","package":"#pkg","name":"Landscape","type":"Component"}),
            json!({"op":"place","diagram":"#d","element":"#a","left":40,"top":40,"width":120,"height":60}),
            json!({"op":"place","diagram":"#d","element":"#b","left":220,"top":40,"width":120,"height":60}),
        ], &crate::ea::BatchOptions { rollback_key: Some("pkg".into()), keep_locks: false }).unwrap();
        let id_of = |key: &str| created["created"].as_array().unwrap().iter().find(|c| c["key"] == key).unwrap()["id"].as_i64().unwrap();
        let (pkg, d) = (id_of("pkg"), id_of("d"));
        let (ops, text) = super::plan(&json!({ "profile": "management", "diagrams": [d] }), &cfg).unwrap();
        println!("{text}");
        crate::ea::run_batch(&cfg, &ops, &crate::ea::BatchOptions::default()).unwrap();
        let legend = q(&format!("select count(*) from t_diagramobjects x join t_object o on o.object_id = x.object_id where x.diagram_id = {d} and o.object_type = 'Text' and o.ntype = 76"));
        let colours = q(&format!("select objectstyle from t_diagramobjects where diagram_id = {d} and object_id = {}", id_of("b")));
        crate::ea::run_batch(&cfg, &[json!({"op":"delete","package":pkg})], &crate::ea::BatchOptions::default()).unwrap();
        crate::ea::shutdown();
        assert_eq!(legend[0][0], "1");
        assert!(colours[0][0].contains("BCol="), "{colours:?}");
    }
}

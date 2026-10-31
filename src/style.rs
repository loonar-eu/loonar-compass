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
approves. Use it for /apply-style; offer a legend (create_legend) on the main diagram afterwards.",
        "input_schema": { "type": "object", "properties": {
            "action": { "type": "string", "enum": ["preview", "apply"] },
            "by": { "type": "string", "enum": ["stereotype", "type", "status"] },
            "colors": { "type": "object" },
            "diagrams": { "type": "array", "items": { "type": "integer" } },
            "package": { "description": "package_id or exact name" }
        }, "required": ["action", "by"] }
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

/// (operations, description).
pub fn plan(input: &Value, cfg: &AppConfig) -> Result<(Vec<Value>, String)> {
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
    let ops: Vec<Value> = rows
        .iter()
        .filter_map(|r| colours.get(&r[2]).map(|c| json!({ "op": "set_color", "diagram": r[0].parse::<i64>().unwrap_or(0), "element": r[1].parse::<i64>().unwrap_or(0), "bgcolor": c })))
        .collect();
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
}

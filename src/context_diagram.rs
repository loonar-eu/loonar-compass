//! A diagram of an element's surroundings (LC-378, /context-diagram): the element with everything related to it up to
//! N steps (optionally only some relationship types), on a new diagram in its package, laid out by EA. Elements are
//! placed, not copied: the diagram is a view of the model.

use crate::config::AppConfig;
use crate::tools::query_rows;
use anyhow::{bail, Result};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};

/// At most this many elements on one context diagram.
const MAX: usize = 40;

pub fn definition() -> Value {
    json!({
        "name": "context_diagram",
        "description": "Make a new diagram of an element's surroundings: the element and the elements related to it up to `depth` steps (1–3, \
default 1), optionally only over some relationship types, in the element's package, laid out automatically (at most 40 elements; the nearest \
first). The elements are placed, not copied. One save the user approves (undoable). Use it for /context-diagram and to show a path between \
two parts of the model.",
        "input_schema": { "type": "object", "properties": {
            "element": { "type": "integer", "description": "object_id" },
            "depth": { "type": "integer" },
            "types": { "type": "array", "items": { "type": "string" }, "description": "Relationship types to follow (empty = all)" },
            "name": { "type": "string", "description": "Diagram name (default: Context of <element>)" }
        }, "required": ["element"] }
    })
}

/// The elements around `start` up to `depth` steps, nearest first, at most MAX (the start included).
pub(crate) fn around(start: i64, edges: &HashMap<i64, Vec<i64>>, depth: usize) -> Vec<i64> {
    let mut order = vec![start];
    let mut seen: HashSet<i64> = HashSet::from([start]);
    let mut level = vec![start];
    for _ in 0..depth {
        let mut next = Vec::new();
        for node in &level {
            for n in edges.get(node).into_iter().flatten() {
                if order.len() >= MAX {
                    return order;
                }
                if seen.insert(*n) {
                    order.push(*n);
                    next.push(*n);
                }
            }
        }
        level = next;
    }
    order
}

pub fn plan(input: &Value, cfg: &AppConfig) -> Result<Vec<Value>> {
    let Some(start) = input["element"].as_i64() else { bail!("give the element (object_id)") };
    let depth = input["depth"].as_u64().unwrap_or(1).clamp(1, 3) as usize;
    let info = query_rows(cfg, &format!("select name, package_id from t_object where object_id = {start}"))?;
    let Some(info) = info.first() else { bail!("there is no element with id {start}") };
    let types: Vec<String> = input["types"].as_array().into_iter().flatten().filter_map(Value::as_str).map(str::to_lowercase).collect();
    let mut edges: HashMap<i64, Vec<i64>> = HashMap::new();
    for c in query_rows(cfg, "select start_object_id, end_object_id, lower(connector_type) from t_connector where connector_type not in ('Sequence', 'Collaboration', 'NoteLink')")? {
        if !types.is_empty() && !types.contains(&c[2]) {
            continue;
        }
        let (Ok(a), Ok(b)) = (c[0].parse::<i64>(), c[1].parse::<i64>()) else { continue };
        edges.entry(a).or_default().push(b);
        edges.entry(b).or_default().push(a);
    }
    let elements = around(start, &edges, depth);
    let name = input["name"].as_str().map(str::to_string).unwrap_or_else(|| format!("Context of {}", info[0]));
    let mut ops = vec![json!({ "op": "create_diagram", "key": "ctx", "package": info[1].parse::<i64>().unwrap_or(0), "name": name, "type": "Logical" })];
    // A grid first (EA's layout then arranges it by the relationships).
    for (i, id) in elements.iter().enumerate() {
        let (col, row) = ((i % 6) as i64, (i / 6) as i64);
        ops.push(json!({ "op": "place", "diagram": "#ctx", "element": id, "left": 40 + col * 200, "top": 40 + row * 130, "width": 140, "height": 70 }));
    }
    ops.push(json!({ "op": "layout_diagram", "diagram": "#ctx", "direction": "down" }));
    Ok(ops)
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    #[test]
    fn nearest_first() {
        let edges: HashMap<i64, Vec<i64>> = HashMap::from([(1, vec![2, 3]), (2, vec![1, 4]), (3, vec![1]), (4, vec![2, 5]), (5, vec![4])]);
        assert_eq!(super::around(1, &edges, 1), vec![1, 2, 3]);
        assert_eq!(super::around(1, &edges, 2), vec![1, 2, 3, 4]);
        assert_eq!(super::around(1, &edges, 3), vec![1, 2, 3, 4, 5]);
    }

    /// LC-378: a context diagram made through EA on a model COPY (removed again):
    /// `LOONAR_TEST_PROPS=E:/livetest/undo/undo.properties cargo test context_diagram_live -- --ignored --nocapture`
    #[test]
    #[ignore = "requires EA and a copy of a test model"]
    fn context_diagram_live() {
        let cfg = crate::config::AppConfig::load(std::path::Path::new(&std::env::var("LOONAR_TEST_PROPS").expect("LOONAR_TEST_PROPS"))).expect("app.properties");
        let q = |sql: &str| crate::tools::query_rows(&cfg, sql).unwrap();
        let start: i64 = q("select start_object_id from t_connector where connector_type = 'Association' order by connector_id")[0][0].parse().unwrap();
        let ops = super::plan(&serde_json::json!({ "element": start, "depth": 1 }), &cfg).unwrap();
        let done = crate::ea::run_batch(&cfg, &ops, &crate::ea::BatchOptions::default()).expect("context diagram");
        let diagram = done["created"].as_array().unwrap().iter().find(|c| c["kind"] == "diagram").unwrap()["id"].as_i64().unwrap();
        let on = q(&format!("select count(*) from t_diagramobjects where diagram_id = {diagram}"))[0][0].clone();
        crate::ea::run_batch(&cfg, &[serde_json::json!({"op":"delete","diagram":diagram})], &crate::ea::BatchOptions::default()).expect("cleanup");
        crate::ea::shutdown();
        println!("{} elements placed, {on} on the diagram", ops.len() - 2);
        assert_eq!(on.parse::<usize>().unwrap(), ops.len() - 2);
    }
}

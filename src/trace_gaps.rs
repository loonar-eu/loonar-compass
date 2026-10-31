//! Gaps in traceability (LC-378, /trace-gaps): requirements no design element realises, and design elements that no
//! requirement leads to – also through several steps (a requirement refined by another that a component realises is
//! covered). EA's matrix shows only direct relationships. Reads only.

use crate::config::AppConfig;
use crate::tools::query_rows;
use anyhow::Result;
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet, VecDeque};

pub fn definition() -> Value {
    json!({
        "name": "trace_gaps",
        "description": "Gaps in traceability in a package (with child packages) or the whole model: requirements that no design element \
realises or traces to, and design elements (components, classes, interfaces, use cases, activities, application components…) that no \
requirement leads to – through up to `depth` steps of trace-like relationships (Realisation, Trace, Dependency, Abstraction, Refine, Satisfy, \
Derive, Usage), in either direction, so requirement → requirement → component counts as covered. Reads only. Use it for /trace-gaps and \
before a release or an audit.",
        "input_schema": { "type": "object", "properties": {
            "package": { "description": "package_id or exact name (optional)" },
            "depth": { "type": "integer", "description": "Steps to follow (1–6, default 3)" }
        } }
    })
}

/// Relationship types that carry traceability (lower case; EA writes both Realisation and Realization).
const TRACE_TYPES: &[&str] = &["realisation", "realization", "trace", "dependency", "abstraction", "refine", "satisfy", "derive", "usage", "informationflow"];

/// Element types that are requirements.
fn is_requirement(object_type: &str, stereotype: &str) -> bool {
    let t = object_type.to_ascii_lowercase();
    let s = stereotype.to_ascii_lowercase();
    t == "requirement" || t == "feature" || s.contains("requirement") || s.contains("userstory") || s.contains("user story")
}

/// Element types that design or implement something.
fn is_design(object_type: &str) -> bool {
    matches!(
        object_type,
        "Component" | "Class" | "Interface" | "UseCase" | "Activity" | "Action" | "Node" | "Artifact" | "Table" | "Screen" | "Process" | "Device" | "ExecutionEnvironment"
    )
}

/// The elements reachable from `start` within `depth` steps over the edges.
fn reachable(start: i64, edges: &HashMap<i64, Vec<i64>>, depth: usize) -> HashSet<i64> {
    let mut seen = HashSet::from([start]);
    let mut queue = VecDeque::from([(start, 0usize)]);
    while let Some((node, d)) = queue.pop_front() {
        if d == depth {
            continue;
        }
        for next in edges.get(&node).into_iter().flatten() {
            if seen.insert(*next) {
                queue.push_back((*next, d + 1));
            }
        }
    }
    seen.remove(&start);
    seen
}

pub fn run(input: &Value, cfg: &AppConfig) -> Result<String> {
    let depth = input["depth"].as_u64().unwrap_or(3).clamp(1, 6) as usize;
    let (scope, title) = crate::model_usage::package_scope(cfg, &input["package"])?;
    // Every element (relationships may lead out of the package), then the ones in scope.
    let all = query_rows(cfg, "select o.object_id, o.object_type, coalesce(o.stereotype,''), o.name, p.name from t_object o join t_package p on p.package_id = o.package_id")?;
    let in_scope: HashSet<i64> = query_rows(cfg, &format!("select o.object_id from t_object o where 1 = 1 {scope}"))?.iter().filter_map(|r| r[0].parse().ok()).collect();
    let info: HashMap<i64, &Vec<String>> = all.iter().filter_map(|r| Some((r[0].parse().ok()?, r))).collect();
    let mut edges: HashMap<i64, Vec<i64>> = HashMap::new();
    for c in query_rows(cfg, "select start_object_id, end_object_id, lower(connector_type) from t_connector")? {
        if !TRACE_TYPES.contains(&c[2].as_str()) {
            continue;
        }
        let (Ok(a), Ok(b)) = (c[0].parse::<i64>(), c[1].parse::<i64>()) else { continue };
        edges.entry(a).or_default().push(b);
        edges.entry(b).or_default().push(a);
    }
    let mut uncovered = Vec::new();
    let mut unjustified = Vec::new();
    for (id, r) in &info {
        if !in_scope.contains(id) {
            continue;
        }
        let reach = reachable(*id, &edges, depth);
        if is_requirement(&r[1], &r[2]) {
            if !reach.iter().any(|o| info.get(o).is_some_and(|x| is_design(&x[1]))) {
                uncovered.push(format!("\"{}\" ({}, package \"{}\", object_id {id})", r[3], r[1], r[4]));
            }
        } else if is_design(&r[1]) && !reach.iter().any(|o| info.get(o).is_some_and(|x| is_requirement(&x[1], &x[2]))) {
            unjustified.push(format!("\"{}\" ({}, package \"{}\", object_id {id})", r[3], r[1], r[4]));
        }
    }
    uncovered.sort();
    unjustified.sort();
    let mut out = format!("Traceability gaps in {title} (up to {depth} steps over trace-like relationships):\n");
    for (heading, list) in [("Requirements no design element realises or traces to", &uncovered), ("Design elements no requirement leads to", &unjustified)] {
        out.push_str(&format!("\n{heading} ({}):\n", list.len()));
        if list.is_empty() {
            out.push_str("  none\n");
        }
        for line in list.iter().take(150) {
            out.push_str(&format!("  - {line}\n"));
        }
        if list.len() > 150 {
            out.push_str(&format!("  … and {} more\n", list.len() - 150));
        }
    }
    out.push_str("\nNothing was changed.\n");
    Ok(out)
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    #[test]
    fn paths_over_several_steps() {
        // requirement 1 → requirement 2 → component 3; 4 alone.
        let edges: HashMap<i64, Vec<i64>> = HashMap::from([(1, vec![2]), (2, vec![1, 3]), (3, vec![2])]);
        assert!(super::reachable(1, &edges, 2).contains(&3));
        assert!(!super::reachable(1, &edges, 1).contains(&3), "one step is not enough");
        assert!(super::reachable(4, &edges, 3).is_empty());
        assert!(super::is_requirement("Requirement", "") && super::is_requirement("Class", "EAUML::UserStory") && !super::is_requirement("Class", ""));
    }

    /// LC-378: on a model copy: `LOONAR_TEST_PROPS=E:/livetest/undo/undo.properties cargo test trace_gaps_live -- --ignored --nocapture`
    #[test]
    #[ignore = "requires the model from LOONAR_TEST_PROPS"]
    fn trace_gaps_live() {
        let cfg = crate::config::AppConfig::load(std::path::Path::new(&std::env::var("LOONAR_TEST_PROPS").expect("LOONAR_TEST_PROPS"))).expect("app.properties");
        let report = super::run(&serde_json::json!({}), &cfg).expect("trace_gaps");
        println!("{}", report.lines().take(12).collect::<Vec<_>>().join("\n"));
        assert!(report.starts_with("Traceability gaps in the whole model"));
    }
}

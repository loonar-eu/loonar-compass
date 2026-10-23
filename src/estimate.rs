//! From the impact of a change to an estimate: the elements to change, the elements related to them (to adapt) and
//! further ones (to check) become WBS rows in plain language, with hours by element type and level, plus the diagrams
//! to update, tests, analysis and documentation and a risk buffer. Norms can be adjusted in `estimate.json` next to
//! the settings: {"hours": {"Class": 4, "Component": 16}, "levels": [1.0, 0.5, 0.25], "tests": 0.3, "analysis": 0.15,
//! "diagram": 1, "hours_per_day": 8}.

use crate::config::AppConfig;
use crate::tools::query_rows;
use anyhow::{bail, Result};
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap, HashSet};

fn default_hours(kind: &str) -> f64 {
    match kind {
        "Component" | "Node" | "Device" | "ExecutionEnvironment" => 16.0,
        "Interface" => 6.0,
        "UseCase" | "Screen" | "GUIElement" => 8.0,
        "Class" | "Table" | "Activity" | "Action" | "Process" => 4.0,
        "Requirement" | "Feature" | "Issue" | "Change" => 2.0,
        "Actor" | "Event" | "Decision" => 1.0,
        _ => 3.0,
    }
}

struct Norms {
    hours: HashMap<String, f64>,
    levels: Vec<f64>,
    tests: f64,
    analysis: f64,
    diagram: f64,
    per_day: f64,
}

fn norms(cfg: &AppConfig) -> Norms {
    let v: Value = std::fs::read_to_string(cfg.base_dir.join("estimate.json")).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or(Value::Null);
    Norms {
        hours: v["hours"].as_object().map(|o| o.iter().filter_map(|(k, v)| v.as_f64().map(|h| (k.clone(), h))).collect()).unwrap_or_default(),
        levels: v["levels"].as_array().map(|a| a.iter().filter_map(|x| x.as_f64()).collect()).filter(|l: &Vec<f64>| !l.is_empty()).unwrap_or_else(|| vec![1.0, 0.5, 0.25]),
        tests: v["tests"].as_f64().unwrap_or(0.3),
        analysis: v["analysis"].as_f64().unwrap_or(0.15),
        diagram: v["diagram"].as_f64().unwrap_or(1.0),
        per_day: v["hours_per_day"].as_f64().unwrap_or(8.0),
    }
}

pub fn estimate(cfg: &AppConfig, elements: &[i64], depth: usize) -> Result<String> {
    if elements.is_empty() {
        bail!("{}", crate::i18n::tr("give the elements the change is about (object_id)"));
    }
    let n = norms(cfg);
    let depth = depth.clamp(0, n.levels.len().saturating_sub(1).max(1));
    // Level of each element: 0 = changed, 1 = related (to adapt), 2+ = further (to check).
    let mut level: HashMap<i64, usize> = elements.iter().map(|e| (*e, 0)).collect();
    let mut frontier: Vec<i64> = elements.to_vec();
    for l in 1..=depth {
        if frontier.is_empty() {
            break;
        }
        let ids = frontier.iter().map(|i| i.to_string()).collect::<Vec<_>>().join(",");
        let rows = query_rows(cfg, &format!(
            "select c.end_object_id as c0 from t_connector c where c.start_object_id in ({ids}) union select c.start_object_id as c0 from t_connector c where c.end_object_id in ({ids})"
        ))?;
        let mut next = Vec::new();
        for r in rows {
            if let Ok(id) = r[0].parse::<i64>() {
                if id > 0 && !level.contains_key(&id) {
                    level.insert(id, l);
                    next.push(id);
                }
            }
        }
        frontier = next;
        if level.len() > 400 {
            break;
        }
    }
    let ids = level.keys().map(|i| i.to_string()).collect::<Vec<_>>().join(",");
    let info: HashMap<i64, Vec<String>> = query_rows(cfg, &format!(
        "select o.object_id as c0, coalesce(o.name,'') as c1, o.object_type as c2, coalesce(o.stereotype,'') as c3, coalesce(p.name,'') as c4, coalesce(o.note,'') as c5, \
         (select count(*) from t_connector c where c.start_object_id = o.object_id or c.end_object_id = o.object_id) as c6 \
         from t_object o left join t_package p on p.package_id = o.package_id where o.object_id in ({ids})"
    ))?
    .into_iter()
    .filter_map(|r| r[0].parse::<i64>().ok().map(|id| (id, r)))
    .collect();
    for e in elements {
        if !info.contains_key(e) {
            bail!("{}", crate::i18n::fmt("there is no element with id {0}", &[e.to_string()]));
        }
    }
    let skip = ["Package", "Note", "Text", "Boundary", "Port", "Decision", "MergeNode", "Synchronization", "InitialNode", "ActivityFinal", "FlowFinal", "StateNode", "ActionPin"];
    let mut rows: Vec<(usize, String, String, String, String, f64)> = Vec::new();
    for (id, l) in &level {
        let Some(r) = info.get(id) else { continue };
        if skip.contains(&r[2].as_str()) {
            continue;
        }
        let base = n.hours.get(&r[2]).copied().or_else(|| n.hours.get(&r[3]).copied()).unwrap_or_else(|| default_hours(&r[2]));
        let factor = n.levels.get(*l).copied().unwrap_or(0.25);
        let task = match l {
            0 => crate::i18n::fmt("Change: {0}", &[r[1].clone()]),
            1 => crate::i18n::fmt("Adapt: {0}", &[r[1].clone()]),
            _ => crate::i18n::fmt("Check the impact on: {0}", &[r[1].clone()]),
        };
        let kind = if r[3].is_empty() { r[2].clone() } else { format!("{} «{}»", r[2], r[3]) };
        rows.push((*l, task, kind, r[4].clone(), id.to_string(), (base * factor * 2.0).round() / 2.0));
    }
    rows.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
    let diagrams = query_rows(cfg, &format!("select distinct d.diagram_id as c0, coalesce(d.name,'') as c1 from t_diagramobjects x join t_diagram d on d.diagram_id = x.diagram_id where x.object_id in ({})", elements.iter().map(|i| i.to_string()).collect::<Vec<_>>().join(",")))?;
    let work: f64 = rows.iter().map(|r| r.5).sum::<f64>() + diagrams.len() as f64 * n.diagram;
    let tests = (work * n.tests * 2.0).round() / 2.0;
    let analysis = (work * n.analysis * 2.0).round() / 2.0;
    let packages: HashSet<&String> = rows.iter().map(|r| &r.3).collect();
    // Risks: elements with many relationships, without a description, a change across many packages.
    let mut risks = Vec::new();
    for (id, l) in &level {
        if let Some(r) = info.get(id) {
            let links: usize = r[6].parse().unwrap_or(0);
            if *l == 0 && links >= 8 {
                risks.push(crate::i18n::fmt("\"{0}\" has {1} relationships: the change reaches far", &[r[1].clone(), links.to_string()]));
            }
            if *l <= 1 && r[5].trim().is_empty() && !skip.contains(&r[2].as_str()) {
                risks.push(crate::i18n::fmt("\"{0}\" has no description: its behaviour has to be clarified first", &[r[1].clone()]));
            }
        }
    }
    if packages.len() >= 3 {
        risks.push(crate::i18n::fmt("the change touches {0} packages (areas): coordinate with their owners", &[packages.len().to_string()]));
    }
    let buffer_share = (0.1 + 0.05 * risks.len().min(4) as f64).min(0.3);
    let buffer = ((work + tests + analysis) * buffer_share * 2.0).round() / 2.0;
    let total = work + tests + analysis + buffer;
    let mut out = vec![crate::i18n::fmt(
        "Estimate from the impact of the change: {0} elements to change, {1} related, {2} further; {3} diagrams. Total {4} h = {5} person-days (work {6} h, tests {7} h, analysis and documentation {8} h, risk buffer {9} h = {10}%).",
        &[
            level.values().filter(|l| **l == 0).count().to_string(),
            level.values().filter(|l| **l == 1).count().to_string(),
            level.values().filter(|l| **l >= 2).count().to_string(),
            diagrams.len().to_string(),
            format!("{total:.1}"), format!("{:.1}", total / n.per_day), format!("{work:.1}"), format!("{tests:.1}"), format!("{analysis:.1}"), format!("{buffer:.1}"), format!("{:.0}", buffer_share * 100.0),
        ],
    )];
    out.push("# | task | type | package | element id | hours".into());
    let mut i = 0;
    for r in &rows {
        i += 1;
        out.push(format!("{i} | {} | {} | {} | {} | {:.1}", r.1, r.2, r.3, r.4, r.5));
    }
    for d in &diagrams {
        i += 1;
        out.push(format!("{i} | {} | Diagram | | | {:.1}", crate::i18n::fmt("Update the diagram: {0}", &[d[1].clone()]), n.diagram));
    }
    for (task, h) in [(crate::i18n::tr("Tests of the change"), tests), (crate::i18n::tr("Analysis and documentation"), analysis), (crate::i18n::tr("Risk buffer"), buffer)] {
        i += 1;
        out.push(format!("{i} | {task} | | | | {h:.1}"));
    }
    let by_kind: BTreeMap<&String, f64> = rows.iter().fold(BTreeMap::new(), |mut m, r| {
        *m.entry(&r.2).or_insert(0.0) += r.5;
        m
    });
    out.push(format!("\n{}: {}", crate::i18n::tr("Hours by type"), by_kind.iter().map(|(k, v)| format!("{k} {v:.1}")).collect::<Vec<_>>().join(", ")));
    if !risks.is_empty() {
        out.push(format!("\n{}:", crate::i18n::tr("Risks")));
        out.extend(risks.into_iter().take(15).map(|r| format!("- {r}")));
    }
    out.push(crate::i18n::tr("This is a model-based baseline: adjust the hours with the team's knowledge; the norms are in estimate.json.").to_string());
    Ok(out.join("\n"))
}

pub fn definition() -> Value {
    json!({
        "name": "estimate_change",
        "description": "Estimate of a change from its impact in the model: the elements to change (object_id) and, along relationships, the related elements \
(to adapt) and further ones (to check) become WBS rows in plain functional language with hours by element type, plus diagrams to update, tests, analysis \
and documentation and a risk buffer; also risks (many relationships, no description, many packages). Present the WBS as a table; on request save it to Excel \
(create_spreadsheet). The norms come from estimate.json next to the settings (default when missing).",
        "input_schema": { "type": "object", "properties": {
            "elements": { "type": "array", "items": { "type": "integer" } },
            "depth": { "type": "integer", "description": "Levels of relationships to follow (default 2)." }
        }, "required": ["elements"] }
    })
}

#[cfg(test)]
mod tests {
    /// Estimate on a test model (LOONAR_TEST_PROPS): the class Product of the shop model. `cargo test estimate_on_model -- --ignored --nocapture`
    #[test]
    #[ignore = "requires a test model"]
    fn estimate_on_model() {
        let cfg = crate::config::AppConfig::load(std::path::Path::new(&std::env::var("LOONAR_TEST_PROPS").unwrap())).unwrap();
        crate::i18n::set(crate::i18n::Lang::Pl);
        let id: i64 = crate::tools::query_rows(&cfg, "select object_id from t_object where name = 'Product' and object_type = 'Class'").unwrap()[0][0].parse().unwrap();
        let out = super::estimate(&cfg, &[id], 2).unwrap();
        println!("{out}");
        assert!(out.contains("Zmiana: Product") && out.contains("Dostosowanie:") && out.contains("Testy zmiany") && out.contains("osobodni"));
    }
}

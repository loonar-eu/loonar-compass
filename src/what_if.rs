//! "What if" (LC-404, /what-if): the consequences of a planned change before anything is saved – deleting or retiring
//! an element, changing its type, or moving it to another package: its relationships and what depends on it (also
//! through several steps), the diagrams and published pages that show it, the requirements it serves, the relationships
//! the new type would break (UML and ArchiMate rules), and the team's creation rules of the target package. Reads only.

use crate::config::AppConfig;
use crate::tools::query_rows;
use anyhow::{bail, Context, Result};
use serde_json::{json, Value};

pub fn definition() -> Value {
    json!({
        "name": "what_if",
        "description": "The consequences of a planned change, before it is made (nothing is saved): change delete, retire (status Retired/\
Deprecated), change_type (to_type, optional to_stereotype) or move (to_package) of an element: its relationships and what depends on it (impact \
through up to 3 steps), the diagrams and published pages (Confluence, Word) that show it, the requirements it realises or traces to, \
relationships that the new type or stereotype would break (UML and ArchiMate 3.2 rules), and the team's creation rules of the target package. \
Use it for /what-if and before a deletion, a retirement or a restructuring.",
        "input_schema": { "type": "object", "properties": {
            "element": { "description": "object_id or exact name" },
            "change": { "type": "string", "enum": ["delete", "retire", "change_type", "move"] },
            "to_type": { "type": "string" },
            "to_stereotype": { "type": "string" },
            "to_package": { "description": "move: package_id or exact name" }
        }, "required": ["element", "change"] }
    })
}

pub fn run(input: &Value, cfg: &AppConfig) -> Result<String> {
    let e = &input["element"];
    let rows = match e.as_i64().or_else(|| e.as_str().and_then(|s| s.trim().parse().ok())) {
        Some(id) => query_rows(cfg, &format!("select object_id, name, object_type, coalesce(stereotype,''), package_id from t_object where object_id = {id}"))?,
        None => query_rows(cfg, &format!("select object_id, name, object_type, coalesce(stereotype,''), package_id from t_object where name = '{}'", crate::model_db::sql_text(cfg.model_source, e.as_str().unwrap_or(""))))?,
    };
    let el = match rows.as_slice() {
        [] => bail!("there is no element {e}"),
        [one] => one.clone(),
        // Several of that name: each one weighed, so the answer rests on the tool's numbers (LC-461: after an error the
        // assistant counted itself, in wrong Polish).
        many => {
            let mut parts = vec![crate::i18n::fmt("Several elements are named {0}: each one below (ask which one is meant).", &[e.to_string()])];
            for r in many.iter().take(5) {
                let mut one = input.clone();
                one["element"] = serde_json::json!(r[0].parse::<i64>().unwrap_or(0));
                parts.push(run(&one, cfg)?);
            }
            return Ok(parts.join("\n\n---\n\n"));
        }
    };
    let id: i64 = el[0].parse().unwrap_or(0);
    let change = input["change"].as_str().unwrap_or("delete");
    let mut out = vec![format!("What if {} \"{}\" ({}{}, id {id}) is {}:", "the element", el[1], el[2], if el[3].is_empty() { String::new() } else { format!(" «{}»", el[3]) }, match change {
        "retire" => "retired".to_string(),
        "change_type" => format!("changed to {}{}", input["to_type"].as_str().unwrap_or("?"), input["to_stereotype"].as_str().map(|s| format!(" «{s}»")).unwrap_or_default()),
        "move" => format!("moved to {}", input["to_package"]),
        _ => "deleted".to_string(),
    })];
    // Relationships, in and out.
    let links = query_rows(cfg, &format!(
        "select c.connector_type, coalesce(c.stereotype,''), case when c.start_object_id = {id} then 'out' else 'in' end, o.object_id, o.name, o.object_type, coalesce(o.stereotype,'') \
         from t_connector c join t_object o on o.object_id = case when c.start_object_id = {id} then c.end_object_id else c.start_object_id end \
         where c.start_object_id = {id} or c.end_object_id = {id}"
    ))?;
    let dependants: Vec<&Vec<String>> = links.iter().filter(|l| l[2] == "in" && matches!(l[0].as_str(), "Dependency" | "Usage" | "Association" | "InformationFlow" | "Realisation" | "Realization")).collect();
    out.push(format!("\nRelationships: {} ({} pointing at it, e.g. {})", links.len(), dependants.len(),
        dependants.iter().take(6).map(|l| format!("{}{} \"{}\"", l[0], if l[1].is_empty() { String::new() } else { format!(" «{}»", l[1]) }, l[4])).collect::<Vec<_>>().join(", ")));
    if matches!(change, "delete" | "retire") {
        let impact = crate::model_db::impact_analysis(&json!({ "element": id, "depth": 3, "direction": "in" }), cfg).unwrap_or_default();
        let lines: Vec<&str> = impact.lines().skip(1).filter(|l| !l.trim().is_empty() && !l.trim_start().starts_with("level |")).collect();
        out.push(format!("What depends on it, up to 3 steps (these elements stay in the model and on their diagrams; only their link to it is affected): {} element(s){}", lines.len(), if lines.is_empty() { String::new() } else { format!("\n  {}", lines.iter().take(12).cloned().collect::<Vec<_>>().join("\n  ")) }));
    }
    // Diagrams and publications.
    let diagrams = query_rows(cfg, &format!("select d.diagram_id, d.name from t_diagramobjects x join t_diagram d on d.diagram_id = x.diagram_id where x.object_id = {id} order by d.name"))?;
    out.push(format!("Diagrams that show it (only this element would be gone from them): {}{}", diagrams.len(), if diagrams.is_empty() { String::new() } else { format!(" ({})", diagrams.iter().take(10).map(|d| format!("\"{}\"", d[1])).collect::<Vec<_>>().join(", ")) }));
    // A ready sentence with the numbers in the user's language and grammar (LC-461: the assistant wrote "zniknie 4
    // elementy" and that the related elements would vanish from the diagram).
    if matches!(change, "delete" | "retire") {
        let (r, d) = (links.len(), diagrams.len());
        let sentence = if crate::i18n::current() == crate::i18n::Lang::Pl {
            let few = |n: usize| (2..=4).contains(&(n % 10)) && !(12..=14).contains(&(n % 100));
            let pl = |n: usize, one: &str, some: &str, many: &str| format!("{n} {}", if n == 1 { one } else if few(n) { some } else { many });
            format!(
                "Po usunięciu „{}” {} razem z nim {}; element zniknie z {}; pozostałe elementy zostają w modelu i na diagramach.",
                el[1], if few(r) { "znikną" } else { "zniknie" }, pl(r, "relacja", "relacje", "relacji"), pl(d, "diagramu", "diagramów", "diagramów")
            )
        } else {
            format!("If \"{}\" is deleted, its {r} relationship(s) go with it; it disappears from {d} diagram(s); the other elements stay in the model and on the diagrams.", el[1])
        };
        out.push(format!("Summary to give the user as it is: {sentence}"));
    }
    let ids: Vec<i64> = diagrams.iter().filter_map(|d| d[0].parse().ok()).collect();
    let published: Vec<String> = std::fs::read_to_string(cfg.base_dir.join("publications.json")).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok())
        .and_then(|v| v.as_array().cloned()).unwrap_or_default().into_iter()
        .filter(|p| p["diagrams"].as_array().into_iter().flatten().filter_map(Value::as_i64).any(|d| ids.contains(&d)))
        .map(|p| format!("{} \"{}\"", p["kind"].as_str().unwrap_or(""), p["title"].as_str().unwrap_or(""))).collect();
    if !published.is_empty() {
        out.push(format!("Published pages and documents that show it (they would be out of date): {}", published.join(", ")));
    }
    // Requirements.
    let requirements: Vec<String> = links.iter().filter(|l| l[5] == "Requirement" || l[6].to_lowercase().contains("requirement")).map(|l| format!("\"{}\"", l[4])).collect();
    if !requirements.is_empty() {
        out.push(format!("Requirements it is linked to (they would lose this link): {}", requirements.join(", ")));
    }
    // A new type: the relationships it would break.
    if change == "change_type" {
        let to_type = input["to_type"].as_str().context("to_type")?;
        let to_stereo = input["to_stereotype"].as_str().unwrap_or(&el[3]);
        let mut broken = Vec::new();
        for l in &links {
            let (s_type, s_st, t_type, t_st) = if l[2] == "out" { (to_type, to_stereo, l[5].as_str(), l[6].as_str()) } else { (l[5].as_str(), l[6].as_str(), to_type, to_stereo) };
            if let Some(p) = crate::governance::relation_problem(&l[0], &l[1], s_type, s_st, t_type, t_st) {
                broken.push(p);
            }
        }
        out.push(if broken.is_empty() { "Relationships the new type would break: none by the UML and ArchiMate rules.".into() } else { format!("Relationships the new type would break ({}):\n  {}", broken.len(), broken.join("\n  ")) });
    }
    // A move: the creation rules of the target.
    if change == "move" {
        let (scope, title) = crate::model_usage::package_scope(cfg, &input["to_package"])?;
        let target: i64 = scope.split('(').nth(1).and_then(|s| s.split([',', ')']).next()).and_then(|p| p.parse().ok()).context("to_package")?;
        match crate::governance::creation_block(cfg, "element_creating", target, &el[2], &el[3]) {
            Some(reason) => out.push(format!("The team's rules for the {title}: {reason}")),
            None => out.push(format!("The team's rules allow it in the {title}.")),
        }
        if el[4].parse::<i64>().ok() == Some(target) {
            out.push("It is already in that package.".into());
        }
    }
    if change == "delete" {
        out.push("A deletion takes its relationships with it; /to-bin puts it into the recycle bin instead, where it can be restored.".into());
    }
    Ok(out.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `LOONAR_TEST_PROPS=… cargo test what_if_live -- --ignored --nocapture`
    #[test]
    #[ignore = "requires a test model"]
    fn what_if_live() {
        let cfg = AppConfig::load(std::path::Path::new(&std::env::var("LOONAR_TEST_PROPS").unwrap())).unwrap();
        let busy: i64 = query_rows(&cfg, "select o.object_id from t_object o join t_connector c on c.end_object_id = o.object_id where o.object_type = 'Class' group by o.object_id order by count(*) desc").unwrap()[0][0].parse().unwrap();
        for change in [json!({ "change": "delete" }), json!({ "change": "change_type", "to_type": "Activity" }), json!({ "change": "move", "to_package": 1 })] {
            let mut input = change.clone();
            input["element"] = json!(busy);
            match run(&input, &cfg) {
                Ok(text) => println!("{text}\n"),
                Err(e) => println!("{change}: {e:#}\n"),
            }
        }
        assert!(run(&json!({ "element": busy, "change": "delete" }), &cfg).unwrap().contains("Relationships:"));
    }
}

#[cfg(test)]
mod same_name_tests {
    /// LC-461, LC-471: a name used twice is weighed for each element with the ready summary; a diagram by name lists its
    /// hidden relationships. `LOONAR_TEST_PROPS=E:/livetest/r440/m5.properties cargo test same_name_live -- --ignored --nocapture`
    #[test]
    #[ignore = "requires the model from LOONAR_TEST_PROPS"]
    fn same_name_live() {
        let cfg = crate::config::AppConfig::load(std::path::Path::new(&std::env::var("LOONAR_TEST_PROPS").unwrap())).unwrap();
        let out = super::run(&serde_json::json!({ "element": "Faktura", "change": "delete" }), &cfg).unwrap();
        println!("{out}");
        assert!(out.matches("Summary to give the user").count() >= 2, "{out}");
        let hidden = crate::visibility::run(&serde_json::json!({ "action": "hidden", "diagram": "BB3 Logiczny" }), &cfg).unwrap().1;
        println!("{hidden}");
        assert!(hidden.contains("BB3 Logiczny"));
    }
}

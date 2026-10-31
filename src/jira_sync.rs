//! Two-way sync of model elements and Jira issues (LC-403, /jira-sync): an element is linked to an issue by a tagged
//! value (Jira = ARCH-123); its name and status are compared with the issue's summary and status, with a map between
//! Jira's statuses and the model's. The differences are shown first; the model is changed only through ea_write (the
//! approval card and undo) and Jira only after the user approves the list – nothing is overwritten without asking.

use crate::config::AppConfig;
use crate::jira::Connection;
use crate::tools::query_rows;
use anyhow::{bail, Result};
use serde_json::{json, Value};

pub fn definition() -> Value {
    json!({
        "name": "jira_sync",
        "description": "Two-way sync of model elements and Jira issues. An element is linked to an issue by a tagged value (default Jira = KEY, \
e.g. Jira = ARCH-123; link one with ea_write update_element tags). action check: the linked elements (of a package, or the given elements) \
whose name or status differs from the issue's summary or status, and the keys Jira does not know – nothing is changed. action to_model: the \
model takes Jira's summary and/or status (fields), saved after the user approves the card. action to_jira: Jira takes the model's names and/or \
statuses (the issue is moved to that status when its workflow allows), after the user approves the list. status_map maps Jira statuses to \
model statuses, e.g. {\"To Do\": \"Proposed\", \"In Progress\": \"Approved\", \"Done\": \"Implemented\"}. Always run check first and ask \
which way. Use it for /jira-sync.",
        "input_schema": { "type": "object", "properties": {
            "action": { "type": "string", "enum": ["check", "to_model", "to_jira"] },
            "package": { "description": "package_id or exact name (with its child packages)" },
            "elements": { "type": "array", "items": { "type": "integer" }, "description": "Only these elements" },
            "tag": { "type": "string", "description": "The tagged value with the issue key (default Jira)" },
            "fields": { "type": "array", "items": { "type": "string", "enum": ["name", "status"] }, "description": "What to sync (default both)" },
            "status_map": { "type": "object", "description": "Jira status → model status" }
        }, "required": ["action"] }
    })
}

/// An element linked to an issue, and what the issue says.
#[derive(Debug, Clone)]
pub struct Linked {
    pub element: i64,
    pub name: String,
    pub status: String,
    pub key: String,
    /// The issue's summary and status; None when Jira does not know the key.
    pub issue: Option<(String, String)>,
}

/// The model status a Jira status stands for (the map, else the same word).
fn model_status(jira: &str, map: &Value) -> String {
    map.as_object()
        .and_then(|m| m.iter().find(|(k, _)| k.eq_ignore_ascii_case(jira)))
        .and_then(|(_, v)| v.as_str())
        .unwrap_or(jira)
        .to_string()
}

/// The Jira status for a model status: the first one the map sends to it, else the same word.
fn jira_status(model: &str, map: &Value) -> String {
    map.as_object()
        .and_then(|m| m.iter().find(|(_, v)| v.as_str().is_some_and(|v| v.eq_ignore_ascii_case(model))))
        .map(|(k, _)| k.clone())
        .unwrap_or_else(|| model.to_string())
}

impl Linked {
    pub fn name_differs(&self) -> bool {
        self.issue.as_ref().is_some_and(|(summary, _)| summary.trim() != self.name.trim())
    }

    pub fn status_differs(&self, map: &Value) -> bool {
        self.issue.as_ref().is_some_and(|(_, status)| !model_status(status, map).eq_ignore_ascii_case(self.status.trim()))
    }
}

/// The elements with an issue key in the tag (of a package or the given ones), each with its issue read from Jira.
pub fn linked(cfg: &AppConfig, input: &Value, jira: &Connection) -> Result<Vec<Linked>> {
    let tag = crate::model_db::sql_text(cfg.model_source, &input["tag"].as_str().map(str::trim).filter(|t| !t.is_empty()).unwrap_or("Jira").to_lowercase());
    let mut filter = String::new();
    if let Some(ids) = input["elements"].as_array().filter(|a| !a.is_empty()) {
        filter = format!(" and o.object_id in ({})", ids.iter().filter_map(Value::as_i64).map(|i| i.to_string()).collect::<Vec<_>>().join(","));
    } else if !input["package"].is_null() {
        filter = crate::model_usage::package_scope(cfg, &input["package"])?.0;
    }
    let rows = query_rows(cfg, &format!(
        "select o.object_id, o.name, coalesce(o.status,''), p.value from t_objectproperties p join t_object o on o.object_id = p.object_id \
         where lower(p.property) = '{tag}' and p.value is not null {filter} order by o.name"
    ))?;
    let mut out = Vec::new();
    for r in rows.into_iter().take(300) {
        let key = r[3].trim().to_uppercase();
        if crate::jira::check_issue_key(&key).is_err() {
            continue;
        }
        let issue = jira.summary_status(&key)?;
        out.push(Linked { element: r[0].parse().unwrap_or(0), name: r[1].clone(), status: r[2].clone(), key, issue });
    }
    Ok(out)
}

fn wants(input: &Value, field: &str) -> bool {
    input["fields"].as_array().filter(|f| !f.is_empty()).is_none_or(|f| f.iter().any(|v| v == field))
}

/// The differences in words.
pub fn report(links: &[Linked], input: &Value) -> String {
    let map = &input["status_map"];
    if links.is_empty() {
        return "No element here has an issue key in its tagged value (link one with ea_write: update_element tags {\"Jira\": \"KEY-1\"}).".into();
    }
    let missing: Vec<&Linked> = links.iter().filter(|l| l.issue.is_none()).collect();
    let differ: Vec<&Linked> = links.iter().filter(|l| wants(input, "name") && l.name_differs() || wants(input, "status") && l.status_differs(map)).collect();
    let mut out = vec![format!("{} linked element(s): {} in sync, {} differ, {} with a key Jira does not know. Nothing was changed.",
        links.len(), links.len() - differ.len() - missing.len(), differ.len(), missing.len())];
    for l in &differ {
        let (summary, status) = l.issue.clone().unwrap_or_default();
        let mut parts = Vec::new();
        if wants(input, "name") && l.name_differs() {
            parts.push(format!("name \"{}\" / Jira \"{summary}\"", l.name));
        }
        if wants(input, "status") && l.status_differs(map) {
            parts.push(format!("status {} / Jira {status}{}", if l.status.is_empty() { "(none)" } else { &l.status }, if model_status(&status, map) != status { format!(" (= {})", model_status(&status, map)) } else { String::new() }));
        }
        out.push(format!("- {} (id {}) ↔ {}: {}", l.name, l.element, l.key, parts.join("; ")));
    }
    for l in missing {
        out.push(format!("- {} (id {}): Jira does not know {}", l.name, l.element, l.key));
    }
    if !differ.is_empty() {
        out.push("Ask which way: to_model (the model takes Jira's) or to_jira (Jira takes the model's).".into());
    }
    out.join("\n")
}

/// The model changes that take Jira's summary and status.
pub fn to_model_ops(links: &[Linked], input: &Value) -> Vec<Value> {
    let map = &input["status_map"];
    links.iter().filter_map(|l| {
        let (summary, status) = l.issue.clone()?;
        let mut op = json!({ "op": "update_element", "element": l.element });
        if wants(input, "name") && l.name_differs() {
            op["name"] = json!(summary.trim());
        }
        if wants(input, "status") && l.status_differs(map) {
            op["status"] = json!(model_status(&status, map));
        }
        (op.as_object().is_some_and(|o| o.len() > 2)).then_some(op)
    }).collect()
}

/// The Jira changes that take the model's names and statuses: (key, new summary, new status), with a line for the approval.
pub fn to_jira_changes(links: &[Linked], input: &Value) -> Vec<(String, Option<String>, Option<String>, String)> {
    let map = &input["status_map"];
    links.iter().filter_map(|l| {
        let (summary, status) = l.issue.clone()?;
        let name = (wants(input, "name") && l.name_differs()).then(|| l.name.trim().to_string());
        let wanted = (wants(input, "status") && l.status_differs(map) && !l.status.trim().is_empty()).then(|| jira_status(l.status.trim(), map));
        if name.is_none() && wanted.is_none() {
            return None;
        }
        let mut line = format!("{}:", l.key);
        if let Some(n) = &name {
            line.push_str(&format!(" summary \"{summary}\" → \"{n}\""));
        }
        if let Some(s) = &wanted {
            line.push_str(&format!(" status {status} → {s}"));
        }
        Some((l.key.clone(), name, wanted, line))
    }).collect()
}

/// Sends the changes to Jira: each issue on its own, a failure (e.g. a status the workflow does not allow) reported.
pub fn apply_to_jira(jira: &Connection, changes: &[(String, Option<String>, Option<String>, String)]) -> String {
    let mut out = Vec::new();
    for (key, name, status, line) in changes {
        let mut result = Ok(());
        if let Some(n) = name {
            result = jira.update_issue(key, Some(n), None, &[], &[]);
        }
        if let (Ok(()), Some(s)) = (&result, status) {
            result = jira.set_status(key, s);
        }
        out.push(match result {
            Ok(()) => format!("✓ {line}"),
            Err(e) => format!("✗ {line} – {e}"),
        });
    }
    out.join("\n")
}

pub fn check_action(input: &Value) -> Result<&str> {
    match input["action"].as_str().unwrap_or("check") {
        a @ ("check" | "to_model" | "to_jira") => Ok(a),
        other => bail!("unknown action {other}: check, to_model or to_jira"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn link(name: &str, status: &str, issue: Option<(&str, &str)>) -> Linked {
        Linked { element: 1, name: name.into(), status: status.into(), key: "ARCH-7".into(), issue: issue.map(|(a, b)| (a.into(), b.into())) }
    }

    #[test]
    fn differences_both_ways() {
        let map = json!({ "To Do": "Proposed", "In Progress": "Approved", "Done": "Implemented" });
        let input = json!({ "status_map": map });
        let links = vec![
            link("Online payments", "Approved", Some(("Online payments", "In Progress"))),
            link("Card payments", "Proposed", Some(("Online payments", "In Progress"))),
            link("Gone", "Proposed", None),
        ];
        let text = report(&links, &input);
        assert!(text.starts_with("3 linked element(s): 1 in sync, 1 differ, 1 with a key Jira does not know"), "{text}");
        assert!(text.contains("name \"Card payments\" / Jira \"Online payments\"; status Proposed / Jira In Progress (= Approved)"), "{text}");
        let ops = to_model_ops(&links, &input);
        assert_eq!(ops, vec![json!({ "op": "update_element", "element": 1, "name": "Online payments", "status": "Approved" })]);
        let only_status = to_model_ops(&links, &json!({ "status_map": map, "fields": ["status"] }));
        assert!(only_status[0]["name"].is_null() && only_status[0]["status"] == "Approved");
        let changes = to_jira_changes(&links, &input);
        assert_eq!(changes.len(), 1);
        assert_eq!((changes[0].1.as_deref(), changes[0].2.as_deref()), (Some("Card payments"), Some("To Do")));
    }

    /// On a model COPY against the Jira imitation: linked elements found by their tag, the model takes Jira's.
    /// `LOONAR_TEST_PROPS=E:/livetest/undo/undo.properties cargo test jira_sync_live -- --ignored --nocapture`
    #[test]
    #[ignore = "requires EA and a copy of a test model"]
    fn jira_sync_live() {
        let cfg = AppConfig::load(std::path::Path::new(&std::env::var("LOONAR_TEST_PROPS").unwrap())).unwrap();
        let mock = crate::jira::mock::start(false, "sync-token-2");
        let jira = Connection { base_url: mock.url.clone(), user: String::new(), token: "sync-token-2".into(), identity: None };
        let batch = |ops: &[Value]| crate::ea::run_batch(&cfg, ops, &crate::ea::BatchOptions::default()).expect("batch");
        let root: i64 = query_rows(&cfg, "select package_id from t_package where parent_id = 0 order by package_id").unwrap()[0][0].parse().unwrap();
        let made = batch(&[
            json!({"op":"create_package","key":"pkg","parent":root,"name":format!("ZZ_jira_{}", chrono::Local::now().format("%H%M%S"))}),
            json!({"op":"create_element","key":"a","package":"#pkg","type":"Requirement","name":"Card payments","status":"Proposed","tags":{"Jira":"arch-7"}}),
            json!({"op":"create_element","key":"b","package":"#pkg","type":"Requirement","name":"Old one","tags":{"Jira":"ARCH-404"}}),
            json!({"op":"create_element","key":"c","package":"#pkg","type":"Requirement","name":"Not linked"}),
        ]);
        let id_of = |k: &str| made["created"].as_array().unwrap().iter().find(|c| c["key"] == k).unwrap()["id"].as_i64().unwrap();
        let (pkg, a) = (id_of("pkg"), id_of("a"));
        let input = json!({ "package": pkg, "status_map": { "In Progress": "Approved" } });
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let links = linked(&cfg, &input, &jira).unwrap();
            let text = report(&links, &input);
            println!("{text}");
            batch(&to_model_ops(&links, &input));
            let after = query_rows(&cfg, &format!("select name, status from t_object where object_id = {a}")).unwrap();
            (text, after, report(&linked(&cfg, &input, &jira).unwrap(), &input))
        }));
        batch(&[json!({"op":"delete","package":pkg})]);
        crate::ea::shutdown();
        let (text, after, again) = result.unwrap();
        assert!(text.starts_with("2 linked element(s): 0 in sync, 1 differ, 1 with a key Jira does not know"), "{text}");
        assert_eq!(after, vec![vec!["Online payments".to_string(), "Approved".to_string()]]);
        assert!(again.starts_with("2 linked element(s): 1 in sync, 0 differ"), "{again}");
    }

    /// Against the Jira imitation: the summary changed and the issue moved through its workflow.
    #[test]
    fn to_jira_through_the_workflow() {
        let mock = crate::jira::mock::start(false, "sync-token-1");
        let jira = Connection { base_url: mock.url.clone(), user: String::new(), token: "sync-token-1".into(), identity: None };
        assert_eq!(jira.summary_status("ARCH-7").unwrap(), Some(("Online payments".to_string(), "In Progress".to_string())));
        assert_eq!(jira.summary_status("ARCH-404").unwrap(), None);
        let links = vec![link("Card payments", "Done", jira.summary_status("ARCH-7").unwrap().as_ref().map(|(a, b)| (a.as_str(), b.as_str())))];
        let done = apply_to_jira(&jira, &to_jira_changes(&links, &json!({})));
        assert!(done.starts_with("✓ ARCH-7: summary \"Online payments\" → \"Card payments\" status In Progress → Done"), "{done}");
        let blocked = apply_to_jira(&jira, &[("ARCH-7".into(), None, Some("Rejected".into()), "ARCH-7: status → Rejected".into())]);
        assert!(blocked.starts_with("✗") && blocked.contains("Rejected"), "{blocked}");
        let requests = mock.requests.lock().unwrap();
        assert!(requests.iter().any(|(m, p, b)| m == "POST" && p.ends_with("/issue/ARCH-7/transitions") && b["transition"]["id"] == "31"), "{requests:?}");
    }
}

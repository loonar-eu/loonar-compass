//! Two baselines of a package compared with each other (LC-402, /compare-baselines): e.g. release A against release B,
//! without opening EA. A baseline is EA's XMI 1.1 of the package kept in t_document; both are read, every object
//! with an xmi.id (elements, attributes, operations, relationships, packages) and its tagged values compared by id,
//! and the noise left out – dates, local ids, positions on diagrams, the order of things. Reads only.

use crate::config::AppConfig;
use crate::tools::query_rows;
use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use std::collections::BTreeMap;

pub fn definition() -> Value {
    json!({
        "name": "compare_baselines",
        "description": "Compares two baselines of a package with each other (e.g. the package as released in two versions), without EA: \
elements, attributes, operations, relationships and packages added, removed and changed (name, type, stereotype, status, notes, tagged values…), \
without noise (dates, local ids, positions on diagrams, order). action list: the baselines of the package (version, date, notes). action compare: \
from and to = a baseline's version or id (default: the two newest). To compare the model now with a baseline use what_changed. Reads only.",
        "input_schema": { "type": "object", "properties": {
            "action": { "type": "string", "enum": ["list", "compare"] },
            "package": { "description": "package_id or exact name" },
            "from": { "type": "string", "description": "The older baseline: its version or id" },
            "to": { "type": "string", "description": "The newer baseline: its version or id" }
        }, "required": ["package"] }
    })
}

/// Tagged values that change without a change of the model.
const NOISE: &[&str] = &[
    "date_created", "date_modified", "created", "modified", "ea_localid", "ea_sourceid", "ea_targetid", "seqno", "tpos", "styleex", "style",
    "ea_stype", "ea_ttype", "ea_ntype", "gentype", "ea_eletype", "ea_qualified", "packagedelementid", "ea_package_id", "lastmodified", "phase_date",
    "headstyle", "linestyle", "linecolor", "linewidth", "virtualinheritance", "diagram", "parent", "owner", "package", "package2", "tagged",
];

/// One object of a baseline: its kind, name and properties.
#[derive(Debug, Default, Clone, PartialEq)]
pub(crate) struct Object {
    pub kind: String,
    pub name: String,
    pub props: BTreeMap<String, String>,
}

fn attr(tag: &str, name: &str) -> Option<String> {
    let key = format!(" {name}=\"");
    let i = tag.find(&key)? + key.len();
    let end = tag[i..].find('"')?;
    Some(tag[i..i + end].replace("&quot;", "\"").replace("&lt;", "<").replace("&gt;", ">").replace("&#xA;", "\n").replace("&#xD;", "").replace("&amp;", "&"))
}

/// The objects of an XMI 1.1 baseline by xmi.id (diagrams and their geometry are left out: positions are noise).
pub(crate) fn objects(xmi: &str) -> BTreeMap<String, Object> {
    let mut out: BTreeMap<String, Object> = BTreeMap::new();
    let mut stack: Vec<(String, Option<String>)> = Vec::new(); // (tag name, xmi.id)
    let mut in_diagram = 0usize;
    let mut rest = xmi;
    while let Some(start) = rest.find('<') {
        rest = &rest[start..];
        let Some(end) = rest.find('>') else { break };
        let tag = &rest[1..end];
        rest = &rest[end + 1..];
        if tag.starts_with('?') || tag.starts_with('!') {
            continue;
        }
        if let Some(name) = tag.strip_prefix('/') {
            if let Some(pos) = stack.iter().rposition(|(n, _)| n == name.trim()) {
                stack.truncate(pos);
            }
            if name.trim() == "UML:Diagram" {
                in_diagram = in_diagram.saturating_sub(1);
            }
            continue;
        }
        let self_closing = tag.ends_with('/');
        let name = tag.split_whitespace().next().unwrap_or("").trim_end_matches('/').to_string();
        if name == "UML:Diagram" && !self_closing {
            in_diagram += 1;
        }
        let id = attr(tag, "xmi.id");
        if in_diagram == 0 {
            if name == "UML:TaggedValue" {
                // A tagged value belongs to the nearest object above it.
                if let (Some(owner), Some(key), value) = (stack.iter().rev().find_map(|(_, id)| id.clone()), attr(tag, "tag"), attr(tag, "value").unwrap_or_default()) {
                    let key_l = key.to_lowercase();
                    if !NOISE.contains(&key_l.as_str()) && !key_l.starts_with("$ea_") && !key_l.starts_with("ea_") {
                        out.entry(owner).or_default().props.insert(key, value);
                    }
                }
            } else if let Some(id) = &id {
                let kind = name.trim_start_matches("UML:").to_string();
                let o = out.entry(id.clone()).or_default();
                o.kind = kind;
                o.name = attr(tag, "name").unwrap_or_default();
                for a in ["visibility", "isAbstract", "type"] {
                    if let Some(v) = attr(tag, a) {
                        o.props.insert(a.to_string(), v);
                    }
                }
            }
        }
        if !self_closing {
            stack.push((name, if in_diagram == 0 { id } else { None }));
        }
    }
    out.retain(|_, o| !o.kind.is_empty() && !matches!(o.kind.as_str(), "Diagram" | "DiagramElement" | "Model" | "Stereotype" | "TaggedValue"));
    out
}

/// What changed from one baseline to the other, in words.
pub(crate) fn compare(from: &BTreeMap<String, Object>, to: &BTreeMap<String, Object>) -> Vec<String> {
    let label = |o: &Object| format!("{} \"{}\"", o.kind, o.name);
    let mut out = Vec::new();
    for (id, o) in to {
        match from.get(id) {
            None => out.push(format!("+ added {}", label(o))),
            Some(old) => {
                let mut changes = Vec::new();
                if old.name != o.name {
                    changes.push(format!("name \"{}\" → \"{}\"", old.name, o.name));
                }
                if old.kind != o.kind {
                    changes.push(format!("kind {} → {}", old.kind, o.kind));
                }
                for key in old.props.keys().chain(o.props.keys()).collect::<std::collections::BTreeSet<_>>() {
                    let (a, b) = (old.props.get(key).map(String::as_str).unwrap_or(""), o.props.get(key).map(String::as_str).unwrap_or(""));
                    if a.trim() != b.trim() {
                        let short = |t: &str| if t.chars().count() > 80 { format!("{}…", t.chars().take(80).collect::<String>()) } else { t.to_string() };
                        changes.push(format!("{key} \"{}\" → \"{}\"", short(a), short(b)));
                    }
                }
                if !changes.is_empty() {
                    out.push(format!("~ changed {}: {}", label(o), changes.join("; ")));
                }
            }
        }
    }
    for (id, o) in from {
        if !to.contains_key(id) {
            out.push(format!("- removed {}", label(o)));
        }
    }
    out
}

/// The baselines of a package: (docid, version, date, notes), newest first.
fn baselines(cfg: &AppConfig, package: &Value) -> Result<(String, Vec<[String; 4]>)> {
    let (scope, title) = crate::model_usage::package_scope(cfg, package)?;
    let id: i64 = scope.split('(').nth(1).and_then(|s| s.split([',', ')']).next()).and_then(|p| p.parse().ok()).context("package")?;
    let guid = query_rows(cfg, &format!("select ea_guid from t_package where package_id = {id}"))?.first().map(|r| r[0].clone()).context("package")?;
    let rows = query_rows(cfg, &format!(
        "select docid, coalesce(version,''), coalesce(docdate,''), coalesce(notes,'') from t_document where doctype = 'Baseline' and elementid = '{}' order by docdate desc",
        crate::model_db::sql_text(cfg.model_source, &guid)
    ))?;
    Ok((title, rows.into_iter().map(|r| [r[0].clone(), r[1].clone(), r[2].clone(), r[3].clone()]).collect()))
}

/// The XMI of a baseline: a zip with str.dat (UTF-16), as EA keeps it.
fn baseline_xmi(cfg: &AppConfig, docid: &str) -> Result<String> {
    use std::io::Read;
    let bytes = crate::template_doctor::document_blob(cfg, docid)?;
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).context("the baseline is not a zip as EA keeps it")?;
    let mut raw = Vec::new();
    zip.by_index(0)?.read_to_end(&mut raw)?;
    Ok(if raw.get(1) == Some(&0) { String::from_utf16_lossy(&raw.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect::<Vec<_>>()) } else { String::from_utf8_lossy(&raw).to_string() })
}

pub fn run(input: &Value, cfg: &AppConfig) -> Result<String> {
    let (title, list) = baselines(cfg, &input["package"])?;
    if input["action"].as_str() == Some("list") || list.len() < 2 {
        let mut out = vec![format!("Baselines of the {title} ({}), newest first:", list.len())];
        out.extend(list.iter().map(|b| format!("- {} | {} | {}{}", b[1], b[2].get(..16).unwrap_or(&b[2]), b[0], if b[3].is_empty() { String::new() } else { format!(" | {}", b[3]) })));
        if list.len() < 2 && input["action"].as_str() != Some("list") {
            out.push("Two baselines are needed to compare (EA: Manage Baselines → New Baseline).".into());
        }
        return Ok(out.join("\n"));
    }
    let pick = |wanted: &Value, default: usize| -> Result<[String; 4]> {
        match wanted.as_str().map(str::trim).filter(|w| !w.is_empty()) {
            None => Ok(list[default].clone()),
            Some(w) => list.iter().find(|b| b[0].eq_ignore_ascii_case(w) || b[1].eq_ignore_ascii_case(w)).cloned().with_context(|| format!("there is no baseline {w} of the {title}")),
        }
    };
    let (from, to) = (pick(&input["from"], 1)?, pick(&input["to"], 0)?);
    if from[0] == to[0] {
        bail!("from and to are the same baseline");
    }
    let (a, b) = (objects(&baseline_xmi(cfg, &from[0])?), objects(&baseline_xmi(cfg, &to[0])?));
    let changes = compare(&a, &b);
    let head = format!("The {title}: baseline \"{}\" ({}) → \"{}\" ({})", from[1], from[2].get(..10).unwrap_or(""), to[1], to[2].get(..10).unwrap_or(""));
    Ok(if changes.is_empty() {
        format!("{head}: no differences in {} objects.", b.len())
    } else {
        format!("{head}: {} difference(s)\n{}{}", changes.len(), changes.iter().take(300).cloned().collect::<Vec<_>>().join("\n"), if changes.len() > 300 { "\n…" } else { "" })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xmi_objects_and_diff() {
        let a = r#"<XMI><UML:Model name="EA Model" xmi.id="MX"><UML:Class name="Order" xmi.id="EAID_1" visibility="public"><UML:ModelElement.taggedValue>
<UML:TaggedValue tag="status" value="Proposed"/><UML:TaggedValue tag="date_modified" value="2026-10-01"/><UML:TaggedValue tag="ea_localid" value="5"/></UML:ModelElement.taggedValue>
<UML:Classifier.feature><UML:Attribute name="date" xmi.id="EAID_A1"><UML:ModelElement.taggedValue><UML:TaggedValue tag="type" value="Date"/></UML:ModelElement.taggedValue></UML:Attribute></UML:Classifier.feature></UML:Class>
<UML:Class name="Gone" xmi.id="EAID_2"/><UML:Diagram name="D" xmi.id="EAID_D"><UML:Diagram.element><UML:DiagramElement geometry="Left=1;" subject="EAID_1"/></UML:Diagram.element></UML:Diagram></UML:Model></XMI>"#;
        let b = a.replace("Proposed", "Approved").replace("2026-10-01", "2026-10-30").replace(r#"<UML:Class name="Gone" xmi.id="EAID_2"/>"#, r#"<UML:Class name="Invoice" xmi.id="EAID_3"/>"#).replace("Left=1;", "Left=99;").replace(r#"value="Date""#, r#"value="DateTime""#);
        let (oa, ob) = (objects(a), objects(&b));
        assert_eq!(oa["EAID_1"].props.get("status").map(String::as_str), Some("Proposed"));
        assert!(!oa.contains_key("EAID_D") && !oa.contains_key("MX"), "{oa:?}");
        let changes = compare(&oa, &ob);
        let all = changes.join("\n");
        assert!(all.contains("~ changed Class \"Order\": status \"Proposed\" → \"Approved\"") && all.contains("+ added Class \"Invoice\"") && all.contains("- removed Class \"Gone\"") && all.contains("Attribute \"date\": type \"Date\" → \"DateTime\""), "{all}");
        assert!(!all.contains("date_modified") && !all.contains("Left="), "noise: {all}");
    }

    /// `LOONAR_TEST_PROPS=… cargo test compare_baselines_live -- --ignored --nocapture`: two baselines around a change, compared.
    #[test]
    #[ignore = "requires EA and a copy of a test model"]
    fn compare_baselines_live() {
        let cfg = AppConfig::load(std::path::Path::new(&std::env::var("LOONAR_TEST_PROPS").unwrap())).unwrap();
        let root: i64 = query_rows(&cfg, "select package_id from t_package where parent_id = 0 order by package_id").unwrap()[0][0].parse().unwrap();
        let opts = crate::ea::BatchOptions::default();
        let created = crate::ea::run_batch(&cfg, &[
            json!({"op":"create_package","key":"pkg","parent":root,"name":format!("ZZ_bl_{}", chrono::Local::now().format("%H%M%S"))}),
            json!({"op":"create_element","key":"a","package":"#pkg","name":"Order","type":"Class"}),
            json!({"op":"create_element","key":"b","package":"#pkg","name":"Gone","type":"Class"}),
        ], &crate::ea::BatchOptions { rollback_key: Some("pkg".into()), keep_locks: false }).unwrap();
        let id_of = |key: &str| created["created"].as_array().unwrap().iter().find(|c| c["key"] == key).unwrap()["id"].as_i64().unwrap();
        let (pkg, a, b) = (id_of("pkg"), id_of("a"), id_of("b"));
        crate::ea::run_batch(&cfg, &[json!({"op":"create_baseline","package":pkg,"version":"1.0","notes":"before"})], &opts).unwrap();
        crate::ea::run_batch(&cfg, &[json!({"op":"update_element","element":a,"status":"Approved","name":"Customer order"}), json!({"op":"delete","element":b}),
            json!({"op":"create_element","package":pkg,"name":"Invoice","type":"Class"})], &opts).unwrap();
        crate::ea::run_batch(&cfg, &[json!({"op":"create_baseline","package":pkg,"version":"2.0","notes":"after"})], &opts).unwrap();
        println!("{}", run(&json!({ "action": "list", "package": pkg }), &cfg).unwrap());
        let text = run(&json!({ "package": pkg, "from": "1.0", "to": "2.0" }), &cfg).unwrap();
        println!("{text}");
        crate::ea::run_batch(&cfg, &[json!({"op":"delete","package":pkg})], &opts).unwrap();
        crate::ea::shutdown();
        assert!(text.contains("Customer order") && text.contains("+ added") && text.contains("- removed") && text.contains("Approved"), "{text}");
    }
}

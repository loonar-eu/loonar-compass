//! Safety net for manual edits in EA (LC-382). With `safety_net.enabled=true` and Compass in a tab of EA, every
//! deletion of an element, a package or a diagram done by hand in EA is preceded by a snapshot: EA asks the add-in,
//! the add-in asks Compass (the guard mechanism), Compass reads the object and what hangs on it from the database
//! and answers "allow" – a snapshot never stops a deletion. The snapshots and a journal of the day's manual changes
//! are kept per model; `safety_net` lists them, shows one, and restores a deleted object as an ordinary save of
//! Compass (approval card, undo journal, audit).

use crate::config::AppConfig;
use crate::tools::query_rows;
use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use std::collections::HashSet;
use std::path::PathBuf;

/// The deleting events the safety net takes a snapshot for.
pub const EVENTS: &[&str] = &["element_deleting", "package_deleting", "diagram_deleting"];
/// A package with more elements gets a snapshot of its first elements only (EA waits for the answer).
const MAX_ELEMENTS: usize = 3000;
/// Snapshots kept per model (the oldest go).
const KEEP: usize = 500;

pub fn definition() -> Value {
    json!({
        "name": "safety_net",
        "description": "The safety net for manual edits in EA (setting safety_net.enabled): snapshots taken just before an element, \
package or diagram was deleted by hand in EA, and the journal of manual changes of a day. action \"list\": the snapshots, newest first \
(id, time, what). \"show\": one snapshot (id). \"today\": the manual changes of a day (date YYYY-MM-DD, default today), with names even of \
deleted objects. \"restore\": recreates a deleted object from a snapshot (id; optional package = package_id to restore into when the \
original package is gone) – elements with their notes, status, tagged values, attributes and operations, relationships to elements that \
still exist, places on diagrams that still exist; a package with its child packages, elements and diagrams; a diagram with the elements \
that still exist. Restored objects get new ids and GUIDs. Restore is a save the user approves.",
        "input_schema": { "type": "object", "properties": {
            "action": { "type": "string", "enum": ["list", "show", "today", "restore"] },
            "id": { "type": "string", "description": "Snapshot id (from list)" },
            "date": { "type": "string", "description": "Day for today (YYYY-MM-DD)" },
            "package": { "type": "integer", "description": "package_id to restore into (restore; only when the original package is gone)" }
        }, "required": ["action"] }
    })
}

fn dir(cfg: &AppConfig) -> PathBuf {
    let key: String = crate::undo::model_key(cfg).chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect();
    let short: String = key.chars().rev().take(80).collect::<String>().chars().rev().collect();
    cfg.base_dir.join("safety_net").join(short)
}

fn in_list(col: &str, ids: &[String]) -> String {
    if ids.is_empty() {
        return "1 = 0".into();
    }
    format!("({})", ids.chunks(900).map(|c| format!("{col} in ({})", c.join(","))).collect::<Vec<_>>().join(" or "))
}

/// The snapshot of an object about to be deleted; written to the model's folder. Returns a short description.
pub fn snapshot(cfg: &AppConfig, event: &str, id: i64) -> Result<String> {
    let mut snap = json!({ "time": chrono::Local::now().to_rfc3339(), "event": event, "id": id });
    let (mut packages, mut element_ids, mut diagram_ids) = (Vec::<String>::new(), Vec::<String>::new(), Vec::<String>::new());
    let what = match event {
        "element_deleting" => {
            element_ids.push(id.to_string());
            // Elements nested in it go with it.
            let mut level = vec![id.to_string()];
            while !level.is_empty() && element_ids.len() < MAX_ELEMENTS {
                level = query_rows(cfg, &format!("select object_id from t_object where {}", in_list("parentid", &level)))?.into_iter().map(|r| r[0].clone()).collect();
                level.retain(|e| !element_ids.contains(e));
                element_ids.extend(level.iter().cloned());
            }
            diagram_ids = query_rows(cfg, &format!("select diagram_id from t_diagram where {}", in_list("parentid", &element_ids)))?.into_iter().map(|r| r[0].clone()).collect();
            "element"
        }
        "package_deleting" => {
            packages.push(id.to_string());
            let mut level = vec![id.to_string()];
            while !level.is_empty() {
                level = query_rows(cfg, &format!("select package_id from t_package where {}", in_list("parent_id", &level)))?.into_iter().map(|r| r[0].clone()).collect();
                level.retain(|p| !packages.contains(p));
                packages.extend(level.iter().cloned());
            }
            element_ids = query_rows(cfg, &format!("select object_id from t_object where object_type <> 'Package' and {} order by object_id", in_list("package_id", &packages)))?
                .into_iter()
                .map(|r| r[0].clone())
                .collect();
            if element_ids.len() > MAX_ELEMENTS {
                snap["truncated"] = json!(element_ids.len());
                element_ids.truncate(MAX_ELEMENTS);
            }
            diagram_ids = query_rows(cfg, &format!("select diagram_id from t_diagram where {}", in_list("package_id", &packages)))?.into_iter().map(|r| r[0].clone()).collect();
            "package"
        }
        "diagram_deleting" => {
            diagram_ids.push(id.to_string());
            "diagram"
        }
        other => bail!("no snapshot for {other}"),
    };
    snap["kind"] = json!(what);
    snap["packages"] = json!(query_rows(cfg, &format!(
        "select package_id as c0, name as c1, parent_id as c2, ea_guid as c3, coalesce(notes,'') as c4 from t_package where {} order by package_id",
        in_list("package_id", &packages)
    ))?
    .into_iter()
    .map(|r| json!({ "package_id": r[0], "name": r[1], "parent_id": r[2], "guid": r[3], "notes": r[4] }))
    .collect::<Vec<_>>());
    let mut elements = Vec::new();
    for chunk in element_ids.chunks(500) {
        let ids = chunk.to_vec();
        let tags = query_rows(cfg, &format!("select object_id as c0, property as c1, coalesce(value,'') as c2 from t_objectproperties where {}", in_list("object_id", &ids)))?;
        let attributes = query_rows(cfg, &format!(
            "select object_id as c0, name as c1, coalesce(type,'') as c2, coalesce(notes,'') as c3, coalesce(scope,'') as c4, coalesce(stereotype,'') as c5 from t_attribute where {} order by object_id, pos",
            in_list("object_id", &ids)
        ))?;
        let operations = query_rows(cfg, &format!(
            "select object_id as c0, name as c1, coalesce(type,'') as c2, coalesce(notes,'') as c3, coalesce(scope,'') as c4, coalesce(stereotype,'') as c5 from t_operation where {} order by object_id, pos",
            in_list("object_id", &ids)
        ))?;
        for r in query_rows(cfg, &format!(
            "select object_id as c0, name as c1, object_type as c2, coalesce(stereotype,'') as c3, coalesce(note,'') as c4, coalesce(status,'') as c5, coalesce(phase,'') as c6, \
             coalesce(alias,'') as c7, coalesce(author,'') as c8, coalesce(version,'') as c9, package_id as c10, coalesce(parentid,0) as c11, ea_guid as c12 from t_object where {} order by object_id",
            in_list("object_id", &ids)
        ))? {
            let of = |rows: &[Vec<String>]| rows.iter().filter(|x| x[0] == r[0]).map(|x| json!({ "name": x[1], "type": x[2], "notes": x[3], "visibility": x[4], "stereotype": x[5] })).collect::<Vec<_>>();
            elements.push(json!({
                "object_id": r[0], "name": r[1], "type": r[2], "stereotype": r[3], "notes": r[4], "status": r[5], "phase": r[6], "alias": r[7],
                "author": r[8], "version": r[9], "package_id": r[10], "parent_id": r[11], "guid": r[12],
                "tags": tags.iter().filter(|t| t[0] == r[0]).map(|t| (t[1].clone(), json!(t[2]))).collect::<serde_json::Map<_, _>>(),
                "attributes": of(&attributes), "operations": of(&operations),
            }));
        }
    }
    snap["elements"] = json!(elements);
    // Relationships of these elements (with the other end's name, for restoring when it still exists).
    let mut connectors = Vec::new();
    for chunk in element_ids.chunks(400) {
        let ids = chunk.to_vec();
        for r in query_rows(cfg, &format!(
            "select c.connector_id as c0, c.connector_type as c1, coalesce(c.stereotype,'') as c2, coalesce(c.name,'') as c3, c.start_object_id as c4, c.end_object_id as c5, \
             s.ea_guid as c6, t.ea_guid as c7 from t_connector c join t_object s on s.object_id = c.start_object_id join t_object t on t.object_id = c.end_object_id \
             where ({} or {}) and c.connector_type not in ('Sequence', 'Collaboration')",
            in_list("c.start_object_id", &ids),
            in_list("c.end_object_id", &ids)
        ))? {
            if !connectors.iter().any(|c: &Value| c["connector_id"] == r[0]) {
                connectors.push(json!({ "connector_id": r[0], "type": r[1], "stereotype": r[2], "name": r[3], "source": r[4], "target": r[5], "source_guid": r[6], "target_guid": r[7] }));
            }
        }
    }
    snap["connectors"] = json!(connectors);
    // Where the elements stood on diagrams, and the diagrams that go with the object.
    let mut placements = Vec::new();
    for chunk in element_ids.chunks(500) {
        for r in query_rows(cfg, &format!(
            "select diagram_id as c0, object_id as c1, rectleft as c2, recttop as c3, rectright as c4, rectbottom as c5 from t_diagramobjects where {}",
            in_list("object_id", &chunk.to_vec())
        ))? {
            placements.push(json!({ "diagram_id": r[0], "object_id": r[1], "left": r[2], "top": r[3], "right": r[4], "bottom": r[5] }));
        }
    }
    snap["placements"] = json!(placements);
    let mut diagrams = Vec::new();
    for r in query_rows(cfg, &format!(
        "select diagram_id as c0, name as c1, diagram_type as c2, package_id as c3, coalesce(parentid,0) as c4, coalesce(notes,'') as c5 from t_diagram where {}",
        in_list("diagram_id", &diagram_ids)
    ))? {
        let objects: Vec<Value> = query_rows(cfg, &format!(
            "select object_id as c0, rectleft as c1, recttop as c2, rectright as c3, rectbottom as c4 from t_diagramobjects where diagram_id = {}",
            r[0].parse::<i64>().unwrap_or(0)
        ))?
        .into_iter()
        .map(|o| json!({ "object_id": o[0], "left": o[1], "top": o[2], "right": o[3], "bottom": o[4] }))
        .collect();
        diagrams.push(json!({ "diagram_id": r[0], "name": r[1], "type": r[2], "package_id": r[3], "parent_id": r[4], "notes": r[5], "objects": objects }));
    }
    snap["diagrams"] = json!(diagrams);
    let name = match what {
        "element" => elements.first().and_then(|e| e["name"].as_str()).unwrap_or("").to_string(),
        "package" => snap["packages"][0]["name"].as_str().unwrap_or("").to_string(),
        _ => diagrams.first().and_then(|d| d["name"].as_str()).unwrap_or("").to_string(),
    };
    snap["name"] = json!(name);
    let summary = format!(
        "{what} \"{name}\": {} element(s), {} relationship(s), {} diagram(s){}",
        elements.len(),
        connectors.len(),
        diagrams.len(),
        snap["truncated"].as_u64().map(|n| format!(" (of {n} elements only the first {MAX_ELEMENTS})")).unwrap_or_default()
    );
    snap["summary"] = json!(summary);
    let folder = dir(cfg);
    std::fs::create_dir_all(&folder)?;
    let file = folder.join(format!("{}-{what}-{id}.json", chrono::Local::now().format("%Y%m%d-%H%M%S%3f")));
    let tmp = file.with_extension("tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(&snap)?)?;
    std::fs::rename(&tmp, &file)?;
    prune(&folder);
    Ok(summary)
}

fn snapshots(cfg: &AppConfig) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir(cfg)).into_iter().flatten().flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|e| e == "json")).collect();
    files.sort();
    files.reverse();
    files
}

fn prune(folder: &std::path::Path) {
    let mut files: Vec<PathBuf> = std::fs::read_dir(folder).into_iter().flatten().flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|e| e == "json")).collect();
    files.sort();
    if files.len() > KEEP {
        for old in &files[..files.len() - KEEP] {
            let _ = std::fs::remove_file(old);
        }
    }
}

/// What a diagram holds, for telling whether a save really changed it (LC-422): object → place, link → hidden.
pub(crate) fn diagram_state(cfg: &AppConfig, diagram: i64) -> Value {
    let objects: serde_json::Map<String, Value> = query_rows(cfg, &format!(
        "select x.object_id, coalesce(o.name,''), x.rectleft, x.recttop, x.rectright, x.rectbottom from t_diagramobjects x join t_object o on o.object_id = x.object_id where x.diagram_id = {diagram}"
    ))
    .unwrap_or_default()
    .into_iter()
    .map(|r| (r[0].clone(), json!({ "name": r[1], "rect": [r[2], r[3], r[4], r[5]] })))
    .collect();
    let links: serde_json::Map<String, Value> = query_rows(cfg, &format!("select connectorid, coalesce(hidden,0), coalesce(path,'') from t_diagramlinks where diagramid = {diagram}"))
        .unwrap_or_default()
        .into_iter()
        .map(|r| (r[0].clone(), json!({ "hidden": r[1] == "1" || r[1].eq_ignore_ascii_case("true"), "path": r[2] })))
        .collect();
    let head = query_rows(cfg, &format!("select name, coalesce(notes,'') from t_diagram where diagram_id = {diagram}")).ok().and_then(|r| r.into_iter().next()).unwrap_or_default();
    json!({ "name": head.first(), "notes": head.get(1), "objects": objects, "links": links })
}

/// What changed between two states of a diagram, in words (empty: nothing).
pub(crate) fn diagram_diff(before: &Value, now: &Value) -> Vec<String> {
    let mut out = Vec::new();
    let name = |v: &Value, id: &str| v["objects"][id]["name"].as_str().unwrap_or("").to_string();
    let (bo, no) = (before["objects"].as_object().cloned().unwrap_or_default(), now["objects"].as_object().cloned().unwrap_or_default());
    let added: Vec<String> = no.keys().filter(|k| !bo.contains_key(*k)).map(|k| name(now, k)).collect();
    let removed: Vec<String> = bo.keys().filter(|k| !no.contains_key(*k)).map(|k| name(before, k)).collect();
    let moved: Vec<String> = no.iter().filter(|(k, v)| bo.get(*k).is_some_and(|b| b["rect"] != v["rect"])).map(|(k, _)| name(now, k)).collect();
    for (label, list) in [("added", added), ("removed", removed), ("moved or resized", moved)] {
        if !list.is_empty() {
            out.push(format!("{} {label}: {}", list.len(), list.iter().take(8).map(|n| format!("\"{n}\"")).collect::<Vec<_>>().join(", ")));
        }
    }
    let (bl, nl) = (before["links"].as_object().cloned().unwrap_or_default(), now["links"].as_object().cloned().unwrap_or_default());
    let shown = nl.iter().filter(|(k, v)| bl.get(*k).is_some_and(|b| b["hidden"] == true && v["hidden"] == false)).count();
    let hidden = nl.iter().filter(|(k, v)| bl.get(*k).is_some_and(|b| b["hidden"] == false && v["hidden"] == true)).count();
    let rerouted = nl.iter().filter(|(k, v)| bl.get(*k).is_some_and(|b| b["path"] != v["path"])).count();
    for (n, what) in [(shown, "relationships shown"), (hidden, "relationships hidden"), (rerouted, "relationships re-routed")] {
        if n > 0 {
            out.push(format!("{n} {what}"));
        }
    }
    if before["name"] != now["name"] || before["notes"] != now["notes"] {
        out.push("its name or notes".into());
    }
    out
}

/// After EA saved a diagram (the add-in's "diagram_changed"): what really changed since its last save, for the user
/// who wonders why EA asked (LC-422). The first save of a diagram is only remembered.
pub fn diagram_saved(cfg: &AppConfig, diagram: i64) -> Option<String> {
    let folder = dir(cfg).join("diagrams");
    let _ = std::fs::create_dir_all(&folder);
    let file = folder.join(format!("{diagram}.json"));
    let now = diagram_state(cfg, diagram);
    let before: Option<Value> = std::fs::read_to_string(&file).ok().and_then(|t| serde_json::from_str(&t).ok());
    let _ = std::fs::write(&file, now.to_string());
    let before = before?;
    let changes = diagram_diff(&before, &now);
    let name = now["name"].as_str().unwrap_or("").to_string();
    Some(if changes.is_empty() {
        crate::i18n::fmt("diagram \"{0}\" saved: nothing in it really changed (EA asks to save also after only looking or selecting)", &[name])
    } else {
        crate::i18n::fmt("diagram \"{0}\" saved: {1}", &[name, changes.join("; ")])
    })
}

/// A manual change of EA (an event of the add-in) in the day's journal: kind, id and what it was.
pub fn journal(cfg: &AppConfig, kind: &str, id: i64, described: Option<&str>) {
    let folder = dir(cfg);
    if std::fs::create_dir_all(&folder).is_err() {
        return;
    }
    let line = json!({ "time": chrono::Local::now().format("%H:%M:%S").to_string(), "event": kind, "id": id, "what": described.unwrap_or("") }).to_string();
    let file = folder.join(format!("journal-{}.jsonl", chrono::Local::now().format("%Y-%m-%d")));
    use std::io::Write;
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(file) {
        let _ = writeln!(f, "{line}");
    }
}

fn load(cfg: &AppConfig, id: &str) -> Result<Value> {
    let id = id.trim();
    if id.is_empty() || id.contains(['/', '\\', '.']) {
        bail!("give the id of a snapshot (from list)");
    }
    let file = dir(cfg).join(format!("{id}.json"));
    Ok(serde_json::from_slice(&std::fs::read(&file).with_context(|| format!("there is no snapshot {id}"))?)?)
}

pub fn run(input: &Value, cfg: &AppConfig) -> Result<String> {
    let off = if cfg.safety_net { String::new() } else { "\n(The safety net is off: set safety_net.enabled=true in the settings and restart Compass in EA.)".to_string() };
    match input["action"].as_str().unwrap_or("list") {
        "list" => {
            let files = snapshots(cfg);
            if files.is_empty() {
                return Ok(format!("No snapshots yet.{off}"));
            }
            let mut out = vec![format!("Snapshots before deletions in EA ({}), newest first:", files.len())];
            for f in files.iter().take(50) {
                let snap: Value = serde_json::from_slice(&std::fs::read(f).unwrap_or_default()).unwrap_or(Value::Null);
                out.push(format!("- {} | {} | {}", f.file_stem().unwrap_or_default().to_string_lossy(), snap["time"].as_str().unwrap_or("").get(..19).unwrap_or(""), snap["summary"].as_str().unwrap_or("")));
            }
            Ok(format!("{}{off}", out.join("\n")))
        }
        "show" => {
            let snap = load(cfg, input["id"].as_str().unwrap_or(""))?;
            let mut out = vec![format!("{} – {}", snap["time"].as_str().unwrap_or(""), snap["summary"].as_str().unwrap_or(""))];
            for p in snap["packages"].as_array().into_iter().flatten().take(30) {
                out.push(format!("  package {} (was id {})", p["name"].as_str().unwrap_or(""), p["package_id"].as_str().unwrap_or("")));
            }
            for e in snap["elements"].as_array().into_iter().flatten().take(60) {
                out.push(format!(
                    "  {} ({}{}) status \"{}\", {} tagged value(s), {} attribute(s)",
                    e["name"].as_str().unwrap_or(""),
                    e["type"].as_str().unwrap_or(""),
                    e["stereotype"].as_str().filter(|s| !s.is_empty()).map(|s| format!(" «{s}»")).unwrap_or_default(),
                    e["status"].as_str().unwrap_or(""),
                    e["tags"].as_object().map(|t| t.len()).unwrap_or(0),
                    e["attributes"].as_array().map(|a| a.len()).unwrap_or(0)
                ));
            }
            for d in snap["diagrams"].as_array().into_iter().flatten().take(30) {
                out.push(format!("  diagram {} ({}, {} element(s))", d["name"].as_str().unwrap_or(""), d["type"].as_str().unwrap_or(""), d["objects"].as_array().map(|o| o.len()).unwrap_or(0)));
            }
            Ok(out.join("\n"))
        }
        "today" => {
            let day = input["date"].as_str().map(str::trim).filter(|d| !d.is_empty()).map(String::from).unwrap_or_else(|| chrono::Local::now().format("%Y-%m-%d").to_string());
            if chrono::NaiveDate::parse_from_str(&day, "%Y-%m-%d").is_err() {
                bail!("date: YYYY-MM-DD");
            }
            let text = std::fs::read_to_string(dir(cfg).join(format!("journal-{day}.jsonl"))).unwrap_or_default();
            let lines: Vec<Value> = text.lines().filter_map(|l| serde_json::from_str(l).ok()).collect();
            if lines.is_empty() {
                return Ok(format!("No manual changes in EA recorded on {day}.{off}"));
            }
            let mut out = vec![format!("Manual changes in EA on {day} ({}):", lines.len())];
            out.extend(lines.iter().map(|l| format!("- {} {} {}{}", l["time"].as_str().unwrap_or(""), l["event"].as_str().unwrap_or("").replace('_', " "), l["id"], l["what"].as_str().filter(|w| !w.is_empty()).map(|w| format!(": {w}")).unwrap_or_default())));
            Ok(out.join("\n"))
        }
        "restore" => {
            let ops = restore_ops(input, cfg)?;
            Ok(format!("{} operation(s) would restore it; run restore to save them (the user approves).", ops.len()))
        }
        other => bail!("unknown action {other}: list, show, today or restore"),
    }
}

/// Whether a row with this id still exists.
fn exists(cfg: &AppConfig, table: &str, column: &str, id: &str) -> bool {
    id.parse::<i64>().is_ok_and(|n| query_rows(cfg, &format!("select 1 from {table} where {column} = {n}")).is_ok_and(|r| !r.is_empty()))
}

/// The operations that recreate the object of a snapshot (an ordinary save of Compass).
pub fn restore_ops(input: &Value, cfg: &AppConfig) -> Result<Vec<Value>> {
    let snap = load(cfg, input["id"].as_str().unwrap_or(""))?;
    let target = input["package"].as_i64();
    let kind = snap["kind"].as_str().unwrap_or("");
    let mut ops = Vec::new();
    // Packages first (a deleted package with its child packages), keyed by the old ids.
    let restored_packages: HashSet<String> = snap["packages"].as_array().into_iter().flatten().filter_map(|p| p["package_id"].as_str().map(String::from)).collect();
    let package_ref = |old: &str| -> Result<Value> {
        if restored_packages.contains(old) {
            return Ok(json!(format!("#p{old}")));
        }
        if exists(cfg, "t_package", "package_id", old) {
            return Ok(json!(old.parse::<i64>().unwrap_or(0)));
        }
        match target {
            Some(t) => Ok(json!(t)),
            None => bail!("the package the object was in (package_id {old}) no longer exists: give package = the package_id to restore into"),
        }
    };
    for p in snap["packages"].as_array().into_iter().flatten() {
        let old = p["package_id"].as_str().unwrap_or("");
        ops.push(json!({ "op": "create_package", "key": format!("p{old}"), "parent": package_ref(p["parent_id"].as_str().unwrap_or("0"))?, "name": p["name"], "notes": p["notes"] }));
    }
    let restored_elements: HashSet<String> = snap["elements"].as_array().into_iter().flatten().filter_map(|e| e["object_id"].as_str().map(String::from)).collect();
    let element_ref = |old: &str, guid: &str| -> Option<Value> {
        if restored_elements.contains(old) {
            return Some(json!(format!("#e{old}")));
        }
        // Still in the model (by its id, or by its GUID when it was restored before).
        if exists(cfg, "t_object", "object_id", old) {
            return Some(json!(old.parse::<i64>().unwrap_or(0)));
        }
        let guid = crate::model_db::sql_text(cfg.model_source, guid);
        query_rows(cfg, &format!("select object_id from t_object where ea_guid = '{guid}'")).ok()?.first().and_then(|r| r[0].parse::<i64>().ok()).map(|n| json!(n))
    };
    // Elements: parents before the elements nested in them (the snapshot keeps them in id order; nested ones come later).
    let mut elements: Vec<&Value> = snap["elements"].as_array().into_iter().flatten().collect();
    elements.sort_by_key(|e| if restored_elements.contains(e["parent_id"].as_str().unwrap_or("0")) { 1 } else { 0 });
    for e in &elements {
        let old = e["object_id"].as_str().unwrap_or("");
        let mut op = json!({ "op": "create_element", "key": format!("e{old}"), "name": e["name"], "type": e["type"], "notes": e["notes"] });
        let parent = e["parent_id"].as_str().unwrap_or("0");
        if restored_elements.contains(parent) {
            op["parent_element"] = json!(format!("#e{parent}"));
        } else {
            op["package"] = package_ref(e["package_id"].as_str().unwrap_or(""))?;
        }
        if e["stereotype"].as_str().is_some_and(|s| !s.is_empty()) {
            op["stereotype"] = e["stereotype"].clone();
        }
        if e["tags"].as_object().is_some_and(|t| !t.is_empty()) {
            op["tags"] = e["tags"].clone();
        }
        ops.push(op);
        let mut update = json!({ "op": "update_element", "element": format!("#e{old}") });
        let mut changed = false;
        for field in ["status", "phase", "alias", "author", "version"] {
            if e[field].as_str().is_some_and(|v| !v.is_empty()) {
                update[field] = e[field].clone();
                changed = true;
            }
        }
        if changed {
            ops.push(update);
        }
        for a in e["attributes"].as_array().into_iter().flatten() {
            let mut op = json!({ "op": "create_attribute", "element": format!("#e{old}"), "name": a["name"], "type": a["type"], "notes": a["notes"] });
            for f in ["visibility", "stereotype"] {
                if a[f].as_str().is_some_and(|v| !v.is_empty()) {
                    op[f] = a[f].clone();
                }
            }
            ops.push(op);
        }
        for m in e["operations"].as_array().into_iter().flatten() {
            let mut op = json!({ "op": "create_operation", "element": format!("#e{old}"), "name": m["name"], "return_type": m["type"], "notes": m["notes"] });
            for f in ["visibility", "stereotype"] {
                if m[f].as_str().is_some_and(|v| !v.is_empty()) {
                    op[f] = m[f].clone();
                }
            }
            ops.push(op);
        }
    }
    // Relationships whose both ends exist again (a deleted diagram's relationships were never deleted).
    if kind != "diagram" {
        for c in snap["connectors"].as_array().into_iter().flatten() {
            let (Some(source), Some(target)) = (
                element_ref(c["source"].as_str().unwrap_or(""), c["source_guid"].as_str().unwrap_or("")),
                element_ref(c["target"].as_str().unwrap_or(""), c["target_guid"].as_str().unwrap_or("")),
            ) else {
                continue;
            };
            if !source.is_string() && !target.is_string() {
                continue; // both ends never went away: the relationship is still there
            }
            let mut op = json!({ "op": "create_connector", "source": source, "target": target, "type": c["type"], "name": c["name"] });
            if c["stereotype"].as_str().is_some_and(|s| !s.is_empty()) {
                op["stereotype"] = c["stereotype"].clone();
            }
            ops.push(op);
        }
    }
    // Diagrams that went with the object, with the elements on them that exist again.
    let place = |diagram: Value, element: Value, o: &Value| {
        let n = |k: &str| o[k].as_str().and_then(|v| v.trim().parse::<i64>().ok()).unwrap_or(0);
        json!({ "op": "place", "diagram": diagram, "element": element, "left": n("left"), "top": -n("top"), "width": (n("right") - n("left")).max(20), "height": (n("top") - n("bottom")).max(20) })
    };
    let restored_diagrams: HashSet<String> = snap["diagrams"].as_array().into_iter().flatten().filter_map(|d| d["diagram_id"].as_str().map(String::from)).collect();
    for d in snap["diagrams"].as_array().into_iter().flatten() {
        let old = d["diagram_id"].as_str().unwrap_or("");
        let mut op = json!({ "op": "create_diagram", "key": format!("d{old}"), "name": d["name"], "type": d["type"], "notes": d["notes"] });
        let parent = d["parent_id"].as_str().unwrap_or("0");
        if restored_elements.contains(parent) {
            op["parent_element"] = json!(format!("#e{parent}"));
        } else {
            op["package"] = package_ref(d["package_id"].as_str().unwrap_or(""))?;
        }
        ops.push(op);
        for o in d["objects"].as_array().into_iter().flatten() {
            let id = o["object_id"].as_str().unwrap_or("");
            if let Some(element) = element_ref(id, "") {
                ops.push(place(json!(format!("#d{old}")), element, o));
            }
        }
    }
    // Places of restored elements on diagrams that still exist.
    for p in snap["placements"].as_array().into_iter().flatten() {
        let diagram = p["diagram_id"].as_str().unwrap_or("");
        if restored_diagrams.contains(diagram) || !exists(cfg, "t_diagram", "diagram_id", diagram) {
            continue;
        }
        ops.push(place(json!(diagram.parse::<i64>().unwrap_or(0)), json!(format!("#e{}", p["object_id"].as_str().unwrap_or(""))), p));
    }
    if ops.is_empty() {
        bail!("nothing to restore in this snapshot");
    }
    Ok(ops)
}

#[cfg(test)]
mod tests {
    /// LC-422 on a model: the first save is remembered, the next one without changes says so.
    #[test]
    #[ignore = "requires a test model"]
    fn diagram_saved_live() {
        let mut cfg = AppConfig::load(std::path::Path::new(&std::env::var("LOONAR_TEST_PROPS").unwrap())).unwrap();
        cfg.base_dir = std::env::temp_dir().join(format!("loonar-dsave-{}", std::process::id()));
        let d: i64 = query_rows(&cfg, "select diagramid from t_diagramlinks").unwrap()[0][0].parse().unwrap();
        assert!(diagram_saved(&cfg, d).is_none());
        let second = diagram_saved(&cfg, d).unwrap();
        println!("{second}");
        assert!(second.contains("nothing in it really changed"));
        let _ = std::fs::remove_dir_all(&cfg.base_dir);
    }

    /// LC-422: what changed on a diagram between two saves.
    #[test]
    fn diagram_differences() {
        let before = json!({ "name": "D", "notes": "", "objects": { "1": { "name": "A", "rect": ["0","0","10","-10"] }, "2": { "name": "B", "rect": ["20","0","30","-10"] } },
            "links": { "7": { "hidden": false, "path": "" } } });
        let mut now = before.clone();
        assert!(diagram_diff(&before, &now).is_empty());
        now["objects"]["1"]["rect"] = json!(["5","0","15","-10"]);
        now["objects"].as_object_mut().unwrap().remove("2");
        now["links"]["7"]["hidden"] = json!(true);
        let d = diagram_diff(&before, &now).join(" | ");
        assert!(d.contains("1 removed: \"B\"") && d.contains("1 moved or resized: \"A\"") && d.contains("1 relationships hidden"), "{d}");
    }

    use super::*;

    /// LC-382 on a model copy: a snapshot of an element and of a package, the operations to restore them.
    /// `LOONAR_TEST_PROPS=… cargo test safety_net_live -- --ignored --nocapture`
    #[test]
    #[ignore = "requires a test model"]
    fn safety_net_live() {
        let mut cfg = AppConfig::load(std::path::Path::new(&std::env::var("LOONAR_TEST_PROPS").unwrap())).unwrap();
        cfg.base_dir = std::env::temp_dir().join(format!("loonar-safety-{}", std::process::id()));
        cfg.safety_net = true;
        let element = query_rows(&cfg, "select o.object_id from t_object o where o.object_type = 'Class' and exists (select 1 from t_attribute a where a.object_id = o.object_id)").unwrap()[0][0].clone();
        let summary = snapshot(&cfg, "element_deleting", element.parse().unwrap()).unwrap();
        println!("{summary}");
        let listed = run(&json!({ "action": "list" }), &cfg).unwrap();
        println!("{listed}");
        let id = listed.lines().nth(1).unwrap().trim_start_matches("- ").split(" | ").next().unwrap().to_string();
        println!("{}", run(&json!({ "action": "show", "id": id }), &cfg).unwrap());
        let ops = restore_ops(&json!({ "id": id }), &cfg).unwrap();
        assert!(ops.iter().any(|o| o["op"] == "create_element") && ops.iter().any(|o| o["op"] == "create_attribute"), "{ops:?}");
        let package = query_rows(&cfg, "select package_id from t_package where parent_id <> 0 and package_id in (select package_id from t_object) order by package_id").unwrap()[0][0].clone();
        println!("{}", snapshot(&cfg, "package_deleting", package.parse().unwrap()).unwrap());
        let id = run(&json!({ "action": "list" }), &cfg).unwrap().lines().nth(1).unwrap().trim_start_matches("- ").split(" | ").next().unwrap().to_string();
        let ops = restore_ops(&json!({ "id": id }), &cfg).unwrap();
        assert_eq!(ops[0]["op"], "create_package", "{ops:?}");
        journal(&cfg, "element", 7, Some("element \"A\""));
        assert!(run(&json!({ "action": "today" }), &cfg).unwrap().contains("element \"A\""));
        let _ = std::fs::remove_dir_all(&cfg.base_dir);
    }

    /// Restoring through EA: an element deleted after its snapshot comes back with its tag, attribute, status,
    /// relationship and place; then a whole package. `LOONAR_TEST_PROPS=… cargo test safety_net_restore_live -- --ignored --nocapture`
    #[test]
    #[ignore = "requires EA and a copy of a test model"]
    fn safety_net_restore_live() {
        let mut cfg = AppConfig::load(std::path::Path::new(&std::env::var("LOONAR_TEST_PROPS").unwrap())).unwrap();
        cfg.base_dir = std::env::temp_dir().join(format!("loonar-safety-r-{}", std::process::id()));
        let q = |sql: &str| query_rows(&cfg, sql).unwrap();
        let root: i64 = q("select package_id from t_package where parent_id = 0 order by package_id")[0][0].parse().unwrap();
        let ops = vec![
            json!({"op":"create_package","key":"pkg","parent":root,"name":format!("ZZ_safety_{}", chrono::Local::now().format("%H%M%S"))}),
            json!({"op":"create_element","key":"a","package":"#pkg","name":"Invoice","type":"Class","notes":"Invoice notes.","tags":{"Owner":"Finance"}}),
            json!({"op":"update_element","element":"#a","status":"Approved"}),
            json!({"op":"create_attribute","element":"#a","name":"total","type":"decimal"}),
            json!({"op":"create_element","key":"b","package":"#pkg","name":"Customer","type":"Class"}),
            json!({"op":"create_connector","source":"#a","target":"#b","type":"Association","name":"billed to"}),
            json!({"op":"create_diagram","key":"d","package":"#pkg","name":"Billing","type":"Logical"}),
            json!({"op":"place","diagram":"#d","element":"#a","left":40,"top":40,"width":130,"height":70}),
            json!({"op":"place","diagram":"#d","element":"#b","left":300,"top":40,"width":130,"height":70}),
        ];
        let created = crate::ea::run_batch(&cfg, &ops, &crate::ea::BatchOptions { rollback_key: Some("pkg".into()), keep_locks: false }).expect("creation");
        let id_of = |key: &str| created["created"].as_array().unwrap().iter().find(|c| c["key"] == key).expect(key)["id"].as_i64().unwrap();
        let (pkg, a, d) = (id_of("pkg"), id_of("a"), id_of("d"));
        let place_before = q(&format!("select rectleft, recttop, rectright, rectbottom from t_diagramobjects where diagram_id = {d} and object_id = {a}"));
        snapshot(&cfg, "element_deleting", a).unwrap();
        crate::ea::run_batch(&cfg, &[json!({"op":"delete","element":a})], &crate::ea::BatchOptions::default()).expect("delete");
        let id = snapshots(&cfg)[0].file_stem().unwrap().to_string_lossy().to_string();
        let restore = restore_ops(&json!({ "id": id }), &cfg).unwrap();
        println!("{restore:#?}");
        crate::ea::run_batch(&cfg, &restore, &crate::ea::BatchOptions::default()).expect("restore");
        let mut problems = Vec::new();
        let back = q(&format!("select object_id, status, note from t_object where package_id = {pkg} and name = 'Invoice'"));
        if back.len() != 1 {
            problems.push(format!("Invoice: {back:?}"));
        } else {
            let n = &back[0][0];
            if back[0][1] != "Approved" { problems.push(format!("status {}", back[0][1])); }
            if q(&format!("select value from t_objectproperties where object_id = {n} and property = 'Owner'")).first().map(|r| r[0].as_str()) != Some("Finance") { problems.push("tag".into()); }
            if q(&format!("select name from t_attribute where object_id = {n}")).len() != 1 { problems.push("attribute".into()); }
            if q(&format!("select connector_id from t_connector where start_object_id = {n} and name = 'billed to'")).len() != 1 { problems.push("relationship".into()); }
            if q(&format!("select rectleft, recttop, rectright, rectbottom from t_diagramobjects where diagram_id = {d} and object_id = {n}")) != place_before { problems.push("place on the diagram".into()); }
        }
        // The whole package.
        snapshot(&cfg, "package_deleting", pkg).unwrap();
        crate::ea::run_batch(&cfg, &[json!({"op":"delete","package":pkg})], &crate::ea::BatchOptions::default()).expect("delete package");
        let id = snapshots(&cfg)[0].file_stem().unwrap().to_string_lossy().to_string();
        let restore = restore_ops(&json!({ "id": id }), &cfg).unwrap();
        let done = crate::ea::run_batch(&cfg, &restore, &crate::ea::BatchOptions::default()).expect("restore package");
        let new_pkg = done["created"].as_array().unwrap().iter().find(|c| c["key"] == format!("p{pkg}")).expect("package")["id"].as_i64().unwrap();
        let names = q(&format!("select name from t_object where package_id = {new_pkg} order by name"));
        if names.iter().map(|r| r[0].as_str()).collect::<Vec<_>>() != ["Customer", "Invoice"] { problems.push(format!("package elements {names:?}")); }
        let diagrams = q(&format!("select diagram_id from t_diagram where package_id = {new_pkg}"));
        if diagrams.len() != 1 || q(&format!("select object_id from t_diagramobjects where diagram_id = {}", diagrams[0][0])).len() != 2 { problems.push(format!("diagram {diagrams:?}")); }
        for p in q("select package_id from t_package where name like 'ZZ_safety_%'") {
            crate::ea::run_batch(&cfg, &[json!({"op":"delete","package":p[0].parse::<i64>().unwrap()})], &crate::ea::BatchOptions::default()).expect("cleanup");
        }
        crate::ea::shutdown();
        let _ = std::fs::remove_dir_all(&cfg.base_dir);
        assert!(problems.is_empty(), "{problems:?}");
    }
}

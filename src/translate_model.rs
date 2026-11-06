//! Translating the content of the model (LC-400, /translate-model): names and notes of packages, elements, attributes,
//! operations and diagrams of a package, into Polish, English, German or French, keeping every id and GUID (the
//! objects are changed, not replaced: relationships, diagrams and references stay). The assistant gets the texts in
//! numbered portions (action texts), translates them, and saves its translations (action apply) – one ordinary save
//! per portion (approval card, undo journal); optionally the original goes into a tagged value (e.g. name.pl).

use crate::config::AppConfig;
use crate::tools::query_rows;
use anyhow::{bail, Context, Result};
use serde_json::{json, Value};

/// Texts per portion (the assistant translates one portion at a time).
const PORTION: usize = 80;

pub fn definition() -> Value {
    json!({
        "name": "translate_model",
        "description": "Translate the content of a package (with child packages) into another language while keeping every id and GUID. \
action texts: the names and notes to translate, in numbered portions of 80 (portion 1, 2, …) – each item has its key (e.g. element:42:name). \
Translate them yourself (keep names short, as names; keep the formatting of notes; do not translate code, identifiers, stereotypes or \
abbreviations the team uses), then action apply with translations [{key, text}] for that portion – one save the user approves, undoable. \
keep_original (e.g. \"pl\") also stores the original text as a tagged value name.pl / notes.pl on elements. Go portion by portion.",
        "input_schema": { "type": "object", "properties": {
            "action": { "type": "string", "enum": ["texts", "apply"] },
            "package": { "description": "package_id or exact name" },
            "portion": { "type": "integer", "description": "texts: which portion (1 = the first)" },
            "fields": { "type": "array", "items": { "type": "string", "enum": ["name", "notes"] }, "description": "What to translate (default both)" },
            "translations": { "type": "array", "items": { "type": "object", "properties": { "key": { "type": "string" }, "text": { "type": "string" } }, "required": ["key", "text"] } },
            "keep_original": { "type": "string", "description": "apply: a language code; the original goes into tagged values name.<code> / notes.<code> of elements" }
        }, "required": ["action", "package"] }
    })
}

/// Every text of the package: (key, text) with key = kind:id:field, in a fixed order.
fn texts(cfg: &AppConfig, package: &Value, fields: &[String]) -> Result<(String, Vec<(String, String)>)> {
    let (scope, title) = crate::model_usage::package_scope(cfg, package)?;
    let ids: String = scope.split('(').nth(1).unwrap_or("").trim_end_matches(')').to_string();
    if ids.is_empty() {
        bail!("package: give the package to translate");
    }
    let want = |f: &str| fields.is_empty() || fields.iter().any(|x| x == f);
    let mut out = Vec::new();
    let mut add = |kind: &str, id: &str, name: &str, notes: &str| {
        if want("name") && !name.trim().is_empty() {
            out.push((format!("{kind}:{id}:name"), name.to_string()));
        }
        if want("notes") && !notes.trim().is_empty() {
            out.push((format!("{kind}:{id}:notes"), notes.to_string()));
        }
    };
    for r in query_rows(cfg, &format!("select package_id, name, coalesce(notes,'') from t_package where package_id in ({ids}) order by package_id"))? {
        add("package", &r[0], &r[1], &r[2]);
    }
    let elements = query_rows(cfg, &format!("select object_id, coalesce(name,''), coalesce(note,'') from t_object where package_id in ({ids}) and object_type <> 'Package' order by object_id"))?;
    for r in &elements {
        add("element", &r[0], &r[1], &r[2]);
    }
    let element_ids: Vec<String> = elements.iter().map(|r| r[0].clone()).collect();
    for chunk in element_ids.chunks(500) {
        let list = chunk.join(",");
        for r in query_rows(cfg, &format!("select id, name, coalesce(notes,'') from t_attribute where object_id in ({list}) order by object_id, pos"))? {
            add("attribute", &r[0], &r[1], &r[2]);
        }
        for r in query_rows(cfg, &format!("select operationid, name, coalesce(notes,'') from t_operation where object_id in ({list}) order by object_id, pos"))? {
            add("operation", &r[0], &r[1], &r[2]);
        }
    }
    for r in query_rows(cfg, &format!("select diagram_id, name, coalesce(notes,'') from t_diagram where package_id in ({ids}) order by diagram_id"))? {
        add("diagram", &r[0], &r[1], &r[2]);
    }
    Ok((title, out))
}

pub fn run(input: &Value, cfg: &AppConfig) -> Result<(Vec<Value>, String)> {
    let fields: Vec<String> = input["fields"].as_array().into_iter().flatten().filter_map(|f| f.as_str().map(String::from)).collect();
    match input["action"].as_str().unwrap_or("texts") {
        "texts" => {
            let (title, all) = texts(cfg, &input["package"], &fields)?;
            let portions = all.len().div_ceil(PORTION).max(1);
            let portion = input["portion"].as_u64().unwrap_or(1).max(1) as usize;
            if portion > portions {
                bail!("there are {portions} portion(s)");
            }
            let items: Vec<Value> = all.iter().skip((portion - 1) * PORTION).take(PORTION).map(|(k, t)| json!({ "key": k, "text": t })).collect();
            Ok((Vec::new(), format!(
                "Texts of the {title} to translate: portion {portion} of {portions} ({} of {} texts). Translate each text and call apply with [{{key, text}}] for this portion.\n{}",
                items.len(),
                all.len(),
                serde_json::to_string_pretty(&items)?
            )))
        }
        "apply" => {
            let list = input["translations"].as_array().context("translations: [{key, text}]")?;
            // Only keys of this package: a key of another package is refused (the ids must be the package's own).
            let (_, all) = texts(cfg, &input["package"], &[])?;
            let originals: std::collections::HashMap<&str, &str> = all.iter().map(|(k, t)| (k.as_str(), t.as_str())).collect();
            let keep = input["keep_original"].as_str().map(str::trim).filter(|c| !c.is_empty() && c.chars().all(|ch| ch.is_ascii_alphabetic()) && c.len() <= 5);
            let mut ops = Vec::new();
            let mut unknown = Vec::new();
            let mut same = 0;
            for t in list {
                let (key, text) = (t["key"].as_str().unwrap_or(""), t["text"].as_str().unwrap_or("").trim_end());
                let Some(original) = originals.get(key) else {
                    unknown.push(key.to_string());
                    continue;
                };
                if text.is_empty() || text == *original {
                    same += 1;
                    continue;
                }
                let mut parts = key.splitn(3, ':');
                let (kind, id, field) = (parts.next().unwrap_or(""), parts.next().unwrap_or("").parse::<i64>().unwrap_or(0), parts.next().unwrap_or(""));
                let mut op = match kind {
                    "package" => json!({ "op": "update_package", "package": id }),
                    "element" => json!({ "op": "update_element", "element": id }),
                    "attribute" => json!({ "op": "update_attribute", "attribute": id }),
                    "operation" => json!({ "op": "update_operation", "operation": id }),
                    "diagram" => json!({ "op": "update_diagram", "diagram": id }),
                    _ => continue,
                };
                op[field] = json!(text);
                if let (Some(code), "element") = (keep, kind) {
                    let original: String = original.chars().take(255).collect();
                    op["tags"] = json!({ format!("{field}.{code}"): original });
                }
                ops.push(op);
            }
            if !unknown.is_empty() {
                bail!("{} key(s) are not texts of this package (take them from action texts): {}", unknown.len(), unknown.iter().take(10).cloned().collect::<Vec<_>>().join(", "));
            }
            if ops.is_empty() {
                return Ok((Vec::new(), "Nothing to change: the translations are the same as the texts.".into()));
            }
            Ok((ops.clone(), format!("{} text(s) translated{}.", ops.len(), if same > 0 { format!(", {same} unchanged") } else { String::new() })))
        }
        other => bail!("unknown action {other}: texts or apply"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `LOONAR_TEST_PROPS=… cargo test translate_model_live -- --ignored --nocapture`: texts, then a translation of one element
    /// saved through EA and checked (same id), undone by saving the original back.
    #[test]
    #[ignore = "requires EA and a copy of a test model"]
    fn translate_model_live() {
        let cfg = AppConfig::load(std::path::Path::new(&std::env::var("LOONAR_TEST_PROPS").unwrap())).unwrap();
        let root: i64 = query_rows(&cfg, "select package_id from t_package where parent_id = 0 order by package_id").unwrap()[0][0].parse().unwrap();
        let created = crate::ea::run_batch(&cfg, &[
            json!({"op":"create_package","key":"pkg","parent":root,"name":format!("ZZ_tr_{}", chrono::Local::now().format("%H%M%S"))}),
            json!({"op":"create_element","key":"a","package":"#pkg","name":"Zamówienie","type":"Class","notes":"Zamówienie klienta."}),
            json!({"op":"create_attribute","element":"#a","name":"data","type":"date"}),
        ], &crate::ea::BatchOptions { rollback_key: Some("pkg".into()), keep_locks: false }).unwrap();
        let id_of = |key: &str| created["created"].as_array().unwrap().iter().find(|c| c["key"] == key).unwrap()["id"].as_i64().unwrap();
        let (pkg, a) = (id_of("pkg"), id_of("a"));
        let (_, text) = run(&json!({ "action": "texts", "package": pkg }), &cfg).unwrap();
        println!("{text}");
        assert!(text.contains(&format!("element:{a}:name")) && text.contains("attribute:"), "{text}");
        let (ops, said) = run(&json!({ "action": "apply", "package": pkg, "keep_original": "pl", "translations": [
            { "key": format!("element:{a}:name"), "text": "Order" }, { "key": format!("element:{a}:notes"), "text": "A customer's order." } ] }), &cfg).unwrap();
        println!("{said} {ops:?}");
        crate::ea::run_batch(&cfg, &ops, &crate::ea::BatchOptions::default()).unwrap();
        let row = query_rows(&cfg, &format!("select name, note from t_object where object_id = {a}")).unwrap();
        let tag = query_rows(&cfg, &format!("select value from t_objectproperties where object_id = {a} and property = 'name.pl'")).unwrap();
        assert!(run(&json!({ "action": "apply", "package": pkg, "translations": [{ "key": "element:1:name", "text": "x" }] }), &cfg).is_err(), "a key of another package is refused");
        crate::ea::run_batch(&cfg, &[json!({"op":"delete","package":pkg})], &crate::ea::BatchOptions::default()).unwrap();
        crate::ea::shutdown();
        assert_eq!(row[0][0], "Order");
        assert!(row[0][1].contains("customer's order"), "{row:?}");
        assert_eq!(tag.first().map(|r| r[0].as_str()), Some("Zamówienie"));
    }
}

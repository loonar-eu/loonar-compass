//! A text export of a package for Git (LC-398, /export-text): every package, element, attribute, operation, tagged
//! value and relationship under its GUID, one property per line, in a fixed order – so two exports differ only where
//! the model changed, and Git shows a readable diff. `diff` compares an earlier export (attached) with the model now
//! and says what was added, removed and changed, property by property. Reads only.

use crate::config::AppConfig;
use crate::tools::query_rows;
use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use std::collections::BTreeMap;

pub fn definition() -> Value {
    json!({
        "name": "export_text",
        "description": "A text export of a package (with child packages) for Git: every package, element, attribute, operation, tagged value and \
relationship under its GUID, one property per line, in a fixed order, so two exports differ only where the model changed. action export: the \
.lcmodel file for the user (shown in the chat with Open and Save as; they keep it in their repository). action diff: an earlier export \
attached to the conversation (file) against the model now – added, removed and changed objects, property by property, by name. Reads only.",
        "input_schema": { "type": "object", "properties": {
            "action": { "type": "string", "enum": ["export", "diff"] },
            "package": { "description": "package_id or exact name" },
            "file": { "type": "string", "description": "diff: the attached earlier .lcmodel export" }
        }, "required": ["action"] }
    })
}

/// One line of a value: line breaks written as \n, backslashes doubled.
fn one_line(text: &str) -> String {
    text.replace('\\', "\\\\").replace("\r\n", "\n").replace('\n', "\\n").replace('\r', "")
}

/// The export: GUID → its properties (name → value), in a fixed order.
type Export = BTreeMap<String, BTreeMap<String, String>>;

fn collect(cfg: &AppConfig, package: &Value) -> Result<(String, Export)> {
    let (scope, title) = crate::model_usage::package_scope(cfg, package)?;
    let ids: Vec<String> = scope.split('(').nth(1).unwrap_or("").trim_end_matches(')').split(',').filter(|s| !s.is_empty()).map(String::from).collect();
    if ids.is_empty() {
        bail!("package: give the package to export");
    }
    let list = ids.join(",");
    let mut out = Export::new();
    let mut put = |guid: &str, key: &str, value: &str| {
        out.entry(guid.to_string()).or_default().insert(key.to_string(), one_line(value));
    };
    let pkg_guid: std::collections::HashMap<String, String> = query_rows(cfg, &format!("select package_id, ea_guid from t_package where package_id in ({list})"))?.into_iter().map(|r| (r[0].clone(), r[1].clone())).collect();
    for r in query_rows(cfg, &format!("select ea_guid, name, parent_id, coalesce(notes,''), coalesce(version,'') from t_package where package_id in ({list})"))? {
        put(&r[0], "kind", "package");
        put(&r[0], "name", &r[1]);
        put(&r[0], "parent", pkg_guid.get(&r[2]).map(String::as_str).unwrap_or(""));
        put(&r[0], "notes", &r[3]);
        put(&r[0], "version", &r[4]);
    }
    let objects = query_rows(cfg, &format!(
        "select object_id, ea_guid, name, object_type, coalesce(stereotype,''), coalesce(status,''), coalesce(alias,''), coalesce(note,''), package_id, coalesce(phase,''), coalesce(version,''), coalesce(author,'') \
         from t_object where object_type <> 'Package' and package_id in ({list})"
    ))?;
    let guid_of: std::collections::HashMap<String, String> = objects.iter().map(|r| (r[0].clone(), r[1].clone())).collect();
    for r in &objects {
        put(&r[1], "kind", "element");
        put(&r[1], "name", &r[2]);
        put(&r[1], "type", &r[3]);
        put(&r[1], "stereotype", &r[4]);
        put(&r[1], "status", &r[5]);
        put(&r[1], "alias", &r[6]);
        put(&r[1], "notes", &r[7]);
        put(&r[1], "package", pkg_guid.get(&r[8]).map(String::as_str).unwrap_or(""));
        put(&r[1], "phase", &r[9]);
        put(&r[1], "version", &r[10]);
        put(&r[1], "author", &r[11]);
    }
    let object_ids: Vec<String> = objects.iter().map(|r| r[0].clone()).collect();
    for chunk in object_ids.chunks(500) {
        let ids = chunk.join(",");
        for r in query_rows(cfg, &format!("select object_id, property, coalesce(value,'') from t_objectproperties where object_id in ({ids})"))? {
            if let Some(g) = guid_of.get(&r[0]) {
                put(g, &format!("tag.{}", r[1]), &r[2]);
            }
        }
        for r in query_rows(cfg, &format!("select ea_guid, object_id, name, coalesce(type,''), coalesce(notes,''), coalesce(scope,''), coalesce(stereotype,'') from t_attribute where object_id in ({ids})"))? {
            put(&r[0], "kind", "attribute");
            put(&r[0], "owner", guid_of.get(&r[1]).map(String::as_str).unwrap_or(""));
            put(&r[0], "name", &r[2]);
            put(&r[0], "type", &r[3]);
            put(&r[0], "notes", &r[4]);
            put(&r[0], "visibility", &r[5]);
            put(&r[0], "stereotype", &r[6]);
        }
        for r in query_rows(cfg, &format!("select ea_guid, object_id, name, coalesce(type,''), coalesce(notes,''), coalesce(scope,''), coalesce(stereotype,''), coalesce(style,'') from t_operation where object_id in ({ids})"))? {
            put(&r[0], "kind", "operation");
            put(&r[0], "owner", guid_of.get(&r[1]).map(String::as_str).unwrap_or(""));
            put(&r[0], "name", &r[2]);
            put(&r[0], "returns", &r[3]);
            put(&r[0], "notes", &r[4]);
            put(&r[0], "visibility", &r[5]);
            put(&r[0], "stereotype", &r[6]);
            put(&r[0], "alias", &r[7]);
        }
        // Relationships that start in the package (the other end may be elsewhere: its GUID is written).
        for r in query_rows(cfg, &format!(
            "select c.ea_guid, c.connector_type, coalesce(c.stereotype,''), coalesce(c.name,''), s.ea_guid, t.ea_guid, coalesce(c.notes,''), coalesce(c.direction,'') \
             from t_connector c join t_object s on s.object_id = c.start_object_id join t_object t on t.object_id = c.end_object_id where c.start_object_id in ({ids})"
        ))? {
            put(&r[0], "kind", "relationship");
            put(&r[0], "type", &r[1]);
            put(&r[0], "stereotype", &r[2]);
            put(&r[0], "name", &r[3]);
            put(&r[0], "source", &r[4]);
            put(&r[0], "target", &r[5]);
            put(&r[0], "notes", &r[6]);
            put(&r[0], "direction", &r[7]);
        }
    }
    // Empty properties are left out (fewer lines; an empty value and a missing one mean the same).
    for props in out.values_mut() {
        props.retain(|k, v| !v.is_empty() || k == "name");
    }
    Ok((title, out))
}

fn write(title: &str, export: &Export) -> String {
    let mut text = format!("# Loonar Compass text export of the {title}\n# One block per object, by GUID; one property per line.\n");
    for (guid, props) in export {
        text.push_str(&format!("\n[{guid}]\n"));
        // kind and name first, the rest by name.
        for key in ["kind", "name"] {
            if let Some(v) = props.get(key) {
                text.push_str(&format!("{key} = {v}\n"));
            }
        }
        for (k, v) in props.iter().filter(|(k, _)| *k != "kind" && *k != "name") {
            text.push_str(&format!("{k} = {v}\n"));
        }
    }
    text
}

pub(crate) fn parse(text: &str) -> Export {
    let mut out = Export::new();
    let mut current: Option<String> = None;
    for line in text.lines() {
        let line = line.trim_end_matches('\r');
        if line.starts_with('#') || line.trim().is_empty() {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            current = Some(line[1..line.len() - 1].to_string());
            out.entry(current.clone().unwrap_or_default()).or_default();
            continue;
        }
        if let (Some(guid), Some((k, v))) = (&current, line.split_once(" = ")) {
            out.entry(guid.clone()).or_default().insert(k.to_string(), v.to_string());
        }
    }
    out
}

/// What changed between two exports, in words.
pub(crate) fn compare(before: &Export, now: &Export) -> Vec<String> {
    let label = |props: &BTreeMap<String, String>| format!("{} \"{}\"", props.get("kind").map(String::as_str).unwrap_or("object"), props.get("name").map(String::as_str).unwrap_or(""));
    let mut out = Vec::new();
    for (guid, props) in now {
        match before.get(guid) {
            None => out.push(format!("+ added {}", label(props))),
            Some(old) => {
                let mut changes = Vec::new();
                for key in old.keys().chain(props.keys()).collect::<std::collections::BTreeSet<_>>() {
                    let (a, b) = (old.get(key).map(String::as_str).unwrap_or(""), props.get(key).map(String::as_str).unwrap_or(""));
                    if a != b {
                        let short = |t: &str| if t.chars().count() > 80 { format!("{}…", t.chars().take(80).collect::<String>()) } else { t.to_string() };
                        changes.push(format!("{key}: \"{}\" → \"{}\"", short(a), short(b)));
                    }
                }
                if !changes.is_empty() {
                    out.push(format!("~ changed {}: {}", label(props), changes.join("; ")));
                }
            }
        }
    }
    for (guid, props) in before {
        if !now.contains_key(guid) {
            out.push(format!("- removed {}", label(props)));
        }
    }
    out
}

pub fn run(input: &Value, cfg: &AppConfig) -> Result<crate::tools::ToolOutput> {
    match input["action"].as_str().unwrap_or("export") {
        "export" => {
            let (title, export) = collect(cfg, &input["package"])?;
            let text = write(&title, &export);
            let name = query_rows(cfg, &format!("select name from t_package where package_id = {}", input["package"].as_i64().unwrap_or(0))).ok().and_then(|r| r.first().map(|r| r[0].clone()))
                .or_else(|| input["package"].as_str().map(String::from))
                .unwrap_or_else(|| "model".into());
            std::fs::create_dir_all(&cfg.attachments_dir).context(crate::i18n::tr("attachments folder"))?;
            let stored = format!("{}_{}.lcmodel", uuid::Uuid::new_v4().simple(), crate::history::sanitize_name(&name));
            std::fs::write(cfg.attachments_dir.join(&stored), text.as_bytes())?;
            Ok(crate::tools::ToolOutput {
                text: format!("Text export of the {title}: {} objects, {} lines. The user sees the file in the chat with Open and Save as…; kept in a Git repository, the next export shows the changes as a diff. Do not name a folder for it.", export.len(), text.lines().count()),
                images: Vec::new(),
                files: vec![stored],
            })
        }
        "diff" => {
            let file = std::path::Path::new(input["file"].as_str().unwrap_or("")).file_name().context("file: the earlier export attached to the conversation")?.to_string_lossy().to_string();
            let before_text = std::fs::read_to_string(crate::history::attachment_path(&cfg.attachments_dir, &file)).with_context(|| format!("there is no attachment {file}"))?;
            let before = parse(&before_text);
            if before.is_empty() {
                bail!("{file} is not a text export of Compass");
            }
            // The package of the earlier export: given, or the root package in the file.
            let package = if input["package"].is_null() || input["package"].as_str() == Some("") {
                let root = before.iter().find(|(_, p)| p.get("kind").map(String::as_str) == Some("package") && !p.get("parent").is_some_and(|parent| before.contains_key(parent)));
                let guid = root.map(|(g, _)| g.clone()).context("the export has no package; give the package")?;
                let id = query_rows(cfg, &format!("select package_id from t_package where ea_guid = '{}'", crate::model_db::sql_text(cfg.model_source, &guid)))?
                    .first().map(|r| r[0].clone()).context("the package of the export is no longer in the model")?;
                json!(id.parse::<i64>().unwrap_or(0))
            } else {
                input["package"].clone()
            };
            let (title, now) = collect(cfg, &package)?;
            let changes = compare(&before, &now);
            let text = if changes.is_empty() {
                format!("{file} and the {title} now: no differences.")
            } else {
                let shown: Vec<&String> = changes.iter().take(300).collect();
                format!("{file} → the {title} now: {} difference(s)\n{}{}", changes.len(), shown.iter().map(|c| c.as_str()).collect::<Vec<_>>().join("\n"), if changes.len() > 300 { "\n…" } else { "" })
            };
            Ok(text.into())
        }
        other => bail!("unknown action {other}: export or diff"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn export_round_trip_and_diff() {
        let mut a = Export::new();
        a.entry("{1}".into()).or_default().extend([("kind".to_string(), "element".to_string()), ("name".into(), "Order".into()), ("notes".into(), one_line("Two\nlines \\ ok"))]);
        a.entry("{2}".into()).or_default().extend([("kind".to_string(), "element".to_string()), ("name".into(), "Gone".into())]);
        let text = write("package \"Shop\"", &a);
        assert_eq!(parse(&text), a);
        assert!(text.contains("notes = Two\\nlines \\\\ ok"), "{text}");
        let mut b = a.clone();
        b.remove("{2}");
        b.get_mut("{1}").unwrap().insert("status".into(), "Approved".into());
        b.entry("{3}".into()).or_default().extend([("kind".to_string(), "element".to_string()), ("name".into(), "New".into())]);
        let changes = compare(&a, &b);
        assert!(changes.iter().any(|c| c == "~ changed element \"Order\": status: \"\" → \"Approved\""), "{changes:?}");
        assert!(changes.contains(&"+ added element \"New\"".to_string()) && changes.contains(&"- removed element \"Gone\"".to_string()), "{changes:?}");
    }

    /// `LOONAR_TEST_PROPS=… cargo test export_text_live -- --ignored --nocapture`: two exports of the same package are equal.
    #[test]
    #[ignore = "requires a test model"]
    fn export_text_live() {
        let cfg = AppConfig::load(std::path::Path::new(&std::env::var("LOONAR_TEST_PROPS").unwrap())).unwrap();
        let package = query_rows(&cfg, "select package_id from t_package where parent_id <> 0 and package_id in (select package_id from t_object) order by package_id").unwrap()[0][0].clone();
        let (title, one) = collect(&cfg, &json!(package.parse::<i64>().unwrap())).unwrap();
        let (_, two) = collect(&cfg, &json!(package.parse::<i64>().unwrap())).unwrap();
        let text = write(&title, &one);
        println!("{}", text.lines().take(30).collect::<Vec<_>>().join("\n"));
        assert_eq!(write(&title, &two), text, "the export is stable");
        assert_eq!(parse(&text), one);
        assert!(compare(&one, &two).is_empty());
    }
}

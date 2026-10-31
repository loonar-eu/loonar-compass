//! What was published from the model and whether it is still current (LC-386): every Confluence page and Word document
//! made from the model is recorded with the diagrams it shows and when; /stale-pages lists the ones whose diagrams – or
//! the elements on them – changed since, so they can be published again. EA's own images in Confluence simply age.

use crate::config::AppConfig;
use crate::tools::query_rows;
use anyhow::Result;
use serde_json::{json, Value};

pub fn definition() -> Value {
    json!({
        "name": "stale_publications",
        "description": "Pages and documents published from the model (Confluence pages, Word documents) that are no longer current: the diagrams \
they show, or elements on those diagrams, changed after the publication. Lists each with what changed, so it can be published again \
(publish_confluence with the same page, or a new document). Reads only. Use it for /stale-pages and before a review.",
        "input_schema": { "type": "object", "properties": {} }
    })
}

fn file(cfg: &AppConfig) -> std::path::PathBuf {
    cfg.base_dir.join("publications.json")
}

/// The diagram ids a page or document shows (diagram:ID in its text).
pub(crate) fn diagrams_in(markdown: &str) -> Vec<i64> {
    let mut ids: Vec<i64> = markdown
        .match_indices("diagram:")
        .filter_map(|(at, _)| markdown[at + 8..].chars().take_while(char::is_ascii_digit).collect::<String>().parse().ok())
        .collect();
    ids.sort();
    ids.dedup();
    ids
}

/// Records a publication (the same target replaces its earlier record).
pub fn record(cfg: &AppConfig, kind: &str, title: &str, target: &str, markdown: &str) {
    let diagrams = diagrams_in(markdown);
    if diagrams.is_empty() {
        return;
    }
    let mut all: Vec<Value> = std::fs::read_to_string(file(cfg)).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
    all.retain(|e| !(e["kind"] == kind && e["target"] == target));
    all.push(json!({ "kind": kind, "title": title, "target": target, "model": crate::undo::model_key(cfg), "diagrams": diagrams,
                     "published": chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string() }));
    let tmp = file(cfg).with_extension("json.tmp");
    if std::fs::write(&tmp, serde_json::to_string_pretty(&all).unwrap_or_default()).is_ok() {
        let _ = std::fs::rename(&tmp, file(cfg));
    }
}

pub fn stale(_input: &Value, cfg: &AppConfig) -> Result<String> {
    let all: Vec<Value> = std::fs::read_to_string(file(cfg)).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
    let key = crate::undo::model_key(cfg);
    let mine: Vec<&Value> = all.iter().filter(|e| e["model"] == key.as_str()).collect();
    let mut out = String::new();
    let mut stale = 0;
    for e in &mine {
        let published = e["published"].as_str().unwrap_or("");
        let list = e["diagrams"].as_array().into_iter().flatten().filter_map(Value::as_i64).map(|d| d.to_string()).collect::<Vec<_>>().join(",");
        if list.is_empty() {
            continue;
        }
        let since = crate::changes::date_text(cfg.model_source, published);
        let diagrams = query_rows(cfg, &format!("select name from t_diagram where diagram_id in ({list}) and modifieddate > {since}")).unwrap_or_default();
        let elements = query_rows(cfg, &format!(
            "select distinct o.name from t_object o join t_diagramobjects x on x.object_id = o.object_id where x.diagram_id in ({list}) and o.modifieddate > {since}"
        ))
        .unwrap_or_default();
        if diagrams.is_empty() && elements.is_empty() {
            continue;
        }
        stale += 1;
        let names = |rows: &[Vec<String>]| rows.iter().take(8).map(|r| format!("\"{}\"", r[0])).collect::<Vec<_>>().join(", ");
        out.push_str(&format!(
            "\n- {} \"{}\" ({}), published {published}:{}{}\n",
            e["kind"].as_str().unwrap_or(""),
            e["title"].as_str().unwrap_or(""),
            e["target"].as_str().unwrap_or(""),
            if diagrams.is_empty() { String::new() } else { format!(" diagrams changed since: {}.", names(&diagrams)) },
            if elements.is_empty() { String::new() } else { format!(" Elements on them changed since: {}{}.", names(&elements), if elements.len() > 8 { ", …" } else { "" }) }
        ));
    }
    Ok(format!("Publications from this model: {}, no longer current: {stale}.{out}", mine.len()))
}

#[cfg(test)]
mod tests {
    #[test]
    fn diagram_references() {
        assert_eq!(super::diagrams_in("![Overview](diagram:12)\ntext ![x](diagram:7) and again diagram:12, image:abc"), vec![7, 12]);
        assert!(super::diagrams_in("no pictures").is_empty());
    }

    /// LC-386: a publication older than the changes of its diagram is listed, a newer one is not (model copy, reads only):
    /// `LOONAR_TEST_PROPS=E:/livetest/undo/undo.properties cargo test stale_publications_live -- --ignored --nocapture`
    #[test]
    #[ignore = "requires the model from LOONAR_TEST_PROPS"]
    fn stale_publications_live() {
        let mut cfg = crate::config::AppConfig::load(std::path::Path::new(&std::env::var("LOONAR_TEST_PROPS").expect("LOONAR_TEST_PROPS"))).expect("app.properties");
        let dir = std::env::temp_dir().join(format!("loonar-pub-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        cfg.base_dir = dir.clone();
        let d: i64 = crate::tools::query_rows(&cfg, "select diagram_id from t_diagram order by modifieddate desc").unwrap()[0][0].parse().unwrap();
        super::record(&cfg, "Confluence page", "Old page", "https://wiki/old", &format!("![x](diagram:{d})"));
        super::record(&cfg, "Word document", "New document", "new.docx", &format!("![x](diagram:{d})"));
        // The first one as if published long ago.
        let text = std::fs::read_to_string(super::file(&cfg)).unwrap().replacen(&chrono::Local::now().format("%Y-%m-%d").to_string(), "2000-01-01", 1);
        std::fs::write(super::file(&cfg), text).unwrap();
        let report = super::stale(&serde_json::json!({}), &cfg).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        println!("{report}");
        assert!(report.contains("Old page") && !report.contains("New document"), "{report}");
    }
}

//! Migration copilot (LC-394, /migration-check): before moving a model (.eap/.eapx → .qea or a database server, or one
//! server to another) or before installing a new EA build – what is in the model, what could break on the way (texts
//! longer than the target's columns, locks, checked-out packages, integrity problems), and the steps in EA; after the
//! move, the row counts of this model and the other one compared table by table. Reads only.

use crate::config::{AppConfig, ModelSource};
use crate::tools::query_rows;
use anyhow::{bail, Context, Result};
use serde_json::{json, Value};

pub fn definition() -> Value {
    json!({
        "name": "migration_check",
        "description": "Migration copilot. action readiness: before moving the model to another repository type (e.g. .eapx → .qea, \
a file → SQL Server/PostgreSQL) or installing a new EA build – EA's version, the model's size and row counts, texts longer than 255 \
characters in names, aliases, stereotypes and tagged values (they are cut by some targets), locks and checked-out packages to release \
first, integrity problems (as /repo-doctor), and the steps in EA (backup, Project Integrity Check, Model Transfer). action compare: after \
the transfer, the row counts of this model and another one (a .qea/.qeax file or an app.properties of a database) table by table – \
tables that differ are named. Reads only.",
        "input_schema": { "type": "object", "properties": {
            "action": { "type": "string", "enum": ["readiness", "compare"] },
            "other": { "type": "string", "description": "compare: full path of the other .qea/.qeax file or app.properties" }
        }, "required": ["action"] }
    })
}

/// The tables a transfer must carry over whole.
const TABLES: &[&str] = &[
    "t_package", "t_object", "t_connector", "t_diagram", "t_diagramobjects", "t_diagramlinks", "t_attribute", "t_operation", "t_operationparams",
    "t_objectproperties", "t_connectortag", "t_attributetag", "t_operationtag", "t_xref", "t_document", "t_glossary", "t_objectrequires",
    "t_objectconstraint", "t_objecttests", "t_objectfiles", "t_image", "t_stereotypes", "t_secuser", "t_secgroup", "t_seclocks", "t_snapshot",
];

fn counts(cfg: &AppConfig) -> Vec<(String, Option<i64>)> {
    TABLES
        .iter()
        .map(|t| (t.to_string(), query_rows(cfg, &format!("select count(*) from {t}")).ok().and_then(|r| r.first().and_then(|r| r[0].parse().ok()))))
        .collect()
}

fn length_fn(cfg: &AppConfig) -> &'static str {
    if cfg.model_source == ModelSource::SqlServer { "len" } else { "length" }
}

fn readiness(cfg: &AppConfig) -> Result<String> {
    let mut out = vec![format!("Migration readiness of {}", cfg.model_label())];
    let version = crate::ea_help::ea_version(cfg);
    out.push(format!("EA: {}", if version.is_empty() { "version not known (ea.path)".to_string() } else { version }));
    if cfg.uses_file() {
        let size = std::fs::metadata(&cfg.model_file).map(|m| m.len()).unwrap_or(0);
        out.push(format!("File: {} ({:.1} MB)", cfg.model_file.display(), size as f64 / 1_048_576.0));
    }
    out.push("\nRows per table:".into());
    for (table, n) in counts(cfg) {
        if let Some(n) = n.filter(|n| *n > 0) {
            out.push(format!("  {table}: {n}"));
        }
    }
    // Texts that do not fit a 255-character column of some targets.
    let len = length_fn(cfg);
    let mut long = Vec::new();
    for (table, column, what) in [("t_object", "name", "element names"), ("t_object", "alias", "element aliases"), ("t_object", "stereotype", "element stereotypes"), ("t_objectproperties", "value", "tagged values"), ("t_connector", "name", "relationship names"), ("t_attribute", "name", "attribute names")] {
        if let Ok(rows) = query_rows(cfg, &format!("select count(*) from {table} where {len}({column}) > 255")) {
            let n: i64 = rows.first().and_then(|r| r[0].parse().ok()).unwrap_or(0);
            if n > 0 {
                long.push(format!("  {n} {what} longer than 255 characters ({table}.{column})"));
            }
        }
    }
    out.push(if long.is_empty() { "\nTexts longer than 255 characters: none.".to_string() } else { format!("\nTexts longer than 255 characters (cut by some targets; tagged values above 255 belong in their notes):\n{}", long.join("\n")) });
    let locks: i64 = query_rows(cfg, "select count(*) from t_seclocks").ok().and_then(|r| r.first().and_then(|r| r[0].parse().ok())).unwrap_or(0);
    let checked_out: Vec<String> = query_rows(cfg, "select name from t_package where packageflags like '%CheckedOutTo=%'").map(|r| r.into_iter().map(|r| r[0].clone()).collect()).unwrap_or_default();
    out.push(format!("\nLocks to release first: {locks}"));
    out.push(format!("Packages checked out in version control: {}{}", checked_out.len(), if checked_out.is_empty() { String::new() } else { format!(" ({})", checked_out.iter().take(10).cloned().collect::<Vec<_>>().join(", ")) }));
    out.push(format!("\n{}", crate::repo_doctor::run(&json!({}), cfg).unwrap_or_else(|e| format!("Integrity: {e:#}"))));
    out.push("\nSteps in EA:\n  1. A backup copy of the model (and of the database).\n  2. Check in or release locks and checked-out packages (above).\n  3. Settings → Model → Integrity → Project Integrity Check, \"Recover/Clean\" only after reading the report.\n  4. Settings → Model → Transfer → Full Project Transfer (the target created from EA's schema for that database).\n  5. /migration-check compare with the new model: every table must have the same number of rows.\n  6. Before a new EA build: open a copy with the new build first, and generate one document and one diagram export.".into());
    Ok(out.join("\n"))
}

fn compare(input: &Value, cfg: &AppConfig) -> Result<String> {
    let other = input["other"].as_str().map(str::trim).filter(|o| !o.is_empty()).context("other: the full path of the other model (.qea, .qeax or app.properties)")?;
    let path = std::path::PathBuf::from(other);
    if !path.is_file() {
        bail!("{other} is not a file");
    }
    let other_cfg = if crate::config::is_model_file(&path) {
        let mut c = cfg.clone();
        c.model_source = ModelSource::File;
        c.model_file = path.clone();
        c
    } else if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("properties")) {
        AppConfig::load(&path)?
    } else {
        bail!("{other}: Compass reads .qea/.qeax files and databases (an app.properties); an .eap/.eapx is read by EA only – compare after moving it to .qea");
    };
    let (here, there) = (counts(cfg), counts(&other_cfg));
    let mut differ = Vec::new();
    let mut same = 0;
    for ((table, a), (_, b)) in here.iter().zip(there.iter()) {
        match (a, b) {
            (Some(a), Some(b)) if a == b => same += 1,
            (Some(a), Some(b)) => differ.push(format!("  {table}: {a} here, {b} there ({:+})", b - a)),
            (None, None) => {}
            (a, b) => differ.push(format!("  {table}: {} here, {} there", a.map(|n| n.to_string()).unwrap_or("–".into()), b.map(|n| n.to_string()).unwrap_or("–".into()))),
        }
    }
    Ok(if differ.is_empty() {
        format!("{} and {}: all {same} tables have the same number of rows.", cfg.model_label(), other_cfg.model_label())
    } else {
        format!("{} and {}: {} table(s) differ ({same} equal):\n{}", cfg.model_label(), other_cfg.model_label(), differ.len(), differ.join("\n"))
    })
}

pub fn run(input: &Value, cfg: &AppConfig) -> Result<String> {
    match input["action"].as_str().unwrap_or("readiness") {
        "readiness" => readiness(cfg),
        "compare" => compare(input, cfg),
        other => bail!("unknown action {other}: readiness or compare"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `LOONAR_TEST_PROPS=… LOONAR_TEST_OTHER=<a .qea> cargo test migration_live -- --ignored --nocapture`
    #[test]
    #[ignore = "requires a test model"]
    fn migration_live() {
        let cfg = AppConfig::load(std::path::Path::new(&std::env::var("LOONAR_TEST_PROPS").unwrap())).unwrap();
        let ready = run(&json!({ "action": "readiness" }), &cfg).unwrap();
        println!("{ready}");
        assert!(ready.contains("t_object:") && ready.contains("Steps in EA"));
        let same = run(&json!({ "action": "compare", "other": cfg.model_file.display().to_string() }), &cfg).unwrap();
        assert!(same.contains("same number of rows"), "{same}");
        let other = run(&json!({ "action": "compare", "other": std::env::var("LOONAR_TEST_OTHER").unwrap() }), &cfg).unwrap();
        println!("{other}");
        assert!(other.contains("differ"));
    }
}

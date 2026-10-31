//! Two questions EA does not answer itself (reads only):
//! - why an element, diagram or package cannot be edited (LC-413, /why-locked): a lock (whose, since when), a group
//!   lock, a package under version control that is not checked out (or checked out by someone else), a locked diagram,
//!   a model file that is read-only;
//! - dead links (LC-427, /dead-links): diagram references, composite elements and $element/$diagram/$package links in
//!   notes that point at objects no longer in the model.

use crate::config::AppConfig;
use crate::tools::query_rows;
use anyhow::{bail, Result};
use serde_json::{json, Value};

pub fn definitions() -> Vec<Value> {
    vec![
        json!({
            "name": "why_locked",
            "description": "Why an element, diagram or package cannot be edited in EA (greyed out): locks with their owner and age (also on its \
package and parent packages), group locks, packages under version control that are not checked out or checked out by someone else, a \
locked diagram, a read-only model file, and whether the model requires user locks. Reads only; says who can release what. Use it for /why-locked.",
            "input_schema": { "type": "object", "properties": {
                "element": { "type": "integer" }, "diagram": { "type": "integer" }, "package": { "type": "integer" }
            } }
        }),
        json!({
            "name": "dead_links",
            "description": "Links in the model that point at nothing: diagram reference objects and composite elements whose diagram is gone, \
and $element://, $diagram:// and $package:// links in notes of elements and diagrams whose target is gone. Optionally only in a package \
(with child packages). Reads only. Use it for /dead-links and before publishing.",
            "input_schema": { "type": "object", "properties": { "package": { "description": "package_id or exact name (optional)" } } }
        }),
    ]
}

fn one(cfg: &AppConfig, sql: &str) -> Option<Vec<String>> {
    query_rows(cfg, sql).ok().and_then(|r| r.into_iter().next())
}

/// Locks of an entity (by GUID): owner and since when.
fn locks_of(cfg: &AppConfig, guid: &str) -> Vec<String> {
    let g = crate::model_db::sql_text(cfg.model_source, guid);
    query_rows(cfg, &format!(
        "select coalesce(u.userlogin,''), coalesce(gr.groupname,''), coalesce(l.timestamp,''), coalesce(l.locktype,'') from t_seclocks l \
         left join t_secuser u on u.userid = l.userid left join t_secgroup gr on gr.groupid = l.groupid where l.entityid = '{g}'"
    ))
    .unwrap_or_default()
    .into_iter()
    .map(|r| {
        let who = if !r[0].is_empty() { format!("the user {}", r[0]) } else if !r[1].is_empty() { format!("the group {}", r[1]) } else { "an account that no longer exists".into() };
        format!("locked by {who}{}{}", if r[2].is_empty() { String::new() } else { format!(" since {}", r[2].get(..16).unwrap_or(&r[2])) }, if r[3].is_empty() { String::new() } else { format!(" ({})", r[3]) })
    })
    .collect()
}

pub fn why_locked(input: &Value, cfg: &AppConfig) -> Result<String> {
    let (label, guid, mut package) = if let Some(e) = input["element"].as_i64() {
        let r = one(cfg, &format!("select name, ea_guid, package_id, object_type from t_object where object_id = {e}")).ok_or_else(|| anyhow::anyhow!("there is no element {e}"))?;
        (format!("{} \"{}\"", r[3], r[0]), r[1].clone(), r[2].parse::<i64>().unwrap_or(0))
    } else if let Some(d) = input["diagram"].as_i64() {
        let r = one(cfg, &format!("select name, ea_guid, package_id, coalesce(locked,0) from t_diagram where diagram_id = {d}")).ok_or_else(|| anyhow::anyhow!("there is no diagram {d}"))?;
        (format!("diagram \"{}\"{}", r[0], if r[3] != "0" && !r[3].is_empty() { " (the diagram itself is locked: Diagram → Lock Diagram)" } else { "" }), r[1].clone(), r[2].parse::<i64>().unwrap_or(0))
    } else if let Some(p) = input["package"].as_i64() {
        let r = one(cfg, &format!("select name, ea_guid from t_package where package_id = {p}")).ok_or_else(|| anyhow::anyhow!("there is no package {p}"))?;
        (format!("package \"{}\"", r[0]), r[1].clone(), p)
    } else {
        bail!("give element, diagram or package");
    };
    let mut findings = Vec::new();
    if label.contains("the diagram itself is locked") {
        findings.push("the diagram is locked for editing (unlock it in EA: Diagram → Lock Diagram)".to_string());
    }
    findings.extend(locks_of(cfg, &guid).into_iter().map(|l| format!("it is {l}")));
    // The package and its parents: locks and version control.
    for _ in 0..40 {
        let Some(p) = one(cfg, &format!("select name, ea_guid, parent_id, coalesce(iscontrolled,0), coalesce(packageflags,''), coalesce(protected,0) from t_package where package_id = {package}")) else { break };
        for l in locks_of(cfg, &p[1]) {
            findings.push(format!("its package \"{}\" is {l}", p[0]));
        }
        if p[3] == "1" || p[3].eq_ignore_ascii_case("true") {
            let out_to = p[4].split(';').find_map(|kv| kv.strip_prefix("CheckedOutTo=")).unwrap_or("");
            findings.push(if out_to.is_empty() {
                format!("the package \"{}\" is under version control and not checked out: check it out (Package Control → Check Out)", p[0])
            } else {
                format!("the package \"{}\" is under version control and checked out to {out_to}", p[0])
            });
        }
        if p[5] == "1" {
            findings.push(format!("the package \"{}\" is protected (read only)", p[0]));
        }
        package = p[2].parse().unwrap_or(0);
        if package == 0 {
            break;
        }
    }
    if cfg.uses_file() && std::fs::metadata(&cfg.model_file).is_ok_and(|m| m.permissions().readonly()) {
        findings.push(format!("the model file {} is read-only on disk", cfg.model_file.display()));
    }
    let require_lock = one(cfg, "select value from t_secpolicies where property = 'RequireLock'").map(|r| r[0].clone()).unwrap_or_default();
    let security = one(cfg, "select value from t_secpolicies where property = 'UserSecurity'").map(|r| r[0].clone()).unwrap_or_default();
    if findings.is_empty() {
        findings.push(if require_lock == "1" {
            "the model requires a user lock before editing (\"Require User Lock to Edit\"): lock it yourself (Ctrl+Shift+L or Lock in the context menu)".into()
        } else {
            "nothing in the model locks it; if EA still greys it out, check your user's permissions (Settings → Security → Users) and whether the project was opened read-only".into()
        });
    }
    let mut out = vec![format!("Why the {label} may not be editable:")];
    out.extend(findings.iter().map(|f| format!("- {f}")));
    if !security.is_empty() || !require_lock.is_empty() {
        out.push(format!("(security: {}, user locks required: {})", if security.is_empty() { "?" } else { &security }, if require_lock == "1" { "yes" } else { "no" }));
    }
    out.push("Locks of other people are released by them or by an EA administrator (Settings → Security → Locks).".into());
    Ok(out.join("\n"))
}

/// The GUIDs in notes after $element://, $diagram:// or $package://.
pub(crate) fn note_links(note: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for kind in ["element", "diagram", "package"] {
        let marker = format!("${kind}://");
        let mut rest = note;
        while let Some(i) = rest.find(&marker) {
            rest = &rest[i + marker.len()..];
            if let Some(end) = rest.find('}') {
                if rest.starts_with('{') {
                    out.push((kind.to_string(), rest[..=end].to_string()));
                }
            }
        }
    }
    out
}

pub fn dead_links(input: &Value, cfg: &AppConfig) -> Result<String> {
    let scope = if input["package"].is_null() || input["package"].as_str() == Some("") { String::new() } else { crate::model_usage::package_scope(cfg, &input["package"])?.0 };
    let mut out = Vec::new();
    for r in query_rows(cfg, &format!(
        "select o.object_id, coalesce(o.name,''), o.object_type, o.pdata1 from t_object o where (o.object_type = 'UMLDiagram' or o.ntype = 8) \
         and o.pdata1 is not null and o.pdata1 <> '' and o.pdata1 <> '0' {scope}"
    ))? {
        let d: i64 = r[3].trim().parse().unwrap_or(0);
        if d > 0 && one(cfg, &format!("select 1 from t_diagram where diagram_id = {d}")).is_none() {
            out.push(format!("- {} \"{}\" (id {}) points at diagram {d}, which is gone", if r[2] == "UMLDiagram" { "diagram reference" } else { "composite element" }, r[1], r[0]));
        }
    }
    let mut check = |owner: String, note: &str| {
        for (kind, guid) in note_links(note) {
            let g = crate::model_db::sql_text(cfg.model_source, &guid);
            let table = match kind.as_str() { "element" => "t_object", "diagram" => "t_diagram", _ => "t_package" };
            if one(cfg, &format!("select 1 from {table} where ea_guid = '{g}'")).is_none() {
                out.push(format!("- {owner}: a ${kind}:// link to {guid}, which is gone"));
            }
        }
    };
    for r in query_rows(cfg, &format!("select o.object_id, coalesce(o.name,''), o.note from t_object o where (o.note like '%$element://%' or o.note like '%$diagram://%' or o.note like '%$package://%') {scope}"))? {
        check(format!("notes of \"{}\" (id {})", r[1], r[0]), &r[2]);
    }
    let dscope = scope.replace("o.package_id", "d.package_id");
    for r in query_rows(cfg, &format!("select d.diagram_id, d.name, d.notes from t_diagram d where (d.notes like '%$element://%' or d.notes like '%$diagram://%' or d.notes like '%$package://%') {dscope}"))? {
        check(format!("notes of diagram \"{}\" (id {})", r[1], r[0]), &r[2]);
    }
    Ok(if out.is_empty() { "No dead links found.".into() } else { format!("Dead links ({}):\n{}", out.len(), out.join("\n")) })
}

#[cfg(test)]
mod tests {
    #[test]
    fn links_in_notes() {
        let links = super::note_links("See <a href=\"$element://{AB-1}\">Order</a> and $diagram://{CD-2} or $package://broken");
        assert_eq!(links, vec![("element".to_string(), "{AB-1}".to_string()), ("diagram".to_string(), "{CD-2}".to_string())]);
    }

    /// `LOONAR_TEST_PROPS=… cargo test readonly_why_live -- --ignored --nocapture`
    #[test]
    #[ignore = "requires a test model"]
    fn readonly_why_live() {
        let cfg = crate::config::AppConfig::load(std::path::Path::new(&std::env::var("LOONAR_TEST_PROPS").unwrap())).unwrap();
        let e: i64 = crate::tools::query_rows(&cfg, "select object_id from t_object where object_type = 'Class'").unwrap()[0][0].parse().unwrap();
        let why = super::why_locked(&serde_json::json!({ "element": e }), &cfg).unwrap();
        println!("{why}");
        assert!(why.starts_with("Why the Class"));
        println!("{}", super::dead_links(&serde_json::json!({}), &cfg).unwrap());
    }
}

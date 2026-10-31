//! The repository doctor (LC-368, /repo-doctor): integrity checks EA's Project Integrity Check does not make or makes
//! badly – relationships whose ends are gone, diagram rows pointing at nothing, stale t_diagramlinks rows (a
//! connector "shown" on a diagram that no longer has one of its ends), GUIDs used twice, packages without their package
//! element, element types EA does not know, packages of elements that do not exist, locks of users that are gone, and
//! the size of an .eap/.eapx file against Jet's 2 GB. Reads only: what can be repaired is repaired through EA (with
//! approval), and the report says what cannot be repaired automatically.

use crate::config::AppConfig;
use crate::tools::query_rows;
use anyhow::Result;
use serde_json::{json, Value};

pub fn definition() -> Value {
    json!({
        "name": "repo_doctor",
        "description": "Integrity check of the whole repository (reads only): relationships whose start or end element is gone, diagram objects \
and diagram links pointing at missing elements, connectors or diagrams, stale diagram links (a connector kept for a diagram that no longer has \
both its ends – EA keeps such rows for years), GUIDs used twice, packages without their package element (and the other way round), elements \
in packages that do not exist, element types EA does not know, locks of users that no longer exist or older than 30 days, and the size of an \
.eap/.eapx file against the 2 GB limit. Each check gives a count and examples. Use it for /repo-doctor, before a migration and when EA behaves \
oddly. Repairs: only through EA operations the user approves (e.g. repair_diagram_links, delete); say which findings need EA's own Project \
Integrity Check or a person.",
        "input_schema": { "type": "object", "properties": {} }
    })
}

/// One check: a title, a query whose rows are the findings (the first column a count is not needed: rows are counted),
/// and how a finding is repaired.
struct Check {
    title: &'static str,
    sql: &'static str,
    repair: &'static str,
}

const CHECKS: &[Check] = &[
    Check {
        title: "Relationships whose start or end element is gone",
        sql: "select c.connector_id, c.connector_type, c.start_object_id, c.end_object_id from t_connector c \
              where not exists (select 1 from t_object s where s.object_id = c.start_object_id) or not exists (select 1 from t_object e where e.object_id = c.end_object_id)",
        repair: "delete the relationship (ea_write delete connector) after checking it is not needed",
    },
    Check {
        title: "Diagram objects pointing at a missing element or diagram",
        sql: "select x.diagram_id, x.object_id from t_diagramobjects x \
              where not exists (select 1 from t_object o where o.object_id = x.object_id) or not exists (select 1 from t_diagram d where d.diagram_id = x.diagram_id)",
        repair: "EA's Project Integrity Check (Settings → Integrity Check) removes them",
    },
    Check {
        title: "Diagram links pointing at a missing connector or diagram",
        sql: "select l.diagramid, l.connectorid from t_diagramlinks l \
              where not exists (select 1 from t_connector c where c.connector_id = l.connectorid) or not exists (select 1 from t_diagram d where d.diagram_id = l.diagramid)",
        repair: "repair_diagram_links on the diagram, or EA's Project Integrity Check",
    },
    Check {
        title: "Stale diagram links (the diagram no longer has both ends of the connector)",
        sql: "select l.diagramid, l.connectorid from t_diagramlinks l join t_connector c on c.connector_id = l.connectorid \
              where not exists (select 1 from t_diagramobjects a where a.diagram_id = l.diagramid and a.object_id = c.start_object_id) \
              or not exists (select 1 from t_diagramobjects b where b.diagram_id = l.diagramid and b.object_id = c.end_object_id)",
        repair: "repair_diagram_links on each diagram (harmless rows, but they slow diagrams and confuse hiding)",
    },
    Check {
        title: "Element GUIDs used twice",
        sql: "select ea_guid, count(*) from t_object group by ea_guid having count(*) > 1",
        repair: "a person decides which element keeps the GUID (an XMI import made the copy); not automatic",
    },
    Check {
        title: "Connector GUIDs used twice",
        sql: "select ea_guid, count(*) from t_connector group by ea_guid having count(*) > 1",
        repair: "a person decides; not automatic",
    },
    Check {
        title: "Diagram GUIDs used twice",
        sql: "select ea_guid, count(*) from t_diagram group by ea_guid having count(*) > 1",
        repair: "a person decides; not automatic",
    },
    Check {
        title: "Packages without their package element",
        sql: "select p.package_id, p.name from t_package p where p.parent_id <> 0 \
              and not exists (select 1 from t_object o where o.object_type = 'Package' and (o.ea_guid = p.ea_guid or o.pdata1 = cast(p.package_id as varchar(20))))",
        repair: "EA's Project Integrity Check recreates the element",
    },
    Check {
        title: "Package elements without their package",
        sql: "select o.object_id, o.name from t_object o where o.object_type = 'Package' and not exists (select 1 from t_package p where p.ea_guid = o.ea_guid or o.pdata1 = cast(p.package_id as varchar(20)))",
        repair: "EA's Project Integrity Check",
    },
    Check {
        title: "Elements in a package that does not exist",
        sql: "select o.object_id, o.name, o.package_id from t_object o where not exists (select 1 from t_package p where p.package_id = o.package_id)",
        repair: "move them to an existing package (ea_write move) – they are invisible in the Project Browser",
    },
    Check {
        title: "Element types missing from the repository's type list (t_objecttypes; newer EA types are often only missing there)",
        sql: "select o.object_type, count(*) from t_object o where not exists (select 1 from t_objecttypes t where t.object_type = o.object_type) group by o.object_type",
        repair: "nothing to do for types EA itself creates (e.g. ProxyConnector); an unknown name comes from a damaged import or an old MDG",
    },
];

fn run_check(cfg: &AppConfig, check: &Check, out: &mut String) -> usize {
    match query_rows(cfg, check.sql) {
        Ok(rows) => {
            out.push_str(&format!("\n{} ({}):\n", check.title, rows.len()));
            if rows.is_empty() {
                out.push_str("  none\n");
            } else {
                for r in rows.iter().take(15) {
                    out.push_str(&format!("  - {}\n", r.join(" | ")));
                }
                if rows.len() > 15 {
                    out.push_str(&format!("  … and {} more\n", rows.len() - 15));
                }
                out.push_str(&format!("  Repair: {}\n", check.repair));
            }
            rows.len()
        }
        Err(e) => {
            out.push_str(&format!("\n{}: could not be checked ({e:#})\n", check.title));
            0
        }
    }
}

pub fn run(_input: &Value, cfg: &AppConfig) -> Result<String> {
    let mut out = format!("Repository doctor of {}:\n", cfg.model_label());
    let mut findings = 0;
    for check in CHECKS {
        findings += run_check(cfg, check, &mut out);
    }
    // Locks: of users that no longer exist, or older than 30 days (a person who left keeps them for ever).
    let locks = query_rows(cfg, "select l.entitytype, l.entityid, coalesce(u.userlogin, ''), l.timestamp from t_seclocks l left join t_secuser u on u.userid = l.userid order by l.timestamp");
    match locks {
        Ok(rows) => {
            let cutoff = (chrono::Local::now() - chrono::Duration::days(30)).format("%Y-%m-%d").to_string();
            let stale: Vec<&Vec<String>> = rows.iter().filter(|r| r[2].is_empty() || r[3].get(..10).is_some_and(|d| d < cutoff.as_str())).collect();
            out.push_str(&format!("\nLocks of users that are gone or older than 30 days ({} of {} locks):\n", stale.len(), rows.len()));
            for r in stale.iter().take(15) {
                out.push_str(&format!("  - {} {} – {} since {}\n", r[0], r[1], if r[2].is_empty() { "(no such user)" } else { r[2].as_str() }, r[3]));
            }
            if !stale.is_empty() {
                out.push_str("  Repair: an EA administrator releases them (Security → Manage Locks); locks kept by version control are released there.\n");
            }
            findings += stale.len();
        }
        Err(_) => out.push_str("\nLocks: the repository has no security tables (security is off)\n"),
    }
    // Jet files: 2 GB at most.
    if cfg.uses_file() {
        let ext = cfg.model_file.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
        if let Ok(meta) = std::fs::metadata(&cfg.model_file) {
            let mb = meta.len() / 1_048_576;
            if ext == "eap" || ext == "eapx" {
                out.push_str(&format!("\nFile size: {mb} MB of Jet's 2048 MB limit{}\n", if mb > 1500 { " – move it to .qea or a database soon" } else { "" }));
            } else {
                out.push_str(&format!("\nFile size: {mb} MB\n"));
            }
        }
    }
    out.push_str(&format!("\n{findings} findings. Nothing was changed.\n"));
    Ok(out)
}

#[cfg(test)]
mod tests {
    /// LC-368: the doctor on a model copy (reads only):
    /// `LOONAR_TEST_PROPS=E:/livetest/undo/undo.properties cargo test repo_doctor_live -- --ignored --nocapture`
    #[test]
    #[ignore = "requires the model from LOONAR_TEST_PROPS"]
    fn repo_doctor_live() {
        let cfg = crate::config::AppConfig::load(std::path::Path::new(&std::env::var("LOONAR_TEST_PROPS").expect("LOONAR_TEST_PROPS"))).expect("app.properties");
        let report = super::run(&serde_json::json!({}), &cfg).expect("repo_doctor");
        println!("{report}");
        assert!(report.contains("Stale diagram links") && report.ends_with("Nothing was changed.\n"));
        assert!(!report.contains("could not be checked"), "every check runs on this engine");
    }
}

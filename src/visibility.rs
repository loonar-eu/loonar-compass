//! Which relationships a diagram shows (LC-373). EA shows every relationship whose two ends are on a diagram, so a
//! relationship drawn for one view appears on all of them; hiding it by hand on every diagram does not last. A policy
//! of a diagram says which relationships it shows (types and stereotypes) and which it hides; Compass applies it on
//! request and – when the policy says "auto" – whenever a relationship is made in EA (an automatic "Freeze Visible").
//! The policies are kept in visibility.json next to the settings, by diagram GUID. Hidden relationships stay in the
//! model and on the other diagrams.

use crate::config::AppConfig;
use crate::tools::query_rows;
use anyhow::{bail, Context, Result};
use serde_json::{json, Map, Value};

pub fn definition() -> Value {
    json!({
        "name": "connector_visibility",
        "description": "Which relationships a diagram shows. EA shows every relationship whose ends are both on a diagram; a policy of the \
diagram keeps it clean. action set: save the policy of a diagram – show_types / show_stereotypes (only these are shown; empty = all), \
hide_types / hide_stereotypes (these never), auto (apply it whenever a relationship is made in EA). action apply: hide what the policy hides and \
show what it shows on that diagram (one save the user approves). action hidden: the relationships between elements of the diagram that are \
hidden there (/show-hidden-links). action list: the policies. Hiding only changes the view: the relationships stay in the model.",
        "input_schema": { "type": "object", "properties": {
            "action": { "type": "string", "enum": ["set", "apply", "hidden", "list"] },
            "diagram": { "type": "integer", "description": "diagram_id" },
            "show_types": { "type": "array", "items": { "type": "string" } },
            "show_stereotypes": { "type": "array", "items": { "type": "string" } },
            "hide_types": { "type": "array", "items": { "type": "string" } },
            "hide_stereotypes": { "type": "array", "items": { "type": "string" } },
            "auto": { "type": "boolean" }
        }, "required": ["action"] }
    })
}

fn file(cfg: &AppConfig) -> std::path::PathBuf {
    cfg.base_dir.join("visibility.json")
}

fn load(cfg: &AppConfig) -> Map<String, Value> {
    std::fs::read_to_string(file(cfg)).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok()).and_then(|v| v.as_object().cloned()).unwrap_or_default()
}

fn save(cfg: &AppConfig, all: &Map<String, Value>) -> Result<()> {
    let tmp = file(cfg).with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_string_pretty(all)?)?;
    std::fs::rename(&tmp, file(cfg))?;
    Ok(())
}

fn list_of(policy: &Value, key: &str) -> Vec<String> {
    policy[key].as_array().into_iter().flatten().filter_map(Value::as_str).map(|s| s.trim().trim_matches(['«', '»']).to_lowercase()).filter(|s| !s.is_empty()).collect()
}

/// Whether the policy shows a relationship of this type and stereotype.
pub(crate) fn shows(policy: &Value, connector_type: &str, stereotype: &str) -> bool {
    let (t, st) = (connector_type.to_lowercase(), stereotype.to_lowercase());
    let has = |list: &[String]| list.iter().any(|x| *x == t || (!st.is_empty() && (st == *x || st.rsplit("::").next() == Some(x.as_str()))));
    let (show_t, show_s) = (list_of(policy, "show_types"), list_of(policy, "show_stereotypes"));
    let mut shown = show_t.is_empty() && show_s.is_empty() || has(&show_t) || has(&show_s);
    if has(&list_of(policy, "hide_types")) || has(&list_of(policy, "hide_stereotypes")) {
        shown = false;
    }
    shown
}

/// The relationships between elements of the diagram: (connector_id, type, stereotype, from, to, hidden there).
fn connectors_of(cfg: &AppConfig, diagram: i64) -> Result<Vec<(i64, String, String, String, String, bool)>> {
    Ok(query_rows(cfg, &format!(
        "select c.connector_id, c.connector_type, coalesce(c.stereotype,''), s.name, t.name, \
         coalesce((select max(l.hidden) from t_diagramlinks l where l.diagramid = {diagram} and l.connectorid = c.connector_id), 0) \
         from t_connector c join t_object s on s.object_id = c.start_object_id join t_object t on t.object_id = c.end_object_id \
         where c.connector_type not in ('Sequence', 'Collaboration') \
         and exists (select 1 from t_diagramobjects a where a.diagram_id = {diagram} and a.object_id = c.start_object_id) \
         and exists (select 1 from t_diagramobjects b where b.diagram_id = {diagram} and b.object_id = c.end_object_id) order by c.connector_id"
    ))?
    .into_iter()
    .map(|r| (r[0].parse().unwrap_or(0), r[1].clone(), r[2].clone(), r[3].clone(), r[4].clone(), r[5] == "1" || r[5].eq_ignore_ascii_case("true")))
    .collect())
}

fn diagram_guid(cfg: &AppConfig, diagram: i64) -> Result<(String, String)> {
    let r = query_rows(cfg, &format!("select ea_guid, name from t_diagram where diagram_id = {diagram}"))?;
    let r = r.first().with_context(|| format!("there is no diagram with id {diagram}"))?;
    Ok((r[0].clone(), r[1].clone()))
}

/// The operations that bring the diagram in line with its policy: hide what it does not show, show what it shows.
pub fn apply_ops(cfg: &AppConfig, diagram: i64, policy: &Value) -> Result<Vec<Value>> {
    let (mut hide, mut show) = (Vec::new(), Vec::new());
    for (id, kind, stereotype, _, _, hidden) in connectors_of(cfg, diagram)? {
        match (shows(policy, &kind, &stereotype), hidden) {
            (false, false) => hide.push(id),
            (true, true) => show.push(id),
            _ => {}
        }
    }
    let mut ops = Vec::new();
    if !hide.is_empty() {
        ops.push(json!({ "op": "hide_connectors", "diagram": diagram, "connectors": hide }));
    }
    if !show.is_empty() {
        ops.push(json!({ "op": "hide_connectors", "diagram": diagram, "connectors": show, "hidden": false }));
    }
    Ok(ops)
}

/// A policy for a diagram: by diagram id from the action, else None.
fn policy_of(cfg: &AppConfig, diagram: i64) -> Result<Option<Value>> {
    let (guid, _) = diagram_guid(cfg, diagram)?;
    Ok(load(cfg).get(&guid).cloned())
}

/// The text of the read-only actions, or the operations of apply.
pub fn run(input: &Value, cfg: &AppConfig) -> Result<(Vec<Value>, String)> {
    let diagram = input["diagram"].as_i64();
    match input["action"].as_str().unwrap_or("list") {
        "set" => {
            let diagram = diagram.context("give the diagram")?;
            let (guid, name) = diagram_guid(cfg, diagram)?;
            let mut policy = Map::new();
            for key in ["show_types", "show_stereotypes", "hide_types", "hide_stereotypes"] {
                if input[key].is_array() {
                    policy.insert(key.into(), input[key].clone());
                }
            }
            policy.insert("auto".into(), json!(input["auto"].as_bool().unwrap_or(false)));
            policy.insert("name".into(), json!(name));
            let mut all = load(cfg);
            all.insert(guid, Value::Object(policy.clone()));
            save(cfg, &all)?;
            let ops = apply_ops(cfg, diagram, &Value::Object(policy))?;
            Ok((Vec::new(), format!("Saved the policy of diagram \"{name}\". Applying it now would hide or show {} groups of relationships: call action apply to do it (the user approves).", ops.len())))
        }
        "apply" => {
            let diagram = diagram.context("give the diagram")?;
            let policy = policy_of(cfg, diagram)?.context("this diagram has no policy yet: action set first")?;
            let ops = apply_ops(cfg, diagram, &policy)?;
            if ops.is_empty() {
                bail!("the diagram already follows its policy");
            }
            Ok((ops, String::new()))
        }
        "hidden" => {
            let diagram = diagram.context("give the diagram")?;
            let (_, name) = diagram_guid(cfg, diagram)?;
            let hidden: Vec<String> = connectors_of(cfg, diagram)?
                .into_iter()
                .filter(|c| c.5)
                .map(|(id, kind, st, from, to, _)| format!("{kind}{} \"{from}\" → \"{to}\" (connector_id {id})", if st.is_empty() { String::new() } else { format!(" «{st}»") }))
                .collect();
            Ok((Vec::new(), format!("Relationships between elements of \"{name}\" that are hidden there ({}):\n{}", hidden.len(), hidden.iter().map(|h| format!("  - {h}")).collect::<Vec<_>>().join("\n"))))
        }
        _ => {
            let all = load(cfg);
            let mut text = format!("Visibility policies ({}):\n", all.len());
            for (guid, p) in all {
                // Each policy with the number of relationships its diagram hides now (LC-415).
                let id = query_rows(cfg, &format!("select diagram_id from t_diagram where ea_guid = '{}'", crate::model_db::sql_text(cfg.model_source, &guid))).ok()
                    .and_then(|r| r.first().and_then(|r| r[0].parse::<i64>().ok()));
                let hidden = id.map(|d| format!(" – {} relationships hidden there", hidden_count(cfg, d))).unwrap_or_default();
                text.push_str(&format!("  - \"{}\"{hidden}: {}\n", p["name"].as_str().unwrap_or("?"), p));
            }
            Ok((Vec::new(), text))
        }
    }
}

/// Relationships between elements of the diagram that are hidden on it (LC-415).
pub(crate) fn hidden_count(cfg: &AppConfig, diagram: i64) -> usize {
    connectors_of(cfg, diagram).map(|c| c.into_iter().filter(|c| c.5).count()).unwrap_or(0)
}

/// After a relationship is made in EA: on every diagram with an "auto" policy that has both its ends and does not
/// show it, it is hidden at once. Returns what was done, for the "Loonar Compass" output tab.
pub fn auto_apply(cfg: &AppConfig, connector: i64) -> Vec<String> {
    let all = load(cfg);
    if !all.values().any(|p| p["auto"] == true) {
        return Vec::new();
    }
    let Ok(rows) = query_rows(cfg, &format!(
        "select d.diagram_id, d.ea_guid, d.name, c.connector_type, coalesce(c.stereotype,'') from t_connector c join t_diagram d on 1 = 1 \
         where c.connector_id = {connector} \
         and exists (select 1 from t_diagramobjects a where a.diagram_id = d.diagram_id and a.object_id = c.start_object_id) \
         and exists (select 1 from t_diagramobjects b where b.diagram_id = d.diagram_id and b.object_id = c.end_object_id)"
    )) else {
        return Vec::new();
    };
    let mut done = Vec::new();
    for r in rows {
        let Some(policy) = all.get(&r[1]).filter(|p| p["auto"] == true) else { continue };
        if shows(policy, &r[3], &r[4]) {
            continue;
        }
        let diagram: i64 = r[0].parse().unwrap_or(0);
        let ops = [json!({ "op": "hide_connectors", "diagram": diagram, "connectors": [connector] })];
        let result = crate::ea::run_batch(cfg, &ops, &crate::ea::BatchOptions::default());
        crate::tools::audit(cfg, &ops, &result);
        if result.is_ok() {
            // How many relationships the diagram hides now (LC-415): the person sees the count, not only this one.
            let total = hidden_count(cfg, diagram);
            done.push(crate::i18n::fmt("hidden on \"{0}\" by its visibility policy ({1} relationships hidden there now; /show-hidden-links lists them)", &[r[2].clone(), total.to_string()]));
        }
    }
    done
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    #[test]
    fn what_a_policy_shows() {
        let only_flows = json!({ "show_types": ["InformationFlow"], "show_stereotypes": ["ArchiMate_Serving"] });
        assert!(super::shows(&only_flows, "InformationFlow", ""));
        assert!(super::shows(&only_flows, "Dependency", "ArchiMate3::ArchiMate_Serving"));
        assert!(!super::shows(&only_flows, "Association", ""));
        let no_generalization = json!({ "hide_types": ["Generalization"] });
        assert!(super::shows(&no_generalization, "Association", "") && !super::shows(&no_generalization, "Generalization", ""));
        assert!(super::shows(&json!({}), "Anything", ""));
    }

    /// LC-373: a policy applied through EA on a model COPY:
    /// `LOONAR_TEST_PROPS=E:/livetest/undo/undo.properties cargo test visibility_live -- --ignored --nocapture`
    #[test]
    #[ignore = "requires EA and a copy of a test model"]
    fn visibility_live() {
        let mut cfg = crate::config::AppConfig::load(std::path::Path::new(&std::env::var("LOONAR_TEST_PROPS").expect("LOONAR_TEST_PROPS"))).expect("app.properties");
        let dir = std::env::temp_dir().join(format!("loonar-vis-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        cfg.base_dir = dir.clone();
        let q = |sql: &str| crate::tools::query_rows(&cfg, sql).unwrap();
        let root: i64 = q("select package_id from t_package where parent_id = 0 order by package_id")[0][0].parse().unwrap();
        let ops = vec![
            json!({"op":"create_package","key":"pkg","parent":root,"name":format!("ZZ_vis_{}", chrono::Local::now().format("%H%M%S"))}),
            json!({"op":"create_diagram","key":"d","package":"#pkg","name":"Flows only","type":"Logical"}),
            json!({"op":"create_element","key":"a","package":"#pkg","name":"A","type":"Class"}),
            json!({"op":"create_element","key":"b","package":"#pkg","name":"B","type":"Class"}),
            json!({"op":"create_connector","key":"flow","source":"#a","target":"#b","type":"InformationFlow"}),
            json!({"op":"create_connector","key":"assoc","source":"#a","target":"#b","type":"Association"}),
            json!({"op":"place","diagram":"#d","element":"#a","left":60,"top":40,"width":110,"height":60}),
            json!({"op":"place","diagram":"#d","element":"#b","left":300,"top":40,"width":110,"height":60}),
        ];
        let created = crate::ea::run_batch(&cfg, &ops, &crate::ea::BatchOptions { rollback_key: Some("pkg".into()), keep_locks: false }).expect("creation");
        let id_of = |key: &str| created["created"].as_array().unwrap().iter().find(|c| c["key"] == key).expect(key)["id"].as_i64().unwrap();
        let (pkg, d, assoc) = (id_of("pkg"), id_of("d"), id_of("assoc"));
        super::run(&json!({ "action": "set", "diagram": d, "show_types": ["InformationFlow"], "auto": true }), &cfg).unwrap();
        let (apply, _) = super::run(&json!({ "action": "apply", "diagram": d }), &cfg).unwrap();
        crate::ea::run_batch(&cfg, &apply, &crate::ea::BatchOptions::default()).expect("apply");
        let hidden = super::run(&json!({ "action": "hidden", "diagram": d }), &cfg).unwrap().1;
        println!("{hidden}");
        // A new association later: the auto policy hides it at once.
        let later = crate::ea::run_batch(&cfg, &[json!({"op":"create_connector","key":"x","source":id_of("b"),"target":id_of("a"),"type":"Dependency"})], &crate::ea::BatchOptions::default()).expect("new");
        let x = later["created"][0]["id"].as_i64().unwrap();
        let auto = super::auto_apply(&cfg, x);
        let hidden_after = super::run(&json!({ "action": "hidden", "diagram": d }), &cfg).unwrap().1;
        crate::ea::run_batch(&cfg, &[json!({"op":"delete","package":pkg})], &crate::ea::BatchOptions::default()).expect("cleanup");
        crate::ea::shutdown();
        let _ = std::fs::remove_dir_all(&dir);
        assert!(hidden.contains(&format!("connector_id {assoc}")) && !hidden.contains("InformationFlow"), "{hidden}");
        assert!(!auto.is_empty() && hidden_after.contains(&format!("connector_id {x}")), "{auto:?} {hidden_after}");
    }
}

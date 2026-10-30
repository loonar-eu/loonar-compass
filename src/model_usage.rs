//! Where an element is used and which elements nobody uses (LC-337, /where-used and /find-orphans): every kind of use
//! EA itself does not show in one place – diagrams, relationships, classifiers of instances and lifelines, types of
//! attributes, parameters and return values, tagged values pointing at the element.

use crate::config::AppConfig;
use crate::tools::query_rows;
use anyhow::{bail, Result};
use serde_json::{json, Value};

pub fn definitions() -> Vec<Value> {
    vec![
        json!({
            "name": "where_used",
            "description": "Every use of one element in the model: diagrams it is on, its relationships (type, direction, other end), instances \
and lifelines classified by it, attributes, parameters and operations typed by it, tagged values pointing at it. Use it for \"where is X used\", \
before deleting or renaming an element, and for /where-used. Element = t_object.object_id or its exact name.",
            "input_schema": { "type": "object", "properties": { "element": { "description": "object_id or exact name" } }, "required": ["element"] }
        }),
        json!({
            "name": "find_orphans",
            "description": "Elements of a package (with child packages) that nothing uses: on no diagram, without relationships, not a classifier \
or type of anything, not pointed at by a tagged value, without child elements. EA's own \"orphans\" search only checks diagrams. \
Use it for cleaning up a model and for /find-orphans. Notes, texts, boundaries and packages are left out.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "package": { "description": "package_id or exact name; empty: the whole model" },
                    "types": { "type": "array", "items": { "type": "string" }, "description": "Only these element types (optional)" }
                }
            }
        }),
    ]
}

fn element_id(cfg: &AppConfig, value: &Value) -> Result<(i64, String, String, String)> {
    let rows = if let Some(id) = value.as_i64().or_else(|| value.as_str().and_then(|s| s.trim().parse().ok())) {
        query_rows(cfg, &format!("select object_id, name, object_type, ea_guid from t_object where object_id = {id}"))?
    } else {
        let name = crate::model_db::sql_text(cfg.model_source, value.as_str().unwrap_or("").trim());
        query_rows(cfg, &format!("select object_id, name, object_type, ea_guid from t_object where name = '{name}' order by object_id"))?
    };
    match rows.as_slice() {
        [] => bail!("{}", crate::i18n::fmt("there is no element with id {0}", &[value.to_string()])),
        [one] => Ok((one[0].parse()?, one[1].clone(), one[2].clone(), one[3].clone())),
        many => bail!("several elements are named {value}: {} - pass the object_id", many.iter().map(|r| format!("{} ({}, id {})", r[1], r[2], r[0])).collect::<Vec<_>>().join("; ")),
    }
}

fn section(out: &mut String, title: &str, rows: Vec<String>) {
    out.push_str(&format!("\n{title} ({}):\n", rows.len()));
    if rows.is_empty() {
        out.push_str("  none\n");
    }
    for r in rows.iter().take(80) {
        out.push_str(&format!("  - {r}\n"));
    }
    if rows.len() > 80 {
        out.push_str(&format!("  … and {} more\n", rows.len() - 80));
    }
}

pub fn where_used(input: &Value, cfg: &AppConfig) -> Result<String> {
    let (id, name, kind, guid) = element_id(cfg, &input["element"])?;
    let guid_q = crate::model_db::sql_text(cfg.model_source, &guid);
    let mut out = format!("Uses of element \"{name}\" ({kind}, id {id}):\n");
    let diagrams = query_rows(cfg, &format!(
        "select d.diagram_id, d.name, d.diagram_type, p.name from t_diagramobjects x join t_diagram d on d.diagram_id = x.diagram_id \
         join t_package p on p.package_id = d.package_id where x.object_id = {id} order by d.name"
    ))?;
    section(&mut out, "Diagrams", diagrams.iter().map(|r| format!("\"{}\" ({}, package \"{}\", diagram_id {})", r[1], r[2], r[3], r[0])).collect());
    let links = query_rows(cfg, &format!(
        "select c.connector_type, coalesce(c.stereotype,''), coalesce(c.name,''), case when c.start_object_id = {id} then 'to' else 'from' end, o.name, o.object_type \
         from t_connector c join t_object o on o.object_id = case when c.start_object_id = {id} then c.end_object_id else c.start_object_id end \
         where (c.start_object_id = {id} or c.end_object_id = {id}) and c.connector_type not in ('Sequence', 'Collaboration') order by 1, 5"
    ))?;
    section(&mut out, "Relationships", links.iter().map(|r| format!("{}{} {} \"{}\" ({}){}", r[0], if r[1].is_empty() { String::new() } else { format!(" «{}»", r[1]) }, r[3], r[4], r[5], if r[2].is_empty() { String::new() } else { format!(" – \"{}\"", r[2]) })).collect());
    let messages = query_rows(cfg, &format!(
        "select count(*) from t_connector c where c.connector_type in ('Sequence', 'Collaboration') and (c.start_object_id = {id} or c.end_object_id = {id} \
         or c.start_object_id in (select object_id from t_object where classifier = {id}) or c.end_object_id in (select object_id from t_object where classifier = {id}))"
    ))?;
    out.push_str(&format!("\nMessages on sequence or communication diagrams to or from it or its lifelines: {}\n", messages.first().map(|r| r[0].as_str()).unwrap_or("0")));
    let instances = query_rows(cfg, &format!(
        "select o.name, o.object_type, p.name from t_object o join t_package p on p.package_id = o.package_id where o.classifier = {id} or o.classifier_guid = '{guid_q}' order by 1"
    ))?;
    section(&mut out, "Instances, lifelines and actions classified by it", instances.iter().map(|r| format!("\"{}\" ({}, package \"{}\")", r[0], r[1], r[2])).collect());
    let attributes = query_rows(cfg, &format!(
        "select o.name, a.name from t_attribute a join t_object o on o.object_id = a.object_id where a.classifier = '{id}' order by 1, 2"
    ))?;
    section(&mut out, "Attributes of this type", attributes.iter().map(|r| format!("{}.{}", r[0], r[1])).collect());
    let parameters = query_rows(cfg, &format!(
        "select o.name, m.name, p.name from t_operationparams p join t_operation m on m.operationid = p.operationid join t_object o on o.object_id = m.object_id \
         where p.classifier = '{id}' order by 1, 2"
    ))?;
    section(&mut out, "Operation parameters of this type", parameters.iter().map(|r| format!("{}.{}({})", r[0], r[1], r[2])).collect());
    let returns = query_rows(cfg, &format!(
        "select o.name, m.name from t_operation m join t_object o on o.object_id = m.object_id where m.classifier = '{id}' order by 1, 2"
    ))?;
    section(&mut out, "Operations returning this type", returns.iter().map(|r| format!("{}.{}()", r[0], r[1])).collect());
    let tags = query_rows(cfg, &format!(
        "select o.name, t.property from t_objectproperties t join t_object o on o.object_id = t.object_id where t.value = '{guid_q}' order by 1"
    ))?;
    section(&mut out, "Tagged values pointing at it", tags.iter().map(|r| format!("{} – tag \"{}\"", r[0], r[1])).collect());
    Ok(out)
}

pub fn find_orphans(input: &Value, cfg: &AppConfig) -> Result<String> {
    let mut scope = String::new();
    let mut title = "the whole model".to_string();
    let package = &input["package"];
    if !package.is_null() && package.as_str() != Some("") {
        let rows = if let Some(id) = package.as_i64().or_else(|| package.as_str().and_then(|s| s.trim().parse().ok())) {
            query_rows(cfg, &format!("select package_id, name from t_package where package_id = {id}"))?
        } else {
            query_rows(cfg, &format!("select package_id, name from t_package where name = '{}'", crate::model_db::sql_text(cfg.model_source, package.as_str().unwrap_or(""))))?
        };
        let Some(root) = rows.first() else { bail!("{}", crate::i18n::fmt("there is no package {0}", &[package.to_string()])) };
        let mut all = vec![root[0].clone()];
        let mut level = vec![root[0].clone()];
        while !level.is_empty() {
            level = query_rows(cfg, &format!("select package_id from t_package where parent_id in ({})", level.join(",")))?.into_iter().map(|r| r[0].clone()).collect();
            level.retain(|p| !all.contains(p));
            all.extend(level.iter().cloned());
        }
        scope = format!("and o.package_id in ({})", all.join(","));
        title = format!("package \"{}\" with its child packages", root[1]);
    }
    let types: Vec<String> = input["types"].as_array().into_iter().flatten().filter_map(|t| t.as_str()).map(|t| format!("'{}'", crate::model_db::sql_text(cfg.model_source, t))).collect();
    let type_filter = if types.is_empty() { String::new() } else { format!("and o.object_type in ({})", types.join(",")) };
    let rows = query_rows(cfg, &format!(
        "select o.object_id, o.name, o.object_type, p.name from t_object o join t_package p on p.package_id = o.package_id \
         where o.object_type not in ('Note', 'Text', 'Boundary', 'Package', 'ProvidedInterface', 'RequiredInterface', 'Port', 'Constraint') {type_filter} {scope} \
         and not exists (select 1 from t_diagramobjects x where x.object_id = o.object_id) \
         and not exists (select 1 from t_connector c where c.start_object_id = o.object_id or c.end_object_id = o.object_id) \
         and not exists (select 1 from t_object i where i.classifier = o.object_id or i.classifier_guid = o.ea_guid) \
         and not exists (select 1 from t_object ch where ch.parentid = o.object_id) \
         and not exists (select 1 from t_attribute a where a.classifier = cast(o.object_id as varchar(20))) \
         and not exists (select 1 from t_operationparams pp where pp.classifier = cast(o.object_id as varchar(20))) \
         and not exists (select 1 from t_operation m where m.classifier = cast(o.object_id as varchar(20))) \
         and not exists (select 1 from t_objectproperties t where t.value = o.ea_guid) \
         order by p.name, o.name"
    ))?;
    let mut out = format!("Elements in {title} that nothing uses: no diagram, no relationship, not a classifier or type, no tagged value pointing at them, no child elements ({}).\n", rows.len());
    for r in rows.iter().take(150) {
        out.push_str(&format!("  - \"{}\" ({}, package \"{}\", id {})\n", r[1], r[2], r[3], r[0]));
    }
    if rows.len() > 150 {
        out.push_str(&format!("  … and {} more\n", rows.len() - 150));
    }
    out.push_str("Deleting or moving them changes the model: only with the user's approval (ea_write), and a baseline first.");
    Ok(out)
}

#[cfg(test)]
mod tests {
    /// where_used and find_orphans on the test model. Run with: `cargo test model_usage_live -- --ignored --nocapture`.
    #[test]
    #[ignore = "requires the model from LOONAR_TEST_PROPS"]
    fn model_usage_live() {
        let cfg = crate::config::AppConfig::load(std::path::Path::new(&std::env::var("LOONAR_TEST_PROPS").expect("LOONAR_TEST_PROPS"))).expect("app.properties");
        let used = super::where_used(&serde_json::json!({ "element": "Chief Financial Officer" }), &cfg).or_else(|_| super::where_used(&serde_json::json!({ "element": 3312 }), &cfg)).expect("where_used");
        println!("{used}");
        assert!(used.contains("Diagrams (") && used.contains("Relationships ("));
        let orphans = super::find_orphans(&serde_json::json!({ "package": 385 }), &cfg).expect("find_orphans");
        println!("{orphans}");
        assert!(orphans.starts_with("Elements in package \"Stakeholders\""));
    }
}

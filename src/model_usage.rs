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
and lifelines classified by it, attributes, parameters and operations typed by it, information flows conveying it, tagged values pointing at it. \
For a relationship (connector): every diagram that shows both its ends, with whether it is visible or hidden there. Use it for \"where is X used\", \
before deleting or renaming, and for /where-used. Element = t_object.object_id or its exact name; connector = t_connector.connector_id.",
            "input_schema": { "type": "object", "properties": {
                "element": { "description": "object_id or exact name" },
                "connector": { "type": "integer", "description": "connector_id (instead of element)" }
            } }
        }),
        json!({
            "name": "find_duplicates",
            "description": "Likely duplicates in a package (with child packages) or the whole model: elements of the same type whose names are \
the same or nearly the same (letter case, spaces, _ and -, a plural s), each with its package and how much uses it (relationships and diagrams) – \
to choose which one to keep; and relationships of the same type and stereotype between the same two elements. Reads only. Use it for \
/find-duplicates and before merging (merge_elements).",
            "input_schema": { "type": "object", "properties": {
                "package": { "description": "package_id or exact name; empty: the whole model" },
                "types": { "type": "array", "items": { "type": "string" }, "description": "Only these element types (optional)" }
            } }
        }),
        json!({
            "name": "elements_without_notes",
            "description": "Elements without a description in a package (with child packages), 30 at a time (offset for the next ones), each with \
what tells what it is: type, stereotype, package, its relationships to other elements and the diagrams it is on. Reads only. For /fill-notes: \
draft a short description of each from this context (in the language of the model's names), show them to the user, and save the accepted ones \
with ea_write update_element notes (the user approves; they can be undone).",
            "input_schema": { "type": "object", "properties": {
                "package": { "description": "package_id or exact name" },
                "types": { "type": "array", "items": { "type": "string" } },
                "offset": { "type": "integer" }
            } }
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
    if let Some(connector) = input["connector"].as_i64() {
        return connector_used(cfg, connector);
    }
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
    // Information flows that convey it (EA keeps them in t_xref, behavior "conveyed") (LC-365).
    let flows = query_rows(cfg, &format!(
        "select c.connector_id, s.name, t.name from t_xref x join t_connector c on c.ea_guid = x.client join t_object s on s.object_id = c.start_object_id \
         join t_object t on t.object_id = c.end_object_id where x.behavior = 'conveyed' and x.description like '%{guid_q}%' order by 2"
    ))
    .unwrap_or_default();
    section(&mut out, "Information flows conveying it", flows.iter().map(|r| format!("\"{}\" → \"{}\" (connector_id {})", r[1], r[2], r[0])).collect());
    Ok(out)
}

/// Where a relationship shows (LC-365): every diagram with both its ends, visible or hidden there (t_diagramlinks).
/// EA's "Find in all diagrams" does not cover connectors.
fn connector_used(cfg: &AppConfig, id: i64) -> Result<String> {
    let c = query_rows(cfg, &format!(
        "select c.connector_type, coalesce(c.stereotype,''), coalesce(c.name,''), s.name, t.name, c.start_object_id, c.end_object_id from t_connector c \
         join t_object s on s.object_id = c.start_object_id join t_object t on t.object_id = c.end_object_id where c.connector_id = {id}"
    ))?;
    let Some(c) = c.first() else { bail!("there is no connector with id {id}") };
    let stereotype = if c[1].is_empty() { String::new() } else { format!(" «{}»", c[1]) };
    let label = if c[2].is_empty() { String::new() } else { format!(" \"{}\"", c[2]) };
    let mut out = format!("Uses of relationship {}{stereotype} \"{}\" → \"{}\" (connector_id {id}){label}:\n", c[0], c[3], c[4]);
    let diagrams = query_rows(cfg, &format!(
        "select d.diagram_id, d.name, p.name, coalesce((select max(l.hidden) from t_diagramlinks l where l.diagramid = d.diagram_id and l.connectorid = {id}), 0) \
         from t_diagram d join t_package p on p.package_id = d.package_id \
         where exists (select 1 from t_diagramobjects a where a.diagram_id = d.diagram_id and a.object_id = {}) \
         and exists (select 1 from t_diagramobjects b where b.diagram_id = d.diagram_id and b.object_id = {}) order by d.name",
        c[5], c[6]
    ))?;
    let hidden = |v: &str| v == "1" || v.eq_ignore_ascii_case("true");
    section(&mut out, "Diagrams where it is visible", diagrams.iter().filter(|r| !hidden(&r[3])).map(|r| format!("\"{}\" (package \"{}\", diagram_id {})", r[1], r[2], r[0])).collect());
    section(&mut out, "Diagrams with both ends where it is hidden", diagrams.iter().filter(|r| hidden(&r[3])).map(|r| format!("\"{}\" (package \"{}\", diagram_id {})", r[1], r[2], r[0])).collect());
    Ok(out)
}

pub fn find_orphans(input: &Value, cfg: &AppConfig) -> Result<String> {
    let (scope, title) = package_scope(cfg, &input["package"])?;
    find_orphans_rest(input, cfg, scope, title)
}

/// " and o.package_id in (…)" for a package (id or exact name) with its child packages, and its title; empty = the
/// whole model.
pub(crate) fn package_scope(cfg: &AppConfig, package: &Value) -> Result<(String, String)> {
    let mut scope = String::new();
    let mut title = "the whole model".to_string();
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
    Ok((scope, title))
}


/// A name for comparing (LC-369): lower case, without spaces, "_", "-" and ".", and without one plural "s" at the end.
pub(crate) fn name_key(name: &str) -> String {
    let key: String = name.to_lowercase().chars().filter(|c| !matches!(c, ' ' | '_' | '-' | '.' | '\t')).collect();
    match key.strip_suffix('s') {
        Some(stem) if stem.chars().count() >= 3 => stem.to_string(),
        _ => key,
    }
}

/// Likely duplicate elements and relationships (LC-369). Reads only.
pub fn find_duplicates(input: &Value, cfg: &AppConfig) -> Result<String> {
    let (scope, title) = package_scope(cfg, &input["package"])?;
    let types: Vec<String> = input["types"].as_array().into_iter().flatten().filter_map(|t| t.as_str()).map(|t| format!("'{}'", crate::model_db::sql_text(cfg.model_source, t))).collect();
    let type_filter = if types.is_empty() { String::new() } else { format!("and o.object_type in ({})", types.join(",")) };
    let rows = query_rows(cfg, &format!(
        "select o.object_id, o.name, o.object_type, p.name, \
         (select count(*) from t_connector c where c.start_object_id = o.object_id or c.end_object_id = o.object_id), \
         (select count(*) from t_diagramobjects x where x.object_id = o.object_id) \
         from t_object o join t_package p on p.package_id = o.package_id \
         where o.object_type not in ('Note', 'Text', 'Boundary', 'Package', 'ProvidedInterface', 'RequiredInterface', 'Port', 'Constraint', 'ActionPin', 'ActivityParameter') \
         and coalesce(o.name, '') <> '' {type_filter} {scope}"
    ))?;
    let mut groups: std::collections::BTreeMap<(String, String), Vec<&Vec<String>>> = std::collections::BTreeMap::new();
    for r in &rows {
        groups.entry((r[2].clone(), name_key(&r[1]))).or_default().push(r);
    }
    let groups: Vec<_> = groups.into_iter().filter(|(_, g)| g.len() > 1).collect();
    let mut out = format!("Likely duplicates in {title}: {} groups of elements.\n", groups.len());
    for ((kind, _), members) in groups.iter().take(100) {
        out.push_str(&format!("\n{kind}:\n"));
        for m in members {
            out.push_str(&format!("  - \"{}\" (object_id {}, package \"{}\") – {} relationships, on {} diagrams\n", m[1], m[0], m[3], m[4], m[5]));
        }
    }
    if groups.len() > 100 {
        out.push_str(&format!("\n… and {} more groups\n", groups.len() - 100));
    }
    let connectors = query_rows(cfg, &format!(
        "select c.connector_type, coalesce(c.stereotype,''), s.name, t.name, count(*), min(c.connector_id), max(c.connector_id) from t_connector c \
         join t_object s on s.object_id = c.start_object_id join t_object t on t.object_id = c.end_object_id join t_object o on o.object_id = c.start_object_id \
         where c.connector_type not in ('Sequence', 'Collaboration', 'NoteLink') {scope} \
         group by c.connector_type, c.stereotype, s.name, t.name, c.start_object_id, c.end_object_id having count(*) > 1 order by 5 desc"
    ))?;
    section(&mut out, "Repeated relationships (type, stereotype, from → to, how many, connector ids from–to)", connectors.iter().map(|r| {
        format!("{}{} \"{}\" → \"{}\": {} times (connector_id {}…{})", r[0], if r[1].is_empty() { String::new() } else { format!(" «{}»", r[1]) }, r[2], r[3], r[4], r[5], r[6])
    }).collect());
    out.push_str("\nNothing was changed. Keep the element with the most uses; the others can be merged into it (merge_elements, with the user's approval).\n");
    Ok(out)
}

/// Elements without notes with their context, 30 at a time (LC-380).
pub fn elements_without_notes(input: &Value, cfg: &AppConfig) -> Result<String> {
    let (scope, title) = package_scope(cfg, &input["package"])?;
    let types: Vec<String> = input["types"].as_array().into_iter().flatten().filter_map(|t| t.as_str()).map(|t| format!("'{}'", crate::model_db::sql_text(cfg.model_source, t))).collect();
    let type_filter = if types.is_empty() { String::new() } else { format!("and o.object_type in ({})", types.join(",")) };
    let offset = input["offset"].as_u64().unwrap_or(0) as usize;
    let rows = query_rows(cfg, &format!(
        "select o.object_id, o.name, o.object_type, coalesce(o.stereotype,''), p.name from t_object o join t_package p on p.package_id = o.package_id \
         where coalesce(o.note,'') = '' and coalesce(o.name,'') <> '' \
         and o.object_type not in ('Note', 'Text', 'Boundary', 'Package', 'ProvidedInterface', 'RequiredInterface', 'Port', 'ActionPin', 'ActivityParameter', \
         'Decision', 'MergeNode', 'Synchronization', 'StateNode', 'InitialNode', 'ActivityFinal', 'FlowFinal', 'Constraint') {type_filter} {scope} order by p.name, o.name"
    ))?;
    let mut out = format!("Elements without a description in {title}: {} (showing {}–{}).\n", rows.len(), offset + 1, (offset + 30).min(rows.len()));
    for r in rows.iter().skip(offset).take(30) {
        let links = query_rows(cfg, &format!(
            "select c.connector_type, case when c.start_object_id = {0} then 'to' else 'from' end, o.name, o.object_type from t_connector c \
             join t_object o on o.object_id = case when c.start_object_id = {0} then c.end_object_id else c.start_object_id end \
             where (c.start_object_id = {0} or c.end_object_id = {0}) and c.connector_type not in ('NoteLink') order by 1",
            r[0]
        ))
        .unwrap_or_default();
        let diagrams = query_rows(cfg, &format!("select d.name from t_diagramobjects x join t_diagram d on d.diagram_id = x.diagram_id where x.object_id = {}", r[0])).unwrap_or_default();
        out.push_str(&format!(
            "\n- \"{}\" ({}{}, id {}, package \"{}\")\n  relationships: {}\n  diagrams: {}\n",
            r[1],
            r[2],
            if r[3].is_empty() { String::new() } else { format!(" «{}»", r[3]) },
            r[0],
            r[4],
            if links.is_empty() { "none".to_string() } else { links.iter().take(12).map(|l| format!("{} {} \"{}\" ({})", l[0], l[1], l[2], l[3])).collect::<Vec<_>>().join("; ") },
            if diagrams.is_empty() { "none".to_string() } else { diagrams.iter().take(6).map(|d| format!("\"{}\"", d[0])).collect::<Vec<_>>().join(", ") }
        ));
    }
    if rows.len() > offset + 30 {
        out.push_str(&format!("\nMore: offset {}.\n", offset + 30));
    }
    Ok(out)
}

/// The rest of find_orphans: the elements nothing uses in the scope.
fn find_orphans_rest(input: &Value, cfg: &AppConfig, scope: String, title: String) -> Result<String> {
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

    /// LC-369: names that differ only in case, spaces, _ - . or a plural s are one key.
    #[test]
    fn duplicate_name_keys() {
        assert_eq!(super::name_key("Customer"), super::name_key("customers"));
        assert_eq!(super::name_key("Order Line"), super::name_key("order_line"));
        assert_eq!(super::name_key("e-mail"), super::name_key("Email"));
        assert_ne!(super::name_key("Customer"), super::name_key("Custom"));
        assert_eq!(super::name_key("Bus"), "bus", "a stem shorter than 3 letters keeps its s");
        assert_eq!(super::name_key("as"), "as");
    }

    /// LC-380: elements without notes with their context, on a model copy:
    /// `LOONAR_TEST_PROPS=E:/livetest/undo/undo.properties cargo test without_notes_live -- --ignored --nocapture`
    #[test]
    #[ignore = "requires the model from LOONAR_TEST_PROPS"]
    fn without_notes_live() {
        let cfg = crate::config::AppConfig::load(std::path::Path::new(&std::env::var("LOONAR_TEST_PROPS").expect("LOONAR_TEST_PROPS"))).expect("app.properties");
        let found = super::elements_without_notes(&serde_json::json!({ "types": ["Class"] }), &cfg).expect("elements_without_notes");
        println!("{}", found.lines().take(9).collect::<Vec<_>>().join("\n"));
        assert!(found.starts_with("Elements without a description in the whole model") && found.contains("relationships:"));
    }

    /// LC-369: duplicates on a test model (reads only):
    /// `LOONAR_TEST_PROPS=E:/livetest/undo/undo.properties cargo test find_duplicates_live -- --ignored --nocapture`
    #[test]
    #[ignore = "requires the model from LOONAR_TEST_PROPS"]
    fn find_duplicates_live() {
        let cfg = crate::config::AppConfig::load(std::path::Path::new(&std::env::var("LOONAR_TEST_PROPS").expect("LOONAR_TEST_PROPS"))).expect("app.properties");
        let found = super::find_duplicates(&serde_json::json!({}), &cfg).expect("find_duplicates");
        println!("{found}");
        assert!(found.starts_with("Likely duplicates in the whole model"));
    }

    /// LC-365: a relationship's diagrams on any test model:
    /// `LOONAR_TEST_PROPS=E:/livetest/undo/undo.properties cargo test connector_where_used_live -- --ignored --nocapture`
    #[test]
    #[ignore = "requires the model from LOONAR_TEST_PROPS"]
    fn connector_where_used_live() {
        let cfg = crate::config::AppConfig::load(std::path::Path::new(&std::env::var("LOONAR_TEST_PROPS").expect("LOONAR_TEST_PROPS"))).expect("app.properties");
        let id: i64 = crate::tools::query_rows(&cfg, "select c.connector_id from t_connector c where exists (select 1 from t_diagramobjects a join t_diagramobjects b on b.diagram_id = a.diagram_id where a.object_id = c.start_object_id and b.object_id = c.end_object_id) order by c.connector_id")
            .unwrap()[0][0].parse().unwrap();
        let used = super::where_used(&serde_json::json!({ "connector": id }), &cfg).expect("where_used for a connector");
        println!("{used}");
        assert!(used.starts_with("Uses of relationship") && used.contains("Diagrams where it is visible (") && !used.contains("Diagrams where it is visible (0)"));
    }
}

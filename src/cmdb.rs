//! CMDB ↔ model: configuration items exported from a CMDB (CSV or JSON, e.g. a ServiceNow list export or a Table API
//! answer, also text fetched by an MCP server in the conversation) are compared with the components of the model by
//! name (or by a CMDB id kept in a tagged value). The report says what is only in the CMDB, what is only in the model,
//! and where the owner or the status differ. Read only: nothing is written to the model.

use crate::config::AppConfig;
use crate::tools::query_rows;
use anyhow::{bail, Context, Result};
use serde_json::Value;
use std::collections::{HashMap, HashSet};

/// One configuration item of the CMDB.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Ci {
    pub id: String,
    pub name: String,
    pub class: String,
    pub owner: String,
    pub status: String,
}

/// One element of the model that stands for a configuration item.
#[derive(Clone, Debug, Default)]
pub struct ModelCi {
    pub object_id: String,
    pub name: String,
    pub kind: String,
    pub package: String,
    pub cmdb_id: String,
    pub owner: String,
    pub status: String,
}

/// The comparison as data (for the report and the check mode).
#[derive(Default, Debug)]
pub struct Comparison {
    pub cmdb_items: usize,
    pub model_items: usize,
    pub matched: usize,
    pub only_cmdb: Vec<String>,
    pub only_model: Vec<String>,
    pub differences: Vec<String>,
}

impl Comparison {
    pub fn count(&self) -> usize {
        self.only_cmdb.len() + self.only_model.len() + self.differences.len()
    }

    pub fn sections(&self) -> Vec<(&'static str, &Vec<String>)> {
        vec![
            (crate::i18n::tr("In the CMDB, not in the model"), &self.only_cmdb),
            (crate::i18n::tr("In the model, not in the CMDB"), &self.only_model),
            (crate::i18n::tr("Different owner or status"), &self.differences),
        ]
    }
}

const NAME: &[&str] = &["name", "nazwa", "display_name", "u_name", "ci", "ci name", "configuration item", "application", "aplikacja"];
const ID: &[&str] = &["sys_id", "id", "ci id", "ci_id", "number", "asset_tag", "identifier"];
const CLASS: &[&str] = &["sys_class_name", "class", "ci class", "type", "category", "klasa", "typ"];
const OWNER: &[&str] = &["owned_by", "owner", "managed_by", "support_group", "właściciel", "wlasciciel", "business owner", "it owner"];
const STATUS: &[&str] = &["install_status", "operational_status", "status", "lifecycle", "life_cycle_stage", "state", "stan"];

/// A header compared without case, spaces, "_" and "-" ("Owned by" = "owned_by").
fn key(text: &str) -> String {
    text.to_lowercase().chars().filter(|c| !matches!(c, ' ' | '_' | '-')).collect()
}

fn column(headers: &[String], names: &[&str]) -> Option<usize> {
    names.iter().find_map(|n| headers.iter().position(|h| key(h) == key(n)))
}

/// Splits one CSV line (quotes with "" inside them).
fn csv_fields(line: &str, delimiter: char) -> Vec<String> {
    let mut out = Vec::new();
    let mut field = String::new();
    let mut quoted = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' if quoted && chars.peek() == Some(&'"') => {
                field.push('"');
                chars.next();
            }
            '"' => quoted = !quoted,
            c if c == delimiter && !quoted => out.push(std::mem::take(&mut field).trim().to_string()),
            c => field.push(c),
        }
    }
    out.push(field.trim().to_string());
    out
}

/// A value of a JSON field: text, a number, or ServiceNow's {"display_value", "value"}.
fn json_text(v: &Value) -> String {
    match v {
        Value::String(s) => s.trim().to_string(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Object(o) => o.get("display_value").or_else(|| o.get("name")).or_else(|| o.get("value")).map(json_text).unwrap_or_default(),
        _ => String::new(),
    }
}

/// Configuration items from a CSV or JSON text.
pub fn parse(text: &str) -> Result<Vec<Ci>> {
    let trimmed = text.trim_start_matches('\u{feff}').trim();
    if trimmed.starts_with('[') || trimmed.starts_with('{') {
        let value: Value = serde_json::from_str(trimmed).context(crate::i18n::tr("the CMDB data is not valid JSON"))?;
        let rows = value.as_array().or_else(|| value["result"].as_array()).or_else(|| value["records"].as_array()).or_else(|| value["items"].as_array())
            .with_context(|| crate::i18n::tr("the CMDB JSON is not a list of items (or {\"result\": […]})"))?;
        let mut out = Vec::new();
        for row in rows {
            let Some(o) = row.as_object() else { continue };
            let lower: HashMap<String, &Value> = o.iter().map(|(k, v)| (key(k), v)).collect();
            let pick = |names: &[&str]| names.iter().find_map(|n| lower.get(&key(n)).map(|v| json_text(v))).unwrap_or_default();
            let ci = Ci { id: pick(ID), name: pick(NAME), class: pick(CLASS), owner: pick(OWNER), status: pick(STATUS) };
            if !ci.name.is_empty() {
                out.push(ci);
            }
        }
        return Ok(out);
    }
    let mut lines = trimmed.lines().filter(|l| !l.trim().is_empty());
    let header = lines.next().with_context(|| crate::i18n::tr("the CMDB data is empty"))?;
    let delimiter = [';', '\t', ','].into_iter().max_by_key(|d| header.matches(*d).count()).unwrap_or(',');
    let headers: Vec<String> = csv_fields(header, delimiter).into_iter().map(|h| h.to_lowercase()).collect();
    let name = column(&headers, NAME).with_context(|| crate::i18n::fmt("no name column in the CMDB data (columns: {0})", &[headers.join(", ")]))?;
    let (id, class, owner, status) = (column(&headers, ID), column(&headers, CLASS), column(&headers, OWNER), column(&headers, STATUS));
    let mut out = Vec::new();
    for line in lines {
        let f = csv_fields(line, delimiter);
        let get = |i: Option<usize>| i.and_then(|i| f.get(i)).cloned().unwrap_or_default();
        let ci = Ci { id: get(id), name: get(Some(name)), class: get(class), owner: get(owner), status: get(status) };
        if !ci.name.is_empty() {
            out.push(ci);
        }
    }
    Ok(out)
}

fn norm(text: &str) -> String {
    text.to_lowercase().chars().filter(|c| c.is_alphanumeric()).collect()
}

/// Element types that stand for configuration items, unless the call names others.
const DEFAULT_TYPES: &[&str] = &["Component", "Node", "Device", "ExecutionEnvironment", "«ArchiMate_ApplicationComponent»", "«ArchiMate_Node»", "«ArchiMate_SystemSoftware»", "«ArchiMate_Device»", "«ArchiMate_TechnologyService»", "«ArchiMate_ApplicationService»"];

/// Elements of the model (or of one package with subpackages) of the given types, with their CMDB id, owner and status.
pub fn model_items(cfg: &AppConfig, package: Option<i64>, types: &[String]) -> Result<Vec<ModelCi>> {
    let filter = match package {
        Some(p) => {
            let ids: Vec<String> = query_rows(cfg, &format!(
                "with recursive tree(package_id) as (select package_id from t_package where package_id = {p} \
                 union all select x.package_id from t_package x join tree t on x.parent_id = t.package_id) select package_id from tree"
            ))?
            .into_iter()
            .map(|r| r[0].clone())
            .collect();
            if ids.is_empty() {
                bail!("{}", crate::i18n::fmt("there is no package with id {0}", &[p.to_string()]));
            }
            format!(" and o.package_id in ({})", ids.join(","))
        }
        None => String::new(),
    };
    let rows = query_rows(cfg, &format!(
        "select o.object_id as c0, coalesce(o.name,'') as c1, o.object_type as c2, coalesce(o.stereotype,'') as c3, coalesce(p.name,'') as c4, coalesce(o.status,'') as c5 \
         from t_object o left join t_package p on p.package_id = o.package_id where o.object_type not in ('Package','Note','Text','Boundary'){filter}"
    ))?;
    let selectors: Vec<String> = if types.is_empty() { DEFAULT_TYPES.iter().map(|s| s.to_string()).collect() } else { types.to_vec() };
    let mut tags: HashMap<String, Vec<(String, String)>> = HashMap::new();
    for r in query_rows(cfg, "select object_id as c0, lower(coalesce(property,'')) as c1, coalesce(value,'') as c2 from t_objectproperties").unwrap_or_default() {
        tags.entry(r[0].clone()).or_default().push((r[1].clone(), r[2].clone()));
    }
    Ok(rows
        .into_iter()
        .filter(|r| selectors.iter().any(|s| crate::governance::selector_matches(s, &r[2], &r[3])))
        .map(|r| {
            let t = tags.get(&r[0]).cloned().unwrap_or_default();
            let tag = |names: &[&str]| t.iter().find(|(k, v)| names.contains(&k.as_str()) && !v.trim().is_empty()).map(|(_, v)| v.trim().to_string()).unwrap_or_default();
            ModelCi {
                object_id: r[0].clone(),
                name: r[1].clone(),
                kind: if r[3].is_empty() { r[2].clone() } else { format!("{} «{}»", r[2], r[3]) },
                package: r[4].clone(),
                cmdb_id: tag(&["sys_id", "cmdb id", "cmdb_id", "ci id", "ci_id", "ci"]),
                owner: tag(&["owner", "owned_by", "właściciel", "wlasciciel", "business owner", "it owner"]),
                status: if r[5].trim().is_empty() { tag(&["status", "lifecycle", "install_status"]) } else { r[5].trim().to_string() },
            }
        })
        .collect())
}

/// Matches the CMDB with the model: by the CMDB id kept in the model, otherwise by name.
pub fn compare_items(cmdb: &[Ci], model: &[ModelCi]) -> Comparison {
    let mut c = Comparison { cmdb_items: cmdb.len(), model_items: model.len(), ..Default::default() };
    let mut used: HashSet<usize> = HashSet::new();
    for ci in cmdb {
        let found = model
            .iter()
            .enumerate()
            .find(|(i, m)| !used.contains(i) && !ci.id.is_empty() && m.cmdb_id == ci.id)
            .or_else(|| model.iter().enumerate().find(|(i, m)| !used.contains(i) && norm(&m.name) == norm(&ci.name) && !norm(&ci.name).is_empty()));
        match found {
            Some((i, m)) => {
                used.insert(i);
                c.matched += 1;
                let mut diff = Vec::new();
                if !ci.owner.is_empty() && !m.owner.is_empty() && norm(&ci.owner) != norm(&m.owner) {
                    diff.push(crate::i18n::fmt("owner: CMDB {0}, model {1}", &[ci.owner.clone(), m.owner.clone()]));
                }
                if !ci.status.is_empty() && !m.status.is_empty() && norm(&ci.status) != norm(&m.status) {
                    diff.push(crate::i18n::fmt("status: CMDB {0}, model {1}", &[ci.status.clone(), m.status.clone()]));
                }
                if !ci.owner.is_empty() && m.owner.is_empty() {
                    diff.push(crate::i18n::fmt("owner only in the CMDB: {0}", &[ci.owner.clone()]));
                }
                if !diff.is_empty() {
                    c.differences.push(format!("{} (#{}): {}", m.name, m.object_id, diff.join("; ")));
                }
            }
            None => c.only_cmdb.push(format!("{}{}{}{}", ci.name, if ci.class.is_empty() { String::new() } else { format!(" ({})", ci.class) },
                if ci.id.is_empty() { String::new() } else { format!(" [{}]", ci.id) }, if ci.owner.is_empty() { String::new() } else { format!(" – {}", ci.owner) })),
        }
    }
    for (i, m) in model.iter().enumerate() {
        if !used.contains(&i) {
            c.only_model.push(format!("{} [{}] #{} ({})", m.name, m.kind, m.object_id, m.package));
        }
    }
    c
}

/// The comparison of CMDB data with the model.
pub fn compare(cfg: &AppConfig, text: &str, package: Option<i64>, types: &[String]) -> Result<Comparison> {
    let cmdb = parse(text)?;
    if cmdb.is_empty() {
        bail!("{}", crate::i18n::tr("no configuration items with a name in the CMDB data"));
    }
    let model = model_items(cfg, package, types)?;
    Ok(compare_items(&cmdb, &model))
}

/// The report for the assistant.
pub fn report(c: &Comparison) -> String {
    let mut out = vec![crate::i18n::fmt(
        "CMDB ↔ model: {0} configuration items, {1} elements of the model, matched {2}; only in the CMDB {3}, only in the model {4}, different owner or status {5}.",
        &[c.cmdb_items.to_string(), c.model_items.to_string(), c.matched.to_string(), c.only_cmdb.len().to_string(), c.only_model.len().to_string(), c.differences.len().to_string()],
    )];
    for (title, items) in c.sections() {
        if !items.is_empty() {
            out.push(format!("\n## {title}"));
            out.extend(items.iter().take(300).cloned());
        }
    }
    out.push(crate::i18n::tr("Matching is by the CMDB id in a tagged value (sys_id, CMDB id) or by name. Present the main differences; on request add the missing elements, owners or ids to the model (ea_write, after the user agrees) or save the report to Excel.").to_string());
    out.join("\n")
}

pub fn definition() -> Value {
    serde_json::json!({
        "name": "cmdb_compare",
        "description": "CMDB ↔ model: compares configuration items from a CMDB (ServiceNow or another) with the components of the model. Give the CMDB data as \
'file' (a full path to a CSV or JSON export; an attachment of the conversation has its saved path) or as 'text' (CSV or JSON, e.g. what an MCP server of the \
CMDB returned in this conversation). Items are matched by the CMDB id kept in a tagged value (sys_id, CMDB id) or by name. The report: only in the CMDB, only \
in the model, different owner (tagged value Owner) or status. By default compares Components, Nodes, Devices, Execution environments and ArchiMate application \
components, nodes, system software and services; 'types' can name others (e.g. [\"Class\", \"«ArchiMate_ApplicationComponent»\"]). Read only.",
        "input_schema": {
            "type": "object",
            "properties": {
                "file": { "type": "string" },
                "text": { "type": "string" },
                "package": { "type": "integer", "description": "Optionally only this package with subpackages." },
                "types": { "type": "array", "items": { "type": "string" } }
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csv_and_json() {
        let csv = "Name;Class;Owned by;Install Status;Sys ID\nOrder Service;Application;Anna Nowak;Installed;abc1\n\"Billing; legacy\";Application;;Retired;abc2\n";
        let items = parse(csv).unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[1].name, "Billing; legacy");
        assert_eq!((items[0].owner.as_str(), items[0].status.as_str(), items[0].id.as_str()), ("Anna Nowak", "Installed", "abc1"));
        let json = r#"{"result":[{"name":"CRM","sys_class_name":"cmdb_ci_appl","owned_by":{"display_value":"Jan Kowalski","value":"x1"},"sys_id":"s9"}]}"#;
        let items = parse(json).unwrap();
        assert_eq!((items[0].name.as_str(), items[0].owner.as_str(), items[0].class.as_str()), ("CRM", "Jan Kowalski", "cmdb_ci_appl"));
        assert!(parse("a,b\n1,2").is_err(), "no name column");
    }

    #[test]
    fn matching() {
        let cmdb = vec![
            Ci { name: "Order Service".into(), owner: "Anna".into(), ..Default::default() },
            Ci { name: "Payments".into(), id: "s1".into(), status: "Retired".into(), ..Default::default() },
            Ci { name: "Fax gateway".into(), ..Default::default() },
        ];
        let model = vec![
            ModelCi { object_id: "1".into(), name: "OrderService".into(), owner: "Piotr".into(), ..Default::default() },
            ModelCi { object_id: "2".into(), name: "Payment gateway".into(), cmdb_id: "s1".into(), status: "Production".into(), ..Default::default() },
            ModelCi { object_id: "3".into(), name: "Warehouse".into(), ..Default::default() },
        ];
        let c = compare_items(&cmdb, &model);
        assert_eq!(c.matched, 2);
        assert_eq!(c.only_cmdb.len(), 1);
        assert!(c.only_cmdb[0].starts_with("Fax gateway"));
        assert_eq!(c.only_model.len(), 1);
        assert!(c.only_model[0].starts_with("Warehouse"));
        assert_eq!(c.differences.len(), 2, "{:?}", c.differences);
    }
}

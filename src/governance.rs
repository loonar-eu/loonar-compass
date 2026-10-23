//! Model governance without the AI: deterministic quality checks of a package (model linter), the organization's
//! rules from `governance.json`, comparison of two packages (as-is vs to-be) and the before/after preview of a write.

use crate::config::AppConfig;
use crate::tools::query_rows;
use anyhow::{Context, Result};
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap, HashSet};

/// At most this many findings of one rule are listed (the count is always complete).
const MAX_PER_RULE: usize = 60;

/// Element types without their own meaning (notes, texts, frames, pseudo-states): no description or relations expected.
const DECORATION: &[&str] = &[
    "Note", "Text", "Boundary", "Port", "ActionPin", "ActivityParameter", "ExpansionNode", "Decision", "MergeNode",
    "Synchronization", "StateNode", "InitialNode", "ActivityFinal", "FlowFinal", "Event", "Constraint", "ProvidedInterface",
    "RequiredInterface", "ActivityPartition", "Partition", "Swimlane", "InteractionFragment", "Sequence", "Object",
];

/// Names EA gives to new elements ("Class1", "Activity 2", "ProvidedInterface1").
const DEFAULT_NAMES: &[&str] = &[
    "class", "activity", "action", "component", "package", "usecase", "use case", "requirement", "actor", "interface",
    "providedinterface", "requiredinterface", "node", "object", "state", "decision", "element", "port", "part", "task",
    "gateway", "event", "process", "artifact", "device", "signal", "lane", "pool", "screen", "table", "entity", "new element",
    "new class", "new activity", "startevent", "endevent", "intermediateevent", "start event", "end event",
    // Names that EA and users give in Polish.
    "nowa klasa", "nowe zadanie", "nowy element", "nowy pakiet", "nowa aktywność", "nowy aktor", "nowy przypadek użycia", "nowy komponent", "zadanie", "zdarzenie",
];

/// Types whose empty name is normal (pseudo-states, gateways, frames): only a default name given by EA is reported.
const UNNAMED_OK: &[&str] = &[
    "Note", "Text", "Boundary", "Port", "ActionPin", "ActivityParameter", "ExpansionNode", "Decision", "MergeNode", "Synchronization",
    "StateNode", "InitialNode", "ActivityFinal", "FlowFinal", "InteractionFragment", "Sequence", "Object", "ActivityPartition", "Partition", "Swimlane",
];

/// Element types a control or object flow may not start or end at.
const NOT_FLOW_NODES: &[&str] = &["Actor", "Class", "Component", "Interface", "Package", "Requirement", "UseCase", "Node", "DataType", "Enumeration"];

/// One rule finding: which rule, object id, kind of object, name, package and what is wrong.
#[derive(Clone)]
struct Finding {
    rule: &'static str,
    id: String,
    kind: &'static str,
    name: String,
    package: String,
    detail: String,
}

fn sql_list(values: &[String]) -> String {
    values.iter().map(|v| format!("'{}'", v.replace('\'', "''"))).collect::<Vec<_>>().join(",")
}

/// Ids of the package and all its subpackages.
fn subtree(cfg: &AppConfig, package: i64) -> Result<Vec<String>> {
    let rows = query_rows(cfg, &format!(
        "with recursive tree(package_id) as (select package_id as c0 from t_package where package_id = {package} \
         union all select p.package_id as c0 from t_package p join tree t on p.parent_id = t.package_id) select package_id as c0 from tree"
    ))?;
    anyhow::ensure!(!rows.is_empty(), "{}", crate::i18n::fmt("there is no package with id {0}", &[package.to_string()]));
    Ok(rows.into_iter().map(|r| r[0].clone()).collect())
}

/// Whether the name is one EA gives by default (type name + optional number) or empty.
pub fn is_default_name(name: &str) -> bool {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return true;
    }
    let base = trimmed.trim_end_matches(|c: char| c.is_ascii_digit()).trim().to_lowercase();
    DEFAULT_NAMES.contains(&base.as_str())
}

/// Simple wildcard match: `*` any text, `?` one character; case-insensitive.
pub fn wildcard(pattern: &str, text: &str) -> bool {
    fn go(p: &[char], t: &[char]) -> bool {
        match (p.first(), t.first()) {
            (None, None) => true,
            (Some('*'), _) => go(&p[1..], t) || (!t.is_empty() && go(p, &t[1..])),
            (Some('?'), Some(_)) => go(&p[1..], &t[1..]),
            (Some(a), Some(b)) => a == b && go(&p[1..], &t[1..]),
            _ => false,
        }
    }
    let p: Vec<char> = pattern.to_lowercase().chars().collect();
    let t: Vec<char> = text.to_lowercase().chars().collect();
    go(&p, &t)
}

/// Whether an element (type, stereotype) matches a rule selector: "Class", "«ArchiMate_ApplicationComponent»", "*".
fn matches_selector(selector: &str, object_type: &str, stereotype: &str) -> bool {
    let s = selector.trim();
    if s == "*" || s.is_empty() {
        return true;
    }
    let s = s.trim_start_matches('«').trim_end_matches('»');
    wildcard(s, object_type) || (!stereotype.is_empty() && stereotype.split(',').any(|st| wildcard(s, st.trim())))
}

/// Rule selector for other modules (event agents): "Class", "«stereotype»", wildcards, "*".
pub fn selector_matches(selector: &str, object_type: &str, stereotype: &str) -> bool {
    matches_selector(selector, object_type, stereotype)
}

/// The organization's rules: `governance.json` next to the settings, overridden by the rules given in the call.
pub fn load_rules(cfg: &AppConfig, given: &Value) -> Value {
    if given.is_object() {
        return given.clone();
    }
    // Local rules first; without them the team rules from the hub policy.
    std::fs::read_to_string(cfg.base_dir.join("governance.json"))
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .or_else(|| crate::hub::team_policy(cfg).map(|p| p["governance"].clone()).filter(Value::is_object))
        .unwrap_or_else(|| json!({}))
}

/// Model linter: checks the package with its subpackages and returns the findings grouped by rule.
pub fn lint(input: &Value, cfg: &AppConfig) -> Result<String> {
    let package = input["package"].as_i64().context(crate::i18n::tr("missing field 'package' (package_id)"))?;
    let rules = load_rules(cfg, &input["rules"]);
    let skip: HashSet<String> = input["skip"].as_array().into_iter().flatten().filter_map(|v| v.as_str()).map(String::from).collect();
    let packages = subtree(cfg, package)?;
    let ids = packages.join(",");
    let decoration = sql_list(&DECORATION.iter().map(|s| s.to_string()).collect::<Vec<_>>());

    let elements = query_rows(cfg, &format!(
        "select o.object_id as c0, coalesce(o.name,'') as c1, o.object_type as c2, coalesce(o.stereotype,'') as c3, coalesce(o.note,'') as c4, p.name as c5, o.package_id as c6 \
         from t_object o join t_package p on p.package_id = o.package_id where o.package_id in ({ids}) and o.object_type <> 'Package'"
    ))?;
    let connected: HashSet<String> = query_rows(cfg, &format!(
        "select start_object_id as c0 from t_connector where start_object_id in (select object_id as c0 from t_object where package_id in ({ids})) \
         union select end_object_id as c0 from t_connector where end_object_id in (select object_id as c0 from t_object where package_id in ({ids}))"
    ))?.into_iter().map(|r| r[0].clone()).collect();
    let on_diagram: HashSet<String> = query_rows(cfg, &format!(
        "select distinct d.object_id as c0 from t_diagramobjects d join t_object o on o.object_id = d.object_id where o.package_id in ({ids})"
    ))?.into_iter().map(|r| r[0].clone()).collect();

    let mut findings: Vec<Finding> = Vec::new();
    let is_decoration = |t: &str| DECORATION.contains(&t);
    let mut same_name: HashMap<(String, String, String), Vec<String>> = HashMap::new();
    for e in &elements {
        let (id, name, otype, stereo, note, pkg) = (&e[0], &e[1], &e[2], &e[3], &e[4], &e[5]);
        let base = |rule, detail: String| Finding { rule, id: id.clone(), kind: "element", name: name.clone(), package: pkg.clone(), detail };
        let shown = if stereo.is_empty() { otype.clone() } else { format!("{otype} «{stereo}»") };
        // Default names given by EA count everywhere (also BPMN events); an empty name only where a name is expected.
        if (!name.trim().is_empty() && is_default_name(name) && !matches!(otype.as_str(), "Note" | "Text" | "Boundary")) || (name.trim().is_empty() && !UNNAMED_OK.contains(&otype.as_str())) {
            findings.push(base("default_name", crate::i18n::fmt("default or empty name ({0})", &[shown.clone()])));
        }
        if !is_decoration(otype) {
            if note.trim().is_empty() {
                findings.push(base("missing_notes", crate::i18n::fmt("no description ({0})", &[shown.clone()])));
            }
            if !connected.contains(id) && !on_diagram.contains(id) {
                findings.push(base("orphan", crate::i18n::fmt("no relationships and on no diagram ({0})", &[shown.clone()])));
            }
            if !name.trim().is_empty() {
                same_name.entry((e[6].clone(), otype.clone(), name.trim().to_lowercase())).or_default().push(id.clone());
            }
        }
        // Naming conventions from the rules: {"naming": [{"applies_to": "Class", "pattern": "*Service", "message": "…"}]}.
        for rule in rules["naming"].as_array().into_iter().flatten() {
            let selector = rule["applies_to"].as_str().unwrap_or("*");
            let pattern = rule["pattern"].as_str().unwrap_or("*");
            if matches_selector(selector, otype, stereo) && !is_decoration(otype) && !wildcard(pattern, name.trim()) {
                let message = rule["message"].as_str().map(String::from).unwrap_or_else(|| crate::i18n::fmt("name does not match the pattern {0}", &[pattern.to_string()]));
                findings.push(base("naming", format!("{message} ({shown})")));
            }
        }
    }
    for ((_, otype, _), list) in same_name.iter().filter(|(_, l)| l.len() > 1) {
        for id in list {
            if let Some(e) = elements.iter().find(|e| &e[0] == id) {
                findings.push(Finding { rule: "duplicate_name", id: id.clone(), kind: "element", name: e[1].clone(), package: e[5].clone(), detail: crate::i18n::fmt("{0} elements of type {1} with this name in the package", &[list.len().to_string(), otype.clone()]) });
            }
        }
    }
    let _ = decoration;

    // Required tagged values: {"required_tags": [{"applies_to": "«ArchiMate_ApplicationComponent»", "tag": "Owner"}]}.
    let required: Vec<&Value> = rules["required_tags"].as_array().into_iter().flatten().collect();
    if !required.is_empty() {
        let mut tags: HashMap<String, HashSet<String>> = HashMap::new();
        for r in query_rows(cfg, &format!(
            "select t.object_id as c0, lower(t.property) as c1, coalesce(t.value,'') as c2 from t_objectproperties t join t_object o on o.object_id = t.object_id where o.package_id in ({ids})"
        ))? {
            if !r[2].trim().is_empty() {
                tags.entry(r[0].clone()).or_default().insert(r[1].clone());
            }
        }
        for e in &elements {
            for rule in &required {
                let tag = rule["tag"].as_str().unwrap_or("");
                if !tag.is_empty() && matches_selector(rule["applies_to"].as_str().unwrap_or("*"), &e[2], &e[3]) && !tags.get(&e[0]).is_some_and(|t| t.contains(&tag.to_lowercase())) {
                    findings.push(Finding { rule: "required_tag", id: e[0].clone(), kind: "element", name: e[1].clone(), package: e[5].clone(), detail: crate::i18n::fmt("missing tagged value {0}", &[tag.to_string()]) });
                }
            }
        }
    }

    // Relationships: broken ends, duplicates, self-relationships and the allowed relationships from the rules.
    let connectors = query_rows(cfg, &format!(
        "select c.connector_id as c0, c.connector_type as c1, coalesce(c.stereotype,'') as c2, c.start_object_id as c3, c.end_object_id as c4, \
         coalesce(s.name,'') as c5, coalesce(s.object_type,'') as c6, coalesce(s.stereotype,'') as c7, coalesce(t.name,'') as c8, coalesce(t.object_type,'') as c9, coalesce(t.stereotype,'') as c10, \
         case when s.object_id is null then 0 else 1 end as c11, case when t.object_id is null then 0 else 1 end as c12, coalesce(p.name,'') as c13 \
         from t_connector c left join t_object s on s.object_id = c.start_object_id left join t_object t on t.object_id = c.end_object_id \
         left join t_package p on p.package_id = s.package_id \
         where c.start_object_id in (select object_id as c0 from t_object where package_id in ({ids})) or c.end_object_id in (select object_id as c0 from t_object where package_id in ({ids}))"
    ))?;
    let allowed: Vec<&Value> = rules["allowed_relations"].as_array().into_iter().flatten().collect();
    let checked_types: HashSet<String> = allowed.iter().filter_map(|r| r["connector_type"].as_str()).map(|s| s.to_lowercase()).collect();
    let mut seen: HashMap<(String, String, String, String), usize> = HashMap::new();
    for c in &connectors {
        let label = format!("{} → {}", c[5], c[8]);
        let kind = if c[2].is_empty() { c[1].clone() } else { format!("{} «{}»", c[1], c[2]) };
        let base = |rule, detail: String| Finding { rule, id: c[0].clone(), kind: "connector", name: label.clone(), package: c[13].clone(), detail };
        if c[11] == "0" || c[12] == "0" {
            findings.push(base("broken_connector", crate::i18n::fmt("{0} points to an element that does not exist", &[kind.clone()])));
            continue;
        }
        if c[3] == c[4] && !matches!(c[1].as_str(), "StateFlow" | "Sequence" | "Transition" | "SelfMessage") {
            findings.push(base("self_relation", crate::i18n::fmt("{0} from the element to itself", &[kind.clone()])));
        }
        if let Some(problem) = uml_problem(&c[1], &c[6], &c[9]) {
            findings.push(base("invalid_relation", problem));
        }
        *seen.entry((c[1].clone(), c[2].clone(), c[3].clone(), c[4].clone())).or_default() += 1;
        if checked_types.contains(&c[1].to_lowercase()) || checked_types.contains(&c[2].to_lowercase()) {
            let ok = allowed.iter().any(|r| {
                let t = r["connector_type"].as_str().unwrap_or("");
                (t.eq_ignore_ascii_case(&c[1]) || t.eq_ignore_ascii_case(&c[2]))
                    && matches_selector(r["source"].as_str().unwrap_or("*"), &c[6], &c[7])
                    && matches_selector(r["target"].as_str().unwrap_or("*"), &c[9], &c[10])
            });
            if !ok {
                let ends = format!("{} → {}", if c[7].is_empty() { &c[6] } else { &c[7] }, if c[10].is_empty() { &c[9] } else { &c[10] });
                findings.push(base("invalid_relation", crate::i18n::fmt("{0} is not allowed between {1}", &[kind.clone(), ends])));
            }
        }
    }
    for ((ctype, stereo, from, to), n) in seen.iter().filter(|(_, n)| **n > 1) {
        let names = connectors.iter().find(|c| &c[3] == from && &c[4] == to).map(|c| format!("{} → {}", c[5], c[8])).unwrap_or_default();
        let kind = if stereo.is_empty() { ctype.clone() } else { format!("{ctype} «{stereo}»") };
        findings.push(Finding { rule: "duplicate_relation", id: format!("{from}→{to}"), kind: "connector", name: names, package: String::new(), detail: crate::i18n::fmt("{0} repeated {1} times", &[kind, n.to_string()]) });
    }

    // Packages and diagrams without content.
    for r in query_rows(cfg, &format!(
        "select p.package_id as c0, p.name as c1 from t_package p where p.package_id in ({ids}) and p.package_id <> {package} \
         and not exists (select 1 as c0 from t_object o where o.package_id = p.package_id and o.object_type <> 'Package') \
         and not exists (select 1 as c0 from t_diagram d where d.package_id = p.package_id) \
         and not exists (select 1 as c0 from t_package c where c.parent_id = p.package_id)"
    ))? {
        findings.push(Finding { rule: "empty_package", id: r[0].clone(), kind: "package", name: r[1].clone(), package: String::new(), detail: crate::i18n::tr("no elements, diagrams or subpackages").to_string() });
    }
    for r in query_rows(cfg, &format!(
        "select d.diagram_id as c0, d.name as c1, p.name as c2 from t_diagram d join t_package p on p.package_id = d.package_id where d.package_id in ({ids}) \
         and not exists (select 1 as c0 from t_diagramobjects o where o.diagram_id = d.diagram_id)"
    ))? {
        findings.push(Finding { rule: "empty_diagram", id: r[0].clone(), kind: "diagram", name: r[1].clone(), package: r[2].clone(), detail: crate::i18n::tr("no elements on the diagram").to_string() });
    }

    findings.retain(|f| !skip.contains(f.rule));
    Ok(report(package, packages.len(), elements.len(), connectors.len(), &findings, &rules))
}

fn rule_title(rule: &str) -> &'static str {
    match rule {
        "default_name" => crate::i18n::tr("Default or empty names"),
        "missing_notes" => crate::i18n::tr("Elements without a description"),
        "orphan" => crate::i18n::tr("Orphaned elements (no relationships, on no diagram)"),
        "duplicate_name" => crate::i18n::tr("Duplicate names in one package"),
        "naming" => crate::i18n::tr("Naming convention violations"),
        "required_tag" => crate::i18n::tr("Missing required tagged values"),
        "broken_connector" => crate::i18n::tr("Relationships to non-existent elements"),
        "self_relation" => crate::i18n::tr("Relationships of an element to itself"),
        "duplicate_relation" => crate::i18n::tr("Duplicate relationships"),
        "invalid_relation" => crate::i18n::tr("Relationships not allowed by the rules"),
        "empty_package" => crate::i18n::tr("Empty packages"),
        "empty_diagram" => crate::i18n::tr("Empty diagrams"),
        _ => crate::i18n::tr("other operations"),
    }
}

/// How serious the rule is: errors break the model, warnings lower its quality, info is worth a look.
fn severity(rule: &str) -> &'static str {
    match rule {
        "broken_connector" | "invalid_relation" | "required_tag" => "error",
        "default_name" | "orphan" | "duplicate_name" | "naming" | "duplicate_relation" | "self_relation" => "warning",
        _ => "info",
    }
}

fn report(package: i64, packages: usize, elements: usize, connectors: usize, findings: &[Finding], rules: &Value) -> String {
    let mut by_rule: BTreeMap<(u8, &str), Vec<&Finding>> = BTreeMap::new();
    for f in findings {
        let order = match severity(f.rule) { "error" => 0, "warning" => 1, _ => 2 };
        by_rule.entry((order, f.rule)).or_default().push(f);
    }
    let custom = ["naming", "required_tags", "allowed_relations"].iter().filter(|k| rules[**k].as_array().is_some_and(|a| !a.is_empty())).count();
    let mut out = vec![crate::i18n::fmt(
        "Model check of package {0}: {1} packages, {2} elements, {3} relationships checked; rule sets from governance.json: {4}. Findings: {5}.",
        &[package.to_string(), packages.to_string(), elements.to_string(), connectors.to_string(), custom.to_string(), findings.len().to_string()],
    )];
    out.push(crate::i18n::tr("Columns: severity | rule | id | kind | name | package | problem").to_string());
    for ((_, rule), list) in &by_rule {
        out.push(format!("\n## {} [{}] — {}", rule_title(rule), severity(rule), list.len()));
        for f in list.iter().take(MAX_PER_RULE) {
            out.push(format!("{} | {} | {} | {} | {} | {} | {}", severity(f.rule), f.rule, f.id, f.kind, f.name, f.package, f.detail));
        }
        if list.len() > MAX_PER_RULE {
            out.push(crate::i18n::fmt("… and {0} more", &[(list.len() - MAX_PER_RULE).to_string()]));
        }
    }
    if findings.is_empty() {
        out.push(crate::i18n::tr("No findings: the package meets all checked rules.").to_string());
    }
    out.join("\n")
}

/// UML/BPMN sanity of a relationship by the types of its ends (without rules of the organization).
fn uml_problem(connector_type: &str, source_type: &str, target_type: &str) -> Option<String> {
    let flow = matches!(connector_type, "ControlFlow" | "ObjectFlow");
    if flow && (NOT_FLOW_NODES.contains(&source_type) || NOT_FLOW_NODES.contains(&target_type)) {
        return Some(crate::i18n::fmt("{0} between {1} and {2}: a flow connects activities, events and gateways", &[connector_type.to_string(), source_type.to_string(), target_type.to_string()]));
    }
    if connector_type == "Generalization" && source_type != target_type && !(matches!(source_type, "Class" | "Interface") && matches!(target_type, "Class" | "Interface")) {
        return Some(crate::i18n::fmt("{0} between {1} and {2}: a generalization connects elements of the same kind", &[connector_type.to_string(), source_type.to_string(), target_type.to_string()]));
    }
    None
}

/// Gap analysis: compares two packages (with subpackages) by element name and type.
pub fn gap(input: &Value, cfg: &AppConfig) -> Result<String> {
    let as_is = input["as_is"].as_i64().context(crate::i18n::tr("missing field 'as_is' (package_id)"))?;
    let to_be = input["to_be"].as_i64().context(crate::i18n::tr("missing field 'to_be' (package_id)"))?;
    let load = |package: i64| -> Result<BTreeMap<(String, String), Vec<String>>> {
        let ids = subtree(cfg, package)?.join(",");
        let mut map = BTreeMap::new();
        for r in query_rows(cfg, &format!(
            "select o.object_id as c0, coalesce(o.name,'') as c1, o.object_type as c2, coalesce(o.stereotype,'') as c3, coalesce(o.note,'') as c4, p.name as c5 \
             from t_object o join t_package p on p.package_id = o.package_id where o.package_id in ({ids}) and o.object_type <> 'Package' and coalesce(o.name,'') <> ''"
        ))? {
            if DECORATION.contains(&r[2].as_str()) {
                continue;
            }
            map.insert((r[1].trim().to_lowercase(), r[2].clone()), r);
        }
        Ok(map)
    };
    let relations = |ids: &[String]| -> Result<HashMap<String, BTreeMap<String, ()>>> {
        let mut out: HashMap<String, BTreeMap<String, ()>> = HashMap::new();
        if ids.is_empty() {
            return Ok(out);
        }
        let list = ids.join(",");
        for r in query_rows(cfg, &format!(
            "select c.start_object_id as c0, c.end_object_id as c1, c.connector_type as c2, coalesce(s.name,'') as c3, coalesce(t.name,'') as c4 from t_connector c \
             join t_object s on s.object_id = c.start_object_id join t_object t on t.object_id = c.end_object_id \
             where c.start_object_id in ({list}) or c.end_object_id in ({list})"
        ))? {
            out.entry(r[0].clone()).or_default().insert(format!("{} → {}", r[2], r[4].trim()), ());
            out.entry(r[1].clone()).or_default().insert(format!("{} ← {}", r[2], r[3].trim()), ());
        }
        Ok(out)
    };
    let a = load(as_is)?;
    let b = load(to_be)?;
    let rel_a = relations(&a.values().map(|r| r[0].clone()).collect::<Vec<_>>())?;
    let rel_b = relations(&b.values().map(|r| r[0].clone()).collect::<Vec<_>>())?;
    let shown = |r: &Vec<String>| if r[3].is_empty() { r[2].clone() } else { format!("{} «{}»", r[2], r[3]) };
    let (mut removed, mut added, mut changed, mut same) = (Vec::new(), Vec::new(), Vec::new(), 0usize);
    for (key, r) in &a {
        match b.get(key) {
            None => removed.push(format!("{} | {} | {} | {}", crate::i18n::tr("only in as-is (to retire)"), r[1], shown(r), r[5])),
            Some(t) => {
                let mut diffs = Vec::new();
                if r[3] != t[3] {
                    diffs.push(crate::i18n::fmt("stereotype {0} → {1}", &[r[3].clone(), t[3].clone()]));
                }
                if r[4].trim() != t[4].trim() {
                    diffs.push(crate::i18n::tr("description changed").to_string());
                }
                let empty = BTreeMap::new();
                let ra = rel_a.get(&r[0]).unwrap_or(&empty);
                let rb = rel_b.get(&t[0]).unwrap_or(&empty);
                let gone: Vec<&String> = ra.keys().filter(|k| !rb.contains_key(*k)).collect();
                let new: Vec<&String> = rb.keys().filter(|k| !ra.contains_key(*k)).collect();
                if !gone.is_empty() {
                    diffs.push(crate::i18n::fmt("relationships removed: {0}", &[gone.iter().map(|s| s.as_str()).collect::<Vec<_>>().join("; ")]));
                }
                if !new.is_empty() {
                    diffs.push(crate::i18n::fmt("relationships added: {0}", &[new.iter().map(|s| s.as_str()).collect::<Vec<_>>().join("; ")]));
                }
                if diffs.is_empty() {
                    same += 1;
                } else {
                    changed.push(format!("{} | {} | {} | {}", crate::i18n::tr("changed"), r[1], shown(r), diffs.join("; ")));
                }
            }
        }
    }
    for (key, r) in &b {
        if !a.contains_key(key) {
            added.push(format!("{} | {} | {} | {}", crate::i18n::tr("only in to-be (new)"), r[1], shown(r), r[5]));
        }
    }
    let mut out = vec![crate::i18n::fmt(
        "Gap analysis as-is (package {0}, {1} elements) vs to-be (package {2}, {3} elements): {4} new, {5} to retire, {6} changed, {7} unchanged.",
        &[as_is.to_string(), a.len().to_string(), to_be.to_string(), b.len().to_string(), added.len().to_string(), removed.len().to_string(), changed.len().to_string(), same.to_string()],
    )];
    out.push(crate::i18n::tr("Columns: gap | name | type | package or differences").to_string());
    out.extend(added.into_iter().take(300));
    out.extend(removed.into_iter().take(300));
    out.extend(changed.into_iter().take(300));
    Ok(out.join("\n"))
}

/// Before/after preview of a write for the approval dialog: what is created, what changes from which value, what is deleted.
pub fn preview(cfg: &AppConfig, ops: &[Value]) -> String {
    let mut lines = Vec::new();
    let one = |sql: String| query_rows(cfg, &sql).ok().and_then(|r| r.into_iter().next());
    let name_of = |table: &str, key: &str, id: i64| -> String {
        one(format!("select coalesce(name,'') as c0 from {table} where {key} = {id}")).map(|r| r[0].clone()).unwrap_or_else(|| format!("#{id}"))
    };
    let target = |op: &Value, field: &str| -> String {
        match &op[field] {
            Value::Number(n) => {
                let id = n.as_i64().unwrap_or(0);
                match field {
                    "package" | "parent" => format!("\"{}\"", name_of("t_package", "package_id", id)),
                    "diagram" => format!("\"{}\"", name_of("t_diagram", "diagram_id", id)),
                    _ => format!("\"{}\"", name_of("t_object", "object_id", id)),
                }
            }
            Value::String(s) => s.clone(),
            _ => String::new(),
        }
    };
    for op in ops.iter().take(80) {
        let name = op["name"].as_str().unwrap_or("");
        let line = match op["op"].as_str().unwrap_or("") {
            "create_package" => format!("+ {} \"{name}\" → {}", crate::i18n::tr("package"), target(op, "parent")),
            "create_element" => format!("+ {} \"{name}\"{} → {}", op["type"].as_str().unwrap_or("element"), op["stereotype"].as_str().map(|s| format!(" «{s}»")).unwrap_or_default(), target(op, "package")),
            "create_diagram" => format!("+ {} \"{name}\" ({}) → {}", crate::i18n::tr("diagram"), op["type"].as_str().unwrap_or("?"), target(op, "package")),
            "create_connector" => format!("+ {} {} → {}", op["type"].as_str().unwrap_or("relationship"), target(op, "source"), target(op, "target")),
            "place" => format!("+ {} → {}", target(op, "element"), target(op, "diagram")),
            "update_element" => {
                let id = op["element"].as_i64().unwrap_or(0);
                let before = one(format!("select coalesce(name,'') as c0, coalesce(stereotype,'') as c1, coalesce(alias,'') as c2, coalesce(note,'') as c3 from t_object where object_id = {id}"));
                let mut changes = Vec::new();
                if let Some(b) = &before {
                    for (field, index) in [("name", 0), ("stereotype", 1), ("alias", 2), ("notes", 3)] {
                        if let Some(after) = op[field].as_str() {
                            if after != b[index] {
                                let short = |s: &str| if s.chars().count() > 60 { format!("{}…", s.chars().take(60).collect::<String>()) } else { s.to_string() };
                                changes.push(format!("{field}: \"{}\" → \"{}\"", short(&b[index]), short(after)));
                            }
                        }
                    }
                }
                let other: Vec<&str> = op.as_object().map(|o| o.keys().map(|k| k.as_str()).filter(|k| !["op", "element", "name", "stereotype", "alias", "notes"].contains(k)).collect()).unwrap_or_default();
                if !other.is_empty() {
                    changes.push(other.join(", "));
                }
                format!("~ \"{}\": {}", before.map(|b| b[0].clone()).unwrap_or_else(|| format!("#{id}")), if changes.is_empty() { crate::i18n::tr("no visible change").to_string() } else { changes.join("; ") })
            }
            "move" => format!("→ {} → {}", ["element", "package", "diagram"].iter().map(|f| target(op, f)).find(|s| !s.is_empty()).unwrap_or_default(), target(op, "target")),
            "delete" | "delete_package" | "delete_scenario" | "delete_attribute" | "delete_operation" => {
                let what = ["element", "package", "diagram", "connector", "scenario", "attribute", "operation"].iter().map(|f| target(op, f)).find(|s| !s.is_empty()).unwrap_or_default();
                format!("− {} {what}", crate::i18n::tr("DELETE"))
            }
            "remove_from_diagram" => format!("− {} ⟵ {}", target(op, "element"), target(op, "diagram")),
            other => format!("· {other} {name}"),
        };
        lines.push(line);
    }
    if ops.len() > 80 {
        lines.push(crate::i18n::fmt("… and {0} more", &[(ops.len() - 80).to_string()]));
    }
    lines.join("\n")
}

/// Live check of one object just created or changed in EA (events from the add-in): what is wrong with it by the
/// built-in checks and the rules. `kind` is "element" or "connector". Empty = nothing to report.
pub fn check_object(cfg: &AppConfig, kind: &str, id: i64) -> Result<Vec<String>> {
    check_object_with(cfg, kind, id, &load_rules(cfg, &Value::Null))
}

pub fn check_object_with(cfg: &AppConfig, kind: &str, id: i64, rules: &Value) -> Result<Vec<String>> {
    let mut out = Vec::new();
    match kind {
        "element" => {
            let Some(e) = query_rows(cfg, &format!(
                "select coalesce(name,'') as c0, object_type as c1, coalesce(stereotype,'') as c2 from t_object where object_id = {id}"
            ))?.into_iter().next() else { return Ok(out) };
            let (name, otype, stereo) = (&e[0], &e[1], &e[2]);
            if DECORATION.contains(&otype.as_str()) || otype == "Package" {
                return Ok(out);
            }
            let shown = if stereo.is_empty() { otype.clone() } else { format!("{otype} «{stereo}»") };
            if is_default_name(name) {
                out.push(crate::i18n::fmt("\"{0}\": {1}", &[name.clone(), crate::i18n::fmt("default or empty name ({0})", &[shown.clone()])]));
            }
            for rule in rules["naming"].as_array().into_iter().flatten() {
                let pattern = rule["pattern"].as_str().unwrap_or("*");
                if matches_selector(rule["applies_to"].as_str().unwrap_or("*"), otype, stereo) && !wildcard(pattern, name.trim()) {
                    let message = rule["message"].as_str().map(String::from).unwrap_or_else(|| crate::i18n::fmt("name does not match the pattern {0}", &[pattern.to_string()]));
                    out.push(crate::i18n::fmt("\"{0}\": {1}", &[name.clone(), message]));
                }
            }
            let required: Vec<&Value> = rules["required_tags"].as_array().into_iter().flatten().filter(|r| matches_selector(r["applies_to"].as_str().unwrap_or("*"), otype, stereo)).collect();
            if !required.is_empty() {
                let tags: HashSet<String> = query_rows(cfg, &format!("select lower(property) as c0, coalesce(value,'') as c1 from t_objectproperties where object_id = {id}"))?
                    .into_iter().filter(|r| !r[1].trim().is_empty()).map(|r| r[0].clone()).collect();
                for rule in required {
                    let tag = rule["tag"].as_str().unwrap_or("");
                    if !tag.is_empty() && !tags.contains(&tag.to_lowercase()) {
                        out.push(crate::i18n::fmt("\"{0}\": {1}", &[name.clone(), crate::i18n::fmt("missing tagged value {0}", &[tag.to_string()])]));
                    }
                }
            }
        }
        "connector" => {
            let Some(c) = query_rows(cfg, &format!(
                "select c.connector_type as c0, coalesce(c.stereotype,'') as c1, c.start_object_id as c2, c.end_object_id as c3, coalesce(s.name,'') as c4, coalesce(s.object_type,'') as c5, coalesce(s.stereotype,'') as c6, \
                 coalesce(t.name,'') as c7, coalesce(t.object_type,'') as c8, coalesce(t.stereotype,'') as c9 from t_connector c left join t_object s on s.object_id = c.start_object_id \
                 left join t_object t on t.object_id = c.end_object_id where c.connector_id = {id}"
            ))?.into_iter().next() else { return Ok(out) };
            let kind = if c[1].is_empty() { c[0].clone() } else { format!("{} «{}»", c[0], c[1]) };
            let label = format!("{} → {}", c[4], c[7]);
            if let Some(problem) = uml_problem(&c[0], &c[5], &c[8]) {
                out.push(crate::i18n::fmt("\"{0}\": {1}", &[label.clone(), problem]));
            }
            if c[2] == c[3] && !matches!(c[0].as_str(), "StateFlow" | "Sequence" | "Transition" | "SelfMessage") {
                out.push(crate::i18n::fmt("\"{0}\": {1}", &[label.clone(), crate::i18n::fmt("{0} from the element to itself", &[kind.clone()])]));
            }
            let same = query_rows(cfg, &format!(
                "select count(*) as c0 from t_connector where start_object_id = {} and end_object_id = {} and connector_type = '{}' and coalesce(stereotype,'') = '{}'",
                c[2], c[3], c[0].replace('\'', "''"), c[1].replace('\'', "''")
            ))?;
            let n: usize = same.first().and_then(|r| r[0].parse().ok()).unwrap_or(1);
            if n > 1 {
                out.push(crate::i18n::fmt("\"{0}\": {1}", &[label.clone(), crate::i18n::fmt("{0} repeated {1} times", &[kind.clone(), n.to_string()])]));
            }
            let allowed: Vec<&Value> = rules["allowed_relations"].as_array().into_iter().flatten().collect();
            let listed = allowed.iter().any(|r| r["connector_type"].as_str().is_some_and(|t| t.eq_ignore_ascii_case(&c[0]) || t.eq_ignore_ascii_case(&c[1])));
            if listed {
                let ok = allowed.iter().any(|r| {
                    let t = r["connector_type"].as_str().unwrap_or("");
                    (t.eq_ignore_ascii_case(&c[0]) || t.eq_ignore_ascii_case(&c[1]))
                        && matches_selector(r["source"].as_str().unwrap_or("*"), &c[5], &c[6])
                        && matches_selector(r["target"].as_str().unwrap_or("*"), &c[8], &c[9])
                });
                if !ok {
                    let ends = format!("{} → {}", if c[6].is_empty() { &c[5] } else { &c[6] }, if c[9].is_empty() { &c[8] } else { &c[9] });
                    out.push(crate::i18n::fmt("\"{0}\": {1}", &[label, crate::i18n::fmt("{0} is not allowed between {1}", &[kind, ends])]));
                }
            }
        }
        _ => {}
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_names_and_wildcards() {
        assert!(is_default_name("Class1"));
        assert!(is_default_name("Activity 2"));
        assert!(is_default_name("ProvidedInterface1"));
        assert!(is_default_name("  "));
        assert!(!is_default_name("Order Service"));
        assert!(!is_default_name("Class Diagram of Orders"));
        assert!(wildcard("*Service", "OrderService"));
        assert!(wildcard("svc-??", "SVC-01"));
        assert!(!wildcard("*Service", "OrderController"));
        assert!(matches_selector("«ArchiMate_*»", "Component", "ArchiMate_ApplicationComponent"));
        assert!(!matches_selector("Class", "Component", ""));
    }
}

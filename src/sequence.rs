//! Sequence diagrams (LC-381): a check of what EA does not check, and a sequence diagram from text.
//! - `sequence_check` (reads only): messages whose receiver has no such operation (the list of missing operations),
//!   gaps and repeats in the message order, replies with no call before them, lifelines without messages, combined
//!   fragments (alt, opt, loop…) with no message inside, lifelines whose classifier is missing.
//! - `sequence_from_text`: a PlantUML-like text (`A -> B : order()`, `B --> A : ok`, `alt`, `loop`, `end`) as a new
//!   sequence diagram whose participants are the model's existing elements (by name) and whose messages are bound to
//!   their operations by name; optionally the missing operations are added. One ordinary save (approval, undo).

use crate::config::AppConfig;
use crate::tools::query_rows;
use anyhow::{bail, Result};
use serde_json::{json, Value};
use std::collections::HashMap;

pub fn definitions() -> Vec<Value> {
    vec![
        json!({
            "name": "sequence_check",
            "description": "Checks sequence diagrams (one diagram, or all in a package with child packages): messages whose receiver \
(the lifeline's classifier, or the element itself) has no operation of that name – the list of missing operations; gaps and repeats \
in the order of messages; replies with no call before them; lifelines without messages; combined fragments (alt, opt, loop…) with no \
message inside; lifelines without a classifier. Reads only. Use it for /check-sequence.",
            "input_schema": { "type": "object", "properties": {
                "diagram": { "description": "diagram_id or exact name of a sequence diagram" },
                "package": { "description": "package_id or exact name: all sequence diagrams in it (with child packages)" }
            } }
        }),
        json!({
            "name": "sequence_from_text",
            "description": "Creates a sequence diagram from text in a PlantUML-like form, bound to the model: participants are existing \
elements found by name (the package first), messages are matched to the receiver's operations by name. Lines: `participant Name`, \
`actor Name`, `participant \"Long name\" as L`; `A -> B : call()` (synchronous), `A ->> B : signal` (asynchronous), `B --> A : result` \
(reply), a time constraint in braces after the message (`A -> B : pay() {< 2s}`, kept as the tagged value \"time constraint\"); `alt condition` / `else condition` / `opt` / `loop` / `par` / `break` … `end` (combined fragments); @startuml, title, \
autonumber, activate and comments are skipped. Missing participants become lifelines; with create_missing_operations the receivers \
get the operations the messages call. Show the user the report (bound / created / missing) after the save.",
            "input_schema": { "type": "object", "properties": {
                "text": { "type": "string" },
                "package": { "description": "package_id or exact name: where the diagram (and new lifelines) go" },
                "name": { "type": "string", "description": "Name of the new diagram" },
                "create_missing_operations": { "type": "boolean", "description": "Add operations the messages call to receivers that lack them (default false)" },
                "numbering": { "type": "boolean", "description": "Show sequence numbers (EA numbers hierarchically by activation; default true)" }
            }, "required": ["text", "package", "name"] }
        }),
    ]
}

/// The operation a message calls: its name without the parameters ("AddBook(string, …)" → "addbook").
fn called(message: &str) -> String {
    message.split('(').next().unwrap_or("").trim().trim_end_matches(';').to_lowercase()
}

/// Kinds of combined fragments (t_object.ntype of an InteractionFragment).
const FRAGMENTS: &[(&str, i64)] = &[("alt", 0), ("opt", 1), ("break", 2), ("par", 3), ("loop", 4), ("critical", 5), ("neg", 6), ("assert", 7), ("strict", 8), ("seq", 9), ("ignore", 10), ("consider", 11)];

fn fragment_name(ntype: &str) -> &'static str {
    FRAGMENTS.iter().find(|(_, n)| n.to_string() == ntype).map(|(k, _)| *k).unwrap_or("fragment")
}

pub fn check(input: &Value, cfg: &AppConfig) -> Result<String> {
    let diagrams: Vec<Vec<String>> = if !input["diagram"].is_null() && input["diagram"].as_str() != Some("") {
        let d = &input["diagram"];
        match d.as_i64().or_else(|| d.as_str().and_then(|s| s.trim().parse().ok())) {
            Some(id) => query_rows(cfg, &format!("select diagram_id, name from t_diagram where diagram_id = {id}"))?,
            None => query_rows(cfg, &format!("select diagram_id, name from t_diagram where diagram_type = 'Sequence' and name = '{}'", crate::model_db::sql_text(cfg.model_source, d.as_str().unwrap_or(""))))?,
        }
    } else {
        let (scope, _) = crate::model_usage::package_scope(cfg, &input["package"])?;
        let scope = scope.replace("o.package_id", "package_id");
        query_rows(cfg, &format!("select diagram_id, name from t_diagram where diagram_type = 'Sequence' {scope} order by name"))?
    };
    if diagrams.is_empty() {
        bail!("no sequence diagram found");
    }
    let mut out = Vec::new();
    let mut missing_all: Vec<String> = Vec::new();
    for d in diagrams.iter().take(200) {
        let id: i64 = d[0].parse().unwrap_or(0);
        let mut findings = Vec::new();
        // Lifelines and fragments on the diagram.
        let objects = query_rows(cfg, &format!(
            "select o.object_id, coalesce(o.name,''), o.object_type, coalesce(o.classifier,0), o.ntype, x.rectleft, x.rectright, x.recttop, x.rectbottom \
             from t_diagramobjects x join t_object o on o.object_id = x.object_id where x.diagram_id = {id}"
        ))?;
        let messages = query_rows(cfg, &format!(
            "select c.connector_id, coalesce(c.name,''), c.seqno, c.start_object_id, c.end_object_id, coalesce(c.pdata4,'0'), c.ptstarty, c.ptstartx, c.ptendx \
             from t_connector c where c.diagramid = {id} and c.connector_type = 'Sequence' order by c.seqno"
        ))?;
        let name_of: HashMap<String, String> = objects.iter().map(|o| (o[0].clone(), if o[1].is_empty() { format!("(id {})", o[0]) } else { o[1].clone() })).collect();
        // The order: 1, 2, 3… without gaps or repeats.
        let numbers: Vec<i64> = messages.iter().filter_map(|m| m[2].parse().ok()).collect();
        let mut seen = std::collections::HashSet::new();
        let repeated: Vec<String> = numbers.iter().filter(|n| !seen.insert(**n)).map(|n| n.to_string()).collect();
        if !repeated.is_empty() {
            findings.push(format!("repeated message numbers: {}", repeated.join(", ")));
        }
        if let (Some(min), Some(max)) = (numbers.iter().min(), numbers.iter().max()) {
            let gaps: Vec<String> = (*min..=*max).filter(|n| !numbers.contains(n)).take(20).map(|n| n.to_string()).collect();
            if !gaps.is_empty() {
                findings.push(format!("gaps in the message order: {}", gaps.join(", ")));
            }
        }
        // Receivers' operations.
        for m in &messages {
            let target = &m[4];
            let reply = m[5] == "1";
            if reply {
                // A reply needs an earlier call in the opposite direction.
                let earlier = messages.iter().take_while(|x| x[0] != m[0]).any(|x| x[3] == m[4] && x[4] == m[3] && x[5] != "1");
                if !earlier {
                    findings.push(format!("reply \"{}\" ({} → {}) has no call before it", m[1], name_of.get(&m[3]).cloned().unwrap_or_default(), name_of.get(target).cloned().unwrap_or_default()));
                }
                continue;
            }
            let op = called(&m[1]);
            if op.is_empty() {
                findings.push(format!("message {} ({} → {}) has no name", m[2], name_of.get(&m[3]).cloned().unwrap_or_default(), name_of.get(target).cloned().unwrap_or_default()));
                continue;
            }
            let Some(o) = objects.iter().find(|o| &o[0] == target) else { continue };
            // The type that should have the operation: the lifeline's classifier, or a class placed directly.
            let owner = if o[3] != "0" && !o[3].is_empty() { o[3].clone() } else if matches!(o[2].as_str(), "Class" | "Interface" | "Component" | "Actor") { o[0].clone() } else { String::new() };
            if owner.is_empty() {
                continue;
            }
            let ops: Vec<String> = query_rows(cfg, &format!("select name from t_operation where object_id = {}", owner.parse::<i64>().unwrap_or(0)))?.into_iter().map(|r| r[0].to_lowercase()).collect();
            if !ops.contains(&op) {
                let owner_name = query_rows(cfg, &format!("select name from t_object where object_id = {}", owner.parse::<i64>().unwrap_or(0)))?.first().map(|r| r[0].clone()).unwrap_or_default();
                let line = format!("{owner_name}.{}()", m[1].split('(').next().unwrap_or("").trim());
                findings.push(format!("message {} \"{}\": {owner_name} has no operation {}", m[2], m[1], m[1].split('(').next().unwrap_or("").trim()));
                if !missing_all.contains(&line) {
                    missing_all.push(line);
                }
            }
        }
        // Lifelines: without messages, without a classifier.
        for o in objects.iter().filter(|o| o[2] == "Sequence") {
            if !messages.iter().any(|m| m[3] == o[0] || m[4] == o[0]) {
                findings.push(format!("lifeline {} has no messages", name_of[&o[0]]));
            }
            if o[3] == "0" || o[3].is_empty() {
                findings.push(format!("lifeline {} has no classifier (its messages cannot be bound to operations)", name_of[&o[0]]));
            }
        }
        // Fragments with no message inside (y between the fragment's top and bottom, x within it). Messages made through
        // EA's API have no position until the diagram is saved in EA: then fragments cannot be checked.
        let laid_out = messages.iter().any(|m| m[6].parse::<i64>().unwrap_or(0) != 0);
        if !laid_out && objects.iter().any(|o| o[2] == "InteractionFragment") {
            findings.push("the messages have no positions yet (open and save the diagram in EA), so fragments are not checked".into());
        }
        for f in objects.iter().filter(|o| o[2] == "InteractionFragment" && laid_out) {
            let n = |i: usize| f[i].parse::<i64>().unwrap_or(0);
            let (left, right, top, bottom) = (n(5), n(6), n(7), n(8));
            let inside = messages.iter().any(|m| {
                let y = m[6].parse::<i64>().unwrap_or(1);
                let (x1, x2) = (m[7].parse::<i64>().unwrap_or(0), m[8].parse::<i64>().unwrap_or(0));
                y <= top && y >= bottom && x1.max(x2) >= left && x1.min(x2) <= right
            });
            if !inside {
                findings.push(format!("{} fragment \"{}\" has no message inside", fragment_name(&f[4]), f[1]));
            }
        }
        // Time constraints of the messages (LC-421), and whether EA shows the numbers.
        let ids: Vec<String> = messages.iter().map(|m| m[0].clone()).collect();
        let constraints = if ids.is_empty() { Vec::new() } else {
            query_rows(cfg, &format!("select c.name, t.value from t_connectortag t join t_connector c on c.connector_id = t.elementid where t.property = 'time constraint' and t.elementid in ({})", ids.join(",")))?
        };
        let numbers_shown = query_rows(cfg, &format!("select coalesce(pdata,'') from t_diagram where diagram_id = {id}"))?.first().is_some_and(|r| r[0].contains("ShowSN=1"));
        if !numbers_shown && messages.len() > 1 {
            findings.push("sequence numbers are not shown (Diagram Properties → Show Sequence Numbers; EA numbers 1, 1.1, 1.2 by activation)".into());
        }
        out.push(format!("{} (diagram_id {}): {} message(s){}", d[1], d[0], messages.len(), if findings.is_empty() { ", nothing found".to_string() } else { String::new() }));
        if !constraints.is_empty() {
            out.push(format!("  time constraints: {}", constraints.iter().map(|c| format!("\"{}\" {{{}}}", c[0], c[1])).collect::<Vec<_>>().join(", ")));
        }
        out.extend(findings.into_iter().map(|f| format!("  - {f}")));
    }
    if !missing_all.is_empty() {
        out.push(format!("\nMissing operations ({}):", missing_all.len()));
        out.extend(missing_all.iter().map(|m| format!("  - {m}")));
    }
    Ok(out.join("\n"))
}

/// One parsed line of the text.
#[derive(Debug, PartialEq)]
enum Line {
    Participant { alias: String, name: String, actor: bool },
    Message { from: String, to: String, text: String, reply: bool, asynchronous: bool },
    Open { kind: String, guard: String },
    Else(String),
    End,
}

fn unquote(text: &str) -> String {
    text.trim().trim_matches('"').trim().to_string()
}

fn parse(text: &str) -> Result<Vec<Line>> {
    let mut out = Vec::new();
    for (n, raw) in text.lines().enumerate() {
        let line = raw.trim();
        let lower = line.to_lowercase();
        let first = lower.split_whitespace().next().unwrap_or("");
        if line.is_empty() || line.starts_with('\'') || line.starts_with("//") || line.starts_with('@') || matches!(first, "title" | "autonumber" | "activate" | "deactivate" | "skinparam" | "hide" | "note" | "==" | "...") {
            continue;
        }
        if matches!(first, "participant" | "actor" | "boundary" | "control" | "entity" | "database" | "collections" | "queue") {
            let rest = line[first.len()..].trim();
            let (name, alias) = match rest.rsplit_once(" as ") {
                Some((name, alias)) => (unquote(name), unquote(alias)),
                None => (unquote(rest), unquote(rest)),
            };
            out.push(Line::Participant { alias, name, actor: first == "actor" });
            continue;
        }
        if let Some((_, kind)) = FRAGMENTS.iter().map(|(k, _)| (k, *k)).find(|(k, _)| first == **k) {
            out.push(Line::Open { kind: kind.to_string(), guard: line[first.len()..].trim().to_string() });
            continue;
        }
        if first == "else" {
            out.push(Line::Else(line[4..].trim().to_string()));
            continue;
        }
        if first == "end" {
            out.push(Line::End);
            continue;
        }
        // A -> B : text (the arrow decides the kind).
        let (head, message) = line.split_once(':').map(|(h, m)| (h, m.trim())).unwrap_or((line, ""));
        let arrows = ["-->>", "->>", "-->", "->"];
        let Some(arrow) = arrows.iter().find(|a| head.contains(**a)) else { bail!("line {}: not understood: {line}", n + 1) };
        let (from, to) = head.split_once(arrow).unwrap_or(("", ""));
        let (from, to) = (unquote(from), unquote(to));
        if from.is_empty() || to.is_empty() {
            bail!("line {}: a message needs both ends: {line}", n + 1);
        }
        out.push(Line::Message { from, to, text: message.to_string(), reply: arrow.starts_with("--"), asynchronous: arrow.ends_with(">>") });
    }
    Ok(out)
}

/// The operations of the diagram and the report: (ops, report).
pub fn from_text(input: &Value, cfg: &AppConfig) -> Result<(Vec<Value>, String)> {
    let lines = parse(input["text"].as_str().unwrap_or(""))?;
    let (scope, _) = crate::model_usage::package_scope(cfg, &input["package"])?;
    let package: i64 = scope.split('(').nth(1).and_then(|s| s.split([',', ')']).next()).and_then(|p| p.parse().ok()).ok_or_else(|| anyhow::anyhow!("package: give the package"))?;
    let name = input["name"].as_str().map(str::trim).filter(|n| !n.is_empty()).unwrap_or("Sequence");
    // Participants in order of appearance (declared or used in a message).
    let mut participants: Vec<(String, String, bool)> = Vec::new();
    let mut add = |alias: &str, name: &str, actor: bool, list: &mut Vec<(String, String, bool)>| {
        if !list.iter().any(|(a, _, _)| a == alias) {
            list.push((alias.to_string(), name.to_string(), actor));
        }
    };
    for l in &lines {
        match l {
            Line::Participant { alias, name, actor } => add(alias, name, *actor, &mut participants),
            Line::Message { from, to, .. } => {
                add(from, from, false, &mut participants);
                add(to, to, false, &mut participants);
            }
            _ => {}
        }
    }
    let messages = lines.iter().filter(|l| matches!(l, Line::Message { .. })).count();
    if messages == 0 {
        bail!("no messages in the text (A -> B : message)");
    }
    let mut ops = vec![json!({ "op": "create_diagram", "key": "seq", "package": package, "name": name, "type": "Sequence" })];
    let mut report = Vec::new();
    // Each participant: an existing element (the package first), or a new lifeline.
    let mut refs: HashMap<String, (Value, Option<i64>)> = HashMap::new();
    let height = 120 + 40 * messages as i64;
    for (i, (alias, pname, actor)) in participants.iter().enumerate() {
        let quoted = crate::model_db::sql_text(cfg.model_source, &pname.to_lowercase());
        let found = query_rows(cfg, &format!(
            "select object_id, object_type, coalesce(classifier,0) from t_object where lower(name) = '{quoted}' and object_type not in ('Package', 'InteractionFragment', 'Note', 'Text') \
             order by case when package_id = {package} then 0 else 1 end, case when object_type in ('Class', 'Component', 'Interface', 'Actor') then 0 else 1 end, object_id"
        ))?;
        let (element, owner) = match found.first() {
            Some(r) => {
                let id: i64 = r[0].parse().unwrap_or(0);
                let classifier: i64 = r[2].parse().unwrap_or(0);
                report.push(format!("{pname}: existing {} (id {id}){}", r[1], if found.len() > 1 { format!(", {} elements have this name", found.len()) } else { String::new() }));
                (json!(id), Some(if classifier > 0 { classifier } else { id }))
            }
            None => {
                let key = format!("p{i}");
                ops.push(json!({ "op": "create_element", "key": key, "package": package, "name": pname, "type": if *actor { "Actor" } else { "Sequence" } }));
                report.push(format!("{pname}: new {}", if *actor { "actor" } else { "lifeline" }));
                (json!(format!("#{key}")), None)
            }
        };
        ops.push(json!({ "op": "place", "diagram": "#seq", "element": element, "left": 60 + i as i64 * 200, "top": 40, "width": 100, "height": height }));
        refs.insert(alias.clone(), (element, owner));
    }
    // Messages, bound to the receivers' operations by name.
    let create_missing = input["create_missing_operations"].as_bool().unwrap_or(false);
    let mut number = 0;
    let mut missing: Vec<String> = Vec::new();
    let mut fragments: Vec<(String, String, usize)> = Vec::new();
    let mut drawn: Vec<(String, String, usize, usize)> = Vec::new();
    for l in &lines {
        match l {
            Line::Message { from, to, text, reply, asynchronous } => {
                number += 1;
                let (source, _) = refs[from].clone();
                let (target, owner) = refs[to].clone();
                // A time constraint written after the message in braces ("place() {< 2s}") (LC-421): kept as the
                // message's tagged value "time constraint", not in its name (which binds it to the operation).
                let (text, constraint) = match text.rfind('{').filter(|_| text.trim_end().ends_with('}')) {
                    Some(at) => (text[..at].trim().to_string(), Some(text[at + 1..].trim().trim_end_matches('}').trim().to_string())),
                    None => (text.clone(), None),
                };
                let text = &text;
                if !reply {
                    if let (Some(owner), op) = (owner, called(text)) {
                        if !op.is_empty() {
                            let has = query_rows(cfg, &format!("select 1 from t_operation where object_id = {owner} and lower(name) = '{}'", crate::model_db::sql_text(cfg.model_source, &op)))?;
                            let short = text.split('(').next().unwrap_or("").trim().to_string();
                            if has.is_empty() && !missing.contains(&format!("{to}.{short}")) {
                                missing.push(format!("{to}.{short}"));
                                if create_missing {
                                    ops.push(json!({ "op": "create_operation", "element": owner, "name": short, "return_type": "void" }));
                                }
                            }
                        }
                    }
                }
                let mut message = json!({ "op": "create_connector", "source": source, "target": target, "type": "Sequence", "name": text, "diagram": "#seq",
                    "sequence": number, "message_sort": if *asynchronous { "Asynchronous" } else { "Synchronous" }, "is_return": reply });
                if let Some(c) = &constraint {
                    message["tags"] = json!({ "time constraint": c });
                }
                ops.push(message);
            }
            Line::Open { kind, guard } => fragments.push((kind.clone(), guard.clone(), number + 1)),
            Line::Else(guard) => {
                if let Some(last) = fragments.last_mut() {
                    last.1 = format!("{} / else {guard}", last.1);
                }
            }
            Line::End => {
                if let Some((kind, guard, first)) = fragments.pop() {
                    drawn.push((kind, guard, first, number));
                }
            }
            Line::Participant { .. } => {}
        }
    }
    // Fragments around their messages: EA lays the messages out by their number, the first at about y 135 and then
    // about 35 apart (EA 17, lifelines 40 from the top; measured on an exported image).
    for (i, (kind, guard, first, last)) in drawn.iter().enumerate() {
        if last < first {
            continue;
        }
        let ntype = FRAGMENTS.iter().find(|(k, _)| k == kind).map(|(_, n)| *n).unwrap_or(0);
        let key = format!("f{i}");
        let depth = drawn.iter().filter(|(_, _, f, l)| f <= first && l >= last).count() as i64 - 1;
        ops.push(json!({ "op": "create_element", "key": key, "package": package, "name": guard, "type": "InteractionFragment", "subtype": ntype }));
        ops.push(json!({ "op": "place", "diagram": "#seq", "element": format!("#{key}"), "left": 30 + depth * 8, "top": 115 + (*first as i64 - 1) * 35 + depth * 5,
            "width": 60 + participants.len() as i64 * 200 - depth * 16, "height": (*last as i64 - *first as i64 + 1) * 35 + 5 - depth * 10 }));
    }
    if !fragments.is_empty() {
        report.push(format!("{} fragment(s) without \"end\" were left out", fragments.len()));
    }
    // EA numbers the messages itself, hierarchically by activation (1, 1.1, 1.2…), when the diagram shows sequence
    // numbers (LC-421): on unless numbering is false.
    if input["numbering"] != false {
        ops.push(json!({ "op": "diagram_options", "diagram": "#seq", "extended": { "ShowSN": "1" } }));
    }
    if !missing.is_empty() {
        report.push(format!("{} the receivers' operation(s) {}: {}", missing.len(), if create_missing { "added" } else { "missing (create_missing_operations adds them)" }, missing.join(", ")));
    }
    Ok((ops, format!("Sequence \"{name}\": {} participant(s), {number} message(s), {} fragment(s)\n{}", participants.len(), drawn.len(), report.join("\n"))))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plantuml_lines() {
        let lines = parse("@startuml\ntitle X\nactor Klient\nparticipant \"Order service\" as OS\nKlient -> OS : place(order)\nalt ok\nOS --> Klient : ok\nelse error\nOS ->> Klient : error\nend\n@enduml").unwrap();
        assert_eq!(lines[0], Line::Participant { alias: "Klient".into(), name: "Klient".into(), actor: true });
        assert_eq!(lines[1], Line::Participant { alias: "OS".into(), name: "Order service".into(), actor: false });
        assert_eq!(lines[2], Line::Message { from: "Klient".into(), to: "OS".into(), text: "place(order)".into(), reply: false, asynchronous: false });
        assert_eq!(lines[3], Line::Open { kind: "alt".into(), guard: "ok".into() });
        assert!(matches!(&lines[4], Line::Message { reply: true, .. }));
        assert!(matches!(&lines[6], Line::Message { reply: false, asynchronous: true, .. }));
        assert_eq!(lines[7], Line::End);
        assert!(parse("what is this").is_err());
        assert_eq!(called("AddBook(string, string)"), "addbook");
    }

    /// `LOONAR_TEST_PROPS=… cargo test sequence_live -- --ignored --nocapture`: the check on the model's sequence diagrams,
    /// and a diagram from text created in EA and checked again.
    #[test]
    #[ignore = "requires EA and a copy of a test model"]
    fn sequence_live() {
        let cfg = AppConfig::load(std::path::Path::new(&std::env::var("LOONAR_TEST_PROPS").unwrap())).unwrap();
        let checked = check(&json!({ "diagram": 111 }), &cfg).unwrap();
        println!("{checked}");
        let q = |sql: &str| query_rows(&cfg, sql).unwrap();
        let root: i64 = q("select package_id from t_package where parent_id = 0 order by package_id")[0][0].parse().unwrap();
        let created = crate::ea::run_batch(&cfg, &[json!({"op":"create_package","key":"pkg","parent":root,"name":format!("ZZ_seq_{}", chrono::Local::now().format("%H%M%S"))})], &crate::ea::BatchOptions::default()).unwrap();
        let pkg = created["created"][0]["id"].as_i64().unwrap();
        let text = "actor Klient\nKlient -> Book : Book(string) {< 2s}\nloop each item\nBook -> BookDB : AddBook(x)\nBook -> BookDB : Audit()\nend\nBookDB --> Book : done\nBook --> Klient : ok";
        let (ops, report) = from_text(&json!({ "text": text, "package": pkg, "name": "From text" }), &cfg).unwrap();
        println!("{report}");
        let done = crate::ea::run_batch(&cfg, &ops, &crate::ea::BatchOptions::default()).expect("create");
        let diagram = done["created"].as_array().unwrap().iter().find(|c| c["key"] == "seq").unwrap()["id"].as_i64().unwrap();
        let again = check(&json!({ "diagram": diagram }), &cfg).unwrap();
        println!("{again}");
        println!("{:?}", q(&format!("select name, seqno, ptstarty, pdata4 from t_connector where diagramid = {diagram} order by seqno")));
        println!("{:?}", q(&format!("select o.name, x.recttop, x.rectbottom from t_diagramobjects x join t_object o on o.object_id = x.object_id where x.diagram_id = {diagram}")));
        assert!(report.contains("Audit"), "{report}");
        assert!(again.contains("5 message(s)") && again.contains("BookDB.Audit()"), "{again}");
        assert!(again.contains("time constraints: \"Book(string)\" {< 2s}") && !again.contains("sequence numbers are not shown"), "{again}");
        let image = std::env::temp_dir().join("loonar-seq-test.png");
        crate::ea::run_batch(&cfg, &[json!({"op":"export_diagram","diagram":diagram,"path":image.to_string_lossy()})], &crate::ea::BatchOptions::default()).unwrap();
        println!("image: {}", image.display());
        crate::ea::run_batch(&cfg, &[json!({"op":"delete","package":pkg})], &crate::ea::BatchOptions::default()).unwrap();
        crate::ea::shutdown();
    }
}

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
autonumber, activate, boxes, dividers, delays and comments are skipped; notes (`note right of A : text`, `note over A, B` … `end note`) \
are drawn right of the lifelines and linked to them, and an arrow's colour (`-[#red]>`) colours the message; `return x` answers the last open call; group … end is a \
frame; arrows to the left (<-), with colours or styles count too. Missing participants become lifelines (with create_classes: classes and \
lifelines of them); with create_missing_operations the receivers \
get the operations the messages call. Show the user the report (bound / created / missing) after the save.",
            "input_schema": { "type": "object", "properties": {
                "text": { "type": "string" },
                "package": { "description": "package_id or exact name: where the diagram (and new lifelines) go" },
                "name": { "type": "string", "description": "Name of the new diagram" },
                "create_missing_operations": { "type": "boolean", "description": "Add operations the messages call to receivers that lack them (default false)" },
                "create_classes": { "type": "boolean", "description": "Participants not in the model become classes (actors Actors) with lifelines that are instances of them, and the classes get the operations the messages call, with their parameters (default false; use it for an imported PlantUML sequence)" },
                "numbering": { "type": "boolean", "description": "Show sequence numbers (EA numbers hierarchically by activation; default true)" }
            }, "required": ["text", "package", "name"] }
        }),
    ]
}

/// The operation a message calls: its name without the parameters ("AddBook(string, …)" → "addbook").
fn called(message: &str) -> String {
    message.split('(').next().unwrap_or("").trim().trim_end_matches(';').to_lowercase()
}

/// Guards and heights of the operands of a fragment: the first operand from the fragment's guard to the first else,
/// each further one from its else; EA lays messages out about 35 apart, and the first operand starts under the
/// fragment's label (about 33 high).
fn operand_sizes(guard: &str, first: usize, last: usize, elses: &[(String, usize)]) -> Vec<(String, i64)> {
    let mut starts: Vec<(String, usize)> = vec![(guard.to_string(), first)];
    starts.extend(elses.iter().cloned());
    let mut out = Vec::new();
    for (n, (g, start)) in starts.iter().enumerate() {
        let end = starts.get(n + 1).map(|(_, next)| *next).unwrap_or(last + 1);
        let messages = end.saturating_sub(*start).max(1) as i64;
        let size = if n == 0 { (messages * 35 - 13).max(22) } else { messages * 35 };
        out.push((g.clone(), size));
    }
    out
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
            let mut owner = if o[3] != "0" && !o[3].is_empty() { o[3].clone() } else if matches!(o[2].as_str(), "Class" | "Interface" | "Component" | "Actor") { o[0].clone() } else { String::new() };
            // A lifeline without a classifier: the class of the same name, as /sequence-from-text binds it (LC-471: the
            // two tools judged the same lifelines differently).
            if owner.is_empty() && !o[1].trim().is_empty() {
                owner = query_rows(cfg, &format!(
                    "select object_id from t_object where lower(name) = '{}' and object_type in ('Class', 'Interface', 'Component') order by object_id",
                    crate::model_db::sql_text(cfg.model_source, &o[1].trim().to_lowercase())
                ))?
                .first()
                .map(|r| r[0].clone())
                .unwrap_or_default();
            }
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
        // EA's API have no position until the diagram is saved in EA: then their places are the ones EA gives them by
        // their order – the first at about y 135, then about 35 apart, between the middles of their lifelines (LC-445;
        // the same measure /sequence-from-text places its fragments by).
        let laid_out = messages.iter().any(|m| m[6].parse::<i64>().unwrap_or(0) != 0);
        let middle = |id: &str| objects.iter().find(|o| o[0] == id).map_or(0, |o| (o[5].parse::<i64>().unwrap_or(0) + o[6].parse::<i64>().unwrap_or(0)) / 2);
        let places: Vec<(i64, i64, i64)> = messages
            .iter()
            .enumerate()
            .map(|(rank, m)| {
                if laid_out {
                    (m[6].parse::<i64>().unwrap_or(1), m[7].parse::<i64>().unwrap_or(0), m[8].parse::<i64>().unwrap_or(0))
                } else {
                    (-(135 + rank as i64 * 35), middle(&m[3]), middle(&m[4]))
                }
            })
            .collect();
        let fragments: Vec<&Vec<String>> = objects.iter().filter(|o| o[2] == "InteractionFragment").collect();
        if !laid_out && !fragments.is_empty() && !messages.is_empty() {
            findings.push("the messages have no places on the diagram yet (EA sets them when the diagram is opened and saved in EA): their order is taken from their numbers".into());
        }
        for f in fragments {
            let n = |i: usize| f[i].parse::<i64>().unwrap_or(0);
            let (left, right, top, bottom) = (n(5), n(6), n(7), n(8));
            let inside = places.iter().any(|(y, x1, x2)| *y <= top && *y >= bottom && *x1.max(x2) >= left && *x1.min(x2) <= right);
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
    Message { from: String, to: String, text: String, reply: bool, asynchronous: bool, color: Option<String> },
    /// A note on lifelines (LC-473): the participants it is over or beside, its text.
    Note { on: Vec<String>, text: String },
    Open { kind: String, guard: String },
    Else(String),
    End,
}

fn unquote(text: &str) -> String {
    text.trim().trim_matches('"').trim().to_string()
}

fn parse(text: &str) -> Result<Vec<Line>> {
    let mut out = Vec::new();
    let mut skip: Option<String> = None;
    // Calls not yet answered, for "return": (caller, callee).
    let mut open_calls: Vec<(String, String)> = Vec::new();
    // A note over more lines: its participants and lines so far.
    let mut open_note: Option<(Vec<String>, Vec<String>)> = None;
    for (n, raw) in text.lines().enumerate() {
        let line = raw.trim();
        let lower = line.to_lowercase();
        let first = lower.split_whitespace().next().unwrap_or("");
        if let Some((on, body)) = open_note.as_mut() {
            if ["end note", "endnote", "end hnote", "endhnote", "end rnote", "endrnote"].iter().any(|e| lower.starts_with(e)) {
                out.push(Line::Note { on: std::mem::take(on), text: body.join("\n") });
                open_note = None;
            } else {
                body.push(line.to_string());
            }
            continue;
        }
        // Blocks that are not messages (notes, legends, boxes' ends, references) are skipped to their end (LC-472).
        if let Some(end) = skip.as_ref() {
            if lower.starts_with(end.as_str()) {
                skip = None;
            }
            continue;
        }
        // Notes (LC-473: they were left out): "note left of A : text", "note over A, B" … "end note".
        if matches!(first, "note" | "hnote" | "rnote") {
            // A note on a message ("note on link") is left out and listed (round 11a: it became a loose note).
            if lower[first.len()..].trim_start().starts_with("on link") {
                if !line.contains(':') {
                    skip = Some("end note".into());
                }
                continue;
            }
            let (head, text) = line.split_once(':').map(|(h, t)| (h.trim(), Some(t.trim().replace("\\n", "\n")))).unwrap_or((line, None));
            let rest = head[first.len()..].trim();
            let rest_lower = rest.to_lowercase();
            let names = match (rest_lower.find(" of "), rest_lower.strip_prefix("over ")) {
                (Some(at), _) => &rest[at + 4..],
                (None, Some(_)) => &rest[5..],
                _ => "",
            };
            // Colours after the names: note over A #pink.
            let on: Vec<String> = names.split(" #").next().unwrap_or("").split(',').map(unquote).filter(|n| !n.is_empty()).collect();
            match text {
                Some(text) => out.push(Line::Note { on, text }),
                None => open_note = Some((on, Vec::new())),
            }
            continue;
        }
        if first == "ref" && !line.contains(':') {
            skip = Some("end ref".into());
            continue;
        }
        if first == "legend" {
            skip = Some("endlegend".into());
            continue;
        }
        if line.is_empty() || line.starts_with('\'') || line.starts_with("//") || line.starts_with('@') || line.starts_with("==") || line.starts_with("...") || line.starts_with("|||") || line.starts_with("||")
            || matches!(first, "title" | "autonumber" | "activate" | "deactivate" | "skinparam" | "hide" | "show" | "note" | "hnote" | "rnote" | "ref" | "box" | "create" | "destroy" | "header" | "footer" | "newpage" | "caption" | "scale" | "!pragma" | "!theme" | "delay" | "space")
            || lower.starts_with("end box") || lower.starts_with("end note") || lower.starts_with("end ref") || lower.starts_with("left to right")
        {
            continue;
        }
        // "return x": the reply to the caller of the last call still open.
        if first == "return" {
            if let Some((caller, callee)) = open_calls.pop() {
                out.push(Line::Message { from: callee, to: caller, text: line[6..].trim().to_string(), reply: true, asynchronous: false, color: None });
            }
            continue;
        }
        if matches!(first, "participant" | "actor" | "boundary" | "control" | "entity" | "database" | "collections" | "queue") {
            // Colours and order after the name: participant A as B #red order 10.
            let mut rest = line[first.len()..].trim().to_string();
            for cut in [" #", " order ", " <<"] {
                if let Some(at) = rest.find(cut) {
                    rest.truncate(at);
                }
            }
            let rest = rest.trim();
            let (name, alias) = match rest.rsplit_once(" as ") {
                Some((name, alias)) => (unquote(name), unquote(alias)),
                None => (unquote(rest), unquote(rest)),
            };
            out.push(Line::Participant { alias, name, actor: first == "actor" });
            continue;
        }
        // group is a fragment of its own kind in PlantUML; in EA the nearest is a critical-free "seq" frame.
        if first == "group" {
            out.push(Line::Open { kind: "seq".into(), guard: line[5..].trim().to_string() });
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
        // Colours and styles of arrows ([#red], ->x, -\\, o->) do not change the message; the colour is the line's (LC-473).
        let color = head.find("[#").and_then(|a| crate::mermaid_ddl::css_color(head[a + 2..].split([']', ',', ';']).next().unwrap_or("")));
        let mut head = head.to_string();
        while let (Some(a), Some(b)) = (head.find("[#"), head.find(']')) {
            if b < a {
                break;
            }
            head.replace_range(a..=b, "");
        }
        let head = head.replace("->x", "->").replace("-\\\\", "->").replace("-//", "->>").replace("o->", "->").replace("->o", "->").replace("<->", "->");
        let arrows = ["-->>", "->>", "-->", "->", "<<--", "<<-", "<--", "<-"];
        let Some(arrow) = arrows.iter().find(|a| head.contains(**a)) else { bail!("line {}: not understood: {line}", n + 1) };
        let (left, right) = head.split_once(arrow).unwrap_or(("", ""));
        let (left, right) = (unquote(left.trim_end_matches(['+', '-', '*', '!'])), unquote(right.trim_start_matches(['+', '-', '*', '!']).trim_end_matches(['+', '-', '*', '!'])));
        if left.is_empty() || right.is_empty() {
            bail!("line {}: a message needs both ends: {line}", n + 1);
        }
        // An arrow pointing left goes from the right end to the left one.
        let (from, to) = if arrow.starts_with('<') { (right, left) } else { (left, right) };
        let reply = arrow.contains("--");
        if reply {
            if let Some(at) = open_calls.iter().rposition(|(caller, callee)| *caller == to && *callee == from) {
                open_calls.remove(at);
            }
        } else if from != to {
            open_calls.push((from.clone(), to.clone()));
        }
        out.push(Line::Message { from, to, text: message.to_string(), reply, asynchronous: arrow.ends_with(">>") || arrow.starts_with("<<"), color });
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
    let mut refs: HashMap<String, (Value, Option<Value>)> = HashMap::new();
    // LC-472: a sequence from PlantUML builds the structure too – each new participant a class (an actor an Actor) and its
    // lifeline an instance of it, the operations the messages call in the classes.
    let create_classes = input["create_classes"].as_bool().unwrap_or(false);
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
                if create_classes && matches!(r[1].as_str(), "Class" | "Component" | "Interface") {
                    // The class itself does not stand on a sequence diagram: a lifeline of it does (LC-472, round 9).
                    let lifeline_key = format!("p{i}");
                    ops.push(json!({ "op": "create_element", "key": lifeline_key, "package": package, "name": pname, "type": "Sequence", "classifier": id }));
                    (json!(format!("#{lifeline_key}")), Some(json!(id)))
                } else {
                    (json!(id), Some(json!(if classifier > 0 { classifier } else { id })))
                }
            }
            None if create_classes => {
                let (class_key, lifeline_key) = (format!("c{i}"), format!("p{i}"));
                ops.push(json!({ "op": "create_element", "key": class_key, "package": package, "name": pname, "type": if *actor { "Actor" } else { "Class" } }));
                if *actor {
                    report.push(format!("{pname}: new actor"));
                    (json!(format!("#{class_key}")), Some(json!(format!("#{class_key}"))))
                } else {
                    ops.push(json!({ "op": "create_element", "key": lifeline_key, "package": package, "name": pname, "type": "Sequence", "classifier": format!("#{class_key}") }));
                    report.push(format!("{pname}: new class with a lifeline of it"));
                    (json!(format!("#{lifeline_key}")), Some(json!(format!("#{class_key}"))))
                }
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
    let create_missing = input["create_missing_operations"].as_bool().unwrap_or(false) || create_classes;
    let mut number = 0;
    let mut missing: Vec<String> = Vec::new();
    // Open fragments: (kind, guard, first message, further operands as (guard, first message)).
    let mut fragments: Vec<(String, String, usize, Vec<(String, usize)>)> = Vec::new();
    let mut drawn: Vec<(String, String, usize, usize, Vec<(String, usize)>)> = Vec::new();
    // Notes: (participants, text, the number of the message before it).
    let mut notes: Vec<(Vec<String>, String, usize)> = Vec::new();
    for l in &lines {
        match l {
            Line::Message { from, to, text, reply, asynchronous, color } => {
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
                            // A class made in this save has no operations yet.
                            let has = match owner.as_i64() {
                                Some(id) => !query_rows(cfg, &format!("select 1 from t_operation where object_id = {id} and lower(name) = '{}'", crate::model_db::sql_text(cfg.model_source, &op)))?.is_empty(),
                                None => false,
                            };
                            let short = text.split('(').next().unwrap_or("").trim().to_string();
                            if !has && !missing.contains(&format!("{to}.{short}")) {
                                missing.push(format!("{to}.{short}"));
                                if create_missing {
                                    // The parameters written in the message: "pay(amount, currency)" or "pay(amount: Money)".
                                    let inside = text.split_once('(').map(|(_, r)| r.rsplit_once(')').map(|(p, _)| p).unwrap_or(r)).unwrap_or("");
                                    let parameters: Vec<Value> = inside.split(',').map(str::trim).filter(|p| !p.is_empty()).map(|p| {
                                        let (n, t) = p.split_once(':').map(|(a, b)| (a.trim(), b.trim())).unwrap_or((p, ""));
                                        json!({ "name": n, "type": t })
                                    }).collect();
                                    let returns = text.rsplit_once(')').map(|(_, r)| r.trim().trim_start_matches(':').trim()).filter(|r| !r.is_empty()).unwrap_or("void");
                                    ops.push(json!({ "op": "create_operation", "element": owner, "name": short, "return_type": returns, "parameters": parameters }));
                                }
                            }
                        }
                    }
                }
                let mut message = json!({ "op": "create_connector", "source": source, "target": target, "type": "Sequence", "name": text, "diagram": "#seq",
                    "sequence": number, "message_sort": if *asynchronous { "Asynchronous" } else { "Synchronous" }, "is_return": reply });
                if let Some(c) = &constraint {
                    message["tags"] = json!({ "time constraint": c });
                    message["constraints"] = json!([{ "name": format!("{{{c}}}"), "type": "Invariant" }]);
                }
                if let Some(c) = color {
                    message["color"] = json!(c);
                }
                ops.push(message);
            }
            Line::Note { on, text } => notes.push((on.clone(), text.clone(), number)),
            Line::Open { kind, guard } => fragments.push((kind.clone(), guard.clone(), number + 1, Vec::new())),
            // Each else is an operand of its own (LC-498: it was added to the fragment's name, drawn as one area).
            Line::Else(guard) => {
                if let Some(last) = fragments.last_mut() {
                    last.3.push((guard.clone(), number + 1));
                }
            }
            Line::End => {
                if let Some((kind, guard, first, elses)) = fragments.pop() {
                    drawn.push((kind, guard, first, number, elses));
                }
            }
            Line::Participant { .. } => {}
        }
    }
    // Fragments around their messages: EA lays the messages out by their number, the first at about y 135 and then
    // about 35 apart (EA 17, lifelines 40 from the top; measured on an exported image).
    for (i, (kind, guard, first, last, elses)) in drawn.iter().enumerate() {
        // An empty fragment is drawn too, small, where it is in the text (LC-456: it was left out while the answer said
        // it had been made); the report says it is empty, /check-sequence finds it.
        let empty = last < first;
        if empty {
            report.push(format!("{kind} fragment \"{guard}\" has no message inside (drawn empty)"));
        }
        let last = if empty { first } else { last };
        let ntype = FRAGMENTS.iter().find(|(k, _)| k == kind).map(|(_, n)| *n).unwrap_or(0);
        let key = format!("f{i}");
        let depth = drawn.iter().filter(|(_, _, f, l, _)| f <= first && l >= last).count() as i64 - 1;
        let mut fragment = json!({ "op": "create_element", "key": key, "package": package, "name": guard, "type": "InteractionFragment", "subtype": ntype });
        if !elses.is_empty() {
            // Operands with their guards; the name stays empty, EA would show it next to the operator as well.
            fragment["name"] = json!("");
            fragment["operands"] = json!(operand_sizes(guard, *first, *last, elses).into_iter().map(|(g, size)| json!({ "guard": g, "size": size })).collect::<Vec<_>>());
        }
        ops.push(fragment);
        ops.push(json!({ "op": "place", "diagram": "#seq", "element": format!("#{key}"), "left": 30 + depth * 8, "top": 115 + (*first as i64 - 1) * 35 + depth * 5,
            "width": 60 + participants.len() as i64 * 200 - depth * 16, "height": if empty { 30 } else { (*last as i64 - *first as i64 + 1) * 35 + 5 - depth * 10 } }));
    }
    if !fragments.is_empty() {
        report.push(format!("{} fragment(s) without \"end\" were left out", fragments.len()));
    }
    // Notes right of the lifelines, at the height of the message before them, linked to their participants: drawn
    // between the lifelines they would cover messages (LC-473).
    // The bottom of the note before: a note starts under it (round 11a: notes of near messages overlapped).
    let mut below = 0i64;
    for (n, (on, text, after)) in notes.iter().enumerate() {
        let key = format!("n{n}");
        ops.push(json!({ "op": "create_element", "key": key, "package": package, "name": "", "type": "Note", "notes": text }));
        let (top, height) = ((115 + *after as i64 * 35 - 15).max(below + 10), 30 + 15 * text.lines().count() as i64);
        below = top + height;
        ops.push(json!({ "op": "place", "diagram": "#seq", "element": format!("#{key}"), "left": 60 + participants.len() as i64 * 200, "top": top,
            "width": 180, "height": height }));
        for name in on {
            if let Some((element, _)) = refs.get(name) {
                ops.push(json!({ "op": "create_connector", "source": format!("#{key}"), "target": element, "type": "NoteLink" }));
            }
        }
    }
    if !notes.is_empty() {
        report.push(format!("{} note(s), right of the lifelines", notes.len()));
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

    /// LC-498: alt with else becomes two operands; each spans its own messages.
    #[test]
    fn else_is_an_operand() {
        let sizes = operand_sizes("ok", 2, 4, &[("error".into(), 3)]);
        assert_eq!(sizes, vec![("ok".to_string(), 22), ("error".to_string(), 70)]);
        let sizes = operand_sizes("a", 1, 5, &[("b".into(), 4), ("c".into(), 5)]);
        assert_eq!(sizes.iter().map(|(g, s)| (g.as_str(), *s)).collect::<Vec<_>>(), vec![("a", 92), ("b", 35), ("c", 35)]);
    }

    #[test]
    fn plantuml_lines() {
        let lines = parse("@startuml\ntitle X\nactor Klient\nparticipant \"Order service\" as OS\nKlient -> OS : place(order)\nalt ok\nOS --> Klient : ok\nelse error\nOS ->> Klient : error\nend\n@enduml").unwrap();
        assert_eq!(lines[0], Line::Participant { alias: "Klient".into(), name: "Klient".into(), actor: true });
        assert_eq!(lines[1], Line::Participant { alias: "OS".into(), name: "Order service".into(), actor: false });
        assert_eq!(lines[2], Line::Message { from: "Klient".into(), to: "OS".into(), text: "place(order)".into(), reply: false, asynchronous: false, color: None });
        assert_eq!(lines[3], Line::Open { kind: "alt".into(), guard: "ok".into() });
        assert!(matches!(&lines[4], Line::Message { reply: true, .. }));
        assert!(matches!(&lines[6], Line::Message { reply: false, asynchronous: true, .. }));
        assert_eq!(lines[7], Line::End);
        assert!(parse("what is this").is_err());
        // LC-473: notes and the colour of an arrow.
        let lines = parse("A -[#red]> B : x\nnote over A, B : razem\nnote right of B\n  dwie\n  linie\nend note").unwrap();
        assert!(matches!(&lines[0], Line::Message { color: Some(c), .. } if c == "#FF0000"), "{lines:?}");
        assert_eq!(lines[1], Line::Note { on: vec!["A".into(), "B".into()], text: "razem".into() });
        assert_eq!(lines[2], Line::Note { on: vec!["B".into()], text: "dwie\nlinie".into() });
        // A note on a message is left out, also over more lines.
        let lines = parse("A -> B : x\nnote on link : y\nnote on link\n  z\nend note\nB -> A : w").unwrap();
        assert!(!lines.iter().any(|l| matches!(l, Line::Note { .. })) && lines.len() == 2, "{lines:?}");
        assert_eq!(called("AddBook(string, string)"), "addbook");
    }

    /// `LOONAR_TEST_PROPS=… cargo test sequence_live -- --ignored --nocapture`: the check on the model's sequence diagrams,
    /// and a diagram from text created in EA and checked again.
    #[test]
    #[ignore = "requires EA and a copy of a test model"]
    fn sequence_live() {
        let cfg = AppConfig::load(std::path::Path::new(&std::env::var("LOONAR_TEST_PROPS").unwrap())).unwrap();
        let q = |sql: &str| query_rows(&cfg, sql).unwrap();
        let root: i64 = q("select package_id from t_package where parent_id = 0 order by package_id")[0][0].parse().unwrap();
        let created = crate::ea::run_batch(&cfg, &[json!({"op":"create_package","key":"pkg","parent":root,"name":format!("ZZ_seq_{}", chrono::Local::now().format("%H%M%S"))})], &crate::ea::BatchOptions::default()).unwrap();
        let pkg = created["created"][0]["id"].as_i64().unwrap();
        let text = "actor Klient\nKlient -> Book : Book(string) {< 2s}\nloop each item\nBook -> BookDB : AddBook(x)\nBook -> BookDB : Audit()\nend\nBookDB --> Book : done\nBook --> Klient : ok
opt empty
end";
        let (ops, report) = from_text(&json!({ "text": text, "package": pkg, "name": "From text" }), &cfg).unwrap();
        println!("{report}");
        let done = crate::ea::run_batch(&cfg, &ops, &crate::ea::BatchOptions::default()).expect("create");
        let diagram = done["created"].as_array().unwrap().iter().find(|c| c["key"] == "seq").unwrap()["id"].as_i64().unwrap();
        let again = check(&json!({ "diagram": diagram }), &cfg).unwrap();
        println!("{again}");
        println!("{:?}", q(&format!("select name, seqno, ptstarty, pdata4 from t_connector where diagramid = {diagram} order by seqno")));
        println!("{:?}", q(&format!("select o.name, x.recttop, x.rectbottom from t_diagramobjects x join t_object o on o.object_id = x.object_id where x.diagram_id = {diagram}")));
        assert!(report.contains("Audit"), "{report}");
        // LC-456: the empty fragment is drawn and found; LC-421: the time constraint is a constraint of the message.
        assert!(again.contains("\"empty\" has no message inside"), "{again}");
        let constraints = q(&format!("select c.\"constraint\", c.constrainttype from t_connectorconstraint c join t_connector x on x.connector_id = c.connectorid where x.diagramid = {diagram}"));
        println!("{constraints:?}");
        assert_eq!(constraints, vec![vec!["{< 2s}".to_string(), "Invariant".to_string()]]);
        assert!(again.contains("5 message(s)") && again.contains("BookDB.Audit()"), "{again}");
        assert!(again.contains("time constraints: \"Book(string)\" {< 2s}") && !again.contains("sequence numbers are not shown"), "{again}");
        let image = std::env::temp_dir().join("loonar-seq-test.png");
        crate::ea::run_batch(&cfg, &[json!({"op":"export_diagram","diagram":diagram,"path":image.to_string_lossy()})], &crate::ea::BatchOptions::default()).unwrap();
        println!("image: {}", image.display());
        crate::ea::run_batch(&cfg, &[json!({"op":"delete","package":pkg})], &crate::ea::BatchOptions::default()).unwrap();
        crate::ea::shutdown();
    }

    /// LC-445: an empty fragment is found on a diagram not saved in EA, on a model copy:
    /// `LOONAR_TEST_PROPS=E:/livetest/r440/m.properties LOONAR_TEST_DIAGRAM=1171 cargo test empty_fragment_live -- --ignored --nocapture`
    #[test]
    #[ignore = "requires the model from LOONAR_TEST_PROPS"]
    fn empty_fragment_live() {
        let cfg = crate::config::AppConfig::load(std::path::Path::new(&std::env::var("LOONAR_TEST_PROPS").expect("LOONAR_TEST_PROPS"))).expect("app.properties");
        let diagram: i64 = std::env::var("LOONAR_TEST_DIAGRAM").expect("LOONAR_TEST_DIAGRAM").parse().unwrap();
        let report = super::check(&serde_json::json!({ "diagram": diagram }), &cfg).expect("check");
        println!("{report}");
        assert!(report.contains("has no message inside"), "{report}");
    }
}

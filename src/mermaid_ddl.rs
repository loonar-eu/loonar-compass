//! Two notation bridges (LC-408):
//! - import_mermaid: a Mermaid classDiagram as classes (attributes, operations) and relationships (inheritance,
//!   realisation, composition, aggregation, association, dependency) in a new package with a diagram – one save;
//!   import_plantuml (LC-472) does the same for a PlantUML class diagram, read through the same model;
//! - table_ddl: CREATE TABLE statements of the Table elements of a package for PostgreSQL, SQL Server, MySQL, Oracle or
//!   SQLite – columns with types mapped to the dialect, NOT NULL, primary keys (EA's PK operations). Reads only.

use crate::config::AppConfig;
use crate::tools::query_rows;
use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use std::collections::BTreeMap;

pub fn definitions() -> Vec<Value> {
    vec![
        json!({
            "name": "import_mermaid",
            "description": "A Mermaid classDiagram (text, or an attached .mmd/.md) as a new package with classes (attributes with types and \
visibility, operations), interfaces and enumerations (<<interface>>, <<enumeration>>), relationships (<|-- inheritance, ..|> realisation, \
*-- composition, o-- aggregation, --> association, ..> dependency, with labels) and a class diagram laid out by EA. action preview shows what \
would be created; apply saves it (one save the user approves).",
            "input_schema": { "type": "object", "properties": {
                "text": { "type": "string" }, "file": { "type": "string" },
                "package": { "description": "package_id or exact name of the parent" }, "name": { "type": "string" },
                "action": { "type": "string", "enum": ["preview", "apply"] }
            }, "required": ["package"] }
        }),
        json!({
            "name": "import_plantuml",
            "description": "A PlantUML class diagram (text between @startuml and @enduml, or an attached .puml/.plantuml/.txt) as a new package \
with classes (attributes with types and visibility, operations), abstract classes, interfaces and enums, relationships (<|-- inheritance, \
..|> realisation, *-- composition, o-- aggregation, --> association, ..> dependency; arrows with directions or colours such as -up-> or \
-[#red]-> too; multiplicities in quotes and labels after a colon) and a class diagram laid out by EA. Aliases (class \"Long name\" as L, into EA's \
Alias), {abstract} operations, the navigability of --> and stereotypes <<…>> are kept; notes (linked to their classes), class fills \
(class A #lightblue) and line colours (-[#red]->) are drawn; packages, notes on relationships and skin parameters are left out (the preview says so). action preview shows what would \
be created; apply saves it (one save the user approves). A PlantUML sequence diagram is recognised and imported as one: its participants become classes (actors Actors) with lifelines that are instances of them, the messages operations of those classes (with their parameters) and messages of a new sequence diagram; existing elements of the same name are used instead.",
            "input_schema": { "type": "object", "properties": {
                "text": { "type": "string" }, "file": { "type": "string" },
                "package": { "description": "package_id or exact name of the parent" }, "name": { "type": "string" },
                "action": { "type": "string", "enum": ["preview", "apply"] }
            }, "required": ["package"] }
        }),
        json!({
            "name": "table_ddl",
            "description": "CREATE TABLE statements of the Table elements of a package (with child packages) for postgres, sqlserver, mysql, \
oracle or sqlite: columns with their types mapped to the dialect, lengths, NOT NULL, primary keys (EA's PK operations). For databases EA \
does not generate, or to compare. Reads only; give the SQL to the user in a code block.",
            "input_schema": { "type": "object", "properties": {
                "package": { "description": "package_id or exact name" },
                "dialect": { "type": "string", "enum": ["postgres", "sqlserver", "mysql", "oracle", "sqlite"] }
            }, "required": ["package", "dialect"] }
        }),
    ]
}

// ---------- Mermaid ----------

#[derive(Debug, Default, Clone, PartialEq)]
pub(crate) struct MClass {
    pub name: String,
    pub kind: String,
    /// A stereotype other than interface or enumeration (PlantUML <<entity>> and the like).
    pub stereotype: String,
    pub abstract_: bool,
    /// The short name of a PlantUML `"Long name" as L` (LC-473): EA's Alias.
    pub alias: String,
    pub attributes: Vec<(String, String, String)>, // (visibility, name, type)
    pub operations: Vec<(String, String, String)>, // (visibility, name(params), return)
    /// The signatures of the abstract operations (Mermaid `op()*`, PlantUML `{abstract}`; LC-473).
    pub abstract_operations: Vec<String>,
}

/// What a PlantUML class diagram draws besides the model (LC-473): notes (text, the classes they are linked to),
/// class fills and line colours of relationships (from, to, colour), the colours as #RRGGBB.
#[derive(Debug, Default, Clone, PartialEq)]
pub(crate) struct Drawing {
    pub notes: Vec<(String, Vec<String>)>,
    pub fills: BTreeMap<String, String>,
    pub line_colors: Vec<(String, String, String)>,
}

/// A PlantUML colour (#lightblue, #ADD8E6, #ABC, lightblue) as #RRGGBB; None when it is not one.
pub(crate) fn css_color(text: &str) -> Option<String> {
    let t = text.trim().trim_start_matches('#').to_lowercase();
    let hex = |h: &str| h.chars().all(|c| c.is_ascii_hexdigit());
    if t.len() == 6 && hex(&t) {
        return Some(format!("#{}", t.to_uppercase()));
    }
    if t.len() == 3 && hex(&t) {
        return Some(format!("#{}", t.chars().flat_map(|c| [c, c]).collect::<String>().to_uppercase()));
    }
    const NAMES: &[(&str, &str)] = &[
        ("red", "FF0000"), ("green", "008000"), ("blue", "0000FF"), ("yellow", "FFFF00"), ("orange", "FFA500"), ("pink", "FFC0CB"),
        ("purple", "800080"), ("violet", "EE82EE"), ("magenta", "FF00FF"), ("cyan", "00FFFF"), ("aqua", "00FFFF"), ("brown", "A52A2A"),
        ("black", "000000"), ("white", "FFFFFF"), ("gray", "808080"), ("grey", "808080"), ("silver", "C0C0C0"), ("gold", "FFD700"),
        ("navy", "000080"), ("teal", "008080"), ("olive", "808000"), ("maroon", "800000"), ("lime", "00FF00"), ("beige", "F5F5DC"),
        ("lightblue", "ADD8E6"), ("lightgreen", "90EE90"), ("lightgray", "D3D3D3"), ("lightgrey", "D3D3D3"), ("lightyellow", "FFFFE0"),
        ("lightpink", "FFB6C1"), ("lightcyan", "E0FFFF"), ("lightcoral", "F08080"), ("lightsalmon", "FFA07A"), ("lightskyblue", "87CEFA"),
        ("skyblue", "87CEEB"), ("darkblue", "00008B"), ("darkgreen", "006400"), ("darkred", "8B0000"), ("darkorange", "FF8C00"),
        ("darkgray", "A9A9A9"), ("darkgrey", "A9A9A9"), ("palegreen", "98FB98"), ("wheat", "F5DEB3"), ("khaki", "F0E68C"),
        ("salmon", "FA8072"), ("coral", "FF7F50"), ("tomato", "FF6347"), ("orchid", "DA70D6"), ("plum", "DDA0DD"), ("lavender", "E6E6FA"),
        ("ivory", "FFFFF0"), ("azure", "F0FFFF"), ("honeydew", "F0FFF0"), ("mistyrose", "FFE4E1"), ("aliceblue", "F0F8FF"), ("steelblue", "4682B4"),
    ];
    NAMES.iter().find(|(n, _)| *n == t).map(|(_, h)| format!("#{h}"))
}

/// The colour in a PlantUML arrow or declaration: "-[#red]->", "-[bold,#00AA00]->", "#back:pink;line:red" (the fill).
fn color_in(text: &str) -> Option<String> {
    let at = text.find('#')?;
    let token: String = text[at + 1..].chars().take_while(|c| c.is_ascii_alphanumeric() || *c == ':').collect();
    css_color(token.strip_prefix("back:").unwrap_or(&token))
}

/// (from, to, type, label, multiplicity at from, multiplicity at to); for a composition or aggregation from = the part,
/// to = the whole (as EA draws it).
pub(crate) type MLink = (String, String, String, String, String, String);

fn visibility(c: char) -> &'static str {
    match c {
        '-' => "Private",
        '#' => "Protected",
        '~' => "Package",
        _ => "Public",
    }
}

/// A member line: "+name : Type", "+Type name", "+method(args) Return".
fn member(line: &str, class: &mut MClass) {
    let abstract_ = line.trim().trim_end_matches('$').ends_with('*');
    let line = line.trim().trim_end_matches(['$', '*']);
    if line.is_empty() || line.starts_with("%%") {
        return;
    }
    if let Some(stereo) = line.strip_prefix("<<").and_then(|s| s.strip_suffix(">>")) {
        match stereo.trim().to_lowercase().as_str() {
            "interface" => class.kind = "Interface".into(),
            "enumeration" | "enum" => class.kind = "Enumeration".into(),
            "abstract" => class.abstract_ = true,
            _ => class.stereotype = stereo.trim().to_string(),
        }
        return;
    }
    let (vis, rest) = match line.chars().next() {
        Some(c @ ('+' | '-' | '#' | '~')) => (visibility(c), line[1..].trim()),
        _ => ("Public", line),
    };
    if let Some(open) = rest.find('(') {
        let close = rest.rfind(')').unwrap_or(rest.len());
        let name = rest[..open].trim().to_string();
        let ret = rest[close.min(rest.len())..].trim_start_matches(')').trim().trim_start_matches(':').trim().to_string();
        let signature = format!("{name}({})", &rest[open + 1..close.max(open + 1)]);
        if abstract_ {
            class.abstract_operations.push(signature.clone());
        }
        class.operations.push((vis.into(), signature, ret));
    } else if let Some((name, ty)) = rest.split_once(':') {
        class.attributes.push((vis.into(), name.trim().to_string(), ty.trim().to_string()));
    } else if let Some((ty, name)) = rest.rsplit_once(' ') {
        class.attributes.push((vis.into(), name.trim().to_string(), ty.trim().to_string()));
    } else {
        class.attributes.push((vis.into(), rest.to_string(), String::new()));
    }
}

/// The relationship arrows of Mermaid, longest first: (arrow, type, reversed = the arrow points left).
const ARROWS: &[(&str, &str, bool)] = &[
    ("<|--", "Generalization", true), ("--|>", "Generalization", false), ("<|..", "Realisation", true), ("..|>", "Realisation", false),
    ("*--", "Composition", true), ("--*", "Composition", false), ("o--", "Aggregation", true), ("--o", "Aggregation", false),
    ("<--", "DirectedAssociation", true), ("-->", "DirectedAssociation", false), ("<..", "Dependency", true), ("..>", "Dependency", false), ("--", "Association", false), ("..", "Dependency", false),
];

pub(crate) fn parse_mermaid(text: &str) -> Result<(BTreeMap<String, MClass>, Vec<MLink>)> {
    let mut classes: BTreeMap<String, MClass> = BTreeMap::new();
    let mut links = Vec::new();
    let mut open: Option<String> = None;
    let mut seen_header = false;
    let ensure = |classes: &mut BTreeMap<String, MClass>, name: &str| {
        classes.entry(name.to_string()).or_insert_with(|| MClass { name: name.to_string(), kind: "Class".into(), ..Default::default() });
    };
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with("%%") || line.starts_with("```") {
            continue;
        }
        if line.starts_with("classDiagram") {
            seen_header = true;
            continue;
        }
        if let Some(name) = &open {
            if line == "}" {
                open = None;
            } else {
                let mut c = classes.remove(name).unwrap_or_default();
                member(line, &mut c);
                classes.insert(name.clone(), c);
            }
            continue;
        }
        if let Some(rest) = line.strip_prefix("class ") {
            let rest = rest.trim();
            let (name, body) = match rest.split_once('{') { Some((n, b)) => (n.trim(), Some(b.trim())), None => (rest.split_whitespace().next().unwrap_or(""), None) };
            let name = name.split('~').next().unwrap_or(name).trim().to_string();
            ensure(&mut classes, &name);
            if let Some(stereo) = rest.split_once("<<").and_then(|(_, s)| s.split_once(">>")).map(|(s, _)| s) {
                let mut c = classes.remove(&name).unwrap_or_default();
                member(&format!("<<{stereo}>>"), &mut c);
                classes.insert(name.clone(), c);
            }
            if body.is_some_and(|b| !b.ends_with('}')) {
                open = Some(name);
            }
            continue;
        }
        if let Some(stereo) = line.strip_prefix("<<").and_then(|s| s.split_once(">>")) {
            let name = stereo.1.trim().to_string();
            ensure(&mut classes, &name);
            let mut c = classes.remove(&name).unwrap_or_default();
            member(&format!("<<{}>>", stereo.0), &mut c);
            classes.insert(name, c);
            continue;
        }
        // "A "1" <|-- "*" B : label": the cardinalities in quotes are the multiplicities of the ends (LC-452).
        let (head, label) = line.split_once(" : ").map(|(h, l)| (h, l.trim())).unwrap_or((line, ""));
        let cleaned: String = head.split('"').enumerate().filter(|(i, _)| i % 2 == 0).map(|(_, p)| p).collect::<Vec<_>>().join(" ");
        if let Some((arrow, kind, reversed)) = ARROWS.iter().find(|(a, _, _)| cleaned.contains(a)) {
            let (a, b) = cleaned.split_once(arrow).unwrap_or(("", ""));
            let (a, b) = (a.trim().to_string(), b.trim().to_string());
            if a.is_empty() || b.is_empty() {
                continue;
            }
            ensure(&mut classes, &a);
            ensure(&mut classes, &b);
            let at = head.find(arrow).unwrap_or(0);
            let quoted = |part: &str, last: bool| -> String {
                let pieces: Vec<&str> = part.split('"').skip(1).step_by(2).collect();
                (if last { pieces.last() } else { pieces.first() }).map(|p| p.trim().to_string()).unwrap_or_default()
            };
            let (card_a, card_b) = (quoted(&head[..at], true), quoted(&head[at + arrow.len()..], false));
            let (from, to, card_from, card_to) = if *reversed { (b, a, card_b, card_a) } else { (a, b, card_a, card_b) };
            links.push((from, to, kind.to_string(), label.to_string(), card_from, card_to));
            continue;
        }
        // "Class : +member"
        if let Some((name, m)) = line.split_once(" : ").filter(|(n, _)| !n.contains(' ')) {
            ensure(&mut classes, name.trim());
            let mut c = classes.remove(name.trim()).unwrap_or_default();
            member(m, &mut c);
            classes.insert(name.trim().to_string(), c);
        }
    }
    if !seen_header && classes.is_empty() {
        bail!("not a Mermaid classDiagram (it starts with \"classDiagram\")");
    }
    Ok((classes, links))
}

/// The diagram text: the attached file or the text given.
fn diagram_text(input: &Value, cfg: &AppConfig) -> Result<String> {
    Ok(match input["file"].as_str().filter(|f| !f.trim().is_empty()) {
        Some(f) => {
            let name = std::path::Path::new(f).file_name().context("file")?.to_string_lossy().to_string();
            std::fs::read_to_string(crate::history::attachment_path(&cfg.attachments_dir, &name)).with_context(|| format!("there is no attachment {name}"))?
        }
        None => input["text"].as_str().unwrap_or("").to_string(),
    })
}

pub fn import_mermaid(input: &Value, cfg: &AppConfig) -> Result<(Vec<Value>, String)> {
    let (classes, links) = parse_mermaid(&diagram_text(input, cfg)?)?;
    build_import(input, cfg, classes, links, "Mermaid import", "Mermaid classDiagram", &[], &Drawing::default())
}

/// A PlantUML class diagram (LC-472): turned into the lines the Mermaid reader knows, then built the same way.
pub fn import_plantuml(input: &Value, cfg: &AppConfig) -> Result<(Vec<Value>, String)> {
    let text = diagram_text(input, cfg)?;
    // A sequence diagram (the most common PlantUML): its participants become classes with lifelines of them, its
    // messages operations of those classes, and the diagram a sequence of these lifelines.
    if is_plantuml_sequence(&text) {
        let title = text.lines().map(str::trim).find(|l| l.to_lowercase().starts_with("title ")).map(|l| l[6..].trim().to_string());
        let name = input["name"].as_str().map(str::trim).filter(|n| !n.is_empty()).map(str::to_string).or(title).unwrap_or_else(|| "PlantUML sequence".into());
        let name = name.as_str();
        let lower: Vec<String> = text.lines().map(|l| l.trim().to_lowercase()).collect();
        let mut left_out = Vec::new();
        if lower.iter().any(|l| l.starts_with("== ") || l.starts_with("...") || l.starts_with("|||")) {
            left_out.push("dividers and delays".to_string());
        }
        if lower.iter().any(|l| l.starts_with("database ") || l.starts_with("queue ") || l.starts_with("boundary ") || l.starts_with("control ") || l.starts_with("entity ") || l.starts_with("collections ")) {
            left_out.push("the shapes of database/queue/boundary/control/entity participants (they are classes)".to_string());
        }
        if lower.iter().any(|l| ["note", "hnote", "rnote"].iter().any(|n| l.strip_prefix(n).is_some_and(|r| r.trim_start().starts_with("on link")))) {
            left_out.push("notes on messages (note on link)".to_string());
        }
        if lower.iter().any(|l| l.starts_with("else")) {
            // EA's API cannot add operands to a fragment (Partitions.AddNew with Update ended the EA process, LC-477).
            left_out.push("separate operands of alt/else (the else conditions are in the fragment's name)".to_string());
        }
        let (ops, report) = crate::sequence::from_text(&json!({ "text": text, "package": input["package"], "name": name, "create_classes": true }), cfg)?;
        let mut summary = format!("PlantUML sequence diagram → diagram \"{name}\" with classes, their operations and lifelines:\n{report}");
        // The same list every time, said as it is (LC-477: answers named a different part of it each time, and one
        // described a divider as imported).
        if left_out.is_empty() {
            summary.push_str("\nLeft out: nothing.");
        } else {
            summary.push_str(&format!("\nLeft out (tell the user every item of this list as it is; nothing else was left out, and these were not imported): {}.", left_out.join("; ")));
        }
        return Ok(if input["action"].as_str() == Some("apply") { (ops, summary) } else { (Vec::new(), format!("{summary}\nNothing saved yet: action apply saves it.")) });
    }
    let (classes, links, left_out, drawing) = parse_plantuml(&text)?;
    let packages: Vec<String> = text.lines().map(str::trim).filter(|l| l.to_lowercase().starts_with("package ")).map(|l| l[8..].trim_end_matches('{').trim().trim_matches('"').to_string()).collect();
    let mut input = input.clone();
    if input["name"].as_str().is_none_or(|n| n.trim().is_empty()) && packages.len() == 1 {
        input["name"] = json!(packages[0]);
    }
    let input = &input;
    build_import(input, cfg, classes, links, "PlantUML import", "PlantUML class diagram", &left_out, &drawing)
}

/// Whether PlantUML text is a sequence diagram: messages (A -> B) and no class declarations or class relationships.
pub(crate) fn is_plantuml_sequence(text: &str) -> bool {
    let lines: Vec<String> = text.lines().map(|l| l.trim().to_lowercase()).collect();
    let classy = lines.iter().any(|l| {
        ["class ", "abstract ", "interface ", "enum ", "entity ", "annotation "].iter().any(|k| l.starts_with(k)) && !l.contains("->")
            || ["<|--", "--|>", "..|>", "<|..", "*--", "--*", "o--", "--o"].iter().any(|a| l.contains(a))
    });
    let messages = lines.iter().any(|l| (l.contains("->") || l.contains("<-")) && !l.starts_with('\''));
    let participants = lines.iter().any(|l| ["participant ", "actor ", "boundary ", "control ", "database ", "collections ", "queue "].iter().any(|k| l.starts_with(k)));
    !classy && messages && (participants || lines.iter().any(|l| l.contains(" : ") && l.contains("->")))
}

/// The arrow of a PlantUML relationship without its direction (-up->, -l->) and colour (-[#red]->) and with its
/// length (--->, ...>) shortened, as Mermaid writes it.
fn plain_arrow(arrow: &str) -> String {
    let mut a = arrow.to_string();
    while let (Some(open), Some(close)) = (a.find('['), a.find(']')) {
        if close < open {
            break;
        }
        a.replace_range(open..=close, "");
    }
    for word in ["up", "down", "left", "right", "u", "d", "l", "r"] {
        for line in ["-", "."] {
            a = a.replace(&format!("{line}{word}{line}"), &format!("{line}{line}"));
        }
    }
    while a.contains("---") {
        a = a.replace("---", "--");
    }
    while a.contains("...") {
        a = a.replace("...", "..");
    }
    // Ends Mermaid has no word for: a nested class (+--), a hash or a cross end, as an association.
    a.replace("+--", "--").replace("--+", "--").replace("#--", "--").replace("--#", "--").replace("x--", "--").replace("--x", "--")
}

/// PlantUML class diagram → (classes, relationships, what was left out, notes and colours). Aliases become the real names.
pub(crate) fn parse_plantuml(text: &str) -> Result<(BTreeMap<String, MClass>, Vec<MLink>, Vec<String>, Drawing)> {
    let mut lines = vec!["classDiagram".to_string()];
    let mut aliases: BTreeMap<String, String> = BTreeMap::new();
    // Name → the short name written with "as" (LC-473: it goes into EA's Alias).
    let mut short_names: BTreeMap<String, String> = BTreeMap::new();
    let mut left_out: Vec<String> = Vec::new();
    let mut skip_until: Option<&str> = None;
    let mut seen = false;
    let mut depth_packages = 0usize;
    let mut in_class = false;
    // Notes and colours by the ids of the text (LC-473); the real names at the end.
    let mut drawing = Drawing::default();
    // Floating notes ("note "…" as N1"): id → index in drawing.notes.
    let mut note_ids: BTreeMap<String, usize> = BTreeMap::new();
    // A note over more lines: its index, its lines; and whether it is on a relationship (not imported).
    let mut open_note: Option<(Option<usize>, Vec<String>)> = None;
    let mut last_class: Option<String> = None;
    for raw in text.lines() {
        let line = raw.trim();
        if let Some((index, body)) = open_note.as_mut() {
            let lower = line.to_lowercase();
            if lower.starts_with("end note") || lower.starts_with("endnote") {
                if let Some(index) = *index {
                    drawing.notes[index].0 = body.join("\n");
                }
                open_note = None;
            } else {
                body.push(line.to_string());
            }
            continue;
        }
        if let Some(end) = skip_until {
            if line.to_lowercase().starts_with(end) {
                skip_until = None;
            }
            continue;
        }
        let lower = line.to_lowercase();
        if line.is_empty() || line.starts_with('\'') || lower.starts_with("@start") || lower.starts_with("@end") || line.starts_with("```") {
            seen |= lower.starts_with("@startuml");
            continue;
        }
        if line.starts_with("/'") {
            if !line.contains("'/") {
                skip_until = Some("'/");
            }
            continue;
        }
        // Not part of the model: looks, titles, notes, hidden parts.
        if ["skinparam", "title", "hide ", "show ", "left to right", "top to bottom", "!", "scale", "caption", "legend", "header", "footer", "set "]
            .iter()
            .any(|w| lower.starts_with(w))
        {
            if lower.starts_with("legend") {
                skip_until = Some("endlegend");
            } else if lower.starts_with("skinparam") && line.ends_with('{') {
                skip_until = Some("}");
            }
            continue;
        }
        // Notes (LC-473): "note left of A : text", "note top of A" … "end note", "note "text" as N1" (linked later with
        // "N1 .. A"), "note right : text" (of the class above). A note on a relationship is left out.
        if lower.starts_with("note ") {
            let (head, text) = match line.split_once(':') {
                Some((h, t)) if !h.contains('"') => (h.trim().to_string(), Some(t.trim().to_string())),
                _ => (line.to_string(), None),
            };
            let words: Vec<&str> = head.split_whitespace().collect();
            if words.get(1).is_some_and(|w| w.eq_ignore_ascii_case("on")) {
                if text.is_none() {
                    open_note = Some((None, Vec::new()));
                }
                if !left_out.iter().any(|x| x.starts_with("notes on relationships")) {
                    left_out.push("notes on relationships".into());
                }
                continue;
            }
            let floating = head.split_once(" as ").filter(|_| words.get(1).is_some_and(|w| w.starts_with('"') || w.eq_ignore_ascii_case("as")));
            let index = drawing.notes.len();
            match floating {
                Some((quoted, id)) => {
                    let quoted = quoted.trim()[4..].trim().trim_matches('"').to_string();
                    drawing.notes.push((quoted.clone(), Vec::new()));
                    note_ids.insert(id.trim().to_string(), index);
                    if quoted.is_empty() && text.is_none() {
                        open_note = Some((Some(index), Vec::new()));
                    }
                }
                None => {
                    let target = match words.iter().position(|w| w.eq_ignore_ascii_case("of")) {
                        Some(at) => words[at + 1..].join(" ").trim_matches('"').to_string(),
                        None => last_class.clone().unwrap_or_default(),
                    };
                    drawing.notes.push((text.clone().unwrap_or_default(), if target.is_empty() { Vec::new() } else { vec![target] }));
                    if text.is_none() {
                        open_note = Some((Some(index), Vec::new()));
                    }
                }
            }
            continue;
        }
        if !in_class && ["package ", "namespace ", "together", "rectangle ", "frame ", "folder "].iter().any(|w| lower.starts_with(w)) {
            if line.ends_with('{') {
                depth_packages += 1;
            }
            if !left_out.contains(&"packages (their classes are imported)".to_string()) {
                left_out.push("packages (their classes are imported)".into());
            }
            continue;
        }
        if in_class {
            if line == "}" {
                in_class = false;
                lines.push("}".into());
                continue;
            }
            // Separators of a class body and member modifiers.
            if ["--", "..", "==", "__"].iter().any(|s| line.starts_with(s) && line.chars().all(|c| "-._= ".contains(c) || c.is_alphanumeric())) && !line.contains('(') && !line.contains(':') {
                continue;
            }
            let member = line.replace("{abstract}", "").replace("{static}", "").replace("{classifier}", "").replace("{field}", "").replace("{method}", "");
            // An abstract operation keeps Mermaid's mark (LC-473: it was imported as an ordinary one).
            let mark = if line.contains("{abstract}") { "*" } else { "" };
            lines.push(format!("{}{mark}", member.trim()));
            continue;
        }
        if line == "}" {
            depth_packages = depth_packages.saturating_sub(1);
            continue;
        }
        // A declaration: [abstract] class|interface|enum|entity|annotation|abstract Name ["Long name" as Alias] [<<st>>] [{]
        let kinds = [("abstract class ", "class", true), ("abstract ", "class", true), ("class ", "class", false), ("interface ", "interface", false),
            ("enum ", "enum", false), ("entity ", "entity", false), ("annotation ", "annotation", false), ("struct ", "class", false), ("protocol ", "interface", false)];
        if let Some((prefix, kind, abstract_)) = kinds.iter().find(|(p, _, _)| lower.starts_with(p)) {
            let mut rest = line[prefix.len()..].trim().to_string();
            let opens = rest.ends_with('{');
            rest = rest.trim_end_matches('{').trim().to_string();
            let stereo = rest.find("<<").and_then(|a| rest[a..].find(">>").map(|b| rest[a + 2..a + b].trim().to_string()));
            if let Some(a) = rest.find("<<") {
                let b = rest[a..].find(">>").map(|b| a + b + 2).unwrap_or(rest.len());
                rest.replace_range(a..b, "");
            }
            // Generics, colours and extends written on the declaration (the fill is kept, LC-473).
            let fill = rest.find(" #").and_then(|at| color_in(&rest[at..]));
            for cut in [" #", " extends ", " implements "] {
                if let Some(at) = rest.find(cut) {
                    rest.truncate(at);
                }
            }
            let rest = rest.trim();
            let (name, alias) = match rest.split_once(" as ") {
                Some((a, b)) => {
                    let (a, b) = (a.trim().trim_matches('"').to_string(), b.trim().trim_matches('"').to_string());
                    // Either "Long name" as L or L as "Long name": the quoted one is the name.
                    if rest.trim_start().starts_with('"') { (a, Some(b)) } else if b.contains(' ') { (b, Some(a)) } else { (a, Some(b)) }
                }
                None => (rest.trim_matches('"').to_string(), None),
            };
            let name = name.split('<').next().unwrap_or(&name).trim().to_string();
            if name.is_empty() {
                continue;
            }
            let id = alias.clone().unwrap_or_else(|| name.replace(' ', "_"));
            aliases.insert(id.clone(), name.clone());
            last_class = Some(id.clone());
            if let Some(fill) = fill {
                drawing.fills.insert(id.clone(), fill);
            }
            if let Some(short) = alias.filter(|a| *a != name) {
                short_names.insert(name.clone(), short);
            }
            // The stereotype lines first: inside an open body they would be read as members.
            lines.push(format!("class {id}"));
            match *kind {
                "interface" => lines.push(format!("<<interface>> {id}")),
                "enum" => lines.push(format!("<<enumeration>> {id}")),
                "entity" | "annotation" => lines.push(format!("<<{kind}>> {id}")),
                _ => {}
            }
            if *abstract_ {
                lines.push(format!("<<abstract>> {id}"));
            }
            if let Some(st) = stereo {
                lines.push(format!("<<{st}>> {id}"));
            }
            if opens {
                lines.push(format!("class {id} {{"));
            }
            in_class = opens;
            continue;
        }
        // A relationship: names, an arrow, optional multiplicities in quotes and a label after a colon.
        let (head, label) = line.split_once(':').map(|(h, l)| (h.trim(), l.trim().trim_start_matches(['<', '>']).trim_end_matches(['<', '>']).trim())).unwrap_or((line, ""));
        let mut parts: Vec<String> = Vec::new();
        let mut quoted = false;
        let mut current = String::new();
        for c in head.chars() {
            if c == '"' {
                quoted = !quoted;
                current.push(c);
            } else if c.is_whitespace() && !quoted {
                if !current.is_empty() {
                    parts.push(std::mem::take(&mut current));
                }
            } else {
                current.push(c);
            }
        }
        if !current.is_empty() {
            parts.push(current);
        }
        // The arrow: a line of - or . with ends and maybe a direction (*-up-, -[#red]->, <|--).
        let arrowish = |p: &str| {
            let ends = |c: char| "<>|*o.-+#x".contains(c);
            p.contains("--") || p.contains("..") || p.contains("-[") || p.contains(".[") || (p.contains('-') && p.starts_with(ends) && p.ends_with(ends) && p.len() > 2)
        };
        let arrow_at = parts.iter().position(|p| !p.starts_with('"') && arrowish(p));
        if let Some(at) = arrow_at {
            let arrow = plain_arrow(&parts[at]);
            let (Some(a), Some(b)) = (parts[..at].iter().find(|p| !p.starts_with('"')), parts[at + 1..].iter().find(|p| !p.starts_with('"'))) else { continue };
            let card_a = parts[..at].iter().rev().find(|p| p.starts_with('"')).cloned().unwrap_or_default();
            let card_b = parts[at + 1..].iter().find(|p| p.starts_with('"')).cloned().unwrap_or_default();
            let (a, b) = (a.trim_matches('"').to_string(), b.trim_matches('"').to_string());
            // A floating note linked to a class: no relationship of the model.
            if let Some((note, other)) = note_ids.get(&a).map(|n| (*n, &b)).or_else(|| note_ids.get(&b).map(|n| (*n, &a))) {
                drawing.notes[note].1.push(other.clone());
                continue;
            }
            if let Some(colour) = color_in(&parts[at]) {
                drawing.line_colors.push((a.replace(' ', "_"), b.replace(' ', "_"), colour));
            }
            for n in [&a, &b] {
                aliases.entry(n.replace(' ', "_")).or_insert_with(|| n.clone());
            }
            lines.push(format!(
                "{} {card_a} {arrow} {card_b} {}{}",
                a.replace(' ', "_"),
                b.replace(' ', "_"),
                if label.is_empty() { String::new() } else { format!(" : {label}") }
            ));
            continue;
        }
        // "Class : member"
        if let Some((n, m)) = line.split_once(" : ") {
            lines.push(format!("{} : {}", n.trim().trim_matches('"').replace(' ', "_"), m.trim()));
        }
    }
    if !seen && aliases.is_empty() {
        bail!("not a PlantUML class diagram (no @startuml and no classes)");
    }
    let (classes, links) = parse_mermaid(&lines.join("\n"))?;
    let real = |id: &str| aliases.get(id).cloned().unwrap_or_else(|| id.replace('_', " "));
    let classes = classes
        .into_values()
        .map(|mut c| {
            c.name = real(&c.name);
            c.alias = short_names.get(&c.name).cloned().unwrap_or_default();
            (c.name.clone(), c)
        })
        .collect();
    let links = links.into_iter().map(|(f, t, k, l, cf, ct)| (real(&f), real(&t), k, l, cf, ct)).collect();
    let drawing = Drawing {
        notes: drawing.notes.into_iter().filter(|(t, _)| !t.is_empty()).map(|(t, on)| (t, on.iter().map(|o| real(&o.replace(' ', "_"))).collect())).collect(),
        fills: drawing.fills.into_iter().map(|(id, c)| (real(&id), c)).collect(),
        line_colors: drawing.line_colors.into_iter().map(|(a, b, c)| (real(&a), real(&b), c)).collect(),
    };
    Ok((classes, links, left_out, drawing))
}

/// The package, diagram, classes and relationships of an imported class diagram: one save.
fn build_import(input: &Value, cfg: &AppConfig, classes: BTreeMap<String, MClass>, links: Vec<MLink>, default_name: &str, notation: &str, left_out: &[String], drawing: &Drawing) -> Result<(Vec<Value>, String)> {
    if classes.is_empty() {
        bail!("no classes in the diagram");
    }
    let (scope, _) = crate::model_usage::package_scope(cfg, &input["package"])?;
    let parent: i64 = scope.split('(').nth(1).and_then(|s| s.split([',', ')']).next()).and_then(|p| p.parse().ok()).context("package")?;
    let name = input["name"].as_str().map(str::trim).filter(|n| !n.is_empty()).unwrap_or(default_name);
    let key = |n: &str| format!("m_{}", n.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect::<String>());
    let mut ops = vec![
        json!({ "op": "create_package", "key": "pkg", "parent": parent, "name": name }),
        json!({ "op": "create_diagram", "key": "dia", "package": "#pkg", "name": name, "type": "Logical" }),
    ];
    for (i, c) in classes.values().enumerate() {
        let k = key(&c.name);
        let mut element = json!({ "op": "create_element", "key": k, "package": "#pkg", "name": c.name, "type": c.kind });
        if !c.stereotype.is_empty() {
            element["stereotype"] = json!(c.stereotype);
        }
        ops.push(element);
        if c.abstract_ || !c.alias.is_empty() {
            let mut update = json!({ "op": "update_element", "element": format!("#{k}") });
            if c.abstract_ {
                update["abstract"] = json!(true);
            }
            if !c.alias.is_empty() {
                update["alias"] = json!(c.alias);
            }
            ops.push(update);
        }
        for (vis, n, ty) in &c.attributes {
            ops.push(json!({ "op": "create_attribute", "element": format!("#{k}"), "name": n, "type": ty, "visibility": vis }));
        }
        for (vis, sig, ret) in &c.operations {
            let (n, params) = sig.split_once('(').unwrap_or((sig, ""));
            let params: Vec<Value> = params.trim_end_matches(')').split(',').map(str::trim).filter(|p| !p.is_empty()).map(|p| {
                let (pn, pt) = p.split_once(':').map(|(a, b)| (a.trim(), b.trim())).or_else(|| p.rsplit_once(' ').map(|(t, n)| (n.trim(), t.trim()))).unwrap_or((p, ""));
                json!({ "name": pn, "type": pt })
            }).collect();
            let mut operation = json!({ "op": "create_operation", "element": format!("#{k}"), "name": n, "return_type": ret, "visibility": vis, "parameters": params });
            if c.abstract_operations.contains(sig) {
                operation["abstract"] = json!(true);
            }
            ops.push(operation);
        }
        ops.push(json!({ "op": "place", "diagram": "#dia", "element": format!("#{k}"), "left": 40 + (i as i64 % 4) * 230, "top": 40 + (i as i64 / 4) * 200, "width": 180, "height": 120 }));
        if let Some(fill) = drawing.fills.get(&c.name) {
            ops.push(json!({ "op": "set_color", "diagram": "#dia", "element": format!("#{k}"), "bgcolor": fill }));
        }
    }
    let mut colored: Vec<usize> = Vec::new();
    for (i, (from, to, kind, label, card_from, card_to)) in links.iter().enumerate() {
        let (kind, subtype) = match kind.as_str() {
            "Composition" => ("Aggregation", Some("Strong")),
            "Aggregation" => ("Aggregation", Some("Weak")),
            "DirectedAssociation" => ("Association", None),
            k => (k, None),
        };
        // In EA an aggregation goes from the part to the whole; parse_mermaid already gives (part, whole).
        let (s, t) = (from, to);
        let mut op = json!({ "op": "create_connector", "source": format!("#{}", key(s)), "target": format!("#{}", key(t)), "type": kind, "name": label });
        if let Some(st) = subtype {
            op["subtype"] = json!(st);
        }
        // An arrow (-->) is navigable towards the target (LC-473).
        if links[i].2 == "DirectedAssociation" {
            op["direction"] = json!("Source -> Destination");
        }
        // The line colour written on the arrow (-[#red]->), each written one used once.
        let colour = drawing.line_colors.iter().enumerate().find(|(n, (a, b, _))| !colored.contains(n) && ((a == from && b == to) || (a == to && b == from)));
        if let Some((n, (_, _, c))) = colour {
            colored.push(n);
            op["color"] = json!(c);
        }
        let cards = !card_from.is_empty() || !card_to.is_empty();
        if cards {
            op["key"] = json!(format!("rel{i}"));
        }
        ops.push(op);
        if cards {
            ops.push(json!({ "op": "update_connector", "connector": format!("#rel{i}"), "source_multiplicity": card_from, "target_multiplicity": card_to }));
        }
    }
    ops.push(json!({ "op": "layout_diagram", "diagram": "#dia", "direction": "down", "spacing": 40 }));
    // Notes, linked to their classes (LC-473), one under another below the laid-out classes: the layout moves only classes, so
    // notes placed before it ended up on top of them (round 11a).
    for (n, (text, on)) in drawing.notes.iter().enumerate() {
        let note = format!("note{n}");
        ops.push(json!({ "op": "create_element", "key": note, "package": "#pkg", "name": "", "type": "Note", "notes": text }));
        let height = 30 + 15 * text.lines().count() as i64;
        ops.push(json!({ "op": "place", "diagram": "#dia", "element": format!("#{note}"), "left": 40, "top": 0, "below_all": true, "width": 260, "height": height }));
        for target in on.iter().filter(|t| classes.contains_key(*t)) {
            ops.push(json!({ "op": "create_connector", "source": format!("#{note}"), "target": format!("#{}", key(target)), "type": "NoteLink" }));
        }
    }
    let mut text = format!("{notation} → package \"{name}\": {} classes, {} relationships, a diagram.", classes.len(), links.len());
    if !drawing.notes.is_empty() {
        text.push_str(&format!(" Notes on the diagram: {}.", drawing.notes.len()));
    }
    if !left_out.is_empty() {
        text.push_str(&format!(" Left out: {}.", left_out.join(", ")));
    }
    Ok(if input["action"].as_str() == Some("apply") { (ops, text) } else { (Vec::new(), format!("{text} Nothing saved yet: action apply saves it.")) })
}

// ---------- DDL ----------

/// A column type in a dialect (generic names mapped; others kept as they are).
pub(crate) fn column_type(dialect: &str, ty: &str, length: &str) -> String {
    let t = ty.trim().to_lowercase();
    let len = length.trim().parse::<u32>().ok().filter(|l| *l > 0);
    let with_len = |base: &str, default: u32| format!("{base}({})", len.unwrap_or(default));
    match (dialect, t.as_str()) {
        (_, "varchar" | "nvarchar" | "string" | "char varying") => match dialect { "oracle" => with_len("VARCHAR2", 255), "sqlserver" => with_len("NVARCHAR", 255), "sqlite" => "TEXT".into(), _ => with_len("VARCHAR", 255) },
        (_, "char" | "nchar") => with_len("CHAR", 1),
        (_, "text" | "clob" | "longtext" | "memo") => match dialect { "oracle" => "CLOB".into(), "sqlserver" => "NVARCHAR(MAX)".into(), "mysql" => "LONGTEXT".into(), _ => "TEXT".into() },
        (_, "int" | "integer" | "int4") => match dialect { "oracle" => "NUMBER(10)".into(), _ => "INTEGER".into() },
        (_, "bigint" | "long" | "int8") => match dialect { "oracle" => "NUMBER(19)".into(), _ => "BIGINT".into() },
        (_, "smallint" | "short") => match dialect { "oracle" => "NUMBER(5)".into(), _ => "SMALLINT".into() },
        (_, "boolean" | "bool" | "bit") => match dialect { "oracle" => "NUMBER(1)".into(), "sqlserver" => "BIT".into(), "sqlite" => "INTEGER".into(), _ => "BOOLEAN".into() },
        (_, "datetime" | "timestamp") => match dialect { "sqlserver" => "DATETIME2".into(), "mysql" => "DATETIME".into(), "sqlite" => "TEXT".into(), _ => "TIMESTAMP".into() },
        (_, "date") => match dialect { "sqlite" => "TEXT".into(), _ => "DATE".into() },
        (_, "decimal" | "numeric" | "money") => match dialect { "oracle" => "NUMBER(18,2)".into(), "sqlite" => "REAL".into(), _ => "DECIMAL(18,2)".into() },
        (_, "float" | "double" | "real") => match dialect { "oracle" => "BINARY_DOUBLE".into(), "sqlserver" => "FLOAT".into(), "postgres" => "DOUBLE PRECISION".into(), _ => "REAL".into() },
        (_, "uuid" | "guid" | "uniqueidentifier") => match dialect { "sqlserver" => "UNIQUEIDENTIFIER".into(), "postgres" => "UUID".into(), "oracle" => "RAW(16)".into(), _ => "CHAR(36)".into() },
        (_, "blob" | "binary" | "varbinary" | "bytea") => match dialect { "postgres" => "BYTEA".into(), "sqlserver" => "VARBINARY(MAX)".into(), _ => "BLOB".into() },
        _ if ty.trim().is_empty() => "VARCHAR(255)".into(),
        _ => ty.trim().to_uppercase(),
    }
}

fn quote_name(dialect: &str, name: &str) -> String {
    let plain = name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') && !name.chars().next().is_some_and(|c| c.is_ascii_digit());
    if plain {
        return name.to_string();
    }
    match dialect { "mysql" => format!("`{}`", name.replace('`', "``")), "sqlserver" => format!("[{}]", name.replace(']', "]]")), _ => format!("\"{}\"", name.replace('"', "\"\"")) }
}

/// The parameters (columns) of a key operation of a table, in their order.
fn key_columns(cfg: &AppConfig, operation: &str) -> Result<Vec<String>> {
    Ok(query_rows(cfg, &format!("select name from t_operationparams where operationid = {operation} order by pos"))?.into_iter().map(|r| r[0].clone()).collect())
}

/// FOREIGN KEY clauses of a table as EA models them (LC-453): an «FK» operation with the table's columns, and an «FK»
/// relationship from this table to the referenced one whose StyleEx says FKINFO=SRC=<FK operation>:DST=<PK operation>.
/// A relationship without FKINFO (drawn by hand) is used when it is the only one left for a single FK operation; the
/// referenced columns are those of the DST operation, else the referenced table's primary key.
fn foreign_keys(cfg: &AppConfig, dialect: &str, table: i64) -> Result<Vec<String>> {
    let fks = query_rows(cfg, &format!("select operationid, name from t_operation where object_id = {table} and upper(coalesce(stereotype,'')) = 'FK' order by pos, name"))?;
    if fks.is_empty() {
        return Ok(Vec::new());
    }
    let links = query_rows(cfg, &format!(
        "select c.end_object_id, coalesce(c.styleex,''), o.name from t_connector c join t_object o on o.object_id = c.end_object_id \
         where c.start_object_id = {table} and upper(coalesce(c.stereotype,'')) = 'FK' order by c.connector_id"
    ))?;
    let info = |style: &str, part: &str| -> Option<String> {
        let at = style.find("FKINFO=")? + "FKINFO=".len();
        style[at..].split(';').next()?.split(':').find_map(|p| p.strip_prefix(&format!("{part}=")).map(str::to_string))
    };
    let mut used = vec![false; links.len()];
    let mut out = Vec::new();
    for fk in &fks {
        let columns = key_columns(cfg, &fk[0])?;
        let mut found = links.iter().position(|l| info(&l[1], "SRC").is_some_and(|src| src == fk[1]));
        if found.is_none() && fks.len() == 1 {
            let free: Vec<usize> = (0..links.len()).filter(|i| !used[*i] && info(&links[*i][1], "SRC").is_none()).collect();
            if free.len() == 1 {
                found = Some(free[0]);
            }
        }
        let Some(i) = found else {
            out.push(format!("  -- {}", crate::i18n::fmt("{0}: the referenced table is not known (no «FK» relationship from this table)", &[fk[1].clone()])));
            continue;
        };
        used[i] = true;
        let (target, target_name) = (&links[i][0], &links[i][2]);
        let pk_of = |name: Option<String>| -> Result<Vec<String>> {
            let condition = match name {
                Some(n) => format!("name = '{}'", crate::model_db::sql_text(cfg.model_source, &n)),
                None => "upper(coalesce(stereotype,'')) = 'PK'".to_string(),
            };
            match query_rows(cfg, &format!("select operationid from t_operation where object_id = {target} and {condition} order by pos"))?.first() {
                Some(r) => key_columns(cfg, &r[0]),
                None => Ok(Vec::new()),
            }
        };
        let mut referenced = pk_of(info(&links[i][1], "DST"))?;
        if referenced.is_empty() {
            referenced = pk_of(None)?;
        }
        if referenced.len() != columns.len() {
            referenced = columns.clone();
        }
        let q = |list: &[String]| list.iter().map(|c| quote_name(dialect, c)).collect::<Vec<_>>().join(", ");
        out.push(format!("  CONSTRAINT {} FOREIGN KEY ({}) REFERENCES {} ({})", quote_name(dialect, &fk[1]), q(&columns), quote_name(dialect, target_name), q(&referenced)));
    }
    Ok(out)
}

pub fn table_ddl(input: &Value, cfg: &AppConfig) -> Result<String> {
    let dialect = input["dialect"].as_str().unwrap_or("postgres");
    if !matches!(dialect, "postgres" | "sqlserver" | "mysql" | "oracle" | "sqlite") {
        bail!("dialect: postgres, sqlserver, mysql, oracle or sqlite");
    }
    let (scope, title) = crate::model_usage::package_scope_shown(cfg, &input["package"])?;
    let tables = query_rows(cfg, &format!("select o.object_id, o.name from t_object o where (o.object_type = 'Table' or (o.object_type = 'Class' and lower(coalesce(o.stereotype,'')) = 'table')) {scope} order by o.name"))?;
    if tables.is_empty() {
        bail!("no Table elements in the {title}");
    }
    let mut out = vec![format!("-- {}", crate::i18n::fmt("tables: {0} (scope: {1}), dialect {2}", &[tables.len().to_string(), title.clone(), dialect.to_string()]))];
    for t in &tables {
        let id: i64 = t[0].parse().unwrap_or(0);
        let columns = query_rows(cfg, &format!("select name, coalesce(type,''), coalesce(length,''), coalesce(allowduplicates,0) from t_attribute where object_id = {id} order by pos"))?;
        let pk: Vec<String> = query_rows(cfg, &format!(
            "select p.name from t_operation o join t_operationparams p on p.operationid = o.operationid where o.object_id = {id} and upper(coalesce(o.stereotype,'')) = 'PK' order by p.pos"
        ))?.into_iter().map(|r| quote_name(dialect, &r[0])).collect();
        let mut lines: Vec<String> = columns.iter().map(|c| {
            let not_null = c[3] == "1" || c[3].eq_ignore_ascii_case("true") || pk.contains(&quote_name(dialect, &c[0]));
            format!("  {} {}{}", quote_name(dialect, &c[0]), column_type(dialect, &c[1], &c[2]), if not_null { " NOT NULL" } else { "" })
        }).collect();
        if !pk.is_empty() {
            lines.push(format!("  PRIMARY KEY ({})", pk.join(", ")));
        }
        lines.extend(foreign_keys(cfg, dialect, id)?);
        out.push(format!("CREATE TABLE {} (\n{}\n);", quote_name(dialect, &t[1]), lines.join(",\n")));
    }
    Ok(out.join("\n\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mermaid_classes_and_links() {
        let (c, l) = parse_mermaid("classDiagram\n  class Order {\n    +String id\n    -date : Date\n    +total() Money\n  }\n  class Shape {\n    <<interface>>\n  }\n  Order \"1\" *-- \"many\" Line : contains\n  Animal <|-- Duck\n  Order ..> Shape\n  Duck : +swim()\n").unwrap();
        assert_eq!(c["Order"].attributes, vec![("Public".to_string(), "id".to_string(), "String".to_string()), ("Private".to_string(), "date".to_string(), "Date".to_string())]);
        assert_eq!(c["Order"].operations[0], ("Public".to_string(), "total()".to_string(), "Money".to_string()));
        assert_eq!(c["Shape"].kind, "Interface");
        let s = |t: &str| t.to_string();
        assert!(l.contains(&(s("Line"), s("Order"), s("Composition"), s("contains"), s("many"), s("1"))), "{l:?}");
        assert!(l.contains(&(s("Duck"), s("Animal"), s("Generalization"), String::new(), String::new(), String::new())));
        // LC-452: the multiplicities of the ends, also of a left-pointing aggregation.
        let (_, l) = parse_mermaid("classDiagram\n  Magazyn \"1\" o-- \"*\" Towar : przechowuje\n").unwrap();
        assert_eq!(l, vec![(s("Towar"), s("Magazyn"), s("Aggregation"), s("przechowuje"), s("*"), s("1"))]);
        assert_eq!(c["Duck"].operations[0].1, "swim()");
    }

    /// LC-472: a PlantUML class diagram as PlantUML users write it.
    #[test]
    fn plantuml_classes_and_links() {
        let text = r#"@startuml
title Zamówienia
skinparam classAttributeIconSize 0
package "Sprzedaż" {
abstract class "Dokument handlowy" as Dok <<entity>> {
  -numer : String
  {abstract} +wystaw() : void
  --
  +data : Date
}
class Zamówienie {
  +suma() : Money
}
}
interface Płatność
enum Status {
  NOWE
  ZAMKNIĘTE
}
note right of Zamówienie : notatka
note left of Status
  dłuższa notatka
end note
Dok <|-- Zamówienie
Zamówienie "1" *-up- "1..*" Pozycja : zawiera >
Zamówienie -[#red]-> Status
Zamówienie ..|> Płatność
Pozycja ...> Status : używa
' komentarz
@enduml"#;
        let (c, l, left, drawing) = parse_plantuml(text).unwrap();
        let s = |t: &str| t.to_string();
        let doc = &c["Dokument handlowy"];
        assert!(doc.abstract_ && doc.stereotype == "entity", "{doc:?}");
        assert_eq!(doc.attributes.len(), 2, "the separator is no attribute: {doc:?}");
        assert_eq!(doc.operations[0].1, "wystaw()");
        assert_eq!(c["Płatność"].kind, "Interface");
        assert_eq!(c["Status"].kind, "Enumeration");
        assert_eq!(c["Status"].attributes.len(), 2);
        assert!(l.contains(&(s("Zamówienie"), s("Dokument handlowy"), s("Generalization"), String::new(), String::new(), String::new())), "{l:?}");
        assert!(l.contains(&(s("Pozycja"), s("Zamówienie"), s("Composition"), s("zawiera"), s("1..*"), s("1"))), "{l:?}");
        assert!(l.iter().any(|x| x.0 == "Zamówienie" && x.1 == "Status" && x.2 == "DirectedAssociation"), "{l:?}");
        assert_eq!(doc.alias, "Dok");
        assert_eq!(doc.abstract_operations, ["wystaw()"]);
        assert!(c["Zamówienie"].alias.is_empty() && c["Zamówienie"].abstract_operations.is_empty());
        assert!(l.iter().any(|x| x.0 == "Zamówienie" && x.1 == "Płatność" && x.2 == "Realisation"), "{l:?}");
        assert!(l.iter().any(|x| x.0 == "Pozycja" && x.1 == "Status" && x.2 == "Dependency" && x.3 == "używa"), "{l:?}");
        assert!(!left.iter().any(|x| x.starts_with("notes")) && left.iter().any(|x| x.starts_with("packages")), "{left:?}");
        // LC-473: notes with their classes, the colour of the arrow.
        assert_eq!(drawing.notes, [(s("notatka"), vec![s("Zamówienie")]), (s("dłuższa notatka"), vec![s("Status")])]);
        assert_eq!(drawing.line_colors, [(s("Zamówienie"), s("Status"), s("#FF0000"))]);
        assert!(!c.contains_key("Zamówienia"), "the title is no class");
        assert!(parse_plantuml("hello").is_err());
        // A floating note linked to a class, a fill, a note on a relationship.
        let (_, l, left, d) = parse_plantuml("@startuml\nclass A #lightblue\nclass B #back:ABC;line:red\nA --> B\nnote \"zwykła\" as N1\nN1 .. B\nnote on link : o relacji\n@enduml").unwrap();
        assert_eq!(l.len(), 1, "the note's link is no relationship: {l:?}");
        assert_eq!(d.notes, [(s("zwykła"), vec![s("B")])]);
        assert_eq!(d.fills.get("A").map(String::as_str), Some("#ADD8E6"));
        assert_eq!(d.fills.get("B").map(String::as_str), Some("#AABBCC"));
        assert!(left.iter().any(|x| x == "notes on relationships"), "{left:?}");
    }

    #[test]
    fn ddl_types() {
        assert_eq!(column_type("oracle", "varchar", "40"), "VARCHAR2(40)");
        assert_eq!(column_type("sqlserver", "text", ""), "NVARCHAR(MAX)");
        assert_eq!(column_type("postgres", "datetime", ""), "TIMESTAMP");
        assert_eq!(quote_name("sqlserver", "Order Line"), "[Order Line]");
    }

    /// `LOONAR_TEST_PROPS=… cargo test mermaid_ddl_live -- --ignored --nocapture`: a Mermaid diagram imported into EA; tables made
    /// with columns and a primary key, their DDL in two dialects.
    #[test]
    #[ignore = "requires EA and a copy of a test model"]
    fn mermaid_ddl_live() {
        let cfg = AppConfig::load(std::path::Path::new(&std::env::var("LOONAR_TEST_PROPS").unwrap())).unwrap();
        let root: i64 = query_rows(&cfg, "select package_id from t_package where parent_id = 0 order by package_id").unwrap()[0][0].parse().unwrap();
        let name = format!("ZZ_mmd_{}", chrono::Local::now().format("%H%M%S"));
        let (ops, text) = import_mermaid(&json!({ "text": "classDiagram\nclass Order {\n +String id\n +total() Money\n}\nOrder \"1\" *-- \"*\" Line : contains\nAnimal <|-- Duck", "package": root, "name": name, "action": "apply" }), &cfg).unwrap();
        println!("{text}");
        let done = crate::ea::run_batch(&cfg, &ops, &crate::ea::BatchOptions { rollback_key: Some("pkg".into()), keep_locks: false }).expect("import");
        let pkg = done["created"].as_array().unwrap().iter().find(|c| c["key"] == "pkg").unwrap()["id"].as_i64().unwrap();
        let classes = query_rows(&cfg, &format!("select name from t_object where package_id = {pkg} order by name")).unwrap();
        let links = query_rows(&cfg, &format!("select c.connector_type, coalesce(c.subtype,''), coalesce(c.sourcecard,''), coalesce(c.destcard,'') from t_connector c join t_object o on o.object_id = c.start_object_id where o.package_id = {pkg} order by 1")).unwrap();
        // Tables: two columns, the first in a primary key (EA's PK operation).
        crate::ea::run_batch(&cfg, &[
            json!({"op":"create_element","key":"t","package":pkg,"name":"customer","type":"Table"}),
            json!({"op":"create_attribute","element":"#t","name":"id","type":"integer"}),
            json!({"op":"create_attribute","element":"#t","name":"full name","type":"varchar"}),
            json!({"op":"create_operation","element":"#t","name":"PK_customer","stereotype":"PK","parameters":[{"name":"id","type":"integer"}]}),
            // LC-453: a foreign key drawn by hand (an «FK» operation and an «FK» relationship without FKINFO).
            json!({"op":"create_element","key":"o","package":pkg,"name":"orders","type":"Table"}),
            json!({"op":"create_attribute","element":"#o","name":"customer_id","type":"integer"}),
            json!({"op":"create_operation","element":"#o","name":"FK_orders_customer","stereotype":"FK","parameters":[{"name":"customer_id","type":"integer"}]}),
            json!({"op":"create_connector","source":"#o","target":"#t","type":"Association","stereotype":"FK"}),
        ], &crate::ea::BatchOptions::default()).unwrap();
        let pg = table_ddl(&json!({ "package": pkg, "dialect": "postgres" }), &cfg).unwrap();
        let ms = table_ddl(&json!({ "package": pkg, "dialect": "sqlserver" }), &cfg).unwrap();
        println!("{pg}\n{ms}");
        crate::ea::run_batch(&cfg, &[json!({"op":"delete","package":pkg})], &crate::ea::BatchOptions::default()).unwrap();
        crate::ea::shutdown();
        assert_eq!(classes.iter().map(|r| r[0].as_str()).collect::<Vec<_>>(), ["Animal", "Duck", "Line", "Order"]);
        assert!(links.iter().any(|l| l[0] == "Aggregation" && l[1] == "Strong") && links.iter().any(|l| l[0] == "Generalization"), "{links:?}");
        // LC-452: the part Line has "*", the whole Order "1".
        assert!(links.iter().any(|l| l[0] == "Aggregation" && l[2] == "*" && l[3] == "1"), "{links:?}");
        assert!(pg.contains("FOREIGN KEY (customer_id) REFERENCES customer (id)"), "{pg}");
        assert!(pg.contains("id INTEGER NOT NULL") && pg.contains("PRIMARY KEY (id)") && ms.contains("[full name] NVARCHAR(255)"), "{pg}\n{ms}");
    }

    /// LC-473: the alias, an abstract operation, an arrow's navigability and a composition's diamond reach EA.
    /// `LOONAR_TEST_PROPS=… cargo test plantuml_details_live -- --ignored --nocapture`
    #[test]
    #[ignore = "requires EA and a copy of a test model"]
    fn plantuml_details_live() {
        let cfg = AppConfig::load(std::path::Path::new(&std::env::var("LOONAR_TEST_PROPS").unwrap())).unwrap();
        let root: i64 = query_rows(&cfg, "select package_id from t_package where parent_id = 0 order by package_id").unwrap()[0][0].parse().unwrap();
        let name = format!("ZZ_puml_{}", chrono::Local::now().format("%H%M%S"));
        let text = "@startuml\nabstract class \"Dokument handlowy\" as Dok {\n  {abstract} +wystaw() : void\n  +drukuj() : void\n}\nclass Pozycja\nclass Status #lightblue\nDok *-- Pozycja\nDok o-- Status\nPozycja -[#red]-> Status\nnote right of Pozycja : uwaga\n@enduml";
        let (ops, text) = import_plantuml(&json!({ "text": text, "package": root, "name": name, "action": "apply" }), &cfg).unwrap();
        println!("{text}");
        let done = crate::ea::run_batch(&cfg, &ops, &crate::ea::BatchOptions { rollback_key: Some("pkg".into()), keep_locks: false }).expect("import");
        let pkg = done["created"].as_array().unwrap().iter().find(|c| c["key"] == "pkg").unwrap()["id"].as_i64().unwrap();
        let alias = query_rows(&cfg, &format!("select coalesce(alias,'') from t_object where package_id = {pkg} and name = 'Dokument handlowy'")).unwrap();
        let methods = query_rows(&cfg, &format!("select m.name, coalesce(m.abstract,'0') from t_operation m join t_object o on o.object_id = m.object_id where o.package_id = {pkg} order by 1")).unwrap();
        let links = query_rows(&cfg, &format!("select c.connector_type, coalesce(c.subtype,''), coalesce(c.direction,''), c.destisaggregate from t_connector c join t_object o on o.object_id = c.start_object_id where o.package_id = {pkg} order by 1, 2")).unwrap();
        let colour = query_rows(&cfg, &format!("select c.linecolor from t_connector c join t_object o on o.object_id = c.start_object_id where o.package_id = {pkg} and c.connector_type = 'Association'")).unwrap();
        let fill = query_rows(&cfg, &format!("select x.objectstyle from t_diagramobjects x join t_object o on o.object_id = x.object_id where o.package_id = {pkg} and o.name = 'Status'")).unwrap();
        let notes = query_rows(&cfg, &format!("select o.note, t.name from t_object o join t_connector c on c.start_object_id = o.object_id join t_object t on t.object_id = c.end_object_id where o.package_id = {pkg} and o.object_type = 'Note' and c.connector_type = 'NoteLink'")).unwrap();
        crate::ea::run_batch(&cfg, &[json!({"op":"delete","package":pkg})], &crate::ea::BatchOptions::default()).unwrap();
        crate::ea::shutdown();
        println!("{alias:?}\n{methods:?}\n{links:?}\n{colour:?}\n{fill:?}\n{notes:?}");
        assert_eq!(colour[0][0], "255", "red is 0x0000FF in EA's BGR");
        assert!(fill[0][0].contains("BCol=15128749"), "{fill:?}");
        assert_eq!(notes, vec![vec!["uwaga".to_string(), "Pozycja".to_string()]]);
        assert_eq!(alias[0][0], "Dok");
        assert!(methods.iter().any(|m| m[0] == "wystaw" && m[1] == "1") && methods.iter().any(|m| m[0] == "drukuj" && m[1] == "0"), "{methods:?}");
        assert!(links.iter().any(|l| l[0] == "Aggregation" && l[1] == "Strong" && l[3] == "2"), "{links:?}");
        assert!(links.iter().any(|l| l[0] == "Aggregation" && l[1] == "Weak" && l[3] == "1"), "{links:?}");
        assert!(links.iter().any(|l| l[0] == "Association" && l[2] == "Source -> Destination"), "{links:?}");
    }
}

#[cfg(test)]
mod plantuml_sequence_tests {
    const TEXT: &str = r#"@startuml
title Składanie zamówienia
autonumber
actor Klient
participant "Sklep WWW" as UI
participant SerwisZamowien
database Magazyn #lightblue
box "Zaplecze"
participant Platnosci
end box
Klient -> UI : zamow(koszyk: Koszyk)
activate UI
UI -> SerwisZamowien : utworz(klient, pozycje)
SerwisZamowien -> Magazyn : rezerwuj(pozycje)
Magazyn --> SerwisZamowien : ok
note right of Magazyn
  rezerwacja na 15 minut
end note
alt płatność online
  SerwisZamowien -> Platnosci : obciaz(kwota: Money)
  return potwierdzenie
else za pobraniem
  SerwisZamowien -[#red]> SerwisZamowien : oznaczZaPobraniem()
end
== Potwierdzenie ==
UI <-- SerwisZamowien : numer
deactivate UI
@enduml"#;

    #[test]
    fn plantuml_sequence_is_recognised() {
        assert!(super::is_plantuml_sequence(TEXT));
        assert!(!super::is_plantuml_sequence("@startuml\nclass A\nA <|-- B\n@enduml"));
    }

    /// LC-472: a PlantUML sequence imported into EA: classes, their operations with parameters, lifelines of them, messages.
    /// `LOONAR_TEST_PROPS=E:/livetest/r440/m5.properties LOONAR_TEST_PACKAGE=1116 cargo test plantuml_sequence_live -- --ignored --nocapture`
    #[test]
    #[ignore = "requires EA and a copy of a test model"]
    fn plantuml_sequence_live() {
        let mut cfg = crate::config::AppConfig::load(std::path::Path::new(&std::env::var("LOONAR_TEST_PROPS").unwrap())).unwrap();
        cfg.attachments_dir = std::env::temp_dir();
        let package: i64 = std::env::var("LOONAR_TEST_PACKAGE").unwrap().parse().unwrap();
        let name = format!("PU {}", chrono::Local::now().format("%H%M%S"));
        let (ops, text) = super::import_plantuml(&serde_json::json!({ "text": TEXT, "package": package, "name": name, "action": "apply" }), &cfg).unwrap();
        println!("{text}");
        let done = crate::ea::run_batch(&cfg, &ops, &crate::ea::BatchOptions::default()).expect("import");
        crate::ea::shutdown();
        let q = |sql: &str| crate::tools::query_rows(&cfg, sql).unwrap();
        let d = done["created"].as_array().unwrap().iter().find(|c| c["key"] == "seq").unwrap()["id"].as_i64().unwrap();
        let ops_of = q("select o.name, op.name, (select count(*) from t_operationparams p where p.operationid = op.operationid) from t_operation op join t_object o on o.object_id = op.object_id where o.name in ('Sklep WWW', 'SerwisZamowien', 'Magazyn', 'Platnosci') order by 1, 2");
        let lifelines = q(&format!("select c.name from t_diagramobjects x join t_object l on l.object_id = x.object_id join t_object c on c.object_id = l.classifier where x.diagram_id = {d} order by 1"));
        let messages = q(&format!("select count(*) from t_connector where diagramid = {d} and connector_type = 'Sequence'"));
        // LC-473: the note linked to its lifeline, the red message.
        let notes = q(&format!("select o.note, l.name from t_diagramobjects x join t_object o on o.object_id = x.object_id join t_connector c on c.start_object_id = o.object_id join t_object l on l.object_id = c.end_object_id where x.diagram_id = {d} and o.object_type = 'Note'"));
        let red = q(&format!("select name from t_connector where diagramid = {d} and linecolor = 255"));
        println!("{ops_of:?}\n{lifelines:?}\n{messages:?}\n{notes:?}\n{red:?}");
        assert!(notes.len() == 1 && notes[0][0].contains("rezerwacja na 15 minut") && notes[0][1] == "Magazyn", "{notes:?}");
        assert_eq!(red, vec![vec!["oznaczZaPobraniem()".to_string()]]);
        assert!(ops_of.iter().any(|r| r[0] == "Platnosci" && r[1] == "obciaz" && r[2] == "1"), "{ops_of:?}");
        assert!(ops_of.iter().any(|r| r[0] == "SerwisZamowien" && r[1] == "utworz" && r[2] == "2"), "{ops_of:?}");
        assert!(lifelines.iter().any(|r| r[0] == "Sklep WWW") && lifelines.iter().any(|r| r[0] == "Magazyn"), "{lifelines:?}");
        assert_eq!(messages[0][0], "8", "{messages:?}");
    }
}

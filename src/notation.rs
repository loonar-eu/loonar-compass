//! Notation bridges (LC-395, /diagram-as-text): a diagram of the model as Mermaid or PlantUML text – class diagrams
//! with attributes, operations and relationships, sequence diagrams with their messages in order, any other diagram as
//! a graph of its elements and relationships – and a C4 view (PlantUML C4) of components, systems and people. For
//! documentation in Git, wikis and Markdown. Reads only.

use crate::config::AppConfig;
use crate::tools::query_rows;
use anyhow::{bail, Result};
use serde_json::{json, Value};
use std::collections::HashMap;

pub fn definition() -> Value {
    json!({
        "name": "diagram_as_text",
        "description": "A diagram of the model as text: format mermaid or plantuml – class diagrams with attributes, operations and \
relationships (inheritance, realisation, composition, aggregation, association, dependency), sequence diagrams with their messages in \
order (replies dashed), any other diagram as a graph of its elements and relationships; format c4 – a C4 view in PlantUML (actors as \
people, components, application components, nodes and systems as containers or systems, relationships with their names). Give the text to \
the user in a code block. Reads only.",
        "input_schema": { "type": "object", "properties": {
            "diagram": { "description": "diagram_id or exact name" },
            "format": { "type": "string", "enum": ["mermaid", "plantuml", "c4"] }
        }, "required": ["diagram", "format"] }
    })
}

/// A name usable as an identifier in Mermaid and PlantUML.
fn ident(id: &str, name: &str) -> String {
    let clean: String = name.chars().filter(|c| c.is_alphanumeric() || *c == '_').collect();
    if clean.is_empty() || clean.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        format!("E{id}")
    } else if clean == name {
        clean
    } else {
        format!("{clean}_{id}")
    }
}

fn quote(text: &str) -> String {
    text.replace('"', "'").replace(['\r', '\n'], " ")
}

struct Element {
    id: String,
    name: String,
    kind: String,
    stereotype: String,
}

struct Link {
    source: String,
    target: String,
    kind: String,
    subtype: String,
    name: String,
    sequence: i64,
    reply: bool,
}

pub fn run(input: &Value, cfg: &AppConfig) -> Result<String> {
    let d = &input["diagram"];
    let rows = match d.as_i64().or_else(|| d.as_str().and_then(|s| s.trim().parse().ok())) {
        Some(id) => query_rows(cfg, &format!("select diagram_id, name, diagram_type from t_diagram where diagram_id = {id}"))?,
        None => query_rows(cfg, &format!("select diagram_id, name, diagram_type from t_diagram where name = '{}'", crate::model_db::sql_text(cfg.model_source, d.as_str().unwrap_or(""))))?,
    };
    let Some(diagram) = rows.first() else { bail!("there is no diagram {d}") };
    let id: i64 = diagram[0].parse().unwrap_or(0);
    let elements: Vec<Element> = query_rows(cfg, &format!(
        "select o.object_id, coalesce(o.name,''), o.object_type, coalesce(o.stereotype,'') from t_diagramobjects x join t_object o on o.object_id = x.object_id \
         where x.diagram_id = {id} and o.object_type not in ('Note', 'Text', 'Boundary', 'InteractionFragment', 'ProxyConnector') order by x.recttop desc, x.rectleft"
    ))?
    .into_iter()
    .map(|r| Element { id: r[0].clone(), name: r[1].clone(), kind: r[2].clone(), stereotype: r[3].clone() })
    .collect();
    let on: Vec<String> = elements.iter().map(|e| e.id.clone()).collect();
    if on.is_empty() {
        bail!("the diagram \"{}\" has no elements", diagram[1]);
    }
    let list = on.join(",");
    let sequence = diagram[2] == "Sequence";
    let links: Vec<Link> = query_rows(cfg, &format!(
        "select c.start_object_id, c.end_object_id, c.connector_type, coalesce(c.subtype,''), coalesce(c.name,''), coalesce(c.seqno,0), coalesce(c.pdata4,'0'), c.connector_id from t_connector c \
         where c.start_object_id in ({list}) and c.end_object_id in ({list}) and {} order by c.seqno, c.connector_id",
        if sequence { format!("c.diagramid = {id} and c.connector_type = 'Sequence'") } else {
            format!("c.connector_type not in ('Sequence', 'NoteLink') and not exists (select 1 from t_diagramlinks l where l.diagramid = {id} and l.connectorid = c.connector_id and l.hidden = 1)")
        }
    ))?
    .into_iter()
    .map(|r| Link { source: r[0].clone(), target: r[1].clone(), kind: r[2].clone(), subtype: r[3].clone(), name: r[4].clone(), sequence: r[5].parse().unwrap_or(0), reply: r[6] == "1" })
    .collect();
    let names: HashMap<String, String> = elements.iter().map(|e| (e.id.clone(), ident(&e.id, &e.name))).collect();
    let format = input["format"].as_str().unwrap_or("mermaid");
    let title = &diagram[1];
    let text = match (format, sequence) {
        ("c4", _) => c4(title, &elements, &links, &names),
        ("plantuml", true) => sequence_text(title, &elements, &links, &names, false),
        ("mermaid", true) => sequence_text(title, &elements, &links, &names, true),
        (f, false) if classes(&elements) => class_text(cfg, title, &elements, &links, &names, f == "mermaid")?,
        (f, false) => graph_text(title, &elements, &links, &names, f == "mermaid"),
        (other, _) => bail!("unknown format {other}: mermaid, plantuml or c4"),
    };
    Ok(format!("{} of the diagram \"{title}\" ({} elements, {} relationships):\n\n{text}", format, elements.len(), links.len()))
}

/// A class diagram: most elements are classes, interfaces, enumerations or data types.
fn classes(elements: &[Element]) -> bool {
    let n = elements.iter().filter(|e| matches!(e.kind.as_str(), "Class" | "Interface" | "Enumeration" | "DataType" | "Table")).count();
    n * 2 >= elements.len()
}

fn class_text(cfg: &AppConfig, title: &str, elements: &[Element], links: &[Link], names: &HashMap<String, String>, mermaid: bool) -> Result<String> {
    let ids = elements.iter().map(|e| e.id.clone()).collect::<Vec<_>>().join(",");
    let attributes = query_rows(cfg, &format!("select object_id, name, coalesce(type,''), coalesce(scope,'') from t_attribute where object_id in ({ids}) order by object_id, pos"))?;
    let operations = query_rows(cfg, &format!("select object_id, name, coalesce(type,''), coalesce(scope,'') from t_operation where object_id in ({ids}) order by object_id, pos"))?;
    let sign = |scope: &str| match scope { "Private" => "-", "Protected" => "#", "Package" => "~", _ => "+" };
    let mut out = vec![if mermaid { "classDiagram".to_string() } else { format!("@startuml\ntitle {}", quote(title)) }];
    for e in elements {
        let n = &names[&e.id];
        let kind = match e.kind.as_str() { "Interface" => "interface", "Enumeration" => "enum", _ => "class" };
        let label = if *n != e.name && !mermaid { format!("\"{}\" as {n}", quote(&e.name)) } else { n.clone() };
        let head = if mermaid {
            format!("class {n}{}", if *n != e.name { format!("[\"{}\"]", quote(&e.name)) } else { String::new() })
        } else {
            format!("{kind} {label}{}", if e.stereotype.is_empty() { String::new() } else { format!(" <<{}>>", e.stereotype) })
        };
        out.push(format!("{head} {{"));
        if mermaid && (kind != "class" || !e.stereotype.is_empty()) {
            out.push(format!("  <<{}>>", if kind != "class" { kind.to_string() } else { e.stereotype.clone() }));
        }
        for a in attributes.iter().filter(|a| a[0] == e.id) {
            out.push(if kind == "enum" { format!("  {}", a[1]) } else { format!("  {}{}{}", sign(&a[3]), a[1], if a[2].is_empty() { String::new() } else if mermaid { format!(" {}", a[2]) } else { format!(" : {}", a[2]) }) });
        }
        for o in operations.iter().filter(|o| o[0] == e.id) {
            out.push(format!("  {}{}(){}", sign(&o[3]), o[1], if o[2].is_empty() { String::new() } else { format!(" {}", o[2]) }));
        }
        out.push("}".into());
    }
    for l in links {
        let (a, b) = (&names[&l.source], &names[&l.target]);
        let arrow = match (l.kind.as_str(), l.subtype.as_str()) {
            ("Generalization", _) => "--|>",
            ("Realisation" | "Realization", _) => "..|>",
            ("Aggregation", "Strong") => "--*",
            ("Aggregation", _) => "--o",
            ("Dependency" | "Usage", _) => "..>",
            _ => "-->",
        };
        out.push(format!("{a} {arrow} {b}{}", if l.name.is_empty() { String::new() } else { format!(" : {}", quote(&l.name)) }));
    }
    if !mermaid {
        out.push("@enduml".into());
    }
    Ok(out.join("\n"))
}

fn sequence_text(title: &str, elements: &[Element], links: &[Link], names: &HashMap<String, String>, mermaid: bool) -> String {
    let mut out = vec![if mermaid { "sequenceDiagram".to_string() } else { format!("@startuml\ntitle {}", quote(title)) }];
    // Participants left to right as on the diagram (the query sorted by position).
    let mut ordered: Vec<&Element> = elements.iter().collect();
    ordered.sort_by_key(|e| links.iter().position(|l| l.source == e.id || l.target == e.id).unwrap_or(usize::MAX));
    for e in ordered {
        let word = if e.kind == "Actor" { "actor" } else { "participant" };
        let shown = if e.name.trim().is_empty() { format!("({})", e.kind) } else { e.name.clone() };
        out.push(if names[&e.id] == e.name {
            format!("{word} {}", names[&e.id])
        } else if mermaid {
            format!("{word} {} as {}", names[&e.id], quote(&shown))
        } else {
            format!("{word} \"{}\" as {}", quote(&shown), names[&e.id])
        });
    }
    let mut sorted: Vec<&Link> = links.iter().collect();
    sorted.sort_by_key(|l| l.sequence);
    for l in sorted {
        let arrow = match (mermaid, l.reply) { (true, true) => "-->>", (true, false) => "->>", (false, true) => "-->", (false, false) => "->" };
        out.push(format!("{} {arrow} {} : {}", names[&l.source], names[&l.target], quote(&l.name)));
    }
    if !mermaid {
        out.push("@enduml".into());
    }
    out.join("\n")
}

fn graph_text(title: &str, elements: &[Element], links: &[Link], names: &HashMap<String, String>, mermaid: bool) -> String {
    let mut out = Vec::new();
    if mermaid {
        out.push("flowchart LR".to_string());
        for e in elements {
            out.push(format!("  {}[\"{}\"]", names[&e.id], quote(&e.name)));
        }
        for l in links {
            let arrow = if matches!(l.kind.as_str(), "Dependency" | "Usage" | "InformationFlow") { "-.->" } else { "-->" };
            out.push(format!("  {} {arrow}{} {}", names[&l.source], if l.name.is_empty() { String::new() } else { format!("|\"{}\"|", quote(&l.name)) }, names[&l.target]));
        }
    } else {
        out.push(format!("@startuml\ntitle {}", quote(title)));
        for e in elements {
            let word = match e.kind.as_str() { "Actor" => "actor", "Component" => "component", "Node" | "Device" | "ExecutionEnvironment" => "node", "UseCase" => "usecase", "Artifact" => "artifact", _ => "rectangle" };
            out.push(format!("{word} \"{}\" as {}", quote(&e.name), names[&e.id]));
        }
        for l in links {
            out.push(format!("{} --> {}{}", names[&l.source], names[&l.target], if l.name.is_empty() { String::new() } else { format!(" : {}", quote(&l.name)) }));
        }
        out.push("@enduml".into());
    }
    out.join("\n")
}

fn c4(title: &str, elements: &[Element], links: &[Link], names: &HashMap<String, String>) -> String {
    let mut out = vec!["@startuml".to_string(), "!include https://raw.githubusercontent.com/plantuml-stdlib/C4-PlantUML/master/C4_Container.puml".into(), format!("title {}", quote(title))];
    for e in elements {
        let stereo = e.stereotype.to_lowercase();
        let macro_name = match e.kind.as_str() {
            "Actor" => "Person",
            _ if stereo.contains("person") || stereo.contains("businessactor") => "Person",
            _ if stereo.contains("database") || stereo.contains("datastore") || e.kind == "Table" => "ContainerDb",
            _ if stereo.contains("system") || stereo.contains("external") => "System_Ext",
            "Component" | "Node" | "Device" | "ExecutionEnvironment" | "Artifact" => "Container",
            _ if stereo.contains("applicationcomponent") || stereo.contains("container") => "Container",
            _ => "System",
        };
        out.push(format!("{macro_name}({}, \"{}\")", names[&e.id], quote(&e.name)));
    }
    for l in links {
        out.push(format!("Rel({}, {}, \"{}\")", names[&l.source], names[&l.target], quote(if l.name.is_empty() { &l.kind } else { &l.name })));
    }
    out.push("@enduml".into());
    out.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identifiers_and_class_text() {
        assert_eq!(ident("7", "Order"), "Order");
        assert_eq!(ident("7", "Order line"), "Orderline_7");
        assert_eq!(ident("7", "1st"), "E7");
        let elements = vec![Element { id: "1".into(), name: "Klient".into(), kind: "Actor".into(), stereotype: String::new() }, Element { id: "2".into(), name: "Order service".into(), kind: "Component".into(), stereotype: String::new() }];
        let links = vec![Link { source: "1".into(), target: "2".into(), kind: "Sequence".into(), subtype: String::new(), name: "place()".into(), sequence: 1, reply: false }, Link { source: "2".into(), target: "1".into(), kind: "Sequence".into(), subtype: String::new(), name: "ok".into(), sequence: 2, reply: true }];
        let names: HashMap<String, String> = elements.iter().map(|e| (e.id.clone(), ident(&e.id, &e.name))).collect();
        let m = sequence_text("T", &elements, &links, &names, true);
        assert!(m.starts_with("sequenceDiagram\nactor Klient\nparticipant Orderservice_2 as Order service\nKlient ->> Orderservice_2 : place()\nOrderservice_2 -->> Klient : ok"), "{m}");
        let c = c4("T", &elements, &links[..1], &names);
        assert!(c.contains("Person(Klient, \"Klient\")") && c.contains("Container(Orderservice_2, \"Order service\")") && c.contains("Rel(Klient, Orderservice_2, \"place()\")"), "{c}");
    }

    /// `LOONAR_TEST_PROPS=… cargo test diagram_as_text_live -- --ignored --nocapture`
    #[test]
    #[ignore = "requires a test model"]
    fn diagram_as_text_live() {
        let cfg = AppConfig::load(std::path::Path::new(&std::env::var("LOONAR_TEST_PROPS").unwrap())).unwrap();
        for (diagram, format) in [(111, "mermaid"), (111, "plantuml")] {
            println!("{}\n", run(&json!({ "diagram": diagram, "format": format }), &cfg).unwrap());
        }
        let class = query_rows(&cfg, "select d.diagram_id from t_diagram d where d.diagram_type = 'Logical' and exists (select 1 from t_diagramobjects x join t_object o on o.object_id = x.object_id where x.diagram_id = d.diagram_id and o.object_type = 'Class') order by d.diagram_id").unwrap()[0][0].clone();
        let text = run(&json!({ "diagram": class.parse::<i64>().unwrap(), "format": "mermaid" }), &cfg).unwrap();
        println!("{text}");
        assert!(text.contains("classDiagram"));
    }
}

//! Diagram editor: an Enterprise Architect diagram opened in an editor window (the maxGraph editor bundled in
//! `assets/editor`, shown in WebView2), where the user moves, resizes, aligns and renames elements, adds new ones
//! from the toolbox and connects them. The changes come back as a list and are saved by EA through the ordinary
//! operations (`set_bounds`, `update_element`, `route_connector`, `create_element`, `place`, `create_connector`).
//!
//! The window runs in its own process (`--edit-diagram`), so its event loop does not meet the main window's one.

use crate::config::AppConfig;
use crate::tools::query_rows;
use anyhow::{bail, Context, Result};
use serde_json::{json, Value};

const HTML: &str = include_str!("../assets/editor/editor.html");
const EDITOR_JS: &str = include_str!("../assets/editor/editor.js");

/// BPMN element stereotypes: a diagram with them gets the BPMN toolbox.
const BPMN_STEREOTYPES: &[&str] = &["StartEvent", "EndEvent", "IntermediateEvent", "Activity", "Gateway", "Pool", "Lane"];

fn visibility(scope: &str) -> &'static str {
    match scope.to_ascii_lowercase().as_str() {
        "private" => "-",
        "protected" => "#",
        "package" => "~",
        _ => "+",
    }
}

/// The diagram as the editor needs it: elements with their boxes (pixels from the top-left corner, Y downwards),
/// class members, connectors between elements on the diagram with their bend points, and the toolbox.
pub fn model(cfg: &AppConfig, diagram_id: i64) -> Result<Value> {
    let d = query_rows(cfg, &format!("select diagram_id, name, diagram_type, package_id from t_diagram where diagram_id = {diagram_id}"))?;
    let d = d.first().with_context(|| crate::i18n::fmt("there is no diagram with id {0}", &[diagram_id.to_string()]))?;
    let on_diagram = format!("(select object_id from t_diagramobjects where diagram_id = {diagram_id})");
    let int = |s: &str| s.trim().parse::<i64>().unwrap_or(0);

    let mut attributes: std::collections::HashMap<i64, Vec<String>> = Default::default();
    for r in query_rows(cfg, &format!("select object_id, name, coalesce(type, ''), coalesce(scope, '') from t_attribute where object_id in {on_diagram} order by object_id, pos"))? {
        let kind = if r[2].is_empty() { String::new() } else { format!(": {}", r[2]) };
        attributes.entry(int(&r[0])).or_default().push(format!("{} {}{kind}", visibility(&r[3]), r[1]));
    }
    let mut operations: std::collections::HashMap<i64, Vec<String>> = Default::default();
    for r in query_rows(cfg, &format!("select object_id, name, coalesce(type, ''), coalesce(scope, '') from t_operation where object_id in {on_diagram} order by object_id, pos"))? {
        let kind = if r[2].is_empty() || r[2] == "void" { String::new() } else { format!(": {}", r[2]) };
        operations.entry(int(&r[0])).or_default().push(format!("{} {}(){kind}", visibility(&r[3]), r[1]));
    }

    let mut elements = Vec::new();
    let mut bpmn = false;
    for r in query_rows(cfg, &format!(
        "select o.object_id, o.name, o.object_type, coalesce(o.stereotype, ''), o.parentid, d.rectleft, d.recttop, d.rectright, d.rectbottom \
         from t_diagramobjects d join t_object o on o.object_id = d.object_id where d.diagram_id = {diagram_id} order by d.sequence"
    ))? {
        let id = int(&r[0]);
        let (left, top, right, bottom) = (int(&r[5]), int(&r[6]).abs(), int(&r[7]), int(&r[8]).abs());
        bpmn |= BPMN_STEREOTYPES.contains(&r[3].as_str());
        elements.push(json!({
            "id": id, "name": r[1], "type": r[2], "stereotype": r[3], "parent": int(&r[4]),
            "left": left, "top": top.min(bottom), "width": (right - left).abs(), "height": (bottom - top).abs(),
            "attributes": attributes.remove(&id).unwrap_or_default(),
            "operations": operations.remove(&id).unwrap_or_default(),
        }));
    }

    let mut hidden = std::collections::HashSet::new();
    let mut paths = std::collections::HashMap::new();
    for r in query_rows(cfg, &format!("select connectorid, coalesce(path, ''), coalesce(hidden, 0) from t_diagramlinks where diagramid = {diagram_id}"))? {
        if r[2] == "1" || r[2].eq_ignore_ascii_case("true") {
            hidden.insert(int(&r[0]));
        }
        paths.insert(int(&r[0]), r[1].clone());
    }
    let mut connectors = Vec::new();
    for r in query_rows(cfg, &format!(
        "select connector_id, connector_type, coalesce(stereotype, ''), coalesce(name, ''), start_object_id, end_object_id, coalesce(subtype, '') \
         from t_connector where start_object_id in {on_diagram} and end_object_id in {on_diagram}"
    ))? {
        let id = int(&r[0]);
        if hidden.contains(&id) {
            continue;
        }
        connectors.push(json!({
            "id": id, "type": r[1], "stereotype": r[2], "name": r[3], "source": int(&r[4]), "target": int(&r[5]), "subtype": r[6],
            "points": parse_path(paths.get(&id).map(String::as_str).unwrap_or("")),
        }));
    }

    let (toolbox, connector_types) = toolbox(&d[2], bpmn);
    Ok(json!({
        "diagram": { "id": diagram_id, "name": d[1], "type": d[2], "package": int(&d[3]), "bpmn": bpmn },
        "elements": elements,
        "connectors": connectors,
        "toolbox": toolbox,
        "connector_types": connector_types,
        "texts": texts(),
        "selftest": cfg!(debug_assertions) && std::env::var("LOONAR_EDITOR_SELFTEST").is_ok(),
    }))
}

/// Bend points of a connector from `t_diagramlinks.path` ("x:y;x:y;", Y stored negative) as [[x, y], ...] with Y downwards.
fn parse_path(path: &str) -> Vec<[i64; 2]> {
    path.split(';')
        .filter_map(|p| {
            let (x, y) = p.split_once(':')?;
            Some([x.trim().parse().ok()?, y.trim().parse::<i64>().ok()?.abs()])
        })
        .collect()
}

/// Elements and connector types offered for adding, by the kind of diagram.
fn toolbox(diagram_type: &str, bpmn: bool) -> (Value, Value) {
    let tool = |label: &str, name: &str, kind: &str, stereotype: &str, w: i64, h: i64| {
        json!({ "label": label, "name": name, "type": kind, "stereotype": stereotype, "width": w, "height": h })
    };
    let conn = |label: &str, kind: &str, stereotype: &str| json!({ "label": label, "type": kind, "stereotype": stereotype });
    let tr = crate::i18n::tr;
    if bpmn || diagram_type.starts_with("BPMN") {
        return (
            json!([
                tool(tr("Task"), tr("New task"), "Activity", "BPMN2.0::Activity", 110, 60),
                tool(tr("Start event"), "", "Event", "BPMN2.0::StartEvent", 30, 30),
                tool(tr("End event"), "", "Event", "BPMN2.0::EndEvent", 30, 30),
                tool(tr("Gateway"), "", "Decision", "BPMN2.0::Gateway", 40, 40),
            ]),
            json!([conn(tr("Sequence flow"), "ControlFlow", "BPMN2.0::SequenceFlow")]),
        );
    }
    match diagram_type {
        "Use Case" => (
            json!([tool(tr("Actor"), tr("New actor"), "Actor", "", 40, 80), tool(tr("Use case"), tr("New use case"), "UseCase", "", 150, 70), tool(tr("Note"), "", "Note", "", 160, 60)]),
            json!([conn(tr("Association"), "Association", ""), conn("«include»", "Dependency", "include"), conn("«extend»", "Dependency", "extend")]),
        ),
        "Activity" => (
            json!([tool(tr("Action"), tr("New action"), "Action", "", 110, 50), tool(tr("Decision"), "", "Decision", "", 40, 40), tool(tr("Note"), "", "Note", "", 160, 60)]),
            json!([conn(tr("Control flow"), "ControlFlow", "")]),
        ),
        "Component" => (
            json!([tool(tr("Component"), tr("New component"), "Component", "", 160, 80), tool(tr("Interface"), tr("New interface"), "Interface", "", 160, 80), tool(tr("Note"), "", "Note", "", 160, 60)]),
            json!([conn(tr("Dependency"), "Dependency", ""), conn(tr("Realization"), "Realisation", ""), conn(tr("Association"), "Association", "")]),
        ),
        _ => (
            json!([tool(tr("Class"), tr("New class"), "Class", "", 160, 90), tool(tr("Interface"), tr("New interface"), "Interface", "", 160, 80), tool(tr("Note"), "", "Note", "", 160, 60)]),
            json!([
                conn(tr("Association"), "Association", ""),
                conn(tr("Generalization"), "Generalization", ""),
                conn(tr("Aggregation"), "Aggregation", ""),
                conn(tr("Dependency"), "Dependency", ""),
                conn(tr("Realization"), "Realisation", ""),
            ]),
        ),
    }
}

/// Interface texts of the editor window in the current language.
fn texts() -> Value {
    let tr = crate::i18n::tr;
    json!({
        "add": tr("Add:"), "connector": tr("Connector:"), "save": tr("Save to EA"), "cancel": tr("Cancel"),
        "hint": tr("Drag from an element to another to connect them. Double-click to rename."),
        "nothing": tr("Nothing has changed."), "add_hint": tr("Adds the element in the middle of the view"),
        "align_left": tr("Align left"), "align_center": tr("Align centers"), "align_top": tr("Align tops"),
        "align_middle": tr("Align middles"), "same_size": tr("Same size as the first selected"), "fit": tr("Fit to window"),
    })
}

/// Turns the editor's changes into EA operations.
pub fn to_ops(model: &Value, changes: &Value) -> Vec<Value> {
    let diagram = model["diagram"]["id"].clone();
    let package = model["diagram"]["package"].clone();
    let list = |k: &str| changes[k].as_array().cloned().unwrap_or_default();
    let mut ops = Vec::new();
    for c in list("created") {
        let key = c["key"].as_str().unwrap_or("new").to_string();
        let mut op = json!({ "op": "create_element", "key": key, "package": package, "name": c["name"], "type": c["type"] });
        if c["stereotype"].as_str().is_some_and(|s| !s.is_empty()) {
            op["stereotype"] = c["stereotype"].clone();
        }
        ops.push(op);
        ops.push(json!({ "op": "place", "diagram": diagram, "element": format!("#{key}"), "left": c["left"], "top": c["top"], "width": c["width"], "height": c["height"] }));
    }
    for c in list("connected") {
        let mut op = json!({ "op": "create_connector", "source": c["source"], "target": c["target"], "type": c["type"] });
        for field in ["stereotype", "name"] {
            if c[field].as_str().is_some_and(|s| !s.is_empty()) {
                op[field] = c[field].clone();
            }
        }
        ops.push(op);
    }
    for c in list("moved") {
        ops.push(json!({ "op": "set_bounds", "diagram": diagram, "element": c["id"], "left": c["left"], "top": c["top"], "width": c["width"], "height": c["height"] }));
    }
    for c in list("renamed") {
        ops.push(json!({ "op": "update_element", "element": c["id"], "name": c["name"] }));
    }
    for c in list("routed") {
        ops.push(json!({ "op": "route_connector", "diagram": diagram, "connector": c["id"], "points": c["points"] }));
    }
    ops
}

/// Opens the editor window and waits until the user saves or closes it. Returns the changes, or None.
pub fn run(model: &Value) -> Result<Option<Value>> {
    use std::{cell::RefCell, rc::Rc};
    use tao::event::{Event, WindowEvent};
    use tao::event_loop::{ControlFlow, EventLoopBuilder};
    use tao::platform::run_return::EventLoopExtRunReturn;

    let page = HTML
        .replace("/*DIAGRAM*/", &format!("window.__DIAGRAM__ = {};", serde_json::to_string(model)?.replace("</", "<\\/")))
        .replace("/*EDITOR_JS*/", &EDITOR_JS.replace("</script", "<\\/script"));
    let mut event_loop = EventLoopBuilder::<String>::with_user_event().build();
    let proxy = event_loop.create_proxy();
    let title = format!("{} – {}", model["diagram"]["name"].as_str().unwrap_or(""), crate::config::APP_NAME);
    let window = tao::window::WindowBuilder::new()
        .with_title(title)
        .with_inner_size(tao::dpi::LogicalSize::new(1280.0, 820.0))
        .build(&event_loop)
        .context(crate::i18n::tr("opening the editor window"))?;
    let _webview = wry::WebViewBuilder::new()
        .with_html(page)
        .with_ipc_handler(move |request| {
            let _ = proxy.send_event(request.body().clone());
        })
        .build(&window)
        .context(crate::i18n::tr("WebView2 (Microsoft Edge) is needed for the diagram editor"))?;
    let result: Rc<RefCell<Option<Value>>> = Rc::new(RefCell::new(None));
    let out = result.clone();
    event_loop.run_return(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;
        match event {
            Event::UserEvent(message) => {
                if let Ok(value) = serde_json::from_str::<Value>(&message) {
                    if value["action"] == "save" {
                        *out.borrow_mut() = Some(value);
                    }
                }
                *control_flow = ControlFlow::Exit;
            }
            Event::WindowEvent { event: WindowEvent::CloseRequested, .. } => *control_flow = ControlFlow::Exit,
            _ => {}
        }
    });
    let changes = result.borrow_mut().take();
    Ok(changes)
}

/// `--edit-diagram <id>`: opens the diagram from the model in the settings, and saves the changes through EA.
pub fn run_from_command_line(cfg: &AppConfig, diagram_id: i64) -> Result<String> {
    let model = model(cfg, diagram_id)?;
    let Some(changes) = run(&model)? else { return Ok("cancelled".into()) };
    let ops = to_ops(&model, &changes);
    if ops.is_empty() {
        bail!("no changes");
    }
    let result = crate::ea::run_batch(cfg, &ops, &crate::ea::BatchOptions::default())?;
    crate::ea::shutdown();
    Ok(format!("{} operations saved: {}", ops.len(), result["created"]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn changes_become_ea_operations() {
        let model = json!({ "diagram": { "id": 8, "package": 11 } });
        let changes = json!({
            "moved": [{ "id": 24, "left": 40, "top": 60, "width": 170, "height": 110 }],
            "renamed": [{ "id": 25, "name": "PremiumCustomer" }],
            "routed": [{ "id": 3, "points": [[100, 200]] }],
            "created": [{ "key": "n1", "name": "Invoice", "type": "Class", "stereotype": "", "left": 300, "top": 300, "width": 160, "height": 90 }],
            "connected": [{ "type": "Association", "stereotype": "", "name": "bills", "source": 26, "target": "#n1" }],
        });
        let ops = to_ops(&model, &changes);
        let names: Vec<&str> = ops.iter().map(|o| o["op"].as_str().unwrap()).collect();
        assert_eq!(names, ["create_element", "place", "create_connector", "set_bounds", "update_element", "route_connector"]);
        assert_eq!(ops[0]["package"], 11);
        assert!(ops[0].get("stereotype").is_none(), "an empty stereotype is not sent");
        assert_eq!(ops[1]["element"], "#n1");
        assert_eq!(ops[2]["target"], "#n1");
        assert_eq!(ops[2]["name"], "bills");
    }

    #[test]
    fn bend_points_come_with_y_downwards() {
        assert_eq!(parse_path("120:-80;300:-80;"), vec![[120, 80], [300, 80]]);
        assert!(parse_path("").is_empty());
    }

    /// The diagram from an EA file as the editor gets it. Run with LOONAR_TEST_QEA (and LOONAR_TEST_DIAGRAM, default 8):
    /// `cargo test diagram_model_from_file -- --ignored --nocapture`.
    #[test]
    #[ignore = "requires LOONAR_TEST_QEA"]
    fn diagram_model_from_file() {
        let file = std::env::var("LOONAR_TEST_QEA").expect("LOONAR_TEST_QEA");
        let dir = std::env::temp_dir().join(format!("loonar-editor-model-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("app.properties"), format!("model.source=file\r\nmodel.file={file}\r\n")).unwrap();
        let cfg = AppConfig::load(&dir.join("app.properties")).unwrap();
        let id: i64 = std::env::var("LOONAR_TEST_DIAGRAM").ok().and_then(|v| v.parse().ok()).unwrap_or(8);
        let m = model(&cfg, id).unwrap();
        println!("{}", serde_json::to_string_pretty(&m["elements"]).unwrap());
        println!("connectors: {}", m["connectors"]);
        println!("toolbox: {}", m["toolbox"]);
        assert!(!m["elements"].as_array().unwrap().is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }
}

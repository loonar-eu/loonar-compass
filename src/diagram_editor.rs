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

/// At most this many elements of the model are offered for putting on the diagram.
const MAX_CANDIDATES: usize = 20000;

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

/// EA visibility as the write operations take it.
fn scope_name(scope: &str) -> &'static str {
    match scope.to_ascii_lowercase().as_str() {
        "private" => "Private",
        "protected" => "Protected",
        "package" => "Package",
        _ => "Public",
    }
}

/// Parameters typed as "name: Type, other: Type" (the type may be left out).
fn parse_parameters(text: &str) -> Vec<Value> {
    text.split(',')
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .map(|p| match p.split_once(':') {
            Some((name, kind)) => json!({ "name": name.trim(), "type": kind.trim() }),
            None => json!({ "name": p, "type": "" }),
        })
        .collect()
}

/// EA colour (a BGR number, -1 or empty = default) as #RRGGBB, or "" for the default colour.
fn hex_color(value: &str) -> String {
    match value.trim().parse::<i64>() {
        Ok(n) if n >= 0 => format!("#{:02X}{:02X}{:02X}", n & 0xFF, (n >> 8) & 0xFF, (n >> 16) & 0xFF),
        _ => String::new(),
    }
}

/// One value of a `key=value;` style string (diagram object and link styles in EA).
fn style_value<'a>(style: &'a str, key: &str) -> Option<&'a str> {
    style.split(';').find_map(|p| p.strip_prefix(key)?.strip_prefix('='))
}

/// The line style name of a diagram link (the names of the `set_line_style` operation).
fn line_style(style: &str) -> &'static str {
    match (style_value(style, "Mode").unwrap_or(""), style_value(style, "TREE").unwrap_or("")) {
        ("1", _) => "direct",
        ("2", _) => "auto",
        ("3", "OS") => "orthogonal_square",
        ("3", "OR") => "orthogonal_rounded",
        ("3", "V") => "tree_vertical",
        ("3", "H") => "tree_horizontal",
        ("3", "LV") => "lateral_vertical",
        ("3", "LH") => "lateral_horizontal",
        ("3", _) => "custom",
        _ => "",
    }
}

/// Tagged values of objects, with the allowed values when the profile defines them ("Values: a,b,c" in the notes).
fn tags(cfg: &AppConfig, sql: &str) -> Result<std::collections::HashMap<i64, Vec<Value>>> {
    let mut out: std::collections::HashMap<i64, Vec<Value>> = Default::default();
    for r in query_rows(cfg, sql)? {
        let values: Vec<String> = r[3]
            .lines()
            .find_map(|l| l.trim().strip_prefix("Values:"))
            .map(|v| v.split(',').map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect())
            .unwrap_or_default();
        out.entry(r[0].trim().parse().unwrap_or(0)).or_default().push(json!({ "name": r[1], "value": r[2], "values": values }));
    }
    Ok(out)
}

/// The diagram as the editor needs it: elements with their boxes (pixels from the top-left corner, Y downwards),
/// class members, connectors between elements on the diagram with their bend points, and the toolbox.
pub fn model(cfg: &AppConfig, diagram_id: i64) -> Result<Value> {
    let d = query_rows(cfg, &format!("select diagram_id, name, diagram_type, package_id from t_diagram where diagram_id = {diagram_id}"))?;
    let d = d.first().with_context(|| crate::i18n::fmt("there is no diagram with id {0}", &[diagram_id.to_string()]))?;
    let on_diagram = format!("(select object_id from t_diagramobjects where diagram_id = {diagram_id})");
    let int = |s: &str| s.trim().parse::<i64>().unwrap_or(0);

    // Class members: the text shown in the compartments, and the fields the properties panel edits.
    let mut attributes: std::collections::HashMap<i64, Vec<String>> = Default::default();
    let mut attribute_list: std::collections::HashMap<i64, Vec<Value>> = Default::default();
    for r in query_rows(cfg, &format!("select object_id, name, type, scope, id, \"default\", isstatic from t_attribute where object_id in {on_diagram} order by object_id, pos"))? {
        let kind = if r[2].is_empty() { String::new() } else { format!(": {}", r[2]) };
        attributes.entry(int(&r[0])).or_default().push(format!("{} {}{kind}", visibility(&r[3]), r[1]));
        attribute_list.entry(int(&r[0])).or_default().push(json!({
            "id": int(&r[4]), "name": r[1], "type": r[2], "visibility": scope_name(&r[3]), "default": r[5], "static": r[6] == "1",
        }));
    }
    let mut parameters: std::collections::HashMap<i64, Vec<String>> = Default::default();
    for r in query_rows(cfg, &format!(
        "select operationid, name, type from t_operationparams where operationid in (select operationid from t_operation where object_id in {on_diagram}) order by operationid, pos"
    ))? {
        let kind = if r[2].is_empty() { String::new() } else { format!(": {}", r[2]) };
        parameters.entry(int(&r[0])).or_default().push(format!("{}{kind}", r[1]));
    }
    let mut operations: std::collections::HashMap<i64, Vec<String>> = Default::default();
    let mut operation_list: std::collections::HashMap<i64, Vec<Value>> = Default::default();
    for r in query_rows(cfg, &format!("select object_id, name, type, scope, operationid, isstatic from t_operation where object_id in {on_diagram} order by object_id, pos"))? {
        let kind = if r[2].is_empty() || r[2] == "void" { String::new() } else { format!(": {}", r[2]) };
        let params = parameters.remove(&int(&r[4])).unwrap_or_default().join(", ");
        operations.entry(int(&r[0])).or_default().push(format!("{} {}({params}){kind}", visibility(&r[3]), r[1]));
        operation_list.entry(int(&r[0])).or_default().push(json!({
            "id": int(&r[4]), "name": r[1], "type": r[2], "visibility": scope_name(&r[3]), "parameters": params, "static": r[5] == "1",
        }));
    }

    // Where the elements are used: the other diagrams they are on, and how many relationships they have in the model
    // (shown before an element is deleted from the model).
    let mut on_diagrams: std::collections::HashMap<i64, Vec<(i64, String)>> = Default::default();
    for r in query_rows(cfg, &format!(
        "select o.object_id, d.diagram_id, d.name from t_diagramobjects o join t_diagram d on d.diagram_id = o.diagram_id where o.object_id in {on_diagram}"
    ))? {
        on_diagrams.entry(int(&r[0])).or_default().push((int(&r[1]), r[2].clone()));
    }
    let mut relations: std::collections::HashMap<i64, i64> = Default::default();
    for r in query_rows(cfg, &format!("select start_object_id, end_object_id from t_connector where start_object_id in {on_diagram} or end_object_id in {on_diagram}"))? {
        *relations.entry(int(&r[0])).or_default() += 1;
        if r[1] != r[0] {
            *relations.entry(int(&r[1])).or_default() += 1;
        }
    }
    let other_diagrams = |id: i64| -> Vec<String> {
        on_diagrams.get(&id).map(|v| v.iter().filter(|(d, _)| *d != diagram_id).map(|(_, n)| n.clone()).collect()).unwrap_or_default()
    };

    let mut element_tags = tags(cfg, &format!("select object_id, property, value, notes from t_objectproperties where object_id in {on_diagram} order by object_id, property"))?;
    let mut elements = Vec::new();
    let mut bpmn = false;
    for r in query_rows(cfg, &format!(
        "select o.object_id, o.name, o.object_type, o.stereotype, o.parentid, d.rectleft, d.recttop, d.rectright, d.rectbottom, \
         o.alias, o.note, o.pdata5, o.status, o.version, o.author, o.phase, o.complexity, o.abstract, o.ea_guid, \
         o.createddate, o.modifieddate, o.backcolor, d.objectstyle, p.name \
         from t_diagramobjects d join t_object o on o.object_id = d.object_id left join t_package p on p.package_id = o.package_id \
         where d.diagram_id = {diagram_id} order by d.sequence"
    ))? {
        let id = int(&r[0]);
        let (left, top, right, bottom) = (int(&r[5]), int(&r[6]).abs(), int(&r[7]), int(&r[8]).abs());
        bpmn |= BPMN_STEREOTYPES.contains(&r[3].as_str());
        elements.push(json!({
            "id": id, "name": r[1], "type": r[2], "stereotype": r[3], "parent": int(&r[4]),
            "left": left, "top": top.min(bottom), "width": (right - left).abs(), "height": (bottom - top).abs(),
            "attributes": attributes.remove(&id).unwrap_or_default(),
            "operations": operations.remove(&id).unwrap_or_default(),
            "attribute_list": attribute_list.remove(&id).unwrap_or_default(),
            "operation_list": operation_list.remove(&id).unwrap_or_default(),
            "other_diagrams": other_diagrams(id),
            "relations": relations.get(&id).copied().unwrap_or(0),
            "props": {
                "alias": r[9], "notes": r[10], "keywords": r[11], "status": r[12], "version": r[13], "author": r[14],
                "phase": r[15], "complexity": r[16], "abstract": r[17] == "1", "guid": r[18],
                "created": r[19].chars().take(19).collect::<String>(), "modified": r[20].chars().take(19).collect::<String>(),
                "color": hex_color(&r[21]), "bgcolor": style_value(&r[22], "BCol").map(hex_color).unwrap_or_default(),
                "package": r[23],
            },
            "tags": element_tags.remove(&id).unwrap_or_default(),
        }));
    }

    let mut hidden = std::collections::HashSet::new();
    let mut paths = std::collections::HashMap::new();
    let mut link_styles = std::collections::HashMap::new();
    for r in query_rows(cfg, &format!("select connectorid, path, hidden, style from t_diagramlinks where diagramid = {diagram_id}"))? {
        if r[2] == "1" || r[2].eq_ignore_ascii_case("true") {
            hidden.insert(int(&r[0]));
        }
        paths.insert(int(&r[0]), r[1].clone());
        link_styles.insert(int(&r[0]), line_style(&r[3]));
    }
    let both_ends = format!("start_object_id in {on_diagram} and end_object_id in {on_diagram}");
    let mut connector_tags = tags(cfg, &format!(
        "select elementid, property, value, notes from t_connectortag where elementid in (select connector_id from t_connector where {both_ends}) order by elementid, property"
    ))?;
    let mut connectors = Vec::new();
    for r in query_rows(cfg, &format!(
        "select connector_id, connector_type, stereotype, name, start_object_id, end_object_id, subtype, \
         direction, notes, sourcecard, sourcerole, destcard, destrole, ea_guid, seqno, ptstarty, pdata1, pdata4, diagramid \
         from t_connector where {both_ends}"
    ))? {
        // Messages of a sequence diagram belong to that diagram only (the same lifelines can be on several).
        if r[1] == "Sequence" && int(&r[18]) != 0 && int(&r[18]) != diagram_id {
            continue;
        }
        let id = int(&r[0]);
        if hidden.contains(&id) {
            continue;
        }
        // Other diagrams that show this relationship: both ends are on them.
        let (source, target) = (int(&r[4]), int(&r[5]));
        let ends: Vec<String> = other_diagrams(source).into_iter().filter(|d| other_diagrams(target).contains(d)).collect();
        connectors.push(json!({
            "id": id, "type": r[1], "stereotype": r[2], "name": r[3], "source": source, "target": target, "subtype": r[6],
            "other_diagrams": ends,
            "points": parse_path(paths.get(&id).map(String::as_str).unwrap_or("")),
            "props": {
                "direction": r[7], "notes": r[8], "source_multiplicity": r[9], "source_role": r[10],
                "target_multiplicity": r[11], "target_role": r[12], "guid": r[13],
                "line_style": link_styles.get(&id).copied().unwrap_or(""),
                // Sequence messages: number, position from the top (0 = not laid out yet), kind, reply.
                "seq": int(&r[14]), "y": int(&r[15]).abs(), "sort": r[16], "return": r[17] == "1",
            },
            "tags": connector_tags.remove(&id).unwrap_or_default(),
        }));
    }

    let (toolbox, connector_types) = toolbox(&d[2], bpmn);
    // Elements of the model that can be put on the diagram (those of the diagram's package first).
    let package_id = int(&d[3]);
    let candidates: Vec<Value> = query_rows(cfg, &format!(
        "select o.object_id, o.name, o.object_type, o.stereotype, p.name from t_object o left join t_package p on p.package_id = o.package_id \
         where o.object_type not in ('Package', 'Text', 'Boundary', 'ProvidedInterface', 'RequiredInterface', 'Port', 'ActionPin', 'ExpansionNode') \
         and o.name is not null and o.name <> '' and o.object_id not in {on_diagram} \
         order by case when o.package_id = {package_id} then 0 else 1 end, o.name"
    ))?
    .into_iter()
    .take(MAX_CANDIDATES)
    .map(|r| json!({ "id": int(&r[0]), "name": r[1], "type": r[2], "stereotype": r[3], "package": r[4] }))
    .collect();
    let statuses: Vec<String> = query_rows(cfg, "select status from t_statustypes order by status").unwrap_or_default().into_iter().map(|r| r[0].clone()).collect();
    Ok(json!({
        "statuses": statuses,
        "candidates": candidates,
        "diagram": { "id": diagram_id, "name": d[1], "type": d[2], "package": int(&d[3]), "bpmn": bpmn },
        "elements": elements,
        "connectors": connectors,
        "toolbox": toolbox,
        "connector_types": connector_types,
        "texts": texts(),
        // Scripted edits in debug builds: LOONAR_EDITOR_SELFTEST=1 (layout and new elements) or =props (the properties panel).
        "selftest": if cfg!(debug_assertions) { std::env::var("LOONAR_EDITOR_SELFTEST").unwrap_or_default() } else { String::new() },
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
    if bpmn || diagram_type.starts_with("BPMN") {
        return (
            json!([
                tool(crate::i18n::tr("Task"), crate::i18n::tr("New task"), "Activity", "BPMN2.0::Activity", 110, 60),
                tool(crate::i18n::tr("Start event"), "", "Event", "BPMN2.0::StartEvent", 30, 30),
                tool(crate::i18n::tr("End event"), "", "Event", "BPMN2.0::EndEvent", 30, 30),
                tool(crate::i18n::tr("Gateway"), "", "Decision", "BPMN2.0::Gateway", 40, 40),
            ]),
            json!([conn(crate::i18n::tr("Sequence flow"), "ControlFlow", "BPMN2.0::SequenceFlow")]),
        );
    }
    match diagram_type {
        "Sequence" => (
            json!([
                tool(crate::i18n::tr("Lifeline"), crate::i18n::tr("New lifeline"), "Sequence", "", 120, 400),
                tool(crate::i18n::tr("Actor"), crate::i18n::tr("New actor"), "Actor", "", 120, 400),
                tool(crate::i18n::tr("Note"), "", "Note", "", 160, 60),
            ]),
            json!([
                { "label": crate::i18n::tr("Message"), "type": "Sequence", "stereotype": "", "sort": "Synchronous", "reply": false },
                { "label": crate::i18n::tr("Asynchronous message"), "type": "Sequence", "stereotype": "", "sort": "Asynchronous", "reply": false },
                { "label": crate::i18n::tr("Reply"), "type": "Sequence", "stereotype": "", "sort": "Synchronous", "reply": true },
            ]),
        ),
        "Use Case" => (
            json!([tool(crate::i18n::tr("Actor"), crate::i18n::tr("New actor"), "Actor", "", 40, 80), tool(crate::i18n::tr("Use case"), crate::i18n::tr("New use case"), "UseCase", "", 150, 70), tool(crate::i18n::tr("Note"), "", "Note", "", 160, 60)]),
            json!([conn(crate::i18n::tr("Association"), "Association", ""), conn("«include»", "Dependency", "include"), conn("«extend»", "Dependency", "extend")]),
        ),
        "Activity" => (
            json!([tool(crate::i18n::tr("Action"), crate::i18n::tr("New action"), "Action", "", 110, 50), tool(crate::i18n::tr("Decision"), "", "Decision", "", 40, 40), tool(crate::i18n::tr("Note"), "", "Note", "", 160, 60)]),
            json!([conn(crate::i18n::tr("Control flow"), "ControlFlow", "")]),
        ),
        "Component" => (
            json!([tool(crate::i18n::tr("Component"), crate::i18n::tr("New component"), "Component", "", 160, 80), tool(crate::i18n::tr("Interface"), crate::i18n::tr("New interface"), "Interface", "", 160, 80), tool(crate::i18n::tr("Note"), "", "Note", "", 160, 60)]),
            json!([conn(crate::i18n::tr("Dependency"), "Dependency", ""), conn(crate::i18n::tr("Realization"), "Realisation", ""), conn(crate::i18n::tr("Association"), "Association", "")]),
        ),
        _ => (
            json!([tool(crate::i18n::tr("Class"), crate::i18n::tr("New class"), "Class", "", 160, 90), tool(crate::i18n::tr("Interface"), crate::i18n::tr("New interface"), "Interface", "", 160, 80), tool(crate::i18n::tr("Note"), "", "Note", "", 160, 60)]),
            json!([
                conn(crate::i18n::tr("Association"), "Association", ""),
                conn(crate::i18n::tr("Generalization"), "Generalization", ""),
                conn(crate::i18n::tr("Aggregation"), "Aggregation", ""),
                conn(crate::i18n::tr("Dependency"), "Dependency", ""),
                conn(crate::i18n::tr("Realization"), "Realisation", ""),
            ]),
        ),
    }
}

/// Interface texts of the editor window in the current language.
fn texts() -> Value {
    let mut out = json!({
        "add": crate::i18n::tr("Add:"), "connector": crate::i18n::tr("Connector:"), "save": crate::i18n::tr("Save to EA"), "cancel": crate::i18n::tr("Cancel"),
        "hint": crate::i18n::tr("Drag from an element to another to connect them. Double-click to rename."),
        "nothing": crate::i18n::tr("Nothing has changed."), "add_hint": crate::i18n::tr("Adds the element in the middle of the view"),
        "align_left": crate::i18n::tr("Align left"), "align_center": crate::i18n::tr("Align centers"), "align_top": crate::i18n::tr("Align tops"),
        "align_middle": crate::i18n::tr("Align middles"), "same_size": crate::i18n::tr("Same size as the first selected"), "fit": crate::i18n::tr("Fit to window"),
        "delete_title": crate::i18n::tr("Delete"), "delete_from_diagram": crate::i18n::tr("Remove from this diagram"),
        "delete_from_diagram_hint": crate::i18n::tr("The element stays in the model and on other diagrams."),
        "hide_connector": crate::i18n::tr("Hide on this diagram"), "hide_connector_hint": crate::i18n::tr("The relationship stays in the model and on other diagrams."),
        "delete_from_model": crate::i18n::tr("Delete from the model"),
        "delete_from_model_hint": crate::i18n::tr("Deleted permanently, with its relationships, from every diagram."),
        "used_on": crate::i18n::tr("Also on {0} other diagrams: {1}"), "used_nowhere": crate::i18n::tr("Not on any other diagram."),
        "relations": crate::i18n::tr("Relationships in the model: {0}"), "cancel_delete": crate::i18n::tr("Cancel"),
        "new_removed": crate::i18n::tr("Removed what was added in the editor."),
    });
    // The rest as a list: one json! of this size exceeds the macro recursion limit.
    let more: &[(&str, &str)] = &[
        ("properties", crate::i18n::tr("Properties")), ("tab_general", crate::i18n::tr("General")), ("tab_tags", crate::i18n::tr("Tagged values")),
        ("tab_appearance", crate::i18n::tr("Appearance")),
        ("f_name", crate::i18n::tr("Name")), ("f_alias", crate::i18n::tr("Alias")), ("f_stereotype", crate::i18n::tr("Stereotype")),
        ("f_keywords", crate::i18n::tr("Keywords")), ("f_status", crate::i18n::tr("Status")), ("f_version", crate::i18n::tr("Version")),
        ("f_author", crate::i18n::tr("Author")), ("f_phase", crate::i18n::tr("Phase")), ("f_complexity", crate::i18n::tr("Complexity")),
        ("f_abstract", crate::i18n::tr("Abstract")), ("f_notes", crate::i18n::tr("Notes")), ("f_type", crate::i18n::tr("Type")),
        ("f_package", crate::i18n::tr("Package")), ("f_guid", "GUID"), ("f_created", crate::i18n::tr("Created")), ("f_modified", crate::i18n::tr("Modified")),
        ("easy", crate::i18n::tr("Easy")), ("medium", crate::i18n::tr("Medium")), ("difficult", crate::i18n::tr("Difficult")),
        ("f_direction", crate::i18n::tr("Direction")), ("dir_none", crate::i18n::tr("Unspecified")), ("dir_forward", crate::i18n::tr("Source → target")),
        ("dir_back", crate::i18n::tr("Target → source")), ("dir_both", crate::i18n::tr("Both directions")),
        ("f_source", crate::i18n::tr("Source")), ("f_target", crate::i18n::tr("Target")),
        ("f_source_multiplicity", crate::i18n::tr("Source multiplicity")), ("f_source_role", crate::i18n::tr("Source role")),
        ("f_target_multiplicity", crate::i18n::tr("Target multiplicity")), ("f_target_role", crate::i18n::tr("Target role")),
        ("nothing_selected", crate::i18n::tr("Select an element or a relationship to see its properties.")),
        ("selected_many", crate::i18n::tr("{0} selected")),
        ("scope_model", crate::i18n::tr("model")), ("scope_diagram", crate::i18n::tr("diagram")),
        ("scope_model_hint", crate::i18n::tr("Changes the model: everywhere the element or relationship is shown.")),
        ("scope_diagram_hint", crate::i18n::tr("Changes only this diagram.")),
        ("no_tags", crate::i18n::tr("No tagged values.")), ("tag_name", crate::i18n::tr("Tag name")), ("tag_value", crate::i18n::tr("Value")),
        ("add_tag", crate::i18n::tr("Add tag")), ("remove_tag", crate::i18n::tr("Remove tag")),
        ("fill_diagram", crate::i18n::tr("Fill color on this diagram")), ("fill_model", crate::i18n::tr("Default fill color")),
        ("default_color", crate::i18n::tr("Default")), ("line_style", crate::i18n::tr("Line style on this diagram")),
        ("ls_direct", crate::i18n::tr("Direct")), ("ls_auto", crate::i18n::tr("Automatic")), ("ls_custom", crate::i18n::tr("Custom")),
        ("ls_orthogonal_square", crate::i18n::tr("Orthogonal, square")), ("ls_orthogonal_rounded", crate::i18n::tr("Orthogonal, rounded")),
        ("ls_tree_vertical", crate::i18n::tr("Tree, vertical")), ("ls_tree_horizontal", crate::i18n::tr("Tree, horizontal")),
        ("ls_lateral_vertical", crate::i18n::tr("Lateral, vertical")), ("ls_lateral_horizontal", crate::i18n::tr("Lateral, horizontal")),
        ("after_save", crate::i18n::tr("Available after the relationship is saved.")),
        ("f_order", crate::i18n::tr("Order")), ("earlier", crate::i18n::tr("Earlier")), ("later", crate::i18n::tr("Later")),
        ("f_message_sort", crate::i18n::tr("Kind")), ("synchronous", crate::i18n::tr("Synchronous")), ("asynchronous", crate::i18n::tr("Asynchronous")),
        ("f_is_return", crate::i18n::tr("Reply")), ("operation_of_target", crate::i18n::tr("Operation of the receiver")),
        ("seq_hint", crate::i18n::tr("Messages go from top to bottom in their order: change it with ▲ ▼ in the panel (Alt+↑ / Alt+↓).")),
        ("tab_members", crate::i18n::tr("Attributes and operations")), ("attributes", crate::i18n::tr("Attributes")),
        ("operations", crate::i18n::tr("Operations")), ("none", crate::i18n::tr("None.")), ("visibility", crate::i18n::tr("Visibility")),
        ("return_type", crate::i18n::tr("Return type")), ("parameters", crate::i18n::tr("Parameters")),
        ("parameters_hint", crate::i18n::tr("name: Type, other: Type")),
        ("parameters_existing", crate::i18n::tr("Parameters of an existing operation are changed in EA.")),
        ("add_attribute", crate::i18n::tr("+ Attribute")), ("add_operation", crate::i18n::tr("+ Operation")),
        ("new_attribute", crate::i18n::tr("newAttribute")), ("new_operation", crate::i18n::tr("newOperation")),
        ("remove_member", crate::i18n::tr("Delete from the model")),
        ("insert", crate::i18n::tr("From the model…")), ("insert_title", crate::i18n::tr("Put an element of the model on the diagram")),
        ("insert_search", crate::i18n::tr("Search by name, type or package")),
        ("insert_hint", crate::i18n::tr("Its relationships with the elements of the diagram appear after saving.")),
        ("insert_none", crate::i18n::tr("No matching elements.")), ("insert_more", crate::i18n::tr("Showing the first {0}. Type more to narrow the list.")),
        ("close", crate::i18n::tr("Close")),
        ("tidy", crate::i18n::tr("Tidy with AI")),
        ("tidy_hint", crate::i18n::tr("Saves the changes, closes the editor and asks Claude in the conversation to tidy up the layout.")),
    ];
    if let Some(o) = out.as_object_mut() {
        o.extend(more.iter().map(|(k, v)| (k.to_string(), json!(v))));
    }
    out
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
        if let Some(key) = c["key"].as_str() {
            op["key"] = json!(key);
        }
        // A message of a sequence diagram: on this diagram, with its number, kind and reply flag.
        if c["type"] == "Sequence" {
            op["diagram"] = diagram.clone();
            for field in ["sequence", "message_sort", "is_return"] {
                if !c[field].is_null() {
                    op[field] = c[field].clone();
                }
            }
        }
        for field in ["stereotype", "name"] {
            if c[field].as_str().is_some_and(|s| !s.is_empty()) {
                op[field] = c[field].clone();
            }
        }
        ops.push(op);
    }
    // Existing elements of the model put on this diagram.
    for c in list("placed") {
        ops.push(json!({ "op": "place", "diagram": diagram, "element": c["id"], "left": c["left"], "top": c["top"], "width": c["width"], "height": c["height"] }));
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
    // Properties from the panel: fields of the model, then what applies to this diagram only.
    let with_fields = |op: &str, target: &str, c: &Value| -> Option<Value> {
        let mut out = json!({ "op": op });
        out[target] = c["id"].clone();
        for (k, v) in c["fields"].as_object().into_iter().flatten() {
            out[k] = v.clone();
        }
        if c["tags"].as_object().is_some_and(|t| !t.is_empty()) {
            out["tags"] = c["tags"].clone();
        }
        (out.as_object().map_or(0, |o| o.len()) > 2).then_some(out)
    };
    for c in list("updated") {
        ops.extend(with_fields("update_element", "element", &c));
    }
    for c in list("updated_connectors") {
        ops.extend(with_fields("update_connector", "connector", &c));
    }
    // Attributes and operations: deleted, changed and added (new ones also on elements created in this save).
    for c in list("members") {
        let attribute = c["kind"] == "attribute";
        let (noun, type_field) = if attribute { ("attribute", "type") } else { ("operation", "return_type") };
        match c["action"].as_str().unwrap_or("") {
            "delete" => {
                let mut op = json!({ "op": format!("delete_{noun}") });
                op[noun] = c["id"].clone();
                ops.push(op);
            }
            action @ ("update" | "create") => {
                let mut op = if action == "create" {
                    json!({ "op": format!("create_{noun}"), "element": c["element"] })
                } else {
                    let mut op = json!({ "op": format!("update_{noun}") });
                    op[noun] = c["id"].clone();
                    op
                };
                for field in ["name", "visibility", "default", "static"] {
                    if !c[field].is_null() && (attribute || field != "default") {
                        op[field] = c[field].clone();
                    }
                }
                if !c["type"].is_null() {
                    op[type_field] = c["type"].clone();
                }
                if action == "create" && !attribute {
                    op["parameters"] = json!(parse_parameters(c["parameters"].as_str().unwrap_or("")));
                }
                ops.push(op);
            }
            _ => {}
        }
    }
    // New order of the messages of a sequence diagram (their numbers; EA lays them out by number).
    for c in list("sequence") {
        ops.push(json!({ "op": "update_connector", "connector": c["id"], "sequence": c["sequence"] }));
    }
    for c in list("colors") {
        ops.push(json!({ "op": "set_color", "diagram": diagram, "element": c["id"], "bgcolor": c["bgcolor"] }));
    }
    for c in list("line_styles") {
        ops.push(json!({ "op": "set_line_style", "diagram": diagram, "connectors": [c["id"]], "style": c["style"] }));
    }
    // Removals last: from this diagram only, or from the model (connectors before elements, which take theirs along).
    for id in list("removed") {
        ops.push(json!({ "op": "remove_from_diagram", "diagram": diagram, "element": id }));
    }
    let hidden = list("hidden");
    if !hidden.is_empty() {
        ops.push(json!({ "op": "hide_connectors", "diagram": diagram, "connectors": hidden, "hidden": true }));
    }
    for id in list("deleted_connectors") {
        ops.push(json!({ "op": "delete", "connector": id }));
    }
    for id in list("deleted") {
        ops.push(json!({ "op": "delete", "element": id }));
    }
    ops
}

/// The editor page with the diagram and the editor script inlined.
fn page(model: &Value) -> Result<String> {
    Ok(HTML
        .replace("/*DIAGRAM*/", &format!("window.__DIAGRAM__ = {};", serde_json::to_string(model)?.replace("</", "<\\/")))
        .replace("/*EDITOR_JS*/", &EDITOR_JS.replace("</script", "<\\/script")))
}

/// Opens the editor window and waits until the user saves or closes it. Returns the changes, or None.
pub fn run(model: &Value) -> Result<Option<Value>> {
    use std::{cell::RefCell, rc::Rc};
    use tao::event::{Event, WindowEvent};
    use tao::event_loop::{ControlFlow, EventLoopBuilder};
    use tao::platform::run_return::EventLoopExtRunReturn;

    let page = page(model)?;
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

/// `--diagram-window <input> <output>`: the editor window of the main application. Reads the diagram from the input
/// file and writes the changes to the output file (nothing when the user closes the window without saving).
pub fn window_from_files(input: &std::path::Path, output: &std::path::Path) -> Result<()> {
    let model: Value = serde_json::from_str(&std::fs::read_to_string(input).context(crate::i18n::tr("reading the diagram"))?)?;
    let _ = std::fs::remove_file(output);
    if let Some(changes) = run(&model)? {
        std::fs::write(output, serde_json::to_string(&changes)?).context(crate::i18n::tr("saving the changes"))?;
    }
    Ok(())
}

/// Opens the editor window for a diagram in a separate process of this application and waits until it closes.
/// Returns the diagram and the changes, or None when the window was closed without saving.
pub fn edit_in_window(cfg: &AppConfig, diagram_id: i64) -> Result<Option<(Value, Value)>> {
    let model = model(cfg, diagram_id)?;
    let dir = std::env::temp_dir().join(format!("loonar-editor-{}", uuid::Uuid::new_v4().simple()));
    std::fs::create_dir_all(&dir)?;
    let (input, output) = (dir.join("diagram.json"), dir.join("changes.json"));
    std::fs::write(&input, serde_json::to_string(&model)?)?;
    let exe = std::env::current_exe().context(crate::i18n::tr("application path"))?;
    let status = std::process::Command::new(exe).arg("--diagram-window").arg(&input).arg(&output).status().context(crate::i18n::tr("opening the editor window"))?;
    let changes = std::fs::read_to_string(&output).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok());
    let _ = std::fs::remove_dir_all(&dir);
    if !status.success() && changes.is_none() {
        bail!("{}", crate::i18n::tr("the diagram editor closed with an error"));
    }
    Ok(changes.map(|c| (model, c)))
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
    fn class_members_become_ea_operations() {
        let model = json!({ "diagram": { "id": 8, "package": 11 } });
        let changes = json!({ "members": [
            { "element": 24, "kind": "attribute", "action": "create", "name": "email", "type": "String", "visibility": "Private" },
            { "id": 7, "kind": "attribute", "action": "update", "type": "Text" },
            { "id": 3, "kind": "operation", "action": "delete" },
            { "element": "#n1", "kind": "operation", "action": "create", "name": "total", "type": "Money", "parameters": "tax: Rate, discount" },
        ]});
        let ops = to_ops(&model, &changes);
        assert_eq!(ops[0], json!({ "op": "create_attribute", "element": 24, "name": "email", "type": "String", "visibility": "Private" }));
        assert_eq!(ops[1], json!({ "op": "update_attribute", "attribute": 7, "type": "Text" }));
        assert_eq!(ops[2], json!({ "op": "delete_operation", "operation": 3 }));
        assert_eq!(ops[3]["return_type"], "Money");
        assert_eq!(ops[3]["parameters"], json!([{ "name": "tax", "type": "Rate" }, { "name": "discount", "type": "" }]));
    }

    #[test]
    fn sequence_messages_get_their_diagram_number_and_kind() {
        let model = json!({ "diagram": { "id": 9, "package": 11 } });
        let changes = json!({
            "connected": [{ "key": "n3", "type": "Sequence", "stereotype": "", "name": "pay()", "source": 1, "target": 2,
                            "sequence": 3, "message_sort": "Asynchronous", "is_return": false }],
            "sequence": [{ "id": 40, "sequence": 1 }, { "id": 41, "sequence": 2 }],
        });
        let ops = to_ops(&model, &changes);
        assert_eq!(ops[0]["op"], "create_connector");
        assert_eq!(ops[0]["diagram"], 9);
        assert_eq!(ops[0]["sequence"], 3);
        assert_eq!(ops[0]["message_sort"], "Asynchronous");
        assert_eq!(ops[1], json!({ "op": "update_connector", "connector": 40, "sequence": 1 }));
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
        // The page itself, for looking at it in a browser (LOONAR_EDITOR_PAGE = output file).
        if let Ok(out) = std::env::var("LOONAR_EDITOR_PAGE") {
            std::fs::write(out, page(&m).unwrap()).unwrap();
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}

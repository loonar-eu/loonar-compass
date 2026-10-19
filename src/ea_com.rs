//! Operations on the EA model through COM automation, run in an open `EA.Repository` session.
//! Equivalent of the former ea_bridge.ps1 script, moved into the application: it works without PowerShell,
//! so also in Wine on macOS. The response has the same shape as before:
//! `{ ok, user, security, created: [...], warnings: [...], error, failed_op, rolled_back, session_lost }`.

use crate::com::{b, i, s, Disp};
use anyhow::{anyhow, bail, Result};
use serde_json::{json, Value};
use std::collections::HashMap;

/// Base UML type for MDG stereotypes given as the type by mistake (e.g. type = "BPMN2.0::StartEvent").
const BASE_TYPES: &[(&str, &str)] = &[
    ("Activity", "Activity"),
    ("BusinessProcess", "Activity"),
    ("StartEvent", "Event"),
    ("IntermediateEvent", "Event"),
    ("EndEvent", "Event"),
    ("Gateway", "Decision"),
    ("Pool", "ActivityPartition"),
    ("Lane", "ActivityPartition"),
    ("Group", "Boundary"),
    ("DataObject", "Artifact"),
    ("DataStore", "Artifact"),
    ("TextAnnotation", "Note"),
    ("SequenceFlow", "ControlFlow"),
    ("MessageFlow", "ControlFlow"),
    ("Association", "Dependency"),
];

/// Navigation cell icon (list "Navigation Cells (Web EA)") for the target diagram type.
const NAV_IMAGES: &[(&str, &str)] = &[
    ("Use Case", "2-61"),
    ("Analysis", "2-43"),
    ("BPMN2.0::Business Process", "2-43"),
    ("Package", "2-56"),
    ("Sequence", "2-58"),
    ("Timing", "2-60"),
    ("Activity", "2-51"),
    ("Custom", "2-57"),
    ("Requirement", "2-57"),
    ("Extended::Requirements", "2-57"),
    ("Logical", "2-44"),
    ("Statechart", "2-53"),
    ("Component", "2-46"),
];

/// Opens the model with the account from the settings. Returns the Repository object.
pub fn open(connection: &str, user: &str, password: &str, ea_exe: &std::path::Path) -> Result<Disp> {
    use windows::Win32::System::Com::{CLSIDFromProgID, CoCreateInstance, IDispatch, CLSCTX_LOCAL_SERVER};
    let clsid = unsafe { CLSIDFromProgID(&windows::core::HSTRING::from("EA.Repository")) }
        .map_err(|e| anyhow!("{}", crate::i18n::fmt("Enterprise Architect is not registered in the system (COM): {0}", &[e.to_string()])))?;
    let created: windows::core::Result<IDispatch> = unsafe { CoCreateInstance(&clsid, None, CLSCTX_LOCAL_SERVER) };
    let repo = match created {
        Ok(repo) => repo,
        Err(_) => {
            // COM activation can be unreliable when EA is already open: start EA as a COM server and try again.
            let _ = std::process::Command::new(ea_exe).arg("/Embedding").spawn();
            std::thread::sleep(std::time::Duration::from_secs(15));
            unsafe { CoCreateInstance(&clsid, None, CLSCTX_LOCAL_SERVER) }
                .map_err(|e| anyhow!("{}", crate::i18n::fmt("could not start EA through COM: {0}", &[e.to_string()])))?
        }
    };
    let repo = Disp(repo);
    if !repo.call("OpenFile2", &[s(connection), s(user), s(password)])?.bool() {
        let detail = repo.call("GetLastError", &[]).map(|v| v.text()).unwrap_or_default();
        let _ = repo.call("Exit", &[]);
        bail!("{}", crate::i18n::fmt("could not open the model: {0}", &[detail]));
    }
    Ok(repo)
}

/// Closes the model and EA.
pub fn close(repo: &Disp) {
    let _ = repo.call("CloseFile", &[]);
    let _ = repo.call("Exit", &[]);
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    Package,
    Element,
    Diagram,
    Connector,
}

impl Kind {
    fn name(self) -> &'static str {
        match self {
            Kind::Package => "package",
            Kind::Element => "element",
            Kind::Diagram => "diagram",
            Kind::Connector => "connector",
        }
    }
}

/// State of one job (list of operations).
struct Job<'a> {
    repo: &'a Disp,
    security: bool,
    /// Keys from the job ("#pkg") pointing to objects created earlier in the same job.
    keys: HashMap<String, (Kind, Disp)>,
    /// Objects that existed before the job and were locked by us (their locks are released at the end).
    locked: HashMap<String, Disp>,
    created: Vec<Value>,
    warnings: Vec<String>,
}

/// Runs a job in the open session. `user` is the account from the settings (compared with the logged-in one).
pub fn run_request(repo: &Disp, user: &str, ops: &[Value], rollback_key: Option<&str>, keep_locks: bool) -> Value {
    let mut result = json!({
        "ok": false, "user": null, "security": false, "created": [], "warnings": [],
        "error": null, "failed_op": null, "rolled_back": false, "session_lost": false
    });
    let security = match repo.get("IsSecurityEnabled") {
        Ok(v) => v.bool(),
        Err(e) => return lost(result, &e.to_string()),
    };
    // Account logged in to the model: with security enabled, OpenFile2 opens the model only with the correct
    // account and password from the settings, so it is that account. (GetCurrentLoginUser is not available through IDispatch.)
    result["security"] = json!(security);
    result["user"] = json!(if security { user } else { "" });

    let mut job = Job { repo, security, keys: HashMap::new(), locked: HashMap::new(), created: Vec::new(), warnings: Vec::new() };
    let mut failed = None;
    for (index, op) in ops.iter().enumerate() {
        if let Err(e) = job.op(op) {
            let mut error = e.to_string();
            // On a refusal caused by a lock: who locks the objects of this operation (read from the model's lock table).
            // EA messages are in English ("locked"); the application's messages are in the UI language.
            let lower = error.to_lowercase();
            if security && ["lock", "zablokowa", "sperr", "verrouill"].iter().any(|word| lower.contains(word)) {
                error.push_str(&job.lock_report(op, user));
            }
            failed = Some((index, error));
            break;
        }
    }
    match failed {
        None => result["ok"] = json!(true),
        Some((index, error)) => {
            result["failed_op"] = json!(index);
            result["error"] = json!(error);
            // Import into a new, dedicated package: on error, delete the whole package so no half-done work is left behind.
            if let Some((Kind::Package, package)) = rollback_key.and_then(|k| job.keys.get(k)).cloned() {
                match job.remove_package(&package) {
                    Ok(()) => {
                        result["rolled_back"] = json!(true);
                        job.created.clear();
                    }
                    Err(e) => job.warnings.push(crate::i18n::fmt("Could not roll back the changes: {0}", &[e.to_string()])),
                }
            }
        }
    }
    if !keep_locks {
        job.release_locks();
    }
    result["created"] = json!(job.created);
    result["warnings"] = json!(job.warnings);
    if let Some(error) = result["error"].as_str() {
        if is_session_lost(error) {
            result["session_lost"] = json!(true);
        }
    }
    result
}

fn lost(mut result: Value, error: &str) -> Value {
    result["error"] = json!(error);
    result["session_lost"] = json!(true);
    result
}

/// Errors of a broken connection to the EA process (e.g. EA was closed): the session has to be reopened.
pub fn is_session_lost(error: &str) -> bool {
    ["RPC", "0x800706B", "0x80010108", "0x800401FD", "serwer RPC", "RPC server"].iter().any(|code| error.contains(code))
}

/// Integer from an operation field (numbers from the model may arrive as fractions).
fn int_of(value: &Value) -> Option<i32> {
    value.as_f64().map(|n| n.round() as i32).or_else(|| value.as_str().and_then(|t| t.trim().parse().ok()))
}

fn int_field(op: &Value, name: &str) -> Result<i32> {
    int_of(&op[name]).ok_or_else(|| anyhow!("{}", crate::i18n::fmt("missing field '{0}' (number)", &[name.to_string()])))
}

fn text_field(op: &Value, name: &str) -> String {
    match &op[name] {
        Value::String(t) => t.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

/// Color from "#RRGGBB" notation to an EA number (BGR order). A number passes through unchanged.
fn color(value: &Value) -> Result<i32> {
    if let Some(text) = value.as_str() {
        let hex = text.trim().trim_start_matches('#');
        if hex.len() == 6 {
            let rgb = u32::from_str_radix(hex, 16).map_err(|_| anyhow!("{}", crate::i18n::fmt("color '{0}' is not in the #RRGGBB format", &[text.to_string()])))?;
            let (r, g, b) = ((rgb >> 16) & 0xFF, (rgb >> 8) & 0xFF, rgb & 0xFF);
            return Ok(((b << 16) | (g << 8) | r) as i32);
        }
        bail!("{}", crate::i18n::fmt("color '{0}' is not in the #RRGGBB format", &[text.to_string()]));
    }
    int_of(value).ok_or_else(|| anyhow!("{}", crate::i18n::tr("invalid color")))
}

/// Rows from the result of `Repository.SQLQuery` (XML: `<Row><column>value</column>…</Row>`), column names in lowercase.
fn xml_rows(xml: &str) -> Vec<HashMap<String, String>> {
    let unescape = |t: &str| t.replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&apos;", "'").replace("&amp;", "&");
    let mut rows = Vec::new();
    for chunk in xml.split("<Row>").skip(1) {
        let body = chunk.split("</Row>").next().unwrap_or("");
        let mut row = HashMap::new();
        let mut rest = body;
        while let Some(start) = rest.find('<') {
            let after = &rest[start + 1..];
            let Some(end) = after.find('>') else { break };
            let tag = &after[..end];
            if tag.ends_with('/') {
                row.insert(tag.trim_end_matches('/').trim().to_lowercase(), String::new());
                rest = &after[end + 1..];
                continue;
            }
            let content = &after[end + 1..];
            let close = format!("</{tag}>");
            let Some(stop) = content.find(&close) else { break };
            row.insert(tag.to_lowercase(), unescape(&content[..stop]));
            rest = &content[stop + close.len()..];
        }
        rows.push(row);
    }
    rows
}

fn text_field_value(value: &Value) -> String {
    match value {
        Value::String(t) => t.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

fn is_key(value: &Value) -> Option<&str> {
    value.as_str().filter(|t| t.starts_with('#')).map(|t| &t[1..])
}

impl Job<'_> {
    fn register(&mut self, key: &Value, kind: Kind, object: &Disp, id: i32, guid: String, name: String) {
        if let Some(key) = key.as_str().filter(|k| !k.is_empty()) {
            self.keys.insert(key.trim_start_matches('#').to_string(), (kind, object.clone()));
        }
        self.created.push(json!({ "key": key, "kind": kind.name(), "id": id, "guid": guid, "name": name }));
    }

    /// Lock on an object that existed before this job (Require User Lock to Edit).
    /// ApplyUserLock on an object already locked by us REMOVES the lock, hence Release first, then Apply.
    fn lock_existing(&mut self, object: &Disp, kind: Kind, id: i32) -> Result<()> {
        if !self.security {
            return Ok(());
        }
        let key = format!("{}:{id}", kind.name());
        if self.locked.contains_key(&key) {
            return Ok(());
        }
        let _ = object.call("ReleaseUserLock", &[]);
        if !object.call("ApplyUserLock", &[])?.bool() {
            bail!("{}", crate::i18n::fmt("Cannot lock the object ({0} id={1}). It is probably locked by another user.", &[kind.name().to_string(), id.to_string()]));
        }
        self.locked.insert(key, object.clone());
        Ok(())
    }

    fn from_key(&self, key: &str, kind: Kind) -> Result<Disp> {
        let (found, object) = self
            .keys
            .get(key)
            .ok_or_else(|| anyhow!("{}", crate::i18n::fmt("Unknown key '#{0}' (the object was not created earlier in this batch).", &[key.to_string()])))?;
        if *found != kind {
            bail!("{}", crate::i18n::fmt("Key '#{0}' refers to an object of type '{1}', but '{2}' was expected.", &[key.to_string(), found.name().to_string(), kind.name().to_string()]));
        }
        Ok(object.clone())
    }

    fn by_ref(&mut self, value: &Value, kind: Kind, getter: &str) -> Result<Disp> {
        if value.is_null() {
            bail!("{}", crate::i18n::fmt("No reference given: {0}.", &[kind.name().to_string()]));
        }
        if let Some(key) = is_key(value) {
            return self.from_key(key, kind);
        }
        let id = int_of(value).ok_or_else(|| anyhow!("{}", crate::i18n::fmt("Invalid reference {0}: {1}", &[kind.name().to_string(), value.to_string()])))?;
        let object = self
            .repo
            .call(getter, &[i(id)])
            .ok()
            .and_then(|v| v.object())
            .ok_or_else(|| anyhow!("{}", crate::i18n::fmt("There is no {0} with id {1}.", &[kind.name().to_string(), id.to_string()])))?;
        self.lock_existing(&object, kind, id)?;
        Ok(object)
    }

    fn package(&mut self, value: &Value) -> Result<Disp> {
        self.by_ref(value, Kind::Package, "GetPackageByID")
    }

    fn element(&mut self, value: &Value) -> Result<Disp> {
        self.by_ref(value, Kind::Element, "GetElementByID")
    }

    fn diagram(&mut self, value: &Value) -> Result<Disp> {
        self.by_ref(value, Kind::Diagram, "GetDiagramByID")
    }

    /// Owner of a new diagram/element: the parent element or the package.
    fn owner(&mut self, op: &Value) -> Result<Disp> {
        if !op["parent_element"].is_null() {
            return self.element(&op["parent_element"]);
        }
        self.package(&op["package"])
    }

    /// Element keywords (the Keywords field in EA, t_object.pdata5 in the database), e.g. for coloring by a legend.
    fn set_keywords(&self, element: &Disp, keywords: &Value) -> Result<()> {
        if keywords.is_null() {
            return Ok(());
        }
        let text = match keywords {
            Value::Array(items) => items.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>().join(", "),
            other => text_field_value(other),
        };
        let id = element.int("ElementID")?;
        self.repo.call("Execute", &[s(&format!("UPDATE t_object SET PDATA5 = '{}' WHERE Object_ID = {id}", text.replace('\'', "''")))])?;
        Ok(())
    }

    /// Diagram legend (Text element, NType 76) with automatic coloring: elements whose filter property
    /// (e.g. Element.Keyword) has the value of a legend entry get its color; the same for connectors (e.g. Connector.Stereotype).
    fn create_legend(&mut self, op: &Value) -> Result<()> {
        let diagram = self.diagram(&op["diagram"])?;
        let package = self.package(&json!(diagram.int("PackageID")?))?;
        let title = { let t = text_field(op, "title"); if t.is_empty() { crate::i18n::tr("Legend").to_string() } else { t } };
        let legend = package.obj("Elements")?.call("AddNew", &[s(&title), s("Text")])?.object().ok_or_else(|| anyhow!("{}", crate::i18n::tr("EA did not create the legend.")))?;
        legend.set("Subtype", i(76))?;
        legend.update(crate::i18n::tr("legend"))?;
        let object_filter = { let f = text_field(op, "filter"); if f.is_empty() { "Element.Keyword".to_string() } else { f } };
        let mut style = format!("LegendTypeObj=Filter={object_filter}:;LegendTypeObjV2=Filter&_eq_^{object_filter}&_sc_^;LegendOpts=63;");
        let connector_filter = text_field(op, "connector_filter");
        if !connector_filter.is_empty() {
            style.push_str(&format!("LegendTypeConn=Filter={connector_filter}:;LegendTypeConnV2=Filter&_eq_^{connector_filter}&_sc_^;"));
        }
        let id = legend.int("ElementID")?;
        let guid = legend.text("ElementGUID")?;
        self.repo.call("Execute", &[s(&format!("UPDATE t_object SET StyleEx = '{}' WHERE Object_ID = {id}", style.replace('\'', "''")))])?;
        // Legend entries: filter value, element fill color or connector line color.
        let mut props = String::new();
        let mut position = 0;
        for entry in op["entries"].as_array().into_iter().flatten() {
            let value = text_field(entry, "value");
            let label = { let l = text_field(entry, "label"); if l.is_empty() { value.clone() } else { l } };
            let fill = color(&entry["color"])?;
            props.push_str(&format!(
                "@PROP=@NAME={value}@ENDNAME;@TYPE=LEGEND_OBJECTSTYLE@ENDTYPE;@VALU=#Back_Ground_Color#={fill};#Pen_Color#=-1;#Pen_Size#=1;#Legend_Type#=LEGEND_OBJECTSTYLE;#Display_Value#={label};@ENDVALU;@PRMT={position}@ENDPRMT;@ENDPROP;"
            ));
            position += 1;
        }
        for entry in op["connector_entries"].as_array().into_iter().flatten() {
            let value = text_field(entry, "value");
            let label = { let l = text_field(entry, "label"); if l.is_empty() { value.clone() } else { l } };
            let pen = color(&entry["color"])?;
            let width = int_of(&entry["width"]).unwrap_or(2);
            props.push_str(&format!(
                "@PROP=@NAME={value}@ENDNAME;@TYPE=LEGEND_LINESTYLE@ENDTYPE;@VALU=#Back_Ground_Color#=16777215;#Pen_Color#={pen};#Pen_Size#={width};#Legend_Type#=LEGEND_LINESTYLE;#Display_Value#={label};@ENDVALU;@PRMT={position}@ENDPRMT;@ENDPROP;"
            ));
            position += 1;
        }
        props.push_str(&format!("@PROP=@NAME=Legend@ENDNAME;@TYPE=LEGEND_STYLE_SETTINGS@ENDTYPE;@VALU=#TITLE#={title};@ENDVALU;@PRMT=@ENDPRMT;@ENDPROP;"));
        let xref = format!("{{{}}}", uuid::Uuid::new_v4().to_string().to_uppercase());
        self.repo.call("Execute", &[s(&format!(
            "INSERT INTO t_xref (XrefID, Name, Type, Visibility, Partition, Description, Client, Supplier) VALUES ('{xref}', 'CustomProperties', 'element property', 'Public', '0', '{}', '{guid}', '<none>')",
            props.replace('\'', "''")
        ))])?;
        // On the diagram: top right corner or the given position.
        let (left, top) = (int_of(&op["left"]).unwrap_or(20), int_of(&op["top"]).unwrap_or(20));
        let height = 40 + 22 * position;
        let object = diagram
            .obj("DiagramObjects")?
            .call("AddNew", &[s(&format!("l={left};r={};t={top};b={};", left + 180, top + height)), s("")])?
            .object()
            .ok_or_else(|| anyhow!("{}", crate::i18n::tr("EA did not add the legend to the diagram.")))?;
        object.set("ElementID", i(id))?;
        object.update(crate::i18n::tr("legend on the diagram"))?;
        self.register(&op["key"], Kind::Element, &legend, id, guid, title);
        Ok(())
    }

    fn set_tags(&self, object: &Disp, tags: &Value) -> Result<()> {
        let Some(tags) = tags.as_object() else { return Ok(()) };
        for (name, value) in tags {
            let collection = object.obj("TaggedValues")?;
            collection.call("Refresh", &[])?;
            // null removes the tagged value.
            if value.is_null() {
                let items = collection.items()?;
                if let Some(index) = items.iter().position(|tag| tag.text("Name").ok().as_deref() == Some(name.as_str())) {
                    collection.call("DeleteAt", &[i(index as i32), b(false)])?;
                    collection.call("Refresh", &[])?;
                }
                continue;
            }
            let value = match value {
                Value::String(t) => t.clone(),
                other => other.to_string(),
            };
            let mut found = None;
            for tag in collection.items()? {
                if tag.text("Name")? == *name {
                    found = Some(tag);
                }
            }
            match found {
                Some(tag) => {
                    tag.set("Value", s(&value))?;
                    tag.update(crate::i18n::tr("tagged value"))?;
                }
                None => {
                    let tag = collection.call_obj("AddNew", &[s(name), s(&value)])?;
                    tag.update(crate::i18n::tr("new tagged value"))?;
                }
            }
        }
        Ok(())
    }

    /// Reply message of a sequence diagram (dashed, drawn back to the caller): the database keeps the flag in
    /// t_connector.pdata4, which COM does not expose.
    fn set_reply(&self, connector: &Disp, value: &Value) -> Result<()> {
        let Some(reply) = value.as_bool() else { return Ok(()) };
        let id = connector.int("ConnectorID")?;
        self.repo.call("Execute", &[s(&format!("UPDATE t_connector SET PDATA4 = '{}' WHERE Connector_ID = {id}", if reply { 1 } else { 0 }))])?;
        Ok(())
    }

    /// Fork/join (Synchronization): EA draws it vertical when t_object.ntype = 1 and horizontal when 0; the size on the
    /// diagram does not decide it (EAExample.qea, 2026-10-19). Set from the box: taller than wide = vertical.
    fn orient_sync(&self, element: &Disp, width: i32, height: i32) -> Result<()> {
        if element.text("Type")? != "Synchronization" {
            return Ok(());
        }
        let id = element.int("ElementID")?;
        self.repo.call("Execute", &[s(&format!("UPDATE t_object SET NType = {} WHERE Object_ID = {id}", if height > width { 1 } else { 0 }))])?;
        Ok(())
    }

    /// Element/connector from an MDG profile: ALWAYS a base UML type + stereotype. AddNew with a qualified type
    /// ("BPMN2.0::StartEvent") leaves an orphaned object without a package in the database (verified 2026-10-13).
    fn new_typed(&self, collection: &Disp, name: &str, kind: &str, stereotype: &str) -> Result<Disp> {
        let mut kind = kind.to_string();
        let mut stereotype = stereotype.to_string();
        if kind.contains("::") {
            if stereotype.is_empty() {
                stereotype = kind.clone();
            }
            let short = kind.rsplit(':').next().unwrap_or("").to_string();
            match BASE_TYPES.iter().find(|(k, _)| *k == short) {
                Some((_, base)) => kind = base.to_string(),
                None => bail!("{}", crate::i18n::fmt("The 'type' field must be a UML type (e.g. Activity, Event, Decision, Class); the profile name ('{0}') belongs in the 'stereotype' field.", &[kind.clone()])),
            }
        }
        let object = collection
            .call("AddNew", &[s(name), s(&kind)])?
            .object()
            .ok_or_else(|| anyhow!("{}", crate::i18n::fmt("EA did not create an object of type '{0}' (no lock on the owner, or unknown type).", &[kind.clone()])))?;
        if !stereotype.is_empty() {
            object.set("StereotypeEx", s(&stereotype))?;
        }
        Ok(object)
    }

    /// Removes the items with the given identifier from an EA collection. Error when nothing was removed.
    fn remove_from(&self, collection: &Disp, id_property: &str, id: i32, what: &str) -> Result<()> {
        collection.call("Refresh", &[])?;
        let mut removed = 0;
        let count = collection.int("Count")?;
        for index in (0..count).rev() {
            if collection.call_obj("GetAt", &[(index as i32).into()])?.int(id_property)? == id {
                collection.call("DeleteAt", &[(index as i32).into(), b(false)])?;
                removed += 1;
            }
        }
        collection.call("Refresh", &[])?;
        if removed == 0 {
            bail!("{}", crate::i18n::fmt("Object to delete not found ({0}).", &[what.to_string()]));
        }
        for item in collection.items()? {
            if item.int(id_property)? == id {
                bail!("{}", crate::i18n::fmt("EA did not delete the object ({0}): no lock, or the object is locked by someone else.", &[what.to_string()]));
            }
        }
        Ok(())
    }

    fn remove_package(&mut self, package: &Disp) -> Result<()> {
        let parent_id = package.int("ParentID")?;
        let parent = self.package(&json!(parent_id))?;
        // With "Require User Lock", EA deletes a package without asking only when all of its contents are
        // locked by us; otherwise it opens a dialog and the session hangs.
        if self.security {
            let _ = package.call("ReleaseUserLockRecursive", &[b(true), b(true), b(true)]);
            if !package.call("ApplyUserLockRecursive", &[b(true), b(true), b(true)])?.bool() {
                bail!("{}", crate::i18n::fmt("Cannot lock the contents of package '{0}' before deleting it.", &[package.text("Name")?]));
            }
        }
        let id = package.int("PackageID")?;
        let packages = parent.obj("Packages")?;
        packages.call("Refresh", &[])?;
        let count = packages.int("Count")?;
        for index in (0..count).rev() {
            if packages.call_obj("GetAt", &[(index as i32).into()])?.int("PackageID")? == id {
                packages.call("DeleteAt", &[(index as i32).into(), b(false)])?;
            }
        }
        packages.call("Refresh", &[])?;
        Ok(())
    }

    /// Description of the locks on the objects the operation touches (element, parent element, diagram, package,
    /// connector source and target) and on the packages of those elements: who locks (account, group), since when, or nobody.
    fn lock_report(&self, op: &Value, user: &str) -> String {
        let mut objects: Vec<(String, String)> = Vec::new(); // (description, GUID)
        let mut add = |label: String, guid: String| {
            if !guid.is_empty() && !objects.iter().any(|(_, g)| *g == guid) {
                objects.push((label, guid));
            }
        };
        let fetch = |value: &Value, kind: Kind, getter: &str| -> Option<Disp> {
            if let Some(key) = is_key(value) {
                return self.keys.get(key).filter(|(k, _)| *k == kind).map(|(_, o)| o.clone());
            }
            int_of(value).and_then(|id| self.repo.call(getter, &[i(id)]).ok()).and_then(|v| v.object())
        };
        let package_label = |package: &Disp| crate::i18n::fmt("package \"{0}\"", &[package.text("Name").unwrap_or_default()]);
        for field in ["element", "parent_element", "source", "target"] {
            if let Some(element) = fetch(&op[field], Kind::Element, "GetElementByID") {
                add(crate::i18n::fmt("element \"{0}\"", &[element.text("Name").unwrap_or_default()]), element.text("ElementGUID").unwrap_or_default());
                if let Some(package) = element.int("PackageID").ok().and_then(|id| self.repo.call("GetPackageByID", &[i(id)]).ok()).and_then(|v| v.object()) {
                    add(package_label(&package), package.text("PackageGUID").unwrap_or_default());
                }
            }
        }
        if let Some(diagram) = fetch(&op["diagram"], Kind::Diagram, "GetDiagramByID") {
            add(crate::i18n::fmt("diagram \"{0}\"", &[diagram.text("Name").unwrap_or_default()]), diagram.text("DiagramGUID").unwrap_or_default());
        }
        for field in ["package", "parent"] {
            if let Some(package) = fetch(&op[field], Kind::Package, "GetPackageByID") {
                add(package_label(&package), package.text("PackageGUID").unwrap_or_default());
            }
        }
        if objects.is_empty() {
            return String::new();
        }
        let list = objects.iter().map(|(_, g)| format!("'{}'", g.replace('\'', ""))).collect::<Vec<_>>().join(",");
        let sql = format!(
            "SELECT l.EntityID AS guid, u.UserLogin AS login, u.FirstName AS first, u.Surname AS last, g.GroupName AS grp, l.Timestamp AS since \
             FROM (t_seclocks l LEFT JOIN t_secuser u ON u.UserID = l.UserID) LEFT JOIN t_secgroup g ON g.GroupID = l.GroupID \
             WHERE l.EntityID IN ({list})"
        );
        let rows = match self.repo.call("SQLQuery", &[s(&sql)]) {
            Ok(xml) => xml_rows(&xml.text()),
            Err(e) => return format!("\n{}", crate::i18n::fmt("Could not read the locks from the model: {0}", &[e.to_string()])),
        };
        let mut lines = vec![format!("\n{}", crate::i18n::tr("Locks in the model:"))];
        for (label, guid) in &objects {
            let holders: Vec<String> = rows
                .iter()
                .filter(|r| r.get("guid").is_some_and(|g| g.eq_ignore_ascii_case(guid)))
                .map(|r| {
                    let get = |k: &str| r.get(k).map(|v| v.trim().to_string()).unwrap_or_default();
                    let who = if !get("login").is_empty() {
                        let name = format!("{} {}", get("first"), get("last")).trim().to_string();
                        let mut who = if name.is_empty() { crate::i18n::fmt("account {0}", &[get("login")]) } else { crate::i18n::fmt("account {0} ({1})", &[get("login"), name.clone()]) };
                        if get("login").eq_ignore_ascii_case(user) {
                            who.push_str(" - ");
                            who.push_str(crate::i18n::tr("the account the assistant logs in with"));
                        }
                        who
                    } else if !get("grp").is_empty() {
                        crate::i18n::fmt("group {0}", &[get("grp")])
                    } else {
                        crate::i18n::tr("unknown owner").to_string()
                    };
                    if get("since").is_empty() { who } else { crate::i18n::fmt("{0}, since {1}", &[who, get("since")]) }
                })
                .collect();
            lines.push(if holders.is_empty() {
                crate::i18n::fmt("- {0}: not locked", &[label.clone()])
            } else {
                crate::i18n::fmt("- {0}: locked by {1}", &[label.clone(), holders.join("; ")])
            });
        }
        if rows.is_empty() {
            lines.push(crate::i18n::tr("None of these objects has a lock in the model's lock table, so the refusal comes from EA itself.").to_string());
        }
        lines.join("\n")
    }

    /// Releases locks: ours on existing objects and the author's automatic locks on new objects,
    /// so the user can edit what was created right away.
    fn release_locks(&mut self) {
        if !self.security {
            return;
        }
        for created in self.created.iter().rev() {
            let id = created["id"].as_i64().unwrap_or(0) as i32;
            let getter = match created["kind"].as_str() {
                Some("package") => "GetPackageByID",
                Some("element") => "GetElementByID",
                Some("diagram") => "GetDiagramByID",
                _ => continue,
            };
            if let Some(object) = self.repo.call(getter, &[i(id)]).ok().and_then(|v| v.object()) {
                if getter == "GetPackageByID" {
                    if object.call("ReleaseUserLockRecursive", &[b(true), b(true), b(true)]).is_err() {
                        let _ = object.call("ReleaseUserLock", &[]);
                    }
                } else {
                    let _ = object.call("ReleaseUserLock", &[]);
                }
            }
        }
        for object in self.locked.values() {
            let _ = object.call("ReleaseUserLock", &[]);
        }
    }

    fn op(&mut self, op: &Value) -> Result<()> {
        let name = op["op"].as_str().unwrap_or("");
        match name {
            "create_package" => {
                let parent = self.package(&op["parent"])?;
                let title = text_field(op, "name");
                let packages = parent.obj("Packages")?;
                let package = packages
                    .call("AddNew", &[s(&title), s("Package")])?
                    .object()
                    .ok_or_else(|| anyhow!("{}", crate::i18n::fmt("EA did not create package '{0}' (no lock on the parent package?).", &[title.clone()])))?;
                package.update(&crate::i18n::fmt("package '{0}'", &[title.clone()]))?;
                let notes = text_field(op, "notes");
                if !notes.is_empty() {
                    package.set("Notes", s(&notes))?;
                    package.update(crate::i18n::tr("package notes"))?;
                }
                packages.call("Refresh", &[])?;
                let (id, guid, real) = (package.int("PackageID")?, package.text("PackageGUID")?, package.text("Name")?);
                self.register(&op["key"], Kind::Package, &package, id, guid, real);
            }
            "create_diagram" => {
                let owner = self.owner(op)?;
                let (title, kind) = (text_field(op, "name"), text_field(op, "type"));
                let diagram = owner
                    .obj("Diagrams")?
                    .call("AddNew", &[s(&title), s(&kind)])?
                    .object()
                    .ok_or_else(|| anyhow!("{}", crate::i18n::fmt("EA did not create diagram '{0}' of type '{1}'.", &[title.clone(), kind.clone()])))?;
                diagram.update(&crate::i18n::fmt("diagram '{0}'", &[title.clone()]))?;
                let notes = text_field(op, "notes");
                if !notes.is_empty() {
                    diagram.set("Notes", s(&notes))?;
                    diagram.update(crate::i18n::tr("diagram notes"))?;
                }
                let (id, guid, real) = (diagram.int("DiagramID")?, diagram.text("DiagramGUID")?, diagram.text("Name")?);
                self.register(&op["key"], Kind::Diagram, &diagram, id, guid, real);
            }
            "create_element" if op["type"] == "Package" && op["parent_element"].is_null() => {
                // A package shown on a diagram: a real package (t_package) whose element is the one placed on diagrams.
                let title = text_field(op, "name");
                let parent = self.package(&op["package"])?;
                let packages = parent.obj("Packages")?;
                let package = packages
                    .call("AddNew", &[s(&title), s("Package")])?
                    .object()
                    .ok_or_else(|| anyhow!("{}", crate::i18n::fmt("EA did not create package '{0}' (no lock on the parent package?).", &[title.clone()])))?;
                package.update(&crate::i18n::fmt("package '{0}'", &[title.clone()]))?;
                packages.call("Refresh", &[])?;
                let element = package.obj("Element")?;
                let notes = text_field(op, "notes");
                if !notes.is_empty() {
                    element.set("Notes", s(&notes))?;
                    element.update(crate::i18n::tr("package notes"))?;
                }
                let (id, guid, real) = (element.int("ElementID")?, element.text("ElementGUID")?, element.text("Name")?);
                self.register(&op["key"], Kind::Element, &element, id, guid, real);
            }
            "create_element" => {
                let title = text_field(op, "name");
                let (kind, stereotype) = (text_field(op, "type"), text_field(op, "stereotype"));
                let element = if op["parent_element"].is_null() {
                    let owner = self.owner(op)?;
                    self.new_typed(&owner.obj("Elements")?, &title, &kind, &stereotype)?
                } else {
                    // Nested element (e.g. a subprocess step): created in the parent element's package and linked
                    // to it through ParentID, the way EA stores it in the database (t_object.parentid). Creating it through
                    // the parent element's Elements collection was rejected by EA in a model with "Require User Lock to Edit"
                    // with the error "Element locked", despite a lock held by the same account (2026-10-15).
                    let parent = self.element(&op["parent_element"])?;
                    let package = self.package(&json!(parent.int("PackageID")?))?;
                    let element = self.new_typed(&package.obj("Elements")?, &title, &kind, &stereotype)?;
                    element.set("ParentID", i(parent.int("ElementID")?))?;
                    element
                };
                let notes = text_field(op, "notes");
                if !notes.is_empty() {
                    element.set("Notes", s(&notes))?;
                }
                // Kind of a pseudostate (StateNode): 3 initial, 4 final, 11 choice; 100 / 101 initial / final of an activity.
                if let Some(n) = int_of(&op["subtype"]) {
                    element.set("Subtype", i(n))?;
                }
                element.update(&crate::i18n::fmt("element '{0}'", &[title.clone()]))?;
                if !op["color"].is_null() {
                    element.call("SetAppearance", &[i(1), i(0), i(color(&op["color"])?)])?;
                    element.update(crate::i18n::tr("element color"))?;
                }
                self.set_tags(&element, &op["tags"])?;
                self.set_keywords(&element, &op["keywords"])?;
                let (id, guid, real) = (element.int("ElementID")?, element.text("ElementGUID")?, element.text("Name")?);
                self.register(&op["key"], Kind::Element, &element, id, guid, real);
            }
            "create_connector" => {
                let source = self.element(&op["source"])?;
                let target = self.element(&op["target"])?;
                let title = text_field(op, "name");
                let connector = self.new_typed(&source.obj("Connectors")?, &title, &text_field(op, "type"), &text_field(op, "stereotype"))?;
                connector.set("SupplierID", i(target.int("ElementID")?))?;
                // A message of a sequence diagram belongs to that diagram and has its place in the order.
                if !op["diagram"].is_null() {
                    connector.set("DiagramID", i(self.diagram(&op["diagram"])?.int("DiagramID")?))?;
                }
                if let Some(n) = int_of(&op["sequence"]) {
                    connector.set("SequenceNo", i(n))?;
                }
                if !op["message_sort"].is_null() {
                    connector.set("TransitionEvent", s(&text_field(op, "message_sort")))?;
                }
                connector.update(&crate::i18n::fmt("connector '{0}'", &[title.clone()]))?;
                self.set_reply(&connector, &op["is_return"])?;
                self.set_tags(&connector, &op["tags"])?;
                let (id, guid, real) = (connector.int("ConnectorID")?, connector.text("ConnectorGUID")?, connector.text("Name")?);
                self.register(&op["key"], Kind::Connector, &connector, id, guid, real);
            }
            "place" => {
                let diagram = self.diagram(&op["diagram"])?;
                let element = self.element(&op["element"])?;
                let (left, top) = (int_field(op, "left")?, int_field(op, "top")?);
                let (right, bottom) = (left + int_field(op, "width")?, top + int_field(op, "height")?);
                let object = diagram
                    .obj("DiagramObjects")?
                    .call("AddNew", &[s(&format!("l={left};r={right};t={top};b={bottom};")), s("")])?
                    .object()
                    .ok_or_else(|| anyhow!("{}", crate::i18n::tr("EA did not add the element to the diagram.")))?;
                object.set("ElementID", i(element.int("ElementID")?))?;
                self.orient_sync(&element, right - left, bottom - top)?;
                if !op["bgcolor"].is_null() {
                    object.set("BackgroundColor", i(color(&op["bgcolor"])?))?;
                }
                object.update(crate::i18n::tr("diagram object"))?;
                let style = text_field(op, "style");
                if !style.is_empty() {
                    object.set("Style", s(&style))?;
                    object.update(crate::i18n::tr("diagram object style"))?;
                }
            }
            "diagram_style" => {
                let diagram = self.diagram(&op["diagram"])?;
                let current = diagram.text("StyleEx")?;
                let joined = format!("{};{};", current.trim_end_matches(';'), text_field(op, "append").trim_matches(';'));
                diagram.set("StyleEx", s(joined.trim_start_matches(';')))?;
                diagram.update(crate::i18n::tr("diagram style"))?;
            }
            "set_composite" => {
                let element = self.element(&op["element"])?;
                let diagram = self.diagram(&op["diagram"])?;
                element.call("SetCompositeDiagram", &[s(&diagram.text("DiagramGUID")?)])?;
                element.update(crate::i18n::tr("composite diagram of the element"))?;
            }
            "update_element" => {
                let element = self.element(&op["element"])?;
                for (field, property) in [
                    ("name", "Name"),
                    ("notes", "Notes"),
                    ("alias", "Alias"),
                    ("status", "Status"),
                    ("version", "Version"),
                    ("author", "Author"),
                    ("phase", "Phase"),
                    ("complexity", "Complexity"),
                ] {
                    if !op[field].is_null() {
                        element.set(property, s(&text_field(op, field)))?;
                    }
                }
                if let Some(abstract_) = op["abstract"].as_bool() {
                    element.set("Abstract", s(if abstract_ { "1" } else { "0" }))?;
                }
                let stereotype = text_field(op, "stereotype");
                if !stereotype.is_empty() || op["clear_stereotype"].as_bool() == Some(true) {
                    element.set("StereotypeEx", s(&stereotype))?;
                }
                element.update(&crate::i18n::fmt("element id={0}", &[element.int("ElementID")?.to_string()]))?;
                self.set_keywords(&element, &op["keywords"])?;
                if !op["color"].is_null() {
                    element.call("SetAppearance", &[i(1), i(0), i(color(&op["color"])?)])?;
                    element.update(crate::i18n::tr("element color"))?;
                }
                self.set_tags(&element, &op["tags"])?;
            }
            "update_connector" => {
                // Properties of an existing relationship: name, stereotype, direction, notes, the multiplicities and roles
                // of both ends, tagged values. Both end elements are locked, as for deleting a connector.
                // A relationship created earlier in this batch is read again from the model: changes made through the
                // object returned when it was created did not reach the database (seen for the direction, 2026-10-18).
                let connector = match is_key(&op["connector"]) {
                    Some(key) => {
                        let id = self.from_key(key, Kind::Connector)?.int("ConnectorID")?;
                        self.repo
                            .call("GetConnectorByID", &[i(id)])
                            .ok()
                            .and_then(|v| v.object())
                            .ok_or_else(|| anyhow!("{}", crate::i18n::fmt("There is no connector with id {0}.", &[id.to_string()])))?
                    }
                    None => {
                        let id = int_field(op, "connector")?;
                        self.repo
                            .call("GetConnectorByID", &[i(id)])
                            .ok()
                            .and_then(|v| v.object())
                            .ok_or_else(|| anyhow!("{}", crate::i18n::fmt("There is no connector with id {0}.", &[id.to_string()])))?
                    }
                };
                self.element(&json!(connector.int("ClientID")?))?;
                self.element(&json!(connector.int("SupplierID")?))?;
                let id = connector.int("ConnectorID")?;
                for (field, property) in [("name", "Name"), ("notes", "Notes"), ("direction", "Direction")] {
                    if !op[field].is_null() {
                        connector.set(property, s(&text_field(op, field)))?;
                    }
                }
                let stereotype = text_field(op, "stereotype");
                if !stereotype.is_empty() || op["clear_stereotype"].as_bool() == Some(true) {
                    connector.set("StereotypeEx", s(&stereotype))?;
                }
                if let Some(n) = int_of(&op["sequence"]) {
                    connector.set("SequenceNo", i(n))?;
                }
                if !op["message_sort"].is_null() {
                    connector.set("TransitionEvent", s(&text_field(op, "message_sort")))?;
                }
                connector.update(&crate::i18n::fmt("connector id={0}", &[id.to_string()]))?;
                self.set_reply(&connector, &op["is_return"])?;
                // EA derives the direction of a relationship from the navigability of its ends: without them a new
                // direction does not stay on a relationship created in the same session (checked 2026-10-18).
                let navigable = |source: bool| -> Option<&'static str> {
                    let direction = op["direction"].as_str()?;
                    let yes = match direction {
                        "Source -> Destination" => !source,
                        "Destination -> Source" => source,
                        "Bi-Directional" => true,
                        _ => false,
                    };
                    Some(if yes { "Navigable" } else { "Unspecified" })
                };
                for (end, prefix) in [("ClientEnd", "source"), ("SupplierEnd", "target")] {
                    let (card, role) = (format!("{prefix}_multiplicity"), format!("{prefix}_role"));
                    let navigability = navigable(prefix == "source");
                    if op[&card].is_null() && op[&role].is_null() && navigability.is_none() {
                        continue;
                    }
                    let end = connector.obj(end)?;
                    if let Some(value) = navigability {
                        end.set("Navigable", s(value))?;
                    }
                    if !op[&card].is_null() {
                        end.set("Cardinality", s(&text_field(op, &card)))?;
                    }
                    if !op[&role].is_null() {
                        end.set("Role", s(&text_field(op, &role)))?;
                    }
                    end.update(crate::i18n::tr("connector end"))?;
                }
                self.set_tags(&connector, &op["tags"])?;
            }
            "move" => {
                // Moves an existing object to another package (relationships and positions on diagrams are kept).
                let package = self.package(&op["package"])?;
                let package_id = package.int("PackageID")?;
                if !op["element"].is_null() {
                    let element = self.element(&op["element"])?;
                    element.set("PackageID", i(package_id))?;
                    element.set("ParentID", i(0))?;
                    element.update(&crate::i18n::fmt("moving element id={0}", &[element.int("ElementID")?.to_string()]))?;
                } else if !op["diagram"].is_null() {
                    let diagram = self.diagram(&op["diagram"])?;
                    diagram.set("PackageID", i(package_id))?;
                    diagram.set("ParentID", i(0))?;
                    diagram.update(&crate::i18n::fmt("moving diagram id={0}", &[diagram.int("DiagramID")?.to_string()]))?;
                } else if !op["child_package"].is_null() {
                    let child = self.package(&op["child_package"])?;
                    if child.int("PackageID")? == package_id {
                        bail!("{}", crate::i18n::tr("A package cannot be moved into itself."));
                    }
                    child.set("ParentID", i(package_id))?;
                    child.update(&crate::i18n::fmt("moving package id={0}", &[child.int("PackageID")?.to_string()]))?;
                } else {
                    bail!("{}", crate::i18n::tr("The move operation requires the element, diagram or child_package field."));
                }
            }
            "ping" => {
                // Checks the connection and login (settings test): changes nothing in the model.
            }
            "delete" => {
                // Deletes an object from the model. Together with an element, EA deletes its connectors and its occurrences on diagrams.
                if !op["element"].is_null() {
                    let element = self.element(&op["element"])?;
                    let id = element.int("ElementID")?;
                    let parent = element.int("ParentID")?;
                    let owner = if parent != 0 { self.element(&json!(parent))? } else { self.package(&json!(element.int("PackageID")?))? };
                    self.remove_from(&owner.obj("Elements")?, "ElementID", id, &crate::i18n::fmt("element id={0}", &[id.to_string()]))?;
                } else if !op["diagram"].is_null() {
                    let diagram = self.diagram(&op["diagram"])?;
                    let id = diagram.int("DiagramID")?;
                    let parent = diagram.int("ParentID")?;
                    let owner = if parent != 0 { self.element(&json!(parent))? } else { self.package(&json!(diagram.int("PackageID")?))? };
                    self.remove_from(&owner.obj("Diagrams")?, "DiagramID", id, &crate::i18n::fmt("diagram id={0}", &[id.to_string()]))?;
                } else if !op["connector"].is_null() {
                    let id = int_field(op, "connector")?;
                    let connector = self
                        .repo
                        .call("GetConnectorByID", &[i(id)])
                        .ok()
                        .and_then(|v| v.object())
                        .ok_or_else(|| anyhow!("{}", crate::i18n::fmt("There is no connector with id {0}.", &[id.to_string()])))?;
                    let source = self.element(&json!(connector.int("ClientID")?))?;
                    self.element(&json!(connector.int("SupplierID")?))?;
                    self.remove_from(&source.obj("Connectors")?, "ConnectorID", id, &crate::i18n::fmt("connector id={0}", &[id.to_string()]))?;
                } else if !op["package"].is_null() {
                    let package = self.package(&op["package"])?;
                    if package.int("ParentID")? == 0 {
                        bail!("{}", crate::i18n::tr("The root package of the model is not deleted."));
                    }
                    self.remove_package(&package)?;
                } else {
                    bail!("{}", crate::i18n::tr("The delete operation requires the element, diagram, connector or package field."));
                }
            }
            "set_color" => {
                // Element fill color on one diagram (does not change the element's default color).
                let diagram = self.diagram(&op["diagram"])?;
                let element_id = self.element(&op["element"])?.int("ElementID")?;
                let objects = diagram.obj("DiagramObjects")?;
                objects.call("Refresh", &[])?;
                let mut found = false;
                for object in objects.items()? {
                    if object.int("ElementID")? == element_id {
                        object.set("BackgroundColor", i(color(&op["bgcolor"])?))?;
                        object.update(crate::i18n::tr("color on the diagram"))?;
                        found = true;
                    }
                }
                if !found {
                    bail!("{}", crate::i18n::fmt("Element id={0} is not on diagram id={1}.", &[element_id.to_string(), diagram.int("DiagramID")?.to_string()]));
                }
            }
            "create_legend" => self.create_legend(op)?,
            "create_attribute" => {
                let element = self.element(&op["element"])?;
                let name = text_field(op, "name");
                let attribute = element.obj("Attributes")?.call_obj("AddNew", &[s(&name), s(&text_field(op, "type"))])?;
                for (field, property) in [("visibility", "Visibility"), ("notes", "Notes"), ("default", "Default"), ("stereotype", "StereotypeEx")] {
                    if !op[field].is_null() {
                        attribute.set(property, s(&text_field(op, field)))?;
                    }
                }
                attribute.update(&crate::i18n::fmt("attribute '{0}'", &[name.clone()]))?;
                element.obj("Attributes")?.call("Refresh", &[])?;
                let id = attribute.int("AttributeID")?;
                self.created.push(json!({ "key": op["key"], "kind": "attribute", "id": id, "guid": attribute.text("AttributeGUID")?, "name": name }));
            }
            "create_operation" => {
                let element = self.element(&op["element"])?;
                let name = text_field(op, "name");
                let operation = element.obj("Methods")?.call_obj("AddNew", &[s(&name), s(&text_field(op, "return_type"))])?;
                for (field, property) in [("visibility", "Visibility"), ("notes", "Notes"), ("stereotype", "StereotypeEx")] {
                    if !op[field].is_null() {
                        operation.set(property, s(&text_field(op, field)))?;
                    }
                }
                operation.update(&crate::i18n::fmt("operation '{0}'", &[name.clone()]))?;
                for parameter in op["parameters"].as_array().into_iter().flatten() {
                    let p = operation.obj("Parameters")?.call_obj("AddNew", &[s(&text_field(parameter, "name")), s(&text_field(parameter, "type"))])?;
                    p.update(crate::i18n::tr("operation parameter"))?;
                }
                element.obj("Methods")?.call("Refresh", &[])?;
                let id = operation.int("MethodID")?;
                self.created.push(json!({ "key": op["key"], "kind": "operation", "id": id, "guid": operation.text("MethodGUID")?, "name": name }));
            }
            "update_attribute" | "delete_attribute" | "update_operation" | "delete_operation" => {
                // An existing attribute or operation of a class: its owner element is locked first.
                let attribute = name.ends_with("_attribute");
                let (getter, id_property, collection, label) = if attribute {
                    ("GetAttributeByID", "AttributeID", "Attributes", crate::i18n::tr("attribute"))
                } else {
                    ("GetMethodByID", "MethodID", "Methods", crate::i18n::tr("operation"))
                };
                let id = int_field(op, if attribute { "attribute" } else { "operation" })?;
                let member = self
                    .repo
                    .call(getter, &[i(id)])
                    .ok()
                    .and_then(|v| v.object())
                    .ok_or_else(|| anyhow!("{}", crate::i18n::fmt("There is no {0} with id {1}.", &[label.to_string(), id.to_string()])))?;
                let owner = self.element(&json!(member.int("ParentID")?))?;
                let what = format!("{label} id={id}");
                if name.starts_with("delete_") {
                    self.remove_from(&owner.obj(collection)?, id_property, id, &what)?;
                } else {
                    let type_property = if attribute { "Type" } else { "ReturnType" };
                    let type_field = if attribute { "type" } else { "return_type" };
                    for (field, property) in [("name", "Name"), (type_field, type_property), ("visibility", "Visibility"), ("notes", "Notes"), ("default", "Default")] {
                        if !op[field].is_null() && (attribute || field != "default") {
                            member.set(property, s(&text_field(op, field)))?;
                        }
                    }
                    if let Some(is_static) = op["static"].as_bool() {
                        member.set("IsStatic", b(is_static))?;
                    }
                    member.update(&what)?;
                    owner.obj(collection)?.call("Refresh", &[])?;
                }
            }
            "clone_package" => {
                // Copy of a package with all its contents (elements, diagrams, child packages) next to the original.
                let package = self.package(&op["package"])?;
                let parent = self.package(&json!(package.int("ParentID")?))?;
                let copy = package.call("Clone", &[])?.object().ok_or_else(|| anyhow!("{}", crate::i18n::tr("EA did not clone the package.")))?;
                let name = text_field(op, "name");
                if !name.is_empty() {
                    copy.set("Name", s(&name))?;
                    copy.update(crate::i18n::tr("name of the package copy"))?;
                }
                parent.obj("Packages")?.call("Refresh", &[])?;
                let (id, guid, real) = (copy.int("PackageID")?, copy.text("PackageGUID")?, copy.text("Name")?);
                self.register(&op["key"], Kind::Package, &copy, id, guid, real);
            }
            "create_baseline" => {
                // Package version (baseline) saved in the model: for comparing and restoring in EA (Manage Baselines).
                // Restoring through COM (DoBaselineMerge) hangs EA on an invisible dialog, so it is not offered.
                let package = self.package(&op["package"])?;
                let project = self.repo.call_obj("GetProjectInterface", &[])?;
                let version = text_field(op, "version");
                let notes = text_field(op, "notes");
                let ok = project.call("CreateBaseline", &[s(&package.text("PackageGUID")?), s(&version), s(&notes)])?.bool();
                if !ok {
                    bail!("{}", crate::i18n::fmt("EA did not create a baseline of package '{0}': {1}", &[package.text("Name")?, project.text("GetLastError").unwrap_or_default()]));
                }
            }
            "update_diagram" => {
                // Name, description and stereotype of an existing diagram.
                let diagram = self.diagram(&op["diagram"])?;
                for (field, property) in [("name", "Name"), ("notes", "Notes"), ("stereotype", "Stereotype")] {
                    if !op[field].is_null() {
                        diagram.set(property, s(&text_field(op, field)))?;
                    }
                }
                diagram.update(&crate::i18n::fmt("diagram id={0}", &[diagram.int("DiagramID")?.to_string()]))?;
            }
            "hide_connectors" => {
                // Hides (or shows) selected connectors on one diagram; they stay in the model and on other diagrams.
                let diagram = self.diagram(&op["diagram"])?;
                let ids: Vec<i32> = op["connectors"].as_array().map(|a| a.iter().filter_map(int_of).collect()).unwrap_or_default();
                if ids.is_empty() {
                    bail!("{}", crate::i18n::tr("Provide a list of connectors (connectors)."));
                }
                let hidden = op["hidden"].as_bool().unwrap_or(true);
                let links = diagram.obj("DiagramLinks")?;
                links.call("Refresh", &[])?;
                let existing = links.items()?;
                for cid in &ids {
                    let link = match existing.iter().find(|l| l.int("ConnectorID").ok() == Some(*cid)) {
                        Some(link) => link.clone(),
                        None => {
                            let link = links.call_obj("AddNew", &[s(""), s("")])?;
                            link.set("ConnectorID", i(*cid))?;
                            link
                        }
                    };
                    link.set("IsHidden", b(hidden))?;
                    link.update(crate::i18n::tr("connector visibility on the diagram"))?;
                }
                diagram.update(crate::i18n::tr("diagram"))?;
            }
            "route_connector" => {
                // Route of one connector on a diagram: bend points and the points where it touches the elements.
                // Coordinates as in "place" (the diagram's top left corner is 0,0, Y axis pointing down); EA stores vertical ones with a minus sign.
                let diagram = self.diagram(&op["diagram"])?;
                let cid = int_field(op, "connector")?;
                let connector = self
                    .repo
                    .call("GetConnectorByID", &[i(cid)])
                    .ok()
                    .and_then(|v| v.object())
                    .ok_or_else(|| anyhow!("{}", crate::i18n::fmt("There is no connector with id {0}.", &[cid.to_string()])))?;
                let (source, target) = (connector.int("ClientID")?, connector.int("SupplierID")?);
                let objects = diagram.obj("DiagramObjects")?;
                objects.call("Refresh", &[])?;
                let rect_of = |element: i32| -> Result<(i32, i32, i32, i32)> {
                    for o in objects.items()? {
                        if o.int("ElementID")? == element {
                            return Ok((o.int("left")?, -o.int("top")?, o.int("right")?, -o.int("bottom")?));
                        }
                    }
                    bail!("{}", crate::i18n::fmt("Element id={0} (end of connector id={1}) is not on diagram id={2}.", &[element.to_string(), cid.to_string(), diagram.int("DiagramID").unwrap_or(0).to_string()]))
                };
                let (s_rect, t_rect) = (rect_of(source)?, rect_of(target)?);
                let point = |v: &Value| -> Option<(i32, i32)> {
                    let a = v.as_array()?;
                    Some((int_of(a.first()?)?, int_of(a.get(1)?)?))
                };
                let points: Vec<(i32, i32)> = op["points"].as_array().map(|a| a.iter().filter_map(point).collect()).unwrap_or_default();
                // Touch point as an offset from the element's center (Y axis pointing up, as in EA).
                let offset = |rect: (i32, i32, i32, i32), p: (i32, i32)| -> (i32, i32) {
                    let (cx, cy) = ((rect.0 + rect.2) / 2, (rect.1 + rect.3) / 2);
                    (p.0 - cx, -(p.1 - cy))
                };
                let links = diagram.obj("DiagramLinks")?;
                links.call("Refresh", &[])?;
                let existing = links.items()?.into_iter().find(|l| l.int("ConnectorID").ok() == Some(cid));
                let link = match existing {
                    Some(link) => link,
                    None => {
                        let link = links.call_obj("AddNew", &[s(""), s("")])?;
                        link.set("ConnectorID", i(cid))?;
                        link
                    }
                };
                let mut style: Vec<String> = link
                    .text("Style")?
                    .split(';')
                    .filter(|p| !p.is_empty() && !p.starts_with("Mode=") && !p.starts_with("TREE="))
                    .map(str::to_string)
                    .collect();
                style.push("Mode=3".into());
                link.set("Style", s(&format!("{};", style.join(";"))))?;
                let path: String = points.iter().map(|(x, y)| format!("{x}:{};", -y)).collect();
                link.set("Path", s(&path))?;
                let mut geometry: Vec<String> = link
                    .text("Geometry")?
                    .split(';')
                    .filter(|p| !p.is_empty() && !["SX=", "SY=", "EX=", "EY=", "EDGE="].iter().any(|k| p.starts_with(k)))
                    .map(str::to_string)
                    .collect();
                if let Some(start) = point(&op["start"]) {
                    let (dx, dy) = offset(s_rect, start);
                    geometry.push(format!("SX={dx}"));
                    geometry.push(format!("SY={dy}"));
                }
                if let Some(end) = point(&op["end"]) {
                    let (dx, dy) = offset(t_rect, end);
                    geometry.push(format!("EX={dx}"));
                    geometry.push(format!("EY={dy}"));
                }
                link.set("Geometry", s(&format!("{};", geometry.join(";"))))?;
                link.update(crate::i18n::tr("connector route"))?;
                diagram.update(crate::i18n::tr("diagram"))?;
            }
            "set_line_style" => {
                // Line style of connectors on a diagram: the diagram link style (t_diagramlinks.style, Mode and TREE)
                // for the given connectors or for all connectors between the diagram's elements.
                let diagram = self.diagram(&op["diagram"])?;
                let style = text_field(op, "style");
                let (mode, tree) = match style.as_str() {
                    "direct" => ("1", ""),
                    "auto" => ("2", ""),
                    "custom" => ("3", ""),
                    "orthogonal_square" => ("3", "OS"),
                    "orthogonal_rounded" => ("3", "OR"),
                    "tree_vertical" => ("3", "V"),
                    "tree_horizontal" => ("3", "H"),
                    "lateral_vertical" => ("3", "LV"),
                    "lateral_horizontal" => ("3", "LH"),
                    other => bail!("{}", crate::i18n::fmt("Unknown line style '{0}'. Allowed: direct, auto, custom, orthogonal_square, orthogonal_rounded, tree_vertical, tree_horizontal, lateral_vertical, lateral_horizontal.", &[other.to_string()])),
                };
                let wanted: Option<Vec<i32>> = op["connectors"].as_array().map(|ids| ids.iter().filter_map(int_of).collect());
                let objects = diagram.obj("DiagramObjects")?;
                objects.call("Refresh", &[])?;
                let on_diagram: Vec<i32> = objects.items()?.iter().filter_map(|o| o.int("ElementID").ok()).collect();
                // Connectors between the diagram's elements (from both ends, without duplicates).
                let mut connectors: Vec<i32> = Vec::new();
                for id in &on_diagram {
                    let Some(element) = self.repo.call("GetElementByID", &[i(*id)]).ok().and_then(|v| v.object()) else { continue };
                    for c in element.obj("Connectors")?.items()? {
                        let (cid, a, z) = (c.int("ConnectorID")?, c.int("ClientID")?, c.int("SupplierID")?);
                        if on_diagram.contains(&a) && on_diagram.contains(&z) && !connectors.contains(&cid) {
                            connectors.push(cid);
                        }
                    }
                }
                if let Some(wanted) = &wanted {
                    connectors.retain(|c| wanted.contains(c));
                    if connectors.is_empty() {
                        bail!("{}", crate::i18n::fmt("None of the given connectors joins elements that are on diagram id={0}.", &[diagram.int("DiagramID")?.to_string()]));
                    }
                }
                let links = diagram.obj("DiagramLinks")?;
                links.call("Refresh", &[])?;
                let existing = links.items()?;
                for cid in &connectors {
                    let link = match existing.iter().find(|l| l.int("ConnectorID").ok() == Some(*cid)) {
                        Some(link) => link.clone(),
                        None => {
                            let link = links.call_obj("AddNew", &[s(""), s("")])?;
                            link.set("ConnectorID", i(*cid))?;
                            link
                        }
                    };
                    let mut parts: Vec<String> = link
                        .text("Style")?
                        .split(';')
                        .filter(|p| !p.is_empty() && !p.starts_with("Mode=") && !p.starts_with("TREE="))
                        .map(str::to_string)
                        .collect();
                    parts.push(format!("Mode={mode}"));
                    if !tree.is_empty() {
                        parts.push(format!("TREE={tree}"));
                    }
                    link.set("Style", s(&format!("{};", parts.join(";"))))?;
                    // Bend points from the previous style would distort the new route.
                    link.set("Path", s(""))?;
                    link.update(crate::i18n::tr("connector line style"))?;
                }
                diagram.update(crate::i18n::tr("diagram"))?;
            }
            "set_bounds" => {
                // Position and size of an element already on the diagram. Connectors are kept because the object is not
                // removed. Coordinates as in "place" (Y axis pointing down); EA stores vertical ones with a minus sign.
                let diagram = self.diagram(&op["diagram"])?;
                let element_id = self.element(&op["element"])?.int("ElementID")?;
                let diagram_id = diagram.int("DiagramID")?;
                let objects = diagram.obj("DiagramObjects")?;
                objects.call("Refresh", &[])?;
                let object = objects
                    .items()?
                    .into_iter()
                    .find(|o| o.int("ElementID").ok() == Some(element_id))
                    .ok_or_else(|| anyhow!("{}", crate::i18n::fmt("Element id={0} is not on diagram id={1}.", &[element_id.to_string(), diagram_id.to_string()])))?;
                let (left, right) = (object.int("left")?, object.int("right")?);
                let (top, bottom) = (-object.int("top")?, -object.int("bottom")?);
                let new_left = int_of(&op["left"]).unwrap_or(left);
                let new_top = int_of(&op["top"]).unwrap_or(top);
                let width = int_of(&op["width"]).unwrap_or(right - left);
                let height = int_of(&op["height"]).unwrap_or(bottom - top);
                if width <= 0 || height <= 0 {
                    bail!("{}", crate::i18n::tr("Width and height must be greater than zero."));
                }
                object.set("left", i(new_left))?;
                object.set("right", i(new_left + width))?;
                object.set("top", i(-new_top))?;
                object.set("bottom", i(-(new_top + height)))?;
                object.update(crate::i18n::tr("position and size on the diagram"))?;
                let element = self.element(&op["element"])?;
                self.orient_sync(&element, width, height)?;
            }
            "remove_from_diagram" => {
                // Removes an element from the diagram: the element stays in the model.
                let diagram = self.diagram(&op["diagram"])?;
                let element_id = self.element(&op["element"])?.int("ElementID")?;
                self.remove_from(&diagram.obj("DiagramObjects")?, "ElementID", element_id, &crate::i18n::fmt("element id={0} on the diagram", &[element_id.to_string()]))?;
                diagram.update(crate::i18n::tr("diagram after removing the element"))?;
            }
            "navigation_cell" => self.navigation_cell(op)?,
            "export_diagram" => {
                // Read only: no lock, so the export also works for a diagram locked by the user.
                let diagram = match is_key(&op["diagram"]) {
                    Some(key) => self.from_key(key, Kind::Diagram)?,
                    None => {
                        let id = int_field(op, "diagram")?;
                        self.repo
                            .call("GetDiagramByID", &[i(id)])
                            .ok()
                            .and_then(|v| v.object())
                            .ok_or_else(|| anyhow!("{}", crate::i18n::fmt("There is no diagram with id {0}.", &[id.to_string()])))?
                    }
                };
                let project = self.repo.call_obj("GetProjectInterface", &[])?;
                project.call("PutDiagramImageToFile", &[s(&diagram.text("DiagramGUID")?), s(&text_field(op, "path")), i(1)])?;
            }
            "delete_package" => {
                let package = self.package(&op["package"])?;
                self.remove_package(&package)?;
            }
            other => bail!("{}", crate::i18n::fmt("Unknown operation '{0}'.", &[other.to_string()])),
        }
        Ok(())
    }

    /// Navigation cell: a tile on a diagram that opens another diagram on double-click. EA stores it as a
    /// Text element with the NavigationCell stereotype; the target and the icon are in fields COM does not expose for writing
    /// (pdata1, styleex), so they go through SQL.
    fn navigation_cell(&mut self, op: &Value) -> Result<()> {
        let diagram = self.diagram(&op["diagram"])?;
        let target = match is_key(&op["target"]) {
            Some(key) => self.from_key(key, Kind::Diagram)?,
            None => {
                let id = int_field(op, "target")?;
                self.repo
                    .call("GetDiagramByID", &[i(id)])
                    .ok()
                    .and_then(|v| v.object())
                    .ok_or_else(|| anyhow!("{}", crate::i18n::fmt("There is no target diagram '{0}'.", &[op["target"].to_string()])))?
            }
        };
        let package_id = diagram.int("PackageID")?;
        let package = self.repo.call_obj("GetPackageByID", &[i(package_id)])?;
        self.lock_existing(&package, Kind::Package, package_id)?;
        let element = package
            .obj("Elements")?
            .call("AddNew", &[s(""), s("Text")])?
            .object()
            .ok_or_else(|| anyhow!("{}", crate::i18n::tr("EA did not create the navigation cell (no lock on the diagram's package?).")))?;
        element.set("Stereotype", s("NavigationCell"))?;
        element.set("Subtype", i(82))?;
        // The cell caption is the Alias field (EA puts the target diagram's name there).
        let caption = if op["caption"].is_null() { target.text("Name")? } else { text_field(op, "caption") };
        element.set("Alias", s(&caption))?;
        element.update(crate::i18n::tr("navigation cell"))?;

        let target_type = target.text("Type")?;
        let mut image = text_field(op, "image");
        if image.is_empty() {
            let meta = target.text("MetaType").unwrap_or_default();
            image = NAV_IMAGES
                .iter()
                .find(|(k, _)| !meta.is_empty() && *k == meta)
                .or_else(|| NAV_IMAGES.iter().find(|(k, _)| *k == target_type))
                .map(|(_, v)| v.to_string())
                .unwrap_or_default();
        }
        let valid_image = image.split_once('-').is_some_and(|(a, b)| {
            !a.is_empty() && !b.is_empty() && a.chars().all(|c| c.is_ascii_digit()) && b.chars().all(|c| c.is_ascii_digit())
        });
        if !image.is_empty() && !valid_image {
            bail!("{}", crate::i18n::tr("The image field has the form 'list-number', e.g. 2-61."));
        }
        let style_ex = if image.is_empty() { String::new() } else { format!("NID={image};") };
        let element_id = element.int("ElementID")?;
        let target_id = target.int("DiagramID")?;
        self.repo.call(
            "Execute",
            &[s(&format!("update t_object set pdata1='{target_id}', ntype=82, styleex='{style_ex}' where object_id={element_id}"))],
        )?;
        let xref = format!("{{{}}}", uuid::Uuid::new_v4().to_string().to_uppercase());
        let link_type = target_type.replace('\'', "''");
        let guid = element.text("ElementGUID")?;
        self.repo.call(
            "Execute",
            &[s(&format!(
                "insert into t_xref (xrefid, name, type, visibility, partition, client, supplier, description) values \
                 ('{xref}', 'CustomProperties', 'element property', 'Public', '0', '{guid}', '<none>', \
                 '@PROP=@NAME=DiagLinkType@ENDNAME;@TYPE=string@ENDTYPE;@VALU={link_type}@ENDVALU;@PRMT=@ENDPRMT;@ENDPROP;')"
            ))],
        )?;

        let (left, top) = (int_field(op, "left")?, int_field(op, "top")?);
        let width = int_of(&op["width"]).unwrap_or(170);
        let height = int_of(&op["height"]).unwrap_or(115);
        let object = diagram
            .obj("DiagramObjects")?
            .call("AddNew", &[s(&format!("l={left};r={};t={top};b={};", left + width, top + height)), s("")])?
            .object()
            .ok_or_else(|| anyhow!("{}", crate::i18n::tr("EA did not add the navigation cell to the diagram.")))?;
        object.set("ElementID", i(element_id))?;
        object.update(crate::i18n::tr("navigation cell on the diagram"))?;
        // Caption font as EA sets it when a cell is added manually.
        object.set("Style", s("font=Calibri;fontsz=140;bold=0;black=0;italic=0;ul=0;charset=0;pitch=0;"))?;
        object.update(crate::i18n::tr("navigation cell style"))?;
        let name = format!("-> {}", target.text("Name")?);
        self.register(&op["key"], Kind::Element, &element, element_id, guid, name);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colors_are_bgr() {
        assert_eq!(color(&json!("#FF0000")).unwrap(), 255);
        assert_eq!(color(&json!("#00FF00")).unwrap(), 65280);
        assert_eq!(color(&json!("0000FF")).unwrap(), 16711680);
        assert_eq!(color(&json!(123)).unwrap(), 123);
        assert!(color(&json!("#12345")).is_err());
    }

    #[test]
    fn numbers_and_keys() {
        assert_eq!(int_of(&json!(12.6)), Some(13));
        assert_eq!(int_of(&json!("7")), Some(7));
        assert_eq!(is_key(&json!("#pkg")), Some("pkg"));
        assert_eq!(is_key(&json!(5)), None);
        assert!(is_session_lost("The RPC server is unavailable. (0x800706BA)"));
        assert!(!is_session_lost("There is no package with id 5."));
    }
}

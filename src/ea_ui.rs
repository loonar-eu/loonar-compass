//! Interaction with the EA window the user has open (not with the background session the assistant uses to write the model):
//! what is open and selected, opening a diagram, showing an element in the project browser
//! and refreshing the view after a write. A running EA registers itself in the system as "EA.App".

use crate::com::{i, Disp};
use anyhow::{anyhow, bail, Context, Result};
use serde_json::{json, Value};
use std::time::Duration;

/// Runs `f` on the Repository of the open EA window, in a separate COM thread, with a timeout
/// (an EA window that is busy, e.g. with an open dialog, must not block the assistant).
fn with_running_ea<T: Send + 'static>(timeout: Duration, f: impl FnOnce(&Disp) -> Result<T> + Send + 'static) -> Result<T> {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        use windows::core::{Interface, HSTRING};
        use windows::Win32::System::Com::{CLSIDFromProgID, CoInitializeEx, IDispatch, COINIT_APARTMENTTHREADED};
        use windows::Win32::System::Ole::GetActiveObject;
        let result = (|| -> Result<T> {
            // SAFETY: COM initialization in this thread and retrieval of the object registered by the running EA.
            let app = unsafe {
                let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
                let clsid = CLSIDFromProgID(&HSTRING::from("EA.App")).context(crate::i18n::tr("EA is not installed"))?;
                let mut unknown = None;
                GetActiveObject(&clsid, None, &mut unknown).map_err(|_| anyhow!("{}", crate::i18n::tr("no Enterprise Architect window is open")))?;
                unknown.ok_or_else(|| anyhow!("{}", crate::i18n::tr("no Enterprise Architect window is open")))?.cast::<IDispatch>()?
            };
            let repo = Disp(app).obj("Repository").context(crate::i18n::tr("the EA window has no model open"))?;
            f(&repo)
        })();
        let _ = tx.send(result);
    });
    rx.recv_timeout(timeout).map_err(|_| anyhow!("{}", crate::i18n::tr("the EA window is not responding (a dialog box may be open)")))?
}

/// The model's connection without credentials: EA connection strings may contain `PWD=`/`Password=`, and this
/// text goes to the language model and to the step list on screen.
fn safe_connection(connection: &str) -> String {
    let secret = |part: &str| {
        let key = part.split('=').next().unwrap_or("").trim().to_ascii_lowercase();
        matches!(key.as_str(), "pwd" | "password" | "uid" | "user id" | "user")
    };
    // EA prefixes DBMS connections with "<name> --- "; the provider part is a ;-separated list.
    let (label, provider) = connection.split_once(" --- ").unwrap_or(("", connection));
    let kept: Vec<&str> = provider.split(';').filter(|part| !part.trim().is_empty() && !secret(part)).collect();
    let cleaned = kept.join(";");
    if label.is_empty() { cleaned } else { format!("{label} --- {cleaned}") }
}

/// Checks whether the open EA window shows the same model the assistant works with (database name in the connection).
fn same_model(repo: &Disp, db_name: &str) -> Result<()> {
    let connection = repo.text("ConnectionString").unwrap_or_default();
    if !db_name.is_empty() && !connection.to_lowercase().contains(&db_name.to_lowercase()) {
        bail!("{}", crate::i18n::fmt("the open EA window shows a different model (connection: {0}), but the assistant works with the database \"{1}\"", &[safe_connection(&connection), db_name.to_string()]));
    }
    Ok(())
}

/// What the user has open and selected in EA.
pub fn context(db_name: &str) -> Result<Value> {
    let db = db_name.to_string();
    with_running_ea(Duration::from_secs(10), move |repo| {
        same_model(repo, &db)?;
        let mut out = json!({ "model": safe_connection(&repo.text("ConnectionString").unwrap_or_default()) });
        if let Some(diagram) = repo.call("GetCurrentDiagram", &[]).ok().and_then(|v| v.object()) {
            let mut selected = Vec::new();
            for object in diagram.obj("SelectedObjects").and_then(|c| c.items()).unwrap_or_default() {
                let id = object.int("ElementID").unwrap_or(0);
                if let Some(element) = repo.call("GetElementByID", &[i(id)]).ok().and_then(|v| v.object()) {
                    selected.push(json!({
                        "object_id": id,
                        "name": element.text("Name").unwrap_or_default(),
                        "type": element.text("Type").unwrap_or_default(),
                        "stereotype": element.text("Stereotype").unwrap_or_default(),
                    }));
                }
            }
            let connector = diagram.obj("SelectedConnector").ok().map(|c| {
                json!({ "connector_id": c.int("ConnectorID").unwrap_or(0), "type": c.text("Type").unwrap_or_default(), "name": c.text("Name").unwrap_or_default() })
            });
            out["current_diagram"] = json!({
                "diagram_id": diagram.int("DiagramID").unwrap_or(0),
                "name": diagram.text("Name").unwrap_or_default(),
                "type": diagram.text("Type").unwrap_or_default(),
                "selected_elements": selected,
                "selected_connector": connector,
            });
        } else {
            out["current_diagram"] = Value::Null;
        }
        // Selection in the project browser: element, package or diagram.
        let kind = repo.call("GetTreeSelectedItemType", &[]).map(|v| v.i32()).unwrap_or(-1);
        if let Some(item) = repo.call("GetTreeSelectedObject", &[]).ok().and_then(|v| v.object()) {
            let (what, id_field) = match kind {
                4 => ("element", "ElementID"),
                5 => ("package", "PackageID"),
                8 => ("diagram", "DiagramID"),
                _ => ("other", ""),
            };
            out["browser_selection"] = json!({
                "kind": what,
                "id": if id_field.is_empty() { 0 } else { item.int(id_field).unwrap_or(0) },
                "name": item.text("Name").unwrap_or_default(),
            });
        }
        Ok(out)
    })
}

/// Opens a diagram in the user's EA window.
pub fn open_diagram(db_name: &str, diagram_id: i32) -> Result<()> {
    let db = db_name.to_string();
    with_running_ea(Duration::from_secs(15), move |repo| {
        same_model(repo, &db)?;
        repo.call("OpenDiagram", &[i(diagram_id)])?;
        Ok(())
    })
}

/// Shows an element in the project browser of the user's EA window.
pub fn show_element(db_name: &str, element_id: i32) -> Result<()> {
    let db = db_name.to_string();
    with_running_ea(Duration::from_secs(15), move |repo| {
        same_model(repo, &db)?;
        let element = repo.call_obj("GetElementByID", &[i(element_id)])?;
        repo.call("ShowInProjectView", &[windows::core::VARIANT::from(windows::core::IUnknown::from(element.0.clone()))])?;
        Ok(())
    })
}

/// Refreshes the changed diagrams (if open) and the project browser in the user's EA window.
/// Does nothing without an EA window or with a different model. Returns the number of refreshed diagrams.
pub fn refresh_after_write(db_name: &str, diagrams: Vec<i32>, packages: Vec<i32>) -> Result<usize> {
    let db = db_name.to_string();
    with_running_ea(Duration::from_secs(10), move |repo| {
        same_model(repo, &db)?;
        let mut reloaded = 0;
        for id in &diagrams {
            if repo.call("ReloadDiagram", &[i(*id)]).is_ok() {
                reloaded += 1;
            }
        }
        for id in &packages {
            let _ = repo.call("RefreshModelView", &[i(*id)]);
        }
        Ok(reloaded)
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn connection_shown_to_the_model_has_no_credentials() {
        let shown = super::safe_connection("model --- DBType=4;Connect=Provider=SSDB;SRC=host,5432@ea;UID=postgres;PWD=Secret1;");
        assert!(!shown.contains("Secret1") && !shown.contains("postgres"), "{shown}");
        assert!(shown.starts_with("model --- ") && shown.contains("SRC=host,5432@ea"), "{shown}");
        assert_eq!(super::safe_connection("C:\\models\\a.qea"), "C:\\models\\a.qea");
    }

    /// Without an open EA window: a clear error within a few seconds, no hang.
    /// With an EA window (model from app.properties) it prints what is open and selected.
    /// Run with: `cargo test ea_ui_context -- --ignored --nocapture`.
    #[test]
    #[ignore = "depends on whether EA is open"]
    fn ea_ui_context() {
        let started = std::time::Instant::now();
        let result = super::context("");
        println!("{result:?} ({} ms)", started.elapsed().as_millis());
        assert!(started.elapsed().as_secs() < 12);
    }
}
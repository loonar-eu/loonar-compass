//! Interaction with the EA window the user has open (not with the background session the assistant uses to write the model):
//! what is open and selected, opening a diagram, showing an element in the project browser
//! and refreshing the view after a write. A running EA registers itself in the system as "EA.App".

use crate::com::{i, Disp};
use anyhow::{anyhow, bail, Context, Result};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
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
            // In a tab of EA: exactly the EA window that hosts Compass (its add-in registered the repository in the
            // Running Object Table), not the first EA found in the system.
            if let Some(name) = ROT_NAME.get() {
                let repo = Disp(unsafe { repository_from_rot(name)? });
                return f(&repo);
            }
            // SAFETY: COM initialization in this thread and retrieval of the object registered by the running EA.
            let app = unsafe {
                let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
                let clsid = CLSIDFromProgID(&HSTRING::from("EA.App")).context(crate::i18n::tr("EA is not installed"))?;
                let mut unknown = None;
                GetActiveObject(&clsid, None, &mut unknown).map_err(|_| anyhow!("{}", crate::i18n::tr("no Enterprise Architect window is open")))?;
                unknown.ok_or_else(|| anyhow!("{}", crate::i18n::tr("no Enterprise Architect window is open")))?.cast::<IDispatch>()?
            };
            let app = Disp(app);
            // The assistant's own background EA registers as "EA.App" too, but it has no visible window: it is not the user's EA.
            if !app.get("Visible").map(|v| v.bool()).unwrap_or(false) {
                bail!("{}", crate::i18n::tr("no Enterprise Architect window is open"));
            }
            let repo = app.obj("Repository").context(crate::i18n::tr("the EA window has no model open"))?;
            f(&repo)
        })();
        let _ = tx.send(result);
    });
    rx.recv_timeout(timeout).map_err(|_| anyhow!("{}", crate::i18n::tr("the EA window is not responding (a dialog box may be open)")))?
}

static ROT_NAME: std::sync::OnceLock<String> = std::sync::OnceLock::new();

/// Compass in a tab of EA: the name under which that EA's add-in registered its repository (`--ea-rot`).
pub fn set_rot_name(name: &str) {
    let _ = ROT_NAME.set(name.to_string());
}

/// In a tab of EA: whether the keyboard is in the Compass window (Windows does not report a child window of
/// another process as focused, so the focus window of the foreground thread is checked).
pub fn keyboard_is_ours() -> bool {
    #[repr(C)]
    struct GuiThreadInfo {
        cb: u32,
        flags: u32,
        active: isize,
        focus: isize,
        capture: isize,
        menu_owner: isize,
        move_size: isize,
        caret: isize,
        caret_rect: [i32; 4],
    }
    #[link(name = "user32")]
    extern "system" {
        fn GetForegroundWindow() -> isize;
        fn GetWindowThreadProcessId(window: isize, process: *mut u32) -> u32;
        fn GetGUIThreadInfo(thread: u32, info: *mut GuiThreadInfo) -> i32;
    }
    // SAFETY: plain WinAPI calls with a local, correctly sized structure.
    unsafe {
        let foreground = GetForegroundWindow();
        let thread = GetWindowThreadProcessId(foreground, std::ptr::null_mut());
        let mut info = GuiThreadInfo { cb: std::mem::size_of::<GuiThreadInfo>() as u32, flags: 0, active: 0, focus: 0, capture: 0, menu_owner: 0, move_size: 0, caret: 0, caret_rect: [0; 4] };
        if GetGUIThreadInfo(thread, &mut info) == 0 || info.focus == 0 {
            return false;
        }
        let mut process = 0u32;
        GetWindowThreadProcessId(info.focus, &mut process);
        process == std::process::id()
    }
}

/// Gives the keyboard to the Compass window docked in EA (joins its input with the thread that has the keyboard now).
pub fn take_keyboard(window: isize) {
    #[link(name = "user32")]
    extern "system" {
        fn GetForegroundWindow() -> isize;
        fn GetWindowThreadProcessId(window: isize, process: *mut u32) -> u32;
        fn AttachThreadInput(from: u32, to: u32, attach: i32) -> i32;
        fn SetFocus(window: isize) -> isize;
    }
    #[link(name = "kernel32")]
    extern "system" {
        fn GetCurrentThreadId() -> u32;
    }
    // SAFETY: plain WinAPI calls without pointers to our data.
    unsafe {
        let ea_thread = GetWindowThreadProcessId(GetForegroundWindow(), std::ptr::null_mut());
        let own = GetCurrentThreadId();
        if ea_thread != 0 && ea_thread != own {
            AttachThreadInput(own, ea_thread, 1);
        }
        SetFocus(window);
    }
}

/// A line for the "Loonar Compass" tab of EA's System Output (the add-in shows what the assistant does there).
/// Nothing outside a tab of EA.
pub fn output(text: &str) {
    let Some(name) = ROT_NAME.get() else { return };
    let file = std::env::temp_dir().join(format!("loonar-ea-output-{name}.txt"));
    let line = format!("{}  {}\n", chrono::Local::now().format("%H:%M:%S"), text.replace('\n', " "));
    use std::io::Write;
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(file) {
        let _ = f.write_all(line.as_bytes());
    }
}

/// References the EA add-in sent with "Add to the Compass conversation" (an element, package or diagram picked in
/// the EA window), taken out of their hand-over file. Empty outside a tab of EA.
pub fn take_references() -> Vec<String> {
    let Some(name) = ROT_NAME.get() else { return Vec::new() };
    let file = std::env::temp_dir().join(format!("loonar-ea-insert-{name}.txt"));
    let Ok(text) = std::fs::read_to_string(&file) else { return Vec::new() };
    let _ = std::fs::remove_file(&file);
    text.lines().map(str::trim).filter(|line| !line.is_empty()).map(str::to_string).collect()
}

/// The repository registered in the Running Object Table under the item moniker `!name`.
///
/// # Safety
/// COM calls; the thread is initialised for COM here.
unsafe fn repository_from_rot(name: &str) -> Result<windows::Win32::System::Com::IDispatch> {
    use windows::core::Interface;
    use windows::Win32::System::Com::{CoInitializeEx, CoTaskMemFree, CreateBindCtx, GetRunningObjectTable, IDispatch, IMoniker, COINIT_APARTMENTTHREADED};
    let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
    let gone = || anyhow!("{}", crate::i18n::tr("the Enterprise Architect window of this Compass tab is no longer available"));
    let rot = GetRunningObjectTable(0).map_err(|_| gone())?;
    let monikers = rot.EnumRunning().map_err(|_| gone())?;
    let context = CreateBindCtx(0)?;
    let wanted = format!("!{name}");
    loop {
        let mut item: [Option<IMoniker>; 1] = [None];
        if monikers.Next(&mut item, None).is_err() {
            break;
        }
        let Some(moniker) = item[0].take() else { break };
        let Ok(display) = moniker.GetDisplayName(&context, None) else { continue };
        let text = display.to_string().unwrap_or_default();
        CoTaskMemFree(Some(display.0 as _));
        if text.eq_ignore_ascii_case(&wanted) {
            return Ok(rot.GetObject(&moniker).map_err(|_| gone())?.cast::<IDispatch>()?);
        }
    }
    Err(gone())
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

/// The model the assistant works with, as the user's EA window is compared against: a database name or an EA file.
#[derive(Clone, Debug)]
pub struct Target {
    db_name: String,
    file: Option<PathBuf>,
}

impl Target {
    pub fn of(cfg: &crate::config::AppConfig) -> Self {
        Self { db_name: cfg.db_name.clone(), file: cfg.uses_file().then(|| cfg.model_file.clone()) }
    }
}

/// The same file under different spellings (short 8.3 names, letter case, `/` or `\`) compares equal.
fn same_file(a: &Path, b: &Path) -> bool {
    let canonical = |p: &Path| std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf()).to_string_lossy().to_lowercase();
    canonical(a) == canonical(b)
}

/// Checks whether the open EA window shows the same model the assistant works with
/// (the database name in the connection, or the same EA file).
fn same_model(repo: &Disp, target: &Target) -> Result<()> {
    let connection = repo.text("ConnectionString").unwrap_or_default();
    match &target.file {
        Some(file) => {
            if !same_file(Path::new(connection.trim()), file) {
                bail!("{}", crate::i18n::fmt("the open EA window shows a different model (connection: {0}), but the assistant works with the file \"{1}\"", &[safe_connection(&connection), file.display().to_string()]));
            }
        }
        None => {
            let db_name = &target.db_name;
            if !db_name.is_empty() && !connection.to_lowercase().contains(&db_name.to_lowercase()) {
                bail!("{}", crate::i18n::fmt("the open EA window shows a different model (connection: {0}), but the assistant works with the database \"{1}\"", &[safe_connection(&connection), db_name.to_string()]));
            }
        }
    }
    Ok(())
}

/// Whether the user's EA window has this model open (answers within a few seconds; no window = false).
pub fn has_model_open(target: &Target) -> bool {
    let target = target.clone();
    with_running_ea(Duration::from_secs(5), move |repo| same_model(repo, &target)).is_ok()
}

/// Runs a write job in the user's EA window (an EA file the user has open: a second EA instance must not
/// open the same `.qea` file). The reply has the same form as from the background session.
pub fn run_in_window(target: &Target, user: String, ops: Vec<Value>, rollback_key: Option<String>, keep_locks: bool, locks: bool, timeout: Duration) -> Result<Value> {
    let target = target.clone();
    with_running_ea(timeout, move |repo| {
        same_model(repo, &target)?;
        Ok(crate::ea_com::run_request(repo, &user, &ops, rollback_key.as_deref(), keep_locks, locks))
    })
}

/// What the user has open and selected in EA.
pub fn context(target: &Target) -> Result<Value> {
    let db = target.clone();
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
pub fn open_diagram(target: &Target, diagram_id: i32) -> Result<()> {
    let db = target.clone();
    with_running_ea(Duration::from_secs(15), move |repo| {
        same_model(repo, &db)?;
        repo.call("OpenDiagram", &[i(diagram_id)])?;
        Ok(())
    })
}

/// Shows an element in the project browser of the user's EA window.
pub fn show_element(target: &Target, element_id: i32) -> Result<()> {
    let db = target.clone();
    with_running_ea(Duration::from_secs(15), move |repo| {
        same_model(repo, &db)?;
        let element = repo.call_obj("GetElementByID", &[i(element_id)])?;
        repo.call("ShowInProjectView", &[windows::core::VARIANT::from(windows::core::IUnknown::from(element.0.clone()))])?;
        Ok(())
    })
}

/// Refreshes the changed diagrams (if open) and the project browser in the user's EA window.
/// Does nothing without an EA window or with a different model. Returns the number of refreshed diagrams.
pub fn refresh_after_write(target: &Target, diagrams: Vec<i32>, packages: Vec<i32>) -> Result<usize> {
    let db = target.clone();
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
        let result = super::context(&super::Target { db_name: String::new(), file: None });
        println!("{result:?} ({} ms)", started.elapsed().as_millis());
        assert!(started.elapsed().as_secs() < 12);
    }
}
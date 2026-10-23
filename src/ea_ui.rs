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
static OWN_WINDOW: std::sync::atomic::AtomicIsize = std::sync::atomic::AtomicIsize::new(0);

/// The window Compass draws in (docked in EA): the keyboard must be there, not in a helper window of the process.
pub fn set_own_window(window: isize) {
    OWN_WINDOW.store(window, std::sync::atomic::Ordering::Relaxed);
}

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
        let own = OWN_WINDOW.load(std::sync::atomic::Ordering::Relaxed);
        if own != 0 {
            return info.focus == own;
        }
        let mut process = 0u32;
        GetWindowThreadProcessId(info.focus, &mut process);
        process == std::process::id()
    }
}

/// A diagnostic line in %TEMP%\loonar-compass-debug.log (EA mode only, small).
pub fn debug(text: &str) {
    let file = std::env::temp_dir().join("loonar-compass-debug.log");
    if std::fs::metadata(&file).map(|m| m.len() > 100_000).unwrap_or(false) {
        let _ = std::fs::remove_file(&file);
    }
    use std::io::Write;
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(file) {
        let _ = writeln!(f, "{} {text}", chrono::Local::now().format("%H:%M:%S%.3f"));
    }
}

/// The size of a window's client area in physical pixels.
pub fn client_size(window: isize) -> Option<(i32, i32)> {
    #[link(name = "user32")]
    extern "system" {
        fn GetClientRect(window: isize, rect: *mut [i32; 4]) -> i32;
    }
    if window == 0 {
        return None;
    }
    let mut rect = [0i32; 4];
    // SAFETY: a local four-integer RECT.
    (unsafe { GetClientRect(window, &mut rect) } != 0).then(|| (rect[2] - rect[0], rect[3] - rect[1])).filter(|(w, h)| *w > 0 && *h > 0)
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
        // The add-in joins the input of both threads (once); here only the focus moves.
        let _ = (GetForegroundWindow, GetWindowThreadProcessId, AttachThreadInput, GetCurrentThreadId);
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

/// Whether reads of a database model go through EA's own connection (Compass in a window of EA): the database
/// open in EA needs no connection or password in Compass's settings.
pub fn reads_through_ea() -> bool {
    ROT_NAME.get().is_some()
}

/// A read-only query run by the EA window of this Compass tab (Repository.SQLQuery): column names and rows.
/// EA returns query results as XML with the column names as element names: an expression column without an alias
/// (count(*), coalesce(…)) is not a valid name and EA fails ("invalid XML character"). Expressions of the outer
/// select list get aliases (expr1, expr2…); plain columns and existing aliases stay as they are.
pub fn alias_expressions(sql: &str) -> String {
    let lower = sql.to_ascii_lowercase();
    let bytes = lower.as_bytes();
    // The outer SELECT: the first "select" at depth 0 that is not inside a WITH clause's parentheses.
    let mut depth = 0i32;
    let mut start = None;
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'(' => depth += 1,
            b')' => depth -= 1,
            b'\'' => {
                i += 1;
                while i < bytes.len() && bytes[i] != b'\'' {
                    i += 1;
                }
            }
            _ if depth == 0 && lower[i..].starts_with("select") && (i == 0 || !bytes[i - 1].is_ascii_alphanumeric()) => {
                start = Some(i + 6);
                break;
            }
            _ => {}
        }
        i += 1;
    }
    let Some(list_start) = start else { return sql.to_string() };
    // The end of the select list: " from " at depth 0 (or the end of the text).
    let mut depth = 0i32;
    let mut j = list_start;
    let mut end = bytes.len();
    let mut items: Vec<(usize, usize)> = Vec::new();
    let mut item_start = list_start;
    while j < bytes.len() {
        match bytes[j] {
            b'(' => depth += 1,
            b')' => depth -= 1,
            b'\'' => {
                j += 1;
                while j < bytes.len() && bytes[j] != b'\'' {
                    j += 1;
                }
            }
            b',' if depth == 0 => {
                items.push((item_start, j));
                item_start = j + 1;
            }
            c if depth == 0 && c.is_ascii_whitespace() && lower[j..].trim_start().starts_with("from") && lower[j..].trim_start()[4..].starts_with(|ch: char| ch.is_whitespace()) => {
                end = j;
                break;
            }
            _ => {}
        }
        j += 1;
    }
    items.push((item_start, end));
    let mut out = String::from(&sql[..list_start]);
    for (n, (a, b)) in items.iter().enumerate() {
        let item = &sql[*a..*b];
        let core = item.trim();
        let lower_core = core.to_ascii_lowercase();
        let distinct = lower_core.starts_with("distinct ");
        let has_alias = {
            let words: Vec<&str> = lower_core.split_whitespace().collect();
            words.len() >= 3 && words[words.len() - 2] == "as"
        };
        let expression = core.contains('(') || core.contains('\'') || core.contains('|') || core.contains('+') || lower_core.starts_with("case ");
        if n > 0 {
            out.push(',');
        }
        if expression && !has_alias && !distinct && core != "*" {
            let lead = &item[..item.len() - item.trim_start().len()];
            let trail = &item[item.trim_end().len()..];
            out.push_str(&format!("{lead}{core} as expr{}{trail}", n + 1));
        } else {
            out.push_str(item);
        }
    }
    out.push_str(&sql[end..]);
    out
}

pub fn sql_query(sql: &str, limit: usize) -> Result<(Vec<String>, Vec<Vec<Option<String>>>, bool)> {
    let sql = alias_expressions(sql);
    // EA shows a wrong query's error in a message box and waits for a click (SuppressEADialogs does not stop it):
    // while the query runs, such a box of this EA is closed here and its text goes back as the error.
    let ea_pid: u32 = ROT_NAME.get().and_then(|n| n.rsplit('.').next()).and_then(|p| p.parse().ok()).unwrap_or(0);
    let done = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let caught = std::sync::Arc::new(std::sync::Mutex::new(None::<String>));
    let watcher = {
        let (done, caught) = (done.clone(), caught.clone());
        std::thread::spawn(move || {
            while !done.load(std::sync::atomic::Ordering::Relaxed) {
                let found = if ea_pid != 0 { close_error_box(ea_pid) } else { ea_pids().into_iter().find_map(close_error_box) };
                if let Some(text) = found {
                    *caught.lock().unwrap_or_else(|p| p.into_inner()) = Some(text);
                }
                std::thread::sleep(Duration::from_millis(150));
            }
        })
    };
    let xml = with_running_ea(Duration::from_secs(60), move |repo| Ok(repo.call("SQLQuery", &[crate::com::s(&sql)])?.text()));
    done.store(true, std::sync::atomic::Ordering::Relaxed);
    let _ = watcher.join();
    if let Some(error) = caught.lock().unwrap_or_else(|p| p.into_inner()).take() {
        bail!("{}", error.lines().filter(|l| !l.trim().is_empty()).collect::<Vec<_>>().join(" "));
    }
    Ok(parse_sql_xml(&xml?, limit))
}

/// Closes a message box of the given EA process that reports a database/SQL error; returns its text.
fn close_error_box(ea_pid: u32) -> Option<String> {
    #[link(name = "user32")]
    extern "system" {
        fn EnumWindows(callback: extern "system" fn(isize, isize) -> i32, param: isize) -> i32;
        fn EnumChildWindows(parent: isize, callback: extern "system" fn(isize, isize) -> i32, param: isize) -> i32;
        fn GetWindowThreadProcessId(window: isize, process: *mut u32) -> u32;
        fn GetClassNameW(window: isize, name: *mut u16, max: i32) -> i32;
        fn GetWindowTextW(window: isize, text: *mut u16, max: i32) -> i32;
        fn IsWindowVisible(window: isize) -> i32;
        fn PostMessageW(window: isize, message: u32, w: usize, l: isize) -> i32;
    }
    struct Search {
        pid: u32,
        found: Vec<isize>,
    }
    fn class_of(window: isize) -> String {
        let mut buffer = [0u16; 64];
        // SAFETY: the buffer and its size match.
        let n = unsafe { GetClassNameW(window, buffer.as_mut_ptr(), 64) };
        String::from_utf16_lossy(&buffer[..n.max(0) as usize])
    }
    fn text_of(window: isize) -> String {
        let mut buffer = vec![0u16; 4096];
        // SAFETY: the buffer and its size match.
        let n = unsafe { GetWindowTextW(window, buffer.as_mut_ptr(), 4096) };
        String::from_utf16_lossy(&buffer[..n.max(0) as usize])
    }
    extern "system" fn top(window: isize, param: isize) -> i32 {
        // SAFETY: param points to the Search below for the duration of EnumWindows.
        let search = unsafe { &mut *(param as *mut Search) };
        let mut pid = 0u32;
        unsafe { GetWindowThreadProcessId(window, &mut pid) };
        if pid == search.pid && unsafe { IsWindowVisible(window) } != 0 && class_of(window) == "#32770" {
            search.found.push(window);
        }
        1
    }
    extern "system" fn child(window: isize, param: isize) -> i32 {
        // SAFETY: param points to the String below for the duration of EnumChildWindows.
        let text = unsafe { &mut *(param as *mut String) };
        if class_of(window) == "Static" {
            text.push_str(&text_of(window));
            text.push('\n');
        }
        1
    }
    if ea_pid == 0 {
        return None;
    }
    let mut search = Search { pid: ea_pid, found: Vec::new() };
    // SAFETY: the callback only uses the Search passed in, which outlives the call.
    unsafe { EnumWindows(top, &mut search as *mut Search as isize) };
    for window in search.found {
        let mut text = String::new();
        unsafe { EnumChildWindows(window, child, &mut text as *mut String as isize) };
        if text.contains("SQL") || text.contains("Database API") || text.contains("XML") || text.contains("Error processing") {
            // IDOK on the box's own OK button.
            unsafe { PostMessageW(window, 0x0111, 1, 0) };
            return Some(text);
        }
    }
    None
}

/// EA's SQLQuery result: `<EADATA><Dataset_0><Data><Row><col>value</col>…</Row>…`. Columns in the order of the
/// first row; a column missing in a row is NULL.
fn parse_sql_xml(xml: &str, limit: usize) -> (Vec<String>, Vec<Vec<Option<String>>>, bool) {
    let unescape = |v: &str| v.replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&apos;", "'").replace("&amp;", "&");
    let mut columns: Vec<String> = Vec::new();
    let mut rows = Vec::new();
    let mut truncated = false;
    for block in xml.split("<Row>").skip(1) {
        let body = block.split("</Row>").next().unwrap_or("");
        let mut cells: Vec<(String, Option<String>)> = Vec::new();
        let mut rest = body;
        while let Some(start) = rest.find('<') {
            let after = &rest[start + 1..];
            let Some(end) = after.find('>') else { break };
            let tag = &after[..end];
            if tag.ends_with('/') {
                cells.push((tag.trim_end_matches('/').trim().to_string(), None));
                rest = &after[end + 1..];
                continue;
            }
            let close = format!("</{tag}>");
            let content = &after[end + 1..];
            let Some(stop) = content.find(&close) else { break };
            cells.push((tag.to_string(), Some(unescape(&content[..stop]))));
            rest = &content[stop + close.len()..];
        }
        if columns.is_empty() {
            columns = cells.iter().map(|(name, _)| name.clone()).collect();
        }
        if rows.len() == limit {
            truncated = true;
            break;
        }
        // Repeated column names (e.g. o.name and p.name) are matched by their order of occurrence.
        rows.push(
            columns
                .iter()
                .enumerate()
                .map(|(index, c)| {
                    let nth = columns[..index].iter().filter(|p| p.eq_ignore_ascii_case(c)).count();
                    cells.iter().filter(|(name, _)| name.eq_ignore_ascii_case(c)).nth(nth).and_then(|(_, v)| v.clone())
                })
                .collect(),
        );
    }
    (columns, rows, truncated)
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

/// Model events the EA add-in passed on (a new relationship, a changed element): ("connector" | "element", id).
pub fn take_events() -> Vec<(String, i64)> {
    let Some(name) = ROT_NAME.get() else { return Vec::new() };
    let file = std::env::temp_dir().join(format!("loonar-ea-event-{name}.txt"));
    let Ok(text) = std::fs::read_to_string(&file) else { return Vec::new() };
    let _ = std::fs::remove_file(&file);
    let mut events: Vec<(String, i64)> = text
        .lines()
        .filter_map(|line| {
            let (kind, id) = line.trim().split_once(':')?;
            Some((kind.to_string(), id.trim().parse().ok()?))
        })
        .collect();
    events.dedup();
    events
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
/// Whether the model file of the settings is open in an EA window (cached for 3 s: it is asked before every read).
pub fn file_open_in_window(cfg: &crate::config::AppConfig) -> bool {
    static CACHE: std::sync::Mutex<Option<(std::time::Instant, PathBuf, bool)>> = std::sync::Mutex::new(None);
    if let Some((when, file, open)) = CACHE.lock().unwrap_or_else(|p| p.into_inner()).as_ref() {
        if *file == cfg.model_file && when.elapsed() < Duration::from_secs(3) {
            return *open;
        }
    }
    // Without any EA window there is nothing to ask (and no COM call to wait for).
    let open = ea_window_exists() && has_model_open(&Target::of(cfg));
    *CACHE.lock().unwrap_or_else(|p| p.into_inner()) = Some((std::time::Instant::now(), cfg.model_file.clone(), open));
    open
}

/// Process ids of the running EA instances (outside a tab of EA the window's process is not known in advance).
fn ea_pids() -> Vec<u32> {
    std::process::Command::new("tasklist")
        .args(["/FI", "IMAGENAME eq EA.exe", "/FO", "CSV", "/NH"])
        .creation_flags_hidden()
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).lines().filter_map(|l| l.split("\",\"").nth(1).and_then(|p| p.trim_matches('"').parse().ok())).collect())
        .unwrap_or_default()
}

/// Whether any visible Enterprise Architect main window exists.
fn ea_window_exists() -> bool {
    std::process::Command::new("tasklist")
        .args(["/FI", "IMAGENAME eq EA.exe", "/NH"])
        .creation_flags_hidden()
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).to_ascii_lowercase().contains("ea.exe"))
        .unwrap_or(false)
}

trait HiddenCommand {
    fn creation_flags_hidden(&mut self) -> &mut Self;
}

impl HiddenCommand for std::process::Command {
    fn creation_flags_hidden(&mut self) -> &mut Self {
        use std::os::windows::process::CommandExt;
        self.creation_flags(0x0800_0000)
    }
}

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
#[cfg(test)]
mod error_box_probe {
    /// Closes an SQL error box of an EA process (LOONAR_TEST_EA_PID) and prints its text.
    #[test]
    #[ignore]
    fn closes_sql_error_box() {
        let pid: u32 = std::env::var("LOONAR_TEST_EA_PID").unwrap().parse().unwrap();
        let mut text = None;
        for _ in 0..200 {
            text = super::close_error_box(pid);
            if text.is_some() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(300));
        }
        println!("caught: {text:?}");
        assert!(text.is_some());
    }
}

#[cfg(test)]
mod alias_tests {
    use super::alias_expressions;

    #[test]
    fn expressions_get_aliases() {
        assert_eq!(alias_expressions("select count(*) from t_object"), "select count(*) as expr1 from t_object");
        assert_eq!(alias_expressions("SELECT o.name, coalesce(o.note,'') FROM t_object o"), "SELECT o.name, coalesce(o.note,'') as expr2 FROM t_object o");
        assert_eq!(alias_expressions("select count(*) as n from t"), "select count(*) as n from t");
        assert_eq!(alias_expressions("select a from t where x in (select count(*) from u)"), "select a from t where x in (select count(*) from u)");
        assert_eq!(alias_expressions("select 'a, from b' from t"), "select 'a, from b' as expr1 from t");
        assert_eq!(alias_expressions("select * from t"), "select * from t");
    }
}

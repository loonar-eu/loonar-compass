//! Writing to Enterprise Architect through COM (`EA.Repository` automation).
//!
//! The model is opened and changed by a separate application thread that talks to EA through COM (module `ea_com`).
//! The EA session is shared by the whole application: it opens on the first call (starting EA and opening
//! the model takes tens of seconds), and subsequent tasks run in the open session in a fraction of a second.
//! The same code works on Windows and in Wine on macOS (without PowerShell).

use crate::config::{secrets, AppConfig};
use crate::ea_com;
use anyhow::{bail, Result};
use serde_json::Value;
use std::os::windows::process::CommandExt;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

const CREATE_NO_WINDOW: u32 = 0x0800_0000;
/// EA can open a modal window (e.g. a login prompt after a wrong password), and then the session waits forever.
const DEFAULT_TIMEOUT_SECS: u64 = 300;

/// Time limit of a single EA task. The `LOONAR_EA_TIMEOUT` variable (seconds) shortens it in tests.
fn timeout() -> Duration {
    let secs = std::env::var("LOONAR_EA_TIMEOUT").ok().and_then(|v| v.parse().ok()).unwrap_or(DEFAULT_TIMEOUT_SECS);
    Duration::from_secs(secs)
}

pub use crate::ops::{describe, MODEL_OPS};

#[derive(Default)]
pub struct BatchOptions {
    /// Key of a package created in this task; on error the package is deleted (all or nothing).
    pub rollback_key: Option<String>,
    /// Keep the user's locks after the write.
    pub keep_locks: bool,
}

static IN_EA_TAB: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Compass runs in a tab of the user's EA window: writes to the model that window has open go through it (also for a
/// database), so the user sees them at once and no second EA instance is started.
pub fn set_in_ea_tab() {
    IN_EA_TAB.store(true, std::sync::atomic::Ordering::Relaxed);
}

pub fn in_ea_tab() -> bool {
    IN_EA_TAB.load(std::sync::atomic::Ordering::Relaxed)
}

/// What EA opens: the path of an EA file, or the connection string of the model database built from the database
/// settings (EA needs the database account in it). The port follows a comma (`SRC=server,port@database`), the same way
/// EA itself writes it.
fn connection_string(cfg: &AppConfig) -> Result<String> {
    if cfg.uses_file() {
        return Ok(cfg.model_file.display().to_string());
    }
    // EA's own connection for the background session: `ea.conn` (the text of an EA shortcut, e.g. for MySQL and
    // Oracle through an ODBC data source) or one built for SQL Server (OLE DB) and PostgreSQL (Pro Cloud-style SSDB).
    // An ea.conn of another kind of database (left from an earlier model) is not used (LC-261).
    if !cfg.ea_conn.trim().is_empty() {
        if ea_conn_matches(&cfg.ea_conn, cfg.model_source) {
            return Ok(cfg.ea_conn.trim().to_string());
        }
    }
    // MySQL and Oracle: EA opens them through an ODBC data source of its own, which Compass cannot guess (never a
    // PostgreSQL connection to their port).
    if matches!(cfg.model_source, crate::config::ModelSource::MySql | crate::config::ModelSource::Oracle) {
        // The example carries the DBType of this kind of database (EA: MySQL 0, Oracle 3).
        let kind = if cfg.model_source == crate::config::ModelSource::Oracle { "3" } else { "0" };
        let other = cfg.ea_conn.split(';').find_map(|p| p.split_once('=').filter(|(k, _)| k.trim().ends_with("DBType")).map(|(_, v)| v.trim().to_string()));
        let mismatch = other.map(|t| format!("{} ", crate::i18n::fmt("The ea.conn in the settings is for another kind of database than the model (DBType {0}).", &[t]))).unwrap_or_default();
        bail!("{mismatch}{}", crate::i18n::fmt("EA needs its own connection to save in a MySQL or Oracle model: set ea.conn to the connection text of this repository in EA (e.g. \"ea_model --- DBType={0};Connect=Provider=MSDASQL.1;Persist Security Info=False;Data Source=<ODBC data source>;\"), or work in Compass inside EA with the repository open there.", &[kind.to_string()]));
    }
    let host = if cfg.db_host.trim().eq_ignore_ascii_case("localhost") { "127.0.0.1" } else { cfg.db_host.trim() };
    if cfg.model_source == crate::config::ModelSource::SqlServer {
        // No user: Windows authentication, as in Compass's own connection (LC-261).
        if cfg.db_user.trim().is_empty() {
            return Ok(format!(
                "{db} --- DBType=1;Connect=Provider=SQLOLEDB.1;Integrated Security=SSPI;Persist Security Info=False;Initial Catalog={db};Data Source={host},{port};LazyLoad=1;",
                db = cfg.db_name,
                port = cfg.db_port,
            ));
        }
        return Ok(format!(
            "{db} --- DBType=1;Connect=Provider=SQLOLEDB.1;Password={pwd};Persist Security Info=True;User ID={user};Initial Catalog={db};Data Source={host},{port};LazyLoad=1;",
            db = cfg.db_name,
            pwd = cfg.db_password,
            user = cfg.db_user,
            port = cfg.db_port,
        ));
    }
    Ok(format!(
        "{db} --- DBType=4;Connect=Provider=SSDB;SRC={host},{port}@{db};UID={user};PWD={pwd};",
        db = cfg.db_name,
        // EA does not connect to the database under the name "localhost" (it then opens an invisible window and waits forever;
        // verified 2026-10-13), but it works with the address 127.0.0.1, so the name is replaced with the address.
        host = if cfg.db_host.trim().eq_ignore_ascii_case("localhost") { "127.0.0.1" } else { cfg.db_host.trim() },
        port = cfg.db_port,
        user = cfg.db_user,
        pwd = cfg.db_password,
    ))
}

/// Whether EA's connection text is for the kind of database of the model (by its `DBType`; text without it is kept).
fn ea_conn_matches(conn: &str, source: crate::config::ModelSource) -> bool {
    use crate::config::ModelSource::*;
    let Some(t) = conn.split(';').find_map(|p| p.split_once('=').filter(|(k, _)| k.trim().rsplit(' ').next().is_some_and(|k| k.eq_ignore_ascii_case("DBType"))).map(|(_, v)| v.trim().to_string())) else {
        return true;
    };
    match source {
        MySql => t == "0",
        SqlServer => t == "1",
        Oracle => t == "3",
        Postgres => t == "4",
        File => false,
    }
}

/// After this many minutes without calls the persistent EA session is closed (releasing the license and memory).
/// The `LOONAR_EA_IDLE` variable (seconds) changes this time in tests.
const DEFAULT_IDLE_SECS: u64 = 600;

fn idle_limit() -> Duration {
    let secs = std::env::var("LOONAR_EA_IDLE").ok().and_then(|v| v.parse().ok()).unwrap_or(DEFAULT_IDLE_SECS);
    Duration::from_secs(secs)
}

/// Login details the session opens the model with.
#[derive(Clone, PartialEq, Eq)]
struct Login {
    connection: String,
    user: String,
    password: String,
    ea_exe: std::path::PathBuf,
}

/// Task for the EA thread: the list of operations and the reply channel.
struct Request {
    ops: Vec<Value>,
    rollback_key: Option<String>,
    keep_locks: bool,
    locks: bool,
    reply: mpsc::Sender<Value>,
}

/// EA thread: holds the open session (the Repository object) and runs tasks one after another.
struct Worker {
    requests: mpsc::Sender<Request>,
    login: Login,
    started_wall: chrono::DateTime<chrono::Local>,
    thread: std::thread::JoinHandle<()>,
}

static WORKER: std::sync::Mutex<Option<Worker>> = std::sync::Mutex::new(None);

impl Worker {
    fn start(login: Login) -> Self {
        let (requests, inbox) = mpsc::channel::<Request>();
        let thread_login = login.clone();
        let thread = std::thread::spawn(move || worker_loop(thread_login, inbox));
        let started_wall = chrono::Local::now();
        *BRIDGE_STARTED.lock().unwrap_or_else(|p| p.into_inner()) = Some(started_wall);
        Self { requests, login, started_wall, thread }
    }
}

/// EA thread loop. COM objects live only in this thread (single-threaded apartment, as EA requires).
fn worker_loop(login: Login, inbox: mpsc::Receiver<Request>) {
    use windows::Win32::System::Com::{CoInitializeEx, COINIT_APARTMENTTHREADED};
    // SAFETY: COM initialization for this thread, once, before any COM objects are used.
    let _ = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
    let mut repo: Option<crate::com::Disp> = None;
    loop {
        let request = match inbox.recv_timeout(idle_limit()) {
            Ok(request) => request,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                // Idle: close EA; the next task will open the session again.
                if let Some(open) = repo.take() {
                    ea_com::close(&open);
                }
                continue;
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        };
        if repo.is_none() {
            match ea_com::open(&login.connection, &login.user, &login.password, &login.ea_exe) {
                Ok(open) => repo = Some(open),
                Err(e) => {
                    let mut failed = serde_json::json!({ "ok": false, "error": format!("{e:#}"), "created": [], "warnings": [] });
                    if ea_com::is_session_lost(&failed["error"].to_string()) {
                        failed["session_lost"] = serde_json::json!(true);
                    }
                    let _ = request.reply.send(failed);
                    continue;
                }
            }
        }
        let open = repo.as_ref().expect("EA session open");
        let result = ea_com::run_request(open, &login.user, &request.ops, request.rollback_key.as_deref(), request.keep_locks, request.locks);
        if result["session_lost"] == true {
            repo = None;
        }
        let _ = request.reply.send(result);
    }
    if let Some(open) = repo.take() {
        ea_com::close(&open);
    }
}

/// Runs a list of operations in the persistent EA session (opening it on first use). Returns the reply
/// (`created`, `warnings`, `user`). An error means the operations were not all carried out; the message says
/// what was left in the model. Calls from different conversations run one after another.
pub fn run_batch(cfg: &AppConfig, ops: &[Value], options: &BatchOptions) -> Result<Value> {
    if ops.is_empty() {
        bail!("{}", crate::i18n::tr("empty list of operations"));
    }
    // A model (file or database) without security opens without an account.
    let anonymous = cfg.ea_user.trim().is_empty();
    // Whether the model (file or database) requires a user lock to edit: from its settings (on by default).
    let locks = cfg.ea_locks;
    let login = Login {
        connection: connection_string(cfg)?,
        user: cfg.ea_user.clone(),
        password: if anonymous {
            String::new()
        } else {
            // Tests and development builds may use a separate EA account of the test model (never in a release build).
            match std::env::var("LOONAR_TEST_EA_PASSWORD") {
                Ok(password) if cfg!(any(test, debug_assertions)) => password,
                // A file without security opens with any account, so a missing password does not stop it; a file with
                // security then reports EA's own login error.
                _ if cfg.uses_file() => secrets::get(&cfg.ea_password_key).unwrap_or_default(),
                _ => secrets::get(&cfg.ea_password_key)?,
            }
        },
        ea_exe: cfg.ea_path.clone(),
    };

    // An EA file the user has open is written through the user's EA window: a second EA instance must not open
    // the same file (a .qea file is meant for one user). The background session for it, if any, is closed.
    if cfg.uses_file() || IN_EA_TAB.load(std::sync::atomic::Ordering::Relaxed) {
        let target = crate::ea_ui::Target::of(cfg);
        let in_window = crate::ea_ui::model_in_window(&target);
        // An EA window that does not answer may have this very file open: a second EA must not write it (E3).
        if in_window.is_none() && cfg.uses_file() {
            bail!("{}", crate::i18n::tr("Your EA window does not answer (a dialog box may be open), and it may have this file open. Close the dialog in EA and try again; nothing was written."));
        }
        if in_window == Some(true) {
            WORKER.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).take();
            let result = crate::ea_ui::run_in_window(&target, cfg.ea_user.clone(), ops.to_vec(), options.rollback_key.clone(), options.keep_locks, locks, timeout())?;
            return check_result(ops, result);
        }
    }

    let mut guard = WORKER.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    // A session opened with different settings is closed (closing the channel ends the thread) and opened again.
    if guard.as_ref().is_some_and(|w| w.login != login) {
        guard.take();
    }
    if guard.is_none() {
        *guard = Some(Worker::start(login));
    }

    let (reply, answer) = mpsc::channel();
    let request = Request { ops: ops.to_vec(), rollback_key: options.rollback_key.clone(), keep_locks: options.keep_locks, locks, reply };
    let sent = guard.as_ref().expect("EA thread").requests.send(request).is_ok();
    let started = guard.as_ref().expect("EA thread").started_wall;
    // Waits in short slices: a message box of the background EA (which nobody sees) stops the job at once,
    // with the box's text, instead of after the whole time limit.
    let deadline = Instant::now() + timeout();
    let result = loop {
        if !sent {
            break Err(mpsc::RecvTimeoutError::Disconnected);
        }
        match answer.recv_timeout(Duration::from_secs(2)) {
            Err(mpsc::RecvTimeoutError::Timeout) if Instant::now() < deadline => {
                if let Some(text) = crate::ea_ui::background_message_box(started) {
                    if let Some(worker) = guard.take() {
                        kill_orphan_ea(worker.started_wall);
                    }
                    bail!("{}", crate::i18n::fmt("EA stopped the operation with a message: {0}. Nothing more was written.", &[if text.is_empty() { "?".to_string() } else { text }]));
                }
            }
            other => break other,
        }
    };
    let result = match result {
        Ok(result) => result,
        Err(mpsc::RecvTimeoutError::Timeout) => {
            // The EA thread hangs (most often on an invisible EA window): abandon it, close the EA started by
            // this session, and the next call starts from scratch.
            if let Some(worker) = guard.take() {
                kill_orphan_ea(worker.started_wall);
            }
            bail!("{}", crate::i18n::fmt("EA did not respond within {0} s and the operation was stopped. The most common causes: a wrong login or password of the EA account, wrong database details (server, account, password) or no license; EA then opens an invisible window and waits.", &[timeout().as_secs().to_string()]));
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => {
            guard.take();
            bail!("{}", crate::i18n::tr("the EA thread ended without a response"));
        }
    };
    if result["session_lost"] == true {
        guard.take();
    }
    // An EA project file is written to disk only when EA closes it: the background session ends after each write,
    // so the next read of the file (and the next write) sees the changes. A job of pings only (opening the session,
    // e.g. for a script run) keeps it open.
    if cfg.uses_file() && !ops.iter().all(|op| op["op"] == "ping") {
        drop(guard);
        shutdown();
    }
    check_result(ops, result)
}

/// A write that failed at one operation: the message for the user and the index of the failed operation, so the save
/// can journal the changes of existing objects made before it (LC-348).
#[derive(Debug)]
pub struct BatchError {
    pub message: String,
    pub failed_at: Option<usize>,
}

impl std::fmt::Display for BatchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for BatchError {}

/// Operations that only add objects (removed again by the rollback) or only read (exports): everything else changes
/// objects that existed before the write, and such a change stays when a later operation fails.
const ADDING_OPS: &[&str] = &[
    "create_package", "create_diagram", "create_element", "create_connector", "create_attribute", "create_operation", "ping",
    "export_diagram", "export_xmi", "generate_report", "create_baseline",
];

/// Turns an EA reply into the result of a write: an error message that says what failed and what was left in the model.
fn check_result(ops: &[Value], result: Value) -> Result<Value> {
    if result["ok"] != true {
        let error = result["error"].as_str().unwrap_or(crate::i18n::tr("unknown error"));
        let at = result["failed_op"].as_i64().filter(|i| *i >= 0);
        let left = result["created"].as_array().map_or(0, Vec::len);
        let mut msg = match at {
            Some(i) => crate::i18n::fmt("operation {0} ({1}) failed: {2}", &[(i + 1).to_string(), op_name(ops, i), error.to_string()]),
            None => error.to_string(),
        };
        // Changes of existing objects made before the failed operation are not rolled back (LC-348): say so.
        let changed_before: Vec<String> = at
            .map(|i| ops.iter().take(i as usize).enumerate().filter(|(_, op)| !ADDING_OPS.contains(&op["op"].as_str().unwrap_or(""))).map(|(n, _)| (n + 1).to_string()).collect())
            .unwrap_or_default();
        if result["rolled_back"] == true && changed_before.is_empty() {
            msg.push(' ');
            msg.push_str(crate::i18n::tr("The changes were rolled back; nothing was left in the model."));
        } else {
            if result["rolled_back"] == true {
                msg.push(' ');
                msg.push_str(crate::i18n::tr("The objects created before the error were removed."));
            } else if left > 0 {
                msg.push(' ');
                msg.push_str(&crate::i18n::fmt("{0} objects created before the error remain in the model.", &[left.to_string()]));
            }
            if !changed_before.is_empty() {
                msg.push(' ');
                msg.push_str(&crate::i18n::fmt("The changes of existing objects made by operations {0} stay in the model (they are in the undo journal).", &[changed_before.join(", ")]));
            }
        }
        return Err(BatchError { message: msg, failed_at: at.map(|i| i as usize) }.into());
    }
    Ok(result)
}

/// Closes the EA session when the application closes. Without this, the EA process started through COM stays in the background
/// and holds a license (the EA thread dies together with the application process before it closes EA). Waits up to 8 s for
/// EA to close, then kills the EA processes started by this session.
pub fn shutdown() {
    // A save in progress holds the session for up to its time limit (E8): closing does not wait for it, the EA started
    // by this session is closed instead.
    let deadline = std::time::Instant::now() + Duration::from_secs(8);
    let mut guard = loop {
        match WORKER.try_lock() {
            Ok(guard) => break guard,
            Err(std::sync::TryLockError::Poisoned(p)) => break p.into_inner(),
            Err(std::sync::TryLockError::WouldBlock) if std::time::Instant::now() < deadline => std::thread::sleep(Duration::from_millis(100)),
            Err(std::sync::TryLockError::WouldBlock) => {
                if let Some(started) = *BRIDGE_STARTED.lock().unwrap_or_else(|p| p.into_inner()) {
                    kill_orphan_ea(started);
                }
                return;
            }
        }
    };
    let worker = guard.take();
    drop(guard);
    let Some(worker) = worker else { return };
    let Worker { requests, started_wall, thread, .. } = worker;
    drop(requests); // end of the channel: the EA thread closes the session (Exit) and finishes
    let deadline = std::time::Instant::now() + Duration::from_secs(8);
    while !thread.is_finished() && std::time::Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(100));
    }
    if !thread.is_finished() {
        kill_orphan_ea(started_wall);
    }
}

/// When the current background EA session was started (for closing it without its lock, E8).
static BRIDGE_STARTED: std::sync::Mutex<Option<chrono::DateTime<chrono::Local>>> = std::sync::Mutex::new(None);

fn op_name(ops: &[Value], index: i64) -> String {
    ops.get(index as usize).and_then(|o| o["op"].as_str()).unwrap_or("?").to_string()
}

/// After a timeout, kills the EA processes started through COM since the bridge started
/// (the command line contains "Embedding"). The user's EA window does not have this parameter and is left untouched.
fn kill_orphan_ea(since: chrono::DateTime<chrono::Local>) {
    let stamp = since.format("%Y-%m-%dT%H:%M:%S").to_string();
    let cmd = format!(
        "$t=[datetime]'{stamp}'; Get-CimInstance Win32_Process -Filter \"Name='EA.exe'\" | \
         Where-Object {{ $_.CommandLine -match 'Embedding' -and $_.CreationDate -ge $t }} | \
         ForEach-Object {{ Stop-Process -Id $_.ProcessId -Force }}"
    );
    let _ = Command::new(crate::config::system_program("powershell.exe"))
        .args(["-NoProfile", "-NonInteractive", "-Command", &cmd])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .creation_flags(CREATE_NO_WINDOW)
        .status();
}


#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::path::Path;

    /// LC-261: a PostgreSQL ea.conn left from an earlier model is not used for SQL Server; no user = Windows authentication.
    #[test]
    fn ea_conn_of_another_database_is_not_used() {
        use crate::config::ModelSource;
        let pg = "ea1 --- DBType=4;Connect=Provider=SSDB;SRC=10.0.0.1,5432@eapx;UID=u;PWD=p;";
        assert!(ea_conn_matches(pg, ModelSource::Postgres));
        assert!(!ea_conn_matches(pg, ModelSource::SqlServer));
        assert!(ea_conn_matches("ea_model --- DBType=0;Connect=Provider=MSDASQL.1;Data Source=x;", ModelSource::MySql));
        assert!(ea_conn_matches("no type here", ModelSource::Oracle));
        let dir = std::env::temp_dir().join(format!("loonar-lc261-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("app.properties"), format!("model.source=sqlserver
db.host=localhost
db.port=1433
db.name=ea_lab
db.user=
ea.conn={pg}
")).unwrap();
        let cfg = AppConfig::load(&dir.join("app.properties")).unwrap();
        let c = connection_string(&cfg).unwrap();
        assert!(c.contains("DBType=1") && c.contains("Integrated Security=SSPI") && c.contains("127.0.0.1,1433") && !c.contains("User ID"), "{c}");
    }

    /// LC-228: a message box of the background EA (here "File not found") stops the job within seconds, with its
    /// text, instead of after the time limit. `cargo test background_box_stops_job -- --ignored --nocapture`
    #[test]
    #[ignore = "requires EA"]
    fn background_box_stops_job() {
        let dir = std::env::temp_dir().join(format!("loonar-lc228-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("app.properties"), "model.source=file
model.file=C:/nie-ma/takiego/pliku.qea
").unwrap();
        let cfg = AppConfig::load(&dir.join("app.properties")).unwrap();
        let start = Instant::now();
        let result = run_batch(&cfg, &[json!({ "op": "set_notes", "element": 1, "notes": "x" })], &BatchOptions::default());
        println!("{:?} after {:?}", result.as_ref().err().map(|e| e.to_string()), start.elapsed());
        assert!(result.is_err());
        assert!(start.elapsed() < Duration::from_secs(60), "stopped quickly");
    }

    /// Closes the EA session at the end of a test, also after a failure (otherwise EA stays in the background and holds a license).
    struct CloseEa;
    impl Drop for CloseEa {
        fn drop(&mut self) {
            shutdown();
        }
    }

    fn test_config() -> AppConfig {
        // LOONAR_TEST_PROPS points the whole series at another model (a second database or a file copy).
        let props = std::env::var("LOONAR_TEST_PROPS").map(std::path::PathBuf::from).unwrap_or_else(|_| Path::new(env!("CARGO_MANIFEST_DIR")).join("app.properties"));
        let mut cfg = AppConfig::load(&props).expect("app.properties");
        if let Ok(user) = std::env::var("LOONAR_TEST_EA_USER") {
            cfg.ea_user = user;
        }
        cfg
    }

    /// Adding a step to a subprocess created in an earlier write (after the locks were released): with
    /// "Require User Lock to Edit", EA used to answer "AddNew: Element locked".
    /// Run with: `cargo test ea_child_of_existing -- --ignored --nocapture`.
    #[test]
    #[ignore = "requires EA, a license and the model database from app.properties"]
    fn ea_child_of_existing() {
        let cfg = test_config();
        let _ea = CloseEa;
        let q = |sql: &str| crate::tools::query_rows(&cfg, sql).expect(sql);
        let root: i64 = q("select package_id from t_package where parent_id = 0 order by package_id limit 1")[0][0].parse().unwrap();
        let name = format!("ZZ_test_mostu_{}", chrono::Local::now().format("%H%M%S"));
        let ops = vec![
            json!({"op":"create_package","key":"pkg","parent":root,"name":name}),
            json!({"op":"create_diagram","key":"dia","package":"#pkg","name":"Process","type":"BPMN2.0::Business Process"}),
            json!({"op":"create_element","key":"sub","package":"#pkg","name":"Subprocess","type":"Activity",
                   "stereotype":"BPMN2.0::Activity","tags":{"activityType":"Sub-Process"}}),
            json!({"op":"place","diagram":"#dia","element":"#sub","left":40,"top":40,"width":600,"height":200}),
        ];
        let created = run_batch(&cfg, &ops, &BatchOptions { rollback_key: Some("pkg".into()), keep_locks: false }).expect("creation");
        let id_of = |key: &str| created["created"].as_array().unwrap().iter().find(|c| c["key"] == key).expect(key)["id"].as_i64().unwrap();
        let (pkg, dia, sub) = (id_of("pkg"), id_of("dia"), id_of("sub"));
        let second = run_batch(
            &cfg,
            &[
                json!({"op":"set_bounds","diagram":dia,"element":sub,"height":260}),
                json!({"op":"create_element","key":"step","parent_element":sub,"name":"Step","type":"Activity","stereotype":"BPMN2.0::Activity"}),
                json!({"op":"place","diagram":dia,"element":"#step","left":120,"top":100,"width":110,"height":60}),
            ],
            &BatchOptions::default(),
        );
        let children = q(&format!("select count(*) from t_object where parentid = {sub}"));
        println!("second write: {second:?} | steps in the subprocess: {}", children[0][0]);
        run_batch(&cfg, &[json!({"op":"delete","package":pkg})], &BatchOptions::default()).expect("deleting the test package");
        shutdown();
        second.expect("adding a step to an existing subprocess");
        assert_eq!(children[0][0], "1");
    }

    /// Keywords and a legend with automatic coloring by Element.Keyword.
    /// Run with: `cargo test ea_keywords_legend -- --ignored --nocapture` (image in %TEMP%\loonar_legend.png).
    #[test]
    #[ignore = "requires EA, a license and the model database from app.properties"]
    fn ea_keywords_legend() {
        let cfg = test_config();
        let _ea = CloseEa;
        let q = |sql: &str| crate::tools::query_rows(&cfg, sql).expect(sql);
        let root: i64 = q("select package_id from t_package where parent_id = 0 order by package_id limit 1")[0][0].parse().unwrap();
        let name = format!("ZZ_test_mostu_{}", chrono::Local::now().format("%H%M%S"));
        let ops = vec![
            json!({"op":"create_package","key":"pkg","parent":root,"name":name}),
            json!({"op":"create_diagram","key":"dia","package":"#pkg","name":"Priorities","type":"Use Case"}),
            json!({"op":"create_element","key":"a","package":"#pkg","name":"UC-1","type":"UseCase","keywords":"prio1"}),
            json!({"op":"create_element","key":"b","package":"#pkg","name":"UC-2","type":"UseCase"}),
            json!({"op":"place","diagram":"#dia","element":"#a","left":40,"top":40,"width":140,"height":70}),
            json!({"op":"place","diagram":"#dia","element":"#b","left":240,"top":40,"width":140,"height":70}),
            json!({"op":"create_legend","diagram":"#dia","title":"Priority","filter":"Element.Keyword","left":440,"top":30,
                   "entries":[{"value":"prio1","color":"#E24B4A","label":"prio1 - critical"},{"value":"prio2","color":"#97C459","label":"prio2 - low"}]}),
        ];
        let created = run_batch(&cfg, &ops, &BatchOptions { rollback_key: Some("pkg".into()), keep_locks: false }).expect("creation");
        let id_of = |key: &str| created["created"].as_array().unwrap().iter().find(|c| c["key"] == key).expect(key)["id"].as_i64().unwrap();
        let (pkg, dia, b) = (id_of("pkg"), id_of("dia"), id_of("b"));
        run_batch(&cfg, &[json!({"op":"update_element","element":b,"keywords":["prio2"]})], &BatchOptions::default()).expect("keyword");
        let keywords = q(&format!("select name, coalesce(pdata5,'') from t_object where package_id = {pkg} and object_type = 'UseCase' order by name"));
        let legend = q(&format!("select ntype, styleex from t_object where package_id = {pkg} and object_type = 'Text'"));
        let image = std::env::temp_dir().join("loonar_legend.png");
        let _ = run_batch(&cfg, &[json!({"op":"export_diagram","diagram":dia,"path":image.display().to_string()})], &BatchOptions::default());
        println!("keywords: {keywords:?} | legend: {legend:?} | image: {}", image.display());
        run_batch(&cfg, &[json!({"op":"delete","package":pkg})], &BatchOptions::default()).expect("deleting the test package");
        assert_eq!(keywords, vec![vec!["UC-1".to_string(), "prio1".to_string()], vec!["UC-2".to_string(), "prio2".to_string()]]);
        assert_eq!(legend[0][0], "76");
    }

    /// Attribute, operation, baseline and package clone on a real EA (restoring a baseline through COM hangs EA, so it is not covered).
    /// Run with: `cargo test ea_class_members_and_baseline -- --ignored --nocapture`.
    #[test]
    #[ignore = "requires EA, a license and the model database from app.properties"]
    fn ea_class_members_and_baseline() {
        let cfg = test_config();
        let _ea = CloseEa;
        let q = |sql: &str| crate::tools::query_rows(&cfg, sql).expect(sql);
        let root: i64 = q("select package_id from t_package where parent_id = 0 order by package_id limit 1")[0][0].parse().unwrap();
        let name = format!("ZZ_test_mostu_{}", chrono::Local::now().format("%H%M%S"));
        let ops = vec![
            json!({"op":"create_package","key":"pkg","parent":root,"name":name}),
            json!({"op":"create_element","key":"a","package":"#pkg","name":"Customer","type":"Class"}),
            json!({"op":"create_attribute","element":"#a","name":"taxId","type":"String","visibility":"Private"}),
            json!({"op":"create_operation","element":"#a","name":"verify","return_type":"Boolean","parameters":[{"name":"code","type":"String"}]}),
            json!({"op":"create_baseline","package":"#pkg","version":"1.0","notes":"before the change"}),
        ];
        let created = run_batch(&cfg, &ops, &BatchOptions { rollback_key: Some("pkg".into()), keep_locks: false }).expect("creation");
        let id_of = |key: &str| created["created"].as_array().unwrap().iter().find(|c| c["key"] == key).expect(key)["id"].as_i64().unwrap();
        let (pkg, a) = (id_of("pkg"), id_of("a"));
        let members = q(&format!("select (select count(*) from t_attribute where object_id = {a}), (select count(*) from t_operation where object_id = {a})"));
        let guid = q(&format!("select ea_guid from t_package where package_id = {pkg}"))[0][0].clone();
        let baselines = q(&format!("select docid from t_document where elementid = '{guid}' and doctype = 'Baseline'"));
        println!("attributes/operations: {:?} | baselines: {baselines:?}", members[0]);
        let cloned = run_batch(&cfg, &[json!({"op":"clone_package","key":"copy","package":pkg,"name":format!("{name}_copy")})], &BatchOptions::default()).expect("clone");
        let copy = cloned["created"][0]["id"].as_i64().unwrap();
        let copied = q(&format!("select count(*) from t_object where package_id = {copy}"));
        println!("elements in the copy: {}", copied[0][0]);
        run_batch(&cfg, &[json!({"op":"delete","package":copy})], &BatchOptions::default()).expect("deleting the copy");
        run_batch(&cfg, &[json!({"op":"delete","package":pkg})], &BatchOptions::default()).expect("deleting the test package");
        shutdown();
        assert_eq!(members[0], vec!["1".to_string(), "1".to_string()]);
        assert_eq!(baselines.len(), 1);
        assert_eq!(copied[0][0], "1");
    }

    /// A new version of an element (EA's former "Clone as New Version"): members, tags, next version, «trace» to the
    /// original. Run with: `cargo test ea_clone_element_version -- --ignored --nocapture`.
    #[test]
    #[ignore = "requires EA, a license and the model database from app.properties"]
    fn ea_clone_element_version() {
        let cfg = test_config();
        let _ea = CloseEa;
        let q = |sql: &str| crate::tools::query_rows(&cfg, sql).expect(sql);
        let root: i64 = q("select package_id from t_package where parent_id = 0 order by package_id limit 1")[0][0].parse().unwrap();
        let name = format!("ZZ_test_wersji_{}", chrono::Local::now().format("%H%M%S"));
        let ops = vec![
            json!({"op":"create_package","key":"pkg","parent":root,"name":name}),
            json!({"op":"create_element","key":"a","package":"#pkg","name":"Zamówienie","type":"Class","stereotype":"entity","notes":"Opis","tags":{"owner":"Team A"}}),
            json!({"op":"create_attribute","element":"#a","name":"total","type":"Decimal","visibility":"Private"}),
            json!({"op":"create_operation","element":"#a","name":"pay","return_type":"Boolean","parameters":[{"name":"amount","type":"Decimal"}]}),
        ];
        let created = run_batch(&cfg, &ops, &BatchOptions { rollback_key: Some("pkg".into()), keep_locks: false }).expect("creation");
        let id_of = |v: &Value, key: &str| v["created"].as_array().unwrap().iter().find(|c| c["key"] == key).expect(key)["id"].as_i64().unwrap();
        let (pkg, a) = (id_of(&created, "pkg"), id_of(&created, "a"));
        let cloned = run_batch(&cfg, &[json!({"op":"clone_element_version","key":"v2","element":a})], &BatchOptions::default()).expect("new version");
        let v2 = id_of(&cloned, "v2");
        let row = q(&format!(
            "select o.name, o.version, coalesce(o.stereotype,''), coalesce(o.note,''), (select count(*) from t_attribute where object_id = o.object_id), \
             (select count(*) from t_operation where object_id = o.object_id), (select count(*) from t_operationparams p join t_operation m on m.operationid = p.operationid where m.object_id = o.object_id), \
             (select count(*) from t_objectproperties where object_id = o.object_id and property = 'owner'), \
             (select count(*) from t_connector c where c.start_object_id = o.object_id and c.end_object_id = {a} and c.connector_type = 'Abstraction' and c.stereotype = 'trace'), \
             (select version from t_object where object_id = {a}) from t_object o where o.object_id = {v2}"
        ));
        println!("new version: {:?}", row[0]);
        run_batch(&cfg, &[json!({"op":"delete","package":pkg})], &BatchOptions::default()).expect("deleting the test package");
        shutdown();
        let r = &row[0];
        assert_eq!(r[0], "Zamówienie");
        assert!(!r[1].is_empty() && r[1] != r[9], "version {} after {}", r[1], r[9]);
        assert_eq!((r[2].as_str(), r[3].as_str()), ("entity", "Opis"));
        assert_eq!((r[4].as_str(), r[5].as_str(), r[6].as_str(), r[7].as_str(), r[8].as_str()), ("1", "1", "1", "1", "1"));
    }

    /// Renaming a diagram (update_diagram) and hiding a connector on the diagram (hide_connectors).
    /// Run with: `cargo test ea_rename_and_hide -- --ignored --nocapture`.
    #[test]
    #[ignore = "requires EA, a license and the model database from app.properties"]
    fn ea_rename_and_hide() {
        let cfg = test_config();
        let _ea = CloseEa;
        let q = |sql: &str| crate::tools::query_rows(&cfg, sql).expect(sql);
        let root: i64 = q("select package_id from t_package where parent_id = 0 order by package_id limit 1")[0][0].parse().unwrap();
        let name = format!("ZZ_test_mostu_{}", chrono::Local::now().format("%H%M%S"));
        let ops = vec![
            json!({"op":"create_package","key":"pkg","parent":root,"name":name}),
            json!({"op":"create_diagram","key":"dia","package":"#pkg","name":"Old name","type":"Activity"}),
            json!({"op":"create_element","key":"a","package":"#pkg","name":"A","type":"Activity"}),
            json!({"op":"create_element","key":"b","package":"#pkg","name":"B","type":"Activity"}),
            json!({"op":"create_connector","key":"c","source":"#a","target":"#b","type":"ControlFlow"}),
            json!({"op":"place","diagram":"#dia","element":"#a","left":40,"top":40,"width":110,"height":60}),
            json!({"op":"place","diagram":"#dia","element":"#b","left":300,"top":40,"width":110,"height":60}),
        ];
        let created = run_batch(&cfg, &ops, &BatchOptions { rollback_key: Some("pkg".into()), keep_locks: false }).expect("creation");
        let id_of = |key: &str| created["created"].as_array().unwrap().iter().find(|c| c["key"] == key).expect(key)["id"].as_i64().unwrap();
        let (pkg, dia, c) = (id_of("pkg"), id_of("dia"), id_of("c"));
        run_batch(
            &cfg,
            &[
                json!({"op":"update_diagram","diagram":dia,"name":"New name","notes":"description"}),
                json!({"op":"hide_connectors","diagram":dia,"connectors":[c]}),
            ],
            &BatchOptions::default(),
        )
        .expect("renaming and hiding");
        let diagram = q(&format!("select name, notes from t_diagram where diagram_id = {dia}"));
        let hidden = q(&format!("select hidden from t_diagramlinks where diagramid = {dia} and connectorid = {c}"));
        let connectors = q(&format!("select count(*) from t_connector where connector_id = {c}"));
        println!("diagram: {diagram:?} | hidden: {hidden:?} | connector in the model: {}", connectors[0][0]);
        run_batch(&cfg, &[json!({"op":"delete","package":pkg})], &BatchOptions::default()).expect("deleting the test package");
        shutdown();
        assert_eq!(diagram[0][0], "New name");
        assert!(matches!(hidden[0][0].as_str(), "1" | "t" | "true"), "hidden: {:?}", hidden);
        assert_eq!(connectors[0][0], "1");
    }

    /// The size of an exported diagram image (LC-305). Run with: `cargo test ea_export_size -- --ignored --nocapture`.
    #[test]
    #[ignore = "requires EA, a license and a test model (LOONAR_TEST_PROPS)"]
    fn ea_export_size() {
        let cfg = test_config();
        let _ea = CloseEa;
        let q = |sql: &str| crate::tools::query_rows(&cfg, sql).expect(sql);
        let root: i64 = q("select package_id from t_package where parent_id = 0 order by package_id limit 1")[0][0].parse().unwrap();
        let ops = vec![
            json!({"op":"create_package","key":"pkg","parent":root,"name":"ZZ_test_export"}),
            json!({"op":"create_diagram","key":"dia","package":"#pkg","name":"Export","type":"Logical"}),
            json!({"op":"create_element","key":"a","package":"#pkg","name":"Requirement A","type":"Requirement"}),
            json!({"op":"place","diagram":"#dia","element":"#a","left":40,"top":40,"width":200,"height":80}),
        ];
        let created = run_batch(&cfg, &ops, &BatchOptions { rollback_key: Some("pkg".into()), keep_locks: false }).expect("creation");
        let id_of = |key: &str| created["created"].as_array().unwrap().iter().find(|c| c["key"] == key).expect(key)["id"].as_i64().unwrap();
        let (pkg, dia) = (id_of("pkg"), id_of("dia"));
        let path = std::env::temp_dir().join("loonar_export_size.png");
        run_batch(&cfg, &[json!({"op":"export_diagram","diagram":dia,"path":path.to_string_lossy()})], &BatchOptions::default()).expect("export");
        let size = image::image_dimensions(&path).expect("png");
        let sharp = std::env::temp_dir().join("loonar_export_size_x2.png");
        run_batch(&cfg, &[json!({"op":"export_diagram","diagram":dia,"path":sharp.to_string_lossy(),"scale":2.0})], &BatchOptions::default()).expect("export x2");
        let size2 = image::image_dimensions(&sharp).expect("png x2");
        // Sizes of the saved PNG files themselves (read back from disk), so a reader of the log sees the exported result.
        println!("EXPORT saved file {} is {}x{} px | saved file {} (scale 2, drawn from the metafile) is {}x{} px", path.display(), size.0, size.1, sharp.display(), size2.0, size2.1);
        assert!(size2.0 >= size.0 * 3 / 2 && size2.1 >= size.1 * 3 / 2, "twice as large");
        run_batch(&cfg, &[json!({"op":"delete","package":pkg})], &BatchOptions::default()).expect("deleting the test package");
        shutdown();
    }

    /// Duplicate links of one connector on a diagram (LC-303): a hidden link left by hide_connectors plus a visible one
    /// added by a script kept the arrow invisible (EA uses the first). Showing the connector and repair_diagram_links
    /// leave one visible link. Run with: `cargo test ea_duplicate_links -- --ignored --nocapture` (LOONAR_TEST_PROPS = a
    /// file copy, e.g. of EABase.qea).
    #[test]
    #[ignore = "requires EA, a license and a test model (LOONAR_TEST_PROPS)"]
    fn ea_duplicate_links() {
        let cfg = test_config();
        let _ea = CloseEa;
        let q = |sql: &str| crate::tools::query_rows(&cfg, sql).expect(sql);
        let root: i64 = q("select package_id from t_package where parent_id = 0 order by package_id limit 1")[0][0].parse().unwrap();
        let ops = vec![
            json!({"op":"create_package","key":"pkg","parent":root,"name":"ZZ_test_links"}),
            json!({"op":"create_diagram","key":"dia","package":"#pkg","name":"Links","type":"Logical"}),
            json!({"op":"create_element","key":"a","package":"#pkg","name":"Requirement A","type":"Requirement"}),
            json!({"op":"create_element","key":"b","package":"#pkg","name":"System B","type":"Component"}),
            json!({"op":"create_connector","key":"c","source":"#a","target":"#b","type":"Dependency"}),
            // A dependency created with a diagram given (as the model did): it must not be bound to that diagram.
            json!({"op":"create_connector","key":"d","source":"#b","target":"#a","type":"Dependency","diagram":"#dia"}),
            json!({"op":"place","diagram":"#dia","element":"#a","left":40,"top":40,"width":120,"height":60}),
            json!({"op":"place","diagram":"#dia","element":"#b","left":320,"top":40,"width":120,"height":60}),
        ];
        let created = run_batch(&cfg, &ops, &BatchOptions { rollback_key: Some("pkg".into()), keep_locks: false }).expect("creation");
        let id_of = |key: &str| created["created"].as_array().unwrap().iter().find(|c| c["key"] == key).expect(key)["id"].as_i64().unwrap();
        let (pkg, dia, c, d) = (id_of("pkg"), id_of("dia"), id_of("c"), id_of("d"));
        let unbound = q(&format!("select diagramid from t_connector where connector_id = {d}"));
        let rows = |label: &str| {
            let r = q(&format!("select instance_id, hidden from t_diagramlinks where diagramid = {dia} and connectorid = {c} order by instance_id"));
            println!("{label}: {r:?}");
            r
        };
        let visible = |r: &Vec<Vec<String>>| r.len() == 1 && matches!(r[0][1].as_str(), "0" | "f" | "false");
        // The broken state: hidden by Compass, then a second, visible row from a script.
        let duplicate = format!("INSERT INTO t_diagramlinks (DiagramID, ConnectorID, Hidden) VALUES ({dia}, {c}, 0)");
        let bind = format!("UPDATE t_connector SET DiagramID = {dia} WHERE Connector_ID = {c}");
        run_batch(&cfg, &[json!({"op":"hide_connectors","diagram":dia,"connectors":[c]}), json!({"op":"execute_sql","statements":[duplicate]})], &BatchOptions::default()).expect("broken state");
        assert_eq!(rows("hidden + duplicate").len(), 2);
        // Showing the connector leaves one visible link.
        run_batch(&cfg, &[json!({"op":"hide_connectors","diagram":dia,"connectors":[c],"hidden":false})], &BatchOptions::default()).expect("show");
        let shown = rows("after hide_connectors hidden=false");
        // The repair operation: duplicates removed, a hidden link with both ends on the diagram shown.
        run_batch(&cfg, &[json!({"op":"hide_connectors","diagram":dia,"connectors":[c]}), json!({"op":"execute_sql","statements":[duplicate, bind]})], &BatchOptions::default()).expect("broken again");
        let repaired = run_batch(&cfg, &[json!({"op":"repair_diagram_links","diagram":dia,"show_hidden":true})], &BatchOptions::default()).expect("repair");
        println!("repair result: {}", repaired["created"]);
        let after = rows("after repair_diagram_links");
        let bound_after = q(&format!("select diagramid from t_connector where connector_id = {c}"));
        println!("created with a diagram: diagramid {unbound:?}; bound one after repair: {bound_after:?}");
        // Only hidden (no duplicate): the repair with show_hidden shows it.
        run_batch(&cfg, &[json!({"op":"hide_connectors","diagram":dia,"connectors":[c]})], &BatchOptions::default()).expect("hidden only");
        let hidden_only = rows("hidden only");
        let shown_by_repair = run_batch(&cfg, &[json!({"op":"repair_diagram_links","diagram":dia,"show_hidden":true})], &BatchOptions::default()).expect("repair hidden");
        println!("repair of a hidden link: {}", shown_by_repair["created"]);
        let after_show = rows("after repair with show_hidden");
        run_batch(&cfg, &[json!({"op":"delete","package":pkg})], &BatchOptions::default()).expect("deleting the test package");
        shutdown();
        assert!(visible(&shown), "{shown:?}");
        assert!(visible(&after), "{after:?}");
        assert_eq!(unbound[0][0], "0", "a dependency is not bound to a diagram");
        assert_eq!(bound_after[0][0], "0", "repair unbinds it");
        assert!(!visible(&hidden_only), "{hidden_only:?}");
        assert_eq!(shown_by_repair["created"][0]["hidden_shown"], 1, "the hidden link is shown");
        assert!(visible(&after_show), "{after_show:?}");
    }

    /// Connector route (route_connector): from the left side of B to the left, then up to the bottom side of A.
    /// Run with: `cargo test ea_route_connector -- --ignored --nocapture` (image in %TEMP%\loonar_route.png).
    #[test]
    #[ignore = "requires EA, a license and the model database from app.properties"]
    fn ea_route_connector() {
        let cfg = test_config();
        let _ea = CloseEa;
        let q = |sql: &str| crate::tools::query_rows(&cfg, sql).expect(sql);
        let root: i64 = q("select package_id from t_package where parent_id = 0 order by package_id limit 1")[0][0].parse().unwrap();
        let name = format!("ZZ_test_mostu_{}", chrono::Local::now().format("%H%M%S"));
        let ops = vec![
            json!({"op":"create_package","key":"pkg","parent":root,"name":name}),
            json!({"op":"create_diagram","key":"dia","package":"#pkg","name":"Route","type":"Activity"}),
            json!({"op":"create_element","key":"a","package":"#pkg","name":"A","type":"Activity"}),
            json!({"op":"create_element","key":"b","package":"#pkg","name":"B","type":"Activity"}),
            json!({"op":"create_connector","key":"c","source":"#b","target":"#a","type":"ControlFlow"}),
            json!({"op":"place","diagram":"#dia","element":"#a","left":60,"top":40,"width":110,"height":60}),
            json!({"op":"place","diagram":"#dia","element":"#b","left":300,"top":200,"width":110,"height":60}),
        ];
        let created = run_batch(&cfg, &ops, &BatchOptions { rollback_key: Some("pkg".into()), keep_locks: false }).expect("creation");
        let id_of = |key: &str| created["created"].as_array().unwrap().iter().find(|c| c["key"] == key).expect(key)["id"].as_i64().unwrap();
        let (pkg, dia, c) = (id_of("pkg"), id_of("dia"), id_of("c"));
        run_batch(
            &cfg,
            &[json!({"op":"route_connector","diagram":dia,"connector":c,"start":[300,230],"points":[[115,230]],"end":[115,100]})],
            &BatchOptions::default(),
        )
        .expect("route");
        let link = q(&format!("select style, path, geometry from t_diagramlinks where diagramid = {dia}"));
        println!("link: {link:?}");
        let image = std::env::temp_dir().join("loonar_route.png");
        let _ = run_batch(&cfg, &[json!({"op":"export_diagram","diagram":dia,"path":image.display().to_string()})], &BatchOptions::default());
        run_batch(&cfg, &[json!({"op":"delete","package":pkg})], &BatchOptions::default()).expect("deleting the test package");
        shutdown();
        assert_eq!(link.len(), 1);
        assert!(link[0][1].contains("115:-230"), "path: {}", link[0][1]);
    }

    /// Connector line style (set_line_style): the diagram link gets Mode=3;TREE=OS.
    /// Run with: `cargo test ea_line_style -- --ignored --nocapture`.
    #[test]
    #[ignore = "requires EA, a license and the model database from app.properties"]
    fn ea_line_style() {
        let cfg = test_config();
        let _ea = CloseEa;
        let q = |sql: &str| crate::tools::query_rows(&cfg, sql).expect(sql);
        let root: i64 = q("select package_id from t_package where parent_id = 0 order by package_id limit 1")[0][0].parse().unwrap();
        let name = format!("ZZ_test_mostu_{}", chrono::Local::now().format("%H%M%S"));
        let ops = vec![
            json!({"op":"create_package","key":"pkg","parent":root,"name":name}),
            json!({"op":"create_diagram","key":"dia","package":"#pkg","name":"Lines","type":"Activity"}),
            json!({"op":"create_element","key":"a","package":"#pkg","name":"A","type":"Activity"}),
            json!({"op":"create_element","key":"b","package":"#pkg","name":"B","type":"Activity"}),
            json!({"op":"create_connector","source":"#a","target":"#b","type":"ControlFlow"}),
            json!({"op":"place","diagram":"#dia","element":"#a","left":40,"top":40,"width":110,"height":60}),
            json!({"op":"place","diagram":"#dia","element":"#b","left":300,"top":200,"width":110,"height":60}),
        ];
        let created = run_batch(&cfg, &ops, &BatchOptions { rollback_key: Some("pkg".into()), keep_locks: false }).expect("creation");
        let id_of = |key: &str| created["created"].as_array().unwrap().iter().find(|c| c["key"] == key).expect(key)["id"].as_i64().unwrap();
        let (pkg, dia) = (id_of("pkg"), id_of("dia"));
        run_batch(&cfg, &[json!({"op":"set_line_style","diagram":dia,"style":"orthogonal_square"})], &BatchOptions::default()).expect("line style");
        let styles = q(&format!("select style from t_diagramlinks where diagramid = {dia}"));
        println!("link styles: {styles:?}");
        let image = std::env::temp_dir().join("loonar_line_style.png");
        let _ = run_batch(&cfg, &[json!({"op":"export_diagram","diagram":dia,"path":image.display().to_string()})], &BatchOptions::default());
        println!("image: {}", image.display());
        run_batch(&cfg, &[json!({"op":"delete","package":pkg})], &BatchOptions::default()).expect("deleting the test package");
        shutdown();
        assert_eq!(styles.len(), 1);
        assert!(styles[0][0].contains("Mode=3") && styles[0][0].contains("TREE=OS"), "style: {}", styles[0][0]);
    }

    /// Refusal because of another account's lock: the error says who holds the lock. The test creates the admin account's lock
    /// directly in the test database. Run with: `cargo test ea_lock_report -- --ignored --nocapture`.
    #[test]
    #[ignore = "requires EA, a license and the model database from app.properties (test database: the test writes a lock)"]
    fn ea_lock_report() {
        crate::i18n::set(crate::i18n::Lang::Pl); // the test checks the Polish lock message
        let cfg = test_config();
        let _ea = CloseEa;
        let q = |sql: &str| crate::tools::query_rows(&cfg, sql).expect(sql);
        let root: i64 = q("select package_id from t_package where parent_id = 0 order by package_id limit 1")[0][0].parse().unwrap();
        let name = format!("ZZ_test_mostu_{}", chrono::Local::now().format("%H%M%S"));
        let ops = vec![
            json!({"op":"create_package","key":"pkg","parent":root,"name":name}),
            json!({"op":"create_element","key":"sub","package":"#pkg","name":"Subprocess","type":"Activity","stereotype":"BPMN2.0::Activity"}),
        ];
        let created = run_batch(&cfg, &ops, &BatchOptions { rollback_key: Some("pkg".into()), keep_locks: false }).expect("creation");
        let id_of = |key: &str| created["created"].as_array().unwrap().iter().find(|c| c["key"] == key).expect(key)["id"].as_i64().unwrap();
        let (pkg, sub) = (id_of("pkg"), id_of("sub"));
        let mut db = postgres::Client::connect(
            &format!("host={} port={} dbname={} user={} password={}", cfg.db_host, cfg.db_port, cfg.db_name, cfg.db_user, cfg.db_password),
            postgres::NoTls,
        )
        .expect("test database");
        let guid: String = db.query_one("select ea_guid from t_object where object_id = $1", &[&(sub as i32)]).unwrap().get(0);
        db.execute(
            "insert into t_seclocks (userid, groupid, entitytype, entityid, timestamp, locktype) \
             select userid, null, 'Element', $1, now(), 'Full' from t_secuser where userlogin = 'admin'",
            &[&guid],
        )
        .expect("admin account lock");
        let refused = run_batch(
            &cfg,
            &[json!({"op":"create_element","key":"step","parent_element":sub,"name":"Step","type":"Activity","stereotype":"BPMN2.0::Activity"})],
            &BatchOptions::default(),
        );
        db.execute("delete from t_seclocks where entityid = $1", &[&guid]).unwrap();
        run_batch(&cfg, &[json!({"op":"delete","package":pkg})], &BatchOptions::default()).expect("deleting the test package");
        shutdown();
        let message = format!("{refused:?}");
        println!("{message}");
        assert!(message.contains("zablokowany przez konto admin"), "no information about the lock owner");
    }

    /// LC-348: undo restores a changed attribute and deletes a created operation; a save that fails part-way says which
    /// changes stay and journals them. Run with a file copy:
    /// `LOONAR_TEST_PROPS=E:/livetest/undo/undo.properties cargo test ea_undo_attributes -- --ignored --nocapture`.
    #[test]
    #[ignore = "requires EA, a license and the model database from app.properties"]
    fn ea_undo_attributes() {
        let cfg = test_config();
        let _ea = CloseEa;
        let q = |sql: &str| crate::tools::query_rows(&cfg, sql).expect(sql);
        let root: i64 = q("select package_id from t_package where parent_id = 0 order by package_id limit 1")[0][0].parse().unwrap();
        let ops = vec![
            json!({"op":"create_package","key":"pkg","parent":root,"name":format!("ZZ_undo_{}", chrono::Local::now().format("%H%M%S"))}),
            json!({"op":"create_element","key":"c","package":"#pkg","name":"Order","type":"Class"}),
            json!({"op":"create_attribute","key":"a","element":"#c","name":"amount","type":"int"}),
        ];
        let created = run_batch(&cfg, &ops, &BatchOptions { rollback_key: Some("pkg".into()), keep_locks: false }).expect("creation");
        let id_of = |key: &str| created["created"].as_array().unwrap().iter().find(|c| c["key"] == key).expect(key)["id"].as_i64().unwrap();
        let (pkg, class, attribute) = (id_of("pkg"), id_of("c"), id_of("a"));

        // A change of the attribute and a new operation in one save, journalled as the save does it.
        let save = vec![
            json!({"op":"update_attribute","attribute":attribute,"name":"total","type":"decimal"}),
            json!({"op":"create_operation","key":"o","element":class,"name":"pay","return_type":"void"}),
        ];
        let (reverse, not_undoable) = crate::undo::before(&cfg, &save);
        let done = run_batch(&cfg, &save, &BatchOptions::default()).expect("save");
        let journal = std::env::temp_dir().join(format!("loonar-undo-{}", std::process::id()));
        std::fs::create_dir_all(&journal).unwrap();
        let mut jcfg = cfg.clone();
        jcfg.base_dir = journal.clone();
        crate::undo::record(&jcfg, "test", &done, reverse.clone(), not_undoable, Vec::new());
        let entry = crate::undo::latest(&jcfg, 1).pop().expect("a journal entry");
        let back: Vec<Value> = entry["reverse"].as_array().unwrap().clone();
        println!("reverse: {back:?}");
        assert!(back.iter().any(|r| r["op"] == "delete_operation"), "the created operation is deleted on undo");
        assert!(back.iter().any(|r| r["op"] == "update_attribute" && r["name"] == "amount" && r["type"] == "int"));
        assert!(back.iter().all(|r| r.get("_at").is_none()));
        run_batch(&cfg, &back, &BatchOptions::default()).expect("undo");
        assert_eq!(q(&format!("select name, type from t_attribute where id = {attribute}"))[0], vec!["amount".to_string(), "int".to_string()]);
        assert!(q(&format!("select operationid from t_operation where object_id = {class}")).is_empty());

        // A save that fails at its second operation: the first change stays and is named.
        let failing = vec![
            json!({"op":"update_attribute","attribute":attribute,"name":"sum"}),
            json!({"op":"update_element","element":987654321,"name":"x"}),
        ];
        let (reverse, _) = crate::undo::before(&cfg, &failing);
        let err = run_batch(&cfg, &failing, &BatchOptions::default()).expect_err("must fail");
        let batch = err.downcast_ref::<BatchError>().expect("a BatchError");
        println!("{}", batch.message);
        assert_eq!(batch.failed_at, Some(1));
        crate::undo::record_partial(&jcfg, "failing", 1, reverse, Vec::new());
        let partial = crate::undo::latest(&jcfg, 1).pop().unwrap();
        assert_eq!(partial["reverse"][0]["name"], "amount");

        run_batch(&cfg, &[json!({"op":"delete","package":pkg})], &BatchOptions::default()).expect("deleting the test package");
        let _ = std::fs::remove_dir_all(&journal);
    }

    /// LC-371: what EA's baseline comparison returns (an experiment kept as a test): a package copied, a baseline,
    /// a change, the comparison in %TEMP%\loonar_baseline_compare.xml.
    /// `LOONAR_TEST_PROPS=E:/livetest/undo/undo.properties cargo test ea_baseline_compare -- --ignored --nocapture`
    #[test]
    #[ignore = "requires EA, a license and the model database from app.properties"]
    fn ea_baseline_compare() {
        let cfg = test_config();
        let _ea = CloseEa;
        let q = |sql: &str| crate::tools::query_rows(&cfg, sql).expect(sql);
        let root: i64 = q("select package_id from t_package where parent_id = 0 order by package_id")[0][0].parse().unwrap();
        let ops = vec![
            json!({"op":"create_package","key":"pkg","parent":root,"name":format!("ZZ_base_{}", chrono::Local::now().format("%H%M%S"))}),
            json!({"op":"create_element","key":"a","package":"#pkg","name":"Order","type":"Class","notes":"Line one.\r\nLine two."}),
            json!({"op":"create_element","key":"b","package":"#pkg","name":"Invoice","type":"Class"}),
        ];
        let created = run_batch(&cfg, &ops, &BatchOptions { rollback_key: Some("pkg".into()), keep_locks: false }).expect("creation");
        let id_of = |key: &str| created["created"].as_array().unwrap().iter().find(|c| c["key"] == key).expect(key)["id"].as_i64().unwrap();
        let (pkg, a, b) = (id_of("pkg"), id_of("a"), id_of("b"));
        run_batch(&cfg, &[json!({"op":"create_baseline","package":pkg,"version":"1.0","notes":"test"})], &BatchOptions::default()).expect("baseline");
        run_batch(&cfg, &[
            json!({"op":"update_element","element":a,"name":"Customer Order","status":"Approved"}),
            json!({"op":"delete","element":b}),
            json!({"op":"create_element","package":pkg,"name":"Payment","type":"Class"}),
        ], &BatchOptions::default()).expect("changes");
        let file = std::env::temp_dir().join("loonar_baseline_compare.xml");
        let result = run_batch(&cfg, &[json!({"op":"baseline_compare","package":pkg,"baseline":"1.0","file":file.display().to_string()})], &BatchOptions::default());
        run_batch(&cfg, &[json!({"op":"delete","package":pkg})], &BatchOptions::default()).expect("cleanup");
        result.expect("compare");
        let xml = std::fs::read_to_string(&file).unwrap();
        println!("{} bytes\n{}", xml.len(), xml.chars().take(3000).collect::<String>());
        let (added, deleted, changed, noise) = crate::changes::baseline_diff(&xml);
        println!("added {added:?}
deleted {deleted:?}
changed {changed:?}
noise {noise}");
        assert!(added.iter().any(|a| a.contains("Payment")) && deleted.iter().any(|d| d.contains("Invoice")));
        assert!(changed.iter().any(|c| c.contains("Name: \"Order\" → \"Customer Order\"") && c.contains("Status") && !c.contains("DateModified")), "{changed:?}");
    }

    /// LC-376: a layout undone from the journal puts every element back; aligning to the top lines them up.
    /// `LOONAR_TEST_PROPS=E:/livetest/undo/undo.properties cargo test ea_layout_undo_and_align -- --ignored --nocapture`
    #[test]
    #[ignore = "requires EA, a license and the model database from app.properties"]
    fn ea_layout_undo_and_align() {
        let cfg = test_config();
        let _ea = CloseEa;
        let q = |sql: &str| crate::tools::query_rows(&cfg, sql).expect(sql);
        let root: i64 = q("select package_id from t_package where parent_id = 0 order by package_id")[0][0].parse().unwrap();
        let ops = vec![
            json!({"op":"create_package","key":"pkg","parent":root,"name":format!("ZZ_layout_{}", chrono::Local::now().format("%H%M%S"))}),
            json!({"op":"create_diagram","key":"d","package":"#pkg","name":"Layout","type":"Logical"}),
            json!({"op":"create_element","key":"a","package":"#pkg","name":"A","type":"Class"}),
            json!({"op":"create_element","key":"b","package":"#pkg","name":"B","type":"Class"}),
            json!({"op":"create_element","key":"c","package":"#pkg","name":"C","type":"Class"}),
            json!({"op":"create_connector","source":"#a","target":"#b","type":"Association"}),
            json!({"op":"create_connector","source":"#b","target":"#c","type":"Association"}),
            json!({"op":"place","diagram":"#d","element":"#a","left":500,"top":300,"width":110,"height":60}),
            json!({"op":"place","diagram":"#d","element":"#b","left":40,"top":40,"width":110,"height":60}),
            json!({"op":"place","diagram":"#d","element":"#c","left":300,"top":420,"width":110,"height":60}),
        ];
        let created = run_batch(&cfg, &ops, &BatchOptions { rollback_key: Some("pkg".into()), keep_locks: false }).expect("creation");
        let id_of = |key: &str| created["created"].as_array().unwrap().iter().find(|c| c["key"] == key).expect(key)["id"].as_i64().unwrap();
        let (pkg, d, a, b) = (id_of("pkg"), id_of("d"), id_of("a"), id_of("b"));
        let places = || q(&format!("select object_id, rectleft, recttop from t_diagramobjects where diagram_id = {d} order by object_id"));
        let before = places();
        let layout = vec![json!({"op":"layout_diagram","diagram":d,"direction":"down"})];
        let (reverse, not_undoable) = crate::undo::before(&cfg, &layout);
        assert!(not_undoable.is_empty() && reverse.len() == 3, "{reverse:?} {not_undoable:?}");
        let done = run_batch(&cfg, &layout, &BatchOptions::default()).expect("layout");
        println!("warnings: {}", done["warnings"]);
        let moved = places();
        let back: Vec<Value> = reverse.into_iter().map(|mut r| { r.as_object_mut().unwrap().remove("_at"); r }).collect();
        run_batch(&cfg, &back, &BatchOptions::default()).expect("undo the layout");
        let restored = places();
        let align = crate::tools::align_ops(&cfg, &json!({ "diagram": d, "elements": [b, a], "how": "top" })).unwrap();
        run_batch(&cfg, &align, &BatchOptions::default()).expect("align");
        let tops = q(&format!("select recttop from t_diagramobjects where diagram_id = {d} and object_id in ({a}, {b})"));
        run_batch(&cfg, &[json!({"op":"delete","package":pkg})], &BatchOptions::default()).expect("cleanup");
        println!("before {before:?}\nafter layout {moved:?}\nrestored {restored:?}");
        assert_eq!(restored, before, "the layout undone");
        assert_eq!(tops[0], tops[1], "aligned to the top");
    }

    /// LC-416: a relationship changed to an ArchiMate type gets its profile synchronized, without a warning.
    /// `LOONAR_TEST_PROPS=E:/livetest/undo/undo.properties cargo test ea_change_type_synch_profile -- --ignored --nocapture`
    #[test]
    #[ignore = "requires EA, a license and the model database from app.properties"]
    fn ea_change_type_synch_profile() {
        let cfg = test_config();
        let _ea = CloseEa;
        let root: i64 = crate::tools::query_rows(&cfg, "select package_id from t_package where parent_id = 0 order by package_id").unwrap()[0][0].parse().unwrap();
        let created = run_batch(&cfg, &[
            json!({"op":"create_package","key":"pkg","parent":root,"name":format!("ZZ_synch_{}", chrono::Local::now().format("%H%M%S"))}),
            json!({"op":"create_element","key":"a","package":"#pkg","name":"CRM","type":"Component","stereotype":"ArchiMate3::ArchiMate_ApplicationComponent"}),
            json!({"op":"create_element","key":"b","package":"#pkg","name":"Billing","type":"Component","stereotype":"ArchiMate3::ArchiMate_ApplicationComponent"}),
            json!({"op":"create_connector","key":"c","source":"#a","target":"#b","type":"Association","stereotype":"ArchiMate3::ArchiMate_Association"}),
        ], &BatchOptions { rollback_key: Some("pkg".into()), keep_locks: false }).expect("creation");
        let id_of = |key: &str| created["created"].as_array().unwrap().iter().find(|c| c["key"] == key).expect(key)["id"].as_i64().unwrap();
        let (pkg, c) = (id_of("pkg"), id_of("c"));
        let changed = run_batch(&cfg, &[json!({"op":"change_connector_type","connector":c,"type":"Association","stereotype":"ArchiMate3::ArchiMate_Serving"})], &BatchOptions::default()).expect("change");
        println!("{changed}");
        let new_id = changed["created"][0]["id"].as_i64().unwrap();
        let stereo = crate::tools::query_rows(&cfg, &format!("select coalesce(c.stereotype,''), coalesce(x.description,'') from t_connector c left join t_xref x on x.client = c.ea_guid and x.name = 'Stereotypes' where c.connector_id = {new_id}")).unwrap();
        println!("{stereo:?}");
        run_batch(&cfg, &[json!({"op":"delete","package":pkg})], &BatchOptions::default()).expect("cleanup");
        assert!(changed["warnings"].as_array().is_none_or(|w| !w.iter().any(|w| w.as_str().unwrap_or("").contains("synchronized"))), "{changed}");
        assert!(stereo[0][1].contains("ArchiMate_Serving"), "{stereo:?}");
    }

    /// LC-374: a relationship of another type keeps its name, notes, tag, its route on one diagram and its hiding on
    /// another. `LOONAR_TEST_PROPS=E:/livetest/undo/undo.properties cargo test ea_change_connector_type -- --ignored --nocapture`
    #[test]
    #[ignore = "requires EA, a license and the model database from app.properties"]
    fn ea_change_connector_type() {
        let cfg = test_config();
        let _ea = CloseEa;
        let q = |sql: &str| crate::tools::query_rows(&cfg, sql).expect(sql);
        let root: i64 = q("select package_id from t_package where parent_id = 0 order by package_id")[0][0].parse().unwrap();
        let ops = vec![
            json!({"op":"create_package","key":"pkg","parent":root,"name":format!("ZZ_type_{}", chrono::Local::now().format("%H%M%S"))}),
            json!({"op":"create_diagram","key":"d1","package":"#pkg","name":"Routed","type":"Logical"}),
            json!({"op":"create_diagram","key":"d2","package":"#pkg","name":"Hidden","type":"Logical"}),
            json!({"op":"create_element","key":"a","package":"#pkg","name":"A","type":"Class"}),
            json!({"op":"create_element","key":"b","package":"#pkg","name":"B","type":"Class"}),
            json!({"op":"create_connector","key":"c","source":"#b","target":"#a","type":"Association","name":"uses","tags":{"Owner":"Team"}}),
            json!({"op":"update_connector","connector":"#c","notes":"Old notes."}),
            json!({"op":"place","diagram":"#d1","element":"#a","left":60,"top":40,"width":110,"height":60}),
            json!({"op":"place","diagram":"#d1","element":"#b","left":300,"top":200,"width":110,"height":60}),
            json!({"op":"place","diagram":"#d2","element":"#a","left":60,"top":40,"width":110,"height":60}),
            json!({"op":"place","diagram":"#d2","element":"#b","left":300,"top":40,"width":110,"height":60}),
        ];
        let created = run_batch(&cfg, &ops, &BatchOptions { rollback_key: Some("pkg".into()), keep_locks: false }).expect("creation");
        let id_of = |key: &str| created["created"].as_array().unwrap().iter().find(|c| c["key"] == key).expect(key)["id"].as_i64().unwrap();
        let (pkg, d1, d2, c) = (id_of("pkg"), id_of("d1"), id_of("d2"), id_of("c"));
        run_batch(&cfg, &[
            json!({"op":"route_connector","diagram":d1,"connector":c,"start":[300,230],"points":[[115,230]],"end":[115,100]}),
            json!({"op":"hide_connectors","diagram":d2,"connectors":[c]}),
        ], &BatchOptions::default()).expect("route and hide");
        let route_before = q(&format!("select path from t_diagramlinks where diagramid = {d1} and connectorid = {c}"));
        let diagrams: Vec<i64> = q(&format!("select distinct diagramid from t_diagramlinks where connectorid = {c}")).iter().map(|r| r[0].parse().unwrap()).collect();
        let done = run_batch(&cfg, &[json!({"op":"change_connector_type","connector":c,"type":"Dependency","diagrams":diagrams})], &BatchOptions::default()).expect("change type");
        let new = done["created"][0]["id"].as_i64().expect("the new connector");
        let problems = {
            let mut p = Vec::new();
            if !q(&format!("select connector_id from t_connector where connector_id = {c}")).is_empty() { p.push("the old one is still there".to_string()); }
            let row = q(&format!("select connector_type, coalesce(name,''), coalesce(notes,'') from t_connector where connector_id = {new}"));
            if row[0] != vec!["Dependency".to_string(), "uses".to_string(), "Old notes.".to_string()] { p.push(format!("new: {:?}", row[0])); }
            if q(&format!("select value from t_connectortag where elementid = {new} and property = 'Owner'")).first().map(|r| r[0].as_str()) != Some("Team") { p.push("tag".into()); }
            if q(&format!("select path from t_diagramlinks where diagramid = {d1} and connectorid = {new}")) != route_before { p.push("route".into()); }
            let hidden = q(&format!("select hidden from t_diagramlinks where diagramid = {d2} and connectorid = {new}"));
            if !hidden.first().is_some_and(|r| r[0] == "1" || r[0].eq_ignore_ascii_case("true")) { p.push(format!("hidden: {hidden:?}")); }
            p
        };
        run_batch(&cfg, &[json!({"op":"delete","package":pkg})], &BatchOptions::default()).expect("cleanup");
        assert!(problems.is_empty(), "{problems:?}");
    }

    /// Linking a BPMN subprocess to the subprocess diagram (set_composite) on a real EA: on double-click EA
    /// opens the diagram, and in the database the element has ntype 8 and the diagram id in pdata1.
    /// Run with: `cargo test ea_set_composite -- --ignored --nocapture`.
    #[test]
    #[ignore = "requires EA, a license and the model database from app.properties"]
    fn ea_set_composite() {
        let cfg = test_config();
        let _ea = CloseEa;
        let q = |sql: &str| crate::tools::query_rows(&cfg, sql).expect(sql);
        let root: i64 = q("select package_id from t_package where parent_id = 0 order by package_id limit 1")[0][0].parse().unwrap();
        let name = format!("ZZ_test_mostu_{}", chrono::Local::now().format("%H%M%S"));
        let ops = vec![
            json!({"op":"create_package","key":"pkg","parent":root,"name":name}),
            json!({"op":"create_diagram","key":"main","package":"#pkg","name":"Process","type":"BPMN2.0::Business Process"}),
            json!({"op":"create_diagram","key":"sub","package":"#pkg","name":"Subprocess","type":"BPMN2.0::Business Process"}),
            json!({"op":"create_element","key":"a","package":"#pkg","name":"Subprocess A","type":"Activity",
                   "stereotype":"BPMN2.0::Activity","tags":{"activityType":"Sub-Process"}}),
            json!({"op":"place","diagram":"#main","element":"#a","left":40,"top":50,"width":110,"height":60}),
        ];
        let created = run_batch(&cfg, &ops, &BatchOptions { rollback_key: Some("pkg".into()), keep_locks: false }).expect("creation");
        let id_of = |key: &str| created["created"].as_array().unwrap().iter().find(|c| c["key"] == key).expect(key)["id"].as_i64().unwrap();
        let (pkg, sub, a) = (id_of("pkg"), id_of("sub"), id_of("a"));
        let before = q(&format!("select ntype, coalesce(pdata1, '') from t_object where object_id = {a}"));
        run_batch(&cfg, &[json!({"op":"set_composite","element":a,"diagram":sub})], &BatchOptions::default()).expect("link");
        let after = q(&format!("select ntype, coalesce(pdata1, '') from t_object where object_id = {a}"));

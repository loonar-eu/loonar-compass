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
fn connection_string(cfg: &AppConfig) -> String {
    if cfg.uses_file() {
        return cfg.model_file.display().to_string();
    }
    format!(
        "{db} --- DBType=4;Connect=Provider=SSDB;SRC={host},{port}@{db};UID={user};PWD={pwd};",
        db = cfg.db_name,
        // EA does not connect to the database under the name "localhost" (it then opens an invisible window and waits forever;
        // verified 2026-10-13), but it works with the address 127.0.0.1, so the name is replaced with the address.
        host = if cfg.db_host.trim().eq_ignore_ascii_case("localhost") { "127.0.0.1" } else { cfg.db_host.trim() },
        port = cfg.db_port,
        user = cfg.db_user,
        pwd = cfg.db_password,
    )
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
        Self { requests, login, started_wall: chrono::Local::now(), thread }
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
        connection: connection_string(cfg),
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
        if crate::ea_ui::has_model_open(&target) {
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
    let result = if sent { answer.recv_timeout(timeout()) } else { Err(mpsc::RecvTimeoutError::Disconnected) };
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
        if result["rolled_back"] == true {
            msg.push(' ');
            msg.push_str(crate::i18n::tr("The changes were rolled back; nothing was left in the model."));
        } else if left > 0 {
            msg.push(' ');
            msg.push_str(&crate::i18n::fmt("{0} objects created before the error remain in the model.", &[left.to_string()]));
        }
        bail!(msg);
    }
    Ok(result)
}

/// Closes the EA session when the application closes. Without this, the EA process started through COM stays in the background
/// and holds a license (the EA thread dies together with the application process before it closes EA). Waits up to 8 s for
/// EA to close, then kills the EA processes started by this session.
pub fn shutdown() {
    let worker = WORKER.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).take();
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
        println!("before: {:?} | after: {:?} | subprocess diagram: {sub}", before[0], after[0]);
        run_batch(&cfg, &[json!({"op":"delete","package":pkg})], &BatchOptions::default()).expect("deleting the test package");
        shutdown();
        assert_eq!(after[0], vec!["8".to_string(), sub.to_string()]);
    }

    /// Verification after a write: a real write passes, a change that did not happen (the false "done" seen in the
    /// head-to-head test) is caught, and deleting an element with relationships lists them for the confirmation.
    /// Run with: `cargo test ea_verify -- --ignored --nocapture` (LOONAR_TEST_PROPS = another model).
    #[test]
    #[ignore = "requires EA, a license and the model database from app.properties"]
    fn ea_verify() {
        let cfg = test_config();
        let _ea = CloseEa;
        let q = |sql: &str| crate::tools::query_rows(&cfg, sql).expect(sql);
        let root: i64 = q("select package_id from t_package where parent_id = 0 order by package_id limit 1")[0][0].parse().unwrap();
        let name = format!("ZZ_test_mostu_verify_{}", chrono::Local::now().format("%H%M%S"));
        let ops = vec![
            json!({"op":"create_package","key":"pkg","parent":root,"name":name}),
            json!({"op":"create_element","key":"a","package":"#pkg","name":"Customer","type":"Class","stereotype":"entity","tags":{"Owner":"Team A"}}),
            json!({"op":"create_element","key":"b","package":"#pkg","name":"Order","type":"Class"}),
            json!({"op":"create_connector","key":"c","source":"#b","target":"#a","type":"Aggregation"}),
        ];
        let created = run_batch(&cfg, &ops, &BatchOptions { rollback_key: Some("pkg".into()), keep_locks: false }).expect("creation");
        let real = crate::verify::verify(&cfg, &ops, &created);
        let id_of = |key: &str| created["created"].as_array().unwrap().iter().find(|c| c["key"] == key).expect(key)["id"].as_i64().unwrap();
        let (pkg, a, b) = (id_of("pkg"), id_of("a"), id_of("b"));
        let update = vec![json!({"op":"update_element","element":a,"clear_stereotype":true,"tags":{"Owner":null}})];
        let done = run_batch(&cfg, &update, &BatchOptions::default()).expect("update");
        let updated = crate::verify::verify(&cfg, &update, &done);
        // Claims nothing was written: these changes are not in the model.
        let claimed = vec![
            json!({"op":"update_element","element":b,"stereotype":"entity","name":"Purchase","tags":{"Owner":"Team B"}}),
            json!({"op":"delete","element":a}),
        ];
        let fake = crate::verify::verify(&cfg, &claimed, &json!({"created": []}));
        let related = crate::verify::relations_of_deleted(&cfg, &[json!({"op":"delete","element":a})]);
        run_batch(&cfg, &[json!({"op":"delete","package":pkg})], &BatchOptions::default()).expect("cleanup");
        shutdown();
        println!("real: {} ok {:?}\nupdated: {} ok {:?}\nfake: {} ok {:?}\nrelated: {related:?}", real.passed, real.problems, updated.passed, updated.problems, fake.passed, fake.problems);
        assert!(real.problems.is_empty() && real.passed >= 6, "a real write verifies");
        assert!(updated.problems.is_empty() && updated.passed >= 2, "clearing a stereotype and a tag verifies");
        assert_eq!(fake.problems.len(), 4, "name, stereotype, tag and a delete that did not happen are all caught");
        assert!(related.len() == 1 && related[0].contains("Aggregation"), "relationships of a deleted element are listed");
    }

    /// New EA operations: automatic layout (overlapping elements are spread out), swimlanes, a linked document, XMI
    /// export and import. Run with: `cargo test ea_layout_xmi_lanes -- --ignored --nocapture` (LOONAR_TEST_PROPS = another model).
    #[test]
    #[ignore = "requires EA, a license and the model database from app.properties"]
    fn ea_layout_xmi_lanes() {
        let cfg = test_config();
        let _ea = CloseEa;
        let q = |sql: &str| crate::tools::query_rows(&cfg, sql).expect(sql);
        let root: i64 = q("select package_id from t_package where parent_id = 0 order by package_id limit 1")[0][0].parse().unwrap();
        let name = format!("ZZ_test_mostu_ops_{}", chrono::Local::now().format("%H%M%S"));
        let mut ops = vec![
            json!({"op":"create_package","key":"pkg","parent":root,"name":name}),
            json!({"op":"create_package","key":"copy","parent":root,"name":format!("{name}_import")}),
            json!({"op":"create_diagram","key":"dia","package":"#pkg","name":"Flow","type":"Activity"}),
        ];
        for (i, step) in ["Receive", "Check", "Approve", "Pack", "Ship", "Close"].iter().enumerate() {
            ops.push(json!({"op":"create_element","key":format!("e{i}"),"package":"#pkg","name":step,"type":"Activity"}));
            ops.push(json!({"op":"place","diagram":"#dia","element":format!("#e{i}"),"left":100,"top":100,"width":110,"height":60}));
            if i > 0 {
                ops.push(json!({"op":"create_connector","source":format!("#e{}", i - 1),"target":format!("#e{i}"),"type":"ControlFlow"}));
            }
        }
        let created = run_batch(&cfg, &ops, &BatchOptions { rollback_key: Some("pkg".into()), keep_locks: false }).expect("creation");
        let id_of = |key: &str| created["created"].as_array().unwrap().iter().find(|c| c["key"] == key).expect(key)["id"].as_i64().unwrap();
        let (pkg, copy, dia, first) = (id_of("pkg"), id_of("copy"), id_of("dia"), id_of("e0"));
        let xmi = std::env::temp_dir().join(format!("loonar-test-{}.xml", std::process::id()));
        let more = vec![
            json!({"op":"layout_diagram","diagram":dia,"direction":"right"}),
            json!({"op":"set_swimlanes","diagram":dia,"lanes":[{"title":"Sales","width":300},{"title":"Warehouse","width":300}]}),
            json!({"op":"set_linked_document","element":first,"text":"Przyjęcie zamówienia\nKrok 1: weryfikacja"}),
            json!({"op":"export_xmi","package":pkg,"file":xmi.to_string_lossy()}),
        ];
        let done = run_batch(&cfg, &more, &BatchOptions::default());
        let positions = q(&format!("select rectleft, recttop, rectright, rectbottom from t_diagramobjects where diagram_id = {dia}"));
        let lanes = q(&format!("select coalesce(swimlanes,'') from t_diagram where diagram_id = {dia}"));
        let guid = q(&format!("select ea_guid from t_object where object_id = {first}"))[0][0].clone();
        let docs = q(&format!("select count(*) from t_document where elementid = '{guid}'"));
        let xmi_text = std::fs::read_to_string(&xmi).unwrap_or_default();
        let imported = if xmi_text.is_empty() { Err(anyhow::anyhow!("no XMI")) } else {
            run_batch(&cfg, &[json!({"op":"import_xmi","package":copy,"file":xmi.to_string_lossy()})], &BatchOptions::default())
        };
        let copied = q(&format!("with recursive t(id) as (select {copy} union all select p.package_id from t_package p join t on p.parent_id = t.id) select count(*) from t_object where package_id in (select id from t) and object_type = 'Activity'"));
        run_batch(&cfg, &[json!({"op":"delete","package":pkg}), json!({"op":"delete","package":copy})], &BatchOptions::default()).expect("cleanup");
        shutdown();
        let _ = std::fs::remove_file(&xmi);
        println!("ops: {done:?}\npositions: {positions:?}\nlanes: {:?}\nlinked docs: {docs:?}\nxmi bytes {} has Approve {}\nimport: {imported:?}\nactivities in the imported package: {copied:?}",
            lanes[0][0].chars().take(200).collect::<String>(), xmi_text.len(), xmi_text.contains("Approve"));
        done.expect("layout, lanes, linked document, export");
        let boxes: Vec<(i64, i64, i64, i64)> = positions.iter().map(|r| (r[0].parse().unwrap(), r[1].parse::<i64>().unwrap().abs(), r[2].parse().unwrap(), r[3].parse::<i64>().unwrap().abs())).collect();
        let overlaps = boxes.iter().enumerate().any(|(i, a)| boxes.iter().skip(i + 1).any(|b| a.0 < b.2 && b.0 < a.2 && a.1.min(a.3) < b.1.max(b.3) && b.1.min(b.3) < a.1.max(a.3)));
        assert!(!overlaps, "after the layout no two elements overlap: {boxes:?}");
        assert!(lanes[0][0].contains("SW1=300") && lanes[0][0].contains("SW2=300"), "two swimlanes saved (their titles are kept by EA outside this column)");
        assert_eq!(docs[0][0], "1", "linked document saved");
        assert!(xmi_text.contains("Approve"), "XMI written");
        imported.expect("import");
        assert_eq!(copied[0][0], "6", "the imported package has the six activities");
    }

    /// EA scripts: saving (a new group, a library and a script that includes it), listing, reading, replacing and,
    /// with an EA window open on the same model (LOONAR_TEST_RUN_SCRIPT=1), running with Repository and Session.Output.
    /// Run with: `cargo test ea_scripts -- --ignored --nocapture` (LOONAR_TEST_PROPS = another model).
    #[test]
    #[ignore = "requires EA, a license and the model database from app.properties"]
    fn ea_scripts() {
        let cfg = test_config();
        let _ea = CloseEa;
        let group = format!("ZZ_test_{}", chrono::Local::now().format("%H%M%S"));
        let library = "function countOf(type) { return Repository.GetElementSet(\"select object_id from t_object where object_type = '\" + type + \"'\", 2).Count; }";
        let main = format!("!INC {group}.Lib\nSession.Output(\"classes: \" + countOf(\"Class\"));\nSession.Output(\"model: \" + Repository.Models.GetAt(0).Name);");
        for (name, code) in [("Lib", library.to_string()), ("Count", main.clone())] {
            let (statements, _) = crate::scripts::save_statements(&cfg, &group, name, "JScript", &code).unwrap();
            run_batch(&cfg, &[json!({"op":"execute_sql","statements":statements})], &BatchOptions::default()).expect("save");
        }
        let listed = crate::scripts::list(&cfg).unwrap();
        let read = crate::scripts::find(&cfg, &format!("{group}.Count")).unwrap();
        let resolved = crate::scripts::resolve_includes(&cfg, &read).unwrap();
        // Replacing keeps one script.
        let (statements, updated) = crate::scripts::save_statements(&cfg, &group, "Count", "JScript", &format!("{main}\nSession.Output(\"v2\");")).unwrap();
        run_batch(&cfg, &[json!({"op":"execute_sql","statements":statements})], &BatchOptions::default()).expect("replace");
        let count_rows = crate::tools::query_rows(&cfg, &format!("select count(*) from t_script where notes like '%Name=_Count_%' and scriptauthor = (select scriptname from t_script where script = '{group}')")).unwrap();
        let ran = if std::env::var("LOONAR_TEST_RUN_SCRIPT").is_ok() { { let s = crate::scripts::find(&cfg, &format!("{group}.Count")).unwrap(); let code = crate::scripts::resolve_includes(&cfg, &s).unwrap(); Some(crate::scripts::run(&cfg, &format!("{group}.Count"), &code)) } } else { None };
        let classes = crate::tools::query_rows(&cfg, "select count(*) from t_object where object_type = 'Class'").unwrap()[0][0].clone();
        // Cleanup: the scripts and the group.
        run_batch(&cfg, &[json!({"op":"execute_sql","statements":[
            format!("delete from t_script where scriptauthor = (select scriptname from t_script where script = '{group}' and scriptcategory = '{}')", crate::scripts::GROUP_CATEGORY),
            format!("delete from t_script where script = '{group}' and scriptcategory = '{}'", crate::scripts::GROUP_CATEGORY),
        ]})], &BatchOptions::default()).expect("cleanup");
        shutdown();
        println!("{listed}\nresolved:\n{resolved}\nreplace: updated={updated} rows={count_rows:?}\nrun: {ran:?} (classes in the model: {classes})");
        assert!(listed.contains(&format!("{group} | Count | JScript")) && listed.contains(&format!("{group} | Lib | JScript")));
        assert_eq!(read.code.replace("\r\n", "\n"), main);
        assert!(resolved.contains("function countOf") && !resolved.contains("!INC"), "the include is resolved");
        assert!(updated && count_rows[0][0] == "1", "replacing keeps one script");
        if let Some(ran) = ran {
            let out = ran.expect("run");
            assert!(out.contains(&format!("classes: {classes}")) && out.contains("v2"), "{out}");
        }
    }

    /// Reversing a relationship, editing a package and a report from an EA template.
    /// Run with: `cargo test ea_reverse_package_report -- --ignored --nocapture` (LOONAR_TEST_PROPS = another model).
    #[test]
    #[ignore = "requires EA, a license and the model database from app.properties"]
    fn ea_reverse_package_report() {
        let cfg = test_config();
        let _ea = CloseEa;
        let q = |sql: &str| crate::tools::query_rows(&cfg, sql).expect(sql);
        let root: i64 = q("select package_id from t_package where parent_id = 0 order by package_id limit 1")[0][0].parse().unwrap();
        let name = format!("ZZ_test_mostu_rep_{}", chrono::Local::now().format("%H%M%S"));
        let created = run_batch(&cfg, &[
            json!({"op":"create_package","key":"pkg","parent":root,"name":name}),
            json!({"op":"create_element","key":"a","package":"#pkg","name":"Order","type":"Class","notes":"An order."}),
            json!({"op":"create_element","key":"b","package":"#pkg","name":"Invoice","type":"Class","notes":"An invoice."}),
            json!({"op":"create_connector","key":"c","source":"#a","target":"#b","type":"Dependency"}),
        ], &BatchOptions { rollback_key: Some("pkg".into()), keep_locks: false }).expect("creation");
        let id_of = |key: &str| created["created"].as_array().unwrap().iter().find(|c| c["key"] == key).expect(key)["id"].as_i64().unwrap();
        let (pkg, a, c) = (id_of("pkg"), id_of("a"), id_of("c"));
        let report = std::env::temp_dir().join(format!("loonar-report-{}.docx", std::process::id()));
        let done = run_batch(&cfg, &[
            json!({"op":"reverse_connector","connector":c}),
            json!({"op":"update_package","package":pkg,"notes":"Billing area","version":"2.0","status":"Approved"}),
            json!({"op":"generate_report","package":pkg,"file":report.to_string_lossy()}),
        ], &BatchOptions::default());
        let ends = q(&format!("select start_object_id, end_object_id from t_connector where connector_id = {c}"));
        let package = q(&format!("select coalesce(notes,''), coalesce(version,'') from t_package where package_id = {pkg}"));
        let status = q(&format!("select coalesce(status,'') from t_object where pdata1 = '{pkg}' and object_type = 'Package'"));
        let size = std::fs::metadata(&report).map(|m| m.len()).unwrap_or(0);
        let _ = std::fs::remove_file(&report);
        run_batch(&cfg, &[json!({"op":"delete","package":pkg})], &BatchOptions::default()).expect("cleanup");
        shutdown();
        println!("ops: {done:?}\nends: {ends:?} (Order = {a})\npackage: {package:?} status {status:?}\nreport bytes: {size}");
        done.expect("ops");
        assert_eq!(ends[0][1], a.to_string(), "the relationship now ends at Order");
        assert_eq!(package[0], vec!["Billing area".to_string(), "2.0".to_string()]);
        assert_eq!(status[0][0], "Approved");
        assert!(size > 5000, "a DOCX report was written");
    }

    /// A write that fails halfway leaves nothing behind (all or nothing), and an XMI export to a path with "/" works.
    /// Run with: `cargo test ea_atomic_write -- --ignored --nocapture` (LOONAR_TEST_PROPS = another model).
    #[test]
    #[ignore = "requires EA, a license and the model database from app.properties"]
    fn ea_atomic_write() {
        let cfg = test_config();
        let _ea = CloseEa;
        let q = |sql: &str| crate::tools::query_rows(&cfg, sql).expect(sql);
        let package: i64 = q("select package_id from t_package where parent_id <> 0 order by package_id limit 1")[0][0].parse().unwrap();
        let name = format!("ZZ_atomic_{}", chrono::Local::now().format("%H%M%S"));
        let failed = run_batch(&cfg, &[
            json!({"op":"create_element","key":"a","package":package,"name":name,"type":"Activity"}),
            json!({"op":"create_element","key":"b","package":package,"name":format!("{name}_2"),"type":"Activity"}),
            json!({"op":"create_connector","source":"#a","target":"#b","type":"ControlFlow"}),
            json!({"op":"place","diagram":999999,"element":"#a","left":10,"top":10,"width":100,"height":50}),
        ], &BatchOptions::default());
        let left = q(&format!("select count(*) from t_object where name like '{name}%'"));
        let file = std::env::temp_dir().join(format!("loonar-xmi-{}/sub/model.xml", std::process::id())).to_string_lossy().replace('\\', "/");
        let exported = run_batch(&cfg, &[json!({"op":"export_xmi","package":package,"file":file})], &BatchOptions::default());
        let size = std::fs::metadata(file.replace('/', "\\")).map(|m| m.len()).unwrap_or(0);
        shutdown();
        println!("failed write: {:?}\nleft in the model: {left:?}\nexport: {exported:?} bytes {size}", failed.as_ref().map(|_| "ok").map_err(|e| e.to_string()));
        let message = failed.expect_err("the write must fail").to_string();
        assert!(message.contains("rolled back") || message.contains("wycofane") || message.contains("nothing was left") || message.contains("nic nie"), "{message}");
        assert_eq!(left[0][0], "0", "nothing of the failed write is left");
        exported.expect("export");
        assert!(size > 1000, "XMI written to a path given with /");
    }

    /// Governance on a real model: seeded defects are found by the linter, the organization's rules work, the gap analysis
    /// sees new / retired / changed elements and the write preview shows before → after. Run on a database and on a file copy:
    /// `cargo test ea_governance -- --ignored --nocapture` (LOONAR_TEST_PROPS = another model).
    #[test]
    #[ignore = "requires EA, a license and the model database from app.properties"]
    fn ea_governance() {
        let cfg = test_config();
        let _ea = CloseEa;
        let q = |sql: &str| crate::tools::query_rows(&cfg, sql).expect(sql);
        let root: i64 = q("select package_id from t_package where parent_id = 0 order by package_id limit 1")[0][0].parse().unwrap();
        let name = format!("ZZ_test_mostu_gov_{}", chrono::Local::now().format("%H%M%S"));
        let ops = vec![
            json!({"op":"create_package","key":"pkg","parent":root,"name":name}),
            json!({"op":"create_package","key":"asis","parent":"#pkg","name":"As-is"}),
            json!({"op":"create_package","key":"tobe","parent":"#pkg","name":"To-be"}),
            json!({"op":"create_package","key":"empty","parent":"#asis","name":"Empty"}),
            json!({"op":"create_diagram","key":"dia","package":"#asis","name":"Components","type":"Component"}),
            json!({"op":"create_diagram","key":"blank","package":"#asis","name":"Blank","type":"Component"}),
            json!({"op":"create_element","key":"c1","package":"#asis","name":"Class1","type":"Class"}),
            json!({"op":"create_element","key":"orders","package":"#asis","name":"OrderService","type":"Component","notes":"Takes orders."}),
            json!({"op":"create_element","key":"billing","package":"#asis","name":"Billing","type":"Component","notes":"Invoices."}),
            json!({"op":"create_element","key":"legacy","package":"#asis","name":"LegacyFax","type":"Component","notes":"Old fax gateway."}),
            json!({"op":"create_element","key":"api","package":"#asis","name":"OrderApi","type":"Interface","notes":"REST."}),
            json!({"op":"place","diagram":"#dia","element":"#orders","left":40,"top":40,"width":140,"height":70}),
            json!({"op":"place","diagram":"#dia","element":"#billing","left":260,"top":40,"width":140,"height":70}),
            json!({"op":"place","diagram":"#dia","element":"#api","left":40,"top":200,"width":140,"height":70}),
            json!({"op":"create_connector","source":"#orders","target":"#billing","type":"Dependency"}),
            json!({"op":"create_connector","source":"#orders","target":"#billing","type":"Dependency"}),
            json!({"op":"create_connector","source":"#billing","target":"#api","type":"Realisation"}),
            json!({"op":"create_connector","source":"#legacy","target":"#legacy","type":"Dependency"}),
            json!({"op":"create_element","key":"orders2","package":"#tobe","name":"OrderService","type":"Component","notes":"Takes orders online."}),
            json!({"op":"create_element","key":"billing2","package":"#tobe","name":"Billing","type":"Component","notes":"Invoices."}),
            json!({"op":"create_element","key":"pay","package":"#tobe","name":"PaymentGateway","type":"Component","notes":"Cards."}),
            json!({"op":"create_connector","source":"#orders2","target":"#pay","type":"Dependency"}),
        ];
        let created = run_batch(&cfg, &ops, &BatchOptions { rollback_key: Some("pkg".into()), keep_locks: false }).expect("creation");
        let id_of = |key: &str| created["created"].as_array().unwrap().iter().find(|c| c["key"] == key).expect(key)["id"].as_i64().unwrap();
        let (pkg, asis, tobe, orders) = (id_of("pkg"), id_of("asis"), id_of("tobe"), id_of("orders"));

        let started = Instant::now();
        let lint = crate::governance::lint(&json!({"package": asis}), &cfg);
        let rules = json!({
            "naming": [{"applies_to": "Component", "pattern": "*Service", "message": "a component name ends with Service"}],
            "required_tags": [{"applies_to": "Component", "tag": "Owner"}],
            "allowed_relations": [{"connector_type": "Realisation", "source": "Component", "target": "Class"}]
        });
        let ruled = crate::governance::lint(&json!({"package": asis, "rules": rules.clone(), "skip": ["missing_notes"]}), &cfg);
        let gap = crate::governance::gap(&json!({"as_is": asis, "to_be": tobe}), &cfg);
        let preview = crate::governance::preview(&cfg, &[
            json!({"op":"update_element","element":orders,"name":"OrderingService","notes":"New."}),
            json!({"op":"create_element","name":"Audit","type":"Component","package":asis}),
            json!({"op":"delete","element":orders}),
        ]);
        let elapsed = started.elapsed().as_millis();
        let live_c1 = crate::governance::check_object_with(&cfg, "element", id_of("c1"), &rules).expect("live element");
        let live_billing = crate::governance::check_object_with(&cfg, "element", id_of("billing"), &rules).expect("live element");
        let realisation = q(&format!("select connector_id from t_connector where start_object_id = {} and connector_type = 'Realisation'", id_of("billing")))[0][0].parse::<i64>().unwrap();
        let selfrel = q(&format!("select connector_id from t_connector where start_object_id = {0} and end_object_id = {0}", id_of("legacy")))[0][0].parse::<i64>().unwrap();
        let dup = q(&format!("select connector_id from t_connector where start_object_id = {} and end_object_id = {}", orders, id_of("billing")))[0][0].parse::<i64>().unwrap();
        let live_real = crate::governance::check_object_with(&cfg, "connector", realisation, &rules).expect("live connector");
        let live_self = crate::governance::check_object_with(&cfg, "connector", selfrel, &rules).expect("live connector");
        let live_dup = crate::governance::check_object_with(&cfg, "connector", dup, &rules).expect("live connector");
        let live_gone = crate::governance::check_object_with(&cfg, "element", 999_999_999, &rules).expect("live missing");
        run_batch(&cfg, &[json!({"op":"delete","package":pkg})], &BatchOptions::default()).expect("deleting the test package");
        shutdown();

        let lint = lint.expect("lint");
        let ruled = ruled.expect("lint with rules");
        let gap = gap.expect("gap");
        println!("--- lint ({elapsed} ms for all checks) ---\n{lint}\n--- with rules ---\n{ruled}\n--- gap ---\n{gap}\n--- preview ---\n{preview}");
        let has = |text: &str, rule: &str, what: &str| text.lines().any(|l| l.contains(&format!("| {rule} |")) && l.contains(what));
        assert!(has(&lint, "default_name", "Class1"), "default name");
        assert!(has(&lint, "orphan", "Class1"), "orphan");
        assert!(!has(&lint, "orphan", "OrderService"), "an element on a diagram is not an orphan");
        assert!(has(&lint, "missing_notes", "Class1"), "missing notes");
        assert!(has(&lint, "duplicate_relation", "OrderService → Billing"), "duplicate relationship");
        assert!(has(&lint, "self_relation", "LegacyFax → LegacyFax"), "self relationship");
        assert!(has(&lint, "empty_package", "Empty"), "empty package");
        assert!(has(&lint, "empty_diagram", "Blank"), "empty diagram");
        assert!(!has(&lint, "empty_diagram", "Components"), "a diagram with elements is not empty");
        assert!(has(&ruled, "naming", "Billing") && !has(&ruled, "naming", "OrderService"), "naming rule");
        assert!(has(&ruled, "required_tag", "Billing"), "required tag");
        assert!(has(&ruled, "invalid_relation", "Billing → OrderApi"), "allowed relationships");
        assert!(!ruled.contains("| missing_notes |"), "skipped rule");
        assert!(gap.lines().any(|l| l.contains("PaymentGateway")), "new in to-be");
        assert!(gap.lines().any(|l| l.contains("LegacyFax")), "retired from as-is");
        assert!(gap.lines().any(|l| l.contains("OrderService") && l.contains("Billing")), "changed relationships");
        assert!(preview.contains("\"OrderService\"") && preview.contains("\"OrderingService\""), "before → after of a name");
        assert!(preview.lines().any(|l| l.starts_with('+') && l.contains("Audit")), "a new element");
        assert!(preview.lines().any(|l| l.starts_with('−')), "a deletion");
        println!("--- live ---
{live_c1:?}
{live_billing:?}
{live_real:?}
{live_self:?}
{live_dup:?}");
        assert!(live_c1.iter().any(|f| f.contains("Class1")), "live: default name");
        assert!(live_billing.iter().any(|f| f.contains("Owner")) && live_billing.iter().any(|f| f.contains("Service")), "live: tag and naming rules");
        assert!(live_real.iter().any(|f| f.contains("Billing → OrderApi")), "live: disallowed relationship");
        assert!(live_self.iter().any(|f| f.contains("LegacyFax → LegacyFax")), "live: self relationship");
        assert!(live_dup.iter().any(|f| f.contains("2")), "live: duplicate");
        assert!(live_gone.is_empty(), "a removed object: nothing");
    }

    /// Changing the position and size of an element on a diagram (set_bounds) on a real EA; the connector must stay.
    /// Run with: `cargo test ea_set_bounds -- --ignored --nocapture`.
    #[test]
    #[ignore = "requires EA, a license and the model database from app.properties"]
    fn ea_set_bounds() {
        let cfg = test_config();
        let _ea = CloseEa;
        let q = |sql: &str| crate::tools::query_rows(&cfg, sql).expect(sql);
        let root: i64 = q("select package_id from t_package where parent_id = 0 order by package_id limit 1")[0][0].parse().unwrap();
        let name = format!("ZZ_test_mostu_{}", chrono::Local::now().format("%H%M%S"));
        let ops = vec![
            json!({"op":"create_package","key":"pkg","parent":root,"name":name}),
            json!({"op":"create_diagram","key":"dia","package":"#pkg","name":"Frames","type":"Activity"}),
            json!({"op":"create_element","key":"a","package":"#pkg","name":"A","type":"Activity"}),
            json!({"op":"create_element","key":"b","package":"#pkg","name":"B","type":"Activity"}),
            json!({"op":"create_connector","source":"#a","target":"#b","type":"ControlFlow"}),
            json!({"op":"place","diagram":"#dia","element":"#a","left":40,"top":50,"width":110,"height":60}),
            json!({"op":"place","diagram":"#dia","element":"#b","left":300,"top":50,"width":110,"height":60}),
        ];
        let created = run_batch(&cfg, &ops, &BatchOptions { rollback_key: Some("pkg".into()), keep_locks: false }).expect("creation");
        let id_of = |key: &str| created["created"].as_array().unwrap().iter().find(|c| c["key"] == key).expect(key)["id"].as_i64().unwrap();
        let (pkg, dia, a) = (id_of("pkg"), id_of("dia"), id_of("a"));
        let before = q(&format!("select rectleft, recttop, rectright, rectbottom from t_diagramobjects where diagram_id = {dia} and object_id = {a}"));
        run_batch(&cfg, &[json!({"op":"set_bounds","diagram":dia,"element":a,"top":80,"width":250,"height":140})], &BatchOptions::default())
            .expect("resizing");
        let after = q(&format!("select rectleft, recttop, rectright, rectbottom from t_diagramobjects where diagram_id = {dia} and object_id = {a}"));
        let links = q(&format!("select count(*) from t_diagramlinks l join t_connector c on c.connector_id = l.connectorid where l.diagramid = {dia}"));
        let connectors = q(&format!("select count(*) from t_connector where start_object_id = {a}"));
        println!("before: {:?} | after: {:?} | connectors: {} (on the diagram: {})", before[0], after[0], connectors[0][0], links[0][0]);
        run_batch(&cfg, &[json!({"op":"delete","package":pkg})], &BatchOptions::default()).expect("deleting the test package");
        shutdown();
        assert_eq!(before[0], vec!["40", "-50", "150", "-110"]);
        assert_eq!(after[0], vec!["40", "-80", "290", "-220"], "left unchanged, top 80, width 250, height 140");
        assert_eq!(connectors[0][0], "1");
    }
    /// Cleanup after interrupted tests: deletes the `ZZ_test_mostu_*` packages from the model.
    /// Run with: `cargo test ea_cleanup -- --ignored --nocapture`.
    #[test]
    #[ignore = "requires EA, a license and the model database from app.properties"]
    fn ea_cleanup() {
        let cfg = test_config();
        let _ea = CloseEa;
        let q = |sql: &str| crate::tools::query_rows(&cfg, sql).expect(sql);
        for row in q("select package_id, name from t_package where name like 'ZZ_test_mostu_%' or name like 'zz-ui-%' order by package_id") {
            let id: i64 = row[0].parse().unwrap();
            println!("deleting package {} ({})", row[1], id);
            run_batch(&cfg, &[json!({"op":"delete_package","package":id})], &BatchOptions::default()).expect("deleting the package");
        }
        println!("test packages in the model: {}", q("select count(*) from t_package where name like 'ZZ_test_mostu_%'")[0][0]);
        println!("account locks in the model: {}", q("select count(*) from t_seclocks")[0][0]);
        println!("objects without a package: {}", q("select count(*) from t_object where package_id = 0")[0][0]);
    }

    /// Navigation cell icon sheet: draws cells with the icons `list-from` … `list-to` (10 per row)
    /// and saves the diagram as PNG, so you can read which number is which icon. The working package is deleted.
    /// Run with: variables LOONAR_ICONS_OUT (PNG file), LOONAR_ICONS_LIST, LOONAR_ICONS_FROM, LOONAR_ICONS_TO,
    /// then `cargo test nav_icons_sheet -- --ignored --nocapture`.
    #[test]
    #[ignore = "requires EA, a license and the model database from app.properties"]
    fn nav_icons_sheet() {
        let cfg = test_config();
        let _ea = CloseEa;
        let env = |name: &str, default: &str| std::env::var(name).unwrap_or_else(|_| default.to_string());
        let out = env("LOONAR_ICONS_OUT", "nav_icons.png");
        let list = env("LOONAR_ICONS_LIST", "2");
        let (from, to): (u32, u32) = (env("LOONAR_ICONS_FROM", "1").parse().unwrap(), env("LOONAR_ICONS_TO", "90").parse().unwrap());
        let q = |sql: &str| crate::tools::query_rows(&cfg, sql).expect(sql);
        let root: i64 = q("select package_id from t_package where parent_id = 0 order by package_id limit 1")[0][0].parse().unwrap();
        let target: i64 = q("select diagram_id from t_diagram order by diagram_id limit 1")[0][0].parse().unwrap();

        let mut ops = vec![
            json!({"op":"create_package","key":"pkg","parent":root,"name":format!("ZZ_test_mostu_icons_{}", chrono::Local::now().format("%H%M%S"))}),
            json!({"op":"create_diagram","key":"dia","package":"#pkg","name":"icons","type":"Package"}),
        ];
        for (n, i) in (from..=to).enumerate() {
            let (col, row) = (n % 10, n / 10);
            ops.push(json!({"op":"navigation_cell","diagram":"#dia","target":target,"image":format!("{list}-{i}"),"left":20 + col * 190,"top":20 + row * 135}));
        }
        ops.push(json!({"op":"export_diagram","diagram":"#dia","path":out}));
        ops.push(json!({"op":"delete_package","package":"#pkg"}));
        let options = BatchOptions { rollback_key: Some("pkg".into()), keep_locks: false };
        run_batch(&cfg, &ops, &options).expect("icon sheet");
        println!("icon sheet {list}-{from}..{list}-{to}: {out}");
    }

    /// Logging in to EA with the database server address given in the LOONAR_TEST_DB_HOST variable (e.g. `localhost`),
    /// to check how EA copes with different ways of writing the address. `cargo test ea_ping_host -- --ignored --nocapture`.
    #[test]
    #[ignore = "requires EA, a license and the model database from app.properties"]
    fn ea_ping_host() {
        let mut cfg = test_config();
        if let Ok(host) = std::env::var("LOONAR_TEST_DB_HOST") {
            cfg.db_host = host;
        }
        let started = Instant::now();
        let result = run_batch(&cfg, &[json!({"op":"ping"})], &BatchOptions::default());
        println!("server {}: {:?} after {} s", cfg.db_host, result.as_ref().map(|r| r["user"].clone()).map_err(|e| e.to_string()), started.elapsed().as_secs());
    }

    /// Fill colors on a real EA: the element's default color, the color when placing it on a diagram, and changing
    /// the color of an element already on a diagram. Run with: `cargo test ea_colors -- --ignored --nocapture`.
    #[test]
    #[ignore = "requires EA, a license and the model database from app.properties"]
    fn ea_colors() {
        let cfg = test_config();
        let _ea = CloseEa;
        let q = |sql: &str| crate::tools::query_rows(&cfg, sql).expect(sql);
        let root: i64 = q("select package_id from t_package where parent_id = 0 order by package_id limit 1")[0][0].parse().unwrap();
        let name = format!("ZZ_test_mostu_{}", chrono::Local::now().format("%H%M%S"));
        let ops = vec![
            json!({"op":"create_package","key":"pkg","parent":root,"name":name}),
            json!({"op":"create_diagram","key":"dia","package":"#pkg","name":"Colors","type":"Activity"}),
            json!({"op":"create_element","key":"a","package":"#pkg","name":"A","type":"Activity","color":"#FF0000"}),
            json!({"op":"create_element","key":"b","package":"#pkg","name":"B","type":"Activity"}),
            json!({"op":"place","diagram":"#dia","element":"#a","left":40,"top":40,"width":110,"height":60}),
            json!({"op":"place","diagram":"#dia","element":"#b","left":200,"top":40,"width":110,"height":60,"bgcolor":"#00FF00"}),
            json!({"op":"set_color","diagram":"#dia","element":"#a","bgcolor":"#0000FF"}),
        ];
        let created = run_batch(&cfg, &ops, &BatchOptions { rollback_key: Some("pkg".into()), keep_locks: false }).expect("writing colors");
        let id_of = |key: &str| created["created"].as_array().unwrap().iter().find(|c| c["key"] == key).expect(key)["id"].as_i64().unwrap();
        let (pkg, a, b) = (id_of("pkg"), id_of("a"), id_of("b"));
        let element = q(&format!("select backcolor from t_object where object_id = {a}"));
        let on_diagram = q(&format!("select object_id, objectstyle from t_diagramobjects where object_id in ({a}, {b}) order by object_id"));
        println!("default color of A: {} | on the diagram: {:?}", element[0][0], on_diagram);
        run_batch(&cfg, &[json!({"op":"delete","package":pkg})], &BatchOptions::default()).expect("deleting the test package");
        shutdown();

        // EA stores colors as a number in BGR order: red = 255, green = 65280, blue = 16711680.
        assert_eq!(element[0][0], "255");
        assert!(on_diagram[0][1].contains("BCol=16711680"), "color of A on the diagram: {}", on_diagram[0][1]);
        assert!(on_diagram[1][1].contains("BCol=65280"), "color of B on the diagram: {}", on_diagram[1][1]);
    }
    /// Moving an element to another package and a navigation cell, on a real EA; the test package is deleted.
    /// Run with: `cargo test ea_move_and_navigation -- --ignored --nocapture`.
    #[test]
    #[ignore = "requires EA, a license and the model database from app.properties"]
    fn ea_move_and_navigation() {
        let cfg = test_config();
        let _ea = CloseEa;
        let q = |sql: &str| crate::tools::query_rows(&cfg, sql).expect(sql);
        let root: i64 = q("select package_id from t_package where parent_id = 0 order by package_id limit 1")[0][0].parse().unwrap();
        let name = format!("ZZ_test_mostu_{}", chrono::Local::now().format("%H%M%S"));

        // Step 1: a package with two subpackages, a requirement in the first one, a target diagram and a start diagram.
        let ops = vec![
            json!({"op":"create_package","key":"pkg","parent":root,"name":name}),
            json!({"op":"create_package","key":"a","parent":"#pkg","name":"A"}),
            json!({"op":"create_package","key":"b","parent":"#pkg","name":"B"}),
            json!({"op":"create_element","key":"req","package":"#a","name":"FR01","type":"Requirement"}),
            json!({"op":"create_element","key":"uc","package":"#a","name":"UC01","type":"UseCase"}),
            json!({"op":"create_connector","source":"#uc","target":"#req","type":"Realisation"}),
            json!({"op":"create_diagram","key":"target","package":"#a","name":"Use cases","type":"Use Case"}),
            json!({"op":"create_diagram","key":"start","package":"#pkg","name":"Start","type":"Package"}),
            json!({"op":"navigation_cell","key":"cell","diagram":"#start","target":"#target","left":40,"top":40}),
        ];
        let created = run_batch(&cfg, &ops, &BatchOptions::default()).expect("creation through COM");
        let id_of = |key: &str| {
            created["created"].as_array().unwrap().iter().find(|c| c["key"] == key).expect(key)["id"].as_i64().unwrap()
        };
        let (pkg, b, req, target, cell) = (id_of("pkg"), id_of("b"), id_of("req"), id_of("target"), id_of("cell"));

        // Step 2: moving the existing requirement to package B (a separate call, the way the assistant does it).
        run_batch(&cfg, &[json!({"op":"move","package":b,"element":req})], &BatchOptions::default()).expect("move through COM");

        let moved = q(&format!("select package_id from t_object where object_id = {req}"));
        let links = q(&format!("select count(*) from t_connector where end_object_id = {req}"));
        let nav = q(&format!("select object_type, stereotype, ntype, pdata1, styleex from t_object where object_id = {cell}"));
        let on_diagram = q(&format!("select count(*) from t_diagramobjects where object_id = {cell}"));
        println!("requirement in package {} (expected {b}), connectors {} | cell: {:?} | on the diagram: {}", moved[0][0], links[0][0], nav[0], on_diagram[0][0]);

        // Step 3: deleting the way the assistant requests it: removal from the diagram, connector, element (cell), diagram.
        let (uc, start) = (id_of("uc"), id_of("start"));
        let cell_guid = q(&format!("select ea_guid from t_object where object_id = {cell}"))[0][0].clone();
        let connector: i64 = q(&format!("select connector_id from t_connector where end_object_id = {req}"))[0][0].parse().unwrap();
        run_batch(&cfg, &[json!({"op":"place","diagram":target,"element":uc,"left":40,"top":40,"width":90,"height":60})], &BatchOptions::default())
            .expect("placing on the diagram");
        let placed = q(&format!("select count(*) from t_diagramobjects where diagram_id = {target} and object_id = {uc}"));
        let deletes = vec![
            json!({"op":"remove_from_diagram","diagram":target,"element":uc}),
            json!({"op":"delete","connector":connector}),
            json!({"op":"delete","element":cell}),
            json!({"op":"delete","diagram":start}),
        ];
        let deleted = run_batch(&cfg, &deletes, &BatchOptions::default());
        let after = q(&format!(
            "select (select count(*) from t_diagramobjects where diagram_id = {target} and object_id = {uc}), \
             (select count(*) from t_object where object_id = {uc}), \
             (select count(*) from t_connector where connector_id = {connector}), \
             (select count(*) from t_object where object_id = {cell}), \
             (select count(*) from t_diagramobjects where object_id = {cell}), \
             (select count(*) from t_diagram where diagram_id = {start}), \
             (select count(*) from t_xref where client = '{cell_guid}')"
        ));
        println!("deleting: {:?} | before: on the diagram {} | after: {:?}", deleted.as_ref().map(|_| "ok"), placed[0][0], after[0]);

        run_batch(&cfg, &[json!({"op":"delete","package":pkg})], &BatchOptions::default()).expect("deleting the test package");
        shutdown();
        let left = q(&format!("select count(*) from t_package where package_id = {pkg}"));

        deleted.expect("deleting through COM");
        assert_eq!(placed[0][0], "1");
        assert_eq!(after[0], vec!["0", "1", "0", "0", "0", "0", "0"], "removed from the diagram, the element stayed, the rest deleted, no leftovers in t_xref");
        assert_eq!(left[0][0], "0", "the test package was not deleted");

        assert_eq!(moved[0][0], b.to_string(), "the requirement was not moved");
        assert_eq!(links[0][0], "1", "the connector did not survive the move");
        assert_eq!(nav[0][0], "Text");
        assert_eq!(nav[0][1], "NavigationCell");
        assert_eq!(nav[0][2], "82");
        assert_eq!(nav[0][3], target.to_string(), "the cell does not point to the target diagram");
        assert_eq!(nav[0][4], "NID=2-61;", "icon for a Use Case diagram");
        assert_eq!(on_diagram[0][0], "1");
        assert_eq!(q("select count(*) from t_seclocks where userid = (select userid from t_secuser where userlogin = 'claude')")[0][0], "0", "locks were left behind");
    }

    /// Test on a real EA and the real database from the settings: creates a test package with a BPMN diagram,
    /// checks it in the database and deletes it. Run with: `cargo test ea_roundtrip -- --ignored --nocapture`.
    #[test]
    #[ignore = "requires EA, a license and the model database from app.properties"]
    fn ea_roundtrip() {
        let cfg = test_config();
        let _ea = CloseEa;
        let name = format!("ZZ_test_mostu_{}", chrono::Local::now().format("%H%M%S"));
        let root = crate::tools::query_rows(&cfg, "select package_id from t_package where parent_id = 0 order by package_id limit 1")
            .expect("root package")[0][0]
            .parse::<i64>()
            .unwrap();

        let ops = vec![
            json!({"op":"create_package","key":"pkg","parent":root,"name":name,"notes":"COM bridge test package. Zażółć gęślą jaźń."}),
            json!({"op":"create_diagram","key":"dia","package":"#pkg","name":"Test BPMN","type":"BPMN2.0::Business Process"}),
            json!({"op":"create_element","key":"start","package":"#pkg","name":"Start","type":"Event","stereotype":"BPMN2.0::StartEvent"}),
            json!({"op":"create_element","key":"task","package":"#pkg","name":"Manual task","type":"Activity","stereotype":"BPMN2.0::Activity","tags":{"taskType":"Manual"}}),
            json!({"op":"create_element","key":"gw","package":"#pkg","name":"","type":"Decision","stereotype":"BPMN2.0::Gateway","tags":{"gatewayType":"Parallel"}}),
            json!({"op":"create_element","key":"end","package":"#pkg","name":"End","type":"Event","stereotype":"BPMN2.0::EndEvent","tags":{"eventDefinition":"Terminate"}}),
            json!({"op":"create_connector","source":"#start","target":"#task","type":"ControlFlow","stereotype":"BPMN2.0::SequenceFlow"}),
            json!({"op":"create_connector","source":"#task","target":"#gw","type":"ControlFlow","stereotype":"BPMN2.0::SequenceFlow","name":"next"}),
            json!({"op":"create_connector","source":"#gw","target":"#end","type":"ControlFlow","stereotype":"BPMN2.0::SequenceFlow"}),
            json!({"op":"place","diagram":"#dia","element":"#start","left":40,"top":72,"width":36,"height":36}),
            json!({"op":"place","diagram":"#dia","element":"#task","left":130,"top":50,"width":100,"height":80}),
            json!({"op":"place","diagram":"#dia","element":"#gw","left":290,"top":65,"width":50,"height":50}),
            json!({"op":"place","diagram":"#dia","element":"#end","left":400,"top":72,"width":36,"height":36}),
        ];
        let result = run_batch(&cfg, &ops, &BatchOptions::default()).expect("write through COM");
        println!("logged in: {} | security: {} | created: {}", result["user"], result["security"], result["created"].as_array().unwrap().len());
        for w in result["warnings"].as_array().unwrap() {
            println!("warning: {w}");
        }

        let q = |sql: &str| crate::tools::query_rows(&cfg, sql).expect(sql);
        let pkg = q(&format!("select package_id, notes from t_package where name = '{name}'"));
        assert_eq!(pkg.len(), 1, "the package should be in the database");
        let pkg_id = &pkg[0][0];
        println!("package {pkg_id}, notes: {}", pkg[0][1]);
        assert!(pkg[0][1].contains("gęślą"), "Polish characters in the notes must pass through unchanged");

        for row in q(&format!("select object_type, stereotype, name from t_object where package_id = {pkg_id} order by object_id")) {
            println!("element: {}", row.join(" | "));
        }
        for row in q(&format!(
            "select p.property, p.value from t_objectproperties p join t_object o on o.object_id = p.object_id \
             where o.package_id = {pkg_id} and p.property in ('taskType','gatewayType','eventDefinition','activityType') order by o.object_id"
        )) {
            println!("tag: {}", row.join(" = "));
        }
        let conns = q(&format!(
            "select count(*) from t_connector c join t_object o on o.object_id = c.start_object_id where o.package_id = {pkg_id}"
        ));
        assert_eq!(conns[0][0], "3", "three flows");
        let placed = q(&format!(
            "select count(*) from t_diagramobjects d join t_diagram g on g.diagram_id = d.diagram_id where g.package_id = {pkg_id}"
        ));
        assert_eq!(placed[0][0], "4", "four objects on the diagram");
        let diagram = &q(&format!("select diagram_type, styleex from t_diagram where package_id = {pkg_id}"))[0];
        assert!(diagram[1].contains("MDGDgm=BPMN2.0::Business Process"), "BPMN diagram: {diagram:?}");
        // Right after the write the user must be able to edit what was created: no locks of the claude account.
        assert_eq!(q("select count(*) from t_seclocks l join t_secuser u on u.userid = l.userid where u.userlogin = 'claude'")[0][0], "0", "locks after the write");
        assert_eq!(q("select count(*) from t_object where package_id = 0")[0][0], "0", "objects without a package after the write");

        // Diagram dump to PNG (for inspection) and cleanup.
        let png = std::env::temp_dir().join("loonar-compass").join("test_bpmn.png");
        let dia_id: i64 = q(&format!("select diagram_id from t_diagram where package_id = {pkg_id}"))[0][0].parse().unwrap();
        run_batch(&cfg, &[json!({"op":"export_diagram","diagram":dia_id,"path":png.to_string_lossy()})], &BatchOptions::default())
            .expect("diagram export");
        println!("png: {} ({} B)", png.display(), std::fs::metadata(&png).map(|m| m.len()).unwrap_or(0));

        run_batch(&cfg, &[json!({"op":"delete_package","package":pkg_id.parse::<i64>().unwrap()})], &BatchOptions::default())
            .expect("deleting the test package");
        assert_eq!(q(&format!("select count(*) from t_package where name = '{name}'"))[0][0], "0", "test package deleted");
        // The bridge must not leave locks or objects without a package behind.
        assert_eq!(q("select count(*) from t_seclocks l join t_secuser u on u.userid = l.userid where u.userlogin = 'claude'")[0][0], "0", "claude account locks after the write");
        assert_eq!(q("select count(*) from t_object where package_id = 0")[0][0], "0", "objects without a package");
        println!("locks and objects without a package after the whole cycle: 0");
    }
}

#[cfg(test)]
mod partition_probe {
    use super::*;
    use serde_json::json;

    /// Tall activity partitions come out vertical (LOONAR_TEST_PROPS = a test copy; LOONAR_PROBE_PNG = image to look at).
    #[test]
    #[ignore]
    fn vertical_partitions() {
        let props = std::env::var("LOONAR_TEST_PROPS").expect("LOONAR_TEST_PROPS");
        let cfg = AppConfig::load(std::path::Path::new(&props)).unwrap();
        let root: i64 = crate::tools::query_rows(&cfg, "select min(package_id) from t_package where parent_id = 0").unwrap()[0][0].parse().unwrap();
        let ops = vec![
            json!({"op":"create_package","key":"pkg","parent":root,"name":"ZZ_partition_probe"}),
            json!({"op":"create_diagram","key":"dia","package":"#pkg","name":"Probe","type":"Activity"}),
            json!({"op":"create_element","key":"a","package":"#pkg","name":"LaneA","type":"ActivityPartition"}),
            json!({"op":"create_element","key":"b","package":"#pkg","name":"LaneB","type":"ActivityPartition"}),
            json!({"op":"place","diagram":"#dia","element":"#a","left":20,"top":20,"width":200,"height":500}),
            json!({"op":"place","diagram":"#dia","element":"#b","left":240,"top":20,"width":500,"height":200}),
        ];
        let created = run_batch(&cfg, &ops, &BatchOptions { rollback_key: Some("pkg".into()), keep_locks: false }).expect("creation");
        let id_of = |key: &str| created["created"].as_array().unwrap().iter().find(|c| c["key"] == key).unwrap()["id"].as_i64().unwrap();
        if let Ok(out) = std::env::var("LOONAR_PROBE_PNG") {
            run_batch(&cfg, &[json!({"op":"export_diagram","diagram":id_of("dia"),"path":out})], &BatchOptions::default()).expect("export");
        }
        run_batch(&cfg, &[json!({"op":"delete_package","package":id_of("pkg")})], &BatchOptions::default()).expect("cleanup");
        shutdown();
    }
}

#[cfg(test)]
mod glossary_probe {
    use super::*;
    use serde_json::json;

    /// A term with its forms goes into the project glossary (LOONAR_TEST_PROPS = a test copy).
    #[test]
    #[ignore]
    fn glossary_term_with_forms() {
        let props = std::env::var("LOONAR_TEST_PROPS").expect("LOONAR_TEST_PROPS");
        let cfg = AppConfig::load(std::path::Path::new(&props)).unwrap();
        let op = json!({"op":"add_glossary_term","term":"zamek","type":"Technical","meaning":"element techniczny utrzymujący zabezpieczenie środowiska",
                        "forms":["zamka","zamkowi","zamkiem","zamku","zamki","zamków","zamkom","zamkami","zamkach"]});
        let result = run_batch(&cfg, &[op.clone()], &BatchOptions::default()).expect("glossary");
        let again = run_batch(&cfg, &[op], &BatchOptions::default()).expect("glossary again");
        shutdown();
        println!("{} | {}", result["warnings"], again["warnings"]);
        let rows = crate::tools::query_rows(&cfg, "select count(*) from t_glossary where meaning like 'element techniczny%'").unwrap();
        assert_eq!(rows[0][0], "10");
    }
}

#[cfg(test)]
mod scenario_probe {
    use super::*;
    use serde_json::json;

    /// A structured scenario with steps on an element (LOONAR_TEST_PROPS = a test copy).
    #[test]
    #[ignore]
    fn structured_scenario() {
        let props = std::env::var("LOONAR_TEST_PROPS").expect("LOONAR_TEST_PROPS");
        let cfg = AppConfig::load(std::path::Path::new(&props)).unwrap();
        let id: i64 = crate::tools::query_rows(&cfg, "select min(object_id) from t_object where object_type in ('UseCase','Class')").unwrap()[0][0].parse().unwrap();
        let op = json!({"op":"create_scenario","element":id,"name":"Zakup produktu","notes":"Klient kupuje produkt.",
            "steps":[{"text":"Klient wybiera produkt","actor":true},{"text":"System pokazuje koszyk"},{"text":"Klient płaci","actor":true,"uses":"Płatność"}]});
        let result = run_batch(&cfg, &[op], &BatchOptions::default());
        let update = run_batch(&cfg, &[json!({"op":"update_scenario","element":id,"scenario":"Zakup produktu","name":"Zakup produktu v2",
            "steps":[{"text":"Klient loguje się","actor":true},{"text":"System pokazuje koszyk"}]})], &BatchOptions::default());
        println!("update: {update:?}");
        let rows2 = crate::tools::query_rows(&cfg, &format!("select scenario, xmlcontent from t_objectscenarios where object_id = {id}")).unwrap();
        println!("after update: {rows2:?}");
        let delete = run_batch(&cfg, &[json!({"op":"delete_scenario","element":id,"scenario":"Zakup produktu v2"})], &BatchOptions::default());
        println!("delete: {delete:?}");
        let rows3 = crate::tools::query_rows(&cfg, &format!("select count(*) from t_objectscenarios where object_id = {id}")).unwrap();
        println!("after delete: {rows3:?}");
        shutdown();
        println!("{result:?}");
        result.expect("scenario");
        let rows: Vec<Vec<String>> = Vec::new();
        let _ = rows;
        assert!(rows2.iter().any(|r| r[0] == "Zakup produktu v2" && r[1].contains("Klient loguje się") && !r[1].contains("Klient płaci")));
        assert_eq!(rows3[0][0], "0");
    }
}

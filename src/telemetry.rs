//! Anonymous usage statistics: at the start a ping with the random installation id, then every hour the summary of the
//! day so far (and of ended days not sent yet) goes to the Loonar feedback service (Cloudflare D1, no IP addresses);
//! only summaries over all installations together are published (GET /v1/summary).
//!
//! What is counted: how often each tool of the assistant was used and how often it failed, answers of the assistant,
//! errors of the AI service, approvals asked / accepted / rejected, views opened, tokens, and the setup of the day
//! (model source, AI provider and model family, approval mode, which integrations are configured). Never: the text
//! of conversations, names or ids from the model, file paths, server addresses, error messages or user names.
//! Tools of the user's own MCP servers count as "mcp" (their names are not sent).
//!
//! Every process (Compass in each EA tab, the standalone window) keeps its own counters in
//! `telemetry/days/<day>/<pid>-<start>.json`; the summary of a day merges them. A day is sent once, after it has
//! ended; the exact JSON sent is kept in `telemetry/sent/<day>.json` for 30 days and shown in the Statistics view.
//!
//! Off: the setting `telemetry.enabled=false`, the environment variable `LOONAR_TELEMETRY=0`, the registry value
//! `Telemetry` = 0 under `HKLM` or `HKCU\SOFTWARE\Policies\Loonar\Compass`, or `"telemetry": false` in the team
//! hub's policy. Then nothing is counted or sent, and unsent days are deleted.

use anyhow::{bail, Context, Result};
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// The feedback service (statistics and bug reports). `LOONAR_FEEDBACK_URL` replaces it (https, or http on this
/// computer for tests).
pub const SERVICE_URL: &str = "https://compass-feedback.loonar-eu.workers.dev";
/// The public repository with the statistics.
pub const STATS_URL: &str = "https://compass-feedback.loonar-eu.workers.dev/v1/summary";
/// Days kept: sent ones (shown in the view) and unsent ones (waiting for a connection).
const KEEP_DAYS: i64 = 30;
/// Counters are written to disk at most this often (and at the end of every answer).
const FLUSH_EVERY: Duration = Duration::from_secs(60);
/// Most different keys in one group (a defence against a runaway counter).
const MAX_KEYS: usize = 120;

struct State {
    dir: PathBuf,
    enabled: bool,
    day: String,
    file: PathBuf,
    counters: Map<String, Value>,
    setup: Value,
    dirty: bool,
    flushed: Instant,
}

static STATE: Mutex<Option<State>> = Mutex::new(None);
/// One send at a time within the process.
static SENDING: Mutex<()> = Mutex::new(());
/// Why the last send failed (shown in the Statistics view; the days wait for the next try).
static LAST_ERROR: Mutex<String> = Mutex::new(String::new());

pub fn last_error() -> String {
    LAST_ERROR.lock().unwrap().clone()
}

fn today() -> String {
    chrono::Local::now().format("%Y-%m-%d").to_string()
}

fn process_file(dir: &Path, day: &str) -> PathBuf {
    let start = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);
    dir.join("days").join(day).join(format!("{}-{start}.json", std::process::id()))
}

/// Starts counting for this process. `dir` = the folder of app.properties (statistics go to `<dir>\telemetry`).
pub fn init(settings_dir: &Path, enabled: bool) {
    let dir = settings_dir.join("telemetry");
    let day = today();
    let file = process_file(&dir, &day);
    let enabled = enabled && policy_block().is_none();
    *STATE.lock().unwrap() = Some(State { dir, enabled, day, file, counters: Map::new(), setup: Value::Null, dirty: false, flushed: Instant::now() });
    if !enabled {
        discard_unsent();
    }
}

/// Switches counting on or off (the Statistics view). Off also deletes the days not sent yet.
pub fn set_enabled(enabled: bool) {
    let enabled = enabled && policy_block().is_none();
    if let Some(s) = STATE.lock().unwrap().as_mut() {
        s.enabled = enabled;
        if !enabled {
            s.counters.clear();
            s.dirty = false;
        }
    }
    if !enabled {
        discard_unsent();
    }
}

pub fn enabled() -> bool {
    STATE.lock().unwrap().as_ref().is_some_and(|s| s.enabled)
}

/// Why statistics are off regardless of the setting: the environment variable or the registry policy.
pub fn policy_block() -> Option<String> {
    if let Ok(v) = std::env::var("LOONAR_TELEMETRY") {
        if matches!(v.trim().to_ascii_lowercase().as_str(), "0" | "off" | "false" | "no") {
            return Some("LOONAR_TELEMETRY".into());
        }
    }
    registry_block()
}

#[cfg(windows)]
fn registry_block() -> Option<String> {
    use windows::core::w;
    use windows::Win32::System::Registry::{RegGetValueW, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, RRF_RT_REG_DWORD};
    for (root, name) in [(HKEY_LOCAL_MACHINE, "HKLM"), (HKEY_CURRENT_USER, "HKCU")] {
        let mut value: u32 = 1;
        let mut size = std::mem::size_of::<u32>() as u32;
        let found = unsafe {
            RegGetValueW(root, w!("SOFTWARE\\Policies\\Loonar\\Compass"), w!("Telemetry"), RRF_RT_REG_DWORD, None, Some(&mut value as *mut u32 as *mut _), Some(&mut size))
        };
        if found.is_ok() && value == 0 {
            return Some(format!("{name}\\SOFTWARE\\Policies\\Loonar\\Compass\\Telemetry = 0"));
        }
    }
    None
}

#[cfg(not(windows))]
fn registry_block() -> Option<String> {
    None
}

fn with_state(f: impl FnOnce(&mut State)) {
    let mut guard = STATE.lock().unwrap();
    let Some(s) = guard.as_mut() else { return };
    if !s.enabled {
        return;
    }
    // A new day: the counters of the previous one go to disk, the new day starts empty.
    let day = today();
    if day != s.day {
        let _ = write_counters(s);
        s.day = day;
        s.file = process_file(&s.dir, &s.day);
        s.counters.clear();
        s.dirty = false;
    }
    f(s);
    s.dirty = true;
    if s.flushed.elapsed() > FLUSH_EVERY {
        let _ = write_counters(s);
    }
}

fn bump(counters: &mut Map<String, Value>, group: &str, key: &str, by: u64) {
    if group.is_empty() {
        let n = counters.get(key).and_then(Value::as_u64).unwrap_or(0);
        counters.insert(key.to_string(), json!(n + by));
        return;
    }
    let map = counters.entry(group.to_string()).or_insert_with(|| json!({}));
    if let Some(map) = map.as_object_mut() {
        if map.len() >= MAX_KEYS && !map.contains_key(key) {
            return;
        }
        let n = map.get(key).and_then(Value::as_u64).unwrap_or(0);
        map.insert(key.to_string(), json!(n + by));
    }
}

/// A key that can be published: tool and view names (lowercase letters, digits, '_'), at most 40 characters.
/// Tools of the user's MCP servers are counted together as "mcp".
fn key_of(name: &str) -> String {
    if name.starts_with("mcp__") {
        return "mcp".into();
    }
    let clean: String = name.chars().filter(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == '_').take(40).collect();
    if clean.is_empty() { "other".into() } else { clean }
}

/// A call of a tool of the assistant and whether it succeeded.
pub fn tool(name: &str, ok: bool) {
    let key = key_of(name);
    if let Some(task) = TASK.lock().unwrap_or_else(|p| p.into_inner()).as_mut() {
        task.tools.push(name.to_string());
        task.failed += !ok as usize;
    }
    with_state(|s| {
        bump(&mut s.counters, "tools", &key, 1);
        if !ok {
            bump(&mut s.counters, "tool_errors", &key, 1);
        }
    });
}

/// An answer of the assistant was started (one user message).
pub fn turn() {
    *TASK.lock().unwrap_or_else(|p| p.into_inner()) = Some(Task { started: Some(Instant::now()), ..Default::default() });
    with_state(|s| bump(&mut s.counters, "", "turns", 1));
}

/// The answer being made: what the user's request turned into (LC-288). No text, only the names of the tools.
#[derive(Default)]
struct Task {
    started: Option<Instant>,
    tools: Vec<String>,
    failed: usize,
    rejected: usize,
    tokens: u64,
    gap: bool,
}

static TASK: Mutex<Option<Task>> = Mutex::new(None);

/// The kind of task, from the tools of the answer: the most telling one wins (a change of the model over the
/// reading that came before it).
pub fn task_category(tools: &[String]) -> &'static str {
    let has = |names: &[&str]| tools.iter().any(|t| names.contains(&t.as_str()));
    let prefix = |p: &str| tools.iter().any(|t| t.starts_with(p));
    if has(&["code_tree", "code_search", "code_read", "code_drift"]) {
        "code"
    } else if has(&["create_presentation"]) {
        "presentation"
    } else if has(&["create_document"]) {
        "document"
    } else if has(&["create_spreadsheet"]) {
        "spreadsheet"
    } else if has(&["create_confluence_page", "publish_confluence", "confluence_read", "confluence_pages"]) {
        "confluence"
    } else if prefix("jira_") {
        "jira"
    } else if has(&["sharepoint", "onedrive", "google_drive"]) {
        "cloud_files"
    } else if has(&["ea_write", "ea_script", "propose_change", "undo_change", "version_control_status"]) {
        "model_change"
    } else if has(&["read_document", "trace_requirements"]) {
        "requirements"
    } else if has(&["model_lint", "gap_analysis", "spell_check", "cmdb_compare"]) {
        "model_check"
    } else if has(&["impact_analysis", "operation_usage", "estimate_change"]) {
        "impact"
    } else if has(&["ea_show_diagram", "render_image"]) {
        "diagram"
    } else if has(&["web_search", "web_fetch"]) {
        "web"
    } else if prefix("mcp__") {
        "mcp"
    } else if tools.iter().any(|t| !matches!(t.as_str(), "task_list" | "ask_user" | "memory_read" | "memory_save" | "memory_delete" | "report_gap")) {
        "model_question"
    } else {
        "chat"
    }
}

fn bucket(n: u64, limits: &[u64]) -> String {
    limits.iter().find(|l| n <= **l).map(|l| format!("le_{l}")).unwrap_or_else(|| format!("gt_{}", limits.last().unwrap_or(&0)))
}

/// The end of the answer: one record of the task (kind, outcome, steps, tokens and time in ranges).
pub fn end_task(ok: bool, stopped: bool) {
    let Some(task) = TASK.lock().unwrap_or_else(|p| p.into_inner()).take() else { return };
    let category = task_category(&task.tools);
    let outcome = if stopped {
        "stopped"
    } else if !ok {
        "error"
    } else if task.gap {
        "cannot"
    } else if task.rejected > 0 {
        "rejected"
    } else if task.failed > 0 {
        "partial"
    } else {
        "done"
    };
    let seconds = task.started.map_or(0, |s| s.elapsed().as_secs());
    with_state(|s| {
        bump(&mut s.counters, "tasks", &format!("{category}__{outcome}"), 1);
        bump(&mut s.counters, "task_steps", &bucket(task.tools.len() as u64, &[0, 2, 5, 10, 20]), 1);
        bump(&mut s.counters, "task_tokens", &bucket(task.tokens, &[5_000, 20_000, 50_000, 100_000, 300_000]), 1);
        bump(&mut s.counters, "task_seconds", &bucket(seconds, &[10, 30, 60, 180, 600]), 1);
    });
}

/// The assistant could not do something (the report_gap tool, a fixed category; LC-289).
pub fn gap(category: &str) {
    if let Some(task) = TASK.lock().unwrap_or_else(|p| p.into_inner()).as_mut() {
        task.gap = true;
    }
    let key = key_of(category);
    with_state(|s| bump(&mut s.counters, "gaps", &key, 1));
}

/// The user rated an answer (LC-290): up, or down with one of the fixed reasons.
pub fn rating(up: bool, reason: &str) {
    let key = if up { "up".to_string() } else { format!("down_{}", key_of(reason)) };
    with_state(|s| bump(&mut s.counters, "ratings", &key, 1));
}

/// The AI service failed (connection, limit, refusal, cut-off answer).
pub fn ai_error() {
    with_state(|s| bump(&mut s.counters, "", "ai_errors", 1));
}

/// Tokens of one model response.
pub fn tokens(input: u64, output: u64) {
    if let Some(task) = TASK.lock().unwrap_or_else(|p| p.into_inner()).as_mut() {
        task.tokens += input + output;
    }
    with_state(|s| {
        bump(&mut s.counters, "tokens", "in", input);
        bump(&mut s.counters, "tokens", "out", output);
    });
}

/// An approval was asked; `accepted` = the user agreed.
pub fn approval(accepted: bool) {
    if let Some(task) = TASK.lock().unwrap_or_else(|p| p.into_inner()).as_mut() {
        task.rejected += !accepted as usize;
    }
    with_state(|s| {
        bump(&mut s.counters, "approvals", "asked", 1);
        bump(&mut s.counters, "approvals", if accepted { "accepted" } else { "rejected" }, 1);
    });
}

/// A view of the application was opened (chat, settings, …).
pub fn view(name: &str) {
    let key = key_of(name);
    with_state(|s| bump(&mut s.counters, "views", &key, 1));
}

/// Something else worth counting: a session in an EA tab or standalone, an event agent run, a bug report…
pub fn event(name: &str) {
    let key = key_of(name);
    with_state(|s| bump(&mut s.counters, "events", &key, 1));
}

/// The setup of the day (only categories, see `setup_of`); the last one of the day is sent.
pub fn setup(setup: Value) {
    with_state(|s| s.setup = setup);
}

/// Writes the counters of this process (end of an answer, closing the application).
pub fn flush() {
    if let Some(s) = STATE.lock().unwrap().as_mut() {
        if s.enabled && s.dirty {
            let _ = write_counters(s);
        }
    }
}

fn write_counters(s: &mut State) -> Result<()> {
    if s.counters.is_empty() && s.setup.is_null() {
        return Ok(());
    }
    std::fs::create_dir_all(s.file.parent().context("no folder")?)?;
    let mut data = s.counters.clone();
    data.insert("app".into(), json!(env!("CARGO_PKG_VERSION")));
    if !s.setup.is_null() {
        data.insert("setup".into(), s.setup.clone());
    }
    let tmp = s.file.with_extension("tmp");
    std::fs::write(&tmp, serde_json::to_vec(&Value::Object(data))?)?;
    std::fs::rename(&tmp, &s.file)?;
    s.dirty = false;
    s.flushed = Instant::now();
    Ok(())
}

/// The setup in categories only: the AI model family (a custom model name is "other"), no addresses or names.
pub fn setup_of(cfg: &crate::config::AppConfig, in_ea_tab: bool, mcp_servers: usize, event_agents: usize) -> Value {
    let model = cfg.ai_model().to_ascii_lowercase();
    let family = ["claude-", "gpt-", "o1", "o3", "o4", "gemini-", "llama", "mistral", "qwen", "deepseek", "phi"]
        .iter()
        .find(|p| model.starts_with(**p))
        .map(|_| model.chars().filter(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_')).take(40).collect::<String>())
        .unwrap_or_else(|| "other".into());
    json!({
        "mode": if in_ea_tab { "ea_tab" } else { "standalone" },
        "model_source": if matches!(cfg.model_source, crate::config::ModelSource::File) { "file" } else { "postgres" },
        "ai_provider": provider_kind(cfg),
        "ai_model": family,
        "approval": cfg.approval_mode.as_str(),
        "jira": !cfg.jira_url.trim().is_empty(),
        "confluence": !cfg.confluence_url.trim().is_empty(),
        "hub": !cfg.hub_url.trim().is_empty(),
        "mcp_servers": mcp_servers,
        "event_agents": event_agents,
    })
}

/// The kind of AI service only, never its address: Anthropic, OpenAI, Azure OpenAI, a model on this computer or
/// another OpenAI-compatible service.
fn provider_kind(cfg: &crate::config::AppConfig) -> &'static str {
    if !cfg.uses_openai() {
        return "anthropic";
    }
    let url = cfg.openai_url.trim().to_ascii_lowercase();
    if url.is_empty() || url.contains("api.openai.com") {
        "openai"
    } else if url.contains(".azure.com") || url.contains("azure") {
        "azure"
    } else if url.contains("localhost") || url.contains("127.0.0.1") || url.contains("[::1]") {
        "local"
    } else {
        "openai_compatible"
    }
}

/// The identifier of this installation: a random UUID, created on first use. It joins the days of one
/// installation in the statistics and is what you quote when you report something.
pub fn install_id() -> String {
    let Some(dir) = STATE.lock().unwrap().as_ref().map(|s| s.dir.clone()) else { return String::new() };
    let path = dir.join("install_id");
    if let Ok(id) = std::fs::read_to_string(&path) {
        let id = id.trim().to_string();
        if uuid::Uuid::parse_str(&id).is_ok() {
            return id;
        }
    }
    let id = uuid::Uuid::new_v4().to_string();
    let _ = std::fs::create_dir_all(&dir);
    let _ = std::fs::write(&path, &id);
    id
}

/// A new identifier: later days can no longer be joined with the earlier ones.
pub fn new_install_id() -> String {
    if let Some(dir) = STATE.lock().unwrap().as_ref().map(|s| s.dir.clone()) {
        let _ = std::fs::remove_file(dir.join("install_id"));
    }
    install_id()
}

fn telemetry_dir() -> Option<PathBuf> {
    STATE.lock().unwrap().as_ref().map(|s| s.dir.clone())
}

fn add_maps(into: &mut Map<String, Value>, from: &Map<String, Value>) {
    for (k, v) in from {
        match v {
            Value::Number(n) => {
                let sum = into.get(k).and_then(Value::as_u64).unwrap_or(0) + n.as_u64().unwrap_or(0);
                into.insert(k.clone(), json!(sum));
            }
            Value::Object(m) => {
                let entry = into.entry(k.clone()).or_insert_with(|| json!({}));
                if let Some(e) = entry.as_object_mut() {
                    add_maps(e, m);
                }
            }
            _ => {}
        }
    }
}

/// The summary of a day as it will be sent: the counters of all processes of that day added up.
pub fn day_summary(day: &str) -> Option<Value> {
    let dir = telemetry_dir()?;
    let folder = dir.join("days").join(day);
    let mut counters = Map::new();
    let mut setup = Value::Null;
    let mut newest = std::time::UNIX_EPOCH;
    let mut app = String::new();
    for entry in std::fs::read_dir(&folder).ok()?.flatten() {
        let path = entry.path();
        if path.extension().is_none_or(|e| e != "json") {
            continue;
        }
        let Ok(Value::Object(data)) = std::fs::read(&path).map_err(anyhow::Error::from).and_then(|b| Ok(serde_json::from_slice(&b)?)) else { continue };
        let modified = entry.metadata().and_then(|m| m.modified()).unwrap_or(std::time::UNIX_EPOCH);
        if modified >= newest {
            newest = modified;
            if let Some(s) = data.get("setup").filter(|s| s.is_object()) {
                setup = s.clone();
            }
            app = data.get("app").and_then(Value::as_str).unwrap_or("").to_string();
        }
        let numbers: Map<String, Value> = data.into_iter().filter(|(k, _)| k != "setup" && k != "app").collect();
        add_maps(&mut counters, &numbers);
    }
    if counters.is_empty() && setup.is_null() {
        return None;
    }
    let mut out = Map::new();
    out.insert("v".into(), json!(1));
    out.insert("install".into(), json!(install_id()));
    out.insert("day".into(), json!(day));
    out.insert("app".into(), json!(if app.is_empty() { env!("CARGO_PKG_VERSION").to_string() } else { app }));
    out.insert("os".into(), json!(os_name()));
    out.insert("lang".into(), json!(crate::i18n::current().code()));
    if setup.is_object() {
        out.insert("setup".into(), setup);
    }
    for (k, v) in counters {
        out.insert(k, v);
    }
    Some(Value::Object(out))
}

/// What today looks like so far (the view shows it before it is sent).
pub fn today_preview() -> Option<Value> {
    flush();
    day_summary(&today())
}

/// "Windows 10" / "Windows 11" and the build number.
fn os_name() -> String {
    let version = os_build();
    match version {
        Some(build) if build >= 22000 => format!("Windows 11 ({build})"),
        Some(build) => format!("Windows 10 ({build})"),
        None => std::env::consts::OS.to_string(),
    }
}

#[cfg(windows)]
fn os_build() -> Option<u32> {
    use windows::core::w;
    use windows::Win32::System::Registry::{RegGetValueW, HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ};
    let mut buf = [0u16; 32];
    let mut size = (buf.len() * 2) as u32;
    let ok = unsafe {
        RegGetValueW(HKEY_LOCAL_MACHINE, w!("SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion"), w!("CurrentBuildNumber"), RRF_RT_REG_SZ, None, Some(buf.as_mut_ptr() as *mut _), Some(&mut size))
    };
    if ok.is_err() {
        return None;
    }
    String::from_utf16_lossy(&buf).trim_end_matches('\0').trim().parse().ok()
}

#[cfg(not(windows))]
fn os_build() -> Option<u32> {
    None
}

fn service_url() -> Result<String> {
    let url = std::env::var("LOONAR_FEEDBACK_URL").ok().filter(|u| !u.trim().is_empty()).unwrap_or_else(|| SERVICE_URL.to_string());
    let url = url.trim().trim_end_matches('/').to_string();
    if crate::net::check_service_url(&url).is_err() {
        bail!("the address of the feedback service is not allowed: {url}");
    }
    Ok(url)
}

fn client() -> Result<reqwest::blocking::Client> {
    Ok(reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(30))
        .redirect(reqwest::redirect::Policy::none())
        .user_agent(concat!("LoonarCompass/", env!("CARGO_PKG_VERSION")))
        .build()?)
}

/// Sends the days that have ended and were not sent yet (at most once per day each). `team_off` = the team hub's
/// policy turns statistics off. Runs in the background; a failed send is tried again later.
pub fn send_due(team_off: bool) {
    if team_off {
        set_enabled(false);
        return;
    }
    if !enabled() {
        return;
    }
    flush();
    std::thread::spawn(|| {
        let _one = SENDING.lock().unwrap();
        // Ended days first, then today so far (sent every hour; the service keeps the newest of a day).
        let result = send_ended_days().and_then(|_| send_today());
        *LAST_ERROR.lock().unwrap() = result.err().map(|e| format!("{e:#}")).unwrap_or_default();
    });
}

/// Today so far, marked partial; the service replaces the earlier summary of the same day and installation.
fn send_today() -> Result<()> {
    let Some(dir) = telemetry_dir() else { return Ok(()) };
    let day = today();
    let Some(Value::Object(mut summary)) = day_summary(&day) else { return Ok(()) };
    summary.insert("partial".into(), json!(true));
    let summary = Value::Object(summary);
    post("/v1/stats", &summary)?;
    let sent_dir = dir.join("sent");
    std::fs::create_dir_all(&sent_dir)?;
    let record = json!({ "sent": chrono::Local::now().to_rfc3339(), "data": summary });
    std::fs::write(sent_dir.join(format!("{day}.partial.json")), serde_json::to_vec_pretty(&record)?)?;
    Ok(())
}

fn post(path: &str, body: &Value) -> Result<()> {
    let response = client()?.post(format!("{}{path}", service_url()?)).header("Content-Type", "application/json").body(serde_json::to_string_pretty(body)?).send()?;
    let status = response.status();
    if !status.is_success() {
        bail!("the service answered {status}");
    }
    Ok(())
}

/// At the start of the application: the installation, its version, language and system (LC-287), so that active
/// installations and their return are known also on days without an answer. Nothing else.
pub fn start_ping(team_off: bool) {
    if team_off || !enabled() {
        return;
    }
    let ping = json!({ "v": 1, "install": install_id(), "app": env!("CARGO_PKG_VERSION"), "os": os_name(), "lang": crate::i18n::current().code(), "day": today() });
    std::thread::spawn(move || {
        let result = post("/v1/start", &ping);
        if let (Ok(()), Some(dir)) = (&result, telemetry_dir()) {
            let _ = std::fs::create_dir_all(dir.join("sent"));
            let record = json!({ "sent": chrono::Local::now().to_rfc3339(), "data": ping });
            let _ = std::fs::write(dir.join("sent").join("start.json"), serde_json::to_vec_pretty(&record).unwrap_or_default());
        }
        if let Err(e) = result {
            *LAST_ERROR.lock().unwrap() = format!("{e:#}");
        }
    });
}

fn send_ended_days() -> Result<()> {
    let Some(dir) = telemetry_dir() else { return Ok(()) };
    prune(&dir);
    let days_dir = dir.join("days");
    let today = today();
    let mut days: Vec<String> = std::fs::read_dir(&days_dir).map(|r| r.flatten().filter_map(|e| e.file_name().into_string().ok()).collect()).unwrap_or_default();
    days.sort();
    let sent_dir = dir.join("sent");
    for day in days.into_iter().filter(|d| d.as_str() < today.as_str()) {
        if sent_dir.join(format!("{day}.json")).exists() {
            let _ = std::fs::remove_dir_all(days_dir.join(&day));
            continue;
        }
        // Another Compass process may be sending the same day: the lock file decides.
        let lock = dir.join(format!("{day}.lock"));
        let stale = std::fs::metadata(&lock).and_then(|m| m.modified()).ok().and_then(|t| t.elapsed().ok()).is_some_and(|age| age > Duration::from_secs(600));
        if stale {
            let _ = std::fs::remove_file(&lock);
        }
        if std::fs::OpenOptions::new().write(true).create_new(true).open(&lock).is_err() {
            continue;
        }
        let result = (|| -> Result<()> {
            let Some(summary) = day_summary(&day) else { return Ok(()) };
            let body = serde_json::to_string_pretty(&summary)?;
            let response = client()?.post(format!("{}/v1/stats", service_url()?)).header("Content-Type", "application/json").body(body.clone()).send()?;
            let status = response.status();
            if !status.is_success() {
                bail!("the service answered {status}");
            }
            std::fs::create_dir_all(&sent_dir)?;
            let record = json!({ "sent": chrono::Local::now().to_rfc3339(), "data": summary });
            std::fs::write(sent_dir.join(format!("{day}.json")), serde_json::to_vec_pretty(&record)?)?;
            Ok(())
        })();
        let _ = std::fs::remove_file(&lock);
        match result {
            Ok(()) => {
                let _ = std::fs::remove_dir_all(days_dir.join(&day));
            }
            Err(e) => return Err(e.context(format!("day {day}"))),
        }
    }
    Ok(())
}

/// Sent records older than 30 days and unsent days older than that are deleted.
fn prune(dir: &Path) {
    let oldest = (chrono::Local::now() - chrono::Duration::days(KEEP_DAYS)).format("%Y-%m-%d").to_string();
    for sub in ["sent", "days"] {
        for entry in std::fs::read_dir(dir.join(sub)).into_iter().flatten().flatten() {
            let name = entry.file_name().to_string_lossy().trim_end_matches(".json").to_string();
            if name.as_str() < oldest.as_str() {
                let path = entry.path();
                let _ = if path.is_dir() { std::fs::remove_dir_all(&path) } else { std::fs::remove_file(&path) };
            }
        }
    }
}

/// Days counted but not sent are deleted (statistics switched off).
fn discard_unsent() {
    if let Some(dir) = telemetry_dir() {
        let _ = std::fs::remove_dir_all(dir.join("days"));
    }
}

/// What was sent in the last 30 days, newest first: (day, time sent, JSON as sent).
pub fn sent_records() -> Vec<(String, String, String)> {
    let Some(dir) = telemetry_dir() else { return Vec::new() };
    let mut out: BTreeMap<String, (String, String)> = BTreeMap::new();
    for entry in std::fs::read_dir(dir.join("sent")).into_iter().flatten().flatten() {
        let day = entry.file_name().to_string_lossy().trim_end_matches(".json").to_string();
        let Ok(record) = std::fs::read(entry.path()).map_err(anyhow::Error::from).and_then(|b| Ok(serde_json::from_slice::<Value>(&b)?)) else { continue };
        let sent = record["sent"].as_str().unwrap_or("").chars().take(16).collect::<String>().replace('T', " ");
        out.insert(day, (sent, serde_json::to_string_pretty(&record["data"]).unwrap_or_default()));
    }
    out.into_iter().rev().map(|(d, (s, j))| (d, s, j)).collect()
}

/// A bug report for the public tracker. Public: the title, the description and the technical information.
/// Private (only the author sees it): the context of the reported answer, the reply address and the images.
pub struct BugReport {
    pub title: String,
    pub description: String,
    pub technical: Option<String>,
    pub private: Option<String>,
    pub images: Vec<crate::report::File>,
    /// A feature request ("💡 Request this feature", LC-289) instead of a bug.
    pub feature: bool,
}

/// Sends a bug report; returns the address of the new issue on GitHub.
pub fn send_bug_report(report: &BugReport) -> Result<String> {
    use base64::Engine;
    let total: usize = report.images.iter().map(|f| f.bytes.len()).sum();
    if total > 8 * 1024 * 1024 {
        bail!("{}", crate::i18n::tr("the images are larger than 8 MB together: remove some or send the report by e-mail"));
    }
    let images: Vec<Value> = report
        .images
        .iter()
        .map(|f| json!({ "name": f.name, "data": base64::engine::general_purpose::STANDARD.encode(&f.bytes) }))
        .collect();
    let body = json!({
        "v": 1,
        "install": install_id_any(),
        "app": env!("CARGO_PKG_VERSION"),
        "os": os_name(),
        "lang": crate::i18n::current().code(),
        "title": report.title,
        "kind": if report.feature { "feature" } else { "bug" },
        "description": report.description,
        "technical": report.technical,
        "private": report.private,
        "images": images,
    });
    let response = client()?.post(format!("{}/v1/report", service_url()?)).json(&body).send()?;
    let status = response.status();
    let answer: Value = response.json().unwrap_or(Value::Null);
    if !status.is_success() {
        bail!("{}", answer["error"].as_str().map(String::from).unwrap_or_else(|| crate::i18n::fmt("the service answered {0}", &[status.to_string()])));
    }
    let url = answer["url"].as_str().context("no address of the issue in the answer")?;
    if !url.starts_with("https://github.com/") {
        bail!("unexpected address of the issue");
    }
    event("bug_report");
    Ok(url.to_string())
}

/// The installation id also when statistics are off (a bug report always carries it, so that the author can ask
/// about it; it is shown in the report before sending).
fn install_id_any() -> String {
    install_id()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_and_merge() {
        assert_eq!(key_of("mcp__github__list_issues"), "mcp");
        assert_eq!(key_of("ea_write"), "ea_write");
        assert_eq!(key_of("Bad Name!"), "adame");
        assert_eq!(key_of("ŻÓŁW"), "other");
        let mut a = json!({ "tools": { "ea_write": 2 }, "turns": 3 }).as_object().unwrap().clone();
        let b = json!({ "tools": { "ea_write": 1, "db_query": 4 }, "turns": 1, "ai_errors": 1 }).as_object().unwrap().clone();
        add_maps(&mut a, &b);
        assert_eq!(Value::Object(a), json!({ "tools": { "ea_write": 3, "db_query": 4 }, "turns": 4, "ai_errors": 1 }));
    }

    #[test]
    fn counts_merges_and_keeps_out_names() {
        let dir = std::env::temp_dir().join(format!("loonar-telemetry-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::env::remove_var("LOONAR_TELEMETRY");
        init(&dir, true);
        tool("ea_write", true);
        tool("ea_write", false);
        tool("mcp__acme_internal__secret_tool", true);
        approval(true);
        approval(false);
        turn();
        flush();
        // A second process of the same day.
        let other = dir.join("telemetry").join("days").join(today()).join("1-1.json");
        std::fs::write(&other, json!({ "app": "0.8.2", "turns": 2, "tools": { "db_query": 5 } }).to_string()).unwrap();
        let s = day_summary(&today()).unwrap();
        assert_eq!(s["tools"], json!({ "ea_write": 2, "mcp": 1, "db_query": 5 }));
        assert_eq!(s["tool_errors"], json!({ "ea_write": 1 }));
        assert_eq!(s["approvals"], json!({ "asked": 2, "accepted": 1, "rejected": 1 }));
        assert_eq!(s["turns"], 3);
        assert!(uuid::Uuid::parse_str(s["install"].as_str().unwrap()).is_ok());
        assert!(!s.to_string().contains("acme"), "names of MCP servers are never sent");
        // Off: nothing more is counted and unsent days are deleted.
        set_enabled(false);
        tool("ea_write", true);
        assert!(day_summary(&today()).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[cfg(test)]
mod service_tests {
    use super::*;

    /// The application against the feedback Worker on this computer (node feedback/dev-server.mjs 8787):
    /// `LOONAR_FEEDBACK_URL=http://127.0.0.1:8787 cargo test feedback_end_to_end -- --ignored --nocapture`
    #[test]
    #[ignore = "requires the feedback dev server"]
    fn feedback_end_to_end() {
        crate::i18n::set(crate::i18n::Lang::En);
        let dir = std::env::temp_dir().join(format!("loonar-feedback-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        init(&dir, true);
        // A day that has ended, counted by this process and by another one.
        let day = (chrono::Local::now() - chrono::Duration::days(1)).format("%Y-%m-%d").to_string();
        let folder = dir.join("telemetry").join("days").join(&day);
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(folder.join("1-1.json"), json!({ "app": "0.9.0", "turns": 3, "tools": { "ea_write": 2, "mcp": 1 }, "setup": { "mode": "ea_tab", "model_source": "file", "ai_provider": "anthropic", "ai_model": "claude-haiku-5-5", "approval": "ask", "jira": false, "confluence": false, "hub": false, "mcp_servers": 1, "event_agents": 0 } }).to_string()).unwrap();
        std::fs::write(folder.join("2-2.json"), json!({ "app": "0.9.0", "turns": 1, "approvals": { "asked": 1, "accepted": 1 } }).to_string()).unwrap();
        send_ended_days().unwrap();
        let sent = sent_records();
        assert_eq!(sent.len(), 1, "the day is recorded as sent");
        assert!(sent[0].2.contains("\"turns\": 4"), "both processes added up: {}", sent[0].2);
        assert!(!folder.exists(), "the counters of a sent day are removed");
        // Sending again sends nothing new.
        send_ended_days().unwrap();
        assert_eq!(sent_records().len(), 1);
        // A bug report with a private part and an image.
        let png = {
            let img = image::RgbaImage::from_pixel(4, 4, image::Rgba([200, 0, 0, 255]));
            let mut out = std::io::Cursor::new(Vec::new());
            image::DynamicImage::ImageRgba8(img).write_to(&mut out, image::ImageFormat::Png).unwrap();
            out.into_inner()
        };
        let url = send_bug_report(&BugReport {
            title: "Wrong answer of the assistant".into(),
            description: "It added Invoice twice.".into(),
            technical: Some("Loonar Compass 0.9.0".into()),
            private: Some("Question:\nAdd Invoice to ACME\n\nAnswer:\nDone.".into()),
            images: vec![crate::report::File { name: "shot.png".into(), bytes: png }],
            feature: false,
        })
        .unwrap();
        assert!(url.starts_with("https://github.com/loonar-eu/loonar-compass/issues/"), "{url}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

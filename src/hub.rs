//! Compass Hub: the team service. One process (`LoonarCompass.exe --hub hub.json`) on a computer or server of the team:
//! - the team's models (data sources: EA databases and project files, read-only),
//! - a shared policy (governance rules for the model check, the minimum approval mode, allowed AI providers),
//! - a central audit trail of the changes saved by every Compass,
//! - shared notes (team decisions, conventions),
//! - a web view for people without EA (search, element details, impact, model check, gaps, audit),
//! - an MCP server (read-only) for other AI clients,
//! - users with tokens and roles: viewer (reading), architect (+ audit, notes), admin (+ policy).
//!
//! Plain HTTP on a single port without extra libraries; put it behind HTTPS (reverse proxy) outside one computer.

use crate::config::AppConfig;
use anyhow::{anyhow, bail, Context, Result};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub const DEFAULT_LISTEN: &str = "127.0.0.1:8730";
const MAX_BODY: usize = 4 * 1024 * 1024;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Role {
    Viewer,
    Architect,
    Admin,
}

impl Role {
    fn parse(text: &str) -> Role {
        match text.trim().to_ascii_lowercase().as_str() {
            "admin" => Role::Admin,
            "architect" => Role::Architect,
            _ => Role::Viewer,
        }
    }
    fn name(self) -> &'static str {
        match self {
            Role::Viewer => "viewer",
            Role::Architect => "architect",
            Role::Admin => "admin",
        }
    }
}

struct Source {
    id: String,
    name: String,
    cfg: AppConfig,
}

/// State of the running hub.
pub struct Hub {
    file: PathBuf,
    dir: PathBuf,
    sources: Vec<Source>,
    users: Vec<(String, String, Role)>,
    policy: Mutex<Value>,
    audit_lock: Mutex<()>,
}

/// Loads `hub.json`: {"listen", "sources": [{"id", "name", "properties"}], "users": [{"name", "token", "role"}], "policy": {…}}.
/// `properties` is the app.properties of a model (as in Compass), relative to hub.json or absolute.
pub fn load(file: &Path) -> Result<(Hub, String)> {
    let text = std::fs::read_to_string(file).with_context(|| crate::i18n::fmt("cannot read {0}", &[file.display().to_string()]))?;
    let value: Value = serde_json::from_str(&text).with_context(|| crate::i18n::fmt("{0} is not valid JSON", &[file.display().to_string()]))?;
    let dir = file.parent().unwrap_or(Path::new(".")).to_path_buf();
    let mut sources = Vec::new();
    for s in value["sources"].as_array().into_iter().flatten() {
        let id = s["id"].as_str().unwrap_or("").trim().to_string();
        if id.is_empty() || !id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_') {
            bail!("{}", crate::i18n::fmt("hub source id '{0}': letters, digits, '-' and '_' only", &[id]));
        }
        let props = dir.join(s["properties"].as_str().unwrap_or(""));
        let cfg = AppConfig::load(&props).with_context(|| crate::i18n::fmt("hub source {0}", &[id.clone()]))?;
        sources.push(Source { name: s["name"].as_str().unwrap_or(&id).to_string(), id, cfg });
    }
    let mut users = Vec::new();
    for u in value["users"].as_array().into_iter().flatten() {
        let token = u["token"].as_str().unwrap_or("").to_string();
        if token.len() < 16 {
            bail!("{}", crate::i18n::fmt("hub user {0}: the token must have at least 16 characters", &[u["name"].as_str().unwrap_or("?").to_string()]));
        }
        users.push((u["name"].as_str().unwrap_or("?").to_string(), token, Role::parse(u["role"].as_str().unwrap_or("viewer"))));
    }
    let listen = value["listen"].as_str().unwrap_or(DEFAULT_LISTEN).to_string();
    let hub = Hub { file: file.to_path_buf(), dir, sources, users, policy: Mutex::new(value["policy"].clone()), audit_lock: Mutex::new(()) };
    Ok((hub, listen))
}

/// Runs the hub until the process ends. `ready` gets the bound address (port 0 = any free port, for the tests).
pub fn serve(hub: Hub, listen: &str, ready: impl FnOnce(String)) -> Result<()> {
    let listener = TcpListener::bind(listen).with_context(|| crate::i18n::fmt("cannot listen on {0}", &[listen.to_string()]))?;
    ready(listener.local_addr()?.to_string());
    let hub = Arc::new(hub);
    for stream in listener.incoming() {
        let Ok(stream) = stream else { continue };
        let hub = hub.clone();
        std::thread::spawn(move || {
            let _ = stream.set_read_timeout(Some(Duration::from_secs(30)));
            if let Err(e) = handle(&hub, stream) {
                log(&hub.dir, &format!("connection error: {e:#}"));
            }
        });
    }
    Ok(())
}

fn log(dir: &Path, line: &str) {
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(dir.join("hub.log")) {
        let _ = writeln!(f, "{} {line}", chrono::Local::now().format("%Y-%m-%d %H:%M:%S"));
    }
}

struct Request {
    method: String,
    path: String,
    query: HashMap<String, String>,
    token: String,
    body: Vec<u8>,
}

fn url_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => out.push(b' '),
            b'%' if i + 2 < bytes.len() => {
                if let Ok(b) = u8::from_str_radix(&text[i + 1..i + 3], 16) {
                    out.push(b);
                    i += 2;
                } else {
                    out.push(b'%');
                }
            }
            b => out.push(b),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn read_request(stream: &TcpStream) -> Result<Request> {
    let mut reader = BufReader::new(stream);
    let mut first = String::new();
    reader.read_line(&mut first)?;
    let mut parts = first.split_whitespace();
    let method = parts.next().unwrap_or("").to_string();
    let target = parts.next().unwrap_or("/").to_string();
    let (mut length, mut token) = (0usize, String::new());
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line)? == 0 {
            break;
        }
        let line = line.trim_end();
        if line.is_empty() {
            break;
        }
        if let Some((name, value)) = line.split_once(':') {
            match name.trim().to_ascii_lowercase().as_str() {
                "content-length" => length = value.trim().parse().unwrap_or(0),
                "authorization" => token = value.trim().strip_prefix("Bearer ").unwrap_or("").trim().to_string(),
                _ => {}
            }
        }
    }
    if length > MAX_BODY {
        bail!("request body too large");
    }
    let mut body = vec![0u8; length];
    reader.read_exact(&mut body)?;
    let (path, query_text) = target.split_once('?').unwrap_or((&target, ""));
    let query = query_text
        .split('&')
        .filter(|p| !p.is_empty())
        .map(|p| {
            let (k, v) = p.split_once('=').unwrap_or((p, ""));
            (url_decode(k), url_decode(v))
        })
        .collect();
    Ok(Request { method, path: url_decode(path), query, token, body })
}

fn respond(mut stream: &TcpStream, status: u16, content_type: &str, body: &[u8]) -> Result<()> {
    let reason = match status {
        200 => "OK",
        201 => "Created",
        202 => "Accepted",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        _ => "Error",
    };
    write!(
        stream,
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\nConnection: close\r\n\r\n",
        body.len()
    )?;
    stream.write_all(body)?;
    stream.flush()?;
    Ok(())
}

/// An error with its HTTP status.
struct HttpError(u16, String);

fn deny(status: u16, message: impl Into<String>) -> HttpError {
    HttpError(status, message.into())
}

impl From<anyhow::Error> for HttpError {
    fn from(e: anyhow::Error) -> Self {
        HttpError(400, format!("{e:#}"))
    }
}

fn handle(hub: &Hub, stream: TcpStream) -> Result<()> {
    let request = read_request(&stream)?;
    if request.method == "GET" && (request.path == "/" || request.path == "/index.html") {
        return respond(&stream, 200, "text/html; charset=utf-8", WEB_PAGE.as_bytes());
    }
    let result = route(hub, &request);
    match result {
        Ok((status, value)) => respond(&stream, status, "application/json; charset=utf-8", value.to_string().as_bytes()),
        Err(HttpError(status, message)) => {
            if status != 401 {
                log(&hub.dir, &format!("{} {} -> {status} {message}", request.method, request.path));
            }
            respond(&stream, status, "application/json; charset=utf-8", json!({ "error": message }).to_string().as_bytes())
        }
    }
}

impl Hub {
    fn user(&self, token: &str) -> Option<(&str, Role)> {
        if token.is_empty() {
            return None;
        }
        // Comparing every byte (no early exit on the first difference).
        self.users.iter().find(|(_, t, _)| t.len() == token.len() && t.bytes().zip(token.bytes()).fold(0u8, |acc, (a, b)| acc | (a ^ b)) == 0).map(|(n, _, r)| (n.as_str(), *r))
    }

    fn source(&self, id: &str) -> Result<&Source, HttpError> {
        self.sources.iter().find(|s| s.id == id).ok_or_else(|| deny(404, crate::i18n::fmt("there is no source {0}", &[id.to_string()])))
    }

    fn rules(&self) -> Value {
        let policy = self.policy.lock().unwrap();
        if policy["governance"].is_object() { policy["governance"].clone() } else { json!({}) }
    }

    fn append_audit(&self, entry: &Value) -> Result<()> {
        let _guard = self.audit_lock.lock().unwrap();
        let mut f = std::fs::OpenOptions::new().create(true).append(true).open(self.dir.join("audit.jsonl"))?;
        writeln!(f, "{entry}")?;
        Ok(())
    }

    fn read_audit(&self, source: Option<&str>, limit: usize) -> Vec<Value> {
        let text = std::fs::read_to_string(self.dir.join("audit.jsonl")).unwrap_or_default();
        let mut entries: Vec<Value> = text.lines().filter_map(|l| serde_json::from_str(l).ok()).filter(|e: &Value| source.is_none_or(|s| e["source"] == s)).collect();
        entries.reverse();
        entries.truncate(limit);
        entries
    }

    fn notes_dir(&self) -> PathBuf {
        self.dir.join("notes")
    }
}

fn note_name(name: &str) -> Result<String, HttpError> {
    let ok = !name.is_empty() && name.len() <= 80 && name.chars().all(|c| c.is_alphanumeric() || matches!(c, '-' | '_' | ' ' | '.')) && !name.starts_with('.');
    if ok { Ok(name.to_string()) } else { Err(deny(400, crate::i18n::tr("a note name: letters, digits, spaces, '-', '_' and '.'"))) }
}

fn require(role: Role, needed: Role) -> Result<(), HttpError> {
    if role < needed {
        return Err(deny(403, crate::i18n::fmt("this needs the role {0}", &[needed.name().to_string()])));
    }
    Ok(())
}

fn int_param(request: &Request, name: &str) -> Result<i64, HttpError> {
    request.query.get(name).and_then(|v| v.parse().ok()).ok_or_else(|| deny(400, crate::i18n::fmt("missing number '{0}'", &[name.to_string()])))
}

fn route(hub: &Hub, request: &Request) -> Result<(u16, Value), HttpError> {
    let segments: Vec<&str> = request.path.trim_matches('/').split('/').collect();
    if request.method == "GET" && segments == ["api", "health"] {
        return Ok((200, json!({ "ok": true, "app": crate::config::APP_NAME, "version": env!("CARGO_PKG_VERSION"), "sources": hub.sources.len() })));
    }
    let (user, role) = hub.user(&request.token).ok_or_else(|| deny(401, crate::i18n::tr("missing or unknown hub token")))?;
    let user = user.to_string();
    let body = || -> Result<Value, HttpError> { serde_json::from_slice(&request.body).map_err(|e| deny(400, format!("JSON: {e}"))) };
    match (request.method.as_str(), segments.as_slice()) {
        ("GET", ["api", "me"]) => Ok((200, json!({ "name": user, "role": role.name() }))),
        ("GET", ["api", "policy"]) => Ok((200, hub.policy.lock().unwrap().clone())),
        ("PUT", ["api", "policy"]) => {
            require(role, Role::Admin)?;
            let policy = body()?;
            if !policy.is_object() {
                return Err(deny(400, crate::i18n::tr("the policy is a JSON object")));
            }
            let mut file: Value = serde_json::from_str(&std::fs::read_to_string(&hub.file).map_err(|e| deny(500, e.to_string()))?).map_err(|e| deny(500, e.to_string()))?;
            file["policy"] = policy.clone();
            std::fs::write(&hub.file, serde_json::to_string_pretty(&file).unwrap()).map_err(|e| deny(500, e.to_string()))?;
            *hub.policy.lock().unwrap() = policy;
            let _ = hub.append_audit(&json!({ "time": chrono::Local::now().to_rfc3339(), "user": user, "source": "", "summary": crate::i18n::tr("team policy changed"), "ok": true }));
            Ok((200, json!({ "ok": true })))
        }
        ("GET", ["api", "sources"]) => Ok((200, json!(hub.sources.iter().map(|s| json!({ "id": s.id, "name": s.name, "kind": if s.cfg.uses_file() { "file" } else { "database" } })).collect::<Vec<_>>()))),
        ("GET", ["api", "sources", id, rest @ ..]) => {
            let source = hub.source(id)?;
            let text = match rest {
                ["packages"] => packages(&source.cfg)?,
                ["search"] => search(&source.cfg, request.query.get("q").map(String::as_str).unwrap_or(""))?,
                ["elements", element] => element_details(&source.cfg, element.parse().map_err(|_| deny(400, "element id"))?)?,
                ["impact", element] => crate::tools::impact_analysis(&json!({ "element": element.parse::<i64>().map_err(|_| deny(400, "element id"))?, "depth": request.query.get("depth").and_then(|d| d.parse::<i64>().ok()).unwrap_or(3), "direction": request.query.get("direction").map(String::as_str).unwrap_or("both") }), &source.cfg)?,
                ["lint", package] => crate::governance::lint(&json!({ "package": package.parse::<i64>().map_err(|_| deny(400, "package id"))?, "rules": hub.rules() }), &source.cfg)?,
                ["gap"] => crate::governance::gap(&json!({ "as_is": int_param(request, "as_is")?, "to_be": int_param(request, "to_be")? }), &source.cfg)?,
                _ => return Err(deny(404, "not found")),
            };
            Ok((200, json!({ "source": source.id, "text": text })))
        }
        ("POST", ["api", "audit"]) => {
            require(role, Role::Architect)?;
            let mut entry = body()?;
            if !entry.is_object() {
                return Err(deny(400, crate::i18n::tr("an audit entry is a JSON object")));
            }
            // Who and when are set by the hub, not by the client.
            entry["user"] = json!(user);
            entry["received"] = json!(chrono::Local::now().to_rfc3339());
            hub.append_audit(&entry).map_err(|e| deny(500, e.to_string()))?;
            Ok((201, json!({ "ok": true })))
        }
        ("GET", ["api", "audit"]) => {
            let limit = request.query.get("limit").and_then(|l| l.parse().ok()).unwrap_or(100usize).min(1000);
            Ok((200, json!(hub.read_audit(request.query.get("source").map(String::as_str), limit))))
        }
        ("GET", ["api", "notes"]) => {
            let mut names: Vec<String> = std::fs::read_dir(hub.notes_dir()).into_iter().flatten().flatten().filter_map(|e| e.file_name().to_str().and_then(|n| n.strip_suffix(".md")).map(String::from)).collect();
            names.sort();
            Ok((200, json!(names)))
        }
        ("GET", ["api", "notes", name]) => {
            let name = note_name(name)?;
            let text = std::fs::read_to_string(hub.notes_dir().join(format!("{name}.md"))).map_err(|_| deny(404, crate::i18n::fmt("there is no note {0}", &[name.clone()])))?;
            Ok((200, json!({ "name": name, "text": text })))
        }
        ("PUT", ["api", "notes", name]) => {
            require(role, Role::Architect)?;
            let name = note_name(name)?;
            let text = body()?["text"].as_str().unwrap_or("").to_string();
            std::fs::create_dir_all(hub.notes_dir()).map_err(|e| deny(500, e.to_string()))?;
            std::fs::write(hub.notes_dir().join(format!("{name}.md")), text).map_err(|e| deny(500, e.to_string()))?;
            Ok((200, json!({ "ok": true })))
        }
        ("POST", ["mcp"]) => mcp(hub, &body()?).map(|reply| match reply {
            Some(reply) => (200, reply),
            None => (202, Value::Null),
        }),
        _ => Err(deny(404, "not found")),
    }
}

fn packages(cfg: &AppConfig) -> Result<String> {
    let rows = crate::tools::query_rows(cfg, "select package_id, parent_id, name from t_package order by parent_id, tpos, name")?;
    let mut out = vec!["package_id | parent_id | name".to_string()];
    out.extend(rows.into_iter().take(2000).map(|r| r.join(" | ")));
    Ok(out.join("\n"))
}

fn search(cfg: &AppConfig, q: &str) -> Result<String> {
    let q = q.trim();
    if q.chars().count() < 2 {
        bail!("{}", crate::i18n::tr("type at least 2 characters"));
    }
    let like = q.to_lowercase().replace('\'', "''").replace('%', "").replace('_', "");
    let rows = crate::tools::query_rows(cfg, &format!(
        "select o.object_id, o.name, o.object_type, coalesce(o.stereotype,''), p.name, substr(coalesce(o.note,''), 1, 120) from t_object o \
         join t_package p on p.package_id = o.package_id where lower(o.name) like '%{like}%' or lower(coalesce(o.alias,'')) like '%{like}%' order by o.name limit 100"
    ))?;
    let mut out = vec![crate::i18n::tr("object_id | name | type | stereotype | package | description").to_string()];
    out.extend(rows.into_iter().map(|r| r.iter().map(|c| c.replace(['\r', '\n'], " ")).collect::<Vec<_>>().join(" | ")));
    Ok(out.join("\n"))
}

fn element_details(cfg: &AppConfig, id: i64) -> Result<String> {
    let q = |sql: String| crate::tools::query_rows(cfg, &sql);
    let row = q(format!(
        "select o.object_id, o.name, o.object_type, coalesce(o.stereotype,''), coalesce(o.alias,''), coalesce(o.status,''), coalesce(o.author,''), p.name, coalesce(o.note,''), o.ea_guid \
         from t_object o join t_package p on p.package_id = o.package_id where o.object_id = {id}"
    ))?
    .into_iter()
    .next()
    .ok_or_else(|| anyhow!("{}", crate::i18n::fmt("there is no element with id {0}", &[id.to_string()])))?;
    let mut out = vec![
        format!("{} ({}{}) — {}", row[1], row[2], if row[3].is_empty() { String::new() } else { format!(" «{}»", row[3]) }, row[7]),
        format!("id {} | GUID {} | alias {} | status {} | author {}", row[0], row[9], row[4], row[5], row[6]),
        String::new(),
        row[8].clone(),
    ];
    let tags = q(format!("select property, coalesce(value,'') from t_objectproperties where object_id = {id} order by property"))?;
    if !tags.is_empty() {
        out.push(format!("\n{}:", crate::i18n::tr("Tagged values")));
        out.extend(tags.into_iter().map(|t| format!("- {} = {}", t[0], t[1])));
    }
    let links = q(format!(
        "select c.connector_type, case when c.start_object_id = {id} then '→' else '←' end, o.object_id, o.name from t_connector c \
         join t_object o on o.object_id = case when c.start_object_id = {id} then c.end_object_id else c.start_object_id end \
         where c.start_object_id = {id} or c.end_object_id = {id} order by 1, 4"
    ))?;
    if !links.is_empty() {
        out.push(format!("\n{}:", crate::i18n::tr("Relationships")));
        out.extend(links.into_iter().map(|l| format!("- {} {} {} (id {})", l[0], l[1], l[3], l[2])));
    }
    let diagrams = q(format!("select d.diagram_id, d.name from t_diagramobjects x join t_diagram d on d.diagram_id = x.diagram_id where x.object_id = {id} order by d.name"))?;
    if !diagrams.is_empty() {
        out.push(format!("\n{}:", crate::i18n::tr("Diagrams")));
        out.extend(diagrams.into_iter().map(|d| format!("- {} (id {})", d[1], d[0])));
    }
    Ok(out.join("\n"))
}

/// Tools of the MCP server (all read-only).
fn mcp_tools() -> Value {
    let source = json!({ "type": "string", "description": "Source id from list_sources." });
    json!([
        { "name": "list_sources", "description": "The team's EA models available in the hub.", "inputSchema": { "type": "object", "properties": {} } },
        { "name": "list_packages", "description": "Package tree of a model (package_id | parent_id | name).", "inputSchema": { "type": "object", "properties": { "source": source }, "required": ["source"] } },
        { "name": "search_elements", "description": "Finds elements by name or alias (part of the text).", "inputSchema": { "type": "object", "properties": { "source": source, "query": { "type": "string" } }, "required": ["source", "query"] } },
        { "name": "element_details", "description": "An element with its description, tagged values, relationships and diagrams.", "inputSchema": { "type": "object", "properties": { "source": source, "element": { "type": "integer" } }, "required": ["source", "element"] } },
        { "name": "impact_analysis", "description": "What depends on an element and what it affects, level by level along relationships.", "inputSchema": { "type": "object", "properties": { "source": source, "element": { "type": "integer" }, "direction": { "type": "string", "enum": ["in", "out", "both"] }, "depth": { "type": "integer" } }, "required": ["source", "element"] } },
        { "name": "model_lint", "description": "Model quality check of a package by the team rules (orphans, default names, missing descriptions, disallowed relationships…).", "inputSchema": { "type": "object", "properties": { "source": source, "package": { "type": "integer" } }, "required": ["source", "package"] } },
        { "name": "gap_analysis", "description": "Gaps between two packages (as-is vs to-be).", "inputSchema": { "type": "object", "properties": { "source": source, "as_is": { "type": "integer" }, "to_be": { "type": "integer" } }, "required": ["source", "as_is", "to_be"] } }
    ])
}

/// MCP (JSON-RPC 2.0 over HTTP POST): initialize, tools/list, tools/call, ping. A notification gets no reply.
fn mcp(hub: &Hub, message: &Value) -> Result<Option<Value>, HttpError> {
    let id = message.get("id").cloned();
    let Some(id) = id else { return Ok(None) };
    let method = message["method"].as_str().unwrap_or("");
    let params = &message["params"];
    let ok = |result: Value| Some(json!({ "jsonrpc": "2.0", "id": id, "result": result }));
    let reply = match method {
        "initialize" => ok(json!({
            "protocolVersion": params["protocolVersion"].as_str().unwrap_or("2025-06-18"),
            "capabilities": { "tools": { "listChanged": false } },
            "serverInfo": { "name": "loonar-compass-hub", "version": env!("CARGO_PKG_VERSION") },
            "instructions": "Read-only access to the team's Enterprise Architect models. Start with list_sources."
        })),
        "ping" => ok(json!({})),
        "tools/list" => ok(json!({ "tools": mcp_tools() })),
        "tools/call" => {
            let args = &params["arguments"];
            let name = params["name"].as_str().unwrap_or("");
            let result: Result<String> = (|| {
                if name == "list_sources" {
                    return Ok(hub.sources.iter().map(|s| format!("{} | {} | {}", s.id, s.name, if s.cfg.uses_file() { "file" } else { "database" })).collect::<Vec<_>>().join("\n"));
                }
                let source = hub.source(args["source"].as_str().unwrap_or("")).map_err(|e| anyhow!(e.1))?;
                let int = |field: &str| args[field].as_i64().with_context(|| crate::i18n::fmt("missing number '{0}'", &[field.to_string()]));
                match name {
                    "list_packages" => packages(&source.cfg),
                    "search_elements" => search(&source.cfg, args["query"].as_str().unwrap_or("")),
                    "element_details" => element_details(&source.cfg, int("element")?),
                    "impact_analysis" => crate::tools::impact_analysis(args, &source.cfg),
                    "model_lint" => crate::governance::lint(&json!({ "package": int("package")?, "rules": hub.rules() }), &source.cfg),
                    "gap_analysis" => crate::governance::gap(args, &source.cfg),
                    other => bail!("unknown tool {other}"),
                }
            })();
            match result {
                Ok(text) => ok(json!({ "content": [{ "type": "text", "text": text }], "isError": false })),
                Err(e) => ok(json!({ "content": [{ "type": "text", "text": format!("{e:#}") }], "isError": true })),
            }
        }
        _ => Some(json!({ "jsonrpc": "2.0", "id": id, "error": { "code": -32601, "message": format!("method not found: {method}") } })),
    };
    Ok(reply)
}

/// The last failure of sending to the hub (shown in the settings; the work goes on without the hub).
static LAST_FAILURE: Mutex<Option<String>> = Mutex::new(None);
/// The team policy read from the hub, with the time it was read (read again after 5 minutes).
static POLICY: Mutex<Option<(std::time::Instant, String, Value)>> = Mutex::new(None);

pub fn note_failure(message: &str) {
    *LAST_FAILURE.lock().unwrap() = Some(message.to_string());
}

pub fn last_failure() -> Option<String> {
    LAST_FAILURE.lock().unwrap().clone()
}

/// The team policy from the hub of the settings (None without a hub or when it does not answer).
pub fn team_policy(cfg: &AppConfig) -> Option<Value> {
    let client = Client::from_config(cfg)?;
    if let Some((when, url, policy)) = POLICY.lock().unwrap().as_ref() {
        if *url == client.url && when.elapsed() < Duration::from_secs(300) {
            return Some(policy.clone());
        }
    }
    match client.policy() {
        Ok(policy) => {
            *POLICY.lock().unwrap() = Some((std::time::Instant::now(), client.url.clone(), policy.clone()));
            Some(policy)
        }
        Err(e) => {
            note_failure(&format!("{e:#}"));
            None
        }
    }
}

/// The approval mode in force: the one from the settings, made stricter by the team policy ("min_approval").
pub fn effective_mode(cfg: &AppConfig) -> crate::config::ApprovalMode {
    let local = cfg.approval_mode;
    let team = team_policy(cfg).and_then(|p| p["min_approval"].as_str().and_then(|m| crate::config::ApprovalMode::parse(m).ok()));
    match team {
        Some(team) if team.strictness() > local.strictness() => team,
        _ => local,
    }
}

/// Client of the hub in Compass: the team policy, sending audit entries, shared notes.
pub struct Client {
    pub url: String,
    pub token: String,
}

impl Client {
    pub fn from_config(cfg: &AppConfig) -> Option<Client> {
        if cfg.hub_url.trim().is_empty() {
            return None;
        }
        let token = crate::config::secrets::get(crate::config::secrets::HUB_TOKEN).ok()?;
        Some(Client { url: cfg.hub_url.trim().trim_end_matches('/').to_string(), token })
    }

    fn call(&self, method: reqwest::Method, path: &str, body: Option<&Value>) -> Result<Value> {
        crate::jira::check_base_url(&self.url).map_err(|_| anyhow!("{}", crate::i18n::tr("The hub address must start with https:// (http:// only on this computer)")))?;
        let client = reqwest::blocking::Client::builder().timeout(Duration::from_secs(20)).build()?;
        let mut request = client.request(method, format!("{}{path}", self.url)).bearer_auth(&self.token);
        if let Some(body) = body {
            request = request.json(body);
        }
        let response = request.send().with_context(|| crate::i18n::tr("no connection to the team hub"))?;
        let status = response.status();
        let value: Value = response.json().unwrap_or(Value::Null);
        if !status.is_success() {
            bail!("{}", crate::i18n::fmt("the team hub replied {0}: {1}", &[status.to_string(), value["error"].as_str().unwrap_or("").to_string()]));
        }
        Ok(value)
    }

    /// Who the token belongs to: (name, role).
    pub fn me(&self) -> Result<(String, String)> {
        let v = self.call(reqwest::Method::GET, "/api/me", None)?;
        Ok((v["name"].as_str().unwrap_or("").to_string(), v["role"].as_str().unwrap_or("").to_string()))
    }

    pub fn policy(&self) -> Result<Value> {
        self.call(reqwest::Method::GET, "/api/policy", None)
    }

    pub fn send_audit(&self, entry: &Value) -> Result<()> {
        self.call(reqwest::Method::POST, "/api/audit", Some(entry)).map(|_| ())
    }

    pub fn notes(&self) -> Result<Vec<String>> {
        Ok(self.call(reqwest::Method::GET, "/api/notes", None)?.as_array().into_iter().flatten().filter_map(|v| v.as_str().map(String::from)).collect())
    }

    pub fn note(&self, name: &str) -> Result<String> {
        Ok(self.call(reqwest::Method::GET, &format!("/api/notes/{}", name.replace(' ', "%20")), None)?["text"].as_str().unwrap_or("").to_string())
    }

    pub fn save_note(&self, name: &str, text: &str) -> Result<()> {
        self.call(reqwest::Method::PUT, &format!("/api/notes/{}", name.replace(' ', "%20")), Some(&json!({ "text": text }))).map(|_| ())
    }
}

/// The web view: one page, the token is kept in the browser (localStorage) and sent as a Bearer header.
const WEB_PAGE: &str = include_str!("hub_web.html");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roles_and_names() {
        assert!(Role::Admin > Role::Architect && Role::Architect > Role::Viewer);
        assert_eq!(Role::parse("ADMIN"), Role::Admin);
        assert_eq!(Role::parse("whatever"), Role::Viewer);
        assert!(note_name("Team conventions").is_ok());
        assert!(note_name("../secret").is_err());
        assert!(note_name(".hidden").is_err());
        assert_eq!(url_decode("a%20b+c%C5%82"), "a b cł");
    }

    /// The hub with two data sources (a PostgreSQL database and an EA project file, from LOONAR_HUB_DB_PROPS and
    /// LOONAR_HUB_FILE_PROPS): roles, policy, audit, notes, the web page, the REST API and MCP on both sources.
    /// Run with: `cargo test hub_two_sources -- --ignored --nocapture`.
    #[test]
    #[ignore = "requires two test models (LOONAR_HUB_DB_PROPS, LOONAR_HUB_FILE_PROPS)"]
    fn hub_two_sources() {
        let dir = std::env::temp_dir().join(format!("loonar-hub-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let config = json!({
            "listen": "127.0.0.1:0",
            "sources": [
                { "id": "shop-db", "name": "Shop (PostgreSQL)", "properties": std::env::var("LOONAR_HUB_DB_PROPS").expect("LOONAR_HUB_DB_PROPS") },
                { "id": "shop-file", "name": "Shop (file)", "properties": std::env::var("LOONAR_HUB_FILE_PROPS").expect("LOONAR_HUB_FILE_PROPS") }
            ],
            "users": [
                { "name": "ania", "token": "admin-token-0123456789", "role": "admin" },
                { "name": "bartek", "token": "architect-token-0123456789", "role": "architect" },
                { "name": "celina", "token": "viewer-token-0123456789", "role": "viewer" }
            ],
            "policy": { "min_approval": "ask", "governance": { "naming": [{ "applies_to": "Class", "pattern": "?*", "message": "a class needs a name" }] } }
        });
        let file = dir.join("hub.json");
        std::fs::write(&file, serde_json::to_string_pretty(&config).unwrap()).unwrap();
        let (hub, listen) = load(&file).expect("hub.json");
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || serve(hub, &listen, |addr| tx.send(addr).unwrap()).unwrap());
        let base = format!("http://{}", rx.recv().unwrap());
        let http = reqwest::blocking::Client::new();
        let get = |path: &str, token: &str| {
            let r = http.get(format!("{base}{path}")).bearer_auth(token).send().unwrap();
            (r.status().as_u16(), r.text().unwrap())
        };
        let (admin, architect, viewer) = ("admin-token-0123456789", "architect-token-0123456789", "viewer-token-0123456789");

        assert_eq!(get("/api/health", "").0, 200);
        assert_eq!(get("/api/sources", "").0, 401, "no token");
        assert_eq!(get("/api/sources", "wrong-token-0123456789").0, 401, "unknown token");
        let (status, page) = get("/", "");
        assert!(status == 200 && page.contains("<title>"), "web page");
        let sources: Value = serde_json::from_str(&get("/api/sources", viewer).1).unwrap();
        assert_eq!(sources.as_array().unwrap().len(), 2);
        assert_eq!(sources[0]["kind"], "database");
        assert_eq!(sources[1]["kind"], "file");

        let started = std::time::Instant::now();
        for source in ["shop-db", "shop-file"] {
            let packages: Value = serde_json::from_str(&get(&format!("/api/sources/{source}/packages"), viewer).1).unwrap();
            let text = packages["text"].as_str().unwrap();
            assert!(text.lines().count() > 1, "{source}: packages");
            let root = text.lines().nth(1).unwrap().split(" | ").next().unwrap().to_string();
            // Any element of the model for search, details and impact.
            let any = crate::tools::query_rows(&AppConfig::load(Path::new(&config["sources"][if source == "shop-db" { 0 } else { 1 }]["properties"].as_str().unwrap())).unwrap(),
                "select object_id, name from t_object where object_type <> 'Package' and length(name) > 3 order by object_id limit 1").unwrap();
            let (element, name) = (any[0][0].clone(), any[0][1].clone());
            let part: String = name.chars().take(4).collect();
            let found: Value = serde_json::from_str(&get(&format!("/api/sources/{source}/search?q={}", part.replace(' ', "%20")), viewer).1).unwrap();
            assert!(found["text"].as_str().unwrap().contains(&name), "{source}: search {part}");
            let details: Value = serde_json::from_str(&get(&format!("/api/sources/{source}/elements/{element}"), viewer).1).unwrap();
            assert!(details["text"].as_str().unwrap().starts_with(&name), "{source}: details");
            assert_eq!(get(&format!("/api/sources/{source}/impact/{element}?depth=2"), viewer).0, 200, "{source}: impact");
            let lint: Value = serde_json::from_str(&get(&format!("/api/sources/{source}/lint/{root}"), viewer).1).unwrap();
            assert!(lint["text"].as_str().unwrap().contains("rule sets from governance.json: 1"), "{source}: the team rules are used: {}", lint["text"]);
            assert_eq!(get(&format!("/api/sources/{source}/gap?as_is={root}&to_be={root}"), viewer).0, 200, "{source}: gap");
            println!("{source}: root {root}, element {element} {name}, lint: {}", lint["text"].as_str().unwrap().lines().next().unwrap());
        }
        println!("REST on both sources: {} ms", started.elapsed().as_millis());
        assert_eq!(get("/api/sources/nope/packages", viewer).0, 404);
        assert_eq!(get("/api/sources/shop-db/search?q=x", viewer).0, 400, "too short");
        let (_, injection) = get("/api/sources/shop-db/search?q=%27%3B%20drop%20table%20t_object%3B--", viewer);
        assert!(!injection.contains("error") || injection.contains("text"), "a quote in the search is escaped: {injection}");

        // Audit: architects send, viewers read; the hub sets the user and the time.
        let post = |path: &str, token: &str, body: Value| http.post(format!("{base}{path}")).bearer_auth(token).json(&body).send().unwrap().status().as_u16();
        assert_eq!(post("/api/audit", viewer, json!({ "summary": "x" })), 403);
        assert_eq!(post("/api/audit", architect, json!({ "source": "shop-db", "summary": "new elements: 2", "user": "forged", "ok": true })), 201);
        let audit: Value = serde_json::from_str(&get("/api/audit?source=shop-db", viewer).1).unwrap();
        assert_eq!(audit[0]["user"], "bartek", "the user comes from the token");

        // Policy: everybody reads, only the admin changes it.
        let put = |path: &str, token: &str, body: Value| http.put(format!("{base}{path}")).bearer_auth(token).json(&body).send().unwrap().status().as_u16();
        assert_eq!(put("/api/policy", architect, json!({})), 403);
        assert_eq!(put("/api/policy", admin, json!({ "min_approval": "plan", "governance": {} })), 200);
        let policy: Value = serde_json::from_str(&get("/api/policy", viewer).1).unwrap();
        assert_eq!(policy["min_approval"], "plan");
        let saved: Value = serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
        assert_eq!(saved["policy"]["min_approval"], "plan", "the policy is saved in hub.json");

        // Shared notes through the Compass client.
        let client = Client { url: base.clone(), token: architect.into() };
        assert_eq!(client.me().unwrap(), ("bartek".to_string(), "architect".to_string()));
        client.save_note("Team conventions", "Services end with Service.").unwrap();
        assert_eq!(client.notes().unwrap(), vec!["Team conventions".to_string()]);
        assert_eq!(client.note("Team conventions").unwrap(), "Services end with Service.");
        assert!(Client { url: base.clone(), token: viewer.into() }.save_note("x", "y").unwrap_err().to_string().contains("403"));
        assert!(matches!(get("/api/notes/..%2Fhub", viewer).0, 400 | 404), "no path traversal");
        assert_eq!(get("/api/notes/..hub", viewer).0, 400, "a name starting with a dot");

        // MCP: initialize, tools/list, tools/call on both sources, a notification, an unknown method.
        let rpc = |body: Value, token: &str| {
            let r = http.post(format!("{base}/mcp")).bearer_auth(token).json(&body).send().unwrap();
            (r.status().as_u16(), r.text().unwrap())
        };
        assert_eq!(rpc(json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {} }), "").0, 401);
        let (_, init) = rpc(json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": { "protocolVersion": "2025-06-18" } }), viewer);
        assert!(init.contains("loonar-compass-hub"), "{init}");
        assert_eq!(rpc(json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }), viewer).0, 202);
        let (_, tools) = rpc(json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" }), viewer);
        assert_eq!(serde_json::from_str::<Value>(&tools).unwrap()["result"]["tools"].as_array().unwrap().len(), 7);
        let (_, listed) = rpc(json!({ "jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": { "name": "list_sources", "arguments": {} } }), viewer);
        assert!(listed.contains("shop-db") && listed.contains("shop-file"), "{listed}");
        for source in ["shop-db", "shop-file"] {
            let (_, called) = rpc(json!({ "jsonrpc": "2.0", "id": 4, "method": "tools/call", "params": { "name": "list_packages", "arguments": { "source": source } } }), viewer);
            let v: Value = serde_json::from_str(&called).unwrap();
            assert_eq!(v["result"]["isError"], false, "{called}");
        }
        let (_, failed) = rpc(json!({ "jsonrpc": "2.0", "id": 5, "method": "tools/call", "params": { "name": "element_details", "arguments": { "source": "shop-db" } } }), viewer);
        assert_eq!(serde_json::from_str::<Value>(&failed).unwrap()["result"]["isError"], true);
        let (_, unknown) = rpc(json!({ "jsonrpc": "2.0", "id": 6, "method": "resources/list" }), viewer);
        assert!(unknown.contains("-32601"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}

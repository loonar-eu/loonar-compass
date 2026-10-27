//! Compass Hub: the team service. One process (`LoonarCompass.exe --hub hub.json`) on a computer or server of the team:
//! - the team's models (data sources: EA databases and project files, read-only),
//! - a shared policy (governance rules for the model check, the minimum approval mode, allowed AI providers),
//! - a central audit trail of the changes saved by every Compass,
//! - shared notes (team decisions, conventions),
//! - a web view for people without EA (search, element details, impact, model check, gaps, audit),
//! - an MCP server (read-only) for other AI clients,
//! - users with roles: viewer (reading), architect (+ audit, notes, change sets), admin (+ policy, users); people sign in
//!   to the web view with a name and a password, Compass and MCP clients use a token. A hub without users gets the
//!   default account admin / passw0rd, whose password must be changed at the first sign-in.
//!
//! Plain HTTP on a single port without extra libraries; put it behind HTTPS (reverse proxy) outside one computer.
//!
//! Hardening: requests are limited in size, header count and time, and authenticated before the body is read; at most
//! MAX_CONNECTIONS connections at a time; repeated wrong tokens from one address are refused for a while; tokens are kept
//! as SHA-256 hashes; internal errors reach the client only as an id (details in hub.log); the web page is served with
//! a strict Content-Security-Policy and may not be framed.

use crate::config::AppConfig;
use anyhow::{anyhow, bail, Context, Result};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{IpAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub const DEFAULT_LISTEN: &str = "127.0.0.1:8730";
const MAX_BODY: usize = 4 * 1024 * 1024;
/// A request to the AI gateway may carry images and documents of the conversation.
const MAX_AI_BODY: usize = 32 * 1024 * 1024;
/// Request line and headers together.
const MAX_HEAD: usize = 16 * 1024;
const MAX_HEADERS: usize = 100;
/// Connections served at the same time; more get 503.
const MAX_CONNECTIONS: usize = 64;
/// The whole request (head and body) must arrive within this time.
const REQUEST_DEADLINE: Duration = Duration::from_secs(30);
/// Wrong tokens from one address within FAILURE_WINDOW before it is refused (429).
const MAX_FAILURES: usize = 20;
const FAILURE_WINDOW: Duration = Duration::from_secs(600);
const MAX_NOTE: usize = 256 * 1024;
const MAX_NOTES: usize = 500;
/// An audit entry sent by a client (after its fields are cut to size).
const MAX_AUDIT_TEXT: usize = 4 * 1024;
/// audit.jsonl and hub.log are renamed with a date when they get bigger than this.
const ROTATE_BYTES: u64 = 20 * 1024 * 1024;
/// Questions (Ask) per user and hour unless the policy says otherwise ("ask_per_hour").
const ASK_PER_HOUR: usize = 20;
/// The longest answer of the assistant in the web view.
const ASK_TIMEOUT: Duration = Duration::from_secs(180);

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
    users: Mutex<Vec<User>>,
    /// Signed-in sessions of the web view: SHA-256 of the session token → user and end of validity.
    sessions: Mutex<HashMap<[u8; 32], (String, Instant)>>,
    /// Unknown tokens already counted for an address: a client repeating one stale token counts once.
    failed_tokens: Mutex<HashMap<[u8; 32], Instant>>,
    policy: Mutex<Value>,
    audit_lock: Mutex<()>,
    ask_lock: Mutex<()>,
    /// Load → change → save of change sets one at a time.
    change_lock: Mutex<()>,
    /// The team's Jira and Confluence ("integrations" in hub.json); tokens from the hub's secrets.
    integrations: Value,
    /// The AI gateway ("ai" in hub.json): which key each person, role or everybody uses.
    ai: Value,
    /// Wrong tokens per address.
    failures: Mutex<HashMap<IpAddr, Vec<Instant>>>,
    /// Questions per user.
    asks: Mutex<HashMap<String, Vec<Instant>>>,
    /// Sign-in with a company account (OIDC), if "oidc" is in hub.json.
    oidc: Option<crate::oidc::Provider>,
    /// People signed in with a company account: name → role from their groups at the last sign-in.
    external: Mutex<HashMap<String, Role>>,
}

/// A user of the hub.
#[derive(Clone, Debug)]
struct User {
    name: String,
    role: Role,
    /// SHA-256 of the API token (Compass, MCP clients).
    token: Option<[u8; 32]>,
    /// "pbkdf2-sha256$<iterations>$<salt hex>$<hash hex>" (sign-in to the web view).
    password: Option<String>,
    /// The password was set by someone else (the default account, a reset): it must be changed before anything else.
    must_change: bool,
}

const DEFAULT_ADMIN: &str = "admin";
const DEFAULT_PASSWORD: &str = "passw0rd";
const PASSWORD_ITERATIONS: u32 = 210_000;
const SESSION_HOURS: u64 = 12;
const MIN_PASSWORD: usize = 10;

/// PBKDF2-SHA256 with a random salt.
fn password_hash(password: &str) -> String {
    use ring::rand::SecureRandom;
    let mut salt = [0u8; 16];
    let _ = ring::rand::SystemRandom::new().fill(&mut salt);
    let mut out = [0u8; 32];
    let iterations = std::num::NonZeroU32::new(PASSWORD_ITERATIONS).unwrap_or(std::num::NonZeroU32::MIN);
    ring::pbkdf2::derive(ring::pbkdf2::PBKDF2_HMAC_SHA256, iterations, &salt, password.as_bytes(), &mut out);
    format!("pbkdf2-sha256${PASSWORD_ITERATIONS}${}${}", hex(&salt), hex(&out))
}

fn unhex(text: &str) -> Option<Vec<u8>> {
    (text.len() % 2 == 0 && text.bytes().all(|b| b.is_ascii_hexdigit())).then(|| (0..text.len()).step_by(2).filter_map(|i| u8::from_str_radix(&text[i..i + 2], 16).ok()).collect())
}

fn password_ok(stored: &str, password: &str) -> bool {
    let parts: Vec<&str> = stored.split('$').collect();
    let [kind, iterations, salt, hash] = parts.as_slice() else { return false };
    let (Some(iterations), Some(salt), Some(hash)) = (iterations.parse::<u32>().ok().and_then(std::num::NonZeroU32::new), unhex(salt), unhex(hash)) else { return false };
    *kind == "pbkdf2-sha256" && ring::pbkdf2::verify(ring::pbkdf2::PBKDF2_HMAC_SHA256, iterations, &salt, password.as_bytes(), &hash).is_ok()
}

/// A password someone chooses: long enough, not the default one, not the name.
fn check_new_password(name: &str, password: &str) -> Result<(), HttpError> {
    if password.chars().count() < MIN_PASSWORD || password == DEFAULT_PASSWORD || password.eq_ignore_ascii_case(name) {
        return Err(deny(400, crate::i18n::fmt("the password needs at least {0} characters and may not be the default password or the user name", &[MIN_PASSWORD.to_string()])));
    }
    Ok(())
}

/// SHA-256 of a token (hub.json may keep only the hash: "token_sha256").
pub fn token_hash(token: &str) -> [u8; 32] {
    Sha256::digest(token.as_bytes()).into()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// A new random token and its SHA-256 for hub.json (`LoonarCompass.exe --hub-token`).
pub fn new_token() -> (String, String) {
    let token = format!("lch_{}{}", uuid::Uuid::new_v4().simple(), uuid::Uuid::new_v4().simple());
    let hash = hex(&token_hash(&token));
    (token, hash)
}

/// Loads `hub.json`: {"listen", "sources": [{"id", "name", "properties"}], "users": [{"name", "token" or "token_sha256", "role"}], "policy": {…}}.
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
    let mut users: Vec<User> = Vec::new();
    for u in value["users"].as_array().into_iter().flatten() {
        let name = u["name"].as_str().unwrap_or("").trim().to_string();
        if name.is_empty() {
            bail!("{}", crate::i18n::tr("a hub user needs a name"));
        }
        let token: Option<[u8; 32]> = match (u["token"].as_str(), u["token_sha256"].as_str()) {
            (_, Some(h)) => Some(
                unhex(&h.trim().to_ascii_lowercase()).and_then(|b| b.try_into().ok())
                    .ok_or_else(|| anyhow!("{}", crate::i18n::fmt("hub user {0}: token_sha256 must be 64 hexadecimal characters", &[name.clone()])))?,
            ),
            (Some(token), None) if token.len() >= 16 => Some(token_hash(token)),
            (Some(_), None) => bail!("{}", crate::i18n::fmt("hub user {0}: the token must have at least 16 characters", &[name.clone()])),
            (None, None) => None,
        };
        let password = u["password_pbkdf2"].as_str().map(String::from);
        if token.is_none() && password.is_none() {
            bail!("{}", crate::i18n::fmt("hub user {0}: give a token or a password", &[name.clone()]));
        }
        if users.iter().any(|x| x.name.eq_ignore_ascii_case(&name) || (token.is_some() && x.token == token)) {
            bail!("{}", crate::i18n::fmt("hub user {0}: the name or the token is used twice", &[name]));
        }
        users.push(User { name, role: Role::parse(u["role"].as_str().unwrap_or("viewer")), token, password, must_change: u["must_change"] == true });
    }
    // A new hub: the default account (its password must be changed at the first sign-in).
    let default_account = users.is_empty();
    if default_account {
        users.push(User { name: DEFAULT_ADMIN.into(), role: Role::Admin, token: None, password: Some(password_hash(DEFAULT_PASSWORD)), must_change: true });
    }
    let listen = value["listen"].as_str().unwrap_or(DEFAULT_LISTEN).to_string();
    let hub = Hub {
        file: file.to_path_buf(), dir, sources, users: Mutex::new(users), sessions: Mutex::new(HashMap::new()), policy: Mutex::new(value["policy"].clone()),
        audit_lock: Mutex::new(()), ask_lock: Mutex::new(()), change_lock: Mutex::new(()), failures: Mutex::new(HashMap::new()), asks: Mutex::new(HashMap::new()),
        integrations: value["integrations"].clone(),
        ai: value["ai"].clone(),
        oidc: match crate::oidc::Config::from_json(&value["oidc"])? {
            // A provider without a client secret (a public client with PKCE) works too.
            Some(config) => Some(crate::oidc::Provider::new(config, crate::config::secrets::get(crate::config::secrets::OIDC_CLIENT_SECRET).unwrap_or_default())),
            None => None,
        },
        external: Mutex::new(HashMap::new()),
        failed_tokens: Mutex::new(HashMap::new()),
    };
    hub.load_sessions();
    if default_account {
        hub.save_users()?;
        log(&hub.dir, &format!("no users in hub.json: the default account {DEFAULT_ADMIN} / {DEFAULT_PASSWORD} was created; change its password at the first sign-in"));
        eprintln!("Compass Hub: no users in hub.json – default account {DEFAULT_ADMIN} / {DEFAULT_PASSWORD} created; its password must be changed at the first sign-in.");
    }
    Ok((hub, listen))
}

/// Runs the hub until the process ends. `ready` gets the bound address (port 0 = any free port, for the tests).
pub fn serve(hub: Hub, listen: &str, ready: impl FnOnce(String)) -> Result<()> {
    let listener = TcpListener::bind(listen).with_context(|| crate::i18n::fmt("cannot listen on {0}", &[listen.to_string()]))?;
    ready(listener.local_addr()?.to_string());
    let hub = Arc::new(hub);
    let active = Arc::new(AtomicUsize::new(0));
    for stream in listener.incoming() {
        let Ok(stream) = stream else { continue };
        let _ = stream.set_read_timeout(Some(Duration::from_secs(10)));
        let _ = stream.set_write_timeout(Some(Duration::from_secs(30)));
        if active.load(Ordering::SeqCst) >= MAX_CONNECTIONS {
            let _ = respond(&stream, 503, "application/json; charset=utf-8", br#"{"error":"busy"}"#);
            continue;
        }
        active.fetch_add(1, Ordering::SeqCst);
        let (hub, counter) = (hub.clone(), active.clone());
        let spawned = std::thread::Builder::new().spawn(move || {
            struct Slot(Arc<AtomicUsize>);
            impl Drop for Slot {
                fn drop(&mut self) {
                    self.0.fetch_sub(1, Ordering::SeqCst);
                }
            }
            let _slot = Slot(counter);
            if let Err(e) = handle(&hub, stream) {
                log(&hub.dir, &format!("connection error: {e:#}"));
            }
        });
        if spawned.is_err() {
            active.fetch_sub(1, Ordering::SeqCst);
        }
    }
    Ok(())
}

/// Renames a file with the date when it gets too big (audit.jsonl, hub.log).
fn rotate(path: &Path) {
    if std::fs::metadata(path).map(|m| m.len() > ROTATE_BYTES).unwrap_or(false) {
        let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("file");
        let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("txt");
        let _ = std::fs::rename(path, path.with_file_name(format!("{stem}-{}.{ext}", chrono::Local::now().format("%Y%m%d-%H%M%S"))));
    }
}

fn log(dir: &Path, line: &str) {
    let path = dir.join("hub.log");
    rotate(&path);
    // Control characters from requests are escaped: a request cannot write lines of its own into the log.
    let line: String = line.chars().take(2000).flat_map(|c| if c.is_control() { c.escape_default().collect::<Vec<_>>() } else { vec![c] }).collect();
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
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

/// Percent-decoding on bytes (a `%` before a multi-byte character is kept as it is).
fn url_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let hex_value = |b: u8| (b as char).to_digit(16).map(|d| d as u8);
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => out.push(b' '),
            b'%' if i + 2 < bytes.len() => match (hex_value(bytes[i + 1]), hex_value(bytes[i + 2])) {
                (Some(h), Some(l)) => {
                    out.push(h * 16 + l);
                    i += 2;
                }
                _ => out.push(b'%'),
            },
            b => out.push(b),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// The request line and the headers (at most MAX_HEAD bytes, MAX_HEADERS lines, within the deadline).
struct Head {
    method: String,
    target: String,
    length: usize,
    token: String,
    /// The last address of X-Forwarded-For (the one the proxy in front of the hub added).
    forwarded: Option<IpAddr>,
    content_type: String,
}

fn read_head(reader: &mut BufReader<&TcpStream>, started: Instant) -> Result<Head, HttpError> {
    let mut used = 0usize;
    // Byte by byte from the buffer, with the deadline checked before every read of the socket (a line sent slowly
    // does not keep the connection).
    let mut next_line = |reader: &mut BufReader<&TcpStream>| -> Result<String, HttpError> {
        let mut raw = Vec::new();
        loop {
            let left = REQUEST_DEADLINE.checked_sub(started.elapsed()).filter(|d| !d.is_zero()).ok_or_else(|| deny(408, "request timeout"))?;
            let _ = reader.get_ref().set_read_timeout(Some(left.min(Duration::from_secs(10))));
            let available = reader.fill_buf().map_err(|e| deny(400, e.to_string()))?;
            if available.is_empty() {
                break;
            }
            let (take, done) = match available.iter().position(|b| *b == b'\n') {
                Some(i) => (i + 1, true),
                None => (available.len(), false),
            };
            raw.extend_from_slice(&available[..take]);
            reader.consume(take);
            used += take;
            if used > MAX_HEAD {
                return Err(deny(431, "request header too large"));
            }
            if done {
                break;
            }
        }
        Ok(String::from_utf8_lossy(&raw).trim_end().to_string())
    };
    let first = next_line(reader)?;
    let mut parts = first.split_whitespace();
    let method = parts.next().unwrap_or("").to_string();
    let target = parts.next().unwrap_or("/").to_string();
    let (mut length, mut token, mut forwarded, mut content_type) = (0usize, String::new(), None, String::new());
    for count in 0.. {
        if count > MAX_HEADERS {
            return Err(deny(431, "too many headers"));
        }
        let line = next_line(reader)?;
        if line.is_empty() {
            break;
        }
        if let Some((name, value)) = line.split_once(':') {
            match name.trim().to_ascii_lowercase().as_str() {
                "content-length" => length = value.trim().parse().map_err(|_| deny(400, "Content-Length"))?,
                "transfer-encoding" => return Err(deny(501, "Transfer-Encoding is not supported")),
                "authorization" => token = value.trim().strip_prefix("Bearer ").unwrap_or("").trim().to_string(),
                // Compass talking to the hub's AI gateway sends the hub token as an Anthropic key.
                "x-api-key" if token.is_empty() => token = value.trim().to_string(),
                "content-type" => content_type = value.trim().to_string(),
                "x-forwarded-for" => forwarded = value.split(',').next_back().and_then(|a| forwarded_address(a.trim())),
                _ => {}
            }
        }
    }
    if length > MAX_AI_BODY || (length > MAX_BODY && !target.starts_with("/ai/")) {
        return Err(deny(413, "request body too large"));
    }
    Ok(Head { method, target, length, token, forwarded, content_type })
}

/// An address of X-Forwarded-For: "10.1.2.3", "2001:db8::1", or with a port as IIS/ARR sends it by default
/// ("10.1.2.3:51234", "[2001:db8::1]:51234").
fn forwarded_address(text: &str) -> Option<IpAddr> {
    text.parse::<IpAddr>().ok().or_else(|| text.parse::<std::net::SocketAddr>().ok().map(|a| a.ip()))
}

fn read_body(reader: &mut BufReader<&TcpStream>, length: usize, started: Instant) -> Result<Vec<u8>, HttpError> {
    let mut body = Vec::with_capacity(length.min(64 * 1024));
    let mut chunk = [0u8; 16 * 1024];
    while body.len() < length {
        if started.elapsed() > REQUEST_DEADLINE {
            return Err(deny(408, "request timeout"));
        }
        let want = chunk.len().min(length - body.len());
        let left = REQUEST_DEADLINE.checked_sub(started.elapsed()).filter(|d| !d.is_zero()).ok_or_else(|| deny(408, "request timeout"))?;
        let _ = reader.get_ref().set_read_timeout(Some(left.min(Duration::from_secs(10))));
        let n = reader.read(&mut chunk[..want]).map_err(|e| deny(400, e.to_string()))?;
        if n == 0 {
            return Err(deny(400, "request body incomplete"));
        }
        body.extend_from_slice(&chunk[..n]);
    }
    Ok(body)
}

fn split_target(target: &str) -> (String, HashMap<String, String>) {
    let (path, query_text) = target.split_once('?').unwrap_or((target, ""));
    let query = query_text
        .split('&')
        .filter(|p| !p.is_empty())
        .map(|p| {
            let (k, v) = p.split_once('=').unwrap_or((p, ""));
            (url_decode(k), url_decode(v))
        })
        .collect();
    (url_decode(path), query)
}

fn respond(mut stream: &TcpStream, status: u16, content_type: &str, body: &[u8]) -> Result<()> {
    let reason = match status {
        200 => "OK",
        201 => "Created",
        204 => "No Content",
        202 => "Accepted",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        408 => "Request Timeout",
        413 => "Payload Too Large",
        429 => "Too Many Requests",
        431 => "Request Header Fields Too Large",
        501 => "Not Implemented",
        503 => "Service Unavailable",
        _ => "Error",
    };
    write!(
        stream,
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\n\
         Content-Security-Policy: default-src 'none'; script-src 'self'; style-src 'unsafe-inline'; connect-src 'self'; img-src 'self'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'\r\n\
         X-Frame-Options: DENY\r\nReferrer-Policy: no-referrer\r\nConnection: close\r\n\r\n",
        body.len()
    )?;
    stream.write_all(body)?;
    stream.flush()?;
    Ok(())
}

/// A file to download (a report).
fn respond_file(mut stream: &TcpStream, content_type: &str, name: &str, body: &[u8]) -> Result<()> {
    let safe: String = name.chars().map(|c| if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_') { c } else { '_' }).collect();
    write!(
        stream,
        "HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nContent-Disposition: attachment; filename=\"{safe}\"\r\nCache-Control: no-store\r\n\
         X-Content-Type-Options: nosniff\r\nContent-Security-Policy: default-src 'none'\r\nConnection: close\r\n\r\n",
        body.len()
    )?;
    stream.write_all(body)?;
    stream.flush()?;
    Ok(())
}

/// The return address of a sign-in started by Compass: http on this computer's loopback, the fixed path.
fn compass_return(url: &str) -> bool {
    reqwest::Url::parse(url).is_ok_and(|u| u.scheme() == "http" && matches!(u.host_str(), Some("127.0.0.1") | Some("localhost")) && u.path() == "/compass-callback" && u.query().is_none())
}

/// A redirect (the company sign-in): to the provider, or back to the page.
fn redirect(mut stream: &TcpStream, location: &str) -> Result<()> {
    write!(
        stream,
        "HTTP/1.1 302 Found\r\nLocation: {location}\r\nContent-Length: 0\r\nCache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\n\
         X-Frame-Options: DENY\r\nReferrer-Policy: no-referrer\r\nConnection: close\r\n\r\n"
    )?;
    stream.flush()?;
    Ok(())
}

/// A small page when the company sign-in did not work, with the way back.
fn sign_in_problem(stream: &TcpStream, message: &str) -> Result<()> {
    let esc = message.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;");
    let page = format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><title>Compass Hub</title></head><body style=\"font-family:Segoe UI,sans-serif;max-width:560px;margin:60px auto;color:#1b2a3a\">\
         <h1 style=\"font-size:22px\">{}</h1><p>{esc}</p><p><a href=\"/\">{}</a></p></body></html>",
        crate::i18n::tr("Sign-in did not work"),
        crate::i18n::tr("Back to the sign-in page")
    );
    respond(stream, 403, "text/html; charset=utf-8", page.as_bytes())
}

/// An error with its HTTP status.
struct HttpError(u16, String);

fn deny(status: u16, message: impl Into<String>) -> HttpError {
    HttpError(status, message.into())
}

/// Internal errors (reading a model, files of the hub): the client gets only an id, the details go to hub.log.
impl From<anyhow::Error> for HttpError {
    fn from(e: anyhow::Error) -> Self {
        HttpError(500, format!("{e:#}"))
    }
}

/// The page and its script (served separately, so the Content-Security-Policy allows only scripts of the hub).
fn web_page() -> (String, String) {
    let start = WEB_PAGE.find("<script>").unwrap_or(WEB_PAGE.len());
    let end = WEB_PAGE.rfind("</script>").unwrap_or(WEB_PAGE.len());
    let script = WEB_PAGE.get(start + "<script>".len()..end).unwrap_or("").to_string();
    let page = format!("{}<script src=\"app.js\"></script>{}", &WEB_PAGE[..start], WEB_PAGE.get(end + "</script>".len()..).unwrap_or(""));
    (page, script)
}

fn handle(hub: &Hub, stream: TcpStream) -> Result<()> {
    let started = Instant::now();
    let peer = stream.peer_addr().map(|a| a.ip()).ok();
    let mut reader = BufReader::new(&stream);
    let head = match read_head(&mut reader, started) {
        Ok(head) => head,
        Err(HttpError(status, message)) => return respond(&stream, status, "application/json; charset=utf-8", json!({ "error": message }).to_string().as_bytes()),
    };
    // Behind a proxy on this computer (IIS/ARR, nginx) every request comes from the loopback address: the client is
    // the address the proxy added to X-Forwarded-For. From any other address the header is not trusted.
    let peer = match (peer, head.forwarded) {
        (Some(ip), Some(client)) if ip.is_loopback() => Some(client),
        (peer, _) => peer,
    };
    let (path, query) = split_target(&head.target);
    if head.method == "GET" {
        match path.as_str() {
            "/" | "/index.html" => return respond(&stream, 200, "text/html; charset=utf-8", web_page().0.as_bytes()),
            "/app.js" => return respond(&stream, 200, "text/javascript; charset=utf-8", web_page().1.as_bytes()),
            "/api/health" => return respond(&stream, 200, "application/json; charset=utf-8", json!({ "ok": true, "app": crate::config::APP_NAME, "version": env!("CARGO_PKG_VERSION") }).to_string().as_bytes()),
            "/favicon.ico" => return respond(&stream, 204, "text/plain", b""),
            "/api/auth" => {
                let info = match &hub.oidc {
                    Some(p) => json!({ "oidc": true, "button": p.config.button, "password_login": p.config.password_login }),
                    None => json!({ "oidc": false, "password_login": true }),
                };
                return respond(&stream, 200, "application/json; charset=utf-8", info.to_string().as_bytes());
            }
            "/auth/login" => {
                let Some(provider) = &hub.oidc else { return respond(&stream, 404, "text/plain", b"not found") };
                // Compass signs in through the browser and gets the session back on its own computer only.
                let return_to = match query.get("return") {
                    Some(r) if compass_return(r) => Some(r.clone()),
                    Some(_) => return sign_in_problem(&stream, &crate::i18n::tr("The return address of the sign-in is not allowed.")),
                    None => None,
                };
                return match provider.start(return_to) {
                    Ok(url) => redirect(&stream, &url),
                    Err(e) => {
                        log(&hub.dir, &format!("company sign-in could not start: {e:#}"));
                        sign_in_problem(&stream, &crate::i18n::tr("The company sign-in is not available right now. Try again later or ask the administrator of the hub."))
                    }
                };
            }
            "/auth/callback" => {
                let Some(provider) = &hub.oidc else { return respond(&stream, 404, "text/plain", b"not found") };
                if let Some(error) = query.get("error") {
                    let detail = query.get("error_description").cloned().unwrap_or_default();
                    log(&hub.dir, &format!("company sign-in refused by the provider: {error} {detail}"));
                    hub.audit_sign_in("", &crate::i18n::fmt("company sign-in refused by the provider: {0}", &[format!("{error} {detail}").trim().to_string()]), peer);
                    return sign_in_problem(&stream, &crate::i18n::fmt("The sign-in was refused: {0}", &[if detail.is_empty() { error.clone() } else { detail }]));
                }
                let (code, state) = (query.get("code").cloned().unwrap_or_default(), query.get("state").cloned().unwrap_or_default());
                return match provider.finish(&code, &state) {
                    Ok(who) => {
                        if hub.users.lock().unwrap_or_else(|p| p.into_inner()).iter().any(|u| u.name.eq_ignore_ascii_case(&who.name)) {
                            log(&hub.dir, &format!("company sign-in of {} refused: a local user has the same name", who.name));
                            hub.audit_sign_in(&who.name, &crate::i18n::tr("company sign-in refused: a local user has the same name"), peer);
                            return sign_in_problem(&stream, &crate::i18n::tr("A local user of the hub has the same name: the administrator has to rename or remove it first."));
                        }
                        let role = Role::parse(&who.role);
                        hub.external.lock().unwrap_or_else(|p| p.into_inner()).insert(who.name.clone(), role);
                        hub.remember_company_account(&who.name, role);
                        let session = format!("lcs_{}{}", uuid::Uuid::new_v4().simple(), uuid::Uuid::new_v4().simple());
                        hub.sessions.lock().unwrap_or_else(|p| p.into_inner()).insert(token_hash(&session), (who.name.clone(), Instant::now() + Duration::from_secs(SESSION_HOURS * 3600)));
                        hub.save_sessions();
                        let place = if who.return_to.is_some() { " (Compass)" } else { "" };
                        log(&hub.dir, &format!("{} signed in with a company account as {}{place}", who.name, role.name()));
                        let _ = hub.append_audit(&json!({ "time": chrono::Local::now().to_rfc3339(), "user": who.name, "source": "", "summary": crate::i18n::fmt("signed in with a company account ({0})", &[format!("{}{place}", role.name())]), "ok": true }));
                        // Compass: the session goes to its own address on the user's computer (never through a proxy).
                        if let Some(back) = &who.return_to {
                            return redirect(&stream, &format!("{back}?session={session}&name={}&role={}", encode(&who.name), role.name()));
                        }
                        // The session goes to the page in the fragment: it never reaches a server log or a proxy.
                        redirect(&stream, &format!("/#session={session}"))
                    }
                    Err(e) => {
                        log(&hub.dir, &format!("company sign-in failed: {e:#}"));
                        let who = e.downcast_ref::<crate::oidc::NoGroup>().map(|n| n.name.clone()).unwrap_or_default();
                        hub.audit_sign_in(&who, &crate::i18n::fmt("company sign-in failed: {0}", &[format!("{e:#}")]), peer);
                        sign_in_problem(&stream, &format!("{e:#}"))
                    }
                };
            }
            _ => {}
        }
    }
    // Signing in to the web view with a name and a password (counted like wrong tokens).
    if head.method == "POST" && path == "/api/login" {
        if head.length > 64 * 1024 {
            return respond(&stream, 413, "application/json; charset=utf-8", br#"{"error":"request body too large"}"#);
        }
        let body = match read_body(&mut reader, head.length, started) {
            Ok(body) => body,
            Err(HttpError(status, message)) => return respond(&stream, status, "application/json; charset=utf-8", json!({ "error": message }).to_string().as_bytes()),
        };
        if hub.oidc.as_ref().is_some_and(|p| !p.config.password_login) {
            return respond(&stream, 403, "application/json; charset=utf-8", json!({ "error": crate::i18n::tr("this hub accepts only the company sign-in") }).to_string().as_bytes());
        }
        let (status, value) = hub.login(&body, peer);
        return respond(&stream, status, "application/json; charset=utf-8", value.to_string().as_bytes());
    }
    // Who is asking, before the body is read. A valid token always passes; an address that sent many wrong tokens
    // gets 429 for a while (a request without a token is not counted: browsers and probes send those).
    let Some((user, role, must_change)) = hub.user(&head.token) else {
        let mut locked = false;
        if let (Some(ip), false) = (peer, head.token.is_empty()) {
            let mut failures = hub.failures.lock().unwrap_or_else(|p| p.into_inner());
            if failures.len() > 10_000 {
                failures.retain(|_, list| list.last().is_some_and(|t| t.elapsed() < FAILURE_WINDOW));
            }
            let list = failures.entry(ip).or_default();
            list.retain(|t| t.elapsed() < FAILURE_WINDOW);
            let mut seen = hub.failed_tokens.lock().unwrap_or_else(|p| p.into_inner());
            seen.retain(|_, t| t.elapsed() < FAILURE_WINDOW);
            if seen.insert(token_hash(&head.token), Instant::now()).is_none() {
                list.push(Instant::now());
            }
            locked = list.len() > MAX_FAILURES;
            if list.len() == MAX_FAILURES {
                log(&hub.dir, &format!("{ip}: {MAX_FAILURES} wrong tokens, refused for {} minutes", FAILURE_WINDOW.as_secs() / 60));
            }
        }
        // The body is read first, so the client (or the proxy in front of the hub) gets the answer instead of a reset.
        let _ = read_body(&mut reader, head.length, started);
        return if locked {
            respond(&stream, 429, "application/json; charset=utf-8", json!({ "error": "too many wrong tokens, try again later" }).to_string().as_bytes())
        } else {
            respond(&stream, 401, "application/json; charset=utf-8", json!({ "error": crate::i18n::tr("missing or unknown hub token") }).to_string().as_bytes())
        };
    };
    let body = match read_body(&mut reader, head.length, started) {
        Ok(body) => body,
        Err(HttpError(status, message)) => return respond(&stream, status, "application/json; charset=utf-8", json!({ "error": message }).to_string().as_bytes()),
    };
    let content_type = head.content_type.clone();
    let request = Request { method: head.method, path, query, token: head.token, body };
    // An account with a password set by someone else (the default one, a reset) can only change it or sign out.
    if must_change && !matches!((request.method.as_str(), request.path.as_str()), ("GET", "/api/me") | ("POST", "/api/password") | ("POST", "/api/logout")) {
        return respond(&stream, 403, "application/json; charset=utf-8", json!({ "error": crate::i18n::tr("change your password first"), "must_change": true }).to_string().as_bytes());
    }
    // A Word or Excel report of a package: a file, not JSON.
    if request.method == "GET" {
        let parts: Vec<&str> = request.path.trim_matches('/').split('/').collect();
        if let ["api", "sources", id, "report"] = parts.as_slice() {
            let result = (|| -> Result<(Vec<u8>, &'static str, String), HttpError> {
                let source = hub.source(id)?;
                let package: i64 = request.query.get("package").and_then(|p| p.parse().ok()).ok_or_else(|| deny(400, crate::i18n::tr("give the package (package_id)")))?;
                let format = request.query.get("format").map(String::as_str).unwrap_or("docx");
                package_report(&source.cfg, &source.name, package, format)
            })();
            return match result {
                Ok((bytes, content_type, name)) => {
                    let _ = hub.append_audit(&json!({ "time": chrono::Local::now().to_rfc3339(), "user": user, "source": id, "summary": crate::i18n::fmt("report {0} downloaded", &[name.clone()]), "ok": true }));
                    respond_file(&stream, content_type, &name, &bytes)
                }
                Err(HttpError(status, message)) => respond(&stream, status, "application/json; charset=utf-8", json!({ "error": message }).to_string().as_bytes()),
            };
        }
    }
    // The team's services for Compass: Claude with the team's key, Jira and Confluence with the team's account.
    let gateway = if request.method == "POST" && request.path == "/ai/v1/messages" {
        Some(hub.ai_gateway(&user, role, &request.body))
    } else if let Some(rest) = request.path.strip_prefix("/proxy/jira") {
        Some(hub.service_proxy("jira", &user, role, &request.method, rest, &request.query, &request.body, &content_type))
    } else if let Some(rest) = request.path.strip_prefix("/proxy/confluence") {
        Some(hub.service_proxy("confluence", &user, role, &request.method, rest, &request.query, &request.body, &content_type))
    } else {
        None
    };
    if let Some(result) = gateway {
        return match result {
            Ok((status, content_type, bytes)) => respond(&stream, status, &content_type, &bytes),
            Err(HttpError(status, message)) => {
                log(&hub.dir, &format!("{} {} ({user}) -> {status} {message}", request.method, request.path));
                respond(&stream, status, "application/json; charset=utf-8", json!({ "type": "error", "error": { "type": "hub_error", "message": message } }).to_string().as_bytes())
            }
        };
    }
    match route(hub, &request, user.to_string(), role) {
        Ok((status, value)) => respond(&stream, status, "application/json; charset=utf-8", value.to_string().as_bytes()),
        Err(HttpError(status, message)) => {
            let shown = if status >= 500 {
                let id = uuid::Uuid::new_v4().simple().to_string()[..8].to_string();
                log(&hub.dir, &format!("{} {} ({user}) -> {status} [{id}] {message}", request.method, request.path));
                crate::i18n::fmt("internal error of the hub (id {0}; details in hub.log)", &[id])
            } else {
                log(&hub.dir, &format!("{} {} ({user}) -> {status} {message}", request.method, request.path));
                message
            };
            respond(&stream, status, "application/json; charset=utf-8", json!({ "error": shown }).to_string().as_bytes())
        }
    }
}

impl Hub {
    /// The user of an API token or of a session of the web view: (name, role, must change the password).
    fn user(&self, token: &str) -> Option<(String, Role, bool)> {
        if token.is_empty() {
            return None;
        }
        let hash = token_hash(token);
        let users = self.users.lock().unwrap_or_else(|p| p.into_inner());
        // The hashes are compared in full for every user (no early exit).
        let mut found = None;
        for u in users.iter() {
            if let Some(h) = &u.token {
                if h.iter().zip(hash.iter()).fold(0u8, |acc, (a, b)| acc | (a ^ b)) == 0 {
                    found = Some((u.name.clone(), u.role, false));
                }
            }
        }
        if found.is_some() {
            return found;
        }
        let mut sessions = self.sessions.lock().unwrap_or_else(|p| p.into_inner());
        sessions.retain(|_, (_, until)| *until > Instant::now());
        let (name, _) = sessions.get(&hash)?.clone();
        if let Some(role) = self.external.lock().unwrap_or_else(|p| p.into_inner()).get(&name) {
            return Some((name.clone(), *role, false));
        }
        users.iter().find(|u| u.name == name).map(|u| (u.name.clone(), u.role, u.must_change))
    }

    /// Sign-in with a name and a password: a session token for the web view.
    fn login(&self, body: &[u8], peer: Option<IpAddr>) -> (u16, Value) {
        let b: Value = serde_json::from_slice(body).unwrap_or(Value::Null);
        let name = b["name"].as_str().unwrap_or("").trim().to_string();
        let password = b["password"].as_str().unwrap_or("").to_string();
        if let Some(ip) = peer {
            let mut failures = self.failures.lock().unwrap_or_else(|p| p.into_inner());
            let list = failures.entry(ip).or_default();
            list.retain(|t| t.elapsed() < FAILURE_WINDOW);
            if list.len() >= MAX_FAILURES {
                drop(failures);
                self.audit_sign_in(&name, &crate::i18n::tr("sign-in blocked: too many wrong sign-ins from this address"), peer);
                return (429, json!({ "error": "too many wrong sign-ins, try again later" }));
            }
        }
        let user = self.users.lock().unwrap_or_else(|p| p.into_inner()).iter().find(|u| u.name.eq_ignore_ascii_case(&name)).cloned();
        let ok = user.as_ref().and_then(|u| u.password.as_deref()).is_some_and(|stored| password_ok(stored, &password));
        let Some(user) = user.filter(|_| ok) else {
            if let Some(ip) = peer {
                let mut failures = self.failures.lock().unwrap_or_else(|p| p.into_inner());
                failures.entry(ip).or_default().push(Instant::now());
            }
            log(&self.dir, &format!("failed sign-in as {name:?}"));
            self.audit_sign_in(&name, &crate::i18n::tr("wrong user name or password"), peer);
            return (401, json!({ "error": crate::i18n::tr("wrong user name or password") }));
        };
        let session = format!("lcs_{}{}", uuid::Uuid::new_v4().simple(), uuid::Uuid::new_v4().simple());
        self.sessions.lock().unwrap_or_else(|p| p.into_inner()).insert(token_hash(&session), (user.name.clone(), Instant::now() + Duration::from_secs(SESSION_HOURS * 3600)));
        self.save_sessions();
        log(&self.dir, &format!("{} signed in", user.name));
        (200, json!({ "token": session, "name": user.name, "role": user.role.name(), "must_change": user.must_change }))
    }

    /// Writes the users into hub.json (tokens and passwords only as hashes).
    fn save_users(&self) -> Result<()> {
        let users: Vec<Value> = self.users.lock().unwrap_or_else(|p| p.into_inner()).iter().map(|u| {
            let mut v = json!({ "name": u.name, "role": u.role.name() });
            if let Some(t) = &u.token {
                v["token_sha256"] = json!(hex(t));
            }
            if let Some(p) = &u.password {
                v["password_pbkdf2"] = json!(p);
            }
            if u.must_change {
                v["must_change"] = json!(true);
            }
            v
        }).collect();
        let mut file: Value = serde_json::from_str(&std::fs::read_to_string(&self.file)?)?;
        file["users"] = json!(users);
        let temp = self.file.with_extension("json.tmp");
        std::fs::write(&temp, serde_json::to_string_pretty(&file)?)?;
        std::fs::rename(&temp, &self.file)?;
        Ok(())
    }

    fn source(&self, id: &str) -> Result<&Source, HttpError> {
        self.sources.iter().find(|s| s.id == id).ok_or_else(|| deny(404, crate::i18n::fmt("there is no source {0}", &[id.to_string()])))
    }

    fn rules(&self) -> Value {
        let policy = self.policy.lock().unwrap();
        if policy["governance"].is_object() { policy["governance"].clone() } else { json!({}) }
    }

    /// Sessions (hashes only, never the tokens) and their owners in sessions.json, so a restart of the hub (an update,
    /// a reboot) does not sign everybody out.
    fn save_sessions(&self) {
        let now = Instant::now();
        let list: Vec<Value> = self.sessions.lock().unwrap_or_else(|p| p.into_inner()).iter().filter(|(_, (_, until))| *until > now)
            .map(|(hash, (name, until))| json!({ "hash": hex(hash), "name": name, "until": chrono::Utc::now().timestamp() + until.duration_since(now).as_secs() as i64 })).collect();
        let _ = std::fs::write(self.dir.join("sessions.json"), Value::Array(list).to_string());
    }

    fn load_sessions(&self) {
        let now = chrono::Utc::now().timestamp();
        let saved: Value = std::fs::read_to_string(self.dir.join("sessions.json")).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or(Value::Null);
        let mut sessions = self.sessions.lock().unwrap_or_else(|p| p.into_inner());
        for s in saved.as_array().into_iter().flatten() {
            let (Some(hash), Some(name), Some(until)) = (s["hash"].as_str(), s["name"].as_str(), s["until"].as_i64()) else { continue };
            let Some(bytes) = (0..hash.len()).step_by(2).map(|i| u8::from_str_radix(hash.get(i..i + 2)?, 16).ok()).collect::<Option<Vec<u8>>>() else { continue };
            if let (Ok(key), true) = (<[u8; 32]>::try_from(bytes.as_slice()), until > now) {
                sessions.insert(key, (name.to_string(), Instant::now() + Duration::from_secs((until - now) as u64)));
            }
        }
        drop(sessions);
        // The roles of company accounts from their last sign-in (the provider gives them again at the next one).
        let accounts: Value = std::fs::read_to_string(self.dir.join("company_accounts.json")).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or(Value::Null);
        let mut external = self.external.lock().unwrap_or_else(|p| p.into_inner());
        for (name, v) in accounts.as_object().into_iter().flatten() {
            if let Some(role) = v["role"].as_str() {
                external.insert(name.clone(), Role::parse(role));
            }
        }
    }

    /// A refused or failed sign-in in the audit trail (result ✗), with the client address.
    fn audit_sign_in(&self, user: &str, reason: &str, peer: Option<IpAddr>) {
        let _ = self.append_audit(&json!({ "time": chrono::Local::now().to_rfc3339(), "user": if user.is_empty() { "?" } else { user },
            "source": "", "summary": crate::i18n::tr("sign-in"), "ok": false,
            "error": match peer { Some(ip) => format!("{reason} ({ip})"), None => reason.to_string() } }));
    }

    /// Company accounts are not users of the hub (their role comes from the provider at every sign-in); the last
    /// sign-in of each is kept in company_accounts.json only to show the administrator who uses the hub.
    fn remember_company_account(&self, name: &str, role: Role) {
        let file = self.dir.join("company_accounts.json");
        let mut all: Value = std::fs::read_to_string(&file).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_else(|| json!({}));
        all[name] = json!({ "role": role.name(), "last_sign_in": chrono::Local::now().to_rfc3339() });
        let _ = std::fs::write(&file, serde_json::to_string_pretty(&all).unwrap_or_default());
    }

    fn company_accounts(&self) -> Vec<Value> {
        let all: Value = std::fs::read_to_string(self.dir.join("company_accounts.json")).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_else(|| json!({}));
        let mut list: Vec<Value> = all.as_object().into_iter().flatten().map(|(name, v)| json!({ "name": name, "role": v["role"], "last_sign_in": v["last_sign_in"] })).collect();
        list.sort_by(|a, b| b["last_sign_in"].as_str().cmp(&a["last_sign_in"].as_str()));
        list
    }

    fn append_audit(&self, entry: &Value) -> Result<()> {
        let _guard = self.audit_lock.lock().unwrap_or_else(|p| p.into_inner());
        rotate(&self.dir.join("audit.jsonl"));
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

    fn changes_dir(&self) -> PathBuf {
        self.dir.join("changes")
    }

    /// Saves a change set atomically (a temporary file renamed over the old one).
    fn save_change(&self, change: &Value) -> Result<()> {
        std::fs::create_dir_all(self.changes_dir())?;
        let id = change["id"].as_str().unwrap_or("x").replace(['/', '\\', '.'], "_");
        let path = self.changes_dir().join(format!("{id}.json"));
        let temp = self.changes_dir().join(format!("{id}.json.tmp"));
        std::fs::write(&temp, serde_json::to_string_pretty(change)?)?;
        std::fs::rename(&temp, &path)?;
        Ok(())
    }

    fn change(&self, id: &str) -> Option<Value> {
        if !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
            return None;
        }
        serde_json::from_str(&std::fs::read_to_string(self.changes_dir().join(format!("{id}.json"))).ok()?).ok()
    }

    fn changes(&self) -> Vec<Value> {
        std::fs::read_dir(self.changes_dir())
            .into_iter()
            .flatten()
            .flatten()
            .filter(|e| e.path().extension().is_some_and(|x| x == "json"))
            .filter_map(|e| serde_json::from_str(&std::fs::read_to_string(e.path()).ok()?).ok())
            .collect()
    }

    fn notes_dir(&self) -> PathBuf {
        self.dir.join("notes")
    }
}

fn note_name(name: &str) -> Result<String, HttpError> {
    // Windows device names (CON, NUL, COM1…) are not files.
    let base = name.split('.').next().unwrap_or("").trim().to_ascii_uppercase();
    let device = matches!(base.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || ((base.starts_with("COM") || base.starts_with("LPT")) && base.chars().count() == 4 && base.chars().nth(3).is_some_and(|c| c.is_numeric()));
    let ok = !name.is_empty() && name.len() <= 80 && name.chars().all(|c| c.is_alphanumeric() || matches!(c, '-' | '_' | ' ' | '.')) && !name.starts_with('.') && !name.ends_with('.') && !name.ends_with(' ') && !device;
    if ok { Ok(name.to_string()) } else { Err(deny(400, crate::i18n::tr("a note name: letters, digits, spaces, '-', '_' and '.'"))) }
}

/// A text field of a client's audit entry, cut to size.
fn short(value: &Value, max: usize) -> Value {
    match value.as_str() {
        Some(s) => json!(s.chars().take(max).collect::<String>()),
        None => Value::Null,
    }
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

fn route(hub: &Hub, request: &Request, user: String, role: Role) -> Result<(u16, Value), HttpError> {
    let segments: Vec<&str> = request.path.trim_matches('/').split('/').collect();
    let body = || -> Result<Value, HttpError> { serde_json::from_slice(&request.body).map_err(|e| deny(400, format!("JSON: {e}"))) };
    match (request.method.as_str(), segments.as_slice()) {
        ("GET", ["api", "me"]) => {
            let must_change = hub.users.lock().unwrap_or_else(|p| p.into_inner()).iter().find(|u| u.name == user).is_some_and(|u| u.must_change);
            Ok((200, json!({ "name": user, "role": role.name(), "must_change": must_change })))
        }
        ("POST", ["api", "logout"]) => {
            hub.sessions.lock().unwrap_or_else(|p| p.into_inner()).remove(&token_hash(&request.token));
            hub.save_sessions();
            Ok((200, json!({ "ok": true })))
        }
        ("POST", ["api", "password"]) => {
            let b = body()?;
            let (old, new) = (b["old"].as_str().unwrap_or(""), b["new"].as_str().unwrap_or(""));
            check_new_password(&user, new)?;
            {
                let mut users = hub.users.lock().unwrap_or_else(|p| p.into_inner());
                let u = users.iter_mut().find(|u| u.name == user).ok_or_else(|| deny(404, "user"))?;
                if !u.password.as_deref().is_some_and(|p| password_ok(p, old)) {
                    return Err(deny(403, crate::i18n::tr("the current password is wrong")));
                }
                u.password = Some(password_hash(new));
                u.must_change = false;
            }
            hub.save_users().map_err(|e| deny(500, format!("{e:#}")))?;
            let _ = hub.append_audit(&json!({ "time": chrono::Local::now().to_rfc3339(), "user": user, "source": "", "summary": "password changed", "ok": true }));
            Ok((200, json!({ "ok": true })))
        }
        ("GET", ["api", "users", "ai"]) => {
            require(role, Role::Admin)?;
            Ok((200, json!(hub.ai_usage_all())))
        }
        ("GET", ["api", "users", "company"]) => {
            require(role, Role::Admin)?;
            Ok((200, json!(hub.company_accounts())))
        }
        ("GET", ["api", "users"]) => {
            require(role, Role::Admin)?;
            let users = hub.users.lock().unwrap_or_else(|p| p.into_inner());
            Ok((200, json!(users.iter().map(|u| json!({ "name": u.name, "role": u.role.name(), "password": u.password.is_some(), "token": u.token.is_some(), "must_change": u.must_change })).collect::<Vec<_>>())))
        }
        ("POST", ["api", "users"]) => {
            require(role, Role::Admin)?;
            let b = body()?;
            let name = b["name"].as_str().unwrap_or("").trim().to_string();
            if name.is_empty() || name.chars().count() > 60 || !name.chars().all(|c| c.is_alphanumeric() || matches!(c, '.' | '-' | '_' | '@')) {
                return Err(deny(400, crate::i18n::tr("a user name: letters, digits and . - _ @")));
            }
            let password = b["password"].as_str().unwrap_or("");
            check_new_password(&name, password)?;
            {
                let mut users = hub.users.lock().unwrap_or_else(|p| p.into_inner());
                if users.iter().any(|u| u.name.eq_ignore_ascii_case(&name)) {
                    return Err(deny(409, crate::i18n::fmt("hub user {0}: the name or the token is used twice", &[name.clone()])));
                }
                users.push(User { name: name.clone(), role: Role::parse(b["role"].as_str().unwrap_or("viewer")), token: None, password: Some(password_hash(password)), must_change: true });
            }
            hub.save_users().map_err(|e| deny(500, format!("{e:#}")))?;
            let _ = hub.append_audit(&json!({ "time": chrono::Local::now().to_rfc3339(), "user": user, "source": "", "summary": format!("user {name} added"), "ok": true }));
            Ok((201, json!({ "ok": true })))
        }
        ("PUT", ["api", "users", name]) => {
            require(role, Role::Admin)?;
            let b = body()?;
            {
                let mut users = hub.users.lock().unwrap_or_else(|p| p.into_inner());
                let admins = users.iter().filter(|u| u.role == Role::Admin).count();
                let u = users.iter_mut().find(|u| u.name == *name).ok_or_else(|| deny(404, crate::i18n::fmt("there is no user {0}", &[name.to_string()])))?;
                if let Some(r) = b["role"].as_str() {
                    let new_role = Role::parse(r);
                    if u.role == Role::Admin && new_role != Role::Admin && admins <= 1 {
                        return Err(deny(400, crate::i18n::tr("the hub needs at least one admin")));
                    }
                    u.role = new_role;
                }
                if let Some(p) = b["password"].as_str() {
                    check_new_password(name, p)?;
                    u.password = Some(password_hash(p));
                    u.must_change = true;
                }
            }
            hub.sessions.lock().unwrap_or_else(|p| p.into_inner()).retain(|_, (n, _)| n != *name);
            hub.save_users().map_err(|e| deny(500, format!("{e:#}")))?;
            let _ = hub.append_audit(&json!({ "time": chrono::Local::now().to_rfc3339(), "user": user, "source": "", "summary": format!("user {name} changed"), "ok": true }));
            Ok((200, json!({ "ok": true })))
        }
        ("DELETE", ["api", "users", name]) => {
            require(role, Role::Admin)?;
            if *name == user {
                return Err(deny(400, crate::i18n::tr("you cannot remove yourself")));
            }
            {
                let mut users = hub.users.lock().unwrap_or_else(|p| p.into_inner());
                let admins = users.iter().filter(|u| u.role == Role::Admin).count();
                let target = users.iter().find(|u| u.name == *name).ok_or_else(|| deny(404, crate::i18n::fmt("there is no user {0}", &[name.to_string()])))?;
                if target.role == Role::Admin && admins <= 1 {
                    return Err(deny(400, crate::i18n::tr("the hub needs at least one admin")));
                }
                users.retain(|u| u.name != *name);
            }
            hub.sessions.lock().unwrap_or_else(|p| p.into_inner()).retain(|_, (n, _)| n != *name);
            hub.save_users().map_err(|e| deny(500, format!("{e:#}")))?;
            let _ = hub.append_audit(&json!({ "time": chrono::Local::now().to_rfc3339(), "user": user, "source": "", "summary": format!("user {name} removed"), "ok": true }));
            Ok((200, json!({ "ok": true })))
        }
        // A new API token of a user (for Compass and MCP clients): shown once, kept as a hash.
        ("POST", ["api", "users", name, "token"]) => {
            require(role, Role::Admin)?;
            let (token, _) = new_token();
            {
                let mut users = hub.users.lock().unwrap_or_else(|p| p.into_inner());
                let u = users.iter_mut().find(|u| u.name == *name).ok_or_else(|| deny(404, crate::i18n::fmt("there is no user {0}", &[name.to_string()])))?;
                u.token = Some(token_hash(&token));
            }
            hub.save_users().map_err(|e| deny(500, format!("{e:#}")))?;
            let _ = hub.append_audit(&json!({ "time": chrono::Local::now().to_rfc3339(), "user": user, "source": "", "summary": format!("new API token for {name}"), "ok": true }));
            Ok((201, json!({ "token": token })))
        }
        ("GET", ["api", "integrations"]) => Ok((200, json!({
            "jira": hub.integrations["jira"]["url"].as_str().is_some_and(|u| !u.trim().is_empty()),
            "confluence": hub.integrations["confluence"]["url"].as_str().is_some_and(|u| !u.trim().is_empty()),
        }))),
        // Jira and Confluence of the team, as actions of an architect (never of the assistant): the hub's service account.
        ("POST", ["api", "sources", id, "jira"]) => {
            require(role, Role::Architect)?;
            let source = hub.source(id)?;
            let b = body()?;
            let (summary, description) = issue_text(&b)?;
            let mut text = description;
            if let Some(element) = b["element"].as_i64() {
                text.push_str(&format!("\n\n{}\n{}", crate::i18n::fmt("Element of the model {0} (id {1}):", &[source.name.clone(), element.to_string()]), element_details(&source.cfg, element).map_err(|e| deny(400, e.to_string()))?.chars().take(4000).collect::<String>()));
            }
            let (key, url) = hub.create_issue(&summary, &text)?;
            let _ = hub.append_audit(&json!({ "time": chrono::Local::now().to_rfc3339(), "user": user, "source": id, "summary": format!("Jira {key}: {summary}"), "ok": true }));
            Ok((201, json!({ "key": key, "url": url })))
        }
        ("POST", ["api", "changes", id, "jira"]) => {
            require(role, Role::Architect)?;
            let b = body()?;
            let (summary, description) = issue_text(&b)?;
            let _guard = hub.change_lock.lock().unwrap_or_else(|p| p.into_inner());
            let mut change = hub.change(id).ok_or_else(|| deny(404, crate::i18n::fmt("there is no change set {0}", &[id.to_string()])))?;
            if change["jira"].is_object() {
                return Err(deny(409, crate::i18n::tr("this change set already has a Jira issue")));
            }
            let text = format!("{description}\n\n{} {} ({})\n{}\n\n{}", id, change["title"].as_str().unwrap_or(""), change["status"].as_str().unwrap_or(""),
                change["summary"].as_str().unwrap_or(""), change["preview"].as_str().unwrap_or("").chars().take(6000).collect::<String>());
            let (key, url) = hub.create_issue(&summary, &text)?;
            change["jira"] = json!({ "key": key, "url": url, "by": user });
            hub.save_change(&change).map_err(|e| deny(500, e.to_string()))?;
            let _ = hub.append_audit(&json!({ "time": chrono::Local::now().to_rfc3339(), "user": user, "source": change["source"], "summary": format!("Jira {key} for {id}"), "ok": true }));
            Ok((201, json!({ "key": key, "url": url })))
        }
        ("POST", ["api", "sources", id, "confluence"]) => {
            require(role, Role::Architect)?;
            let source = hub.source(id)?;
            let b = body()?;
            let package = b["package"].as_i64().ok_or_else(|| deny(400, crate::i18n::fmt("missing number '{0}'", &["package".to_string()])))?;
            let (title, storage) = package_page(&source.cfg, &source.name, package)?;
            let title = b["title"].as_str().map(str::trim).filter(|t| !t.is_empty()).map(|t| t.chars().take(200).collect::<String>()).unwrap_or(title);
            let (connection, space, parent) = hub.confluence()?;
            let (page_id, page_url, created) = connection.publish(&space, &title, parent.as_deref(), None, &storage, &[]).map_err(|e| deny(502, format!("Confluence: {e:#}")))?;
            let _ = hub.append_audit(&json!({ "time": chrono::Local::now().to_rfc3339(), "user": user, "source": id, "summary": format!("Confluence page {page_id}: {title}"), "ok": true }));
            Ok((201, json!({ "title": title, "url": page_url, "created": created })))
        }
        ("GET", ["api", "policy"]) => {
            let mut policy = hub.policy.lock().unwrap().clone();
            // Compass working through the hub needs the team's project and space as its defaults.
            if policy["jira_via_hub"] == true {
                policy["jira_project"] = hub.integrations["jira"]["project"].clone();
            }
            if policy["confluence_via_hub"] == true {
                policy["confluence_space"] = hub.integrations["confluence"]["space"].clone();
            }
            Ok((200, policy))
        }
        ("PUT", ["api", "policy"]) => {
            require(role, Role::Admin)?;
            let policy = body()?;
            if !policy.is_object() {
                return Err(deny(400, crate::i18n::tr("the policy is a JSON object")));
            }
            let mut file: Value = serde_json::from_str(&std::fs::read_to_string(&hub.file).map_err(|e| deny(500, e.to_string()))?).map_err(|e| deny(500, e.to_string()))?;
            file["policy"] = policy.clone();
            let temp = hub.file.with_extension("json.tmp");
            std::fs::write(&temp, serde_json::to_string_pretty(&file).unwrap_or_default()).and_then(|_| std::fs::rename(&temp, &hub.file)).map_err(|e| deny(500, e.to_string()))?;
            *hub.policy.lock().unwrap() = policy;
            let _ = hub.append_audit(&json!({ "time": chrono::Local::now().to_rfc3339(), "user": user, "source": "", "summary": crate::i18n::tr("team policy changed"), "ok": true }));
            Ok((200, json!({ "ok": true })))
        }
        ("GET", ["api", "sources"]) => Ok((200, json!(hub.sources.iter().map(|s| json!({ "id": s.id, "name": s.name, "kind": if s.cfg.uses_file() { "file" } else { "database" } })).collect::<Vec<_>>()))),
        // Questions of people without EA: the assistant reads the source and answers (it cannot change the model).
        // Read-only GraphQL over a model (any role).
        ("POST", ["api", "sources", id, "graphql"]) => {
            let source = hub.source(id)?;
            Ok((200, crate::graphql::execute(&source.cfg, &body()?)))
        }
        ("GET", ["api", "graphql", "schema"]) => Ok((200, json!({ "sdl": crate::graphql::sdl() }))),
        // Flat tables of a model for Power BI, Excel and other tools (Get Data → Web, with the Authorization header).
        ("GET", ["api", "sources", id, "table", table]) => {
            let source = hub.source(id)?;
            let sql = match *table {
                "elements" => "select o.object_id, o.name, o.object_type, o.stereotype, o.status, o.package_id, p.name as package, o.author, o.version, o.phase, o.createddate, o.modifieddate from t_object o left join t_package p on p.package_id = o.package_id where o.object_type <> 'Package'",
                "connectors" => "select c.connector_id, c.connector_type, c.stereotype, c.name, c.start_object_id, s.name as source, c.end_object_id, t.name as target from t_connector c left join t_object s on s.object_id = c.start_object_id left join t_object t on t.object_id = c.end_object_id",
                "packages" => "select package_id, name, parent_id, version, createddate, modifieddate from t_package",
                "diagrams" => "select diagram_id, name, diagram_type, package_id, author, createddate, modifieddate from t_diagram",
                "attributes" => "select a.id, a.object_id, o.name as element, a.name, a.type from t_attribute a left join t_object o on o.object_id = a.object_id",
                "tagged_values" => "select v.propertyid, v.object_id, o.name as element, v.property, v.value from t_objectproperties v left join t_object o on o.object_id = v.object_id",
                _ => return Err(deny(404, crate::i18n::tr("tables: elements, connectors, packages, diagrams, attributes, tagged_values"))),
            };
            let sql = crate::model_db::portable_sql(source.cfg.model_source, &format!("{sql} limit 100000"));
            let names: Vec<String> = sql
                .split(" from ")
                .next()
                .unwrap_or("")
                .trim_start_matches("select ")
                .trim_start_matches("top 100000 ")
                .split(", ")
                .map(|c| c.rsplit(" as ").next().unwrap_or(c).rsplit('.').next().unwrap_or(c).trim().to_string())
                .collect();
            let rows = crate::tools::query_rows(&source.cfg, &sql)?;
            Ok((200, Value::Array(rows.into_iter().map(|r| Value::Object(names.iter().cloned().zip(r.into_iter().map(Value::String)).collect())).collect())))
        }
        ("POST", ["api", "sources", id, "ask"]) => {
            let source = hub.source(id)?;
            let question = body()?["question"].as_str().unwrap_or("").trim().to_string();
            if question.chars().count() < 3 {
                return Err(deny(400, crate::i18n::tr("type a question")));
            }
            if question.chars().count() > 2000 {
                return Err(deny(400, crate::i18n::tr("the question is too long (at most 2000 characters)")));
            }
            // Who may ask (the policy, "ask_min_role"; everyone by default) and how often ("ask_per_hour").
            let (min_role, per_hour) = {
                let policy = hub.policy.lock().unwrap_or_else(|p| p.into_inner());
                (Role::parse(policy["ask_min_role"].as_str().unwrap_or("viewer")), policy["ask_per_hour"].as_u64().map(|n| n as usize).unwrap_or(ASK_PER_HOUR))
            };
            require(role, min_role)?;
            {
                let mut asks = hub.asks.lock().unwrap_or_else(|p| p.into_inner());
                let list = asks.entry(user.clone()).or_default();
                list.retain(|t| t.elapsed() < Duration::from_secs(3600));
                if list.len() >= per_hour {
                    return Err(deny(429, crate::i18n::fmt("at most {0} questions per hour", &[per_hour.to_string()])));
                }
            }
            // One question at a time: the cost of the model stays under control; the others are told to wait.
            let _turn = match hub.ask_lock.try_lock() {
                Ok(guard) => guard,
                Err(std::sync::TryLockError::Poisoned(p)) => p.into_inner(),
                Err(std::sync::TryLockError::WouldBlock) => return Err(deny(429, crate::i18n::tr("the assistant is answering another question, try again in a moment"))),
            };
            hub.asks.lock().unwrap_or_else(|p| p.into_inner()).entry(user.clone()).or_default().push(Instant::now());
            let started = Instant::now();
            let prompt = format!(
                "[A question from {user} through the team hub; read-only] {question}\n\nAnswer for a person who does not use Enterprise Architect: plain language, \
in the language of the question, short. You can only read the model (db_query and the other reading tools); you cannot change it."
            );
            let answer = crate::agent_ro::run(&source.cfg, &prompt, ASK_TIMEOUT)?;
            let _ = hub.append_audit(&json!({ "time": chrono::Local::now().to_rfc3339(), "user": user, "source": id, "summary": format!("question: {}", question.chars().take(200).collect::<String>()), "ok": true }));
            Ok((200, json!({ "answer": answer, "seconds": started.elapsed().as_secs() })))
        }
        ("GET", ["api", "sources", id, rest @ ..]) => {
            let source = hub.source(id)?;
            if rest == ["search"] && request.query.get("q").map(|q| q.trim().chars().count()).unwrap_or(0) < 2 {
                return Err(deny(400, crate::i18n::tr("type at least 2 characters")));
            }
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
            let sent = body()?;
            if !sent.is_object() {
                return Err(deny(400, crate::i18n::tr("an audit entry is a JSON object")));
            }
            // Only the known fields, cut to size; who and when are set by the hub (the client's time is kept apart).
            let entry = json!({
                "time": chrono::Local::now().to_rfc3339(),
                "user": user,
                "client_time": short(&sent["time"], 40),
                "source": short(&sent["source"], 200),
                "database": short(&sent["database"], 300),
                "ea_user": short(&sent["ea_user"], 100),
                "approval": short(&sent["approval"], 20),
                "summary": short(&sent["summary"], MAX_AUDIT_TEXT),
                "operations": sent["operations"].as_u64().map(Value::from).unwrap_or(Value::Null),
                "ok": sent["ok"].as_bool().unwrap_or(false),
                "error": short(&sent["error"], MAX_AUDIT_TEXT / 2),
            });
            hub.append_audit(&entry).map_err(|e| deny(500, e.to_string()))?;
            Ok((201, json!({ "ok": true })))
        }
        ("GET", ["api", "audit"]) => {
            let limit = request.query.get("limit").and_then(|l| l.parse().ok()).unwrap_or(100usize).min(1000);
            let mut entries = hub.read_audit(request.query.get("source").map(String::as_str), limit);
            // Connection details of the workstations are for the admin only.
            if role < Role::Admin {
                for e in entries.iter_mut() {
                    if let Some(o) = e.as_object_mut() {
                        o.remove("database");
                        o.remove("ea_user");
                    }
                }
            }
            Ok((200, json!(entries)))
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
            if text.len() > MAX_NOTE {
                return Err(deny(413, crate::i18n::fmt("a note may have at most {0} KB", &[(MAX_NOTE / 1024).to_string()])));
            }
            let exists = hub.notes_dir().join(format!("{name}.md")).is_file();
            if !exists && std::fs::read_dir(hub.notes_dir()).map(|d| d.count()).unwrap_or(0) >= MAX_NOTES {
                return Err(deny(400, crate::i18n::fmt("at most {0} notes", &[MAX_NOTES.to_string()])));
            }
            std::fs::create_dir_all(hub.notes_dir()).map_err(|e| deny(500, e.to_string()))?;
            std::fs::write(hub.notes_dir().join(format!("{name}.md")), text).map_err(|e| deny(500, e.to_string()))?;
            Ok((200, json!({ "ok": true })))
        }
        // Change sets: proposed by the assistant, reviewed by the team, applied after approval.
        ("POST", ["api", "changes"]) => {
            require(role, Role::Architect)?;
            let b = body()?;
            let given = b["ops"].as_array().filter(|o| !o.is_empty()).ok_or_else(|| deny(400, crate::i18n::tr("a change set needs operations")))?;
            for (i, op) in given.iter().enumerate() {
                let name = op["op"].as_str().unwrap_or("");
                if !crate::ea::MODEL_OPS.contains(&name) {
                    return Err(deny(400, crate::i18n::fmt("operation {0} ('{1}') is not allowed. Allowed: {2}", &[(i + 1).to_string(), name.to_string(), crate::ea::MODEL_OPS.join(", ")])));
                }
                if matches!(name, "export_xmi" | "import_xmi" | "generate_report") {
                    crate::tools::check_file_path(op["file"].as_str().unwrap_or("")).map_err(|e| deny(400, e.to_string()))?;
                }
            }
            let normalized = crate::tools::normalize_ops(given);
            let ops = &normalized;
            let source = hub.source(b["source"].as_str().unwrap_or(""))?;
            // What the reviewers see is computed here from the operations, not taken from the author.
            let summary = crate::ea::describe(ops);
            let preview = crate::governance::preview(&source.cfg, ops);
            let _guard = hub.change_lock.lock().unwrap_or_else(|p| p.into_inner());
            let mut id = format!("CH-{}", chrono::Local::now().format("%Y%m%d-%H%M%S%3f"));
            while hub.changes_dir().join(format!("{id}.json")).exists() {
                id = format!("{id}x");
            }
            let change = json!({
                "id": id, "source": source.id, "title": b["title"].as_str().unwrap_or("Change").chars().take(200).collect::<String>(), "summary": summary, "preview": preview,
                "ops": ops, "author": user, "created": chrono::Local::now().to_rfc3339(), "status": "pending", "comments": []
            });
            hub.save_change(&change).map_err(|e| deny(500, e.to_string()))?;
            Ok((201, json!({ "id": id })))
        }
        ("GET", ["api", "changes"]) => {
            let status = request.query.get("status").cloned();
            let mut list: Vec<Value> = hub.changes().into_iter().filter(|c| status.as_deref().is_none_or(|s| c["status"] == s)).collect();
            list.sort_by(|a, b| b["created"].as_str().cmp(&a["created"].as_str()));
            Ok((200, json!(list.into_iter().map(|mut c| { c.as_object_mut().map(|o| o.remove("ops")); c }).collect::<Vec<_>>())))
        }
        ("GET", ["api", "changes", id]) => Ok((200, hub.change(id).ok_or_else(|| deny(404, crate::i18n::fmt("there is no change set {0}", &[id.to_string()])))?)),
        ("POST", ["api", "changes", id, action]) => {
            let _guard = hub.change_lock.lock().unwrap_or_else(|p| p.into_inner());
            let mut change = hub.change(id).ok_or_else(|| deny(404, crate::i18n::fmt("there is no change set {0}", &[id.to_string()])))?;
            let b = body()?;
            let now = chrono::Local::now().to_rfc3339();
            match *action {
                "comment" => {
                    let text = b["text"].as_str().unwrap_or("").trim().chars().take(MAX_AUDIT_TEXT).collect::<String>();
                    if text.is_empty() {
                        return Err(deny(400, crate::i18n::tr("empty comment")));
                    }
                    if change["comments"].as_array().is_some_and(|c| c.len() >= 200) {
                        return Err(deny(400, crate::i18n::tr("too many comments")));
                    }
                    change["comments"].as_array_mut().map(|c| c.push(json!({ "user": user, "time": now, "text": text })));
                }
                "decision" => {
                    require(role, Role::Architect)?;
                    if change["status"] != "pending" {
                        return Err(deny(400, crate::i18n::fmt("the change set is already {0}", &[change["status"].as_str().unwrap_or("").to_string()])));
                    }
                    // Four eyes: the author does not approve their own change set (an admin neither).
                    if change["author"] == user.as_str() {
                        return Err(deny(403, crate::i18n::tr("the author cannot approve their own change set")));
                    }
                    let approve = b["approve"] == true;
                    change["status"] = json!(if approve { "approved" } else { "rejected" });
                    change["decided_by"] = json!(user);
                    change["decided"] = json!(now);
                    change["reason"] = short(&b["reason"], MAX_AUDIT_TEXT);
                }
                // Applying in two steps: one client claims the approved set, applies it and reports the result.
                "claim" => {
                    require(role, Role::Architect)?;
                    if change["status"] != "approved" {
                        return Err(deny(409, crate::i18n::fmt("the change set is {0}: only an approved change set can be applied", &[change["status"].as_str().unwrap_or("").to_string()])));
                    }
                    change["status"] = json!("applying");
                    change["claimed_by"] = json!(user);
                    change["claimed"] = json!(now);
                }
                "applied" => {
                    require(role, Role::Architect)?;
                    if change["status"] != "applying" || change["claimed_by"] != user.as_str() {
                        return Err(deny(409, crate::i18n::tr("only the user who claimed the change set reports it as applied")));
                    }
                    if b["ok"] == true {
                        change["status"] = json!("applied");
                        change["applied_by"] = json!(user);
                        change["applied"] = json!(now);
                        change["result"] = short(&b["result"], MAX_AUDIT_TEXT / 2);
                    } else {
                        change["status"] = json!("approved");
                        change["last_failure"] = json!({ "user": user, "time": now, "result": short(&b["result"], MAX_AUDIT_TEXT / 2) });
                    }
                    if let Some(o) = change.as_object_mut() {
                        o.remove("claimed_by");
                        o.remove("claimed");
                    }
                }
                // A claim left by a Compass that stopped: an admin checks the model, then frees or closes the set.
                "reset" => {
                    require(role, Role::Admin)?;
                    if change["status"] != "applying" {
                        return Err(deny(409, crate::i18n::tr("only a change set being applied can be reset")));
                    }
                    change["status"] = json!(if b["applied"] == true { "applied" } else { "approved" });
                    change["reset_by"] = json!(user);
                    change["reset"] = json!(now);
                    if let Some(o) = change.as_object_mut() {
                        o.remove("claimed_by");
                        o.remove("claimed");
                    }
                }
                _ => return Err(deny(404, "not found")),
            }
            hub.save_change(&change).map_err(|e| deny(500, e.to_string()))?;
            let _ = hub.append_audit(&json!({ "time": now, "user": user, "source": change["source"], "summary": format!("{} {}: {}", action, id, change["title"].as_str().unwrap_or("")), "ok": true }));
            Ok((200, change))
        }
        ("POST", ["mcp"]) => mcp(hub, &body()?).map(|reply| match reply {
            Some(reply) => (200, reply),
            None => (202, Value::Null),
        }),
        _ => Err(deny(404, "not found")),
    }
}

fn packages(cfg: &AppConfig) -> Result<String> {
    let rows = crate::tools::query_rows(cfg, "select package_id as c0, parent_id as c1, name as c2 from t_package order by parent_id, tpos, name")?;
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
        "select o.object_id as c0, o.name as c1, o.object_type as c2, coalesce(o.stereotype,'') as c3, p.name as c4, substr(coalesce(o.note,''), 1, 120) as c5 from t_object o \
         join t_package p on p.package_id = o.package_id where lower(o.name) like '%{like}%' or lower(coalesce(o.alias,'')) like '%{like}%' order by o.name limit 100"
    ))?;
    let mut out = vec![crate::i18n::tr("object_id | name | type | stereotype | package | description").to_string()];
    out.extend(rows.into_iter().map(|r| r.iter().map(|c| c.replace(['\r', '\n'], " ")).collect::<Vec<_>>().join(" | ")));
    Ok(out.join("\n"))
}

/// Summary and description of a new issue from the request (cut to Jira's limits).
fn issue_text(b: &Value) -> Result<(String, String), HttpError> {
    let summary: String = b["summary"].as_str().unwrap_or("").trim().chars().filter(|c| !c.is_control()).take(250).collect();
    if summary.chars().count() < 3 {
        return Err(deny(400, crate::i18n::tr("type a summary for the issue")));
    }
    let description: String = b["description"].as_str().unwrap_or("").chars().take(20_000).collect();
    Ok((summary, description))
}

fn xml(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// The rest of a relationship query (after its first five columns): the types, ids and classifiers of both ends, for
/// [`named_ends`].
const ENDS: &str = "s.object_type as c5, s.object_id as c6, coalesce(sc.name,'') as c7, t.object_type as c8, t.object_id as c9, coalesce(tc.name,'') as c10 \
    from t_connector c join t_object s on s.object_id = c.start_object_id join t_object t on t.object_id = c.end_object_id \
    left join t_object sc on sc.object_id = s.classifier left join t_object tc on tc.object_id = t.classifier";

/// Relationship rows with the names of the ends (columns `source` and `target`) filled in where EA has none, e.g.
/// unnamed lifelines: ":Classifier" or "(unnamed Type #id)". The extra columns of [`ENDS`] are dropped.
fn named_ends(mut rows: Vec<Vec<String>>, source: usize, target: usize) -> Vec<Vec<String>> {
    let label = |name: &str, kind: &str, id: &str, classifier: &str| {
        if !name.trim().is_empty() {
            name.to_string()
        } else if !classifier.trim().is_empty() {
            format!(":{classifier}")
        } else {
            format!("({} {kind} #{id})", crate::i18n::tr("unnamed"))
        }
    };
    for r in &mut rows {
        if r.len() >= 11 {
            r[source] = label(&r[source], &r[5], &r[6], &r[7]);
            r[target] = label(&r[target], &r[8], &r[9], &r[10]);
        }
        r.truncate(5);
    }
    rows
}

/// A Confluence page (storage format) of a package with its subpackages: elements with their descriptions and the
/// relationships between them. Text only: the hub has no EA to draw diagrams.
/// A report of a package with its subpackages: Word (description, elements table, relationships) or Excel
/// (sheets Elements and Relationships). The hub has no EA, so there are no diagram pictures.
fn package_report(cfg: &AppConfig, model: &str, package: i64, format: &str) -> Result<(Vec<u8>, &'static str, String), HttpError> {
    let q = |sql: String| crate::tools::query_rows(cfg, &sql).map_err(|e| HttpError(500, format!("{e:#}")));
    let root = q(format!("select name as c0, coalesce(notes,'') as c1 from t_package where package_id = {package}"))?
        .into_iter()
        .next()
        .ok_or_else(|| deny(404, crate::i18n::fmt("there is no package with id {0}", &[package.to_string()])))?;
    let ids: Vec<String> = q(format!(
        "with recursive tree(package_id) as (select package_id as c0 from t_package where package_id = {package} \
         union all select p.package_id as c0 from t_package p join tree t on p.parent_id = t.package_id) select package_id as c0 from tree"
    ))?
    .into_iter()
    .map(|r| r[0].clone())
    .collect();
    let ids = ids.join(",");
    let elements = q(format!(
        "select coalesce(o.name,'') as c0, o.object_type as c1, coalesce(o.stereotype,'') as c2, coalesce(o.status,'') as c3, p.name as c4, coalesce(o.note,'') as c5 \
         from t_object o join t_package p on p.package_id = o.package_id where o.package_id in ({ids}) and o.object_type not in ('Package','Note','Text','Boundary') order by p.name, o.name"
    ))?;
    let relations = named_ends(q(format!(
        "select coalesce(s.name,'') as c0, c.connector_type as c1, coalesce(c.stereotype,'') as c2, coalesce(t.name,'') as c3, coalesce(c.name,'') as c4, {ENDS} \
         where s.package_id in ({ids}) and t.package_id in ({ids}) order by 1, 4"
    ))?, 0, 3);
    let base: String = root[0].chars().map(|c| if c.is_alphanumeric() { c } else { '_' }).collect();
    let date = chrono::Local::now().format("%Y-%m-%d").to_string();
    if format == "xlsx" {
        let sheet = |name: &str, columns: &[&'static str], rows: &[Vec<String>]| crate::xlsx::Sheet {
            name: name.to_string(),
            columns: columns.iter().map(|c| crate::i18n::tr(c).to_string()).collect(),
            rows: rows.iter().map(|r| r.iter().map(|v| json!(v)).collect()).collect(),
        };
        let bytes = crate::xlsx::build(&[
            sheet(crate::i18n::tr("Elements"), &["name", "type", "stereotype", "status", "package", "description"], &elements),
            sheet(crate::i18n::tr("Relationships"), &["source", "type", "stereotype", "target", "name"], &relations),
        ])
        .map_err(|e| HttpError(500, format!("{e:#}")))?;
        return Ok((bytes, "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet", format!("{base}_{date}.xlsx")));
    }
    let cell = |t: &str| t.replace('|', "/").replace(['\r', '\n'], " ");
    let mut md = String::new();
    md.push_str(&format!("*{}*\n\n", crate::i18n::fmt("Report of the model {0} from the team hub, {1}.", &[model.to_string(), date.clone()])));
    if !root[1].trim().is_empty() {
        md.push_str(&format!("{}\n\n", root[1].trim()));
    }
    md.push_str(&format!("## {} ({})\n\n| {} | {} | {} | {} |\n|---|---|---|---|\n", crate::i18n::tr("Elements"), elements.len(), crate::i18n::tr("name"), crate::i18n::tr("type"), crate::i18n::tr("package"), crate::i18n::tr("description")));
    for e in elements.iter().take(2000) {
        let kind = if e[2].is_empty() { e[1].clone() } else { format!("{} «{}»", e[1], e[2]) };
        md.push_str(&format!("| {} | {} | {} | {} |\n", cell(&e[0]), cell(&kind), cell(&e[4]), cell(&e[5].chars().take(500).collect::<String>())));
    }
    if !relations.is_empty() {
        md.push_str(&format!("\n## {} ({})\n\n", crate::i18n::tr("Relationships"), relations.len()));
        for r in relations.iter().take(2000) {
            let kind = if r[2].is_empty() { r[1].clone() } else { format!("{} «{}»", r[1], r[2]) };
            md.push_str(&format!("- {} → {} ({kind}){}\n", r[0], r[3], if r[4].is_empty() { String::new() } else { format!(": {}", r[4]) }));
        }
    }
    let bytes = crate::docx::build(&root[0], &md, |_, _| anyhow::bail!("no pictures in hub reports")).map_err(|e| HttpError(500, format!("{e:#}")))?;
    Ok((bytes, "application/vnd.openxmlformats-officedocument.wordprocessingml.document", format!("{base}_{date}.docx")))
}

fn package_page(cfg: &AppConfig, model: &str, package: i64) -> Result<(String, String), HttpError> {
    let q = |sql: String| crate::tools::query_rows(cfg, &sql).map_err(|e| HttpError(500, format!("{e:#}")));
    let root = q(format!("select name as c0, coalesce(notes,'') as c1 from t_package where package_id = {package}"))?
        .into_iter()
        .next()
        .ok_or_else(|| deny(404, crate::i18n::fmt("there is no package with id {0}", &[package.to_string()])))?;
    let ids: Vec<String> = q(format!(
        "with recursive tree(package_id) as (select package_id as c0 from t_package where package_id = {package} \
         union all select p.package_id as c0 from t_package p join tree t on p.parent_id = t.package_id) select package_id as c0 from tree"
    ))?
    .into_iter()
    .map(|r| r[0].clone())
    .collect();
    let ids = ids.join(",");
    let elements = q(format!(
        "select o.object_id as c0, coalesce(o.name,'') as c1, o.object_type as c2, coalesce(o.stereotype,'') as c3, coalesce(o.note,'') as c4, p.name as c5 \
         from t_object o join t_package p on p.package_id = o.package_id where o.package_id in ({ids}) and o.object_type not in ('Package','Note','Text','Boundary') \
         order by p.name, o.name"
    ))?;
    let relations = named_ends(q(format!(
        "select c.connector_type as c0, coalesce(c.stereotype,'') as c1, coalesce(s.name,'') as c2, coalesce(t.name,'') as c3, coalesce(c.name,'') as c4, {ENDS} \
         where s.package_id in ({ids}) and t.package_id in ({ids}) order by 3, 4"
    ))?, 2, 3);
    let mut page = String::new();
    page.push_str(&format!("<p><em>{}</em></p>", xml(&crate::i18n::fmt("Published from the model {0} by the team hub on {1}. Edit the model, not this page: the next publication replaces it.", &[model.to_string(), chrono::Local::now().format("%Y-%m-%d %H:%M").to_string()]))));
    if !root[1].trim().is_empty() {
        page.push_str(&format!("<p>{}</p>", xml(root[1].trim())));
    }
    page.push_str(&format!("<h2>{}</h2><table><tbody><tr><th>{}</th><th>{}</th><th>{}</th><th>{}</th></tr>", xml(crate::i18n::tr("Elements")),
        xml(crate::i18n::tr("name")), xml(crate::i18n::tr("type")), xml(crate::i18n::tr("package")), xml(crate::i18n::tr("description"))));
    for e in elements.iter().take(1000) {
        let kind = if e[3].is_empty() { e[2].clone() } else { format!("{} «{}»", e[2], e[3]) };
        let note: String = e[4].chars().take(600).collect();
        page.push_str(&format!("<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>", xml(&e[1]), xml(&kind), xml(&e[5]), xml(&note).replace('\n', "<br/>")));
    }
    page.push_str("</tbody></table>");
    if !relations.is_empty() {
        page.push_str(&format!("<h2>{}</h2><ul>", xml(crate::i18n::tr("Relationships"))));
        for r in relations.iter().take(1000) {
            let kind = if r[1].is_empty() { r[0].clone() } else { format!("{} «{}»", r[0], r[1]) };
            page.push_str(&format!("<li>{} → {} ({}{})</li>", xml(&r[2]), xml(&r[3]), xml(&kind), if r[4].is_empty() { String::new() } else { format!(": {}", xml(&r[4])) }));
        }
        page.push_str("</ul>");
    }
    Ok((format!("{} – {}", root[0], model), page))
}

impl Hub {
    /// The AI gateway: the hub keeps the Claude keys ("ai": {"keys": [{"for": "<user>" | "role:<role>" | "*", "secret": "<name>"}]}),
    /// Compass sends its requests here with the hub token. The first entry for the person, then for their role,
    /// then for everybody gives the key (a hub secret, e.g. LOONAR_SECRET_CLAUDE_KEY_TEAM). Usage per person goes to
    /// ai_usage.jsonl. The request is passed to Anthropic as it is.
    fn ai_gateway(&self, user: &str, role: Role, body: &[u8]) -> Result<(u16, String, Vec<u8>), HttpError> {
        let keys = self.ai["keys"].as_array().cloned().unwrap_or_default();
        let wanted = [user.to_lowercase(), format!("role:{}", role.name()), "*".to_string()];
        let secret = wanted.iter().find_map(|w| keys.iter().find(|k| k["for"].as_str().is_some_and(|f| f.trim().to_lowercase() == *w)).and_then(|k| k["secret"].as_str()))
            .ok_or_else(|| deny(403, crate::i18n::tr("the hub has no AI key for you: ask the administrator of the hub")))?;
        let key = crate::config::secrets::get(secret).map_err(|e| HttpError(500, format!("{e:#}")))?;
        // Limits ("ai": {"limits": [{"for": …, "tokens_per_day": N, "tokens_per_month": N}]}): the first entry for the
        // person, their role or everybody; input and output tokens counted from ai_usage.jsonl.
        let limits = self.ai["limits"].as_array().cloned().unwrap_or_default();
        if let Some(limit) = wanted.iter().find_map(|w| limits.iter().find(|l| l["for"].as_str().is_some_and(|f| f.trim().to_lowercase() == *w))) {
            let (day, month) = self.ai_usage_of(user);
            if limit["tokens_per_day"].as_u64().is_some_and(|max| day >= max) {
                return Err(deny(429, crate::i18n::fmt("your AI limit for today is used up ({0} tokens); it renews tomorrow or ask the administrator of the hub", &[day.to_string()])));
            }
            if limit["tokens_per_month"].as_u64().is_some_and(|max| month >= max) {
                return Err(deny(429, crate::i18n::fmt("your AI limit for this month is used up ({0} tokens); ask the administrator of the hub", &[month.to_string()])));
            }
        }
        let request: Value = serde_json::from_slice(body).map_err(|_| deny(400, "the request is not JSON"))?;
        if let Some(models) = self.ai["models"].as_array() {
            let model = request["model"].as_str().unwrap_or("");
            if !models.iter().any(|m| m.as_str() == Some(model)) {
                return Err(deny(403, crate::i18n::fmt("the model {0} is not allowed by the hub", &[model.to_string()])));
            }
        }
        let http = reqwest::blocking::Client::builder().timeout(Duration::from_secs(600)).build().map_err(|e| HttpError(500, e.to_string()))?;
        let response = http.post(crate::claude::API_URL).header("x-api-key", key).header("anthropic-version", crate::claude::API_VERSION)
            .header("content-type", "application/json").body(body.to_vec()).send().map_err(|e| deny(502, format!("Anthropic: {e}")))?;
        let status = response.status().as_u16();
        let bytes = response.bytes().map_err(|e| deny(502, format!("Anthropic: {e}")))?.to_vec();
        if let Ok(answer) = serde_json::from_slice::<Value>(&bytes) {
            let usage = json!({ "time": chrono::Local::now().to_rfc3339(), "user": user, "key": secret, "model": request["model"], "status": status,
                "input_tokens": answer["usage"]["input_tokens"], "output_tokens": answer["usage"]["output_tokens"],
                "cache_creation_input_tokens": answer["usage"]["cache_creation_input_tokens"], "cache_read_input_tokens": answer["usage"]["cache_read_input_tokens"] });
            if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(self.dir.join("ai_usage.jsonl")) {
                let _ = writeln!(f, "{usage}");
            }
        }
        Ok((status, "application/json".into(), bytes))
    }

    /// Tokens (input + output) the person used today and this month, from ai_usage.jsonl.
    fn ai_usage_of(&self, user: &str) -> (u64, u64) {
        let text = std::fs::read_to_string(self.dir.join("ai_usage.jsonl")).unwrap_or_default();
        let now = chrono::Local::now();
        let (today, this_month) = (now.format("%Y-%m-%d").to_string(), now.format("%Y-%m").to_string());
        let (mut day, mut month) = (0u64, 0u64);
        for entry in text.lines().filter_map(|l| serde_json::from_str::<Value>(l).ok()).filter(|e| e["user"].as_str().is_some_and(|u| u.eq_ignore_ascii_case(user))) {
            let time = entry["time"].as_str().unwrap_or("");
            // What the request costs, in input tokens: tokens read from the cache count a tenth, as Anthropic bills them.
            let tokens = entry["input_tokens"].as_u64().unwrap_or(0) + entry["output_tokens"].as_u64().unwrap_or(0)
                + entry["cache_creation_input_tokens"].as_u64().unwrap_or(0) + entry["cache_read_input_tokens"].as_u64().unwrap_or(0) / 10;
            if time.starts_with(&this_month) {
                month += tokens;
                if time.starts_with(&today) {
                    day += tokens;
                }
            }
        }
        (day, month)
    }

    /// The AI usage of every person, today and this month (the Users tab of the administrator).
    fn ai_usage_all(&self) -> Vec<Value> {
        let text = std::fs::read_to_string(self.dir.join("ai_usage.jsonl")).unwrap_or_default();
        let mut people: Vec<String> = text.lines().filter_map(|l| serde_json::from_str::<Value>(l).ok()).filter_map(|e| e["user"].as_str().map(String::from)).collect();
        people.sort_by_key(|p| p.to_lowercase());
        people.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
        people.iter().map(|p| {
            let (day, month) = self.ai_usage_of(p);
            let key = text.lines().rev().filter_map(|l| serde_json::from_str::<Value>(l).ok()).find(|e| e["user"].as_str().is_some_and(|u| u.eq_ignore_ascii_case(p))).and_then(|e| e["key"].as_str().map(String::from)).unwrap_or_default();
            json!({ "user": p, "tokens_today": day, "tokens_month": month, "key": key })
        }).collect()
    }

    /// Jira or Confluence through the hub, with the team's account from "integrations": only their REST API, viewers
    /// only read, every change is in the audit trail with the person who made it.
    #[allow(clippy::too_many_arguments)]
    fn service_proxy(&self, kind: &str, user: &str, role: Role, method: &str, rest: &str, query: &HashMap<String, String>, body: &[u8], content_type: &str) -> Result<(u16, String, Vec<u8>), HttpError> {
        let s = &self.integrations[kind];
        let base = s["url"].as_str().unwrap_or("").trim().trim_end_matches('/').to_string();
        if base.is_empty() {
            return Err(deny(404, crate::i18n::fmt("the hub has no {0}: the administrator sets \"integrations\" in hub.json", &[kind.to_string()])));
        }
        if !rest.starts_with("/rest/") || rest.contains("..") {
            return Err(deny(403, "only the REST API goes through the hub"));
        }
        if method != "GET" && role == Role::Viewer {
            return Err(deny(403, crate::i18n::tr("a viewer only reads")));
        }
        let secret = if kind == "jira" { crate::config::secrets::JIRA_TOKEN } else { crate::config::secrets::CONFLUENCE_TOKEN };
        let token = crate::config::secrets::get(secret).map_err(|e| HttpError(500, format!("{e:#}")))?;
        let mut url = reqwest::Url::parse(&format!("{base}{rest}")).map_err(|_| deny(400, "invalid address"))?;
        for (k, v) in query {
            url.query_pairs_mut().append_pair(k, v);
        }
        let http = reqwest::blocking::Client::builder().timeout(Duration::from_secs(60)).build().map_err(|e| HttpError(500, e.to_string()))?;
        let method_value = reqwest::Method::from_bytes(method.as_bytes()).map_err(|_| deny(405, "method"))?;
        let mut request = http.request(method_value, url).header("Accept", "application/json").header("X-Atlassian-Token", "no-check");
        let account = s["user"].as_str().unwrap_or("").trim();
        request = if account.is_empty() { request.bearer_auth(&token) } else { request.basic_auth(account, Some(&token)) };
        if !body.is_empty() {
            request = request.header("Content-Type", if content_type.is_empty() { "application/json" } else { content_type }).body(body.to_vec());
        }
        let response = request.send().map_err(|e| deny(502, format!("{kind}: {e}")))?;
        let status = response.status().as_u16();
        let response_type = response.headers().get(reqwest::header::CONTENT_TYPE).and_then(|c| c.to_str().ok()).unwrap_or("application/json").to_string();
        let bytes = response.bytes().map_err(|e| deny(502, format!("{kind}: {e}")))?.to_vec();
        if method != "GET" {
            let _ = self.append_audit(&json!({ "time": chrono::Local::now().to_rfc3339(), "user": user, "source": "", "summary": format!("{kind}: {method} {rest}"), "ok": status < 400 }));
        }
        Ok((status, response_type, bytes))
    }

    /// The team's Jira from hub.json ("integrations": {"jira": {"url", "user", "project", "issue_type"}}); token: secret jira_token.
    fn create_issue(&self, summary: &str, description: &str) -> Result<(String, String), HttpError> {
        let j = &self.integrations["jira"];
        let url = j["url"].as_str().unwrap_or("").trim().to_string();
        let project = j["project"].as_str().unwrap_or("").trim().to_string();
        if url.is_empty() || project.is_empty() {
            return Err(deny(400, crate::i18n::tr("the hub has no Jira: the administrator sets \"integrations\" in hub.json")));
        }
        crate::jira::check_base_url(&url).map_err(|e| deny(500, e.to_string()))?;
        let token = crate::config::secrets::get(crate::config::secrets::JIRA_TOKEN).map_err(|e| HttpError(500, format!("{e:#}")))?;
        let connection = crate::jira::Connection { base_url: url, user: j["user"].as_str().unwrap_or("").trim().to_string(), token };
        let labels = vec!["compass-hub".to_string()];
        connection
            .create_issue(&project, j["issue_type"].as_str().unwrap_or("Task"), summary, description, &labels, None)
            .map_err(|e| deny(502, format!("Jira: {e:#}")))
    }

    /// The team's Confluence ("integrations": {"confluence": {"url", "user", "space", "parent"}}); token: secret confluence_token.
    fn confluence(&self) -> Result<(crate::confluence::Connection, String, Option<String>), HttpError> {
        let c = &self.integrations["confluence"];
        let url = c["url"].as_str().unwrap_or("").trim().to_string();
        let space = c["space"].as_str().unwrap_or("").trim().to_string();
        if url.is_empty() || space.is_empty() {
            return Err(deny(400, crate::i18n::tr("the hub has no Confluence: the administrator sets \"integrations\" in hub.json")));
        }
        crate::confluence::check_base_url(&url).map_err(|e| deny(500, e.to_string()))?;
        let token = crate::config::secrets::get(crate::config::secrets::CONFLUENCE_TOKEN).map_err(|e| HttpError(500, format!("{e:#}")))?;
        let parent = c["parent"].as_str().map(|p| p.trim().to_string()).filter(|p| !p.is_empty());
        Ok((crate::confluence::Connection { base_url: url, user: c["user"].as_str().unwrap_or("").trim().to_string(), token }, space, parent))
    }
}

fn element_details(cfg: &AppConfig, id: i64) -> Result<String> {
    let q = |sql: String| crate::tools::query_rows(cfg, &sql);
    let row = q(format!(
        "select o.object_id as c0, o.name as c1, o.object_type as c2, coalesce(o.stereotype,'') as c3, coalesce(o.alias,'') as c4, coalesce(o.status,'') as c5, coalesce(o.author,'') as c6, p.name as c7, coalesce(o.note,'') as c8, o.ea_guid as c9 \
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
    let tags = q(format!("select property as c0, coalesce(value,'') as c1 from t_objectproperties where object_id = {id} order by property"))?;
    if !tags.is_empty() {
        out.push(format!("\n{}:", crate::i18n::tr("Tagged values")));
        out.extend(tags.into_iter().map(|t| format!("- {} = {}", t[0], t[1])));
    }
    let links = q(format!(
        "select c.connector_type as c0, case when c.start_object_id = {id} then '→' else '←' end as c1, o.object_id as c2, o.name as c3 from t_connector c \
         join t_object o on o.object_id = case when c.start_object_id = {id} then c.end_object_id else c.start_object_id end \
         where c.start_object_id = {id} or c.end_object_id = {id} order by 1, 4"
    ))?;
    if !links.is_empty() {
        out.push(format!("\n{}:", crate::i18n::tr("Relationships")));
        out.extend(links.into_iter().map(|l| format!("- {} {} {} (id {})", l[0], l[1], l[3], l[2])));
    }
    let diagrams = q(format!("select d.diagram_id as c0, d.name as c1 from t_diagramobjects x join t_diagram d on d.diagram_id = x.diagram_id where x.object_id = {id} order by d.name"))?;
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
        { "name": "list_sources", "description": "The team's EA models available in the hub.", "inputSchema": { "type": "object", "properties": {} }, "annotations": { "readOnlyHint": true } },
        { "name": "list_packages", "description": "Package tree of a model (package_id | parent_id | name).", "inputSchema": { "type": "object", "properties": { "source": source }, "required": ["source"] }, "annotations": { "readOnlyHint": true } },
        { "name": "search_elements", "description": "Finds elements by name or alias (part of the text).", "inputSchema": { "type": "object", "properties": { "source": source, "query": { "type": "string" } }, "required": ["source", "query"] }, "annotations": { "readOnlyHint": true } },
        { "name": "element_details", "description": "An element with its description, tagged values, relationships and diagrams.", "inputSchema": { "type": "object", "properties": { "source": source, "element": { "type": "integer" } }, "required": ["source", "element"] }, "annotations": { "readOnlyHint": true } },
        { "name": "impact_analysis", "description": "What depends on an element and what it affects, level by level along relationships.", "inputSchema": { "type": "object", "properties": { "source": source, "element": { "type": "integer" }, "direction": { "type": "string", "enum": ["in", "out", "both"] }, "depth": { "type": "integer" } }, "required": ["source", "element"] }, "annotations": { "readOnlyHint": true } },
        { "name": "model_lint", "description": "Model quality check of a package by the team rules (orphans, default names, missing descriptions, disallowed relationships…).", "inputSchema": { "type": "object", "properties": { "source": source, "package": { "type": "integer" } }, "required": ["source", "package"] }, "annotations": { "readOnlyHint": true } },
        { "name": "gap_analysis", "description": "Gaps between two packages (as-is vs to-be).", "inputSchema": { "type": "object", "properties": { "source": source, "as_is": { "type": "integer" }, "to_be": { "type": "integer" } }, "required": ["source", "as_is", "to_be"] }, "annotations": { "readOnlyHint": true } }
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

/// After a new sign-in: the next question asks the hub again.
pub fn forget_policy() {
    *POLICY.lock().unwrap_or_else(|p| p.into_inner()) = None;
}

/// URL encoding and decoding of one value, for the sign-in of Compass.
pub fn encode_value(text: &str) -> String {
    encode(text)
}

pub fn url_decode_value(text: &str) -> String {
    url_decode(text)
}

pub fn note_failure(message: &str) {
    *LAST_FAILURE.lock().unwrap() = Some(message.to_string());
}

pub fn last_failure() -> Option<String> {
    LAST_FAILURE.lock().unwrap().clone()
}

/// Web access as the team policy allows it: Err(reason) when the hub's policy has "web_access": false, otherwise the
/// domains to allow (the team's "web_allowed_domains" replace the user's own list).
pub fn web_access(cfg: &AppConfig) -> Result<Vec<String>, String> {
    let Some(policy) = team_policy(cfg) else {
        return Ok(cfg.web_domains.clone());
    };
    if policy["web_access"] == false {
        return Err(crate::i18n::tr("the team policy of the hub switches web access off").into());
    }
    match policy["web_allowed_domains"].as_array() {
        Some(list) => Ok(list.iter().filter_map(|d| d.as_str()).map(|d| d.trim().to_lowercase()).filter(|d| !d.is_empty()).collect()),
        None => Ok(cfg.web_domains.clone()),
    }
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
            let _ = std::fs::write(cfg.base_dir.join("hub_policy_cache.json"), json!({ "url": client.url, "time": chrono::Utc::now().to_rfc3339(), "policy": policy }).to_string());
            Some(policy)
        }
        Err(e) => {
            note_failure(&format!("{e:#}"));
            cached_policy(cfg, &client.url)
        }
    }
}

/// The team policy already received from the hub of the settings, without asking the hub (for the window).
pub fn known_team_policy(cfg: &AppConfig) -> Option<Value> {
    let url = cfg.hub_url.trim().trim_end_matches('/');
    if url.is_empty() {
        return None;
    }
    let known = POLICY.lock().unwrap_or_else(|p| p.into_inner()).as_ref().filter(|(_, u, _)| u.trim_end_matches('/') == url).map(|(_, _, p)| p.clone());
    known.or_else(|| cached_policy(cfg, url))
}

/// The last policy received from this hub, for at most a week (the hub may be down for a while).
fn cached_policy(cfg: &AppConfig, url: &str) -> Option<Value> {
    let cache: Value = serde_json::from_str(&std::fs::read_to_string(cfg.base_dir.join("hub_policy_cache.json")).ok()?).ok()?;
    let time = chrono::DateTime::parse_from_rfc3339(cache["time"].as_str()?).ok()?;
    (cache["url"] == url && chrono::Utc::now().signed_duration_since(time).num_days() < 7).then(|| cache["policy"].clone())
}

/// With the computer rule RequireHub: the assistant works only connected to the hub and with its policy.
/// Err is the message for the user (what to do).
pub fn required(cfg: &AppConfig) -> Result<(), String> {
    if !cfg.policy.require_hub {
        return Ok(());
    }
    if cfg.hub_url.trim().is_empty() {
        return Err(crate::i18n::tr("Your organisation requires the team hub, but no hub address is set: ask IT for the address of the hub.").into());
    }
    if !crate::config::secrets::exists(crate::config::secrets::HUB_TOKEN) {
        return Err(crate::i18n::tr("Your organisation requires the team hub: sign in to it in Settings → Team (Compass Hub) → Sign in with your company account.").into());
    }
    match team_policy(cfg) {
        Some(_) => Ok(()),
        None => Err(crate::i18n::fmt("Your organisation requires the team hub, and it cannot be reached or your sign-in has expired: sign in again in Settings → Team (Compass Hub). Problem: {0}", &[last_failure().unwrap_or_default()])),
    }
}

/// The hub address and token when the team policy routes this service through the hub ("ai_via_hub",
/// "jira_via_hub", "confluence_via_hub"): Compass then uses the team's key or account kept on the hub.
pub fn via_hub(cfg: &AppConfig, service: &str) -> Option<(String, String)> {
    let policy = team_policy(cfg)?;
    if policy[format!("{service}_via_hub").as_str()] != true {
        return None;
    }
    let client = Client::from_config(cfg)?;
    Some((client.url, client.token))
}

/// A default the team policy gives (e.g. "jira_project", "confluence_space").
pub fn team_default(cfg: &AppConfig, key: &str) -> Option<String> {
    team_policy(cfg)?[key].as_str().map(str::trim).filter(|v| !v.is_empty()).map(String::from)
}

/// The strictest minimum of the computer's policy and the team policy already known (no request to the hub: for the
/// window, which must not wait for the network).
pub fn known_min_mode(cfg: &AppConfig) -> Option<crate::config::ApprovalMode> {
    let team = POLICY.lock().unwrap_or_else(|p| p.into_inner()).as_ref().filter(|(_, url, _)| url.trim_end_matches('/') == cfg.hub_url.trim_end_matches('/'))
        .and_then(|(_, _, p)| p["min_approval"].as_str().and_then(|m| crate::config::ApprovalMode::parse(m).ok()));
    [cfg.policy.min_approval, team].into_iter().flatten().max_by_key(|m| m.strictness())
}

/// The approval mode in force: the one from the settings, made stricter by the team policy ("min_approval").
pub fn effective_mode(cfg: &AppConfig) -> crate::config::ApprovalMode {
    // The settings already carry the computer's minimum (MachinePolicy::apply).
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
        #[cfg(test)]
        if let Ok(token) = std::env::var("LOONAR_TEST_HUB_CLIENT_TOKEN") {
            return Some(Client { url: cfg.hub_url.trim().trim_end_matches('/').to_string(), token });
        }
        let token = crate::config::secrets::get(crate::config::secrets::HUB_TOKEN).ok()?;
        Some(Client { url: cfg.hub_url.trim().trim_end_matches('/').to_string(), token })
    }

    fn call(&self, method: reqwest::Method, path: &str, body: Option<&Value>) -> Result<Value> {
        crate::jira::check_base_url(&self.url).map_err(|_| anyhow!("{}", crate::i18n::tr("The hub address must start with https:// (http:// only on this computer)")))?;
        let client = reqwest::blocking::Client::builder().redirect(crate::net::same_origin_redirects()).timeout(Duration::from_secs(20)).build()?;
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
        Ok(self.call(reqwest::Method::GET, &format!("/api/notes/{}", encode(name)), None)?["text"].as_str().unwrap_or("").to_string())
    }

    pub fn propose(&self, change: &Value) -> Result<String> {
        Ok(self.call(reqwest::Method::POST, "/api/changes", Some(change))?["id"].as_str().unwrap_or("").to_string())
    }

    pub fn changes(&self, status: Option<&str>) -> Result<Vec<Value>> {
        let path = match status { Some(s) => format!("/api/changes?status={}", encode(s)), None => "/api/changes".into() };
        Ok(self.call(reqwest::Method::GET, &path, None)?.as_array().cloned().unwrap_or_default())
    }

    pub fn change(&self, id: &str) -> Result<Value> {
        self.call(reqwest::Method::GET, &format!("/api/changes/{}", change_id(id)?), None)
    }

    pub fn change_action(&self, id: &str, action: &str, body: &Value) -> Result<Value> {
        if !matches!(action, "comment" | "decision" | "claim" | "applied" | "reset") {
            bail!("unknown change set action {action}");
        }
        self.call(reqwest::Method::POST, &format!("/api/changes/{}/{action}", change_id(id)?), Some(body))
    }

    pub fn save_note(&self, name: &str, text: &str) -> Result<()> {
        self.call(reqwest::Method::PUT, &format!("/api/notes/{}", encode(name)), Some(&json!({ "text": text }))).map(|_| ())
    }
}

/// Percent-encoding of a path segment or a query value.
fn encode(text: &str) -> String {
    text.bytes().map(|b| if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') { (b as char).to_string() } else { format!("%{b:02X}") }).collect()
}

/// A change set id as the hub gives it (CH-…: letters, digits and '-').
fn change_id(id: &str) -> Result<&str> {
    let id = id.trim();
    if id.is_empty() || id.len() > 40 || !id.starts_with("CH-") || !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        bail!("{}", crate::i18n::fmt("there is no change set {0}", &[id.to_string()]));
    }
    Ok(id)
}

/// The web view: one page, the token is kept in the browser (localStorage) and sent as a Bearer header.
const WEB_PAGE: &str = include_str!("hub_web.html");

#[cfg(test)]
mod tests {
    use super::*;

    /// Unnamed ends of relationships (e.g. lifelines) get the classifier's name or their type and id.
    #[test]
    fn unnamed_relationship_ends() {
        crate::i18n::set(crate::i18n::Lang::En);
        let row = |s: &str, sc: &str, t: &str, tc: &str| -> Vec<String> {
            [s, "Sequence", "", t, "m()", "Object", "7", sc, "Object", "9", tc].iter().map(|v| v.to_string()).collect()
        };
        let rows = named_ends(vec![row("", "Customer", "", ""), row("A", "", "B", "X")], 0, 3);
        assert_eq!(rows[0], vec![":Customer", "Sequence", "", "(unnamed Object #9)", "m()"]);
        assert_eq!(rows[1], vec!["A", "Sequence", "", "B", "m()"]);
    }

    #[test]
    fn roles_and_names() {
        assert!(Role::Admin > Role::Architect && Role::Architect > Role::Viewer);
        assert_eq!(Role::parse("ADMIN"), Role::Admin);
        assert_eq!(Role::parse("whatever"), Role::Viewer);
        assert!(note_name("Team conventions").is_ok());
        assert!(note_name("../secret").is_err());
        assert!(note_name(".hidden").is_err());
        assert_eq!(url_decode("a%20b+c%C5%82"), "a b cł");
        assert_eq!(url_decode("/%1é"), "/%1é", "no panic on a multi-byte character after %");
        assert_eq!(url_decode("%€%"), "%€%");
        assert_eq!(url_decode("%4"), "%4");
        assert!(note_name("COM1").is_err() && note_name("nul.txt").is_err() && note_name("Console").is_ok());
        assert_eq!(encode("a b/ł"), "a%20b%2F%C5%82");
        assert!(change_id("CH-20261023-101010123").is_ok() && change_id("../policy").is_err());
        let (token, hash) = new_token();
        assert!(token.len() > 60 && hex(&token_hash(&token)) == hash);
    }

    /// The hub with two data sources (a PostgreSQL database and an EA project file, from LOONAR_HUB_DB_PROPS and
    /// LOONAR_HUB_FILE_PROPS): roles, policy, audit, notes, the web page, the REST API and MCP on both sources.
    /// Run with: `cargo test hub_two_sources -- --ignored --nocapture`.
    /// Behind a proxy on this computer the client is the last X-Forwarded-For address (wrong sign-ins of one client do
    /// not lock out the others), and a wrong sign-in is in the audit trail with ✗ and that address. The header from
    /// another computer is not trusted (the test can only connect from loopback, so it checks the trusted case).
    #[test]
    fn sign_in_failures_behind_proxy() {
        crate::i18n::set(crate::i18n::Lang::En);
        let dir = std::env::temp_dir().join(format!("loonar-hub-proxy-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("hub.json"), json!({ "listen": "127.0.0.1:0", "sources": [] }).to_string()).unwrap();
        let (hub, listen) = load(&dir.join("hub.json")).unwrap();
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || serve(hub, &listen, |a| tx.send(a).unwrap()).unwrap());
        let base = format!("http://{}", rx.recv().unwrap());
        let http = reqwest::blocking::Client::new();
        let login = |client: &str, password: &str| {
            http.post(format!("{base}/api/login")).header("X-Forwarded-For", format!("203.0.113.9, {client}"))
                .json(&json!({ "name": "admin", "password": password })).send().unwrap().status().as_u16()
        };
        for _ in 0..MAX_FAILURES {
            assert_eq!(login("10.0.0.1", "wrong"), 401);
        }
        assert_eq!(login("10.0.0.1", "wrong"), 429, "the client that guessed is locked out");
        assert_eq!(login("10.0.0.2", "passw0rd"), 200, "another client behind the same proxy is not");
        let audit = std::fs::read_to_string(dir.join("audit.jsonl")).unwrap();
        let failed: Vec<Value> = audit.lines().filter_map(|l| serde_json::from_str(l).ok()).filter(|e: &Value| e["ok"] == false).collect();
        assert!(failed.len() > MAX_FAILURES, "wrong sign-ins and the lock-out are in the audit trail: {audit}");
        assert!(failed.iter().all(|e| e["user"] == "admin" && e["error"].as_str().unwrap().contains("(10.0.0.1)")), "{audit}");
        assert_eq!(forwarded_address("10.1.2.3:51234"), Some("10.1.2.3".parse().unwrap()), "IIS/ARR adds the port by default");
        assert_eq!(forwarded_address("[2001:db8::1]:51234"), Some("2001:db8::1".parse().unwrap()));
        assert_eq!(forwarded_address("2001:db8::1"), Some("2001:db8::1".parse().unwrap()));
        assert_eq!(forwarded_address("unknown"), None);
    }

    /// The AI gateway and the Jira proxy with real services: `LOONAR_SECRET_CLAUDE_KEY_TEAM=… LOONAR_SECRET_JIRA_TOKEN=…
    /// LOONAR_TEST_JIRA_URL=… LOONAR_TEST_JIRA_USER=… cargo test hub_gateways -- --ignored --nocapture`
    #[test]
    #[ignore = "requires a Claude key and a Jira account"]
    fn hub_gateways() {
        crate::i18n::set(crate::i18n::Lang::En);
        crate::config::secrets::use_environment();
        let dir = std::env::temp_dir().join(format!("loonar-hub-gw-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("hub.json"), json!({ "listen": "127.0.0.1:0", "sources": [],
            "users": [{ "name": "anna", "token": "anna-token-0123456789", "role": "architect" }, { "name": "bartek", "token": "bartek-token-0123456789", "role": "viewer" }],
            "integrations": { "jira": { "url": std::env::var("LOONAR_TEST_JIRA_URL").unwrap(), "user": std::env::var("LOONAR_TEST_JIRA_USER").unwrap(), "project": "LC" } },
            "ai": { "keys": [{ "for": "role:architect", "secret": "claude_key_team" }], "limits": [{ "for": "anna", "tokens_per_day": 1 }] } }).to_string()).unwrap();
        let (hub, listen) = load(&dir.join("hub.json")).unwrap();
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || serve(hub, &listen, |a| tx.send(a).unwrap()).unwrap());
        let base = format!("http://{}", rx.recv().unwrap());
        let http = reqwest::blocking::Client::new();
        let ask = |key: &str| http.post(format!("{base}/ai/v1/messages")).header("x-api-key", key).header("anthropic-version", "2023-06-01")
            .json(&json!({ "model": "claude-haiku-5-5", "max_tokens": 50, "messages": [{ "role": "user", "content": "Say OK." }] })).send().unwrap();
        let first = ask("anna-token-0123456789");
        assert_eq!(first.status(), 200, "{}", first.text().unwrap_or_default());
        assert_eq!(ask("anna-token-0123456789").status(), 429, "the daily limit of 1 token is used up");
        assert_eq!(ask("bartek-token-0123456789").status(), 403, "no key for a viewer");
        assert_eq!(ask("wrong-token").status(), 401);
        let usage = std::fs::read_to_string(dir.join("ai_usage.jsonl")).unwrap();
        assert!(usage.contains("\"user\":\"anna\""), "{usage}");
        // Jira through the hub: reading works for everybody, writing not for a viewer, nothing outside the REST API.
        let me = http.get(format!("{base}/proxy/jira/rest/api/3/myself")).bearer_auth("bartek-token-0123456789").send().unwrap();
        assert_eq!(me.status(), 200);
        assert_eq!(http.post(format!("{base}/proxy/jira/rest/api/3/issue")).bearer_auth("bartek-token-0123456789").json(&json!({})).send().unwrap().status(), 403);
        assert_eq!(http.get(format!("{base}/proxy/jira/secure/Dashboard.jspa")).bearer_auth("anna-token-0123456789").send().unwrap().status(), 403);
        // Compass reads the team's project from the policy.
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Compass's sign-in may return only to its own one-time address on the loopback.
    #[test]
    fn compass_return_address() {
        assert!(compass_return("http://127.0.0.1:53211/compass-callback"));
        assert!(compass_return("http://localhost:53211/compass-callback"));
        for bad in ["https://127.0.0.1:1/compass-callback", "http://evil.example/compass-callback", "http://127.0.0.1:1/other", "http://127.0.0.1:1/compass-callback?x=1", "http://127.0.0.1.evil.example/compass-callback"] {
            assert!(!compass_return(bad), "{bad}");
        }
    }

    /// A new hub gets the default account admin / passw0rd, which can do nothing until its password is changed; the
    /// admin manages users and API tokens; wrong passwords are counted.
    #[test]
    fn hub_accounts() {
        crate::i18n::set(crate::i18n::Lang::En);
        assert!(password_ok(&password_hash("Correct horse 1"), "Correct horse 1") && !password_ok(&password_hash("Correct horse 1"), "correct horse 1"));
        let dir = std::env::temp_dir().join(format!("loonar-hub-accounts-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("hub.json"), json!({ "listen": "127.0.0.1:0", "sources": [] }).to_string()).unwrap();
        let (hub, listen) = load(&dir.join("hub.json")).unwrap();
        let saved: Value = serde_json::from_str(&std::fs::read_to_string(dir.join("hub.json")).unwrap()).unwrap();
        assert_eq!(saved["users"][0]["name"], "admin");
        assert!(saved["users"][0]["password_pbkdf2"].as_str().unwrap().starts_with("pbkdf2-sha256$") && saved["users"][0]["must_change"] == true, "only the hash is saved");
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || serve(hub, &listen, |a| tx.send(a).unwrap()).unwrap());
        let base = format!("http://{}", rx.recv().unwrap());
        let http = reqwest::blocking::Client::new();
        let call = |method: reqwest::Method, path: &str, token: &str, body: Value| {
            let r = http.request(method, format!("{base}{path}")).bearer_auth(token).json(&body).send().unwrap();
            (r.status().as_u16(), r.json::<Value>().unwrap_or(Value::Null))
        };
        let login = |name: &str, password: &str| call(reqwest::Method::POST, "/api/login", "", json!({ "name": name, "password": password }));
        assert_eq!(login("admin", "wrong").0, 401);
        let (status, session) = login("admin", "passw0rd");
        assert_eq!(status, 200, "{session}");
        assert_eq!(session["must_change"], true);
        let s = session["token"].as_str().unwrap().to_string();
        assert_eq!(call(reqwest::Method::GET, "/api/sources", &s, Value::Null).0, 403, "nothing before the password is changed");
        assert_eq!(call(reqwest::Method::GET, "/api/me", &s, Value::Null).1["must_change"], true);
        assert_eq!(call(reqwest::Method::POST, "/api/password", &s, json!({ "old": "passw0rd", "new": "short" })).0, 400);
        assert_eq!(call(reqwest::Method::POST, "/api/password", &s, json!({ "old": "passw0rd", "new": "passw0rd" })).0, 400, "not the default one");
        assert_eq!(call(reqwest::Method::POST, "/api/password", &s, json!({ "old": "nope", "new": "Team hub 2026!" })).0, 403);
        assert_eq!(call(reqwest::Method::POST, "/api/password", &s, json!({ "old": "passw0rd", "new": "Team hub 2026!" })).0, 200);
        assert_eq!(call(reqwest::Method::GET, "/api/sources", &s, Value::Null).0, 200, "after the change the session works");
        assert_eq!(login("admin", "passw0rd").0, 401, "the default password no longer works");
        // Users: an architect with a first password and an API token.
        assert_eq!(call(reqwest::Method::POST, "/api/users", &s, json!({ "name": "ola", "role": "architect", "password": "First pass 123" })).0, 201);
        assert_eq!(call(reqwest::Method::POST, "/api/users", &s, json!({ "name": "OLA", "role": "viewer", "password": "First pass 123" })).0, 409);
        let (_, token) = call(reqwest::Method::POST, "/api/users/ola/token", &s, Value::Null);
        let api = token["token"].as_str().unwrap().to_string();
        assert_eq!(call(reqwest::Method::GET, "/api/me", &api, Value::Null).1["name"], "ola", "the API token works");
        let (_, ola) = login("ola", "First pass 123");
        assert_eq!(ola["must_change"], true, "a password set by the admin must be changed");
        assert_eq!(call(reqwest::Method::GET, "/api/users", &api, Value::Null).0, 403, "an architect does not manage users");
        assert_eq!(call(reqwest::Method::DELETE, "/api/users/admin", &s, Value::Null).0, 400, "not oneself");
        assert_eq!(call(reqwest::Method::PUT, "/api/users/admin", &s, json!({ "role": "viewer" })).0, 400, "the last admin stays");
        let saved = std::fs::read_to_string(dir.join("hub.json")).unwrap();
        assert!(!saved.contains("First pass 123") && !saved.contains(&api) && !saved.contains("Team hub 2026!"), "no password or token in hub.json");
        assert_eq!(call(reqwest::Method::DELETE, "/api/users/ola", &s, Value::Null).0, 200);
        assert_eq!(call(reqwest::Method::GET, "/api/me", &api, Value::Null).0, 401, "the token of a removed user stops working");
        assert_eq!(call(reqwest::Method::POST, "/api/logout", &s, Value::Null).0, 200);
        assert_eq!(call(reqwest::Method::GET, "/api/me", &s, Value::Null).0, 401, "signed out");
        for _ in 0..20 {
            login("admin", "guess");
        }
        assert_eq!(login("admin", "Team hub 2026!").0, 429, "too many wrong passwords from this address");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Jira and Confluence as actions of an architect in the hub (imitations on localhost): a viewer cannot, an
    /// architect creates an issue for an element and for a change set and publishes a package page.
    /// `cargo test hub_integrations -- --ignored --nocapture` with LOONAR_HUB_FILE_PROPS.
    #[test]
    #[ignore = "requires a test model (LOONAR_HUB_FILE_PROPS)"]
    fn hub_integrations() {
        crate::i18n::set(crate::i18n::Lang::En);
        crate::config::secrets::use_environment();
        std::env::set_var("LOONAR_SECRET_JIRA_TOKEN", "jira-test-token");
        std::env::set_var("LOONAR_SECRET_CONFLUENCE_TOKEN", "confluence-test-token");
        let jira = crate::jira::mock::start(false, "jira-test-token");
        // Confluence imitation: no page yet, the created page gets id 777.
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let confluence_url = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
        let published: Arc<Mutex<Vec<String>>> = Default::default();
        let log = published.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { continue };
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut first = String::new();
                let _ = reader.read_line(&mut first);
                let mut length = 0;
                loop {
                    let mut line = String::new();
                    let _ = reader.read_line(&mut line);
                    if line.trim_end().is_empty() {
                        break;
                    }
                    if let Some(v) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                        length = v.trim().parse().unwrap_or(0);
                    }
                }
                let mut body = vec![0u8; length];
                let _ = reader.read_exact(&mut body);
                let answer = if first.starts_with("POST") {
                    log.lock().unwrap().push(String::from_utf8_lossy(&body).to_string());
                    r#"{"id":"777","_links":{"webui":"/pages/777"}}"#
                } else {
                    r#"{"results":[]}"#
                };
                let _ = write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{answer}", answer.len());
            }
        });
        let dir = std::env::temp_dir().join(format!("loonar-hub-int-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("hub.json"), json!({ "listen": "127.0.0.1:0",
            "sources": [{ "id": "shop", "name": "Shop", "properties": std::env::var("LOONAR_HUB_FILE_PROPS").unwrap() }],
            "users": [{ "name": "anna", "token": "architect-token-0123456789", "role": "architect" }, { "name": "olek", "token": "viewer-token-0123456789", "role": "viewer" }],
            "integrations": { "jira": { "url": jira.url, "project": "ARCH" }, "confluence": { "url": confluence_url, "space": "ARCH" } } }).to_string()).unwrap();
        let (hub, listen) = load(&dir.join("hub.json")).unwrap();
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || serve(hub, &listen, |a| tx.send(a).unwrap()).unwrap());
        let base = format!("http://{}", rx.recv().unwrap());
        let http = reqwest::blocking::Client::new();
        let post = |path: &str, token: &str, body: Value| {
            let r = http.post(format!("{base}{path}")).bearer_auth(token).json(&body).send().unwrap();
            (r.status().as_u16(), r.json::<Value>().unwrap_or(Value::Null))
        };
        let (architect, viewer) = ("architect-token-0123456789", "viewer-token-0123456789");
        let integrations: Value = http.get(format!("{base}/api/integrations")).bearer_auth(viewer).send().unwrap().json().unwrap();
        assert_eq!(integrations, json!({ "jira": true, "confluence": true }));
        let cfg = AppConfig::load(std::path::Path::new(&std::env::var("LOONAR_HUB_FILE_PROPS").unwrap())).unwrap();
        let element: i64 = crate::tools::query_rows(&cfg, "select min(object_id) as c0 from t_object where object_type = 'Class'").unwrap()[0][0].parse().unwrap();
        let package: i64 = crate::tools::query_rows(&cfg, &format!("select package_id as c0 from t_object where object_id = {element}")).map(|r| r[0][0].parse().unwrap()).unwrap();
        assert_eq!(post("/api/sources/shop/jira", viewer, json!({ "summary": "Check it", "element": element })).0, 403, "a viewer cannot");
        let (status, issue) = post("/api/sources/shop/jira", architect, json!({ "summary": "Check the element", "element": element }));
        assert_eq!(status, 201, "{issue}");
        assert!(issue["key"].as_str().unwrap().starts_with("ARCH-"), "{issue}");
        let sent = jira.requests.lock().unwrap().iter().find(|(m, p, _)| m == "POST" && p.contains("/issue")).cloned().unwrap();
        assert!(sent.2.to_string().contains("compass-hub") && sent.2.to_string().contains("Element of the model Shop"), "{}", sent.2);
        assert_eq!(post("/api/sources/shop/jira", architect, json!({ "summary": "x" })).0, 400, "a summary is needed");
        let created: Value = http.post(format!("{base}/api/changes")).bearer_auth(architect).json(&json!({ "source": "shop", "title": "Rename", "ops": [{ "op": "update_element", "element": element, "name": "Renamed" }] })).send().unwrap().json().unwrap();
        let id = created["id"].as_str().unwrap().to_string();
        assert_eq!(post(&format!("/api/changes/{id}/jira"), architect, json!({ "summary": "Review the rename" })).0, 201);
        assert_eq!(post(&format!("/api/changes/{id}/jira"), architect, json!({ "summary": "Again" })).0, 409, "one issue per change set");
        let (status, page) = post("/api/sources/shop/confluence", architect, json!({ "package": package }));
        assert_eq!(status, 201, "{page}");
        assert!(page["url"].as_str().unwrap().ends_with("/pages/777") && page["created"] == true, "{page}");
        let body = published.lock().unwrap().last().cloned().unwrap();
        assert!(body.contains("<table>") && body.contains("Published from the model Shop"), "{body}");
        let audit: Value = http.get(format!("{base}/api/audit?limit=10")).bearer_auth(viewer).send().unwrap().json().unwrap();
        assert!(audit.as_array().unwrap().iter().filter(|e| e["summary"].as_str().is_some_and(|s| s.starts_with("Jira") || s.starts_with("Confluence"))).count() >= 3, "{audit}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Company sign-in end to end against a local Keycloak (scratchpad kc/compass-realm.json):
    /// `docker run -p 127.0.0.1:8180:8080 … quay.io/keycloak/keycloak:26.0 start-dev --import-realm`, then
    /// `LOONAR_HUB_FILE_PROPS=… cargo test hub_oidc_keycloak -- --ignored --nocapture`
    #[test]
    #[ignore = "requires a local Keycloak and a test model"]
    fn hub_oidc_keycloak() {
        crate::i18n::set(crate::i18n::Lang::En);
        crate::config::secrets::use_environment();
        std::env::set_var("LOONAR_SECRET_OIDC_CLIENT_SECRET", "lab-test-client-secret-5f1c9e");
        let dir = std::env::temp_dir().join(format!("loonar-hub-oidc-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("hub.json"), json!({ "listen": "127.0.0.1:18731",
            "sources": [{ "id": "shop", "name": "Shop", "properties": std::env::var("LOONAR_HUB_FILE_PROPS").unwrap() }],
            "users": [{ "name": "admin", "token": "admin-token-0123456789", "role": "admin" }],
            "oidc": { "issuer": "http://127.0.0.1:8180/realms/compass", "client_id": "compass-hub", "redirect_uri": "http://127.0.0.1:18731/auth/callback" } }).to_string()).unwrap();
        let (hub, listen) = load(&dir.join("hub.json")).unwrap();
        std::thread::spawn(move || serve(hub, &listen, |_| {}));
        std::thread::sleep(Duration::from_millis(300));
        let http = reqwest::blocking::Client::builder().redirect(reqwest::redirect::Policy::none()).build().unwrap();
        let base = "http://127.0.0.1:18731";
        assert_eq!(http.get(format!("{base}/api/auth")).send().unwrap().json::<Value>().unwrap()["oidc"], true);

        // A browser: the hub sends it to Keycloak, the login form is filled in, Keycloak sends it back with a code.
        let sign_in_at = |login: &str, user: &str, password: &str| -> reqwest::blocking::Response {
            let to_provider = http.get(format!("{base}{login}")).send().unwrap();
            assert_eq!(to_provider.status(), 302);
            let url = to_provider.headers()["location"].to_str().unwrap().to_string();
            assert!(url.contains("code_challenge_method=S256") && url.contains("nonce="));
            let page = http.get(&url).send().unwrap();
            let cookies: Vec<String> = page.headers().get_all("set-cookie").iter().filter_map(|c| c.to_str().ok()).map(|c| c.split(';').next().unwrap().to_string()).collect();
            let html = page.text().unwrap();
            let action = html.split("action=\"").nth(1).unwrap().split('"').next().unwrap().replace("&amp;", "&");
            let back = http.post(&action).header("Cookie", cookies.join("; ")).form(&[("username", user), ("password", password), ("credentialId", "")]).send().unwrap();
            assert_eq!(back.status(), 302, "Keycloak accepted the password");
            let callback = back.headers()["location"].to_str().unwrap().to_string();
            assert!(callback.starts_with(&format!("{base}/auth/callback?")), "{callback}");
            http.get(&callback).send().unwrap()
        };
        let sign_in = |user: &str, password: &str| sign_in_at("/auth/login", user, password);
        // Compass: the session goes back to its one-time address on this computer; any other return is refused.
        let compass = sign_in_at("/auth/login?return=http%3A%2F%2F127.0.0.1%3A55123%2Fcompass-callback", "anna", "Lab-Test-anna-7Q2");
        assert_eq!(compass.status(), 302);
        let back = compass.headers()["location"].to_str().unwrap().to_string();
        assert!(back.starts_with("http://127.0.0.1:55123/compass-callback?session=lcs_") && back.contains("&role=architect"), "{back}");
        let session = back.split("session=").nth(1).unwrap().split('&').next().unwrap();
        assert_eq!(http.get(format!("{base}/api/me")).bearer_auth(session).send().unwrap().json::<Value>().unwrap()["name"], "anna");
        assert_eq!(http.get(format!("{base}/auth/login?return=https%3A%2F%2Fevil.example%2Fcompass-callback")).send().unwrap().status(), 403);
        let anna = sign_in("anna", "Lab-Test-anna-7Q2");
        assert_eq!(anna.status(), 302);
        let location = anna.headers()["location"].to_str().unwrap().to_string();
        let session = location.strip_prefix("/#session=").expect("the session in the fragment").to_string();
        let me: Value = http.get(format!("{base}/api/me")).bearer_auth(&session).send().unwrap().json().unwrap();
        println!("anna: {me}");
        assert_eq!(me["role"], "architect");
        assert_eq!(me["name"], "anna");
        let celina = sign_in("celina", "Lab-Test-celina-9X4");
        let session = celina.headers()["location"].to_str().unwrap().strip_prefix("/#session=").unwrap().to_string();
        let me: Value = http.get(format!("{base}/api/me")).bearer_auth(&session).send().unwrap().json().unwrap();
        assert_eq!(me["role"], "admin", "the highest role of the groups");
        // Without a Compass group: no session, a page with the reason.
        let ewa = sign_in("ewa", "Lab-Test-ewa-3K8");
        assert_eq!(ewa.status(), 403);
        assert!(ewa.text().unwrap().contains("not in any group of the hub"));
        // A callback with a state the hub did not start is refused.
        let forged = http.get(format!("{base}/auth/callback?code=x&state=forged")).send().unwrap();
        assert_eq!(forged.status(), 403);
        let audit = std::fs::read_to_string(dir.join("audit.jsonl")).unwrap_or_default();
        assert!(audit.contains("signed in with a company account"), "the sign-in is in the audit trail");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Team event agents from the hub policy reach Compass: `LOONAR_HUB_FILE_PROPS=… cargo test hub_team_agents -- --ignored`
    #[test]
    #[ignore = "requires a test model"]
    fn hub_team_agents() {
        crate::i18n::set(crate::i18n::Lang::En);
        crate::config::secrets::use_environment();
        std::env::set_var("LOONAR_SECRET_HUB_TOKEN", "admin-token-team-agents-0123");
        let dir = std::env::temp_dir().join(format!("loonar-hub-agents-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("hub.json"), json!({ "listen": "127.0.0.1:18732",
            "sources": [{ "id": "shop", "name": "Shop", "properties": std::env::var("LOONAR_HUB_FILE_PROPS").unwrap() }],
            "users": [{ "name": "admin", "token": "admin-token-team-agents-0123", "role": "admin" }],
            "policy": { "event_agents": [{ "name": "Keep requirements", "event": "element_deleting", "mode": "guard", "applies_to": "Requirement", "prompt": "Block approved requirements." }] } }).to_string()).unwrap();
        let (hub, listen) = load(&dir.join("hub.json")).unwrap();
        std::thread::spawn(move || serve(hub, &listen, |_| {}));
        std::thread::sleep(Duration::from_millis(300));
        let props = dir.join("app.properties");
        std::fs::write(&props, format!("{}\nhub.url=http://127.0.0.1:18732\n", std::fs::read_to_string(std::env::var("LOONAR_HUB_FILE_PROPS").unwrap()).unwrap())).unwrap();
        let cfg = crate::config::AppConfig::load(&props).unwrap();
        let agents = crate::agents::load_all(&cfg);
        assert!(agents.iter().any(|a| a.name == "Keep requirements (team)" && a.is_guard()), "{agents:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Word and Excel reports of a package from the hub: `LOONAR_HUB_FILE_PROPS=… cargo test hub_reports -- --ignored`
    #[test]
    #[ignore = "requires a test model"]
    fn hub_reports() {
        crate::i18n::set(crate::i18n::Lang::En);
        let dir = std::env::temp_dir().join(format!("loonar-hub-reports-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("hub.json"), json!({ "listen": "127.0.0.1:18733",
            "sources": [{ "id": "shop", "name": "Shop", "properties": std::env::var("LOONAR_HUB_FILE_PROPS").unwrap() }],
            "users": [{ "name": "olek", "token": "viewer-token-reports-0123", "role": "viewer" }] }).to_string()).unwrap();
        let (hub, listen) = load(&dir.join("hub.json")).unwrap();
        let cfg = hub.sources[0].cfg.clone();
        std::thread::spawn(move || serve(hub, &listen, |_| {}));
        std::thread::sleep(Duration::from_millis(300));
        let package = crate::tools::query_rows(&cfg, "select package_id from t_package where name = 'Shop domain'").unwrap()[0][0].clone();
        let http = reqwest::blocking::Client::new();
        for (format, kind) in [("docx", "wordprocessingml"), ("xlsx", "spreadsheetml")] {
            let r = http.get(format!("http://127.0.0.1:18733/api/sources/shop/report?package={package}&format={format}")).bearer_auth("viewer-token-reports-0123").send().unwrap();
            assert_eq!(r.status(), 200);
            assert!(r.headers()["content-type"].to_str().unwrap().contains(kind));
            assert!(r.headers()["content-disposition"].to_str().unwrap().contains(&format!(".{format}")));
            let bytes = r.bytes().unwrap();
            assert!(bytes.starts_with(b"PK") && bytes.len() > 2000, "{format}: {} bytes", bytes.len());
            if format == "docx" {
                let text = crate::documents::text_of("r.docx", &bytes).unwrap();
                assert!(text.contains("Order") && text.contains("Relationships"), "{text}");
            }
        }
        let missing = http.get("http://127.0.0.1:18733/api/sources/shop/report?package=999999").bearer_auth("viewer-token-reports-0123").send().unwrap();
        assert_eq!(missing.status(), 404);
        assert_eq!(http.get(format!("http://127.0.0.1:18733/api/sources/shop/report?package={package}")).send().unwrap().status(), 401, "only signed-in users");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// GraphQL and the Power BI tables: `LOONAR_HUB_FILE_PROPS=… cargo test hub_graphql -- --ignored --nocapture`
    #[test]
    #[ignore = "requires a test model"]
    fn hub_graphql() {
        crate::i18n::set(crate::i18n::Lang::En);
        let dir = std::env::temp_dir().join(format!("loonar-hub-graphql-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("hub.json"), json!({ "listen": "127.0.0.1:18734",
            "sources": [{ "id": "shop", "name": "Shop", "properties": std::env::var("LOONAR_HUB_FILE_PROPS").unwrap() }],
            "users": [{ "name": "olek", "token": "viewer-token-graphql-0123", "role": "viewer" }] }).to_string()).unwrap();
        let (hub, listen) = load(&dir.join("hub.json")).unwrap();
        std::thread::spawn(move || serve(hub, &listen, |_| {}));
        std::thread::sleep(Duration::from_millis(300));
        let http = reqwest::blocking::Client::new();
        let q = |query: &str, variables: Value| -> Value {
            http.post("http://127.0.0.1:18734/api/sources/shop/graphql").bearer_auth("viewer-token-graphql-0123").json(&json!({ "query": query, "variables": variables })).send().unwrap().json().unwrap()
        };
        let answer = q("query($n: String) { elements(name: $n, type: \"Class\") { id name type package { name } attributes { name type } connectors { type source { name } target { name } } } }", json!({ "n": "order" }));
        println!("{}", serde_json::to_string_pretty(&answer).unwrap().chars().take(1500).collect::<String>());
        let order = answer["data"]["elements"].as_array().unwrap().iter().find(|e| e["name"] == "Order").expect("Order");
        assert_eq!(order["package"]["name"], "Shop domain");
        assert!(order["attributes"].as_array().unwrap().iter().any(|a| a["name"] == "date"));
        assert!(!order["connectors"].as_array().unwrap().is_empty());
        let roots = q("{ packages { name packages { name diagrams { name type } } } }", Value::Null);
        assert!(roots["data"]["packages"].as_array().is_some_and(|p| !p.is_empty()), "{roots}");
        let bad = q("{ nothing }", Value::Null);
        assert!(!bad["errors"].as_array().unwrap().is_empty());
        let table: Value = http.get("http://127.0.0.1:18734/api/sources/shop/table/elements").bearer_auth("viewer-token-graphql-0123").send().unwrap().json().unwrap();
        assert!(table.as_array().unwrap().iter().any(|r| r["name"] == "Order" && r["package"] == "Shop domain"), "{}", table.to_string().chars().take(300).collect::<String>());
        let schema: Value = http.get("http://127.0.0.1:18734/api/graphql/schema").bearer_auth("viewer-token-graphql-0123").send().unwrap().json().unwrap();
        assert!(schema["sdl"].as_str().unwrap().contains("type Element"));
        let _ = std::fs::remove_dir_all(&dir);
    }

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
                "select object_id as c0, name as c1 from t_object where object_type <> 'Package' and length(name) > 3 order by object_id limit 1").unwrap();
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

        // Hardening: a malformed request does not stop the hub; headers and bodies are limited; security headers.
        let raw = |bytes: &[u8]| {
            let mut s = std::net::TcpStream::connect(base.trim_start_matches("http://")).unwrap();
            s.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
            s.write_all(bytes).unwrap();
            let mut out = String::new();
            let _ = s.read_to_string(&mut out);
            out
        };
        let _ = raw("GET /%1é HTTP/1.1\r\n\r\n".as_bytes());
        let _ = raw("GET /api/x?q=%€ HTTP/1.1\r\n\r\n".as_bytes());
        assert_eq!(get("/api/health", "").0, 200, "the hub is still running after malformed requests");
        assert!(raw(format!("GET / HTTP/1.1\r\nX: {}\r\n\r\n", "a".repeat(20_000)).as_bytes()).starts_with("HTTP/1.1 431"));
        assert!(raw(b"POST /api/audit HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\n").starts_with("HTTP/1.1 501"));
        let page = http.get(format!("{base}/")).send().unwrap();
        assert!(page.headers()["content-security-policy"].to_str().unwrap().contains("frame-ancestors 'none'") && page.headers()["x-frame-options"] == "DENY");
        assert!(!page.text().unwrap().contains("localStorage.setItem"), "the script is served apart, the token is kept for the session only");
        assert_eq!(get("/app.js", "").0, 200);
        // Change sets: what reviewers see is computed from the operations; nobody approves their own; apply needs a claim.
        let created = http.post(format!("{base}/api/changes")).bearer_auth(architect)
            .json(&json!({ "source": "shop-file", "title": "t", "summary": "harmless rename", "preview": "harmless", "ops": [{ "op": "delete", "element": "1" }] }))
            .send().unwrap().json::<Value>().unwrap();
        let id = created["id"].as_str().unwrap().to_string();
        let change: Value = serde_json::from_str(&get(&format!("/api/changes/{id}"), viewer).1).unwrap();
        assert!(change["summary"].as_str().unwrap() != "harmless rename" && change["ops"][0]["element"] == 1, "{change}");
        assert_eq!(post(&format!("/api/changes/{id}/decision"), architect, json!({ "approve": true })), 403, "the author cannot approve");
        assert_eq!(post(&format!("/api/changes/{id}/decision"), admin, json!({ "approve": true })), 200, "another user approves");
        assert_eq!(post(&format!("/api/changes/{id}/applied"), architect, json!({ "ok": true })), 409, "applied only after a claim");
        assert_eq!(post(&format!("/api/changes/{id}/claim"), architect, json!({})), 200);
        assert_eq!(post(&format!("/api/changes/{id}/claim"), admin, json!({})), 409, "claimed once");
        assert_eq!(post(&format!("/api/changes/{id}/applied"), admin, json!({ "ok": true })), 409, "only the claimer reports");
        assert_eq!(post(&format!("/api/changes/{id}/applied"), architect, json!({ "ok": true })), 200);
        assert_eq!(post("/api/changes", architect, json!({ "source": "shop-file", "ops": [{ "op": "export_xmi", "package": 1, "file": "\\\\server\\x.xml" }] })), 400, "network paths are refused");
        // Audit: only known fields, the hub's own time; the workstation details only for the admin.
        assert_eq!(post("/api/audit", architect, json!({ "time": "2000-01-01", "summary": "x", "database": "db@host", "anything": "else" })), 201);
        let entries: Value = serde_json::from_str(&get("/api/audit?limit=5", viewer).1).unwrap();
        let entry = entries.as_array().unwrap().iter().find(|e| e["summary"] == "x").unwrap().clone();
        assert!(entry["time"] != "2000-01-01" && entry.get("anything").is_none() && entry.get("database").is_none(), "{entry}");
        // Wrong tokens: after 20 the address waits; requests without a token are not counted; a valid token still works.
        for _ in 0..30 {
            get("/favicon.ico", "");
            get("/api/me", "");
        }
        assert_eq!(get("/api/me", "wrong-token-0123456789").0, 401, "requests without a token do not lock the address");
        for _ in 0..20 {
            get("/api/me", "wrong-token-0123456789");
        }
        assert_eq!(get("/api/me", "wrong-token-0123456789").0, 429, "this address is refused for a while");
        assert_eq!(get("/api/me", viewer).0, 200, "a valid token is never locked out");
        assert_eq!(get("/favicon.ico", "").0, 204);
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[cfg(test)]
mod ask_tests {
    use super::*;

    /// Questions of stakeholders on both sources (LOONAR_HUB_DB_PROPS, LOONAR_HUB_FILE_PROPS), answered by the model of
    /// the source's settings (an API key on this computer). Run with: `cargo test hub_ask -- --ignored --nocapture`.
    #[test]
    #[ignore = "requires an API key and two test models"]
    fn hub_ask() {
        crate::i18n::set(crate::i18n::Lang::Pl);
        let dir = std::env::temp_dir().join(format!("loonar-hub-ask-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("hub.json"), json!({ "listen": "127.0.0.1:0",
            "sources": [{ "id": "db", "name": "DB", "properties": std::env::var("LOONAR_HUB_DB_PROPS").unwrap() }, { "id": "file", "name": "File", "properties": std::env::var("LOONAR_HUB_FILE_PROPS").unwrap() }],
            "users": [{ "name": "pm", "token": "viewer-token-0123456789", "role": "viewer" }] }).to_string()).unwrap();
        let (hub, listen) = load(&dir.join("hub.json")).unwrap();
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || serve(hub, &listen, |a| tx.send(a).unwrap()).unwrap());
        let base = format!("http://{}", rx.recv().unwrap());
        let http = reqwest::blocking::Client::builder().redirect(crate::net::same_origin_redirects()).timeout(Duration::from_secs(300)).build().unwrap();
        for (source, sql) in [("db", LOONAR_DB_CLASSES), ("file", LOONAR_DB_CLASSES)] {
            let props = std::env::var(if source == "db" { "LOONAR_HUB_DB_PROPS" } else { "LOONAR_HUB_FILE_PROPS" }).unwrap();
            let classes = crate::tools::query_rows(&AppConfig::load(Path::new(&props)).unwrap(), sql).unwrap()[0][0].clone();
            let r = http.post(format!("{base}/api/sources/{source}/ask")).bearer_auth("viewer-token-0123456789").json(&json!({ "question": "Ile klas (elementów typu Class) jest w tym modelu? Podaj liczbę." })).send().unwrap();
            let status = r.status().as_u16();
            let v: Value = r.json().unwrap();
            println!("{source}: {status} {} s, expected {classes}: {}", v["seconds"], v["answer"].as_str().unwrap_or(&v.to_string()).chars().take(300).collect::<String>());
            assert_eq!(status, 200);
            assert!(v["answer"].as_str().unwrap_or("").contains(&classes), "{source}: the number of classes");
        }
        let forbidden = http.post(format!("{base}/api/sources/db/ask")).json(&json!({ "question": "x y z" })).send().unwrap().status().as_u16();
        assert_eq!(forbidden, 401);
        let audit = std::fs::read_to_string(dir.join("audit.jsonl")).unwrap();
        assert!(audit.contains("question: Ile klas"), "questions are in the audit trail");
        let _ = std::fs::remove_dir_all(&dir);
    }

    const LOONAR_DB_CLASSES: &str = "select count(*) from t_object where object_type = 'Class'";
}

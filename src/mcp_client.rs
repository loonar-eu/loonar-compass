//! MCP client: the user's own MCP servers (a local program over stdio, or an address over Streamable HTTP) give the
//! assistant extra tools (e.g. ServiceNow, Azure DevOps, GitLab, a company knowledge base, another model hub).
//!
//! Servers are listed in `mcp_servers.json` next to the settings:
//! {"servers": [
//!   {"name": "hub", "url": "http://127.0.0.1:8730/mcp", "headers": {"Authorization": "Bearer ${secret:hub_token}"}},
//!   {"name": "files", "command": "node", "args": ["server.js"], "env": {"ROOT": "C:/docs"}, "trusted": false, "enabled": true}
//! ]}
//! `${secret:NAME}` takes a value from Windows Credential Manager (the application's entries), `${env:NAME}` from the
//! environment, so tokens do not have to be written into the file. Tools of a server that is not "trusted" ask for consent
//! before each call, unless the tool declares itself read-only (annotations.readOnlyHint).

use anyhow::{anyhow, bail, Context, Result};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc;
use std::sync::Mutex;
use std::time::Duration;

const PROTOCOL: &str = "2025-06-18";
const TIMEOUT: Duration = Duration::from_secs(120);
/// Tool names sent to the model: mcp__<server>__<tool>, at most 64 characters, letters, digits, '_' and '-'.
const PREFIX: &str = "mcp__";

#[derive(Clone, Debug)]
pub struct ServerConfig {
    pub name: String,
    pub command: Option<String>,
    pub args: Vec<String>,
    pub env: Vec<(String, String)>,
    pub url: Option<String>,
    pub headers: Vec<(String, String)>,
    /// Sign-in other than a header: {"type": "basic", "user", "password"} or {"type": "oauth2", "token_url", "client_id",
    /// "client_secret", "scope"} (secrets as ${secret:mcp_…}).
    pub auth: Value,
    pub trusted: bool,
    /// The server is the MCP endpoint of the team hub from the settings: its headers may use ${secret:hub_token}.
    pub is_team_hub: bool,
}

pub fn config_file(base_dir: &Path) -> PathBuf {
    base_dir.join("mcp_servers.json")
}

/// `${secret:NAME}` and `${env:NAME}` in a value.
fn expand(text: &str) -> Result<String> {
    expand_for(text, false)
}

/// As `expand`; `team_hub`: the value goes to the team hub of the settings, so the hub token may be used.
fn expand_for(text: &str, team_hub: bool) -> Result<String> {
    let mut out = String::new();
    let mut rest = text;
    while let Some(start) = rest.find("${") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        let end = after.find('}').ok_or_else(|| anyhow!("{}", crate::i18n::fmt("unclosed ${…} in {0}", &[text.to_string()])))?;
        let inner = &after[..end];
        let value = if let Some(name) = inner.strip_prefix("secret:") {
            // Compass's own keys and passwords never go to an MCP server.
            let base = name.trim().split(':').next().unwrap_or("").to_ascii_lowercase();
            let hub_token = team_hub && base.eq_ignore_ascii_case(crate::config::secrets::HUB_TOKEN);
            if !hub_token && crate::config::secrets::ALL.iter().any(|s| s.eq_ignore_ascii_case(&base)) {
                bail!("{}", crate::i18n::fmt("the secret {0} belongs to Compass and cannot be given to an MCP server; save the server's token under another name", &[name.trim().to_string()]));
            }
            crate::config::secrets::get(name.trim()).with_context(|| crate::i18n::fmt("no secret {0} in Credential Manager", &[name.to_string()]))?
        } else if let Some(name) = inner.strip_prefix("env:") {
            std::env::var(name.trim()).with_context(|| crate::i18n::fmt("no environment variable {0}", &[name.to_string()]))?
        } else {
            bail!("{}", crate::i18n::fmt("unknown placeholder {0} (use secret: or env:)", &[inner.to_string()]));
        };
        out.push_str(&value);
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    Ok(out)
}

/// On Windows npx, npm, pnpm and yarn are .cmd files: "npx" in the configuration starts npx.cmd.
fn windows_command(command: &str) -> String {
    let lower = command.trim().to_ascii_lowercase();
    if cfg!(windows) && matches!(lower.as_str(), "npx" | "npm" | "pnpm" | "yarn") {
        format!("{}.cmd", command.trim())
    } else {
        command.to_string()
    }
}

/// Ready-made servers for the "MCP servers" card: (label, server). `hub_url`: the hub of the settings (or empty).
pub fn templates(hub_url: &str) -> Vec<(&'static str, Value)> {
    let mut list = vec![
        ("Azure DevOps", json!({ "name": "azure_devops", "command": "npx", "args": ["-y", "@azure-devops/mcp", "YOUR-ORGANIZATION"] })),
        ("GitHub", json!({ "name": "github", "url": "https://api.githubcopilot.com/mcp/", "headers": { "Authorization": "Bearer ${secret:mcp_github}" } })),
        ("ServiceNow / CMDB", json!({ "name": "servicenow", "url": "https://YOUR-SERVICENOW-MCP-SERVER/mcp", "headers": { "Authorization": "Bearer ${secret:mcp_servicenow}" } })),
        ("Files (a local folder)", json!({ "name": "files", "command": "npx", "args": ["-y", "@modelcontextprotocol/server-filesystem", "C:/docs"] })),
    ];
    let hub = hub_url.trim().trim_end_matches('/');
    if !hub.is_empty() {
        list.push(("Compass Hub", json!({ "name": "hub", "url": format!("{hub}/mcp"), "headers": { "Authorization": "Bearer ${secret:hub_token}" }, "trusted": true })));
    }
    list
}

/// Adds a server to the text of mcp_servers.json (an empty text starts a new file); a name already used gets a number.
pub fn add_server(text: &str, server: &Value) -> Result<String> {
    let mut value: Value = if text.trim().is_empty() { json!({ "servers": [] }) } else { serde_json::from_str(text).with_context(|| crate::i18n::tr("mcp_servers.json is not valid JSON"))? };
    let list = value["servers"].as_array_mut().with_context(|| crate::i18n::tr("mcp_servers.json is not valid JSON"))?;
    let mut server = server.clone();
    let base = server["name"].as_str().unwrap_or("server").to_string();
    let mut name = base.clone();
    let mut n = 2;
    while list.iter().any(|s| s["name"] == name.as_str()) {
        name = format!("{base}_{n}");
        n += 1;
    }
    server["name"] = json!(name);
    list.push(server);
    Ok(serde_json::to_string_pretty(&value)?)
}

/// A token of an MCP server saved in Credential Manager: names start with "mcp_" (Compass's own names are refused).
pub fn save_secret(name: &str, value: &str) -> Result<()> {
    let name = name.trim();
    if !name.starts_with("mcp_") || name.len() < 5 || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        bail!("{}", crate::i18n::tr("the name of an MCP token starts with mcp_ and has only letters, digits and _ (e.g. mcp_github)"));
    }
    crate::config::secrets::set(name, value.trim())
}

fn safe_name(text: &str) -> String {
    text.chars().map(|c| if c.is_ascii_alphanumeric() || c == '_' || c == '-' { c } else { '_' }).collect()
}

/// Servers from the file (missing file = none). Disabled servers are left out.
pub fn load(base_dir: &Path) -> Result<Vec<ServerConfig>> {
    let file = config_file(base_dir);
    let Ok(text) = std::fs::read_to_string(&file) else { return Ok(Vec::new()) };
    if text.trim().is_empty() {
        return Ok(Vec::new());
    }
    let mut servers = parse(&text)?;
    // The team hub of the settings (same scheme, host and port, path /mcp) may get the hub token.
    let hub_url = std::fs::read_to_string(base_dir.join("app.properties")).ok().and_then(|p| crate::config::parse_properties(&p).get("hub.url").cloned()).unwrap_or_default();
    if let Ok(hub) = reqwest::Url::parse(hub_url.trim()) {
        for s in servers.iter_mut() {
            if let Some(url) = s.url.as_deref().and_then(|u| reqwest::Url::parse(u).ok()) {
                s.is_team_hub = url.scheme() == hub.scheme() && url.host_str() == hub.host_str() && url.port_or_known_default() == hub.port_or_known_default() && url.path().trim_end_matches('/').ends_with("/mcp");
            }
        }
    }
    Ok(servers)
}

pub fn parse(text: &str) -> Result<Vec<ServerConfig>> {
    let value: Value = serde_json::from_str(text).with_context(|| crate::i18n::tr("mcp_servers.json is not valid JSON"))?;
    let mut out = Vec::new();
    for s in value["servers"].as_array().into_iter().flatten() {
        if s["enabled"] == false {
            continue;
        }
        let name = safe_name(s["name"].as_str().unwrap_or("").trim());
        if name.is_empty() {
            bail!("{}", crate::i18n::tr("every MCP server needs a name"));
        }
        let pairs = |v: &Value| -> Vec<(String, String)> {
            v.as_object().map(|o| o.iter().map(|(k, v)| (k.clone(), v.as_str().map(String::from).unwrap_or_else(|| v.to_string()))).collect()).unwrap_or_default()
        };
        let server = ServerConfig {
            name: name.clone(),
            command: s["command"].as_str().map(String::from),
            args: s["args"].as_array().into_iter().flatten().filter_map(|a| a.as_str().map(String::from)).collect(),
            env: pairs(&s["env"]),
            url: s["url"].as_str().map(String::from),
            headers: pairs(&s["headers"]),
            auth: s["auth"].clone(),
            trusted: s["trusted"] == true,
            is_team_hub: false,
        };
        if server.command.is_none() == server.url.is_none() {
            bail!("{}", crate::i18n::fmt("MCP server {0}: give either \"command\" (a local program) or \"url\" (HTTP)", &[name]));
        }
        if let Some(url) = &server.url {
            crate::jira::check_base_url(url).map_err(|_| anyhow!("{}", crate::i18n::fmt("MCP server {0}: the address must start with https:// (http:// only on this computer)", &[name.clone()])))?;
        }
        out.push(server);
    }
    Ok(out)
}

enum Transport {
    Stdio { child: Child, stdin: ChildStdin, lines: mpsc::Receiver<String> },
    Http { client: reqwest::blocking::Client, url: String, headers: Vec<(String, String)>, session: Option<String> },
}

struct Connection {
    config: ServerConfig,
    transport: Transport,
    next_id: i64,
    tools: Vec<Value>,
}

impl Drop for Connection {
    fn drop(&mut self) {
        if let Transport::Stdio { child, .. } = &mut self.transport {
            let _ = child.kill();
        }
    }
}

impl Connection {
    fn open(config: &ServerConfig) -> Result<Self> {
        let transport = if let Some(command) = &config.command {
            let mut cmd = Command::new(windows_command(&expand(command)?));
            for a in &config.args {
                cmd.arg(expand(a)?);
            }
            // The server gets the basic Windows environment and its own variables, not everything Compass has.
            cmd.env_clear();
            for name in ["SystemRoot", "SystemDrive", "windir", "ComSpec", "PATH", "PATHEXT", "TEMP", "TMP", "USERPROFILE", "USERNAME", "USERDOMAIN", "HOMEDRIVE", "HOMEPATH",
                "APPDATA", "LOCALAPPDATA", "ProgramData", "ProgramFiles", "ProgramFiles(x86)", "CommonProgramFiles", "NUMBER_OF_PROCESSORS", "PROCESSOR_ARCHITECTURE",
                "HTTP_PROXY", "HTTPS_PROXY", "NO_PROXY", "http_proxy", "https_proxy", "no_proxy", "NODE_EXTRA_CA_CERTS", "JAVA_HOME", "ALLUSERSPROFILE",
                "PUBLIC", "OS", "COMPUTERNAME", "USERDNSDOMAIN", "CommonProgramW6432", "ProgramW6432"] {
                if let Some(value) = std::env::var_os(name) {
                    cmd.env(name, value);
                }
            }
            for (k, v) in &config.env {
                cmd.env(k, expand(v)?);
            }
            cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null());
            #[cfg(windows)]
            {
                use std::os::windows::process::CommandExt;
                cmd.creation_flags(0x0800_0000);
            }
            let mut child = cmd.spawn().with_context(|| crate::i18n::fmt("MCP server {0}: could not start {1}", &[config.name.clone(), command.clone()]))?;
            let stdin = child.stdin.take().context("stdin")?;
            let stdout = child.stdout.take().context("stdout")?;
            let (tx, lines) = mpsc::channel();
            std::thread::spawn(move || {
                for line in BufReader::new(stdout).lines() {
                    let Ok(line) = line else { break };
                    if tx.send(line).is_err() {
                        break;
                    }
                }
            });
            Transport::Stdio { child, stdin, lines }
        } else {
            let mut headers = config.headers.iter().map(|(k, v)| Ok((k.clone(), expand_for(v, config.is_team_hub)?))).collect::<Result<Vec<_>>>()?;
            let field = |k: &str| -> Result<String> { expand_for(config.auth[k].as_str().unwrap_or(""), config.is_team_hub) };
            match config.auth["type"].as_str() {
                Some("basic") => headers.push(("Authorization".into(), crate::auth::basic(&field("user")?, &field("password")?))),
                Some("oauth2") => {
                    let token = crate::auth::client_credentials(&field("token_url")?, &field("client_id")?, &field("client_secret")?, &field("scope")?)?;
                    headers.push(("Authorization".into(), format!("Bearer {token}")));
                }
                Some(other) => bail!("{}", crate::i18n::fmt("MCP server {0}: unknown auth type {1} (basic, oauth2)", &[config.name.clone(), other.to_string()])),
                None => {}
            }
            let client = reqwest::blocking::Client::builder().timeout(TIMEOUT).redirect(crate::net::same_origin_redirects()).build()?;
            Transport::Http { client, url: config.url.clone().unwrap_or_default(), headers, session: None }
        };
        let mut conn = Connection { config: config.clone(), transport, next_id: 0, tools: Vec::new() };
        conn.request("initialize", json!({ "protocolVersion": PROTOCOL, "capabilities": {}, "clientInfo": { "name": "loonar-compass", "version": env!("CARGO_PKG_VERSION") } }))?;
        conn.notify("notifications/initialized")?;
        let mut tools = Vec::new();
        let mut cursor: Option<String> = None;
        loop {
            let params = match &cursor { Some(c) => json!({ "cursor": c }), None => json!({}) };
            let page = conn.request("tools/list", params)?;
            tools.extend(page["tools"].as_array().cloned().unwrap_or_default());
            cursor = page["nextCursor"].as_str().map(String::from);
            if cursor.is_none() || tools.len() > 500 {
                break;
            }
        }
        conn.tools = tools;
        Ok(conn)
    }

    fn notify(&mut self, method: &str) -> Result<()> {
        let message = json!({ "jsonrpc": "2.0", "method": method });
        match &mut self.transport {
            Transport::Stdio { stdin, .. } => {
                writeln!(stdin, "{message}")?;
                stdin.flush()?;
            }
            Transport::Http { client, url, headers, session } => {
                let mut r = client.post(url.as_str()).header("Accept", "application/json, text/event-stream").json(&message);
                for (k, v) in headers.iter() {
                    r = r.header(k.as_str(), v.as_str());
                }
                if let Some(s) = session {
                    r = r.header("Mcp-Session-Id", s.as_str());
                }
                let _ = r.send();
            }
        }
        Ok(())
    }

    fn request(&mut self, method: &str, params: Value) -> Result<Value> {
        self.next_id += 1;
        let id = self.next_id;
        let message = json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params });
        let name = self.config.name.clone();
        let reply: Value = match &mut self.transport {
            Transport::Stdio { stdin, lines, child } => {
                writeln!(stdin, "{message}").with_context(|| crate::i18n::fmt("MCP server {0} has stopped", &[name.clone()]))?;
                stdin.flush()?;
                let deadline = std::time::Instant::now() + TIMEOUT;
                loop {
                    let left = deadline.saturating_duration_since(std::time::Instant::now());
                    let line = lines.recv_timeout(left).map_err(|_| {
                        let gone = child.try_wait().ok().flatten().is_some();
                        anyhow!("{}", if gone { crate::i18n::fmt("MCP server {0} has stopped", &[name.clone()]) } else { crate::i18n::fmt("MCP server {0} did not answer in time", &[name.clone()]) })
                    })?;
                    let Ok(v) = serde_json::from_str::<Value>(line.trim()) else { continue };
                    if v["id"] == json!(id) {
                        break v;
                    }
                }
            }
            Transport::Http { client, url, headers, session } => {
                let mut r = client.post(url.as_str()).header("Accept", "application/json, text/event-stream").json(&message);
                for (k, v) in headers.iter() {
                    r = r.header(k.as_str(), v.as_str());
                }
                if let Some(s) = session.as_ref() {
                    r = r.header("Mcp-Session-Id", s.as_str());
                }
                let response = r.send().with_context(|| crate::i18n::fmt("MCP server {0}: no connection", &[name.clone()]))?;
                if let Some(s) = response.headers().get("mcp-session-id").and_then(|v| v.to_str().ok()) {
                    *session = Some(s.to_string());
                }
                let status = response.status();
                let event_stream = response.headers().get("content-type").and_then(|v| v.to_str().ok()).is_some_and(|c| c.contains("text/event-stream"));
                let body = response.text().unwrap_or_default();
                if !status.is_success() {
                    let detail = serde_json::from_str::<Value>(&body).ok().and_then(|v| v["error"].as_str().map(String::from)).unwrap_or_else(|| body.chars().take(200).collect());
                    bail!("{}", crate::i18n::fmt("MCP server {0} replied {1}: {2}", &[name.clone(), status.to_string(), detail]));
                }
                if event_stream {
                    body.lines()
                        .filter_map(|l| l.strip_prefix("data:"))
                        .filter_map(|d| serde_json::from_str::<Value>(d.trim()).ok())
                        .find(|v| v["id"] == json!(id))
                        .ok_or_else(|| anyhow!("{}", crate::i18n::fmt("MCP server {0}: no answer in the event stream", &[name.clone()])))?
                } else {
                    serde_json::from_str(&body).with_context(|| crate::i18n::fmt("MCP server {0}: the answer is not JSON", &[name.clone()]))?
                }
            }
        };
        if let Some(error) = reply.get("error") {
            bail!("{}", crate::i18n::fmt("MCP server {0}: {1}", &[name, error["message"].as_str().unwrap_or(&error.to_string()).to_string()]));
        }
        Ok(reply["result"].clone())
    }
}

/// Open connections, by server name (kept for the whole session of the application).
static CONNECTIONS: Mutex<Option<HashMap<String, Connection>>> = Mutex::new(None);

fn with_connection<T>(config: &ServerConfig, f: impl FnOnce(&mut Connection) -> Result<T>) -> Result<T> {
    let mut guard = CONNECTIONS.lock().unwrap_or_else(|p| p.into_inner());
    let map = guard.get_or_insert_with(HashMap::new);
    let stale = map.get_mut(&config.name).is_some_and(|c| {
        c.config.command != config.command || c.config.url != config.url || c.config.args != config.args || match &mut c.transport {
            Transport::Stdio { child, .. } => child.try_wait().ok().flatten().is_some(),
            Transport::Http { .. } => false,
        }
    });
    if stale {
        map.remove(&config.name);
    }
    if !map.contains_key(&config.name) {
        let conn = Connection::open(config)?;
        map.insert(config.name.clone(), conn);
    }
    let conn = map.get_mut(&config.name).expect("connection");
    let result = f(conn);
    if result.is_err() {
        // A broken connection is opened again next time.
        if let Transport::Http { .. } = conn.transport {
        } else {
            map.remove(&config.name);
        }
    }
    result
}

/// Closes all connections (stops the local server programs).
pub fn shutdown() {
    CONNECTIONS.lock().unwrap_or_else(|p| p.into_inner()).take();
}

fn tool_name(server: &str, tool: &str) -> String {
    let full = format!("{PREFIX}{server}__{}", safe_name(tool));
    full.chars().take(64).collect()
}

/// Tool definitions of all configured servers, in the format of the model API. Servers that do not start are skipped
/// (their error is returned in the second value, to show to the user once).
pub fn definitions(base_dir: &Path) -> (Vec<Value>, Vec<String>) {
    let mut defs = Vec::new();
    let mut errors = Vec::new();
    let servers = match load(base_dir) {
        Ok(s) => s,
        Err(e) => return (defs, vec![format!("{e:#}")]),
    };
    for server in servers {
        match with_connection(&server, |c| Ok(c.tools.clone())) {
            Ok(tools) => {
                for t in tools {
                    let Some(name) = t["name"].as_str() else { continue };
                    let read_only = t["annotations"]["readOnlyHint"] == true;
                    let mut schema = t["inputSchema"].clone();
                    if !schema.is_object() {
                        schema = json!({ "type": "object", "properties": {} });
                    }
                    defs.push(json!({
                        "name": tool_name(&server.name, name),
                        "description": format!("[MCP server \"{}\"{}] {}", server.name, if read_only { ", read-only" } else { "" }, t["description"].as_str().unwrap_or("")),
                        "input_schema": schema
                    }));
                }
            }
            Err(e) => errors.push(format!("{}: {e:#}", server.name)),
        }
    }
    (defs, errors)
}

pub fn is_mcp_tool(name: &str) -> bool {
    name.starts_with(PREFIX)
}

/// (server, tool) of a tool name sent to the model.
fn split(base_dir: &Path, name: &str) -> Result<(ServerConfig, String)> {
    for server in load(base_dir)? {
        let prefix = format!("{PREFIX}{}__", server.name);
        if let Some(rest) = name.strip_prefix(&prefix) {
            let tool = with_connection(&server, |c| {
                Ok(c.tools.iter().filter_map(|t| t["name"].as_str()).find(|t| tool_name(&server.name, t) == name).map(String::from).unwrap_or_else(|| rest.to_string()))
            })?;
            return Ok((server, tool));
        }
    }
    bail!("{}", crate::i18n::fmt("unknown MCP tool {0}", &[name.to_string()]))
}

/// Whether a call needs the user's consent: a server not marked trusted and a tool not declared read-only.
/// Every call asks unless the user marked the server "trusted": the server's own claim that a tool only reads is not
/// enough (a "read-only" tool can still send what it gets somewhere else).
pub fn needs_consent(base_dir: &Path, name: &str) -> Result<(bool, String, String)> {
    let (server, tool) = split(base_dir, name)?;
    Ok((!server.trusted, server.name, tool))
}

/// Calls the tool; the text content of the result (images and other content are described in one line each).
pub fn call(base_dir: &Path, name: &str, input: &Value) -> Result<String> {
    let (server, tool) = split(base_dir, name)?;
    let result = with_connection(&server, |c| c.request("tools/call", json!({ "name": tool, "arguments": input })))?;
    let mut parts = Vec::new();
    for item in result["content"].as_array().into_iter().flatten() {
        match item["type"].as_str() {
            Some("text") => parts.push(item["text"].as_str().unwrap_or("").to_string()),
            Some(other) => parts.push(format!("[{other} content]")),
            None => {}
        }
    }
    if parts.is_empty() && !result["structuredContent"].is_null() {
        parts.push(result["structuredContent"].to_string());
    }
    let text = parts.join("\n");
    if result["isError"] == true {
        bail!("{}", crate::i18n::fmt("MCP tool {0} reported an error: {1}", &[tool, text]));
    }
    Ok(if text.is_empty() { "(no content)".into() } else { text })
}

/// Settings test: each server with the number of its tools (or its error).
pub fn check(base_dir: &Path) -> Result<String> {
    let servers = load(base_dir)?;
    if servers.is_empty() {
        return Ok(crate::i18n::tr("No MCP servers configured.").into());
    }
    shutdown();
    let mut lines = Vec::new();
    let mut failed = 0;
    for s in &servers {
        match with_connection(s, |c| Ok(c.tools.len())) {
            Ok(n) => lines.push(crate::i18n::fmt("{0}: {1} tools", &[s.name.clone(), n.to_string()])),
            Err(e) => {
                failed += 1;
                lines.push(format!("{}: {e:#}", s.name));
            }
        }
    }
    if failed > 0 {
        bail!("{}", lines.join("; "));
    }
    Ok(lines.join("; "))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A tiny MCP server over stdio written in PowerShell, so the test needs nothing installed.
    const PS_SERVER: &str = r#"
$ErrorActionPreference = 'Stop'
while ($null -ne ($line = [Console]::In.ReadLine())) {
  $m = $line | ConvertFrom-Json
  if ($null -eq $m.id) { continue }
  switch ($m.method) {
    'initialize' { $r = @{ protocolVersion = '2025-06-18'; capabilities = @{ tools = @{} }; serverInfo = @{ name = 'ps-test'; version = '1' } } }
    'tools/list' { $r = @{ tools = @(
        @{ name = 'echo'; description = 'Echoes the text'; inputSchema = @{ type = 'object'; properties = @{ text = @{ type = 'string' } } }; annotations = @{ readOnlyHint = $true } },
        @{ name = 'create ticket'; description = 'Creates a ticket'; inputSchema = @{ type = 'object'; properties = @{ title = @{ type = 'string' } } } }
      ) } }
    'tools/call' {
      if ($m.params.name -eq 'echo') { $r = @{ content = @(@{ type = 'text'; text = 'echo: ' + $m.params.arguments.text }); isError = $false } }
      else { $r = @{ content = @(@{ type = 'text'; text = 'ticket T-1 created: ' + $m.params.arguments.title }); isError = $false } }
    }
    default { $r = $null }
  }
  $out = @{ jsonrpc = '2.0'; id = $m.id; result = $r } | ConvertTo-Json -Depth 10 -Compress
  [Console]::Out.WriteLine($out); [Console]::Out.Flush()
}
"#;

    #[test]
    fn templates_and_adding() {
        let tpl = templates("https://hub.example.com/");
        assert_eq!(tpl.last().unwrap().1["url"], "https://hub.example.com/mcp");
        let mut text = String::new();
        for (_, server) in &tpl {
            text = add_server(&text, server).unwrap();
        }
        text = add_server(&text, &tpl[0].1).unwrap();
        let servers = parse(&text).unwrap();
        assert_eq!(servers.len(), tpl.len() + 1);
        assert!(servers.iter().any(|s| s.name == "azure_devops_2"), "a second copy gets a number");
        assert_eq!(windows_command("npx"), if cfg!(windows) { "npx.cmd" } else { "npx" });
        assert!(save_secret("claude_api_key", "x").is_err() && save_secret("mcp x", "x").is_err());
    }

    #[test]
    fn parse_and_names() {
        let servers = parse(r#"{"servers":[{"name":"a b","url":"http://localhost:1/mcp"},{"name":"x","command":"cmd","enabled":false}]}"#).unwrap();
        assert_eq!(servers.len(), 1);
        assert_eq!(servers[0].name, "a_b");
        assert!(parse(r#"{"servers":[{"name":"y","url":"http://example.com/mcp"}]}"#).is_err(), "plain http only on this computer");
        assert!(parse(r#"{"servers":[{"name":"z"}]}"#).is_err(), "command or url");
        assert_eq!(tool_name("srv", "create ticket"), "mcp__srv__create_ticket");
        assert!(expand("Bearer ${env:LOONAR_TEST_NOT_SET_123}").is_err());
        std::env::set_var("LOONAR_TEST_MCP_TOKEN", "t0k");
        assert_eq!(expand("Bearer ${env:LOONAR_TEST_MCP_TOKEN}").unwrap(), "Bearer t0k");
        crate::i18n::set(crate::i18n::Lang::En);
        assert!(format!("{:#}", expand("Bearer ${secret:claude_api_key}").unwrap_err()).contains("belongs to Compass"));
        assert!(format!("{:#}", expand("Bearer ${secret:hub_token}").unwrap_err()).contains("belongs to Compass"), "only the team hub's own MCP endpoint gets the hub token");
    }

    #[test]
    fn stdio_server() {
        let dir = std::env::temp_dir().join(format!("loonar-mcp-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let script = dir.join("server.ps1");
        std::fs::write(&script, PS_SERVER).unwrap();
        std::fs::write(config_file(&dir), json!({ "servers": [{ "name": "local", "command": "powershell", "args": ["-NoProfile", "-ExecutionPolicy", "Bypass", "-File", script.to_string_lossy()] }] }).to_string()).unwrap();
        let (defs, errors) = definitions(&dir);
        assert!(errors.is_empty(), "{errors:?}");
        let names: Vec<&str> = defs.iter().filter_map(|d| d["name"].as_str()).collect();
        assert_eq!(names, ["mcp__local__echo", "mcp__local__create_ticket"]);
        assert!(defs[0]["description"].as_str().unwrap().contains("read-only"));
        assert_eq!(call(&dir, "mcp__local__echo", &json!({ "text": "hello" })).unwrap(), "echo: hello");
        assert!(needs_consent(&dir, "mcp__local__echo").unwrap().0, "a read-only tool of a server that is not trusted asks too");
        assert!(needs_consent(&dir, "mcp__local__create_ticket").unwrap().0, "other tools ask");
        assert_eq!(call(&dir, "mcp__local__create_ticket", &json!({ "title": "Fix model" })).unwrap(), "ticket T-1 created: Fix model");
        assert!(check(&dir).unwrap().contains("local: 2"));
        shutdown();
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[cfg(test)]
mod hub_tests {
    use super::*;

    /// Streamable HTTP against Compass Hub with two sources (LOONAR_HUB_DB_PROPS, LOONAR_HUB_FILE_PROPS), the token
    /// taken from the environment in the server entry. Run with: `cargo test mcp_http_hub -- --ignored --nocapture`.
    #[test]
    #[ignore = "requires two test models"]
    fn mcp_http_hub() {
        let dir = std::env::temp_dir().join(format!("loonar-mcp-hub-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let hub_json = dir.join("hub.json");
        std::fs::write(&hub_json, json!({
            "listen": "127.0.0.1:0",
            "sources": [
                { "id": "shop-db", "name": "Shop (PostgreSQL)", "properties": std::env::var("LOONAR_HUB_DB_PROPS").unwrap() },
                { "id": "shop-file", "name": "Shop (file)", "properties": std::env::var("LOONAR_HUB_FILE_PROPS").unwrap() }
            ],
            "users": [{ "name": "viewer", "token": "viewer-token-0123456789", "role": "viewer" }]
        }).to_string()).unwrap();
        let (hub, listen) = crate::hub::load(&hub_json).unwrap();
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || crate::hub::serve(hub, &listen, |a| tx.send(a).unwrap()).unwrap());
        let addr = rx.recv().unwrap();
        std::env::set_var("LOONAR_TEST_HUB_TOKEN", "viewer-token-0123456789");
        std::fs::write(config_file(&dir), json!({ "servers": [{ "name": "hub", "url": format!("http://{addr}/mcp"), "headers": { "Authorization": "Bearer ${env:LOONAR_TEST_HUB_TOKEN}" } }] }).to_string()).unwrap();
        let (defs, errors) = definitions(&dir);
        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(defs.len(), 7, "the hub's seven tools");
        assert!(needs_consent(&dir, "mcp__hub__search_elements").unwrap().0, "a server that is not trusted asks, even for read-only tools");
        let sources = call(&dir, "mcp__hub__list_sources", &json!({})).unwrap();
        assert!(sources.contains("shop-db") && sources.contains("shop-file"), "{sources}");
        for source in ["shop-db", "shop-file"] {
            let packages = call(&dir, "mcp__hub__list_packages", &json!({ "source": source })).unwrap();
            assert!(packages.lines().count() > 1, "{source}: {packages}");
            let found = call(&dir, "mcp__hub__search_elements", &json!({ "source": source, "query": "Or" })).unwrap();
            println!("{source}: {} packages, search: {}", packages.lines().count() - 1, found.lines().count() - 1);
        }
        let failed = call(&dir, "mcp__hub__element_details", &json!({ "source": "shop-db" })).unwrap_err().to_string();
        assert!(failed.contains("reported an error"), "{failed}");
        std::fs::write(config_file(&dir), json!({ "servers": [{ "name": "hub", "url": format!("http://{addr}/mcp"), "headers": { "Authorization": "Bearer wrong-token-0000000000" } }] }).to_string()).unwrap();
        shutdown();
        let (_, errors) = definitions(&dir);
        assert!(errors.len() == 1 && errors[0].contains("401"), "{errors:?}");
        shutdown();
    }
}

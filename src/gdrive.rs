//! Google Drive with the user's own Google account (signed in in the browser on first use, see `user_oauth`): list
//! a folder or search by name, read a file's text (Google Docs, Sheets and Slides exported as text, other files
//! downloaded and read like read_document) and upload a file the assistant made (always asked).
//!
//! Settings: gdrive.client_id (an OAuth client of the type "Desktop app" in Google Cloud), the secret
//! gdrive_client_secret (Google's desktop clients have one, it is not really secret); the refresh token is kept as
//! gdrive_refresh_token.

use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use std::time::Duration;

const API: &str = "https://www.googleapis.com/drive/v3";
const AUTH_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
const FOLDER: &str = "application/vnd.google-apps.folder";
const DOC: &str = "application/vnd.google-apps.document";
const SHEET: &str = "application/vnd.google-apps.spreadsheet";
const SLIDES: &str = "application/vnd.google-apps.presentation";
/// At most this many items in one listing.
const LIST_LIMIT: usize = 200;
const MAX_DOWNLOAD: u64 = 60 * 1024 * 1024;
const FILE_FIELDS: &str = "id,name,mimeType,size,modifiedTime,webViewLink";

/// A test or debug build may talk to a local server instead of Google.
fn overridden(var: &str, default: &str) -> String {
    std::env::var(var).ok().filter(|v| !v.trim().is_empty() && (cfg!(test) || cfg!(debug_assertions))).unwrap_or_else(|| default.to_string())
}

/// The Google sign-in of Compass.
pub fn provider(cfg: &crate::config::AppConfig) -> Result<crate::user_oauth::Provider> {
    let client_id = cfg.gdrive_client_id.trim();
    if client_id.is_empty() {
        bail!("{}", crate::i18n::tr("Google Drive is not set up: give the client ID in Settings → Google Drive"));
    }
    Ok(crate::user_oauth::Provider {
        name: "Google Drive",
        auth_url: overridden("LOONAR_GDRIVE_AUTH_URL", AUTH_URL),
        token_url: overridden("LOONAR_GDRIVE_TOKEN_URL", TOKEN_URL),
        client_id: client_id.to_string(),
        client_secret: crate::config::secrets::get(crate::config::secrets::GDRIVE_CLIENT_SECRET).ok().filter(|s| !s.trim().is_empty()),
        scopes: "https://www.googleapis.com/auth/drive".to_string(),
        loopback_host: "127.0.0.1",
        secret: crate::config::secrets::GDRIVE_REFRESH_TOKEN,
        extra: "&access_type=offline&prompt=consent",
    })
}

/// Text inside a Drive query string: backslashes and single quotes escaped.
fn quoted(text: &str) -> String {
    text.replace('\\', "\\\\").replace('\'', "\\'")
}

/// A Drive file id rather than a name: long, letters, digits, "-" and "_" only.
fn looks_like_id(text: &str) -> bool {
    text.len() >= 20 && text.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

fn kind(item: &Value) -> &'static str {
    match item["mimeType"].as_str().unwrap_or("") {
        FOLDER => "folder",
        DOC => "Google Doc",
        SHEET => "Google Sheet",
        SLIDES => "Google Slides",
        m if m.starts_with("application/vnd.google-apps.") => "Google file",
        _ => "file",
    }
}

pub struct Drive {
    api: String,
    upload: String,
    token: String,
    http: reqwest::blocking::Client,
}

impl Drive {
    /// Signs in (from memory, by the refresh token or in the browser).
    pub fn connect(cfg: &crate::config::AppConfig) -> Result<Drive> {
        let token = crate::user_oauth::access_token(&provider(cfg)?)?;
        let api = overridden("LOONAR_GDRIVE_API", API).trim_end_matches('/').to_string();
        // https://www.googleapis.com/drive/v3 → https://www.googleapis.com/upload/drive/v3
        let upload = match api.find("/drive/") {
            Some(at) => format!("{}/upload{}", &api[..at], &api[at..]),
            None => format!("{api}/upload"),
        };
        let http = reqwest::blocking::Client::builder().timeout(Duration::from_secs(120)).redirect(crate::net::same_origin_redirects()).build()?;
        Ok(Drive { api, upload, token, http })
    }

    fn check(response: reqwest::blocking::Response) -> Result<reqwest::blocking::Response> {
        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().unwrap_or_default();
            let body: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
            let reason = body["error"]["message"].as_str().map(String::from).unwrap_or(text);
            bail!("{}", crate::i18n::fmt("Google Drive answered {0}: {1}", &[status.to_string(), reason]));
        }
        Ok(response)
    }

    fn get(&self, path: &str, query: &[(&str, &str)]) -> Result<reqwest::blocking::Response> {
        let mut query = query.to_vec();
        query.push(("supportsAllDrives", "true"));
        Self::check(self.http.get(format!("{}{path}", self.api)).bearer_auth(&self.token).query(&query).send()?)
    }

    /// Files matching a Drive query (at most LIST_LIMIT); true when more were not listed.
    fn search(&self, q: &str, order: Option<&str>) -> Result<(Vec<Value>, bool)> {
        let fields = format!("nextPageToken,files({FILE_FIELDS})");
        let size = LIST_LIMIT.to_string();
        let mut query = vec![("q", q), ("fields", fields.as_str()), ("pageSize", size.as_str()), ("includeItemsFromAllDrives", "true")];
        if let Some(order) = order {
            query.push(("orderBy", order));
        }
        let answer: Value = self.get("/files", &query)?.json()?;
        let mut files = answer["files"].as_array().cloned().unwrap_or_default();
        let more = files.len() > LIST_LIMIT || answer["nextPageToken"].as_str().is_some_and(|t| !t.is_empty());
        files.truncate(LIST_LIMIT);
        Ok((files, more))
    }

    /// The item named `name` in the folder `parent` (folders only when `folder_only`).
    fn child(&self, parent: &str, name: &str, folder_only: bool) -> Result<Option<Value>> {
        let mut q = format!("'{}' in parents and name = '{}' and trashed = false", quoted(parent), quoted(name));
        if folder_only {
            q.push_str(&format!(" and mimeType = '{FOLDER}'"));
        }
        Ok(self.search(&q, None)?.0.into_iter().next())
    }

    /// The id of a folder: "" or "root" = My Drive, an id, or a name path from the root ("Projects/2026").
    pub fn folder_id(&self, folder: &str) -> Result<String> {
        let folder = folder.trim().trim_matches('/');
        if folder.is_empty() || folder.eq_ignore_ascii_case("root") {
            return Ok("root".to_string());
        }
        if !folder.contains('/') && looks_like_id(folder) {
            return Ok(folder.to_string());
        }
        let mut id = "root".to_string();
        for part in folder.split('/').filter(|p| !p.trim().is_empty()) {
            let item = self.child(&id, part.trim(), true)?.with_context(|| crate::i18n::fmt("there is no folder {0} in Google Drive", &[folder.to_string()]))?;
            id = item["id"].as_str().unwrap_or("").to_string();
        }
        Ok(id)
    }

    /// The id of the folder to upload into: like `folder_id`, but missing folders of the name path are created, as
    /// OneDrive does (LC-282: "send it to the folder Compass-test" ended with "there is no folder" on Google Drive).
    pub fn upload_folder_id(&self, folder: &str) -> Result<String> {
        let folder = folder.trim().trim_matches('/');
        if folder.is_empty() || folder.eq_ignore_ascii_case("root") || (!folder.contains('/') && looks_like_id(folder)) {
            return self.folder_id(folder);
        }
        let mut id = "root".to_string();
        for part in folder.split('/').map(str::trim).filter(|p| !p.is_empty()) {
            id = match self.child(&id, part, true)? {
                Some(item) => item["id"].as_str().unwrap_or("").to_string(),
                None => self.create_folder(&id, part)?,
            };
        }
        Ok(id)
    }

    fn create_folder(&self, parent: &str, name: &str) -> Result<String> {
        let response = self
            .http
            .post(format!("{}/files", self.api))
            .bearer_auth(&self.token)
            .query(&[("supportsAllDrives", "true"), ("fields", "id")])
            .json(&json!({ "name": name, "mimeType": FOLDER, "parents": [parent] }))
            .send()?;
        let answer: Value = Self::check(response)?.json()?;
        answer["id"].as_str().map(String::from).with_context(|| crate::i18n::fmt("there is no folder {0} in Google Drive", &[name.to_string()]))
    }

    /// A file by its name path ("Projects/2026/Spec").
    pub fn item_at(&self, path: &str) -> Result<Value> {
        let path = path.trim().trim_matches('/');
        let (folder, name) = path.rsplit_once('/').unwrap_or(("", path));
        if name.trim().is_empty() {
            bail!("give id or path");
        }
        let parent = self.folder_id(folder)?;
        self.child(&parent, name.trim(), false)?.with_context(|| crate::i18n::fmt("there is no file {0} in Google Drive", &[path.to_string()]))
    }

    pub fn item(&self, id: &str) -> Result<Value> {
        Ok(self.get(&format!("/files/{}", encode(id)), &[("fields", FILE_FIELDS)])?.json()?)
    }

    /// The text of a file: Google Docs, Sheets and Slides exported, other files downloaded and read.
    pub fn text(&self, item: &Value) -> Result<String> {
        let id = item["id"].as_str().context("no file id from Google Drive")?;
        let name = item["name"].as_str().unwrap_or("");
        let export = match item["mimeType"].as_str().unwrap_or("") {
            FOLDER => bail!("{}", crate::i18n::fmt("{0} is a folder: list it instead", &[name.to_string()])),
            DOC | SLIDES => Some("text/plain"),
            SHEET => Some("text/csv"),
            m if m.starts_with("application/vnd.google-apps.") => bail!("{}", crate::i18n::fmt("{0} is a Google file of a type whose text cannot be read", &[name.to_string()])),
            _ => None,
        };
        if let Some(mime) = export {
            let bytes = self.get(&format!("/files/{}/export", encode(id)), &[("mimeType", mime)])?.bytes()?;
            return Ok(String::from_utf8_lossy(&bytes).trim_start_matches('\u{feff}').to_string());
        }
        let size = item["size"].as_str().and_then(|s| s.parse::<u64>().ok()).or(item["size"].as_u64()).unwrap_or(0);
        if size > MAX_DOWNLOAD {
            bail!("{}", crate::i18n::tr("the document is larger than 60 MB"));
        }
        let bytes = self.get(&format!("/files/{}", encode(id)), &[("alt", "media")])?.bytes()?;
        if bytes.len() as u64 > MAX_DOWNLOAD {
            bail!("{}", crate::i18n::tr("the document is larger than 60 MB"));
        }
        crate::documents::text_of(name, &bytes)
    }

    /// Uploads a file into a folder (by id); returns its web address.
    pub fn upload(&self, parent: &str, name: &str, bytes: &[u8]) -> Result<String> {
        let boundary = format!("loonar-{:016x}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(0));
        let metadata = json!({ "name": name, "parents": [parent] });
        let mut body = format!("--{boundary}\r\nContent-Type: application/json; charset=UTF-8\r\n\r\n{metadata}\r\n--{boundary}\r\nContent-Type: application/octet-stream\r\n\r\n").into_bytes();
        body.extend_from_slice(bytes);
        body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
        let response = self
            .http
            .post(format!("{}/files", self.upload))
            .bearer_auth(&self.token)
            .query(&[("uploadType", "multipart"), ("supportsAllDrives", "true"), ("fields", "id,name,webViewLink")])
            .header("Content-Type", format!("multipart/related; boundary={boundary}"))
            .body(body)
            .send()?;
        let answer: Value = Self::check(response)?.json()?;
        Ok(answer["webViewLink"].as_str().map(String::from).unwrap_or_else(|| format!("https://drive.google.com/file/d/{}/view", answer["id"].as_str().unwrap_or(""))))
    }
}

/// Settings → Google Drive → "Save, sign in and test": signs in and lists the root of My Drive.
pub fn check(cfg: &crate::config::AppConfig) -> Result<String> {
    let drive = Drive::connect(cfg)?;
    let (items, _) = drive.search("'root' in parents and trashed = false", None)?;
    Ok(crate::i18n::fmt("Saved. Signed in to Google Drive: {0} items in My Drive.", &[items.len().to_string()]))
}

/// One path segment of a URL (a file id) percent-encoded.
fn encode(text: &str) -> String {
    text.bytes().map(|b| if b.is_ascii_alphanumeric() || b"-._~".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") }).collect()
}

fn line(item: &Value) -> String {
    let size = item["size"].as_str().and_then(|s| s.parse::<u64>().ok()).or(item["size"].as_u64()).unwrap_or(0);
    format!(
        "{} | {} | {} | {} KB | {}",
        kind(item),
        item["name"].as_str().unwrap_or(""),
        item["id"].as_str().unwrap_or(""),
        size / 1024,
        item["modifiedTime"].as_str().unwrap_or("").chars().take(16).collect::<String>()
    )
}

/// The tool: list, read or upload.
pub fn run(input: &Value, ctx: &crate::tools::ToolContext) -> Result<String> {
    let action = input["action"].as_str().unwrap_or("list");
    // Checked before signing in: a refused upload never opens the browser.
    let upload = if action == "upload" {
        let given = input["file"].as_str().context("give file (a file in the attachments folder, e.g. one you created)")?.trim();
        if given.contains(['/', '\\', ':']) {
            bail!("{}", crate::i18n::tr("upload only files from the attachments folder (documents and presentations you created)"));
        }
        Some(attachment_named(&ctx.cfg.attachments_dir, given).with_context(|| crate::i18n::tr("upload only files from the attachments folder (documents and presentations you created)"))?)
    } else {
        None
    };
    if !matches!(action, "list" | "read" | "upload") {
        bail!("{}", crate::i18n::fmt("unknown action {0}: use list, read or upload", &[action.to_string()]));
    }
    let drive = Drive::connect(ctx.cfg)?;
    match action {
        "list" => {
            let search = input["query"].as_str().map(str::trim).filter(|q| !q.is_empty());
            let folder = input["folder"].as_str().unwrap_or("");
            let (title, (items, more)) = if let Some(text) = search {
                (format!("Google Drive files with \"{text}\" in the name"), drive.search(&format!("name contains '{}' and trashed = false", quoted(text)), None)?)
            } else {
                let id = drive.folder_id(folder)?;
                let shown = if id == "root" { "My Drive".to_string() } else { folder.trim_matches('/').to_string() };
                (format!("Google Drive folder {shown}"), drive.search(&format!("'{}' in parents and trashed = false", quoted(&id)), Some("folder,name"))?)
            };
            let lines: Vec<String> = items.iter().map(line).collect();
            let tail = if more { format!("\n[… more items: only the first {LIST_LIMIT} are listed]") } else { String::new() };
            Ok(format!("{title} ({} items; kind | name | id | size | modified):\n{}{tail}", lines.len(), lines.join("\n")))
        }
        "read" => {
            let item = match (input["id"].as_str().map(str::trim).filter(|s| !s.is_empty()), input["path"].as_str().map(str::trim).filter(|s| !s.is_empty())) {
                (Some(id), _) => drive.item(id)?,
                (None, Some(path)) => drive.item_at(path)?,
                (None, None) => bail!("give id or path"),
            };
            let name = item["name"].as_str().unwrap_or("").to_string();
            let text = drive.text(&item)?;
            Ok(format!("<external_data source=\"Google Drive {name}\">\n{}\n</external_data>", crate::documents::limited(text)))
        }
        _ => {
            let file = upload.unwrap_or_default();
            let local = ctx.cfg.attachments_dir.join(&file);
            let folder = input["folder"].as_str().unwrap_or("").trim().trim_matches('/');
            let name = input["name"].as_str().map(str::trim).filter(|n| !n.is_empty()).map(String::from).unwrap_or_else(|| crate::history::display_name(&file).to_string());
            let shown = if folder.is_empty() || folder.eq_ignore_ascii_case("root") { crate::i18n::tr("My Drive").to_string() } else { folder.to_string() };
            ctx.approver.confirm_upload(&crate::i18n::fmt(crate::tools::GDRIVE_UPLOAD_QUESTION, &[name.clone(), shown]))?;
            let parent = drive.upload_folder_id(folder)?;
            let url = drive.upload(&parent, &name, &std::fs::read(&local)?)?;
            Ok(format!("Uploaded to Google Drive: {url}"))
        }
    }
}

/// The file in the attachments folder: its stored name ("<id>_Name.docx") or, as the model usually says it, the
/// name the user sees ("Name.docx"); then the newest such file.
fn attachment_named(dir: &std::path::Path, given: &str) -> Option<String> {
    if dir.join(given).is_file() {
        return Some(given.to_string());
    }
    std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .filter(|e| e.path().is_file())
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            let shown = crate::history::display_name(&name).to_string();
            (shown.eq_ignore_ascii_case(given) || shown.eq_ignore_ascii_case(&crate::history::sanitize_name(given))).then(|| (e.metadata().and_then(|m| m.modified()).ok(), name))
        })
        .max_by_key(|(modified, _)| *modified)
        .map(|(_, name)| name)
}

pub fn definition() -> Value {
    json!({
        "name": "google_drive",
        "description": "The user's Google Drive (their own Google account, signed in in the browser on first use; set up in Settings): \
list a folder (action list, folder = folder id or name path like \"Projects/2026\", empty = My Drive) or find files by name (action list, query), \
read a file's text (action read, id or path: Google Docs, Sheets (as CSV), Slides, PDF, Word, PowerPoint, Visio, Excel, text) \
or upload a file you created in this conversation (action upload, file = its name in the attachments folder, folder, optional name; the user approves every upload; \
a folder of the path that does not exist yet is created by the upload, so upload straight to the folder the user named).",
        "input_schema": {
            "type": "object",
            "properties": {
                "action": { "type": "string", "enum": ["list", "read", "upload"] },
                "folder": { "type": "string", "description": "folder id or name path from My Drive (\"Projects/2026\"); empty or \"root\" = My Drive; upload creates missing folders" },
                "query": { "type": "string", "description": "list: text to find in file names (instead of folder)" },
                "id": { "type": "string", "description": "read: file id (from list)" },
                "path": { "type": "string", "description": "read: name path of the file (\"Projects/2026/Spec\")" },
                "file": { "type": "string", "description": "upload: the file's name in the attachments folder" },
                "name": { "type": "string", "description": "upload: name in Google Drive (default: the file's name)" }
            },
            "required": ["action"]
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader, Read, Write};

    fn decode(text: &str) -> String {
        let bytes = text.as_bytes();
        let mut out = Vec::new();
        let mut i = 0;
        while i < bytes.len() {
            match bytes[i] {
                b'%' if i + 2 < bytes.len() => {
                    out.push(u8::from_str_radix(std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or("3F"), 16).unwrap_or(b'?'));
                    i += 3;
                    continue;
                }
                b'+' => out.push(b' '),
                b => out.push(b),
            }
            i += 1;
        }
        String::from_utf8_lossy(&out).into_owned()
    }

    fn param(target: &str, name: &str) -> String {
        target.split_once('?').map(|(_, q)| q).unwrap_or("").split('&').find_map(|p| p.split_once('=').filter(|(k, _)| *k == name).map(|(_, v)| decode(v))).unwrap_or_default()
    }

    /// Google's token endpoint and Drive API on this computer.
    fn mock() -> String {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap().to_string();
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut first = String::new();
                reader.read_line(&mut first).unwrap();
                let (method, target) = { let mut p = first.split_whitespace(); (p.next().unwrap_or("").to_string(), p.next().unwrap_or("").to_string()) };
                let (mut length, mut auth) = (0, String::new());
                loop {
                    let mut h = String::new();
                    reader.read_line(&mut h).unwrap();
                    if h.trim().is_empty() { break; }
                    let lower = h.to_ascii_lowercase();
                    if let Some(v) = lower.strip_prefix("content-length:") { length = v.trim().parse().unwrap_or(0); }
                    if lower.starts_with("authorization:") { auth = h.trim().to_string(); }
                }
                let mut body = vec![0; length];
                reader.read_exact(&mut body).unwrap();
                let body = String::from_utf8_lossy(&body).to_string();
                let path = target.split('?').next().unwrap_or("").to_string();
                let q = param(&target, "q");
                let folder = |id: &str, name: &str| format!(r#"{{"id":"{id}","name":"{name}","mimeType":"{FOLDER}"}}"#);
                let files = |items: &[String]| format!(r#"{{"files":[{}]}}"#, items.join(",")).into_bytes();
                let (status, content): (u16, Vec<u8>) = if path == "/token" {
                    if method == "POST" && body.contains("grant_type=refresh_token") && body.contains("refresh_token=gd-refresh") && body.contains("client_id=gd-client") {
                        (200, br#"{"access_token":"gd-access","expires_in":3600}"#.to_vec())
                    } else {
                        (400, br#"{"error":"invalid_grant"}"#.to_vec())
                    }
                } else if auth != "authorization: Bearer gd-access" && auth != "Authorization: Bearer gd-access" {
                    (401, br#"{"error":{"message":"no token"}}"#.to_vec())
                } else if path == "/drive/v3/files" && method == "GET" {
                    if q == format!("'root' in parents and name = 'Projects' and trashed = false and mimeType = '{FOLDER}'") {
                        (200, files(&[folder("f-proj", "Projects")]))
                    } else if q == format!("'f-proj' in parents and name = '2026' and trashed = false and mimeType = '{FOLDER}'") {
                        (200, files(&[folder("f-2026", "2026")]))
                    } else if q == "'f-2026' in parents and trashed = false" && param(&target, "supportsAllDrives") == "true" && param(&target, "includeItemsFromAllDrives") == "true" {
                        (200, files(&[
                            folder("f-old", "Old"),
                            format!(r#"{{"id":"doc-1","name":"Spec","mimeType":"{DOC}","modifiedTime":"2026-10-20T10:11:12.000Z"}}"#),
                            format!(r#"{{"id":"sheet-1","name":"Budget","mimeType":"{SHEET}"}}"#),
                            r#"{"id":"md-1","name":"Order.md","mimeType":"text/markdown","size":"2048"}"#.to_string(),
                        ]))
                    } else if q == "'f-2026' in parents and name = 'Order.md' and trashed = false" {
                        (200, files(&[r#"{"id":"md-1","name":"Order.md","mimeType":"text/markdown","size":"2048"}"#.to_string()]))
                    } else if q == "name contains 'O\\'Brien' and trashed = false" {
                        (200, files(&[r#"{"id":"ob-1","name":"O'Brien notes.txt","mimeType":"text/plain","size":"10"}"#.to_string()]))
                    } else {
                        (200, files(&[]))
                    }
                } else if path == "/drive/v3/files/doc-1" {
                    (200, format!(r#"{{"id":"doc-1","name":"Spec","mimeType":"{DOC}"}}"#).into_bytes())
                } else if path == "/drive/v3/files/sheet-1" {
                    (200, format!(r#"{{"id":"sheet-1","name":"Budget","mimeType":"{SHEET}"}}"#).into_bytes())
                } else if path == "/drive/v3/files/doc-1/export" && param(&target, "mimeType") == "text/plain" {
                    (200, "\u{feff}W1 The customer places an order.".as_bytes().to_vec())
                } else if path == "/drive/v3/files/sheet-1/export" && param(&target, "mimeType") == "text/csv" {
                    (200, b"item,cost\nlicence,100".to_vec())
                } else if path == "/drive/v3/files/md-1" && param(&target, "alt") == "media" {
                    (200, b"# Order\nW2 The order is paid.".to_vec())
                } else if path == "/drive/v3/files" && method == "POST" {
                    // A missing folder of an upload path is created (LC-282).
                    if body.contains(r#""name":"Reports""#) && body.contains(FOLDER) && body.contains(r#""parents":["root"]"#) {
                        (200, br#"{"id":"f-new"}"#.to_vec())
                    } else {
                        (400, br#"{"error":{"message":"bad folder"}}"#.to_vec())
                    }
                } else if path == "/upload/drive/v3/files" && method == "POST" && param(&target, "uploadType") == "multipart" {
                    if body.contains(r#""parents":["f-new"]"#) && body.contains(r#""name":"Review.pptx""#) {
                        (200, br#"{"id":"up-2","name":"Review.pptx","webViewLink":"https://drive.google.com/file/d/up-2/view"}"#.to_vec())
                    } else if body.contains(r#""parents":["f-2026"]"#) && body.contains(r#""name":"Review.pptx""#) && body.contains("pptx bytes") {
                        (200, br#"{"id":"up-1","name":"Review.pptx","webViewLink":"https://drive.google.com/file/d/up-1/view"}"#.to_vec())
                    } else {
                        (400, br#"{"error":{"message":"bad multipart"}}"#.to_vec())
                    }
                } else {
                    (404, br#"{"error":{"message":"not found"}}"#.to_vec())
                };
                let mut out = stream;
                let _ = write!(out, "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", content.len());
                let _ = out.write_all(&content);
            }
        });
        addr
    }

    #[test]
    fn gdrive_round_trip() {
        crate::i18n::set(crate::i18n::Lang::En);
        assert_eq!(quoted("O'Brien"), "O\\'Brien");
        assert!(looks_like_id("1AbCdEfGhIjKlMnOpQrStUvWxYz") && !looks_like_id("Projects"));
        let addr = mock();
        std::env::set_var("LOONAR_GDRIVE_API", format!("http://{addr}/drive/v3"));
        std::env::set_var("LOONAR_GDRIVE_TOKEN_URL", format!("http://{addr}/token"));
        std::env::set_var("LOONAR_OAUTH_NO_BROWSER", "1");
        crate::config::secrets::use_environment();
        std::env::set_var("LOONAR_SECRET_GDRIVE_REFRESH_TOKEN", "gd-refresh");
        let dir = std::env::temp_dir().join(format!("loonar-gd-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("attachments")).unwrap();
        let props = dir.join("app.properties");
        std::fs::write(&props, "model.source=file\nmodel.file=C:/none.qea\ngdrive.client_id=gd-client\nattachments.dir=attachments\n").unwrap();
        let cfg = crate::config::AppConfig::load(&props).unwrap();
        let p = provider(&cfg).unwrap();
        assert_eq!(check(&cfg).unwrap(), "Saved. Signed in to Google Drive: 0 items in My Drive.");
        assert_eq!(p.loopback_host, "127.0.0.1");
        assert!(p.extra.contains("access_type=offline"));
        let approver = crate::tools::Approver::new(crate::config::ApprovalMode::Auto, Box::new(|_| true));
        let ctx = crate::tools::ToolContext { cfg: &cfg, approver: &approver };

        let listed = run(&json!({ "action": "list", "folder": "Projects/2026" }), &ctx).unwrap();
        assert!(listed.contains("folder | Old | f-old | 0 KB"), "{listed}");
        assert!(listed.contains("Google Doc | Spec | doc-1 | 0 KB | 2026-10-20T10:11"), "{listed}");
        assert!(listed.contains("Google Sheet | Budget | sheet-1"), "{listed}");
        assert!(listed.contains("file | Order.md | md-1 | 2 KB"), "{listed}");
        let found = run(&json!({ "action": "list", "query": "O'Brien" }), &ctx).unwrap();
        assert!(found.contains("file | O'Brien notes.txt | ob-1"), "quotes escaped: {found}");

        let doc = run(&json!({ "action": "read", "id": "doc-1" }), &ctx).unwrap();
        assert!(doc.starts_with("<external_data source=\"Google Drive Spec\">\nW1 The customer places an order."), "{doc}");
        let sheet = run(&json!({ "action": "read", "id": "sheet-1" }), &ctx).unwrap();
        assert!(sheet.contains("item,cost\nlicence,100"), "{sheet}");
        let md = run(&json!({ "action": "read", "path": "Projects/2026/Order.md" }), &ctx).unwrap();
        assert!(md.contains("W2 The order is paid.") && md.ends_with("</external_data>"), "{md}");

        std::fs::write(cfg.attachments_dir.join("27f7f074a214445481c5a853f29f5b70_Review.pptx"), b"pptx bytes").unwrap();
        let up = run(&json!({ "action": "upload", "file": "Review.pptx", "folder": "Projects/2026" }), &ctx).unwrap();
        assert_eq!(up, "Uploaded to Google Drive: https://drive.google.com/file/d/up-1/view");
        let into_new = run(&json!({ "action": "upload", "file": "Review.pptx", "folder": "Reports" }), &ctx).unwrap();
        assert_eq!(into_new, "Uploaded to Google Drive: https://drive.google.com/file/d/up-2/view", "a missing folder is created");
        assert!(run(&json!({ "action": "upload", "file": "..\\app.properties" }), &ctx).is_err(), "only attachments");
        assert!(run(&json!({ "action": "upload", "file": "app.properties" }), &ctx).is_err(), "only attachments");
        let asked = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
        let seen = asked.clone();
        let refusing = crate::tools::Approver::new(crate::config::ApprovalMode::Auto, Box::new(move |s| { *seen.lock().unwrap() = s; false }));
        let ctx2 = crate::tools::ToolContext { cfg: &cfg, approver: &refusing };
        assert!(run(&json!({ "action": "upload", "file": "Review.pptx", "folder": "Projects/2026" }), &ctx2).is_err(), "uploads are always asked");
        assert_eq!(asked.lock().unwrap().as_str(), "Upload Review.pptx to Google Drive: Projects/2026");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

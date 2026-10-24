//! SharePoint document libraries through Microsoft Graph: list a folder, read a document (its text, like
//! read_document) and upload a file the assistant made (always asked). Sign-in: an app registration in Entra ID
//! with the application permission Sites.Selected (or Sites.ReadWrite.All), OAuth2 client credentials.
//!
//! Settings: sharepoint.site (https://contoso.sharepoint.com/sites/Architecture), sharepoint.library (default
//! "Documents"), sharepoint.tenant, sharepoint.client_id; the secret sharepoint_client_secret.

use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use std::time::Duration;

const GRAPH: &str = "https://graph.microsoft.com/v1.0";
/// Files up to this size go in one request; larger ones through an upload session.
const SIMPLE_UPLOAD: usize = 4 * 1024 * 1024;

pub struct Site {
    graph: String,
    token: String,
    drive: String,
    http: reqwest::blocking::Client,
}

fn encode_path(path: &str) -> String {
    path.trim_matches('/')
        .split('/')
        .filter(|p| !p.is_empty())
        .map(|p| p.bytes().map(|b| if b.is_ascii_alphanumeric() || b"-._~ ".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") }).collect::<String>().replace(' ', "%20"))
        .collect::<Vec<_>>()
        .join("/")
}

impl Site {
    /// Signs in and finds the document library of the configured site.
    pub fn connect(cfg: &crate::config::AppConfig) -> Result<Site> {
        let site = cfg.sharepoint_site.trim().trim_end_matches('/');
        if site.is_empty() || cfg.sharepoint_client_id.trim().is_empty() || cfg.sharepoint_tenant.trim().is_empty() {
            bail!("{}", crate::i18n::tr("SharePoint is not set up: give the site, the tenant and the client id in Settings → SharePoint"));
        }
        let url = reqwest::Url::parse(site).with_context(|| crate::i18n::fmt("not an address: {0}", &[site.to_string()]))?;
        let graph = std::env::var("LOONAR_GRAPH_URL").ok().filter(|_| cfg!(debug_assertions) || cfg!(test)).unwrap_or_else(|| GRAPH.to_string());
        let token_url = std::env::var("LOONAR_GRAPH_TOKEN_URL").ok().filter(|_| cfg!(debug_assertions) || cfg!(test)).unwrap_or_else(|| format!("https://login.microsoftonline.com/{}/oauth2/v2.0/token", cfg.sharepoint_tenant.trim()));
        let secret = crate::config::secrets::get(crate::config::secrets::SHAREPOINT_SECRET)?;
        let token = crate::auth::client_credentials(&token_url, cfg.sharepoint_client_id.trim(), &secret, "https://graph.microsoft.com/.default")?;
        let http = reqwest::blocking::Client::builder().timeout(Duration::from_secs(120)).redirect(crate::net::same_origin_redirects()).build()?;
        let mut s = Site { graph, token, drive: String::new(), http };
        let host = url.host_str().unwrap_or("");
        let path = url.path().trim_end_matches('/');
        let site_info = s.get_json(&format!("/sites/{host}:{path}"))?;
        let site_id = site_info["id"].as_str().context("no site id from Microsoft Graph")?.to_string();
        let drives = s.get_json(&format!("/sites/{site_id}/drives"))?;
        let wanted = if cfg.sharepoint_library.trim().is_empty() { "Documents" } else { cfg.sharepoint_library.trim() };
        let drive = drives["value"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|d| d["name"].as_str().is_some_and(|n| n.eq_ignore_ascii_case(wanted)) || (wanted == "Documents" && d["name"] == "Shared Documents"))
            .with_context(|| crate::i18n::fmt("the site has no document library \"{0}\"", &[wanted.to_string()]))?;
        s.drive = drive["id"].as_str().unwrap_or("").to_string();
        Ok(s)
    }

    fn get(&self, path: &str) -> Result<reqwest::blocking::Response> {
        let response = self.http.get(format!("{}{path}", self.graph)).bearer_auth(&self.token).send()?;
        if !response.status().is_success() {
            let status = response.status();
            let body: Value = response.json().unwrap_or(Value::Null);
            bail!("{}", crate::i18n::fmt("Microsoft Graph answered {0}: {1}", &[status.to_string(), body["error"]["message"].as_str().unwrap_or("").to_string()]));
        }
        Ok(response)
    }

    fn get_json(&self, path: &str) -> Result<Value> {
        Ok(self.get(path)?.json()?)
    }

    /// The files and folders of a folder of the library ("" = the root).
    pub fn list(&self, folder: &str) -> Result<Vec<Value>> {
        let path = if folder.trim_matches('/').is_empty() { format!("/drives/{}/root/children", self.drive) } else { format!("/drives/{}/root:/{}:/children", self.drive, encode_path(folder)) };
        Ok(self.get_json(&path)?["value"].as_array().cloned().unwrap_or_default())
    }

    pub fn download(&self, path: &str) -> Result<Vec<u8>> {
        let response = self.get(&format!("/drives/{}/root:/{}:/content", self.drive, encode_path(path)))?;
        let bytes = response.bytes()?;
        if bytes.len() > 60 * 1024 * 1024 {
            bail!("{}", crate::i18n::tr("the document is larger than 60 MB"));
        }
        Ok(bytes.to_vec())
    }

    /// Uploads a file into a folder; returns its web address.
    pub fn upload(&self, folder: &str, name: &str, bytes: &[u8]) -> Result<String> {
        let target = encode_path(&format!("{}/{}", folder.trim_matches('/'), name));
        let answer: Value = if bytes.len() <= SIMPLE_UPLOAD {
            let response = self.http.put(format!("{}/drives/{}/root:/{target}:/content", self.graph, self.drive)).bearer_auth(&self.token).body(bytes.to_vec()).send()?;
            if !response.status().is_success() {
                bail!("{}", crate::i18n::fmt("Microsoft Graph answered {0}: {1}", &[response.status().to_string(), response.text().unwrap_or_default()]));
            }
            response.json()?
        } else {
            let session: Value = self
                .http
                .post(format!("{}/drives/{}/root:/{target}:/createUploadSession", self.graph, self.drive))
                .bearer_auth(&self.token)
                .json(&json!({ "item": { "@microsoft.graph.conflictBehavior": "replace" } }))
                .send()?
                .json()?;
            let upload = session["uploadUrl"].as_str().context("no upload session")?;
            let mut last = Value::Null;
            for (i, chunk) in bytes.chunks(5 * 320 * 1024).enumerate() {
                let start = i * 5 * 320 * 1024;
                let end = start + chunk.len() - 1;
                let response = self.http.put(upload).header("Content-Range", format!("bytes {start}-{end}/{}", bytes.len())).body(chunk.to_vec()).send()?;
                if !response.status().is_success() {
                    bail!("{}", crate::i18n::fmt("Microsoft Graph answered {0}: {1}", &[response.status().to_string(), response.text().unwrap_or_default()]));
                }
                last = response.json().unwrap_or(Value::Null);
            }
            last
        };
        Ok(answer["webUrl"].as_str().unwrap_or("").to_string())
    }
}

/// The tool: list, read or upload.
pub fn run(input: &Value, ctx: &crate::tools::ToolContext) -> Result<String> {
    let action = input["action"].as_str().unwrap_or("list");
    let site = Site::connect(ctx.cfg)?;
    match action {
        "list" => {
            let folder = input["folder"].as_str().unwrap_or("");
            let items = site.list(folder)?;
            let lines: Vec<String> = items
                .iter()
                .map(|i| {
                    let kind = if i["folder"].is_object() { "folder" } else { "file" };
                    format!("{kind} | {} | {} KB | {}", i["name"].as_str().unwrap_or(""), i["size"].as_u64().unwrap_or(0) / 1024, i["lastModifiedDateTime"].as_str().unwrap_or("").chars().take(16).collect::<String>())
                })
                .collect();
            Ok(format!("SharePoint folder /{} ({} items):\n{}", folder.trim_matches('/'), lines.len(), lines.join("\n")))
        }
        "read" => {
            let path = input["path"].as_str().context("give path")?;
            let bytes = site.download(path)?;
            let text = crate::documents::text_of(path, &bytes)?;
            Ok(format!("<external_data source=\"SharePoint {path}\">\n{}\n</external_data>", crate::documents::limited(text)))
        }
        "upload" => {
            let file = input["file"].as_str().context("give file (a file in the attachments folder, e.g. one you created)")?;
            let local = ctx.cfg.attachments_dir.join(file);
            if file.contains(['/', '\\', ':']) || !local.is_file() {
                bail!("{}", crate::i18n::tr("upload only files from the attachments folder (documents and presentations you created)"));
            }
            let folder = input["folder"].as_str().unwrap_or("");
            let name = input["name"].as_str().map(String::from).unwrap_or_else(|| crate::history::display_name(file).to_string());
            ctx.approver.confirm_upload(&crate::i18n::fmt("Upload {0} to SharePoint: {1}/{2}", &[name.clone(), ctx.cfg.sharepoint_site.trim().to_string(), format!("{}/{}", ctx.cfg.sharepoint_library.trim(), folder.trim_matches('/'))]))?;
            let url = site.upload(folder, &name, &std::fs::read(&local)?)?;
            Ok(format!("Uploaded to SharePoint: {url}"))
        }
        other => bail!("{}", crate::i18n::fmt("unknown action {0}: use list, read or upload", &[other.to_string()])),
    }
}

pub fn definition() -> Value {
    json!({
        "name": "sharepoint",
        "description": "The team's SharePoint document library (set up in Settings): list a folder (action list, folder), read a document's text (action read, path: PDF, Word, PowerPoint, Visio, Excel) \
or upload a file you created in this conversation (action upload, file = its name in the attachments folder, folder, optional name; the user approves every upload).",
        "input_schema": {
            "type": "object",
            "properties": {
                "action": { "type": "string", "enum": ["list", "read", "upload"] },
                "folder": { "type": "string" },
                "path": { "type": "string" },
                "file": { "type": "string" },
                "name": { "type": "string" }
            },
            "required": ["action"]
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader, Read, Write};

    /// A Microsoft Graph and token endpoint on this computer: site, drives, children, content, upload.
    fn mock() -> String {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap().to_string();
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                let (method, path) = { let mut p = line.split_whitespace(); (p.next().unwrap_or("").to_string(), p.next().unwrap_or("").to_string()) };
                let mut length = 0;
                loop {
                    let mut h = String::new();
                    reader.read_line(&mut h).unwrap();
                    if h.trim().is_empty() { break; }
                    if let Some(v) = h.to_ascii_lowercase().strip_prefix("content-length:") { length = v.trim().parse().unwrap_or(0); }
                }
                let mut body = vec![0; length];
                reader.read_exact(&mut body).unwrap();
                let (status, content): (u16, Vec<u8>) = if path.ends_with("/token") {
                    (200, br#"{"access_token":"graph-token","expires_in":3600}"#.to_vec())
                } else if path.contains("/sites/contoso.sharepoint.com:/sites/Arch") {
                    (200, br#"{"id":"site-1"}"#.to_vec())
                } else if path.ends_with("/sites/site-1/drives") {
                    (200, br#"{"value":[{"id":"drive-1","name":"Documents"}]}"#.to_vec())
                } else if path.ends_with("/root:/Specs:/children") {
                    (200, br#"{"value":[{"name":"Order.md","size":2048,"file":{}},{"name":"Old","folder":{}}]}"#.to_vec())
                } else if path.ends_with("/root:/Specs/Order.md:/content") && method == "GET" {
                    (200, b"# Order\nW1 The customer places an order.".to_vec())
                } else if method == "PUT" && path.contains("/root:/Reports/") {
                    (201, format!(r#"{{"webUrl":"https://contoso.sharepoint.com/sites/Arch/Documents/Reports/x ({} bytes)"}}"#, body.len()).into_bytes())
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
    fn library_round_trip() {
        crate::i18n::set(crate::i18n::Lang::En);
        let addr = mock();
        std::env::set_var("LOONAR_GRAPH_URL", format!("http://{addr}/v1.0"));
        std::env::set_var("LOONAR_GRAPH_TOKEN_URL", format!("http://{addr}/tenant/oauth2/v2.0/token"));
        crate::config::secrets::use_environment();
        std::env::set_var("LOONAR_SECRET_SHAREPOINT_CLIENT_SECRET", "sp-test-secret");
        let dir = std::env::temp_dir().join(format!("loonar-sp-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("attachments")).unwrap();
        let props = dir.join("app.properties");
        std::fs::write(&props, "model.source=file\nmodel.file=C:/none.qea\nsharepoint.site=https://contoso.sharepoint.com/sites/Arch\nsharepoint.tenant=t1\nsharepoint.client_id=c1\nattachments.dir=attachments\n").unwrap();
        let cfg = crate::config::AppConfig::load(&props).unwrap();
        let approver = crate::tools::Approver::new(crate::config::ApprovalMode::Auto, Box::new(|_| true));
        let ctx = crate::tools::ToolContext { cfg: &cfg, approver: &approver };
        let listed = run(&json!({ "action": "list", "folder": "Specs" }), &ctx).unwrap();
        assert!(listed.contains("file | Order.md") && listed.contains("folder | Old"), "{listed}");
        let read = run(&json!({ "action": "read", "path": "Specs/Order.md" }), &ctx).unwrap();
        assert!(read.contains("W1 The customer places an order"));
        std::fs::write(cfg.attachments_dir.join("abc_Review.pptx"), b"pptx bytes").unwrap();
        let up = run(&json!({ "action": "upload", "file": "abc_Review.pptx", "folder": "Reports" }), &ctx).unwrap();
        assert!(up.contains("Uploaded to SharePoint: https://contoso.sharepoint.com/sites/Arch/Documents/Reports/x (10 bytes)"), "{up}");
        assert!(run(&json!({ "action": "upload", "file": "..\\\\app.properties" }), &ctx).is_err(), "only attachments");
        let refusing = crate::tools::Approver::new(crate::config::ApprovalMode::Auto, Box::new(|_| false));
        let ctx2 = crate::tools::ToolContext { cfg: &cfg, approver: &refusing };
        assert!(run(&json!({ "action": "upload", "file": "abc_Review.pptx", "folder": "Reports" }), &ctx2).is_err(), "uploads are always asked");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

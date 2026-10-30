//! Confluence pages from the model: Markdown content with Confluence extensions converted to the storage
//! format (storage XHTML, to paste into the source editor or publish via the REST API),
//! diagrams and images as page attachments.
//!
//! Syntax extensions (besides headings, paragraphs, **bold**, `code`, lists, tables and [links](url)):
//! - `[TOC]` on its own line: table of contents,
//! - `:::info Title` … `:::` (also note, warning, tip, expand): panel or expand section,
//! - `{status:Green:YES}` (colours Grey, Red, Yellow, Green, Blue, Purple): status label,
//! - `{jira:PROJ-123}`: Jira issue, `{user:login}`: mention of a person,
//! - ` ```language ` … ` ``` `: code block,
//! - `![caption](diagram:ID)` / `![caption](image:NAME)`: image as a page attachment.

use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use std::path::Path;
use std::time::Duration;

/// Page image: attachment name and PNG.
pub struct Attachment {
    pub file_name: String,
    pub png: Vec<u8>,
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// Inline formatting: **bold**, `code`, [text](url), {status:…}, {jira:…}, {user:…}.
fn inline(text: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    while !rest.is_empty() {
        if let Some(after) = rest.strip_prefix("**") {
            if let Some(end) = after.find("**") {
                out.push_str(&format!("<strong>{}</strong>", inline(&after[..end])));
                rest = &after[end + 2..];
                continue;
            }
        }
        if let Some(after) = rest.strip_prefix('`') {
            if let Some(end) = after.find('`') {
                out.push_str(&format!("<code>{}</code>", escape(&after[..end])));
                rest = &after[end + 1..];
                continue;
            }
        }
        if let Some(after) = rest.strip_prefix('[') {
            if let Some((label, tail)) = after.split_once("](") {
                if let Some(end) = tail.find(')') {
                    if !label.contains(']') {
                        out.push_str(&format!("<a href=\"{}\">{}</a>", escape(&tail[..end]), inline(label)));
                        rest = &tail[end + 1..];
                        continue;
                    }
                }
            }
        }
        if let Some(after) = rest.strip_prefix('{') {
            if let Some(end) = after.find('}') {
                let inner = &after[..end];
                let macro_xml = if let Some(s) = inner.strip_prefix("status:") {
                    let (colour, title) = s.split_once(':').unwrap_or(("Grey", s));
                    Some(format!(
                        "<ac:structured-macro ac:name=\"status\"><ac:parameter ac:name=\"colour\">{}</ac:parameter><ac:parameter ac:name=\"title\">{}</ac:parameter></ac:structured-macro>",
                        escape(colour.trim()),
                        escape(title.trim())
                    ))
                } else if let Some(key) = inner.strip_prefix("jira:") {
                    Some(format!("<ac:structured-macro ac:name=\"jira\"><ac:parameter ac:name=\"key\">{}</ac:parameter></ac:structured-macro>", escape(key.trim())))
                } else {
                    inner.strip_prefix("user:").map(|login| format!("<ac:link><ri:user ri:username=\"{}\" /></ac:link>", escape(login.trim())))
                };
                if let Some(xml) = macro_xml {
                    out.push_str(&xml);
                    rest = &after[end + 1..];
                    continue;
                }
            }
        }
        let c = rest.chars().next().unwrap_or(' ');
        out.push_str(&escape(&c.to_string()));
        rest = &rest[c.len_utf8()..];
    }
    out
}

fn cells(line: &str) -> Vec<String> {
    line.trim().trim_matches('|').split('|').map(|c| c.trim().to_string()).collect()
}

/// Attachment name from the caption: letters, digits, hyphens.
fn attachment_name(caption: &str, index: usize) -> String {
    let mut name: String = caption
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    // At most 60 characters (not bytes: cutting inside a multi-byte letter such as "ł" would panic).
    name = name.chars().take(60).collect();
    if name.is_empty() {
        name = crate::i18n::fmt("image-{0}", &[index.to_string()]);
    }
    format!("{name}.png")
}

/// Converts the content to storage XHTML. `resolve(source)` returns the image PNG (e.g. `diagram:12`).
pub fn to_storage(markdown: &str, mut resolve: impl FnMut(&str) -> Result<Vec<u8>>) -> Result<(String, Vec<Attachment>)> {
    let mut out = String::new();
    let mut attachments: Vec<Attachment> = Vec::new();
    // Stack of open panels (:::info …): closing tags.
    let mut panels: Vec<&'static str> = Vec::new();
    let lines: Vec<&str> = markdown.lines().collect();
    let mut i = 0;
    let mut paragraph: Vec<String> = Vec::new();
    let flush = |paragraph: &mut Vec<String>, out: &mut String| {
        if !paragraph.is_empty() {
            out.push_str(&format!("<p>{}</p>", paragraph.join("<br />")));
            paragraph.clear();
        }
    };
    while i < lines.len() {
        let line = lines[i];
        let trimmed = line.trim();
        // Code block.
        if let Some(lang) = trimmed.strip_prefix("```") {
            flush(&mut paragraph, &mut out);
            let mut code = Vec::new();
            i += 1;
            while i < lines.len() && !lines[i].trim().starts_with("```") {
                code.push(lines[i]);
                i += 1;
            }
            let language = if lang.trim().is_empty() { String::new() } else { format!("<ac:parameter ac:name=\"language\">{}</ac:parameter>", escape(lang.trim())) };
            out.push_str(&format!(
                "<ac:structured-macro ac:name=\"code\">{language}<ac:plain-text-body><![CDATA[{}]]></ac:plain-text-body></ac:structured-macro>",
                code.join("\n").replace("]]>", "]]]]><![CDATA[>")
            ));
            i += 1;
            continue;
        }
        // Panels and expand sections.
        if let Some(spec) = trimmed.strip_prefix(":::") {
            flush(&mut paragraph, &mut out);
            let spec = spec.trim();
            if spec.is_empty() {
                if let Some(close) = panels.pop() {
                    out.push_str(close);
                }
            } else {
                let (kind, title) = spec.split_once(' ').map(|(k, t)| (k, t.trim())).unwrap_or((spec, ""));
                let kind = match kind.to_lowercase().as_str() {
                    "note" | "uwaga" => "note",
                    "warning" | "ostrzezenie" | "ostrzeżenie" => "warning",
                    "tip" | "wskazowka" | "wskazówka" => "tip",
                    "expand" | "rozwin" | "rozwiń" => "expand",
                    _ => "info",
                };
                out.push_str(&format!("<ac:structured-macro ac:name=\"{kind}\">"));
                if !title.is_empty() {
                    out.push_str(&format!("<ac:parameter ac:name=\"title\">{}</ac:parameter>", escape(title)));
                }
                out.push_str("<ac:rich-text-body>");
                panels.push("</ac:rich-text-body></ac:structured-macro>");
            }
            i += 1;
            continue;
        }
        if trimmed.eq_ignore_ascii_case("[toc]") {
            flush(&mut paragraph, &mut out);
            out.push_str("<ac:structured-macro ac:name=\"toc\" />");
            i += 1;
            continue;
        }
        // Table.
        if trimmed.starts_with('|') {
            flush(&mut paragraph, &mut out);
            out.push_str("<table><tbody>");
            let mut first = true;
            while i < lines.len() && lines[i].trim().starts_with('|') {
                let row = lines[i].trim();
                if row.chars().all(|c| matches!(c, '|' | '-' | ':' | ' ')) {
                    i += 1;
                    continue;
                }
                let tag = if first { "th" } else { "td" };
                out.push_str("<tr>");
                for cell in cells(row) {
                    out.push_str(&format!("<{tag}>{}</{tag}>", inline(&cell).replace("&lt;br&gt;", "<br />")));
                }
                out.push_str("</tr>");
                first = false;
                i += 1;
            }
            out.push_str("</tbody></table>");
            continue;
        }
        // Image.
        if let Some(rest) = trimmed.strip_prefix("![") {
            if let Some((caption, source)) = rest.split_once("](") {
                flush(&mut paragraph, &mut out);
                let source = source.trim_end_matches(')');
                let png = resolve(source).with_context(|| crate::i18n::fmt("image \"{0}\"", &[source.to_string()]))?;
                let mut name = attachment_name(caption, attachments.len() + 1);
                if attachments.iter().any(|a| a.file_name == name) {
                    name = format!("{}-{}.png", name.trim_end_matches(".png"), attachments.len() + 1);
                }
                out.push_str(&format!("<p><ac:image ac:align=\"center\"><ri:attachment ri:filename=\"{}\" /></ac:image></p>", escape(&name)));
                if !caption.trim().is_empty() {
                    out.push_str(&format!("<p style=\"text-align: center;\"><em>{}</em></p>", inline(caption.trim())));
                }
                attachments.push(Attachment { file_name: name, png });
                i += 1;
                continue;
            }
        }
        // Headings.
        let heading = [("#### ", "h4"), ("### ", "h3"), ("## ", "h2"), ("# ", "h1")].iter().find_map(|(p, tag)| trimmed.strip_prefix(p).map(|t| (*tag, t)));
        if let Some((tag, text)) = heading {
            flush(&mut paragraph, &mut out);
            out.push_str(&format!("<{tag}>{}</{tag}>", inline(text)));
            i += 1;
            continue;
        }
        // Lists (one level, or indented by two spaces per level).
        let bullet = |l: &str| l.trim_start().strip_prefix("- ").or_else(|| l.trim_start().strip_prefix("* ")).map(str::to_string);
        let numbered = |l: &str| {
            let t = l.trim_start();
            t.split_once(". ").filter(|(n, _)| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit())).map(|(_, rest)| rest.to_string())
        };
        if bullet(line).is_some() || numbered(line).is_some() {
            flush(&mut paragraph, &mut out);
            let ordered = numbered(line).is_some();
            let tag = if ordered { "ol" } else { "ul" };
            out.push_str(&format!("<{tag}>"));
            while i < lines.len() {
                let item = if ordered { numbered(lines[i]) } else { bullet(lines[i]) };
                let Some(item) = item else { break };
                out.push_str(&format!("<li>{}</li>", inline(&item)));
                i += 1;
            }
            out.push_str(&format!("</{tag}>"));
            continue;
        }
        if trimmed.is_empty() {
            flush(&mut paragraph, &mut out);
        } else {
            paragraph.push(inline(trimmed));
        }
        i += 1;
    }
    flush(&mut paragraph, &mut out);
    while let Some(close) = panels.pop() {
        out.push_str(close);
    }
    Ok((out, attachments))
}

/// Confluence connection settings.
pub struct Connection {
    /// Base URL, e.g. https://confluence.example.com or https://example.atlassian.net/wiki.
    pub base_url: String,
    /// Empty = personal access token (PAT, Confluence Data Center/Server); e-mail = Confluence Cloud (e-mail + API token).
    pub user: String,
    pub token: String,
    /// Client certificate for a server that asks for one besides the token (mTLS).
    pub identity: Option<reqwest::Identity>,
}

/// The Confluence address must use HTTPS (the token travels with every request); plain HTTP is accepted only
/// for a server on this computer.
/// The Confluence address without the part of a page copied from the browser (".../spaces/...", "/display/...").
pub fn base(url: &str) -> String {
    crate::net::site_base(url, &["/spaces", "/display", "/pages", "/rest", "/dashboard.action", "/plugins"])
}

pub fn check_base_url(url: &str) -> Result<()> {
    if crate::net::check_service_url(url).is_err() {
        bail!("{}", crate::i18n::tr("The Confluence address must start with https://"));
    }
    Ok(())
}

/// Page id: digits only (it is put into the request path).
pub fn check_page_id(id: &str) -> Result<()> {
    if id.is_empty() || !id.bytes().all(|b| b.is_ascii_digit()) {
        bail!("{}", crate::i18n::fmt("'{0}' is not a Confluence page ID (digits only)", &[id.to_string()]));
    }
    Ok(())
}

/// Space key: letters, digits, '_' and '~' (personal spaces).
pub fn check_space_key(key: &str) -> Result<()> {
    if key.is_empty() || !key.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'~') {
        bail!("{}", crate::i18n::fmt("'{0}' is not a Confluence space key", &[key.to_string()]));
    }
    Ok(())
}

/// Page that a publication would overwrite: id, current version number and title.
pub struct ExistingPage {
    pub id: String,
    pub version: i64,
    pub title: String,
}

impl Connection {
    fn client(&self) -> Result<reqwest::blocking::Client> {
        check_base_url(&self.base_url)?;
        let builder = reqwest::blocking::Client::builder().redirect(crate::net::same_origin_redirects()).timeout(Duration::from_secs(60));
        let builder = match &self.identity { Some(identity) => builder.identity(identity.clone()), None => builder };
        builder.build().context(crate::i18n::tr("HTTP client"))
    }

    /// The page that publishing would update: the given id, or a page with the same title in the space.
    pub fn existing_page(&self, space: &str, title: &str, page_id: Option<&str>) -> Result<Option<ExistingPage>> {
        let client = self.client()?;
        let page = match page_id.map(str::trim).filter(|id| !id.is_empty()) {
            Some(id) => {
                check_page_id(id)?;
                Some(self.send(client.get(self.url(&format!("/rest/api/content/{id}?expand=version,space"))), crate::i18n::tr("reading the page"))?)
            }
            None => {
                check_space_key(space.trim())?;
                let found = self.send(
                    client.get(self.url("/rest/api/content")).query(&[("spaceKey", space.trim()), ("title", title), ("expand", "version,space")]),
                    crate::i18n::tr("searching for the page"),
                )?;
                found["results"].as_array().and_then(|r| r.first()).cloned()
            }
        };
        Ok(page.and_then(|p| {
            Some(ExistingPage {
                id: p["id"].as_str()?.to_string(),
                version: p["version"]["number"].as_i64().unwrap_or(1),
                title: p["title"].as_str().unwrap_or(title).to_string(),
            })
        }))
    }

    fn auth(&self, request: reqwest::blocking::RequestBuilder) -> reqwest::blocking::RequestBuilder {
        if self.user.trim().is_empty() {
            request.bearer_auth(&self.token)
        } else {
            request.basic_auth(self.user.trim(), Some(&self.token))
        }
    }

    fn url(&self, path: &str) -> String {
        format!("{}{path}", base(&self.base_url))
    }

    fn send(&self, request: reqwest::blocking::RequestBuilder, what: &str) -> Result<Value> {
        let response = self.auth(request).header("Accept", "application/json").send().with_context(|| crate::i18n::fmt("{0}: no connection to Confluence", &[what.to_string()]))?;
        let status = response.status();
        let text = response.text().unwrap_or_default();
        if !status.is_success() {
            if crate::net::asks_for_client_certificate(status.as_u16(), &text) {
                bail!("{}", crate::i18n::fmt("{0}: the server asks for a client certificate (mTLS): choose its file (.p12, .pfx or .pem) and password in Settings", &[what.to_string()]));
            }
            let message = serde_json::from_str::<Value>(&text).ok().and_then(|v| v["message"].as_str().map(str::to_string)).unwrap_or_else(|| text.chars().take(300).collect());
            bail!("{}", crate::i18n::fmt("{0}: Confluence replied {1}: {2}", &[what.to_string(), status.to_string(), message]));
        }
        if crate::net::not_json(&text) {
            bail!("{}", crate::i18n::fmt("{0}: Confluence answered with a web page instead of data: check the Confluence address in Settings (e.g. https://confluence.company.com or https://company.com/confluence, without /spaces or /display)", &[what.to_string()]));
        }
        Ok(serde_json::from_str(&text).unwrap_or(Value::Null))
    }

    /// Title and storage-format body of a page (for reading requirements from it).
    pub fn page_body(&self, page_id: &str) -> Result<(String, String)> {
        check_page_id(page_id.trim())?;
        let client = self.client()?;
        let page = self.send(client.get(self.url(&format!("/rest/api/content/{}?expand=body.storage", page_id.trim()))), crate::i18n::tr("reading the page"))?;
        Ok((page["title"].as_str().unwrap_or("").to_string(), page["body"]["storage"]["value"].as_str().unwrap_or("").to_string()))
    }

    /// Connection check: the space exists and the token has access to it. Returns the space name.
    pub fn check(&self, space: &str) -> Result<String> {
        let client = self.client()?;
        let value = self.send(client.get(self.url(&format!("/rest/api/space/{}", space.trim()))), crate::i18n::tr("checking the space"))?;
        value["name"].as_str().map(str::to_string).with_context(|| crate::i18n::fmt("{0}: Confluence answered without the space: check the Confluence address in Settings", &[crate::i18n::tr("checking the space").to_string()]))
    }

    /// Creates or updates the page (by id, or without an id by title in the space) and uploads the attachments.
    /// Returns (page id, page URL, whether it was created).
    pub fn publish(
        &self,
        space: &str,
        title: &str,
        parent_id: Option<&str>,
        page_id: Option<&str>,
        storage: &str,
        attachments: &[Attachment],
    ) -> Result<(String, String, bool)> {
        let client = self.client()?;
        check_space_key(space.trim())?;
        if let Some(parent) = parent_id.map(str::trim).filter(|p| !p.is_empty()) {
            check_page_id(parent)?;
        }
        // Existing page: by id or by title in the space.
        let existing = match page_id.map(str::trim).filter(|id| !id.is_empty()) {
            Some(id) => {
                check_page_id(id)?;
                Some(self.send(client.get(self.url(&format!("/rest/api/content/{id}?expand=version,space"))), crate::i18n::tr("reading the page"))?)
            }
            None => {
                let found = self.send(
                    client.get(self.url("/rest/api/content")).query(&[("spaceKey", space.trim()), ("title", title), ("expand", "version,space")]),
                    crate::i18n::tr("searching for the page"),
                )?;
                found["results"].as_array().and_then(|r| r.first()).cloned()
            }
        };
        let (page, created) = match existing {
            Some(page) => {
                let id = page["id"].as_str().context(crate::i18n::tr("page without an id"))?.to_string();
                let version = page["version"]["number"].as_i64().unwrap_or(1) + 1;
                let space_key = page["space"]["key"].as_str().unwrap_or(space).to_string();
                let body = json!({
                    "id": id, "type": "page", "title": title, "space": { "key": space_key },
                    "body": { "storage": { "value": storage, "representation": "storage" } },
                    "version": { "number": version, "message": crate::config::APP_NAME }
                });
                (self.send(client.put(self.url(&format!("/rest/api/content/{id}"))).json(&body), crate::i18n::tr("updating the page"))?, false)
            }
            None => {
                let mut body = json!({
                    "type": "page", "title": title, "space": { "key": space.trim() },
                    "body": { "storage": { "value": storage, "representation": "storage" } }
                });
                if let Some(parent) = parent_id.filter(|p| !p.trim().is_empty()) {
                    body["ancestors"] = json!([{ "id": parent.trim() }]);
                }
                (self.send(client.post(self.url("/rest/api/content")).json(&body), crate::i18n::tr("creating the page"))?, true)
            }
        };
        let id = page["id"].as_str().context(crate::i18n::tr("Confluence did not return a page id"))?.to_string();
        for attachment in attachments {
            self.upload(&client, &id, attachment)?;
        }
        let link = format!(
            "{}{}",
            page["_links"]["base"].as_str().map(str::to_string).unwrap_or_else(|| self.base_url.trim().trim_end_matches('/').to_string()),
            page["_links"]["webui"].as_str().unwrap_or("")
        );
        Ok((id, link, created))
    }

    /// Page attachment: a new one, or a new version of an existing one with the same name.
    fn upload(&self, client: &reqwest::blocking::Client, page_id: &str, attachment: &Attachment) -> Result<()> {
        let existing = self.send(
            client.get(self.url(&format!("/rest/api/content/{page_id}/child/attachment"))).query(&[("filename", attachment.file_name.as_str())]),
            crate::i18n::tr("reading attachments"),
        )?;
        let path = match existing["results"].as_array().and_then(|r| r.first()).and_then(|a| a["id"].as_str()) {
            Some(att_id) => format!("/rest/api/content/{page_id}/child/attachment/{att_id}/data"),
            None => format!("/rest/api/content/{page_id}/child/attachment"),
        };
        let part = reqwest::blocking::multipart::Part::bytes(attachment.png.clone()).file_name(attachment.file_name.clone()).mime_str("image/png")?;
        let form = reqwest::blocking::multipart::Form::new().part("file", part);
        self.send(client.post(self.url(&path)).header("X-Atlassian-Token", "no-check").multipart(form), &crate::i18n::fmt("attachment {0}", &[attachment.file_name.clone()]))?;
        Ok(())
    }
}

/// Registry of published pages (for refreshing): page id, URL, title, space, source file, date.
pub fn remember(file: &Path, entry: Value) {
    let mut all: Vec<Value> = std::fs::read_to_string(file).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
    all.retain(|e| e["page_id"] != entry["page_id"]);
    all.push(entry);
    if let Some(dir) = file.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(file, serde_json::to_string_pretty(&all).unwrap_or_default());
}

pub fn published(file: &Path) -> Vec<Value> {
    std::fs::read_to_string(file).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    #[test]
    fn converts_markdown_with_confluence_macros() {
        let md = "[TOC]\n# Scope\nText with **important** and `code` and {status:Green:YES} and {jira:PROJ-1}.\n\n:::info Note\nPanel content\n:::\n\n- item <b>\n- second\n\n| System | Description |\n|---|---|\n| CRM | change |\n\n![Diagram 1](diagram:5)\n";
        let (xml, attachments) = super::to_storage(md, |_| Ok(vec![1, 2, 3])).unwrap();
        assert!(xml.contains("<ac:structured-macro ac:name=\"toc\" />"));
        assert!(xml.contains("<h1>Scope</h1>"));
        assert!(xml.contains("<strong>important</strong>") && xml.contains("<code>code</code>"));
        assert!(xml.contains("ac:name=\"status\"") && xml.contains(">YES<") && xml.contains("PROJ-1"));
        assert!(xml.contains("ac:name=\"info\"") && xml.contains(">Note<"));
        assert!(xml.contains("<li>item &lt;b&gt;</li>"), "HTML characters escaped: {xml}");
        assert!(xml.contains("<th>System</th>") && xml.contains("<td>CRM</td>"));
        assert_eq!(attachments[0].file_name, "Diagram-1.png");
        assert!(xml.contains("ri:filename=\"Diagram-1.png\""));
    }

    /// Publishing against a local server mimicking the Confluence REST API: creating a page with an attachment,
    /// then an update (version +1) and a new attachment version; personal access token as Bearer.
    #[test]
    fn publishes_and_updates_against_mock_server() {
        use std::io::{BufRead, BufReader, Read, Write};
        use std::sync::{Arc, Mutex};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let log: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let server_log = log.clone();
        std::thread::spawn(move || {
            let mut page_exists = false;
            let mut attachment_exists = false;
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { break };
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                loop {
                    let mut request_line = String::new();
                    if reader.read_line(&mut request_line).unwrap_or(0) == 0 {
                        break;
                    }
                    let mut length = 0usize;
                    let mut headers = String::new();
                    loop {
                        let mut line = String::new();
                        reader.read_line(&mut line).unwrap();
                        if line == "\r\n" || line.is_empty() {
                            break;
                        }
                        if let Some(v) = line.to_lowercase().strip_prefix("content-length:") {
                            length = v.trim().parse().unwrap_or(0);
                        }
                        headers.push_str(&line);
                    }
                    let mut body = vec![0u8; length];
                    reader.read_exact(&mut body).unwrap();
                    let body = String::from_utf8_lossy(&body).to_string();
                    let request = request_line.trim().to_string();
                    server_log.lock().unwrap().push(format!("{request}\n{headers}\n{body}"));
                    let path = request.split(' ').nth(1).unwrap_or("").to_string();
                    let reply = if request.starts_with("GET /rest/api/content?") {
                        if page_exists {
                            r#"{"results":[{"id":"1001","version":{"number":1},"space":{"key":"TEST"}}]}"#.to_string()
                        } else {
                            r#"{"results":[]}"#.to_string()
                        }
                    } else if request.starts_with("POST /rest/api/content ") || request.starts_with("PUT /rest/api/content/1001 ") {
                        page_exists = true;
                        format!(r#"{{"id":"1001","_links":{{"base":"http://127.0.0.1:{port}","webui":"/pages/1001"}}}}"#)
                    } else if path.contains("/child/attachment") && request.starts_with("GET") {
                        if attachment_exists { r#"{"results":[{"id":"att7"}]}"#.to_string() } else { r#"{"results":[]}"#.to_string() }
                    } else if path.contains("/child/attachment") && request.starts_with("POST") {
                        attachment_exists = true;
                        r#"{"results":[]}"#.to_string()
                    } else {
                        r#"{}"#.to_string()
                    };
                    let _ = write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{reply}", reply.len());
                }
            }
        });
        let connection = super::Connection { base_url: format!("http://127.0.0.1:{port}"), user: String::new(), token: "pat-123".into(), identity: None };
        let pictures = || vec![super::Attachment { file_name: "Diagram-1.png".into(), png: vec![137, 80, 78, 71] }];
        let (id, url, created) = connection.publish("TEST", "Test page", Some("500"), None, "<p>a</p>", &pictures()).unwrap();
        assert_eq!((id.as_str(), created), ("1001", true));
        assert!(url.ends_with("/pages/1001"));
        let (_, _, created_again) = connection.publish("TEST", "Test page", None, None, "<p>b</p>", &pictures()).unwrap();
        assert!(!created_again, "the second publish updates the page");
        let log = log.lock().unwrap().join("\n=====\n");
        assert!(log.contains("authorization: Bearer pat-123") || log.contains("Authorization: Bearer pat-123"), "personal access token as Bearer");
        assert!(log.contains(r#""ancestors":[{"id":"500"}]"#), "parent page on creation");
        assert!(log.contains(r#""number":2"#), "update with version 2");
        assert!(log.to_lowercase().contains("x-atlassian-token: no-check"), "header on attachment uploads");
        assert!(log.contains("POST /rest/api/content/1001/child/attachment/att7/data"), "new version of the existing attachment");
    }

    #[test]
    fn long_captions_with_non_ascii_letters_make_valid_names() {
        // 60 bytes would fall inside "ł"/"ż": the name is cut by characters.
        let caption = "Proces obsługi zamówienia w sklepie internetowym z płatnością i wysyłką żółtą";
        let name = super::attachment_name(caption, 1);
        assert!(name.ends_with(".png") && name.chars().count() <= 64, "{name}");
        assert_eq!(super::attachment_name("!!!", 3), crate::i18n::fmt("image-{0}", &["3".to_string()]) + ".png");
    }

    #[test]
    fn rejects_plain_http_ids_and_keys_that_break_the_path() {
        assert!(super::check_base_url("https://example.atlassian.net/wiki").is_ok());
        assert!(super::check_base_url("http://localhost:8090").is_ok());
        assert!(super::check_base_url("http://confluence.example.com").is_err());
        assert!(super::check_base_url("http://localhost.example.com").is_err());
        assert!(super::check_page_id("12345").is_ok());
        assert!(super::check_page_id("12/../34").is_err());
        assert!(super::check_space_key("~jsmith").is_ok() && super::check_space_key("TEAM_1").is_ok());
        assert!(super::check_space_key("A B").is_err());
    }
}
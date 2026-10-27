//! Jira as a source of context for the model: searching issues (JQL), reading an issue with its description,
//! comments and links, and on request creating an issue, adding a comment or a link to a model element.
//! Jira Cloud (https://company.atlassian.net, account e-mail + API token, REST API 3 with ADF descriptions) and
//! Jira Server / Data Center (personal access token, REST API 2 with plain-text descriptions).

use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use std::time::Duration;

/// Fields read for lists of issues.
const LIST_FIELDS: &str = "summary,status,issuetype,priority,assignee,labels,updated";

pub struct Connection {
    pub base_url: String,
    /// Account e-mail (Cloud); empty for a personal access token (Server / Data Center).
    pub user: String,
    pub token: String,
    /// Client certificate for a server that asks for one besides the token (mTLS).
    pub identity: Option<reqwest::Identity>,
}

/// The Jira address must use HTTPS (the token travels with every request); plain HTTP only for a server on this computer.
pub fn check_base_url(url: &str) -> Result<()> {
    if crate::net::check_service_url(url).is_err() {
        bail!("{}", crate::i18n::tr("The Jira address must start with https://"));
    }
    Ok(())
}

/// Issue key, e.g. ARCH-123 (it goes into the request path).
pub fn check_issue_key(key: &str) -> Result<()> {
    let ok = key.split_once('-').is_some_and(|(project, number)| {
        !project.is_empty() && project.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_') && !number.is_empty() && number.bytes().all(|b| b.is_ascii_digit())
    });
    if !ok {
        bail!("{}", crate::i18n::fmt("'{0}' is not a Jira issue key (e.g. ARCH-123)", &[key.to_string()]));
    }
    Ok(())
}

/// Project key: letters, digits and '_'.
pub fn check_project_key(key: &str) -> Result<()> {
    if key.is_empty() || !key.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_') {
        bail!("{}", crate::i18n::fmt("'{0}' is not a Jira project key", &[key.to_string()]));
    }
    Ok(())
}

/// Text of an Atlassian Document Format value (Cloud descriptions and comments); plain text is returned as it is.
pub fn adf_text(value: &Value) -> String {
    fn walk(node: &Value, out: &mut String) {
        if let Some(text) = node["text"].as_str() {
            out.push_str(text);
        }
        match node["type"].as_str() {
            Some("hardBreak") => out.push('\n'),
            Some("mention") => out.push_str(node["attrs"]["text"].as_str().unwrap_or("")),
            _ => {}
        }
        for child in node["content"].as_array().into_iter().flatten() {
            walk(child, out);
        }
        if matches!(node["type"].as_str(), Some("paragraph" | "heading" | "listItem" | "codeBlock" | "tableRow" | "blockquote")) {
            out.push('\n');
        }
    }
    match value {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        other => {
            let mut out = String::new();
            walk(other, &mut out);
            out.trim_end().to_string()
        }
    }
}

/// Plain text as an Atlassian Document Format document: one paragraph per line, empty lines skipped.
pub fn to_adf(text: &str) -> Value {
    let paragraphs: Vec<Value> = text
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| json!({ "type": "paragraph", "content": [{ "type": "text", "text": l }] }))
        .collect();
    json!({ "type": "doc", "version": 1, "content": paragraphs })
}

/// The Jira address without the part of a page copied from the browser (".../browse/KEY-1", "/projects/...").
pub fn base(url: &str) -> String {
    crate::net::site_base(url, &["/browse", "/projects", "/secure", "/rest", "/issues", "/plugins"])
}

impl Connection {
    /// Jira Cloud (atlassian.net, or an account e-mail with an API token) uses REST API 3, the server editions API 2.
    pub fn cloud(&self) -> bool {
        self.base_url.to_ascii_lowercase().contains(".atlassian.net") || !self.user.trim().is_empty()
    }

    fn api(&self) -> &'static str {
        if self.cloud() { "/rest/api/3" } else { "/rest/api/2" }
    }

    fn client(&self) -> Result<reqwest::blocking::Client> {
        check_base_url(&self.base_url)?;
        let builder = reqwest::blocking::Client::builder().redirect(crate::net::same_origin_redirects()).timeout(Duration::from_secs(60));
        let builder = match &self.identity { Some(identity) => builder.identity(identity.clone()), None => builder };
        builder.build().context(crate::i18n::tr("HTTP client"))
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}{path}", base(&self.base_url), self.api())
    }

    fn send(&self, request: reqwest::blocking::RequestBuilder, what: &str) -> Result<Value> {
        let request = if self.user.trim().is_empty() { request.bearer_auth(&self.token) } else { request.basic_auth(self.user.trim(), Some(&self.token)) };
        let response = request.header("Accept", "application/json").send().with_context(|| crate::i18n::fmt("{0}: no connection to Jira", &[what.to_string()]))?;
        let status = response.status();
        let text = response.text().unwrap_or_default();
        if !status.is_success() {
            let parsed = serde_json::from_str::<Value>(&text).ok();
            let message = parsed
                .as_ref()
                .and_then(|v| {
                    let mut parts: Vec<String> = v["errorMessages"].as_array().into_iter().flatten().filter_map(|m| m.as_str().map(String::from)).collect();
                    if let Some(errors) = v["errors"].as_object() {
                        parts.extend(errors.iter().map(|(k, m)| format!("{k}: {}", m.as_str().unwrap_or(""))));
                    }
                    (!parts.is_empty()).then(|| parts.join("; "))
                })
                .unwrap_or_else(|| text.chars().take(300).collect());
            bail!("{}", crate::i18n::fmt("{0}: Jira replied {1}: {2}", &[what.to_string(), status.to_string(), message]));
        }
        if crate::net::not_json(&text) {
            bail!("{}", crate::i18n::fmt("{0}: Jira answered with a web page instead of data: check the Jira address in Settings (e.g. https://jira.company.com or https://company.com/jira, without /browse or /projects)", &[what.to_string()]));
        }
        Ok(serde_json::from_str(&text).unwrap_or(Value::Null))
    }

    /// Connection check: the project exists and the token has access to it. Returns the project name.
    pub fn check(&self, project: &str) -> Result<String> {
        check_project_key(project.trim())?;
        let value = self.send(self.client()?.get(self.url(&format!("/project/{}", project.trim()))), crate::i18n::tr("checking the project"))?;
        Ok(value["name"].as_str().unwrap_or(project).to_string())
    }

    /// Issues found by JQL: one line per issue (key | type | status | priority | assignee | summary | labels | updated).
    pub fn search(&self, jql: &str, max: usize) -> Result<String> {
        let client = self.client()?;
        let max = max.clamp(1, 100).to_string();
        // Cloud: /search/jql (the older /search is being withdrawn there); servers: /search.
        let path = if self.cloud() { "/search/jql" } else { "/search" };
        let value = self.send(client.get(self.url(path)).query(&[("jql", jql), ("maxResults", max.as_str()), ("fields", LIST_FIELDS)]), crate::i18n::tr("searching issues"))?;
        let issues = value["issues"].as_array().cloned().unwrap_or_default();
        let mut lines = vec![crate::i18n::fmt("Jira issues for \"{0}\": {1}", &[jql.to_string(), issues.len().to_string()])];
        lines.push(crate::i18n::tr("Columns: key | type | status | priority | assignee | summary | labels | updated").to_string());
        for issue in &issues {
            let f = &issue["fields"];
            let labels: Vec<&str> = f["labels"].as_array().into_iter().flatten().filter_map(|l| l.as_str()).collect();
            lines.push(format!(
                "{} | {} | {} | {} | {} | {} | {} | {}",
                issue["key"].as_str().unwrap_or(""),
                f["issuetype"]["name"].as_str().unwrap_or(""),
                f["status"]["name"].as_str().unwrap_or(""),
                f["priority"]["name"].as_str().unwrap_or(""),
                f["assignee"]["displayName"].as_str().unwrap_or("-"),
                f["summary"].as_str().unwrap_or(""),
                labels.join(","),
                f["updated"].as_str().unwrap_or("").get(..10).unwrap_or("")
            ));
        }
        Ok(lines.join("\n"))
    }

    /// One issue: summary, type, status, description, links to other issues and web pages, the latest comments.
    pub fn issue(&self, key: &str) -> Result<String> {
        check_issue_key(key)?;
        let client = self.client()?;
        let value = self.send(
            client.get(self.url(&format!("/issue/{key}"))).query(&[("fields", "summary,status,issuetype,priority,assignee,reporter,labels,description,issuelinks,comment,parent,fixVersions,components,updated")]),
            crate::i18n::tr("reading the issue"),
        )?;
        let f = &value["fields"];
        let mut out = vec![format!("{key}: {}", f["summary"].as_str().unwrap_or(""))];
        let names = |list: &Value| list.as_array().into_iter().flatten().filter_map(|v| v["name"].as_str().or(v.as_str())).collect::<Vec<_>>().join(", ");
        out.push(format!(
            "{}: {} | {}: {} | {}: {} | {}: {}",
            crate::i18n::tr("type"), f["issuetype"]["name"].as_str().unwrap_or(""),
            crate::i18n::tr("status"), f["status"]["name"].as_str().unwrap_or(""),
            crate::i18n::tr("priority"), f["priority"]["name"].as_str().unwrap_or(""),
            crate::i18n::tr("assignee"), f["assignee"]["displayName"].as_str().unwrap_or("-")
        ));
        if let Some(parent) = f["parent"]["key"].as_str() {
            out.push(format!("{}: {parent} {}", crate::i18n::tr("parent"), f["parent"]["fields"]["summary"].as_str().unwrap_or("")));
        }
        for (label, field) in [(crate::i18n::tr("labels"), "labels"), (crate::i18n::tr("components"), "components"), (crate::i18n::tr("versions"), "fixVersions")] {
            let list = names(&f[field]);
            if !list.is_empty() {
                out.push(format!("{label}: {list}"));
            }
        }
        out.push(format!("\n{}:\n{}", crate::i18n::tr("Description"), adf_text(&f["description"])));
        let links: Vec<String> = f["issuelinks"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|l| {
                let (relation, other) = if l["outwardIssue"].is_object() { (l["type"]["outward"].as_str(), &l["outwardIssue"]) } else { (l["type"]["inward"].as_str(), &l["inwardIssue"]) };
                Some(format!("{} {} {}", relation.unwrap_or("relates to"), other["key"].as_str()?, other["fields"]["summary"].as_str().unwrap_or("")))
            })
            .collect();
        if !links.is_empty() {
            out.push(format!("\n{}:\n{}", crate::i18n::tr("Linked issues"), links.join("\n")));
        }
        if let Ok(remote) = self.send(client.get(self.url(&format!("/issue/{key}/remotelink"))), crate::i18n::tr("reading the issue")) {
            let web: Vec<String> = remote.as_array().into_iter().flatten().map(|r| format!("{} {}", r["object"]["title"].as_str().unwrap_or(""), r["object"]["url"].as_str().unwrap_or(""))).collect();
            if !web.is_empty() {
                out.push(format!("\n{}:\n{}", crate::i18n::tr("Web links"), web.join("\n")));
            }
        }
        let comments = f["comment"]["comments"].as_array().cloned().unwrap_or_default();
        if !comments.is_empty() {
            out.push(format!("\n{}:", crate::i18n::fmt("Comments ({0}, the latest 10)", &[comments.len().to_string()])));
            for c in comments.iter().rev().take(10).rev() {
                out.push(format!("- {} ({}): {}", c["author"]["displayName"].as_str().unwrap_or(""), c["created"].as_str().unwrap_or("").get(..10).unwrap_or(""), adf_text(&c["body"]).replace('\n', " ")));
            }
        }
        Ok(out.join("\n"))
    }

    fn text_value(&self, text: &str) -> Value {
        if self.cloud() { to_adf(text) } else { json!(text) }
    }

    /// Creates an issue in the project. Returns its key and web address.
    pub fn create_issue(&self, project: &str, issue_type: &str, summary: &str, description: &str, labels: &[String], parent: Option<&str>) -> Result<(String, String)> {
        check_project_key(project.trim())?;
        let mut fields = json!({
            "project": { "key": project.trim() },
            "issuetype": { "name": if issue_type.trim().is_empty() { "Task" } else { issue_type.trim() } },
            "summary": summary,
            "description": self.text_value(description),
        });
        if !labels.is_empty() {
            // Jira labels have no spaces.
            fields["labels"] = json!(labels.iter().map(|l| l.trim().replace(' ', "_")).filter(|l| !l.is_empty()).collect::<Vec<_>>());
        }
        if let Some(parent) = parent.map(str::trim).filter(|p| !p.is_empty()) {
            check_issue_key(parent)?;
            fields["parent"] = json!({ "key": parent });
        }
        let created = self.send(self.client()?.post(self.url("/issue")).json(&json!({ "fields": fields })), crate::i18n::tr("creating the issue"))?;
        let key = created["key"].as_str().context(crate::i18n::tr("Jira did not return the issue key"))?.to_string();
        Ok((key.clone(), self.browse(&key)))
    }

    pub fn add_comment(&self, key: &str, text: &str) -> Result<()> {
        check_issue_key(key)?;
        self.send(self.client()?.post(self.url(&format!("/issue/{key}/comment"))).json(&json!({ "body": self.text_value(text) })), crate::i18n::tr("adding the comment"))?;
        Ok(())
    }

    /// Web link on the issue (e.g. to the model element: its GUID in the title, so the link can be found again).
    pub fn add_web_link(&self, key: &str, url: &str, title: &str) -> Result<()> {
        check_issue_key(key)?;
        let body = json!({ "globalId": format!("loonar-compass:{url}"), "object": { "url": url, "title": title } });
        self.send(self.client()?.post(self.url(&format!("/issue/{key}/remotelink"))).json(&body), crate::i18n::tr("adding the link"))?;
        Ok(())
    }

    pub fn browse(&self, key: &str) -> String {
        format!("{}/browse/{key}", self.base_url.trim().trim_end_matches('/'))
    }
}

#[cfg(test)]
pub mod mock {
    //! A Jira imitation on localhost for the tests: Cloud (API 3, ADF, basic auth) or Server (API 2, text, bearer).
    use serde_json::{json, Value};
    use std::io::{BufRead, BufReader, Read, Write};
    use std::net::TcpListener;
    use std::sync::{Arc, Mutex};

    pub struct Mock {
        pub url: String,
        /// Requests received: (method, path with query, body).
        pub requests: Arc<Mutex<Vec<(String, String, Value)>>>,
    }

    pub fn start(cloud: bool, token: &'static str) -> Mock {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
        let requests: Arc<Mutex<Vec<(String, String, Value)>>> = Default::default();
        let log = requests.clone();
        std::thread::spawn(move || {
            let mut next = 101;
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { continue };
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut first = String::new();
                if reader.read_line(&mut first).is_err() {
                    continue;
                }
                let mut length = 0;
                let mut auth = String::new();
                loop {
                    let mut line = String::new();
                    reader.read_line(&mut line).unwrap_or(0);
                    let l = line.trim_end();
                    if l.is_empty() {
                        break;
                    }
                    let lower = l.to_ascii_lowercase();
                    if let Some(v) = lower.strip_prefix("content-length:") {
                        length = v.trim().parse().unwrap_or(0);
                    }
                    if lower.starts_with("authorization:") {
                        auth = l["authorization:".len()..].trim().to_string();
                    }
                }
                let mut body = vec![0u8; length];
                let _ = reader.read_exact(&mut body);
                let mut parts = first.split_whitespace();
                let (method, path) = (parts.next().unwrap_or("").to_string(), parts.next().unwrap_or("").to_string());
                let body: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
                log.lock().unwrap().push((method.clone(), path.clone(), body.clone()));
                let api = if cloud { "/rest/api/3" } else { "/rest/api/2" };
                let authorized = if cloud {
                    use base64::Engine;
                    auth == format!("Basic {}", base64::engine::general_purpose::STANDARD.encode(format!("architect@example.com:{token}")))
                } else {
                    auth == format!("Bearer {token}")
                };
                let text = |t: &str| if cloud { json!({"type":"doc","version":1,"content":[{"type":"paragraph","content":[{"type":"text","text":t}]}]}) } else { json!(t) };
                let issue = json!({"key":"ARCH-7","fields":{"summary":"Online payments","issuetype":{"name":"Story"},"status":{"name":"In Progress"},
                    "priority":{"name":"High"},"assignee":{"displayName":"Ann Analyst"},"labels":["payments","checkout"],"updated":"2026-10-20T10:00:00.000+0200",
                    "description": text("The shop accepts card payments through a payment gateway."),
                    "issuelinks":[{"type":{"outward":"blocks","inward":"is blocked by"},"outwardIssue":{"key":"ARCH-9","fields":{"summary":"Invoice e-mail"}}}],
                    "comment":{"comments":[{"author":{"displayName":"Bob Owner"},"created":"2026-10-21T09:00:00.000+0200","body":text("Use the existing Billing component.")}]}}});
                let (status, reply) = if !authorized {
                    (401, json!({"errorMessages":["Unauthorized"]}))
                } else if !path.starts_with(api) {
                    (404, json!({"errorMessages":["wrong API version"]}))
                } else {
                    let rest = &path[api.len()..];
                    match (method.as_str(), rest) {
                        ("GET", p) if p.starts_with("/project/ARCH") => (200, json!({"key":"ARCH","name":"Architecture"})),
                        ("GET", p) if p.starts_with("/project/") => (404, json!({"errorMessages":["No project could be found with key 'X'."]})),
                        ("GET", p) if p.starts_with(if cloud { "/search/jql?" } else { "/search?" }) => (200, json!({"issues":[issue.clone(), {"key":"ARCH-9","fields":{"summary":"Invoice e-mail","issuetype":{"name":"Task"},"status":{"name":"To Do"},"labels":[]}}]})),
                        ("GET", p) if p.starts_with("/issue/ARCH-7/remotelink") => (200, json!([{"object":{"title":"EA: Billing","url":"ea://{GUID}"}}])),
                        ("GET", p) if p.starts_with("/issue/ARCH-7") => (200, issue.clone()),
                        ("GET", p) if p.starts_with("/issue/") => (404, json!({"errorMessages":["Issue does not exist or you do not have permission to see it."]})),
                        ("POST", "/issue") => {
                            let ok_description = if cloud { body["fields"]["description"]["type"] == "doc" } else { body["fields"]["description"].is_string() };
                            if body["fields"]["summary"].as_str().unwrap_or("").is_empty() || !ok_description {
                                (400, json!({"errors":{"summary":"You must specify a summary of the issue."}}))
                            } else {
                                next += 1;
                                (201, json!({"id": next.to_string(), "key": format!("ARCH-{next}")}))
                            }
                        }
                        ("POST", p) if p.ends_with("/comment") => (201, json!({"id":"1"})),
                        ("POST", p) if p.ends_with("/remotelink") => (201, json!({"id":1})),
                        _ => (404, json!({"errorMessages":["not found"]})),
                    }
                };
                let text = reply.to_string();
                let _ = write!(stream, "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{text}", text.len());
                let _ = stream.flush();
            }
        });
        Mock { url, requests }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn connection(mock: &mock::Mock, cloud: bool, token: &str) -> Connection {
        Connection { base_url: mock.url.clone(), user: if cloud { "architect@example.com".into() } else { String::new() }, token: token.into(), identity: None }
    }

    #[test]
    fn keys_and_addresses() {
        assert!(check_issue_key("ARCH-12").is_ok());
        assert!(check_issue_key("ARCH-12/../x").is_err());
        assert!(check_issue_key("12").is_err());
        assert!(check_project_key("ARCH").is_ok());
        assert!(check_project_key("AR CH").is_err());
        assert!(check_base_url("https://x.atlassian.net").is_ok());
        assert!(check_base_url("http://jira.example.com").is_err());
        assert!(check_base_url("http://localhost:8080").is_ok());
        assert_eq!(adf_text(&to_adf("One\n\nTwo")), "One\nTwo");
    }

    /// Both editions against the imitation: check, search, issue, create, comment, link, and the errors shown to the user.
    #[test]
    fn cloud_and_server_against_mock() {
        for cloud in [true, false] {
            let mock = mock::start(cloud, "secret-1");
            let jira = connection(&mock, cloud, "secret-1");
            assert_eq!(jira.cloud(), cloud);
            assert_eq!(jira.check("ARCH").unwrap(), "Architecture");
            let missing = jira.check("NOPE").unwrap_err().to_string();
            assert!(missing.contains("404") && missing.contains("No project"), "{missing}");

            let found = jira.search("project = ARCH AND labels = payments", 20).unwrap();
            assert!(found.contains("ARCH-7 | Story | In Progress | High | Ann Analyst | Online payments | payments,checkout | 2026-10-20"), "{found}");
            assert!(found.contains("ARCH-9 | Task | To Do"), "{found}");

            let issue = jira.issue("ARCH-7").unwrap();
            assert!(issue.contains("card payments through a payment gateway"), "{issue}");
            assert!(issue.contains("blocks ARCH-9 Invoice e-mail"), "{issue}");
            assert!(issue.contains("Bob Owner") && issue.contains("existing Billing component"), "{issue}");
            assert!(issue.contains("EA: Billing ea://{GUID}"), "{issue}");
            assert!(jira.issue("ARCH-404").unwrap_err().to_string().contains("does not exist"));

            let (key, url) = jira.create_issue("ARCH", "Story", "Model: PaymentGateway", "Line 1\nLine 2", &["from model".into()], None).unwrap();
            assert_eq!(key, "ARCH-102");
            assert_eq!(url, format!("{}/browse/ARCH-102", mock.url));
            let bad = jira.create_issue("ARCH", "Story", "", "x", &[], None).unwrap_err().to_string();
            assert!(bad.contains("400") && bad.contains("summary"), "{bad}");
            jira.add_comment("ARCH-7", "Checked against the model.").unwrap();
            jira.add_web_link("ARCH-7", "ea://{1234}", "EA: PaymentGateway").unwrap();

            let requests = mock.requests.lock().unwrap();
            let created = requests.iter().find(|r| r.0 == "POST" && r.1.ends_with("/issue")).unwrap();
            assert_eq!(created.2["fields"]["labels"], json!(["from_model"]));
            if cloud {
                assert_eq!(created.2["fields"]["description"]["content"].as_array().unwrap().len(), 2, "ADF paragraphs");
                assert!(requests.iter().any(|r| r.1.starts_with("/rest/api/3/search/jql?")));
            } else {
                assert_eq!(created.2["fields"]["description"], json!("Line 1\nLine 2"));
                assert!(requests.iter().any(|r| r.1.starts_with("/rest/api/2/search?")));
            }
            drop(requests);

            let wrong = connection(&mock, cloud, "bad").check("ARCH").unwrap_err().to_string();
            assert!(wrong.contains("401"), "{wrong}");
        }
    }
}

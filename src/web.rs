//! Reading web pages for providers without server-side web tools (OpenAI and compatible gateways): the `web_fetch`
//! tool, run by Compass itself. With Claude the assistant uses Anthropic's own web search and web fetch instead.
//!
//! The rules keep a page from becoming a way to send the model out or to reach the company network:
//! - only an address the user wrote in the conversation (or one a fetched page or a tool result contained),
//!   never one the assistant made up, so data cannot be smuggled out in a query string;
//! - http or https only, no user name or password, the allowed domains if there are any;
//! - never an address of this computer or of a private network (also after every redirect);
//! - GET only, a time limit, at most 2 MB read and 20 000 characters of text passed on.
use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use std::io::Read;
use std::net::{IpAddr, ToSocketAddrs};
use std::time::Duration;

const MAX_BYTES: u64 = 2 * 1024 * 1024;
const MAX_TEXT: usize = 20_000;

pub fn definition() -> Value {
    json!({
        "name": "web_fetch",
        "description": "Reads a web page and returns its text. Only for an address the user gave in the conversation (or that a page read earlier linked to); \
do not guess addresses. Use it when the user asks about a page, documentation or an article on the internet.",
        "input_schema": {
            "type": "object",
            "properties": { "url": { "type": "string", "description": "The full address (https://…) exactly as it appears in the conversation." } },
            "required": ["url"]
        }
    })
}

/// Whether the address appears in what the user or the tools put into the conversation (not in the assistant's own text).
pub fn seen_in_conversation(url: &str, messages: &[Value]) -> bool {
    let wanted = url.trim().trim_end_matches('/');
    !wanted.is_empty() && messages.iter().filter(|m| m["role"] == "user").any(|m| m["content"].to_string().replace("\\/", "/").contains(wanted))
}

/// An address of this computer, of a private network, link-local or otherwise not public.
fn private(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => v4.is_loopback() || v4.is_private() || v4.is_link_local() || v4.is_unspecified() || v4.is_broadcast() || v4.is_documentation()
            || v4.octets()[0] == 100 && (64..128).contains(&v4.octets()[1]) || v4.octets()[0] == 0,
        IpAddr::V6(v6) => v6.is_loopback() || v6.is_unspecified() || (v6.segments()[0] & 0xfe00) == 0xfc00 || (v6.segments()[0] & 0xffc0) == 0xfe80
            || v6.to_ipv4_mapped().is_some_and(|v4| private(IpAddr::V4(v4))),
    }
}

/// Checks one address (also every redirect): scheme, credentials, allowed domains, and that the host is public.
fn check(url: &reqwest::Url, domains: &[String]) -> Result<()> {
    if !matches!(url.scheme(), "http" | "https") {
        bail!("only http and https addresses can be read");
    }
    if !url.username().is_empty() || url.password().is_some() {
        bail!("an address with a user name or password is not read");
    }
    let host = url.host_str().context("the address has no host")?.to_lowercase();
    if !domains.is_empty() && !domains.iter().any(|d| host == *d || host.ends_with(&format!(".{d}"))) {
        bail!("{host} is not among the allowed domains ({})", domains.join(", "));
    }
    let port = url.port_or_known_default().unwrap_or(443);
    let addresses: Vec<IpAddr> = (host.trim_matches(['[', ']']), port).to_socket_addrs().with_context(|| format!("cannot resolve {host}"))?.map(|a| a.ip()).collect();
    if addresses.is_empty() || addresses.iter().any(|ip| private(*ip)) {
        bail!("{host} is an address of this computer or of a private network: it is not read");
    }
    Ok(())
}

/// Text of an HTML page: without scripts, styles and tags, with paragraphs kept as lines.
pub(crate) fn html_text(html: &str) -> String {
    let lower = html.to_lowercase();
    let mut text = String::new();
    let mut i = 0;
    while i < html.len() {
        let rest = &lower[i..];
        if let Some(tag) = ["<script", "<style", "<noscript", "<svg"].iter().find(|t| rest.starts_with(**t)) {
            // Past the closing tag, including its ">".
            let close = format!("</{}", &tag[1..]);
            i = lower[i..].find(&close).and_then(|e| lower[i + e..].find('>').map(|g| i + e + g + 1)).unwrap_or(html.len());
            continue;
        }
        if rest.starts_with('<') {
            let end = lower[i..].find('>').map(|e| i + e + 1).unwrap_or(html.len());
            if ["<p", "<br", "<li", "<h1", "<h2", "<h3", "<h4", "<tr", "<div", "</p", "</li", "</h"].iter().any(|t| rest.starts_with(t)) {
                text.push('\n');
            }
            i = end;
            continue;
        }
        let next = html[i..].find('<').map(|e| i + e).unwrap_or(html.len());
        text.push_str(&html[i..next]);
        i = next;
    }
    let text = text.replace("&nbsp;", " ").replace("&amp;", "&").replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&#39;", "'");
    text.lines().map(|l| l.split_whitespace().collect::<Vec<_>>().join(" ")).filter(|l| !l.is_empty()).collect::<Vec<_>>().join("\n")
}

/// Reads the page (the rules above) and returns its address, title and text.
pub fn fetch(url: &str, domains: &[String]) -> Result<String> {
    let mut current = reqwest::Url::parse(url.trim()).context("not a valid address")?;
    let client = reqwest::blocking::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(20))
        .user_agent(concat!("LoonarCompass/", env!("CARGO_PKG_VERSION")))
        .build()?;
    for _ in 0..5 {
        check(&current, domains)?;
        let response = client.get(current.clone()).send().with_context(|| format!("cannot read {current}"))?;
        if response.status().is_redirection() {
            let location = response.headers().get(reqwest::header::LOCATION).and_then(|l| l.to_str().ok()).context("a redirect without an address")?;
            current = current.join(location).context("a redirect to an invalid address")?;
            continue;
        }
        if !response.status().is_success() {
            bail!("{current} answered {}", response.status());
        }
        let html = response.headers().get(reqwest::header::CONTENT_TYPE).and_then(|c| c.to_str().ok()).is_none_or(|c| c.contains("html"));
        let mut body = Vec::new();
        response.take(MAX_BYTES).read_to_end(&mut body)?;
        let raw = String::from_utf8_lossy(&body);
        let title = if html {
            let lower = raw.to_lowercase();
            lower.find("<title").and_then(|s| lower[s..].find('>').map(|g| s + g + 1)).and_then(|s| lower[s..].find("</title").map(|e| raw[s..s + e].trim().to_string())).unwrap_or_default()
        } else {
            String::new()
        };
        let mut text = if html { html_text(&raw) } else { raw.to_string() };
        if text.chars().count() > MAX_TEXT {
            text = text.chars().take(MAX_TEXT).collect::<String>() + "\n[…the rest of the page is left out]";
        }
        return Ok(format!("Address: {current}\nTitle: {title}\n\n{text}"));
    }
    bail!("too many redirects")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_public_addresses_the_user_gave() {
        let messages = vec![
            json!({ "role": "user", "content": "Przeczytaj https://example.com/docs/page i streść." }),
            json!({ "role": "assistant", "content": "Sprawdzę też https://evil.example/?data=model" }),
        ];
        assert!(seen_in_conversation("https://example.com/docs/page", &messages));
        assert!(!seen_in_conversation("https://evil.example/?data=model", &messages), "not an address the assistant wrote itself");
        for ip in ["127.0.0.1", "10.1.2.3", "192.168.0.5", "172.16.0.1", "169.254.169.254", "100.64.0.1", "::1", "fd00::1", "fe80::1"] {
            assert!(private(ip.parse().unwrap()), "{ip}");
        }
        assert!(!private("93.184.216.34".parse().unwrap()));
        assert!(check(&reqwest::Url::parse("http://127.0.0.1:8730/").unwrap(), &[]).is_err());
        assert!(check(&reqwest::Url::parse("file:///c:/x").unwrap(), &[]).is_err());
        assert!(check(&reqwest::Url::parse("https://user:pw@example.com/").unwrap(), &[]).is_err());
        assert!(check(&reqwest::Url::parse("https://example.org/").unwrap(), &["example.com".to_string()]).is_err());
        assert_eq!(html_text("<html><head><style>x{}</style><script>alert(1)</script></head><body><h1>Tytuł</h1><p>Akapit &amp; więcej</p></body></html>"), "Tytuł\nAkapit & więcej");
    }
}

//! Network rules shared by the HTTP clients of the application (not used by the installer).

/// Redirects of API clients: only within the same scheme, host and port (a redirect elsewhere would carry API keys
/// in headers such as x-api-key, which reqwest does not remove, and could downgrade to plain http).
pub fn same_origin_redirects() -> reqwest::redirect::Policy {
    reqwest::redirect::Policy::custom(|attempt| {
        let same = attempt.previous().first().is_some_and(|first| {
            first.scheme() == attempt.url().scheme() && first.host_str() == attempt.url().host_str() && first.port_or_known_default() == attempt.url().port_or_known_default()
        });
        if same && attempt.previous().len() < 5 { attempt.follow() } else { attempt.stop() }
    })
}

/// An address of a service: https://…, or http:// only for this computer (localhost, 127.0.0.1, [::1]); no user name
/// or password in the address.
pub fn check_service_url(url: &str) -> Result<(), ()> {
    let parsed = reqwest::Url::parse(url.trim()).map_err(|_| ())?;
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err(());
    }
    match parsed.scheme() {
        "https" => Ok(()),
        "http" if matches!(parsed.host_str(), Some("localhost") | Some("127.0.0.1") | Some("[::1]") | Some("::1")) => Ok(()),
        _ => Err(()),
    }
}

/// The address of a site from an address a person copied from the browser: everything from the first of `pages`
/// (e.g. "/spaces/" of Confluence or "/browse/" of Jira) is left out (LC-285: ".../confluence/spaces" as the address
/// sent every request to a web page instead of the REST API).
pub fn site_base(url: &str, pages: &[&str]) -> String {
    let url = url.trim().trim_end_matches('/');
    let lower = url.to_ascii_lowercase();
    let scheme_end = lower.find("://").map_or(0, |i| i + 3);
    let cut = pages
        .iter()
        .filter_map(|p| {
            let p = p.trim_end_matches('/');
            lower[scheme_end..].find(&format!("{p}/")).or_else(|| lower[scheme_end..].ends_with(p).then(|| lower.len() - scheme_end - p.len()))
        })
        .min();
    match cut {
        Some(at) => url[..scheme_end + at].trim_end_matches('/').to_string(),
        None => url.to_string(),
    }
}

/// A successful answer that is not data: a web page (sign-in, an address that is not the REST API).
/// Whether an address answers a REST request with a web page instead of data (checked without signing in, so a wrong
/// address is reported also before a token is entered). False when it cannot be reached at all.
pub fn answers_with_page(url: &str) -> bool {
    let Ok(client) = reqwest::blocking::Client::builder().redirect(same_origin_redirects()).timeout(std::time::Duration::from_secs(15)).build() else { return false };
    client.get(url).header("Accept", "application/json").send().ok().and_then(|r| r.text().ok()).is_some_and(|t| not_json(&t))
}

/// The key a copied address names: …/projects/KEY…, …/browse/KEY-12, …/spaces/KEY…, …/display/KEY/…
pub fn key_in_address(url: &str, kinds: &[&str]) -> Option<String> {
    for kind in kinds {
        if let Some(i) = url.find(&format!("/{kind}/")) {
            let rest = &url[i + kind.len() + 2..];
            let key: String = rest.chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '~').collect();
            let key = if *kind == "browse" { key.split('-').next().unwrap_or("").to_string() } else { key };
            let key = key.trim_end_matches(|c: char| c.is_ascii_digit() && *kind == "browse").to_string();
            if !key.is_empty() && key.chars().next().is_some_and(|c| c.is_ascii_alphabetic() || c == '~') {
                return Some(key);
            }
        }
    }
    None
}

pub fn not_json(text: &str) -> bool {
    let t = text.trim_start();
    !t.is_empty() && serde_json::from_str::<serde_json::Value>(t).is_err()
}

#[cfg(test)]
mod site_base_tests {
    #[test]
    fn keys_in_copied_addresses() {
        assert_eq!(super::key_in_address("https://x.atlassian.net/jira/software/projects/LC/boards/1", &["projects", "browse"]).as_deref(), Some("LC"));
        assert_eq!(super::key_in_address("https://jira.firma.pl/browse/ARCH-12", &["projects", "browse"]).as_deref(), Some("ARCH"));
        assert_eq!(super::key_in_address("https://x.atlassian.net/wiki/spaces/PM/pages/1/Title", &["spaces", "display"]).as_deref(), Some("PM"));
        assert_eq!(super::key_in_address("https://jira.firma.pl", &["projects", "browse"]), None);
        assert_eq!(crate::jira::base("https://x.atlassian.net/jira/software/projects/LC/boards/1"), "https://x.atlassian.net");
        assert_eq!(crate::jira::base("https://firma.pl/jira/browse/LC-1"), "https://firma.pl/jira");
    }

    #[test]
    fn copied_addresses() {
        let c = ["/spaces", "/display", "/pages", "/rest", "/dashboard.action"];
        assert_eq!(super::site_base("https://jira.example.com/confluence/spaces", &c), "https://jira.example.com/confluence");
        assert_eq!(super::site_base("https://jira.example.com/confluence/spaces/P1805/pages/447655091/Wymagania", &c), "https://jira.example.com/confluence");
        assert_eq!(super::site_base("https://x.atlassian.net/wiki/spaces/LC/overview", &c), "https://x.atlassian.net/wiki");
        assert_eq!(super::site_base("https://confluence.example.com/display/ARCH/Home", &c), "https://confluence.example.com");
        assert_eq!(super::site_base("https://confluence.example.com/", &c), "https://confluence.example.com");
        let j = ["/browse", "/projects", "/secure", "/rest", "/issues"];
        assert_eq!(super::site_base("https://jira.example.com/jira/browse/ITSPX-1", &j), "https://jira.example.com/jira");
        assert_eq!(super::site_base("https://jira.example.com/jira", &j), "https://jira.example.com/jira");
        assert!(super::not_json("<!DOCTYPE html><html>"));
        assert!(!super::not_json("") && !super::not_json("{\"a\":1}"));
    }
}

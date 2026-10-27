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
pub fn not_json(text: &str) -> bool {
    let t = text.trim_start();
    !t.is_empty() && serde_json::from_str::<serde_json::Value>(t).is_err()
}

#[cfg(test)]
mod site_base_tests {
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

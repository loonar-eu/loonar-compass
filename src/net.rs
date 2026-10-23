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

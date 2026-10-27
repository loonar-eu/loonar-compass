//! Sign-in to services that are not plain API keys: HTTP Basic and OAuth2 client credentials (a token from the
//! organisation's identity provider: Entra ID for Azure AI Foundry, an API gateway, an MCP server behind SSO).
//! Tokens are kept in memory until shortly before they expire.

use anyhow::{bail, Context, Result};
use base64::Engine;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

static TOKENS: Mutex<Option<HashMap<String, (String, Instant)>>> = Mutex::new(None);

/// The value of an Authorization header for HTTP Basic.
pub fn basic(user: &str, password: &str) -> String {
    format!("Basic {}", base64::engine::general_purpose::STANDARD.encode(format!("{user}:{password}")))
}

/// A bearer token with the OAuth2 client credentials flow (cached until a minute before it expires).
pub fn client_credentials(token_url: &str, client_id: &str, client_secret: &str, scope: &str) -> Result<String> {
    if crate::net::check_service_url(token_url).is_err() {
        bail!("{}", crate::i18n::fmt("the token address {0} must be https:// (http:// only on this computer)", &[token_url.to_string()]));
    }
    let key = format!("{token_url}\u{1}{client_id}\u{1}{scope}");
    if let Some((token, until)) = TOKENS.lock().unwrap().as_ref().and_then(|m| m.get(&key)).cloned() {
        if Instant::now() < until {
            return Ok(token);
        }
    }
    let http = reqwest::blocking::Client::builder().timeout(Duration::from_secs(20)).redirect(reqwest::redirect::Policy::none()).build()?;
    let mut form = vec![("grant_type", "client_credentials"), ("client_id", client_id), ("client_secret", client_secret)];
    if !scope.trim().is_empty() {
        form.push(("scope", scope));
    }
    let body = form.iter().map(|(k, v)| format!("{k}={}", url_encode(v))).collect::<Vec<_>>().join("&");
    let response = http.post(token_url).header("Content-Type", "application/x-www-form-urlencoded").body(body).send().with_context(|| crate::i18n::fmt("cannot reach {0}", &[token_url.to_string()]))?;
    let status = response.status();
    let answer: Value = response.json().unwrap_or(Value::Null);
    if !status.is_success() {
        bail!("{}", crate::i18n::fmt("the identity provider refused the sign-in: {0}", &[answer["error_description"].as_str().or(answer["error"].as_str()).unwrap_or(status.as_str()).to_string()]));
    }
    let token = answer["access_token"].as_str().context("no access_token in the answer of the identity provider")?.to_string();
    let lifetime = answer["expires_in"].as_u64().or_else(|| answer["expires_in"].as_str().and_then(|s| s.parse().ok())).unwrap_or(3600);
    let until = Instant::now() + Duration::from_secs(lifetime.saturating_sub(60).max(30));
    TOKENS.lock().unwrap().get_or_insert_with(HashMap::new).insert(key, (token.clone(), until));
    Ok(token)
}

fn url_encode(text: &str) -> String {
    text.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_header() {
        assert_eq!(basic("anna", "s3cret"), "Basic YW5uYTpzM2NyZXQ=");
    }

    /// Client credentials against the local Keycloak of the hub tests (client compass-hub with service accounts on):
    /// `cargo test client_credentials_keycloak -- --ignored`
    #[test]
    #[ignore = "requires the local Keycloak with a service account client"]
    fn client_credentials_keycloak() {
        let token = client_credentials("http://127.0.0.1:8180/realms/compass/protocol/openid-connect/token", "compass-gateway", "lab-test-gateway-secret-7a2", "").unwrap();
        assert!(token.split('.').count() == 3, "a JWT");
        let again = client_credentials("http://127.0.0.1:8180/realms/compass/protocol/openid-connect/token", "compass-gateway", "lab-test-gateway-secret-7a2", "").unwrap();
        assert_eq!(token, again, "cached");
        assert!(client_credentials("http://127.0.0.1:8180/realms/compass/protocol/openid-connect/token", "compass-gateway", "wrong", "x").is_err());
    }
}

//! Signing in with the user's own account to a cloud drive (OneDrive, Google Drive) from Compass: OAuth 2.0
//! authorisation code with PKCE in the browser, the answer comes to a one-time address on this computer
//! (http://localhost:<port>/ or http://127.0.0.1:<port>/). The refresh token is kept in Windows Credential Manager, the
//! access token only in memory; the browser opens again only when the refresh token no longer works (LC-281, LC-282).
use anyhow::{bail, Context, Result};
use base64::Engine;
use serde_json::Value;
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// How long Compass waits for the person to finish the sign-in in the browser.
const WAIT: Duration = Duration::from_secs(300);

/// One provider of the sign-in.
pub struct Provider {
    /// Shown in messages, e.g. "OneDrive".
    pub name: &'static str,
    pub auth_url: String,
    pub token_url: String,
    pub client_id: String,
    /// Google's desktop clients have a (not secret) client secret; Microsoft's public clients none.
    pub client_secret: Option<String>,
    pub scopes: String,
    /// The host of the one-time address: Microsoft wants "localhost", Google takes "127.0.0.1".
    pub loopback_host: &'static str,
    /// The Credential Manager name of the refresh token.
    pub secret: &'static str,
    /// Extra parameters of the sign-in page (e.g. access_type=offline and prompt=consent for Google).
    pub extra: &'static str,
}

/// Access tokens in memory: provider secret name → (token, valid until).
static TOKENS: Mutex<Option<HashMap<&'static str, (String, Instant)>>> = Mutex::new(None);

fn b64url(bytes: &[u8]) -> String {
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

fn random() -> String {
    let mut bytes = [0u8; 32];
    ring::rand::SecureRandom::fill(&ring::rand::SystemRandom::new(), &mut bytes).expect("random");
    b64url(&bytes)
}

/// The PKCE challenge of a verifier (S256).
pub fn challenge(verifier: &str) -> String {
    use sha2::Digest;
    b64url(&sha2::Sha256::digest(verifier.as_bytes()))
}

fn encode(text: &str) -> String {
    text.bytes().map(|b| if b.is_ascii_alphanumeric() || b"-._~".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") }).collect()
}

fn decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' if i + 2 < bytes.len() => {
                out.push(u8::from_str_radix(std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or("00"), 16).unwrap_or(b'?'));
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

fn query_value(query: &str, name: &str) -> Option<String> {
    query.split('&').find_map(|pair| {
        let (k, v) = pair.split_once('=')?;
        (k == name).then(|| decode(v))
    })
}

fn http() -> Result<reqwest::blocking::Client> {
    reqwest::blocking::Client::builder().timeout(Duration::from_secs(60)).redirect(reqwest::redirect::Policy::none()).build().context("HTTP client")
}

/// A valid access token: from memory, by the stored refresh token, or by signing in in the browser.
pub fn access_token(provider: &Provider) -> Result<String> {
    if let Some((token, until)) = TOKENS.lock().unwrap_or_else(|p| p.into_inner()).as_ref().and_then(|m| m.get(provider.secret)).cloned() {
        if Instant::now() < until {
            return Ok(token);
        }
    }
    if let Ok(refresh) = crate::config::secrets::get(provider.secret) {
        if let Ok(token) = refresh_token(provider, &refresh) {
            return Ok(token);
        }
    }
    sign_in(provider)
}

/// Forgets the sign-in (the stored refresh token and the token in memory).
pub fn sign_out(provider: &Provider) {
    let _ = crate::config::secrets::delete(provider.secret);
    if let Some(m) = TOKENS.lock().unwrap_or_else(|p| p.into_inner()).as_mut() {
        m.remove(provider.secret);
    }
}

fn keep(provider: &Provider, answer: &Value) -> Result<String> {
    let token = answer["access_token"].as_str().with_context(|| crate::i18n::fmt("{0}: the sign-in returned no access token", &[provider.name.to_string()]))?.to_string();
    let seconds = answer["expires_in"].as_u64().unwrap_or(3600).saturating_sub(60);
    if let Some(refresh) = answer["refresh_token"].as_str() {
        crate::config::secrets::set(provider.secret, refresh)?;
    }
    TOKENS.lock().unwrap_or_else(|p| p.into_inner()).get_or_insert_with(HashMap::new).insert(provider.secret, (token.clone(), Instant::now() + Duration::from_secs(seconds)));
    Ok(token)
}

fn token_request(provider: &Provider, form: &[(&str, &str)]) -> Result<Value> {
    let mut form: Vec<(&str, &str)> = form.to_vec();
    form.push(("client_id", provider.client_id.as_str()));
    if let Some(secret) = &provider.client_secret {
        form.push(("client_secret", secret.as_str()));
    }
    let response = http()?.post(&provider.token_url).form(&form).send().with_context(|| crate::i18n::fmt("{0}: no connection to the sign-in service", &[provider.name.to_string()]))?;
    let status = response.status();
    let answer: Value = response.json().unwrap_or(Value::Null);
    if !status.is_success() {
        let reason = answer["error_description"].as_str().or(answer["error"].as_str()).unwrap_or("").to_string();
        bail!("{}", crate::i18n::fmt("{0}: the sign-in service answered {1}: {2}", &[provider.name.to_string(), status.to_string(), reason]));
    }
    Ok(answer)
}

fn refresh_token(provider: &Provider, refresh: &str) -> Result<String> {
    let answer = token_request(provider, &[("grant_type", "refresh_token"), ("refresh_token", refresh), ("scope", provider.scopes.as_str())])?;
    keep(provider, &answer)
}

/// The sign-in in the browser: opens the provider's page and waits for the code on a one-time local address.
pub fn sign_in(provider: &Provider) -> Result<String> {
    if provider.client_id.trim().is_empty() {
        bail!("{}", crate::i18n::fmt("{0} is not set up: give the client ID in Settings", &[provider.name.to_string()]));
    }
    let listener = TcpListener::bind("127.0.0.1:0").context("cannot open a local port for the sign-in")?;
    let port = listener.local_addr()?.port();
    listener.set_nonblocking(true)?;
    let redirect = format!("http://{}:{port}/", provider.loopback_host);
    let (verifier, state) = (random(), random());
    let url = format!(
        "{}?client_id={}&response_type=code&redirect_uri={}&scope={}&code_challenge={}&code_challenge_method=S256&state={state}{}",
        provider.auth_url,
        encode(provider.client_id.trim()),
        encode(&redirect),
        encode(&provider.scopes),
        challenge(&verifier),
        provider.extra
    );
    if std::env::var("LOONAR_OAUTH_NO_BROWSER").is_err() {
        std::process::Command::new(crate::config::system_program("rundll32.exe")).arg("url.dll,FileProtocolHandler").arg(&url).spawn().context("cannot open the browser")?;
    } else {
        // Tests: the "browser" is told where to go through this file.
        let _ = std::fs::write(std::env::temp_dir().join(format!("loonar-oauth-url-{}", std::process::id())), &url);
    }
    let deadline = Instant::now() + WAIT;
    let code = loop {
        match listener.accept() {
            Ok((mut stream, _)) => {
                stream.set_nonblocking(false)?;
                stream.set_read_timeout(Some(Duration::from_secs(10)))?;
                let mut line = String::new();
                BufReader::new(&stream).read_line(&mut line)?;
                let target = line.split_whitespace().nth(1).unwrap_or("");
                let Some(query) = target.strip_prefix("/?") else {
                    let _ = stream.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
                    continue;
                };
                let ok = query_value(query, "state").as_deref() == Some(state.as_str()) && query_value(query, "code").is_some();
                let page = if ok { crate::i18n::tr("Signed in. You can close this tab and go back to Loonar Compass.") } else { crate::i18n::tr("The sign-in did not succeed. You can close this tab.") };
                let body = format!("<!doctype html><meta charset=\"utf-8\"><title>Loonar Compass</title><p style=\"font-family:Segoe UI,sans-serif;margin:3em\">{page}</p>");
                let _ = write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n{body}", body.len());
                if let Some(error) = query_value(query, "error") {
                    bail!("{}", crate::i18n::fmt("{0}: the sign-in was refused: {1}", &[provider.name.to_string(), query_value(query, "error_description").unwrap_or(error)]));
                }
                if !ok {
                    bail!("{}", crate::i18n::fmt("{0}: the sign-in answer is not valid", &[provider.name.to_string()]));
                }
                break query_value(query, "code").unwrap_or_default();
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                if Instant::now() > deadline {
                    bail!("{}", crate::i18n::tr("The sign-in was not finished in the browser within 5 minutes."));
                }
                std::thread::sleep(Duration::from_millis(200));
            }
            Err(e) => return Err(e.into()),
        }
    };
    let answer = token_request(provider, &[("grant_type", "authorization_code"), ("code", code.as_str()), ("redirect_uri", redirect.as_str()), ("code_verifier", verifier.as_str())])?;
    keep(provider, &answer)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pkce_and_query() {
        // RFC 7636 appendix B.
        assert_eq!(challenge("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"), "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM");
        assert_eq!(query_value("code=a%2Fb+c&state=x", "code").as_deref(), Some("a/b c"));
        assert_eq!(encode("http://localhost:5/"), "http%3A%2F%2Flocalhost%3A5%2F");
    }
}

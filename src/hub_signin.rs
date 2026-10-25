//! Signing in to the team hub with the company account, from Compass: the browser opens the hub's company sign-in
//! (AD FS, Entra ID, Keycloak…), and the hub hands the session back to a one-time address on this computer
//! (http://127.0.0.1:<port>/compass-callback). The session becomes the hub token in Credential Manager.
use anyhow::{bail, Context, Result};
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::time::{Duration, Instant};

/// How long Compass waits for the person to finish the sign-in in the browser.
const WAIT: Duration = Duration::from_secs(300);

fn query_value(query: &str, name: &str) -> Option<String> {
    query.split('&').find_map(|pair| {
        let (k, v) = pair.split_once('=')?;
        (k == name).then(|| crate::hub::url_decode_value(v))
    })
}

/// Opens the browser at the hub's company sign-in and waits for the session. Returns (user name, role).
pub fn sign_in(hub_url: &str) -> Result<(String, String)> {
    let hub_url = hub_url.trim().trim_end_matches('/');
    if hub_url.is_empty() {
        bail!("{}", crate::i18n::tr("Give the address of the hub first."));
    }
    let listener = TcpListener::bind("127.0.0.1:0").context("cannot open a local port for the sign-in")?;
    let port = listener.local_addr()?.port();
    listener.set_nonblocking(true)?;
    let back = format!("http://127.0.0.1:{port}/compass-callback");
    let url = format!("{hub_url}/auth/login?return={}", crate::hub::encode_value(&back));
    std::process::Command::new(crate::config::system_program("rundll32.exe")).arg("url.dll,FileProtocolHandler").arg(&url).spawn().context("cannot open the browser")?;
    let deadline = Instant::now() + WAIT;
    loop {
        match listener.accept() {
            Ok((mut stream, _)) => {
                stream.set_nonblocking(false)?;
                stream.set_read_timeout(Some(Duration::from_secs(10)))?;
                let mut line = String::new();
                BufReader::new(&stream).read_line(&mut line)?;
                let target = line.split_whitespace().nth(1).unwrap_or("");
                let Some(query) = target.strip_prefix("/compass-callback?") else {
                    let _ = stream.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
                    continue;
                };
                let (session, name, role) = (query_value(query, "session"), query_value(query, "name").unwrap_or_default(), query_value(query, "role").unwrap_or_default());
                let page = crate::i18n::tr("Signed in. You can close this tab and go back to Loonar Compass.");
                let body = format!("<!doctype html><meta charset=\"utf-8\"><title>Loonar Compass</title><p style=\"font-family:Segoe UI,sans-serif;margin:3em\">{page}</p>");
                let _ = write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n{body}", body.len());
                let session = session.filter(|s| s.starts_with("lcs_")).context("the hub did not return a session")?;
                crate::config::secrets::set(crate::config::secrets::HUB_TOKEN, &session)?;
                crate::hub::forget_policy();
                return Ok((name, role));
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                if Instant::now() > deadline {
                    bail!("{}", crate::i18n::tr("The sign-in was not finished in the browser within 5 minutes."));
                }
                std::thread::sleep(Duration::from_millis(200));
            }
            Err(e) => return Err(e.into()),
        }
    }
}

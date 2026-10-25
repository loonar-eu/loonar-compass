//! Checking for a newer release: once a day, in the background, Compass asks GitHub for the latest release of the
//! project (a plain request with no identifier of the installation) and, when it is newer, tells the user with a
//! bar in the window (in EA: once in System Output). Nothing is downloaded or installed. Off with the setting
//! `app.update_check=false` or for the whole team with the hub policy `"update_check": false`.
use crate::config::AppConfig;
use anyhow::{Context, Result};
use serde_json::{json, Value};
use std::time::Duration;

const LATEST: &str = "https://api.github.com/repos/loonar-eu/loonar-compass/releases/latest";
const EVERY_HOURS: i64 = 24;

/// A release newer than this one.
#[derive(Clone, Debug)]
pub struct Release {
    pub version: String,
    pub url: String,
}

/// "v0.9.0", "0.9", "1.0.2" → comparable numbers.
fn numbers(version: &str) -> Vec<u64> {
    let mut parts: Vec<u64> = version.trim().trim_start_matches(['v', 'V']).split(['.', '-', '+']).map_while(|p| p.parse().ok()).collect();
    while parts.len() < 3 {
        parts.push(0);
    }
    parts
}

pub fn newer(candidate: &str, current: &str) -> bool {
    numbers(candidate) > numbers(current)
}

fn state_file(cfg: &AppConfig) -> std::path::PathBuf {
    cfg.base_dir.join("update_check.json")
}

fn state(cfg: &AppConfig) -> Value {
    std::fs::read_to_string(state_file(cfg)).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_else(|| json!({}))
}

/// "Not now" for this version: the bar comes back only for a later one.
pub fn dismiss(cfg: &AppConfig, version: &str) {
    let mut s = state(cfg);
    s["dismissed"] = json!(version);
    let _ = std::fs::write(state_file(cfg), s.to_string());
}

fn fetch_latest() -> Result<Release> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(10))
        .user_agent(concat!("LoonarCompass/", env!("CARGO_PKG_VERSION")))
        .redirect(crate::net::same_origin_redirects())
        .build()?;
    let release: Value = client.get(LATEST).header("Accept", "application/vnd.github+json").send()?.error_for_status()?.json()?;
    let tag = release["tag_name"].as_str().context("no tag in the latest release")?;
    let url = release["html_url"].as_str().filter(|u| u.starts_with("https://github.com/loonar-eu/")).unwrap_or("https://github.com/loonar-eu/loonar-compass/releases/latest");
    Ok(Release { version: tag.trim_start_matches(['v', 'V']).to_string(), url: url.to_string() })
}

/// The newer release to tell the user about, or None: switched off (setting or team policy), checked less than a day
/// ago with nothing new, the user said "not now" to it, or GitHub does not answer (silently: nothing to report).
pub fn check(cfg: &AppConfig) -> Option<Release> {
    if !cfg.update_check || crate::hub::team_policy(cfg).is_some_and(|p| p["update_check"] == false) {
        return None;
    }
    let mut s = state(cfg);
    let now = chrono::Utc::now();
    let recent = s["checked"].as_str().and_then(|t| chrono::DateTime::parse_from_rfc3339(t).ok()).is_some_and(|t| now.signed_duration_since(t).num_hours() < EVERY_HOURS);
    let release = if recent {
        Release { version: s["latest"].as_str()?.to_string(), url: s["url"].as_str().unwrap_or("").to_string() }
    } else {
        let release = fetch_latest().ok()?;
        s["checked"] = json!(now.to_rfc3339());
        s["latest"] = json!(release.version);
        s["url"] = json!(release.url);
        let _ = std::fs::write(state_file(cfg), s.to_string());
        release
    };
    (newer(&release.version, env!("CARGO_PKG_VERSION")) && s["dismissed"].as_str() != Some(release.version.as_str())).then_some(release)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_compare_as_numbers() {
        assert!(newer("v1.0.0", "0.9.0") && newer("0.10", "0.9.0") && newer("0.9.1", "0.9"));
        assert!(!newer("v0.9.0", "0.9.0") && !newer("0.8.2", "0.9.0") && !newer("nonsense", "0.9.0"));
    }
}

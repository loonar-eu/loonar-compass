//! Client certificates (mTLS) for Jira and Confluence on company servers that ask for one besides the token (LC-279):
//! a PKCS#12 file (.p12, .pfx) with its password in Windows Credential Manager, or a PEM file with the certificate
//! and its private key.
use anyhow::{anyhow, Context, Result};
use base64::Engine;

use crate::config::{secrets, AppConfig};

/// The client certificate of Jira from the settings (None without one).
pub fn jira(cfg: &AppConfig) -> Result<Option<reqwest::Identity>> {
    load(&cfg.jira_client_cert, secrets::JIRA_CERT_PASSWORD)
}

/// The client certificate of Confluence from the settings (None without one).
pub fn confluence(cfg: &AppConfig) -> Result<Option<reqwest::Identity>> {
    load(&cfg.confluence_client_cert, secrets::CONFLUENCE_CERT_PASSWORD)
}

fn load(path: &str, password_secret: &str) -> Result<Option<reqwest::Identity>> {
    let path = path.trim().trim_matches('"');
    if path.is_empty() {
        return Ok(None);
    }
    let password = secrets::get(password_secret).unwrap_or_default();
    identity(path, &password).map(Some)
}

/// The identity for the HTTPS client: PEM files as they are, PKCS#12 converted to PEM (key and certificate chain).
pub fn identity(path: &str, password: &str) -> Result<reqwest::Identity> {
    let bytes = std::fs::read(path).with_context(|| crate::i18n::fmt("the client certificate {0} cannot be read", &[path.to_string()]))?;
    let lower = path.to_ascii_lowercase();
    let pem = if lower.ends_with(".pem") || lower.ends_with(".crt") || lower.ends_with(".key") { bytes } else { pkcs12_to_pem(&bytes, password)? };
    reqwest::Identity::from_pem(&pem).map_err(|e| anyhow!("{}: {e}", crate::i18n::tr("the client certificate cannot be used (it needs the certificate and its private key)")))
}

fn pkcs12_to_pem(der: &[u8], password: &str) -> Result<Vec<u8>> {
    let store = p12_keystore::KeyStore::from_pkcs12(der, password)
        .map_err(|e| anyhow!("{}: {e}", crate::i18n::tr("the client certificate cannot be opened: a wrong password, or not a PKCS#12 file (.p12, .pfx)")))?;
    let (_, chain) = store.private_key_chain().context(crate::i18n::tr("the client certificate file has no private key"))?;
    let mut pem = block("PRIVATE KEY", chain.key());
    for certificate in chain.chain() {
        pem.push_str(&block("CERTIFICATE", certificate.as_der()));
    }
    Ok(pem.into_bytes())
}

fn block(label: &str, der: &[u8]) -> String {
    let body = base64::engine::general_purpose::STANDARD.encode(der);
    let lines: Vec<&str> = body.as_bytes().chunks(64).map(|c| std::str::from_utf8(c).unwrap_or("")).collect();
    format!("-----BEGIN {label}-----\n{}\n-----END {label}-----\n", lines.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A missing file and a file that is not PKCS#12 give clear errors, not a panic.
    #[test]
    fn clear_errors() {
        assert!(identity("C:/nie/ma/takiego.p12", "x").is_err());
        let file = std::env::temp_dir().join(format!("loonar-lc279-{}.p12", std::process::id()));
        std::fs::write(&file, b"not a certificate").unwrap();
        let e = identity(file.to_str().unwrap(), "x").unwrap_err().to_string();
        assert!(!e.is_empty());
        assert_eq!(block("X", &[0u8; 3]), "-----BEGIN X-----\nAAAA\n-----END X-----\n");
    }

    /// The user's real client certificate, if given: `LOONAR_TEST_P12=path;password cargo test real_p12 -- --ignored`.
    #[test]
    #[ignore = "requires a client certificate"]
    fn real_p12() {
        let spec = std::env::var("LOONAR_TEST_P12").unwrap();
        let (path, password) = spec.split_once(';').unwrap();
        identity(path, password).unwrap();
    }
}

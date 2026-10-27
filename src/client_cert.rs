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

fn pkcs12_to_pem(data: &[u8], password: &str) -> Result<Vec<u8>> {
    // Files exported by Windows and older tools are BER with indefinite lengths; the parser takes DER only.
    let der = pkcs12_der(data).unwrap_or_else(|| data.to_vec());
    let store = p12_keystore::KeyStore::from_pkcs12(&der, password)
        .map_err(|e| anyhow!("{}: {e}", crate::i18n::tr("the client certificate cannot be opened: a wrong password, or not a PKCS#12 file (.p12, .pfx)")))?;
    let (_, chain) = store.private_key_chain().context(crate::i18n::tr("the client certificate file has no private key"))?;
    let mut pem = block("PRIVATE KEY", chain.key());
    for certificate in chain.chain() {
        pem.push_str(&block("CERTIFICATE", certificate.as_der()));
    }
    Ok(pem.into_bytes())
}

/// A PKCS#12 file as DER, also inside: the authenticated safe and the unencrypted safe contents are BER of their
/// own, carried in OCTET STRINGs. Their re-encoding changes the bytes the integrity MAC covers, so the MAC is left
/// out (a wrong password still fails: the private key does not decrypt). None when the file is not BER PKCS#12.
fn pkcs12_der(data: &[u8]) -> Option<Vec<u8>> {
    let pfx = ber_to_der(data)?;
    if pfx == data {
        return None; // already DER: as it is, with its MAC
    }
    let items = children(&pfx)?; // version, authSafe (ContentInfo), macData
    let auth_safe = data_content(items.get(1)?)?;
    let safes = ber_to_der(&auth_safe)?;
    let mut infos = Vec::new();
    for info in children(&safes)? {
        // ContentInfo { OID data, [0] { OCTET STRING SafeContents } }; encrypted ones stay as they are.
        match data_content(&info).and_then(|inner| ber_to_der(&inner)) {
            Some(contents) => infos.push(content_info(&contents)),
            None => infos.push(encrypted_der(&info).unwrap_or(info)),
        }
    }
    let auth_safe = content_info(&encode(0x30, &infos.concat()));
    Some(encode(0x30, &[items.first()?.clone(), auth_safe].concat()))
}

const DATA_OID: [u8; 11] = [0x06, 0x09, 0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x01, 0x07, 0x01];

/// The DER parts of a DER SEQUENCE.
fn children(der: &[u8]) -> Option<Vec<Vec<u8>>> {
    let (len, header) = read_length(der.get(1..)?)?;
    let content = der.get(1 + header..1 + header + len?)?;
    let mut out = Vec::new();
    let mut at = 0;
    while at < content.len() {
        let (len, header) = read_length(content.get(at + 1..)?)?;
        let end = at + 1 + header + len?;
        out.push(content.get(at..end)?.to_vec());
        at = end;
    }
    Some(out)
}

/// The bytes of a ContentInfo of type data (the value of its OCTET STRING).
fn data_content(info: &[u8]) -> Option<Vec<u8>> {
    let parts = children(info)?;
    if parts.first()?.as_slice() != DATA_OID {
        return None;
    }
    let explicit = children(parts.get(1)?)?; // [0] { OCTET STRING }
    let octets = explicit.first()?;
    let (len, header) = read_length(octets.get(1..)?)?;
    Some(octets.get(1 + header..1 + header + len?)?.to_vec())
}

/// A ContentInfo of type encryptedData whose encrypted content ([0] IMPLICIT OCTET STRING) BER split into parts:
/// the parts joined into one primitive value, as DER wants.
fn encrypted_der(info: &[u8]) -> Option<Vec<u8>> {
    const ENCRYPTED_DATA_OID: [u8; 11] = [0x06, 0x09, 0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x01, 0x07, 0x06];
    let parts = children(info)?;
    if parts.first()?.as_slice() != ENCRYPTED_DATA_OID {
        return None;
    }
    let explicit = children(parts.get(1)?)?; // [0] { EncryptedData }
    let data = children(explicit.first()?)?; // version, EncryptedContentInfo
    let mut eci = children(data.get(1)?)?; // contentType, algorithm, [0] encryptedContent
    let content = eci.get(2)?;
    if content.first()? != &0xa0 {
        return None;
    }
    let mut joined = Vec::new();
    for part in children(content)? {
        let (len, header) = read_length(part.get(1..)?)?;
        joined.extend_from_slice(part.get(1 + header..1 + header + len?)?);
    }
    eci[2] = encode(0x80, &joined);
    let data = encode(0x30, &[data.first()?.clone(), encode(0x30, &eci.concat())].concat());
    Some(encode(0x30, &[parts[0].clone(), encode(0xa0, &data)].concat()))
}

fn content_info(inner: &[u8]) -> Vec<u8> {
    encode(0x30, &[DATA_OID.to_vec(), encode(0xa0, &encode(0x04, inner))].concat())
}

/// BER to DER: indefinite lengths become definite and constructed OCTET STRINGs (BER splits long ones into parts)
/// become one primitive value. None when the data is not a complete BER value.
fn ber_to_der(data: &[u8]) -> Option<Vec<u8>> {
    let (out, used) = convert(data)?;
    (used == data.len()).then_some(out)
}

/// One value: (its DER encoding, the bytes it took in the input).
fn convert(data: &[u8]) -> Option<(Vec<u8>, usize)> {
    let tag = *data.first()?;
    if tag & 0x1f == 0x1f {
        return None; // high tag numbers do not occur in PKCS#12
    }
    let (length, header) = read_length(&data[1..])?;
    let start = 1 + header;
    let constructed = tag & 0x20 != 0;
    if !constructed {
        let len = length?;
        let content = data.get(start..start + len)?;
        return Some((encode(tag, content), start + len));
    }
    // Constructed: the parts until the end (definite) or until 00 00 (indefinite).
    let mut parts = Vec::new();
    let mut at = start;
    loop {
        match length {
            Some(len) if at >= start + len => break,
            None if data.get(at..at + 2)? == [0, 0] => {
                at += 2;
                break;
            }
            _ => {}
        }
        let (part, used) = convert(data.get(at..)?)?;
        parts.push(part);
        at += used;
    }
    if let Some(len) = length {
        if at != start + len {
            return None;
        }
    }
    if tag == 0x24 {
        // A constructed OCTET STRING: the contents of its parts as one primitive OCTET STRING.
        let mut content = Vec::new();
        for part in &parts {
            let (len, header) = read_length(&part[1..])?;
            content.extend_from_slice(part.get(1 + header..1 + header + len?)?);
        }
        return Some((encode(0x04, &content), at));
    }
    Some((encode(tag, &parts.concat()), at))
}

/// The length after a tag: (Some(length) or None for indefinite, bytes of the length field).
fn read_length(data: &[u8]) -> Option<(Option<usize>, usize)> {
    let first = *data.first()?;
    if first < 0x80 {
        return Some((Some(first as usize), 1));
    }
    if first == 0x80 {
        return Some((None, 1));
    }
    let n = (first & 0x7f) as usize;
    if n > 4 {
        return None;
    }
    let mut len = 0usize;
    for b in data.get(1..1 + n)? {
        len = (len << 8) | *b as usize;
    }
    Some((Some(len), 1 + n))
}

fn encode(tag: u8, content: &[u8]) -> Vec<u8> {
    let mut out = vec![tag];
    let len = content.len();
    if len < 0x80 {
        out.push(len as u8);
    } else {
        let bytes: Vec<u8> = len.to_be_bytes().iter().copied().skip_while(|b| *b == 0).collect();
        out.push(0x80 | bytes.len() as u8);
        out.extend(bytes);
    }
    out.extend_from_slice(content);
    out
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

    /// BER with an indefinite length and a constructed OCTET STRING becomes DER (LC-279: files from Java).
    #[test]
    fn ber_becomes_der() {
        // SEQUENCE (indefinite) { INTEGER 5, OCTET STRING constructed (indefinite) { "ab", "c" } }
        let ber = [0x30, 0x80, 0x02, 0x01, 0x05, 0x24, 0x80, 0x04, 0x02, b'a', b'b', 0x04, 0x01, b'c', 0x00, 0x00, 0x00, 0x00];
        let der = ber_to_der(&ber).unwrap();
        assert_eq!(der, vec![0x30, 0x08, 0x02, 0x01, 0x05, 0x04, 0x03, b'a', b'b', b'c']);
        // DER stays as it is.
        assert_eq!(ber_to_der(&der).unwrap(), der);
        // Long content uses the long form of the length.
        let long = vec![0x55u8; 300];
        let mut prim = vec![0x04, 0x82, 0x01, 0x2c];
        prim.extend(&long);
        assert_eq!(ber_to_der(&prim).unwrap(), prim);
        assert!(ber_to_der(&[0x30, 0x80, 0x02]).is_none());
    }

    /// The real client certificate of Jira from the settings, with its password from Credential Manager:
    /// `cargo test jira_certificate_from_settings -- --ignored`.
    #[test]
    #[ignore = "requires the user's client certificate and its password in Credential Manager"]
    fn jira_certificate_from_settings() {
        let path = std::env::var("LOONAR_TEST_P12").unwrap_or_else(|_| "C:/certyfikaty/jira-client.p12".into());
        let password = secrets::get(secrets::JIRA_CERT_PASSWORD).expect("certificate password in Credential Manager");
        if let Err(e) = identity(&path, &password) {
            panic!("{e:#}");
        }
    }

    /// A real read-only connection to the Jira of the settings with its certificate:
    /// `LOONAR_TEST_JIRA=<address>;<project> cargo test jira_with_certificate -- --ignored --nocapture`.
    #[test]
    #[ignore = "requires the user's Jira, its token and client certificate"]
    fn jira_with_certificate() {
        let spec = std::env::var("LOONAR_TEST_JIRA").unwrap();
        let (url, project) = spec.split_once(';').unwrap();
        let password = secrets::get(secrets::JIRA_CERT_PASSWORD).unwrap();
        let connection = crate::jira::Connection {
            base_url: url.into(),
            user: String::new(),
            token: secrets::get(secrets::JIRA_TOKEN).unwrap(),
            identity: Some(identity("C:/certyfikaty/jira-client.p12", &password).unwrap()),
        };
        match connection.check(project) {
            Ok(name) => println!("connected: project {project} = {name}"),
            Err(e) => panic!("{e:#}"),
        }
    }

    /// A real read-only connection to Confluence with its token and certificate from the settings:
    /// `LOONAR_TEST_CONFLUENCE=<address>;<space> cargo test confluence_with_certificate -- --ignored --nocapture`.
    #[test]
    #[ignore = "requires the user's Confluence, its token and client certificate"]
    fn confluence_with_certificate() {
        let spec = std::env::var("LOONAR_TEST_CONFLUENCE").unwrap();
        let (url, space) = spec.split_once(';').unwrap();
        let password = secrets::get(secrets::CONFLUENCE_CERT_PASSWORD).or_else(|_| secrets::get(secrets::JIRA_CERT_PASSWORD)).unwrap();
        let identity = Some(identity("C:/certyfikaty/jira-client.p12", &password).unwrap());
        for (name, token) in [("Confluence token", secrets::get(secrets::CONFLUENCE_TOKEN)), ("Jira token", secrets::get(secrets::JIRA_TOKEN))] {
            let Ok(token) = token else { continue };
            let connection = crate::confluence::Connection { base_url: url.into(), user: String::new(), token, identity: identity.clone() };
            match connection.check(space) {
                Ok(n) => println!("{name}: connected, space {space} = {n}"),
                Err(e) => println!("{name}: {e:#}"),
            }
        }
    }
}

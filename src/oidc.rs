//! Sign-in to Compass Hub with a company account (OpenID Connect): AD FS, Microsoft Entra ID, Keycloak, Okta…
//!
//! Authorization code flow with PKCE (S256), state and nonce; a confidential client (the secret is the hub secret
//! `OIDC_CLIENT_SECRET`). The ID token is checked by the hub itself: RS256 signature against the provider's keys
//! (JWKS), issuer, audience, expiry and nonce. Groups from the token give the role in the hub.
//!
//! `hub.json`:
//! "oidc": { "issuer": "https://adfs.lab.local/adfs", "client_id": "…", "redirect_uri": "https://hub.lab.local/auth/callback",
//!           "groups_claim": "groups", "roles": { "admin": ["Compass-Admins"], "architect": ["Compass-Architects"], "viewer": ["Compass-Viewers"] },
//!           "password_login": true }

use anyhow::{anyhow, bail, Context, Result};
use base64::Engine;
use serde_json::Value;
use sha2::Digest;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// How long a sign-in may take at the provider (state and PKCE verifier are kept this long).
const PENDING_FOR: Duration = Duration::from_secs(600);
/// Provider metadata and keys are read again after this time (or at once for an unknown key id).
const METADATA_FOR: Duration = Duration::from_secs(3600);
/// Allowed clock difference between the hub and the provider.
const SKEW_SECS: i64 = 120;

#[derive(Clone, Debug)]
pub struct Config {
    pub issuer: String,
    pub client_id: String,
    pub redirect_uri: String,
    pub scopes: String,
    pub groups_claim: String,
    pub name_claims: Vec<String>,
    /// (role name, groups that give it), the highest role first.
    pub roles: Vec<(String, Vec<String>)>,
    pub password_login: bool,
    pub button: String,
}

impl Config {
    /// The "oidc" section of hub.json; `None` without it.
    pub fn from_json(v: &Value) -> Result<Option<Config>> {
        if v.is_null() {
            return Ok(None);
        }
        let text = |k: &str| v[k].as_str().map(str::trim).unwrap_or("").to_string();
        let issuer = text("issuer").trim_end_matches('/').to_string();
        let client_id = text("client_id");
        let redirect_uri = text("redirect_uri");
        if issuer.is_empty() || client_id.is_empty() || redirect_uri.is_empty() {
            bail!("{}", crate::i18n::tr("hub.json \"oidc\": give issuer, client_id and redirect_uri"));
        }
        for url in [&issuer, &redirect_uri] {
            if crate::net::check_service_url(url).is_err() {
                bail!("{}", crate::i18n::fmt("hub.json \"oidc\": {0} must be an https:// address (http:// only on this computer)", &[url.to_string()]));
            }
        }
        let list = |k: &str| -> Vec<String> {
            match &v["roles"][k] {
                Value::Array(a) => a.iter().filter_map(|g| g.as_str()).map(|g| g.trim().to_string()).filter(|g| !g.is_empty()).collect(),
                Value::String(s) => vec![s.trim().to_string()],
                _ => Vec::new(),
            }
        };
        let roles = vec![
            ("admin".to_string(), if v["roles"].is_null() { vec!["Compass-Admins".to_string()] } else { list("admin") }),
            ("architect".to_string(), if v["roles"].is_null() { vec!["Compass-Architects".to_string()] } else { list("architect") }),
            ("viewer".to_string(), if v["roles"].is_null() { vec!["Compass-Viewers".to_string()] } else { list("viewer") }),
        ];
        let name_claims = match v["name_claim"].as_str() {
            Some(c) if !c.trim().is_empty() => vec![c.trim().to_string()],
            _ => ["upn", "preferred_username", "email", "unique_name", "sub"].iter().map(|s| s.to_string()).collect(),
        };
        Ok(Some(Config {
            issuer,
            client_id,
            redirect_uri,
            scopes: v["scopes"].as_str().map(str::trim).filter(|s| !s.is_empty()).unwrap_or("openid profile email").to_string(),
            groups_claim: v["groups_claim"].as_str().map(str::trim).filter(|s| !s.is_empty()).unwrap_or("groups").to_string(),
            name_claims,
            roles,
            password_login: v["password_login"] != false,
            button: v["button"].as_str().map(str::trim).filter(|s| !s.is_empty()).unwrap_or("Sign in with your company account").to_string(),
        }))
    }
}

#[derive(Clone)]
struct Metadata {
    issuer: String,
    authorization_endpoint: String,
    token_endpoint: String,
    jwks_uri: String,
    /// key id → (modulus, exponent)
    keys: HashMap<String, (Vec<u8>, Vec<u8>)>,
    read: Instant,
}

struct Pending {
    verifier: String,
    nonce: String,
    started: Instant,
}

/// The signed-in person: name, e-mail (if given) and role name.
#[derive(Debug, Clone, PartialEq)]
pub struct Identity {
    pub name: String,
    pub email: String,
    pub role: String,
    pub groups: Vec<String>,
}

pub struct Provider {
    pub config: Config,
    secret: String,
    metadata: Mutex<Option<Metadata>>,
    pending: Mutex<HashMap<String, Pending>>,
}

fn b64url(bytes: &[u8]) -> String {
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

fn unb64url(text: &str) -> Result<Vec<u8>> {
    base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(text.trim_end_matches('=')).context("not base64url")
}

fn random() -> String {
    let mut bytes = [0u8; 32];
    ring::rand::SecureRandom::fill(&ring::rand::SystemRandom::new(), &mut bytes).expect("random");
    b64url(&bytes)
}

fn client() -> Result<reqwest::blocking::Client> {
    Ok(reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(20))
        .redirect(crate::net::same_origin_redirects())
        .user_agent(concat!("CompassHub/", env!("CARGO_PKG_VERSION")))
        .build()?)
}

fn url_encode(text: &str) -> String {
    text.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

impl Provider {
    pub fn new(config: Config, secret: String) -> Provider {
        Provider { config, secret, metadata: Mutex::new(None), pending: Mutex::new(HashMap::new()) }
    }

    fn metadata(&self, refresh: bool) -> Result<Metadata> {
        if !refresh {
            if let Some(m) = self.metadata.lock().unwrap_or_else(|p| p.into_inner()).as_ref() {
                if m.read.elapsed() < METADATA_FOR {
                    return Ok(m.clone());
                }
            }
        }
        let http = client()?;
        let discovery = format!("{}/.well-known/openid-configuration", self.config.issuer);
        let doc: Value = http.get(&discovery).send().and_then(|r| r.error_for_status()).with_context(|| crate::i18n::fmt("cannot read {0}", &[discovery.clone()]))?.json()?;
        let field = |k: &str| doc[k].as_str().map(String::from).with_context(|| format!("{k} missing in {discovery}"));
        let jwks_uri = field("jwks_uri")?;
        for url in [&jwks_uri, &field("token_endpoint")?, &field("authorization_endpoint")?] {
            if crate::net::check_service_url(url).is_err() {
                bail!("the provider gave an address that is not https: {url}");
            }
        }
        let jwks: Value = http.get(&jwks_uri).send().and_then(|r| r.error_for_status())?.json()?;
        let mut keys = HashMap::new();
        for k in jwks["keys"].as_array().into_iter().flatten() {
            if k["kty"] != "RSA" || k["use"].as_str().is_some_and(|u| u != "sig") {
                continue;
            }
            let (Some(n), Some(e)) = (k["n"].as_str(), k["e"].as_str()) else { continue };
            keys.insert(k["kid"].as_str().unwrap_or("").to_string(), (unb64url(n)?, unb64url(e)?));
        }
        if keys.is_empty() {
            bail!("no RSA signing keys at {jwks_uri}");
        }
        let m = Metadata { issuer: field("issuer")?, authorization_endpoint: field("authorization_endpoint")?, token_endpoint: field("token_endpoint")?, jwks_uri, keys, read: Instant::now() };
        *self.metadata.lock().unwrap_or_else(|p| p.into_inner()) = Some(m.clone());
        Ok(m)
    }

    /// The address of the provider's sign-in page for a new sign-in.
    pub fn start(&self) -> Result<String> {
        let m = self.metadata(false)?;
        let (state, verifier, nonce) = (random(), random(), random());
        let challenge = b64url(&sha2::Sha256::digest(verifier.as_bytes()));
        {
            let mut pending = self.pending.lock().unwrap_or_else(|p| p.into_inner());
            pending.retain(|_, p| p.started.elapsed() < PENDING_FOR);
            if pending.len() > 2000 {
                bail!("too many sign-ins in progress, try again in a few minutes");
            }
            pending.insert(state.clone(), Pending { verifier, nonce: nonce.clone(), started: Instant::now() });
        }
        let separator = if m.authorization_endpoint.contains('?') { '&' } else { '?' };
        Ok(format!(
            "{}{separator}response_type=code&client_id={}&redirect_uri={}&scope={}&state={}&nonce={}&code_challenge={}&code_challenge_method=S256",
            m.authorization_endpoint,
            url_encode(&self.config.client_id),
            url_encode(&self.config.redirect_uri),
            url_encode(&self.config.scopes),
            state,
            nonce,
            challenge
        ))
    }

    /// The provider sent the user back with a code: exchange it, check the ID token and give the identity.
    pub fn finish(&self, code: &str, state: &str) -> Result<Identity> {
        let pending = self.pending.lock().unwrap_or_else(|p| p.into_inner()).remove(state).filter(|p| p.started.elapsed() < PENDING_FOR);
        let pending = pending.ok_or_else(|| anyhow!("{}", crate::i18n::tr("the sign-in expired or was not started here: start it again")))?;
        let m = self.metadata(false)?;
        let form = [
            ("grant_type", "authorization_code"),
            ("code", code),
            ("redirect_uri", self.config.redirect_uri.as_str()),
            ("client_id", self.config.client_id.as_str()),
            ("client_secret", self.secret.as_str()),
            ("code_verifier", pending.verifier.as_str()),
        ];
        let body = form.iter().map(|(k, v)| format!("{k}={}", url_encode(v))).collect::<Vec<_>>().join("&");
        let response = client()?.post(&m.token_endpoint).header("Content-Type", "application/x-www-form-urlencoded").body(body).send()?;
        let status = response.status();
        let tokens: Value = response.json().unwrap_or(Value::Null);
        if !status.is_success() {
            bail!("{}", crate::i18n::fmt("the provider refused the code: {0}", &[tokens["error_description"].as_str().or(tokens["error"].as_str()).unwrap_or(status.as_str()).to_string()]));
        }
        let id_token = tokens["id_token"].as_str().context("no id_token in the answer of the provider")?;
        let claims = self.verify(id_token, &pending.nonce)?;
        self.identity(&claims)
    }

    /// Checks the signature and the claims of an ID token; returns the claims.
    pub fn verify(&self, token: &str, nonce: &str) -> Result<Value> {
        let parts: Vec<&str> = token.split('.').collect();
        if parts.len() != 3 {
            bail!("the ID token is not a JWT");
        }
        let header: Value = serde_json::from_slice(&unb64url(parts[0])?)?;
        if header["alg"] != "RS256" {
            bail!("unsupported signature algorithm {} (RS256 is required)", header["alg"]);
        }
        let kid = header["kid"].as_str().unwrap_or("").to_string();
        let mut m = self.metadata(false)?;
        if !m.keys.contains_key(&kid) && !(kid.is_empty() && m.keys.len() == 1) {
            m = self.metadata(true)?;
        }
        let (n, e) = m.keys.get(&kid).or_else(|| if kid.is_empty() && m.keys.len() == 1 { m.keys.values().next() } else { None }).context("the ID token is signed with an unknown key")?;
        let key = ring::signature::RsaPublicKeyComponents { n: n.as_slice(), e: e.as_slice() };
        key.verify(&ring::signature::RSA_PKCS1_2048_8192_SHA256, format!("{}.{}", parts[0], parts[1]).as_bytes(), &unb64url(parts[2])?)
            .map_err(|_| anyhow!("the signature of the ID token is not valid"))?;
        let claims: Value = serde_json::from_slice(&unb64url(parts[1])?)?;
        check_claims(&claims, &m.issuer, &self.config.client_id, nonce, chrono::Utc::now().timestamp())?;
        Ok(claims)
    }

    pub fn identity(&self, claims: &Value) -> Result<Identity> {
        identity_of(&self.config, claims)
    }
}

/// Issuer, audience, time and nonce of the claims.
pub fn check_claims(claims: &Value, issuer: &str, client_id: &str, nonce: &str, now: i64) -> Result<()> {
    if claims["iss"].as_str() != Some(issuer) {
        bail!("the ID token comes from another issuer ({})", claims["iss"]);
    }
    let audience_ok = match &claims["aud"] {
        Value::String(a) => a == client_id,
        Value::Array(list) => list.iter().any(|a| a == client_id),
        _ => false,
    };
    if !audience_ok {
        bail!("the ID token is meant for another application");
    }
    let exp = claims["exp"].as_i64().context("the ID token has no expiry")?;
    if exp + SKEW_SECS < now {
        bail!("the ID token has expired");
    }
    if claims["nbf"].as_i64().is_some_and(|nbf| nbf - SKEW_SECS > now) || claims["iat"].as_i64().is_some_and(|iat| iat - SKEW_SECS > now) {
        bail!("the ID token is not valid yet (check the clock of the hub)");
    }
    if claims["nonce"].as_str() != Some(nonce) {
        bail!("the ID token does not belong to this sign-in (nonce)");
    }
    Ok(())
}

/// Name, e-mail and role from the claims. Groups: a list or one string; "/Compass-Admins" (Keycloak's full path)
/// and "LAB\\Compass-Admins" count as "Compass-Admins". No group of the hub = no access.
pub fn identity_of(config: &Config, claims: &Value) -> Result<Identity> {
    let name = config.name_claims.iter().find_map(|c| claims[c.as_str()].as_str().map(str::trim).filter(|s| !s.is_empty())).context("the ID token has no user name")?.to_string();
    let email = claims["email"].as_str().unwrap_or("").to_string();
    let groups: Vec<String> = match &claims[config.groups_claim.as_str()] {
        Value::Array(list) => list.iter().filter_map(|g| g.as_str()).map(String::from).collect(),
        Value::String(one) => vec![one.clone()],
        _ => Vec::new(),
    };
    let short = |g: &str| g.trim().trim_start_matches('/').rsplit(['/', '\\']).next().unwrap_or("").to_lowercase();
    let mine: Vec<String> = groups.iter().map(|g| short(g)).collect();
    for (role, wanted) in &config.roles {
        if wanted.iter().any(|w| mine.contains(&short(w))) {
            return Ok(Identity { name, email, role: role.clone(), groups });
        }
    }
    bail!("{}", crate::i18n::fmt("{0} is not in any group of the hub ({1}): ask the administrator for access", &[name, config.roles.iter().flat_map(|(_, g)| g.clone()).collect::<Vec<_>>().join(", ")]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn config() -> Config {
        Config::from_json(&json!({ "issuer": "https://adfs.lab.local/adfs", "client_id": "c1", "redirect_uri": "https://hub.lab.local/auth/callback" })).unwrap().unwrap()
    }

    #[test]
    fn roles_from_groups() {
        let c = config();
        let who = |groups: Value| identity_of(&c, &json!({ "upn": "anna@lab.local", "groups": groups }));
        assert_eq!(who(json!(["Domain Users", "Compass-Architects"])).unwrap().role, "architect");
        assert_eq!(who(json!(["/Compass-Viewers", "/Compass-Admins"])).unwrap().role, "admin", "the highest role wins; Keycloak paths");
        assert_eq!(who(json!("LAB\\Compass-Viewers")).unwrap().role, "viewer", "one group as a string, with the domain");
        assert!(who(json!(["Domain Users"])).is_err(), "no group of the hub = no access");
        assert_eq!(identity_of(&c, &json!({ "preferred_username": "bartek", "groups": ["compass-viewers"] })).unwrap().name, "bartek");
        assert!(Config::from_json(&json!({ "issuer": "http://adfs.lab.local/adfs", "client_id": "c", "redirect_uri": "https://h/auth/callback" })).is_err(), "http only on this computer");
        assert!(Config::from_json(&Value::Null).unwrap().is_none());
    }

    #[test]
    fn claims_are_checked() {
        let now = 1_800_000_000;
        let ok = json!({ "iss": "https://i", "aud": ["c1", "x"], "exp": now + 300, "iat": now, "nonce": "n" });
        assert!(check_claims(&ok, "https://i", "c1", "n", now).is_ok());
        assert!(check_claims(&ok, "https://other", "c1", "n", now).is_err());
        assert!(check_claims(&ok, "https://i", "c2", "n", now).is_err());
        assert!(check_claims(&ok, "https://i", "c1", "m", now).is_err());
        assert!(check_claims(&ok, "https://i", "c1", "n", now + 1000).is_err(), "expired");
    }
}

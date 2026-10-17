//! Chat history: one JSON file per session in the `history` folder (next to the application).
//! Attachments are copied to `attachments` under a unique name.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: String, // "user" | "assistant"
    pub text: String,
    pub attachments: Vec<String>, // file names in the attachments folder
    pub created: String,          // RFC 3339
}

/// The EA model a conversation works with. A conversation keeps its model: choosing another one starts a new conversation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum ModelChoice {
    /// An EA project file (`.qea`/`.qeax`); `ea_user` only for a file with user security.
    File {
        path: String,
        #[serde(default)]
        ea_user: String,
    },
    /// A model in a PostgreSQL database. Passwords are not stored here: the settings' passwords when the connection is
    /// the one from the settings, otherwise Credential Manager entries named after the connection.
    Database {
        name: String,
        host: String,
        port: u16,
        #[serde(default)]
        user: String,
        #[serde(default)]
        sslmode: String,
        #[serde(default)]
        ea_user: String,
    },
}

impl ModelChoice {
    /// The model the settings point to (for new conversations and conversations from before version 0.5).
    pub fn of(cfg: &crate::config::AppConfig) -> Self {
        if cfg.uses_file() {
            Self::File { path: cfg.model_file.display().to_string(), ea_user: cfg.ea_user.clone() }
        } else {
            Self::Database {
                name: cfg.db_name.clone(),
                host: cfg.db_host.clone(),
                port: cfg.db_port,
                user: cfg.db_user.clone(),
                sslmode: cfg.db_sslmode.as_str().to_string(),
                ea_user: cfg.ea_user.clone(),
            }
        }
    }

    /// The EA account of this model (empty for a file without security).
    pub fn ea_user(&self) -> &str {
        match self {
            Self::File { ea_user, .. } | Self::Database { ea_user, .. } => ea_user,
        }
    }

    /// Credential Manager entry with the database password of a connection that differs from the settings.
    pub fn db_secret_name(&self) -> Option<String> {
        match self {
            Self::Database { name, host, port, user, .. } => Some(format!("db_password:{user}@{host}:{port}/{name}")),
            Self::File { .. } => None,
        }
    }

    /// Credential Manager entry with the EA account password of a model that differs from the settings.
    pub fn ea_secret_name(&self) -> String {
        format!("ea_password:{}@{}", self.ea_user(), self.describe())
    }

    /// Whether the database connection is the one from the settings (then the settings' password is used).
    pub fn uses_settings_database(&self, base: &crate::config::AppConfig) -> bool {
        matches!(self, Self::Database { name, host, port, user, .. }
            if !base.uses_file() && &base.db_name == name && &base.db_host == host && base.db_port == *port && &base.db_user == user)
    }

    /// Whether the EA account is the settings' account for the settings' model (then the settings' password is used).
    pub fn uses_settings_ea_account(&self, base: &crate::config::AppConfig) -> bool {
        self.ea_user() == base.ea_user && (self.uses_settings_database(base) || *self == Self::of(base))
    }

    /// Short name: the file name or the database name.
    pub fn label(&self) -> String {
        match self {
            Self::File { path, .. } => Path::new(path).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| path.clone()),
            Self::Database { name, .. } => name.clone(),
        }
    }

    /// Full description for a tooltip: the file path, or database@server:port.
    pub fn describe(&self) -> String {
        match self {
            Self::File { path, .. } => path.clone(),
            Self::Database { name, host, port, .. } => format!("{name}@{host}:{port}"),
        }
    }

    /// The settings for working in this conversation: the application settings with this conversation's model.
    pub fn apply(&self, base: &crate::config::AppConfig) -> Result<crate::config::AppConfig> {
        let mut cfg = base.clone();
        match self {
            Self::File { path, .. } => {
                let file = PathBuf::from(path);
                if !file.is_file() {
                    anyhow::bail!("{}", crate::i18n::fmt("The model file of this conversation no longer exists: {0}. Choose another model in the conversation header.", &[path.clone()]));
                }
                cfg.model_source = crate::config::ModelSource::File;
                cfg.model_file = file;
            }
            Self::Database { name, host, port, user, sslmode, .. } => {
                cfg.model_source = crate::config::ModelSource::Postgres;
                cfg.db_name = name.clone();
                cfg.db_host = host.clone();
                cfg.db_port = *port;
                cfg.db_user = user.clone();
                cfg.db_sslmode = crate::config::SslMode::parse(sslmode)?;
                if !self.uses_settings_database(base) {
                    let key = self.db_secret_name().unwrap_or_default();
                    cfg.db_password = crate::config::secrets::get(&key).unwrap_or_default();
                }
            }
        }
        cfg.ea_user = self.ea_user().to_string();
        if !self.uses_settings_ea_account(base) {
            cfg.ea_password_key = self.ea_secret_name();
        }
        Ok(cfg)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub title: String,
    pub messages: Vec<Message>,
    /// Optimistic locking: incremented on every save. A save with an outdated revision is rejected.
    #[serde(default)]
    pub revision: u64,
    /// The model of this conversation (empty in a new conversation until it is chosen or the first message is sent).
    #[serde(default)]
    pub model: Option<ModelChoice>,
}

#[derive(Debug, thiserror::Error)]
pub enum SaveError {
    #[error("{}", conflict_message(.expected, .found))]
    Conflict { expected: u64, found: u64 },
    #[error(transparent)]
    Io(#[from] anyhow::Error),
}

/// Version-conflict message in the interface language (shown in the status bar).
fn conflict_message(expected: &u64, found: &u64) -> String {
    crate::i18n::fmt("version conflict: the file has revision {0}, but the application expected {1}. The conversation was changed elsewhere - load it again.", &[found.to_string(), expected.to_string()])
}

impl Session {
    pub fn new() -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            title: crate::i18n::tr("New conversation").into(),
            messages: Vec::new(),
            revision: 0,
            model: None,
        }
    }

    pub fn path(&self, history_dir: &Path) -> PathBuf {
        history_dir.join(format!("{}.json", self.id))
    }

    /// Save with optimistic locking: `self.revision` must equal the revision on disk (or the file must not exist yet).
    /// After a successful save the revision is incremented by 1.
    pub fn save(&mut self, history_dir: &Path) -> Result<(), SaveError> {
        std::fs::create_dir_all(history_dir).map_err(|e| anyhow::Error::from(e).context(crate::i18n::tr("history folder")))?;
        let path = self.path(history_dir);
        if path.exists() {
            let on_disk = Session::load(&path).map_err(SaveError::Io)?;
            if on_disk.revision != self.revision {
                return Err(SaveError::Conflict { expected: self.revision, found: on_disk.revision });
            }
        }
        self.revision += 1;
        let json = serde_json::to_string_pretty(self).map_err(|e| anyhow::Error::from(e))?;
        // write via a temporary file so that an interrupted save does not damage the history
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, json).map_err(|e| anyhow::Error::from(e).context(crate::i18n::tr("saving the conversation")))?;
        std::fs::rename(&tmp, &path).map_err(|e| anyhow::Error::from(e).context(crate::i18n::tr("replacing the conversation file")))?;
        Ok(())
    }

    pub fn load(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path)?;
        Ok(serde_json::from_str(&text)?)
    }

    /// All sessions from the folder, newest first (the file name is a UUID, so the order is by mtime).
    pub fn list(history_dir: &Path) -> Vec<(PathBuf, Session)> {
        let mut out: Vec<(std::time::SystemTime, PathBuf, Session)> = std::fs::read_dir(history_dir)
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "json"))
            .filter_map(|p| {
                let mtime = std::fs::metadata(&p).ok()?.modified().ok()?;
                let s = Session::load(&p).ok()?;
                Some((mtime, p, s))
            })
            .collect();
        out.sort_by(|a, b| b.0.cmp(&a.0));
        out.into_iter().map(|(_, p, s)| (p, s)).collect()
    }
}

/// Saves the attachment bytes in the `attachments` folder under a unique name; returns the name stored on disk.
/// Name format: `<uuid without hyphens>_<original name>`.
pub fn store_bytes(attachments_dir: &Path, original: &str, bytes: &[u8]) -> Result<String> {
    std::fs::create_dir_all(attachments_dir)?;
    let name = format!("{}_{}", uuid::Uuid::new_v4().simple(), sanitize_name(original));
    std::fs::write(attachments_dir.join(&name), bytes).context(crate::i18n::tr("saving the attachment"))?;
    Ok(name)
}

/// Original file name from the stored name (without the uuid prefix).
pub fn display_name(stored: &str) -> &str {
    if stored.len() > 33 && stored.as_bytes()[32] == b'_' {
        &stored[33..]
    } else {
        stored
    }
}

pub fn sanitize_name(original: &str) -> String {
    original
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '.' || c == '-' || c == '_' { c } else { '_' })
        .take(80)
        .collect()
}

/// Copies the attachment file into the `attachments` folder; returns the name it was saved under.
pub fn store_attachment(attachments_dir: &Path, source: &Path) -> Result<String> {
    std::fs::create_dir_all(attachments_dir)?;
    let ext = source.extension().and_then(|e| e.to_str()).unwrap_or("bin");
    let stem = source.file_stem().and_then(|s| s.to_str()).unwrap_or(crate::i18n::tr("file"));
    let name = format!("{}_{}.{}", uuid::Uuid::new_v4().simple(), sanitize(stem), ext);
    std::fs::copy(source, attachments_dir.join(&name)).context(crate::i18n::tr("copying the attachment"))?;
    Ok(name)
}

fn sanitize(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .take(40)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings(text: &str) -> crate::config::AppConfig {
        let dir = std::env::temp_dir().join(format!("loonar-model-choice-{}-{}", std::process::id(), text.len()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("app.properties"), text).unwrap();
        crate::config::AppConfig::load(&dir.join("app.properties")).unwrap()
    }

    /// A conversation keeps its own model: the settings' database, another database, or a file.
    #[test]
    fn conversation_model_overrides_the_settings() {
        let base = settings("db.host=srv\r\ndb.port=5432\r\ndb.name=ea\r\ndb.user=reader\r\ndb.password=secret\r\nea.user=claude\r\n");
        let default = ModelChoice::of(&base);
        assert!(default.uses_settings_database(&base) && default.uses_settings_ea_account(&base));
        let same = default.apply(&base).unwrap();
        assert_eq!((same.db_name.as_str(), same.db_password.as_str(), same.ea_password_key.as_str()), ("ea", "secret", crate::config::secrets::EA_PASSWORD));

        let other = ModelChoice::Database { name: "ea2".into(), host: "srv".into(), port: 5432, user: "reader".into(), sslmode: "require".into(), ea_user: "claude".into() };
        assert!(!other.uses_settings_database(&base));
        let cfg = other.apply(&base).unwrap();
        assert_eq!(cfg.db_name, "ea2");
        assert_eq!(cfg.db_sslmode, crate::config::SslMode::Require);
        assert_eq!(other.db_secret_name().unwrap(), "db_password:reader@srv:5432/ea2");
        assert_eq!(cfg.ea_password_key, "ea_password:claude@ea2@srv:5432", "another model has its own EA password entry");

        let file = std::env::temp_dir().join(format!("loonar-model-choice-{}.qea", std::process::id()));
        std::fs::write(&file, b"x").unwrap();
        let in_file = ModelChoice::File { path: file.display().to_string(), ea_user: String::new() };
        let cfg = in_file.apply(&base).unwrap();
        assert!(cfg.uses_file() && cfg.model_file == file && cfg.ea_user.is_empty());
        assert_eq!(in_file.label(), file.file_name().unwrap().to_string_lossy());
        std::fs::remove_file(&file).unwrap();
        assert!(in_file.apply(&base).is_err(), "a missing file is reported");

        // Saved in the conversation file and read back.
        let mut session = Session::new();
        session.model = Some(other.clone());
        let back: Session = serde_json::from_str(&serde_json::to_string(&session).unwrap()).unwrap();
        assert_eq!(back.model, Some(other));
        let old: Session = serde_json::from_str(r#"{"id":"x","title":"t","messages":[]}"#).unwrap();
        assert_eq!(old.model, None, "conversations from before 0.5 have no model yet");
    }
}

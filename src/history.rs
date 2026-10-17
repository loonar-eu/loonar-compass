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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub title: String,
    pub messages: Vec<Message>,
    /// Optimistic locking: incremented on every save. A save with an outdated revision is rejected.
    #[serde(default)]
    pub revision: u64,
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

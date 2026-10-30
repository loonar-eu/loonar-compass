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
        /// The file requires a user lock to edit ("Require User Lock to Edit").
        #[serde(default = "locks_default")]
        locks: bool,
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
        /// postgres (also when empty: conversations from before 0.9), sqlserver, mysql or oracle.
        #[serde(default)]
        engine: String,
        #[serde(default)]
        ea_user: String,
        /// The model requires a user lock to edit ("Require User Lock to Edit").
        #[serde(default = "locks_default")]
        locks: bool,
    },
}

fn locks_default() -> bool {
    true
}

impl ModelChoice {
    /// The model the settings point to (for new conversations and conversations from before version 0.5).
    pub fn of(cfg: &crate::config::AppConfig) -> Self {
        if cfg.uses_file() {
            Self::File { path: cfg.model_file.display().to_string(), ea_user: cfg.ea_user.clone(), locks: cfg.ea_locks }
        } else {
            Self::Database {
                name: cfg.db_name.clone(),
                host: cfg.db_host.clone(),
                port: cfg.db_port,
                user: cfg.db_user.clone(),
                sslmode: cfg.db_sslmode.as_str().to_string(),
                engine: cfg.model_source.as_str().to_string(),
                ea_user: cfg.ea_user.clone(),
                locks: cfg.ea_locks,
            }
        }
    }

    /// The same model (file or database), whatever the account or options: a conversation belongs to the project
    /// open in EA when this holds.
    pub fn same_project(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::File { path: a, .. }, Self::File { path: b, .. }) => {
                let canonical = |p: &str| std::fs::canonicalize(p).map(|p| p.to_string_lossy().to_lowercase()).unwrap_or_else(|_| p.to_lowercase());
                canonical(a) == canonical(b)
            }
            (Self::Database { name: a, host: ha, port: pa, .. }, Self::Database { name: b, host: hb, port: pb, .. }) => {
                a.eq_ignore_ascii_case(b) && ha.eq_ignore_ascii_case(hb) && pa == pb
            }
            _ => false,
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
            Self::File { path, locks, .. } => {
                cfg.ea_locks = *locks;
                let file = PathBuf::from(path);
                if !file.is_file() {
                    anyhow::bail!("{}", crate::i18n::fmt("The model file of this conversation no longer exists: {0}. Choose another model in the conversation header.", &[path.clone()]));
                }
                cfg.model_source = crate::config::ModelSource::File;
                cfg.model_file = file;
            }
            Self::Database { name, host, port, user, sslmode, locks, engine, .. } => {
                cfg.ea_locks = *locks;
                cfg.model_source = crate::config::ModelSource::parse(engine).ok().filter(|k| *k != crate::config::ModelSource::File).unwrap_or(crate::config::ModelSource::Postgres);
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
    /// Places where the conversation went on with another model (continued from another file or database, imported).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub moves: Vec<ModelMove>,
    /// What the application did while making an answer (tools, reads, saves), by the `created` of that answer. The
    /// assistant gets it with the history: without it, it cannot know in the next turn what it already read or changed.
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub steps: std::collections::BTreeMap<String, Vec<String>>,
    /// The earlier part of a long conversation as a summary (LC-301): the model gets it instead of the messages before
    /// `from`. Kept with the conversation, so every later turn sends the same text and reads it from the cache.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<EarlierSummary>,
}

/// A summary of the messages before the message created at `from`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EarlierSummary {
    pub from: String,
    pub text: String,
}

/// The first kept message as the model gets it: the summary of everything before it, then its own text.
pub fn with_summary(summary: &str, text: &str) -> String {
    format!(
        "<summary_of_earlier_conversation>\n{}\n</summary_of_earlier_conversation>\n(The earlier messages of this conversation were replaced by this summary.)\n\n{text}",
        summary.trim()
    )
}

/// The conversation went on with another model from message `at` on (LC-292).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelMove {
    pub at: usize,
    /// The earlier model (`describe()`), as it was then.
    pub from: String,
    pub to: String,
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
            moves: Vec::new(),
            steps: Default::default(),
            summary: None,
        }
    }

    /// A copy of the conversation that goes on with `model`; the original stays with its own model (LC-292).
    pub fn continue_in(&self, model: ModelChoice) -> Session {
        let mut copy = self.clone();
        copy.id = uuid::Uuid::new_v4().to_string();
        copy.revision = 0;
        copy.record_move(model);
        copy
    }

    /// The conversation itself goes on with `model`: from the next message on the assistant is told about the change.
    pub fn record_move(&mut self, model: ModelChoice) {
        let from = self.model.as_ref().map(|m| m.describe()).unwrap_or_default();
        if !self.messages.is_empty() && from != model.describe() {
            let at = self.messages.len();
            self.moves.retain(|m| m.at != at);
            self.moves.push(ModelMove { at, from, to: model.describe() });
        }
        self.model = Some(model);
    }

    /// The messages as the language model gets them: after every change of the model, the first user message starts with
    /// a note of the application, so the assistant does not trust names and ids from the earlier model blindly.
    pub fn messages_for_model(&self) -> Vec<Message> {
        let mut out = self.messages.clone();
        for mv in &self.moves {
            if let Some(m) = out.iter_mut().skip(mv.at).find(|m| m.role == "user") {
                let earlier = if mv.from.is_empty() { String::from("another model") } else { mv.from.clone() };
                m.text = format!(
                    "[Note from the application, not from the user: from this message on the conversation works with another EA model: {} \
                     (the messages before were about {earlier}). Packages, elements and diagrams named earlier may be missing here or have \
                     other ids: check them in this model before you rely on them or change them.]\n\n{}",
                    mv.to, m.text
                );
            }
        }
        // The steps of an answer go to the next message of the user as a note of the application, not into the answer
        // itself: written into its own earlier answers, the model copied the record into its next visible answer
        // (found in the 0.9.2 lab tests).
        for i in 0..out.len() {
            if out[i].role != "assistant" {
                continue;
            }
            let Some(steps) = self.steps.get(&out[i].created).filter(|s| !s.is_empty()) else { continue };
            if let Some(next) = out[i + 1..].iter_mut().find(|m| m.role == "user") {
                next.text = format!(
                    "{STEPS_NOTE}\n{}]\n\n{}",
                    steps.iter().map(|s| format!("- {s}")).collect::<Vec<_>>().join("\n"),
                    next.text
                );
            }
        }
        // A long conversation: the messages before the summary point are replaced by their summary.
        if let Some(summary) = &self.summary {
            if let Some(at) = out.iter().position(|m| m.created == summary.from && m.role == "user") {
                out.drain(..at);
                out[0].text = with_summary(&summary.text, &out[0].text);
            }
        }
        out
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

    /// Moves the conversation to `history\bin` (it can be brought back by moving the file back; the list skips it).
    pub fn move_to_bin(&self, history_dir: &Path) -> Result<PathBuf> {
        let bin = history_dir.join("bin");
        std::fs::create_dir_all(&bin)?;
        let target = bin.join(format!("{}.json", self.id));
        std::fs::rename(self.path(history_dir), &target)?;
        Ok(target)
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
/// The start of the application's note with the steps of the previous answer (see `messages_for_model`).
pub const STEPS_NOTE: &str = "[Note from the application, not from the user and not to be repeated in your answer: steps done while making your previous answer:";

/// What the user wrote in a message prepared for the model: the notes of the application put before it (the steps of
/// the previous answer, a change of the model, a summary) are left out (found in the 0.9.2 tests: a template named in
/// the steps of the previous answer made the next plain question look like a request for a document).
pub fn own_text(text: &str) -> &str {
    let mut rest = text;
    loop {
        let trimmed = rest.trim_start();
        let note = trimmed.starts_with("[Note from the application") || trimmed.starts_with("<summary_of_earlier_conversation>");
        let end = if trimmed.starts_with('<') { trimmed.find("</summary_of_earlier_conversation>").map(|i| i + "</summary_of_earlier_conversation>".len()) } else { trimmed.find("]\n\n").map(|i| i + 1) };
        match (note, end) {
            (true, Some(e)) => rest = &trimmed[e..],
            _ => return trimmed,
        }
    }
}

/// An answer without notes of the application the model may have copied into it (its steps record).
pub fn without_app_notes(text: &str) -> String {
    let mut out = text.to_string();
    for start in [STEPS_NOTE, "[Note from the application, not from the user", "[Record of the application"] {
        while let Some(i) = out.find(start) {
            let end = out[i..].find(']').map_or(out.len(), |e| i + e + 1);
            out.replace_range(i..end, "");
        }
    }
    out.trim_end().to_string()
}

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

/// The file type of an exported conversation (a ZIP: `conversation.json` and the attachments).
pub const EXPORT_EXTENSION: &str = "lcchat";
const EXPORT_FORMAT: &str = "loonar-compass-conversation";
const MAX_IMPORT_BYTES: u64 = 200 * 1024 * 1024;

#[derive(Serialize, Deserialize)]
struct Exported {
    format: String,
    version: u32,
    app: String,
    exported: String,
    title: String,
    /// The model the conversation worked with (`describe()`: the file path or database@server:port, no accounts or passwords).
    model: String,
    messages: Vec<Message>,
    #[serde(default)]
    moves: Vec<ModelMove>,
    #[serde(default)]
    steps: std::collections::BTreeMap<String, Vec<String>>,
    #[serde(default)]
    summary: Option<EarlierSummary>,
}

fn not_exported() -> anyhow::Error {
    anyhow::anyhow!("{}", crate::i18n::tr("This is not a conversation exported from Loonar Compass."))
}

/// Writes the conversation with its attachments to `target` (LC-292).
pub fn export(session: &Session, attachments_dir: &Path, target: &Path) -> Result<()> {
    use std::io::Write;
    let exported = Exported {
        format: EXPORT_FORMAT.into(),
        version: 1,
        app: env!("CARGO_PKG_VERSION").into(),
        exported: chrono::Utc::now().to_rfc3339(),
        title: session.title.clone(),
        model: session.model.as_ref().map(|m| m.describe()).unwrap_or_default(),
        messages: session.messages.clone(),
        moves: session.moves.clone(),
        steps: session.steps.clone(),
        summary: session.summary.clone(),
    };
    let tmp = target.with_extension("lcchat.tmp");
    {
        let file = std::fs::File::create(&tmp).with_context(|| tmp.display().to_string())?;
        let mut zip = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default();
        zip.start_file("conversation.json", options)?;
        zip.write_all(serde_json::to_string_pretty(&exported)?.as_bytes())?;
        let mut done = std::collections::HashSet::new();
        for name in session.messages.iter().flat_map(|m| &m.attachments) {
            if done.insert(name.clone()) {
                // An attachment deleted by hand is left out; the message keeps its name.
                if let Ok(bytes) = std::fs::read(attachments_dir.join(name)) {
                    zip.start_file(format!("attachments/{name}"), options)?;
                    zip.write_all(&bytes)?;
                }
            }
        }
        zip.finish()?;
    }
    std::fs::rename(&tmp, target).with_context(|| target.display().to_string())?;
    Ok(())
}

/// Reads an exported conversation as a new conversation with `model`; the attachments are stored under new names.
pub fn import(source: &Path, attachments_dir: &Path, model: ModelChoice) -> Result<Session> {
    use std::io::Read;
    let file = std::fs::File::open(source).with_context(|| source.display().to_string())?;
    anyhow::ensure!(file.metadata()?.len() <= MAX_IMPORT_BYTES, "{}", crate::i18n::tr("The file is too large for an exported conversation."));
    let mut zip = zip::ZipArchive::new(file).map_err(|_| not_exported())?;
    let mut text = String::new();
    zip.by_name("conversation.json").map_err(|_| not_exported())?.take(MAX_IMPORT_BYTES).read_to_string(&mut text)?;
    let exported: Exported = serde_json::from_str(&text).map_err(|_| not_exported())?;
    anyhow::ensure!(exported.format == EXPORT_FORMAT, not_exported());
    anyhow::ensure!(exported.version == 1, "{}", crate::i18n::tr("The conversation was exported by a newer version of Loonar Compass: update the application."));
    anyhow::ensure!(exported.messages.iter().all(|m| m.role == "user" || m.role == "assistant"), not_exported());
    let mut renamed = std::collections::HashMap::new();
    let mut total = 0u64;
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i)?;
        let Some(stored) = entry.name().strip_prefix("attachments/").map(str::to_string) else { continue };
        // Only a plain file name: nothing is written outside the attachments folder.
        if stored.is_empty() || stored.contains(['/', '\\', ':']) || stored.contains("..") {
            continue;
        }
        let mut bytes = Vec::new();
        entry.by_ref().take(MAX_IMPORT_BYTES).read_to_end(&mut bytes)?;
        total += bytes.len() as u64;
        anyhow::ensure!(total <= MAX_IMPORT_BYTES, "{}", crate::i18n::tr("The file is too large for an exported conversation."));
        renamed.insert(stored.clone(), store_bytes(attachments_dir, display_name(&stored), &bytes)?);
    }
    let mut session = Session::new();
    session.title = exported.title.chars().take(200).collect();
    session.messages = exported.messages;
    for m in &mut session.messages {
        m.attachments = m.attachments.iter().map(|a| renamed.get(a).cloned().unwrap_or_else(|| a.clone())).collect();
    }
    let count = session.messages.len();
    session.moves = exported.moves.into_iter().filter(|m| m.at <= count).collect();
    session.steps = exported.steps;
    session.summary = exported.summary;
    let to = model.describe();
    if count > 0 && exported.model != to {
        session.moves.retain(|m| m.at != count);
        session.moves.push(ModelMove { at: count, from: exported.model, to });
    }
    session.model = Some(model);
    Ok(session)
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

        let other = ModelChoice::Database { name: "ea2".into(), host: "srv".into(), port: 5432, user: "reader".into(), engine: String::new(), sslmode: "require".into(), ea_user: "claude".into(), locks: false };
        assert!(!other.uses_settings_database(&base));
        let cfg = other.apply(&base).unwrap();
        assert_eq!(cfg.db_name, "ea2");
        assert!(!cfg.ea_locks, "the lock option of the conversation model");
        assert_eq!(cfg.db_sslmode, crate::config::SslMode::Require);
        assert_eq!(other.db_secret_name().unwrap(), "db_password:reader@srv:5432/ea2");
        assert_eq!(cfg.ea_password_key, "ea_password:claude@ea2@srv:5432", "another model has its own EA password entry");

        let file = std::env::temp_dir().join(format!("loonar-model-choice-{}.qea", std::process::id()));
        std::fs::write(&file, b"x").unwrap();
        let in_file = ModelChoice::File { path: file.display().to_string(), ea_user: String::new(), locks: true };
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

    fn db() -> ModelChoice {
        ModelChoice::Database { name: "sklep".into(), host: "srv".into(), port: 5432, user: "u".into(), sslmode: String::new(), engine: String::new(), ea_user: String::new(), locks: true }
    }

    fn file(path: &str) -> ModelChoice {
        ModelChoice::File { path: path.into(), ea_user: String::new(), locks: true }
    }

    fn talk() -> Session {
        let mut s = Session::new();
        s.model = Some(db());
        for (role, text) in [("user", "Opisz pakiet Sprzedaż"), ("assistant", "Pakiet ma 3 elementy.")] {
            s.messages.push(Message { role: role.into(), text: text.into(), attachments: Vec::new(), created: String::new() });
        }
        s
    }

    #[test]
    fn continued_conversation_is_a_copy_and_tells_the_assistant() {
        let original = talk();
        let mut copy = original.continue_in(file("C:/m/sklep.qea"));
        assert_ne!(copy.id, original.id);
        assert_eq!(original.model, Some(db()), "the original keeps its model");
        assert_eq!(copy.moves.len(), 1);
        assert_eq!(copy.moves[0].at, 2);
        // No note until the next user message; then it starts with the note, the stored text stays as typed.
        assert!(copy.messages_for_model().iter().all(|m| !m.text.contains("Note from the application")));
        copy.messages.push(Message { role: "user".into(), text: "Dodaj element".into(), attachments: Vec::new(), created: String::new() });
        let sent = copy.messages_for_model();
        assert!(sent[2].text.starts_with("[Note from the application") && sent[2].text.contains("sklep@srv:5432") && sent[2].text.ends_with("Dodaj element"));
        assert_eq!(copy.messages[2].text, "Dodaj element");
        // The steps of an answer go to the model with the next user message (not into the answer, which the model would
        // copy), not to the stored text.
        let mut with_steps = talk();
        with_steps.steps.insert(with_steps.messages[1].created.clone(), vec!["Reading the Confluence page \"W2\" ✓".into()]);
        with_steps.messages.push(Message { role: "user".into(), text: "Czy czytałeś Confluence?".into(), attachments: Vec::new(), created: "t9".into() });
        let sent = with_steps.messages_for_model();
        assert_eq!(sent[1].text, "Pakiet ma 3 elementy.");
        assert!(sent[2].text.starts_with(STEPS_NOTE) && sent[2].text.contains("Confluence page") && sent[2].text.ends_with("Czy czytałeś Confluence?"));
        assert_eq!(with_steps.messages[1].text, "Pakiet ma 3 elementy.");
        assert_eq!(without_app_notes(&format!("Gotowe.\n\n{STEPS_NOTE}\n- db_query ✓]")), "Gotowe.");
        assert_eq!(without_app_notes("Gotowe. [Record of the application, not visible to the user: steps]"), "Gotowe.");
        assert_eq!(own_text(&sent[2].text), "Czy czytałeś Confluence?");
        assert_eq!(own_text("Dodaj element"), "Dodaj element");
        // A summary replaces the messages before its point, the same way in every turn; the stored messages stay.
        let mut long = talk();
        long.messages.push(Message { role: "user".into(), text: "Dalej".into(), attachments: Vec::new(), created: "t3".into() });
        long.summary = Some(EarlierSummary { from: "t3".into(), text: "Opisano pakiet Sprzedaż.".into() });
        let sent = long.messages_for_model();
        assert_eq!(sent.len(), 1);
        assert!(sent[0].text.starts_with("<summary_of_earlier_conversation>") && sent[0].text.ends_with("Dalej"));
        assert_eq!(long.messages_for_model()[0].text, sent[0].text);
        assert_eq!(long.messages.len(), 3);
        // The same model again is no move.
        let mut same = talk();
        same.record_move(db());
        assert!(same.moves.is_empty());
    }

    #[test]
    fn export_and_import_keep_messages_and_attachments() {
        let dir = std::env::temp_dir().join(format!("lc292-{}", uuid::Uuid::new_v4().simple()));
        let (att, other_att) = (dir.join("a"), dir.join("b"));
        let mut s = talk();
        let stored = store_bytes(&att, "shot.png", b"png-bytes").unwrap();
        s.messages[0].attachments.push(stored.clone());
        let target = dir.join("talk.lcchat");
        export(&s, &att, &target).unwrap();
        let back = import(&target, &other_att, file("C:/m/sklep.qea")).unwrap();
        assert_ne!(back.id, s.id);
        assert_eq!(back.messages.len(), 2);
        assert_eq!(back.model, Some(file("C:/m/sklep.qea")));
        assert_eq!(back.moves.last().map(|m| (m.at, m.from.as_str())), Some((2, "sklep@srv:5432")));
        let name = &back.messages[0].attachments[0];
        assert_ne!(name, &stored, "a new name in the attachments folder");
        assert_eq!(std::fs::read(other_att.join(name)).unwrap(), b"png-bytes");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn import_rejects_other_files_and_paths_outside_the_folder() {
        use std::io::Write;
        let dir = std::env::temp_dir().join(format!("lc292-{}", uuid::Uuid::new_v4().simple()));
        std::fs::create_dir_all(&dir).unwrap();
        let plain = dir.join("x.lcchat");
        std::fs::write(&plain, b"not a zip").unwrap();
        assert!(import(&plain, &dir.join("att"), db()).is_err());
        // A conversation with an attachment entry that tries to leave the folder: the entry is skipped.
        let evil = dir.join("evil.lcchat");
        let mut zip = zip::ZipWriter::new(std::fs::File::create(&evil).unwrap());
        let o = zip::write::SimpleFileOptions::default();
        zip.start_file("conversation.json", o).unwrap();
        zip.write_all(br#"{"format":"loonar-compass-conversation","version":1,"app":"0.9.1","exported":"","title":"t","model":"m","messages":[{"role":"user","text":"x","attachments":["../../evil.txt"],"created":""}]}"#).unwrap();
        zip.start_file("attachments/../../evil.txt", o).unwrap();
        zip.write_all(b"boom").unwrap();
        zip.finish().unwrap();
        let back = import(&evil, &dir.join("att"), db()).unwrap();
        assert_eq!(back.messages.len(), 1);
        assert!(!dir.join("evil.txt").exists() && !std::env::temp_dir().join("evil.txt").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}

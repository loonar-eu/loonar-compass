//! Configuration: `app.properties` (next to the application) and secrets from Windows Credential Manager.

use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// The only model the application uses. It is not a user setting.
pub const CLAUDE_MODEL: &str = "claude-haiku-5-5";
/// Claude models offered in the settings (any other id can still be set in app.properties).
pub const CLAUDE_MODELS: [&str; 4] = ["claude-haiku-5-5", "claude-sonnet-5-5", "claude-opus-5-5", "claude-fable-5-1"];
/// OpenAI models offered in the settings (any other id can be typed in).
pub const OPENAI_MODELS: [&str; 3] = ["gpt-5.1", "gpt-5", "gpt-5-mini"];
/// Effort levels of the Claude API (output_config.effort); "" = the model's default.
pub const EFFORTS: [&str; 6] = ["", "low", "medium", "high", "xhigh", "max"];

/// Product name: user data folder, window titles, shortcuts.
pub const APP_NAME: &str = "Loonar Compass";
/// Product name of the pre-release versions (0.1–0.3): their data and secrets are migrated on first start.
const LEGACY_APP_NAME: &str = "Loonar EA Assistant";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApprovalMode {
    /// The model shows a plan first; you approve it once.
    Plan,
    /// Every write requires a separate approval.
    Ask,
    /// Writes without asking (publishing to Confluence still asks).
    Auto,
}

impl ApprovalMode {
    pub fn parse(s: &str) -> Result<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "plan" => Ok(Self::Plan),
            "ask" => Ok(Self::Ask),
            "auto" => Ok(Self::Auto),
            other => anyhow::bail!("{}", crate::i18n::fmt("unknown approval mode '{0}' (plan, ask, auto)", &[other.to_string()])),
        }
    }

    /// How strictly writes are approved: auto < plan < ask (the team policy can only make it stricter).
    pub fn strictness(&self) -> u8 {
        match self {
            Self::Auto => 0,
            Self::Plan => 1,
            Self::Ask => 2,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Plan => "plan",
            Self::Ask => "ask",
            Self::Auto => "auto",
        }
    }
}

/// Where the EA model is (`model.source`): a database repository (PostgreSQL directly; SQL Server, MySQL and Oracle
/// through their ODBC drivers) or an EA project file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelSource {
    /// A model database on a PostgreSQL server (`db.*` settings).
    Postgres,
    /// An EA project file `.qea` or `.qeax` (a SQLite database), path in `model.file`.
    File,
    /// A repository on Microsoft SQL Server (ODBC Driver 18/17 for SQL Server or Windows' own "SQL Server" driver).
    SqlServer,
    /// A repository on MySQL or MariaDB (MySQL Connector/ODBC).
    MySql,
    /// A repository on Oracle (Oracle's ODBC driver, `db.name` = the service name).
    Oracle,
}

impl ModelSource {
    pub fn parse(s: &str) -> Result<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "postgres" | "postgresql" => Ok(Self::Postgres),
            "file" => Ok(Self::File),
            "sqlserver" | "mssql" => Ok(Self::SqlServer),
            "mysql" | "mariadb" => Ok(Self::MySql),
            "oracle" => Ok(Self::Oracle),
            other => anyhow::bail!("{}", crate::i18n::fmt("unknown model source '{0}' (postgres, sqlserver, mysql, oracle, file)", &[other.to_string()])),
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Postgres => "postgres",
            Self::File => "file",
            Self::SqlServer => "sqlserver",
            Self::MySql => "mysql",
            Self::Oracle => "oracle",
        }
    }

    /// The model is read through an ODBC driver.
    pub fn is_odbc(&self) -> bool {
        matches!(self, Self::SqlServer | Self::MySql | Self::Oracle)
    }

    /// The usual port of the database server.
    pub fn default_port(&self) -> u16 {
        match self {
            Self::SqlServer => 1433,
            Self::MySql => 3306,
            Self::Oracle => 1521,
            _ => 5432,
        }
    }
}

/// Extensions of the EA project files the application opens (both are SQLite databases; `.qeax` is the shared variant).
pub const MODEL_FILE_EXTENSIONS: [&str; 2] = ["qea", "qeax"];

/// Whether the path has one of the supported extensions (`.qea`, `.qeax`, any letter case).
pub fn is_model_file(path: &Path) -> bool {
    path.extension().and_then(|e| e.to_str()).is_some_and(|e| MODEL_FILE_EXTENSIONS.iter().any(|x| e.eq_ignore_ascii_case(x)))
}

/// Encryption of the connection to the model database (`db.sslmode`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SslMode {
    /// TLS when the server offers it, otherwise unencrypted (works with every server).
    Prefer,
    /// TLS required; the certificate is not verified.
    Require,
    /// TLS required, the certificate and host name are verified against the Windows certificate store.
    VerifyFull,
}

impl SslMode {
    pub const ALL: [SslMode; 3] = [SslMode::Prefer, SslMode::Require, SslMode::VerifyFull];

    pub fn parse(s: &str) -> Result<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "prefer" => Ok(Self::Prefer),
            "require" => Ok(Self::Require),
            "verify-full" => Ok(Self::VerifyFull),
            other => anyhow::bail!("{}", crate::i18n::fmt("unknown database encryption mode '{0}' (prefer, require, verify-full)", &[other.to_string()])),
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Prefer => "prefer",
            Self::Require => "require",
            Self::VerifyFull => "verify-full",
        }
    }
}

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub base_dir: PathBuf,
    pub claude_model: String,
    /// Effort (output_config.effort): low | medium | high | xhigh | max; empty = the model's default.
    pub claude_effort: String,
    /// Claude through another address (Azure AI Foundry, a company gateway): the base URL, e.g.
    /// https://<resource>.services.ai.azure.com/anthropic (`claude.url`; empty = api.anthropic.com).
    pub claude_url: String,
    /// OAuth2 client credentials for the AI service instead of an API key (`ai.oauth.token_url`, `ai.oauth.client_id`,
    /// `ai.oauth.scope`; the secret `ai_oauth_client_secret`), e.g. Entra ID with scope https://cognitiveservices.azure.com/.default.
    pub ai_oauth_token_url: String,
    pub ai_oauth_client_id: String,
    pub ai_oauth_scope: String,
    /// Code execution in Anthropic's sandbox for calculations and data analysis (`claude.code_execution`, off by
    /// default): the data the assistant passes there goes to Anthropic like the rest of the conversation.
    pub code_execution: bool,
    /// Show a summary of the model's reasoning in the steps of an answer (`claude.show_thinking`).
    pub show_thinking: bool,
    /// Model provider: "anthropic" (Claude) or "openai".
    pub ai_provider: String,
    /// OpenAI model id (used when the provider is OpenAI).
    pub openai_model: String,
    /// Endpoint compatible with the OpenAI API (empty = OpenAI): Azure OpenAI, Ollama, LM Studio, a company gateway.
    pub openai_url: String,
    pub claude_max_tokens: u32,
    /// Workspace ID from the Anthropic Console: required for an API key that is not scoped to a workspace
    /// (empty = the header is not sent).
    pub claude_workspace_id: String,
    pub model_source: ModelSource,
    /// EA project file (`.qea`/`.qeax`) when `model_source` is `File`.
    pub model_file: PathBuf,
    pub db_host: String,
    pub db_port: u16,
    pub db_name: String,
    pub db_user: String,
    pub db_password: String,
    pub db_sslmode: SslMode,
    /// SQL Server, MySQL, Oracle: a full ODBC connection string instead of the one built from host, port and name
    /// (`db.odbc`, without the password: it is added from the secret).
    pub db_odbc: String,
    pub ea_path: PathBuf,
    pub ea_user: String,
    /// Credential Manager entry with the EA account password (the settings' account, or a conversation's own account).
    pub ea_password_key: String,
    /// The model (file or database) has "Require User Lock to Edit": the assistant locks what it changes (default on).
    pub ea_locks: bool,
    pub approval_mode: ApprovalMode,
    pub history_dir: PathBuf,
    pub attachments_dir: PathBuf,
    /// Assistant memory: notes kept between conversations (.md files).
    pub memory_dir: PathBuf,
    /// Log of the changes the assistant made to the model (JSON Lines, one record per line).
    pub audit_file: PathBuf,
    /// Document templates (.md files) that the assistant fills from the model.
    pub templates_dir: PathBuf,
    /// Language of the interface and of the default documents: pl, en, de, fr (no entry = pl, as before version 0.3).
    pub ui_language: String,
    /// Confluence: base URL, e-mail (Confluence Cloud only; empty = personal access token, PAT), space and parent page.
    pub confluence_url: String,
    pub confluence_user: String,
    pub confluence_space: String,
    pub confluence_parent: String,
    /// Jira: address, account e-mail (Cloud; empty for a personal access token) and the default project key.
    pub jira_url: String,
    pub jira_user: String,
    pub jira_project: String,
    /// Team hub: address and the id of this model among the hub sources (for the audit trail).
    pub hub_url: String,
    pub hub_source: String,
    /// The Miro board the last diagram went to (its id): offered first next time.
    pub miro_board: String,
    /// Registry of the pages published by the assistant (for refreshing).
    pub confluence_pages_file: PathBuf,
}

/// Full path of a Windows program (explorer.exe in the Windows folder, the others in System32). Programs are never
/// started by a bare name, so a same-named file in the current or download folder cannot be started instead.
pub fn system_program(name: &str) -> PathBuf {
    let windows = std::env::var_os("SystemRoot").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(r"C:\Windows"));
    match name.to_ascii_lowercase().as_str() {
        "explorer.exe" => windows.join(name),
        "powershell.exe" => windows.join(r"System32\WindowsPowerShell\v1.0\powershell.exe"),
        _ => windows.join("System32").join(name),
    }
}

fn appdata() -> PathBuf {
    std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."))
}

/// User data folder (settings, conversation history, attachments) of an installed application:
/// `%APPDATA%\Loonar Compass`.
pub fn user_data_dir() -> PathBuf {
    appdata().join(APP_NAME)
}

/// Moves the data of the pre-release product name (`%APPDATA%\Loonar EA Assistant`) to the current
/// folder, once, when the current folder does not exist yet. On failure the old folder stays as it is.
pub fn migrate_legacy_data_dir() {
    let current = user_data_dir();
    let legacy = appdata().join(LEGACY_APP_NAME);
    if !current.exists() && legacy.is_dir() {
        let _ = std::fs::rename(&legacy, &current);
    }
}

/// Path of a file or folder in `base` that had a Polish name before version 0.3: renames the old one
/// when only the old one exists, and keeps using the old one if renaming fails.
fn renamed(base: &Path, legacy: &str, current: &str) -> PathBuf {
    let (old, new) = (base.join(legacy), base.join(current));
    if old.exists() && !new.exists() && std::fs::rename(&old, &new).is_err() {
        return old;
    }
    new
}

/// `key=value` parser (simplified java.util.Properties format): `#`/`!` comments.
pub fn parse_properties(text: &str) -> HashMap<String, String> {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#') && !l.starts_with('!'))
        .filter_map(|l| {
            let idx = l.find('=')?;
            Some((l[..idx].trim().to_string(), l[idx + 1..].trim().to_string()))
        })
        .collect()
}

/// Looks for `app.properties` next to the executable (a portable copy keeps its data there).
/// Development and test builds also look in the parent folders (running from `target\debug` finds the
/// file in the project folder); a release build never does, so a stray `app.properties` in a parent
/// folder cannot take over the configuration.
pub fn find_properties(start: &Path) -> Option<PathBuf> {
    let depth = if cfg!(any(debug_assertions, test)) { 4 } else { 1 };
    start
        .ancestors()
        .take(depth)
        .map(|d| d.join("app.properties"))
        .find(|p| p.is_file())
}

impl AppConfig {
    /// Whether the conversation model is OpenAI's (otherwise Claude).
    pub fn uses_openai(&self) -> bool {
        self.ai_provider == "openai"
    }

    /// The model id that answers (of the chosen provider).
    pub fn ai_model(&self) -> &str {
        if self.uses_openai() { &self.openai_model } else { &self.claude_model }
    }

    pub fn uses_file(&self) -> bool {
        self.model_source == ModelSource::File
    }

    /// Short name of the model for messages: the database name or the file name.
    pub fn model_label(&self) -> String {
        if self.uses_file() {
            self.model_file.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
        } else {
            self.db_name.clone()
        }
    }

    /// The working folder is the folder containing `app.properties` (history and attachments next to the application).
    pub fn load(props_path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(props_path)
            .with_context(|| crate::i18n::fmt("cannot read {0}", &[props_path.display().to_string()]))?;
        let p = parse_properties(&text);
        let base_dir = props_path.parent().unwrap_or(Path::new(".")).to_path_buf();
        // Missing keys get default values: settings are saved in groups, so the file may be incomplete.
        let get_or = |k: &str, d: &str| p.get(k).cloned().unwrap_or_else(|| d.to_string());
        // Explicit paths from the file win; otherwise the default name, renaming the pre-0.3 Polish one.
        let path_or = |k: &str, legacy: &str, current: &str| match p.get(k) {
            Some(v) if !v.is_empty() => base_dir.join(v),
            _ => renamed(&base_dir, legacy, current),
        };

        Ok(Self {
            // The model is fixed: the `claude.model` entry in the file is informational only and does not change the model.
            claude_model: get_or("claude.model", CLAUDE_MODEL),
            claude_effort: get_or("claude.effort", ""),
            show_thinking: get_or("claude.show_thinking", "false") == "true",
            code_execution: get_or("claude.code_execution", "false") == "true",
            claude_url: get_or("claude.url", ""),
            ai_oauth_token_url: get_or("ai.oauth.token_url", ""),
            ai_oauth_client_id: get_or("ai.oauth.client_id", ""),
            ai_oauth_scope: get_or("ai.oauth.scope", ""),
            ai_provider: get_or("ai.provider", "anthropic"),
            openai_model: get_or("openai.model", OPENAI_MODELS[0]),
            openai_url: get_or("openai.url", ""),
            claude_max_tokens: get_or("claude.max_tokens", "10000").parse().context("claude.max_tokens")?,
            claude_workspace_id: get_or("claude.workspace_id", ""),
            model_source: ModelSource::parse(&get_or("model.source", "postgres")).context("model.source")?,
            model_file: PathBuf::from(get_or("model.file", "")),
            db_host: get_or("db.host", ""),
            db_port: get_or("db.port", &ModelSource::parse(&get_or("model.source", "postgres")).map(|s| s.default_port()).unwrap_or(5432).to_string()).parse().context("db.port")?,
            db_name: get_or("db.name", ""),
            db_user: get_or("db.user", "postgres"),
            db_password: migrate_db_password(props_path, p.get("db.password").map(String::as_str).unwrap_or("")),
            db_sslmode: SslMode::parse(&get_or("db.sslmode", "prefer")).context("db.sslmode")?,
            db_odbc: get_or("db.odbc", ""),
            // Older files have the path stored with doubled backslashes (C:\\Program Files\\...): convert it to a normal one.
            ea_path: PathBuf::from({
                let raw = get_or("ea.path", r"C:\Program Files\Sparx Systems\EA\EA.exe");
                if raw.contains(":\\\\") { raw.replace("\\\\", "\\") } else { raw }
            }),
            ea_user: get_or("ea.user", ""),
            ea_password_key: secrets::EA_PASSWORD.to_string(),
            ea_locks: get_or("ea.locks", "true") != "false",
            approval_mode: ApprovalMode::parse(&get_or("approval.mode", "ask"))?,
            history_dir: base_dir.join(get_or("history.dir", "history")),
            attachments_dir: base_dir.join(get_or("attachments.dir", "attachments")),
            memory_dir: base_dir.join(get_or("memory.dir", "memory")),
            audit_file: path_or("audit.file", "dziennik_zmian.jsonl", "audit_log.jsonl"),
            templates_dir: path_or("templates.dir", "szablony", "templates"),
            // Settings without a language come from versions before 0.3, which were Polish only.
            ui_language: get_or("ui.language", "pl"),
            confluence_url: get_or("confluence.url", ""),
            confluence_user: get_or("confluence.user", ""),
            confluence_space: get_or("confluence.space", ""),
            confluence_parent: get_or("confluence.parent", ""),
            jira_url: get_or("jira.url", ""),
            jira_user: get_or("jira.user", ""),
            jira_project: get_or("jira.project", ""),
            hub_url: get_or("hub.url", ""),
            hub_source: get_or("hub.source", ""),
            miro_board: get_or("miro.board", ""),
            confluence_pages_file: renamed(&base_dir, "confluence_strony.json", "confluence_pages.json"),
            base_dir,
        })
    }
}

/// The database password lives in Credential Manager. Versions before 0.3 kept it in `app.properties`:
/// in an installed application such a password is moved to Credential Manager and removed from the file (it stays
/// in the file only if Credential Manager refuses it). A portable copy (its own `app.properties` next to the program,
/// e.g. a test setup) keeps a password from its file, so it never overwrites the installed application's password.
fn migrate_db_password(props_path: &Path, from_file: &str) -> String {
    if from_file.is_empty() {
        return secrets::get(secrets::DB_PASSWORD).unwrap_or_default();
    }
    if is_installed(props_path) && secrets::set(secrets::DB_PASSWORD, from_file).is_ok() {
        let _ = remove_properties(props_path, &["db.password"]);
    }
    from_file.to_string()
}

/// Whether these are the settings of the installed application (in the user profile) rather than of a portable copy.
pub fn is_installed(props_path: &Path) -> bool {
    props_path.parent().is_some_and(|dir| dir == user_data_dir())
}

/// Removes the given keys from `app.properties`; other lines stay unchanged.
pub fn remove_properties(path: &Path, keys: &[&str]) -> Result<()> {
    let text = std::fs::read_to_string(path)?;
    let kept: Vec<&str> = text
        .lines()
        .filter(|line| {
            let t = line.trim_start();
            !keys.iter().any(|k| t.starts_with(&format!("{k}=")) || t.starts_with(&format!("{k} =")))
        })
        .collect();
    std::fs::write(path, kept.join("\r\n") + "\r\n")
        .with_context(|| crate::i18n::fmt("saving {0}", &[path.display().to_string()]))?;
    Ok(())
}

/// Writes the given keys to `app.properties` (creates the file if it does not exist).
/// Other lines, including comments, stay unchanged; missing keys are appended at the end.
pub fn save_properties(path: &Path, updates: &[(&str, String)]) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).with_context(|| crate::i18n::fmt("settings folder {0}", &[parent.display().to_string()]))?;
    }
    let text = if path.exists() { std::fs::read_to_string(path)? } else { String::new() };
    let mut done = vec![false; updates.len()];
    let mut lines: Vec<String> = text
        .lines()
        .map(|line| {
            let t = line.trim();
            if !t.starts_with('#') {
                for (i, (k, v)) in updates.iter().enumerate() {
                    if t.starts_with(&format!("{k}=")) {
                        done[i] = true;
                        return format!("{k}={v}");
                    }
                }
            }
            line.to_string()
        })
        .collect();
    for (i, (k, v)) in updates.iter().enumerate() {
        if !done[i] {
            lines.push(format!("{k}={v}"));
        }
    }
    std::fs::write(path, lines.join("\r\n") + "\r\n")
        .with_context(|| crate::i18n::fmt("saving {0}", &[path.display().to_string()]))?;
    Ok(())
}

/// Secrets in Windows Credential Manager (Claude API key, passwords, Confluence token).
pub mod secrets {
    use anyhow::{Context, Result};

    const SERVICE: &str = "loonar-compass";
    /// Service name of the pre-release versions: secrets found there are moved to `SERVICE`.
    const LEGACY_SERVICE: &str = "loonar-ea-assistant";
    pub const CLAUDE_API_KEY: &str = "claude_api_key";
    pub const OPENAI_API_KEY: &str = "openai_api_key";
    pub const EA_PASSWORD: &str = "ea_password";
    pub const DB_PASSWORD: &str = "db_password";
    pub const CONFLUENCE_TOKEN: &str = "confluence_token";
    pub const MIRO_TOKEN: &str = "miro_token";
    pub const JIRA_TOKEN: &str = "jira_token";
    /// The client secret for OAuth2 sign-in to the AI service.
    pub const AI_OAUTH_SECRET: &str = "ai_oauth_client_secret";
    /// The client secret of the hub at the company sign-in (OIDC).
    pub const OIDC_CLIENT_SECRET: &str = "oidc_client_secret";
    pub const HUB_TOKEN: &str = "hub_token";
    /// Every secret the application stores (removed together on uninstall).
    pub const ALL: [&str; 8] = [CLAUDE_API_KEY, OPENAI_API_KEY, EA_PASSWORD, DB_PASSWORD, CONFLUENCE_TOKEN, MIRO_TOKEN, JIRA_TOKEN, HUB_TOKEN];

    /// Secrets from the environment instead of Credential Manager: the team hub as a service or in a container.
    static FROM_ENVIRONMENT: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

    pub fn use_environment() {
        FROM_ENVIRONMENT.store(true, std::sync::atomic::Ordering::Relaxed);
    }

    /// `LOONAR_SECRET_<NAME>` or the file named by `LOONAR_SECRET_<NAME>_FILE` (Docker secrets); NAME in capitals,
    /// other characters as "_" (claude_api_key → LOONAR_SECRET_CLAUDE_API_KEY).
    fn from_environment(name: &str) -> Option<String> {
        let key: String = name.chars().map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_uppercase() } else { '_' }).collect();
        if let Ok(value) = std::env::var(format!("LOONAR_SECRET_{key}")) {
            return Some(value.trim().to_string()).filter(|v| !v.is_empty());
        }
        let file = std::env::var(format!("LOONAR_SECRET_{key}_FILE")).ok()?;
        std::fs::read_to_string(file).ok().map(|v| v.trim().to_string()).filter(|v| !v.is_empty())
    }

    pub fn get(name: &str) -> Result<String> {
        if FROM_ENVIRONMENT.load(std::sync::atomic::Ordering::Relaxed) || cfg!(not(windows)) {
            if let Some(value) = from_environment(name) {
                return Ok(value);
            }
            if cfg!(not(windows)) {
                let key: String = name.chars().map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_uppercase() } else { '_' }).collect();
                anyhow::bail!("{}", crate::i18n::fmt("secret '{0}' is not set: give it as LOONAR_SECRET_{1} (or a file named in LOONAR_SECRET_{1}_FILE)", &[name.to_string(), key]));
            }
        }
        if let Ok(value) = keyring::Entry::new(SERVICE, name).and_then(|e| e.get_password()) {
            return Ok(value);
        }
        // Copied, not moved: a pre-release version still installed next to this one keeps working.
        // Uninstalling removes both (see `delete`).
        let value = keyring::Entry::new(LEGACY_SERVICE, name)?
            .get_password()
            .with_context(|| crate::i18n::fmt("secret '{0}' not found in Credential Manager", &[name.to_string()]))?;
        let _ = set(name, &value);
        Ok(value)
    }

    pub fn set(name: &str, value: &str) -> Result<()> {
        keyring::Entry::new(SERVICE, name)?.set_password(value)?;
        Ok(())
    }

    pub fn exists(name: &str) -> bool {
        get(name).is_ok()
    }

    /// File in the user data folder with the names of the secrets saved for conversation connections (their names
    /// are built from the connection), so that uninstalling can remove them too.
    pub const CONNECTION_LIST: &str = "connection_secrets.txt";

    /// Saves a secret of a conversation connection and records its name in `CONNECTION_LIST` in `data_dir`.
    pub fn set_for_connection(data_dir: &std::path::Path, name: &str, value: &str) -> Result<()> {
        set(name, value)?;
        let list = data_dir.join(CONNECTION_LIST);
        let known = std::fs::read_to_string(&list).unwrap_or_default();
        if !known.lines().any(|l| l == name) {
            let _ = std::fs::create_dir_all(data_dir);
            std::fs::write(&list, format!("{known}{name}\n")).with_context(|| crate::i18n::fmt("saving {0}", &[list.display().to_string()]))?;
        }
        Ok(())
    }

    /// Names of the conversation connection secrets recorded in `data_dir`.
    pub fn connection_names(data_dir: &std::path::Path) -> Vec<String> {
        std::fs::read_to_string(data_dir.join(CONNECTION_LIST)).unwrap_or_default().lines().filter(|l| !l.trim().is_empty()).map(str::to_string).collect()
    }

    /// Removes the secret (also under the pre-release service name); a missing secret is not an error.
    pub fn delete(name: &str) -> Result<()> {
        for service in [SERVICE, LEGACY_SERVICE] {
            match keyring::Entry::new(service, name)?.delete_credential() {
                Ok(()) | Err(keyring::Error::NoEntry) => {}
                Err(e) => return Err(e.into()),
            }
        }
        Ok(())
    }
}

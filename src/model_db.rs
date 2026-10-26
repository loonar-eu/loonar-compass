//! Reading the EA model: a PostgreSQL repository (read-only session) or an EA project file (SQLite, read-only), or
//! through the EA window Compass works in. The read-only tools of the assistant (db_query, impact_analysis,
//! operation_usage, model_lint, gap_analysis) are defined here: the desktop application and the team hub share them.

use crate::config::AppConfig;
use anyhow::{bail, Context, Result};
use postgres::SimpleQueryMessage;
use serde_json::{json, Value};
use std::time::Duration;

pub(crate) const MAX_ROWS: usize = 200;

/// Where the model is: the SQL dialect of db_query.
pub const MODEL_IN_POSTGRES: &str = "The model is in a PostgreSQL database: write db_query queries in PostgreSQL SQL (ILIKE for case-insensitive search).";
pub const MODEL_IN_FILE: &str = "The model is an Enterprise Architect project file (.qea/.qeax), which is a SQLite database: write db_query queries in SQLite SQL. \
There is no ILIKE (LIKE already ignores the case of ASCII letters; for other letters compare lower(...)), no :: casts (use CAST(x AS type)), \
booleans are 0/1, and functions such as string_agg are group_concat. The tables and columns are the same as in a database model.";

pub const MODEL_IN_SQLSERVER: &str = "The model is in a Microsoft SQL Server database: write db_query queries in T-SQL (SELECT TOP n instead of LIMIT, + or CONCAT for joining text, \
LIKE ignores the letter case with the usual collations, no ILIKE, no :: casts). The tables and columns are the same as in any EA repository.";
pub const MODEL_IN_MYSQL: &str = "The model is in a MySQL database: write db_query queries in MySQL SQL (LIMIT n, CONCAT() for joining text, LIKE ignores the letter case, no ILIKE, \
no :: casts, GROUP_CONCAT instead of string_agg). The tables and columns are the same as in any EA repository.";
pub const MODEL_IN_ORACLE: &str = "The model is in an Oracle database: write db_query queries in Oracle SQL (FETCH FIRST n ROWS ONLY instead of LIMIT, || for joining text, \
LIKE is case-sensitive so compare UPPER(...), no ILIKE, LISTAGG instead of string_agg, no AS before table aliases). The tables and columns are the same as in any EA repository.";

/// The SQL dialect note for the assistant.
pub fn dialect(cfg: &AppConfig) -> &'static str {
    match cfg.model_source {
        crate::config::ModelSource::File => MODEL_IN_FILE,
        crate::config::ModelSource::Postgres => MODEL_IN_POSTGRES,
        crate::config::ModelSource::SqlServer => MODEL_IN_SQLSERVER,
        crate::config::ModelSource::MySql => MODEL_IN_MYSQL,
        crate::config::ModelSource::Oracle => MODEL_IN_ORACLE,
    }
}

/// A value inside an ODBC connection string: in braces, a closing brace doubled.
fn odbc_value(value: &str) -> String {
    format!("{{{}}}", value.replace('}', "}}"))
}

/// The installed ODBC driver for the database: the newest known one.
fn odbc_driver(source: crate::config::ModelSource) -> Result<String> {
    let env = odbc_api::environment().context("ODBC")?;
    let installed: Vec<String> = env.drivers().map(|list| list.into_iter().map(|d| d.description).collect()).unwrap_or_default();
    let wanted: &[&str] = match source {
        crate::config::ModelSource::SqlServer => &["ODBC Driver 18 for SQL Server", "ODBC Driver 17 for SQL Server", "ODBC Driver 13 for SQL Server", "SQL Server Native Client 11.0", "SQL Server"],
        crate::config::ModelSource::MySql => &["MySQL ODBC 9", "MySQL ODBC 8", "MariaDB ODBC"],
        _ => &["Oracle in"],
    };
    for w in wanted {
        if let Some(d) = installed.iter().find(|d| d.as_str() == *w).or_else(|| installed.iter().find(|d| d.starts_with(w) && (source != crate::config::ModelSource::MySql || d.contains("Unicode")))).or_else(|| installed.iter().find(|d| d.starts_with(w))) {
            return Ok(d.clone());
        }
    }
    bail!("{}", crate::i18n::fmt("no ODBC driver for {0} is installed on this computer (installed: {1}); install the database vendor's ODBC driver (64-bit) or give the full connection string in db.odbc", &[source.as_str().to_string(), installed.join(", ")]))
}

/// The ODBC connection string of a SQL Server, MySQL or Oracle model: `db.odbc` or built from the settings;
/// the password is always added from the secret.
pub fn odbc_connection_string(cfg: &AppConfig) -> Result<String> {
    use crate::config::{ModelSource, SslMode};
    let password = odbc_value(&cfg.db_password);
    if !cfg.db_odbc.trim().is_empty() {
        let base = cfg.db_odbc.trim().trim_end_matches(';');
        return Ok(if cfg.db_password.is_empty() { format!("{base};") } else { format!("{base};PWD={password};") });
    }
    let driver = odbc_driver(cfg.model_source)?;
    let user = odbc_value(&cfg.db_user);
    Ok(match cfg.model_source {
        ModelSource::SqlServer => {
            let tls = sqlserver_tls(&driver, cfg.db_sslmode);
            // Without a password: Windows authentication (the account Compass or the hub runs under), not a SQL login.
            let login = if cfg.db_password.is_empty() { "Trusted_Connection=Yes;".to_string() } else { format!("UID={user};PWD={password};") };
            // A named instance (SERVER\SQLEXPRESS) is found by the SQL Server Browser service, without a port.
            let server = if cfg.db_host.contains('\\') { cfg.db_host.clone() } else { format!("{},{}", cfg.db_host, cfg.db_port) };
            format!("Driver={{{driver}}};Server={server};Database={};{login}{tls}APP=Loonar Compass;", odbc_value(&cfg.db_name))
        }
        ModelSource::MySql => format!("Driver={{{driver}}};Server={};Port={};Database={};User={user};Password={password};", cfg.db_host, cfg.db_port, odbc_value(&cfg.db_name)),
        _ => format!("Driver={{{driver}}};DBQ={}:{}/{};UID={user};PWD={password};", cfg.db_host, cfg.db_port, cfg.db_name),
    })
}

/// TLS attributes for a SQL Server driver: "Encrypt=optional" exists only from ODBC Driver 18 (Driver 17 and the old
/// "SQL Server" driver know only yes/no and do not encrypt by default).
fn sqlserver_tls(driver: &str, mode: crate::config::SslMode) -> &'static str {
    use crate::config::SslMode;
    let version: u32 = driver.strip_prefix("ODBC Driver ").and_then(|rest| rest.split_whitespace().next()).and_then(|v| v.parse().ok()).unwrap_or(0);
    match mode {
        SslMode::VerifyFull => "Encrypt=yes;TrustServerCertificate=no;",
        SslMode::Require => "Encrypt=yes;TrustServerCertificate=yes;",
        _ if version >= 18 => "Encrypt=optional;",
        _ => "",
    }
}

/// Positions of a word outside string literals and at parenthesis depth 0 (case-insensitive, whole word).
fn top_level_words(sql: &str, word: &str) -> Vec<usize> {
    let lower = sql.to_ascii_lowercase();
    let bytes = lower.as_bytes();
    let (mut depth, mut quoted, mut out) = (0i32, false, Vec::new());
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i];
        if c == b'\'' {
            quoted = !quoted;
        } else if !quoted {
            match c {
                b'(' => depth += 1,
                b')' => depth -= 1,
                _ if depth == 0 && lower[i..].starts_with(word) => {
                    let before = i == 0 || !(bytes[i - 1].is_ascii_alphanumeric() || bytes[i - 1] == b'_');
                    let after = bytes.get(i + word.len()).is_none_or(|b| !(b.is_ascii_alphanumeric() || *b == b'_'));
                    if before && after {
                        out.push(i);
                    }
                }
                _ => {}
            }
        }
        i += 1;
    }
    out
}

/// Replaces a text outside string literals (case-insensitive).
fn replace_outside_literals(sql: &str, from: &str, to: &str) -> String {
    let mut out = String::with_capacity(sql.len());
    let lower = sql.to_ascii_lowercase();
    let from_lower = from.to_ascii_lowercase();
    let mut quoted = false;
    let mut i = 0;
    while i < sql.len() {
        let c = sql.as_bytes()[i];
        if c == b'\'' {
            quoted = !quoted;
        }
        if !quoted && lower[i..].starts_with(&from_lower) {
            out.push_str(to);
            i += from.len();
            continue;
        }
        let ch = sql[i..].chars().next().unwrap();
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

/// The queries of the assistant and of Compass are written in the PostgreSQL / SQLite way; for SQL Server, MySQL and
/// Oracle the common differences are translated: WITH RECURSIVE, a LIMIT at the end, || for joining text, ILIKE,
/// CAST AS VARCHAR on MySQL and a SELECT without FROM on Oracle.
pub fn portable_sql(source: crate::config::ModelSource, sql: &str) -> String {
    use crate::config::ModelSource;
    let mut q = sql.trim().trim_end_matches(';').trim().to_string();
    if !source.is_odbc() {
        return q;
    }
    q = replace_outside_literals(&q, " ilike ", " like ");
    if source != ModelSource::MySql {
        q = replace_outside_literals(&q, "with recursive ", "with ");
    }
    // A LIMIT n at the very end of the query (at depth 0).
    let mut limit: Option<String> = None;
    if let Some(&at) = top_level_words(&q, "limit").last() {
        let number = q[at + 5..].trim();
        if !number.is_empty() && number.chars().all(|c| c.is_ascii_digit()) {
            limit = Some(number.to_string());
            q = q[..at].trim_end().to_string();
        }
    }
    match source {
        ModelSource::SqlServer => {
            q = replace_outside_literals(&q, "||", "+");
            // SQLite's and PostgreSQL's substr is SUBSTRING in T-SQL (MySQL and Oracle know SUBSTR).
            for name in ["substr(", "SUBSTR(", "Substr("] {
                q = replace_outside_literals(&q, name, "substring(");
            }
            if let Some(n) = limit {
                // TOP goes after the last top-level SELECT (after a WITH part), after DISTINCT if there is one.
                if let Some(&at) = top_level_words(&q, "select").last() {
                    let mut insert = at + "select".len();
                    if q[insert..].trim_start().to_ascii_lowercase().starts_with("distinct") {
                        insert = insert + q[insert..].to_ascii_lowercase().find("distinct").unwrap_or(0) + "distinct".len();
                    }
                    q.insert_str(insert, &format!(" top {n}"));
                }
            }
        }
        ModelSource::Oracle => {
            if let Some(n) = limit {
                q.push_str(&format!(" fetch first {n} rows only"));
            }
            if top_level_words(&q, "from").is_empty() {
                q.push_str(" from dual");
            }
        }
        _ => {
            q = replace_outside_literals(&q, " as varchar(", " as char(");
            if let Some(n) = limit {
                q.push_str(&format!(" limit {n}"));
            }
        }
    }
    q
}

/// A read-only query through ODBC: autocommit off and a rollback at the end, so nothing the query might do stays.
fn odbc_select(cfg: &AppConfig, sql: &str, limit: usize) -> Result<Selected> {
    use odbc_api::{buffers::TextRowSet, ConnectionOptions, Cursor, ResultSetMetadata};
    let env = odbc_api::environment().context("ODBC")?;
    let conn = env
        .connect_with_connection_string(&odbc_connection_string(cfg)?, ConnectionOptions { login_timeout_sec: Some(10), ..Default::default() })
        .map_err(|e| anyhow::anyhow!("{}", e.to_string().replace(&cfg.db_password, "***")))
        .context(crate::i18n::tr("connecting to the model database"))?;
    conn.set_autocommit(false)?;
    if cfg.model_source == crate::config::ModelSource::MySql {
        // || joins text and "…" quotes names, as in PostgreSQL.
        let _ = conn.execute("SET SESSION sql_mode = CONCAT(@@sql_mode, ',PIPES_AS_CONCAT,ANSI_QUOTES')", ());
    }
    let sql = portable_sql(cfg.model_source, sql);
    let sql = sql.as_str();
    let mut out = Selected { columns: Vec::new(), rows: Vec::new(), truncated: false };
    let result = (|| -> Result<()> {
        if let Some(mut cursor) = conn.execute(sql, ()).context(crate::i18n::tr("running the query"))? {
            out.columns = cursor.column_names()?.collect::<std::result::Result<Vec<_>, _>>()?;
            let mut buffers = TextRowSet::for_cursor(500, &mut cursor, Some(65_536))?;
            let mut rows = cursor.bind_buffer(&mut buffers)?;
            'fetch: while let Some(batch) = rows.fetch()? {
                for r in 0..batch.num_rows() {
                    if out.rows.len() == limit {
                        out.truncated = true;
                        break 'fetch;
                    }
                    out.rows.push((0..batch.num_cols()).map(|c| batch.at(c, r).map(|b| String::from_utf8_lossy(b).into_owned())).collect());
                }
            }
        }
        Ok(())
    })();
    let _ = conn.rollback();
    result.map(|_| out)
}

/// Connection to the model database, encrypted with TLS according to `db.sslmode`:
/// `prefer` (default: TLS when the server offers it), `require` (TLS or no connection) and `verify-full`
/// (TLS with the server certificate checked against the Windows certificate store, including the host name).
/// `prefer` and `require` do not verify the certificate, exactly like libpq's modes of the same names.
pub fn connect(cfg: &AppConfig) -> Result<postgres::Client> {
    let mut pg = postgres::Config::new();
    pg.host(&cfg.db_host)
        .port(cfg.db_port)
        .dbname(&cfg.db_name)
        .user(&cfg.db_user)
        .password(&cfg.db_password)
        .application_name(crate::config::APP_NAME)
        .connect_timeout(Duration::from_secs(10));
    let verify = cfg.db_sslmode == crate::config::SslMode::VerifyFull;
    pg.ssl_mode(if cfg.db_sslmode == crate::config::SslMode::Prefer { postgres::config::SslMode::Prefer } else { postgres::config::SslMode::Require });
    let tls = native_tls::TlsConnector::builder()
        .danger_accept_invalid_certs(!verify)
        .danger_accept_invalid_hostnames(!verify)
        .build()
        .context(crate::i18n::tr("TLS for the database connection"))?;
    let mut client = pg
        .connect(postgres_native_tls::MakeTlsConnector::new(tls))
        .context(crate::i18n::tr("connecting to the model database"))?;
    // Read-only session (even a faulty query cannot change the data) with a time limit per query.
    client.batch_execute("SET SESSION CHARACTERISTICS AS TRANSACTION READ ONLY; SET statement_timeout = '60s'")?;
    Ok(client)
}

/// Time limit of a query on an EA file (the same as `statement_timeout` on PostgreSQL).
const FILE_QUERY_LIMIT: Duration = Duration::from_secs(60);

/// Opens the SQLite database of an EA project file (`.qea`/`.qeax`) read-only: the file cannot be changed through
/// this connection, and a query that runs longer than the limit is stopped.
pub fn open_model_file(path: &std::path::Path) -> Result<rusqlite::Connection> {
    use rusqlite::OpenFlags;
    if !crate::config::is_model_file(path) {
        bail!("{}", crate::i18n::tr("the model file must be an Enterprise Architect project file .qea or .qeax"));
    }
    if !path.is_file() {
        bail!("{}", crate::i18n::fmt("file {0} does not exist", &[path.display().to_string()]));
    }
    let conn = rusqlite::Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX)
        .context(crate::i18n::tr("opening the model file"))?;
    // EA may be writing to the file at the same moment: wait for its write to finish instead of failing at once.
    conn.busy_timeout(Duration::from_secs(10))?;
    conn.execute_batch("PRAGMA query_only = ON")?;
    // Defensive mode: no writable schema, no shadow-table tricks from a model file someone else made.
    let _ = conn.set_db_config(rusqlite::config::DbConfig::SQLITE_DBCONFIG_DEFENSIVE, true);
    let _ = conn.set_db_config(rusqlite::config::DbConfig::SQLITE_DBCONFIG_TRUSTED_SCHEMA, false);
    let started = std::time::Instant::now();
    conn.progress_handler(10_000, Some(move || started.elapsed() > FILE_QUERY_LIMIT));
    Ok(conn)
}

/// Server functions that read files, wait, or act on other sessions or the server; read-only mode does not stop them,
/// so queries that use them are refused. `load_extension` is the SQLite one (EA file models).
const FORBIDDEN_FUNCTIONS: &[&str] = &[
    "pg_read_file", "pg_read_binary_file", "pg_ls_", "pg_stat_file", "lo_import", "lo_export", "lo_get", "lo_open",
    "dblink", "pg_terminate_backend", "pg_cancel_backend", "pg_sleep", "set_config", "pg_reload_conf",
    "pg_rotate_logfile", "pg_promote", "pg_switch_wal", "pg_create_restore_point", "pg_logical_emit_message",
    "pg_file_", "pg_advisory", "txid_current", "nextval", "setval", "load_extension",
    // Dynamic SQL inside a SELECT (a blocked function called by name in a string).
    "query_to_xml", "cursor_to_xml", "table_to_xml", "schema_to_xml", "database_to_xml", "query_to_json", "pg_execute_server_program", "pg_signal_backend",
    "lo_from_bytea", "lo_put", "lo_create", "lo_unlink", "readfile", "writefile", "edit(", "fts3_tokenizer",
];

/// Words of statements that change data or the session: a query may not contain them outside strings and quoted names
/// (data-modifying WITH, SELECT … INTO, and statements EA's own connection would run when the query goes through EA).
const WRITE_WORDS: &[&str] = &[
    "insert", "update", "delete", "merge", "upsert", "truncate", "alter", "create", "drop", "grant", "revoke", "copy", "call", "do", "execute",
    "prepare", "deallocate", "pragma", "attach", "detach", "vacuum", "reindex", "analyze", "cluster", "refresh", "lock", "listen", "notify",
    "unlisten", "load", "reset", "set", "begin", "commit", "rollback", "savepoint", "release", "into", "comment", "security", "import",
    "discard", "checkpoint", "returning",
];

/// The first word of WRITE_WORDS in the query outside string literals, quoted names and comments; or a Unicode-escaped
/// name (U&"…"), which could hide a blocked function.
pub(crate) fn write_word(sql: &str) -> Option<String> {
    let chars: Vec<char> = sql.chars().collect();
    let mut i = 0;
    let mut word = String::new();
    let check = |word: &mut String| -> Option<String> {
        let w = word.to_ascii_lowercase();
        word.clear();
        WRITE_WORDS.contains(&w.as_str()).then_some(w)
    };
    while i < chars.len() {
        let c = chars[i];
        if c.is_alphanumeric() || c == '_' || c == '$' {
            word.push(c);
            i += 1;
            continue;
        }
        if let Some(w) = check(&mut word) {
            return Some(w);
        }
        match c {
            '&' if i > 0 && matches!(chars[i - 1], 'u' | 'U') && matches!(chars.get(i + 1), Some('"') | Some('\'')) => return Some("U&".into()),
            '\'' | '"' => {
                // A literal or a quoted name up to its closing quote ('' and "" are escaped quotes).
                i += 1;
                while i < chars.len() {
                    if chars[i] == c {
                        if chars.get(i + 1) == Some(&c) {
                            i += 2;
                            continue;
                        }
                        break;
                    }
                    i += 1;
                }
            }
            '-' if chars.get(i + 1) == Some(&'-') => {
                while i < chars.len() && chars[i] != '\n' {
                    i += 1;
                }
            }
            '/' if chars.get(i + 1) == Some(&'*') => {
                i += 2;
                while i + 1 < chars.len() && !(chars[i] == '*' && chars[i + 1] == '/') {
                    i += 1;
                }
                i += 1;
            }
            _ => {}
        }
        i += 1;
    }
    check(&mut word)
}

/// Result of a read-only query: column names and rows (`None` = NULL). `truncated`: there were more rows than the limit.
struct Selected {
    columns: Vec<String>,
    rows: Vec<Vec<Option<String>>>,
    truncated: bool,
}

/// Runs a read-only query on the model (database or file) and returns at most `limit` rows.
fn select(cfg: &AppConfig, sql: &str, limit: usize) -> Result<Selected> {
    let mut out = Selected { columns: Vec::new(), rows: Vec::new(), truncated: false };
    // Compass in a window of EA: a database model is read through EA's own connection (no settings needed).
    if !cfg.uses_file() && crate::ea_ui::reads_through_ea() {
        let (columns, rows, truncated) = crate::ea_ui::sql_query(sql, limit)?;
        return Ok(Selected { columns, rows, truncated });
    }
    // An EA file open in an EA window: EA writes it to disk only when it closes the file, so the current model is
    // read through that window (the file on disk may not have the latest changes yet).
    if cfg.uses_file() && crate::ea_ui::file_open_in_window(cfg) {
        let (columns, rows, truncated) = crate::ea_ui::sql_query(sql, limit)?;
        return Ok(Selected { columns, rows, truncated });
    }
    if cfg.uses_file() {
        let conn = open_model_file(&cfg.model_file)?;
        let mut run = || -> rusqlite::Result<()> {
            let mut stmt = conn.prepare(sql)?;
            out.columns = stmt.column_names().iter().map(|c| c.to_string()).collect();
            let width = stmt.column_count();
            let mut rows = stmt.query([])?;
            while let Some(row) = rows.next()? {
                if out.rows.len() == limit {
                    out.truncated = true;
                    break;
                }
                out.rows.push((0..width).map(|i| row.get_ref(i).map(sqlite_text)).collect::<rusqlite::Result<_>>()?);
            }
            Ok(())
        };
        if let Err(e) = run() {
            if matches!(&e, rusqlite::Error::SqliteFailure(f, _) if f.code == rusqlite::ErrorCode::OperationInterrupted) {
                bail!("{}", crate::i18n::fmt("the query took longer than {0} s and was stopped", &[FILE_QUERY_LIMIT.as_secs().to_string()]));
            }
            return Err(anyhow::Error::new(e).context(crate::i18n::tr("running the query")));
        }
        return Ok(out);
    }
    if cfg.model_source.is_odbc() {
        return odbc_select(cfg, sql, limit);
    }
    let mut client = connect(cfg)?;
    for msg in client.simple_query(sql).context(crate::i18n::tr("running the query"))? {
        if let SimpleQueryMessage::Row(row) = msg {
            if out.columns.is_empty() {
                out.columns = row.columns().iter().map(|c| c.name().to_string()).collect();
            }
            if out.rows.len() == limit {
                out.truncated = true;
                break;
            }
            out.rows.push((0..row.len()).map(|i| row.get(i).map(str::to_string)).collect());
        }
    }
    Ok(out)
}

/// A SQLite value as text (the same way PostgreSQL returns it in a simple query).
fn sqlite_text(value: rusqlite::types::ValueRef) -> Option<String> {
    use rusqlite::types::ValueRef;
    match value {
        ValueRef::Null => None,
        ValueRef::Integer(n) => Some(n.to_string()),
        ValueRef::Real(x) => Some(x.to_string()),
        ValueRef::Text(s) => Some(String::from_utf8_lossy(s).into_owned()),
        ValueRef::Blob(b) => Some(format!("<binary data, {} bytes>", b.len())),
    }
}

/// Read-only query; rows as strings (NULL as an empty string).
pub fn query_rows(cfg: &AppConfig, sql: &str) -> Result<Vec<Vec<String>>> {
    let selected = select(cfg, sql, usize::MAX)?;
    Ok(selected.rows.into_iter().map(|row| row.into_iter().map(Option::unwrap_or_default).collect()).collect())
}

pub fn db_query(sql: &str, cfg: &AppConfig) -> Result<String> {
    let s = sql.trim().trim_end_matches(';').trim();
    let lower = s.to_ascii_lowercase();
    if !(lower.starts_with("select") || lower.starts_with("with")) {
        bail!("{}", crate::i18n::tr("only SELECT or WITH queries are allowed"));
    }
    if s.contains(';') {
        bail!("{}", crate::i18n::tr("one query at a time (no semicolons inside)"));
    }
    if let Some(f) = FORBIDDEN_FUNCTIONS.iter().find(|f| lower.contains(*f)) {
        bail!("{}", crate::i18n::fmt("the query uses a server function that is not allowed ({0}); only reading the model is allowed", &[f.trim_end_matches('_').to_string()]));
    }
    // Quoting the check could read differently from the database (dollar quotes, backslash escapes, comments,
    // [..] and `..` names) is not needed for reading the model: refused.
    if s.contains('$') || s.contains('\\') || s.contains('`') || s.contains('[') || s.contains("--") || s.contains("/*") {
        bail!("{}", crate::i18n::tr("the query uses $, \\, `, [ ] or comments: write it without them (plain SELECT on the model tables)"));
    }
    if let Some(word) = write_word(s) {
        bail!("{}", crate::i18n::fmt("the query contains {0}: only reading the model is allowed (a SELECT without INSERT, UPDATE, DELETE, INTO…)", &[word.to_uppercase()]));
    }

    let selected = select(cfg, s, MAX_ROWS)?;
    if selected.rows.is_empty() {
        return Ok("(no rows)".into());
    }
    let mut out = selected.columns.join(" | ");
    out.push('\n');
    for row in &selected.rows {
        let cols: Vec<&str> = row.iter().map(|v| v.as_deref().unwrap_or("NULL")).collect();
        out.push_str(&cols.join(" | "));
        out.push('\n');
    }
    if selected.truncated {
        out.push_str(&format!("...(truncated to {MAX_ROWS} rows)\n"));
    }
    Ok(out)
}

/// Where the operations (services) of a class, component or interface are used: messages on sequence diagrams addressed
/// to the element or to its lifeline (formal link to the operation or a matching name), actions invoking the operation
/// on activity diagrams; with the diagram, package and sender. At the end, the operations nobody uses.
pub fn operation_usage(input: &Value, cfg: &AppConfig) -> Result<String> {
    let element = input["element"].as_i64().context(crate::i18n::tr("missing field 'element' (object_id of the class)"))?;
    let filter = input["operation"].as_str().unwrap_or("").to_lowercase().replace('\'', "");
    let owner = query_rows(cfg, &format!("select o.name, o.object_type from t_object o where o.object_id = {element}"))?;
    let owner = owner.first().with_context(|| crate::i18n::fmt("there is no element with id {0}", &[element.to_string()]))?;
    let ops = query_rows(cfg, &format!(
        "select operationid, name, ea_guid from t_operation where object_id = {element} order by pos, name"
    ))?;
    let ops: Vec<Vec<String>> = ops.into_iter().filter(|o| filter.is_empty() || o[1].to_lowercase().contains(&filter)).collect();
    if ops.is_empty() {
        return Ok(format!("Element \"{}\" has no operations{}.", owner[0], if filter.is_empty() { String::new() } else { format!(" matching \"{filter}\"") }));
    }
    // Message recipients: the element itself and the lifelines (objects) whose classifier is this element.
    let messages = query_rows(cfg, &format!(
        "select c.connector_id, coalesce(c.name,''), d.name, p.name, s.name, coalesce(c.styleex,'') || coalesce(c.pdata1,'') || coalesce(c.pdata2,'') || coalesce(c.pdata3,'') || coalesce(c.pdata5,'') \
         from t_connector c join t_diagram d on d.diagram_id = c.diagramid join t_package p on p.package_id = d.package_id \
         join t_object s on s.object_id = c.start_object_id \
         where c.connector_type = 'Sequence' and (c.end_object_id = {element} or c.end_object_id in (select object_id from t_object where classifier = {element}))"
    ))?;
    let normalize = |name: &str| name.split('(').next().unwrap_or("").trim().to_lowercase();
    let mut out = format!("Operation usage of element \"{}\" ({}):\n", owner[0], owner[1]);
    let mut unused = Vec::new();
    for op in &ops {
        let (name, guid) = (&op[1], &op[2]);
        let mut hits: Vec<String> = messages
            .iter()
            .filter(|m| m[5].contains(guid.as_str()) || normalize(&m[1]) == normalize(name))
            .map(|m| format!("  - diagram \"{}\" (package \"{}\"): message \"{}\" from \"{}\"{}", m[2], m[3], m[1], m[4], if m[5].contains(guid.as_str()) { " [linked to the operation]" } else { "" }))
            .collect();
        // Actions on activity diagrams that invoke this operation (action classifier = operation).
        let actions = query_rows(cfg, &format!(
            "select a.name, d.name, p.name from t_object a join t_diagramobjects x on x.object_id = a.object_id \
             join t_diagram d on d.diagram_id = x.diagram_id join t_package p on p.package_id = d.package_id \
             where a.classifier_guid = '{}'",
            guid.replace('\'', "")
        ))?;
        hits.extend(actions.iter().map(|a| format!("  - diagram \"{}\" (package \"{}\"): action \"{}\" invokes the operation", a[1], a[2], a[0])));
        hits.sort();
        hits.dedup();
        if hits.is_empty() {
            unused.push(name.clone());
        } else {
            out.push_str(&format!("{name} — used {} time(s):\n{}\n", hits.len(), hits.join("\n")));
        }
    }
    if !unused.is_empty() {
        out.push_str(&format!("Not used on any diagram ({}): {}\n", unused.len(), unused.join(", ")));
    }
    out.push_str("Note: messages without a formal link to the operation are matched by name (without parameters) and recipient.");
    Ok(out)
}

/// Impact analysis: elements reachable from the starting element through relationships (in depth, level by level),
/// with the relationship through which the impact arrives, the path and the diagrams they appear on.
pub fn impact_analysis(input: &Value, cfg: &AppConfig) -> Result<String> {
    let start = input["element"].as_i64().context(crate::i18n::tr("missing field 'element' (object_id)"))?;
    let depth = input["depth"].as_i64().unwrap_or(3).clamp(1, 6);
    let direction = input["direction"].as_str().unwrap_or("both");
    let types: Vec<String> = input["connector_types"]
        .as_array()
        .map(|a| a.iter().filter_map(|v| v.as_str()).map(|s| s.replace('\'', "")).collect())
        .unwrap_or_default();
    let type_filter = if types.is_empty() {
        String::new()
    } else {
        format!(" and c.connector_type in ({})", types.iter().map(|t| format!("'{t}'")).collect::<Vec<_>>().join(","))
    };
    let name_of = |id: i64| -> Result<Vec<String>> {
        query_rows(cfg, &format!(
            "select o.name, o.object_type, coalesce(o.stereotype,''), p.name from t_object o join t_package p on p.package_id = o.package_id where o.object_id = {id}"
        ))?
        .into_iter()
        .next()
        .with_context(|| crate::i18n::fmt("there is no element with id {0}", &[start.to_string()]))
    };
    let root = name_of(start)?;
    // id -> (level, path of names)
    let mut seen: std::collections::HashMap<i64, (i64, String)> = std::collections::HashMap::new();
    seen.insert(start, (0, root[0].clone()));
    let mut frontier = vec![start];
    let mut lines = Vec::new();
    for level in 1..=depth {
        if frontier.is_empty() {
            break;
        }
        let ids = frontier.iter().map(|i| i.to_string()).collect::<Vec<_>>().join(",");
        let mut parts = Vec::new();
        if direction != "in" {
            parts.push(format!(
                "select c.start_object_id, c.end_object_id, c.connector_type, coalesce(c.stereotype,''), coalesce(c.name,''), 'outgoing' \
                 from t_connector c where c.start_object_id in ({ids}){type_filter}"
            ));
        }
        if direction != "out" {
            parts.push(format!(
                "select c.end_object_id, c.start_object_id, c.connector_type, coalesce(c.stereotype,''), coalesce(c.name,''), 'incoming' \
                 from t_connector c where c.end_object_id in ({ids}){type_filter}"
            ));
        }
        let rows = query_rows(cfg, &parts.join(" union all "))?;
        let mut next = Vec::new();
        for r in rows {
            let (from, to): (i64, i64) = (r[0].parse().unwrap_or(0), r[1].parse().unwrap_or(0));
            if to == 0 || seen.contains_key(&to) {
                continue;
            }
            let Ok(info) = name_of(to) else { continue };
            let path = format!("{} → {}", seen.get(&from).map(|s| s.1.clone()).unwrap_or_default(), info[0]);
            let via = if r[3].is_empty() { r[2].clone() } else { format!("{} «{}»", r[2], r[3]) };
            lines.push(format!("{level} | {} | {} | {} | {} | {via} {} | {path}", to, info[0], if info[2].is_empty() { info[1].clone() } else { format!("{} «{}»", info[1], info[2]) }, info[3], r[5]));
            seen.insert(to, (level, path));
            next.push(to);
            if seen.len() > 400 {
                break;
            }
        }
        frontier = next;
    }
    let impacted: Vec<String> = seen.keys().filter(|id| **id != start).map(|i| i.to_string()).collect();
    let mut out = format!(
        "Impact analysis for \"{}\" ({} in package \"{}\"), direction: {direction}, depth: {depth}. Affected elements: {}.\n",
        root[0], root[1], root[3], impacted.len()
    );
    if impacted.is_empty() {
        out.push_str("No related elements.");
        return Ok(out);
    }
    out.push_str("level | object_id | name | type | package | relationship | path\n");
    out.push_str(&lines.join("\n"));
    let diagrams = query_rows(cfg, &format!(
        "select d.name, count(distinct o.object_id) from t_diagramobjects o join t_diagram d on d.diagram_id = o.diagram_id \
         where o.object_id in ({}) group by d.name order by 2 desc limit 30",
        impacted.join(",")
    ))?;
    if !diagrams.is_empty() {
        out.push_str("\n\nDiagrams with affected elements (name | number of elements):\n");
        out.push_str(&diagrams.iter().map(|r| format!("{} | {}", r[0], r[1])).collect::<Vec<_>>().join("\n"));
    }
    if seen.len() > 400 {
        out.push_str("\n\nResult truncated to 400 elements: narrow down the direction, the depth or the relationship types.");
    }
    Ok(out)
}

/// Definitions of the read-only tools (for the assistant of the desktop and the read-only assistant of the hub).
pub fn read_only_definitions() -> Vec<Value> {
    let list = json!([
        {
            "name": "db_query",
            "description": "Read-only access (SELECT/WITH only) to the EA model: a PostgreSQL database or an EA project file (.qea/.qeax, a SQLite database); the system prompt says which, and you write the query in that SQL dialect. Table and column names are lowercase. \
Tables: t_package (package_id, name, parent_id, ea_guid, modifieddate), t_object (object_id, name, alias, object_type, stereotype, package_id, parentid, note, pdata1, ea_guid, modifieddate), \
t_diagram (diagram_id, name, diagram_type, package_id, parentid, ea_guid, modifieddate), t_diagramobjects (diagram_id, object_id, rectleft, recttop, rectright, rectbottom; the Y axis is inverted in the database: recttop and rectbottom are negative, so an element placed with place at top=50 and height=115 has recttop=-50 and rectbottom=-165, and that is the correct position on the diagram), \
t_connector (connector_id, name, connector_type, stereotype, start_object_id, end_object_id), t_objectproperties (object_id, property, value), t_operation. \
Result: at most 200 rows.",
            "input_schema": {
                "type": "object",
                "properties": { "sql": { "type": "string", "description": "A single SELECT or WITH query." } },
                "required": ["sql"]
            }
        },
        {
            "name": "operation_usage",
            "description": "Where the operations (services) of a class, component or interface are used: on which diagrams (sequence: messages to the element \
or its lifeline; activity: actions that invoke the operation), in which packages and who calls them; at the end, a list of unused operations. \
Use it when the user asks \"where is this service/operation/method used\", \"who calls it\", \"is this used anywhere\". \
operation optionally narrows the result down to operations whose name contains this text.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "element": { "type": "integer", "description": "object_id of the class, component or interface (from db_query)." },
                    "operation": { "type": "string", "description": "Optional fragment of the operation name." }
                },
                "required": ["element"]
            }
        },
        {
            "name": "model_lint",
            "description": "Model linter (governance) without AI guessing: deterministic checks of a package with all its subpackages. \
Finds default or empty names (Class1, Activity 2), elements without a description, orphaned elements (no relationships and on no diagram), \
duplicate names, relationships to non-existent elements, duplicate relationships and self-relationships, empty packages and diagrams, \
and the organization's rules from governance.json (or given in 'rules'): naming conventions, required tagged values and allowed relationships (MDG / metamodel). \
Each finding has a severity (error, warning, info), the rule, the object id and the problem. Present the findings grouped by severity with concrete fix proposals; \
fix them only after the user agrees (ea_write). On request turn the result into an Excel workbook or a document. \
The linter is a deterministic baseline: when the user asks for a review, also judge the model yourself (e.g. db_query for meaning, inheritance that makes no sense such as an invoice specialising a product, flows that skip steps) and report both, marking which findings come from the linter.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "package": { "type": "integer", "description": "package_id of the checked package (subpackages are included)." },
                    "skip": { "type": "array", "items": { "type": "string" }, "description": "Rules to leave out, e.g. [\"missing_notes\"]. Rules: default_name, missing_notes, orphan, duplicate_name, naming, required_tag, broken_connector, self_relation, duplicate_relation, invalid_relation, empty_package, empty_diagram." },
                    "rules": { "type": "object", "description": "Optionally rules instead of governance.json: {\"naming\": [{\"applies_to\": \"Class\" or \"«stereotype»\", \"pattern\": \"*Service\" (wildcards * ?), \"message\": \"…\"}], \"required_tags\": [{\"applies_to\": \"…\", \"tag\": \"Owner\"}], \"allowed_relations\": [{\"connector_type\": \"Realisation\", \"source\": \"Component\", \"target\": \"Interface\"}]} (a relationship type listed there is allowed only between the listed ends)." }
                },
                "required": ["package"]
            }
        },
        {
            "name": "gap_analysis",
            "description": "Gap analysis between two packages (with subpackages), e.g. as-is vs to-be architecture: matches elements by name and type and returns \
what is new (only in to-be), what is to retire (only in as-is) and what changed (stereotype, description, relationships added or removed). \
Present it as a gap table; on request save it as an Excel workbook (create_spreadsheet) or a document.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "as_is": { "type": "integer", "description": "package_id of the current state (as-is)." },
                    "to_be": { "type": "integer", "description": "package_id of the target state (to-be)." }
                },
                "required": ["as_is", "to_be"]
            }
        },
        {
            "name": "impact_analysis",
            "description": "Change impact analysis: starting from the given element, follows relationships in depth (level by level) and returns all affected elements \
with the level, type, package, the relationship through which the impact arrives and the path, plus the diagrams they appear on. \
direction: out (what the element depends on / affects through outgoing relationships), in (what depends on the element), both (default). \
Describe the result to the user clearly (what is affected and why), and on request turn it into an image, a document or an Excel workbook.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "element": { "type": "integer", "description": "object_id of the starting element (from db_query)." },
                    "direction": { "type": "string", "enum": ["in", "out", "both"] },
                    "depth": { "type": "integer", "description": "Number of levels, 1-6 (default 3)." },
                    "connector_types": { "type": "array", "items": { "type": "string" }, "description": "Optionally only these relationship types, e.g. [\"Realisation\", \"Dependency\"]." }
                },
                "required": ["element"]
            }
        }
    ]);
    list.as_array().cloned().unwrap_or_default()
}

#[cfg(test)]
mod odbc_tests {
    use serde_json::json;

    /// A write through EA into a SQL Server repository (EA opens it with the OLE DB connection Compass builds):
    /// `LOONAR_TEST_MSSQL_RW="127.0.0.1;14330;ea_lab;sa;<password>" cargo test sqlserver_write_through_ea -- --ignored --nocapture`
    #[test]
    #[ignore = "requires EA, a licence and a SQL Server EA repository"]
    fn sqlserver_write_through_ea() {
        crate::i18n::set(crate::i18n::Lang::En);
        let spec = std::env::var("LOONAR_TEST_MSSQL_RW").unwrap();
        let parts: Vec<&str> = spec.split(';').collect();
        let props = std::env::temp_dir().join(format!("loonar-mssql-rw-{}.properties", std::process::id()));
        std::fs::write(&props, format!("model.source=sqlserver\ndb.host={}\ndb.port={}\ndb.name={}\ndb.user={}\napproval.mode=auto\n", parts[0], parts[1], parts[2], parts[3])).unwrap();
        let mut cfg = crate::config::AppConfig::load(&props).unwrap();
        cfg.db_password = parts[4].to_string();
        let package = super::query_rows(&cfg, "select package_id from t_package where name = 'Shop domain'").unwrap()[0][0].parse::<i64>().unwrap();
        let name = format!("MssqlProbe{}", std::process::id());
        let approver = crate::tools::Approver::new(crate::config::ApprovalMode::Auto, Box::new(|_| true));
        let ctx = crate::tools::ToolContext { cfg: &cfg, approver: &approver };
        let result = crate::tools::run("ea_write", &json!({ "ops": [{ "op": "create_element", "key": "c", "package": package, "name": name, "type": "Class" }] }), &ctx);
        println!("{:?}", result.as_ref().map(|r| r.text.lines().take(3).collect::<Vec<_>>().join(" | ")));
        result.unwrap();
        let found = super::query_rows(&cfg, &format!("select count(*) from t_object where name = '{name}'")).unwrap();
        assert_eq!(found[0][0], "1", "the class is in SQL Server");
    }

    #[test]
    fn sql_is_made_portable() {
        use crate::config::ModelSource::*;
        assert_eq!(super::portable_sql(SqlServer, "select name from t_object where name ilike 'a%' limit 5;"), "select top 5 name from t_object where name like 'a%'");
        assert_eq!(super::portable_sql(SqlServer, "with recursive tree(id) as (select 1 limit 1) select distinct id from tree limit 10"), "with tree(id) as (select 1 limit 1) select distinct top 10 id from tree");
        assert_eq!(super::portable_sql(SqlServer, "select a || 'x || y' || b from t"), "select a + 'x || y' + b from t");
        assert_eq!(super::portable_sql(SqlServer, "select substr(note, 1, 120), 'substr(' from t_object"), "select substring(note, 1, 120), 'substr(' from t_object");
        assert_eq!(super::portable_sql(SqlServer, "select substring(note, 1, 5) from t"), "select substring(note, 1, 5) from t");
        use crate::config::SslMode;
        assert_eq!(super::sqlserver_tls("ODBC Driver 17 for SQL Server", SslMode::Prefer), "", "Driver 17 does not know Encrypt=optional");
        assert_eq!(super::sqlserver_tls("ODBC Driver 18 for SQL Server", SslMode::Prefer), "Encrypt=optional;");
        assert_eq!(super::sqlserver_tls("SQL Server", SslMode::Require), "Encrypt=yes;TrustServerCertificate=yes;");
        assert_eq!(super::portable_sql(Oracle, "select count(*) limit 3"), "select count(*) fetch first 3 rows only from dual");
        assert_eq!(super::portable_sql(MySql, "select cast(x as varchar(10)) from t limit 2"), "select cast(x as char(10)) from t limit 2");
        assert_eq!(super::portable_sql(Postgres, "select 1 limit 2;"), "select 1 limit 2");
    }

    /// The read tools on a SQL Server repository (the test model copied into a SQL Server container):
    /// `LOONAR_TEST_MSSQL="127.0.0.1;14330;ea_lab;compass;<password>" cargo test sqlserver_repository -- --ignored --nocapture`
    #[test]
    #[ignore = "requires a SQL Server with an EA repository"]
    fn sqlserver_repository() {
        crate::i18n::set(crate::i18n::Lang::En);
        let spec = std::env::var("LOONAR_TEST_MSSQL").unwrap();
        let parts: Vec<&str> = spec.split(';').collect();
        let props = std::env::temp_dir().join(format!("loonar-mssql-{}.properties", std::process::id()));
        std::fs::write(&props, format!("model.source=sqlserver\ndb.host={}\ndb.port={}\ndb.name={}\ndb.user={}\n", parts[0], parts[1], parts[2], parts[3])).unwrap();
        let mut cfg = crate::config::AppConfig::load(&props).unwrap();
        cfg.db_password = parts[4].to_string();
        println!("connection: {}", super::odbc_connection_string(&cfg).unwrap().replace(parts[4], "***"));
        let rows = super::query_rows(&cfg, "select count(*) from t_object").unwrap();
        assert_eq!(rows[0][0], "29");
        println!("{}", super::db_query("SELECT TOP 3 name, object_type FROM t_object ORDER BY name", &cfg).unwrap());
        let mut failures = Vec::new();
        let package = super::query_rows(&cfg, "select package_id from t_package where name = 'Shop domain'").unwrap()[0][0].parse::<i64>().unwrap();
        let element = super::query_rows(&cfg, "select object_id from t_object where name = 'Order'").unwrap()[0][0].parse::<i64>().unwrap();
        let checks: Vec<(&str, anyhow::Result<String>)> = vec![
            ("model_lint", crate::governance::lint(&json!({ "package": package }), &cfg)),
            ("impact_analysis", super::impact_analysis(&json!({ "element": element }), &cfg)),
            ("operation_usage", super::operation_usage(&json!({ "element": element }), &cfg)),
            ("estimate", crate::estimate::estimate(&cfg, &[element], 2)),
            ("mdg", crate::mdg::run(&json!({}), &cfg)),
            ("version_control_status", crate::mdg::version_control_status(&json!({}), &cfg)),
            ("agents describe", crate::agents::describe(&cfg, "element_deleting", element).map(|d| d.0).ok_or_else(|| anyhow::anyhow!("no description"))),
        ];
        for (name, result) in checks {
            match result {
                Ok(text) => println!("OK {name}: {}", text.lines().next().unwrap_or("")),
                Err(e) => {
                    println!("FAILED {name}: {e:#}");
                    failures.push(name);
                }
            }
        }
        assert!(failures.is_empty(), "failed on SQL Server: {failures:?}");
    }
}


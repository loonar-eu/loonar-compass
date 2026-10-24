#![allow(dead_code)]
//! Compass Hub as its own program: `compass-hub hub.json` (or `--hub hub.json`) on Windows or Linux, in a container,
//! or as a Windows service. Secrets (AI keys, Jira and Confluence tokens, database passwords) come from the environment:
//! LOONAR_SECRET_<NAME> or the file LOONAR_SECRET_<NAME>_FILE (on Windows also from Credential Manager).

#[path = "../../src/agent_ro.rs"]
mod agent_ro;
#[path = "../../src/claude.rs"]
mod claude;
#[path = "../../src/config.rs"]
mod config;
#[path = "../../src/confluence.rs"]
mod confluence;
#[path = "../../src/estimate.rs"]
mod estimate;
#[path = "../../src/governance.rs"]
mod governance;
#[path = "../../src/hub.rs"]
mod hub;
#[path = "../../src/i18n.rs"]
mod i18n;
#[path = "../../src/jira.rs"]
mod jira;
#[path = "../../src/model_db.rs"]
mod model_db;
#[path = "../../src/net.rs"]
mod net;
#[path = "../../src/auth.rs"]
mod auth;
#[path = "../../src/graphql.rs"]
mod graphql;
#[path = "../../src/docx.rs"]
mod docx;
#[path = "../../src/xlsx.rs"]
mod xlsx;
#[path = "../../src/oidc.rs"]
mod oidc;
#[path = "../../src/openai.rs"]
mod openai;
#[path = "../../src/ops.rs"]
mod ops;

/// The hub reads the models directly: there is no EA window to read through.
mod ea_ui {
    pub fn reads_through_ea() -> bool {
        false
    }

    pub fn file_open_in_window(_: &crate::config::AppConfig) -> bool {
        false
    }

    pub fn sql_query(_: &str, _: usize) -> anyhow::Result<(Vec<String>, Vec<Vec<Option<String>>>, bool)> {
        anyhow::bail!("no EA window")
    }
}

/// The names the shared modules use in the desktop application.
mod tools {
    pub use crate::model_db::{impact_analysis, query_rows};
    pub use crate::ops::{check_file_path, normalize_ops};
}

mod ea {
    pub use crate::ops::{describe, MODEL_OPS};
}

#[cfg(windows)]
mod service;

const USAGE: &str = "compass-hub <hub.json>            run the hub (also: --hub <hub.json>)
compass-hub --hub-token            print a new token and its SHA-256 for hub.json
compass-hub --version
Windows service (as administrator):
compass-hub --service-install <hub.json>
compass-hub --service-uninstall
Secrets: LOONAR_SECRET_<NAME> or LOONAR_SECRET_<NAME>_FILE, e.g. LOONAR_SECRET_CLAUDE_API_KEY, LOONAR_SECRET_JIRA_TOKEN.";

fn main() {
    config::secrets::use_environment();
    let args: Vec<String> = std::env::args().collect();
    let arg = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned();
    if args.iter().any(|a| a == "--version") {
        println!("Compass Hub {}", env!("CARGO_PKG_VERSION"));
        return;
    }
    if args.iter().any(|a| a == "--hub-token") {
        let (token, hash) = hub::new_token();
        println!("token (give it to the user, keep it secret): {token}\n\"token_sha256\" for hub.json: {hash}");
        return;
    }
    #[cfg(windows)]
    {
        if let Some(file) = arg("--service-install") {
            std::process::exit(service::install(&file).map(|_| 0).unwrap_or_else(|e| {
                eprintln!("{e:#}");
                1
            }));
        }
        if args.iter().any(|a| a == "--service-uninstall") {
            std::process::exit(service::uninstall().map(|_| 0).unwrap_or_else(|e| {
                eprintln!("{e:#}");
                1
            }));
        }
        if let Some(file) = arg("--service") {
            service::run(file);
            return;
        }
    }
    let file = arg("--hub").or_else(|| args.get(1).filter(|a| !a.starts_with('-')).cloned());
    let Some(file) = file else {
        eprintln!("{USAGE}");
        std::process::exit(2);
    };
    if let Err(e) = run(&file) {
        eprintln!("Compass Hub: {e:#}");
        std::process::exit(1);
    }
}

/// Loads hub.json and serves until the process ends.
pub fn run(file: &str) -> anyhow::Result<()> {
    let (hub, listen) = hub::load(std::path::Path::new(file))?;
    hub::serve(hub, &listen, |address| eprintln!("Compass Hub {} listening on {address}", env!("CARGO_PKG_VERSION")))
}

//! Check mode for CI pipelines: `LoonarCompass.exe --check …` runs the model check (built-in, ArchiMate and team
//! rules), the model ↔ code comparison and the requirements chain without the window and without the AI, prints a
//! Markdown report, optionally writes it and a JUnit file, and ends with an exit code: 0 passed, 1 findings at or above
//! `--fail-on`, 2 wrong arguments or a model that cannot be read.
//!
//! Severities: rule findings as the model check rates them (error / warning / info); a requirement not in the model and
//! a model ↔ code difference are warnings; a requirement without code or tests is info.

use crate::config::AppConfig;
use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

const USAGE: &str = "LoonarCompass.exe --check [--model <file.qea|file.qeax|app.properties>] [--package <id or name>] [--rules <governance.json>] \
[--code <source folder>] [--requirements <file.docx|.md|.txt|.html>] [--report <report.md>] [--junit <junit.xml>] [--fail-on error|warning|info|never] [--lang en|pl|de|fr]";

#[derive(Default, Debug)]
pub struct Options {
    pub model: Option<PathBuf>,
    pub package: Option<String>,
    pub rules: Option<PathBuf>,
    pub code: Option<PathBuf>,
    pub requirements: Option<PathBuf>,
    pub report: Option<PathBuf>,
    pub junit: Option<PathBuf>,
    /// 0 error, 1 warning, 2 info, 3 never.
    pub fail_on: u8,
    pub lang: String,
}

fn rank(severity: &str) -> u8 {
    match severity {
        "error" => 0,
        "warning" => 1,
        _ => 2,
    }
}

pub fn parse(args: &[String]) -> Result<Options> {
    let mut o = Options { fail_on: 0, lang: "en".into(), ..Default::default() };
    let start = args.iter().position(|a| a == "--check").map(|i| i + 1).unwrap_or(args.len());
    let mut i = start;
    while i < args.len() {
        let key = args[i].as_str();
        let value = args.get(i + 1).cloned().with_context(|| format!("{key}: missing value\n{USAGE}"))?;
        match key {
            "--model" => o.model = Some(value.into()),
            "--package" => o.package = Some(value),
            "--rules" => o.rules = Some(value.into()),
            "--code" => o.code = Some(value.into()),
            "--requirements" => o.requirements = Some(value.into()),
            "--report" => o.report = Some(value.into()),
            "--junit" => o.junit = Some(value.into()),
            "--fail-on" => {
                o.fail_on = match value.as_str() {
                    "error" => 0,
                    "warning" => 1,
                    "info" => 2,
                    "never" => 3,
                    other => bail!("--fail-on {other}: use error, warning, info or never\n{USAGE}"),
                }
            }
            "--lang" => o.lang = value,
            other => bail!("unknown argument {other}\n{USAGE}"),
        }
        i += 2;
    }
    Ok(o)
}

/// The model to check: an EA file directly (read only, no EA needed) or the settings of a Compass installation.
fn model_config(o: &Options, default_props: &Path) -> Result<AppConfig> {
    let path = o.model.clone().unwrap_or_else(|| default_props.to_path_buf());
    let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    let mut cfg = if matches!(ext.as_str(), "qea" | "qeax") {
        anyhow::ensure!(path.is_file(), "model file {} does not exist", path.display());
        let dir = std::env::temp_dir().join(format!("loonar-check-{}", std::process::id()));
        std::fs::create_dir_all(&dir)?;
        let props = dir.join("app.properties");
        let file = std::fs::canonicalize(&path).unwrap_or(path.clone());
        let file = file.display().to_string().trim_start_matches(r"\\?\").replace('\\', "/");
        std::fs::write(&props, format!("model.source=file\nmodel.file={file}\nui.language={}\n", o.lang))?;
        let cfg = AppConfig::load(&props);
        let _ = std::fs::remove_dir_all(&dir);
        cfg?
    } else {
        AppConfig::load(&path)?
    };
    // CI secrets come from the environment, not from Credential Manager.
    if let Ok(password) = std::env::var("LOONAR_DB_PASSWORD") {
        cfg.db_password = password;
    }
    Ok(cfg)
}

/// The packages to check: the given one (id or name) or the root packages of the model.
fn packages(cfg: &AppConfig, given: Option<&str>) -> Result<Vec<(i64, String)>> {
    let rows = match given.map(str::trim).filter(|g| !g.is_empty()) {
        Some(g) if g.chars().all(|c| c.is_ascii_digit()) => crate::tools::query_rows(cfg, &format!("select package_id as c0, name as c1 from t_package where package_id = {g}"))?,
        Some(g) => crate::tools::query_rows(cfg, &format!("select package_id as c0, name as c1 from t_package where name = '{}' order by package_id", g.replace('\'', "''")))?,
        None => crate::tools::query_rows(cfg, "select package_id as c0, name as c1 from t_package where parent_id = 0 order by package_id")?,
    };
    if rows.is_empty() {
        bail!("package {} not found in the model", given.unwrap_or("(root)"));
    }
    Ok(rows.into_iter().take(if given.is_some() { 1 } else { usize::MAX }).filter_map(|r| r[0].parse().ok().map(|id| (id, r[1].clone()))).collect())
}

/// One reported item: section, title, severity and lines for the report and JUnit.
struct Item {
    suite: &'static str,
    name: String,
    severity: &'static str,
    lines: Vec<String>,
}

fn xml(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn junit(items: &[Item], fail_on: u8, suites: &[&str]) -> String {
    let mut out = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<testsuites name=\"Loonar Compass check\">\n");
    for suite in suites {
        let list: Vec<&Item> = items.iter().filter(|i| i.suite == *suite).collect();
        let failures = list.iter().filter(|i| rank(i.severity) <= fail_on.min(2) && fail_on < 3).count();
        out.push_str(&format!("  <testsuite name=\"{}\" tests=\"{}\" failures=\"{}\">\n", xml(suite), list.len().max(1), failures));
        if list.is_empty() {
            out.push_str(&format!("    <testcase classname=\"{}\" name=\"no findings\"/>\n", xml(suite)));
        }
        for i in list {
            out.push_str(&format!("    <testcase classname=\"{}\" name=\"{}\">", xml(suite), xml(&i.name)));
            if rank(i.severity) <= fail_on && fail_on < 3 {
                out.push_str(&format!("<failure message=\"{} ({})\">{}</failure>", xml(&i.name), i.severity, xml(&i.lines.join("\n"))));
            } else {
                out.push_str(&format!("<system-out>{}</system-out>", xml(&i.lines.join("\n"))));
            }
            out.push_str("</testcase>\n");
        }
        out.push_str("  </testsuite>\n");
    }
    out.push_str("</testsuites>\n");
    out
}

/// Runs the check; returns the report, the JUnit text and whether it failed.
pub fn check(o: &Options, default_props: &Path) -> Result<(String, String, bool)> {
    let cfg = model_config(o, default_props)?;
    let rules: Value = match &o.rules {
        Some(p) => serde_json::from_str(&std::fs::read_to_string(p).with_context(|| format!("cannot read {}", p.display()))?).with_context(|| format!("{} is not valid JSON", p.display()))?,
        None => crate::governance::load_rules(&cfg, &Value::Null),
    };
    let mut items: Vec<Item> = Vec::new();
    let mut md = vec![format!("# Loonar Compass check – {}", chrono::Local::now().format("%Y-%m-%d %H:%M"))];
    let model_name = o.model.as_ref().map(|p| p.display().to_string()).unwrap_or_else(|| "settings of this computer".into());
    md.push(format!("Model: {model_name}"));

    // 1. Model check of the package (or of every root package).
    let mut all = Vec::new();
    let mut checked = (0usize, 0usize, 0usize);
    for (id, name) in packages(&cfg, o.package.as_deref())? {
        let r = crate::governance::lint_result(&json!({ "package": id, "rules": rules }), &cfg)?;
        checked.0 += r.packages;
        checked.1 += r.elements;
        checked.2 += r.connectors;
        md.push(format!("Package: {name} (#{id})"));
        all.extend(r.findings);
    }
    let mut rules_seen: Vec<&'static str> = Vec::new();
    for f in &all {
        if !rules_seen.contains(&f.rule) {
            rules_seen.push(f.rule);
        }
    }
    rules_seen.sort_by_key(|r| (rank(crate::governance::severity(r)), *r));
    md.push(format!("\n## {}\n", crate::i18n::tr("Model check")));
    md.push(crate::i18n::fmt("{0} packages, {1} elements, {2} relationships checked; findings: {3}.", &[checked.0.to_string(), checked.1.to_string(), checked.2.to_string(), all.len().to_string()]));
    for rule in &rules_seen {
        let list: Vec<&crate::governance::Finding> = all.iter().filter(|f| f.rule == *rule).collect();
        let lines: Vec<String> = list.iter().map(|f| format!("{} #{} {} ({}): {}", f.kind, f.id, f.name, f.package, f.detail)).collect();
        md.push(format!("\n### {} [{}] — {}\n", crate::governance::rule_title(rule), crate::governance::severity(rule), list.len()));
        md.extend(lines.iter().take(200).map(|l| format!("- {l}")));
        items.push(Item { suite: "Model check", name: format!("{} ({})", crate::governance::rule_title(rule), list.len()), severity: crate::governance::severity(rule), lines });
    }

    // 2. Model ↔ code.
    if let Some(code) = &o.code {
        let package = o.package.as_ref().and_then(|_| packages(&cfg, o.package.as_deref()).ok()).and_then(|p| p.first().map(|p| p.0));
        let d = crate::code_drift::drift(&cfg, code, package)?;
        md.push(format!("\n## {}\n", crate::i18n::tr("Model and code")));
        md.push(crate::i18n::fmt("{0} source files, {1} types, {2} tables, {3} API operations; differences: {4}.", &[d.files.to_string(), d.types.to_string(), d.tables.to_string(), d.operations.to_string(), d.count().to_string()]));
        for (title, list) in d.sections() {
            if list.is_empty() {
                continue;
            }
            md.push(format!("\n### {title} — {}\n", list.len()));
            md.extend(list.iter().take(200).map(|l| format!("- {l}")));
            items.push(Item { suite: "Model and code", name: format!("{title} ({})", list.len()), severity: "warning", lines: list.clone() });
        }
    }

    // 3. Requirements: coverage in the model and, with a source folder, the chain to code, tests and issues.
    if let Some(doc) = &o.requirements {
        let text = crate::traceability::read_document(doc)?;
        let reqs = crate::traceability::extract(&text);
        let t = crate::traceability::trace_data(&cfg, &reqs, None)?;
        let chain = match &o.code {
            Some(code) => Some(crate::traceability::chain(&t, &reqs, code)?),
            None => None,
        };
        md.push(format!("\n## {}\n", crate::i18n::tr("Requirements")));
        md.push(format!("| {} | {} | {} | {} |", crate::i18n::tr("requirement"), crate::i18n::tr("model"), crate::i18n::tr("chain"), crate::i18n::tr("details")));
        md.push("|---|---|---|---|".into());
        for (n, r) in t.rows.iter().enumerate() {
            let link = chain.as_ref().and_then(|c| c.get(n));
            let model = r.traced.iter().map(|(id, name, _, _)| format!("{name} #{id}")).collect::<Vec<_>>().join(", ");
            let (severity, chain_text, details) = match (r.coverage, link) {
                (crate::traceability::Coverage::Missing | crate::traceability::Coverage::Candidates, _) => ("warning", crate::i18n::tr("BROKEN: not in the model").to_string(), r.candidates.join(", ")),
                (crate::traceability::Coverage::ContentDiffers, _) => ("warning", crate::i18n::tr("ID MATCHES, CONTENT DIFFERS").to_string(), model.clone()),
                (_, Some(c)) => (
                    if c.status == crate::traceability::ChainStatus::Complete { "ok" } else { "info" },
                    crate::traceability::chain_label(c.status).to_string(),
                    format!("{} | {} | {}", c.code.join(", "), c.tests.join(", "), c.issues.join(", ")),
                ),
                (_, None) => ("ok", crate::i18n::tr("covered").to_string(), String::new()),
            };
            md.push(format!("| {} | {} | {} | {} |", r.id, model.replace('|', "/"), chain_text, details.replace('|', "/")));
            if severity != "ok" {
                items.push(Item { suite: "Requirements", name: format!("{} {}", r.id, r.text.chars().take(80).collect::<String>()), severity, lines: vec![chain_text, model, details] });
            }
        }
        if let Some(c) = &chain {
            md.push(String::new());
            md.push(crate::traceability::chain_report(c).lines().next().unwrap_or("").to_string());
        }
    }

    let failed = o.fail_on < 3 && items.iter().any(|i| rank(i.severity) <= o.fail_on);
    let errors = items.iter().filter(|i| i.severity == "error").count();
    let warnings = items.iter().filter(|i| i.severity == "warning").count();
    md.insert(2, crate::i18n::fmt("Result: {0} – {1} error groups, {2} warning groups (fail on: {3}).", &[
        if failed { "FAILED".to_string() } else { "PASSED".to_string() }, errors.to_string(), warnings.to_string(),
        ["error", "warning", "info", "never"][o.fail_on as usize].to_string(),
    ]));
    Ok((md.join("\n") + "\n", junit(&items, o.fail_on, &["Model check", "Model and code", "Requirements"]), failed))
}

#[link(name = "kernel32")]
extern "system" {
    fn AttachConsole(process: u32) -> i32;
    fn GetStdHandle(kind: u32) -> isize;
}

/// The application is a window program: its output reaches a console only after attaching to the parent's one. When
/// the output is redirected (a CI runner, `| Out-Host`), the inherited handles are used as they are.
fn attach_console() {
    const STD_OUTPUT_HANDLE: u32 = -11i32 as u32;
    const ATTACH_PARENT_PROCESS: u32 = u32::MAX;
    unsafe {
        let handle = GetStdHandle(STD_OUTPUT_HANDLE);
        if handle == 0 || handle == -1 {
            AttachConsole(ATTACH_PARENT_PROCESS);
        }
    }
}

/// `--check` from the command line: prints the report and returns the exit code.
pub fn run(args: &[String], default_props: &Path) -> i32 {
    attach_console();
    let o = match parse(args) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("{e:#}");
            return 2;
        }
    };
    crate::i18n::set(crate::i18n::Lang::parse(&o.lang));
    match check(&o, default_props) {
        Ok((report, junit_text, failed)) => {
            println!("{report}");
            if let Some(p) = &o.report {
                if let Err(e) = std::fs::write(p, &report) {
                    eprintln!("cannot write {}: {e}", p.display());
                    return 2;
                }
            }
            if let Some(p) = &o.junit {
                if let Err(e) = std::fs::write(p, &junit_text) {
                    eprintln!("cannot write {}: {e}", p.display());
                    return 2;
                }
            }
            i32::from(failed)
        }
        Err(e) => {
            eprintln!("Loonar Compass check: {e:#}");
            2
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arguments() {
        let args: Vec<String> = ["x.exe", "--check", "--model", "m.qea", "--fail-on", "warning", "--code", "src", "--lang", "pl"].iter().map(|s| s.to_string()).collect();
        let o = parse(&args).unwrap();
        assert_eq!(o.model, Some(PathBuf::from("m.qea")));
        assert_eq!((o.fail_on, o.lang.as_str(), o.code.is_some()), (1, "pl", true));
        assert!(parse(&["--check".to_string(), "--fail-on".to_string(), "sometimes".to_string()]).is_err());
        assert!(parse(&["--check".to_string(), "--bogus".to_string(), "1".to_string()]).is_err());
        assert!(parse(&["--check".to_string(), "--model".to_string()]).is_err());
    }

    #[test]
    fn junit_output() {
        let items = vec![
            Item { suite: "Model check", name: "Orphans (2)".into(), severity: "warning", lines: vec!["a <b>".into()] },
            Item { suite: "Model check", name: "Broken (1)".into(), severity: "error", lines: vec!["c".into()] },
        ];
        let x = junit(&items, 0, &["Model check", "Requirements"]);
        assert!(x.contains("tests=\"2\" failures=\"1\"") && x.contains("<failure message=\"Broken (1) (error)\">c</failure>") && x.contains("a &lt;b&gt;"), "{x}");
        assert!(x.contains("<testsuite name=\"Requirements\" tests=\"1\" failures=\"0\">"));
        assert!(junit(&items, 3, &["Model check"]).contains("failures=\"0\""));
    }
}

#[cfg(test)]
mod model_tests {
    /// The check against a model file (LOONAR_TEST_QEA) with a small source folder. `cargo test check_against_model_file -- --ignored --nocapture`
    #[test]
    #[ignore = "requires a test model file"]
    fn check_against_model_file() {
        let qea = std::env::var("LOONAR_TEST_QEA").unwrap();
        let dir = std::env::temp_dir().join(format!("loonar-check-src-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("src/test")).unwrap();
        std::fs::write(dir.join("Customer.java"), "public class Customer {\n  private String name;\n}\n").unwrap();
        std::fs::write(dir.join("src/test/CustomerTest.java"), "class CustomerTest { Customer c; }\n").unwrap();
        std::fs::write(dir.join("req.md"), "W1 Klient zakłada konto Customer w sklepie internetowym.\nW2 Program lojalnościowy dla stałych klientów VIP.\n").unwrap();
        let args: Vec<String> = ["--check", "--model", &qea, "--code", dir.to_str().unwrap(), "--requirements", dir.join("req.md").to_str().unwrap(), "--fail-on", "warning"].iter().map(|s| s.to_string()).collect();
        let o = super::parse(&args).unwrap();
        crate::i18n::set(crate::i18n::Lang::En);
        let (report, junit, failed) = super::check(&o, std::path::Path::new("none")).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        println!("{report}\n{junit}");
        assert!(report.contains("## Model check") && report.contains("## Model and code") && report.contains("## Requirements"));
        assert!(failed, "warnings with --fail-on warning");
        assert!(junit.contains("<testsuite name=\"Requirements\""));
    }
}

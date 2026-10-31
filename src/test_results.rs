//! Test results in the model (LC-399, /test-results): a JUnit XML report or a CSV (test, result[, requirement]) is
//! matched to the requirements of a package – by a requirement id in the test's name (the requirement's alias, or a key
//! like REQ-12 at the start of its name), by a "requirement" column, or through a Test Case element of the same name
//! linked to requirements – and gives a coverage report: requirements passed, failing, and without tests. apply writes
//! the result on each requirement as tagged values (test.result, test.date, test.failed), one ordinary save.

use crate::config::AppConfig;
use crate::tools::query_rows;
use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use std::collections::BTreeMap;

pub fn definition() -> Value {
    json!({
        "name": "test_results",
        "description": "Test results against requirements: a JUnit XML report or a CSV (columns test, result and optionally requirement) \
attached to the conversation is matched to the requirements of a package (with child packages) – by a requirement id in the test name \
(the requirement's alias or a key like REQ-12 in its name), by the requirement column, or through a Test Case element of the same name \
linked to requirements. action report (default): coverage – requirements passed, failing (with the failed tests), without any test, and \
tests that match no requirement. action apply: writes test.result (passed/failed n/m), test.date and test.failed as tagged values on the \
requirements (one save the user approves, undoable).",
        "input_schema": { "type": "object", "properties": {
            "file": { "type": "string", "description": "The attached .xml (JUnit) or .csv" },
            "package": { "description": "package_id or exact name with the requirements" },
            "action": { "type": "string", "enum": ["report", "apply"] }
        }, "required": ["file", "package"] }
    })
}

/// One test run: name (with its class), passed, an explicit requirement.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Test {
    pub name: String,
    pub passed: bool,
    pub requirement: String,
}

fn attr(tag: &str, name: &str) -> String {
    let key = format!(" {name}=\"");
    tag.find(&key).and_then(|i| tag[i + key.len()..].split('"').next()).map(|v| v.replace("&quot;", "\"").replace("&amp;", "&").replace("&lt;", "<").replace("&gt;", ">")).unwrap_or_default()
}

/// The test cases of a JUnit XML report (testsuites/testsuite/testcase; failure, error = failed; skipped is left out).
pub(crate) fn junit(xml: &str) -> Vec<Test> {
    let mut out = Vec::new();
    let mut rest = xml;
    while let Some(start) = rest.find("<testcase") {
        rest = &rest[start..];
        let tag_end = rest.find('>').unwrap_or(rest.len());
        let tag = &rest[..tag_end];
        let self_closing = tag.ends_with('/');
        let body_end = if self_closing { tag_end } else { rest.find("</testcase>").unwrap_or(tag_end) };
        let body = &rest[tag_end.min(body_end)..body_end];
        let (name, class) = (attr(tag, "name"), attr(tag, "classname"));
        if !body.contains("<skipped") {
            out.push(Test { name: if class.is_empty() { name } else { format!("{class}.{name}") }, passed: !body.contains("<failure") && !body.contains("<error"), requirement: String::new() });
        }
        rest = &rest[body_end.max(1)..];
    }
    out
}

/// A CSV or Excel sheet with a header: test (or name), result (or status: pass/passed/ok/success/true = passed),
/// requirement (optional).
pub(crate) fn csv(name: &str, bytes: &[u8]) -> Result<Vec<Test>> {
    let rows = crate::import_sheet::read_rows(name, bytes)?;
    let Some((header, data)) = rows.split_first() else { bail!("the CSV is empty") };
    let find = |names: &[&str]| header.iter().position(|h| names.contains(&h.trim().to_lowercase().as_str()));
    let test = find(&["test", "name", "test name", "testcase", "test case"]).context("the CSV needs a column test (or name)")?;
    let result = find(&["result", "status", "outcome"]).context("the CSV needs a column result (or status)")?;
    let requirement = find(&["requirement", "req", "requirement id"]);
    Ok(data
        .iter()
        .filter(|r| r.get(test).is_some_and(|t| !t.trim().is_empty()))
        .map(|r| {
            let outcome = r.get(result).map(|v| v.trim().to_lowercase()).unwrap_or_default();
            Test {
                name: r[test].trim().to_string(),
                passed: matches!(outcome.as_str(), "pass" | "passed" | "ok" | "success" | "succeeded" | "true" | "green"),
                requirement: requirement.and_then(|i| r.get(i)).map(|v| v.trim().to_string()).unwrap_or_default(),
            }
        })
        .collect())
}

/// Keys like REQ-12, FR_3, US-104 in a text (letters, a dash or underscore, digits).
pub(crate) fn keys_in(text: &str) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        if chars[i].is_ascii_alphabetic() && (i == 0 || !chars[i - 1].is_ascii_alphanumeric()) {
            let start = i;
            while i < chars.len() && chars[i].is_ascii_alphabetic() {
                i += 1;
            }
            if i < chars.len() && (chars[i] == '-' || chars[i] == '_') && chars.get(i + 1).is_some_and(|c| c.is_ascii_digit()) {
                i += 1;
                while i < chars.len() && chars[i].is_ascii_digit() {
                    i += 1;
                }
                out.push(chars[start..i].iter().collect::<String>().to_uppercase().replace('_', "-"));
                continue;
            }
        }
        i += 1;
    }
    out
}

struct Requirement {
    id: String,
    name: String,
    keys: Vec<String>,
}

fn plan(input: &Value, cfg: &AppConfig) -> Result<(Vec<Value>, String)> {
    let file = std::path::Path::new(input["file"].as_str().unwrap_or("")).file_name().context("file")?.to_string_lossy().to_string();
    let bytes = std::fs::read(crate::history::attachment_path(&cfg.attachments_dir, &file)).with_context(|| format!("there is no attachment {file}"))?;
    let text = String::from_utf8_lossy(&bytes);
    let tests = if text.trim_start().trim_start_matches('\u{feff}').starts_with('<') { junit(&text) } else { csv(&file, &bytes)? };
    if tests.is_empty() {
        bail!("{file}: no test results found (JUnit XML <testcase> or a CSV with test and result)");
    }
    let (scope, title) = crate::model_usage::package_scope(cfg, &input["package"])?;
    let requirements: Vec<Requirement> = query_rows(cfg, &format!(
        "select o.object_id, o.name, coalesce(o.alias,'') from t_object o where (o.object_type in ('Requirement', 'Feature') or lower(coalesce(o.stereotype,'')) like '%requirement%') {scope} order by o.name"
    ))?
    .into_iter()
    .map(|r| {
        let mut keys = keys_in(&r[2]);
        keys.extend(keys_in(&r[1]).into_iter().take(1));
        if !r[2].trim().is_empty() && keys.is_empty() {
            keys.push(r[2].trim().to_uppercase());
        }
        Requirement { id: r[0].clone(), name: r[1].clone(), keys }
    })
    .collect();
    if requirements.is_empty() {
        bail!("no requirements in the {title}");
    }
    // Test Case elements linked to requirements: test name → requirement ids.
    let ids = requirements.iter().map(|r| r.id.clone()).collect::<Vec<_>>().join(",");
    let mut by_case: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for r in query_rows(cfg, &format!(
        "select lower(t.name), case when c.start_object_id = t.object_id then c.end_object_id else c.start_object_id end from t_connector c \
         join t_object t on (t.object_id = c.start_object_id or t.object_id = c.end_object_id) and t.object_type = 'TestCase' \
         where c.start_object_id in ({ids}) or c.end_object_id in ({ids})"
    ))? {
        by_case.entry(r[0].clone()).or_default().push(r[1].clone());
    }
    // requirement id → (passed, failed names)
    let mut results: BTreeMap<String, (usize, Vec<String>)> = BTreeMap::new();
    let mut unmatched = Vec::new();
    for t in &tests {
        let mut hit: Vec<String> = Vec::new();
        let wanted: Vec<String> = if t.requirement.is_empty() { keys_in(&t.name) } else { vec![t.requirement.to_uppercase()] };
        for r in &requirements {
            if r.keys.iter().any(|k| wanted.contains(k)) || (!t.requirement.is_empty() && r.name.eq_ignore_ascii_case(&t.requirement)) {
                hit.push(r.id.clone());
            }
        }
        let short = t.name.rsplit('.').next().unwrap_or(&t.name).to_lowercase();
        for linked in by_case.get(&t.name.to_lowercase()).into_iter().chain(by_case.get(&short)).flatten() {
            if !hit.contains(linked) {
                hit.push(linked.clone());
            }
        }
        if hit.is_empty() {
            unmatched.push(t.name.clone());
        }
        for id in hit {
            let entry = results.entry(id).or_default();
            if t.passed {
                entry.0 += 1;
            } else {
                entry.1.push(t.name.clone());
            }
        }
    }
    let (mut passed, mut failing, mut untested) = (Vec::new(), Vec::new(), Vec::new());
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let mut ops = Vec::new();
    for r in &requirements {
        match results.get(&r.id) {
            None => untested.push(r.name.clone()),
            Some((ok, failed)) => {
                let total = ok + failed.len();
                let result = if failed.is_empty() { format!("passed {ok}/{total}") } else { format!("failed {}/{total}", failed.len()) };
                if failed.is_empty() {
                    passed.push(format!("{} ({result})", r.name));
                } else {
                    failing.push(format!("{} ({result}): {}", r.name, failed.iter().take(5).cloned().collect::<Vec<_>>().join(", ")));
                }
                let failed_text: String = failed.join(", ").chars().take(250).collect();
                ops.push(json!({ "op": "update_element", "element": r.id.parse::<i64>().unwrap_or(0), "tags": { "test.result": result, "test.date": today, "test.failed": if failed.is_empty() { Value::Null } else { json!(failed_text) } } }));
            }
        }
    }
    let covered = passed.len() + failing.len();
    let mut out = vec![format!(
        "{file} against the {title}: {} test(s); {covered} of {} requirement(s) covered ({}%), {} passing, {} failing, {} without tests; {} test(s) match no requirement.",
        tests.len(),
        requirements.len(),
        covered * 100 / requirements.len(),
        passed.len(),
        failing.len(),
        untested.len(),
        unmatched.len()
    )];
    let section = |out: &mut Vec<String>, title: &str, list: &[String]| {
        if !list.is_empty() {
            out.push(format!("\n{title} ({}):", list.len()));
            out.extend(list.iter().take(100).map(|l| format!("- {l}")));
        }
    };
    section(&mut out, "Failing", &failing);
    section(&mut out, "Without tests", &untested);
    section(&mut out, "Passing", &passed);
    section(&mut out, "Tests that match no requirement", &unmatched);
    Ok((ops, out.join("\n")))
}

pub fn run(input: &Value, cfg: &AppConfig) -> Result<(Vec<Value>, String)> {
    let (ops, text) = plan(input, cfg)?;
    if input["action"].as_str() == Some("apply") {
        Ok((ops, text))
    } else {
        Ok((Vec::new(), text))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn junit_csv_and_keys() {
        let xml = r#"<?xml version="1.0"?><testsuites><testsuite name="s"><testcase classname="shop.OrderTest" name="REQ-12 places an order" time="0.1"/>
<testcase classname="shop.OrderTest" name="req_13_cancel"><failure message="x">boom</failure></testcase>
<testcase name="skipped one"><skipped/></testcase><testcase name="plain"></testcase></testsuite></testsuites>"#;
        let tests = junit(xml);
        assert_eq!(tests.len(), 3, "{tests:?}");
        assert!(tests[0].passed && !tests[1].passed && tests[2].passed);
        assert_eq!(tests[0].name, "shop.OrderTest.REQ-12 places an order");
        assert_eq!(keys_in("shop.OrderTest.req_13_cancel"), vec!["REQ-13".to_string()]);
        assert_eq!(keys_in("FR-1 and US_204, not abc-x"), vec!["FR-1".to_string(), "US-204".to_string()]);
        let rows = csv("r.csv", b"Test;Result;Requirement\nlogin works;Passed;Login\nlogout;FAIL;\n").unwrap();
        assert_eq!(rows[0], Test { name: "login works".into(), passed: true, requirement: "Login".into() });
        assert!(!rows[1].passed);
    }

    /// `LOONAR_TEST_PROPS=… cargo test test_results_live -- --ignored --nocapture`: a CSV against the requirements of the model.
    #[test]
    #[ignore = "requires a test model"]
    fn test_results_live() {
        let mut cfg = AppConfig::load(std::path::Path::new(&std::env::var("LOONAR_TEST_PROPS").unwrap())).unwrap();
        let reqs = query_rows(&cfg, "select name, package_id from t_object where object_type = 'Requirement' order by package_id, name").unwrap();
        let package = reqs[0][1].clone();
        let names: Vec<&String> = reqs.iter().filter(|r| r[1] == package).map(|r| &r[0]).take(3).collect();
        cfg.attachments_dir = std::env::temp_dir().join(format!("loonar-tests-{}", std::process::id()));
        std::fs::create_dir_all(&cfg.attachments_dir).unwrap();
        let mut csv_text = "test,result,requirement\n".to_string();
        csv_text.push_str(&format!("t1,passed,{}\n", names[0]));
        if names.len() > 1 {
            csv_text.push_str(&format!("t2,failed,{}\n", names[1]));
        }
        csv_text.push_str("t3,passed,\n");
        std::fs::write(cfg.attachments_dir.join("results.csv"), csv_text).unwrap();
        let (ops, text) = plan(&json!({ "file": "results.csv", "package": package.parse::<i64>().unwrap() }), &cfg).unwrap();
        println!("{text}\n{ops:?}");
        let _ = std::fs::remove_dir_all(&cfg.attachments_dir);
        assert!(text.contains("Passing (1)") && text.contains("match no requirement (1)"), "{text}");
        assert_eq!(ops.len(), names.len().min(2));
    }
}

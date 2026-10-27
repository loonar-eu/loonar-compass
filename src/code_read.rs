//! Reading source code for the assistant (LC-286): the files of a folder, a search for text in them and a part of one
//! file, read only. Folders listed in Settings (`code.folders`) are read without asking; any other folder asks first,
//! because what is read goes to the AI model. Build output, dependencies and binary files are left out.
use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

use crate::code_drift::SKIP_DIRS;

/// At most this many files are listed, matches returned and lines read in one call.
const MAX_FILES: usize = 2000;
const MAX_MATCHES: usize = 200;
const MAX_LINES: usize = 2000;
/// Larger files are not searched or read (generated code, data, logs).
const MAX_FILE_BYTES: u64 = 2_000_000;

pub fn definitions() -> Vec<Value> {
    vec![
        json!({
            "name": "code_tree",
            "description": "Lists the source files of a folder (a local clone of a repository), read only: relative paths with sizes, without build output, dependencies (node_modules, target, .git…) and binary files. Use it first to see how the code is organised.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Full path to the folder." },
                    "filter": { "type": "string", "description": "Optional: only paths containing this text, e.g. \".java\" or \"service\"." }
                },
                "required": ["path"]
            }
        }),
        json!({
            "name": "code_search",
            "description": "Searches the source files of a folder for text (case-insensitive; several alternatives separated by |), read only. Returns file:line and the line, at most 200 matches. Use it to find where a class, operation, table, endpoint or term is defined and used.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Full path to the folder." },
                    "query": { "type": "string", "description": "Text to find, e.g. \"OrderService\" or \"createOrder|placeOrder\"." },
                    "filter": { "type": "string", "description": "Optional: only files whose path contains this text, e.g. \".java\"." }
                },
                "required": ["path", "query"]
            }
        }),
        json!({
            "name": "code_read",
            "description": "Reads one source file (or lines from-to of it), read only, with line numbers; at most 2000 lines per call.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Full path to the file." },
                    "from": { "type": "integer", "description": "First line (1-based), optional." },
                    "to": { "type": "integer", "description": "Last line, optional." }
                },
                "required": ["path"]
            }
        }),
    ]
}

/// A full local path without "..", inside a code folder from Settings or approved by the user now.
pub fn allowed(cfg: &crate::config::AppConfig, given: &str, ask: &dyn Fn(&str) -> Result<()>) -> Result<PathBuf> {
    let text = given.trim().trim_matches('"').replace('/', "\\");
    let b = text.as_bytes();
    let drive = b.len() >= 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && b[2] == b'\\';
    if !drive || text.split('\\').any(|p| p == "..") {
        bail!("{}", crate::i18n::fmt("{0}: give a full path on a local drive; network shares and relative paths are not allowed", &[given.to_string()]));
    }
    let path = PathBuf::from(&text);
    let inside = cfg.code_folders.iter().any(|root| {
        let root = root.trim().replace('/', "\\").trim_end_matches('\\').to_lowercase();
        !root.is_empty() && (text.to_lowercase() == root || text.to_lowercase().starts_with(&format!("{root}\\")))
    });
    if !inside {
        ask(&crate::i18n::fmt("Read the source code in {0}? What is read goes to the AI model of the conversation. (Folders listed in Settings → Source code are read without asking.)", &[text.clone()]))?;
    }
    Ok(path)
}

fn skipped_dir(name: &str) -> bool {
    SKIP_DIRS.contains(&name) || name.starts_with('.')
}

/// Text files of the folder (relative path, full path, size), sorted.
fn files(root: &Path, filter: &str) -> Result<Vec<(String, PathBuf, u64)>> {
    if !root.is_dir() {
        bail!("{}", crate::i18n::fmt("{0} is not a folder", &[root.display().to_string()]));
    }
    let filter = filter.trim().to_lowercase().replace('/', "\\");
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            let Ok(meta) = entry.metadata() else { continue };
            if meta.is_dir() {
                if !skipped_dir(&name) {
                    stack.push(path);
                }
                continue;
            }
            let rel = path.strip_prefix(root).unwrap_or(&path).display().to_string();
            if meta.len() > MAX_FILE_BYTES || (!filter.is_empty() && !rel.to_lowercase().contains(&filter)) || binary_name(&name) {
                continue;
            }
            out.push((rel, path, meta.len()));
            if out.len() > MAX_FILES * 5 {
                break;
            }
        }
    }
    out.sort();
    Ok(out)
}

fn binary_name(name: &str) -> bool {
    let lower = name.to_lowercase();
    [".png", ".jpg", ".jpeg", ".gif", ".ico", ".pdf", ".zip", ".jar", ".war", ".class", ".exe", ".dll", ".so", ".dylib", ".bin", ".woff", ".woff2", ".ttf", ".eot", ".mp4", ".mp3", ".qea", ".qeax", ".eap", ".docx", ".xlsx", ".pptx", ".gz", ".7z", ".lock"]
        .iter()
        .any(|e| lower.ends_with(e))
}

/// The text of a file, or None for a binary one.
fn text_of(path: &Path) -> Option<String> {
    let bytes = std::fs::read(path).ok()?;
    if bytes.iter().take(8000).any(|b| *b == 0) {
        return None;
    }
    Some(String::from_utf8_lossy(&bytes).into_owned())
}

pub fn tree(root: &Path, filter: &str) -> Result<String> {
    let all = files(root, filter)?;
    let mut out = crate::i18n::fmt("Source files in {0}: {1}", &[root.display().to_string(), all.len().to_string()]) + "\n";
    for (rel, _, size) in all.iter().take(MAX_FILES) {
        out.push_str(&format!("{rel}  ({} KB)\n", size.div_ceil(1024)));
    }
    if all.len() > MAX_FILES {
        out.push_str(&format!("… {} more; narrow it with 'filter'.\n", all.len() - MAX_FILES));
    }
    Ok(out)
}

pub fn search(root: &Path, query: &str, filter: &str) -> Result<String> {
    let terms: Vec<String> = query.split('|').map(|t| t.trim().to_lowercase()).filter(|t| !t.is_empty()).collect();
    if terms.is_empty() {
        bail!("{}", crate::i18n::fmt("missing field '{0}'", &["query".to_string()]));
    }
    let mut hits = Vec::new();
    let mut files_hit = 0;
    for (rel, path, _) in files(root, filter)? {
        let Some(text) = text_of(&path) else { continue };
        let mut found = false;
        for (n, line) in text.lines().enumerate() {
            let lower = line.to_lowercase();
            if terms.iter().any(|t| lower.contains(t.as_str())) {
                found = true;
                if hits.len() < MAX_MATCHES {
                    let line: String = line.trim().chars().take(240).collect();
                    hits.push(format!("{rel}:{}: {line}", n + 1));
                }
            }
        }
        files_hit += found as usize;
    }
    let mut out = crate::i18n::fmt("\"{0}\" in {1}: {2} files", &[query.to_string(), root.display().to_string(), files_hit.to_string()]) + "\n";
    out.push_str(&hits.join("\n"));
    if hits.len() >= MAX_MATCHES {
        out.push_str("\n… more matches; narrow the query or the filter.");
    }
    Ok(out)
}

pub fn read(path: &Path, from: Option<i64>, to: Option<i64>) -> Result<String> {
    let meta = std::fs::metadata(path).with_context(|| crate::i18n::fmt("{0} does not exist", &[path.display().to_string()]))?;
    if !meta.is_file() || meta.len() > MAX_FILE_BYTES * 5 {
        bail!("{}", crate::i18n::fmt("{0} is not a source file that can be read", &[path.display().to_string()]));
    }
    let text = text_of(path).with_context(|| crate::i18n::fmt("{0} is a binary file", &[path.display().to_string()]))?;
    let lines: Vec<&str> = text.lines().collect();
    let first = from.unwrap_or(1).max(1) as usize;
    let last = to.map(|t| t.max(1) as usize).unwrap_or(lines.len()).min(lines.len()).min(first + MAX_LINES - 1);
    let mut out = format!("{} ({} lines), lines {first}-{last}:\n", path.display(), lines.len());
    for (i, line) in lines.iter().enumerate().take(last).skip(first - 1) {
        out.push_str(&format!("{:>5}  {line}\n", i + 1));
    }
    if last < lines.len() {
        out.push_str(&format!("… {} more lines (use from/to).\n", lines.len() - last));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tree_search_read() {
        let dir = std::env::temp_dir().join(format!("loonar-code-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("src/order")).unwrap();
        std::fs::create_dir_all(dir.join("node_modules/x")).unwrap();
        std::fs::create_dir_all(dir.join("target")).unwrap();
        std::fs::write(dir.join("src/order/OrderService.java"), "class OrderService {\n  void createOrder() {}\n  void cancel() { payments.refund(); }\n}\n").unwrap();
        std::fs::write(dir.join("src/Pay.kt"), "fun refund() = 1\n").unwrap();
        std::fs::write(dir.join("node_modules/x/OrderService.js"), "OrderService\n").unwrap();
        std::fs::write(dir.join("target/OrderService.class"), [0u8, 1, 2]).unwrap();
        std::fs::write(dir.join("logo.png"), [0u8, 1]).unwrap();
        let t = tree(&dir, "").unwrap();
        assert!(t.contains("src\\order\\OrderService.java") && t.contains("src\\Pay.kt") && !t.contains("node_modules") && !t.contains("logo.png"), "{t}");
        let s = search(&dir, "createorder|refund", "").unwrap();
        assert!(s.contains("OrderService.java:2:") && s.contains("OrderService.java:3:") && s.contains("Pay.kt:1:") && s.contains(": 2 files"), "{s}");
        assert!(search(&dir, "refund", ".kt").unwrap().contains(": 1 files"));
        let r = read(&dir.join("src/order/OrderService.java"), Some(2), Some(3)).unwrap();
        assert!(r.contains("    2  ") && r.contains("createOrder") && !r.contains("class OrderService {") && r.contains("1 more lines"), "{r}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn only_full_local_paths_and_folders_from_settings() {
        let props = std::env::temp_dir().join(format!("loonar-code-{}.properties", std::process::id()));
        std::fs::write(&props, "").unwrap();
        let mut cfg = crate::config::AppConfig::load(&props).unwrap();
        cfg.code_folders = vec!["C:\\repos".into()];
        let never = |_: &str| -> Result<()> { bail!("asked") };
        assert!(allowed(&cfg, "C:\\repos\\shop\\src", &never).is_ok());
        assert!(allowed(&cfg, "C:/repos/shop", &never).is_ok());
        assert!(allowed(&cfg, "C:\\other", &never).is_err(), "outside the folders it asks");
        assert!(allowed(&cfg, "C:\\repos\\..\\Windows", &never).is_err());
        assert!(allowed(&cfg, "\\\\server\\share", &never).is_err());
        assert!(allowed(&cfg, "C:\\reposX", &never).is_err());
    }
}

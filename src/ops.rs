//! Operations on the model that a write or a change set may contain: the list the assistant may use, their short
//! description, ids as EA reads them, and the files they may write or read. Shared by the desktop and the team hub.

use anyhow::{bail, Result};
use serde_json::{json, Value};

/// Operations the model may request. `delete_package`, `export_diagram` and `ping` are for internal use only.
pub const MODEL_OPS: &[&str] = &[
    "create_package",
    "create_diagram",
    "create_element",
    "create_connector",
    "place",
    "diagram_style",
    "set_composite",
    "update_element",
    "update_connector",
    "move",
    "navigation_cell",
    "delete",
    "remove_from_diagram",
    "set_color",
    "set_bounds",
    "set_line_style",
    "route_connector",
    "update_diagram",
    "hide_connectors",
    "repair_diagram_links",
    "create_attribute",
    "create_legend",
    "create_operation",
    "update_attribute",
    "delete_attribute",
    "update_operation",
    "delete_operation",
    "clone_package",
    "create_baseline",
    "add_glossary_term",
    "create_scenario",
    "update_scenario",
    "delete_scenario",
    "layout_diagram",
    "export_xmi",
    "import_xmi",
    "set_linked_document",
    "set_swimlanes",
    "reverse_connector",
    "update_package",
    "generate_report",
    "version_control",
];

/// Short description of the operations in the UI language: for the approval dialog, the step list, the change log and the model.
pub fn describe(ops: &[Value]) -> String {
    let mut count = std::collections::BTreeMap::new();
    for op in ops {
        *count.entry(op["op"].as_str().unwrap_or("?")).or_insert(0usize) += 1;
    }
    let label = |k: &str| match k {
        "create_package" => crate::i18n::tr("new packages"),
        "create_diagram" => crate::i18n::tr("new diagrams"),
        "create_element" => crate::i18n::tr("new elements"),
        "create_connector" => crate::i18n::tr("new connectors"),
        "place" => crate::i18n::tr("elements placed on diagrams"),
        "update_element" => crate::i18n::tr("changed elements"),
        "update_connector" => crate::i18n::tr("changed relationships"),
        "move" => crate::i18n::tr("objects moved to another package"),
        "navigation_cell" => crate::i18n::tr("navigation cells (links to diagrams)"),
        "delete" => crate::i18n::tr("objects DELETED from the model"),
        "set_color" => crate::i18n::tr("changed colors on diagrams"),
        "set_bounds" => crate::i18n::tr("changed position or size on diagrams"),
        "set_line_style" => crate::i18n::tr("changed connector line styles on diagrams"),
        "route_connector" => crate::i18n::tr("routed connector paths on diagrams"),
        "update_diagram" => crate::i18n::tr("changed diagram names or notes"),
        "hide_connectors" => crate::i18n::tr("connectors hidden or shown on diagrams"),
        "repair_diagram_links" => crate::i18n::tr("diagram links repaired (duplicates removed, hidden connectors shown)"),
        "create_attribute" => crate::i18n::tr("new attributes"),
        "create_legend" => crate::i18n::tr("diagram legends"),
        "create_operation" => crate::i18n::tr("new operations"),
        "update_attribute" => crate::i18n::tr("changed attributes"),
        "delete_attribute" => crate::i18n::tr("attributes DELETED"),
        "update_operation" => crate::i18n::tr("changed operations"),
        "delete_operation" => crate::i18n::tr("operations DELETED"),
        "clone_package" => crate::i18n::tr("package copies (with all contents)"),
        "create_baseline" => crate::i18n::tr("package baselines (saved versions)"),
        "add_glossary_term" => crate::i18n::tr("terms in the project glossary"),
        "create_scenario" | "update_scenario" => crate::i18n::tr("structured scenarios of elements"),
        "delete_scenario" => crate::i18n::tr("structured scenarios DELETED"),
        "remove_from_diagram" => crate::i18n::tr("elements removed from diagrams (they stay in the model)"),
        "diagram_style" => crate::i18n::tr("diagram style settings"),
        "layout_diagram" => crate::i18n::tr("diagrams laid out automatically"),
        "export_xmi" => crate::i18n::tr("packages exported to XMI"),
        "import_xmi" => crate::i18n::tr("XMI files IMPORTED into packages"),
        "set_linked_document" => crate::i18n::tr("linked documents of elements"),
        "set_swimlanes" => crate::i18n::tr("diagram swimlanes"),
        "reverse_connector" => crate::i18n::tr("relationships reversed"),
        "update_package" => crate::i18n::tr("changed packages"),
        "generate_report" => crate::i18n::tr("documents from EA templates"),
        "version_control" => crate::i18n::tr("version control of packages (check in / check out / get latest)"),
        "set_composite" => crate::i18n::tr("element-to-diagram links"),
        other => other_static(other),
    };
    let mut lines: Vec<String> = count.iter().map(|(k, n)| crate::i18n::fmt("{0}: {1}@@count", &[label(k).to_string(), n.to_string()])).collect();

    // Names of what is created at the top level: packages and diagrams (there are often dozens of elements).
    for op in ops {
        let name = op["name"].as_str().unwrap_or("");
        match op["op"].as_str() {
            Some("create_package") => lines.push(crate::i18n::fmt("package \"{0}\"", &[name.to_string()])),
            Some("create_diagram") => lines.push(crate::i18n::fmt("diagram \"{0}\" ({1})", &[name.to_string(), op["type"].as_str().unwrap_or("?").to_string()])),
            _ => {}
        }
    }
    lines.join("\n")
}

fn other_static(_: &str) -> &'static str {
    crate::i18n::tr("other operations")
}

/// Reference fields of operations: an id given as text ("123") or as 123.0 becomes a number before any check, so the
/// checks see the same object that EA changes.
pub const REF_FIELDS: &[&str] = &[
    "element", "package", "diagram", "connector", "child_package", "source", "target", "parent", "classifier", "from", "to", "object", "attribute",
    "operation", "target_package", "owner", "element_id", "diagram_id", "package_id", "connector_id",
];

pub fn normalize_ops(ops: &[Value]) -> Vec<Value> {
    ops.iter()
        .map(|op| {
            let mut op = op.clone();
            if let Some(fields) = op.as_object_mut() {
                for (name, value) in fields.iter_mut() {
                    if REF_FIELDS.contains(&name.as_str()) {
                        // A reference as EA reads it (ea_com int_of): a number rounded, or a whole number in text.
                        if let Some(f) = value.as_f64().filter(|f| f.is_finite() && f.abs() < 2e9) {
                            *value = json!(f.round() as i64);
                        } else if let Some(n) = value.as_str().and_then(|s| s.trim().parse::<i64>().ok()) {
                            *value = json!(n);
                        }
                    } else if let Some(n) = value.as_f64().filter(|f| f.fract() == 0.0 && !value.is_i64() && !value.is_u64() && f.abs() < 1e15).map(|f| f as i64) {
                        *value = json!(n);
                    }
                }
            }
            op
        })
        .collect()
}

/// A file an operation writes or reads (XMI, reports): a full local path, not a network share, a device or a system folder.
pub fn check_file_path(file: &str) -> Result<std::path::PathBuf> {
    let text = file.trim().replace('/', "\\");
    let bytes = text.as_bytes();
    let drive = bytes.len() > 3 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' && bytes[2] == b'\\';
    // No "." or ".." segments, no streams (":"), no segments ending in a dot or a space, no doubled separators, no wildcards.
    let segments: Vec<&str> = text[3.min(text.len())..].split('\\').collect();
    let odd = segments.iter().any(|s| s.is_empty() || *s == "." || *s == ".." || s.ends_with('.') || s.ends_with(' '))
        || text[2.min(text.len())..].contains(':') || text.contains('*') || text.contains('?') || text.contains('"') || text.contains('<') || text.contains('>') || text.contains('|');
    if !drive || odd {
        bail!("{}", crate::i18n::fmt("{0}: give a full path on a local drive; network shares and relative paths are not allowed", &[file.to_string()]));
    }
    // The real path: the deepest existing folder resolved by Windows (short 8.3 names, junctions, links), then the rest.
    let path = std::path::PathBuf::from(&text);
    let mut existing = path.as_path();
    let mut rest: Vec<std::ffi::OsString> = Vec::new();
    while !existing.exists() {
        rest.push(existing.file_name().map(|n| n.to_os_string()).unwrap_or_default());
        existing = existing.parent().unwrap_or(std::path::Path::new(&text[..3]));
        if existing.as_os_str().len() <= 3 {
            break;
        }
    }
    let mut real = std::fs::canonicalize(existing).unwrap_or_else(|_| existing.to_path_buf());
    for name in rest.iter().rev() {
        real.push(name);
    }
    let shown = real.display().to_string();
    let shown = shown.strip_prefix(r"\\?\").unwrap_or(&shown).to_string();
    if shown.starts_with(r"\\") || shown.starts_with(r"UNC\") {
        bail!("{}", crate::i18n::fmt("{0}: give a full path on a local drive; network shares and relative paths are not allowed", &[file.to_string()]));
    }
    let lower = format!("{}\\", shown.to_lowercase());
    let windows = std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".into()).to_lowercase();
    // System folders, the program folders, autostart and the profile's application data (its Temp folder is allowed).
    let forbidden_start = [format!("{windows}\\"), r"c:\program files".to_string(), r"c:\programdata\".to_string()];
    let forbidden_inside = [r"\start menu\", r"\startup\", r"\appdata\roaming\", r"\appdata\locallow\", r"\appdata\local\microsoft\", r"\appdata\local\programs\"];
    if forbidden_start.iter().any(|f| lower.starts_with(f.as_str())) || forbidden_inside.iter().any(|f| lower.contains(f)) {
        bail!("{}", crate::i18n::fmt("{0}: system and application folders are not allowed", &[file.to_string()]));
    }
    Ok(std::path::PathBuf::from(shown))
}


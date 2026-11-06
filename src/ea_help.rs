//! "How do I do this in EA?" (LC-375): the version of the user's Enterprise Architect (from EA.exe), the address of
//! Sparx's user guide for that version, and a small base of known EA problems with their workarounds, so the assistant
//! answers with the right ribbon path for that version instead of a guess (other chatbots invented EA features).

use crate::config::AppConfig;
use anyhow::Result;
use serde_json::{json, Value};

pub fn definition() -> Value {
    json!({
        "name": "ea_howto",
        "description": "Help with Enterprise Architect itself: the user's EA version (from EA.exe), the address of Sparx's user guide for that \
version, and known EA problems with their workarounds that match the question. Use it when the user asks how to do something in EA, cannot \
find a command, or describes odd EA behaviour. Answer with the exact ribbon path for that version; give a guide page only when you know it (you \
may search the web for the page in that version's guide); if the API allows it, offer to do it (ea_write). Never invent a menu or option.",
        "input_schema": { "type": "object", "properties": {
            "question": { "type": "string", "description": "What the user wants to do or what goes wrong, in a few words" }
        }, "required": ["question"] }
    })
}

/// A known EA problem: words that point to it, what helps, the source of the knowledge.
struct Known {
    words: &'static [&'static str],
    problem: &'static str,
    help: &'static str,
}

const KNOWN: &[Known] = &[
    Known {
        words: &["changed by another user", "reload", "lost changes", "zmieniony przez innego", "przeładow"],
        problem: "EA says the diagram was changed by another user (also when working alone on a local file) and reloading drops unsaved changes.",
        help: "Save the diagram before you accept a reload. Compass can compare the diagram in the database with the open one before you reload (ask it). Automatic reloading of changed diagrams is an option of EA's preferences; check its exact place in the guide of this version.",
    },
    Known {
        words: &["lock", "connector", "blokad", "konektor", "relacj"],
        problem: "Relationships can be changed although their elements are locked.",
        help: "An administrator turns on the security option \"Apply Locks to Connectors\"; without it connector changes skip the locks.",
    },
    Known {
        words: &["lock", "left", "blokad", "odszed", "manage locks"],
        problem: "Locks of a person who left stay, and Manage Locks does not show them.",
        help: "Locks kept by version control are released in version control, not in Manage Locks. /repo-doctor lists old locks and locks of users that no longer exist.",
    },
    Known {
        words: &["quicklinker", "archimate", "relationship", "uml only", "tylko uml"],
        problem: "In ArchiMate the Quicklinker offers only UML relationships.",
        help: "In the Quicklinker menu untick \"Filter to toolbox\".",
    },
    Known {
        words: &["hide", "connector", "every diagram", "all diagrams", "ukry", "wszystkich diagram", "freeze visible"],
        problem: "A relationship drawn on one diagram appears on every diagram that shows both its ends.",
        help: "EA does this by design. Hide it per diagram (Compass: hide_connectors, or the visibility policy of a diagram), or use \"Freeze Visible\" on a diagram that must not get new ones.",
    },
    Known {
        words: &["matrix", "hidden", "macierz", "ukryt"],
        problem: "The Relationship Matrix of a diagram leaves out hidden relationships.",
        help: "Use the matrix of the packages (Compass: /matrix) to see all relationships, hidden ones included.",
    },
    Known {
        words: &["status", "bulk", "many elements", "masow", "wiele element", "tagged value", "stereotype"],
        problem: "Changing many elements at once.",
        help: "EA's Update Status (on a package) changes only status, phase and version. For stereotypes, authors and tagged values use Compass (/bulk-set) – a preview first, then one approval.",
    },
    Known {
        words: &["sequence", "message", "move", "fragment", "sekwencj", "komunikat"],
        problem: "Moving messages on a sequence diagram does not move empty fragments and notes with them.",
        help: "Hold Alt while dragging to move a message without the rest, and move empty fragments by hand afterwards; Compass can rebuild a sequence from a description.",
    },
    Known {
        words: &["integrity", "spójność", "integralno", "broken", "uszkodz"],
        problem: "Damaged rows in the repository (relationships to missing elements, stale diagram links).",
        help: "EA's Project Integrity Check repairs part of it, but it does not check everything (e.g. element types): /repo-doctor in Compass lists more.",
    },
    Known {
        words: &["layout", "układ", "uklad", "nothing happens", "nic się nie dzieje"],
        problem: "Automatic layout of a diagram sometimes does nothing, without an error.",
        help: "Compass checks the positions after a layout and says when nothing moved (/tidy-diagram).",
    },
    Known {
        words: &["develop", "ribbon", "wstążk", "wstazk", "missing", "brak", "workspace"],
        problem: "A ribbon or a window is missing after an update.",
        help: "Ribbons depend on the perspective and the workspace: switch to a perspective that has it, or reset the workspace.",
    },
    Known {
        words: &["svg", "export", "eksport", "image", "obraz"],
        problem: "Saving a diagram as SVG through the API reported success without a file (fixed in EA 17.0 build 1702).",
        help: "Update EA; Compass exports diagrams as PNG (twice the size for documents).",
    },
];

/// The file version of EA.exe ("17.1.1716"), or "" when it cannot be read.
pub(crate) fn ea_version(cfg: &AppConfig) -> String {
    static CACHE: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    CACHE
        .get_or_init(|| {
            let path = cfg.ea_path.display().to_string().replace('\'', "''");
            let mut cmd = std::process::Command::new(crate::config::system_program("powershell.exe"));
            cmd.args(["-NoProfile", "-NonInteractive", "-Command", &format!("(Get-Item -LiteralPath '{path}').VersionInfo.FileVersion")]);
            #[cfg(windows)]
            {
                use std::os::windows::process::CommandExt;
                cmd.creation_flags(0x0800_0000);
            }
            cmd.output().ok().map(|o| String::from_utf8_lossy(&o.stdout).trim().replace(", ", ".").replace(',', ".")).unwrap_or_default()
        })
        .clone()
}

/// The guide of a version: https://sparxsystems.com/enterprise_architect_user_guide/<major.minor>/
pub(crate) fn guide_url(version: &str) -> String {
    let mut parts = version.split('.');
    match (parts.next(), parts.next()) {
        (Some(major), Some(minor)) if major.parse::<u32>().is_ok() => format!("https://sparxsystems.com/enterprise_architect_user_guide/{major}.{}/", minor.chars().next().unwrap_or('0')),
        _ => "https://sparxsystems.com/enterprise_architect_user_guide/".to_string(),
    }
}

/// The known problems whose words appear in the question, best first.
pub(crate) fn matching(question: &str) -> Vec<&'static str> {
    let q = question.to_lowercase();
    let mut scored: Vec<(usize, usize)> = KNOWN.iter().enumerate().map(|(i, k)| (k.words.iter().filter(|w| q.contains(*w)).count(), i)).filter(|(n, _)| *n > 0).collect();
    scored.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    scored.into_iter().take(3).map(|(_, i)| KNOWN[i].problem).collect()
}

/// What Compass can do itself for a "how do I…" question (LC-417): words of the question → the tool or command.
const DO_IT: &[(&[&str], &str)] = &[
    (&["layout", "arrange", "tidy", "układ", "uporządk", "rozmieść"], "layout_diagram (the whole diagram), place_new_elements (only new ones), align_elements"),
    (&["align", "wyrówn", "distribute", "same size"], "align_elements"),
    (&["colour", "color", "kolor", "style", "styl"], "apply_style (/apply-style) or set_color"),
    (&["hide", "show connector", "ukryj", "pokaż relac", "visibility", "widoczn"], "connector_visibility (/visibility, /show-hidden-links) or hide_connectors"),
    (&["baseline", "compare", "porówn"], "create_baseline, compare_baselines (/compare-baselines), what_changed"),
    (&["rename", "zmień nazw", "status", "stereotype", "stereotyp", "tagged value", "wartość tag", "bulk", "masow"], "bulk_set (/bulk-set) or update_element"),
    (&["delete", "usuń", "remove"], "what_if (/what-if) first, then recycle_bin (/to-bin) or a delete the user approves"),
    (&["merge", "duplicate", "scal", "duplikat"], "find_duplicates and merge_elements (/merge-elements)"),
    (&["relationship type", "connector type", "typ relacj", "change type"], "change_connector_type"),
    (&["document", "report", "rtf", "dokument", "raport"], "create_document, template_doctor (/template-doctor)"),
    (&["lock", "greyed", "grayed", "wyszarz", "blokad"], "why_locked (/why-locked)"),
    (&["import", "excel", "csv", "openapi", "swagger"], "import_sheet (/import-sheet), import_api (/import-api)"),
    (&["export", "mermaid", "plantuml", "xmi"], "diagram_as_text (/diagram-as-text), export_text (/export-text) or export_xmi"),
    (&["sequence", "sekwenc"], "sequence_from_text (/sequence-from-text), sequence_check (/check-sequence)"),
    (&["translate", "tłumacz", "language"], "translate_model (/translate-model)"),
];

/// The tools of Compass that do what the question asks (empty when nothing fits).
pub(crate) fn can_do(question: &str) -> Vec<&'static str> {
    let q = question.to_lowercase();
    DO_IT.iter().filter(|(words, _)| words.iter().any(|w| q.contains(w))).map(|(_, tools)| *tools).collect()
}

pub fn run(input: &Value, cfg: &AppConfig) -> Result<String> {
    let question = input["question"].as_str().unwrap_or("");
    let version = ea_version(cfg);
    let mut out = format!(
        "Enterprise Architect on this computer: {}.\nUser guide of this version: {}\n",
        if version.is_empty() { "version unknown (EA.exe not found at ea.path)".to_string() } else { version.clone() },
        guide_url(&version)
    );
    let found = matching(question);
    if found.is_empty() {
        out.push_str("\nNo known EA problem matches. Answer from your knowledge of this EA version; say when you are not sure of a menu path.\n");
    } else {
        out.push_str("\nKnown EA problems that match:\n");
        for problem in found {
            let k = KNOWN.iter().find(|k| k.problem == problem).unwrap();
            out.push_str(&format!("- {}\n  What helps: {}\n", k.problem, k.help));
        }
    }
    // "Do it for me" (LC-417): the steps in EA first, then the offer to do it with Compass's own tools (the user approves the save).
    let tools = can_do(question);
    if !tools.is_empty() {
        out.push_str(&format!("\nCompass can do this itself: {}. After the steps in EA, offer it (\"I can do it for you\") – a change is one save the user approves.\n", tools.join("; ")));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    /// LC-417: questions that Compass can do itself are matched to its tools.
    #[test]
    fn do_it_for_me() {
        assert!(super::can_do("How do I align these classes and change their colour?").iter().any(|t| t.contains("align_elements")));
        assert!(super::can_do("jak zmienić status wielu elementów").iter().any(|t| t.contains("bulk_set")));
        assert!(super::can_do("how do I add a hyperlink to a website").is_empty());
    }

    /// LC-375: the version of the EA installed in the lab: `cargo test ea_version_live -- --ignored --nocapture`
    #[test]
    #[ignore = "requires Enterprise Architect"]
    fn ea_version_live() {
        let props = std::env::temp_dir().join(format!("loonar-howto-{}.properties", std::process::id()));
        std::fs::write(&props, "model.source=file
model.file=C:/m/x.qea
").unwrap();
        let cfg = crate::config::AppConfig::load(&props).unwrap();
        let out = super::run(&serde_json::json!({ "question": "Quicklinker shows only UML in ArchiMate" }), &cfg).unwrap();
        println!("{out}");
        assert!(out.contains("enterprise_architect_user_guide/1") && out.contains("Filter to toolbox"));
    }

    #[test]
    fn guide_and_known_problems() {
        assert_eq!(super::guide_url("17.1.1716"), "https://sparxsystems.com/enterprise_architect_user_guide/17.1/");
        assert_eq!(super::guide_url("16.10.1628"), "https://sparxsystems.com/enterprise_architect_user_guide/16.1/");
        assert_eq!(super::guide_url(""), "https://sparxsystems.com/enterprise_architect_user_guide/");
        assert!(super::matching("Quicklinker w ArchiMate pokazuje tylko UML")[0].contains("Quicklinker"));
        assert!(super::matching("diagram changed by another user, I lost changes after reload")[0].contains("changed by another user"));
        assert!(super::matching("jak zmienić kolor tła").is_empty());
    }
}

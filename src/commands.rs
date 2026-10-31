//! Commands typed in the chat (LC-337): `/name arguments`. Typing `/` shows the list with a line about each; a command
//! becomes a precise request to the assistant (the tool it should use, what to show), so the answer does not depend on
//! how the user words it. `/glossary-add` opens its own window (glossary_ui).

pub struct Command {
    pub name: &'static str,
    /// What to type after the name (shown in the list).
    pub args: &'static str,
    /// One line about the command (an i18n key).
    pub about: &'static str,
    /// The request sent to the assistant (an i18n key; {0} = the arguments, or "the one open in EA" when none).
    pub request: &'static str,
}

pub const ALL: &[Command] = &[
    Command { name: "/tidy-diagram", args: "[diagram]", about: "Tidy up a diagram: layout, spacing, straight connectors", request: "Tidy up the diagram {0}: arrange the elements with layout_diagram in the direction that suits the diagram, keep related elements together, give labels room, then straighten and route the connectors so they do not cross elements; show the result and say what changed." },
    Command { name: "/where-used", args: "<element>", about: "Every use of an element: diagrams, relationships, types, tags", request: "Where is the element {0} used? Use where_used and list every use, grouped." },
    Command { name: "/find-orphans", args: "[package]", about: "Elements nothing uses (more than EA's orphans search)", request: "Find the elements that nothing uses in {0} with find_orphans; list them grouped by package and suggest what to do, without changing anything yet." },
    Command { name: "/find-duplicates", args: "[package]", about: "Elements and relationships that are probably duplicates", request: "Find the likely duplicates in {0} with find_duplicates; for each group say which one to keep (the most used) and why, without changing anything yet." },
    Command { name: "/repo-doctor", args: "", about: "Integrity of the repository: broken links, stale rows, duplicate GUIDs, old locks", request: "Check the integrity of this repository with repo_doctor; explain the findings in plain words, most serious first, and say which you can repair (with my approval) and which need EA's Integrity Check or a person." },
    Command { name: "/matrix", args: "<source package> × <target package> [relationship type]", about: "Relationship matrix of two packages (also as EA's own matrix)", request: "Build the relationship matrix {0} with package_matrix and offer to save it in EA as a Matrix Specification." },
    Command { name: "/new-version", args: "<element>", about: "Clone an element as a new version with a trace to the original", request: "Clone the element {0} as a new version with a trace relationship to the original (clone_element_version)." },
    Command { name: "/doc-package", args: "<package>", about: "A readable Word document of a package: diagrams, elements, relationships", request: "Write a readable Word document of the package {0}: its purpose, every diagram as a picture with a short description, a table of its elements with type and description, and the important relationships; use create_document." },
    Command { name: "/template-from-doc", args: "[template name]", about: "Turn the attached document into an EA report template", request: "Turn the attached document into an EA document template {0} with save_document_template: keep its headings and order, put the parts about the package, its diagrams and each element into the matching parts with EA fields; tell me how to generate a report with it." },
    Command { name: "/send-to-miro", args: "[diagram]", about: "Send a diagram to a Miro board", request: "Open \"Send to Miro\" for the diagram {0} (miro_send)." },
    Command { name: "/glossary-add", args: "[term]", about: "Add a term to the EA project glossary (Polish forms included)", request: "" },
];

/// Commands whose name starts with what is typed (the whole list for "/").
pub fn matching(input: &str) -> Vec<&'static Command> {
    let typed = input.trim_start();
    if !typed.starts_with('/') || typed.contains(char::is_whitespace) {
        return Vec::new();
    }
    let typed = typed.to_lowercase().replace('_', "-");
    ALL.iter().filter(|c| c.name.starts_with(&typed)).collect()
}

/// The request for a typed command (None: not a command of this list, sent as it is).
pub fn request(input: &str) -> Option<String> {
    let typed = input.trim();
    let (name, args) = typed.split_once(char::is_whitespace).unwrap_or((typed, ""));
    let name = name.to_lowercase().replace('_', "-");
    let command = ALL.iter().find(|c| c.name == name && !c.request.is_empty())?;
    let args = args.trim();
    let target = if args.is_empty() { crate::i18n::tr("(the one open or selected in EA; ask if there is none)").to_string() } else { format!("\"{args}\"") };
    Some(crate::i18n::fmt(command.request, &[target]))
}

#[cfg(test)]
mod tests {
    #[test]
    fn list_and_requests() {
        assert_eq!(super::matching("/").len(), super::ALL.len());
        assert_eq!(super::matching("/where").iter().map(|c| c.name).collect::<Vec<_>>(), vec!["/where-used"]);
        assert!(super::matching("/where-used Klient").is_empty());
        assert!(super::matching("hello").is_empty());
        let r = super::request("/where_used Klient").unwrap();
        assert!(r.contains("\"Klient\"") && r.contains("where_used"), "{r}");
        assert!(super::request("/tidy-diagram").unwrap().contains("EA"));
        assert!(super::request("/glossary-add x").is_none(), "its own window");
        assert!(super::request("/nonsense").is_none());
    }
}

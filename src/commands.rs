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
    Command { name: "/what-changed", args: "[since: date or days] [author] [package]", about: "What changed in the model, by whom, in business language", request: "What changed in the model {0}? Use what_changed and summarise it in business language: what was added, what was changed, by whom and where; say what it cannot show (relationships, deletions)." },
    Command { name: "/validate-model", args: "[package]", about: "Check a package against the team's rules (naming, tags, relationships, stereotypes)", request: "Check {0} against the team's modelling rules with model_lint (team_rules show tells which rules are in force). Group the findings by rule, most serious first, and offer fixes; if the team has no rules yet, ask me what they should be and save them with team_rules." },
    Command { name: "/explain", args: "<diagram> [for whom: board, CFO, auditor, developer]", about: "Explain a diagram in plain language for a chosen audience", request: "Explain the diagram {0} for its audience in plain business language: look at it (ea_show_diagram) and read its elements and relationships from the model; say what it shows, why it matters, the main flow or structure in a few steps and the open questions; no modelling jargon unless the audience is technical. Offer a one-page summary in Word or speaker notes for PowerPoint." },
    Command { name: "/trace-gaps", args: "[package] [steps]", about: "Requirements nothing realises and design nothing justifies, through several steps", request: "Find the gaps in traceability in {0} with trace_gaps; explain the most important ones first and suggest which relationships are missing, without changing anything yet." },
    Command { name: "/how-in-ea", args: "<what you want to do>", about: "How to do something in your version of Enterprise Architect", request: "How do I do this in Enterprise Architect: {0}? Use ea_howto for my EA version and the known problems; give the exact steps for that version, and offer to do it for me when you can." },
    Command { name: "/bulk-set", args: "<which elements> <what to set>", about: "Change many elements at once: status, stereotype, author, tags, names", request: "Bulk change: {0}. Use bulk_set: first action preview and show me the numbered table before → after; save (action apply) only after I agree, leaving out the rows I name." },
    Command { name: "/to-bin", args: "<elements>", about: "Put elements into the model's recycle bin instead of deleting them", request: "Put {0} into the model's recycle bin (recycle_bin move) instead of deleting; first show where each is used (where_used)." },
    Command { name: "/merge-elements", args: "<duplicate> into <element to keep>", about: "Merge a duplicate into another element: relationships, diagrams and types move over", request: "Merge {0}: find both elements, show what would change with merge_elements action preview, and merge only after I agree (action apply)." },
    Command { name: "/show-hidden-links", args: "<diagram>", about: "Relationships between elements of a diagram that are hidden there", request: "Which relationships between the elements of the diagram {0} are hidden there? Use connector_visibility action hidden and say which of them should probably be visible." },
    Command { name: "/visibility", args: "<diagram> <what to show or hide>", about: "Which relationships a diagram shows – kept automatically", request: "Set the visibility policy of the diagram {0} with connector_visibility (action set; auto when I want it kept automatically), then apply it after I agree." },
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

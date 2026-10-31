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
    Command { name: "/where-used", args: "<element or relationship>", about: "Every use of an element: diagrams, relationships, types, tags", request: "Where is {0} used? Use where_used (an element, or a relationship by its connector_id) and list every use, grouped." },
    Command { name: "/find-orphans", args: "[package]", about: "Elements nothing uses (more than EA's orphans search)", request: "Find the elements that nothing uses in {0} with find_orphans; list them grouped by package and suggest what to do, without changing anything yet." },
    Command { name: "/find-duplicates", args: "[package]", about: "Elements and relationships that are probably duplicates", request: "Find the likely duplicates in {0} with find_duplicates; for each group say which one to keep (the most used) and why, without changing anything yet." },
    Command { name: "/repo-doctor", args: "", about: "Integrity of the repository: broken links, stale rows, duplicate GUIDs, old locks", request: "Check the integrity of this repository with repo_doctor; explain the findings in plain words, most serious first, and say which you can repair (with my approval) and which need EA's Integrity Check or a person." },
    Command { name: "/what-changed", args: "[since: date or days] [author] [package]", about: "What changed in the model, by whom, in business language", request: "What changed in the model {0}? Use what_changed and summarise it in business language: what was added, what was changed, by whom and where; say what it cannot show (relationships, deletions)." },
    Command { name: "/validate-model", args: "[package]", about: "Check a package against the team's rules (naming, tags, relationships, stereotypes)", request: "Check {0} against the team's modelling rules with model_lint (team_rules show tells which rules are in force). Group the findings by rule, most serious first, and offer fixes; if the team has no rules yet, ask me what they should be and save them with team_rules." },
    Command { name: "/explain", args: "<diagram> [for whom: board, CFO, auditor, developer]", about: "Explain a diagram in plain language for a chosen audience", request: "Explain the diagram {0} for its audience in plain business language: look at it (ea_show_diagram) and read its elements and relationships from the model; say what it shows, why it matters, the main flow or structure in a few steps and the open questions; no modelling jargon unless the audience is technical. Offer a one-page summary in Word or speaker notes for PowerPoint." },
    Command { name: "/trace-gaps", args: "[package] [steps]", about: "Requirements nothing realises and design nothing justifies, through several steps", request: "Find the gaps in traceability in {0} with trace_gaps; explain the most important ones first and suggest which relationships are missing, without changing anything yet." },
    Command { name: "/template-doctor", args: "<element or template> [package] [document]", about: "Why an element is missing from a generated document; what an EA template filters", request: "Template doctor: {0}. Use template_doctor – why_missing for an element (with the package, template and the attached DOCX when I gave them), compare for a whole package against a DOCX, explain or list for templates – and tell me in plain words what to change in EA so the document is right." },
    Command { name: "/safety-net", args: "[what to restore | today]", about: "Snapshots before deletions in EA, today's manual changes, restoring a deleted object", request: "Safety net: {0}. Use safety_net – list the snapshots (or today's manual changes) and, when I name what to bring back, restore it from its snapshot (I will approve the save)." },
    Command { name: "/check-sequence", args: "[diagram or package]", about: "Sequence diagrams: missing operations, numbering, empty fragments, unanswered replies", request: "Check the sequence diagrams {0} with sequence_check; list the missing operations first and say what to fix, without changing anything yet." },
    Command { name: "/sequence-from-text", args: "<package> <name> <text or PlantUML>", about: "A sequence diagram from text, bound to the model's elements and operations", request: "Create a sequence diagram from this text with sequence_from_text (participants are the model's existing elements; tell me which operations are missing and ask before adding them): {0}" },
    Command { name: "/import-api", args: "<attached OpenAPI, AsyncAPI or JSON Schema> into <package>", about: "An API specification as a clean model: classes, enumerations, interfaces with operations, a diagram", request: "Import the API specification {0} with import_api: show me the preview first, then save it after I agree." },
    Command { name: "/export-text", args: "<package> | diff <attached .lcmodel>", about: "A text export of a package for Git, or what changed since an earlier export", request: "Text export for Git: {0}. Use export_text – export gives the .lcmodel file; with an attached earlier export, diff explains what changed, most important first." },
    Command { name: "/diagram-as-text", args: "<diagram> [mermaid | plantuml | c4]", about: "A diagram as Mermaid, PlantUML or a C4 view, for Markdown, wikis and Git", request: "Give me the diagram {0} as text with diagram_as_text (Mermaid unless I named PlantUML or C4), in a code block." },
    Command { name: "/test-results", args: "<attached JUnit .xml or .csv> [package]", about: "Test results against requirements: coverage, failing and untested requirements", request: "Match the test results {0} to the requirements with test_results: show the coverage report first, failing requirements first; write the results on the requirements only after I agree." },
    Command { name: "/translate-model", args: "<package> into <language> [keep the original]", about: "Translate names and notes of a package into another language, keeping all ids", request: "Translate the package {0} with translate_model: take the texts portion by portion, translate them, show me the first portion before saving, then save each portion (I approve each save)." },
    Command { name: "/migration-check", args: "[compare <other model>]", about: "Before moving the model or a new EA build: what could break, and the counts after the transfer", request: "Migration check {0}: use migration_check (readiness, or compare when I named another model) and give me a short plan with the risks first." },
    Command { name: "/mdg-check", args: "<technology or attached .xml> [against <other version>]", about: "Problems of an MDG technology, or what changed between two versions", request: "Check the MDG technology {0} with mdg_technologies (action lint, or compare when I named another version); say what to fix first and what would break existing models." },
    Command { name: "/compare-baselines", args: "<package> [from version] [to version]", about: "Two baselines of a package compared with each other, without noise", request: "Compare the baselines of {0} with compare_baselines (list them first if I named no versions) and tell me what changed between them in business language, most important first." },
    Command { name: "/how-in-ea", args: "<what you want to do>", about: "How to do something in your version of Enterprise Architect", request: "How do I do this in Enterprise Architect: {0}? Use ea_howto for my EA version and the known problems; give the exact steps for that version, and offer to do it for me when you can." },
    Command { name: "/bulk-set", args: "<which elements> <what to set>", about: "Change many elements at once: status, stereotype, author, tags, names", request: "Bulk change: {0}. Use bulk_set: first action preview and show me the numbered table before → after; save (action apply) only after I agree, leaving out the rows I name." },
    Command { name: "/to-bin", args: "<elements>", about: "Put elements into the model's recycle bin instead of deleting them", request: "Put {0} into the model's recycle bin (recycle_bin move) instead of deleting; first show where each is used (where_used)." },
    Command { name: "/merge-elements", args: "<duplicate> into <element to keep>", about: "Merge a duplicate into another element: relationships, diagrams and types move over", request: "Merge {0}: find both elements, show what would change with merge_elements action preview, and merge only after I agree (action apply)." },
    Command { name: "/show-hidden-links", args: "<diagram>", about: "Relationships between elements of a diagram that are hidden there", request: "Which relationships between the elements of the diagram {0} are hidden there? Use connector_visibility action hidden and say which of them should probably be visible." },
    Command { name: "/visibility", args: "<diagram> <what to show or hide>", about: "Which relationships a diagram shows – kept automatically", request: "Set the visibility policy of the diagram {0} with connector_visibility (action set; auto when I want it kept automatically), then apply it after I agree." },
    Command { name: "/fill-notes", args: "[package] [types]", about: "Draft descriptions for elements that have none", request: "Draft descriptions for the elements without one in {0}: use elements_without_notes, write one or two plain sentences for each from its context, show them as a numbered list and save the ones I accept with ea_write (update_element notes)." },
    Command { name: "/apply-style", args: "<package or diagrams> by <stereotype | type | status>", about: "One colour scheme for many diagrams, by stereotype, type or status", request: "Colour {0} with apply_style: show the preview (which value gets which colour), apply after I agree, and offer a legend on the main diagram." },
    Command { name: "/context-diagram", args: "<element> [steps] [relationship types]", about: "A new diagram of an element and what is related to it", request: "Make a context diagram of {0} with context_diagram (after I approve), then show it and say what it shows." },
    Command { name: "/exec-view", args: "<diagram>", about: "A presentation copy of a diagram for management: details hidden, colours by type", request: "Make a presentation copy of the diagram {0} with exec_view (after I approve), show it, and offer a short explanation for management." },
    Command { name: "/clone-diagram", args: "<diagram> view | variant | selection", about: "A new view, a variant (AS-IS → TO-BE) or a copy of chosen elements of a diagram", request: "Clone the diagram {0} with clone_diagram: show the preview (what is new and what is shared), then do it after I agree." },
    Command { name: "/stale-pages", args: "", about: "Confluence pages and documents that the model has outgrown", request: "Which pages and documents published from the model are no longer current? Use stale_publications, say what changed for each and offer to publish them again." },
    Command { name: "/import-sheet", args: "<attached .xlsx or .csv> into <package>", about: "Import a sheet as elements: mapped columns, matched to existing ones, preview first", request: "Import the attached sheet {0} with import_sheet: look at its columns, propose the mapping (name, type, notes, tags, relationships), show the preview create / update / no change, and save only after I agree." },
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
/// A read-only command typed as it is (with at most a package, a number of days or an id): run the tool at once,
/// without the AI (LC-389) – the result is shown as it is, and the user can ask the assistant to explain it.
pub fn direct(input: &str) -> Option<(&'static str, serde_json::Value)> {
    use serde_json::json;
    let typed = input.trim();
    let (name, args) = typed.split_once(char::is_whitespace).unwrap_or((typed, ""));
    let args = args.trim();
    let package = || if args.is_empty() { serde_json::Value::Null } else { args.parse::<i64>().map(|n| json!(n)).unwrap_or_else(|_| json!(args)) };
    match name.to_lowercase().replace('_', "-").as_str() {
        "/repo-doctor" if args.is_empty() => Some(("repo_doctor", json!({}))),
        "/stale-pages" if args.is_empty() => Some(("stale_publications", json!({}))),
        "/find-orphans" => Some(("find_orphans", json!({ "package": package() }))),
        "/find-duplicates" => Some(("find_duplicates", json!({ "package": package() }))),
        "/trace-gaps" => Some(("trace_gaps", json!({ "package": package() }))),
        "/what-changed" if args.is_empty() || args.trim_end_matches(['d', 'D']).parse::<u32>().is_ok() || chrono::NaiveDate::parse_from_str(args, "%Y-%m-%d").is_ok() => {
            Some(("what_changed", json!({ "since": args })))
        }
        "/show-hidden-links" => args.parse::<i64>().ok().map(|d| ("connector_visibility", json!({ "action": "hidden", "diagram": d }))),
        _ => None,
    }
}

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
        // LC-389: read-only commands typed plainly run without the AI; with words for the assistant they do not.
        assert_eq!(super::direct("/repo-doctor").unwrap().0, "repo_doctor");
        assert_eq!(super::direct("/find-orphans 385").unwrap().1["package"], 385);
        assert_eq!(super::direct("/what-changed 14").unwrap().1["since"], "14");
        assert!(super::direct("/what-changed this week by Anna").is_none());
        assert!(super::direct("/repo-doctor and explain it").is_none());
        assert!(super::direct("/merge-elements A into B").is_none());
        assert!(super::request("/glossary-add x").is_none(), "its own window");
        assert!(super::request("/nonsense").is_none());
    }
}

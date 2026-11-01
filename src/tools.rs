//! Tools for the model:
//! - `db_query`: reads the model from PostgreSQL or from an EA file (SQLite) (SELECT/WITH only),
//! - `ea_write`: generic write to EA through COM (packages, diagrams, elements, connectors, layout).
//! There are no importers or format-specific logic here: the model decides what to draw
//! (from a photo, a description or a file) and builds the list of operations itself.
//! Writes go through approval according to the mode from the settings (plan / ask / auto).

use crate::config::{AppConfig, ApprovalMode};
use crate::ea::{self, BatchOptions};
use crate::memory;
pub use crate::model_db::{impact_analysis, query_rows};
pub use crate::ops::{check_file_path, normalize_ops, REF_FIELDS};
use crate::model_db::{db_query, operation_usage};
#[cfg(test)]
use crate::model_db::write_word;
use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use std::cell::Cell;


/// The width of attached images in the description (LC-305).
static IMAGE_WIDTH_SCHEMA: std::sync::LazyLock<Value> = std::sync::LazyLock::new(|| {
    json!({ "type": "integer", "description": "Jira Server / Data Center: the width in pixels the images are shown with in the description; omitted = their own size (diagram images are exported at twice the size, sharp). To resize an image already in the description, change its markup there with jira_update_issue: !file.png|width=900!" })
});

/// What the user asked for in a document and where the document covers it (LC-308).
static DOC_CHECKLIST_SCHEMA: std::sync::LazyLock<Value> = std::sync::LazyLock::new(|| {
    json!({ "type": "array", "minItems": 1,
            "items": { "type": "object", "properties": {
                "ask": { "type": "string", "description": "One thing the user asked for, in their words." },
                "where": { "type": "string", "description": "The heading of the section that covers it, exactly as in the markdown (or a phrase quoted from the document)." }
            }, "required": ["ask", "where"] },
            "description": "Every request of the user about this document from the whole conversation (content, sections, each requirement or item they named, sources such as Jira issues or pages, images, diagrams, format, language, length), one item each, with where the document covers it. Write it BEFORE the markdown, then make the markdown cover every item. An item you cannot cover still gets a section marked \"to be completed\" with the reason." })
});

/// The template a document follows (LC-308).
static DOC_TEMPLATE_SCHEMA: std::sync::LazyLock<Value> = std::sync::LazyLock::new(|| {
    json!({ "type": "string", "description": "The name of the template from <templates> that the document follows (read it with read_template first and keep every heading of it, in its order), or \"none\" when no template fits." })
});

/// Normalised heading or phrase: lower case, without #, numbering and punctuation at the ends.
fn doc_key(text: &str) -> String {
    let t = text.trim().trim_start_matches('#').trim();
    let t = t.trim_start_matches(|c: char| c.is_ascii_digit() || c == '.' || c == ')').trim();
    t.trim_matches(|c: char| c.is_ascii_punctuation() || c.is_whitespace()).replace("**", "").to_lowercase()
}

/// Document discipline (LC-308): before a document or page is made, every request of the user has a place in it and
/// every heading of the chosen template is there. Returns a line for the result, or an error telling what is missing.
fn doc_discipline(input: &Value, markdown: &str, ctx: &ToolContext) -> Result<String> {
    let items = input["checklist"].as_array().filter(|a| !a.is_empty()).with_context(|| {
        "Not made: first list in \"checklist\" every request of the user about this document (from the whole conversation: content, sections, each requirement or item they named, sources, images, format) with the section that covers it, then call again."
    })?;
    let text = markdown.to_lowercase();
    let headings: Vec<String> = markdown.lines().filter(|l| l.trim_start().starts_with('#')).map(doc_key).collect();
    let mut missing = Vec::new();
    for item in items {
        let ask = item["ask"].as_str().unwrap_or("").trim();
        let place = doc_key(item["where"].as_str().unwrap_or(""));
        if ask.is_empty() || place.is_empty() || !(headings.iter().any(|h| h.contains(&place)) || text.contains(&place)) {
            missing.push(format!("- {ask} → \"{}\"", item["where"].as_str().unwrap_or("")));
        }
    }
    let mut problems = Vec::new();
    if !missing.is_empty() {
        problems.push(format!("These requests have no place in the document (the section or phrase is not in the markdown):
{}", missing.join("
")));
    }
    let template = input["template"].as_str().unwrap_or("").trim();
    if !template.is_empty() && !template.eq_ignore_ascii_case("none") {
        let content = crate::templates::read(&ctx.cfg.templates_dir, template)?;
        let lacking: Vec<String> = content
            .lines()
            .filter(|l| l.trim_start().starts_with('#'))
            .map(|l| (l.trim().to_string(), doc_key(l)))
            .filter(|(_, k)| !k.is_empty() && !k.contains(['<', '{', '[']))
            .filter(|(_, k)| !headings.iter().any(|h| h.contains(k.as_str())))
            .map(|(l, _)| format!("- {l}"))
            .collect();
        if !lacking.is_empty() {
            problems.push(format!("Headings of the template \"{template}\" missing in the document (keep each, in order; mark what you do not know \"to be completed\"):
{}", lacking.join("
")));
        }
    }
    if !problems.is_empty() {
        bail!("Not made yet - complete the document and call again.
{}", problems.join("
"));
    }
    Ok(format!("Checked: {} requests of the user have their place in the document{}.", items.len(), if template.is_empty() || template.eq_ignore_ascii_case("none") { String::new() } else { format!(", all headings of the template \"{template}\" are there") }))
}

/// The attachments of the Jira tools (LC-304).
static ATTACHMENTS_SCHEMA: std::sync::LazyLock<Value> = std::sync::LazyLock::new(|| {
    json!({ "type": "array", "items": { "type": "string" },
            "description": "Files to attach: diagram:ID (an image of the diagram exported from the model), image:FILE_NAME (an image shown in this conversation) or file:FILE_NAME (another file of the conversation, e.g. a document you made)." })
});

/// The questions that are asked in every approval mode (they send something outside the model or run code), by kind.
/// After the first one of a kind, the approval card offers to approve all of that kind until Compass is closed.
const SAVE_SCRIPT_QUESTION: &str = "Save the EA script {0}.{1} ({2}, {3} lines){4}:\n{5}";
const RUN_SCRIPT_QUESTION: &str = "Run the EA script {0}.{1} ({2}) in the open EA window? A script can change anything in the model.\n{3}";
const JIRA_CREATE_QUESTION: &str = "Create a Jira issue in project {0}:\n{1}: {2}\n\n{3}";
const JIRA_ADD_QUESTION: &str = "Add to the Jira issue {0}:\n{1}";
const JIRA_UPDATE_QUESTION: &str = "Change the Jira issue {0}:\n{1}";
const JIRA_ATTACH_QUESTION: &str = "Attach to the Jira issue {0}:\n{1}";
const CONFLUENCE_REPLACE_QUESTION: &str = "Publishing to Confluence will REPLACE the content of an existing page:\n\"{0}\" (ID {1}, version {2}) in space {3}.\nThe previous version stays in the page history.";
const CONFLUENCE_NEW_QUESTION: &str = "Publishing a new page to Confluence:\n\"{0}\" in space {1}";
const MEMORY_SAVE_QUESTION: &str = "Save a note to the assistant's memory:\n\"{0}\"\n{1}";
const MEMORY_REMOVE_QUESTION: &str = "Remove the note \"{0}\" from the assistant's memory";
const MCP_QUESTION: &str = "Call the tool {0} of the MCP server {1}:\n{2}";
pub const GDRIVE_UPLOAD_QUESTION: &str = "Upload {0} to Google Drive: {1}";
pub const SHAREPOINT_UPLOAD_QUESTION: &str = "Upload {0} to {1}: {2}/{3}";

/// Kinds of always-asked actions: (kind, its questions, the button of the approval card).
const TRUSTABLE: &[(&str, &[&str], &str)] = &[
    ("script", &[SAVE_SCRIPT_QUESTION, RUN_SCRIPT_QUESTION], "Approve all scripts until Compass is closed"),
    ("jira", &[JIRA_CREATE_QUESTION, JIRA_ADD_QUESTION, JIRA_UPDATE_QUESTION, JIRA_ATTACH_QUESTION], "Approve all Jira issues until Compass is closed"),
    ("confluence", &[CONFLUENCE_REPLACE_QUESTION, CONFLUENCE_NEW_QUESTION], "Approve all Confluence publishing until Compass is closed"),
    ("memory", &[MEMORY_SAVE_QUESTION, MEMORY_REMOVE_QUESTION], "Approve all memory notes until Compass is closed"),
    ("mcp", &[MCP_QUESTION], "Approve all MCP tool calls until Compass is closed"),
    ("upload", &[GDRIVE_UPLOAD_QUESTION, SHAREPOINT_UPLOAD_QUESTION], "Approve all uploads until Compass is closed"),
];

fn trusted() -> &'static std::sync::Mutex<std::collections::HashSet<&'static str>> {
    static SET: std::sync::OnceLock<std::sync::Mutex<std::collections::HashSet<&'static str>>> = std::sync::OnceLock::new();
    SET.get_or_init(Default::default)
}

/// The kind of an approval question and the label of its "approve all" button (in the interface language).
pub fn trustable_kind(summary: &str) -> Option<(&'static str, &'static str)> {
    TRUSTABLE.iter().find_map(|(kind, questions, button)| {
        questions
            .iter()
            .any(|q| {
                let prefix = crate::i18n::tr(q).split("{0}").next().unwrap_or("").to_string();
                !prefix.trim().is_empty() && summary.starts_with(&prefix)
            })
            .then(|| (*kind, crate::i18n::tr(button)))
    })
}

/// The user approved all actions of this kind until Compass is closed.
pub fn trust(kind: &'static str) {
    if let Ok(mut set) = trusted().lock() {
        set.insert(kind);
    }
}

fn is_trusted(kind: &str) -> bool {
    trusted().lock().map(|set| set.contains(kind)).unwrap_or(false)
}

/// The builds of EA scripts the user approved while Compass runs (see `Approver::confirm_script`).
fn approved_scripts() -> &'static std::sync::Mutex<std::collections::HashSet<String>> {
    static SET: std::sync::OnceLock<std::sync::Mutex<std::collections::HashSet<String>>> = std::sync::OnceLock::new();
    SET.get_or_init(Default::default)
}

/// Words that make a string literal code rather than data: SQL statements (Repository.Execute, SQLQuery) and what runs
/// text as code or a program. A literal with one of them keeps its content in the build (LC-346).
const CODE_WORDS: &[&str] = &[
    "insert", "update", "delete", "merge", "upsert", "truncate", "alter", "create", "drop", "grant", "revoke", "exec", "execute",
    "replace", "select", "into", "commit", "rollback", "declare", "waitfor", "shutdown", "eval", "wscript", "cscript", "shell",
    "activexobject", "createobject", "powershell", "cmd", "run",
];

/// Whether a string literal's content holds one of CODE_WORDS as a whole word (any letter case).
fn code_like(content: &str) -> bool {
    content.split(|c: char| !(c.is_alphanumeric() || c == '_')).any(|w| CODE_WORDS.iter().any(|k| w.eq_ignore_ascii_case(k)))
}

/// The build of a script: its code without the content of string literals and without numbers, whitespace collapsed.
/// Two scripts with the same build differ only in their data. A literal that looks like code (SQL, eval, a shell) keeps
/// its content: Repository.Execute("UPDATE …") and Repository.Execute("DELETE …") are two builds, not one (LC-346).
pub fn script_shape(code: &str) -> String {
    let mut out = String::new();
    let mut chars = code.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' | '\'' => {
                let mut content = String::new();
                while let Some(d) = chars.next() {
                    if d == '\\' {
                        if let Some(e) = chars.next() {
                            content.push(d);
                            content.push(e);
                        }
                    } else if d == c {
                        break;
                    } else {
                        content.push(d);
                    }
                }
                out.push(c);
                if code_like(&content) {
                    out.push_str(&content.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase());
                }
                out.push(c);
            }
            c if c.is_ascii_digit() && !out.chars().last().is_some_and(|l| l.is_alphanumeric() || l == '_') => {
                while chars.peek().is_some_and(|d| d.is_ascii_digit() || *d == '.') {
                    chars.next();
                }
                out.push('0');
            }
            c if c.is_whitespace() => {}
            c => out.push(c),
        }
    }
    // Data of another length has the same build: a run of repeated entries (after the texts and numbers are gone, e.g.
    // `0:"",0:"",0:""` or `{id:0,a:""},{id:0,a:""}`) collapses into one entry.
    let mut parts: Vec<&str> = out.split(',').collect();
    let mut changed = true;
    while changed {
        changed = false;
        for k in 1..=8 {
            let mut i = 0;
            while i + 2 * k <= parts.len() {
                if parts[i..i + k] == parts[i + k..i + 2 * k] {
                    parts.drain(i + k..i + 2 * k);
                    changed = true;
                } else {
                    i += 1;
                }
            }
        }
    }
    parts.join(",")
}

/// Write approval. `ask` asks the user in a dialog and waits for the answer (called from the worker thread).
pub struct Approver {
    pub mode: ApprovalMode,
    pub ask: Box<dyn Fn(String) -> bool + Send>,
    /// `plan` mode: after the first approval, further writes in the same answer no longer ask.
    approved_in_turn: Cell<bool>,
}

impl Approver {
    pub fn new(mode: ApprovalMode, ask: Box<dyn Fn(String) -> bool + Send>) -> Self {
        Self { mode, ask, approved_in_turn: Cell::new(false) }
    }

    /// Asks the user (the answer is counted in the usage statistics: asked, accepted, rejected).
    fn ask_user(&self, summary: &str) -> bool {
        let accepted = (self.ask)(summary.to_string());
        crate::telemetry::approval(accepted);
        accepted
    }

    fn confirm(&self, summary: &str) -> Result<()> {
        let must_ask = match self.mode {
            ApprovalMode::Auto => false,
            ApprovalMode::Ask => true,
            ApprovalMode::Plan => !self.approved_in_turn.get(),
        };
        if must_ask && !self.ask_user(summary) {
            bail!("{}", crate::i18n::tr("the user rejected this change; nothing was saved"));
        }
        self.approved_in_turn.set(true);
        Ok(())
    }

    /// Destructive changes (deleting, moving, copying whole packages) ask also in `plan` mode after an earlier
    /// approval in the same answer: the plan the user approved may not have shown them.
    fn confirm_destructive(&self, summary: &str) -> Result<()> {
        if self.mode != ApprovalMode::Auto && !self.ask_user(summary) {
            bail!("{}", crate::i18n::tr("the user rejected this change; nothing was saved"));
        }
        self.approved_in_turn.set(true);
        Ok(())
    }

    /// Consent regardless of the mode (also in auto) for a risky change of the model (deleting elements with relationships).
    fn confirm_risky(&self, summary: &str) -> Result<()> {
        if !self.ask_user(summary) {
            bail!("{}", crate::i18n::tr("the user rejected this change; nothing was saved"));
        }
        self.approved_in_turn.set(true);
        Ok(())
    }

    /// Consent for sending a file outside the application (an upload): always asked, unless the user approved all uploads.
    pub fn confirm_upload(&self, summary: &str) -> Result<()> {
        self.confirm_kind("upload", summary)
    }

    /// An always-asked action of a kind the user can approve once for all (see `TRUSTABLE`).
    pub fn confirm_kind(&self, kind: &str, summary: &str) -> Result<()> {
        if is_trusted(kind) {
            return Ok(());
        }
        self.confirm_always(summary)
    }

    /// Consent for an EA script (saving or running). Once the user approved a script, scripts of the same build (the same
    /// code with other texts and numbers in it, e.g. the next batch of aliases) are approved without asking again while
    /// Compass runs; anything else in the code asks again.
    fn confirm_script(&self, action: &str, code: &str, summary: &str) -> Result<()> {
        if is_trusted("script") {
            return Ok(());
        }
        let shape = format!("{action}:{}", script_shape(code));
        if approved_scripts().lock().map(|s| s.contains(&shape)).unwrap_or(false) {
            return Ok(());
        }
        self.confirm_always(summary)?;
        if let Ok(mut set) = approved_scripts().lock() {
            set.insert(shape);
        }
        Ok(())
    }

    /// Consent regardless of the mode (also in auto): for actions that send content outside the application and the model.
    fn confirm_always(&self, summary: &str) -> Result<()> {
        if !self.ask_user(summary) {
            bail!("{}", crate::i18n::tr("the user rejected this operation; nothing was sent"));
        }
        Ok(())
    }
}

/// Context of tool calls within one model answer.
pub struct ToolContext<'a> {
    pub cfg: &'a AppConfig,
    pub approver: &'a Approver,
}

/// Tool definitions in the Messages API format.
pub fn definitions() -> Value {
    let mut list = definitions_core();
    if let Some(items) = list.as_array_mut() {
        items.extend(crate::model_db::read_only_definitions());
        items.push(crate::changelog::definition());
        items.push(crate::miro::definition());
        items.push(crate::matrix::definition());
        items.extend(crate::model_usage::definitions());
        items.push(crate::repo_doctor::definition());
        items.push(crate::changes::definition());
        items.push(crate::governance::rules_definition());
        items.push(crate::trace_gaps::definition());
        items.push(crate::template_doctor::definition());
        items.push(crate::safety_net::definition());
        items.extend(crate::sequence::definitions());
        items.push(crate::api_import::definition());
        items.push(crate::text_export::definition());
        items.push(crate::notation::definition());
        items.push(crate::test_results::definition());
        items.push(crate::translate_model::definition());
        items.push(crate::migration::definition());
        items.push(crate::baseline_diff::definition());
        items.extend(crate::readonly_why::definitions());
        items.push(crate::what_if::definition());
        items.extend(crate::mermaid_ddl::definitions());
        items.push(crate::ea_help::definition());
        items.push(crate::bulk::definition());
        items.push(crate::recycle::definition());
        items.push(crate::merge::definition());
        items.push(crate::visibility::definition());
        items.push(crate::style::definition());
        items.push(crate::context_diagram::definition());
        items.push(crate::diagram_clone::definition());
        items.push(crate::publications::definition());
        items.push(crate::import_sheet::definition());
        items.push(crate::jira_sync::definition());
        items.push(json!({
            "name": "exec_view",
            "description": "A presentation copy of a diagram for management or business (the original stays as it is): a new diagram next to it with \
the same elements in the same places, attributes, operations and stereotypes hidden, relationship labels hidden, elements coloured by type, \
optionally a title. One save the user approves (the copy can be deleted or undone). Use it for /exec-view; offer a short explanation (/explain) or \
a slide of the copy with speaker notes (create_presentation, notes).",
            "input_schema": { "type": "object", "properties": {
                "diagram": { "type": "integer" },
                "name": { "type": "string", "description": "Name of the copy (default: <name> – presentation)" }
            }, "required": ["diagram"] }
        }));
        items.push(json!({
            "name": "align_elements",
            "description": "Line up elements of a diagram without moving the others: align left / right / top / bottom / center (horizontal centres) / \
middle (vertical centres) to the first element given, distribute_horizontally / distribute_vertically (equal gaps between the outermost ones), \
same_size (the size of the first one), snap (to a 10 px grid). One save the user approves; undoable. Elements = object_ids on that diagram.",
            "input_schema": { "type": "object", "properties": {
                "diagram": { "type": "integer" },
                "elements": { "type": "array", "items": { "type": "integer" } },
                "how": { "type": "string", "enum": ["left", "right", "top", "bottom", "center", "middle", "distribute_horizontally", "distribute_vertically", "same_size", "snap"] }
            }, "required": ["diagram", "elements", "how"] }
        }));
        items.push(json!({
            "name": "place_new_elements",
            "description": "Incremental layout: places new elements of a diagram in free space next to an element they are related to (or the \
nearest one), and moves nothing else – unlike layout_diagram, which rearranges the whole diagram. Default: the elements that lie on top of \
others (e.g. just added at the same spot); or give their object_ids. One save the user approves; undoable.",
            "input_schema": { "type": "object", "properties": {
                "diagram": { "type": "integer" },
                "elements": { "type": "array", "items": { "type": "integer" } },
                "spacing": { "type": "integer", "description": "Gap in pixels (default 40)" }
            }, "required": ["diagram"] }
        }));
        items.push(json!({
            "name": "change_connector_type",
            "description": "Give relationships another type (e.g. ArchiMate Composition → Serving, Association → Dependency), also many at once: a new \
relationship between the same ends with the old name, direction, notes, tagged values and stereotype (or the given one) gets the old route, style and \
visibility on every diagram, and the old one is deleted. One save the user approves (a baseline first: not undoable by operations); the new relationship \
is checked by the ArchiMate 3.2 and team rules. Use connector ids from the model.",
            "input_schema": { "type": "object", "properties": {
                "connector": { "type": "integer" },
                "connectors": { "type": "array", "items": { "type": "integer" } },
                "type": { "type": "string", "description": "EA connector type, e.g. Dependency, Realisation, Association, Aggregation, InformationFlow" },
                "stereotype": { "type": "string", "description": "e.g. ArchiMate3::ArchiMate_Serving (empty keeps the old one)" }
            }, "required": ["type"] }
        }));
        items.push(crate::scripts::definition());
        items.push(crate::traceability::definition());
        items.push(crate::code_drift::definition());
        items.extend(crate::code_read::definitions());
        items.push(crate::cmdb::definition());
        items.push(crate::mdg::definition());
        items.push(crate::sharepoint::definition());
        items.push(crate::sharepoint::onedrive_definition());
        items.push(crate::gdrive::definition());
        items.push(crate::instructions::definition());
        items.push(crate::documents::definition());
        items.push(crate::mdg::version_control_definition());
        items.push(crate::ea_api::definition());
        items.push(crate::undo::definition());
        items.push(crate::estimate::definition());
        items.push(json!({
            "name": "propose_change",
            "description": "Proposes a change of the model to the team instead of saving it: the operations (as for ea_write) with a summary and a before → after preview \
go to the team hub as a change set for review; the team comments and an architect approves it in the hub's web view. Use it when the user asks to propose, \
submit for review or prepare a change for approval. Nothing is changed in the model now.",
            "input_schema": { "type": "object", "properties": {
                "title": { "type": "string" },
                "ops": { "type": "array", "items": { "type": "object" }, "description": "Operations as for ea_write." }
            }, "required": ["title", "ops"] }
        }));
        items.push(json!({
            "name": "change_sets",
            "description": "Change sets in the team hub: action list (optionally status pending, approved, rejected, applied), show (id: summary, preview, comments, decision) \
or apply (id: an APPROVED change set of this model is saved to the model with the usual approval and verification; the hub records it as applied).",
            "input_schema": { "type": "object", "properties": {
                "action": { "type": "string", "enum": ["list", "show", "apply"] },
                "id": { "type": "string" },
                "status": { "type": "string" }
            }, "required": ["action"] }
        }));
        items.push(json!({
            "name": "review_comments",
            "description": "Review comments and decisions (ADR) the team left on elements of this model in the hub's web view (LC-392). action list \
(optionally element = object_id, status open/resolved/proposed/accepted/rejected): who said what about which element; action add (element, text, \
kind comment or decision); action status (id, status). Review requests (LC-407): action request asks the owners (tagged value Owner, else the \
author; or to = a hub user) to confirm or update elements (element, elements or package; text = the question; architect role); action requests \
lists the ones waiting for the user (all: true – everybody's, status any for the answered ones too); action answer (id, status confirmed or \
needs_change, text, kind decision to record the answer as a decision). Use it when the user asks what reviewers said, to answer open comments, to \
record a decision about an element, or to ask owners to review their elements.",
            "input_schema": { "type": "object", "properties": {
                "action": { "type": "string", "enum": ["list", "add", "status", "request", "requests", "answer"] },
                "element": { "type": "integer" },
                "elements": { "type": "array", "items": { "type": "integer" } },
                "package": { "type": "integer" },
                "to": { "type": "string" },
                "all": { "type": "boolean" },
                "text": { "type": "string" },
                "kind": { "type": "string", "enum": ["comment", "decision"] },
                "id": { "type": "string" },
                "status": { "type": "string" }
            }, "required": ["action"] }
        }));
        items.push(json!({
            "name": "spell_check",
            "description": "Spelling of the model's texts in the language of the conversation (the language of Compass), with the Windows dictionary: \
notes of elements and diagrams, scenario steps and the project glossary (EA's own spell checker has only English dictionaries). Names of elements \
and glossary terms count as known words. Returns the possible mistakes with suggestions; correct them only after the user agrees (ea_write: update_element notes, update_scenario).",
            "input_schema": {
                "type": "object",
                "properties": { "package": { "type": "integer", "description": "Optionally only this package with subpackages (package_id); without it the whole model." } }
            }
        }));
    }
    list
}

fn definitions_core() -> Value {
    json!([
        {
            "name": "ea_write",
            "description": "Writes to the Enterprise Architect model through COM. Runs a list of operations in the given order, in a single EA session. \
You refer to an object created earlier in the same list by its key with a hash sign (e.g. \"#pkg\"), and to an object that already exists in the model \
(including one created in a previous call) by its numeric id. The result returns the ids of all created objects. \
Operations (field op): \
create_package {key, parent, name, notes?}; \
create_diagram {key, package | parent_element, name, type} (type e.g. \"Logical\", \"Component\", \"Sequence\", \"Use Case\", \"Activity\", \"Custom\", \"BPMN2.0::Business Process\"); \
create_element {key, package | parent_element, name, type, stereotype?, notes?, tags?, color?, keywords?} (type is the UML type: Class, Component, Interface, Actor, UseCase, Requirement, Activity, Event, Decision, Note, Boundary...; parent_element nests the element inside another one, e.g. the contents of a subprocess); \
a fork/join is type Synchronization placed as a thin bar (e.g. 8x80 vertical, 80x8 horizontal; the application sets EA's orientation from the size); create_element also takes subtype: for type StateNode 3 = initial state, 4 = final state, 11 = choice, 100 / 101 = initial / final node of an activity; \
create_connector {source, target, type, stereotype?, name?, tags?, diagram?, sequence?, message_sort?, is_return?, subtype?, constraints?} (constraints = [{name, type?}] UML constraints of the connector, e.g. a time or duration constraint of a message \"{< 2s}\" (type Invariant unless given); type: Association, Aggregation, Dependency, Generalization, Realisation, ControlFlow, InformationFlow, Sequence...; aggregation and composition are the connector type Aggregation (never an Association with a stereotype): source = the part, target = the whole (the diamond is at the whole), subtype Weak = shared aggregation (default), Strong = composition; a message of a sequence diagram: type Sequence, diagram = that diagram, sequence = its number in the order, message_sort Synchronous or Asynchronous, is_return true for a reply; EA lays messages out from top to bottom by that number); \
place {diagram, element, left, top, width, height, bgcolor?} (bgcolor is the fill color on this diagram, written as #RRGGBB (e.g. #FFE599); pixels; the top-left corner of the diagram is 0,0, the Y axis grows downwards; place a frame that contains other elements before its contents); \
update_element {element, name?, notes?, alias?, stereotype?, clear_stereotype?, status?, version?, author?, phase?, complexity?, abstract?, tags?, color?, keywords?} (keywords = the Keywords field in EA, t_object.pdata5 in the database, a text or a list; used, among other things, by legends for coloring; complexity is '1' easy, '2' medium, '3' difficult; abstract is true or false; a tag with the value null is removed); \
update_connector {connector, name?, stereotype?, clear_stereotype?, direction?, notes?, source_multiplicity?, source_role?, target_multiplicity?, target_role?, tags?, sequence?, message_sort?, is_return?} (properties of an existing relationship, connector = t_connector.connector_id; direction: 'Unspecified', 'Source -> Destination', 'Destination -> Source' or 'Bi-Directional'; multiplicities e.g. '1', '0..1', '1..*'); create_legend {diagram, title?, filter?, entries, connector_filter?, connector_entries?, left?, top?} (a diagram legend with automatic coloring, as in EA: filter = the element property the legend colors by, Element.Keyword by default, others e.g. Element.Stereotype, Element.Status, Element.Type; entries = [{value, color #RRGGBB, label?}] the fill color of elements with that value; connector_filter e.g. Connector.Stereotype and connector_entries = [{value, color, width?}] the line color of connectors; elements are colored automatically once they have the value set, e.g. the keyword prio1; in the database a legend is a t_object of type Text with ntype 76); \
move {package, element | diagram | child_package} (moves an existing element, diagram or package into the package given in package; connectors and positions on diagrams are kept, so never recreate objects just to change their package); \
delete {element | diagram | connector | package} (permanently deletes an object from the model; an element takes its connectors and diagram occurrences with it, a package all of its contents; delete only what the user asked for or what you created by mistake yourself, and tell the user what you deleted; take the connector id from t_connector.connector_id); \
remove_from_diagram {diagram, element} (removes the element from the diagram, the element stays in the model); \
reverse_connector {connector} (swaps the source and the target of a relationship); \
update_package {package, name?, notes?, version?, status?, stereotype?}; \
version_control {package, action, comment?} (version control of a package that is already under version control in EA: action check_out, check_in, get_latest (replaces the package with the version from version control), put_latest (checks in and keeps it checked out) or resync (reads the version control status of the package again); comment for check_out, check_in and put_latest; always asked; call version_control_status first); \
generate_report {package, file, template?} (a document from an EA document template, like Publish → Report Builder: file = a file name ending .docx, .pdf or .rtf (kept with the conversation's files; give only the name unless the user asked for a folder) or a full path the user named; template = the name of an EA template, default \"Model Report\"; use it when the user wants EA's own report format or a company template; for documents you write yourself use create_document); \
layout_diagram {diagram, direction?, spacing?} (EA's automatic layout of the whole diagram, like Layout → Diagram Layout: direction down (default), right, up or left; spacing in pixels, default 40; use it after creating a diagram with many elements or when the user asks to tidy a diagram; then check the result with ea_show_diagram); \
export_xmi {package, file} (exports the package with its contents and diagrams to an XMI file; file = a full path ending .xml or .xmi, by default in the attachments folder); \
import_xmi {package, file, keep_guids?} (imports an XMI file into the package; GUIDs are replaced unless keep_guids is true, so an import never overwrites existing objects; only on the user's request); \
set_linked_document {element, text} (the element's linked document (Ctrl+Alt+D in EA), from plain text, one paragraph per line; replaces an existing one); \
set_swimlanes {diagram, lanes: [{title, width?, color?}], orientation?} (diagram swimlanes (Diagram → Swimlanes), replaces existing ones; orientation vertical (default) or horizontal); \
set_composite {element, diagram} (links the element to an existing diagram as its composite diagram (in EA: New Child Diagram → Composite → Select Composite Diagram): when the element is double-clicked, EA opens that diagram instead of the properties; this is how you link a BPMN subprocess to the subprocess diagram, also when the diagram is in an ordinary package; in the database the linked element has t_object.ntype = 8 and the diagram id in the pdata1 column); \
create_attribute {element, name, type?, visibility?, notes?, default?, stereotype?, alias?} (a class attribute, visibility: Public|Private|Protected|Package); create_operation {element, name, return_type?, visibility?, notes?, stereotype?, alias?, parameters?: [{name, type}]} (a class operation with parameters; alias is the operation's alias in EA, e.g. the REST path of an endpoint: set it here, no script is needed); update_attribute {attribute, name?, type?, visibility?, notes?, default?, stereotype?, alias?, static?} and delete_attribute {attribute} (attribute = t_attribute.id); update_operation {operation, name?, return_type?, visibility?, notes?, stereotype?, alias?, static?} and delete_operation {operation} (operation = t_operation.operationid); clone_package {package, name?, key?} (a copy of the package with all its contents next to the original, e.g. a variant to experiment with); save_document_template {name, package_part, element_part, diagram_part?} (an EA document template for Publish → Report Builder, made e.g. from a document the user gave: each part is Markdown (# headings, - lists, **bold**) with EA fields in braces; package_part is written once per package ({Pkg.Name}, {Pkg.Notes}, {Pkg.Version}…), diagram_part for each diagram ({Diagram.Name}, {Diagram.Notes}, {Diagram.DiagramImg} the picture), element_part for each element ({Element.Name}, {Element.Type}, {Element.Stereotype}, {Element.Notes}, {Element.Status}, {Element.Version}, {Element.Author}…; the value of one tagged value is {Element.valueOf(TagName)}, e.g. {Element.valueOf(Owner)}: there is no field for all tagged values, so name each tag the document shows); a template with the same name is replaced; then generate_report with template = that name makes the document); create_matrix_specification {package, name, source_package, target_package, source_type, target_type, link_type?, direction?, children?, key?} (EA's own Relationship Matrix saved in the model as a Matrix Specification artifact in `package`: the user double-clicks it in the Browser and EA shows the live matrix; source_type / target_type: element types of rows and columns, e.g. Requirement, UseCase, Component; link_type: e.g. Realisation (default), Dependency, Association, Abstraction for trace; direction: source_to_target (default), target_to_source or both; children: include child packages, default true); clone_element_version {element, version?, name?, key?} (EA's former \"Clone as New Version\": a copy of the element in the same package with its notes, attributes, operations with parameters and tagged values, the next version number (or version), and a «trace» relationship from the new version to the original, which stays as it is); update_scenario {element, scenario, name?, type?, notes?, steps?} (changes an existing structured scenario found by its name or GUID; steps, when given, replace all steps); delete_scenario {element, scenario}; create_scenario {element, name, type?, notes?, steps: [{text, actor?, uses?, results?}]} (a STRUCTURED SCENARIO of an element, e.g. a use case: the Scenarios tab of the element, with numbered steps; ALWAYS use this, never notes, when the user asks for scenarios, flows or structured scenarios; type: Basic Path (default), Alternate or Exception; actor = true for a step done by the actor, false for the system; in the database t_objectscenarios, steps in its xmlcontent); add_glossary_term {term, type?, meaning, forms?: [..]} (a term in the EA PROJECT GLOSSARY, the list EA highlights in notes and documents with its meaning - not a model element; t_glossary holds the terms; EA highlights only the exact written form, so for a Polish term ALWAYS pass in forms every inflected form of it: singular and plural in all seven cases from the nominative to the vocative, for a multi-word term the whole phrase inflected; type: the one the user names, otherwise Glossary (the type appears in EA once used); ask the user for the meaning if they did not give it); create_baseline {package, version, notes?} (saves a version of the package in the model; ALWAYS make a baseline before deleting from or substantially restructuring an existing package that you did not create in this conversation, and tell the user they can restore it in EA: package in the browser → Manage Baselines → Compare/Restore; baselines are in t_document with doctype = 'Baseline'); \
update_diagram {diagram, name?, notes?, stereotype?} (changes the name, notes or stereotype of an existing diagram); repair_diagram_links {diagram, show_hidden?} \
(connectors not visible on a diagram although both ends are on it. It fixes, in this order: connectors that are not messages but are bound to one \
diagram (t_connector.diagramid <> 0: EA draws them only on that diagram) - set back to 0; duplicate links of one connector in t_diagramlinks (EA uses \
the first, often a hidden one); with show_hidden: true, hidden links whose both ends are on the diagram. When the user says arrows or relationships are \
missing on a diagram, check in this order with db_query: t_connector.diagramid of the connectors between the diagram's elements (non-zero outside \
sequence/communication messages = the cause), then t_diagramlinks rows with hidden = 1 and connectorid with more than one row (select connectorid, \
count(*) from t_diagramlinks where diagramid = N group by connectorid having count(*) > 1); then repair with this operation. An empty geometry or path \
is fine, EA then draws a straight line - never blame it first. Never add or change t_diagramlinks rows or t_connector.diagramid with SQL or an EA \
script: use hide_connectors, route_connector, set_line_style or this operation); \
hide_connectors {diagram, connectors, hidden?} (hides the given connectors on this one diagram only, a list of ids from t_connector.connector_id; hidden=false shows them again; the connectors stay in the model and on other diagrams, so this is how you declutter a dense diagram instead of deleting relationships; in the database: t_diagramlinks.hidden); \
route_connector {diagram, connector, start?, points?, end?} (routes one connector along the given path, e.g. following the user's sketch: start = [x, y] the attachment point on the edge of the source element, end = [x, y] the attachment point on the edge of the target element, points = a list of bend points [[x, y], ...]; coordinates as in place (pixels from the top-left corner of the diagram, Y axis downwards), and take the element positions from t_diagramobjects (rectleft, -recttop, rectright, -rectbottom); for purely horizontal and vertical lines, each successive point (from start through points to end) must have the same x or y coordinate as the previous one; example: from the left side of element B to the left, then up to the bottom side of element A: start at the middle of B's left side, one bend point below the middle of A at the height of start, end at the middle of A's bottom side; connector = t_connector.connector_id); \
set_line_style {diagram, style, connectors?} (the routing of connector lines on the diagram, as in EA Line Style: style = direct (straight) | auto (automatic) | custom (manually bent) | orthogonal_square (orthogonal) | orthogonal_rounded (orthogonal, rounded) | tree_vertical | tree_horizontal | lateral_vertical | lateral_horizontal; connectors = a list of connector ids (t_connector.connector_id), without it all connectors between the diagram's elements get the style; bend points are cleared; in the database the style is in t_diagramlinks.style as Mode= and TREE=); \
set_bounds {diagram, element, left?, top?, width?, height?} (moves or resizes an element that is already on the diagram; connectors are kept; omitted fields stay unchanged; for enlarging frames, pools, lanes and subprocesses, instead of removing them and placing them again); \
set_color {diagram, element, bgcolor} (the fill color of an element already on the diagram, #RRGGBB (e.g. #FFE599); the element's default color on all diagrams is set by the color field in create_element and update_element); \
diagram_options {diagram, extended?, style?} (display options of a diagram, keys with the value \"1\" or \"0\": extended = EA's ExtendedStyle keys such as ShowSN (sequence numbers on a sequence diagram: EA numbers the messages itself, hierarchically by activation, so a message sent by a lifeline while a call to it is active gets the sub-number, 1, 1.1, 1.2; the numbers are not stored in the messages), HideAtts, HideOps, HideStereo, ShowNotes; style = StyleEx keys such as SuppConnectorLabels); \
navigation_cell {diagram, target, left, top, width?, height?, image?, caption?} (a navigation cell, i.e. a link tile on the diagram given in diagram that opens the diagram given in target when double-clicked; the caption is the name of the target diagram unless you give caption; in the database a cell is a t_object of type Text with the stereotype NavigationCell, the caption is in the alias column and the target diagram (diagram_id) in the pdata1 column; you change the caption of an existing cell with update_element {alias}; default size 170x115; the icon is chosen automatically for the target diagram type, give the image field only when explicitly asked to; build start pages and model tables of contents from such cells laid out in an even grid, e.g. every 200 px horizontally and 140 px vertically). \
BPMN 2.0: task = type Activity + stereotype BPMN2.0::Activity (tags taskType: Abstract|Manual|User|Service|Script|Send|Receive|BusinessRule; subprocess: tag activityType=Sub-Process); \
embedded subprocess (expanded, with its steps visible inside): a subprocess element without set_composite, you create its steps with parent_element = the subprocess \
(start, tasks, end and the SequenceFlow flows between them) and place them on the same diagram inside the subprocess rectangle (first place the subprocess, then the steps; \
the subprocess must be large enough for the steps to fit with a margin, e.g. 600x200, and move the elements after it to the right with set_bounds); \
collapsed subprocess (steps on a separate diagram): the steps in a separate diagram and set_composite of the subprocess with that diagram; \
events = type Event + stereotype BPMN2.0::StartEvent | BPMN2.0::IntermediateEvent | BPMN2.0::EndEvent (tag eventDefinition: None|Message|Timer|Signal|Error|Conditional|Escalation|Terminate; intermediate: tag catchOrThrow=Catch|Throw); \
gateway = type Decision + stereotype BPMN2.0::Gateway (tag gatewayType: Exclusive|Parallel|Inclusive|Event|Complex); \
flow = type ControlFlow + stereotype BPMN2.0::SequenceFlow; pool/lane = type ActivityPartition + stereotype BPMN2.0::Pool | BPMN2.0::Lane. \
Typical sizes: task 110x60, event 30x30, gateway 42x42. \
Layout: if the source (file, photo) gives positions, reproduce them; otherwise lay the elements out legibly from left to right, \
at least 50 px apart and without overlapping. Never put everything at a single point. \
Save large diagrams in several calls of about 40 operations each.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "ops": {
                        "type": "array",
                        "items": { "type": "object", "properties": { "op": { "type": "string" } }, "required": ["op"] }
                    },
                    "expect_unchanged": {
                        "type": "array",
                        "description": "Optimistic locking: existing objects that must not have changed since you read them. Give the modifieddate read with db_query.",
                        "items": {
                            "type": "object",
                            "properties": {
                                "kind": { "type": "string", "enum": ["package", "element", "diagram"] },
                                "id": { "type": "integer" },
                                "modified": { "type": "string" }
                            },
                            "required": ["kind", "id", "modified"]
                        }
                    }
                },
                "required": ["ops"]
            }
        },
        {
            "name": "ea_show_diagram",
            "description": "Fetches a diagram from EA as an image (PNG, exactly as EA draws it). You receive the image and at the same time the user sees it in the chat, \
with the option to save it to disk. Use it when the user wants to see or download a diagram, when you need to describe what a diagram shows, \
and after your own drawing, to check the result and fix the layout. Take the diagram id from db_query (t_diagram.diagram_id).",
            "input_schema": {
                "type": "object",
                "properties": { "diagram_id": { "type": "integer" } },
                "required": ["diagram_id"]
            }
        },
        {
            "name": "render_image",
            "description": "Draws an image from SVG and shows it as a PNG in the chat (the user can save it), and you see it too. \
Use it when the user wants an image of something that is not an EA diagram: a matrix (e.g. UC x requirements), a table, a chart, a legend, a schematic. \
Write a complete SVG with width and height in pixels, a background and legible text (Segoe UI font; accented characters such as Polish letters work); \
matrix cells as rectangles with text, bold headers (font-weight=\"bold\"), wrap or shorten long names so that they fit. \
After drawing, look at the result and fix it if anything overlaps.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "svg": { "type": "string", "description": "A complete SVG document." },
                    "name": { "type": "string", "description": "A short file name, e.g. \"UC-FR matrix\"." }
                },
                "required": ["svg", "name"]
            }
        },
        {
            "name": "create_presentation",
            "description": "Creates a PowerPoint presentation (PPTX, 16:9) and shows it in the chat: a title slide, then slides with a title, bullet points (two levels) and/or a picture \
(diagram:ID for a diagram from EA, image:NAME for an image drawn with render_image; with bullets the picture goes on the right). pdf: true also makes a PDF with Microsoft PowerPoint. \
Use it when the user asks for slides or a presentation; keep slides short (at most 6 bullets). The title slide counts: when the user asks for N slides, give N-1 slides here, so the file has exactly N.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "file_name": { "type": "string" },
                    "title": { "type": "string" },
                    "subtitle": { "type": "string" },
                    "slides": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "title": { "type": "string" },
                                "bullets": { "type": "array", "items": { "anyOf": [{ "type": "string" }, { "type": "object", "properties": { "text": { "type": "string" }, "level": { "type": "integer" } }, "required": ["text"] }] } },
                                "image": { "type": "string", "description": "diagram:ID or image:NAME" },
                                "notes": { "type": "string", "description": "Speaker notes: what to say on this slide (lines become paragraphs)" }
                            },
                            "required": ["title"]
                        }
                    },
                    "pdf": { "type": "boolean" }
                },
                "required": ["file_name", "title", "slides"]
            }
        },
        {
            "name": "create_document",
            "description": "Creates a Word document (DOCX) and shows it to the user in the chat with the \"Open\" and \"Save as…\" buttons. \
Content in Markdown, laid out in a polished style (coloured headings, tables with a navy header and striped rows): # to #### headings, paragraphs, \
lists (- item, 1. item, - [ ] / - [x] checkboxes), tables (| a | b | with a |---|---| row), > quotes or notes (a highlighted box), --- separator, \
code blocks in ``` fences with a language (```json, ```http, ```xml, ```sql…): every line and its indentation is kept exactly, monospaced in a grey box, \
JSON in colour - write requests and responses the way Postman shows them (method and URL, headers, pretty-printed JSON body). \
Inline: **bold**, *italic*, `code`, ~~strike~~, ==highlight==, [text](https://…) links, and colours with HTML tags: \
<span style=\"color:red\">…</span> (any colour name or #RRGGBB), <span style=\"background:yellow\">…</span>, <u>…</u>, and for marking changes \
<del>removed text</del> (red, struck through) and <ins>added text</ins> (green, underlined). When the user asks for colours, changes in red or other \
formatting, use these: never tell the user Word cannot do it. \
Images on separate lines: ![caption](diagram:ID) inserts a diagram from EA (id from t_diagram.diagram_id), ![caption](image:NAME) inserts an image \
drawn earlier with render_image (NAME from its result). Use it when the user asks for a document, description, specification or report for Word.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "file_name": { "type": "string", "description": "File name without extension." },
                    "title": { "type": "string", "description": "Title at the beginning of the document (optional)." },
                    "markdown": { "type": "string", "description": "Document content in Markdown." },
                    "format": { "type": "string", "enum": ["docx", "pdf", "both"], "description": "docx (default), pdf (made with Microsoft Word on this computer) or both." },
                    "checklist": DOC_CHECKLIST_SCHEMA.clone(),
                    "template": DOC_TEMPLATE_SCHEMA.clone(),
                },
                "required": ["file_name", "markdown", "checklist", "template"]
            }
        },
        {
            "name": "ea_ui",
            "description": "Works with the Enterprise Architect window that the user has open (the same model): \
action=context returns the open diagram, the elements and the connector selected on it, and the selection in the project browser \
(use it when the user says \"this\", \"the selected\", \"this diagram\", \"here\"); action=open_diagram {diagram} opens the diagram in the user's EA window; \
action=show_element {element} shows the element in the project browser; action=open_diagram with element too opens the diagram with that \
element selected (after where_used: \"show me where it is\"). After every ea_write the changed diagrams refresh \
in the user's window automatically, so don't ask the user to press F5 (unless the EA window was closed or shows a different model).",
            "input_schema": {
                "type": "object",
                "properties": {
                    "action": { "type": "string", "enum": ["context", "open_diagram", "show_element"] },
                    "diagram": { "type": "integer" },
                    "element": { "type": "integer" }
                },
                "required": ["action"]
            }
        },
        {
            "name": "jira_search",
            "description": "Searches Jira issues with JQL (Jira from the settings) and returns key, type, status, priority, assignee, summary, labels and update date. \
Use it when the user refers to the backlog, a sprint, an epic or issues, e.g. project = ARCH AND status != Done ORDER BY updated DESC, or text ~ \"payment\". \
Read single issues with jira_issue.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "jql": { "type": "string", "description": "JQL query." },
                    "max": { "type": "integer", "description": "At most this many issues, 1-100 (default 30)." }
                },
                "required": ["jql"]
            }
        },
        {
            "name": "jira_issue",
            "description": "Reads one Jira issue: summary, type, status, priority, assignee, parent, labels, components, versions, the full description, \
linked issues, web links (also links to model elements) and the latest comments. Use it as context: e.g. draw a use case, component or BPMN diagram \
from a story, check whether the model covers an epic, or find the model elements an issue is about.",
            "input_schema": {
                "type": "object",
                "properties": { "key": { "type": "string", "description": "Issue key, e.g. ARCH-123." } },
                "required": ["key"]
            }
        },
        {
            "name": "jira_create_issue",
            "description": "Creates a Jira issue (e.g. from model elements, lint findings or a gap analysis). The user always approves it before it is sent. \
Optionally adds a web link, e.g. to the model element (title \"EA: name\", url ea://{GUID}); afterwards you may save the issue key on the element \
(ea_write, a tagged value such as Jira = ARCH-123).",
            "input_schema": {
                "type": "object",
                "properties": {
                    "project": { "type": "string", "description": "Project key; empty = the default project from the settings." },
                    "type": { "type": "string", "description": "Issue type name, e.g. Task, Story, Bug, Epic (default Task)." },
                    "summary": { "type": "string" },
                    "description": { "type": "string", "description": "Plain text; one paragraph per line." },
                    "labels": { "type": "array", "items": { "type": "string" } },
                    "parent": { "type": "string", "description": "Optionally the parent issue (epic or story) key." },
                    "attachments": ATTACHMENTS_SCHEMA.clone(),
                    "image_width": IMAGE_WIDTH_SCHEMA.clone(),
                    "link_url": { "type": "string" },
                    "link_title": { "type": "string" }
                },
                "required": ["summary"]
            }
        },
        {
            "name": "jira_attach",
            "description": "Adds files to an existing Jira issue: images of diagrams straight from the model (diagram:ID), images shown in this \
conversation (image:FILE_NAME) or other files of the conversation (file:FILE_NAME, e.g. a Word document you made). On Jira Server / Data Center the \
images are also shown in the description. jira_create_issue and jira_update_issue take the same attachments, so a new issue gets its diagrams in the \
same step. The user approves it (once for all Jira changes if they choose).",
            "input_schema": {
                "type": "object",
                "properties": {
                    "key": { "type": "string" },
                    "attachments": ATTACHMENTS_SCHEMA.clone(),
                    "image_width": IMAGE_WIDTH_SCHEMA.clone(),
                    "show_in_description": { "type": "boolean", "description": "Jira Server / Data Center: also show the images at the end of the description (default true)." }
                },
                "required": ["key", "attachments"]
            }
        },
        {
            "name": "jira_update_issue",
            "description": "Changes an existing Jira issue: a new summary, a new description (it replaces the whole description: read the issue \
with jira_issue first and send the full corrected text) and labels to add or remove. Use it to correct issues, also ones created earlier in this \
conversation. The user approves it (once for all Jira changes if they choose).",
            "input_schema": {
                "type": "object",
                "properties": {
                    "key": { "type": "string" },
                    "summary": { "type": "string" },
                    "description": { "type": "string", "description": "The whole new description, plain text; one paragraph per line." },
                    "add_labels": { "type": "array", "items": { "type": "string" } },
                    "remove_labels": { "type": "array", "items": { "type": "string" } },
                    "attachments": ATTACHMENTS_SCHEMA.clone(),
                    "image_width": IMAGE_WIDTH_SCHEMA.clone()
                },
                "required": ["key"]
            }
        },
        {
            "name": "jira_comment",
            "description": "Adds a comment and/or a web link to an existing Jira issue. The user always approves it before it is sent.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "key": { "type": "string" },
                    "comment": { "type": "string" },
                    "link_url": { "type": "string" },
                    "link_title": { "type": "string" }
                },
                "required": ["key"]
            }
        },
        {
            "name": "jira_download_attachments",
            "description": "Downloads the attachments of a Jira issue (jira_issue lists them) into this conversation: images come back so you can see them \
and go into documents and pages as ![caption](image:NAME) (create_document, create_confluence_page) or to other issues as image:NAME (jira_attach); \
other files as file:NAME. Use it when a document, page or analysis should include or describe pictures from Jira (screens, mock-ups, diagrams).",
            "input_schema": {
                "type": "object",
                "properties": {
                    "key": { "type": "string", "description": "Issue key, e.g. PRJ-123." },
                    "names": { "type": "array", "items": { "type": "string" }, "description": "Only these file names (default: all, at most 20)." }
                },
                "required": ["key"]
            }
        },
        {
            "name": "create_confluence_page",
            "description": "Prepares a Confluence page: the page code in the source editor format (storage XHTML) and the images as attachments. The user gets a card \
with the \"Copy page code\" and \"Open images folder\" buttons. Content in Markdown: # headings, paragraphs, **bold**, `code`, lists, tables, [link](url), \
[TOC] (table of contents), panels :::info Title / :::note / :::warning / :::tip / :::expand Title … closed with :::, statuses {status:Green:YES} \
(colors Grey, Red, Yellow, Green, Blue, Purple), issues {jira:KEY-123}, mentions {user:login}, code blocks ```language, \
images on a separate line ![caption](diagram:ID) or ![caption](image:NAME). When the document has a template (the <templates> section), call read_template first \
and stick to its structure; template lines starting with \"> \" are hints for you, do not put them on the page.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "title": { "type": "string", "description": "Page title." },
                    "markdown": { "type": "string", "description": "Page content." },
                    "checklist": DOC_CHECKLIST_SCHEMA.clone(),
                    "template": DOC_TEMPLATE_SCHEMA.clone(),
                },
                "required": ["title", "markdown", "checklist", "template"]
            }
        },
        {
            "name": "publish_confluence",
            "description": "Publishes to Confluence through the API a page prepared by create_confluence_page (creates a new one or updates an existing one: by page_id, \
or without it by the title in the space) and uploads the images as attachments. Diagrams are exported from EA again, so this is also how you refresh a page after changes in the model. \
The user always approves the publication. Use it only when explicitly asked to publish or refresh a page.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "page": { "type": "string", "description": "The .confluence.xml file from the create_confluence_page result (or the source file from confluence_pages)." },
                    "title": { "type": "string", "description": "Optionally a different page title." },
                    "space": { "type": "string", "description": "Space key; by default from the settings." },
                    "parent_id": { "type": "string", "description": "Id of the parent page; by default from the settings." },
                    "page_id": { "type": "string", "description": "Id of an existing page to update." }
                },
                "required": ["page"]
            }
        },
        {
            "name": "confluence_read",
            "description": "Reads a Confluence page as text: by page_id, or by title in a space (space: by default the space from the settings or the team hub). Use it when the user asks about the content of a Confluence page.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "title": { "type": "string", "description": "Exact title of the page." },
                    "space": { "type": "string", "description": "Space key; by default from the settings." },
                    "page_id": { "type": "string", "description": "Id of the page (instead of the title)." }
                }
            }
        },
        {
            "name": "confluence_pages",
            "description": "List of Confluence pages published earlier by the assistant (id, title, space, source file, date, URL), e.g. to refresh them after changes in the model.",
            "input_schema": { "type": "object", "properties": {} }
        },
        {
            "name": "read_template",
            "description": "Reads a document template (listed in the <templates> section). Lines starting with \"> \" are hints for you.",
            "input_schema": { "type": "object", "properties": { "name": { "type": "string" } }, "required": ["name"] }
        },
        {
            "name": "save_template",
            "description": "Saves or overwrites a document template (Markdown; first line \"> Template: description\", hints in \"> \" lines), e.g. at the user's request, from a sample document.",
            "input_schema": {
                "type": "object",
                "properties": { "name": { "type": "string" }, "content": { "type": "string" } },
                "required": ["name", "content"]
            }
        },
        {
            "name": "create_spreadsheet",
            "description": "Creates an Excel workbook (XLSX) and shows it to the user in the chat with the \"Open\" and \"Save as…\" buttons. \
Use it when the user wants something in Excel: a list of elements, a matrix (e.g. UC x requirements, with X in the cells), an impact analysis result, a summary. \
Get the data with db_query first. The first row of each sheet holds the headers (bold, with a filter); give numbers as numbers.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "file_name": { "type": "string", "description": "File name without extension." },
                    "sheets": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "name": { "type": "string", "description": "Sheet name (up to 31 characters)." },
                                "columns": { "type": "array", "items": { "type": "string" } },
                                "rows": { "type": "array", "items": { "type": "array" } }
                            },
                            "required": ["name", "columns", "rows"]
                        }
                    }
                },
                "required": ["file_name", "sheets"]
            }
        },
        {
            "name": "memory_save",
            "description": "Saves a note to the memory kept between conversations (a local Markdown file). The same name overwrites the earlier note, \
so update existing notes instead of creating similar ones. You receive the saved notes in every later conversation in the <memory> section.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "name": { "type": "string", "description": "Short name of the note, e.g. \"Line style in BPMN\"." },
                    "description": { "type": "string", "description": "One sentence: what it is about (for the index)." },
                    "content": { "type": "string", "description": "Content of the note, concise, with the reasoning when it is known." },
                    "only_this_model": { "type": "boolean", "description": "Notes are shared by all models. Set true ONLY when the user explicitly asks to remember something just for this model (this file or database); the note is then shown only in conversations with this model." }
                },
                "required": ["name", "description", "content"]
            }
        },
        {
            "name": "memory_delete",
            "description": "Removes an outdated or wrong note from memory (the name or the file name from the <memory> section).",
            "input_schema": {
                "type": "object",
                "properties": { "name": { "type": "string" } },
                "required": ["name"]
            }
        },
        {
            "name": "memory_read",
            "description": "Reads the full content of a note from memory (needed only when the <memory> section contains just the index).",
            "input_schema": {
                "type": "object",
                "properties": { "name": { "type": "string" } },
                "required": ["name"]
            }
        }
    ])
}

/// Tool result: text for the model and images (file names in the attachments folder)
/// that the model should see and the application should show to the user in the chat.
pub struct ToolOutput {
    pub text: String,
    pub images: Vec<String>,
    /// Files for the user (e.g. DOCX): shown in the chat with buttons, not sent to the model.
    pub files: Vec<String>,
}

impl From<String> for ToolOutput {
    fn from(text: String) -> Self {
        Self { text, images: Vec::new(), files: Vec::new() }
    }
}

/// Runs the tool with the given name.
pub fn run(name: &str, input: &Value, ctx: &ToolContext) -> Result<ToolOutput> {
    if crate::mcp_client::is_mcp_tool(name) {
        let (ask, server, tool) = crate::mcp_client::needs_consent(&ctx.cfg.base_dir, name)?;
        if ask {
            let args = serde_json::to_string_pretty(input).unwrap_or_default();
            ctx.approver.confirm_kind("mcp", &crate::i18n::fmt(MCP_QUESTION, &[tool, server, args.chars().take(20_000).collect()]))?;
        }
        return crate::mcp_client::call(&ctx.cfg.base_dir, name, input).map(Into::into);
    }
    match name {
        "db_query" => db_query(input["sql"].as_str().with_context(|| crate::i18n::fmt("missing field '{0}'", &["sql".to_string()]))?, ctx.cfg).map(Into::into),
        "ea_write" => ea_write(input, ctx).map(Into::into),
        "ea_show_diagram" => ea_show_diagram(input, ctx),
        "miro_send" => crate::miro::tool(input, ctx.cfg).map(Into::into),
        "package_matrix" => crate::matrix::tool(input, ctx.cfg),
        "where_used" => {
            let mut text = crate::model_usage::where_used(input, ctx.cfg)?;
            text.push_str("\n(The user can ask to open a diagram with the element selected – ea_ui open_diagram {diagram, element} – or to show it in the project browser – ea_ui show_element.)");
            if input["thumbnails"] != true || input["element"].is_null() {
                return Ok(text.into());
            }
            // Pictures of the first diagrams it is on (LC-411).
            let id = input["element"].as_i64().or_else(|| query_rows(ctx.cfg, &format!("select object_id from t_object where name = '{}'", crate::model_db::sql_text(ctx.cfg.model_source, input["element"].as_str().unwrap_or("")))).ok()?.first()?[0].parse().ok());
            let diagrams: Vec<i64> = id.map(|e| query_rows(ctx.cfg, &format!("select distinct diagram_id from t_diagramobjects where object_id = {e}")).unwrap_or_default()).unwrap_or_default()
                .into_iter().filter_map(|r| r[0].parse().ok()).take(4).collect();
            let mut images = Vec::new();
            for d in diagrams {
                if let Ok(shown) = ea_show_diagram(&json!({ "diagram_id": d }), ctx) {
                    images.extend(shown.images);
                }
            }
            Ok(ToolOutput { text, images, files: Vec::new() })
        }
        "find_orphans" => crate::model_usage::find_orphans(input, ctx.cfg).map(Into::into),
        "find_duplicates" => crate::model_usage::find_duplicates(input, ctx.cfg).map(Into::into),
        "elements_without_notes" => crate::model_usage::elements_without_notes(input, ctx.cfg).map(Into::into),
        "repo_doctor" => crate::repo_doctor::run(input, ctx.cfg).map(Into::into),
        "what_changed" => crate::changes::run(input, ctx.cfg).map(Into::into),
        "team_rules" => team_rules(input, ctx).map(Into::into),
        "trace_gaps" => crate::trace_gaps::run(input, ctx.cfg).map(Into::into),
        "template_doctor" => crate::template_doctor::run(input, ctx.cfg).map(Into::into),
        // Restoring from the safety net is one ordinary save (approval, undo journal, audit, verification).
        "safety_net" if input["action"].as_str() == Some("restore") => {
            let ops = crate::safety_net::restore_ops(input, ctx.cfg)?;
            ea_write(&json!({ "ops": ops }), ctx).map(Into::into)
        }
        "safety_net" => crate::safety_net::run(input, ctx.cfg).map(Into::into),
        "sequence_check" => crate::sequence::check(input, ctx.cfg).map(Into::into),
        "export_text" => crate::text_export::run(input, ctx.cfg),
        "diagram_as_text" => crate::notation::run(input, ctx.cfg).map(Into::into),
        "migration_check" => crate::migration::run(input, ctx.cfg).map(Into::into),
        "compare_baselines" => crate::baseline_diff::run(input, ctx.cfg).map(Into::into),
        "why_locked" => crate::readonly_why::why_locked(input, ctx.cfg).map(Into::into),
        "what_if" => crate::what_if::run(input, ctx.cfg).map(Into::into),
        "table_ddl" => crate::mermaid_ddl::table_ddl(input, ctx.cfg).map(Into::into),
        "import_mermaid" => {
            let (ops, text) = crate::mermaid_ddl::import_mermaid(input, ctx.cfg)?;
            if ops.is_empty() { Ok(text.into()) } else { ea_write(&json!({ "ops": ops }), ctx).map(|saved| format!("{text}

{saved}")).map(Into::into) }
        }
        "dead_links" => crate::readonly_why::dead_links(input, ctx.cfg).map(Into::into),
        // Translating the model (LC-400): texts reads only; apply is one ordinary save of the translations.
        "translate_model" => {
            let (ops, text) = crate::translate_model::run(input, ctx.cfg)?;
            if ops.is_empty() { Ok(text.into()) } else { ea_write(&json!({ "ops": ops }), ctx).map(|saved| format!("{text}\n\n{saved}")).map(Into::into) }
        }
        // Test results (LC-399): the report reads only; apply writes tagged values on the requirements (one save).
        "test_results" => {
            let (ops, text) = crate::test_results::run(input, ctx.cfg)?;
            if ops.is_empty() { Ok(text.into()) } else { ea_write(&json!({ "ops": ops }), ctx).map(|saved| format!("{text}\n\n{saved}")).map(Into::into) }
        }
        // An API specification as a model (LC-397): preview reads only, apply is one ordinary save.
        "import_api" => {
            let (ops, text) = crate::api_import::run(input, ctx.cfg)?;
            if ops.is_empty() { Ok(text.into()) } else { ea_write(&json!({ "ops": ops }), ctx).map(|saved| format!("{text}

{saved}")).map(Into::into) }
        }
        // A sequence from text is one ordinary save; the report (bound, new, missing operations) goes with its result.
        "sequence_from_text" => {
            let (ops, report) = crate::sequence::from_text(input, ctx.cfg)?;
            ea_write(&json!({ "ops": ops }), ctx).map(|saved| format!("{report}\n\n{saved}")).map(Into::into)
        }
        "ea_howto" => crate::ea_help::run(input, ctx.cfg).map(Into::into),
        // A bulk change: the preview reads only; apply is one ordinary save (approval, undo journal, audit, verification).
        "bulk_set" if input["action"].as_str() == Some("apply") => {
            let ops = crate::bulk::ops_to_apply(input, ctx.cfg)?;
            ea_write(&json!({ "ops": ops }), ctx).map(Into::into)
        }
        "bulk_set" => crate::bulk::preview(input, ctx.cfg).map(Into::into),
        // Another type for relationships (LC-374): one save; the ArchiMate/team rules are checked on the new ones.
        "change_connector_type" => {
            let ids: Vec<i64> = input["connectors"].as_array().into_iter().flatten().filter_map(Value::as_i64).chain(input["connector"].as_i64()).collect();
            let kind = input["type"].as_str().filter(|t| !t.trim().is_empty()).context("give the new type")?;
            let mut ops = Vec::new();
            let mut ends = Vec::new();
            for id in &ids {
                if let Some(r) = query_rows(ctx.cfg, &format!("select start_object_id, end_object_id from t_connector where connector_id = {id}"))?.into_iter().next() {
                    ends.push((r[0].clone(), r[1].clone()));
                }
                let diagrams: Vec<i64> = query_rows(ctx.cfg, &format!("select distinct diagramid from t_diagramlinks where connectorid = {id}"))?.iter().filter_map(|r| r[0].parse().ok()).collect();
                let mut op = json!({ "op": "change_connector_type", "connector": id, "type": kind, "diagrams": diagrams });
                if let Some(st) = input["stereotype"].as_str() {
                    op["stereotype"] = json!(st);
                }
                ops.push(op);
            }
            if ops.is_empty() {
                bail!("give connector or connectors");
            }
            let saved = ea_write(&json!({ "ops": ops }), ctx)?;
            // The new relationships: between the same ends, of the new type.
            let kind_sql = crate::model_db::sql_text(ctx.cfg.model_source, kind);
            let new_ids: Vec<i64> = ends
                .iter()
                .flat_map(|(a, b)| query_rows(ctx.cfg, &format!("select connector_id from t_connector where start_object_id = {a} and end_object_id = {b} and lower(connector_type) = lower('{kind_sql}')")).unwrap_or_default())
                .filter_map(|r| r[0].parse().ok())
                .collect();
            let findings: Vec<String> = new_ids.iter().flat_map(|id| crate::governance::check_object(ctx.cfg, "connector", *id).unwrap_or_default()).collect();
            Ok(if findings.is_empty() { saved } else { format!("{saved}\nModel check of the new relationship: {}", findings.join("; ")) }.into())
        }
        "exec_view" => {
            let ops = exec_view_ops(ctx.cfg, input)?;
            ea_write(&json!({ "ops": ops }), ctx).map(Into::into)
        }
        // Publishing a stale page again (LC-426): the same source, space and page, with fresh diagrams; the usual approval.
        "stale_publications" if input["action"].as_str() == Some("republish") => {
            let wanted = input["target"].as_str().unwrap_or("");
            let record = crate::publications::find(ctx.cfg, wanted).with_context(|| format!("no publication \"{wanted}\" is recorded"))?;
            if record["again"]["page"].is_string() {
                publish_confluence(&json!({ "page": record["again"]["page"], "space": record["again"]["space"], "page_id": record["again"]["page_id"] }), ctx)
            } else {
                Ok(format!("\"{}\" is a {}: create it again with create_document from its template and the current model.", record["title"].as_str().unwrap_or(""), record["kind"].as_str().unwrap_or("document")).into())
            }
        }
        "stale_publications" => crate::publications::stale(input, ctx.cfg).map(Into::into),
        "jira_sync" => jira_sync(input, ctx).map(Into::into),
        "import_sheet" => {
            let (ops, text) = crate::import_sheet::run(input, ctx.cfg)?;
            if ops.is_empty() { Ok(text.into()) } else { ea_write(&json!({ "ops": ops }), ctx).map(Into::into) }
        }
        "clone_diagram" => {
            let (ops, text) = crate::diagram_clone::plan(input, ctx.cfg)?;
            if input["action"].as_str() == Some("apply") { ea_write(&json!({ "ops": ops }), ctx).map(Into::into) } else { Ok(format!("{text}\nNothing was changed yet: action apply does it after approval.").into()) }
        }
        "context_diagram" => {
            let ops = crate::context_diagram::plan(input, ctx.cfg)?;
            ea_write(&json!({ "ops": ops }), ctx).map(Into::into)
        }
        "apply_style" => {
            let (ops, text) = crate::style::plan(input, ctx.cfg)?;
            if input["action"].as_str() == Some("apply") && !ops.is_empty() { ea_write(&json!({ "ops": ops }), ctx).map(Into::into) } else { Ok(text.into()) }
        }
        "align_elements" => {
            let ops = align_ops(ctx.cfg, input)?;
            ea_write(&json!({ "ops": ops }), ctx).map(Into::into)
        }
        "place_new_elements" => {
            let (ops, text) = place_new_ops(ctx.cfg, input)?;
            if ops.is_empty() { Ok(text.into()) } else { ea_write(&json!({ "ops": ops }), ctx).map(|saved| format!("{text}\n\n{saved}")).map(Into::into) }
        }
        // Visibility policies of diagrams (LC-373): apply is an ordinary save, the rest reads or keeps a local setting.
        "connector_visibility" => {
            let (ops, text) = crate::visibility::run(input, ctx.cfg)?;
            if ops.is_empty() { Ok(text.into()) } else { ea_write(&json!({ "ops": ops }), ctx).map(Into::into) }
        }
        // Merge and replace: preview reads; apply is one save, always asked, with a baseline first.
        "merge_elements" if input["action"].as_str() == Some("replace_on_diagrams") => {
            let (ops, text) = crate::merge::replace_on_diagrams(input, ctx.cfg)?;
            if ops.is_empty() { Ok(text.into()) } else { ea_write(&json!({ "ops": ops }), ctx).map(|saved| format!("{text}\n\n{saved}")).map(Into::into) }
        }
        "merge_elements" => {
            let (op, text) = crate::merge::plan(input, ctx.cfg)?;
            if input["action"].as_str() == Some("apply") { ea_write(&json!({ "ops": [op] }), ctx).map(Into::into) } else { Ok(format!("{text}\n\nNothing was changed yet: show this to the user; action apply does it after approval.").into()) }
        }
        // The recycle bin: list reads; move and restore are ordinary saves.
        "recycle_bin" => {
            let (ops, text) = crate::recycle::plan(input, ctx.cfg)?;
            if ops.is_empty() { Ok(text.into()) } else { ea_write(&json!({ "ops": ops }), ctx).map(Into::into) }
        }
        "memory_save" => {
            let name = input["name"].as_str().with_context(|| crate::i18n::fmt("missing field '{0}'", &["name".to_string()]))?;
            let content = input["content"].as_str().with_context(|| crate::i18n::fmt("missing field '{0}'", &["content".to_string()]))?;
            if content.chars().count() > LONGEST_APPROVAL {
                bail!("{}", crate::i18n::fmt("the text is too long to approve ({0} characters, at most {1}): make it shorter", &[content.chars().count().to_string(), LONGEST_APPROVAL.to_string()]));
            }
            // Memory goes into every later conversation: always asked, with the whole text.
            ctx.approver.confirm_kind("memory", &crate::i18n::fmt(MEMORY_SAVE_QUESTION, &[name.to_string(), shorten_long(content)]))?;
            let model = if input["only_this_model"] == true { ctx.cfg.model_label() } else { String::new() };
            let file = memory::save(&ctx.cfg.memory_dir, name, input["description"].as_str().unwrap_or(""), content, &model)?;
            let scope = if model.is_empty() { "for all models".to_string() } else { format!("only for the model {model}") };
            Ok(format!("Saved note '{name}' to memory (file {file}), {scope}.").into())
        }
        "memory_delete" => {
            let name = input["name"].as_str().with_context(|| crate::i18n::fmt("missing field '{0}'", &["name".to_string()]))?;
            ctx.approver.confirm_kind("memory", &crate::i18n::fmt(MEMORY_REMOVE_QUESTION, &[name.to_string()]))?;
            memory::delete(&ctx.cfg.memory_dir, name)?;
            Ok(format!("Removed note '{name}' from memory.").into())
        }
        "render_image" => render_image(input, ctx),
        "create_document" => create_document(input, ctx),
        "create_presentation" => create_presentation(input, ctx),
        "create_spreadsheet" => create_spreadsheet(input, ctx),
        "create_confluence_page" => create_confluence_page(input, ctx),
        "jira_download_attachments" => jira_download_attachments(input, ctx),
        "publish_confluence" => publish_confluence(input, ctx),
        "confluence_pages" => confluence_pages(ctx.cfg).map(Into::into),
        "confluence_read" => confluence_read(input, ctx.cfg).map(Into::into),
        "read_template" => crate::templates::read(&ctx.cfg.templates_dir, input["name"].as_str().with_context(|| crate::i18n::fmt("missing field '{0}'", &["name".to_string()]))?).map(Into::into),
        "save_template" => {
            let name = input["name"].as_str().with_context(|| crate::i18n::fmt("missing field '{0}'", &["name".to_string()]))?;
            let content = input["content"].as_str().with_context(|| crate::i18n::fmt("missing field '{0}'", &["content".to_string()]))?;
            if content.chars().count() > LONGEST_APPROVAL {
                bail!("{}", crate::i18n::fmt("the text is too long to approve ({0} characters, at most {1}): make it shorter", &[content.chars().count().to_string(), LONGEST_APPROVAL.to_string()]));
            }
            // Templates guide later documents: saving one is always asked, with the whole text.
            let question = if crate::templates::exists(&ctx.cfg.templates_dir, name) { "Overwrite the document template \"{0}\":\n{1}" } else { "Save the document template \"{0}\":\n{1}" };
            ctx.approver.confirm_always(&crate::i18n::fmt(question, &[name.to_string(), shorten_long(content)]))?;
            let file = crate::templates::save(&ctx.cfg.templates_dir, name, content)?;
            Ok(format!("Saved template \"{name}\" (file {file} in the templates folder).").into())
        }
        "impact_analysis" => impact_analysis(input, ctx.cfg).map(Into::into),
        "model_lint" => crate::governance::lint(input, ctx.cfg).map(Into::into),
        "ea_script" => ea_script(input, ctx).map(Into::into),
        "trace_requirements" => trace_requirements(input, ctx).map(Into::into),
        "propose_change" => propose_change(input, ctx).map(Into::into),
        "undo_change" => undo_change(input, ctx).map(Into::into),
        "estimate_change" => crate::estimate::estimate(ctx.cfg, &input["elements"].as_array().into_iter().flatten().filter_map(|v| v.as_i64()).collect::<Vec<_>>(), input["depth"].as_u64().unwrap_or(2) as usize).map(Into::into),
        "change_sets" => change_sets(input, ctx).map(Into::into),
        "review_comments" => review_comments(input, ctx).map(Into::into),
        "mdg_technologies" if input["action"] == "build" => mdg_build(input, ctx),
        "mdg_technologies" => crate::mdg::run(input, ctx.cfg).map(Into::into),
        "sharepoint" => crate::sharepoint::run(input, ctx).map(Into::into),
        "onedrive" => crate::sharepoint::run_onedrive(input, ctx).map(Into::into),
        "google_drive" => crate::gdrive::run(input, ctx).map(Into::into),
        "model_instructions" => crate::instructions::run(input, ctx).map(Into::into),
        "read_document" => {
            let path = read_path(ctx, input["file"].as_str().with_context(|| crate::i18n::fmt("missing field '{0}'", &["file".to_string()]))?, false)?;
            let size = std::fs::metadata(&path).with_context(|| crate::i18n::fmt("cannot read {0}", &[path.display().to_string()]))?.len();
            if size > 60 * 1024 * 1024 {
                bail!("{}", crate::i18n::tr("the document is larger than 60 MB"));
            }
            let bytes = std::fs::read(&path).with_context(|| crate::i18n::fmt("cannot read {0}", &[path.display().to_string()]))?;
            let text = crate::documents::text_of(&path.display().to_string(), &bytes)?;
            Ok(format!("{} ({} characters):
{}", path.display(), text.chars().count(), crate::documents::limited(text)).into())
        }
        "version_control_status" => crate::mdg::version_control_status(input, ctx.cfg).map(Into::into),
        "ea_api" => crate::ea_api::run(input, ctx.cfg).map(Into::into),
        "cmdb_compare" => {
            let text = match input["file"].as_str().map(str::trim).filter(|f| !f.is_empty()) {
                Some(file) => {
                    let path = read_path(ctx, file, false)?;
                    let meta = std::fs::metadata(&path).with_context(|| crate::i18n::fmt("cannot read {0}", &[path.display().to_string()]))?;
                    if meta.len() > 20 * 1024 * 1024 {
                        bail!("{}", crate::i18n::tr("the CMDB file is larger than 20 MB: export fewer columns or items"));
                    }
                    std::fs::read_to_string(&path).with_context(|| crate::i18n::fmt("cannot read {0}", &[path.display().to_string()]))?
                }
                None => input["text"].as_str().filter(|t| !t.trim().is_empty()).with_context(|| crate::i18n::tr("give 'file' or 'text' with the CMDB data"))?.to_string(),
            };
            let types: Vec<String> = input["types"].as_array().into_iter().flatten().filter_map(|v| v.as_str()).map(String::from).collect();
            crate::cmdb::compare(ctx.cfg, &text, input["package"].as_i64(), &types).map(|c| crate::cmdb::report(&c).into())
        }
        "code_tree" | "code_search" | "code_read" => {
            let path = crate::code_read::allowed(ctx.cfg, input["path"].as_str().unwrap_or(""), &|question| ctx.approver.confirm_always(question))?;
            let filter = input["filter"].as_str().unwrap_or("");
            match name {
                "code_tree" => crate::code_read::tree(&path, filter),
                "code_search" => crate::code_read::search(&path, input["query"].as_str().unwrap_or(""), filter),
                _ => crate::code_read::read(&path, input["from"].as_i64(), input["to"].as_i64()),
            }
            .map(Into::into)
        }
        "code_drift" => {
            let path = read_path(ctx, input["path"].as_str().unwrap_or(""), true)?;
            crate::code_drift::compare(ctx.cfg, &path, input["package"].as_i64()).map(Into::into)
        }
        "spell_check" => crate::model_spell::check(ctx.cfg, input["package"].as_i64(), crate::i18n::current().code()).map(Into::into),
        "app_changelog" => Ok(crate::changelog::lookup(input).into()),
        "jira_search" => jira(ctx.cfg)?.search(input["jql"].as_str().with_context(|| crate::i18n::fmt("missing field '{0}'", &["jql".to_string()]))?, input["max"].as_u64().unwrap_or(30) as usize).map(Into::into),
        "jira_issue" => jira(ctx.cfg)?.issue(input["key"].as_str().with_context(|| crate::i18n::fmt("missing field '{0}'", &["key".to_string()]))?.trim()).map(Into::into),
        "jira_create_issue" => jira_create_issue(input, ctx).map(Into::into),
        "jira_comment" => jira_comment(input, ctx).map(Into::into),
        "jira_update_issue" => jira_update_issue(input, ctx).map(Into::into),
        "jira_attach" => jira_attach(input, ctx).map(Into::into),
        "gap_analysis" => crate::governance::gap(input, ctx.cfg).map(Into::into),
        "operation_usage" => operation_usage(input, ctx.cfg).map(Into::into),
        "ea_ui" => {
            let db = &crate::ea_ui::Target::of(ctx.cfg);
            match input["action"].as_str().unwrap_or("context") {
                "context" => Ok(serde_json::to_string_pretty(&crate::ea_ui::context(db)?)?.into()),
                "open_diagram" => {
                    let id = input["diagram"].as_i64().with_context(|| crate::i18n::fmt("missing field '{0}'", &["diagram".to_string()]))? as i32;
                    crate::ea_ui::open_diagram(db, id)?;
                    Ok(format!("Opened diagram id={id} in the user's EA window.").into())
                }
                "open_diagram" if input["element"].as_i64().is_some() => {
                    let (d, e) = (input["diagram"].as_i64().unwrap_or(0) as i32, input["element"].as_i64().unwrap_or(0) as i32);
                    crate::ea_ui::open_diagram_at(db, d, e)?;
                    Ok(format!("Opened diagram id={d} with element id={e} selected in the user's EA window.").into())
                }
                "show_element" => {
                    let id = input["element"].as_i64().with_context(|| crate::i18n::fmt("missing field '{0}'", &["element".to_string()]))? as i32;
                    crate::ea_ui::show_element(db, id)?;
                    Ok(format!("Showed element id={id} in the project browser of the user's EA window.").into())
                }
                other => bail!("{}", crate::i18n::fmt("unknown ea_ui action: {0}", &[other.to_string()])),
            }
        }
        "memory_read" => memory::read(&ctx.cfg.memory_dir, input["name"].as_str().with_context(|| crate::i18n::fmt("missing field '{0}'", &["name".to_string()]))?).map(Into::into),
        other => bail!("{}", crate::i18n::fmt("unknown tool: {0}", &[other.to_string()])),
    }
}

/// Draws an image from SVG (matrix, table, chart, schematic) and shows it in the chat and to the model.
fn render_image(input: &Value, ctx: &ToolContext) -> Result<ToolOutput> {
    let svg = input["svg"].as_str().with_context(|| crate::i18n::fmt("missing field '{0}'", &["svg".to_string()]))?;
    let name = input["name"].as_str().unwrap_or(crate::i18n::tr("image"));
    let png = crate::render::svg_to_png(svg, 1.5)?;
    std::fs::create_dir_all(&ctx.cfg.attachments_dir).context(crate::i18n::tr("attachments folder"))?;
    let stored = format!("{}_{}.png", uuid::Uuid::new_v4().simple(), crate::history::sanitize_name(name));
    std::fs::write(ctx.cfg.attachments_dir.join(&stored), &png).context(crate::i18n::tr("saving the image"))?;
    Ok(ToolOutput {
        text: format!(
            "Image \"{name}\" is in the attached PNG (the user sees it in the chat and can save it). Check that the text fits and nothing overlaps. \
             You can insert it into a document in create_document with the tag ![caption](image:{stored})."
        ),
        images: vec![stored],
        files: Vec::new(),
    })
}

/// Image for a document or page: `diagram:ID` (export from EA) or `image:NAME` (an image drawn earlier).
fn picture(ctx: &ToolContext, source: &str) -> Result<Vec<u8>> {
    if let Some(id) = source.strip_prefix("diagram:") {
        let id: i64 = id.trim().parse().context(crate::i18n::tr("diagram id"))?;
        let path = std::env::temp_dir().join(format!("loonar_img_{}.png", uuid::Uuid::new_v4().simple()));
        // Images for Jira, Confluence and documents: twice the size, drawn from the diagram's vectors (LC-305).
        let op = json!({ "op": "export_diagram", "diagram": id, "path": path.to_string_lossy(), "scale": 2.0 });
        ea::run_batch(ctx.cfg, &[op], &BatchOptions::default())?;
        let png = std::fs::read(&path).with_context(|| crate::i18n::fmt("EA did not save the image of diagram {0}", &[id.to_string()]))?;
        let _ = std::fs::remove_file(&path);
        Ok(png)
    } else if let Some(stored) = source.strip_prefix("image:") {
        let stored = std::path::Path::new(stored.trim()).file_name().context(crate::i18n::tr("image name"))?;
        std::fs::read(ctx.cfg.attachments_dir.join(stored)).with_context(|| crate::i18n::fmt("there is no image {0}", &[stored.to_string_lossy().to_string()]))
    } else {
        bail!("{}", crate::i18n::fmt("unknown image source '{0}' (use diagram:ID or image:FILE_NAME)", &[source.to_string()]));
    }
}

/// First line of a page source (.confluence.md): the page title.
const TITLE_MARKER: &str = "<!-- title:";
/// The same marker in pages created before version 0.3.
const LEGACY_TITLE_MARKER: &str = "<!-- tytuł:";

/// Folder with the images of a Confluence page (`<page>_images`; pages created before version 0.3 use `<page>_zalaczniki`).
pub fn page_images_dir(attachments_dir: &std::path::Path, stored_page: &str) -> std::path::PathBuf {
    let current = attachments_dir.join(stored_page.replace(".confluence.xml", "_images"));
    let legacy = attachments_dir.join(stored_page.replace(".confluence.xml", "_zalaczniki"));
    if !current.exists() && legacy.is_dir() { legacy } else { current }
}

/// Confluence page: source (.confluence.md), page code for the source editor (.confluence.xml) and images in a folder next to them.
/// A presentation copy of a diagram (LC-377): same elements and places, details hidden, coloured by type.
fn exec_view_ops(cfg: &AppConfig, input: &Value) -> Result<Vec<Value>> {
    let diagram = input["diagram"].as_i64().context("diagram")?;
    let d = query_rows(cfg, &format!("select name, package_id, diagram_type from t_diagram where diagram_id = {diagram}"))?;
    let d = d.first().with_context(|| format!("there is no diagram with id {diagram}"))?;
    let name = input["name"].as_str().map(str::to_string).unwrap_or_else(|| format!("{} – presentation", d[0]));
    let mut ops = vec![json!({ "op": "create_diagram", "key": "view", "package": d[1].parse::<i64>().unwrap_or(0), "name": name, "type": d[2] })];
    let objects = query_rows(cfg, &format!(
        "select x.object_id, x.rectleft, x.recttop, x.rectright, x.rectbottom, o.object_type from t_diagramobjects x join t_object o on o.object_id = x.object_id \
         where x.diagram_id = {diagram} order by x.sequence desc"
    ))?;
    let mut types: Vec<String> = objects.iter().map(|r| r[5].clone()).collect();
    types.sort();
    types.dedup();
    let colours = crate::style::colours(&types, &Value::Null);
    for r in &objects {
        let n = |i: usize| r[i].parse::<i64>().unwrap_or(0);
        ops.push(json!({ "op": "place", "diagram": "#view", "element": n(0), "left": n(1), "top": -n(2), "width": n(3) - n(1), "height": n(2) - n(4) }));
        if !matches!(r[5].as_str(), "Note" | "Text" | "Boundary" | "ActivityPartition" | "Partition" | "Swimlane") {
            ops.push(json!({ "op": "set_color", "diagram": "#view", "element": n(0), "bgcolor": colours[&r[5]] }));
        }
    }
    ops.push(json!({ "op": "diagram_options", "diagram": "#view",
        "extended": { "HideAtts": "1", "HideOps": "1", "HideStereo": "1", "HideEStereo": "1", "ShowNotes": "0" },
        "style": { "SuppConnectorLabels": "1" } }));
    Ok(ops)
}

/// Rectangles of elements on a diagram as (id, left, top, width, height) with top downwards (LC-376).
fn rects(cfg: &AppConfig, diagram: i64, ids: &[i64]) -> Result<Vec<(i64, i64, i64, i64, i64)>> {
    let list = ids.iter().map(i64::to_string).collect::<Vec<_>>().join(",");
    let rows = query_rows(cfg, &format!("select object_id, rectleft, recttop, rectright, rectbottom from t_diagramobjects where diagram_id = {diagram} and object_id in ({list})"))?;
    let mut out: Vec<(i64, i64, i64, i64, i64)> = rows
        .iter()
        .map(|r| {
            let n = |i: usize| r[i].parse::<i64>().unwrap_or(0);
            (n(0), n(1), -n(2), n(3) - n(1), n(2) - n(4))
        })
        .collect();
    // In the order given (the first one is the reference).
    out.sort_by_key(|r| ids.iter().position(|i| *i == r.0).unwrap_or(usize::MAX));
    if out.len() != ids.len() {
        bail!("some of the elements are not on diagram {diagram}");
    }
    Ok(out)
}

/// Relationships of moved elements whose drawn route (bend points or fixed ends) would now go astray: routed straight
/// again, the others on the diagram untouched (LC-418).
pub(crate) fn reroute_ops(cfg: &AppConfig, diagram: i64, moved: &[i64]) -> Vec<Value> {
    if moved.is_empty() {
        return Vec::new();
    }
    let list = moved.iter().map(i64::to_string).collect::<Vec<_>>().join(",");
    query_rows(cfg, &format!(
        "select l.connectorid from t_diagramlinks l join t_connector c on c.connector_id = l.connectorid where l.diagramid = {diagram} \
         and (c.start_object_id in ({list}) or c.end_object_id in ({list})) \
         and (coalesce(l.path,'') <> '' or coalesce(l.geometry,'') like '%SX=%' or coalesce(l.geometry,'') like '%EX=%')"
    ))
    .unwrap_or_default()
    .into_iter()
    .filter_map(|r| r[0].parse::<i64>().ok())
    .map(|c| json!({ "op": "route_connector", "diagram": diagram, "connector": c }))
    .collect()
}

/// A rectangle on a diagram: (left, top, width, height), Y pointing down.
type Rect = (i64, i64, i64, i64);

fn overlaps(a: Rect, b: Rect, margin: i64) -> bool {
    a.0 < b.0 + b.2 + margin && b.0 < a.0 + a.2 + margin && a.1 < b.1 + b.3 + margin && b.1 < a.1 + a.3 + margin
}

fn contains(outer: Rect, inner: Rect) -> bool {
    outer.0 <= inner.0 && outer.1 <= inner.1 && outer.0 + outer.2 >= inner.0 + inner.2 && outer.1 + outer.3 >= inner.1 + inner.3 && outer != inner
}

/// A free place for a box of w × h next to `anchor`: right, below, left, above, then further out in rings; never on
/// top of the `taken` ones (with `gap` between) and never at negative coordinates.
pub(crate) fn free_spot(taken: &[Rect], anchor: Rect, w: i64, h: i64, gap: i64) -> (i64, i64) {
    let free = |l: i64, t: i64| l >= 0 && t >= 0 && !taken.iter().any(|r| overlaps((l, t, w, h), *r, gap / 2));
    for ring in 0..12 {
        let step_x = (anchor.2.max(w) + gap) * ring;
        let step_y = (anchor.3.max(h) + gap) * ring;
        let right = (anchor.0 + anchor.2 + gap + step_x, anchor.1);
        let below = (anchor.0, anchor.1 + anchor.3 + gap + step_y);
        let left = (anchor.0 - w - gap - step_x, anchor.1);
        let above = (anchor.0, anchor.1 - h - gap - step_y);
        let diagonal = (anchor.0 + anchor.2 + gap + step_x, anchor.1 + anchor.3 + gap + step_y);
        for (l, t) in [right, below, left, above, diagonal] {
            if free(l, t) {
                return (l, t);
            }
        }
    }
    // Everything around is taken: below all of it.
    let bottom = taken.iter().map(|r| r.1 + r.3).max().unwrap_or(0);
    (anchor.0.max(0), bottom + gap)
}

/// Incremental layout (LC-376): places new elements of a diagram (the given ones, or by default those lying on top of
/// others) next to an element they are related to, in free space, and leaves everything else where it is.
pub(crate) fn place_new_ops(cfg: &AppConfig, input: &Value) -> Result<(Vec<Value>, String)> {
    let diagram = input["diagram"].as_i64().context("diagram")?;
    let gap = input["spacing"].as_i64().unwrap_or(40).clamp(10, 200);
    let rows = query_rows(cfg, &format!(
        "select x.object_id, x.rectleft, x.recttop, x.rectright, x.rectbottom, o.name from t_diagramobjects x join t_object o on o.object_id = x.object_id \
         where x.diagram_id = {diagram} and o.object_type not in ('Boundary', 'ActivityPartition', 'Partition', 'Swimlane', 'Text', 'InteractionFragment') order by x.object_id"
    ))?;
    let all: Vec<(i64, Rect, String)> = rows
        .iter()
        .map(|r| {
            let n = |i: usize| r[i].parse::<i64>().unwrap_or(0);
            (n(0), (n(1), -n(2), n(3) - n(1), n(2) - n(4)), r[5].clone())
        })
        .collect();
    // Containers (a frame, a package with elements drawn inside) do not count as overlaps.
    let is_container = |rect: Rect| all.iter().any(|(_, other, _)| contains(rect, *other));
    let given: Vec<i64> = input["elements"].as_array().into_iter().flatten().filter_map(Value::as_i64).collect();
    let moving: Vec<i64> = if given.is_empty() {
        // Lying on top of an older element (a higher object_id is the newer one).
        all.iter()
            .filter(|(id, rect, _)| !is_container(*rect) && all.iter().any(|(other, r, _)| other < id && !is_container(*r) && overlaps(*rect, *r, 0)))
            .map(|(id, _, _)| *id)
            .collect()
    } else {
        given.iter().copied().filter(|g| all.iter().any(|(id, _, _)| id == g)).collect()
    };
    if moving.is_empty() {
        return Ok((Vec::new(), "Nothing to place: no element lies on top of another (give the elements to place).".into()));
    }
    let mut taken: Vec<(i64, Rect)> = all.iter().filter(|(id, rect, _)| !moving.contains(id) && !is_container(*rect)).map(|(id, r, _)| (*id, *r)).collect();
    if taken.is_empty() {
        return Ok((Vec::new(), "Nothing stays in place on this diagram: use layout_diagram for the whole diagram.".into()));
    }
    let mut ops = Vec::new();
    let mut lines = Vec::new();
    for id in &moving {
        let Some((_, rect, name)) = all.iter().find(|(e, _, _)| e == id) else { continue };
        // The anchor: an element on the diagram it is related to, otherwise the nearest one.
        let related: Vec<i64> = query_rows(cfg, &format!(
            "select case when start_object_id = {id} then end_object_id else start_object_id end from t_connector where (start_object_id = {id} or end_object_id = {id})"
        ))?
        .into_iter()
        .filter_map(|r| r[0].parse().ok())
        .collect();
        let anchor = taken
            .iter()
            .find(|(t, _)| related.contains(t))
            .or_else(|| taken.iter().min_by_key(|(_, r)| (r.0 - rect.0).abs() + (r.1 - rect.1).abs()))
            .map(|(_, r)| *r)
            .unwrap_or((0, 0, 0, 0));
        let rects: Vec<Rect> = taken.iter().map(|(_, r)| *r).collect();
        let (left, top) = free_spot(&rects, anchor, rect.2.max(20), rect.3.max(20), gap);
        if (left, top) != (rect.0, rect.1) {
            ops.push(json!({ "op": "set_bounds", "diagram": diagram, "element": id, "left": left, "top": top }));
            lines.push(format!("- {name}: ({}, {}) → ({left}, {top})", rect.0, rect.1));
        }
        taken.push((*id, (left, top, rect.2, rect.3)));
    }
    let moved: Vec<i64> = ops.iter().filter_map(|o| o["element"].as_i64()).collect();
    let rerouted = reroute_ops(cfg, diagram, &moved);
    let note = if rerouted.is_empty() { String::new() } else { format!("\n{} relationship route(s) of the moved elements drawn straight again.", rerouted.len()) };
    ops.extend(rerouted);
    Ok((ops, format!("{} element(s) placed in free space, nothing else moved:\n{}{note}", lines.len(), lines.join("\n"))))
}

/// set_bounds operations that align, distribute or resize the given elements (LC-376).
pub(crate) fn align_ops(cfg: &AppConfig, input: &Value) -> Result<Vec<Value>> {
    let diagram = input["diagram"].as_i64().context("diagram")?;
    let ids: Vec<i64> = input["elements"].as_array().into_iter().flatten().filter_map(Value::as_i64).collect();
    let how = input["how"].as_str().unwrap_or("");
    if ids.len() < if how == "snap" { 1 } else { 2 } {
        bail!("give at least two elements (one for snap)");
    }
    let mut r = rects(cfg, diagram, &ids)?;
    let mut ops: Vec<Value> = align(&mut r, how)?.into_iter().map(|(id, left, top, width, height)| json!({ "op": "set_bounds", "diagram": diagram, "element": id, "left": left, "top": top, "width": width, "height": height })).collect();
    // Only the routes of what moved are drawn again (LC-418).
    let moved: Vec<i64> = ops.iter().filter_map(|o| o["element"].as_i64()).collect();
    ops.extend(reroute_ops(cfg, diagram, &moved));
    Ok(ops)
}

/// The new rectangles (only the ones that change).
pub(crate) fn align(r: &mut [(i64, i64, i64, i64, i64)], how: &str) -> Result<Vec<(i64, i64, i64, i64, i64)>> {
    let before: Vec<_> = r.to_vec();
    let first = r[0];
    match how {
        "left" => r.iter_mut().for_each(|e| e.1 = first.1),
        "right" => r.iter_mut().for_each(|e| e.1 = first.1 + first.3 - e.3),
        "top" => r.iter_mut().for_each(|e| e.2 = first.2),
        "bottom" => r.iter_mut().for_each(|e| e.2 = first.2 + first.4 - e.4),
        "center" => r.iter_mut().for_each(|e| e.1 = first.1 + first.3 / 2 - e.3 / 2),
        "middle" => r.iter_mut().for_each(|e| e.2 = first.2 + first.4 / 2 - e.4 / 2),
        "same_size" => r.iter_mut().for_each(|e| {
            e.3 = first.3;
            e.4 = first.4;
        }),
        "snap" => r.iter_mut().for_each(|e| {
            e.1 = (e.1 + 5) / 10 * 10;
            e.2 = (e.2 + 5) / 10 * 10;
        }),
        "distribute_horizontally" | "distribute_vertically" => {
            let horizontal = how == "distribute_horizontally";
            r.sort_by_key(|e| if horizontal { e.1 } else { e.2 });
            let (start, end) = if horizontal { (r[0].1, r[r.len() - 1].1 + r[r.len() - 1].3) } else { (r[0].2, r[r.len() - 1].2 + r[r.len() - 1].4) };
            let total: i64 = r.iter().map(|e| if horizontal { e.3 } else { e.4 }).sum();
            let gap = (end - start - total) / (r.len() as i64 - 1).max(1);
            let mut at = start;
            for e in r.iter_mut() {
                if horizontal { e.1 = at; at += e.3 + gap; } else { e.2 = at; at += e.4 + gap; }
            }
        }
        other => bail!("unknown alignment {other}"),
    }
    Ok(r.iter().filter(|e| !before.contains(e)).copied().collect())
}

/// A read-only tool run straight from a command, without the AI (LC-389).
pub fn run_direct(name: &str, input: &Value, cfg: &AppConfig) -> Result<String> {
    match name {
        "repo_doctor" => crate::repo_doctor::run(input, cfg),
        "stale_publications" => crate::publications::stale(input, cfg),
        "find_orphans" => crate::model_usage::find_orphans(input, cfg),
        "find_duplicates" => crate::model_usage::find_duplicates(input, cfg),
        "trace_gaps" => crate::trace_gaps::run(input, cfg),
        "dead_links" => crate::readonly_why::dead_links(input, cfg),
        "what_changed" => crate::changes::run(input, cfg),
        "connector_visibility" if input["action"] == "hidden" => crate::visibility::run(input, cfg).map(|(_, text)| text),
        other => bail!("{other} is not run without the assistant"),
    }
}

/// The team's modelling rules (LC-372): shown, or saved to governance.json after the user approves the whole text.
fn team_rules(input: &Value, ctx: &ToolContext) -> Result<String> {
    let file = ctx.cfg.base_dir.join("governance.json");
    if input["action"].as_str() != Some("save") {
        let local = std::fs::read_to_string(&file).ok();
        let rules = crate::governance::load_rules(ctx.cfg, &Value::Null);
        let source = if local.is_some() { file.display().to_string() } else if rules.as_object().is_some_and(|o| !o.is_empty()) { "the team hub's policy".to_string() } else { "none yet".to_string() };
        return Ok(format!("Rules in force (from {source}):\n{}", serde_json::to_string_pretty(&rules)?));
    }
    let rules = &input["rules"];
    crate::governance::check_rules(rules)?;
    let text = serde_json::to_string_pretty(rules)?;
    ctx.approver.confirm(&crate::i18n::fmt("Save the team's modelling rules to {0}?\n\n{1}", &[file.display().to_string(), text.clone()]))?;
    let tmp = file.with_extension("json.tmp");
    std::fs::write(&tmp, &text)?;
    std::fs::rename(&tmp, &file)?;
    Ok(format!("Saved the rules to {}. model_lint, the live check in EA and the CI check use them from now on.", file.display()))
}

/// Undoes the latest saves of the assistant (reverse operations from the journal).
fn undo_change(input: &Value, ctx: &ToolContext) -> Result<String> {
    if input["action"].as_str() != Some("undo") {
        return crate::undo::list_text(ctx.cfg);
    }
    let count = input["count"].as_u64().unwrap_or(1).clamp(1, 20) as usize;
    let entries = crate::undo::latest(ctx.cfg, count);
    if entries.is_empty() {
        bail!("{}", crate::i18n::tr("there is nothing to undo in this model"));
    }
    let summary: Vec<String> = entries.iter().map(|e| format!("- {} {}", e["time"].as_str().unwrap_or("").get(..19).unwrap_or("").replace('T', " "), e["summary"].as_str().unwrap_or("").replace('\n', "; "))).collect();
    // The journal is a file: its operations are checked like the assistant's own, and the user sees what undoing does
    // (what is deleted with its current name: colleagues may have added to it since).
    let mut what = Vec::new();
    for e in &entries {
        let ops = normalize_ops(&e["reverse"].as_array().cloned().unwrap_or_default());
        if let Some(bad) = ops.iter().find(|op| !ea::MODEL_OPS.contains(&op["op"].as_str().unwrap_or(""))) {
            bail!("{}", crate::i18n::fmt("the undo journal contains an operation that is not allowed ({0}); nothing was undone", &[bad["op"].to_string()]));
        }
        what.extend(destructive_details(ctx.cfg, &ops));
    }
    let details = if what.is_empty() { String::new() } else { format!("\n\n{}", what.join("\n")) };
    ctx.approver.confirm_risky(&format!("{}{details}", crate::i18n::fmt("Undo the latest saves of the assistant ({0}):\n{1}", &[entries.len().to_string(), summary.join("\n")])))?;
    let mut out = Vec::new();
    let mut undone = Vec::new();
    for e in &entries {
        let ops: Vec<Value> = normalize_ops(&e["reverse"].as_array().cloned().unwrap_or_default());
        let label = e["summary"].as_str().unwrap_or("").replace('\n', "; ");
        if !ops.is_empty() {
            // An undo is a write like any other: it goes to the audit log and the hub's trail (LC-348).
            let result = ea::run_batch(ctx.cfg, &ops, &BatchOptions::default());
            audit(ctx.cfg, &ops, &result);
            match result {
                Ok(done) => {
                    let check = crate::verify::verify(ctx.cfg, &ops, &done);
                    out.push(if check.problems.is_empty() { format!("Undone and verified: {label}") } else { format!("Undone, but the model differs: {label}: {}", check.problems.join("; ")) });
                }
                Err(err) => {
                    out.push(format!("COULD NOT undo: {label}: {err:#}"));
                    break;
                }
            }
        }
        let rest: Vec<&str> = e["not_undoable"].as_array().into_iter().flatten().filter_map(|v| v.as_str()).collect();
        if !rest.is_empty() {
            let baselines: Vec<&str> = e["baselines"].as_array().into_iter().flatten().filter_map(|v| v.as_str()).collect();
            out.push(format!("Not undone by operations ({}){}", rest.join(", "), if baselines.is_empty() { String::new() } else { format!(" - restore the EA baseline: {} (package → Manage Baselines → Restore)", baselines.join("; ")) }));
        }
        undone.push(e["id"].as_str().unwrap_or("").to_string());
    }
    crate::undo::mark_undone(ctx.cfg, &undone);
    refresh_user_ea(ctx.cfg, &[], &json!({}), &[]);
    Ok(out.join("\n"))
}

fn hub_client(cfg: &AppConfig) -> Result<crate::hub::Client> {
    crate::hub::Client::from_config(cfg).with_context(|| crate::i18n::tr("no team hub in the settings (Settings → Team (Compass Hub))"))
}

fn hub_source(cfg: &AppConfig) -> String {
    if !cfg.hub_source.trim().is_empty() {
        cfg.hub_source.trim().to_string()
    } else if cfg.uses_file() {
        cfg.model_file.display().to_string()
    } else {
        format!("{}@{}:{}", cfg.db_name, cfg.db_host, cfg.db_port)
    }
}

/// A change set for review in the hub (nothing is saved in the model).
fn propose_change(input: &Value, ctx: &ToolContext) -> Result<String> {
    let ops = input["ops"].as_array().with_context(|| crate::i18n::fmt("missing list '{0}'", &["ops".to_string()]))?;
    for (i, op) in ops.iter().enumerate() {
        let name = op["op"].as_str().unwrap_or("");
        if !ea::MODEL_OPS.contains(&name) {
            bail!("{}", crate::i18n::fmt("operation {0} ('{1}') is not allowed. Allowed: {2}", &[(i + 1).to_string(), name.to_string(), ea::MODEL_OPS.join(", ")]));
        }
    }
    let ops = &normalize_ops(ops);
    let client = hub_client(ctx.cfg)?;
    if ctx.cfg.hub_source.trim().is_empty() {
        bail!("{}", crate::i18n::tr("this model has no id in the team hub: enter it in Settings → Team (Compass Hub) → model id in the hub"));
    }
    let title = input["title"].as_str().unwrap_or("Change").to_string();
    let id = client.propose(&json!({
        "source": hub_source(ctx.cfg), "title": title, "summary": ea::describe(ops), "preview": crate::governance::preview(ctx.cfg, ops), "ops": ops
    }))?;
    Ok(format!("Proposed to the team as change set {id} (\"{title}\"): nothing was changed in the model. Tell the user it waits for review in the hub ({}/), where the team comments and an architect approves it; afterwards it can be applied with change_sets apply.", client.url))
}

/// Review comments and decisions on elements in the hub (LC-392).
fn review_comments(input: &Value, ctx: &ToolContext) -> Result<String> {
    let client = hub_client(ctx.cfg)?;
    if ctx.cfg.hub_source.trim().is_empty() {
        bail!("{}", crate::i18n::tr("this model has no id in the team hub: enter it in Settings → Team (Compass Hub) → model id in the hub"));
    }
    let source = hub_source(ctx.cfg);
    match input["action"].as_str().unwrap_or("list") {
        "list" => {
            let list = client.comments(&source, input["element"].as_i64(), input["status"].as_str())?;
            if list.is_empty() {
                return Ok("No review comments or decisions here.".into());
            }
            let mut out = vec![format!("{} comment(s) and decision(s):", list.len())];
            out.extend(list.iter().map(|c| format!(
                "- {} | {} | {} on {} (id {}) by {} {}: {}",
                c["id"].as_str().unwrap_or(""),
                c["status"].as_str().unwrap_or(""),
                c["kind"].as_str().unwrap_or(""),
                c["name"].as_str().unwrap_or(""),
                c["element"],
                c["user"].as_str().unwrap_or(""),
                c["time"].as_str().unwrap_or("").get(..10).unwrap_or(""),
                c["text"].as_str().unwrap_or("").replace('\n', " ")
            )));
            Ok(out.join("\n"))
        }
        "add" => {
            let c = client.add_comment(&source, &json!({ "element": input["element"], "text": input["text"], "kind": input["kind"] }))?;
            Ok(format!("Added {} {} on {}.", c["kind"].as_str().unwrap_or(""), c["id"].as_str().unwrap_or(""), c["name"].as_str().unwrap_or("")))
        }
        "status" => {
            let c = client.comment_status(input["id"].as_str().unwrap_or(""), input["status"].as_str().unwrap_or(""))?;
            Ok(format!("{} is now {}.", c["id"].as_str().unwrap_or(""), c["status"].as_str().unwrap_or("")))
        }
        // LC-407: ask the owners to confirm or update elements; see and answer the requests.
        "request" => {
            let mut body = json!({ "question": input["text"] });
            for k in ["elements", "package", "to"] {
                if !input[k].is_null() {
                    body[k] = input[k].clone();
                }
            }
            if !input["element"].is_null() && input["elements"].is_null() {
                body["elements"] = json!([input["element"]]);
            }
            let r = client.review_requests(&source, &body)?;
            let mut out = vec![format!("{} review request(s) sent in the hub:", r["created"].as_array().map_or(0, Vec::len))];
            out.extend(r["created"].as_array().into_iter().flatten().map(|c| format!("- {} → {}", c["name"].as_str().unwrap_or(""), c["to"].as_str().unwrap_or(""))));
            out.extend(r["skipped"].as_array().into_iter().flatten().map(|c| format!("- skipped {}: {}", c["name"].as_str().unwrap_or(""), c["why"].as_str().unwrap_or(""))));
            Ok(out.join("\n"))
        }
        "requests" => {
            let list = client.requests(input["all"] == true, input["status"].as_str())?;
            if list.is_empty() {
                return Ok("No review requests.".into());
            }
            Ok(list.iter().map(|c| format!("- {} | {} | {} (id {}) asked by {} to {}: {}{}", c["id"].as_str().unwrap_or(""), c["status"].as_str().unwrap_or(""), c["name"].as_str().unwrap_or(""), c["element"],
                c["user"].as_str().unwrap_or(""), c["to"].as_str().unwrap_or(""), c["text"].as_str().unwrap_or(""),
                c["answer"].as_str().filter(|a| !a.is_empty()).map(|a| format!(" → {a}")).unwrap_or_default())).collect::<Vec<_>>().join("\n"))
        }
        "answer" => {
            let c = client.answer_request(input["id"].as_str().unwrap_or(""), &json!({ "answer": input["status"], "text": input["text"], "decision": input["kind"] == "decision" }))?;
            Ok(format!("{} on {}: {}.", c["id"].as_str().unwrap_or(""), c["name"].as_str().unwrap_or(""), c["status"].as_str().unwrap_or("")))
        }
        other => bail!("unknown action {other}: list, add, status, request, requests or answer"),
    }
}

fn change_sets(input: &Value, ctx: &ToolContext) -> Result<String> {
    let client = hub_client(ctx.cfg)?;
    match input["action"].as_str().unwrap_or("list") {
        "list" => {
            let list = client.changes(input["status"].as_str())?;
            let mut out = vec![format!("Change sets: {}", list.len()), "id | status | title | author | model".into()];
            out.extend(list.iter().map(|c| format!("{} | {} | {} | {} | {}", c["id"].as_str().unwrap_or(""), c["status"].as_str().unwrap_or(""), c["title"].as_str().unwrap_or(""), c["author"].as_str().unwrap_or(""), c["source"].as_str().unwrap_or(""))));
            Ok(out.join("\n"))
        }
        "show" => {
            let c = client.change(input["id"].as_str().unwrap_or(""))?;
            let comments: Vec<String> = c["comments"].as_array().into_iter().flatten().map(|m| format!("- {}: {}", m["user"].as_str().unwrap_or(""), m["text"].as_str().unwrap_or(""))).collect();
            Ok(format!("{} | {} | {} | by {}{}\n{}\n\n{}\n\nComments:\n{}", c["id"].as_str().unwrap_or(""), c["status"].as_str().unwrap_or(""), c["title"].as_str().unwrap_or(""), c["author"].as_str().unwrap_or(""),
                c["decided_by"].as_str().map(|d| format!(", decided by {d}")).unwrap_or_default(), c["summary"].as_str().unwrap_or(""), c["preview"].as_str().unwrap_or(""), if comments.is_empty() { "-".into() } else { comments.join("\n") }))
        }
        "apply" => {
            let id = input["id"].as_str().unwrap_or("").to_string();
            let c = client.change(&id)?;
            if c["status"] == "applying" {
                bail!("{}", crate::i18n::fmt("change set {0} is being applied by {1} (or that attempt stopped): an admin of the hub checks the model and resets it", &[id.clone(), c["claimed_by"].as_str().unwrap_or("").to_string()]));
            }
            if c["status"] != "approved" {
                bail!("{}", crate::i18n::fmt("change set {0} is {1}: only an approved change set can be applied", &[id.clone(), c["status"].as_str().unwrap_or("").to_string()]));
            }
            if c["source"].as_str().unwrap_or("") != hub_source(ctx.cfg) {
                bail!("{}", crate::i18n::fmt("change set {0} is for the model {1}, not for this one", &[id.clone(), c["source"].as_str().unwrap_or("").to_string()]));
            }
            // Claimed first: only one Compass applies a change set, and only that one reports the result.
            client.change_action(&id, "claim", &json!({}))?;
            let result = ea_write(&json!({ "ops": c["ops"] }), ctx);
            let report = json!({ "ok": result.is_ok(), "result": result.as_ref().map(|t| t.chars().take(2000).collect::<String>()).unwrap_or_else(|e| e.to_string()) });
            let mut reported = client.change_action(&id, "applied", &report);
            for wait in [2, 5, 10] {
                if reported.is_ok() {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_secs(wait));
                reported = client.change_action(&id, "applied", &report);
            }
            if let Err(e) = &reported {
                crate::hub::note_failure(&format!("{e:#}"));
            }
            result.map(|text| format!("Change set {id} applied.\n{text}{}", if reported.is_err() { "\n(The hub did not record the result: tell the user to check the change set in the hub.)" } else { "" }))
        }
        other => bail!("{}", crate::i18n::fmt("unknown change_sets action: {0}", &[other.to_string()])),
    }
}

/// Requirements of a document (file, Confluence page or text) matched with the model.
fn trace_requirements(input: &Value, ctx: &ToolContext) -> Result<String> {
    let text = if let Some(file) = input["file"].as_str().filter(|f| !f.trim().is_empty()) {
        let path = read_path(ctx, file, false)?;
        crate::traceability::read_document(&path)?
    } else if let Some(page) = input["confluence_page"].as_str().filter(|p| !p.trim().is_empty()) {
        let connection = confluence_connection(ctx.cfg)?;
        crate::traceability::html_text(&connection.page_body(page)?.1)
    } else {
        input["text"].as_str().filter(|t| !t.trim().is_empty()).with_context(|| crate::i18n::tr("give 'file', 'confluence_page' or 'text'"))?.to_string()
    };
    let requirements = crate::traceability::extract(&text);
    let code = match input["code"].as_str().map(str::trim).filter(|c| !c.is_empty()) {
        Some(c) => Some(read_path(ctx, c, true)?),
        None => None,
    };
    crate::traceability::trace_with_code(ctx.cfg, &requirements, input["package"].as_i64(), code.as_deref())
}

/// EA scripts: list, read, save (approved like a write) and run (approved every time).
fn ea_script(input: &Value, ctx: &ToolContext) -> Result<String> {
    let name = input["name"].as_str().unwrap_or("").to_string();
    match input["action"].as_str().unwrap_or("list") {
        "list" => crate::scripts::list(ctx.cfg),
        "read" => {
            let s = crate::scripts::find(ctx.cfg, &name)?;
            Ok(format!("{}.{} ({})\n{}", s.group, s.name, s.language, s.code))
        }
        "save" => {
            let group = input["group"].as_str().filter(|g| !g.trim().is_empty()).unwrap_or("Compass").to_string();
            let language = input["language"].as_str().unwrap_or("JScript").to_string();
            let code = input["code"].as_str().with_context(|| crate::i18n::fmt("missing field '{0}'", &["code".to_string()]))?.to_string();
            refuse_alias_script(&code)?;
            let (statements, updated) = crate::scripts::save_statements(ctx.cfg, &group, &name, &language, &code)?;
            // A script in the model can be run later by anyone from EA: saving it is always asked, with the whole code.
            ctx.approver.confirm_script("save", &code, &crate::i18n::fmt(SAVE_SCRIPT_QUESTION, &[
                group.clone(), name.clone(), language.clone(), code.lines().count().to_string(),
                if updated { crate::i18n::tr(" - REPLACES the existing script").to_string() } else { String::new() }, script_for_approval(&code)?,
            ]))?;
            let save = [json!({ "op": "execute_sql", "statements": statements })];
            let result = ea::run_batch(ctx.cfg, &save, &BatchOptions::default());
            audit(ctx.cfg, &save, &result);
            result?;
            let saved = crate::scripts::find(ctx.cfg, &format!("{group}.{name}"))?;
            if saved.code.replace("\r\n", "\n").trim() != code.replace("\r\n", "\n").trim() {
                bail!("{}", crate::i18n::tr("the script was not saved correctly (the model has different code)"));
            }
            Ok(format!("Saved and verified the EA script {group}.{name} ({language}, {} lines){}.", code.lines().count(), if updated { ", replacing the previous version" } else { "" }))
        }
        "run" => {
            let s = crate::scripts::find(ctx.cfg, &name)?;
            // The whole code with its includes is shown, and exactly that code runs.
            let resolved = crate::scripts::resolve_includes(ctx.cfg, &s)?;
            refuse_alias_script(&resolved)?;
            ctx.approver.confirm_script("run", &resolved, &crate::i18n::fmt(RUN_SCRIPT_QUESTION, &[s.group.clone(), s.name.clone(), s.language.clone(), script_for_approval(&resolved)?]))?;
            crate::scripts::run(ctx.cfg, &name, &resolved)
        }
        other => bail!("{}", crate::i18n::fmt("unknown ea_script action: {0}", &[other.to_string()])),
    }
}

/// Connection to Jira from the settings, or through the hub when the team policy says so.
fn jira(cfg: &AppConfig) -> Result<crate::jira::Connection> {
    if let Some((hub, token)) = crate::hub::via_hub(cfg, "jira") {
        return Ok(crate::jira::Connection { base_url: format!("{hub}/proxy/jira"), user: String::new(), token, identity: None });
    }
    if cfg.jira_url.trim().is_empty() {
        bail!("{}", crate::i18n::tr("Jira is not configured: enter its address and token in Settings → Jira"));
    }
    // Tests talk to a Jira imitation with a test token (never the user's).
    #[cfg(test)]
    if let Ok(token) = std::env::var("LOONAR_TEST_JIRA_TOKEN") {
        return Ok(crate::jira::Connection { base_url: cfg.jira_url.clone(), user: cfg.jira_user.clone(), token, identity: None });
    }
    let token = crate::config::secrets::get(crate::config::secrets::JIRA_TOKEN).context(crate::i18n::tr("no Jira token in the settings"))?;
    Ok(crate::jira::Connection { base_url: cfg.jira_url.clone(), user: cfg.jira_user.clone(), token, identity: crate::client_cert::jira(cfg)? })
}

/// Model elements and Jira issues in sync (LC-403): check reads only; to_model goes through ea_write (approval card,
/// undo); to_jira sends after the user approves the list of changes.
fn jira_sync(input: &Value, ctx: &ToolContext) -> Result<String> {
    let action = crate::jira_sync::check_action(input)?;
    let connection = jira(ctx.cfg)?;
    let links = crate::jira_sync::linked(ctx.cfg, input, &connection)?;
    match action {
        "to_model" => {
            let ops = crate::jira_sync::to_model_ops(&links, input);
            if ops.is_empty() {
                return Ok(format!("Nothing to take from Jira.\n{}", crate::jira_sync::report(&links, input)));
            }
            ea_write(&json!({ "ops": ops }), ctx)
        }
        "to_jira" => {
            let changes = crate::jira_sync::to_jira_changes(&links, input);
            if changes.is_empty() {
                return Ok(format!("Nothing to send to Jira.\n{}", crate::jira_sync::report(&links, input)));
            }
            let lines: Vec<String> = changes.iter().map(|c| c.3.clone()).collect();
            ctx.approver.confirm_kind("jira", &crate::i18n::fmt("Change {0} Jira issue(s) to match the model?\n\n{1}", &[changes.len().to_string(), lines.join("\n")]))?;
            Ok(crate::jira_sync::apply_to_jira(&connection, &changes))
        }
        _ => Ok(crate::jira_sync::report(&links, input)),
    }
}

/// New Jira issue: always with the user's consent (the content leaves the application).
fn jira_create_issue(input: &Value, ctx: &ToolContext) -> Result<String> {
    let connection = jira(ctx.cfg)?;
    let team_project = crate::hub::team_default(ctx.cfg, "jira_project").unwrap_or_default();
    let own_project = if ctx.cfg.jira_project.trim().is_empty() { team_project.as_str() } else { ctx.cfg.jira_project.trim() };
    let project = input["project"].as_str().map(str::trim).filter(|p| !p.is_empty()).unwrap_or(own_project).to_string();
    if project.is_empty() {
        bail!("{}", crate::i18n::tr("no Jira project: give 'project' or set the default project in Settings → Jira"));
    }
    let summary = input["summary"].as_str().unwrap_or("").trim().to_string();
    let issue_type = input["type"].as_str().unwrap_or("Task").to_string();
    let mut description = input["description"].as_str().unwrap_or("").to_string();
    let files = jira_files(ctx, input)?;
    // Jira Server / Data Center shows attached images in the description through wiki markup.
    if !connection.cloud() {
        let markup = jira_image_markup(&files.iter().map(|(n, _)| n.clone()).collect::<Vec<_>>(), input["image_width"].as_u64());
        if !markup.is_empty() {
            description = format!("{}\n\n{markup}", description.trim_end());
        }
    }
    let labels: Vec<String> = input["labels"].as_array().into_iter().flatten().filter_map(|v| v.as_str()).map(String::from).collect();
    let parent = input["parent"].as_str();
    let link = input["link_url"].as_str().filter(|s| !s.trim().is_empty());
    let mut extra = Vec::new();
    if !labels.is_empty() {
        extra.push(format!("labels: {}", labels.join(", ")));
    }
    if let Some(p) = parent {
        extra.push(format!("parent: {p}"));
    }
    if let Some(l) = link {
        extra.push(format!("🔗 {l}"));
    }
    if !files.is_empty() {
        extra.push(jira_files_lines(&files));
    }
    ctx.approver.confirm_kind("jira", &crate::i18n::fmt(JIRA_CREATE_QUESTION, &[project.clone(), issue_type.clone(), summary.clone(), format!("{}{}", shorten_for_approval(&description), if extra.is_empty() { String::new() } else { format!("\n\n{}", extra.join("\n")) })]))?;
    let (key, url) = connection.create_issue(&project, &issue_type, &summary, &description, &labels, parent)?;
    let attached = jira_upload(&connection, &key, files)?;
    if let Some(link) = link {
        connection.add_web_link(&key, link, input["link_title"].as_str().unwrap_or(link))?;
    }
    let note = if attached.is_empty() { String::new() } else { format!(" {}", crate::i18n::fmt("Attached: {0}", &[attached.join(", ")])) };
    Ok(format!("{}{note}", crate::i18n::fmt("Created the Jira issue {0}: {1}", &[key, url])))
}

/// Comment or web link on an existing issue: always with the user's consent.
/// The files to attach to a Jira issue (LC-304): diagram:ID (an image exported from the model), image:NAME or file:NAME (a
/// file of the conversation's attachments). Returns (file name, bytes).
fn jira_files(ctx: &ToolContext, input: &Value) -> Result<Vec<(String, Vec<u8>)>> {
    let mut files = Vec::new();
    for source in input["attachments"].as_array().into_iter().flatten().filter_map(|v| v.as_str()).map(str::trim).filter(|s| !s.is_empty()) {
        if let Some(id) = source.strip_prefix("diagram:") {
            let id: i64 = id.trim().parse().context(crate::i18n::tr("diagram id"))?;
            let name = query_rows(ctx.cfg, &format!("select name from t_diagram where diagram_id = {id}"))?
                .first()
                .and_then(|r| r.first().cloned())
                .with_context(|| crate::i18n::fmt("there is no diagram with id {0}", &[id.to_string()]))?;
            files.push((format!("{}.png", crate::history::sanitize_name(&name)), picture(ctx, source)?));
        } else if let Some(stored) = source.strip_prefix("image:").or_else(|| source.strip_prefix("file:")) {
            let stored = std::path::Path::new(stored.trim()).file_name().context(crate::i18n::tr("image name"))?.to_string_lossy().to_string();
            let bytes = std::fs::read(crate::history::attachment_path(&ctx.cfg.attachments_dir, &stored)).with_context(|| crate::i18n::fmt("there is no file {0} in this conversation", &[stored.clone()]))?;
            files.push((crate::history::display_name(&stored).to_string(), bytes));
        } else {
            bail!("{}", crate::i18n::fmt("unknown attachment '{0}' (use diagram:ID, image:FILE_NAME or file:FILE_NAME)", &[source.to_string()]));
        }
    }
    if files.len() > 20 {
        bail!("{}", crate::i18n::tr("at most 20 files at once"));
    }
    Ok(files)
}

thread_local! {
    /// Downloaded images the model already saw in this answer (an answer runs on one thread): not shown twice.
    static SHOWN_IN_ANSWER: std::cell::RefCell<std::collections::HashSet<String>> = std::cell::RefCell::new(std::collections::HashSet::new());
}

/// A new answer: images shown in earlier answers are not in the model's view any more (the history keeps texts).
pub fn begin_answer() {
    SHOWN_IN_ANSWER.with(|s| s.borrow_mut().clear());
}

/// Aliases of attributes and operations are written by ea_write, not by a script (found in the 0.9.2 lab tests: a
/// small model wrote alias scripts again and again, each asking for approval, and the aliases were not saved).
fn refuse_alias_script(code: &str) -> Result<()> {
    let lower = code.to_lowercase();
    let sets_alias = [".alias", ".styleex"].iter().any(|p| lower.split(p).skip(1).any(|rest| rest.trim_start().starts_with('=') && !rest.trim_start().starts_with("==")));
    if sets_alias && (lower.contains("attribute") || lower.contains("method") || lower.contains("operation")) {
        bail!("Not saved and not run: aliases of attributes and operations are set with ea_write, without a script and without approval cards: \
update_attribute {{attribute, alias}} and update_operation {{operation, alias}} (ids from t_attribute.id and t_operation.operationid; many in one ea_write call). \
Element aliases: update_element {{element, alias}}.");
    }
    Ok(())
}

/// Images the model sees from one jira_download_attachments call (each costs about 1 500 tokens in every later round).
const DOWNLOADED_IMAGES_SHOWN: usize = 4;

/// Downloads attachments of an issue into the conversation's files (LC-309).
fn jira_download_attachments(input: &Value, ctx: &ToolContext) -> Result<ToolOutput> {
    let key = input["key"].as_str().with_context(|| crate::i18n::fmt("missing field '{0}'", &["key".to_string()]))?.trim().to_string();
    let wanted: Vec<String> = input["names"].as_array().into_iter().flatten().filter_map(|v| v.as_str()).map(|n| n.trim().to_lowercase()).filter(|n| !n.is_empty()).collect();
    let connection = jira(ctx.cfg)?;
    let all = connection.attachments(&key)?;
    let chosen: Vec<_> = all.iter().filter(|(name, _, _)| wanted.is_empty() || wanted.contains(&name.to_lowercase())).take(20).collect();
    if chosen.is_empty() {
        let names = all.iter().map(|(n, _, _)| n.as_str()).collect::<Vec<_>>().join(", ");
        return Ok(format!("{key} has no such attachments. Attachments: {}", if names.is_empty() { "none" } else { &names }).into());
    }
    std::fs::create_dir_all(&ctx.cfg.attachments_dir).context(crate::i18n::tr("attachments folder"))?;
    let (mut lines, mut images) = (Vec::new(), Vec::new());
    for (name, url, _) in chosen {
        // Downloaded earlier in this conversation: the same file again, not shown again (the model saw it already;
        // found in the 0.9.2 tests: four downloads of one issue doubled the prompt).
        let suffix = format!("_{}_{}", key, crate::history::sanitize_name(name));
        let earlier = std::fs::read_dir(&ctx.cfg.attachments_dir).ok().into_iter().flatten().flatten()
            .map(|e| e.file_name().to_string_lossy().to_string())
            .find(|f| f.len() == 32 + suffix.len() && f.ends_with(&suffix));
        if let Some(stored) = earlier {
            let lower = name.to_lowercase();
            let image = [".png", ".jpg", ".jpeg", ".gif"].iter().any(|e| lower.ends_with(e));
            let seen = SHOWN_IN_ANSWER.with(|s| s.borrow().contains(&stored));
            if image && !seen && images.len() < DOWNLOADED_IMAGES_SHOWN {
                SHOWN_IN_ANSWER.with(|s| s.borrow_mut().insert(stored.clone()));
                images.push(stored.clone());
                lines.push(format!("- {name}: image:{stored}"));
            } else {
                lines.push(format!("- {name}: {}:{stored}{}", if image { "image" } else { "file" }, if seen { " (you saw it earlier in this answer)" } else { "" }));
            }
            continue;
        }
        match connection.download(url) {
            Ok(bytes) => {
                let stored = format!("{}_{}_{}", uuid::Uuid::new_v4().simple(), key, crate::history::sanitize_name(name));
                std::fs::write(ctx.cfg.attachments_dir.join(&stored), &bytes).with_context(|| crate::i18n::fmt("saving {0}", &[name.clone()]))?;
                let lower = name.to_lowercase();
                let image = [".png", ".jpg", ".jpeg", ".gif"].iter().any(|e| lower.ends_with(e));
                if image && images.len() < DOWNLOADED_IMAGES_SHOWN {
                    SHOWN_IN_ANSWER.with(|s| s.borrow_mut().insert(stored.clone()));
                    images.push(stored.clone());
                }
                lines.push(format!("- {name}: {}:{stored}", if image { "image" } else { "file" }));
            }
            Err(e) => lines.push(format!("- {name}: not downloaded ({e:#})")),
        }
    }
    let note = if images.len() == DOWNLOADED_IMAGES_SHOWN {
        format!(" You see the first {DOWNLOADED_IMAGES_SHOWN} images; the others are in the conversation by their names: use them without downloading again.")
    } else {
        " Use these names; do not download them again.".to_string()
    };
    Ok(ToolOutput { text: format!("Attachments of {key} in this conversation:\n{}{note}", lines.join("\n")), images, files: Vec::new() })
}

/// The files of `jira_files` as lines of the approval question.
fn jira_files_lines(files: &[(String, Vec<u8>)]) -> String {
    files.iter().map(|(name, bytes)| format!("📎 {name} ({} KB)", bytes.len().div_ceil(1024))).collect::<Vec<_>>().join("\n")
}

/// Uploads the files to the issue; on Jira Server / Data Center the images are also shown in the description (wiki
/// markup !name!) when `show_in` holds the description to extend. Returns the names Jira stored.
fn jira_upload(connection: &crate::jira::Connection, key: &str, files: Vec<(String, Vec<u8>)>) -> Result<Vec<String>> {
    let mut stored = Vec::new();
    for (name, bytes) in files {
        stored.push(connection.add_attachment(key, &name, bytes)?);
    }
    Ok(stored)
}

/// Wiki markup that shows the images in the description of a Jira Server / Data Center issue.
/// `width`: the width in pixels the image is shown with; None = its own size (LC-305: thumbnails were too small to read).
fn jira_image_markup(names: &[String], width: Option<u64>) -> String {
    names
        .iter()
        .filter(|n| matches!(n.to_lowercase().rsplit('.').next(), Some("png" | "jpg" | "jpeg" | "gif")))
        .map(|n| match width {
            Some(w) => format!("!{n}|width={w}!"),
            None => format!("!{n}!"),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn jira_attach(input: &Value, ctx: &ToolContext) -> Result<String> {
    let connection = jira(ctx.cfg)?;
    let key = input["key"].as_str().with_context(|| crate::i18n::fmt("missing field '{0}'", &["key".to_string()]))?.trim().to_string();
    let files = jira_files(ctx, input)?;
    if files.is_empty() {
        bail!("{}", crate::i18n::tr("no files to attach"));
    }
    ctx.approver.confirm_kind("jira", &crate::i18n::fmt(JIRA_ATTACH_QUESTION, &[key.clone(), jira_files_lines(&files)]))?;
    let names: Vec<String> = files.iter().map(|(n, _)| n.clone()).collect();
    let stored = jira_upload(&connection, &key, files)?;
    // Jira Server / Data Center: the images also at the end of the description (unless the model says not to).
    if !connection.cloud() && input["show_in_description"] != false {
        let markup = jira_image_markup(&stored, input["image_width"].as_u64());
        if !markup.is_empty() {
            let current = connection.description(&key)?;
            connection.update_issue(&key, None, Some(&format!("{}

{markup}", current.trim_end())), &[], &[])?;
        }
    }
    let _ = names;
    Ok(format!("{} ({})", crate::i18n::fmt("Attached to the Jira issue {0}: {1}", &[key.clone(), stored.join(", ")]), connection.browse(&key)))
}

fn jira_update_issue(input: &Value, ctx: &ToolContext) -> Result<String> {
    let connection = jira(ctx.cfg)?;
    let key = input["key"].as_str().with_context(|| crate::i18n::fmt("missing field '{0}'", &["key".to_string()]))?.trim().to_string();
    let summary = input["summary"].as_str().map(str::trim).filter(|s| !s.is_empty());
    let files = jira_files(ctx, input)?;
    let with_images;
    let description = match input["description"].as_str() {
        // A new description on Jira Server / Data Center shows the attached images too.
        Some(d) if !connection.cloud() && !jira_image_markup(&files.iter().map(|(n, _)| n.clone()).collect::<Vec<_>>(), input["image_width"].as_u64()).is_empty() => {
            with_images = format!("{}\n\n{}", d.trim_end(), jira_image_markup(&files.iter().map(|(n, _)| n.clone()).collect::<Vec<_>>(), input["image_width"].as_u64()));
            Some(with_images.as_str())
        }
        other => other,
    };
    let list = |name: &str| -> Vec<String> { input[name].as_array().into_iter().flatten().filter_map(|v| v.as_str()).map(String::from).collect() };
    let (add, remove) = (list("add_labels"), list("remove_labels"));
    let mut what = Vec::new();
    if let Some(s) = summary {
        what.push(crate::i18n::fmt("summary: {0}", &[s.to_string()]));
    }
    if let Some(d) = description {
        what.push(crate::i18n::fmt("new description:\n{0}", &[shorten_for_approval(d)]));
    }
    if !add.is_empty() {
        what.push(crate::i18n::fmt("add labels: {0}", &[add.join(", ")]));
    }
    if !remove.is_empty() {
        what.push(crate::i18n::fmt("remove labels: {0}", &[remove.join(", ")]));
    }
    if !files.is_empty() {
        what.push(jira_files_lines(&files));
    }
    ctx.approver.confirm_kind("jira", &crate::i18n::fmt(JIRA_UPDATE_QUESTION, &[key.clone(), what.join("\n")]))?;
    if summary.is_some() || description.is_some() || !add.is_empty() || !remove.is_empty() {
        connection.update_issue(&key, summary, description, &add, &remove)?;
    }
    jira_upload(&connection, &key, files)?;
    Ok(crate::i18n::fmt("Changed the Jira issue {0}: {1}", &[key.clone(), connection.browse(&key)]))
}

fn jira_comment(input: &Value, ctx: &ToolContext) -> Result<String> {
    let connection = jira(ctx.cfg)?;
    let key = input["key"].as_str().with_context(|| crate::i18n::fmt("missing field '{0}'", &["key".to_string()]))?.trim().to_string();
    let comment = input["comment"].as_str().unwrap_or("").trim().to_string();
    let link = input["link_url"].as_str().map(str::trim).filter(|s| !s.is_empty()).map(String::from);
    if comment.is_empty() && link.is_none() {
        bail!("{}", crate::i18n::tr("give 'comment' or 'link_url'"));
    }
    let mut what = Vec::new();
    if !comment.is_empty() {
        what.push(shorten_for_approval(&comment));
    }
    if let Some(link) = &link {
        what.push(format!("🔗 {link}"));
    }
    ctx.approver.confirm_kind("jira", &crate::i18n::fmt(JIRA_ADD_QUESTION, &[key.clone(), what.join("\n")]))?;
    if !comment.is_empty() {
        connection.add_comment(&key, &comment)?;
    }
    if let Some(link) = &link {
        connection.add_web_link(&key, link, input["link_title"].as_str().unwrap_or(link))?;
    }
    Ok(crate::i18n::fmt("Updated the Jira issue {0}: {1}", &[key.clone(), connection.browse(&key)]))
}

/// The text in an approval card: whole up to 20 000 characters (the card scrolls). LC-355: 600 characters let
/// instructions hidden further in a description go unseen.
fn shorten_for_approval(text: &str) -> String {
    const MAX: usize = 20_000;
    if text.chars().count() > MAX { format!("{}…", text.chars().take(MAX).collect::<String>()) } else { text.to_string() }
}

fn create_confluence_page(input: &Value, ctx: &ToolContext) -> Result<ToolOutput> {
    let title = input["title"].as_str().with_context(|| crate::i18n::fmt("missing field '{0}'", &["title".to_string()]))?;
    let markdown = input["markdown"].as_str().with_context(|| crate::i18n::fmt("missing field '{0}'", &["markdown".to_string()]))?;
    let checked = doc_discipline(input, markdown, ctx)?;
    std::fs::create_dir_all(&ctx.cfg.attachments_dir).context(crate::i18n::tr("attachments folder"))?;
    let (storage, attachments) = crate::confluence::to_storage(markdown, |source| picture(ctx, source))?;
    let stem = format!("{}_{}", uuid::Uuid::new_v4().simple(), crate::history::sanitize_name(title));
    let stored = format!("{stem}.confluence.xml");
    let dir = page_images_dir(&ctx.cfg.attachments_dir, &stored);
    std::fs::create_dir_all(&dir).context(crate::i18n::tr("page images folder"))?;
    for a in &attachments {
        std::fs::write(dir.join(&a.file_name), &a.png).with_context(|| crate::i18n::fmt("saving {0}", &[a.file_name.clone()]))?;
    }
    std::fs::write(ctx.cfg.attachments_dir.join(format!("{stem}.confluence.md")), format!("{TITLE_MARKER} {title} -->\n{markdown}")).context(crate::i18n::tr("saving the page source"))?;
    std::fs::write(ctx.cfg.attachments_dir.join(&stored), &storage).context(crate::i18n::tr("saving the page code"))?;
    // Confluence of the team hub ("confluence_via_hub") counts as configured too (LC-263).
    let publish_hint = if ctx.cfg.confluence_url.trim().is_empty() && crate::hub::via_hub(ctx.cfg, "confluence").is_none() {
        "Publishing through the API is not configured (Settings → Confluence), so the user pastes the code into the source editor."
    } else {
        "You can publish it with the publish_confluence tool (page = the file name above) when the user asks for it."
    };
    Ok(ToolOutput {
        text: format!(
            "Page \"{title}\" is ready: file {stored} ({} KB of code, images: {}). In the chat the user sees a card with the buttons \
             \"{}\" (to paste into the Confluence source editor) and \"{}\" (to attach the images). {publish_hint} {checked}",
            storage.len() / 1024,
            attachments.len(),
            crate::i18n::tr("Copy page code"),
            crate::i18n::tr("Open images folder")
        ),
        images: Vec::new(),
        files: vec![stored],
    })
}

pub(crate) fn confluence_connection(cfg: &AppConfig) -> Result<crate::confluence::Connection> {
    if let Some((hub, token)) = crate::hub::via_hub(cfg, "confluence") {
        return Ok(crate::confluence::Connection { base_url: format!("{hub}/proxy/confluence"), user: String::new(), token, identity: None });
    }
    if cfg.confluence_url.trim().is_empty() {
        bail!("{}", crate::i18n::tr("Confluence is not configured: complete Settings → Confluence (address, space, token)"));
    }
    let token = crate::config::secrets::get(crate::config::secrets::CONFLUENCE_TOKEN).context(crate::i18n::tr("no Confluence token in the settings"))?;
    Ok(crate::confluence::Connection { base_url: cfg.confluence_url.clone(), user: cfg.confluence_user.clone(), token, identity: crate::client_cert::confluence(cfg)? })
}

/// Publishes or updates a page in Confluence. Diagrams are exported from EA again, so the page shows the current model.
/// The Markdown source of a page made by create_confluence_page. The model often gives the name without the unique
/// prefix, with .md instead of .xml, or even the page title (LC-271), so a missing file is looked up by the end of its
/// name (also as the title turned into a file name) or by the title saved in it (newest first).
fn page_source(dir: &std::path::Path, page: &str) -> std::path::PathBuf {
    let stem = page.trim_end_matches(".confluence.xml").trim_end_matches(".confluence.md").trim_end_matches(".xml").trim_end_matches(".md").trim();
    let exact = dir.join(format!("{stem}.confluence.md"));
    if exact.is_file() {
        return exact;
    }
    let ends = [format!("{}.confluence.md", stem.to_lowercase()), format!("{}.confluence.md", crate::history::sanitize_name(stem).to_lowercase())];
    let titled = |path: &std::path::Path| {
        std::fs::read_to_string(path).ok().and_then(|s| s.lines().next().map(str::to_string)).is_some_and(|first| {
            let title = first.trim().trim_start_matches(TITLE_MARKER).trim_start_matches(LEGACY_TITLE_MARKER).trim_end_matches("-->").trim().to_lowercase();
            title == stem.to_lowercase()
        })
    };
    std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| {
            let name = e.file_name().to_string_lossy().to_lowercase();
            name.ends_with(".confluence.md") && (ends.iter().any(|w| name.ends_with(w.as_str())) || titled(&e.path()))
        })
        .max_by_key(|e| e.metadata().and_then(|m| m.modified()).ok())
        .map(|e| e.path())
        .unwrap_or(exact)
}

fn publish_confluence(input: &Value, ctx: &ToolContext) -> Result<ToolOutput> {
    let page = input["page"].as_str().context(crate::i18n::tr("missing field 'page' (the .confluence.xml file from create_confluence_page)"))?;
    let page = std::path::Path::new(page.trim()).file_name().context(crate::i18n::tr("page file name"))?.to_string_lossy().to_string();
    let source_file = page_source(&ctx.cfg.attachments_dir, &page);
    let page = source_file.file_name().map(|n| n.to_string_lossy().replace(".confluence.md", ".confluence.xml")).unwrap_or(page);
    let source = std::fs::read_to_string(&source_file).with_context(|| crate::i18n::fmt("the page source {0} does not exist", &[source_file.display().to_string()]))?;
    let (title_line, markdown) = source.split_once('\n').unwrap_or(("", &source));
    let saved_title = title_line.trim().trim_start_matches(TITLE_MARKER).trim_start_matches(LEGACY_TITLE_MARKER).trim_end_matches("-->").trim().to_string();
    let title = input["title"].as_str().map(str::to_string).unwrap_or(saved_title);
    let space = input["space"].as_str().map(str::to_string).unwrap_or_else(|| {
        if ctx.cfg.confluence_space.trim().is_empty() { crate::hub::team_default(ctx.cfg, "confluence_space").unwrap_or_default() } else { ctx.cfg.confluence_space.clone() }
    });
    if space.trim().is_empty() {
        bail!("{}", crate::i18n::tr("specify the Confluence space (space) or set a default one in the settings"));
    }
    let parent = input["parent_id"].as_str().map(str::to_string).or_else(|| input["parent_id"].as_i64().map(|i| i.to_string())).unwrap_or_else(|| ctx.cfg.confluence_parent.clone());
    let page_id = input["page_id"].as_str().map(str::to_string).or_else(|| input["page_id"].as_i64().map(|i| i.to_string()));
    let connection = confluence_connection(ctx.cfg)?;
    // The approval dialog says plainly whether an existing page will be overwritten (also one found by its title).
    let existing = connection.existing_page(&space, &title, page_id.as_deref())?;
    let question = match &existing {
        Some(page) => crate::i18n::fmt(CONFLUENCE_REPLACE_QUESTION, &[page.title.clone(), page.id.clone(), page.version.to_string(), space.clone()]),
        None => crate::i18n::fmt(CONFLUENCE_NEW_QUESTION, &[title.clone(), space.clone()]),
    };
    // The card shows what will be published, not only where (LC-355).
    ctx.approver.confirm_kind("confluence", &format!("{question}

{}", shorten_for_approval(markdown)))?;
    // Diagrams under their GUID (LC-426): publishing again replaces the same attachments whatever the captions.
    let guid_name = |source: &str| -> Option<String> {
        let id: i64 = source.strip_prefix("diagram:")?.trim().parse().ok()?;
        let guid = query_rows(ctx.cfg, &format!("select ea_guid from t_diagram where diagram_id = {id}")).ok()?.first()?[0].clone();
        Some(format!("diagram-{}.png", guid.trim_matches(['{', '}'])))
    };
    let (storage, attachments) = crate::confluence::to_storage_named(markdown, |source| picture(ctx, source), guid_name)?;
    let target_id = existing.as_ref().map(|page| page.id.clone());
    let (id, link, created) = connection.publish(&space, &title, Some(&parent), target_id.as_deref(), &storage, &attachments)?;
    // What it shows of the model, to tell later when it is no longer current (LC-386), and how to publish it again (LC-426).
    crate::publications::record_with(ctx.cfg, "Confluence page", &title, &link, markdown, json!({ "page": page, "space": space, "page_id": id }));
    crate::confluence::remember(
        &ctx.cfg.confluence_pages_file,
        json!({ "page_id": id, "title": title, "space": space, "url": link, "source": page, "published": chrono::Local::now().to_rfc3339() }),
    );
    Ok(format!(
        "{} page \"{title}\" in Confluence ({space}), attachments: {}. URL: {link}",
        if created { "Created" } else { "Updated" },
        attachments.len()
    )
    .into())
}

/// List of pages published by the assistant (for refreshing after changes in the model).
/// The text of a Confluence page (by id, or by title in a space), shortened for the model.
fn confluence_read(input: &Value, cfg: &AppConfig) -> Result<String> {
    const LONGEST: usize = 40_000;
    let connection = confluence_connection(cfg)?;
    let page_id = match input["page_id"].as_str().map(str::trim).filter(|id| !id.is_empty()) {
        Some(id) => id.to_string(),
        None => {
            let title = input["title"].as_str().map(str::trim).filter(|t| !t.is_empty()).context(crate::i18n::tr("give 'page_id' or 'title' of the page"))?;
            let space = input["space"].as_str().map(str::trim).filter(|s| !s.is_empty()).map(str::to_string).unwrap_or_else(|| {
                if cfg.confluence_space.trim().is_empty() { crate::hub::team_default(cfg, "confluence_space").unwrap_or_default() } else { cfg.confluence_space.clone() }
            });
            if space.trim().is_empty() {
                bail!("{}", crate::i18n::tr("specify the Confluence space (space) or set a default one in the settings"));
            }
            connection.existing_page(&space, title, None)?.with_context(|| crate::i18n::fmt("no page titled \"{0}\" in the space {1}", &[title.to_string(), space.clone()]))?.id
        }
    };
    let (title, body) = connection.page_body(&page_id)?;
    let text = crate::web::html_text(&body);
    let shortened: String = text.chars().take(LONGEST).collect();
    let more = if shortened.len() < text.len() { "\n[…the rest of the page left out…]" } else { "" };
    Ok(format!("Confluence page \"{title}\" (page_id {page_id}):\n\n{shortened}{more}"))
}

fn confluence_pages(cfg: &AppConfig) -> Result<String> {
    let pages = crate::confluence::published(&cfg.confluence_pages_file);
    if pages.is_empty() {
        return Ok("The assistant has not published any page yet.".into());
    }
    let mut out = String::from("Published pages (page_id | title | space | source file | last published | URL):\n");
    for p in pages {
        out.push_str(&format!(
            "{} | {} | {} | {} | {} | {}\n",
            p["page_id"].as_str().unwrap_or(""),
            p["title"].as_str().unwrap_or(""),
            p["space"].as_str().unwrap_or(""),
            p["source"].as_str().unwrap_or(""),
            p["published"].as_str().unwrap_or("").chars().take(16).collect::<String>().replace('T', " "),
            p["url"].as_str().unwrap_or("")
        ));
    }
    out.push_str("To refresh a page after changes in the model: publish_confluence with page = the source file and page_id (diagrams are exported again).");
    Ok(out)
}

/// Builds a Word document from Markdown, with diagrams from EA and images drawn earlier.
fn create_document(input: &Value, ctx: &ToolContext) -> Result<ToolOutput> {
    let markdown = input["markdown"].as_str().with_context(|| crate::i18n::fmt("missing field '{0}'", &["markdown".to_string()]))?;
    let file_name = input["file_name"].as_str().unwrap_or(crate::i18n::tr("Document"));
    let title = input["title"].as_str().unwrap_or("");
    let checked = doc_discipline(input, markdown, ctx)?;
    std::fs::create_dir_all(&ctx.cfg.attachments_dir).context(crate::i18n::tr("attachments folder"))?;
    let mut pictures = 0;
    let bytes = crate::docx::build(title, markdown, |source, caption| {
        pictures += 1;
        let png = picture(ctx, source)?;
        Ok(crate::docx::Picture { png, caption: caption.to_string() })
    })?;
    let stored = format!("{}_{}.docx", uuid::Uuid::new_v4().simple(), crate::history::sanitize_name(file_name.trim_end_matches(".docx")));
    crate::publications::record(ctx.cfg, "Word document", if title.is_empty() { file_name } else { title }, file_name, markdown);
    std::fs::write(ctx.cfg.attachments_dir.join(&stored), &bytes).context(crate::i18n::tr("saving the document"))?;
    let format = input["format"].as_str().unwrap_or("docx");
    if format == "pdf" || format == "both" {
        let pdf = stored.trim_end_matches(".docx").to_string() + ".pdf";
        crate::office::docx_to_pdf(&ctx.cfg.attachments_dir.join(&stored), &ctx.cfg.attachments_dir.join(&pdf)).context(crate::i18n::tr("making the PDF with Microsoft Word"))?;
        let mut files = vec![pdf];
        if format == "both" {
            files.push(stored);
        } else {
            let _ = std::fs::remove_file(ctx.cfg.attachments_dir.join(&stored));
        }
        return Ok(ToolOutput { text: format!("Document \"{file_name}\" is ready as PDF{} (images: {pictures}). The user sees it in the chat. {checked}", if format == "both" { " and DOCX" } else { "" }), images: Vec::new(), files });
    }
    Ok(ToolOutput {
        text: format!(
            "Document \"{file_name}.docx\" is ready ({} KB, images: {pictures}). The user sees it in the chat with the \"Open\" and \"Save as…\" buttons. It is kept only in Compass: do not name a folder or path for it; the user chooses where to keep it with \"Save as…\". {checked}",
            bytes.len() / 1024
        ),
        images: Vec::new(),
        files: vec![stored],
    })
}

/// Builds a PowerPoint presentation (and a PDF of it with PowerPoint, if asked) and shows it as a file in the chat.
fn create_presentation(input: &Value, ctx: &ToolContext) -> Result<ToolOutput> {
    let file_name = input["file_name"].as_str().unwrap_or(crate::i18n::tr("Presentation"));
    let title = input["title"].as_str().unwrap_or(file_name);
    let subtitle = input["subtitle"].as_str().unwrap_or("");
    let mut slides = Vec::new();
    for s in input["slides"].as_array().with_context(|| crate::i18n::fmt("missing list '{0}'", &["slides".to_string()]))?.iter().take(60) {
        let bullets = s["bullets"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|b| match b {
                Value::String(t) => Some((t.clone(), 0u8)),
                Value::Object(_) => Some((b["text"].as_str()?.to_string(), b["level"].as_u64().unwrap_or(0).min(1) as u8)),
                _ => None,
            })
            .take(14)
            .collect();
        let picture = match s["image"].as_str().map(str::trim).filter(|i| !i.is_empty()) {
            Some(source) => Some(picture(ctx, source)?),
            None => None,
        };
        slides.push(crate::pptx::Slide { title: s["title"].as_str().unwrap_or("").to_string(), bullets, picture, notes: s["notes"].as_str().unwrap_or("").to_string() });
    }
    let bytes = crate::pptx::build(title, subtitle, &slides)?;
    std::fs::create_dir_all(&ctx.cfg.attachments_dir).context(crate::i18n::tr("attachments folder"))?;
    let stored = format!("{}_{}.pptx", uuid::Uuid::new_v4().simple(), crate::history::sanitize_name(file_name.trim_end_matches(".pptx")));
    std::fs::write(ctx.cfg.attachments_dir.join(&stored), &bytes).context(crate::i18n::tr("saving the document"))?;
    let mut files = vec![stored.clone()];
    if input["pdf"] == true {
        let pdf = stored.trim_end_matches(".pptx").to_string() + ".pdf";
        crate::office::pptx_to_pdf(&ctx.cfg.attachments_dir.join(&stored), &ctx.cfg.attachments_dir.join(&pdf)).context(crate::i18n::tr("making the PDF with Microsoft PowerPoint"))?;
        files.push(pdf);
    }
    Ok(ToolOutput {
        text: format!("Presentation \"{file_name}.pptx\" is ready: {} slides{}. The user sees it in the chat with the \"Open\" and \"Save as…\" buttons. It is kept only in Compass: do not name a folder or path for it; the user chooses where to keep it with \"Save as…\".", slides.len() + 1, if files.len() > 1 { ", also as PDF" } else { "" }),
        images: Vec::new(),
        files,
    })
}

/// A new MDG technology file from the user's description (LC-409), shown as a file in the chat.
fn mdg_build(input: &Value, ctx: &ToolContext) -> Result<ToolOutput> {
    let (xml, text) = crate::mdg::build_report(&input["spec"], ctx.cfg)?;
    std::fs::create_dir_all(&ctx.cfg.attachments_dir).context(crate::i18n::tr("attachments folder"))?;
    let id = input["spec"]["id"].as_str().unwrap_or("MDG").trim();
    let stored = format!("{}_{}.xml", uuid::Uuid::new_v4().simple(), crate::history::sanitize_name(id));
    std::fs::write(ctx.cfg.attachments_dir.join(&stored), xml.as_bytes()).context(crate::i18n::tr("saving the document"))?;
    Ok(ToolOutput { text, images: Vec::new(), files: vec![stored] })
}

/// Builds an Excel workbook from data given by the model (e.g. db_query results) and shows it as a file in the chat.
fn create_spreadsheet(input: &Value, ctx: &ToolContext) -> Result<ToolOutput> {
    let file_name = input["file_name"].as_str().unwrap_or(crate::i18n::tr("Workbook"));
    let sheets: Vec<crate::xlsx::Sheet> = input["sheets"]
        .as_array()
        .with_context(|| crate::i18n::fmt("missing list '{0}'", &["sheets".to_string()]))?
        .iter()
        .map(|s| crate::xlsx::Sheet {
            name: s["name"].as_str().unwrap_or("").to_string(),
            columns: s["columns"].as_array().map(|c| c.iter().map(|v| v.as_str().map(str::to_string).unwrap_or_else(|| v.to_string())).collect()).unwrap_or_default(),
            rows: s["rows"].as_array().map(|r| r.iter().map(|row| row.as_array().cloned().unwrap_or_default()).collect()).unwrap_or_default(),
        })
        .collect();
    let bytes = crate::xlsx::build(&sheets)?;
    std::fs::create_dir_all(&ctx.cfg.attachments_dir).context(crate::i18n::tr("attachments folder"))?;
    let stored = format!("{}_{}.xlsx", uuid::Uuid::new_v4().simple(), crate::history::sanitize_name(file_name.trim_end_matches(".xlsx")));
    std::fs::write(ctx.cfg.attachments_dir.join(&stored), &bytes).context(crate::i18n::tr("saving the workbook"))?;
    let rows: usize = sheets.iter().map(|s| s.rows.len()).sum();
    Ok(ToolOutput {
        text: format!("Workbook \"{file_name}.xlsx\" is ready (sheets: {}, rows: {rows}). The user sees it in the chat with the \"Open\" and \"Save as…\" buttons. It is kept only in Compass: do not name a folder or path for it; the user chooses where to keep it with \"Save as…\".", sheets.len()),
        images: Vec::new(),
        files: vec![stored],
    })
}

/// Exports a diagram from EA to PNG (through COM) and returns it as an image for the model and for the chat.
fn ea_show_diagram(input: &Value, ctx: &ToolContext) -> Result<ToolOutput> {
    let id = input["diagram_id"].as_i64().with_context(|| crate::i18n::fmt("missing field '{0}'", &["diagram_id".to_string()]))?;
    let rows = query_rows(ctx.cfg, &format!("select name from t_diagram where diagram_id = {id}"))?;
    let name = rows.first().and_then(|r| r.first()).with_context(|| crate::i18n::fmt("there is no diagram with id {0}", &[id.to_string()]))?;

    std::fs::create_dir_all(&ctx.cfg.attachments_dir).context(crate::i18n::tr("attachments folder"))?;
    let stored = format!("{}_{}.png", uuid::Uuid::new_v4().simple(), crate::history::sanitize_name(name));
    let path = ctx.cfg.attachments_dir.join(&stored);
    // A diagram open in the user's EA window is exported from that window's copy: reload it first, or the image
    // shows the diagram from before the last write (e.g. without the routes just saved).
    let _ = crate::ea_ui::refresh_after_write(&crate::ea_ui::Target::of(ctx.cfg), vec![id as i32], Vec::new());
    let op = json!({ "op": "export_diagram", "diagram": id, "path": path.to_string_lossy() });
    ea::run_batch(ctx.cfg, &[op], &BatchOptions::default())?;

    let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
    if size == 0 {
        bail!("{}", crate::i18n::fmt("EA did not save an image of diagram \"{0}\" (the diagram may be empty)", &[name.clone()]));
    }
    // Which diagram the image shows: the chat offers "Edit diagram" for it.
    let _ = std::fs::write(ctx.cfg.attachments_dir.join(format!("{stored}.diagram")), id.to_string());
    // In a tab of EA the diagram is shown in EA itself; the image only lets the model check its work.
    if ea::in_ea_tab() {
        crate::ea_ui::open_diagram(&crate::ea_ui::Target::of(ctx.cfg), id as i32)?;
        return Ok(ToolOutput {
            text: format!("Diagram \"{name}\" (id {id}) is now open in the user's Enterprise Architect window, next to this chat. The attached image is only for your own check: the user does not see it in the chat, so refer to the diagram in EA."),
            images: vec![stored],
            files: Vec::new(),
        });
    }
    Ok(ToolOutput {
        text: format!("Diagram \"{name}\" (id {id}) is in the attached image. The user sees the same image in the chat and can save it to disk."),
        images: vec![stored],
        files: Vec::new(),
    })
}

/// The beginning of a longer text for the approval dialog.
fn shorten_for_dialog(text: &str) -> String {
    const MAX: usize = 400;
    let mut short: String = text.chars().take(MAX).collect();
    if text.chars().count() > MAX {
        short.push('…');
    }
    short
}

/// Names of the objects that a batch deletes, moves or copies, for the approval dialog (the user must see WHAT
/// disappears, not just how many). Objects referred to by a key from the same batch (`#key`) are new and not listed.
pub(crate) fn destructive_details(cfg: &AppConfig, ops: &[Value]) -> Vec<String> {
    let name_of = |table: &str, id_column: &str, id: i64| -> String {
        let sql = format!("select name from {table} where {id_column} = {id}");
        query_rows(cfg, &sql)
            .ok()
            .and_then(|rows| rows.first().and_then(|r| r.first().cloned()))
            .filter(|n| !n.is_empty())
            .map(|n| format!("\"{n}\""))
            .unwrap_or_else(|| format!("ID {id}"))
    };
    let mut lines = Vec::new();
    for op in ops {
        let kind = op["op"].as_str().unwrap_or("");
        let file = op["file"].as_str().unwrap_or("").to_string();
        let exists = !file.is_empty() && std::path::Path::new(&file.replace('/', "\\")).exists();
        match kind {
            "export_xmi" | "generate_report" => {
                lines.push(crate::i18n::fmt("WRITE the file {0}{1}", &[file, if exists { crate::i18n::tr(" (REPLACES the existing file)").to_string() } else { String::new() }]));
                continue;
            }
            "import_xmi" => {
                lines.push(crate::i18n::fmt("IMPORT the file {0} into package {1}", &[file, op["package"].as_i64().map(|id| name_of("t_package", "package_id", id)).unwrap_or_else(|| op["package"].to_string())]));
                continue;
            }
            "delete_attribute" | "delete_operation" | "delete_scenario" => {
                // An attribute or operation is given by its own id; its element is found from it (LC-442: "element null").
                let (field, table, key) = match kind {
                    "delete_attribute" => ("attribute", "t_attribute", "id"),
                    "delete_operation" => ("operation", "t_operation", "operationid"),
                    _ => ("scenario", "", ""),
                };
                let (what, element) = match op[field].as_i64().filter(|_| !table.is_empty()) {
                    Some(id) => query_rows(cfg, &format!("select coalesce(f.name,'') as c0, f.object_id as c1 from {table} f where f.{key} = {id}"))
                        .ok()
                        .and_then(|r| r.into_iter().next())
                        .map(|r| (format!("{field} \"{}\"", r[0]), r[1].parse::<i64>().map(|e| name_of("t_object", "object_id", e)).unwrap_or_default()))
                        .unwrap_or_else(|| (format!("{field} ID {id}"), String::new())),
                    None => (format!("{field} {}", op[field].as_str().unwrap_or("")), op["element"].as_i64().map(|id| name_of("t_object", "object_id", id)).unwrap_or_default()),
                };
                lines.push(crate::i18n::fmt("DELETE {0} of element {1}", &[what, element]));
                continue;
            }
            "version_control" => {
                let package = op["package"].as_i64().map(|id| name_of("t_package", "package_id", id)).unwrap_or_else(|| op["package"].to_string());
                lines.push(crate::i18n::fmt("VERSION CONTROL: {0} of package {1}", &[op["action"].as_str().unwrap_or("?").replace('_', " ").to_uppercase(), package]));
                continue;
            }
            "merge_elements" => {
                let name = |field: &str| op[field].as_i64().map(|id| name_of("t_object", "object_id", id)).unwrap_or_default();
                lines.push(crate::i18n::fmt("MERGE {0} into {1} and DELETE {0}", &[name("remove"), name("keep")]));
                continue;
            }
            "set_swimlanes" | "set_linked_document" | "update_scenario" => {
                lines.push(crate::i18n::fmt("REPLACE {0}", &[crate::ea::describe(std::slice::from_ref(op)).trim().to_string()]));
                continue;
            }
            _ => {}
        }
        if !matches!(kind, "delete" | "move" | "clone_package") {
            continue;
        }
        let target = [("element", "t_object", "object_id"), ("diagram", "t_diagram", "diagram_id"), ("child_package", "t_package", "package_id"), ("package", "t_package", "package_id"), ("connector", "t_connector", "connector_id")]
            .iter()
            .find_map(|(field, table, column)| {
                // For "move" the "package" field is the destination, not the moved object.
                if kind == "move" && *field == "package" {
                    return None;
                }
                op[*field].as_i64().map(|id| (*field, name_of(table, column, id)))
            });
        let Some((field, mut name)) = target else { continue };
        // A relationship without a name: its type and ends (LC-441).
        if field == "connector" {
            if let Some(r) = op["connector"].as_i64().and_then(|id| {
                query_rows(cfg, &format!(
                    "select c.connector_type as c0, coalesce(s.name,'') as c1, coalesce(t.name,'') as c2 from t_connector c left join t_object s on s.object_id = c.start_object_id \
                     left join t_object t on t.object_id = c.end_object_id where c.connector_id = {id}"
                ))
                .ok()?
                .into_iter()
                .next()
            }) {
                name = format!("{name} ({} \"{}\" → \"{}\")", r[0], r[1], r[2]);
            }
        }
        let what = match field {
            "element" => crate::i18n::tr("element"),
            "diagram" => crate::i18n::tr("diagram"),
            "package" | "child_package" => crate::i18n::tr("package"),
            _ => crate::i18n::tr("connector"),
        };
        // Deleting an element: its relationships and diagrams in the card (LC-367).
        let reach = if kind == "delete" && field == "element" { op["element"].as_i64().and_then(|id| crate::recycle::element_reach(cfg, id)) } else { None };
        lines.push(match kind {
            "delete" => match reach {
                Some(reach) => format!("{} – {reach}", crate::i18n::fmt("DELETE {0} {1}", &[what.to_string(), name])),
                None => crate::i18n::fmt("DELETE {0} {1}", &[what.to_string(), name]),
            },
            "move" => crate::i18n::fmt("MOVE {0} {1} to package {2}", &[
                what.to_string(),
                name,
                match &op["package"] {
                    Value::Number(id) => name_of("t_package", "package_id", id.as_i64().unwrap_or(0)),
                    // A package made earlier in the same write ("#bin"): its name (LC-442, round 5).
                    Value::String(key) => ops
                        .iter()
                        .find(|o| o["key"].as_str().is_some_and(|k| k.trim_start_matches('#') == key.trim_start_matches('#')))
                        .and_then(|o| o["name"].as_str())
                        .map(|n| format!("\"{n}\" ({})", crate::i18n::tr("new@@created in this write")))
                        .unwrap_or_else(|| key.clone()),
                    other => other.to_string(),
                },
            ]),
            _ => crate::i18n::fmt("COPY package {0} with all its contents", &[name]),
        });
    }
    lines
}

/// A local file or folder the assistant wants to read (requirements, source code): no network shares or devices; outside
/// the attachments of the conversation the user is asked first (its text goes to the AI provider).
fn read_path(ctx: &ToolContext, given: &str, folder: bool) -> Result<std::path::PathBuf> {
    let given = given.trim();
    let attachment = crate::history::attachment_path(&ctx.cfg.attachments_dir, given);
    if !folder && !given.contains(['\\', '/', ':']) && attachment.is_file() {
        return Ok(attachment);
    }
    let text = given.replace('/', "\\");
    let bytes = text.as_bytes();
    let drive = bytes.len() >= 3 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' && bytes[2] == b'\\';
    if !drive || text.split('\\').any(|p| p == "..") {
        bail!("{}", crate::i18n::fmt("{0}: give a full path on a local drive; network shares and relative paths are not allowed", &[given.to_string()]));
    }
    let path = std::path::PathBuf::from(&text);
    if !path.starts_with(&ctx.cfg.attachments_dir) {
        ctx.approver.confirm_always(&crate::i18n::fmt("Read {0}? Its text goes to the AI model of the conversation.", &[path.display().to_string()]))?;
    }
    Ok(path)
}

/// The longest text shown whole in an approval; longer ones are refused (what is approved is what is saved or run).
const LONGEST_APPROVAL: usize = 20_000;

/// The whole code of a script for the approval, with risky calls pointed out; a longer script is refused.
fn script_for_approval(code: &str) -> Result<String> {
    if code.chars().count() > LONGEST_APPROVAL {
        bail!("{}", crate::i18n::fmt("the script is too long to approve ({0} characters, at most {1}): run it from EA yourself", &[code.chars().count().to_string(), LONGEST_APPROVAL.to_string()]));
    }
    let risky = ["ActiveXObject", "CreateObject", "GetObject", "WScript.Shell", "Shell.Application", "Scripting.FileSystemObject", "winmgmts", ".Run(", ".Exec(", "XMLHTTP", "ADODB", "Repository.Execute", ".Execute("];
    let found: Vec<&str> = risky.iter().filter(|r| code.to_lowercase().contains(&r.to_lowercase())).copied().collect();
    Ok(if found.is_empty() {
        code.to_string()
    } else {
        format!("{}\n\n{code}", crate::i18n::fmt("⚠ The script uses: {0} (access to files, programs or the network outside EA).", &[found.join(", ")]))
    })
}

/// A text for an approval (callers refuse texts longer than LONGEST_APPROVAL).
fn shorten_long(text: &str) -> String {
    text.to_string()
}

fn ea_write(input: &Value, ctx: &ToolContext) -> Result<String> {
    let given = input["ops"].as_array().with_context(|| crate::i18n::fmt("missing list '{0}'", &["ops".to_string()]))?;
    for (i, op) in given.iter().enumerate() {
        let name = op["op"].as_str().unwrap_or("");
        if !ea::MODEL_OPS.contains(&name) {
            bail!("{}", crate::i18n::fmt("operation {0} ('{1}') is not allowed. Allowed: {2}", &[(i + 1).to_string(), name.to_string(), ea::MODEL_OPS.join(", ")]));
        }
    }
    let mut normalized = normalize_ops(given);
    let mut risky_files = Vec::new();
    for op in normalized.iter_mut() {
        let kind = op["op"].as_str().unwrap_or("").to_string();
        let file = op["file"].as_str().unwrap_or("").trim().to_string();
        if matches!(kind.as_str(), "export_xmi" | "generate_report") && !file.is_empty() && !file.contains(['\\', '/', ':']) {
            // A bare file name: the file goes with the conversation's files, as the documents Compass writes (LC-466: the
            // assistant picked a public folder itself when a full path was asked for).
            let _ = std::fs::create_dir_all(&ctx.cfg.attachments_dir);
            op["file"] = json!(ctx.cfg.attachments_dir.join(&file).display().to_string());
            continue;
        }
        if matches!(kind.as_str(), "export_xmi" | "import_xmi" | "generate_report") {
            // Files outside the model are always asked (also in auto), with the real full path in the dialog and for EA.
            let path = check_file_path(&file)?;
            op["file"] = json!(path.display().to_string());
            risky_files.push(path);
        }
    }
    let ops = &normalized;
    check_unchanged(ctx.cfg, &input["expect_unchanged"])?;
    let summary = crate::i18n::fmt("Saving to the EA model:\n{0}\n\nBefore → after:\n{1}", &[ea::describe(ops), crate::governance::preview(ctx.cfg, ops)]);
    let destructive = destructive_details(ctx.cfg, ops);
    // Deleting an element that has relationships, deleting a package, importing XMI and replacing an existing file
    // always ask (also in auto), with what is affected.
    let related = crate::verify::relations_of_deleted(ctx.cfg, ops);
    let package_deleted = ops.iter().any(|op| op["op"] == "delete" && (op["package"].is_i64() || op["child_package"].is_i64()));
    // Version control is always asked, also in auto (LC-349): get_latest replaces the whole package.
    let version_control = ops.iter().any(|op| op["op"] == "version_control" || op["op"] == "merge_elements");
    if !related.is_empty() || package_deleted || version_control || !risky_files.is_empty() {
        ctx.approver.confirm_risky(&format!("{summary}\n\n{}\n\n{}", destructive.join("\n"), related.join("\n")))?;
    } else if destructive.is_empty() {
        ctx.approver.confirm(&summary)?;
    } else {
        ctx.approver.confirm_destructive(&format!("{summary}\n\n{}", destructive.join("\n")))?;
    }

    // Time machine: what reverses this save, read before it; a baseline of the packages a deletion touches.
    let (reverse, not_undoable) = crate::undo::before(ctx.cfg, ops);
    let mut baselines = Vec::new();
    for package in crate::undo::packages_to_baseline(ctx.cfg, ops) {
        let version = format!("Compass {}", chrono::Local::now().format("%Y-%m-%d %H:%M:%S"));
        let baseline = [json!({ "op": "create_baseline", "package": package, "version": version, "notes": crate::i18n::tr("Made by Compass before a change that cannot be undone by operations") })];
        let made = ea::run_batch(ctx.cfg, &baseline, &BatchOptions::default());
        audit(ctx.cfg, &baseline, &made);
        if made.is_ok() {
            baselines.push(format!("package {package}: {version}"));
        }
    }
    // Diagrams the deleted elements and relationships are on (known only before the save): reloaded afterwards and
    // checked for links left behind (LC-412).
    let touched = diagrams_of_deleted(ctx.cfg, ops);
    let result = ea::run_batch(ctx.cfg, ops, &BatchOptions::default());
    audit_in_mode(ctx.cfg, ctx.approver.mode, ops, &result);
    match &result {
        Ok(done) => crate::undo::record(ctx.cfg, &ea::describe(ops), done, reverse, not_undoable, baselines.clone()),
        // A save that failed part-way: the changes of existing objects before the failed operation stay; journal them.
        Err(e) => {
            if let Some(at) = e.downcast_ref::<ea::BatchError>().and_then(|b| b.failed_at) {
                crate::undo::record_partial(ctx.cfg, &ea::describe(ops), at, reverse, baselines.clone());
            }
        }
    }
    if let Ok(done) = &result {
        refresh_user_ea(ctx.cfg, ops, done, &touched);
    }
    let done = result?;
    let mut text = format_created(&done);
    let ghosts = ghost_links(ctx.cfg, &touched);
    if !ghosts.is_empty() {
        text.push_str(&format!("\nLinks left on diagrams after the deletion (EA did not remove them; repair_diagram_links on each removes them): {}\n", ghosts.join(", ")));
    }
    if !related.is_empty() {
        text.push_str(&format!("\nDeleted together with relationships (tell the user): {}\n", related.join(" | ")));
    }
    // A relationship made by this write is hidden on the diagrams whose visibility policy is "auto", as one made in EA
    // is (LC-373: the policy missed the relationships Compass made itself).
    for created in done["created"].as_array().into_iter().flatten().filter(|c| c["kind"] == "connector") {
        if let Some(id) = created["id"].as_i64() {
            for line in crate::visibility::auto_apply(ctx.cfg, id) {
                text.push_str(&format!("\nRelationship {id}: {line} (tell the user)\n"));
            }
        }
    }
    // The model is read again: only what is really there may be reported as done.
    let check = crate::verify::verify(ctx.cfg, ops, &done);
    if check.problems.is_empty() && check.passed == 0 {
        // Nothing comparable in the model (e.g. a layout, colors): EA's confirmation of the save is the evidence.
        text.push_str("\nEA confirmed the save; these operations have no model values to compare afterwards.\n");
    } else if check.problems.is_empty() {
        text.push_str(&format!("\nVerified in the model after the write: {} checks passed.\n", check.passed));
    } else {
        text.push_str(&format!(
            "\nVERIFICATION FAILED - the model does not contain what this write asked for ({} checks passed, {} failed):\n- {}\n\
Tell the user exactly which of these changes did not happen; do not say they were done. You may try to fix them with another write.\n",
            check.passed, check.problems.len(), check.problems.join("\n- ")
        ));
    }
    Ok(text)
}

/// The diagrams that show the elements and relationships a save deletes (LC-412).
fn diagrams_of_deleted(cfg: &AppConfig, ops: &[Value]) -> Vec<i32> {
    let mut out = Vec::new();
    for op in ops.iter().filter(|o| o["op"] == "delete") {
        let sql = if let Some(e) = op["element"].as_i64() {
            format!("select distinct diagram_id from t_diagramobjects where object_id = {e}")
        } else if let Some(c) = op["connector"].as_i64() {
            format!("select distinct diagramid from t_diagramlinks where connectorid = {c}")
        } else {
            continue;
        };
        out.extend(query_rows(cfg, &sql).unwrap_or_default().into_iter().filter_map(|r| r[0].parse::<i32>().ok()));
    }
    out.sort_unstable();
    out.dedup();
    out
}

/// Diagram links that point at a relationship no longer in the model, or whose ends are no longer on the diagram.
fn ghost_links(cfg: &AppConfig, diagrams: &[i32]) -> Vec<String> {
    if diagrams.is_empty() {
        return Vec::new();
    }
    let list = diagrams.iter().map(i32::to_string).collect::<Vec<_>>().join(",");
    query_rows(cfg, &format!(
        "select l.diagramid, count(*) from t_diagramlinks l left join t_connector c on c.connector_id = l.connectorid where l.diagramid in ({list}) and (c.connector_id is null \
         or not exists (select 1 from t_diagramobjects a where a.diagram_id = l.diagramid and a.object_id = c.start_object_id) \
         or not exists (select 1 from t_diagramobjects b where b.diagram_id = l.diagramid and b.object_id = c.end_object_id)) group by l.diagramid"
    ))
    .unwrap_or_default()
    .into_iter()
    .map(|r| format!("diagram {} ({} link(s))", r[0], r[1]))
    .collect()
}

/// After a write, refreshes the changed diagrams and packages in the user's EA window (in the background, without waiting; does nothing without an EA window).
fn refresh_user_ea(cfg: &AppConfig, ops: &[Value], result: &Value, extra: &[i32]) {
    let mut diagrams: Vec<i32> = extra.to_vec();
    let mut packages: Vec<i32> = Vec::new();
    for op in ops {
        if let Some(id) = op["diagram"].as_i64() {
            diagrams.push(id as i32);
        }
        for field in ["package", "parent"] {
            if let Some(id) = op[field].as_i64() {
                packages.push(id as i32);
            }
        }
    }
    for created in result["created"].as_array().into_iter().flatten() {
        if created["kind"] == "package" {
            if let Some(id) = created["id"].as_i64() {
                packages.push(id as i32);
            }
        }
    }
    diagrams.sort_unstable();
    diagrams.dedup();
    packages.sort_unstable();
    packages.dedup();
    let db = crate::ea_ui::Target::of(cfg);
    std::thread::spawn(move || {
        let _ = crate::ea_ui::refresh_after_write(&db, diagrams, packages);
    });
}

/// Appends the write to the change log (failed ones too): when, who, in which model, what and with what result.
pub(crate) fn audit(cfg: &AppConfig, ops: &[Value], result: &Result<Value>) {
    audit_in_mode(cfg, crate::hub::effective_mode(cfg), ops, result);
}

/// The audit entry with the approval mode really in force (LC-349: a hub policy can raise the local mode, and the
/// entry showed the local one).
fn audit_in_mode(cfg: &AppConfig, mode: ApprovalMode, ops: &[Value], result: &Result<Value>) {
    use std::io::Write;
    let entry = json!({
        "time": chrono::Local::now().to_rfc3339(),
        "database": if cfg.uses_file() { cfg.model_file.display().to_string() } else { format!("{}@{}:{}", cfg.db_name, cfg.db_host, cfg.db_port) },
        "ea_user": cfg.ea_user,
        "approval": mode.as_str(),
        "summary": ea::describe(ops),
        "ops": ops,
        "ok": result.is_ok(),
        "created": result.as_ref().ok().map(|r| r["created"].clone()).unwrap_or(Value::Null),
        "error": result.as_ref().err().map(|e| format!("{e:#}")),
    });
    if let Some(dir) = cfg.audit_file.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(&cfg.audit_file) {
        let _ = writeln!(file, "{entry}");
    }
    // The team audit trail: the same entry to the hub, in the background (a hub that is down does not stop the work).
    if let Some(client) = crate::hub::Client::from_config(cfg) {
        let mut entry = entry;
        entry["source"] = json!(if cfg.hub_source.trim().is_empty() { entry["database"].clone() } else { json!(cfg.hub_source.trim()) });
        entry.as_object_mut().map(|o| o.remove("ops"));
        entry["operations"] = json!(ops.len());
        std::thread::spawn(move || {
            if let Err(e) = client.send_audit(&entry) {
                crate::hub::note_failure(&format!("{e:#}"));
            }
        });
    }
}

fn format_created(result: &Value) -> String {
    let mut out = String::from("Saved in the EA model. Created objects (key | kind | id | name):\n");
    for o in result["created"].as_array().into_iter().flatten() {
        out.push_str(&format!(
            "{} | {} | {} | {}\n",
            o["key"].as_str().unwrap_or("-"),
            o["kind"].as_str().unwrap_or("?"),
            o["id"],
            o["name"].as_str().unwrap_or("")
        ));
    }
    for w in result["warnings"].as_array().into_iter().flatten() {
        out.push_str(&format!("Warning: {}\n", w.as_str().unwrap_or("")));
    }
    out.push_str("If the user has an EA window open with this model, the changed diagrams and packages refresh there automatically; otherwise the user will see the changes after refreshing the package (F5).");
    out
}

/// Optimistic locking: compares `modifieddate` from the database with the value the model read earlier.
/// The check and the write are not a single transaction, so this detects changes made before the write, not during it.
fn check_unchanged(cfg: &AppConfig, expect: &Value) -> Result<()> {
    for item in expect.as_array().into_iter().flatten() {
        let id = item["id"].as_i64().with_context(|| crate::i18n::fmt("expect_unchanged: missing '{0}'", &["id".to_string()]))?;
        let seen = item["modified"].as_str().with_context(|| crate::i18n::fmt("expect_unchanged: missing '{0}'", &["modified".to_string()]))?;
        let (table, key, what) = match item["kind"].as_str() {
            Some("package") => ("t_package", "package_id", crate::i18n::tr("package")),
            Some("element") => ("t_object", "object_id", crate::i18n::tr("element")),
            Some("diagram") => ("t_diagram", "diagram_id", crate::i18n::tr("diagram")),
            other => bail!("{}", crate::i18n::fmt("expect_unchanged: unknown kind {0}", &[format!("{other:?}")])),
        };
        // No cast in SQL: "as text" exists only in PostgreSQL and SQLite (LC-344); every engine returns the date as text
        // through query_rows, and normalize_stamp compares the first 19 characters.
        let rows = query_rows(cfg, &format!("select modifieddate from {table} where {key} = {id}"))?;
        let now = rows.first().and_then(|r| r.first()).with_context(|| crate::i18n::fmt("{0} with id {1} no longer exists", &[what.to_string(), id.to_string()]))?;
        if normalize_stamp(now) != normalize_stamp(seen) {
            bail!("{}", crate::i18n::fmt("version conflict: {0} with id {1} was changed after you read it (was {2}, now {3}). Read the model again and plan the change once more. Nothing was saved.", &[what.to_string(), id.to_string(), seen.to_string(), now.to_string()]));
        }
    }
    Ok(())
}

/// The first 19 characters of the timestamp (`YYYY-MM-DD HH:MM:SS`), with the letter T replaced by a space.
fn normalize_stamp(s: &str) -> String {
    s.trim().replace('T', " ").chars().take(19).collect()
}

#[cfg(test)]
mod tests {
    /// LC-376 in EA: an element added on top of another goes next to the one it is related to; nothing else moves.
    /// `LOONAR_TEST_PROPS=… cargo test place_new_live -- --ignored --nocapture`
    #[test]
    #[ignore = "requires EA and a copy of a test model"]
    fn place_new_live() {
        let cfg = AppConfig::load(std::path::Path::new(&std::env::var("LOONAR_TEST_PROPS").unwrap())).unwrap();
        let q = |sql: &str| query_rows(&cfg, sql).unwrap();
        let root: i64 = q("select package_id from t_package where parent_id = 0 order by package_id")[0][0].parse().unwrap();
        let ops = vec![
            json!({"op":"create_package","key":"pkg","parent":root,"name":format!("ZZ_place_{}", chrono::Local::now().format("%H%M%S"))}),
            json!({"op":"create_diagram","key":"d","package":"#pkg","name":"Place","type":"Logical"}),
            json!({"op":"create_element","key":"a","package":"#pkg","name":"Order","type":"Class"}),
            json!({"op":"create_element","key":"c","package":"#pkg","name":"Invoice","type":"Class"}),
            json!({"op":"create_element","key":"b","package":"#pkg","name":"Customer","type":"Class"}),
            json!({"op":"create_connector","key":"ab","source":"#a","target":"#b","type":"Association"}),
            json!({"op":"place","diagram":"#d","element":"#a","left":40,"top":40,"width":120,"height":70}),
            json!({"op":"place","diagram":"#d","element":"#c","left":200,"top":40,"width":120,"height":70}),
            json!({"op":"place","diagram":"#d","element":"#b","left":50,"top":50,"width":120,"height":70}),
        ];
        let created = crate::ea::run_batch(&cfg, &ops, &crate::ea::BatchOptions { rollback_key: Some("pkg".into()), keep_locks: false }).unwrap();
        let id_of = |key: &str| created["created"].as_array().unwrap().iter().find(|c| c["key"] == key).unwrap()["id"].as_i64().unwrap();
        let (pkg, d, a, b, c) = (id_of("pkg"), id_of("d"), id_of("a"), id_of("b"), id_of("c"));
        // A bent route of the relationship of the element that will move (LC-418).
        crate::ea::run_batch(&cfg, &[json!({"op":"route_connector","diagram":d,"connector":id_of("ab"),"points":[[300,300]]})], &crate::ea::BatchOptions::default()).unwrap();
        let before = rects(&cfg, d, &[a, c]).unwrap();
        let (ops, text) = super::place_new_ops(&cfg, &json!({ "diagram": d })).unwrap();
        println!("{text}");
        assert!(ops.iter().any(|o| o["op"] == "route_connector"), "{ops:?}");
        crate::ea::run_batch(&cfg, &ops, &crate::ea::BatchOptions::default()).unwrap();
        let path = query_rows(&cfg, &format!("select coalesce(path,'') from t_diagramlinks where diagramid = {d} and connectorid = {}", id_of("ab"))).unwrap();
        assert_eq!(path[0][0], "", "the bend is gone");
        let after = rects(&cfg, d, &[a, c, b]).unwrap();
        crate::ea::run_batch(&cfg, &[json!({"op":"delete","package":pkg})], &crate::ea::BatchOptions::default()).unwrap();
        crate::ea::shutdown();
        assert_eq!(&after[..2], &before[..], "the others stay");
        let r = |x: (i64, i64, i64, i64, i64)| (x.1, x.2, x.3, x.4);
        assert!(!overlaps(r(after[2]), r(after[0]), 0) && !overlaps(r(after[2]), r(after[1]), 0), "{after:?}");
    }

    /// LC-412: the check for links left on diagrams runs on the model (.qea and SQL Server).
    /// `LOONAR_TEST_PROPS=… cargo test ghost_links_live -- --ignored --nocapture`
    #[test]
    #[ignore = "requires a test model"]
    fn ghost_links_live() {
        let cfg = AppConfig::load(std::path::Path::new(&std::env::var("LOONAR_TEST_PROPS").unwrap())).unwrap();
        let diagrams: Vec<i32> = query_rows(&cfg, "select diagram_id from t_diagram").unwrap().iter().take(400).filter_map(|r| r[0].parse().ok()).collect();
        println!("{:?}", super::ghost_links(&cfg, &diagrams));
        if let Some(row) = query_rows(&cfg, "select object_id from t_diagramobjects").unwrap().first() {
            let element: i64 = row[0].parse().unwrap();
            assert!(!super::diagrams_of_deleted(&cfg, &[json!({"op":"delete","element":element})]).is_empty());
        }
    }

    /// LC-376: a free place next to the anchor, never on top of what is there.
    #[test]
    fn free_spot_next_to_the_anchor() {
        let taken = vec![(100, 100, 120, 60), (260, 100, 120, 60)];
        // Right of the first is taken by the second: below it.
        assert_eq!(super::free_spot(&taken, taken[0], 120, 60, 40), (100, 200));
        // The second has the right side free.
        assert_eq!(super::free_spot(&taken, taken[1], 120, 60, 40), (420, 100));
        let spot = super::free_spot(&taken, taken[0], 120, 60, 40);
        assert!(!taken.iter().any(|r| super::overlaps((spot.0, spot.1, 120, 60), *r, 0)));
    }

    /// LC-377: a presentation copy through EA on a model COPY (removed again):
    /// `LOONAR_TEST_PROPS=E:/livetest/undo/undo.properties cargo test exec_view_live -- --ignored --nocapture`
    #[test]
    #[ignore = "requires EA and a copy of a test model"]
    fn exec_view_live() {
        let cfg = crate::config::AppConfig::load(std::path::Path::new(&std::env::var("LOONAR_TEST_PROPS").expect("LOONAR_TEST_PROPS"))).expect("app.properties");
        let q = |sql: &str| super::query_rows(&cfg, sql).unwrap();
        let diagram: i64 = q("select d.diagram_id from t_diagram d join t_diagramobjects x on x.diagram_id = d.diagram_id join t_object o on o.object_id = x.object_id \
                              where d.diagram_type = 'Logical' and o.object_type = 'Class' group by d.diagram_id order by count(*) desc")[0][0].parse().unwrap();
        let ops = super::exec_view_ops(&cfg, &serde_json::json!({ "diagram": diagram })).unwrap();
        let done = crate::ea::run_batch(&cfg, &ops, &crate::ea::BatchOptions::default()).expect("presentation copy");
        let copy = done["created"].as_array().unwrap().iter().find(|c| c["kind"] == "diagram").unwrap()["id"].as_i64().unwrap();
        let count = |d: i64| q(&format!("select count(*) from t_diagramobjects where diagram_id = {d}"))[0][0].clone();
        let pdata = q(&format!("select pdata, styleex from t_diagram where diagram_id = {copy}"))[0].clone();
        let (original, copied) = (count(diagram), count(copy));
        let image = std::env::temp_dir().join("loonar_exec_view.png");
        let _ = crate::ea::run_batch(&cfg, &[serde_json::json!({"op":"export_diagram","diagram":copy,"path":image.display().to_string()})], &crate::ea::BatchOptions::default());
        crate::ea::run_batch(&cfg, &[serde_json::json!({"op":"delete","diagram":copy})], &crate::ea::BatchOptions::default()).expect("cleanup");
        crate::ea::shutdown();
        println!("{original} → {copied}; {pdata:?}; image {}", image.display());
        assert_eq!(original, copied);
        assert!(pdata[0].contains("HideAtts=1") && pdata[0].contains("HideOps=1") && pdata[1].contains("SuppConnectorLabels=1"));
    }

    /// LC-376: alignment, distribution and size from the first element.
    #[test]
    fn aligning_rectangles() {
        let base = vec![(1, 100, 50, 120, 60), (2, 300, 80, 100, 40), (3, 520, 20, 80, 60)];
        let mut r = base.clone();
        assert_eq!(super::align(&mut r, "top").unwrap(), vec![(2, 300, 50, 100, 40), (3, 520, 50, 80, 60)]);
        let mut r = base.clone();
        let spread = super::align(&mut r, "distribute_horizontally").unwrap();
        // From 100 to 600: 300 of boxes, two gaps of 100.
        assert_eq!(spread, vec![(2, 320, 80, 100, 40)]);
        let mut r = base.clone();
        assert_eq!(super::align(&mut r, "same_size").unwrap().len(), 2);
        let mut r = base;
        assert!(super::align(&mut r, "sideways").is_err());
    }

    /// LC-344: optimistic locking on a real SQL Server (Windows sign-in):
    /// `LOONAR_TEST_MSSQL_WIN="127.0.0.1;1433;ea_lab" cargo test check_unchanged_on_sql_server -- --ignored --nocapture`
    #[test]
    #[ignore = "requires a SQL Server with an EA repository"]
    fn check_unchanged_on_sql_server() {
        let spec = std::env::var("LOONAR_TEST_MSSQL_WIN").unwrap();
        let parts: Vec<&str> = spec.split(';').collect();
        let props = std::env::temp_dir().join(format!("loonar-mssql-lock-{}.properties", std::process::id()));
        std::fs::write(&props, format!("model.source=sqlserver
db.host={}
db.port={}
db.name={}
db.user=
", parts[0], parts[1], parts[2])).unwrap();
        let cfg = crate::config::AppConfig::load(&props).unwrap();
        let row = &super::query_rows(&cfg, "select object_id, modifieddate from t_object order by object_id").unwrap()[0];
        println!("element {} modified {}", row[0], row[1]);
        let same = serde_json::json!([{ "kind": "element", "id": row[0].parse::<i64>().unwrap(), "modified": row[1] }]);
        super::check_unchanged(&cfg, &same).unwrap();
        let older = serde_json::json!([{ "kind": "element", "id": row[0].parse::<i64>().unwrap(), "modified": "2001-01-01 00:00:00" }]);
        assert!(super::check_unchanged(&cfg, &older).is_err());
    }

    use super::*;
    use std::path::Path;

    /// LC-271: the page source is found also by a name without the unique prefix or with .md.
    #[test]
    fn page_source_by_short_name() {
        let dir = std::env::temp_dir().join(format!("loonar-lc271-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let full = dir.join("0a1b2c_Domena_lista_klas.confluence.md");
        std::fs::write(&full, "<!-- title: x -->\n# x").unwrap();
        for given in ["0a1b2c_Domena_lista_klas.confluence.xml", "Domena_lista_klas.confluence.xml", "lista_klas.confluence.md", "LISTA_KLAS"] {
            assert_eq!(page_source(&dir, given), full, "{given}");
        }
        // The page title instead of a file name (the file name has the title made safe, the title is in the file).
        let titled = dir.join("9f8e7d_Domena_zamówień_–_klasy.confluence.md");
        std::fs::write(&titled, "<!-- title: Domena zamówień – klasy -->\n# x").unwrap();
        assert_eq!(page_source(&dir, "Domena zamówień – klasy.confluence.md"), titled);
        assert_eq!(page_source(&dir, "Domena zamówień – klasy"), titled);
        assert_eq!(page_source(&dir, "other.confluence.xml"), dir.join("other.confluence.md"));
    }

    /// The developer's app.properties; without it (CI) the default settings, enough for the tests that refuse before touching a model.
    fn test_config() -> AppConfig {
        // LOONAR_TEST_PROPS points the live tests at another model (a file copy), like the EA tests.
        let props = std::env::var("LOONAR_TEST_PROPS").map(std::path::PathBuf::from).unwrap_or_else(|_| Path::new(env!("CARGO_MANIFEST_DIR")).join("app.properties"));
        if props.exists() {
            return AppConfig::load(&props).expect("app.properties");
        }
        let dir = std::env::temp_dir().join(format!("loonar-tools-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("app.properties"), "").unwrap();
        AppConfig::load(&dir.join("app.properties")).unwrap()
    }

    /// LC-308: a document is made only when every request of the user and every heading of its template is in it.
    #[test]
    fn alias_scripts_are_refused() {
        assert!(refuse_alias_script("var a = el.Attributes.GetAt(0); a.Alias = 'GET /x'; a.Update();").is_err());
        assert!(refuse_alias_script("op = el.Methods.GetAt(i); op.StyleEx = 'alias=x'; op.Update();").is_err());
        assert!(refuse_alias_script("if (el.Alias == 'x') Session.Output(el.Name);").is_ok());
        assert!(refuse_alias_script("Session.Output(Repository.Models.Count);").is_ok());
    }

    #[test]
    fn document_discipline() {
        let mut cfg = test_config();
        cfg.templates_dir = std::env::temp_dir().join(format!("loonar-doc-discipline-{}", std::process::id()));
        std::fs::create_dir_all(&cfg.templates_dir).unwrap();
        std::fs::write(cfg.templates_dir.join("Spec.md"), "# Spec
## 1. Goal
> hint
## 2. Requirements
## 3. Screens of <system>
").unwrap();
        let approver = Approver::new(ApprovalMode::Auto, Box::new(|_| true));
        let ctx = ToolContext { cfg: &cfg, approver: &approver };
        let md = "# Spec
## 1. Goal
Text.
## 2. Requirements
- W1 Login
![Screen](image:a.png)
";
        let ask = |checklist: Value, template: &str| doc_discipline(&json!({ "checklist": checklist, "template": template }), md, &ctx);
        assert!(ask(json!([]), "none").unwrap_err().to_string().contains("checklist"));
        let missing = ask(json!([{ "ask": "W1", "where": "Requirements" }, { "ask": "W2 Logout", "where": "W2 Logout" }]), "none").unwrap_err().to_string();
        assert!(missing.contains("W2 Logout") && !missing.contains("- W1"), "{missing}");
        let ok = ask(json!([{ "ask": "each requirement", "where": "2. Requirements" }, { "ask": "screens from Jira", "where": "![Screen]" }]), "Spec").unwrap();
        assert!(ok.contains("2 requests") && ok.contains("Spec"), "{ok}");
        let lacking = doc_discipline(&json!({ "checklist": [{ "ask": "goal", "where": "Goal" }], "template": "Spec" }), "# Spec
## Goal
", &ctx).unwrap_err().to_string();
        assert!(lacking.contains("## 2. Requirements") && !lacking.contains("<system>"), "{lacking}");
    }

    /// Persistent EA session: the first call starts EA (tens of seconds), the following ones reuse the open session.
    /// Run with: `cargo test session_reuse_is_fast -- --ignored --nocapture`.
    #[test]
    #[ignore = "requires EA, a license and the model database from app.properties"]
    fn session_reuse_is_fast() {
        let cfg = test_config();
        let approver = Approver::new(ApprovalMode::Auto, Box::new(|_| true));
        let ctx = ToolContext { cfg: &cfg, approver: &approver };
        let rows = query_rows(&cfg, "select d.diagram_id from t_diagram d order by (select count(*) from t_diagramobjects o where o.diagram_id = d.diagram_id) desc limit 1").unwrap();
        let id: i64 = rows[0][0].parse().unwrap();
        let mut times = Vec::new();
        for _ in 0..3 {
            let started = std::time::Instant::now();
            let out = run("ea_show_diagram", &json!({ "diagram_id": id }), &ctx).expect("fetching the diagram");
            times.push(started.elapsed().as_secs_f32());
            let _ = std::fs::remove_file(cfg.attachments_dir.join(&out.images[0]));
        }
        println!("times of successive diagram fetches (s): {times:.1?}");
        assert!(times[1] < 15.0 && times[2] < 15.0, "subsequent calls should reuse the open session");
    }

    /// Optimistic locking on a real database: a stale timestamp rejects the write, a current one lets it through.
    /// Run with: `cargo test stale_object_is_rejected -- --ignored --nocapture`.
    #[test]
    #[ignore = "requires the model database from app.properties"]
    fn stale_object_is_rejected() {
        let cfg = test_config();
        let row = &query_rows(&cfg, "select package_id, modifieddate::text from t_package order by package_id limit 1").unwrap()[0];
        let id: i64 = row[0].parse().unwrap();
        let stale = check_unchanged(&cfg, &json!([{ "kind": "package", "id": id, "modified": "2000-01-01 00:00:00" }]));
        let fresh = check_unchanged(&cfg, &json!([{ "kind": "package", "id": id, "modified": row[1] }]));
        println!("stale: {:?} | fresh: {:?}", stale.as_ref().map_err(|e| e.to_string()), fresh.as_ref().map_err(|e| e.to_string()));
        assert!(stale.is_err());
        assert!(fresh.is_ok());
    }
    /// An EA file model: reads through SQLite give the same output as from a database, and nothing can write to the file.
    #[test]
    fn file_model_is_read_only() {
        let dir = std::env::temp_dir().join(format!("loonar-file-model-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("Shop.qea");
        let _ = std::fs::remove_file(&file);
        {
            let db = rusqlite::Connection::open(&file).unwrap();
            db.execute_batch(
                "create table t_package (Package_ID integer primary key, Name text, Parent_ID integer, ModifiedDate text);
                 insert into t_package values (1, 'Model', 0, '2026-10-17 17:33:05'), (2, 'Sales', 1, null);",
            )
            .unwrap();
        }
        std::fs::write(dir.join("app.properties"), format!("model.source=file\r\nmodel.file={}\r\n", file.display())).unwrap();
        let cfg = AppConfig::load(&dir.join("app.properties")).unwrap();
        assert!(cfg.uses_file());

        let out = db_query("select package_id, name, modifieddate from t_package order by package_id", &cfg).unwrap();
        assert_eq!(out, "Package_ID | Name | ModifiedDate\n1 | Model | 2026-10-17 17:33:05\n2 | Sales | NULL\n");
        assert!(check_unchanged(&cfg, &json!([{ "kind": "package", "id": 1, "modified": "2026-10-17T17:33:05" }])).is_ok());
        assert!(db_query("insert into t_package values (3, 'X', 1, null)", &cfg).is_err());
        assert!(db_query("with x as (select 1) delete from t_package", &cfg).is_err(), "a write hidden in WITH must fail");
        assert!(db_query("select load_extension('evil')", &cfg).is_err());
        assert_eq!(query_rows(&cfg, "select count(*) from t_package").unwrap(), vec![vec!["2".to_string()]]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Writing to an EA file (.qea) through EA and reading the result back from the file.
    /// The file comes from LOONAR_TEST_QEA (a model without security). With the file open in an EA window the write goes
    /// through that window, otherwise through EA in the background.
    /// Run with: `cargo test file_model_write_through_ea -- --ignored --nocapture`.
    #[test]
    #[ignore = "requires EA, a license and LOONAR_TEST_QEA"]
    fn file_model_write_through_ea() {
        let file = std::path::PathBuf::from(std::env::var("LOONAR_TEST_QEA").expect("LOONAR_TEST_QEA"));
        let dir = std::env::temp_dir().join(format!("loonar-ea-file-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("app.properties"), format!("model.source=file\r\nmodel.file={}\r\napproval.mode=auto\r\n", file.display())).unwrap();
        let cfg = AppConfig::load(&dir.join("app.properties")).unwrap();
        let root = query_rows(&cfg, "select package_id from t_package where parent_id = 0 order by package_id limit 1").unwrap()[0][0].clone();
        let name = format!("ZZ_file_test_{}", chrono::Local::now().format("%H%M%S"));
        let started = std::time::Instant::now();
        let ops = vec![
            json!({"op":"create_package","key":"pkg","parent": root.parse::<i64>().unwrap(),"name": name}),
            json!({"op":"create_diagram","key":"dgm","package":"#pkg","name":"Order process","type":"Activity"}),
            json!({"op":"create_element","key":"a","package":"#pkg","type":"Action","name":"Place order"}),
            json!({"op":"place","diagram":"#dgm","element":"#a","left":40,"top":40,"width":120,"height":60}),
        ];
        let result = ea::run_batch(&cfg, &ops, &BatchOptions::default()).expect("write through EA");
        println!("write: {} s, user {:?}, security {}", started.elapsed().as_secs(), result["user"], result["security"]);
        let rows = query_rows(&cfg, &format!("select p.name, (select count(*) from t_object o where o.package_id = p.package_id) from t_package p where p.name = '{name}'")).unwrap();
        println!("{rows:?}");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0][1], "1");
        ea::shutdown();
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn stamps_compare_by_seconds() {
        assert_eq!(normalize_stamp("2026-10-13T05:51:01.123"), normalize_stamp(" 2026-10-13 05:51:01 "));
        assert_ne!(normalize_stamp("2026-10-13 05:51:01"), normalize_stamp("2026-10-13 05:51:02"));
    }

    /// The model cannot request operations outside the list (e.g. deleting a package); the refusal comes before asking for approval and before EA.
    #[test]
    fn model_cannot_request_internal_ops() {
        let cfg = test_config();
        let asked = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let flag = asked.clone();
        let approver = Approver::new(ApprovalMode::Ask, Box::new(move |_| { flag.store(true, std::sync::atomic::Ordering::Relaxed); true }));
        let ctx = ToolContext { cfg: &cfg, approver: &approver };
        let err = run("ea_write", &json!({"ops":[{"op":"delete_package","package":1}]}), &ctx).err().expect("refusal");
        assert!(format!("{err:#}").contains("'delete_package'"), "{err:#}");
        assert!(!asked.load(std::sync::atomic::Ordering::Relaxed), "must not ask for approval of a disallowed operation");
    }

    /// Queries that change data are refused before they reach the database (also through EA's own connection).
    #[test]
    fn queries_cannot_write() {
        assert_eq!(write_word("select name from t_object where note like '%delete%' and \"Update\" = 1"), None);
        assert_eq!(write_word("with d as (delete from t_object returning 1) select * from d").as_deref(), Some("delete"));
        assert_eq!(write_word("select * into newtable from t_object").as_deref(), Some("into"));
        assert_eq!(write_word("with x as (select 1) DELETE from t_connector").as_deref(), Some("delete"));
        assert_eq!(write_word("select U&\"pg\\005fread\\005ffile\"('x')").as_deref(), Some("U&"));
        assert_eq!(write_word("select 1 -- update\n from t_object /* drop */"), None);
        assert_eq!(write_word("select o.name, offset_x, reset_flag from t_object o order by 1 offset 5"), None);
        assert!(db_query("select query_to_xml('select 1', true, false, '')", &test_config()).is_err());
        // Ids as text or fractions are numbers for the checks, as they are for EA.
        let ops = normalize_ops(&[json!({"op":"delete","element":"123"}), json!({"op":"update_element","element":7.0,"name":"2024"}), json!({"op":"create_connector","source":"#a","target":"5"})]);
        assert_eq!(ops[0]["element"], 123);
        assert_eq!(ops[1]["element"], 7);
        assert_eq!(ops[1]["name"], "2024", "names stay text");
        assert_eq!(normalize_ops(&[json!({"op":"delete","package":12.4})])[0]["package"], 12, "rounded as EA rounds it");
        assert!(check_file_path(r"C:\PROGRA~1\x.xml").is_err(), "a short name of Program Files is resolved and refused");
        assert!(check_file_path(r"C:\x\.\y.xml").is_err() && check_file_path(r"C:\x\a.xml:s").is_err());
        assert!(check_file_path(r"C:\x\y.\z.xml").is_err() && check_file_path(r"C:\x\\z.xml").is_err() && check_file_path(r"C:\Users\u\AppData\Roaming\x.xml").is_err());
        let temp = std::env::temp_dir().join("loonar-export-test").join("m.xml");
        assert!(check_file_path(&temp.display().to_string()).is_ok(), "the profile's Temp folder (also as a short name) is allowed");
        for sql in ["select $$x$$ from t_object", "select e'a\\'b' from t_object", "select [name] from t_object", "select 1 /* x */ from t_object", "select 1 -- x"] {
            assert!(db_query(sql, &test_config()).is_err(), "{sql}");
        }
        assert_eq!((ops[2]["source"].as_str(), ops[2]["target"].as_i64()), (Some("#a"), Some(5)));
        // Files of XMI and reports: local full paths outside system folders.
        assert!(check_file_path(r"C:\Export\model.xml").is_ok() && check_file_path("D:/Reports/a.docx").is_ok());
        assert!(check_file_path(r"\\server\share\m.xml").is_err() && check_file_path("model.xml").is_err() && check_file_path(r"C:\Export\..\Windows\x.xml").is_err());
        assert!(check_file_path(r"C:\Windows\Temp\x.xml").is_err() && check_file_path(r"C:\Users\a\AppData\Roaming\Microsoft\Windows\Start Menu\Programs\Startup\x.xml").is_err());
    }

    /// Approval modes: ask asks every time, plan once per turn, auto never; a refusal blocks the write.
    #[test]
    fn approval_modes() {
        let count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let make = |mode, answer: bool| {
            let c = count.clone();
            Approver::new(mode, Box::new(move |_| { c.fetch_add(1, std::sync::atomic::Ordering::Relaxed); answer }))
        };
        let n = || count.load(std::sync::atomic::Ordering::Relaxed);

        let auto = make(ApprovalMode::Auto, false);
        assert!(auto.confirm("x").is_ok() && n() == 0);

        let ask = make(ApprovalMode::Ask, true);
        assert!(ask.confirm("a").is_ok() && ask.confirm("b").is_ok());
        assert_eq!(n(), 2);

        let plan = make(ApprovalMode::Plan, true);
        assert!(plan.confirm("a").is_ok() && plan.confirm("b").is_ok());
        assert_eq!(n(), 3, "plan asks once");

        let refuse = make(ApprovalMode::Ask, false);
        assert!(refuse.confirm("a").is_err());
    }

    /// A real diagram fetch from EA through COM. Run with: `cargo test show_diagram -- --ignored --nocapture`.
    #[test]
    #[ignore = "requires EA, a license and the model database from app.properties"]
    fn show_diagram_exports_png() {
        let cfg = test_config();
        let approver = Approver::new(ApprovalMode::Auto, Box::new(|_| true));
        let ctx = ToolContext { cfg: &cfg, approver: &approver };
        let rows = query_rows(&cfg, "select d.diagram_id, d.name from t_diagram d order by (select count(*) from t_diagramobjects o where o.diagram_id = d.diagram_id) desc limit 1").unwrap();
        let id: i64 = rows[0][0].parse().unwrap();
        println!("diagram: {} (id {id})", rows[0][1]);
        let started = std::time::Instant::now();
        let out = run("ea_show_diagram", &json!({ "diagram_id": id }), &ctx).expect("fetching the diagram");
        println!("time: {} s", started.elapsed().as_secs());
        assert_eq!(out.images.len(), 1);
        let path = cfg.attachments_dir.join(&out.images[0]);
        println!("png: {}", path.display());
        println!("text for the model: {}", out.text);
        assert!(std::fs::metadata(&path).unwrap().len() > 1000, "the image must not be empty");
    }
    /// Impact analysis on the test database: the element with the most relationships has neighbors at level 1.
    /// Run with: `cargo test impact_analysis_on_test_db -- --ignored --nocapture`.
    #[test]
    #[ignore = "requires the model database from app.properties"]
    fn impact_analysis_on_test_db() {
        let cfg = crate::config::AppConfig::load(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("app.properties")).unwrap();
        let top = query_rows(&cfg, "select start_object_id from t_connector group by 1 order by count(*) desc limit 1").unwrap();
        let id: i64 = top[0][0].parse().unwrap();
        let out = impact_analysis(&serde_json::json!({ "element": id, "depth": 2 }), &cfg).unwrap();
        println!("{out}");
        assert!(out.contains("\n1 | "), "no elements at level 1");
    }
    /// Operation usage on a model from the database (app.properties by default; another database: LOONAR_TEST_DB, element: LOONAR_TEST_ELEMENT).
    /// Run with: `cargo test operation_usage_on_db -- --ignored --nocapture`.
    #[test]
    #[ignore = "requires the model database"]
    fn operation_usage_on_db() {
        let mut cfg = crate::config::AppConfig::load(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("app.properties")).unwrap();
        if let Ok(db) = std::env::var("LOONAR_TEST_DB") {
            cfg.db_name = db;
        }
        let element: i64 = match std::env::var("LOONAR_TEST_ELEMENT") {
            Ok(id) => id.parse().unwrap(),
            Err(_) => query_rows(&cfg, "select object_id from t_operation group by 1 order by count(*) desc limit 1").unwrap()[0][0].parse().unwrap(),
        };
        let out = operation_usage(&serde_json::json!({ "element": element }), &cfg).unwrap();
        println!("{out}");
        assert!(out.starts_with("Operation usage"));
    }
}

#[cfg(test)]
mod script_shape_tests {
    use super::script_shape;

    #[test]
    fn scripts_differing_only_in_data_have_one_build() {
        let a = "var map = {535:\"/cart/{cartId}/standard\",536:\"/cart/x\",537:\"/a\"};
for (var id in map) { var m = Repository.GetMethodByID(id); m.Style = map[id]; m.Update(); }";
        let b = "var map = {
 701:\"/offer/{id}\",
 702:\"/offer/y\",
 703:\"/b\",
 704:\"/c\" };
for (var id in map) { var m = Repository.GetMethodByID(id); m.Style = map[id]; m.Update(); }";
        assert_eq!(script_shape(a), script_shape(b));
        let objects = |n: usize| format!("var rows = [{}]; rows.forEach(function (r) {{ Session.Output(r.name); }});", (0..n).map(|i| format!("{{id:{i},name:\"n{i}\"}}")).collect::<Vec<_>>().join(","));
        assert_eq!(script_shape(&objects(3)), script_shape(&objects(9)));
    }

    #[test]
    fn other_code_is_another_build() {
        let a = "var map = {1:\"/a\",2:\"/b\",3:\"/c\"}; for (var id in map) { var m = Repository.GetMethodByID(id); m.Style = map[id]; m.Update(); }";
        let b = "var map = {1:\"/a\",2:\"/b\",3:\"/c\"}; for (var id in map) { var m = Repository.GetMethodByID(id); m.Delete(); }";
        assert_ne!(script_shape(a), script_shape(b));
        assert_ne!(script_shape("Session.Output(\"x\");"), script_shape("eval(\"x\");"));
    }

    /// LC-346: an approved UPDATE through Repository.Execute does not approve a DELETE (the SQL is in a string).
    #[test]
    fn sql_in_strings_is_part_of_the_build() {
        let update = "Repository.Execute(\"UPDATE t_object SET Status = 'Approved' WHERE Object_ID = 5\");";
        let delete = "Repository.Execute(\"DELETE FROM t_object\");";
        assert_ne!(script_shape(update), script_shape(delete));
        let other_id = "Repository.Execute(\"UPDATE t_object SET Status = 'Approved' WHERE Object_ID = 7\");";
        assert_ne!(script_shape(update), script_shape(other_id), "the SQL text is compared as it is");
        // Plain data in strings is still data.
        assert_eq!(script_shape("e.Notes = \"Zamówienie klienta\"; e.Update();"), script_shape("e.Notes = \"Faktura\"; e.Update();"));
        assert_ne!(script_shape("var s = 'Session'; WScript.Run('a')"), script_shape("var s = 'Session'; WScript.Run('cmd /c del x')"));
    }
}

#[cfg(test)]
mod trust_tests {
    #[test]
    fn approval_questions_are_recognised_by_kind() {
        let jira = crate::i18n::fmt(super::JIRA_CREATE_QUESTION, &["PROJX".into(), "Story".into(), "US-05".into(), "text".into()]);
        assert_eq!(super::trustable_kind(&jira).map(|k| k.0), Some("jira"));
        let page = crate::i18n::fmt(super::CONFLUENCE_NEW_QUESTION, &["T".into(), "PM".into()]);
        assert_eq!(super::trustable_kind(&page).map(|k| k.0), Some("confluence"));
        assert_eq!(super::trustable_kind("Delete 5 elements with their relationships"), None);
    }
}

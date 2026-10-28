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

/// The build of a script: its code without the content of string literals and without numbers, whitespace collapsed.
/// Two scripts with the same build differ only in their data.
pub fn script_shape(code: &str) -> String {
    let mut out = String::new();
    let mut chars = code.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' | '\'' => {
                out.push(c);
                while let Some(d) = chars.next() {
                    if d == '\\' {
                        chars.next();
                    } else if d == c {
                        break;
                    }
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
create_connector {source, target, type, stereotype?, name?, tags?, diagram?, sequence?, message_sort?, is_return?, subtype?} (type: Association, Aggregation, Dependency, Generalization, Realisation, ControlFlow, InformationFlow, Sequence...; aggregation and composition are the connector type Aggregation (never an Association with a stereotype): source = the part, target = the whole (the diamond is at the whole), subtype Weak = shared aggregation (default), Strong = composition; a message of a sequence diagram: type Sequence, diagram = that diagram, sequence = its number in the order, message_sort Synchronous or Asynchronous, is_return true for a reply; EA lays messages out from top to bottom by that number); \
place {diagram, element, left, top, width, height, bgcolor?} (bgcolor is the fill color on this diagram, written as #RRGGBB (e.g. #FFE599); pixels; the top-left corner of the diagram is 0,0, the Y axis grows downwards; place a frame that contains other elements before its contents); \
update_element {element, name?, notes?, alias?, stereotype?, clear_stereotype?, status?, version?, author?, phase?, complexity?, abstract?, tags?, color?, keywords?} (keywords = the Keywords field in EA, t_object.pdata5 in the database, a text or a list; used, among other things, by legends for coloring; complexity is '1' easy, '2' medium, '3' difficult; abstract is true or false; a tag with the value null is removed); \
update_connector {connector, name?, stereotype?, clear_stereotype?, direction?, notes?, source_multiplicity?, source_role?, target_multiplicity?, target_role?, tags?, sequence?, message_sort?, is_return?} (properties of an existing relationship, connector = t_connector.connector_id; direction: 'Unspecified', 'Source -> Destination', 'Destination -> Source' or 'Bi-Directional'; multiplicities e.g. '1', '0..1', '1..*'); create_legend {diagram, title?, filter?, entries, connector_filter?, connector_entries?, left?, top?} (a diagram legend with automatic coloring, as in EA: filter = the element property the legend colors by, Element.Keyword by default, others e.g. Element.Stereotype, Element.Status, Element.Type; entries = [{value, color #RRGGBB, label?}] the fill color of elements with that value; connector_filter e.g. Connector.Stereotype and connector_entries = [{value, color, width?}] the line color of connectors; elements are colored automatically once they have the value set, e.g. the keyword prio1; in the database a legend is a t_object of type Text with ntype 76); \
move {package, element | diagram | child_package} (moves an existing element, diagram or package into the package given in package; connectors and positions on diagrams are kept, so never recreate objects just to change their package); \
delete {element | diagram | connector | package} (permanently deletes an object from the model; an element takes its connectors and diagram occurrences with it, a package all of its contents; delete only what the user asked for or what you created by mistake yourself, and tell the user what you deleted; take the connector id from t_connector.connector_id); \
remove_from_diagram {diagram, element} (removes the element from the diagram, the element stays in the model); \
reverse_connector {connector} (swaps the source and the target of a relationship); \
update_package {package, name?, notes?, version?, status?, stereotype?}; \
version_control {package, action, comment?} (version control of a package that is already under version control in EA: action check_out, check_in, get_latest (replaces the package with the version from version control), put_latest (checks in and keeps it checked out) or resync (reads the version control status of the package again); comment for check_out, check_in and put_latest; always asked; call version_control_status first); \
generate_report {package, file, template?} (a document from an EA document template, like Publish → Report Builder: file = a full path ending .docx, .pdf or .rtf, by default in the attachments folder; template = the name of an EA template, default \"Model Report\"; use it when the user wants EA's own report format or a company template; for documents you write yourself use create_document); \
layout_diagram {diagram, direction?, spacing?} (EA's automatic layout of the whole diagram, like Layout → Diagram Layout: direction down (default), right, up or left; spacing in pixels, default 40; use it after creating a diagram with many elements or when the user asks to tidy a diagram; then check the result with ea_show_diagram); \
export_xmi {package, file} (exports the package with its contents and diagrams to an XMI file; file = a full path ending .xml or .xmi, by default in the attachments folder); \
import_xmi {package, file, keep_guids?} (imports an XMI file into the package; GUIDs are replaced unless keep_guids is true, so an import never overwrites existing objects; only on the user's request); \
set_linked_document {element, text} (the element's linked document (Ctrl+Alt+D in EA), from plain text, one paragraph per line; replaces an existing one); \
set_swimlanes {diagram, lanes: [{title, width?, color?}], orientation?} (diagram swimlanes (Diagram → Swimlanes), replaces existing ones; orientation vertical (default) or horizontal); \
set_composite {element, diagram} (links the element to an existing diagram as its composite diagram (in EA: New Child Diagram → Composite → Select Composite Diagram): when the element is double-clicked, EA opens that diagram instead of the properties; this is how you link a BPMN subprocess to the subprocess diagram, also when the diagram is in an ordinary package; in the database the linked element has t_object.ntype = 8 and the diagram id in the pdata1 column); \
create_attribute {element, name, type?, visibility?, notes?, default?, stereotype?, alias?} (a class attribute, visibility: Public|Private|Protected|Package); create_operation {element, name, return_type?, visibility?, notes?, stereotype?, alias?, parameters?: [{name, type}]} (a class operation with parameters; alias is the operation's alias in EA, e.g. the REST path of an endpoint: set it here, no script is needed); update_attribute {attribute, name?, type?, visibility?, notes?, default?, stereotype?, alias?, static?} and delete_attribute {attribute} (attribute = t_attribute.id); update_operation {operation, name?, return_type?, visibility?, notes?, stereotype?, alias?, static?} and delete_operation {operation} (operation = t_operation.operationid); clone_package {package, name?, key?} (a copy of the package with all its contents next to the original, e.g. a variant to experiment with); update_scenario {element, scenario, name?, type?, notes?, steps?} (changes an existing structured scenario found by its name or GUID; steps, when given, replace all steps); delete_scenario {element, scenario}; create_scenario {element, name, type?, notes?, steps: [{text, actor?, uses?, results?}]} (a STRUCTURED SCENARIO of an element, e.g. a use case: the Scenarios tab of the element, with numbered steps; ALWAYS use this, never notes, when the user asks for scenarios, flows or structured scenarios; type: Basic Path (default), Alternate or Exception; actor = true for a step done by the actor, false for the system; in the database t_objectscenarios, steps in its xmlcontent); add_glossary_term {term, type?, meaning, forms?: [..]} (a term in the EA PROJECT GLOSSARY, the list EA highlights in notes and documents with its meaning - not a model element; t_glossary holds the terms; EA highlights only the exact written form, so for a Polish term ALWAYS pass in forms every inflected form of it: singular and plural in all seven cases from the nominative to the vocative, for a multi-word term the whole phrase inflected; type: the one the user names, otherwise Glossary (the type appears in EA once used); ask the user for the meaning if they did not give it); create_baseline {package, version, notes?} (saves a version of the package in the model; ALWAYS make a baseline before deleting from or substantially restructuring an existing package that you did not create in this conversation, and tell the user they can restore it in EA: package in the browser → Manage Baselines → Compare/Restore; baselines are in t_document with doctype = 'Baseline'); \
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
                                "image": { "type": "string", "description": "diagram:ID or image:NAME" }
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
Content in Markdown: # / ## / ### headings, paragraphs with **bold**, lists (- item, 1. item), tables (| a | b | with a |---|---| row), \
images on separate lines: ![caption](diagram:ID) inserts a diagram from EA (id from t_diagram.diagram_id), ![caption](image:NAME) inserts an image \
drawn earlier with render_image (NAME from its result). Use it when the user asks for a document, description, specification or report for Word.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "file_name": { "type": "string", "description": "File name without extension." },
                    "title": { "type": "string", "description": "Title at the beginning of the document (optional)." },
                    "markdown": { "type": "string", "description": "Document content in Markdown." },
                    "format": { "type": "string", "enum": ["docx", "pdf", "both"], "description": "docx (default), pdf (made with Microsoft Word on this computer) or both." }
                },
                "required": ["file_name", "markdown"]
            }
        },
        {
            "name": "ea_ui",
            "description": "Works with the Enterprise Architect window that the user has open (the same model): \
action=context returns the open diagram, the elements and the connector selected on it, and the selection in the project browser \
(use it when the user says \"this\", \"the selected\", \"this diagram\", \"here\"); action=open_diagram {diagram} opens the diagram in the user's EA window; \
action=show_element {element} shows the element in the project browser. After every ea_write the changed diagrams refresh \
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
                    "attachments": ATTACHMENTS_SCHEMA.clone()
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
                    "attachments": ATTACHMENTS_SCHEMA.clone()
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
                    "markdown": { "type": "string", "description": "Page content." }
                },
                "required": ["title", "markdown"]
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
            ctx.approver.confirm_kind("mcp", &crate::i18n::fmt(MCP_QUESTION, &[tool, server, args.chars().take(1500).collect()]))?;
        }
        return crate::mcp_client::call(&ctx.cfg.base_dir, name, input).map(Into::into);
    }
    match name {
        "db_query" => db_query(input["sql"].as_str().with_context(|| crate::i18n::fmt("missing field '{0}'", &["sql".to_string()]))?, ctx.cfg).map(Into::into),
        "ea_write" => ea_write(input, ctx).map(Into::into),
        "ea_show_diagram" => ea_show_diagram(input, ctx),
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
        let op = json!({ "op": "export_diagram", "diagram": id, "path": path.to_string_lossy() });
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
            match ea::run_batch(ctx.cfg, &ops, &BatchOptions::default()) {
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
    refresh_user_ea(ctx.cfg, &[], &json!({}));
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
            let (statements, updated) = crate::scripts::save_statements(ctx.cfg, &group, &name, &language, &code)?;
            // A script in the model can be run later by anyone from EA: saving it is always asked, with the whole code.
            ctx.approver.confirm_script("save", &code, &crate::i18n::fmt(SAVE_SCRIPT_QUESTION, &[
                group.clone(), name.clone(), language.clone(), code.lines().count().to_string(),
                if updated { crate::i18n::tr(" - REPLACES the existing script").to_string() } else { String::new() }, script_for_approval(&code)?,
            ]))?;
            ea::run_batch(ctx.cfg, &[json!({ "op": "execute_sql", "statements": statements })], &BatchOptions::default())?;
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
        let markup = jira_image_markup(&files.iter().map(|(n, _)| n.clone()).collect::<Vec<_>>());
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
            let bytes = std::fs::read(ctx.cfg.attachments_dir.join(&stored)).with_context(|| crate::i18n::fmt("there is no file {0} in this conversation", &[stored.clone()]))?;
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
fn jira_image_markup(names: &[String]) -> String {
    names
        .iter()
        .filter(|n| matches!(n.to_lowercase().rsplit('.').next(), Some("png" | "jpg" | "jpeg" | "gif")))
        .map(|n| format!("!{n}|thumbnail!"))
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
        Some(d) if !connection.cloud() && !jira_image_markup(&files.iter().map(|(n, _)| n.clone()).collect::<Vec<_>>()).is_empty() => {
            with_images = format!("{}\n\n{}", d.trim_end(), jira_image_markup(&files.iter().map(|(n, _)| n.clone()).collect::<Vec<_>>()));
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

fn shorten_for_approval(text: &str) -> String {
    if text.chars().count() > 600 { format!("{}…", text.chars().take(600).collect::<String>()) } else { text.to_string() }
}

fn create_confluence_page(input: &Value, ctx: &ToolContext) -> Result<ToolOutput> {
    let title = input["title"].as_str().with_context(|| crate::i18n::fmt("missing field '{0}'", &["title".to_string()]))?;
    let markdown = input["markdown"].as_str().with_context(|| crate::i18n::fmt("missing field '{0}'", &["markdown".to_string()]))?;
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
             \"Copy page code\" (to paste into the Confluence source editor) and \"Open images folder\" (to attach the images). {publish_hint}",
            storage.len() / 1024,
            attachments.len()
        ),
        images: Vec::new(),
        files: vec![stored],
    })
}

fn confluence_connection(cfg: &AppConfig) -> Result<crate::confluence::Connection> {
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
    ctx.approver.confirm_kind("confluence", &question)?;
    let (storage, attachments) = crate::confluence::to_storage(markdown, |source| picture(ctx, source))?;
    let target_id = existing.as_ref().map(|page| page.id.clone());
    let (id, link, created) = connection.publish(&space, &title, Some(&parent), target_id.as_deref(), &storage, &attachments)?;
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
    std::fs::create_dir_all(&ctx.cfg.attachments_dir).context(crate::i18n::tr("attachments folder"))?;
    let mut pictures = 0;
    let bytes = crate::docx::build(title, markdown, |source, caption| {
        pictures += 1;
        let png = picture(ctx, source)?;
        Ok(crate::docx::Picture { png, caption: caption.to_string() })
    })?;
    let stored = format!("{}_{}.docx", uuid::Uuid::new_v4().simple(), crate::history::sanitize_name(file_name.trim_end_matches(".docx")));
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
        return Ok(ToolOutput { text: format!("Document \"{file_name}\" is ready as PDF{} (images: {pictures}). The user sees it in the chat.", if format == "both" { " and DOCX" } else { "" }), images: Vec::new(), files });
    }
    Ok(ToolOutput {
        text: format!(
            "Document \"{file_name}.docx\" is ready ({} KB, images: {pictures}). The user sees it in the chat with the \"Open\" and \"Save as…\" buttons. It is kept only in Compass: do not name a folder or path for it; the user chooses where to keep it with \"Save as…\".",
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
        slides.push(crate::pptx::Slide { title: s["title"].as_str().unwrap_or("").to_string(), bullets, picture });
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
fn destructive_details(cfg: &AppConfig, ops: &[Value]) -> Vec<String> {
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
                lines.push(crate::i18n::fmt("DELETE {0} of element {1}", &[kind.trim_start_matches("delete_").to_string(), op["element"].as_i64().map(|id| name_of("t_object", "object_id", id)).unwrap_or_else(|| op["element"].to_string())]));
                continue;
            }
            "version_control" => {
                let package = op["package"].as_i64().map(|id| name_of("t_package", "package_id", id)).unwrap_or_else(|| op["package"].to_string());
                lines.push(crate::i18n::fmt("VERSION CONTROL: {0} of package {1}", &[op["action"].as_str().unwrap_or("?").replace('_', " ").to_uppercase(), package]));
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
        let Some((field, name)) = target else { continue };
        let what = match field {
            "element" => crate::i18n::tr("element"),
            "diagram" => crate::i18n::tr("diagram"),
            "package" | "child_package" => crate::i18n::tr("package"),
            _ => crate::i18n::tr("connector"),
        };
        lines.push(match kind {
            "delete" => crate::i18n::fmt("DELETE {0} {1}", &[what.to_string(), name]),
            "move" => crate::i18n::fmt("MOVE {0} {1} to package {2}", &[
                what.to_string(),
                name,
                op["package"].as_i64().map(|id| name_of("t_package", "package_id", id)).unwrap_or_else(|| op["package"].to_string()),
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
    let attachment = ctx.cfg.attachments_dir.join(given);
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
        if matches!(kind.as_str(), "export_xmi" | "import_xmi" | "generate_report") {
            // Files outside the model are always asked (also in auto), with the real full path in the dialog and for EA.
            let path = check_file_path(op["file"].as_str().unwrap_or(""))?;
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
    if !related.is_empty() || package_deleted || !risky_files.is_empty() {
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
        let made = ea::run_batch(ctx.cfg, &[json!({ "op": "create_baseline", "package": package, "version": version, "notes": crate::i18n::tr("Made by Compass before a change that cannot be undone by operations") })], &BatchOptions::default());
        if made.is_ok() {
            baselines.push(format!("package {package}: {version}"));
        }
    }
    let result = ea::run_batch(ctx.cfg, ops, &BatchOptions::default());
    audit(ctx.cfg, ops, &result);
    if let Ok(done) = &result {
        crate::undo::record(ctx.cfg, &ea::describe(ops), done, reverse, not_undoable, baselines.clone());
    }
    if let Ok(done) = &result {
        refresh_user_ea(ctx.cfg, ops, done);
    }
    let done = result?;
    let mut text = format_created(&done);
    if !related.is_empty() {
        text.push_str(&format!("\nDeleted together with relationships (tell the user): {}\n", related.join(" | ")));
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

/// After a write, refreshes the changed diagrams and packages in the user's EA window (in the background, without waiting; does nothing without an EA window).
fn refresh_user_ea(cfg: &AppConfig, ops: &[Value], result: &Value) {
    let mut diagrams: Vec<i32> = Vec::new();
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
fn audit(cfg: &AppConfig, ops: &[Value], result: &Result<Value>) {
    use std::io::Write;
    let entry = json!({
        "time": chrono::Local::now().to_rfc3339(),
        "database": if cfg.uses_file() { cfg.model_file.display().to_string() } else { format!("{}@{}:{}", cfg.db_name, cfg.db_host, cfg.db_port) },
        "ea_user": cfg.ea_user,
        "approval": cfg.approval_mode.as_str(),
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
        let rows = query_rows(cfg, &format!("select cast(modifieddate as text) from {table} where {key} = {id}"))?;
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
        let props = Path::new(env!("CARGO_MANIFEST_DIR")).join("app.properties");
        if props.exists() {
            return AppConfig::load(&props).expect("app.properties");
        }
        let dir = std::env::temp_dir().join(format!("loonar-tools-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("app.properties"), "").unwrap();
        AppConfig::load(&dir.join("app.properties")).unwrap()
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
}

#[cfg(test)]
mod trust_tests {
    #[test]
    fn approval_questions_are_recognised_by_kind() {
        let jira = crate::i18n::fmt(super::JIRA_CREATE_QUESTION, &["ITSPX".into(), "Story".into(), "US-05".into(), "text".into()]);
        assert_eq!(super::trustable_kind(&jira).map(|k| k.0), Some("jira"));
        let page = crate::i18n::fmt(super::CONFLUENCE_NEW_QUESTION, &["T".into(), "PM".into()]);
        assert_eq!(super::trustable_kind(&page).map(|k| k.0), Some("confluence"));
        assert_eq!(super::trustable_kind("Delete 5 elements with their relationships"), None);
    }
}

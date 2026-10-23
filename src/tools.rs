//! Tools for the model:
//! - `db_query`: reads the model from PostgreSQL or from an EA file (SQLite) (SELECT/WITH only),
//! - `ea_write`: generic write to EA through COM (packages, diagrams, elements, connectors, layout).
//! There are no importers or format-specific logic here: the model decides what to draw
//! (from a photo, a description or a file) and builds the list of operations itself.
//! Writes go through approval according to the mode from the settings (plan / ask / auto).

use crate::config::{AppConfig, ApprovalMode};
use crate::ea::{self, BatchOptions};
use crate::memory;
use anyhow::{bail, Context, Result};
use postgres::SimpleQueryMessage;
use serde_json::{json, Value};
use std::cell::Cell;
use std::time::Duration;

const MAX_ROWS: usize = 200;

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

    fn confirm(&self, summary: &str) -> Result<()> {
        let must_ask = match self.mode {
            ApprovalMode::Auto => false,
            ApprovalMode::Ask => true,
            ApprovalMode::Plan => !self.approved_in_turn.get(),
        };
        if must_ask && !(self.ask)(summary.to_string()) {
            bail!("{}", crate::i18n::tr("the user rejected this change; nothing was saved"));
        }
        self.approved_in_turn.set(true);
        Ok(())
    }

    /// Destructive changes (deleting, moving, copying whole packages) ask also in `plan` mode after an earlier
    /// approval in the same answer: the plan the user approved may not have shown them.
    fn confirm_destructive(&self, summary: &str) -> Result<()> {
        if self.mode != ApprovalMode::Auto && !(self.ask)(summary.to_string()) {
            bail!("{}", crate::i18n::tr("the user rejected this change; nothing was saved"));
        }
        self.approved_in_turn.set(true);
        Ok(())
    }

    /// Consent regardless of the mode (also in auto) for a risky change of the model (deleting elements with relationships).
    fn confirm_risky(&self, summary: &str) -> Result<()> {
        if !(self.ask)(summary.to_string()) {
            bail!("{}", crate::i18n::tr("the user rejected this change; nothing was saved"));
        }
        self.approved_in_turn.set(true);
        Ok(())
    }

    /// Consent regardless of the mode (also in auto): for actions that send content outside the application and the model.
    fn confirm_always(&self, summary: &str) -> Result<()> {
        if !(self.ask)(summary.to_string()) {
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
        items.push(crate::scripts::definition());
        items.push(crate::traceability::definition());
        items.push(crate::code_drift::definition());
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
            "name": "db_query",
            "description": "Read-only access (SELECT/WITH only) to the EA model: a PostgreSQL database or an EA project file (.qea/.qeax, a SQLite database); the system prompt says which, and you write the query in that SQL dialect. Table and column names are lowercase. \
Tables: t_package (package_id, name, parent_id, ea_guid, modifieddate), t_object (object_id, name, alias, object_type, stereotype, package_id, parentid, note, pdata1, ea_guid, modifieddate), \
t_diagram (diagram_id, name, diagram_type, package_id, parentid, ea_guid, modifieddate), t_diagramobjects (diagram_id, object_id, rectleft, recttop, rectright, rectbottom; the Y axis is inverted in the database: recttop and rectbottom are negative, so an element placed with place at top=50 and height=115 has recttop=-50 and rectbottom=-165, and that is the correct position on the diagram), \
t_connector (connector_id, name, connector_type, stereotype, start_object_id, end_object_id), t_objectproperties (object_id, property, value), t_operation. \
Result: at most 200 rows.",
            "input_schema": {
                "type": "object",
                "properties": { "sql": { "type": "string", "description": "A single SELECT or WITH query." } },
                "required": ["sql"]
            }
        },
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
generate_report {package, file, template?} (a document from an EA document template, like Publish → Report Builder: file = a full path ending .docx, .pdf or .rtf, by default in the attachments folder; template = the name of an EA template, default \"Model Report\"; use it when the user wants EA's own report format or a company template; for documents you write yourself use create_document); \
layout_diagram {diagram, direction?, spacing?} (EA's automatic layout of the whole diagram, like Layout → Diagram Layout: direction down (default), right, up or left; spacing in pixels, default 40; use it after creating a diagram with many elements or when the user asks to tidy a diagram; then check the result with ea_show_diagram); \
export_xmi {package, file} (exports the package with its contents and diagrams to an XMI file; file = a full path ending .xml or .xmi, by default in the attachments folder); \
import_xmi {package, file, keep_guids?} (imports an XMI file into the package; GUIDs are replaced unless keep_guids is true, so an import never overwrites existing objects; only on the user's request); \
set_linked_document {element, text} (the element's linked document (Ctrl+Alt+D in EA), from plain text, one paragraph per line; replaces an existing one); \
set_swimlanes {diagram, lanes: [{title, width?, color?}], orientation?} (diagram swimlanes (Diagram → Swimlanes), replaces existing ones; orientation vertical (default) or horizontal); \
set_composite {element, diagram} (links the element to an existing diagram as its composite diagram (in EA: New Child Diagram → Composite → Select Composite Diagram): when the element is double-clicked, EA opens that diagram instead of the properties; this is how you link a BPMN subprocess to the subprocess diagram, also when the diagram is in an ordinary package; in the database the linked element has t_object.ntype = 8 and the diagram id in the pdata1 column); \
create_attribute {element, name, type?, visibility?, notes?, default?, stereotype?} (a class attribute, visibility: Public|Private|Protected|Package); create_operation {element, name, return_type?, visibility?, notes?, stereotype?, parameters?: [{name, type}]} (a class operation with parameters); update_attribute {attribute, name?, type?, visibility?, notes?, default?, static?} and delete_attribute {attribute} (attribute = t_attribute.id); update_operation {operation, name?, return_type?, visibility?, notes?, static?} and delete_operation {operation} (operation = t_operation.operationid); clone_package {package, name?, key?} (a copy of the package with all its contents next to the original, e.g. a variant to experiment with); update_scenario {element, scenario, name?, type?, notes?, steps?} (changes an existing structured scenario found by its name or GUID; steps, when given, replace all steps); delete_scenario {element, scenario}; create_scenario {element, name, type?, notes?, steps: [{text, actor?, uses?, results?}]} (a STRUCTURED SCENARIO of an element, e.g. a use case: the Scenarios tab of the element, with numbered steps; ALWAYS use this, never notes, when the user asks for scenarios, flows or structured scenarios; type: Basic Path (default), Alternate or Exception; actor = true for a step done by the actor, false for the system; in the database t_objectscenarios, steps in its xmlcontent); add_glossary_term {term, type?, meaning, forms?: [..]} (a term in the EA PROJECT GLOSSARY, the list EA highlights in notes and documents with its meaning - not a model element; t_glossary holds the terms; EA highlights only the exact written form, so for a Polish term ALWAYS pass in forms every inflected form of it: singular and plural in all seven cases from the nominative to the vocative, for a multi-word term the whole phrase inflected; type: the one the user names, otherwise Slownik (the type appears in EA once used); ask the user for the meaning if they did not give it); create_baseline {package, version, notes?} (saves a version of the package in the model; ALWAYS make a baseline before deleting from or substantially restructuring an existing package that you did not create in this conversation, and tell the user they can restore it in EA: package in the browser → Manage Baselines → Compare/Restore; baselines are in t_document with doctype = 'Baseline'); \
update_diagram {diagram, name?, notes?, stereotype?} (changes the name, notes or stereotype of an existing diagram); hide_connectors {diagram, connectors, hidden?} (hides the given connectors on this one diagram only, a list of ids from t_connector.connector_id; hidden=false shows them again; the connectors stay in the model and on other diagrams, so this is how you declutter a dense diagram instead of deleting relationships; in the database: t_diagramlinks.hidden); \
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
                    "markdown": { "type": "string", "description": "Document content in Markdown." }
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
            "name": "operation_usage",
            "description": "Where the operations (services) of a class, component or interface are used: on which diagrams (sequence: messages to the element \
or its lifeline; activity: actions that invoke the operation), in which packages and who calls them; at the end, a list of unused operations. \
Use it when the user asks \"where is this service/operation/method used\", \"who calls it\", \"is this used anywhere\". \
operation optionally narrows the result down to operations whose name contains this text.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "element": { "type": "integer", "description": "object_id of the class, component or interface (from db_query)." },
                    "operation": { "type": "string", "description": "Optional fragment of the operation name." }
                },
                "required": ["element"]
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
                    "link_url": { "type": "string" },
                    "link_title": { "type": "string" }
                },
                "required": ["summary"]
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
            "name": "model_lint",
            "description": "Model linter (governance) without AI guessing: deterministic checks of a package with all its subpackages. \
Finds default or empty names (Class1, Activity 2), elements without a description, orphaned elements (no relationships and on no diagram), \
duplicate names, relationships to non-existent elements, duplicate relationships and self-relationships, empty packages and diagrams, \
and the organization's rules from governance.json (or given in 'rules'): naming conventions, required tagged values and allowed relationships (MDG / metamodel). \
Each finding has a severity (error, warning, info), the rule, the object id and the problem. Present the findings grouped by severity with concrete fix proposals; \
fix them only after the user agrees (ea_write). On request turn the result into an Excel workbook or a document. \
The linter is a deterministic baseline: when the user asks for a review, also judge the model yourself (e.g. db_query for meaning, inheritance that makes no sense such as an invoice specialising a product, flows that skip steps) and report both, marking which findings come from the linter.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "package": { "type": "integer", "description": "package_id of the checked package (subpackages are included)." },
                    "skip": { "type": "array", "items": { "type": "string" }, "description": "Rules to leave out, e.g. [\"missing_notes\"]. Rules: default_name, missing_notes, orphan, duplicate_name, naming, required_tag, broken_connector, self_relation, duplicate_relation, invalid_relation, empty_package, empty_diagram." },
                    "rules": { "type": "object", "description": "Optionally rules instead of governance.json: {\"naming\": [{\"applies_to\": \"Class\" or \"«stereotype»\", \"pattern\": \"*Service\" (wildcards * ?), \"message\": \"…\"}], \"required_tags\": [{\"applies_to\": \"…\", \"tag\": \"Owner\"}], \"allowed_relations\": [{\"connector_type\": \"Realisation\", \"source\": \"Component\", \"target\": \"Interface\"}]} (a relationship type listed there is allowed only between the listed ends)." }
                },
                "required": ["package"]
            }
        },
        {
            "name": "gap_analysis",
            "description": "Gap analysis between two packages (with subpackages), e.g. as-is vs to-be architecture: matches elements by name and type and returns \
what is new (only in to-be), what is to retire (only in as-is) and what changed (stereotype, description, relationships added or removed). \
Present it as a gap table; on request save it as an Excel workbook (create_spreadsheet) or a document.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "as_is": { "type": "integer", "description": "package_id of the current state (as-is)." },
                    "to_be": { "type": "integer", "description": "package_id of the target state (to-be)." }
                },
                "required": ["as_is", "to_be"]
            }
        },
        {
            "name": "impact_analysis",
            "description": "Change impact analysis: starting from the given element, follows relationships in depth (level by level) and returns all affected elements \
with the level, type, package, the relationship through which the impact arrives and the path, plus the diagrams they appear on. \
direction: out (what the element depends on / affects through outgoing relationships), in (what depends on the element), both (default). \
Describe the result to the user clearly (what is affected and why), and on request turn it into an image, a document or an Excel workbook.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "element": { "type": "integer", "description": "object_id of the starting element (from db_query)." },
                    "direction": { "type": "string", "enum": ["in", "out", "both"] },
                    "depth": { "type": "integer", "description": "Number of levels, 1-6 (default 3)." },
                    "connector_types": { "type": "array", "items": { "type": "string" }, "description": "Optionally only these relationship types, e.g. [\"Realisation\", \"Dependency\"]." }
                },
                "required": ["element"]
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
            ctx.approver.confirm_always(&crate::i18n::fmt("Call the tool {0} of the MCP server {1}:\n{2}", &[tool, server, args.chars().take(1500).collect()]))?;
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
            // Memory goes into every later conversation, so saving it follows the approval mode like a model change.
            ctx.approver.confirm(&crate::i18n::fmt("Save a note to the assistant's memory:\n\"{0}\"\n{1}", &[name.to_string(), shorten_for_dialog(content)]))?;
            let model = if input["only_this_model"] == true { ctx.cfg.model_label() } else { String::new() };
            let file = memory::save(&ctx.cfg.memory_dir, name, input["description"].as_str().unwrap_or(""), content, &model)?;
            let scope = if model.is_empty() { "for all models".to_string() } else { format!("only for the model {model}") };
            Ok(format!("Saved note '{name}' to memory (file {file}), {scope}.").into())
        }
        "memory_delete" => {
            let name = input["name"].as_str().with_context(|| crate::i18n::fmt("missing field '{0}'", &["name".to_string()]))?;
            ctx.approver.confirm(&crate::i18n::fmt("Remove the note \"{0}\" from the assistant's memory", &[name.to_string()]))?;
            memory::delete(&ctx.cfg.memory_dir, name)?;
            Ok(format!("Removed note '{name}' from memory.").into())
        }
        "render_image" => render_image(input, ctx),
        "create_document" => create_document(input, ctx),
        "create_spreadsheet" => create_spreadsheet(input, ctx),
        "create_confluence_page" => create_confluence_page(input, ctx),
        "publish_confluence" => publish_confluence(input, ctx),
        "confluence_pages" => confluence_pages(ctx.cfg).map(Into::into),
        "read_template" => crate::templates::read(&ctx.cfg.templates_dir, input["name"].as_str().with_context(|| crate::i18n::fmt("missing field '{0}'", &["name".to_string()]))?).map(Into::into),
        "save_template" => {
            let name = input["name"].as_str().with_context(|| crate::i18n::fmt("missing field '{0}'", &["name".to_string()]))?;
            let content = input["content"].as_str().with_context(|| crate::i18n::fmt("missing field '{0}'", &["content".to_string()]))?;
            if crate::templates::list(&ctx.cfg.templates_dir).iter().any(|(existing, _)| existing.eq_ignore_ascii_case(name)) {
                ctx.approver.confirm(&crate::i18n::fmt("Overwrite the document template \"{0}\"", &[name.to_string()]))?;
            }
            let file = crate::templates::save(&ctx.cfg.templates_dir, name, content)?;
            Ok(format!("Saved template \"{name}\" (file {file} in the templates folder).").into())
        }
        "impact_analysis" => impact_analysis(input, ctx.cfg).map(Into::into),
        "model_lint" => crate::governance::lint(input, ctx.cfg).map(Into::into),
        "ea_script" => ea_script(input, ctx).map(Into::into),
        "trace_requirements" => trace_requirements(input, ctx).map(Into::into),
        "code_drift" => crate::code_drift::compare(ctx.cfg, std::path::Path::new(input["path"].as_str().unwrap_or("").trim()), input["package"].as_i64()).map(Into::into),
        "spell_check" => crate::model_spell::check(ctx.cfg, input["package"].as_i64(), crate::i18n::current().code()).map(Into::into),
        "jira_search" => jira(ctx.cfg)?.search(input["jql"].as_str().with_context(|| crate::i18n::fmt("missing field '{0}'", &["jql".to_string()]))?, input["max"].as_u64().unwrap_or(30) as usize).map(Into::into),
        "jira_issue" => jira(ctx.cfg)?.issue(input["key"].as_str().with_context(|| crate::i18n::fmt("missing field '{0}'", &["key".to_string()]))?.trim()).map(Into::into),
        "jira_create_issue" => jira_create_issue(input, ctx).map(Into::into),
        "jira_comment" => jira_comment(input, ctx).map(Into::into),
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
/// Requirements of a document (file, Confluence page or text) matched with the model.
fn trace_requirements(input: &Value, ctx: &ToolContext) -> Result<String> {
    let text = if let Some(file) = input["file"].as_str().filter(|f| !f.trim().is_empty()) {
        let path = std::path::Path::new(file.trim());
        let path = if path.is_file() { path.to_path_buf() } else { ctx.cfg.attachments_dir.join(file.trim()) };
        if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("docx")) {
            crate::traceability::docx_text(&path)?
        } else if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("html") || e.eq_ignore_ascii_case("htm")) {
            crate::traceability::html_text(&std::fs::read_to_string(&path)?)
        } else {
            std::fs::read_to_string(&path).with_context(|| crate::i18n::fmt("cannot read {0}", &[path.display().to_string()]))?
        }
    } else if let Some(page) = input["confluence_page"].as_str().filter(|p| !p.trim().is_empty()) {
        if ctx.cfg.confluence_url.trim().is_empty() {
            bail!("{}", crate::i18n::tr("Confluence is not configured in the settings"));
        }
        let token = crate::config::secrets::get(crate::config::secrets::CONFLUENCE_TOKEN).context(crate::i18n::tr("no Confluence token"))?;
        let connection = crate::confluence::Connection { base_url: ctx.cfg.confluence_url.clone(), user: ctx.cfg.confluence_user.clone(), token };
        crate::traceability::html_text(&connection.page_body(page)?.1)
    } else {
        input["text"].as_str().filter(|t| !t.trim().is_empty()).with_context(|| crate::i18n::tr("give 'file', 'confluence_page' or 'text'"))?.to_string()
    };
    let requirements = crate::traceability::extract(&text);
    crate::traceability::trace(ctx.cfg, &requirements, input["package"].as_i64())
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
            let preview: String = code.lines().take(25).collect::<Vec<_>>().join("\n");
            ctx.approver.confirm(&crate::i18n::fmt("Save the EA script {0}.{1} ({2}, {3} lines){4}:\n{5}", &[
                group.clone(), name.clone(), language.clone(), code.lines().count().to_string(),
                if updated { crate::i18n::tr(" - REPLACES the existing script").to_string() } else { String::new() }, preview,
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
            let preview: String = s.code.lines().take(25).collect::<Vec<_>>().join("\n");
            ctx.approver.confirm_always(&crate::i18n::fmt("Run the EA script {0}.{1} ({2}) in the open EA window? A script can change anything in the model.\n{3}", &[s.group.clone(), s.name.clone(), s.language.clone(), preview]))?;
            crate::scripts::run(ctx.cfg, &name)
        }
        other => bail!("{}", crate::i18n::fmt("unknown ea_script action: {0}", &[other.to_string()])),
    }
}

/// Connection to Jira from the settings.
fn jira(cfg: &AppConfig) -> Result<crate::jira::Connection> {
    if cfg.jira_url.trim().is_empty() {
        bail!("{}", crate::i18n::tr("Jira is not configured: enter its address and token in Settings → Jira"));
    }
    // Tests talk to a Jira imitation with a test token (never the user's).
    #[cfg(test)]
    if let Ok(token) = std::env::var("LOONAR_TEST_JIRA_TOKEN") {
        return Ok(crate::jira::Connection { base_url: cfg.jira_url.clone(), user: cfg.jira_user.clone(), token });
    }
    let token = crate::config::secrets::get(crate::config::secrets::JIRA_TOKEN).context(crate::i18n::tr("no Jira token in the settings"))?;
    Ok(crate::jira::Connection { base_url: cfg.jira_url.clone(), user: cfg.jira_user.clone(), token })
}

/// New Jira issue: always with the user's consent (the content leaves the application).
fn jira_create_issue(input: &Value, ctx: &ToolContext) -> Result<String> {
    let connection = jira(ctx.cfg)?;
    let project = input["project"].as_str().map(str::trim).filter(|p| !p.is_empty()).unwrap_or(ctx.cfg.jira_project.trim()).to_string();
    if project.is_empty() {
        bail!("{}", crate::i18n::tr("no Jira project: give 'project' or set the default project in Settings → Jira"));
    }
    let summary = input["summary"].as_str().unwrap_or("").trim().to_string();
    let issue_type = input["type"].as_str().unwrap_or("Task").to_string();
    let description = input["description"].as_str().unwrap_or("").to_string();
    let labels: Vec<String> = input["labels"].as_array().into_iter().flatten().filter_map(|v| v.as_str()).map(String::from).collect();
    let parent = input["parent"].as_str();
    let link = input["link_url"].as_str().filter(|s| !s.trim().is_empty());
    ctx.approver.confirm_always(&crate::i18n::fmt("Create a Jira issue in project {0}:\n{1}: {2}\n\n{3}", &[project.clone(), issue_type.clone(), summary.clone(), shorten_for_approval(&description)]))?;
    let (key, url) = connection.create_issue(&project, &issue_type, &summary, &description, &labels, parent)?;
    if let Some(link) = link {
        connection.add_web_link(&key, link, input["link_title"].as_str().unwrap_or(link))?;
    }
    Ok(crate::i18n::fmt("Created the Jira issue {0}: {1}", &[key, url]))
}

/// Comment or web link on an existing issue: always with the user's consent.
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
    ctx.approver.confirm_always(&crate::i18n::fmt("Add to the Jira issue {0}:\n{1}", &[key.clone(), what.join("\n")]))?;
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
    let publish_hint = if ctx.cfg.confluence_url.trim().is_empty() {
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
    if cfg.confluence_url.trim().is_empty() {
        bail!("{}", crate::i18n::tr("Confluence is not configured: complete Settings → Confluence (address, space, token)"));
    }
    let token = crate::config::secrets::get(crate::config::secrets::CONFLUENCE_TOKEN).context(crate::i18n::tr("no Confluence token in the settings"))?;
    Ok(crate::confluence::Connection { base_url: cfg.confluence_url.clone(), user: cfg.confluence_user.clone(), token })
}

/// Publishes or updates a page in Confluence. Diagrams are exported from EA again, so the page shows the current model.
fn publish_confluence(input: &Value, ctx: &ToolContext) -> Result<ToolOutput> {
    let page = input["page"].as_str().context(crate::i18n::tr("missing field 'page' (the .confluence.xml file from create_confluence_page)"))?;
    let page = std::path::Path::new(page.trim()).file_name().context(crate::i18n::tr("page file name"))?.to_string_lossy().to_string();
    let source_file = ctx.cfg.attachments_dir.join(page.replace(".confluence.xml", ".confluence.md"));
    let source = std::fs::read_to_string(&source_file).with_context(|| crate::i18n::fmt("the page source {0} does not exist", &[source_file.display().to_string()]))?;
    let (title_line, markdown) = source.split_once('\n').unwrap_or(("", &source));
    let saved_title = title_line.trim().trim_start_matches(TITLE_MARKER).trim_start_matches(LEGACY_TITLE_MARKER).trim_end_matches("-->").trim().to_string();
    let title = input["title"].as_str().map(str::to_string).unwrap_or(saved_title);
    let space = input["space"].as_str().map(str::to_string).unwrap_or_else(|| ctx.cfg.confluence_space.clone());
    if space.trim().is_empty() {
        bail!("{}", crate::i18n::tr("specify the Confluence space (space) or set a default one in the settings"));
    }
    let parent = input["parent_id"].as_str().map(str::to_string).or_else(|| input["parent_id"].as_i64().map(|i| i.to_string())).unwrap_or_else(|| ctx.cfg.confluence_parent.clone());
    let page_id = input["page_id"].as_str().map(str::to_string).or_else(|| input["page_id"].as_i64().map(|i| i.to_string()));
    let connection = confluence_connection(ctx.cfg)?;
    // The approval dialog says plainly whether an existing page will be overwritten (also one found by its title).
    let existing = connection.existing_page(&space, &title, page_id.as_deref())?;
    let question = match &existing {
        Some(page) => crate::i18n::fmt(
            "Publishing to Confluence will REPLACE the content of an existing page:\n\"{0}\" (ID {1}, version {2}) in space {3}.\nThe previous version stays in the page history.",
            &[page.title.clone(), page.id.clone(), page.version.to_string(), space.clone()],
        ),
        None => crate::i18n::fmt("Publishing a new page to Confluence:\n\"{0}\" in space {1}", &[title.clone(), space.clone()]),
    };
    ctx.approver.confirm_always(&question)?;
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
    Ok(ToolOutput {
        text: format!(
            "Document \"{file_name}.docx\" is ready ({} KB, images: {pictures}). The user sees it in the chat with the \"Open\" and \"Save as…\" buttons.",
            bytes.len() / 1024
        ),
        images: Vec::new(),
        files: vec![stored],
    })
}

/// Where the operations (services) of a class, component or interface are used: messages on sequence diagrams addressed
/// to the element or to its lifeline (formal link to the operation or a matching name), actions invoking the operation
/// on activity diagrams; with the diagram, package and sender. At the end, the operations nobody uses.
fn operation_usage(input: &Value, cfg: &AppConfig) -> Result<String> {
    let element = input["element"].as_i64().context(crate::i18n::tr("missing field 'element' (object_id of the class)"))?;
    let filter = input["operation"].as_str().unwrap_or("").to_lowercase().replace('\'', "");
    let owner = query_rows(cfg, &format!("select o.name, o.object_type from t_object o where o.object_id = {element}"))?;
    let owner = owner.first().with_context(|| crate::i18n::fmt("there is no element with id {0}", &[element.to_string()]))?;
    let ops = query_rows(cfg, &format!(
        "select operationid, name, ea_guid from t_operation where object_id = {element} order by pos, name"
    ))?;
    let ops: Vec<Vec<String>> = ops.into_iter().filter(|o| filter.is_empty() || o[1].to_lowercase().contains(&filter)).collect();
    if ops.is_empty() {
        return Ok(format!("Element \"{}\" has no operations{}.", owner[0], if filter.is_empty() { String::new() } else { format!(" matching \"{filter}\"") }));
    }
    // Message recipients: the element itself and the lifelines (objects) whose classifier is this element.
    let messages = query_rows(cfg, &format!(
        "select c.connector_id, coalesce(c.name,''), d.name, p.name, s.name, coalesce(c.styleex,'') || coalesce(c.pdata1,'') || coalesce(c.pdata2,'') || coalesce(c.pdata3,'') || coalesce(c.pdata5,'') \
         from t_connector c join t_diagram d on d.diagram_id = c.diagramid join t_package p on p.package_id = d.package_id \
         join t_object s on s.object_id = c.start_object_id \
         where c.connector_type = 'Sequence' and (c.end_object_id = {element} or c.end_object_id in (select object_id from t_object where classifier = {element}))"
    ))?;
    let normalize = |name: &str| name.split('(').next().unwrap_or("").trim().to_lowercase();
    let mut out = format!("Operation usage of element \"{}\" ({}):\n", owner[0], owner[1]);
    let mut unused = Vec::new();
    for op in &ops {
        let (name, guid) = (&op[1], &op[2]);
        let mut hits: Vec<String> = messages
            .iter()
            .filter(|m| m[5].contains(guid.as_str()) || normalize(&m[1]) == normalize(name))
            .map(|m| format!("  - diagram \"{}\" (package \"{}\"): message \"{}\" from \"{}\"{}", m[2], m[3], m[1], m[4], if m[5].contains(guid.as_str()) { " [linked to the operation]" } else { "" }))
            .collect();
        // Actions on activity diagrams that invoke this operation (action classifier = operation).
        let actions = query_rows(cfg, &format!(
            "select a.name, d.name, p.name from t_object a join t_diagramobjects x on x.object_id = a.object_id \
             join t_diagram d on d.diagram_id = x.diagram_id join t_package p on p.package_id = d.package_id \
             where a.classifier_guid = '{}'",
            guid.replace('\'', "")
        ))?;
        hits.extend(actions.iter().map(|a| format!("  - diagram \"{}\" (package \"{}\"): action \"{}\" invokes the operation", a[1], a[2], a[0])));
        hits.sort();
        hits.dedup();
        if hits.is_empty() {
            unused.push(name.clone());
        } else {
            out.push_str(&format!("{name} — used {} time(s):\n{}\n", hits.len(), hits.join("\n")));
        }
    }
    if !unused.is_empty() {
        out.push_str(&format!("Not used on any diagram ({}): {}\n", unused.len(), unused.join(", ")));
    }
    out.push_str("Note: messages without a formal link to the operation are matched by name (without parameters) and recipient.");
    Ok(out)
}

/// Impact analysis: elements reachable from the starting element through relationships (in depth, level by level),
/// with the relationship through which the impact arrives, the path and the diagrams they appear on.
pub(crate) fn impact_analysis(input: &Value, cfg: &AppConfig) -> Result<String> {
    let start = input["element"].as_i64().context(crate::i18n::tr("missing field 'element' (object_id)"))?;
    let depth = input["depth"].as_i64().unwrap_or(3).clamp(1, 6);
    let direction = input["direction"].as_str().unwrap_or("both");
    let types: Vec<String> = input["connector_types"]
        .as_array()
        .map(|a| a.iter().filter_map(|v| v.as_str()).map(|s| s.replace('\'', "")).collect())
        .unwrap_or_default();
    let type_filter = if types.is_empty() {
        String::new()
    } else {
        format!(" and c.connector_type in ({})", types.iter().map(|t| format!("'{t}'")).collect::<Vec<_>>().join(","))
    };
    let name_of = |id: i64| -> Result<Vec<String>> {
        query_rows(cfg, &format!(
            "select o.name, o.object_type, coalesce(o.stereotype,''), p.name from t_object o join t_package p on p.package_id = o.package_id where o.object_id = {id}"
        ))?
        .into_iter()
        .next()
        .with_context(|| crate::i18n::fmt("there is no element with id {0}", &[start.to_string()]))
    };
    let root = name_of(start)?;
    // id -> (level, path of names)
    let mut seen: std::collections::HashMap<i64, (i64, String)> = std::collections::HashMap::new();
    seen.insert(start, (0, root[0].clone()));
    let mut frontier = vec![start];
    let mut lines = Vec::new();
    for level in 1..=depth {
        if frontier.is_empty() {
            break;
        }
        let ids = frontier.iter().map(|i| i.to_string()).collect::<Vec<_>>().join(",");
        let mut parts = Vec::new();
        if direction != "in" {
            parts.push(format!(
                "select c.start_object_id, c.end_object_id, c.connector_type, coalesce(c.stereotype,''), coalesce(c.name,''), 'outgoing' \
                 from t_connector c where c.start_object_id in ({ids}){type_filter}"
            ));
        }
        if direction != "out" {
            parts.push(format!(
                "select c.end_object_id, c.start_object_id, c.connector_type, coalesce(c.stereotype,''), coalesce(c.name,''), 'incoming' \
                 from t_connector c where c.end_object_id in ({ids}){type_filter}"
            ));
        }
        let rows = query_rows(cfg, &parts.join(" union all "))?;
        let mut next = Vec::new();
        for r in rows {
            let (from, to): (i64, i64) = (r[0].parse().unwrap_or(0), r[1].parse().unwrap_or(0));
            if to == 0 || seen.contains_key(&to) {
                continue;
            }
            let Ok(info) = name_of(to) else { continue };
            let path = format!("{} → {}", seen.get(&from).map(|s| s.1.clone()).unwrap_or_default(), info[0]);
            let via = if r[3].is_empty() { r[2].clone() } else { format!("{} «{}»", r[2], r[3]) };
            lines.push(format!("{level} | {} | {} | {} | {} | {via} {} | {path}", to, info[0], if info[2].is_empty() { info[1].clone() } else { format!("{} «{}»", info[1], info[2]) }, info[3], r[5]));
            seen.insert(to, (level, path));
            next.push(to);
            if seen.len() > 400 {
                break;
            }
        }
        frontier = next;
    }
    let impacted: Vec<String> = seen.keys().filter(|id| **id != start).map(|i| i.to_string()).collect();
    let mut out = format!(
        "Impact analysis for \"{}\" ({} in package \"{}\"), direction: {direction}, depth: {depth}. Affected elements: {}.\n",
        root[0], root[1], root[3], impacted.len()
    );
    if impacted.is_empty() {
        out.push_str("No related elements.");
        return Ok(out);
    }
    out.push_str("level | object_id | name | type | package | relationship | path\n");
    out.push_str(&lines.join("\n"));
    let diagrams = query_rows(cfg, &format!(
        "select d.name, count(distinct o.object_id) from t_diagramobjects o join t_diagram d on d.diagram_id = o.diagram_id \
         where o.object_id in ({}) group by d.name order by 2 desc limit 30",
        impacted.join(",")
    ))?;
    if !diagrams.is_empty() {
        out.push_str("\n\nDiagrams with affected elements (name | number of elements):\n");
        out.push_str(&diagrams.iter().map(|r| format!("{} | {}", r[0], r[1])).collect::<Vec<_>>().join("\n"));
    }
    if seen.len() > 400 {
        out.push_str("\n\nResult truncated to 400 elements: narrow down the direction, the depth or the relationship types.");
    }
    Ok(out)
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
        text: format!("Workbook \"{file_name}.xlsx\" is ready (sheets: {}, rows: {rows}). The user sees it in the chat with the \"Open\" and \"Save as…\" buttons.", sheets.len()),
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

fn ea_write(input: &Value, ctx: &ToolContext) -> Result<String> {
    let ops = input["ops"].as_array().with_context(|| crate::i18n::fmt("missing list '{0}'", &["ops".to_string()]))?;
    for (i, op) in ops.iter().enumerate() {
        let name = op["op"].as_str().unwrap_or("");
        if !ea::MODEL_OPS.contains(&name) {
            bail!("{}", crate::i18n::fmt("operation {0} ('{1}') is not allowed. Allowed: {2}", &[(i + 1).to_string(), name.to_string(), ea::MODEL_OPS.join(", ")]));
        }
    }
    check_unchanged(ctx.cfg, &input["expect_unchanged"])?;
    let summary = crate::i18n::fmt("Saving to the EA model:\n{0}\n\nBefore → after:\n{1}", &[ea::describe(ops), crate::governance::preview(ctx.cfg, ops)]);
    let destructive = destructive_details(ctx.cfg, ops);
    // Deleting an element that has relationships always asks (also in auto), with the relationships listed.
    let related = crate::verify::relations_of_deleted(ctx.cfg, ops);
    if !related.is_empty() {
        ctx.approver.confirm_risky(&format!("{summary}\n\n{}\n\n{}", destructive.join("\n"), related.join("\n")))?;
    } else if destructive.is_empty() {
        ctx.approver.confirm(&summary)?;
    } else {
        ctx.approver.confirm_destructive(&format!("{summary}\n\n{}", destructive.join("\n")))?;
    }

    let result = ea::run_batch(ctx.cfg, ops, &BatchOptions::default());
    audit(ctx.cfg, ops, &result);
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
    if check.problems.is_empty() {
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

/// Connection to the model database, encrypted with TLS according to `db.sslmode`:
/// `prefer` (default: TLS when the server offers it), `require` (TLS or no connection) and `verify-full`
/// (TLS with the server certificate checked against the Windows certificate store, including the host name).
/// `prefer` and `require` do not verify the certificate, exactly like libpq's modes of the same names.
pub fn connect(cfg: &AppConfig) -> Result<postgres::Client> {
    let mut pg = postgres::Config::new();
    pg.host(&cfg.db_host)
        .port(cfg.db_port)
        .dbname(&cfg.db_name)
        .user(&cfg.db_user)
        .password(&cfg.db_password)
        .application_name(crate::config::APP_NAME)
        .connect_timeout(Duration::from_secs(10));
    let verify = cfg.db_sslmode == crate::config::SslMode::VerifyFull;
    pg.ssl_mode(if cfg.db_sslmode == crate::config::SslMode::Prefer { postgres::config::SslMode::Prefer } else { postgres::config::SslMode::Require });
    let tls = native_tls::TlsConnector::builder()
        .danger_accept_invalid_certs(!verify)
        .danger_accept_invalid_hostnames(!verify)
        .build()
        .context(crate::i18n::tr("TLS for the database connection"))?;
    let mut client = pg
        .connect(postgres_native_tls::MakeTlsConnector::new(tls))
        .context(crate::i18n::tr("connecting to the model database"))?;
    // Read-only session (even a faulty query cannot change the data) with a time limit per query.
    client.batch_execute("SET SESSION CHARACTERISTICS AS TRANSACTION READ ONLY; SET statement_timeout = '60s'")?;
    Ok(client)
}

/// Time limit of a query on an EA file (the same as `statement_timeout` on PostgreSQL).
const FILE_QUERY_LIMIT: Duration = Duration::from_secs(60);

/// Opens the SQLite database of an EA project file (`.qea`/`.qeax`) read-only: the file cannot be changed through
/// this connection, and a query that runs longer than the limit is stopped.
pub fn open_model_file(path: &std::path::Path) -> Result<rusqlite::Connection> {
    use rusqlite::OpenFlags;
    if !crate::config::is_model_file(path) {
        bail!("{}", crate::i18n::tr("the model file must be an Enterprise Architect project file .qea or .qeax"));
    }
    if !path.is_file() {
        bail!("{}", crate::i18n::fmt("file {0} does not exist", &[path.display().to_string()]));
    }
    let conn = rusqlite::Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX)
        .context(crate::i18n::tr("opening the model file"))?;
    // EA may be writing to the file at the same moment: wait for its write to finish instead of failing at once.
    conn.busy_timeout(Duration::from_secs(10))?;
    conn.execute_batch("PRAGMA query_only = ON")?;
    let started = std::time::Instant::now();
    conn.progress_handler(10_000, Some(move || started.elapsed() > FILE_QUERY_LIMIT));
    Ok(conn)
}

/// Server functions that read files, wait, or act on other sessions or the server; read-only mode does not stop them,
/// so queries that use them are refused. `load_extension` is the SQLite one (EA file models).
const FORBIDDEN_FUNCTIONS: &[&str] = &[
    "pg_read_file", "pg_read_binary_file", "pg_ls_", "pg_stat_file", "lo_import", "lo_export", "lo_get", "lo_open",
    "dblink", "pg_terminate_backend", "pg_cancel_backend", "pg_sleep", "set_config", "pg_reload_conf",
    "pg_rotate_logfile", "pg_promote", "pg_switch_wal", "pg_create_restore_point", "pg_logical_emit_message",
    "pg_file_", "pg_advisory", "txid_current", "nextval", "setval", "load_extension",
];

/// Result of a read-only query: column names and rows (`None` = NULL). `truncated`: there were more rows than the limit.
struct Selected {
    columns: Vec<String>,
    rows: Vec<Vec<Option<String>>>,
    truncated: bool,
}

/// Runs a read-only query on the model (database or file) and returns at most `limit` rows.
fn select(cfg: &AppConfig, sql: &str, limit: usize) -> Result<Selected> {
    let mut out = Selected { columns: Vec::new(), rows: Vec::new(), truncated: false };
    // Compass in a window of EA: a database model is read through EA's own connection (no settings needed).
    if !cfg.uses_file() && crate::ea_ui::reads_through_ea() {
        let (columns, rows, truncated) = crate::ea_ui::sql_query(sql, limit)?;
        return Ok(Selected { columns, rows, truncated });
    }
    // An EA file open in an EA window: EA writes it to disk only when it closes the file, so the current model is
    // read through that window (the file on disk may not have the latest changes yet).
    if cfg.uses_file() && crate::ea_ui::file_open_in_window(cfg) {
        let (columns, rows, truncated) = crate::ea_ui::sql_query(sql, limit)?;
        return Ok(Selected { columns, rows, truncated });
    }
    if cfg.uses_file() {
        let conn = open_model_file(&cfg.model_file)?;
        let mut run = || -> rusqlite::Result<()> {
            let mut stmt = conn.prepare(sql)?;
            out.columns = stmt.column_names().iter().map(|c| c.to_string()).collect();
            let width = stmt.column_count();
            let mut rows = stmt.query([])?;
            while let Some(row) = rows.next()? {
                if out.rows.len() == limit {
                    out.truncated = true;
                    break;
                }
                out.rows.push((0..width).map(|i| row.get_ref(i).map(sqlite_text)).collect::<rusqlite::Result<_>>()?);
            }
            Ok(())
        };
        if let Err(e) = run() {
            if matches!(&e, rusqlite::Error::SqliteFailure(f, _) if f.code == rusqlite::ErrorCode::OperationInterrupted) {
                bail!("{}", crate::i18n::fmt("the query took longer than {0} s and was stopped", &[FILE_QUERY_LIMIT.as_secs().to_string()]));
            }
            return Err(anyhow::Error::new(e).context(crate::i18n::tr("running the query")));
        }
        return Ok(out);
    }
    let mut client = connect(cfg)?;
    for msg in client.simple_query(sql).context(crate::i18n::tr("running the query"))? {
        if let SimpleQueryMessage::Row(row) = msg {
            if out.columns.is_empty() {
                out.columns = row.columns().iter().map(|c| c.name().to_string()).collect();
            }
            if out.rows.len() == limit {
                out.truncated = true;
                break;
            }
            out.rows.push((0..row.len()).map(|i| row.get(i).map(str::to_string)).collect());
        }
    }
    Ok(out)
}

/// A SQLite value as text (the same way PostgreSQL returns it in a simple query).
fn sqlite_text(value: rusqlite::types::ValueRef) -> Option<String> {
    use rusqlite::types::ValueRef;
    match value {
        ValueRef::Null => None,
        ValueRef::Integer(n) => Some(n.to_string()),
        ValueRef::Real(x) => Some(x.to_string()),
        ValueRef::Text(s) => Some(String::from_utf8_lossy(s).into_owned()),
        ValueRef::Blob(b) => Some(format!("<binary data, {} bytes>", b.len())),
    }
}

/// Read-only query; rows as strings (NULL as an empty string).
pub fn query_rows(cfg: &AppConfig, sql: &str) -> Result<Vec<Vec<String>>> {
    let selected = select(cfg, sql, usize::MAX)?;
    Ok(selected.rows.into_iter().map(|row| row.into_iter().map(Option::unwrap_or_default).collect()).collect())
}

fn db_query(sql: &str, cfg: &AppConfig) -> Result<String> {
    let s = sql.trim().trim_end_matches(';').trim();
    let lower = s.to_ascii_lowercase();
    if !(lower.starts_with("select") || lower.starts_with("with")) {
        bail!("{}", crate::i18n::tr("only SELECT or WITH queries are allowed"));
    }
    if s.contains(';') {
        bail!("{}", crate::i18n::tr("one query at a time (no semicolons inside)"));
    }
    if let Some(f) = FORBIDDEN_FUNCTIONS.iter().find(|f| lower.contains(*f)) {
        bail!("{}", crate::i18n::fmt("the query uses a server function that is not allowed ({0}); only reading the model is allowed", &[f.trim_end_matches('_').to_string()]));
    }

    let selected = select(cfg, s, MAX_ROWS)?;
    if selected.rows.is_empty() {
        return Ok("(no rows)".into());
    }
    let mut out = selected.columns.join(" | ");
    out.push('\n');
    for row in &selected.rows {
        let cols: Vec<&str> = row.iter().map(|v| v.as_deref().unwrap_or("NULL")).collect();
        out.push_str(&cols.join(" | "));
        out.push('\n');
    }
    if selected.truncated {
        out.push_str(&format!("...(truncated to {MAX_ROWS} rows)\n"));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn test_config() -> AppConfig {
        AppConfig::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("app.properties")).expect("app.properties")
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
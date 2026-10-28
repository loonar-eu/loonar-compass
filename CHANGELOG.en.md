# Changelog

All notable changes to Loonar Compass (named Loonar EA Assistant until version 0.3).

## Version 0.9.1 (2026-10-27)

- **Jira and Confluence with a client certificate:** company servers that ask for a client certificate (mTLS) besides the token now work with Compass. On the Jira and Confluence cards in Settings there are the fields “Client certificate” (path to a .p12/.pfx or PEM file) and “Certificate password” (kept in Windows Credential Manager). Without the certificate such a server closed the connection (“An existing connection was forcibly closed by the remote host”).
- **Simpler installer:** no “Configuration” page – you enter the key, the model and the EA account in the application, which guides you through the settings at its first start.
- **Retrying the installation:** when the installation cannot replace the files because Compass is running, the error page has the buttons “Close Compass and try again” and “Try again” – no need to start over.
- **Jira and Confluence addresses pasted from the browser:** the part of a page (e.g. “/spaces/…”, “/display/…”, “/browse/…”) is cut off, and when the server sends a web page instead of data, Compass says to check the address (before, the check said “Connected” and pages came back empty).
- **The assistant reads source code:** new read-only tools – the files of a folder, a search in the code (file:line) and reading a file – in local repository clones. Folders listed in Settings → “Source code” (a card also shown in EA; the “…” button adds a folder) are read without asking; for others it asks first. So it finds by itself where a class, operation or table is implemented and what depends on it.
- **OneDrive and Google Drive:** the assistant lists folders, reads documents (Word, Excel, PowerPoint, PDF, Visio, text; on Google Drive also Google Docs, Sheets and Slides) and, each time with your consent, uploads documents it made. You sign in with your own Microsoft or Google account in the browser once (OAuth with PKCE); the refresh token is kept in Windows Credential Manager. Settings: the “OneDrive” card (an app registration in Entra ID as a public client) and the “Google Drive” card (an OAuth client “Desktop app” in Google Cloud).
- **New version and statistics every hour:** Compass checks for a new version every hour (also while it stays open), and the statistics – when on – are sent at the start (installation id, version, system, language) and every hour (a summary of the day: kinds of tasks and how they ended, ratings of answers, what the assistant could not do). Under answers: 👍/👎 and – when the assistant cannot do something – “💡 Request this feature”.
- **A conversation in another model, export and import:** after moving a project (e.g. from a database to a file) a conversation can go on with the new model. In the EA tab: “⋯” → “Conversations of other models…” (also a link under the list) → “Continue here”. A copy is made, the original stays with its own model, and the memory notes of the earlier model can be pinned to the new one. The assistant is told that the earlier messages were about another model and checks the elements mentioned first. In the separate window the model of a conversation changes as before with the “Model: …” button. A conversation with its attachments can be exported to a `.lcchat` file (the “⋯” menu in EA, right click on a conversation in the window) and imported on another computer – it goes to the model open in that window. The file holds the file path or the database and server name, no accounts or passwords.
- **The assistant remembers what it did:** the steps of every answer (reads, searches, saves in EA, Confluence pages) stay in the conversation and the assistant gets them in later turns. Before, the history held only the text of answers, so when asked later it could deny that it had, e.g., read Confluence although it had.
- **The assistant works to the end on its own:** it does the whole task in one answer instead of showing the state and asking “shall I go on?” every few steps. Sensible choices (names, files, format, order) it makes itself and names in the answer; it asks only when a wrong decision would change much in the model. Notes like “now I read…” are in the steps of the work, not separate messages in the chat. The limit is 40 steps, and when it is reached the assistant says what is done and what is left instead of ending with an error. When it still stops before the work is done (open tasks in its plan, or “write go on”), Compass sends it on by itself, up to 3 times in one answer.
- **Fewer questions about EA scripts:** after the first approval, further scripts of the same build – the same code with other data, e.g. the next batch of aliases – are saved and run without asking (until Compass is closed); other code asks as before. The assistant now sets the alias and stereotype of operations and attributes directly when writing, so aliases (e.g. REST paths) need no script. The approval card of a script also has the button “Approve all scripts until Compass is closed”, and changes that an ordinary write can make the assistant makes without scripts.
- **One approval for a series:** the approval card of a Jira issue, Confluence publishing, a memory note, an MCP tool call, an upload and an EA script has the button “Approve all … until Compass is closed”. Ten Jira issues are one click instead of ten.
- **Stories and use cases under an epic on Jira Server / Data Center:** an issue with an epic as its parent is joined through the “Epic Link” field (`parent` is only for sub-tasks there). Before, Jira rejected it with “Issue type … is not a sub-task but a parent is specified”. Jira Cloud works as before.

## Version 0.9 (2026-10-27)

Company sign-in to the hub, models on SQL Server, MySQL and Oracle, the team's modelling instructions in the repository, documents in PDF, PowerPoint, Visio and Excel, presentations, SharePoint, guards that stop deletions, and a more careful assistant: questions, a plan, sub-agents and protection against hidden instructions.

### Compass Hub

- **Company sign-in (OpenID Connect):** AD FS, Microsoft Entra ID, Keycloak, Okta and other providers, configured in the `"oidc"` section of `hub.json`. The web view shows "Sign in with your company account".
  - Authorization code with PKCE; the hub checks the ID token itself (RS256 signature against the provider's keys, issuer, audience, expiry, nonce).
  - Roles from groups (`roles`, by default Compass-Admins, Compass-Architects, Compass-Viewers): the highest role wins, no group means no access. A local user with the same name is refused.
  - The client secret is the hub secret `OIDC_CLIENT_SECRET`. `"password_login": false` leaves only the company sign-in; API tokens still work. Every sign-in goes to the audit trail.
- **Hub behind IIS/ARR and sign-in audit:** `allatclaims` added automatically for AD FS; the lockout after wrong passwords uses the client address from `X-Forwarded-For` (only from a proxy on the same computer); failed and refused sign-ins in the "Audit trail" with ✗; the "Users" tab lists company accounts with role and last sign-in (read only); the Windows service stops and restarts cleanly. Guide: the ARR setting `reverseRewriteHostInResponseHeaders="false"`.
- **Event agents for the team:** the policy key `"event_agents"` (the same format as `event_agents.json`, guards too). The agents work in everybody's Compass in EA, marked "(team)".
- **"Team policy" tab (admin):** a JSON editor of the policy (min_approval, governance, event_agents, ask_min_role, ask_per_hour).
- **Word and Excel reports:** "Word report of the package" and "Excel report of the package" in the Model check tab: description, elements, relationships (no diagram pictures). Every download is audited.
- **GraphQL and Power BI:** read-only `POST /api/sources/{id}/graphql` (packages, elements with attributes, operations, tagged values and relationships, diagrams; schema at `GET /api/graphql/schema`) and flat tables `GET /api/sources/{id}/table/{elements|connectors|packages|diagrams|attributes|tagged_values}` for Power BI (Get Data → Web → Advanced, header `Authorization: Bearer <API token>`). Any role.

### Models on SQL Server, MySQL and Oracle

- **New repositories:** besides PostgreSQL and .qea files, Compass reads EA repositories on SQL Server, MySQL/MariaDB and Oracle: in the separate window, in the hub and in check mode. Settings → Model has the choices SQL Server, MySQL and Oracle.
- **ODBC driver:** the vendor's 64-bit ODBC driver is found automatically (SQL Server: ODBC Driver 18 or 17, or Windows' own "SQL Server" driver). A full connection string without the password can be given in `db.odbc` (for example for Oracle).
- **Read only:** every read is rolled back.
- **Saving through EA** (the EA edition must support DBMS repositories):
  - SQL Server: Compass builds EA's connection by itself (OLE DB provider SQLOLEDB), as for PostgreSQL.
  - MySQL and Oracle: EA's connection comes from the new setting `ea.conn` in app.properties, the text of an EA connection shortcut through an ODBC data source (e.g. `ea_lab --- DBType=0;Connect=Provider=MSDASQL.1;Data Source=<ODBC DSN>;…`). Without it, changes go through Compass in EA, which works with any repository open in EA, as before.
- **Docker:** the hub image has unixODBC; add the vendor's Linux ODBC driver in an image built on it.

### Modelling instructions in the repository

- **The team's rulebook in EA:** an Artifact «CompassInstructions» ("Loonar Compass – modelling instructions") whose notes are the rules of the repository (naming, types and stereotypes, packages, diagrams, tagged values, checks). The assistant follows them in every answer, for everybody who opens the repository.
- **Starting wizard:** the assistant analyzes the repository and drafts the rules; saving and adding a rule are changes of the model shown for approval.
  - Example: "Prepare modelling instructions for this repository".
  - Example: "Add to the instructions: activities start with a verb".

### Conversation

- **Clarifying questions as a form:** when something important is unclear, the assistant shows "Questions to you" with choices or a short answer ("Something else…"); "Send answers" sends them as the next message ("My answers:").
- **Plan of larger work:** "📋 Plan: 1 of 4 done" with ✓ ▶ ○ in the steps (and in EA's System Output), updated as the assistant works.
- **Sub-agents:** for larger analyses the assistant hands a task to a sub-agent with the role impact, review or document, which reads the model in its own context and returns a compact result ("Sub-agent (impact): …").
- **Pick from the model:** the ⌖ button next to Paste searches elements, packages and diagrams by name (empty = top packages) and adds them to the message as references, such as «Element: Order (Class, id 26)».
- **The assistant's reasoning:** the 💭 button next to Paste shows a summary of the reasoning as the step "💭 The assistant's reasoning" (Claude only; the effort stays in Settings).
- **Code in Anthropic's sandbox:** the ∑ button next to Paste (off by default; Claude only) lets the assistant run code for calculations and data analysis; its output is the step "Code in the sandbox". The data passed there goes to Anthropic, like the conversation.
- **Web (🌐):** a button next to ∑ (off by default). With Claude, web search and page reading through Anthropic; with OpenAI, Compass reads only pages whose addresses were given in the conversation (never addresses of this computer or the company network). Allowed domains: `claude.web_domains`. Hub policy: `"web_access": false` switches it off for everybody, `"web_allowed_domains"` limits the domains. Searches and pages are shown in the steps of the answer.
- **New version notice:** once a day Compass checks the latest release on GitHub and shows a bar "What's new and download / Not now" (in EA: a line in System Output). Nothing is downloaded or installed by itself, no installation identifier. Switch off: Settings → New releases (newsletter) or `app.update_check=false`; hub policy `"update_check": false`.
- **Computer policy (Group Policy, Intune) and company sign-in:** the key `HKLM/HKCU\SOFTWARE\Policies\Loonar\Compass` – `HubUrl` (hub address locked), `RequireHub` (the assistant only with the hub and its policy; if the hub is down, the last policy up to a week old), `WebAccess`, `UpdateCheck`, `MinApproval`, `AllowedProviders`. ADMX/ADML templates in `deploy\policies`. "Sign in with your company account" in Settings → Team: the hub's sign-in in the browser, the session comes back to 127.0.0.1 and goes to Credential Manager.
- **AI keys, Jira and Confluence on the hub:** the `/ai/v1/messages` gateway – the hub keeps the Claude keys, per person (name or AD UPN), role or everybody, with daily and monthly token limits and a model list; usage per person in the Users tab. Jira and Confluence through the hub with the team's account (a viewer only reads, changes in the audit trail). Policy: `ai_via_hub`, `jira_via_hub`, `confluence_via_hub`.
- **A guard without a decision blocks:** when a guard does not decide within 40 s or cannot (no AI, the hub down), the deletion is stopped with a message (before, it went ahead).
- **Quiet installation:** `LoonarCompass-Setup --quiet [--all-users] [--dir …] [--lang …] [--no-desktop] [--no-start-menu] [--no-ea-addin]` – deployment with Intune, Configuration Manager or a script; for all users the EA add-in works for everybody.
- **Guard of deletions in EA:** EA no longer freezes while the guard decides, and the guard really decides (before, the deletion went ahead after 40 s without a decision).
- **Reading Confluence pages:** the assistant reads a page by title or id, also through the hub.
- **Hub installation guide for IT** (English and Polish).
- **Compass in EA from the start:** the Compass window is visible as soon as EA starts; without a project it shows "No project is open…", Compass starts by itself when a file or repository is opened and returns to that note when the project is closed.
- **Hub restart:** sessions of signed-in people survive a restart of the hub; the same invalid token counts once towards the address lockout; Compass asks to sign in again at once when the hub sign-in has expired and describes hub errors briefly (no IIS HTML pages).
- **EA project glossary:** the default type of a new term is `Glossary` (before: `Slownik`).
- **Long conversations:** above about 100 000 tokens, the earlier part is replaced by a summary for the next answers ("The conversation is long: summarising its earlier part"); the newest messages stay.
- **Deleting a conversation:** right-click a conversation → "Delete the conversation" (in Compass in EA: the 🗑 button next to the conversation name). It is moved to `history\bin` and can be brought back; not while the assistant works in it.

### Documents, presentations and SharePoint

- **Reading documents:** the tool `read_document` and attachments read PDF (every page; attach a scan without a text layer so the model reads the page images), PowerPoint .pptx (slides with notes), Visio .vsdx (pages, shapes with text, connections A → B), Excel .xlsx (sheets, cells) and Word. Old .doc, .ppt, .xls and .vsd files must be saved in the new format.
  - Requirements for `trace_requirements` and `--requirements` can come from these formats.
- **PowerPoint presentations:** `create_presentation` makes a 16:9 deck: a title slide, slides with bullets on two levels and/or a diagram or image, optionally also as PDF (through Microsoft PowerPoint).
- **PDF documents:** `create_document` with the format pdf or both (through Microsoft Word). PDF needs Microsoft Office on the computer.
- **SharePoint:** the new card Settings → SharePoint (site address, document library, tenant ID, application (client) ID, client secret; "Save and test SharePoint connection"). It needs an app registration in Entra ID with the Microsoft Graph application permission Sites.Selected (granted to the site) or Sites.ReadWrite.All.
  - The assistant lists a folder, reads a document's text and uploads a file it has made; every upload is asked.

### Enterprise Architect

- **Event agents for 18 events:** created, changed and deleting of elements, connectors, diagrams and packages; created and deleting of attributes, methods and glossary terms.
- **Guards:** `"mode": "guard"` on a `*_deleting` event. EA asks Compass before deleting; the guard answers ALLOW or BLOCK with a reason, and EA shows "Loonar Compass stopped this deletion:" and cancels. Without an answer within 40 s the deletion goes ahead.
  - Example: `{"name": "Keep approved", "event": "element_deleting", "mode": "guard", "applies_to": "Requirement", "prompt": "Block deleting a requirement whose status is Approved."}`.
- **MDG technologies:** the tool `mdg_technologies`: the profiles the model uses (with counts), EA's and your technology files, the technologies imported into the model, stereotypes with metatype and what they apply to, and their tagged values with type, values and default.
- **The EA API:** the tool `ea_api` reads the automation interface from EA.tlb of the installed EA (classes, properties read or read/write, methods with parameters, enumerations); the assistant uses it before writing EA scripts.
- **Version control of packages:** the tool `version_control_status` (packages under version control, configuration, file, who has them checked out) and the model operation `version_control` (check_out, check_in, get_latest, put_latest, resync, with a comment). Always asked; the package must already be under version control in EA.

### Security and sign-in

- **Protection against hidden instructions (prompt injection):** safety rules in every prompt; results of documents, Jira, Confluence, MCP, the CMDB and code are marked as data; an answer that repeats the internal instructions word for word is withheld.
- **MCP servers over HTTP:** `"auth": {"type": "basic", "user", "password"}` or `{"type": "oauth2", "token_url", "client_id", "client_secret", "scope"}` in `mcp_servers.json` (secrets as `${secret:mcp_…}`).
- **The AI service with OAuth2:** new fields on the AI model card: "OAuth2 token address" (optional, instead of an API key), then "OAuth2 client ID", "OAuth2 scope" and "OAuth2 client secret" (kept in Credential Manager). For example Entra ID with the scope `https://cognitiveservices.azure.com/.default`. Saved as `ai.oauth.*`.
- **Claude through another address:** the field "Claude address" (Claude only; empty = Anthropic), for example Azure AI Foundry `https://<resource>.services.ai.azure.com/anthropic` or a company gateway. Saved as `claude.url`.

## Version 0.8.2 (2026-10-23)

Compass Hub as a program of its own (Windows, Linux, Docker) with user accounts, the model check in CI pipelines, ArchiMate rules, the full traceability chain, comparison with a CMDB and ready-made MCP servers.

### Compass Hub as its own program

- **Standalone `compass-hub`:** a small program for Windows and Linux, with no window and no EA. Run it with `compass-hub hub.json`. `LoonarCompass.exe --hub` still works.
- **Windows service:** `compass-hub --service-install C:\Hub\hub.json` (as administrator) makes the hub start with Windows; `--service-uninstall` removes the service.
- **Docker on Linux:** the `compass-hub` image (`hub/Dockerfile`) keeps its data and `hub.json` in the `/data` volume and runs as a normal user.
  - Example: `docker run -d -p 8730:8730 -v /srv/compass-hub:/data -e LOONAR_SECRET_CLAUDE_API_KEY=… compass-hub`.
- **Hub secrets:** read from environment variables `LOONAR_SECRET_<NAME>` or files `LOONAR_SECRET_<NAME>_FILE` (e.g. `CLAUDE_API_KEY`, `JIRA_TOKEN`, `CONFLUENCE_TOKEN`, `DB_PASSWORD`).

### Accounts and sign-in in the hub

- **Sign-in screen:** a new screen with user name and password. Passwords are stored only as PBKDF2 hashes, and a session lasts 12 hours. Signing in with a token is still possible.
- **Default account:** a new hub without users has the account **admin / passw0rd** (with a zero). Its password must be changed at the first sign-in; until then the account can do nothing else.
- **"Users" tab (admin):**
  - add a user, with a first password the user must change;
  - change a user's role or reset their password;
  - create a new API token for Compass and MCP clients (shown once);
  - remove a user (one admin always remains).
- **Lockout:** after 20 wrong passwords from one address, the hub refuses that address for 10 minutes.

### Jira and Confluence from the hub

- **Service account:** the administrator adds an `"integrations"` section to `hub.json` (Jira: address and project; Confluence: address and space) and gives the tokens as hub secrets.
- **Buttons for architects in the web view:**
  - "Create Jira issue" on an element and on a change set;
  - "Publish this package to Confluence" in a package's model check. The page is text only (elements with descriptions and relationships), with no diagram pictures, because the hub has no EA.
- **Audit and Ask:** every action goes to the audit trail. Ask still only reads the model.

### The model check in CI pipelines

- **Check mode:** `LoonarCompass.exe --check --model model.qea [--code src] [--requirements BRD.docx] [--cmdb cmdb.csv] [--report report.md] [--junit junit.xml] [--fail-on error|warning]`.
- **Output:** a Markdown report, a JUnit file for GitLab, GitHub Actions and Azure DevOps, and exit code 0 (OK), 1 (findings) or 2 (error). No AI and no EA needed.

### Model, requirements, code and CMDB

- **ArchiMate 3.2 rules:** the model check checks the relationships of ArchiMate elements (e.g. access only to passive structure, influence only to motivation). Switch them off with `"archimate": false` in governance.json.
- **Traceability chain:** `trace_requirements` with a source folder follows each requirement → model → code → test → issue, and shows where the chain breaks.
  - Example: "Compare the requirements in C:\Projects\BRD.docx with the model and the code in C:\repo\shop".
- **CMDB vs model:** a CSV/JSON export (e.g. from ServiceNow), or data fetched through an MCP server, is compared with the components of the model. The report shows what is missing in the model, what is not in the CMDB, and where the owner or status differs.
  - Example: "Compare the CMDB export C:\Export\cmdb_ci_appl.csv with the components of the model".

### MCP servers

- **Ready-made servers:** Azure DevOps, GitHub, ServiceNow / CMDB, local files and Compass Hub on the "MCP servers" card. Pick one from the list, then add your organization or address.
- **MCP server tokens:** saved from the card into Credential Manager (names start with `mcp_`).
- **Node.js programs:** `npx` in the configuration works on Windows (started as `npx.cmd`).

## Version 0.8.1 (2026-10-23)

A security release after an audit of the whole application. Recommended for everybody, especially teams with Compass Hub. Some actions now ask for consent more often than before; this is on purpose.

### Compass Hub

- **Robustness:** a malformed request no longer stops the hub. Requests are limited in size, header count and time, and the token is checked before the body is read.
- **Tokens:** hub.json may hold "token_sha256" (the token's hash) instead of "token"; `LoonarCompass.exe --hub-token` creates a new random token and its hash. Names and tokens must be unique. After 20 wrong tokens an address is refused for 10 minutes (a valid token still works).
- **Change sets:** the hub computes the description and preview the reviewer sees from the operations, and the web view shows the operations. Nobody, not even an admin, approves their own set. Applying has two steps (claimed by one Compass, then reported), so a set cannot be applied twice. An admin can free a set that got stuck.
- **Ask:** the assistant in the web view only reads the model (no files, Jira, Confluence or MCP servers) and answers one question at a time, with a time limit. New policy settings: "ask_min_role" (who may ask) and "ask_per_hour" (20 questions per person and hour by default).
- **Web view:** Content-Security-Policy and protection against framing. The token is kept only for the browser session, and there is a "Sign out" button. Internal errors are shown as an id; the details go to hub.log.
- **Audit trail:** accepts only known fields, stamped with the hub's time. Workstation details are visible to the admin only.

### Consent and saves to the model

- **Files:** XMI export, documents from EA templates and XMI import always ask for consent, also in auto mode, and show the full path. Only full paths on a local drive are accepted, not network shares or system folders.
- **Deleting:** an id given as text or as a fraction no longer skips the question when deleting an element with relationships. Deleting a package always asks.
- **Memory, templates and scripts:** saving a memory note, saving a document template, and saving or running an EA script always ask, showing the whole text or code. A script runs exactly as shown, with its includes.
- **Reading files:** before the assistant reads a file outside the conversation's attachments (requirements document, source folder), it asks.
- **SQL queries:** the assistant can only read. Queries with write words (INSERT, UPDATE, DELETE, INTO…) or unusual quoting are refused, also when reading through the EA window.
- **Event agents:** they only read the model. Object names go into their prompt as data, not instructions.
- **Undo:** the undo journal is checked before it runs, and the consent shows what will be deleted.

### MCP servers and connections

- **MCP consent:** every MCP tool call asks for consent unless the server has "trusted": true. The server's own "read-only" claim is no longer enough.
- **Secrets:** Compass's own keys and passwords cannot be passed to an MCP server through ${secret:…}. The exception is ${secret:hub_token} for the /mcp endpoint of the hub in Settings. A local MCP program gets only basic environment variables plus those in "env".
- **Redirects and addresses:** HTTP redirects are followed only within the same server, so API keys do not go anywhere else. An http:// address is allowed only for this computer, with no user name or password in it.

### Other

- **Diagram editor:** names from the model are shown as text (no HTML), the page has a CSP, and only objects of the open diagram can be deleted or changed.
- **EA add-in:** the log no longer holds the connection string, Compass starts in its own folder, and DLLs are not searched in the current folder.
- **EA error boxes:** only the boxes that appear during Compass's query are closed.
- **SQLite 3.50.2:** fixes known vulnerabilities. Model files are opened in defensive mode.
- **Recommendation for companies:** a separate PostgreSQL role with minimal rights (SELECT, plus INSERT, UPDATE and DELETE on the t_* tables for saves through EA; never superuser), and connection encryption with certificate verification.

## Version 0.8 (2026-10-23)

The biggest release so far. Compass now checks model quality, verifies every save and lets you undo it, compares the model with requirements and with code, estimates changes and works as a team tool. Each feature below has an example prompt: type it in the conversation as it is, or in your own words.

### Choice of AI model

- **Providers and models.** Anthropic (Claude) or OpenAI, any model (from the list or your own id) and the effort level, from "model default" to max. Where: Settings → "AI model" card.
  - The default is unchanged: Claude Haiku 5.5.
  - Each provider has its own key in Windows Credential Manager; data goes only to the provider you chose.
- **Any OpenAI-compatible endpoint.** Azure OpenAI, Ollama, LM Studio or a company gateway. With a local model, nothing leaves your computer.

### Model quality and team rules

- **Model check.** Rule-based, not guessed by the AI. Every finding has a severity: error, warning or information. It finds:
  - default and empty names, elements without a description, orphans;
  - repeated names and relationships, relationships to elements that do not exist;
  - empty packages and diagrams, flows that break UML or BPMN rules.
  - Example: "Check the quality of the Domain package and list the main problems."
- **Organisation rules.** In governance.json next to the settings, or from the team hub: naming conventions, required tagged values, allowed relationships between types (metamodel / MDG).
  - Example: "Check whether all components in the Systems package have the tagged value Owner."
- **As-is / to-be gap analysis.** What is new, what is to be retired and what changed (stereotype, description, relationships); to Excel on request.
  - Example: "Compare the As-is package with the To-be package and save the gaps to Excel."
- **Spelling in the Compass language.** Element and diagram notes, scenario steps and the project glossary are checked with the Windows spell checker. Element names and glossary terms count as known words; corrections only after your approval.
  - Saving the language in Settings also switches EA's spell checker to that language, if EA has a dictionary for it.
  - Language missing: Windows Settings → Time & language → Language → add the language.
  - Example: "Check the spelling of the notes in the whole model and fix the mistakes."

### Saves you can trust

- **"Before → after" preview.** The approval window shows what is created, which value changes to which, and what is deleted.
- **Verification after every save.** Compass reads the model again and compares it with the request. If anything differs, the assistant tells you instead of saying "done". It checks that:
  - created objects exist, relationships have the right ends, elements are in the requested package;
  - names, descriptions, stereotypes and tagged values have the new values;
  - deleted objects are gone, XMI files and reports were written.
- **All-or-nothing saves.** If a save fails halfway, the objects it had already created are removed.
- **Safe deletion.** Deleting an element that has relationships always needs your approval, even in auto mode, and the window lists those relationships.
- **Time machine.** Every save goes into an undo journal with the reverse operations; what was created or changed can be undone. Compass does not undo a deletion: before it, Compass takes an EA baseline of the package, which you restore in EA. Compass always asks before undoing.
  - Examples: "Undo the last change.", "Show the history of the changes you made in this model.", "Undo the last two changes."

### Requirements, code and estimates

- **Requirements against the model.** A Word document, text or Markdown, a Confluence page or text pasted into the conversation becomes a traceability matrix: covered, likely covered, not covered, "same id, different content", and model elements that no requirement asks for. Trace relationships are created only after your approval.
  - Example: "Compare the requirements in C:\Projects\BRD.docx with the model: what is covered and what is missing?", then "Add trace relationships for the covered requirements."
- **Model against code.** A source folder, read only: classes, interfaces, enums and records with their fields (Java, Kotlin, C#, TypeScript, Rust, Python), SQL tables with columns, API operations (OpenAPI operationId). The report shows what is missing from the model, what is no longer in the code and how the fields differ.
  - Example: "Compare the Domain package with the code in C:\repo\shop – what is missing in the model and what is missing in the code?"
- **Change estimate.** The impact of a change becomes a list of tasks in plain language (change, adapt, check) with hours by element type and level of relationship, diagram updates, tests, analysis, documentation and a risk buffer; to Excel on request.
  - The norms are in estimate.json next to the settings. This is one way to estimate, not the only one: adjust the norms to your team.
  - Example: "Estimate a change to the Order class: what has to change and how long will it take?"

### New model operations

- **Automatic diagram layout.** Down, right, up or left, with the spacing you choose.
  - Example: "Lay out the Domain model diagram from left to right."
- **XMI export and import of a package.** Import gives new GUIDs unless you ask to keep them.
  - Example: "Export the Domain package to C:\Export\domain.xml."
- **Documents from EA document templates.** DOCX, PDF or RTF, as with Publish → Report Builder.
  - Example: "Generate a report from the Model Report template for the Domain package to C:\Reports\domain.docx."
- **More editing.** Diagram swimlanes, linked documents of elements, reversing a relationship, package details (name, description, version, status, stereotype). Aggregation and composition are created as proper UML relationships.
  - Examples: "Add the lanes Sales and Warehouse to the Order process diagram.", "Reverse the relationship between Customer and Order."
- **Structured scenarios.** Scenarios of elements with actor and system steps.
  - Example: "Add a five-step basic scenario to the use case Place order."

### EA scripts

- **Read, save and run scripts.** The assistant lists groups and scripts and reads a script. It saves JScript, VBScript and JavaScript (a missing group is created; approved like any save) and runs a script after your approval every time, in the open EA window or in the background.
  - Repository and Session.Output work as in EA; "!INC Group.Script" includes are resolved from the model.
  - Example: "Create and run a JScript EA script that prints the names of all packages in the model."

### Teamwork: Compass Hub

- **An optional team service.** `LoonarCompass.exe --hub hub.json` on a team computer or server. hub.json holds the listen address, the team's models (EA databases and files), users with tokens and roles (viewer, architect, admin) and the policy.
  - In Compass: Settings → "Team (Compass Hub)" card, with the address and your token from the hub administrator.
- **Shared policy.** Model-check rules and the minimum approval mode apply on every workstation; every save goes to the central audit trail; shared team notes.
- **Change sets for review.** Instead of saving, the assistant proposes the change to the hub. A second architect comments on it and approves it in the web view (four-eyes rule); the author then applies the approved change set, with verification.
  - Examples: "Propose a change to the team for review: add the class Warehouse to the Domain package.", "Show the approved change sets and apply change set 3."
- **A web view for people without EA.** The hub address in a browser, with the tabs Ask, Search, Model check, Gaps, Audit trail, Team notes and Change sets. In "Ask", a stakeholder asks a question about the model and the assistant answers read-only.
- **The hub's MCP server.** Read-only, for other AI tools: sources, package tree, element search and details, impact analysis, model check and gap analysis.

### Jira and your own MCP servers

- **Jira (Cloud and Data Center).** Issues by key or JQL search as context for the model; after your approval, new issues, comments and links to elements. Where: Settings → "Jira" card.
  - Examples: "Read issue SHOP-123 and check which model elements it concerns.", "Create a Jira issue for the missing description of the Invoice class."
- **Your own MCP servers.** "MCP servers" card, stored in mcp_servers.json: a local program (stdio) or HTTP; tokens from ${secret:name} (Credential Manager) or ${env:NAME}.
  - A tool that is not read-only asks for approval before every call, unless its server has "trusted": true.
  - "Save and test MCP servers" shows how many tools each server has.
  - Example: "Through the MCP server hub, list the team's model sources."

### Compass in Enterprise Architect

- **Live check.** A newly drawn relationship or a changed element is checked straight away (rules and the spelling of notes); remarks appear on the "Loonar Compass" tab of the System Output window.
- **Event agents.** "Event agents (Compass in EA)" card, stored in event_agents.json: when an element is created or changed, or a relationship or diagram is created, the assistant does what the rule says.
  - It reads the model and answers in System Output (lines starting with 🤖); it never changes the model by itself; the same agent and object at most once a minute.
  - Example rule for a new component: "check that it has a description and an owner, and suggest the missing relationships".
- **Renaming a conversation.** The pencil next to the conversation list.
- **Reading a model file open in EA.** Reads go through the EA window, so they see the latest changes even before EA writes them to disk.

### Other changes and fixes

- Settings cards are no longer numbered; the first card is "AI model".
- Progress messages say "The assistant …" instead of "Claude …".
- File models: after every save the changes are on disk at once, so the next reads see the new state.
- EA error dialogs (SQL, XML) no longer block background work; queries with expressions and repeated column names return correct results.
- User guide in 4 languages: a new chapter of use cases with ready-made prompts, the chapter "Compass in Enterprise Architect" and the cards "AI model", "Jira", "Team", "MCP servers" and "Event agents".

## Version 0.7.5 (2026-10-22)

- Fix: Compass in EA fills its whole panel and follows every change of its size, also in a floating window and in a tab group.
- "About" and the footer show the full version number (e.g. 0.7.5).

## Version 0.7.4 (2026-10-21)

- Fix: Compass in EA again follows every change of its panel's size (in 0.7.3 it kept the size it opened with).

## Version 0.7.3 (2026-10-21)

- Compass in EA: typing works after "Add to the Compass conversation", after clicking the browser or a diagram and after switching panels (the keyboard goes to the Compass window).
- The Compass window fits the visible part of the EA panel, so the bottom icon bar stays on screen.
- A wrong query of the assistant to a database open in EA no longer stops EA with an error box: Compass closes it and passes the error to the assistant, which corrects the query.

## Version 0.7.2 (2026-10-21)

- Compass in EA works with any database open in EA, also one not in the settings: it reads the model through EA's own connection, with no passwords or configuration.
- The Compass window always docks in EA's panel (before, it sometimes stayed beside it as a separate window).

## Version 0.7.1 (2026-10-21)

- EA project glossary: the assistant adds a term with its meaning and type (by default "Slownik"); a Polish term goes in with all its forms (singular and plural, from the nominative to the vocative), so EA highlights every form in notes. In the message field: right-click a word → "Add to the EA project glossary", or ask in the conversation.
- Compass in EA follows the width of its panel: text wraps instead of disappearing on the right.
- Context menu of an element, package or diagram: "Add to the Compass conversation" is again directly under Specialize.
- The keyboard works after switching EA panels (e.g. Notes → Compass); Compass closes together with EA.

## Version 0.7 (2026-10-21)

Compass in Enterprise Architect:

- EA add-in: the "Loonar Compass" button on the Start ribbon (Add-Ins group) opens Compass as a docked window in EA, in the same place as Properties (EA remembers where you put it). The installer has an "Add Loonar Compass to Enterprise Architect" option; it works with any EA installation, also a trial.
- System Output has a "Loonar Compass" tab: your messages and the assistant's steps (model reads, writes to EA, results) with the time. The chat keeps one line with the last step.
- In EA, Compass always works with the project open in that EA window (file or database): no model to choose and nothing to confirm. The list shows only the conversations of that project, and opening another project in EA switches Compass. Without an open project Compass does not work.
- Writes go through the same EA window, so changes show at once in the browser and on diagrams.
- "Show diagram" and "Edit diagram" open the diagram in EA, without images in the chat.
- Context menu of an element, package or diagram: "Add to the Compass conversation" puts a reference to the object into the message, e.g. «Package: Shop (id 5)».
- Layout of the docked window: a smaller font, the project's conversations in a drop-down with a "+" button, view icons at the bottom, pages scroll sideways and up and down; Ctrl+V pastes pictures, files and text.
- In EA the settings keep only Claude, change approval, Confluence and Miro; the model and EA come from the open project.

Everywhere:

- Dictionary: right-click an underlined word in the message field → "Add to the dictionary". A Polish expression goes into the Windows dictionary in all its forms (singular and plural, from the nominative to the vocative).

## Version 0.6.5 (2026-10-20)

- The EA account belongs to the model, not to the app: the EA account and password are set in card "2. Model", with the file or database. Card "3. Enterprise Architect" has only the path to EA.exe and "Save and test EA access".
- File and database: a checkbox "The model has security enabled (EA user accounts)". Without it the account and password fields are disabled and EA opens the model without a login.
- New option "The model requires a lock to edit (Require User Lock to Edit)": with it the assistant locks the packages and elements it changes and releases the locks afterwards; without it, it does not lock.
- The test of the model settings and of a conversation's model saves a test package in the model, checks it and deletes it at once.
- A new conversation starts with confirming its model ("Confirm the model and start"). The conversation's model is then fixed and shown read-only; changing the default model in the settings does not change conversations in progress.
- Fix: a conversation appears on the list right after its first message. Before, a conversation in which the assistant was still working was missing from the list.
- Fix: activity diagram partitions (swimlanes) taller than wide are drawn vertical in EA, with the name on top.
- Fix: the preview of a diagram open in the EA window shows the state after the last write (EA reloads the diagram before exporting the image).

## Version 0.6.4 (2026-10-19)

- Drawing from a picture: the assistant reproduces the layout of the attachment (rows, order, position relative to neighbours, how lines run) instead of laying the diagram out anew; it measures the picture first, keeps at least 40 px between elements, and afterwards checks every relationship first, then the layout (at most two rounds of corrections).
- A line style for the whole diagram is applied after the relationships are created and keeps lines routed by hand.
- An object node (ObjectNode) of an activity diagram is saved as an EA object instead of stopping the save.
- The assistant's short notes between steps are in the language of the conversation.

## Version 0.6.3 (2026-10-19)

- Diagram editor: saving to EA keeps the lines exactly as the editor shows them (horizontal and vertical segments, where they touch the elements), also for new relationships. Before, EA got only the points moved by hand and drew the lines diagonally.

## Version 0.6.2 (2026-10-19)

- Fix: a fork/join on an EA activity diagram was drawn horizontal although it was meant to be vertical. The application now sets the orientation from the shape (taller than wide = vertical).
- Diagram editor: "Fork / join" in the activity diagram toolbox.

## Version 0.6.1 (2026-10-19)

- Fix: the diagram editor did not open diagrams of a model in a PostgreSQL database (error “column \"default\" does not exist”). Models in .qea/.qeax files worked.

## Version 0.6 (2026-10-18)

Diagram editor:

- Every EA diagram in the conversation has an "Edit diagram" button. It opens the diagram in an editor window (WebView2): moving, resizing, aligning, renaming, undo (Ctrl+Z / Ctrl+Y). The editor changes the model, not a picture: saving goes through EA, with the approval panel and the change log.
- New elements from a toolbox that fits the kind of diagram: class, component, use case, activity, BPMN, state machine (states, initial, final, choice), deployment (nodes, devices, execution environments, artifacts), package (real packages) and sequence. A new element goes to a free spot; connect by dragging from one element to another.
- Connecting: an element under the mouse shows a blue arrow; drag it to another element. The kind of relationship comes from the "Connector" list.
- BPMN: task types (user, service, manual, script, send, receive, business rule), event definitions (message, timer, signal, error…) and gateway types (exclusive, inclusive, parallel, complex, event-based) as icons and markers, the type chosen in the "General" tab, an intermediate event in the toolbox. Classes grow by themselves when attributes and operations are added.
- "From the model…": puts elements that already exist in the model on the diagram, with a search.
- Deleting asks: from this diagram or from the model. When deleting from the model it shows on how many and which other diagrams the element is and how many relationships it has.
- "Properties" panel: the fields of EA's general tab (name, alias, stereotype, keywords, status, version, author, phase, complexity, abstract, notes; package, GUID and dates read-only), attributes and operations of classes, tagged values (with the profile's pick-lists, e.g. BPMN), appearance on the diagram (color, line style). Each field is marked "model" or "diagram".
- Relationships: name, stereotype, direction, multiplicities and roles of both ends, notes, tagged values.
- Sequence diagrams: lifelines, messages in their EA order (reordered with ▲ ▼ or Alt+↑ / Alt+↓), synchronous, asynchronous and reply messages, a name taken from an operation of the receiver.
- "Tidy with AI": saves the changes and asks the assistant in the conversation to arrange the diagram so it reads well.
- New write operations for the assistant: changing relationships, changing and deleting attributes and operations, the element subtype (pseudostates), sequence messages with their number, kind and reply flag; removing a tagged value.

Miro (from EA to Miro):

- Settings card "7. Miro": access token (in Windows Credential Manager) and a connection test.
- "Send to Miro" under a diagram: board choice, a thumbnail of the board with the place for the diagram (the application finds a free spot; the frame can be dragged), sent as editable shapes and connectors in a frame named after the diagram. Sequence diagrams: lifelines and numbered messages in their EA order.

Documentation:

- New chapters of the guide: "9. Diagram editor" and "10. Sending diagrams to Miro" (the following chapters moved by two); on the website, a documentation icon in the header on phones.

## Version 0.5 (2026-10-17)

A model for each conversation:

- Every conversation has its own model. The conversation header shows it ("Model: …"); a click opens the model of the conversation, where you can check or change it.
- A new conversation says which model it will use: the default from the settings. "Choose another model…" lets you pick another PostgreSQL database (server, port, database, account, password, encryption) or an EA file (.qea, .qeax), with its EA account, and test the connection.
- Passwords of a connection that differs from the settings are kept in Windows Credential Manager; uninstalling removes them together with the other secrets when you choose so.
- Dropping a .qea or .qeax file on the window makes it the model of the conversation (in a conversation with messages, a new conversation starts).
- Existing conversations get a copy of the model from the settings, so they keep working with it when the settings change later.
- The assistant's memory stays shared by all models. A note is limited to one model only when you ask for it ("remember this only for this model").

## Version 0.4 (2026-10-17)

Models in files:

- The model can now be an Enterprise Architect project file .qea or .qeax: Settings → "2. Model" → "EA file (.qea, .qeax)" and the path to the file. No database server, account or password is needed.
- The assistant reads the file directly and only for reading. Changes are saved by EA: through your EA window when you have the file open there (so a second EA never opens the same file), otherwise by EA in the background.
- User security in a file is optional: without it, the EA account and password stay empty. A file with security enabled works like a database model, with an EA account.
- The installer offers the same choice on the configuration page.
- The assistant writes its queries in the SQL of the model's storage (SQLite for files, PostgreSQL for databases).

## Version 0.3 (2026-10-17)

New name and public release:

- New name: Loonar Compass (formerly Loonar EA Assistant). Settings, conversations, memory, templates and passwords from the previous version are carried over automatically on the first start; files with Polish names in the data folder have been given English names (`audit_log.jsonl`, `confluence_pages.json`).
- The source code is public on GitHub (github.com/loonar-eu/loonar-compass), and the loonar.eu website describes the application in four languages, with frequently asked questions (FAQ).
- A new freeware license (the Polish version is binding; EN/DE/FR translations): use is allowed also for commercial purposes, without time or territorial limits, with rules on AI output, data and open-source components. The licenses of the open-source components are listed in the THIRD-PARTY-NOTICES.txt file and in the "About" view.
- "Support the project" view (heart icon in the navigation bar): voluntary support via suppi.pl/loonar.
- Privacy policy and legal notice on loonar.eu; in the application, short notices next to the API key (what is sent to Anthropic), in the bug report and in the newsletter card, and below the message field a reminder that answers are prepared by an AI model.
- A new background for the splash screen and the installer banner (a sky map with a globe grid and a compass rose).

Security and privacy:

- The database password is stored in Windows Credential Manager, like the API key, the EA account password and the Confluence token; a password previously saved in the settings file is moved there automatically.
- Encryption of the connection to the model database (TLS) on the "2. Model database (PostgreSQL)" card: preferred (default), required, or required with the certificate verified against the Windows certificate store. The connection test says whether the connection is encrypted and warns against a superuser account.
- Confluence over HTTPS only (http:// only for a server on the same computer). Before publishing, the assistant checks whether a page with that title already exists and says explicitly that its content will be replaced.
- Deleting and moving objects and copying packages require a separate approval also in plan mode, and the approval panel names the objects. Saving to the assistant's memory and overwriting a template go through approval just like changes to the model.
- The assistant treats the content of attachments, documents and the model as data, not as instructions. An image drawn by the assistant cannot load a file from disk or from the network, and its size is limited.
- Reading from the database: a query time limit and a block on server functions that read files or affect other sessions. The connection string of the open EA window, with its login details, is never sent to the model.
- System programs and libraries are started only from the Windows folder, never from the downloads folder.

Installation:

- Installation without administrator rights: by default just for the current user (`%LOCALAPPDATA%\Programs\Loonar Compass`); an installation for all users asks for administrator rights. Settings and passwords always go to the profile of the logged-in user.
- The earlier version (Loonar EA Assistant) installed just for you is removed during installation; one installed for all users can be removed with a button in the installer.
- Uninstalling removes, on request, the whole data folder and all secrets from Credential Manager (including the Confluence token and the database password).
- The installer window fits on small screens and at high display scaling; the "Quick Launch" shortcut, the "Model" field and the Workspace ID field have been removed.

Stability:

- Requests to the Claude API are retried on overload, rate limits and temporary server errors; an invalid API key and blocking by a proxy have clear messages.
- Proxy from the Windows Internet settings (in addition to the HTTPS_PROXY variable).
- On an unexpected error, the application shows a message and writes the details to a file instead of disappearing; an error in the settings file is described instead of "no settings".
- Fixed a crash on long image captions with non-ASCII letters on Confluence pages.
- The application and the installer also start on older releases of Windows 10 and without the Visual C++ Redistributable package.

Languages:

- Four languages: Polish, English, German and French. The installer starts with a language choice (English by default), which sets the language of the application, the assistant's answers and the default document templates. You can change the language in Settings (the "4. Language and change approval" card).
- Everything available without AI is in four languages: the application and installer windows, document templates, the license, the user guide and the changelog.
- The assistant answers in the application's language unless you write to it in another language.
- After a language change, untouched default templates switch to the new language immediately, while the assistant's memory and your own or modified templates are translated by Claude. Without a connection to Claude, an orange exclamation mark with an explanation in the tooltip appears on the Settings, Assistant memory and Document templates icons, and the translation completes automatically once the connection works (or when you click "Translate now"). Conversation history is never translated.
- A new, simpler logo (flat, two-color, legible at small sizes and suitable for print) in the application and installer icons and in the "About" view.

Confluence documentation:

- Confluence pages prepared by the assistant from the model and the conversation: page code in the source editor format (storage XHTML) with headings, tables, lists, a table of contents, panels (info, note, warning, tip, expand section), statuses, Jira issues, user mentions and code blocks; diagrams from EA and images as attachments. In the chat, a page card with the "Copy page code" and "Open images folder" buttons.
- Publishing to Confluence via the REST API (on request, always after approval, also in auto mode): a new page under a parent page or an update of an existing one (by ID or title), images as attachments (existing ones get a new version). Diagrams are re-exported on every publication, so republishing refreshes the page after changes in the model; the assistant remembers published pages.
- Settings, "6. Confluence" card: address, token (Data Center: personal access token; Cloud: email and API token), space key and parent page, with the "Save and test Confluence connection" button.

Document templates:

- Document templates that the assistant fills with content from the model (a Confluence page or a Word document), in the `templates` folder next to the conversation history: Preliminary feasibility analysis, BRD - Business requirements, System change description, Process description. Templates are Markdown files with instructions for the assistant; you can modify them, add your own or ask the assistant to create a template from a sample document.
- "Document templates" view in the navigation bar: a list with descriptions, "Use" (inserts into the chat a request to prepare the document), "Edit" and "Open templates folder".

## Version 0.2 (2026-10-15)

Memory:

- Memory across conversations: the assistant saves notes (modeling conventions, preferences, corrections, project facts, anything after "remember…") as local Markdown files and knows them in every new conversation. A note on the same topic is updated instead of duplicated; outdated ones are removed. Saving a note in the middle of an answer no longer interrupts further work (previously the API returned a 400 error about the "thinking" block signature).
- "Assistant memory" view in the navigation bar: a list of saved notes with dates, a "Remove" button and "Open memory folder" (the `memory` folder next to the conversation history, with the `MEMORY.md` index).

Enterprise Architect:

- Routing a connector along a given path, e.g. following a sketch in a screenshot: bend points and the points where it meets element edges (horizontal and vertical segments only, or at any angle).
- Changing the line style of connectors on a diagram (like Line Style in EA): direct, auto routing, custom line, orthogonal square, orthogonal rounded, tree and lateral; for selected connectors or for all connectors on the diagram.
- Change impact analysis: from a given element, following relationships in depth (up to 6 levels, in one or both directions, optionally only selected relationship types), with the relationship through which the impact arrives, the path and the diagrams containing the affected elements; result as a description, an image, a document or an Excel file.
- Where operations (services) of a class, component or interface are used: sequence diagrams (messages to the element or its lifeline, linked to the operation or with a matching name) and activity diagrams (actions calling the operation), with the package and the caller, plus a list of operations not used on any diagram.
- Class attributes and operations (with parameters), copying a package with all its contents, and a baseline (saved version) of a package. Before deleting or substantially restructuring an existing package, the assistant is instructed to create a baseline (this is a recommendation for the assistant, not a safeguard: for important packages, ask for a baseline explicitly); it is restored in EA (Manage Baselines → Compare/Restore), because restoring through automation makes EA hang.
- Working with the open EA window (same model): the assistant knows which diagram you have open and what is selected on it (also in the Project Browser), opens diagrams and highlights elements in the Project Browser, and after saving refreshes the changed diagrams and packages in your EA window by itself, without F5.
- Audit log: every save by the assistant to the model (including failed ones) in the `dziennik_zmian.jsonl` file next to the conversation history; "Audit log" view in the navigation bar with result, time, account and database, and export to Excel.
- Element keywords (the Keywords field in EA) when creating and editing.
- Diagram legend with automatic coloring, as in EA: filter by element property (keyword by default, also stereotype, status, type) and/or connector property (e.g. stereotype), entries with value, color and label.
- Renaming an existing diagram and changing its description and stereotype.
- Hiding selected connectors on one diagram (and showing them again): relationships remain in the model and on other diagrams, so a dense diagram can be decluttered without deleting relationships.

- Changing the position and size of an element that is already on a diagram (e.g. enlarging a frame, pool or subprocess) without removing it from the diagram; connectors remain.
- Linking an element (e.g. a BPMN subprocess) to an existing diagram as a composite diagram, so that a double-click opens the diagram. The operation was in the application, but the assistant did not know about it and told you to make the change manually in EA.
- Embedded BPMN subprocesses: steps as child elements of the subprocess, drawn inside its rectangle on the same diagram (just as the BPMN import from Camunda saves them); in addition to collapsed subprocesses, whose steps are on a separate diagram.
- Adding steps to an existing subprocess: the child element is created in the subprocess's package and linked to it (as in the EA database). Previously EA rejected this with the "Element locked" error, even though the lock was held by the assistant itself.

Startup:

- Closing the application also closes its EA session. Previously the EA process stayed in the background and held a license seat, so subsequent EA sessions could wait indefinitely.
- The window also opens on less capable graphics cards (e.g. in Wine via Vulkan): the application asks the graphics card only for the features it reports.
- Communication with EA via COM without the system propsys library, so it also works in Wine.

Documents and images:

- Word documents (DOCX) created by the assistant: headings, paragraphs, lists, tables, diagrams from EA and previously drawn images; in the chat, a file card with the "Open" and "Save as…" buttons.
- Excel workbooks (XLSX) created by the assistant: multiple sheets, headers with a filter and a frozen first row, numbers as numbers (e.g. a list of elements, a matrix, an impact analysis result).
- Images drawn by the assistant (e.g. a use case and requirements matrix, a table, a chart, a schematic) as PNG in the chat, with "Save as…"; the assistant sees the result and can improve it.

Contact:

- "Report a bug" view in the navigation bar: subject, description, images (dragged into the window, pasted from the clipboard or chosen from disk, with thumbnails), an optional email address for replies and technical information (without passwords and keys). "Send report" opens a ready-to-send message to mariusz@loonar.eu in the Windows mail program with the images as attachments (Simple MAPI); if the mail program does not support this, it opens the message without images and the folder with the images to drag in.
- Settings, "5. New releases (newsletter)" card: email address and consent to news about new releases; the choice is remembered in the settings, and subscribing and unsubscribing are sent to the author as a message from the mail program.

Appearance:

- New window design, features unchanged: a navy navigation bar on the left with icons (Chat, Settings, Changelog, About) instead of tabs at the top, a warm light background as in Claude, Segoe UI font with semibold headings.
- Conversation list grouped into "Today", "Yesterday", "Last 7 days" and "Earlier"; a new conversation with the "+" button, the selected conversation highlighted, status dots unchanged.
- Conversation header with the title and a "Claude is working" indicator while the assistant is working.
- Your messages in a beige bubble as in Claude; the assistant's answers with the application icon, in a width comfortable for reading.
- Message field as a single card: text, paste button, model and approval mode, and round send and stop buttons.
- Step list as a card: expanded while working, collapsed to one line when finished (clicking the header expands it).
- New conversation screen with the logo and a hint about what you can do.
- Settings as white cards with a shadow; Changelog and About with page margins.

Settings:

- Neutral default values, used only when there are no settings yet (existing ones are not changed by the installer or the application): database server, database name, EA account and passwords empty (the server field shows a gray hint "e.g. 127.0.0.1"), port 5432, EA model "Model", Max tokens 10000.
- The path to EA.exe can be chosen in a file selection dialog (the "…" button), also in the installer.
- Package locking on save is always enabled; the option has been removed from the settings.
- When EA refuses to save because of a lock, the error states who is locking the element, its package and diagram (account or group, since when), or that there is no lock in the model.

Installation:

- "What's new" page in the installer, right after the welcome page: on an update, the changes since the installed version; on a new installation, the changes in the current version.
- Installation and uninstallation progress bar (percentage and current step) instead of a spinning wheel.

## Version 0.1 (2026-10-13)

First version, complete: all features in the list below are verified. A standalone desktop application for working with an Enterprise Architect model with the help of Claude Haiku 5.5.

### Working (verified)

Model and conversation:

- Conversation with the Claude Haiku 5.5 model via the Anthropic API, with Workspace ID support.
- Adding messages while the assistant is working: the message is queued and the assistant takes it into account at the next step, without interruption.
- Step list in the chat while the assistant is working: what it is doing (database query, scope of the save to EA, diagram retrieval), how long it takes and the result of each step. The list is not saved in the conversation history.
- Working in several conversations at once: while the assistant is working, you can start a new conversation or return to another one, and the started work continues in the background (an orange dot next to the conversation name while working, a green one after it finishes, until you look at that conversation). Saves to EA from different conversations are executed one after another.
- "Stop" button: stops the work after the current step; further planned steps are not executed.
- Save approval modes: ask (a panel with a description of the changes and the "Approve" / "Reject" buttons before each save; a rejected save does not change the model), plan (a single question before the first save in an answer, further saves in that answer without asking) and auto (without asking).
- The assistant knows the application it runs in: it answers questions about its features and limitations based on this changelog.
- The assistant refers to model objects by name, package and GUID, without internal database numbers.

Enterprise Architect:

- Reading the EA model from a PostgreSQL database (packages, elements, diagrams, connectors). The database session is read-only. A database port other than 5432 also works when saving through EA.
- Saving to the EA model via COM: the assistant creates packages, diagrams, elements and connectors and lays out elements on a diagram. It logs in with the EA account from the settings and releases the locks after saving, so the result can be edited right away.
- A persistent EA session for the whole application: the first EA call takes from a few to over ten seconds (EA startup and opening the model), subsequent ones under a second. The application talks to EA directly via COM, without PowerShell. The session closes after 10 minutes of inactivity and when the application is closed.
- Drawing by the assistant on the fly, without separate importers: from a BPMN file (elements at the positions from the file), from a photo, from a sketch and from a description alone.
- Correct BPMN 2.0 types in EA: tasks, subprocesses, events, gateways and flows, with stereotypes and tags.
- Moving existing elements, diagrams and packages to another package; connectors remain.
- Deleting from the model: elements, connectors, diagrams and packages, and removing an element from a diagram.
- Fill colors: the default element color and the element color on a selected diagram.
- Navigation cells: tile links to diagrams on a start diagram, with a label (name of the target diagram) and an icon matching its type.
- Retrieving a diagram from EA as a PNG image (as EA draws it) and showing it in the chat, with the "Save as…" and "Open full size" buttons.
- Optimistic locking of saves: the assistant can specify objects that must not have changed since it read them; if they have changed, the save is rejected.

Attachments:

- Dragging files into the window, from Explorer and from the browser (downloaded files): up to 5 files, up to 10 MB each.
- Pasting from the clipboard as an attachment: an image (e.g. a screenshot) or files copied in Explorer, with Ctrl+V or the button next to the message field.
- PDFs and images are passed to the Claude model directly, DOCX as text, text files (BPMN, XML, CSV, code…) as text.
- ZIP files are unpacked: images, PDFs and text files inside are passed to the Claude model as if they were attached separately (up to 20 files).
- An attachment whose file is no longer on disk leaves a mention in the conversation and does not block further work in that conversation.

Window and chat:

- Multi-line message field: long text wraps, Enter sends, Shift+Enter inserts a new line.
- The up arrow in an empty message field recalls your last message from the conversation; further presses go back to earlier ones, the down arrow goes forward.
- Spell checking in the message field with the Windows system dictionary, in the language from the system settings: misspelled words are underlined in red.
- Formatting of the assistant's answers in the chat: bold, bulleted and numbered lists, tables, headings, inline code and code blocks.
- Time next to each message in the chat (for older messages also the date).
- An opened conversation is shown from the end, i.e. from the latest messages.
- A circle with an arrow above the message field when the chat is scrolled up: jumps to the end of the conversation.
- Searching conversations: by title and by message content.
- Renaming a conversation: pencil next to the selected conversation, Enter saves, Esc cancels.
- Conversation history saved in files and returning to earlier conversations.
- The window stays responsive while the assistant is working (the work runs in the background).
- Constant window size when moving between monitors with different scaling.
- Light window color scheme, "Changelog" and "About" tabs, large application logo on the "About" tab.
- Splash screen with the graphic from the Loonar business card (globe, keyboard, stars), the application name and version, and the owner's details.
- Startup without a terminal window, a bar with the owner's details.
- Application icon in the exe file: visible in Explorer and when pinned to the taskbar.

Settings:

- Settings in the window in four framed groups: API key, model database, EA account, approval mode. The API key and the EA account password are stored in Windows Credential Manager. The Claude model is shown read-only.
- Each group is saved separately ("Save and test…" buttons), with the test result below the group: API key (a request to the model), database (connection and read), EA (login via COM with the given account).

Installation:

- Installer as a wizard: license, destination folder (Program Files by default), shortcuts (Start menu, desktop, Quick Launch), optional configuration (API key, database, EA account, approval mode) and summary.
- Updating by running the installer again: settings, conversation history and attachments are kept.
- Uninstalling from the Start menu or from Windows Settings; user data and secrets are removed only on request.
- Settings, conversation history and attachments are stored in the user profile (`%APPDATA%\Loonar EA Assistant`), separately for each user of the computer.
- User guide (PDF) in the installation folder and in the Start menu.

### Prerequisites

- The EA model is a project file .qea or .qeax, or is in a PostgreSQL database. Older file formats (.eap, .eapx, .feap) and other databases are not supported; EA can convert an .eap/.eapx file to .qea with a project transfer.
- A model in a database must have user security enabled (Enable Security, "Require User Lock to Edit" mode) and a user account for the assistant (any name), with a password and edit permissions.
- In a model file, user security is optional; with security enabled, the file needs such an account too.

### Known limitations

- The first EA call after starting the application (or after a 10-minute break) takes from a few to over ten seconds. During this time an EA process runs in the background and occupies one license seat.
- The assistant saves large diagrams in batches; a very long answer may exceed the token limit (the "Max tokens" setting).
- When one operation of a save fails, the earlier operations of the same save stay in the model; the message says what was saved.
- Italics in the assistant's answers are not formatted.
- On a monitor with different scaling than the primary one, the window is slightly less sharp.
- When the EA window is not open or shows a different model, changes saved by the assistant become visible in EA after refreshing the package (F5).
- Proxies with a PAC script and NTLM/Kerberos authentication on the proxy are not supported.
- The TLS encryption in the settings applies to the assistant's reading of the model; the connection of EA itself to the database depends on EA's settings.
- The installer is not digitally signed yet, so Windows SmartScreen may show a warning the first time it is run.

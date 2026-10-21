# Changelog

All notable changes to Loonar Compass (named Loonar EA Assistant until version 0.3).

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

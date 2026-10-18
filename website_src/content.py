# -*- coding: utf-8 -*-
# Texts for loonar.eu (Loonar Compass page). Values are HTML-safe fragments.

ICONS = {
 "ask": '<path d="M4 5h16v11H9l-5 4z"/><path d="M8 9h8M8 12h5"/>',
 "edit": '<rect x="3" y="3" width="8" height="8" rx="1.5"/><rect x="13" y="13" width="8" height="8" rx="1.5"/><path d="M11 7h4a2 2 0 0 1 2 2v4"/>',
 "bpmn": '<circle cx="4.5" cy="12" r="2.5"/><rect x="9" y="9" width="6" height="6" rx="1.5"/><path d="M19.5 9.5l2.5 2.5-2.5 2.5-2.5-2.5z"/><path d="M7 12h2M15 12h2"/>',
 "layout": '<path d="M4 6h7v12h9"/><circle cx="4" cy="6" r="1.6"/><circle cx="20" cy="18" r="1.6"/><path d="M14 4h6M14 8h6"/>',
 "lock": '<rect x="5" y="10" width="14" height="10" rx="2"/><path d="M8 10V7a4 4 0 0 1 8 0v3"/><path d="M12 14v2"/>',
 "check": '<circle cx="12" cy="12" r="9"/><path d="M8 12.5l2.7 2.7L16 10"/>',
 "impact": '<circle cx="6" cy="12" r="2.5"/><circle cx="18" cy="5" r="2"/><circle cx="18" cy="12" r="2"/><circle cx="18" cy="19" r="2"/><path d="M8.3 11l7.8-5M8.5 12h7.5M8.3 13l7.8 5"/>',
 "window": '<rect x="3" y="4" width="18" height="16" rx="2"/><path d="M3 8h18M7 12h4v5H7zM14 12h3M14 15h3"/>',
 "doc": '<path d="M7 3h7l5 5v13H7z"/><path d="M14 3v5h5M10 13h6M10 17h6"/>',
 "page": '<path d="M5 4h14v16H5z"/><path d="M8 8h8M8 11h8M8 14h4"/><path d="M15 17l2-2 2 2M17 15v6"/>',
 "memory": '<path d="M6 3h12v18l-6-4-6 4z"/><path d="M9 8h6"/>',
 "lang": '<circle cx="12" cy="12" r="9"/><path d="M3 12h18M12 3c2.5 2.6 3.8 5.6 3.8 9s-1.3 6.4-3.8 9c-2.5-2.6-3.8-5.6-3.8-9S9.5 5.6 12 3z"/>',
 "local": '<rect x="3" y="4" width="18" height="12" rx="2"/><path d="M8 20h8M12 16v4"/>',
 "key": '<circle cx="8" cy="15" r="4"/><path d="M11 12l9-9M17 6l2 2M15 8l2 2"/>',
 "send": '<path d="M4 12l16-8-6 16-3-6z"/><path d="M11 14l9-10"/>',
 "dl": '<path d="M12 4v11M7 10l5 5 5-5M5 20h14"/>',
 "wallet": '<rect x="3" y="6" width="18" height="13" rx="2"/><path d="M3 10h18M16 15h2"/>',
 "shield": '<path d="M12 3l7 3v5.5c0 4.3-2.9 7.9-7 9.5-4.1-1.6-7-5.2-7-9.5V6z"/><path d="M9 12l2.2 2.2L15.5 10"/>',
 "heart": '<path d="M12 20s-7.5-4.6-7.5-10.1A4.2 4.2 0 0 1 12 7.4a4.2 4.2 0 0 1 7.5 2.5C19.5 15.4 12 20 12 20z"/>',
}
# Brand marks (filled, monochrome): (viewBox, SVG content) - the GitHub mark and the LinkedIn "in" logo.
MARKS = {
 "book": ("0 0 24 24", '<path fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" d="M4 19.5A2.5 2.5 0 0 1 6.5 17H20V3H6.5A2.5 2.5 0 0 0 4 5.5v14zM4 19.5A2.5 2.5 0 0 0 6.5 22H20v-5"/>'),
 "github": ("0 0 16 16", '<path d="M8 0C3.58 0 0 3.58 0 8c0 3.54 2.29 6.53 5.47 7.59.4.07.55-.17.55-.38 0-.19-.01-.82-.01-1.49-2.01.37-2.53-.49-2.69-.94-.09-.23-.48-.94-.82-1.13-.28-.15-.68-.52-.01-.53.63-.01 1.08.58 1.23.82.72 1.21 1.87.87 2.33.66.07-.52.28-.87.51-1.07-1.78-.2-3.64-.89-3.64-3.95 0-.87.31-1.59.82-2.15-.08-.2-.36-1.02.08-2.12 0 0 .67-.21 2.2.82.64-.18 1.32-.27 2-.27.68 0 1.36.09 2 .27 1.53-1.04 2.2-.82 2.2-.82.44 1.1.16 1.92.08 2.12.51.56.82 1.27.82 2.15 0 3.07-1.87 3.75-3.65 3.95.29.25.54.73.54 1.48 0 1.07-.01 1.93-.01 2.2 0 .21.15.46.55.38A8.01 8.01 0 0 0 16 8c0-4.42-3.58-8-8-8z"/>'),
 "linkedin": ("0 0 24 24", '<path d="M20.45 20.45h-3.55v-5.57c0-1.33-.03-3.04-1.85-3.04-1.86 0-2.14 1.45-2.14 2.94v5.67H9.35V9h3.41v1.56h.05c.48-.9 1.64-1.85 3.37-1.85 3.6 0 4.27 2.37 4.27 5.46v6.28zM5.34 7.43a2.06 2.06 0 1 1 0-4.13 2.06 2.06 0 0 1 0 4.13zm1.78 13.02H3.56V9h3.56v11.45zM22.22 0H1.77C.79 0 0 .77 0 1.73v20.54C0 23.23.79 24 1.77 24h20.45c.98 0 1.78-.77 1.78-1.73V1.73C24 .77 23.2 0 22.22 0z"/>'),
}
FEATURE_ICONS = ["ask","edit","bpmn","layout","lock","check","impact","window","doc","page","memory","lang"]
SEC_ICONS = ["local","key","shield","lock","send","wallet"]
# FAQ: anchor ids (same in every language), in the order of the `faq` lists below.
# Every answer is checked against the code, the user guide and the licence of the current release;
# re-check them when the application changes. Placeholders: {lic} = link to the licence file,
# {ver} = version, {setup} = installer file name, {repo} = GitHub repository, {releases} = GitHub releases page
# (all from build.py).
FAQ_IDS = ["faq-free", "faq-cost", "faq-windows", "faq-ea", "faq-admin", "faq-smartscreen", "faq-safety", "faq-data",
           "faq-storage", "faq-encryption", "faq-proxy", "faq-languages", "faq-confluence", "faq-update", "faq-uninstall",
           "faq-contact"]

T = {}

T["en"] = dict(
 locale="en_US", name_lang="English",
 title="Loonar Compass – AI assistant for Sparx Enterprise Architect",
 desc="Freeware Windows app: talk to your Sparx Enterprise Architect model in plain language. Ask questions, create and change diagrams through EA, approve every change. Uses Anthropic’s Claude API with your own API key.",
 og_desc="Talk to your Enterprise Architect model: ask, draw, document — every change written by EA and approved by you.",
 skip="Skip to content", lang_label="Language",
 nav=["Features","How it works","Security","Requirements","FAQ","Support"], nav_dl="Download",
 eyebrow="Freeware · Windows · Sparx Enterprise Architect",
 h1='AI assistant for Sparx Enterprise Architect — <em>talk to your model</em>',
 lead="Ask questions in plain language, have diagrams drawn and reworked, and turn the model into documents. Every change is written by Enterprise Architect itself — and only after you approve it.",
 cta1="Download free", cta2="See what it can do",
 meta="Version <a href=\"{changelog}\">{ver}</a> · Windows 10/11 · uses Anthropic’s Claude API with your own API key",
 hero_alt="Illustration of the Loonar Compass window: a request to draw an online shop order process as BPMN, the list of steps and the assistant's answer.",
 f_h="What it does", f_sub="One conversation instead of dozens of dialogs — from a quick question to a complete, consistently styled diagram.",
 features=[
  ("Ask your model","Questions in natural language: what is in a package, which elements realise a requirement, what has changed. The assistant queries the model — a PostgreSQL repository or a .qea/.qeax file — read-only, and names objects by name, package and GUID. Each conversation can work with its own model."),
  ("Create and change","Packages, elements, diagrams and connectors; class attributes and operations with parameters; keywords, colours, position and size. Move, rename, delete — or copy a whole package with its contents."),
  ("BPMN the way EA expects it","Proper BPMN 2.0 types with stereotypes and tags: tasks, events, gateways, flows, collapsed and embedded subprocesses. Drawn from a BPMN file, a photo, a sketch or just a description."),
  ("Diagrams that read well","Legends with automatic colouring, line styles and connector routing along a path you give, connectors hidden on one diagram only, navigation cells that link to other diagrams."),
  ("Written by EA itself","Changes go through EA’s own automation interface (COM) — when the model uses security, under an EA user account, so its permissions and locks apply. Before a bigger restructuring the assistant is instructed to take a baseline first."),
  ("You approve every change","Three approval modes: ask, plan and auto. Every save, failed ones included, lands in the Audit log with time, account and result — and exports to Excel."),
  ("Impact analysis","Follow relationships from an element up to six levels deep, with paths and the diagrams affected. See where a service operation is used on sequence and activity diagrams — and which operations are used nowhere."),
  ("Works with your open EA","Knows which diagram is open and what is selected, opens diagrams, highlights elements in the Project Browser and refreshes whatever it changed — no F5 needed."),
  ("Word, Excel and images","Word documents with diagrams from EA, Excel workbooks (element lists, matrices, impact results) and PNG images such as a use case–requirement matrix."),
  ("Confluence pages","Pages built from the model and the conversation: code for the Source Editor, or published through the REST API after your approval, with diagrams as attachments. Driven by document templates you can edit."),
  ("Assistant memory","Remembers what you agree on — modelling conventions, preferences, corrections — as local Markdown files, and applies it in every new conversation, whichever model it works with. A note is limited to one model only when you ask for it."),
  ("Four languages","English, Polish, German and French: the application, the assistant’s answers, the document templates and the user guide."),
 ],
 h_h="How it works", h_sub="You stay in charge at every step.",
 steps=[
  ("You ask","Type a question or a task; attach a PDF, a screenshot, a sketch or a BPMN file."),
  ("Claude plans","The Claude model decides what to look up and what to change. You follow every step live in the chat."),
  ("It reads and writes the model","Reads straight from the PostgreSQL repository or the .qea/.qeax file (read-only); writes only through Enterprise Architect via COM."),
  ("You approve","See the planned changes, then approve or reject. Saved changes are logged and show up in your EA window."),
 ],
 s_h="A look inside", s_sub="The interface with example content from a fictional online shop.",
 caps=["Every save waits for your approval (mode ask).","Diagrams come back to the chat as images — exactly as EA draws them.","The Audit log records every save — export it to Excel."],
 sec_h="Security and privacy", sec_sub="Your model stays where it is. The assistant works on your computer, with your accounts.",
 sec=[
  ("Runs locally","A desktop application for Windows, installed for your user account. Settings, conversation history, memory and the Audit log are stored in your user profile."),
  ("Secrets stay in Windows","The Claude API key, the EA account password, the database password and the Confluence token are kept in Windows Credential Manager — never in plain text in the settings file."),
  ("Encrypted connections","The Claude API and Confluence are used only over HTTPS. The connection to PostgreSQL can be encrypted with TLS — preferred, required, or required with the server certificate checked against the Windows certificate store. A model file (.qea, .qeax) needs no database connection: the assistant reads it directly, read-only."),
  ("Writes only through EA","The database session is read-only, and a model file is opened read-only. Every change is made by Enterprise Architect — when the model uses security, under the EA account you set up, with that account’s permissions and locks. Content of attachments, documents and the model is treated as data, not as instructions."),
  ("What is sent to Anthropic","To answer, Claude receives the conversation, your attachments, remembered notes, templates in use, the results of the assistant’s queries and the diagram images it shows — not the whole model. Check your organisation’s rules before using it on company models."),
  ("Your own API key","The app uses your Claude API key from the Anthropic Console. The app itself is free; Anthropic bills Claude usage to the account the key belongs to."),
 ],
 r_h="Requirements", r_need="Required", r_opt="Optional",
 req=[
  "Windows 10 or 11 (64-bit)",
  "Sparx Enterprise Architect with a licence — to save changes, the assistant starts EA in the background (it uses a licence seat while it runs) or, for a model file you have open, uses your EA window",
  "An EA model: a .qea or .qeax project file, or a PostgreSQL database with security enabled (“Require User Lock to Edit”)",
  "For a database model: access to the database (server, port, name, account) and an EA user account for the assistant with edit rights; a model file needs only its path (and the account only if security is enabled)",
  "A Claude API key from the Anthropic Console and an internet connection",
 ],
 opt=[
  "Confluence (Data Center or Cloud) — to publish pages directly",
  "A default mail program in Windows — for bug reports and release news",
 ],
 r_note="Older file formats (.eap, .eapx, .feap) and other databases are not supported; EA converts an .eap/.eapx file to .qea with a project transfer (Settings → Model → Transfer → Full Project Transfer).",
 faq_h="FAQ", faq_sub="Straight answers about cost, requirements, safety and privacy.",
 faq=[
  ("Is it really free? Can I use it at work?",
   '<p>Yes. Loonar Compass is freeware: free of charge for any purpose, including professional and commercial use, on any number of computers. You may also pass it on to others — free of charge, complete and unchanged, together with the licence file.</p>'
   '<p>The source code is public on <a href="{repo}">GitHub</a>, so you can see exactly what the application does with your model. It is source-available, not open source: modifying the application, creating derivative works, selling it or renting it out is not allowed. <a href="{lic}">Read the licence</a>.</p>'),
  ("What does it cost to run, and which Claude model does it use?",
   '<p>The application is free. The assistant works with your own Claude API key from the Anthropic Console, and Anthropic bills its use to the account the key belongs to — pay as you go, by the amount of text the model reads and writes. Current prices are on <a href="https://claude.com/pricing#api">Anthropic’s pricing page</a>.</p>'
   '<p>The model is always Claude Haiku 5.5 (<code>claude-haiku-5-5</code>); it is not a setting. “Max tokens” (10,000 by default) only caps the length of a single answer: you pay for the text actually generated.</p>'),
  ("Which operating systems does it run on?",
   '<p>Windows 10 and Windows 11, 64-bit. It is a native Windows desktop application and works together with Enterprise Architect installed on the same computer.</p>'),
  ("Which Enterprise Architect setups are supported?",
   '<ul><li>Enterprise Architect installed on the same computer, with a valid licence. To save changes, the assistant starts it in the background, where it takes one licence seat; it closes again after 10 minutes without work and when you close the application. If the model file is open in your EA window, the changes go through that window instead — a second EA never opens the same file.</li>'
   '<li>The model as an EA project file .qea or .qeax — in Settings, “2. Model” → “EA file (.qea, .qeax)” and its path in “Model file”, with no database server, account or password — or in a PostgreSQL database that your computer can reach (server, port, database name, account). A .qea file is meant for one person, .qeax for a small team sharing the file (Sparx recommends it for fewer than 5 users). Older file formats (.eap, .eapx, .feap) and repositories in other databases are not supported; Enterprise Architect converts an .eap/.eapx file to .qea with a project transfer (Settings → Model → Transfer → Full Project Transfer).</li>'
   '<li>For a model in a database: user security enabled (Settings → Security → Enable Security), in “Require User Lock to Edit” mode. In a model file, security is optional; without it, the assistant needs no EA account.</li>'
   '<li>With security enabled: an EA user account for the assistant, with a password and edit rights — ideally a separate one, so that its changes and locks are easy to tell from yours.</li><li>Several models: every conversation has its own model, shown in its header (“Model: …”). A new conversation uses the default from Settings; “Choose another model…” — or a click on the header, which opens “Model of this conversation” — lets you pick another PostgreSQL database (server, port, database, account, password, encryption) or EA file (.qea, .qeax), with its EA account, and test the connection. Dropping a .qea/.qeax file on the window makes it the model of the conversation. Conversations from earlier versions get a copy of the model from Settings. Enterprise Architect in the background keeps one model open at a time, so switching it to another model takes from a few seconds to about 25 seconds.</li></ul>'
   '<p>The application does not check for a particular EA version or edition. What matters is that your Enterprise Architect can open this model: the file, or the database repository with security enabled.</p>'),
  ("Do I need administrator rights to install it?",
   '<p>No. By default (“Just for me”) the installer installs Loonar Compass for your user account only, in <code>%LOCALAPPDATA%\\Programs\\Loonar Compass</code>, and needs no administrator rights. If you choose “For all users of this computer” instead (in C:\\Program Files\\Loonar Compass), the installer asks for administrator rights.</p>'
   '<p>The application itself always runs as a normal user, and every user has their own settings and conversations. If someone else approves an installation for all users with an administrator account, enter your configuration later in the application (Settings), not in the installer — otherwise it ends up in the administrator’s profile.</p>'),
  ("Why may Windows SmartScreen warn me about the installer?",
   '<p>Because the installer of version {ver} is not digitally signed yet (with a code signing certificate), so Windows does not know its publisher. Microsoft Defender SmartScreen may therefore show “Windows protected your PC” when you start it, and your browser may warn that the file is not commonly downloaded. The default installation for your user account needs no administrator rights, so no administrator prompt appears; only an installation for all users asks for them — and shows the publisher as unknown. A signed installer is planned and should follow soon. Until then:</p>'
   '<ul><li>Download the installer only from loonar.eu or from the project’s <a href="{releases}">releases on GitHub</a>, over HTTPS.</li>'
   '<li>Compare its checksum with the SHA-256 shown next to the download button — in PowerShell: <code>Get-FileHash .\\{setup}</code>.</li>'
   '<li>In the SmartScreen window, click “More info” and then “Run anyway”.</li></ul>'
   '<p>On Windows 11 with Smart App Control turned on, unsigned applications are blocked and cannot be started this way; in that case, please wait for the signed version.</p>'),
  ("Can the assistant damage my model?",
   '<p>It saves only what you allow, and only in the way Enterprise Architect allows:</p>'
   '<ul><li><strong>Approval modes:</strong> ask — you approve every save; plan — one approval per answer; auto — no questions, for test models. Publishing to Confluence always needs your approval, and the request says explicitly when an existing page will be replaced.</li>'
   '<li><strong>Writes only through EA:</strong> the database is read in a read-only session, a model file read-only; every change is made by Enterprise Architect — with security enabled, under the assistant’s EA account, with that account’s permissions and locks. Objects locked by other users are left alone.</li>'
   '<li><strong>Instructions only from you:</strong> the assistant is instructed to treat the content of attachments, documents and the model as data, not as instructions to follow.</li>'
   '<li><strong>Audit log:</strong> every save, failed ones included, is recorded with time, account and result, and can be exported to Excel.</li>'
   '<li><strong>Baselines:</strong> before deleting or substantially restructuring an existing package, the assistant is instructed to save a baseline of it first; you restore it in EA (Manage Baselines → Compare/Restore).</li></ul>'
   '<p>Still, deleting is permanent and AI can make mistakes — the application reminds you of this, so review the proposed changes before you approve them. For important models use ask mode and keep backups of the database or the model file — the application comes without warranty.</p>'),
  ("What data leaves my computer, and where does it go?",
   '<p>To answer, the assistant sends only what it needs to Anthropic’s Claude API (api.anthropic.com): your messages and attachments, remembered notes, the templates in use, and what it has read from the model in that conversation — the results of its queries to the model (names and descriptions of packages, elements, diagrams) and images of the diagrams it showed. Not the whole model. After a language change, memory and templates are also sent for translation.</p>'
   '<p>Beyond that, data goes only where you send it: published pages to your Confluence; bug reports and newsletter sign-ups are e-mails you check and send yourself from your mail program. The application sends nothing to Loonar on its own. Before using it on company models, check your organisation’s rules and the data terms of your Anthropic account.</p>'),
  ("Where are my settings, conversations and passwords stored?",
   '<p>On your computer, in your Windows user profile — every user of the computer has their own:</p>'
   '<ul><li><code>%APPDATA%\\Loonar Compass</code>: settings (app.properties), conversation history, attachments, assistant memory, document templates and the audit log.</li>'
   '<li>Windows Credential Manager: the Claude API key, the EA account password, the database password and the Confluence token, as well as the passwords of a conversation’s model that differs from Settings. None of them is kept in plain text in the settings file.</li></ul>'
   '<p>The program itself is installed in <code>%LOCALAPPDATA%\\Programs\\Loonar Compass</code> by default, or in C:\\Program Files\\Loonar Compass when installed for all users.</p>'),
  ("Is the communication encrypted?",
   '<p>Yes, wherever the application connects by itself:</p>'
   '<ul><li><strong>Claude API and Confluence:</strong> only over HTTPS. A Confluence address starting with <code>http://</code> is accepted only for a server on the same computer (localhost).</li>'
   '<li><strong>Model database (PostgreSQL):</strong> TLS, set with “Connection encryption” on the “2. Model” card in Settings (option “PostgreSQL database”): “Preferred” (default) — TLS whenever the server offers it, otherwise an unencrypted connection; “Required” — TLS or no connection; “Required, certificate verified” — TLS, and the server’s certificate and name are checked against the Windows certificate store, including your company’s certificates. “Preferred” and “Required” do not check the certificate. After “Save and test connection”, the application tells you whether the connection is encrypted. In app.properties the setting is <code>db.sslmode</code> = <code>prefer</code>, <code>require</code> or <code>verify-full</code>.</li>'
   '<li><strong>Model file (.qea, .qeax):</strong> no database connection — the assistant reads the file directly, read-only.</li></ul>'
   '<p>Tip for database administrators: with <code>hostssl</code> entries in pg_hba.conf, the server accepts only TLS connections from all clients. Enterprise Architect’s own connection to the database follows EA’s settings, not the application’s.</p>'),
  ("Does it work behind a corporate proxy or firewall?",
   '<p>Allow these connections: HTTPS (port 443) to api.anthropic.com, for a database model your PostgreSQL server (port 5432 by default; a direct connection, not through the proxy) and, if you publish pages, your Confluence.</p>'
   '<p>For HTTPS, the application uses the proxy set in the Windows Internet settings (system proxy). You can also set the <code>HTTPS_PROXY</code> or <code>ALL_PROXY</code> environment variable, e.g. <code>http://proxy.example.com:8080</code> (exceptions in <code>NO_PROXY</code>); it takes precedence over the system proxy. Restart the application after a change. For basic authentication, the user name and password can be part of the address in that variable; automatic detection and configuration scripts (WPAD, PAC) and proxy sign-in with your Windows account (NTLM, Kerberos) are not supported.</p>'
   '<p>HTTPS certificates are also checked against the Windows certificate store, so a company root certificate used for traffic inspection is accepted once it is installed in Windows.</p>'),
  ("Which languages are available?",
   '<p>English, Polish, German and French: the application and installer windows, the assistant’s answers, the default document templates, the user guide, the licence and the changelog. The installer starts with a language choice (English by default); later you change it in Settings, on the “4. Language and change approval” card.</p>'
   '<p>The assistant answers in the application’s language unless you write to it in another one. Conversation history is never translated.</p>'),
  ("Does it work with Confluence Cloud and Data Center?",
   '<p>Yes, with both — and Confluence is optional. Without it, the assistant still prepares pages: you paste the code into the source editor yourself (“Copy page code”, “Open images folder”).</p>'
   '<p>To publish directly, fill in the “6. Confluence” card in Settings: for Data Center the address and a personal access token; for Cloud the address (https://your-company.atlassian.net/wiki), the account’s e-mail and an API token; plus the space key and, optionally, a parent page. The assistant then creates or updates pages through the REST API, with diagrams as attachments — always after your approval (the request says explicitly when an existing page will be replaced; the previous version stays in the page history) and with the permissions of that token.</p>'),
  ("How do I update? Are my settings and conversations kept?",
   '<p>Close the application and run the installer of the newer version. It suggests the same folder, shows what has changed since your version and keeps your settings, conversation history, attachments, memory, templates and audit log; the key, passwords and tokens in Credential Manager stay as well. New versions are published here and on <a href="{releases}">GitHub</a>.</p>'),
  ("How do I uninstall it, and what stays on my computer?",
   '<p>Close the application and use “Uninstall Loonar Compass” in the Start menu or its entry in Windows Settings → Apps → Installed apps. This removes the program files, the shortcuts and the system entry. Your data stays unless you tick one of these options:</p>'
   '<ul><li>“Remove settings, conversations, attachments, memory, templates and the audit log” — this deletes the whole <code>%APPDATA%\\Loonar Compass</code> folder;</li>'
   '<li>“Remove the API key, passwords and tokens from Credential Manager” — the Claude API key, the EA account and database passwords and the Confluence token.</li></ul>'
   '<p>Your EA model, files you saved elsewhere and pages published to Confluence are not affected.</p>'),
  ("How do I report a bug or hear about new versions?",
   '<p>In the application, open “Report a bug” in the navigation bar, describe what happened, add screenshots and click “Send report”: a ready e-mail to mariusz@loonar.eu opens in your default mail program, where you check it and send it. Technical information (application version, Windows version, AI model, approval mode, last message — no passwords or keys) is added only while that option is ticked (it is by default).</p>'
   '<p>For release news, enter your e-mail on the “5. New releases (newsletter)” card in Settings and tick the consent. Or simply write to <a href="mailto:mariusz@loonar.eu">mariusz@loonar.eu</a>.</p>'),
 ],
 d_h="Download", d_sub="Free for everyone — download, install, connect your model.",
 d_meta="Windows installer · {size} MB · {date}",
 d_btn="Download installer", d_hash="SHA-256 checksum:",
 d_lic="Freeware: free for any use, including commercial. Modifying the application is not allowed.",
 d_lic_link="Read the licence",
 d_lang="The installer starts with a language choice: English (default), Polish, German or French.",
 d_update="To update, just run a newer installer — your settings and history are kept.",
 d_admin="Installs for your user account — no administrator rights needed.",
 gh_label="Source code on GitHub", d_all="All releases",
 sup_h="Support the project",
 sup_text="Loonar Compass is free, also for commercial use, and it will stay free. If it saves you time, you can support its development with a voluntary contribution. It pays, among other things, for a code-signing certificate, so that Windows recognises the application as coming from a verified publisher.",
 sup_btn="Support on suppi.pl",
 sup_more='Other ways to help: report bugs and ideas (“Report a bug” in the app), recommend the application to other Enterprise Architect users, or star the project on <a href="{repo}">GitHub</a>.',
 ab_h="About me",
 ab_why_h="Why this application exists",
 ab_why="AI is great at producing diagrams as code — PlantUML, Mermaid. But that is diagramming, not modelling: each such diagram is a momentary view, and the knowledge about the system is not kept anywhere. Simon Brown, the author of the C4 model, draws exactly this line between diagrams and models: a model is a single source of truth about elements and their relationships, from which many consistent views are derived — and that is what makes governance possible, such as impact analysis and consistency checks. Enterprise Architect keeps such a model. Loonar Compass lets you work with that model through a conversation with AI.",
 ab_name_h="Why “Compass”",
 ab_name="A compass gives you orientation. The logo is a constellation of nodes: like a navigator finding the way by the stars, the assistant helps you find your way in a large architecture model.",
 ab_role="Principal Technical Product Manager · requirements engineering trainer (IREB CPRE)",
 ab_me_h="Who is behind it",
 ab_me="I’m a Principal Technical Product Manager from Stargard, Poland, with years of experience building digital products — product strategy and the full lifecycle, from market research, customer analysis and requirements to delivery — for cloud and telecommunication services, including integrating AWS, Azure and Google Cloud with on-premise systems. I graduated in computer science from the Szczecin University of Technology and train requirements engineering (IREB CPRE). I’m not a programmer: Loonar Compass was built with AI coding assistants — I decide what the application must do and how it should behave for architects, and the code was written by AI. That is its strength: it is made by someone who works with Enterprise Architect models every day and knows what architects need.",
 ab_li="Mariusz Głowacki on LinkedIn",
 c_h="Contact", c_text='Questions, ideas or a bug? Write to <a href="mailto:mariusz@loonar.eu">mariusz@loonar.eu</a> — or use “Report a bug” in the app.',
 f_addr="ul. Gombrowicza 12, 73-110 Stargard, Poland",
 f_tm="Sparx Systems and Enterprise Architect are registered trademarks of Sparx Systems Pty Ltd. Claude and Anthropic are trademarks of Anthropic, PBC. Atlassian and Confluence are trademarks of Atlassian. Microsoft, Windows, Word, Excel, GitHub and LinkedIn are trademarks of the Microsoft group of companies. PostgreSQL is a registered trademark of the PostgreSQL Community Association. Loonar Compass is an independent product and is not affiliated with, endorsed or sponsored by these companies.",
 f_stats="No cookies. Anonymous visit statistics (GoatCounter) that keep no personal data. Your language choice is remembered in your browser.",
 f_privacy_link="Privacy", f_legal_link="Legal notice",
 f_top="Back to top",
 # privacy policy / legal notice pages
 doc_updated="Last updated:", doc_home="Back to the home page",
 gc_off="Do not count my visits in this browser", gc_on="Count my visits again",
 gc_state_on="Visits from this browser are counted (anonymously).", gc_state_off="Visits from this browser are not counted.",
 # 404 page
 nf_title="Page not found", nf_text="The page you are looking for does not exist or has moved.", nf_link="Go to the home page",
 # mock-up
 m_today="Today",
 m_convs=["Order process (BPMN)","Requirements from a PDF","Impact: payment service"],
 m_user="Draw the order process of our online shop as BPMN in the <b>Sales</b> package. Make payment an embedded subprocess.",
 m_steps_h="Steps in the last answer: 3",
 m_steps=["Claude is analyzing and planning (step 1)","Reading the model database: package Sales","Saving to EA, 14 operations: diagram Order process"],
 m_bot="Done. The <b>Sales</b> package has a new BPMN diagram <b>Order process</b>: 3 tasks, a gateway and the embedded subprocess <b>Payment</b> with 3 steps. I have opened it in your EA window.",
 m_ph="Write a message, drag in files or paste an image (Ctrl+V)…",
 m_mode="claude-haiku-5-5 · mode ask",
 m_appr_h="Claude wants to save changes to the model",
 m_appr=["New BPMN diagram “Order process” in Sales","3 tasks, 1 gateway, start and end events","Embedded subprocess “Payment” with 3 steps","9 sequence flows"],
 m_approve="Approve", m_reject="Reject",
 m_saveas="Save as…", m_full="Open full size", m_png="Order process.png",
 d_recv=["Receive","order"], d_gw="In stock?", d_pay="Payment",
 dg_sub=[["Choose","method"],["Authorise"],["Confirm"]],
 d_ship=["Ship","order"], d_notify=["Notify","customer"], d_yes="yes", d_no="no",
 m_log_h="Audit log", m_log_btn="Export to Excel",
 m_log_cols=["Time","Result","EA account","Changes"],
 m_log=[("14:02","OK","Order process: 14 operations"),("13:47","OK","Customer: 3 attributes, 2 operations"),("13:21","ERROR","Shipping: package locked by another user"),("11:05","OK","Legend on the Checkout diagram")],
 m_err="ERROR",
 banner=None,
)

T["pl"] = dict(
 locale="pl_PL", name_lang="Polski",
 title="Loonar Compass – asystent AI do Sparx Enterprise Architect",
 desc="Darmowa aplikacja dla Windows: rozmawiaj z modelem Sparx Enterprise Architect zwykłym językiem. Pytaj, twórz i zmieniaj diagramy przez EA, zatwierdzaj każdą zmianę. Korzysta z Claude API firmy Anthropic (Twój klucz API).",
 og_desc="Porozmawiaj ze swoim modelem w Enterprise Architect: pytaj, rysuj, dokumentuj — każdą zmianę zapisuje EA, a zatwierdzasz Ty.",
 skip="Przejdź do treści", lang_label="Język",
 nav=["Funkcje","Jak to działa","Bezpieczeństwo","Wymagania","FAQ","Wesprzyj"], nav_dl="Pobierz",
 eyebrow="Freeware · Windows · Sparx Enterprise Architect",
 h1='Asystent AI do Sparx Enterprise Architect — <em>porozmawiaj ze swoim modelem</em>',
 lead="Zadawaj pytania zwykłym językiem, zlecaj rysowanie i przebudowę diagramów, zamieniaj model w dokumenty. Każdą zmianę zapisuje sam Enterprise Architect — i to dopiero wtedy, gdy ją zatwierdzisz.",
 cta1="Pobierz za darmo", cta2="Zobacz, co potrafi",
 meta="Wersja <a href=\"{changelog}\">{ver}</a> · Windows 10/11 · korzysta z Claude API firmy Anthropic (Twój klucz API)",
 hero_alt="Ilustracja okna Loonar Compass: prośba o narysowanie procesu zamówienia w sklepie internetowym w BPMN, lista kroków i odpowiedź asystenta.",
 f_h="Co potrafi", f_sub="Jedna rozmowa zamiast dziesiątek okien dialogowych — od szybkiego pytania po kompletny, spójnie sformatowany diagram.",
 features=[
  ("Pytaj o model","Pytania zwykłym językiem: co jest w pakiecie, które elementy realizują wymaganie, co się zmieniło. Asystent odpytuje model — repozytorium w PostgreSQL albo plik .qea/.qeax — tylko do odczytu i wskazuje obiekty po nazwie, pakiecie i GUID. Każda rozmowa może pracować z własnym modelem."),
  ("Twórz i zmieniaj","Pakiety, elementy, diagramy i połączenia; atrybuty i operacje klas z parametrami; słowa kluczowe, kolory, położenie i rozmiar. Przenoszenie, zmiana nazwy, usuwanie — i kopiowanie całego pakietu z zawartością."),
  ("BPMN tak, jak oczekuje EA","Poprawne typy BPMN 2.0 ze stereotypami i tagami: zadania, zdarzenia, bramki, przepływy, podprocesy zwinięte i osadzone. Z pliku BPMN, ze zdjęcia, ze szkicu albo z samego opisu."),
  ("Czytelne diagramy","Legendy z automatycznym kolorowaniem, style linii i prowadzenie połączeń po zadanej trasie, ukrywanie połączeń tylko na jednym diagramie, kafelki nawigacyjne do innych diagramów."),
  ("Zapisuje sam EA","Zmiany idą przez interfejs automatyzacji EA (COM) — w modelu z zabezpieczeniami na koncie użytkownika modelu, więc obowiązują jego uprawnienia i blokady. Przed większą przebudową asystent ma polecenie zrobić baseline pakietu."),
  ("Każdą zmianę zatwierdzasz","Trzy tryby zatwierdzania: ask, plan i auto. Każdy zapis, także nieudany, trafia do dziennika zmian z czasem, kontem i wynikiem — z eksportem do Excela."),
  ("Analiza wpływu","Od elementu wzdłuż relacji do sześciu poziomów w głąb, ze ścieżkami i diagramami, na których leżą dotknięte elementy. Gdzie używana jest operacja usługi na diagramach sekwencji i aktywności — i które operacje nie są używane nigdzie."),
  ("Współpracuje z otwartym EA","Wie, który diagram masz otwarty i co jest zaznaczone, otwiera diagramy, podświetla elementy w przeglądarce projektu i sam odświeża to, co zmienił — bez F5."),
  ("Word, Excel i obrazki","Dokumenty Word z diagramami z EA, skoroszyty Excel (listy elementów, macierze, wyniki analizy wpływu) i obrazki PNG, np. macierz przypadków użycia i wymagań."),
  ("Strony Confluence","Strony z treści modelu i rozmowy: kod do edytora źródła (Source Editor) albo publikacja przez REST API po Twoim zatwierdzeniu, z diagramami jako załącznikami. Według szablonów dokumentów, które możesz zmieniać."),
  ("Pamięć asystenta","Zapamiętuje ustalenia — konwencje modelowania, preferencje, poprawki — w lokalnych plikach Markdown i stosuje je w każdej nowej rozmowie, niezależnie od modelu. Ustalenie dotyczy tylko jednego modelu wtedy, gdy o to poprosisz."),
  ("Cztery języki","Polski, angielski, niemiecki i francuski: aplikacja, odpowiedzi asystenta, szablony dokumentów i instrukcja użytkownika."),
 ],
 h_h="Jak to działa", h_sub="Na każdym kroku decydujesz Ty.",
 steps=[
  ("Pytasz","Wpisujesz pytanie albo zadanie, dołączasz PDF, zrzut ekranu, szkic lub plik BPMN."),
  ("Claude planuje","Model Claude decyduje, co sprawdzić i co zmienić. Każdy krok widzisz na bieżąco w czacie."),
  ("Czyta i zapisuje model","Czyta wprost z repozytorium PostgreSQL albo z pliku .qea/.qeax (tylko do odczytu), a zapisuje wyłącznie przez Enterprise Architect (COM)."),
  ("Zatwierdzasz","Widzisz planowane zmiany i je zatwierdzasz albo odrzucasz. Zapis trafia do dziennika zmian i od razu do Twojego okna EA."),
 ],
 s_h="Zajrzyj do środka", s_sub="Interfejs z przykładową treścią — fikcyjny sklep internetowy.",
 caps=["Każdy zapis czeka na Twoje zatwierdzenie (tryb ask).","Diagramy wracają do czatu jako obrazki — dokładnie tak, jak rysuje je EA.","Dziennik zmian zapisuje każdą operację — z eksportem do Excela."],
 sec_h="Bezpieczeństwo i prywatność", sec_sub="Model zostaje tam, gdzie jest. Asystent działa na Twoim komputerze i na Twoich kontach.",
 sec=[
  ("Działa lokalnie","Aplikacja desktopowa dla Windows, instalowana na Twoim koncie użytkownika. Ustawienia, historia rozmów, pamięć i dziennik zmian są w Twoim profilu użytkownika."),
  ("Sekrety zostają w Windows","Klucz API Claude, hasło konta EA, hasło do bazy i token Confluence są w Menedżerze poświadczeń Windows — nigdy zwykłym tekstem w pliku ustawień."),
  ("Szyfrowane połączenia","Z Claude API i Confluence aplikacja łączy się wyłącznie przez HTTPS. Połączenie z PostgreSQL może być szyfrowane TLS — preferowane, wymagane albo wymagane z weryfikacją certyfikatu serwera w magazynie certyfikatów Windows. Plik modelu (.qea, .qeax) nie wymaga połączenia z bazą: asystent czyta go bezpośrednio, tylko do odczytu."),
  ("Zapis tylko przez EA","Sesja bazy jest tylko do odczytu, a plik modelu asystent otwiera tylko do odczytu. Każdą zmianę wykonuje Enterprise Architect — w modelu z zabezpieczeniami na skonfigurowanym koncie EA, z jego uprawnieniami i blokadami. Treść załączników, dokumentów i modelu asystent ma traktować jako dane, a nie polecenia."),
  ("Co trafia do Anthropic","Żeby odpowiedzieć, Claude dostaje treść rozmowy, załączniki, zapamiętane ustalenia, użyte szablony, wyniki zapytań asystenta do modelu i pokazane obrazki diagramów — nie cały model. Przed użyciem na modelu firmowym sprawdź zasady swojej organizacji."),
  ("Twój własny klucz API","Aplikacja korzysta z Twojego klucza API Claude z konsoli Anthropic. Sama aplikacja jest bezpłatna; za użycie Claude Anthropic rozlicza konto, do którego należy klucz."),
 ],
 r_h="Wymagania", r_need="Niezbędne", r_opt="Opcjonalnie",
 req=[
  "Windows 10 lub 11 (64-bitowy)",
  "Sparx Enterprise Architect z licencją — do zapisu zmian asystent uruchamia EA w tle (na czas pracy zajmuje ono jedno stanowisko licencji), a przy pliku modelu otwartym w Twoim EA korzysta z Twojego okna",
  "Model EA w pliku projektu .qea lub .qeax albo w bazie PostgreSQL z włączonymi zabezpieczeniami (tryb „Require User Lock to Edit”)",
  "Dla modelu w bazie: dostęp do bazy (serwer, port, nazwa, konto) i konto użytkownika modelu EA dla asystenta z prawem edycji; dla pliku modelu wystarczy ścieżka (konto tylko przy włączonych zabezpieczeniach)",
  "Klucz API Claude z konsoli Anthropic i połączenie z Internetem",
 ],
 opt=[
  "Confluence (Data Center lub Cloud) — do publikowania stron",
  "Domyślny program pocztowy w Windows — do zgłaszania błędów i informacji o nowych wydaniach",
 ],
 r_note="Starsze formaty plików (.eap, .eapx, .feap) i inne bazy danych nie są obsługiwane; plik .eap/.eapx EA przeniesie do .qea przez transfer projektu (Settings → Model → Transfer → Full Project Transfer).",
 faq_h="Najczęstsze pytania", faq_sub="Krótko i konkretnie o kosztach, wymaganiach, bezpieczeństwie i prywatności.",
 faq=[
  ("Czy to naprawdę za darmo? Mogę używać aplikacji w pracy?",
   '<p>Tak. Loonar Compass to freeware: bezpłatnie do każdego zastosowania, także zawodowego i komercyjnego, na dowolnej liczbie komputerów. Możesz go też przekazywać innym — bezpłatnie, w całości, bez zmian i razem z plikiem licencji.</p>'
   '<p>Kod źródłowy jest publiczny na <a href="{repo}">GitHubie</a>, więc możesz sprawdzić, co dokładnie aplikacja robi z Twoim modelem. To kod udostępniony do wglądu (source-available), a nie open source: nie wolno aplikacji modyfikować ani tworzyć na jej podstawie utworów zależnych, sprzedawać jej ani wynajmować. <a href="{lic}">Przeczytaj licencję</a>.</p>'),
  ("Ile kosztuje korzystanie i z jakiego modelu Claude korzysta aplikacja?",
   '<p>Sama aplikacja jest bezpłatna. Asystent działa na Twoim własnym kluczu API Claude z konsoli Anthropic, a za użycie Anthropic rozlicza konto, do którego należy klucz — według zużycia, czyli ilości tekstu, który model czyta i pisze. Aktualne ceny znajdziesz w <a href="https://claude.com/pricing#api">cenniku Anthropic</a>.</p>'
   '<p>Model to zawsze Claude Haiku 5.5 (<code>claude-haiku-5-5</code>) — nie jest to ustawienie. „Max tokenów” (domyślnie 10\u00a0000) ogranicza tylko długość jednej odpowiedzi: płacisz za tekst faktycznie wygenerowany.</p>'),
  ("Na jakich systemach działa?",
   '<p>Na Windows 10 i Windows 11 w wersji 64-bitowej. To natywna aplikacja desktopowa dla Windows, która współpracuje z Enterprise Architect zainstalowanym na tym samym komputerze.</p>'),
  ("Jakie konfiguracje Enterprise Architect są obsługiwane?",
   '<ul><li>Enterprise Architect zainstalowany na tym samym komputerze, z ważną licencją. Do zapisu zmian asystent uruchamia go w tle i na ten czas EA zajmuje jedno miejsce w licencji; zamyka się po 10 minutach bez pracy i przy zamknięciu aplikacji. Jeśli plik modelu masz otwarty w swoim EA, zmiany idą przez Twoje okno — drugi EA nigdy nie otwiera tego samego pliku.</li>'
   '<li>Model w pliku projektu EA .qea lub .qeax — w Ustawieniach „2. Model” → „Plik EA (.qea, .qeax)” i ścieżka w polu „Plik modelu”, bez serwera bazy, konta i hasła — albo w bazie PostgreSQL dostępnej z Twojego komputera (serwer, port, nazwa bazy, konto). Plik .qea jest dla jednej osoby, .qeax dla małego zespołu, który współdzieli plik (Sparx zaleca go dla mniej niż 5 użytkowników). Starsze formaty plików (.eap, .eapx, .feap) i repozytoria w innych bazach nie są obsługiwane; plik .eap/.eapx Enterprise Architect przeniesie do .qea przez transfer projektu (Settings → Model → Transfer → Full Project Transfer).</li>'
   '<li>Dla modelu w bazie: włączone zabezpieczenia użytkowników (Settings → Security → Enable Security) w trybie „Require User Lock to Edit”. W pliku modelu zabezpieczenia są opcjonalne — bez nich asystent nie potrzebuje konta EA.</li>'
   '<li>Przy włączonych zabezpieczeniach: konto użytkownika EA dla asystenta, z hasłem i prawem edycji — najlepiej osobne, żeby jego zmiany i blokady łatwo było odróżnić od Twoich.</li><li>Kilka modeli: każda rozmowa ma własny model, widoczny w jej nagłówku („Model: …”). Nowa rozmowa korzysta z domyślnego modelu z Ustawień; „Wybierz inny model…” — albo kliknięcie nagłówka, które otwiera „Model tej rozmowy” — pozwala wskazać inną bazę PostgreSQL (serwer, port, baza, konto, hasło, szyfrowanie) albo plik EA (.qea, .qeax), z kontem EA, i sprawdzić połączenie. Plik .qea/.qeax upuszczony na okno staje się modelem rozmowy. Rozmowy z wcześniejszych wersji dostają kopię modelu z Ustawień. Enterprise Architect w tle ma otwarty jeden model naraz, więc przełączenie go na inny model trwa od kilku do około 25 sekund.</li></ul>'
   '<p>Aplikacja nie sprawdza wersji ani edycji EA. Liczy się to, czy Twój Enterprise Architect otwiera ten model: plik albo repozytorium w bazie z włączonymi zabezpieczeniami.</p>'),
  ("Czy do instalacji potrzebne są uprawnienia administratora?",
   '<p>Nie. Domyślnie („Tylko dla mnie”) instalator instaluje Loonar Compass tylko dla Twojego konta użytkownika, w folderze <code>%LOCALAPPDATA%\\Programs\\Loonar Compass</code>, i nie potrzebuje uprawnień administratora. Jeśli zamiast tego wybierzesz „Dla wszystkich użytkowników tego komputera” (w C:\\Program Files\\Loonar Compass), instalator poprosi o uprawnienia administratora.</p>'
   '<p>Sama aplikacja zawsze działa na zwykłym koncie, a każdy użytkownik ma własne ustawienia i rozmowy. Jeśli instalację dla wszystkich użytkowników zatwierdza ktoś inny, z konta administratora, konfigurację podaj później w aplikacji (Ustawienia), a nie w instalatorze — inaczej trafi do profilu administratora.</p>'),
  ("Dlaczego Windows SmartScreen może ostrzegać przed instalatorem?",
   '<p>Bo instalator wersji {ver} nie jest jeszcze podpisany cyfrowo (certyfikatem do podpisywania kodu), więc Windows nie zna jego wydawcy. Dlatego Microsoft Defender SmartScreen może przy uruchomieniu wyświetlić okno „System Windows ochronił ten komputer”, a przeglądarka może ostrzec, że plik nie jest często pobierany. Domyślna instalacja na Twoim koncie nie wymaga uprawnień administratora, więc pytanie o zgodę administratora się nie pojawi; prosi o nie — i pokazuje nieznanego wydawcę — tylko instalacja dla wszystkich użytkowników. Podpisany instalator jest planowany i powinien pojawić się wkrótce. Do tego czasu:</p>'
   '<ul><li>Pobieraj instalator tylko z loonar.eu albo ze strony <a href="{releases}">wydań projektu na GitHubie</a>, przez HTTPS.</li>'
   '<li>Porównaj jego sumę kontrolną z SHA-256 podaną przy przycisku pobierania — w PowerShell: <code>Get-FileHash .\\{setup}</code>.</li>'
   '<li>W oknie SmartScreen kliknij „Więcej informacji”, a potem „Uruchom mimo to”.</li></ul>'
   '<p>W Windows 11 z włączoną Inteligentną kontrolą aplikacji (Smart App Control) niepodpisane aplikacje są blokowane i nie da się ich w ten sposób uruchomić — wtedy poczekaj na wersję podpisaną.</p>'),
  ("Czy asystent może zepsuć mi model?",
   '<p>Zapisuje tylko to, na co pozwolisz, i tylko tak, jak pozwala Enterprise Architect:</p>'
   '<ul><li><strong>Tryby zatwierdzania:</strong> ask — zatwierdzasz każdy zapis; plan — jedno zatwierdzenie na odpowiedź; auto — bez pytania, do modeli testowych. Publikacja w Confluence zawsze wymaga zatwierdzenia, a prośba wyraźnie mówi, kiedy istniejąca strona zostanie zastąpiona.</li>'
   '<li><strong>Zapis tylko przez EA:</strong> bazę asystent czyta w sesji tylko do odczytu, a plik modelu tylko do odczytu; każdą zmianę wykonuje Enterprise Architect — w modelu z zabezpieczeniami na koncie EA asystenta, z jego uprawnieniami i blokadami. Obiektów zablokowanych przez innych użytkowników nie zmieni.</li>'
   '<li><strong>Polecenia tylko od Ciebie:</strong> treść załączników, dokumentów i modelu asystent ma traktować jako dane, a nie jako polecenia do wykonania.</li>'
   '<li><strong>Dziennik zmian:</strong> każdy zapis, także nieudany, trafia do dziennika z czasem, kontem i wynikiem; dziennik wyeksportujesz do Excela.</li>'
   '<li><strong>Baseline:</strong> przed usunięciem albo większą przebudową istniejącego pakietu asystent ma polecenie najpierw zapisać jego baseline; przywracasz go w EA (Manage Baselines → Compare/Restore).</li></ul>'
   '<p>Mimo to usuwanie jest nieodwracalne, a AI może się mylić — aplikacja o tym przypomina, więc przed zatwierdzeniem przejrzyj proponowane zmiany. Przy ważnych modelach używaj trybu ask i rób kopie zapasowe bazy albo pliku modelu — aplikacja jest dostarczana bez gwarancji.</p>'),
  ("Jakie dane opuszczają mój komputer i dokąd trafiają?",
   '<p>Żeby odpowiedzieć, asystent wysyła do API Claude w Anthropic (api.anthropic.com) tylko to, czego potrzebuje: Twoje wiadomości i załączniki, zapamiętane ustalenia, użyte szablony oraz to, co w danej rozmowie odczytał z modelu — wyniki zapytań do modelu (nazwy i opisy pakietów, elementów, diagramów) i obrazki pokazanych diagramów. Nie cały model. Po zmianie języka do tłumaczenia trafiają też pamięć i szablony.</p>'
   '<p>Poza tym dane idą tylko tam, dokąd sam je wyślesz: publikowane strony do Twojego Confluence, a zgłoszenia błędów i zapis na newsletter to e-maile, które sprawdzasz i wysyłasz ze swojego programu pocztowego. Aplikacja sama niczego nie wysyła do Loonar. Przed użyciem na modelu firmowym sprawdź zasady swojej organizacji i warunki przetwarzania danych na Twoim koncie Anthropic.</p>'),
  ("Gdzie są przechowywane ustawienia, rozmowy i hasła?",
   '<p>Na Twoim komputerze, w profilu użytkownika Windows — każdy użytkownik komputera ma własne:</p>'
   '<ul><li><code>%APPDATA%\\Loonar Compass</code>: ustawienia (app.properties), historia rozmów, załączniki, pamięć asystenta, szablony dokumentów i dziennik zmian.</li>'
   '<li>Menedżer poświadczeń Windows: klucz API Claude, hasło konta EA, hasło do bazy i token Confluence, a także hasła modelu rozmowy innego niż w Ustawieniach. Żaden z nich nie jest zapisany zwykłym tekstem w pliku ustawień.</li></ul>'
   '<p>Sam program instaluje się domyślnie w <code>%LOCALAPPDATA%\\Programs\\Loonar Compass</code>, a przy instalacji dla wszystkich użytkowników w C:\\Program Files\\Loonar Compass.</p>'),
  ("Czy komunikacja jest szyfrowana?",
   '<p>Tak, wszędzie tam, gdzie aplikacja łączy się sama:</p>'
   '<ul><li><strong>Claude API i Confluence:</strong> wyłącznie przez HTTPS. Adres Confluence zaczynający się od <code>http://</code> jest akceptowany tylko dla serwera na tym samym komputerze (localhost).</li>'
   '<li><strong>Baza modelu (PostgreSQL):</strong> TLS, ustawiany polem „Szyfrowanie połączenia” na karcie „2. Model” w Ustawieniach (opcja „Baza PostgreSQL”): „Preferowane” (domyślnie) — TLS, gdy serwer go oferuje, w przeciwnym razie połączenie bez szyfrowania; „Wymagane” — TLS albo brak połączenia; „Wymagane, z weryfikacją certyfikatu” — TLS, a certyfikat i nazwa serwera są sprawdzane w magazynie certyfikatów Windows, także z certyfikatami firmowymi. „Preferowane” i „Wymagane” nie sprawdzają certyfikatu. Po „Zapisz i sprawdź połączenie” aplikacja informuje, czy połączenie jest szyfrowane. W app.properties to ustawienie <code>db.sslmode</code> = <code>prefer</code>, <code>require</code> albo <code>verify-full</code>.</li>'
   '<li><strong>Plik modelu (.qea, .qeax):</strong> bez połączenia z bazą — asystent czyta plik bezpośrednio, tylko do odczytu.</li></ul>'
   '<p>Wskazówka dla administratorów bazy: dzięki wpisom <code>hostssl</code> w pliku pg_hba.conf serwer przyjmuje od wszystkich klientów wyłącznie połączenia TLS. Własne połączenie Enterprise Architect z bazą działa według ustawień EA, nie aplikacji.</p>'),
  ("Czy działa za firmowym proxy albo zaporą?",
   '<p>Potrzebne są połączenia: HTTPS (port 443) do api.anthropic.com, przy modelu w bazie do Twojego serwera PostgreSQL (domyślnie port 5432; połączenie bezpośrednie, nie przez proxy) i — jeśli publikujesz strony — do Twojego Confluence.</p>'
   '<p>Dla HTTPS aplikacja korzysta z serwera proxy ustawionego w opcjach internetowych Windows (proxy systemowe). Możesz też ustawić zmienną środowiskową <code>HTTPS_PROXY</code> albo <code>ALL_PROXY</code>, np. <code>http://proxy.example.com:8080</code> (wyjątki w <code>NO_PROXY</code>) — ma ona pierwszeństwo przed proxy systemowym. Po zmianie uruchom aplikację ponownie. Przy uwierzytelnianiu podstawowym (basic) login i hasło mogą być częścią adresu w tej zmiennej; automatyczne wykrywanie ustawień i skrypty automatycznej konfiguracji (WPAD, PAC) oraz logowanie do proxy kontem Windows (NTLM, Kerberos) nie są obsługiwane.</p>'
   '<p>Certyfikaty HTTPS są sprawdzane także z magazynem certyfikatów Windows, więc firmowy certyfikat główny używany do inspekcji ruchu zadziała, jeśli jest zainstalowany w Windows.</p>'),
  ("W jakich językach jest dostępna?",
   '<p>Po polsku, angielsku, niemiecku i francusku: okna aplikacji i instalatora, odpowiedzi asystenta, domyślne szablony dokumentów, instrukcja użytkownika, licencja i lista zmian. Instalator zaczyna od wyboru języka (domyślnie angielski), a później zmienisz go w Ustawieniach, na karcie „4. Język i zatwierdzanie zmian”.</p>'
   '<p>Asystent odpowiada w języku aplikacji, chyba że napiszesz do niego w innym. Historia rozmów nigdy nie jest tłumaczona.</p>'),
  ("Czy działa z Confluence Cloud i Data Center?",
   '<p>Tak, z oboma — a Confluence jest opcjonalny. Bez niego asystent i tak przygotuje stronę: kod wklejasz sam w edytorze źródła („Kopiuj kod strony”, „Otwórz folder z obrazkami”).</p>'
   '<p>Żeby publikować bezpośrednio, uzupełnij kartę „6. Confluence” w Ustawieniach: dla Data Center adres i token osobisty (PAT), dla Cloud adres (https://twoja-firma.atlassian.net/wiki), e-mail konta i token API, a do tego klucz przestrzeni i opcjonalnie stronę nadrzędną. Asystent tworzy wtedy albo aktualizuje strony przez REST API, z diagramami jako załącznikami — zawsze po Twoim zatwierdzeniu (prośba wyraźnie mówi, kiedy istniejąca strona zostanie zastąpiona; poprzednia wersja zostaje w historii strony) i z uprawnieniami tego tokena.</p>'),
  ("Jak zaktualizować aplikację? Czy ustawienia i rozmowy zostaną?",
   '<p>Zamknij aplikację i uruchom instalator nowszej wersji. Zaproponuje ten sam folder, pokaże, co się zmieniło od Twojej wersji, i zachowa ustawienia, historię rozmów, załączniki, pamięć, szablony i dziennik zmian; klucz, hasła i tokeny w Menedżerze poświadczeń też zostają. Nowe wersje pojawiają się tutaj i na <a href="{releases}">GitHubie</a>.</p>'),
  ("Jak odinstalować aplikację i co zostanie na komputerze?",
   '<p>Zamknij aplikację i użyj skrótu „Odinstaluj Loonar Compass” w menu Start albo pozycji w Ustawieniach Windows → Aplikacje → Zainstalowane aplikacje. Usuwane są pliki programu, skróty i wpis w systemie. Twoje dane zostają, chyba że zaznaczysz jedną z opcji:</p>'
   '<ul><li>„Usuń ustawienia, rozmowy, załączniki, pamięć asystenta, szablony dokumentów i dziennik zmian” — kasuje cały folder <code>%APPDATA%\\Loonar Compass</code>;</li>'
   '<li>„Usuń klucz API, hasła i tokeny z Menedżera poświadczeń” — klucz API Claude, hasła konta EA i bazy oraz token Confluence.</li></ul>'
   '<p>Twój model EA, pliki zapisane gdzie indziej i strony opublikowane w Confluence pozostają bez zmian.</p>'),
  ("Jak zgłosić błąd albo dostawać informacje o nowych wersjach?",
   '<p>W aplikacji otwórz „Zgłoś błąd” na pasku nawigacji, opisz, co się stało, dodaj zrzuty ekranu i kliknij „Wyślij zgłoszenie”: w domyślnym programie pocztowym otworzy się gotowa wiadomość do mariusz@loonar.eu, którą sprawdzasz i wysyłasz. Informacje techniczne (wersja aplikacji, wersja Windows, model AI, tryb zatwierdzania, ostatni komunikat — bez haseł i kluczy) są dołączane tylko wtedy, gdy ta opcja jest zaznaczona (domyślnie jest).</p>'
   '<p>Informacje o nowych wydaniach: wpisz e-mail na karcie „5. Nowe wydania (newsletter)” w Ustawieniach i zaznacz zgodę. Możesz też po prostu napisać na <a href="mailto:mariusz@loonar.eu">mariusz@loonar.eu</a>.</p>'),
 ],
 d_h="Pobierz", d_sub="Bezpłatnie dla każdego — pobierz, zainstaluj, podłącz swój model.",
 d_meta="Instalator dla Windows · {size} MB · {date}",
 d_btn="Pobierz instalator", d_hash="Suma kontrolna SHA-256:",
 d_lic="Freeware: bezpłatnie do każdego zastosowania, także komercyjnego. Zmienianie aplikacji jest zabronione.",
 d_lic_link="Przeczytaj licencję",
 d_lang="Instalator zaczyna od wyboru języka: angielski (domyślnie), polski, niemiecki lub francuski.",
 d_update="Aktualizacja: uruchom nowszy instalator — ustawienia i historia rozmów zostają.",
 d_admin="Instaluje się na Twoim koncie użytkownika — bez uprawnień administratora.",
 gh_label="Kod źródłowy na GitHubie", d_all="Wszystkie wydania",
 sup_h="Wesprzyj projekt",
 sup_text="Loonar Compass jest bezpłatny, także do użytku komercyjnego, i taki pozostanie. Jeśli oszczędza Ci czas, możesz wesprzeć jego rozwój dobrowolną wpłatą. Wpłaty pokrywają m.in. certyfikat do podpisywania kodu, dzięki któremu Windows rozpozna aplikację jako pochodzącą od zweryfikowanego wydawcy.",
 sup_btn="Wesprzyj na suppi.pl",
 sup_more='Inne sposoby wsparcia: zgłaszaj błędy i pomysły („Zgłoś błąd” w aplikacji), poleć aplikację innym użytkownikom Enterprise Architect albo daj projektowi gwiazdkę na <a href="{repo}">GitHubie</a>.',
 ab_h="O mnie",
 ab_why_h="Skąd ta aplikacja",
 ab_why="AI świetnie generuje diagramy jako kod — PlantUML, Mermaid. To jednak rysowanie diagramów, a nie modelowanie: każdy taki diagram jest chwilowym widokiem, a wiedza o systemie nigdzie się nie zapisuje. Simon Brown, autor modelu C4, wyraźnie odróżnia diagramy od modeli: model to jedno źródło prawdy o elementach i ich relacjach, z którego powstaje wiele spójnych widoków — i dopiero to umożliwia nadzór nad architekturą, np. analizę wpływu i kontrolę spójności. Taki model przechowuje Enterprise Architect. Loonar Compass pozwala pracować z tym modelem w rozmowie z AI.",
 ab_name_h="Dlaczego „Compass”",
 ab_name="Kompas daje orientację. Logo to konstelacja węzłów: jak nawigator, który odnajduje drogę po gwiazdach, asystent pomaga odnaleźć się w dużym modelu architektury.",
 ab_role="Principal Technical Product Manager · trener inżynierii wymagań (IREB CPRE)",
 ab_me_h="Kto za tym stoi",
 ab_me="Jestem Principal Technical Product Managerem ze Stargardu. Od lat tworzę produkty cyfrowe — od strategii produktu, badań rynku, analizy klientów i wymagań po wdrożenie — dla usług chmurowych i telekomunikacyjnych, w tym integracji AWS, Azure i Google Cloud z systemami on-premise. Jestem absolwentem informatyki Politechniki Szczecińskiej i trenerem inżynierii wymagań (IREB CPRE). Nie jestem programistą: Loonar Compass powstał z pomocą asystentów AI do programowania — ja decyduję, co aplikacja ma robić i jak ma działać dla architektów, a kod napisała AI. To mocna strona aplikacji: tworzy ją ktoś, kto na co dzień pracuje z modelami w Enterprise Architect i wie, czego potrzebują architekci.",
 ab_li="Mariusz Głowacki na LinkedIn",
 c_h="Kontakt", c_text='Pytanie, pomysł, błąd? Napisz na <a href="mailto:mariusz@loonar.eu">mariusz@loonar.eu</a> albo użyj „Zgłoś błąd” w aplikacji.',
 f_addr="ul. Gombrowicza 12, 73-110 Stargard",
 f_tm="Sparx Systems i Enterprise Architect są zarejestrowanymi znakami towarowymi Sparx Systems Pty Ltd. Claude i Anthropic są znakami towarowymi Anthropic, PBC. Atlassian i Confluence są znakami towarowymi Atlassian. Microsoft, Windows, Word, Excel, GitHub i LinkedIn są znakami towarowymi grupy Microsoft. PostgreSQL jest zarejestrowanym znakiem towarowym PostgreSQL Community Association. Loonar Compass jest niezależnym produktem, niepowiązanym z tymi firmami, nieautoryzowanym ani niesponsorowanym przez nie.",
 f_stats="Bez plików cookie. Anonimowe statystyki odwiedzin (GoatCounter), które nie przechowują danych osobowych. Wybór języka zapamiętuje Twoja przeglądarka.",
 f_privacy_link="Prywatność", f_legal_link="Nota prawna",
 f_top="Do góry",
 doc_updated="Ostatnia aktualizacja:", doc_home="Wróć na stronę główną",
 gc_off="Nie licz moich odwiedzin w tej przeglądarce", gc_on="Licz znowu moje odwiedziny",
 gc_state_on="Odwiedziny z tej przeglądarki są liczone (anonimowo).", gc_state_off="Odwiedziny z tej przeglądarki nie są liczone.",
 nf_title="Nie znaleziono strony", nf_text="Strona, której szukasz, nie istnieje albo została przeniesiona.", nf_link="Przejdź do strony głównej",
 m_today="Dzisiaj",
 m_convs=["Proces zamówienia (BPMN)","Wymagania z dokumentu PDF","Wpływ: usługa płatności"],
 m_user="Narysuj proces zamówienia w naszym sklepie internetowym jako BPMN w pakiecie <b>Sprzedaż</b>. Płatność zrób jako podproces osadzony.",
 m_steps_h="Kroki ostatniej odpowiedzi: 3",
 m_steps=["Claude analizuje i planuje (krok 1)","Odczyt z bazy modelu: pakiet Sprzedaż","Zapis do EA (operacje: 14): diagram Proces zamówienia"],
 m_bot="Gotowe. W pakiecie <b>Sprzedaż</b> jest nowy diagram BPMN <b>Proces zamówienia</b>: 3 zadania, bramka i podproces osadzony <b>Płatność</b> z 3 krokami. Otworzyłem go w Twoim oknie EA.",
 m_ph="Napisz wiadomość, przeciągnij pliki albo wklej obraz (Ctrl+V)…",
 m_mode="claude-haiku-5-5 · tryb ask",
 m_appr_h="Claude chce zapisać zmiany w modelu",
 m_appr=["Nowy diagram BPMN „Proces zamówienia” w pakiecie Sprzedaż","3 zadania, 1 bramka, zdarzenie początkowe i końcowe","Podproces osadzony „Płatność” z 3 krokami","9 przepływów sekwencji"],
 m_approve="Zatwierdź", m_reject="Odrzuć",
 m_saveas="Zapisz jako…", m_full="Otwórz w pełnym rozmiarze", m_png="Proces zamówienia.png",
 d_recv=["Przyjmij","zamówienie"], d_gw="Na stanie?", d_pay="Płatność",
 dg_sub=[["Wybierz","metodę"],["Autoryzuj"],["Potwierdź"]],
 d_ship=["Wyślij","zamówienie"], d_notify=["Powiadom","klienta"], d_yes="tak", d_no="nie",
 m_log_h="Dziennik zmian", m_log_btn="Eksportuj do Excela",
 m_log_cols=["Czas","Wynik","Konto EA","Zmiany"],
 m_log=[("14:02","OK","Proces zamówienia: 14 operacji"),("13:47","OK","Klient: 3 atrybuty, 2 operacje"),("13:21","BŁĄD","Wysyłka: pakiet zablokowany przez innego użytkownika"),("11:05","OK","Legenda na diagramie Kasa")],
 m_err="BŁĄD",
)

T["de"] = dict(
 locale="de_DE", name_lang="Deutsch",
 title="Loonar Compass – KI-Assistent für Sparx Enterprise Architect",
 desc="Kostenlose Windows-App: Sprechen Sie mit Ihrem Sparx-Enterprise-Architect-Modell. Fragen stellen, Diagramme über EA anlegen und ändern, jede Änderung selbst freigeben. Nutzt die Claude-API von Anthropic mit Ihrem eigenen API-Schlüssel.",
 og_desc="Sprechen Sie mit Ihrem Enterprise-Architect-Modell: fragen, zeichnen, dokumentieren – EA schreibt jede Änderung, Sie geben sie frei.",
 skip="Zum Inhalt springen", lang_label="Sprache",
 nav=["Funktionen","So funktioniert’s","Sicherheit","Voraussetzungen","FAQ","Unterstützen"], nav_dl="Download",
 eyebrow="Freeware · Windows · Sparx Enterprise Architect",
 h1='KI-Assistent für Sparx Enterprise Architect – <em>sprechen Sie mit Ihrem Modell</em>',
 lead="Stellen Sie Fragen in natürlicher Sprache, lassen Sie Diagramme zeichnen und umbauen und machen Sie aus dem Modell Dokumente. Jede Änderung schreibt Enterprise Architect selbst – und erst, wenn Sie sie freigegeben haben.",
 cta1="Kostenlos herunterladen", cta2="Funktionen ansehen",
 meta="Version <a href=\"{changelog}\">{ver}</a> · Windows 10/11 · nutzt die Claude-API von Anthropic mit Ihrem eigenen API-Schlüssel",
 hero_alt="Illustration des Fensters von Loonar Compass: eine Anfrage, den Bestellprozess eines Onlineshops als BPMN zu zeichnen, die Schrittliste und die Antwort des Assistenten.",
 f_h="Was er kann", f_sub="Ein Gespräch statt Dutzender Dialogfenster – von der schnellen Frage bis zum vollständigen, einheitlich gestalteten Diagramm.",
 features=[
  ("Fragen Sie Ihr Modell","Fragen in natürlicher Sprache: Was liegt in einem Paket, welche Elemente realisieren eine Anforderung, was hat sich geändert? Der Assistent fragt das Modell ab – ein PostgreSQL-Repository oder eine .qea/.qeax-Datei, nur lesend – und benennt Objekte mit Name, Paket und GUID. Jede Unterhaltung kann mit ihrem eigenen Modell arbeiten."),
  ("Anlegen und ändern","Pakete, Elemente, Diagramme und Konnektoren; Attribute und Operationen von Klassen mit Parametern; Schlüsselwörter, Farben, Position und Größe. Verschieben, umbenennen, löschen – oder ein ganzes Paket samt Inhalt kopieren."),
  ("BPMN, wie EA es erwartet","Korrekte BPMN-2.0-Typen mit Stereotypen und Tags: Tasks, Ereignisse, Gateways, Flüsse, zugeklappte und eingebettete Teilprozesse. Aus einer BPMN-Datei, einem Foto, einer Skizze oder nur einer Beschreibung."),
  ("Gut lesbare Diagramme","Legenden mit automatischer Einfärbung, Linienstile und Konnektorführung entlang eines vorgegebenen Pfads, Konnektoren nur auf einem Diagramm ausblenden, Navigationskacheln zu anderen Diagrammen."),
  ("EA schreibt selbst","Änderungen laufen über die Automatisierungsschnittstelle von EA (COM) – bei aktivierter Modellsicherheit unter einem EA-Benutzerkonto, mit dessen Berechtigungen und Sperren. Vor größeren Umbauten ist der Assistent angewiesen, eine Baseline anzulegen."),
  ("Jede Änderung geben Sie frei","Drei Freigabemodi: ask, plan und auto. Jede Speicherung, auch eine fehlgeschlagene, steht im Änderungsprotokoll mit Zeit, Konto und Ergebnis – exportierbar nach Excel."),
  ("Auswirkungsanalyse","Von einem Element entlang der Beziehungen bis zu sechs Ebenen tief, mit Pfaden und betroffenen Diagrammen. Wo eine Service-Operation in Sequenz- und Aktivitätsdiagrammen verwendet wird – und welche nirgends."),
  ("Arbeitet mit Ihrem offenen EA","Kennt das geöffnete Diagramm und die Auswahl, öffnet Diagramme, hebt Elemente im Projekt-Browser hervor und aktualisiert Geändertes selbst – ganz ohne F5."),
  ("Word, Excel und Bilder","Word-Dokumente mit Diagrammen aus EA, Excel-Arbeitsmappen (Elementlisten, Matrizen, Analyseergebnisse) und PNG-Bilder wie eine Matrix aus Anwendungsfällen und Anforderungen."),
  ("Confluence-Seiten","Seiten aus Modell und Gespräch: Code für den Quelltext-Editor oder – nach Ihrer Freigabe – über die REST-API veröffentlicht, Diagramme als Anhänge. Auf Basis anpassbarer Dokumentvorlagen."),
  ("Gedächtnis des Assistenten","Merkt sich Vereinbarungen – Modellierungskonventionen, Vorlieben, Korrekturen – als lokale Markdown-Dateien und berücksichtigt sie in jedem neuen Gespräch, unabhängig vom Modell. Auf ein einzelnes Modell beschränkt wird eine Notiz nur, wenn Sie darum bitten."),
  ("Vier Sprachen","Deutsch, Englisch, Polnisch und Französisch: Anwendung, Antworten des Assistenten, Dokumentvorlagen und Benutzerhandbuch."),
 ],
 h_h="So funktioniert’s", h_sub="Sie behalten bei jedem Schritt die Kontrolle.",
 steps=[
  ("Sie fragen","Frage oder Aufgabe eingeben; PDF, Screenshot, Skizze oder BPMN-Datei anhängen."),
  ("Claude plant","Das Claude-Modell entscheidet, was nachzuschlagen und was zu ändern ist. Jeden Schritt sehen Sie live im Chat."),
  ("Lesen und Schreiben im Modell","Liest direkt aus dem PostgreSQL-Repository oder der .qea/.qeax-Datei (nur lesend) und schreibt ausschließlich über Enterprise Architect (COM)."),
  ("Sie geben frei","Sie sehen die geplanten Änderungen und geben sie frei oder lehnen sie ab. Gespeichertes landet im Änderungsprotokoll und sofort in Ihrem EA-Fenster."),
 ],
 s_h="Ein Blick hinein", s_sub="Die Oberfläche mit Beispielinhalten aus einem fiktiven Onlineshop.",
 caps=["Jede Speicherung wartet auf Ihre Freigabe (Modus ask).","Diagramme kommen als Bild in den Chat – genau so, wie EA sie zeichnet.","Das Änderungsprotokoll hält jede Speicherung fest – mit Export nach Excel."],
 sec_h="Sicherheit und Datenschutz", sec_sub="Ihr Modell bleibt, wo es ist. Der Assistent arbeitet auf Ihrem Rechner, mit Ihren Konten.",
 sec=[
  ("Läuft lokal","Eine Desktop-Anwendung für Windows, installiert für Ihr Benutzerkonto. Einstellungen, Gesprächsverlauf, Gedächtnis und Änderungsprotokoll liegen in Ihrem Benutzerprofil."),
  ("Geheimnisse bleiben in Windows","API-Schlüssel für Claude, Passwort des EA-Kontos, Datenbankpasswort und Confluence-Token liegen in der Windows-Anmeldeinformationsverwaltung – nie im Klartext in der Einstellungsdatei."),
  ("Verschlüsselte Verbindungen","Claude-API und Confluence werden nur über HTTPS angesprochen. Die Verbindung zu PostgreSQL lässt sich mit TLS verschlüsseln – bevorzugt, erforderlich oder erforderlich mit Prüfung des Serverzertifikats gegen den Windows-Zertifikatspeicher. Eine Modelldatei (.qea, .qeax) braucht keine Datenbankverbindung: Der Assistent liest sie direkt und nur lesend."),
  ("Schreiben nur über EA","Die Datenbanksitzung ist nur lesend, eine Modelldatei wird schreibgeschützt geöffnet. Jede Änderung führt Enterprise Architect aus – bei aktivierter Sicherheit unter dem eingerichteten EA-Konto, mit dessen Berechtigungen und Sperren. Inhalte von Anhängen, Dokumenten und Modell soll der Assistent als Daten behandeln, nicht als Anweisungen."),
  ("Was an Anthropic geht","Für die Antwort erhält Claude das Gespräch, Ihre Anhänge, gemerkte Vereinbarungen, verwendete Vorlagen, die Ergebnisse der Abfragen des Assistenten und die gezeigten Diagrammbilder – nicht das ganze Modell. Prüfen Sie vor dem Einsatz an Firmenmodellen die Regeln Ihrer Organisation."),
  ("Ihr eigener API-Schlüssel","Die App nutzt Ihren Claude-API-Schlüssel aus der Anthropic Console. Die App selbst ist kostenlos; die Nutzung von Claude rechnet Anthropic mit dem Konto ab, zu dem der Schlüssel gehört."),
 ],
 r_h="Voraussetzungen", r_need="Erforderlich", r_opt="Optional",
 req=[
  "Windows 10 oder 11 (64 Bit)",
  "Sparx Enterprise Architect mit Lizenz – zum Speichern startet der Assistent EA im Hintergrund (das währenddessen einen Lizenzplatz belegt) oder nutzt bei einer dort geöffneten Modelldatei Ihr EA-Fenster",
  "Ein EA-Modell als Projektdatei .qea oder .qeax oder in einer PostgreSQL-Datenbank mit aktivierter Sicherheit („Require User Lock to Edit“)",
  "Für ein Datenbankmodell: Zugang zur Datenbank (Server, Port, Name, Konto) und ein EA-Benutzerkonto für den Assistenten mit Bearbeitungsrechten; für eine Modelldatei genügt der Pfad (ein Konto nur bei aktivierter Sicherheit)",
  "Ein Claude-API-Schlüssel aus der Anthropic Console und eine Internetverbindung",
 ],
 opt=[
  "Confluence (Data Center oder Cloud) – zum direkten Veröffentlichen von Seiten",
  "Ein Standard-E-Mail-Programm in Windows – für Fehlermeldungen und Neuigkeiten zu Versionen",
 ],
 r_note="Ältere Dateiformate (.eap, .eapx, .feap) und andere Datenbanken werden nicht unterstützt; eine .eap/.eapx-Datei überführt EA per Projekttransfer in eine .qea-Datei (Settings → Model → Transfer → Full Project Transfer).",
 faq_h="Häufige Fragen", faq_sub="Klare Antworten zu Kosten, Voraussetzungen, Sicherheit und Datenschutz.",
 faq=[
  ("Ist das wirklich kostenlos? Darf ich es beruflich nutzen?",
   '<p>Ja. Loonar Compass ist Freeware: kostenlos für jeden Zweck, auch beruflich und kommerziell, auf beliebig vielen Computern. Sie dürfen die Anwendung auch weitergeben – kostenlos, vollständig, unverändert und zusammen mit den Dateien LICENSE und THIRD-PARTY-NOTICES.</p>'
   '<p>Der Quellcode ist auf <a href="{repo}">GitHub</a> öffentlich, sodass Sie genau sehen, was die Anwendung mit Ihrem Modell macht. Er ist einsehbar (Source-available), aber nicht Open Source: Nicht erlaubt sind Änderungen an der Anwendung und abgeleitete Werke sowie Verkauf oder Vermietung. <a href="{lic}">Lizenz lesen</a>.</p>'),
  ("Was kostet der Betrieb, und welches Claude-Modell wird verwendet?",
   '<p>Die Anwendung selbst ist kostenlos. Der Assistent arbeitet mit Ihrem eigenen Claude-API-Schlüssel aus der Anthropic Console; Anthropic rechnet die Nutzung mit dem Konto ab, zu dem der Schlüssel gehört – nach Verbrauch, also nach der Textmenge, die das Modell liest und schreibt. Aktuelle Preise finden Sie auf der <a href="https://claude.com/pricing#api">Preisseite von Anthropic</a>.</p>'
   '<p>Das Modell ist immer Claude Haiku 5.5 (<code>claude-haiku-5-5</code>) und keine Einstellung. „Max. Tokens“ (standardmäßig 10.000) begrenzt nur die Länge einer einzelnen Antwort: Sie zahlen für den tatsächlich erzeugten Text.</p>'),
  ("Unter welchen Betriebssystemen läuft es?",
   '<p>Unter Windows 10 und Windows 11, jeweils 64 Bit. Es ist eine native Windows-Desktopanwendung und arbeitet mit Enterprise Architect zusammen, das auf demselben Computer installiert ist.</p>'),
  ("Welche Enterprise-Architect-Umgebungen werden unterstützt?",
   '<ul><li>Enterprise Architect auf demselben Computer, mit gültiger Lizenz. Zum Speichern startet der Assistent es im Hintergrund, wo es einen Lizenzplatz belegt; nach 10 Minuten ohne Arbeit und beim Schließen der Anwendung wird es wieder beendet. Ist die Modelldatei in Ihrem EA geöffnet, laufen die Änderungen stattdessen über Ihr Fenster – ein zweites EA öffnet nie dieselbe Datei.</li>'
   '<li>Das Modell als EA-Projektdatei .qea oder .qeax – in den Einstellungen „2. Modell“ → „EA-Datei (.qea, .qeax)“ und der Pfad unter „Modelldatei“, ohne Datenbankserver, Konto und Passwort – oder in einer PostgreSQL-Datenbank, die Ihr Computer erreicht (Server, Port, Datenbankname, Konto). Eine .qea-Datei ist für eine Person gedacht, .qeax für ein kleines Team, das die Datei gemeinsam nutzt (Sparx empfiehlt es für weniger als 5 Benutzer). Ältere Dateiformate (.eap, .eapx, .feap) und Repositorys in anderen Datenbanken werden nicht unterstützt; eine .eap/.eapx-Datei überführt Enterprise Architect per Projekttransfer in eine .qea-Datei (Settings → Model → Transfer → Full Project Transfer).</li>'
   '<li>Für ein Modell in einer Datenbank: aktivierte Benutzersicherheit (Settings → Security → Enable Security) im Modus „Require User Lock to Edit“. In einer Modelldatei ist die Sicherheit optional – ohne sie braucht der Assistent kein EA-Konto.</li>'
   '<li>Bei aktivierter Sicherheit: ein EA-Benutzerkonto für den Assistenten mit Passwort und Bearbeitungsrechten – am besten ein eigenes, damit sich seine Änderungen und Sperren leicht von Ihren unterscheiden lassen.</li><li>Mehrere Modelle: Jede Unterhaltung hat ihr eigenes Modell, das in ihrer Kopfzeile steht („Modell: …“). Eine neue Unterhaltung verwendet das Standardmodell aus den Einstellungen; mit „Anderes Modell wählen…“ – oder einem Klick auf die Kopfzeile, der „Modell dieser Unterhaltung“ öffnet – wählen Sie eine andere PostgreSQL-Datenbank (Server, Port, Datenbank, Konto, Passwort, Verschlüsselung) oder EA-Datei (.qea, .qeax) samt EA-Konto und testen die Verbindung. Eine auf das Fenster gezogene .qea/.qeax-Datei wird zum Modell der Unterhaltung. Unterhaltungen aus früheren Versionen erhalten eine Kopie des Modells aus den Einstellungen. Enterprise Architect im Hintergrund hat jeweils ein Modell geöffnet; der Wechsel zu einem anderen Modell dauert einige Sekunden bis etwa 25 Sekunden.</li></ul>'
   '<p>Die Anwendung prüft weder Version noch Edition von EA. Entscheidend ist, dass Ihr Enterprise Architect dieses Modell öffnen kann: die Datei oder das Datenbank-Repository mit aktivierter Sicherheit.</p>'),
  ("Brauche ich Administratorrechte für die Installation?",
   '<p>Nein. Standardmäßig („Nur für mich“) installiert das Installationsprogramm Loonar Compass nur für Ihr Benutzerkonto, unter <code>%LOCALAPPDATA%\\Programs\\Loonar Compass</code>, und braucht dafür keine Administratorrechte. Wählen Sie stattdessen „Für alle Benutzer dieses Computers“ (unter C:\\Program Files\\Loonar Compass), fragt das Installationsprogramm nach Administratorrechten.</p>'
   '<p>Die Anwendung selbst läuft immer als normaler Benutzer, und jeder Benutzer hat eigene Einstellungen und Unterhaltungen. Bestätigt jemand anderes eine Installation für alle Benutzer mit einem Administratorkonto, geben Sie Ihre Konfiguration später in der Anwendung ein (Einstellungen), nicht im Installationsprogramm – sonst landet sie im Profil des Administrators.</p>'),
  ("Warum kann Windows SmartScreen vor dem Installationsprogramm warnen?",
   '<p>Weil das Installationsprogramm der Version {ver} noch nicht digital signiert ist (mit einem Codesignaturzertifikat) und Windows den Herausgeber daher nicht kennt. Microsoft Defender SmartScreen kann deshalb beim Start „Der Computer wurde durch Windows geschützt“ melden, und Ihr Browser kann warnen, dass die Datei selten heruntergeladen wird. Die Standardinstallation für Ihr Benutzerkonto braucht keine Administratorrechte, daher erscheint keine Administratorabfrage; nur die Installation für alle Benutzer fragt danach – und zeigt den Herausgeber als unbekannt an. Ein signiertes Installationsprogramm ist geplant und sollte bald folgen. Bis dahin:</p>'
   '<ul><li>Laden Sie das Installationsprogramm nur von loonar.eu oder aus den <a href="{releases}">Releases des Projekts auf GitHub</a> über HTTPS herunter.</li>'
   '<li>Vergleichen Sie seine Prüfsumme mit der SHA-256 neben der Download-Schaltfläche – in PowerShell: <code>Get-FileHash .\\{setup}</code>.</li>'
   '<li>Klicken Sie im SmartScreen-Fenster auf „Weitere Informationen“ und dann auf „Trotzdem ausführen“.</li></ul>'
   '<p>Unter Windows 11 mit aktivierter intelligenter App-Steuerung (Smart App Control) werden nicht signierte Anwendungen blockiert und lassen sich so nicht starten – warten Sie in diesem Fall bitte auf die signierte Version.</p>'),
  ("Kann der Assistent mein Modell beschädigen?",
   '<p>Er speichert nur, was Sie zulassen, und nur so, wie Enterprise Architect es erlaubt:</p>'
   '<ul><li><strong>Freigabemodi:</strong> ask – Sie geben jede Speicherung frei; plan – eine Freigabe pro Antwort; auto – ohne Rückfrage, für Testmodelle. Veröffentlichungen in Confluence brauchen immer Ihre Freigabe, und die Anfrage sagt ausdrücklich, wenn eine bestehende Seite ersetzt wird.</li>'
   '<li><strong>Schreiben nur über EA:</strong> Die Datenbank wird in einer schreibgeschützten Sitzung gelesen, eine Modelldatei schreibgeschützt; jede Änderung führt Enterprise Architect aus – bei aktivierter Sicherheit unter dem EA-Konto des Assistenten, mit dessen Berechtigungen und Sperren. Objekte, die andere Benutzer gesperrt haben, bleiben unangetastet.</li>'
   '<li><strong>Anweisungen nur von Ihnen:</strong> Inhalte von Anhängen, Dokumenten und Modell soll der Assistent als Daten behandeln, nicht als auszuführende Anweisungen.</li>'
   '<li><strong>Änderungsprotokoll:</strong> Jede Speicherung, auch eine fehlgeschlagene, wird mit Zeit, Konto und Ergebnis festgehalten und lässt sich nach Excel exportieren.</li>'
   '<li><strong>Baselines:</strong> Vor dem Löschen oder größeren Umbau eines bestehenden Pakets ist der Assistent angewiesen, zuerst eine Baseline davon anzulegen; wiederhergestellt wird sie in EA (Manage Baselines → Compare/Restore).</li></ul>'
   '<p>Trotzdem: Löschen ist endgültig, und KI kann sich irren – die Anwendung weist darauf hin; prüfen Sie die vorgeschlagenen Änderungen daher vor der Freigabe. Nutzen Sie bei wichtigen Modellen den Modus ask und sichern Sie die Datenbank bzw. die Modelldatei – die Anwendung wird unentgeltlich und im vorliegenden Zustand bereitgestellt (Einzelheiten in der Lizenz).</p>'),
  ("Welche Daten verlassen meinen Computer, und wohin gehen sie?",
   '<p>Für eine Antwort sendet der Assistent an die Claude-API von Anthropic (api.anthropic.com) nur, was er braucht: Ihre Nachrichten und Anhänge, gemerkte Vereinbarungen, verwendete Vorlagen und das, was er in dieser Unterhaltung aus dem Modell gelesen hat – die Ergebnisse seiner Abfragen am Modell (Namen und Beschreibungen von Paketen, Elementen, Diagrammen) und Bilder der gezeigten Diagramme. Nicht das ganze Modell. Nach einem Sprachwechsel gehen auch Gedächtnis und Vorlagen zur Übersetzung dorthin.</p>'
   '<p>Darüber hinaus gehen Daten nur dorthin, wohin Sie sie schicken: veröffentlichte Seiten in Ihr Confluence; Fehlermeldungen und die Newsletter-Anmeldung sind E-Mails, die Sie in Ihrem E-Mail-Programm prüfen und selbst senden. Von sich aus sendet die Anwendung nichts an Loonar. Prüfen Sie vor dem Einsatz an Firmenmodellen die Regeln Ihrer Organisation und die Datenverarbeitungsbedingungen Ihres Anthropic-Kontos.</p>'),
  ("Wo werden Einstellungen, Unterhaltungen und Passwörter gespeichert?",
   '<p>Auf Ihrem Computer, in Ihrem Windows-Benutzerprofil – jeder Benutzer des Computers hat eigene:</p>'
   '<ul><li><code>%APPDATA%\\Loonar Compass</code>: Einstellungen (app.properties), Unterhaltungsverlauf, Anhänge, Gedächtnis des Assistenten, Dokumentvorlagen und Änderungsprotokoll.</li>'
   '<li>Windows-Anmeldeinformationsverwaltung: Claude-API-Schlüssel, Passwort des EA-Kontos, Datenbankpasswort und Confluence-Token sowie die Passwörter eines Unterhaltungsmodells, das von den Einstellungen abweicht. Keines davon steht im Klartext in der Einstellungsdatei.</li></ul>'
   '<p>Das Programm selbst liegt standardmäßig unter <code>%LOCALAPPDATA%\\Programs\\Loonar Compass</code>, bei einer Installation für alle Benutzer unter C:\\Program Files\\Loonar Compass.</p>'),
  ("Ist die Kommunikation verschlüsselt?",
   '<p>Ja, überall dort, wo die Anwendung selbst Verbindungen aufbaut:</p>'
   '<ul><li><strong>Claude-API und Confluence:</strong> nur über HTTPS. Eine Confluence-Adresse mit <code>http://</code> wird nur für einen Server auf demselben Computer (localhost) akzeptiert.</li>'
   '<li><strong>Modelldatenbank (PostgreSQL):</strong> TLS, einstellbar mit „Verschlüsselung der Verbindung“ auf der Karte „2. Modell“ in den Einstellungen (Option „PostgreSQL-Datenbank“): „Bevorzugt“ (Standard) – TLS, sofern der Server es anbietet, sonst eine unverschlüsselte Verbindung; „Erforderlich“ – TLS oder keine Verbindung; „Erforderlich, mit Zertifikatsprüfung“ – TLS, wobei Zertifikat und Name des Servers gegen den Windows-Zertifikatspeicher geprüft werden, einschließlich der Zertifikate Ihres Unternehmens. „Bevorzugt“ und „Erforderlich“ prüfen das Zertifikat nicht. Nach „Speichern und Verbindung prüfen“ zeigt die Anwendung an, ob die Verbindung verschlüsselt ist. In app.properties heißt die Einstellung <code>db.sslmode</code> = <code>prefer</code>, <code>require</code> oder <code>verify-full</code>.</li>'
   '<li><strong>Modelldatei (.qea, .qeax):</strong> keine Datenbankverbindung – der Assistent liest die Datei direkt und nur lesend.</li></ul>'
   '<p>Tipp für Datenbankadministratoren: Mit <code>hostssl</code>-Einträgen in pg_hba.conf nimmt der Server von allen Clients nur TLS-Verbindungen an. Die eigene Verbindung von Enterprise Architect zur Datenbank richtet sich nach den Einstellungen von EA, nicht nach denen der Anwendung.</p>'),
  ("Funktioniert es hinter einem Firmen-Proxy oder einer Firewall?",
   '<p>Benötigt werden diese Verbindungen: HTTPS (Port 443) zu api.anthropic.com, bei einem Datenbankmodell zu Ihrem PostgreSQL-Server (standardmäßig Port 5432; direkt, nicht über den Proxy) und – wenn Sie Seiten veröffentlichen – zu Ihrem Confluence.</p>'
   '<p>Für HTTPS verwendet die Anwendung den Proxy aus den Windows-Internetoptionen (System-Proxy). Sie können auch die Umgebungsvariable <code>HTTPS_PROXY</code> oder <code>ALL_PROXY</code> setzen, z.\u00a0B. <code>http://proxy.example.com:8080</code> (Ausnahmen in <code>NO_PROXY</code>); sie hat Vorrang vor dem System-Proxy. Starten Sie die Anwendung nach einer Änderung neu. Bei Standardauthentifizierung (Basic) können Benutzername und Passwort Teil der Adresse in dieser Variablen sein; automatische Erkennung und Skripte zur automatischen Konfiguration (WPAD, PAC) sowie die Proxy-Anmeldung mit dem Windows-Konto (NTLM, Kerberos) werden nicht unterstützt.</p>'
   '<p>HTTPS-Zertifikate werden auch gegen den Windows-Zertifikatspeicher geprüft – ein firmeneigenes Stammzertifikat für die Prüfung des Datenverkehrs wird also akzeptiert, sobald es in Windows installiert ist.</p>'),
  ("In welchen Sprachen gibt es die Anwendung?",
   '<p>Deutsch, Englisch, Polnisch und Französisch: Fenster der Anwendung und des Installationsprogramms, Antworten des Assistenten, Standard-Dokumentvorlagen, Benutzerhandbuch, Lizenz und Änderungsliste. Das Installationsprogramm fragt zuerst nach der Sprache (Standard: Englisch); später ändern Sie sie in den Einstellungen auf der Karte „4. Sprache und Freigabe von Änderungen“.</p>'
   '<p>Der Assistent antwortet in der Sprache der Anwendung, außer Sie schreiben ihm in einer anderen. Der Unterhaltungsverlauf wird nie übersetzt.</p>'),
  ("Funktioniert es mit Confluence Cloud und Data Center?",
   '<p>Ja, mit beiden – und Confluence ist optional. Auch ohne erstellt der Assistent Seiten; den Code fügen Sie dann selbst im Quelltext-Editor ein („Seitencode kopieren“, „Bilderordner öffnen“).</p>'
   '<p>Zum direkten Veröffentlichen füllen Sie in den Einstellungen die Karte „6. Confluence“ aus: für Data Center Adresse und persönliches Zugriffstoken (PAT), für Cloud Adresse (https://ihre-firma.atlassian.net/wiki), E-Mail des Kontos und API-Token, dazu den Bereichsschlüssel und optional eine übergeordnete Seite. Der Assistent legt Seiten dann über die REST-API an oder aktualisiert sie, mit Diagrammen als Anhängen – immer erst nach Ihrer Freigabe (die Anfrage sagt ausdrücklich, wenn eine bestehende Seite ersetzt wird; die vorherige Version bleibt im Seitenverlauf erhalten) und mit den Berechtigungen dieses Tokens.</p>'),
  ("Wie aktualisiere ich? Bleiben Einstellungen und Unterhaltungen erhalten?",
   '<p>Schließen Sie die Anwendung und starten Sie das Installationsprogramm der neueren Version. Es schlägt denselben Ordner vor, zeigt die Änderungen seit Ihrer Version und behält Einstellungen, Unterhaltungsverlauf, Anhänge, Gedächtnis, Vorlagen und Änderungsprotokoll; Schlüssel, Passwörter und Tokens in der Anmeldeinformationsverwaltung bleiben ebenfalls. Neue Versionen erscheinen hier und auf <a href="{releases}">GitHub</a>.</p>'),
  ("Wie deinstalliere ich es, und was bleibt auf dem Computer?",
   '<p>Schließen Sie die Anwendung und verwenden Sie „Loonar Compass deinstallieren“ im Startmenü oder den Eintrag unter Windows-Einstellungen → Apps → Installierte Apps. Entfernt werden Programmdateien, Verknüpfungen und der Systemeintrag. Ihre Daten bleiben erhalten, es sei denn, Sie aktivieren eine dieser Optionen:</p>'
   '<ul><li>„Einstellungen, Unterhaltungen, Anhänge, Gedächtnis des Assistenten, Dokumentvorlagen und Änderungsprotokoll entfernen“ – löscht den ganzen Ordner <code>%APPDATA%\\Loonar Compass</code>;</li>'
   '<li>„API-Schlüssel, Passwörter und Tokens aus der Anmeldeinformationsverwaltung entfernen“ – Claude-API-Schlüssel, Passwörter des EA-Kontos und der Datenbank sowie Confluence-Token.</li></ul>'
   '<p>Ihr EA-Modell, anderswo gespeicherte Dateien und in Confluence veröffentlichte Seiten bleiben unberührt.</p>'),
  ("Wie melde ich einen Fehler, und wie erfahre ich von neuen Versionen?",
   '<p>Öffnen Sie in der Anwendung „Fehler melden“ in der Navigationsleiste, beschreiben Sie, was passiert ist, fügen Sie Screenshots hinzu und klicken Sie auf „Meldung senden“: In Ihrem Standard-E-Mail-Programm öffnet sich eine fertige Nachricht an mariusz@loonar.eu, die Sie prüfen und senden. Technische Informationen (Anwendungsversion, Windows-Version, KI-Modell, Freigabemodus, letzte Meldung – ohne Passwörter und Schlüssel) werden nur angehängt, solange diese Option aktiviert ist (standardmäßig ja).</p>'
   '<p>Neuigkeiten zu Versionen: E-Mail-Adresse auf der Karte „5. Neue Versionen (Newsletter)“ in den Einstellungen eintragen und die Einwilligung ankreuzen. Oder schreiben Sie einfach an <a href="mailto:mariusz@loonar.eu">mariusz@loonar.eu</a>.</p>'),
 ],
 d_h="Download", d_sub="Kostenlos für alle – herunterladen, installieren, Modell verbinden.",
 d_meta="Windows-Installer · {size} MB · {date}",
 d_btn="Installer herunterladen", d_hash="SHA-256-Prüfsumme:",
 d_lic="Freeware: kostenlos für jeden Zweck, auch kommerziell. Änderungen an der Anwendung sind nicht erlaubt.",
 d_lic_link="Lizenz lesen",
 d_lang="Das Installationsprogramm fragt zuerst nach der Sprache: Englisch (Standard), Polnisch, Deutsch oder Französisch.",
 d_update="Zum Aktualisieren einfach einen neueren Installer starten – Einstellungen und Verlauf bleiben erhalten.",
 d_admin="Installation für Ihr Benutzerkonto – ohne Administratorrechte.",
 gh_label="Quellcode auf GitHub", d_all="Alle Versionen",
 sup_h="Projekt unterstützen",
 sup_text="Loonar Compass ist kostenlos, auch für die kommerzielle Nutzung, und bleibt es. Wenn es Ihnen Zeit spart, können Sie die Weiterentwicklung mit einem freiwilligen Beitrag unterstützen. Damit wird unter anderem ein Codesignaturzertifikat bezahlt, mit dem Windows die Anwendung als Software eines verifizierten Herausgebers erkennt.",
 sup_btn="Auf suppi.pl unterstützen",
 sup_more='Weitere Möglichkeiten zu helfen: Melden Sie Fehler und Ideen („Fehler melden“ in der App), empfehlen Sie die Anwendung anderen Enterprise-Architect-Nutzern oder geben Sie dem Projekt einen Stern auf <a href="{repo}">GitHub</a>.',
 ab_h="Über mich",
 ab_why_h="Warum es diese Anwendung gibt",
 ab_why="KI erzeugt hervorragend Diagramme als Code – PlantUML, Mermaid. Das ist aber Diagrammzeichnen, kein Modellieren: Jedes solche Diagramm ist eine Momentaufnahme, und das Wissen über das System wird nirgends festgehalten. Simon Brown, der Autor des C4-Modells, unterscheidet genau zwischen Diagrammen und Modellen: Ein Modell ist die eine verlässliche Quelle (Single Source of Truth) für Elemente und ihre Beziehungen, aus der sich viele konsistente Sichten ableiten lassen – und erst das ermöglicht Governance wie Auswirkungsanalysen und Konsistenzprüfungen. Enterprise Architect hält ein solches Modell. Mit Loonar Compass arbeiten Sie mit diesem Modell im Gespräch mit einer KI.",
 ab_name_h="Warum „Compass“",
 ab_name="Ein Kompass gibt Orientierung. Das Logo ist eine Konstellation von Knoten: So wie ein Navigator seinen Weg anhand der Sterne findet, hilft Ihnen der Assistent, sich in einem großen Architekturmodell zurechtzufinden.",
 ab_role="Principal Technical Product Manager · Trainer für Requirements Engineering (IREB CPRE)",
 ab_me_h="Wer dahintersteht",
 ab_me="Ich bin Principal Technical Product Manager aus Stargard (Polen) und entwickle seit Jahren digitale Produkte – von Produktstrategie, Marktforschung, Kundenanalyse und Anforderungen bis zur Umsetzung – für Cloud- und Telekommunikationsdienste, einschließlich der Integration von AWS, Azure und Google Cloud mit On-Premise-Systemen. Ich habe Informatik an der Technischen Universität Stettin (Politechnika Szczecińska) studiert und bin Trainer für Requirements Engineering (IREB CPRE). Programmierer bin ich nicht: Loonar Compass ist mit KI-Programmierassistenten entstanden – ich lege fest, was die Anwendung tun und wie sie sich für Architekten verhalten soll, den Code hat die KI geschrieben. Das ist ihre Stärke: Sie stammt von jemandem, der täglich mit Enterprise-Architect-Modellen arbeitet und weiß, was Architekten brauchen.",
 ab_li="Mariusz Głowacki auf LinkedIn",
 c_h="Kontakt", c_text='Fragen, Ideen oder einen Fehler gefunden? Schreiben Sie an <a href="mailto:mariusz@loonar.eu">mariusz@loonar.eu</a> – oder nutzen Sie „Fehler melden“ in der App.',
 f_addr="ul. Gombrowicza 12, 73-110 Stargard, Polen",
 f_tm="Sparx Systems und Enterprise Architect sind eingetragene Marken der Sparx Systems Pty Ltd. Claude und Anthropic sind Marken der Anthropic, PBC. Atlassian und Confluence sind Marken von Atlassian. Microsoft, Windows, Word, Excel, GitHub und LinkedIn sind Marken der Microsoft-Unternehmensgruppe. PostgreSQL ist eine eingetragene Marke der PostgreSQL Community Association. Loonar Compass ist ein unabhängiges Produkt, steht in keiner Verbindung zu diesen Unternehmen und wird von ihnen weder unterstützt noch gesponsert.",
 f_stats="Keine Cookies. Anonyme Besuchsstatistik (GoatCounter), die keine personenbezogenen Daten dauerhaft speichert. Ihre Sprachwahl merkt sich Ihr Browser.",
 f_privacy_link="Datenschutz", f_legal_link="Impressum",
 f_top="Nach oben",
 doc_updated="Stand:", doc_home="Zur Startseite",
 gc_off="Meine Besuche in diesem Browser nicht zählen", gc_on="Meine Besuche wieder zählen",
 gc_state_on="Besuche aus diesem Browser werden (anonym) gezählt.", gc_state_off="Besuche aus diesem Browser werden nicht gezählt.",
 nf_title="Seite nicht gefunden", nf_text="Die gesuchte Seite gibt es nicht, oder sie wurde verschoben.", nf_link="Zur Startseite",
 m_today="Heute",
 m_convs=["Bestellprozess (BPMN)","Anforderungen aus einem PDF","Auswirkung: Zahlungsdienst"],
 m_user="Zeichne den Bestellprozess unseres Onlineshops als BPMN im Paket <b>Vertrieb</b>. Die Zahlung bitte als eingebetteten Teilprozess.",
 m_steps_h="Schritte der letzten Antwort: 3",
 m_steps=["Claude analysiert und plant (Schritt 1)","Lesen der Modelldatenbank: Paket Vertrieb","Speichern in EA, 14 Operationen: Diagramm Bestellprozess"],
 m_bot="Fertig. Im Paket <b>Vertrieb</b> gibt es ein neues BPMN-Diagramm <b>Bestellprozess</b>: 3 Tasks, ein Gateway und den eingebetteten Teilprozess <b>Zahlung</b> mit 3 Schritten. Ich habe es in Ihrem EA-Fenster geöffnet.",
 m_ph="Nachricht schreiben, Dateien hineinziehen oder Bild einfügen (Strg+V)…",
 m_mode="claude-haiku-5-5 · Modus ask",
 m_appr_h="Claude möchte Änderungen im Modell speichern",
 m_appr=["Neues BPMN-Diagramm „Bestellprozess“ im Paket Vertrieb","3 Tasks, 1 Gateway, Start- und Endereignis","Eingebetteter Teilprozess „Zahlung“ mit 3 Schritten","9 Sequenzflüsse"],
 m_approve="Freigeben", m_reject="Ablehnen",
 m_saveas="Speichern unter…", m_full="In voller Größe öffnen", m_png="Bestellprozess.png",
 d_recv=["Bestellung","annehmen"], d_gw="Auf Lager?", d_pay="Zahlung",
 dg_sub=[["Zahlart","wählen"],["Prüfen"],["Bestätigen"]],
 d_ship=["Bestellung","versenden"], d_notify=["Kunden","informieren"], d_yes="ja", d_no="nein",
 m_log_h="Änderungsprotokoll", m_log_btn="Nach Excel exportieren",
 m_log_cols=["Zeit","Ergebnis","EA-Konto","Änderungen"],
 m_log=[("14:02","OK","Bestellprozess: 14 Operationen"),("13:47","OK","Kunde: 3 Attribute, 2 Operationen"),("13:21","FEHLER","Versand: Paket von anderem Benutzer gesperrt"),("11:05","OK","Legende im Diagramm Kasse")],
 m_err="FEHLER",
)

T["fr"] = dict(
 locale="fr_FR", name_lang="Français",
 title="Loonar Compass – assistant IA pour Sparx Enterprise Architect",
 desc="Application Windows gratuite : dialoguez avec votre modèle Sparx Enterprise Architect. Posez des questions, créez et modifiez des diagrammes via EA, validez chaque modification. Utilise l’API Claude d’Anthropic avec votre propre clé API.",
 og_desc="Parlez à votre modèle Enterprise Architect : questionnez, dessinez, documentez — chaque modification est enregistrée par EA et validée par vous.",
 skip="Aller au contenu", lang_label="Langue",
 nav=["Fonctionnalités","Fonctionnement","Sécurité","Prérequis","FAQ","Soutenir"], nav_dl="Télécharger",
 eyebrow="Freeware · Windows · Sparx Enterprise Architect",
 h1='L’assistant IA pour Sparx Enterprise Architect — <em>parlez à votre modèle</em>',
 lead="Posez vos questions en langage naturel, faites dessiner et remanier vos diagrammes, transformez le modèle en documents. Chaque modification est enregistrée par Enterprise Architect lui-même — et seulement après votre validation.",
 cta1="Télécharger gratuitement", cta2="Découvrir les fonctionnalités",
 meta="Version <a href=\"{changelog}\">{ver}</a> · Windows 10/11 · utilise l’API Claude d’Anthropic avec votre propre clé API",
 hero_alt="Illustration de la fenêtre de Loonar Compass : une demande de dessiner en BPMN le processus de commande d’une boutique en ligne, la liste des étapes et la réponse de l’assistant.",
 f_h="Ce qu’il sait faire", f_sub="Une conversation au lieu de dizaines de boîtes de dialogue — de la question rapide au diagramme complet et homogène.",
 features=[
  ("Interrogez votre modèle","Des questions en langage naturel : que contient ce paquetage, quels éléments réalisent cette exigence, qu’est-ce qui a changé ? L’assistant interroge le modèle — référentiel PostgreSQL ou fichier .qea/.qeax — en lecture seule, et désigne chaque objet par son nom, son paquetage et son GUID. Chaque conversation peut travailler avec son propre modèle."),
  ("Créez et modifiez","Paquetages, éléments, diagrammes et connecteurs ; attributs et opérations de classes avec paramètres ; mots-clés, couleurs, position et taille. Déplacer, renommer, supprimer — ou copier un paquetage entier avec son contenu."),
  ("Du BPMN comme EA l’attend","De vrais types BPMN 2.0 avec stéréotypes et tags : tâches, événements, passerelles, flux, sous-processus réduits ou intégrés. À partir d’un fichier BPMN, d’une photo, d’un croquis ou d’une simple description."),
  ("Des diagrammes lisibles","Légendes avec coloration automatique, styles de ligne et tracé des connecteurs selon un chemin donné, connecteurs masqués sur un seul diagramme, tuiles de navigation vers d’autres diagrammes."),
  ("C’est EA qui enregistre","Les modifications passent par l’interface d’automatisation d’EA (COM) — si la sécurité du modèle est activée, avec un compte utilisateur EA, ses droits et ses verrous. Avant une restructuration importante, l’assistant a pour consigne de créer une baseline."),
  ("Vous validez chaque modification","Trois modes de validation : ask, plan et auto. Chaque enregistrement, même en échec, figure dans le journal des modifications avec l’heure, le compte et le résultat — exportable vers Excel."),
  ("Analyse d’impact","Depuis un élément, le long des relations jusqu’à six niveaux, avec les chemins et les diagrammes concernés. Où une opération de service est utilisée dans les diagrammes de séquence et d’activité — et lesquelles ne le sont nulle part."),
  ("Travaille avec votre EA ouvert","Connaît le diagramme ouvert et la sélection, ouvre des diagrammes, met en évidence les éléments dans le navigateur de projet et rafraîchit ce qu’il a modifié — sans F5."),
  ("Word, Excel et images","Documents Word avec les diagrammes d’EA, classeurs Excel (listes d’éléments, matrices, résultats d’analyse) et images PNG, par exemple une matrice cas d’utilisation–exigences."),
  ("Pages Confluence","Des pages issues du modèle et de la conversation : code pour l’éditeur de source, ou publication via l’API REST après votre validation, diagrammes en pièces jointes. À partir de modèles de documents modifiables."),
  ("Mémoire de l’assistant","Retient ce que vous convenez — conventions de modélisation, préférences, corrections — dans des fichiers Markdown locaux et l’applique dans chaque nouvelle conversation, quel que soit le modèle. Une note n’est limitée à un seul modèle que si vous le demandez."),
  ("Quatre langues","Français, anglais, polonais et allemand : l’application, les réponses de l’assistant, les modèles de documents et le guide de l’utilisateur."),
 ],
 h_h="Comment ça marche", h_sub="Vous gardez la main à chaque étape.",
 steps=[
  ("Vous demandez","Saisissez une question ou une tâche ; joignez un PDF, une capture d’écran, un croquis ou un fichier BPMN."),
  ("Claude planifie","Le modèle Claude décide quoi consulter et quoi modifier. Vous suivez chaque étape en direct dans le chat."),
  ("Il lit et écrit le modèle","Lecture directe du référentiel PostgreSQL ou du fichier .qea/.qeax (en lecture seule) ; écriture uniquement à travers Enterprise Architect (COM)."),
  ("Vous validez","Vous voyez les modifications prévues, vous les validez ou les refusez. L’enregistrement est consigné au journal et apparaît aussitôt dans votre fenêtre EA."),
 ],
 s_h="Aperçu", s_sub="L’interface avec un contenu d’exemple : une boutique en ligne fictive.",
 caps=["Chaque enregistrement attend votre validation (mode ask).","Les diagrammes reviennent dans le chat sous forme d’image, tels qu’EA les dessine.","Le journal des modifications consigne chaque enregistrement — exportable vers Excel."],
 sec_h="Sécurité et confidentialité", sec_sub="Votre modèle reste où il est. L’assistant travaille sur votre ordinateur, avec vos comptes.",
 sec=[
  ("Fonctionne en local","Une application de bureau pour Windows, installée pour votre compte utilisateur. Paramètres, historique des conversations, mémoire et journal des modifications sont stockés dans votre profil utilisateur."),
  ("Les secrets restent dans Windows","La clé API Claude, le mot de passe du compte EA, le mot de passe de la base et le jeton Confluence sont conservés dans le Gestionnaire d’identification Windows — jamais en clair dans le fichier de paramètres."),
  ("Connexions chiffrées","L’API Claude et Confluence ne sont utilisés qu’en HTTPS. La connexion à PostgreSQL peut être chiffrée en TLS — de préférence, obligatoirement, ou obligatoirement avec vérification du certificat du serveur dans le magasin de certificats Windows. Un fichier de modèle (.qea, .qeax) ne demande aucune connexion à une base : l’assistant le lit directement, en lecture seule."),
  ("Écriture uniquement via EA","La session de base de données est en lecture seule, et un fichier de modèle est ouvert en lecture seule. Chaque modification est effectuée par Enterprise Architect — si la sécurité est activée, avec le compte EA configuré, ses droits et ses verrous. Le contenu des pièces jointes, des documents et du modèle est traité comme des données, et non comme des instructions."),
  ("Ce qui est envoyé à Anthropic","Pour répondre, Claude reçoit la conversation, vos pièces jointes, les notes mémorisées, les modèles utilisés, les résultats des requêtes de l’assistant et les images de diagrammes affichées — pas tout le modèle. Vérifiez les règles de votre organisation avant de l’utiliser sur des modèles d’entreprise."),
  ("Votre propre clé API","L’application utilise votre clé API Claude issue de la console Anthropic. L’application est gratuite ; Anthropic facture l’utilisation de Claude au compte auquel appartient la clé."),
 ],
 r_h="Prérequis", r_need="Indispensable", r_opt="Facultatif",
 req=[
  "Windows 10 ou 11 (64 bits)",
  "Sparx Enterprise Architect avec licence — pour enregistrer, l’assistant lance EA en arrière-plan (EA occupe alors une place de licence) ou, pour un fichier de modèle ouvert dans votre EA, passe par votre fenêtre",
  "Un modèle EA sous forme de fichier de projet .qea ou .qeax, ou dans une base PostgreSQL avec la sécurité activée (« Require User Lock to Edit »)",
  "Pour un modèle en base : l’accès à la base (serveur, port, nom, compte) et un compte utilisateur EA pour l’assistant, avec droits de modification ; pour un fichier de modèle, son chemin suffit (un compte seulement si la sécurité est activée)",
  "Une clé API Claude de la console Anthropic et une connexion Internet",
 ],
 opt=[
  "Confluence (Data Center ou Cloud) — pour publier directement des pages",
  "Un logiciel de messagerie par défaut sous Windows — pour signaler des bugs et recevoir les nouveautés",
 ],
 r_note="Les anciens formats de fichier (.eap, .eapx, .feap) et les autres bases de données ne sont pas pris en charge ; EA convertit un fichier .eap/.eapx en .qea par un transfert de projet (Settings → Model → Transfer → Full Project Transfer).",
 faq_h="Questions fréquentes", faq_sub="Des réponses claires sur le coût, les prérequis, la sécurité et la confidentialité.",
 faq=[
  ("Est-ce vraiment gratuit ? Puis-je l’utiliser au travail ?",
   '<p>Oui. Loonar Compass est un freeware : gratuit pour tout usage, y compris professionnel et commercial, sur autant d’ordinateurs que vous le souhaitez. Vous pouvez aussi le transmettre à d’autres personnes — gratuitement, en entier, sans modification et avec le fichier de licence.</p>'
   '<p>Le code source est public sur <a href="{repo}">GitHub</a> : vous pouvez voir exactement ce que l’application fait de votre modèle. Il est consultable (source-available), mais ce n’est pas de l’open source : sont interdites la modification de l’application et la création d’œuvres dérivées, ainsi que sa vente ou sa location. <a href="{lic}">Lire la licence</a>.</p>'),
  ("Combien coûte son utilisation, et quel modèle Claude utilise-t-il ?",
   '<p>L’application est gratuite. L’assistant fonctionne avec votre propre clé API Claude, issue de la console Anthropic, et Anthropic facture l’utilisation au compte auquel appartient la clé — à la consommation, c’est-à-dire selon la quantité de texte que le modèle lit et écrit. Les tarifs actuels figurent sur la <a href="https://claude.com/pricing#api">page de tarifs d’Anthropic</a>.</p>'
   '<p>Le modèle est toujours Claude Haiku 5.5 (<code>claude-haiku-5-5</code>) ; ce n’est pas un paramètre. « Jetons max. » (10\u202f000 par défaut) limite seulement la longueur d’une réponse : vous payez le texte réellement généré.</p>'),
  ("Sur quels systèmes fonctionne-t-il ?",
   '<p>Sur Windows 10 et Windows 11, en 64 bits. C’est une application de bureau Windows native, qui travaille avec Enterprise Architect installé sur le même ordinateur.</p>'),
  ("Quelles configurations d’Enterprise Architect sont prises en charge ?",
   '<ul><li>Enterprise Architect installé sur le même ordinateur, avec une licence valide. Pour enregistrer, l’assistant le lance en arrière-plan, où il occupe une place de licence ; il se ferme après 10 minutes sans activité et à la fermeture de l’application. Si le fichier de modèle est ouvert dans votre EA, les modifications passent plutôt par votre fenêtre — un second EA n’ouvre jamais le même fichier.</li>'
   '<li>Le modèle sous forme de fichier de projet EA .qea ou .qeax — dans les Paramètres, « 2. Modèle » → « Fichier EA (.qea, .qeax) » et son chemin dans « Fichier du modèle », sans serveur de base, compte ni mot de passe — ou dans une base PostgreSQL accessible depuis votre ordinateur (serveur, port, nom de la base, compte). Un fichier .qea est prévu pour une seule personne, un .qeax pour une petite équipe qui partage le fichier (Sparx le recommande pour moins de 5 utilisateurs). Les anciens formats de fichier (.eap, .eapx, .feap) et les référentiels dans d’autres bases ne sont pas pris en charge ; Enterprise Architect convertit un fichier .eap/.eapx en .qea par un transfert de projet (Settings → Model → Transfer → Full Project Transfer).</li>'
   '<li>Pour un modèle en base : la sécurité utilisateur activée (Settings → Security → Enable Security), en mode « Require User Lock to Edit ». Dans un fichier de modèle, la sécurité est facultative ; sans elle, l’assistant n’a pas besoin de compte EA.</li>'
   '<li>Si la sécurité est activée : un compte utilisateur EA pour l’assistant, avec mot de passe et droits de modification — de préférence un compte distinct, pour distinguer facilement ses modifications et ses verrous des vôtres.</li><li>Plusieurs modèles : chaque conversation a son propre modèle, affiché dans son en-tête (« Modèle : … »). Une nouvelle conversation utilise le modèle par défaut des Paramètres ; « Choisir un autre modèle… » — ou un clic sur l’en-tête, qui ouvre « Modèle de cette conversation » — permet de choisir une autre base PostgreSQL (serveur, port, base, compte, mot de passe, chiffrement) ou un autre fichier EA (.qea, .qeax), avec son compte EA, et de tester la connexion. Un fichier .qea/.qeax déposé sur la fenêtre devient le modèle de la conversation. Les conversations des versions précédentes reçoivent une copie du modèle des Paramètres. Enterprise Architect en arrière-plan garde un seul modèle ouvert à la fois ; le basculer vers un autre modèle prend de quelques secondes à environ 25 secondes.</li></ul>'
   '<p>L’application ne vérifie ni la version ni l’édition d’EA. L’essentiel est que votre Enterprise Architect puisse ouvrir ce modèle : le fichier, ou le référentiel en base avec la sécurité activée.</p>'),
  ("Faut-il des droits d’administrateur pour l’installer ?",
   '<p>Non. Par défaut (« Pour moi uniquement »), le programme d’installation installe Loonar Compass uniquement pour votre compte utilisateur, dans <code>%LOCALAPPDATA%\\Programs\\Loonar Compass</code>, sans droits d’administrateur. Si vous choisissez plutôt « Pour tous les utilisateurs de cet ordinateur » (dans C:\\Program Files\\Loonar Compass), le programme d’installation demande des droits d’administrateur.</p>'
   '<p>L’application elle-même s’exécute toujours en tant qu’utilisateur standard, et chaque utilisateur a ses propres paramètres et conversations. Si une autre personne valide une installation pour tous les utilisateurs avec un compte administrateur, saisissez votre configuration plus tard dans l’application (Paramètres), pas dans le programme d’installation — sinon elle serait enregistrée dans le profil de l’administrateur.</p>'),
  ("Pourquoi Windows SmartScreen peut-il mettre en garde contre le programme d’installation ?",
   '<p>Parce que le programme d’installation de la version {ver} n’est pas encore signé numériquement (avec un certificat de signature de code) : Windows ne connaît donc pas son éditeur. Microsoft Defender SmartScreen peut ainsi afficher « Windows a protégé votre ordinateur » au lancement, et votre navigateur peut signaler que ce fichier est rarement téléchargé. L’installation par défaut pour votre compte utilisateur ne demande pas de droits d’administrateur, donc aucune demande d’autorisation administrateur n’apparaît ; seule l’installation pour tous les utilisateurs en demande — et affiche alors un éditeur inconnu. Un programme d’installation signé est prévu et devrait suivre prochainement. D’ici là :</p>'
   '<ul><li>Téléchargez le programme d’installation uniquement depuis loonar.eu ou depuis les <a href="{releases}">versions publiées du projet sur GitHub</a>, en HTTPS.</li>'
   '<li>Comparez sa somme de contrôle avec le SHA-256 affiché à côté du bouton de téléchargement — dans PowerShell : <code>Get-FileHash .\\{setup}</code>.</li>'
   '<li>Dans la fenêtre SmartScreen, cliquez sur « Informations complémentaires », puis sur « Exécuter quand même ».</li></ul>'
   '<p>Sous Windows 11, lorsque le Contrôle intelligent des applications (Smart App Control) est activé, les applications non signées sont bloquées et ne peuvent pas être lancées ainsi ; attendez alors la version signée.</p>'),
  ("L’assistant peut-il abîmer mon modèle ?",
   '<p>Il n’enregistre que ce que vous autorisez, et uniquement de la manière permise par Enterprise Architect :</p>'
   '<ul><li><strong>Modes de validation :</strong> ask — vous validez chaque enregistrement ; plan — une validation par réponse ; auto — sans question, pour les modèles de test. La publication dans Confluence demande toujours votre validation, et la demande indique explicitement quand une page existante sera remplacée.</li>'
   '<li><strong>Écriture uniquement via EA :</strong> la base est lue dans une session en lecture seule, un fichier de modèle en lecture seule ; chaque modification est effectuée par Enterprise Architect — si la sécurité est activée, avec le compte EA de l’assistant, ses droits et ses verrous. Les objets verrouillés par d’autres utilisateurs ne sont pas touchés.</li>'
   '<li><strong>Vos instructions uniquement :</strong> l’assistant a pour consigne de traiter le contenu des pièces jointes, des documents et du modèle comme des données, et non comme des instructions à suivre.</li>'
   '<li><strong>Journal des modifications :</strong> chaque enregistrement, même en échec, y figure avec l’heure, le compte et le résultat, et peut être exporté vers Excel.</li>'
   '<li><strong>Baselines :</strong> avant de supprimer ou de restructurer en profondeur un paquetage existant, l’assistant a pour consigne d’en créer d’abord une baseline ; vous la restaurez dans EA (Manage Baselines → Compare/Restore).</li></ul>'
   '<p>Cela dit, une suppression est définitive et l’IA peut se tromper — l’application le rappelle : relisez donc les modifications proposées avant de les valider. Pour les modèles importants, utilisez le mode ask et sauvegardez la base ou le fichier du modèle — l’application est fournie sans garantie.</p>'),
  ("Quelles données quittent mon ordinateur, et où vont-elles ?",
   '<p>Pour répondre, l’assistant envoie à l’API Claude d’Anthropic (api.anthropic.com) uniquement ce dont il a besoin : vos messages et pièces jointes, les notes mémorisées, les modèles de documents utilisés et ce qu’il a lu dans le modèle au cours de la conversation — les résultats de ses requêtes sur le modèle (noms et descriptions des paquetages, éléments, diagrammes) et les images des diagrammes affichés. Pas tout le modèle. Après un changement de langue, la mémoire et les modèles de documents sont aussi envoyés pour traduction.</p>'
   '<p>Pour le reste, les données ne vont que là où vous les envoyez : les pages publiées dans votre Confluence ; les signalements de bug et l’inscription à la newsletter sont des e-mails que vous vérifiez et envoyez vous-même depuis votre messagerie. L’application n’envoie rien d’elle-même à Loonar. Avant de l’utiliser sur des modèles d’entreprise, vérifiez les règles de votre organisation et les conditions de traitement des données de votre compte Anthropic.</p>'),
  ("Où sont stockés mes paramètres, mes conversations et mes mots de passe ?",
   '<p>Sur votre ordinateur, dans votre profil utilisateur Windows — chaque utilisateur de l’ordinateur a les siens :</p>'
   '<ul><li><code>%APPDATA%\\Loonar Compass</code> : paramètres (app.properties), historique des conversations, pièces jointes, mémoire de l’assistant, modèles de documents et journal des modifications.</li>'
   '<li>Gestionnaire d’identification Windows : la clé API Claude, le mot de passe du compte EA, le mot de passe de la base et le jeton Confluence, ainsi que les mots de passe du modèle d’une conversation qui diffère des Paramètres. Aucun d’eux n’est enregistré en clair dans le fichier de paramètres.</li></ul>'
   '<p>Le programme lui-même s’installe par défaut dans <code>%LOCALAPPDATA%\\Programs\\Loonar Compass</code>, ou dans C:\\Program Files\\Loonar Compass s’il est installé pour tous les utilisateurs.</p>'),
  ("Les communications sont-elles chiffrées ?",
   '<p>Oui, partout où l’application établit elle-même une connexion :</p>'
   '<ul><li><strong>API Claude et Confluence :</strong> uniquement en HTTPS. Une adresse Confluence en <code>http://</code> n’est acceptée que pour un serveur situé sur le même ordinateur (localhost).</li>'
   '<li><strong>Base de données du modèle (PostgreSQL) :</strong> TLS, à régler avec « Chiffrement de la connexion » sur la carte « 2. Modèle » des Paramètres (option « Base PostgreSQL ») : « Préféré » (par défaut) — TLS dès que le serveur le propose, sinon une connexion non chiffrée ; « Obligatoire » — TLS ou pas de connexion ; « Obligatoire, certificat vérifié » — TLS, avec vérification du certificat et du nom du serveur dans le magasin de certificats Windows, y compris les certificats de votre entreprise. « Préféré » et « Obligatoire » ne vérifient pas le certificat. Après « Enregistrer et tester la connexion », l’application indique si la connexion est chiffrée. Dans app.properties, le paramètre est <code>db.sslmode</code> = <code>prefer</code>, <code>require</code> ou <code>verify-full</code>.</li>'
   '<li><strong>Fichier du modèle (.qea, .qeax) :</strong> aucune connexion à une base — l’assistant lit le fichier directement, en lecture seule.</li></ul>'
   '<p>Conseil pour les administrateurs de bases de données : avec des entrées <code>hostssl</code> dans pg_hba.conf, le serveur n’accepte que des connexions TLS, quel que soit le client. La connexion propre d’Enterprise Architect à la base suit les paramètres d’EA, et non ceux de l’application.</p>'),
  ("Fonctionne-t-il derrière un proxy ou un pare-feu d’entreprise ?",
   '<p>Il faut autoriser ces connexions : HTTPS (port 443) vers api.anthropic.com, pour un modèle en base votre serveur PostgreSQL (port 5432 par défaut ; connexion directe, pas via le proxy) et, si vous publiez des pages, votre Confluence.</p>'
   '<p>Pour le HTTPS, l’application utilise le proxy défini dans les options Internet de Windows (proxy système). Vous pouvez aussi définir la variable d’environnement <code>HTTPS_PROXY</code> ou <code>ALL_PROXY</code>, par exemple <code>http://proxy.example.com:8080</code> (exceptions dans <code>NO_PROXY</code>) ; elle a priorité sur le proxy système. Redémarrez l’application après une modification. Avec une authentification de base (basic), le nom d’utilisateur et le mot de passe peuvent figurer dans l’adresse de cette variable ; la détection automatique et les scripts de configuration automatique (WPAD, PAC) ainsi que l’authentification au proxy avec le compte Windows (NTLM, Kerberos) ne sont pas pris en charge.</p>'
   '<p>Les certificats HTTPS sont aussi vérifiés avec le magasin de certificats Windows : un certificat racine d’entreprise utilisé pour l’inspection du trafic est donc accepté dès qu’il est installé dans Windows.</p>'),
  ("Dans quelles langues est-il disponible ?",
   '<p>En français, anglais, polonais et allemand : fenêtres de l’application et du programme d’installation, réponses de l’assistant, modèles de documents par défaut, guide de l’utilisateur, licence et liste des nouveautés. Le programme d’installation commence par le choix de la langue (anglais par défaut) ; vous la changez ensuite dans les Paramètres, sur la carte « 4. Langue et validation des modifications ».</p>'
   '<p>L’assistant répond dans la langue de l’application, sauf si vous lui écrivez dans une autre. L’historique des conversations n’est jamais traduit.</p>'),
  ("Fonctionne-t-il avec Confluence Cloud et Data Center ?",
   '<p>Oui, avec les deux — et Confluence reste facultatif. Sans lui, l’assistant prépare quand même les pages : vous collez le code vous-même dans l’éditeur de source (« Copier le code de la page », « Ouvrir le dossier des images »).</p>'
   '<p>Pour publier directement, remplissez la carte « 6. Confluence » dans les Paramètres : pour Data Center, l’adresse et un jeton d’accès personnel (PAT) ; pour Cloud, l’adresse (https://votre-entreprise.atlassian.net/wiki), l’e-mail du compte et un jeton API ; ainsi que la clé de l’espace et, si besoin, une page parente. L’assistant crée ou met à jour les pages via l’API REST, avec les diagrammes en pièces jointes — toujours après votre validation (la demande indique explicitement quand une page existante sera remplacée ; la version précédente reste dans l’historique de la page) et avec les droits de ce jeton.</p>'),
  ("Comment mettre à jour ? Mes paramètres et mes conversations sont-ils conservés ?",
   '<p>Fermez l’application et lancez le programme d’installation de la nouvelle version. Il propose le même dossier, affiche les nouveautés depuis votre version et conserve vos paramètres, l’historique des conversations, les pièces jointes, la mémoire, les modèles de documents et le journal des modifications ; la clé, les mots de passe et les jetons du Gestionnaire d’identification restent aussi en place. Les nouvelles versions sont publiées ici et sur <a href="{releases}">GitHub</a>.</p>'),
  ("Comment le désinstaller, et que reste-t-il sur l’ordinateur ?",
   '<p>Fermez l’application, puis utilisez « Désinstaller Loonar Compass » dans le menu Démarrer ou son entrée dans Paramètres Windows → Applications → Applications installées. Les fichiers du programme, les raccourcis et l’entrée système sont supprimés. Vos données restent, sauf si vous cochez l’une de ces options :</p>'
   '<ul><li>« Supprimer les paramètres, les conversations, les pièces jointes, la mémoire de l’assistant, les modèles de documents et le journal des modifications » — efface tout le dossier <code>%APPDATA%\\Loonar Compass</code> ;</li>'
   '<li>« Supprimer la clé API, les mots de passe et les jetons du Gestionnaire d’identification » — la clé API Claude, les mots de passe du compte EA et de la base, et le jeton Confluence.</li></ul>'
   '<p>Votre modèle EA, les fichiers enregistrés ailleurs et les pages publiées dans Confluence ne sont pas touchés.</p>'),
  ("Comment signaler un bug ou être informé des nouvelles versions ?",
   '<p>Dans l’application, ouvrez « Signaler un bug » dans la barre de navigation, décrivez ce qui s’est passé, ajoutez des captures d’écran et cliquez sur « Envoyer le signalement » : un message prêt à partir vers mariusz@loonar.eu s’ouvre dans votre messagerie par défaut ; vous le vérifiez et l’envoyez. Les informations techniques (version de l’application, version de Windows, modèle d’IA, mode de validation, dernier message — sans mots de passe ni clés) ne sont jointes que si cette option reste cochée (elle l’est par défaut).</p>'
   '<p>Pour les nouvelles versions, saisissez votre e-mail sur la carte « 5. Nouvelles versions (newsletter) » des Paramètres et cochez le consentement. Ou écrivez simplement à <a href="mailto:mariusz@loonar.eu">mariusz@loonar.eu</a>.</p>'),
 ],
 d_h="Télécharger", d_sub="Gratuit pour tous — téléchargez, installez, connectez votre modèle.",
 d_meta="Programme d’installation Windows · {size} Mo · {date}",
 d_btn="Télécharger l’installateur", d_hash="Somme de contrôle SHA-256 :",
 d_lic="Freeware : gratuit pour tout usage, y compris commercial. La modification de l’application est interdite.",
 d_lic_link="Lire la licence",
 d_lang="Le programme d’installation commence par le choix de la langue : anglais (par défaut), polonais, allemand ou français.",
 d_update="Pour mettre à jour, lancez simplement un installateur plus récent — vos paramètres et votre historique sont conservés.",
 d_admin="S’installe pour votre compte utilisateur — sans droits d’administrateur.",
 gh_label="Code source sur GitHub", d_all="Toutes les versions",
 sup_h="Soutenir le projet",
 sup_text="Loonar Compass est gratuit, y compris pour un usage commercial, et le restera. S’il vous fait gagner du temps, vous pouvez soutenir son développement par une contribution volontaire. Elle finance notamment un certificat de signature de code, grâce auquel Windows reconnaîtra l’application comme provenant d’un éditeur vérifié.",
 sup_btn="Soutenir sur suppi.pl",
 sup_more='Autres façons d’aider : signalez les bugs et les idées (« Signaler un bug » dans l’application), recommandez l’application à d’autres utilisateurs d’Enterprise Architect ou donnez une étoile au projet sur <a href="{repo}">GitHub</a>.',
 ab_h="À propos de moi",
 ab_why_h="Pourquoi cette application",
 ab_why="L’IA excelle à produire des diagrammes sous forme de code — PlantUML, Mermaid. Mais c’est du dessin de diagrammes, pas de la modélisation : chaque diagramme n’est qu’une vue du moment, et la connaissance du système n’est conservée nulle part. Simon Brown, l’auteur du modèle C4, distingue précisément diagrammes et modèles : un modèle est une source unique de vérité sur les éléments et leurs relations, d’où l’on tire de nombreuses vues cohérentes — et c’est ce qui rend possible la gouvernance, comme l’analyse d’impact et le contrôle de cohérence. Enterprise Architect conserve un tel modèle. Loonar Compass vous permet de travailler avec ce modèle en conversant avec une IA.",
 ab_name_h="Pourquoi « Compass »",
 ab_name="Une boussole donne l’orientation. Le logo est une constellation de nœuds : comme un navigateur qui trouve sa route grâce aux étoiles, l’assistant vous aide à vous orienter dans un grand modèle d’architecture.",
 ab_role="Principal Technical Product Manager · formateur en ingénierie des exigences (IREB CPRE)",
 ab_me_h="Qui est derrière",
 ab_me="Je suis Principal Technical Product Manager, basé à Stargard (Pologne), et je conçois depuis des années des produits numériques — de la stratégie produit, des études de marché, de l’analyse des clients et des exigences jusqu’à la livraison — pour des services cloud et de télécommunication, y compris l’intégration d’AWS, d’Azure et de Google Cloud avec des systèmes sur site. Diplômé en informatique de l’Université de technologie de Szczecin (Politechnika Szczecińska), je suis aussi formateur en ingénierie des exigences (IREB CPRE). Je ne suis pas développeur : Loonar Compass a été créé avec des assistants de programmation IA — je décide de ce que l’application doit faire et de son comportement pour les architectes, et le code a été écrit par l’IA. C’est sa force : elle est conçue par quelqu’un qui travaille chaque jour avec des modèles Enterprise Architect et sait ce dont les architectes ont besoin.",
 ab_li="Mariusz Głowacki sur LinkedIn",
 c_h="Contact", c_text='Une question, une idée, un bug ? Écrivez à <a href="mailto:mariusz@loonar.eu">mariusz@loonar.eu</a> — ou utilisez « Signaler un bug » dans l’application.',
 f_addr="ul. Gombrowicza 12, 73-110 Stargard, Pologne",
 f_tm="Sparx Systems et Enterprise Architect sont des marques déposées de Sparx Systems Pty Ltd. Claude et Anthropic sont des marques d’Anthropic, PBC. Atlassian et Confluence sont des marques d’Atlassian. Microsoft, Windows, Word, Excel, GitHub et LinkedIn sont des marques du groupe Microsoft. PostgreSQL est une marque déposée de la PostgreSQL Community Association. Loonar Compass est un produit indépendant, sans lien avec ces sociétés, qui ne l’approuvent ni ne le parrainent.",
 f_stats="Pas de cookies. Statistiques de visite anonymes (GoatCounter), qui ne conservent aucune donnée personnelle. Votre choix de langue est mémorisé par votre navigateur.",
 f_privacy_link="Confidentialité", f_legal_link="Mentions légales",
 f_top="Haut de page",
 doc_updated="Dernière mise à jour :", doc_home="Retour à la page d’accueil",
 gc_off="Ne pas compter mes visites dans ce navigateur", gc_on="Compter de nouveau mes visites",
 gc_state_on="Les visites depuis ce navigateur sont comptées (anonymement).", gc_state_off="Les visites depuis ce navigateur ne sont pas comptées.",
 nf_title="Page introuvable", nf_text="La page que vous cherchez n’existe pas ou a été déplacée.", nf_link="Aller à la page d’accueil",
 m_today="Aujourd’hui",
 m_convs=["Processus de commande (BPMN)","Exigences d’un PDF","Impact : service de paiement"],
 m_user="Dessine le processus de commande de notre boutique en ligne en BPMN dans le paquetage <b>Ventes</b>. Le paiement doit être un sous-processus intégré.",
 m_steps_h="Étapes de la dernière réponse : 3",
 m_steps=["Claude analyse et planifie (étape 1)","Lecture de la base de données du modèle : paquetage Ventes","Enregistrement dans EA, 14 opérations : diagramme Processus de commande"],
 m_bot="C’est fait. Le paquetage <b>Ventes</b> contient un nouveau diagramme BPMN <b>Processus de commande</b> : 3 tâches, une passerelle et le sous-processus intégré <b>Paiement</b> en 3 étapes. Je l’ai ouvert dans votre fenêtre EA.",
 m_ph="Écrivez un message, glissez des fichiers ou collez une image (Ctrl+V)…",
 m_mode="claude-haiku-5-5 · mode ask",
 m_appr_h="Claude veut enregistrer des modifications dans le modèle",
 m_appr=["Nouveau diagramme BPMN « Processus de commande » dans Ventes","3 tâches, 1 passerelle, événements de début et de fin","Sous-processus intégré « Paiement » en 3 étapes","9 flux de séquence"],
 m_approve="Valider", m_reject="Refuser",
 m_saveas="Enregistrer sous…", m_full="Ouvrir en taille réelle", m_png="Processus de commande.png",
 d_recv=["Recevoir la","commande"], d_gw="En stock ?", d_pay="Paiement",
 dg_sub=[["Choisir le","moyen"],["Autoriser"],["Confirmer"]],
 d_ship=["Expédier la","commande"], d_notify=["Prévenir","le client"], d_yes="oui", d_no="non",
 m_log_h="Journal des modifications", m_log_btn="Exporter vers Excel",
 m_log_cols=["Heure","Résultat","Compte EA","Modifications"],
 m_log=[("14:02","OK","Processus de commande : 14 opérations"),("13:47","OK","Client : 3 attributs, 2 opérations"),("13:21","ERREUR","Expédition : paquetage verrouillé par un autre utilisateur"),("11:05","OK","Légende du diagramme Caisse")],
 m_err="ERREUR",
)

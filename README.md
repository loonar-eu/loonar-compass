<p align="center">
  <img src="assets/brand/loonar-logo-1024.png" alt="Loonar Compass logo" width="120">
</p>

<h1 align="center">Loonar Compass</h1>

<p align="center">
  A free AI assistant for <b>Sparx Enterprise Architect</b>: talk to your model in plain language.<br>
  <a href="https://loonar.eu">loonar.eu</a> · English · Polski · Deutsch · Français
</p>

---

Loonar Compass is a Windows desktop application that connects to an Enterprise Architect model
stored in PostgreSQL and lets you work with it through a chat. It uses Anthropic's Claude API with
your own API key.

- **Ask about the model** – which elements realise a requirement, what changed, where an operation
  is used. The assistant queries the model repository (read-only) and answers with names, packages
  and GUIDs.
- **Create and change** packages, elements, diagrams, connectors, BPMN processes (including embedded
  subprocesses), attributes and operations, keywords, legends and line styles – always through EA's
  own automation interface (COM), with an EA user account, so EA security and locks apply.
- **You approve every change** (modes *ask*, *plan* and *auto*); every write lands in an audit log
  that can be exported to Excel. Big rebuilds can be preceded by a baseline.
- **Impact analysis** across relationships and diagrams, and *where is this operation used*.
- **Works with the open EA window** – knows the current diagram and selection, opens diagrams and
  refreshes what it changed.
- **Documents** – Word, Excel and PNG images (e.g. matrices) and Confluence pages, from editable
  document templates; Confluence pages can be published through the REST API.
- **Memory** – remembers agreements and preferences as local Markdown files.
- **Four languages** – English, Polish, German and French (UI, documentation, templates; the
  assistant answers in your language).

## Download

Get the installer from **[loonar.eu](https://loonar.eu)**. The installer starts with a language
choice and keeps your settings and conversations on update. The current version is not digitally
signed yet, so Windows SmartScreen may warn about an unknown publisher; compare the SHA-256 checksum
shown on the download page.

## Requirements

- Windows 10 or 11 (64-bit)
- Sparx Enterprise Architect with a licence, installed on the same computer
- An EA model in a PostgreSQL repository, with model security enabled and an EA user account for
  the assistant
- A Claude API key ([Anthropic Console](https://console.anthropic.com)); API usage is billed by
  Anthropic to your account
- Optional: Confluence (Cloud or Data Center) for publishing pages, a Windows mail program for bug
  reports

## Privacy

The application runs on your computer and has no telemetry. To answer, it sends the conversation,
attachments and the model excerpts it needs directly to the Anthropic API under your account.
Secrets (API key, passwords, tokens) are kept in the Windows Credential Manager. Model changes are
made only through Enterprise Architect with the permissions of the configured EA user. Details:
[loonar.eu/privacy](https://loonar.eu/privacy/).

## Building from source

The repository is published so that you can see exactly what the application does with your model.

Prerequisites: Windows, [Rust](https://rustup.rs) (stable, MSVC toolchain), Visual Studio Build
Tools with the Windows SDK, Microsoft Edge (for the PDF manuals), Python 3 (website generator only).

```powershell
cargo build --release            # the application: target\release\
cargo test --release             # unit tests (tests marked #[ignore] need EA, a licence and a database)
.\build_manual.ps1               # user manuals (HTML -> PDF) in four languages
.\build_installer.ps1            # installer: dist\
```

| Path | Contents |
|---|---|
| `src/` | the application (egui UI, Claude client, tools, EA COM automation, documents) |
| `installer/` | the setup wizard (its own small Rust program) |
| `i18n/strings.src` | UI texts: `en ‖ pl ‖ de ‖ fr` per line |
| `templates/` | default document templates per language |
| `docs/` | user manuals (HTML sources) and screenshots |
| `website/`, `website_src/` | the loonar.eu site and its generator |
| `assets/` | icons and logo (`assets/brand/` has the logo in several variants) |

## Licence

Loonar Compass is **freeware**: free to use for any purpose, including commercial use, on any number
of computers, and free to pass on unchanged. The source code is published for transparency; the
licence does not allow modified versions or derivative works. See [LICENSE.en.txt](LICENSE.en.txt)
(English translation) – the Polish [LICENSE.txt](LICENSE.txt) is binding. Third-party components keep
their own licences, listed in `THIRD-PARTY-NOTICES.txt`.

Bug reports and ideas are welcome as GitHub issues or from the app (*Report a bug*). Pull requests
cannot be accepted. Security issues: see [SECURITY.md](SECURITY.md).

## How it is made

Loonar Compass is designed and product-managed by Mariusz Głowacki, a Principal Technical Product
Manager who works with Enterprise Architect models every day. The code is written with AI coding
assistants: the product decisions — what the application does and how it must behave for architects —
are his; the implementation is AI-generated, reviewed and tested against real EA models.

## Support the project

Loonar Compass is free. If it saves you time, you can support its development (and a code-signing
certificate) at **[suppi.pl/loonar](https://suppi.pl/loonar)**.

---

Sparx Systems and Enterprise Architect are registered trademarks of Sparx Systems Pty Ltd. Claude and
Anthropic are trademarks of Anthropic, PBC. Atlassian, Confluence and Jira are trademarks of
Atlassian. Microsoft, Windows, Word and Excel are trademarks of the Microsoft group of companies.
PostgreSQL is a trademark of the PostgreSQL Community Association of Canada. Loonar Compass is an
independent product of Loonar Mariusz Głowacki and is not affiliated with, endorsed or sponsored by
these companies.

© 2026 Mariusz Głowacki (Loonar) · ul. Gombrowicza 12, 73-110 Stargard, Poland · mariusz@loonar.eu

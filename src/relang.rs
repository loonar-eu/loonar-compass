//! Moving the user's content to the new language after the language is changed in the settings.
//!
//! - Untouched default templates are replaced with the built-in ones in the new language (no AI).
//! - Claude translates the assistant memory and the user's own and modified templates.
//! - Conversation history is never translated: it stays in the original language.
//! Without a connection to Claude the task is saved in the settings (`ui.language_pending = pl>en`)
//! and retried at the next opportunity (application start, saving the settings, the "Translate now" button).

use crate::config::{self, secrets, AppConfig};
use crate::i18n::Lang;
use crate::{memory, templates};
use anyhow::{bail, Context, Result};
use serde_json::json;
use std::path::Path;

const PENDING_KEY: &str = "ui.language_pending";

/// Pending move from the settings: (from language, to language).
pub fn pending(props: &Path) -> Option<(Lang, Lang)> {
    let saved = std::fs::read_to_string(props).map(|t| config::parse_properties(&t)).unwrap_or_default();
    let value = saved.get(PENDING_KEY)?.trim().to_string();
    let (from, to) = value.split_once('>')?;
    Some((Lang::parse(from), Lang::parse(to)))
}

pub fn set_pending(props: &Path, from: Lang, to: Lang) -> Result<()> {
    // When a move from another language was already pending, the original source language counts.
    let origin = pending(props).map(|(f, _)| f).unwrap_or(from);
    let value = if origin == to { String::new() } else { format!("{}>{}", origin.code(), to.code()) };
    config::save_properties(props, &[(PENDING_KEY, value)])
}

fn clear_pending(props: &Path) -> Result<()> {
    config::save_properties(props, &[(PENDING_KEY, String::new())])
}

/// Translation of one text by Claude (keeps the Markdown structure and the application's markers).
fn translate(cfg: &AppConfig, key: &str, text: &str, to: Lang) -> Result<String> {
    let client = crate::claude::ClaudeClient::new(key.to_string(), cfg.claude_model.clone(), 8000)?;
    let system = format!(
        "You translate documents into {}. Return ONLY the translated document, nothing else. Keep the Markdown structure, \
         tables, the first line marker word before the colon translated naturally (e.g. 'Template:', 'Vorlage:', 'Modèle:', 'Szablon:'), \
         and keep unchanged: placeholders like {{status:Colour:TEXT}} (translate only TEXT), {{jira:...}}, {{user:...}}, ![caption](diagram:ID) \
         and ![caption](image:NAME) (translate only the caption), [TOC], ':::' lines (translate only the title after the keyword), code blocks, \
         identifiers, names of systems, products and model elements, and the lines 'NAME:' / 'DESCRIPTION:' keys if present (translate their values).",
        to.english_name()
    );
    let response = client.send_value(&system, &[json!({ "role": "user", "content": text })], None)?;
    let out: String = response["content"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|b| b["text"].as_str())
        .collect::<Vec<_>>()
        .join("");
    if out.trim().is_empty() {
        bail!("{}", crate::i18n::tr("Claude returned an empty translation"));
    }
    Ok(out.trim().to_string())
}

/// Performs the move. Returns a description of the result; error = no connection to Claude or another problem (the task stays pending).
pub fn run(cfg: &AppConfig, props: &Path, from: Lang, to: Lang) -> Result<String> {
    // Default templates without AI (always, even without a connection).
    let custom_templates = templates::switch_defaults(&cfg.templates_dir, from, to);
    let entries = memory::list(&cfg.memory_dir);
    if custom_templates.is_empty() && entries.is_empty() {
        clear_pending(props)?;
        return Ok(crate::i18n::tr("Done: the templates are now in the new language; memory was empty.").to_string());
    }
    let key = secrets::get(secrets::CLAUDE_API_KEY).context(crate::i18n::tr("no Claude API key"))?;
    let mut templates_done = 0;
    for path in &custom_templates {
        let text = std::fs::read_to_string(path).with_context(|| crate::i18n::fmt("reading {0}", &[path.display().to_string()]))?;
        let name = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        let translated = translate(cfg, &key, &format!("NAME: {name}\n\n{text}"), to)?;
        let (new_name, body) = match translated.split_once('\n') {
            Some((first, rest)) if first.trim_start().to_uppercase().starts_with("NAME:") => {
                (first.trim_start()[5..].trim().to_string(), rest.trim_start().to_string())
            }
            _ => (name.clone(), translated.clone()),
        };
        let new_name = if new_name.is_empty() { name.clone() } else { new_name };
        templates::save(&cfg.templates_dir, &new_name, &body)?;
        if new_name != name {
            let _ = std::fs::remove_file(path);
        }
        templates_done += 1;
    }
    let mut memory_done = 0;
    for entry in &entries {
        let source = format!("NAME: {}\nDESCRIPTION: {}\n\n{}", entry.name, entry.description, entry.content);
        let translated = translate(cfg, &key, &source, to)?;
        let mut name = entry.name.clone();
        let mut description = entry.description.clone();
        let mut body = Vec::new();
        let mut header = true;
        for line in translated.lines() {
            let upper = line.trim_start().to_uppercase();
            if header && upper.starts_with("NAME:") {
                name = line.trim_start()[5..].trim().to_string();
            } else if header && upper.starts_with("DESCRIPTION:") {
                description = line.trim_start()[12..].trim().to_string();
            } else {
                if !line.trim().is_empty() {
                    header = false;
                }
                if !header {
                    body.push(line);
                }
            }
        }
        let content = body.join("\n");
        memory::save(&cfg.memory_dir, &name, &description, content.trim())?;
        if memory::slug(&name) != memory::slug(&entry.name) {
            let _ = memory::delete(&cfg.memory_dir, &entry.name);
        }
        memory_done += 1;
    }
    clear_pending(props)?;
    Ok(crate::i18n::fmt("Done: translated templates ({0}) and memory notes ({1}).", &[templates_done.to_string(), memory_done.to_string()]))
}
